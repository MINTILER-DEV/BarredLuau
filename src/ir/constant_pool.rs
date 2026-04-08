use crate::ir::ir_types::{ConstantId, ConstantPool, ConstantValue};

pub trait ConstantPoolExt {
    fn intern(&mut self, value: ConstantValue) -> ConstantId;
}

impl ConstantPoolExt for ConstantPool {
    fn intern(&mut self, value: ConstantValue) -> ConstantId {
        if let Some(index) = self.values.iter().position(|existing| existing == &value) {
            return ConstantId(index as u32);
        }
        let index = self.values.len();
        self.values.push(value);
        ConstantId(index as u32)
    }
}
