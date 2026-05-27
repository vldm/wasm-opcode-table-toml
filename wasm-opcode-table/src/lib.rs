pub mod instructions;

pub use instructions::{
    parse_instructions_toml, validate_instructions_table, validate_stack_entry, ControlFrame,
    ControlKind, Immediate, Instruction, InstructionsTable, LabelTarget, Opcode, StackEntry,
    StackType, TypeExpr, ValidateError,
};

#[cfg(feature = "instructions-toml")]
pub use instructions::{instructions, INSTRUCTIONS_TOML};

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../instructions.toml"
    ));

    #[test]
    fn table_parses() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        assert_eq!(table.instructions.len(), 437);
        assert_eq!(table.instructions[0].name, "unreachable");
        assert_eq!(table.instructions[0].opcode, Opcode::Single(0));
        validate_instructions_table(&table).unwrap();
    }

    #[test]
    fn multi_byte_opcode_and_immediates() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        let fill = table
            .instructions
            .iter()
            .find(|i| i.name == "table.fill")
            .expect("table.fill");
        assert_eq!(fill.opcode, Opcode::Multi(vec![0xFC, 17]));
        let imms = fill.immediates.as_ref().expect("immediates");
        assert_eq!(imms[0].ty, "tableidx");
        assert_eq!(imms[0].name.as_deref(), Some("x"));
    }

    #[test]
    fn control_stack_entry() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        let block = table
            .instructions
            .iter()
            .find(|i| i.name == "block")
            .expect("block");
        let stack = block.stack_type.as_ref().expect("stack-type");
        match &stack.to[0] {
            StackEntry::Control(cf) => {
                assert_eq!(cf.control, ControlKind::Block);
                assert_eq!(cf.start.0, "params(bt)");
                assert_eq!(cf.end.0, "results(bt)");
                assert_eq!(cf.label, LabelTarget::End);
            }
            _ => panic!("expected control frame"),
        }
    }

    #[test]
    fn types_of_vs_type_of() {
        let table = parse_instructions_toml(SAMPLE).unwrap();

        let block = table
            .instructions
            .iter()
            .find(|i| i.name == "block")
            .expect("block");
        match &block.stack_type.as_ref().unwrap().from[0] {
            StackEntry::TypesOf(expr) => assert_eq!(expr.0, "params(bt)"),
            _ => panic!("expected types-of"),
        }

        let local_get = table
            .instructions
            .iter()
            .find(|i| i.name == "local.get")
            .expect("local.get");
        match &local_get.stack_type.as_ref().unwrap().to[0] {
            StackEntry::TypeOf(expr) => assert_eq!(expr.0, "x"),
            _ => panic!("expected type-of"),
        }
    }

    #[test]
    fn loop_label_is_start() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        let loop_insn = table
            .instructions
            .iter()
            .find(|i| i.name == "loop")
            .expect("loop");
        match &loop_insn.stack_type.as_ref().unwrap().to[0] {
            StackEntry::Control(cf) => assert_eq!(cf.label, LabelTarget::Start),
            _ => panic!("expected control frame"),
        }
    }
}

#[cfg(all(test, feature = "instructions-toml"))]
mod embedded_tests {
    use super::*;

    #[test]
    fn embedded_entrypoint() {
        let table = instructions();
        assert_eq!(table.instructions.len(), 437);
        assert!(!INSTRUCTIONS_TOML.is_empty());
        validate_instructions_table(table).unwrap();
    }
}
