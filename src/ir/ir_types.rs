use crate::ir::opcode_registry::Opcode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegisterId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrototypeId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantValue {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ConstantPool {
    pub values: Vec<ConstantValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    None,
    Register(RegisterId),
    Constant(ConstantId),
    Immediate(i32),
    Prototype(PrototypeId),
    Upvalue(u16),
    Boolean(bool),
}

impl Default for Operand {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub a: Operand,
    pub b: Operand,
    pub c: Operand,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FunctionProto {
    pub name: Option<String>,
    pub parameters: Vec<String>,
    pub is_vararg: bool,
    pub vararg_register: Option<u16>,
    pub max_registers: u16,
    pub upvalues: Vec<String>,
    pub constants: ConstantPool,
    pub instructions: Vec<Instruction>,
    pub child_prototypes: Vec<PrototypeId>,
    pub local_names: Vec<Option<String>>,
    pub return_arity: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramBlob {
    pub magic: [u8; 4],
    pub version: u16,
    pub feature_flags: u32,
    pub entry_prototype: PrototypeId,
    pub prototypes: Vec<FunctionProto>,
    pub checksum: u32,
}

impl Default for ProgramBlob {
    fn default() -> Self {
        Self {
            magic: *b"BRLU",
            version: 1,
            feature_flags: 0,
            entry_prototype: PrototypeId(0),
            prototypes: Vec::new(),
            checksum: 0,
        }
    }
}
