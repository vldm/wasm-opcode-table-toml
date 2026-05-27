#!/usr/bin/env rust-script
//! Compare `instructions.toml` against a Spectec binary-instructions grammar.
//!
//! ```cargo
//! [dependencies]
//! wasm-opcode-table = { path = ".." }
//! nom = "7"
//! ```
//!
//! Usage:
//!   ./script/diff-spectec.rs <spectec-file> [instructions.toml]
//!
//! Requires [rust-script](https://github.com/fornwall/rust-script): `cargo install rust-script`

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

use nom::{
    bytes::complete::{tag, tag_no_case},
    character::complete::{hex_digit1, multispace0, space0},
    combinator::{opt, rest},
    sequence::preceded,
    IResult, Parser,
};
use wasm_opcode_table::{parse_instructions_toml, InstructionsTable, Opcode};

fn main() {
    if let Err(e) = run() {
        eprintln!("diff-spectec: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let spectec_path = args.next().map(PathBuf::from).ok_or_else(|| usage())?;

    let instructions_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_instructions_path);

    if args.next().is_some() {
        return Err(usage());
    }

    let instructions_src = fs::read_to_string(&instructions_path).map_err(|e| {
        format!(
            "failed to read instructions {}: {e}",
            instructions_path.display()
        )
    })?;
    let spectec_src = fs::read_to_string(&spectec_path)
        .map_err(|e| format!("failed to read spectec {}: {e}", spectec_path.display()))?;

    let table = parse_instructions_toml(&instructions_src)
        .map_err(|e| format!("failed to parse {}: {e}", instructions_path.display()))?;

    let rules = parse_spectec(&spectec_src);
    let mut missing = missing_opcodes(&table, &rules);
    missing.sort_by(|a, b| a.opcode.cmp(&b.opcode).then_with(|| a.ast.cmp(&b.ast)));

    eprintln!(
        "instructions: {} ({} rows)",
        instructions_path.display(),
        table.instructions.len()
    );
    eprintln!(
        "spectec:      {} ({} rules)",
        spectec_path.display(),
        rules.len()
    );

    let mut table_out = String::new();
    print_missing_table(&mut table_out, &missing).map_err(|e| e.to_string())?;
    print!("{table_out}");
    io::stdout().flush().map_err(|e| e.to_string())?;

    if !missing.is_empty() {
        process::exit(2);
    }

    Ok(())
}

fn default_instructions_path() -> PathBuf {
    PathBuf::from("instructions.toml")
}

fn usage() -> String {
    "usage: diff-spectec <spectec-file> [instructions.toml]\n\
     \n\
     Compare opcode keys in instructions.toml against Spectec grammar rules.\n\
     Prints a table of missing instructions; exits 2 when any are missing."
        .to_string()
}

// --- Spectec parser (nom) ---

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpectecRule {
    opcode: Opcode,
    ast: String,
    section: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MissingOpcode {
    opcode: Opcode,
    ast: String,
    section: String,
}

/// Leading `0xNN` byte in a Spectec rule left-hand side.
fn hex_byte(input: &str) -> IResult<&str, u8> {
    let (input, digits) = preceded(tag_no_case("0x"), hex_digit1).parse(input)?;
    let value = u8::from_str_radix(digits, 16).map_err(|_| {
        nom::Err::Failure(nom::error::make_error(
            input,
            nom::error::ErrorKind::HexDigit,
        ))
    })?;
    Ok((input, value))
}

/// `N:Bu32` opcode index suffix (decimal `N`).
fn bu32_index(input: &str) -> IResult<&str, u32> {
    let (input, digits) = nom::character::complete::digit1.parse(input)?;
    let (input, _) = tag(":Bu32").parse(input)?;
    let value = digits.parse().map_err(|_| {
        nom::Err::Failure(nom::error::make_error(input, nom::error::ErrorKind::Digit))
    })?;
    Ok((input, value))
}

/// Opcode key: one `0xNN` byte, optionally followed by `N:Bu32`.
fn opcode_key(input: &str) -> IResult<&str, Opcode> {
    let (input, byte) = preceded(multispace0, hex_byte).parse(input)?;
    let (input, index) = opt(preceded(space0, bu32_index)).parse(input)?;
    Ok((
        input,
        match index {
            Some(i) => Opcode::Multi(byte, i),
            None => Opcode::Single(byte),
        },
    ))
}

/// Split rule content at `=>` into left-hand encoding and AST name.
fn rule_arrow(input: &str) -> IResult<&str, (&str, &str)> {
    let (input, left) = nom::bytes::complete::take_until("=>").parse(input)?;
    let (input, _) = tag("=>").parse(input)?;
    let (input, ast) = rest.parse(input)?;
    Ok((input, (left.trim(), ast.trim())))
}

fn parse_section_line(line: &str) -> Option<String> {
    let title = line.trim().strip_prefix(";;")?.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_lowercase())
    }
}

fn parse_spectec(source: &str) -> Vec<SpectecRule> {
    let mut section = String::from("unknown");
    let mut rules = BTreeMap::<Opcode, SpectecRule>::new();
    let mut pending_left: Option<String> = None;

    for line in source.lines() {
        if let Some(sec) = parse_section_line(line) {
            section = sec;
            continue;
        }

        if line.trim() == "| ..." {
            continue;
        }

        if line.contains("=>") {
            let content = rule_content(line, pending_left.take());
            pending_left = None;
            if let Ok((_, (left, ast))) = rule_arrow.parse(content.as_str()) {
                if !ast.is_empty() {
                    if let Ok((_, opcode)) = opcode_key.parse(left) {
                        rules.insert(
                            opcode,
                            SpectecRule {
                                opcode,
                                ast: ast.to_string(),
                                section: section.clone(),
                            },
                        );
                    }
                }
            }
            continue;
        }

        let trimmed = line.trim();
        if trimmed.starts_with('|') {
            let fragment = line.split('|').nth(1).map(str::trim).unwrap_or("");
            if !fragment.is_empty() {
                pending_left = Some(match pending_left.take() {
                    Some(prev) => format!("{prev} {fragment}"),
                    None => fragment.to_string(),
                });
            }
        } else if pending_left.is_some() && !trimmed.is_empty() && !trimmed.starts_with("grammar") {
            pending_left = Some(format!("{} {}", pending_left.take().unwrap(), trimmed));
        }
    }

    rules.into_values().collect()
}

fn rule_content(line: &str, pending: Option<String>) -> String {
    if let Some(pipe) = line.find('|') {
        return line[pipe + 1..].trim().to_string();
    }
    if let Some(prev) = pending {
        return format!("{prev} {}", line.trim());
    }
    line.trim().to_string()
}

fn missing_opcodes(table: &InstructionsTable, spectec: &[SpectecRule]) -> Vec<MissingOpcode> {
    let present: BTreeSet<Opcode> = table.instructions.iter().map(|i| i.opcode).collect();

    spectec
        .iter()
        .filter(|rule| !present.contains(&rule.opcode))
        .map(|rule| MissingOpcode {
            opcode: rule.opcode,
            ast: rule.ast.clone(),
            section: rule.section.clone(),
        })
        .collect()
}

fn format_opcode(opcode: Opcode) -> String {
    match opcode {
        Opcode::Single(b) => format!("0x{b:02X}"),
        Opcode::Multi(prefix, index) => format!("[0x{prefix:02X}, {index}]"),
    }
}

fn print_missing_table(w: &mut impl fmt::Write, missing: &[MissingOpcode]) -> fmt::Result {
    if missing.is_empty() {
        writeln!(
            w,
            "No missing instructions (all Spectec opcodes are in the table)."
        )?;
        return Ok(());
    }

    let op_col = missing
        .iter()
        .map(|m| format_opcode(m.opcode).len())
        .max()
        .unwrap_or(6)
        .max(6);
    let ast_col = missing
        .iter()
        .map(|m| m.ast.len())
        .max()
        .unwrap_or(3)
        .max(3);

    writeln!(
        w,
        "{:<op_col$}  {:<ast_col$}  {}",
        "opcode", "spectec", "section"
    )?;
    writeln!(w, "{}", "-".repeat(op_col + ast_col + 12))?;

    for m in missing {
        writeln!(
            w,
            "{:<op_col$}  {:<ast_col$}  {}",
            format_opcode(m.opcode),
            m.ast,
            m.section
        )?;
    }

    writeln!(w)?;
    writeln!(w, "{} missing instruction(s).", missing.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
;; Control instructions

grammar Binstr/control : instr = ...
  | 0x08 x:Btagidx => THROW x
  | 0x10 x:Bfuncidx => CALL x
  | 0xFB 24:Bu32 (null_1?, null_2?):Bcastop
    l:Blabelidx ht_1:Bheaptype ht_2:Bheaptype => BR_ON_CAST l (REF null_1? ht_1) (REF null_2? ht_2)
  | 0xFC 10:Bu32 x_1:Bmemidx x_2:Bmemidx => MEMORY.COPY x_1 x_2
"#;

    #[test]
    fn hex_byte_parses() {
        assert_eq!(hex_byte("0x08 x").unwrap().1, 0x08);
        assert_eq!(hex_byte("0xFB ").unwrap().1, 0xFB);
    }

    #[test]
    fn bu32_index_parses() {
        assert_eq!(bu32_index("24:Bu32 ").unwrap().1, 24);
        assert_eq!(bu32_index("256:Bu32").unwrap().1, 256);
    }

    #[test]
    fn opcode_key_parses() {
        assert_eq!(
            opcode_key("0x08 x:Btagidx").unwrap().1,
            Opcode::Single(0x08)
        );
        assert_eq!(
            opcode_key("0xFC 10:Bu32 x_1").unwrap().1,
            Opcode::Multi(0xFC, 10)
        );
        assert_eq!(
            opcode_key("0xFB 24:Bu32 (null_1?)").unwrap().1,
            Opcode::Multi(0xFB, 24)
        );
    }

    #[test]
    fn rule_arrow_parses() {
        let (_, (left, ast)) = rule_arrow("0x08 x:Btagidx => THROW x").unwrap();
        assert_eq!(left, "0x08 x:Btagidx");
        assert_eq!(ast, "THROW x");
    }

    #[test]
    fn parse_spectec_sample() {
        let rules = parse_spectec(SAMPLE);
        assert!(rules.iter().any(|r| r.opcode == Opcode::Single(0x08)));
        assert!(rules.iter().any(|r| r.opcode == Opcode::Multi(0xFB, 24)));
        assert!(rules.iter().any(|r| r.opcode == Opcode::Multi(0xFC, 10)));
    }
}
