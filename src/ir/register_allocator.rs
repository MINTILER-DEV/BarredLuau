use crate::ir::ir_types::RegisterId;

#[derive(Clone, Debug, Default)]
pub struct RegisterAllocator {
    next: u16,
    high_water: u16,
}

impl RegisterAllocator {
    pub fn alloc(&mut self) -> RegisterId {
        let register = RegisterId(self.next);
        self.next = self.next.saturating_add(1);
        self.high_water = self.high_water.max(self.next);
        register
    }

    pub fn alloc_block(&mut self, count: u16) -> RegisterId {
        let start = self.next;
        self.next = self.next.saturating_add(count);
        self.high_water = self.high_water.max(self.next);
        RegisterId(start)
    }

    pub fn high_water_mark(&self) -> u16 {
        self.high_water
    }
}
