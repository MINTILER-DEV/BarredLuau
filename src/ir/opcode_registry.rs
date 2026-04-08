use std::collections::BTreeMap;

use crate::error::CompileError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum Opcode {
    LoadNil = 0,
    LoadBool = 1,
    LoadNumber = 2,
    LoadString = 3,
    Move = 4,
    GetGlobal = 5,
    SetGlobal = 6,
    GetTable = 7,
    SetTable = 8,
    NewTable = 9,
    Call = 10,
    Return = 11,
    Jump = 12,
    JumpIf = 13,
    JumpIfNot = 14,
    Closure = 15,
    GetUpvalue = 16,
    SetUpvalue = 17,
    Concat = 18,
    Add = 19,
    Sub = 20,
    Mul = 21,
    Div = 22,
    Mod = 23,
    Pow = 24,
    Eq = 25,
    Lt = 26,
    Le = 27,
    Len = 28,
    Not = 29,
}

impl Opcode {
    pub const ALL: [Opcode; 30] = [
        Opcode::LoadNil,
        Opcode::LoadBool,
        Opcode::LoadNumber,
        Opcode::LoadString,
        Opcode::Move,
        Opcode::GetGlobal,
        Opcode::SetGlobal,
        Opcode::GetTable,
        Opcode::SetTable,
        Opcode::NewTable,
        Opcode::Call,
        Opcode::Return,
        Opcode::Jump,
        Opcode::JumpIf,
        Opcode::JumpIfNot,
        Opcode::Closure,
        Opcode::GetUpvalue,
        Opcode::SetUpvalue,
        Opcode::Concat,
        Opcode::Add,
        Opcode::Sub,
        Opcode::Mul,
        Opcode::Div,
        Opcode::Mod,
        Opcode::Pow,
        Opcode::Eq,
        Opcode::Lt,
        Opcode::Le,
        Opcode::Len,
        Opcode::Not,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Opcode::LoadNil => "LoadNil",
            Opcode::LoadBool => "LoadBool",
            Opcode::LoadNumber => "LoadNumber",
            Opcode::LoadString => "LoadString",
            Opcode::Move => "Move",
            Opcode::GetGlobal => "GetGlobal",
            Opcode::SetGlobal => "SetGlobal",
            Opcode::GetTable => "GetTable",
            Opcode::SetTable => "SetTable",
            Opcode::NewTable => "NewTable",
            Opcode::Call => "Call",
            Opcode::Return => "Return",
            Opcode::Jump => "Jump",
            Opcode::JumpIf => "JumpIf",
            Opcode::JumpIfNot => "JumpIfNot",
            Opcode::Closure => "Closure",
            Opcode::GetUpvalue => "GetUpvalue",
            Opcode::SetUpvalue => "SetUpvalue",
            Opcode::Concat => "Concat",
            Opcode::Add => "Add",
            Opcode::Sub => "Sub",
            Opcode::Mul => "Mul",
            Opcode::Div => "Div",
            Opcode::Mod => "Mod",
            Opcode::Pow => "Pow",
            Opcode::Eq => "Eq",
            Opcode::Lt => "Lt",
            Opcode::Le => "Le",
            Opcode::Len => "Len",
            Opcode::Not => "Not",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpcodeRegistry {
    forward: BTreeMap<Opcode, u16>,
    reverse: BTreeMap<u16, Opcode>,
}

impl Default for OpcodeRegistry {
    fn default() -> Self {
        Self::sequential()
    }
}

impl OpcodeRegistry {
    pub fn sequential() -> Self {
        let mut forward = BTreeMap::new();
        let mut reverse = BTreeMap::new();
        for opcode in Opcode::ALL {
            let encoded = opcode as u16;
            forward.insert(opcode, encoded);
            reverse.insert(encoded, opcode);
        }
        Self { forward, reverse }
    }

    pub fn randomized(seed: u32) -> Self {
        let mut encoded_values: Vec<u16> = (0..Opcode::ALL.len() as u16).collect();
        let mut state = seed.max(1);
        for index in (1..encoded_values.len()).rev() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let target = (state as usize) % (index + 1);
            encoded_values.swap(index, target);
        }

        let mut forward = BTreeMap::new();
        let mut reverse = BTreeMap::new();
        for (opcode, encoded) in Opcode::ALL.into_iter().zip(encoded_values) {
            forward.insert(opcode, encoded);
            reverse.insert(encoded, opcode);
        }
        Self { forward, reverse }
    }

    pub fn encode(&self, opcode: Opcode) -> u16 {
        self.forward[&opcode]
    }

    pub fn decode(&self, encoded: u16) -> Result<Opcode, CompileError> {
        self.reverse
            .get(&encoded)
            .copied()
            .ok_or_else(|| CompileError::Deserialize(format!("Unknown opcode id `{encoded}`")))
    }

    pub fn pairs(&self) -> impl Iterator<Item = (Opcode, u16)> + '_ {
        self.forward
            .iter()
            .map(|(opcode, encoded)| (*opcode, *encoded))
    }

    pub fn checksum(&self) -> u32 {
        self.pairs().fold(2166136261u32, |hash, (opcode, encoded)| {
            let hash = hash ^ (opcode as u32);
            hash.wrapping_mul(16777619) ^ u32::from(encoded)
        })
    }
}
