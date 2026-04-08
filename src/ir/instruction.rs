use crate::ir::ir_types::{Instruction, Operand, RegisterId};
use crate::ir::opcode_registry::Opcode;

pub trait InstructionExt {
    fn new(opcode: Opcode, a: Operand, b: Operand, c: Operand) -> Instruction;
    fn unary(opcode: Opcode, dst: RegisterId, source: Operand) -> Instruction;
    fn binary(opcode: Opcode, dst: RegisterId, left: Operand, right: Operand) -> Instruction;
}

impl InstructionExt for Instruction {
    fn new(opcode: Opcode, a: Operand, b: Operand, c: Operand) -> Instruction {
        Instruction { opcode, a, b, c }
    }

    fn unary(opcode: Opcode, dst: RegisterId, source: Operand) -> Instruction {
        Instruction {
            opcode,
            a: Operand::Register(dst),
            b: source,
            c: Operand::None,
        }
    }

    fn binary(opcode: Opcode, dst: RegisterId, left: Operand, right: Operand) -> Instruction {
        Instruction {
            opcode,
            a: Operand::Register(dst),
            b: left,
            c: right,
        }
    }
}
