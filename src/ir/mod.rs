pub mod constant_pool;
pub mod function_proto;
pub mod instruction;
pub mod ir_types;
pub mod opcode_registry;
pub mod register_allocator;

pub use constant_pool::ConstantPoolExt;
pub use function_proto::FunctionProtoExt;
pub use instruction::InstructionExt;
pub use ir_types::{
    ConstantId, ConstantPool, ConstantValue, FunctionProto, Instruction, Operand, ProgramBlob,
    PrototypeId, RegisterId,
};
pub use opcode_registry::{Opcode, OpcodeRegistry};
pub use register_allocator::RegisterAllocator;
