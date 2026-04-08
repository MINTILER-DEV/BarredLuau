use crate::ir::ir_types::{FunctionProto, Instruction};

pub trait FunctionProtoExt {
    fn push_instruction(&mut self, instruction: Instruction) -> usize;
    fn set_local_name(&mut self, register: u16, name: Option<String>);
}

impl FunctionProtoExt for FunctionProto {
    fn push_instruction(&mut self, instruction: Instruction) -> usize {
        self.instructions.push(instruction);
        self.instructions.len() - 1
    }

    fn set_local_name(&mut self, register: u16, name: Option<String>) {
        let index = register as usize;
        if self.local_names.len() <= index {
            self.local_names.resize(index + 1, None);
        }
        self.local_names[index] = name;
    }
}
