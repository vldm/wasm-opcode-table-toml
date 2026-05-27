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

    const RELAXED_SIMD_NAMES: &[&str] = &[
        "i8x16.relaxed_swizzle",
        "i32x4.relaxed_trunc_f32x4_s",
        "i32x4.relaxed_trunc_f32x4_u",
        "i32x4.relaxed_trunc_f64x2_s_zero",
        "i32x4.relaxed_trunc_f64x2_u_zero",
        "f32x4.relaxed_madd",
        "f32x4.relaxed_nmadd",
        "f64x2.relaxed_madd",
        "f64x2.relaxed_nmadd",
        "i8x16.relaxed_laneselect",
        "i16x8.relaxed_laneselect",
        "i32x4.relaxed_laneselect",
        "i64x2.relaxed_laneselect",
        "f32x4.relaxed_min",
        "f32x4.relaxed_max",
        "f64x2.relaxed_min",
        "f64x2.relaxed_max",
        "i16x8.relaxed_q15mulr_s",
        "i16x8.relaxed_dot_i8x16_i7x16_s",
        "i32x4.relaxed_dot_i8x16_i7x16_add_s",
    ];

    const SAMPLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../instructions.toml"
    ));

    #[test]
    fn table_parses() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        assert_eq!(table.instructions.len(), 457);
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
        assert_eq!(fill.opcode, Opcode::Multi(0xFC, 17));
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
    fn relaxed_simd_instructions() {
        let table = parse_instructions_toml(SAMPLE).unwrap();
        let relaxed: Vec<_> = table
            .instructions
            .iter()
            .filter(|i| i.feature.as_deref() == Some("relaxed-simd"))
            .collect();
        assert_eq!(relaxed.len(), 20);

        for (idx, name) in RELAXED_SIMD_NAMES.iter().enumerate() {
            let insn = table
                .instructions
                .iter()
                .find(|i| i.name == *name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(insn.opcode, Opcode::Multi(0xFD, idx as u32 + 256));
            assert_eq!(insn.category, "vector");
            assert_eq!(insn.feature.as_deref(), Some("relaxed-simd"));
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
        assert_eq!(table.instructions.len(), 457);
        assert!(!INSTRUCTIONS_TOML.is_empty());
        validate_instructions_table(table).unwrap();
    }
}
