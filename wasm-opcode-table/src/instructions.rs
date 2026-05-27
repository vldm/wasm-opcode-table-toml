//! Schema for [`instructions.toml`](../../instructions.toml).

use serde::{Deserialize, Deserializer};

/// Root document: `[[instructions]]` array-of-tables.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstructionsTable {
    pub instructions: Vec<Instruction>,
}

/// A single WebAssembly instruction / opcode entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Instruction {
    pub name: String,
    #[serde(default)]
    pub variant: Option<String>,
    pub opcode: Opcode,
    pub category: String,
    #[serde(default)]
    pub immediates: Option<Vec<Immediate>>,
    #[serde(default, rename = "stack-type")]
    pub stack_type: Option<StackType>,
    #[serde(default)]
    pub feature: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
}

/// Opcode as written in TOML: an integer (`0x6A`) or a two-element array (`[0xFC, 17]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    /// `opcode = 0xNN`
    Single(u8),
    /// `opcode = [prefix, index]` — prefix byte and opcode index.
    Multi(u8, u32),
}

impl<'de> Deserialize<'de> for Opcode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_helpers::deserialize_opcode(deserializer)
    }
}

/// Immediate operand in the instruction encoding.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Immediate {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "binary-order")]
    pub binary_order: Option<u64>,
}

/// Stack signature (`stack-type.from` / `stack-type.to`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StackType {
    pub from: Vec<StackEntry>,
    pub to: Vec<StackEntry>,
}

/// Symbolic or concrete type expression in a stack slot descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TypeExpr(pub String);

/// Control construct pushed on the control stack (`block` / `loop` / `if`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ControlFrame {
    pub control: ControlKind,
    pub start: TypeExpr,
    pub end: TypeExpr,
    pub label: LabelTarget,
}

/// WebAssembly control instruction kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    Block,
    Loop,
    If,
}

/// Branch target for a control frame: break (`end`) or continue (`start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelTarget {
    /// Branch exits the construct (break) — `label = "end"` in TOML.
    End,
    /// Branch targets the head of a loop (continue) — `label = "start"` in TOML.
    Start,
}

/// One stack slot descriptor in `stack-type.from` / `stack-type.to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackEntry {
    Type(TypeExpr),
    TypeOf(TypeExpr),
    TypesOf(TypeExpr),
    Unreachable,
    Control(ControlFrame),
}

impl<'de> Deserialize<'de> for StackEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_helpers::deserialize_stack_entry(deserializer)
    }
}

mod serde_helpers {
    use super::{ControlFrame, Opcode, StackEntry, TypeExpr};
    use serde::de;
    use serde::{Deserialize, Deserializer};

    pub(super) fn deserialize_opcode<'de, D>(deserializer: D) -> Result<Opcode, D::Error>
    where
        D: Deserializer<'de>,
    {
        OpcodeRaw::deserialize(deserializer).and_then(|raw| {
            raw.try_into()
                .map_err(|e: OpcodeTomlError| de::Error::custom(e))
        })
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OpcodeRaw {
        Single(u64),
        Multi(Vec<u64>),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum OpcodeTomlError {
        SingleOutOfRange(u64),
        MultiWrongLength(usize),
        MultiOutOfRange { prefix: u64, index: u64 },
    }

    impl std::fmt::Display for OpcodeTomlError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::SingleOutOfRange(v) => write!(f, "single opcode {v} does not fit in u8"),
                Self::MultiWrongLength(len) => {
                    write!(f, "multi opcode must have exactly 2 elements, got {len}")
                }
                Self::MultiOutOfRange { prefix, index } => write!(
                    f,
                    "multi opcode [{prefix}, {index}] does not fit in (u8, u32)"
                ),
            }
        }
    }

    impl TryFrom<OpcodeRaw> for Opcode {
        type Error = OpcodeTomlError;

        fn try_from(raw: OpcodeRaw) -> Result<Self, Self::Error> {
            match raw {
                OpcodeRaw::Single(value) => {
                    let byte = u8::try_from(value).map_err(|_| OpcodeTomlError::SingleOutOfRange(value))?;
                    Ok(Self::Single(byte))
                }
                OpcodeRaw::Multi(values) => {
                    let [prefix, index] = values
                        .try_into()
                        .map_err(|v: Vec<u64>| OpcodeTomlError::MultiWrongLength(v.len()))?;
                    let prefix = u8::try_from(prefix).map_err(|_| OpcodeTomlError::MultiOutOfRange {
                        prefix,
                        index,
                    })?;
                    let index = u32::try_from(index).map_err(|_| OpcodeTomlError::MultiOutOfRange {
                        prefix: u64::from(prefix),
                        index,
                    })?;
                    Ok(Self::Multi(prefix, index))
                }
            }
        }
    }

    pub(super) fn deserialize_stack_entry<'de, D>(deserializer: D) -> Result<StackEntry, D::Error>
    where
        D: Deserializer<'de>,
    {
        StackEntryRaw::deserialize(deserializer).map(Into::into)
    }

    /// Variant order matches most-specific-first for untagged matching.
    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    #[serde(untagged)]
    enum StackEntryRaw {
        Control(ControlFrame),
        Unreachable(UnreachableEntry),
        TypeOf(TypeOfEntry),
        TypesOf(TypesOfEntry),
        Type(TypeEntry),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    struct TypeEntry {
        #[serde(rename = "type")]
        ty: TypeExpr,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    struct TypeOfEntry {
        #[serde(rename = "type-of")]
        expr: TypeExpr,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    struct TypesOfEntry {
        #[serde(rename = "types-of")]
        expr: TypeExpr,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    struct UnreachableEntry {
        #[serde(
            rename = "unreachable",
            deserialize_with = "deserialize_unreachable_true"
        )]
        _marker: (),
    }

    impl From<StackEntryRaw> for StackEntry {
        fn from(raw: StackEntryRaw) -> Self {
            match raw {
                StackEntryRaw::Control(frame) => StackEntry::Control(frame),
                StackEntryRaw::Unreachable(_) => StackEntry::Unreachable,
                StackEntryRaw::TypeOf(e) => StackEntry::TypeOf(e.expr),
                StackEntryRaw::TypesOf(e) => StackEntry::TypesOf(e.expr),
                StackEntryRaw::Type(e) => StackEntry::Type(e.ty),
            }
        }
    }

    fn deserialize_unreachable_true<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Ok(())
        } else {
            Err(de::Error::custom("unreachable must be true"))
        }
    }
}

/// Validation error for stack-type invariants not expressible in the type system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidateError {
    ControlLabelMismatch {
        control: ControlKind,
        label: LabelTarget,
    },
}

/// Check control-frame label invariants (`loop` → `Start`, `block`/`if` → `End`).
///
/// # Errors
///
/// Returns [`ValidateError::ControlLabelMismatch`] when a control frame's `label`
/// does not match its `control` kind.
pub fn validate_stack_entry(entry: &StackEntry) -> Result<(), ValidateError> {
    let StackEntry::Control(frame) = entry else {
        return Ok(());
    };
    let ok = matches!(
        (frame.control, frame.label),
        (ControlKind::Loop, LabelTarget::Start)
            | (ControlKind::Block | ControlKind::If, LabelTarget::End)
    );
    if ok {
        Ok(())
    } else {
        Err(ValidateError::ControlLabelMismatch {
            control: frame.control,
            label: frame.label,
        })
    }
}

/// Validate every stack entry in the table.
///
/// # Errors
///
/// Returns the first [`ValidateError`] produced by [`validate_stack_entry`].
pub fn validate_instructions_table(table: &InstructionsTable) -> Result<(), ValidateError> {
    for instruction in &table.instructions {
        if let Some(stack) = &instruction.stack_type {
            for entry in stack.from.iter().chain(&stack.to) {
                validate_stack_entry(entry)?;
            }
        }
    }
    Ok(())
}

/// Parse TOML source into an [`InstructionsTable`].
///
/// # Errors
///
/// Returns a TOML deserialization error when `source` is invalid or does not match the schema.
pub fn parse_instructions_toml(source: &str) -> Result<InstructionsTable, toml::de::Error> {
    toml::from_str(source)
}

#[cfg(feature = "instructions-toml")]
mod embedded {
    use std::sync::OnceLock;

    use super::{InstructionsTable, parse_instructions_toml};

    /// Raw TOML embedded at compile time from the repo-root `instructions.toml`.
    pub const INSTRUCTIONS_TOML: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../instructions.toml"));

    static PARSED: OnceLock<InstructionsTable> = OnceLock::new();

    /// Lazily parsed embedded instruction table.
    ///
    /// # Panics
    ///
    /// Panics if the embedded `instructions.toml` does not match the schema. The embedded
    /// file is validated at build time by design and should always parse successfully.
    pub fn instructions() -> &'static InstructionsTable {
        PARSED.get_or_init(|| {
            parse_instructions_toml(INSTRUCTIONS_TOML)
                .expect("embedded instructions.toml must match schema")
        })
    }
}

#[cfg(feature = "instructions-toml")]
pub use embedded::{INSTRUCTIONS_TOML, instructions};
