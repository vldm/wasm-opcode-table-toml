# WebAssembly Opcode Table (TOML)

This repository contains a structured reference of the WebAssembly instruction set and its opcodes, formatted as a [TOML](https://toml.io) file.


## Overview

The [instructions.toml](./instructions.toml) file lists all instructions as an array-of-tables under the key `[[instructions]]`.

Each entry represents a single instruction (or more precisely, a single opcode) and includes details such as its name, category, binary opcode, immediate arguments, and stack signature.

| Key          | Type              | Value                                                                     |
| ------------ | ----------------- | ------------------------------------------------------------------------- |
| `name`       | string            | The name of the instruction.                                              |
| `variant`    | string (optional) | A unique identifier for a specific variant of the instruction, if needed. |
| `opcode`     | number or array   | The binary opcode for the instruction.                                    |
| `category`   | string            | The category of the instruction as defined in the spec.                   |
| `immediates` | array (optional)  | The immediate arguments, if any.                                          |
| `stack-type` | table (optional)  | The instruction type, describing stack input and output types.            |
| `feature`    | string (optional) | The feature proposal to which the instruction belongs, if any.            |
| `since`      | string (optional) | The first major version of WebAssembly that includes the instruction.     |

## Examples

```toml
[[instructions]]
name = "i32.add"
opcode = 0x6A
category = "numeric"
stack-type = { from = [ { type = "i32" }, { type = "i32" } ], to = [ { type = "i32" } ] }
since = "1"
```

```toml
[[instructions]]
name = "table.fill"
opcode = [0xFC, 17]
category = "table"
immediates = [ { type = "tableidx", name = "x" } ]
stack-type = { from = [ { type = "i32" }, { type-of = "x" }, { type = "i32" } ], to = [] }
feature = "reference-types"
since = "2"
```

### Source of data
Original source of data is [ohorn/wasm-opcode-table-toml](https://github.com/ohorn/wasm-opcode-table-toml).
Later updated using LLM with spec from [WebAssembly Spec](https://github.com/WebAssembly/spec).

## Rust crate (`wasm-opcode-table`)

This repository is also the [`wasm-opcode-table`](https://crates.io/crates/wasm-opcode-table) crate — a typed schema and parser for `instructions.toml`. Use it in tools, validators, or codegen without hand-maintaining opcode tables.

**Always available:**

- Structs for instructions, opcodes, immediates, and stack signatures (`StackEntry`, `ControlFrame`, …)
- [`parse_instructions_toml`](https://docs.rs/wasm-opcode-table/latest/wasm_opcode_table/fn.parse_instructions_toml.html) to parse any TOML string
- [`validate_instructions_table`](https://docs.rs/wasm-opcode-table/latest/wasm_opcode_table/fn.validate_instructions_table.html) for control-frame invariants

**Optional feature `instructions-toml`:** embeds `instructions.toml` from the crate package at compile time via `include_str!` and exposes [`instructions()`](https://docs.rs/wasm-opcode-table/latest/wasm_opcode_table/fn.instructions.html) for a lazily parsed `&'static InstructionsTable`.

## Usage


```toml
[dependencies]
wasm-opcode-table = { version = "0.1", features = ["instructions-toml"] }
```

```rust
use std::fs;
use wasm_opcode_table::{instructions, parse_instructions_toml, Instruction, StackEntry};

// Load and parse from a file (no embed feature required)
let source = fs::read_to_string("instructions.toml")?;
let table = parse_instructions_toml(&source)?;

// Or use the bundled table (requires `instructions-toml`)
let table = instructions();
let add = table.instructions.iter().find(|i| i.name == "i32.add").unwrap();
```


# Contributing 

Feel free to create an issue if you find any mistake, or didn't find an instruction you expected.
Or create a pull request directly if you want to fix something.

Please avoid storing editors/llms configs in the repository.

## Finding missing instructions (`script/diff-spectec.rs`)

The [rust-script](https://github.com/fornwall/rust-script) in [`script/diff-spectec.rs`](./script/diff-spectec.rs) compares opcode keys in [`instructions.toml`](./instructions.toml) against a Spectec grammar file from the [WebAssembly spec](https://github.com/WebAssembly/spec/blob/main/specification/wasm-latest/5.3-binary.instructions.spectec). Use it to see which rows still need to be added to the table.

```bash
# One-time: install rust-script
cargo install rust-script

# Fetch the latest binary-instructions grammar (optional)
curl -sL -o spec.instructions.spectec \
  https://raw.githubusercontent.com/WebAssembly/spec/main/specification/wasm-latest/5.3-binary.instructions.spectec

# Compare against the bundled instructions.toml (default second argument)
./script/diff-spectec.rs spec.instructions.spectec

# Or pass a custom instructions file
./script/diff-spectec.rs spec.instructions.spectec path/to/instructions.toml
```

Example output (truncated):

```
opcode          spectec                              section
--------------  -----------------------------------  ----------------
0x08            THROW x                              control instructions
[0xFB, 24]      BR_ON_CAST l (REF null_1? ht_1) ...  control instructions

42 missing instruction(s).
```

Exit code `2` means there are missing opcodes; `0` means the table covers every rule in the Spectec file.

## Tests
Run tests from the repository root: `cargo test --features instructions-toml`.
