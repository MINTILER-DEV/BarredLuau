use std::collections::BTreeMap;

use crate::error::CompileError;
use crate::ir::{FunctionProto, Operand};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label(pub usize);

#[derive(Clone, Copy, Debug)]
enum Slot {
    B,
}

#[derive(Clone, Debug, Default)]
pub struct LabelResolver {
    next_label: usize,
    positions: BTreeMap<Label, usize>,
    patches: Vec<(usize, Label, Slot)>,
}

impl LabelResolver {
    pub fn new_label(&mut self) -> Label {
        let label = Label(self.next_label);
        self.next_label += 1;
        label
    }

    pub fn mark(&mut self, label: Label, position: usize) {
        self.positions.insert(label, position);
    }

    pub fn patch_jump_b(&mut self, instruction_index: usize, label: Label) {
        self.patches.push((instruction_index, label, Slot::B));
    }

    pub fn resolve(self, proto: &mut FunctionProto) -> Result<(), CompileError> {
        for (instruction_index, label, slot) in self.patches {
            let target =
                self.positions.get(&label).copied().ok_or_else(|| {
                    CompileError::Serialize(format!("Unresolved label {}", label.0))
                })?;
            let offset = target as i32 - instruction_index as i32 - 1;
            match slot {
                Slot::B => proto.instructions[instruction_index].b = Operand::Immediate(offset),
            }
        }
        Ok(())
    }
}
