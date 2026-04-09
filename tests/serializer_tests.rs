use barred_luau::ir::{
    ConstantPool, ConstantValue, FunctionProto, Instruction, Opcode, OpcodeRegistry, Operand,
    ProgramBlob, PrototypeId, RegisterId,
};
use barred_luau::serializer::{BlobReader, BlobWriter};

#[test]
fn serializer_roundtrips_program_blob() {
    let registry = OpcodeRegistry::sequential();
    let program = ProgramBlob {
        feature_flags: 0xA5,
        entry_prototype: PrototypeId(0),
        prototypes: vec![FunctionProto {
            name: Some("main".to_string()),
            parameters: vec!["arg".to_string()],
            is_vararg: false,
            vararg_register: None,
            max_registers: 2,
            upvalues: vec!["captured".to_string()],
            constants: ConstantPool {
                values: vec![
                    ConstantValue::Number(4.0),
                    ConstantValue::String("hi".to_string()),
                ],
            },
            instructions: vec![
                Instruction {
                    opcode: Opcode::LoadNumber,
                    a: Operand::Register(RegisterId(0)),
                    b: Operand::Constant(barred_luau::ir::ConstantId(0)),
                    c: Operand::None,
                },
                Instruction {
                    opcode: Opcode::Return,
                    a: Operand::Register(RegisterId(0)),
                    b: Operand::Immediate(1),
                    c: Operand::None,
                },
            ],
            child_prototypes: vec![],
            local_names: vec![Some("arg".to_string()), Some("tmp".to_string())],
            return_arity: 1,
        }],
        ..ProgramBlob::default()
    };

    let bytes = BlobWriter::serialize_program(&program, &registry).expect("serialize");
    let decoded = BlobReader::deserialize_program(&bytes, &registry).expect("deserialize");
    assert_eq!(decoded.feature_flags, program.feature_flags);
    assert_eq!(decoded.prototypes, program.prototypes);
}
