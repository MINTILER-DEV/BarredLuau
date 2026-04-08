use crate::error::CompileError;
use crate::ir::{ConstantValue, Operand, ProgramBlob, opcode_registry::OpcodeRegistry};
use crate::serializer::checksum::{fnv1a32, seal_metadata};

#[derive(Clone, Debug, Default)]
pub struct BlobWriter {
    bytes: Vec<u8>,
}

impl BlobWriter {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn write_var_u32(&mut self, mut value: u32) {
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            self.write_u8(byte);
            if value == 0 {
                break;
            }
        }
    }

    pub fn write_string(&mut self, value: &str) {
        self.write_var_u32(value.len() as u32);
        self.write_bytes(value.as_bytes());
    }

    pub fn serialize_program(
        program: &ProgramBlob,
        registry: &OpcodeRegistry,
    ) -> Result<Vec<u8>, CompileError> {
        let mut writer = Self::new();
        writer.write_bytes(&program.magic);
        writer.write_u16(program.version);
        writer.write_u32(program.feature_flags);
        writer.write_var_u32(program.entry_prototype.0);
        writer.write_var_u32(program.prototypes.len() as u32);

        for prototype in &program.prototypes {
            writer.write_u8(prototype.name.is_some() as u8);
            if let Some(name) = &prototype.name {
                writer.write_string(name);
            }
            writer.write_var_u32(prototype.parameters.len() as u32);
            for parameter in &prototype.parameters {
                writer.write_string(parameter);
            }
            writer.write_u16(prototype.max_registers);
            writer.write_u8(prototype.return_arity);
            writer.write_var_u32(prototype.upvalues.len() as u32);
            for upvalue in &prototype.upvalues {
                writer.write_string(upvalue);
            }
            writer.write_var_u32(prototype.child_prototypes.len() as u32);
            for child in &prototype.child_prototypes {
                writer.write_var_u32(child.0);
            }
            writer.write_var_u32(prototype.local_names.len() as u32);
            for local_name in &prototype.local_names {
                writer.write_u8(local_name.is_some() as u8);
                if let Some(local_name) = local_name {
                    writer.write_string(local_name);
                }
            }
            writer.write_var_u32(prototype.constants.values.len() as u32);
            for constant in &prototype.constants.values {
                match constant {
                    ConstantValue::Nil => writer.write_u8(0),
                    ConstantValue::Boolean(value) => {
                        writer.write_u8(1);
                        writer.write_u8(u8::from(*value));
                    }
                    ConstantValue::Number(value) => {
                        writer.write_u8(2);
                        writer.write_f64(*value);
                    }
                    ConstantValue::String(value) => {
                        writer.write_u8(3);
                        writer.write_string(value);
                    }
                }
            }

            writer.write_var_u32(prototype.instructions.len() as u32);
            for instruction in &prototype.instructions {
                writer.write_u16(registry.encode(instruction.opcode));
                writer.write_operand(&instruction.a)?;
                writer.write_operand(&instruction.b)?;
                writer.write_operand(&instruction.c)?;
            }
        }

        let checksum = seal_metadata(&program.magic, &writer.bytes[4..], program.feature_flags);
        writer.write_u32(checksum);
        Ok(writer.into_bytes())
    }

    fn write_operand(&mut self, operand: &Operand) -> Result<(), CompileError> {
        match operand {
            Operand::None => self.write_u8(0),
            Operand::Register(register) => {
                self.write_u8(1);
                self.write_var_u32(u32::from(register.0));
            }
            Operand::Constant(constant) => {
                self.write_u8(2);
                self.write_var_u32(constant.0);
            }
            Operand::Immediate(value) => {
                self.write_u8(3);
                self.write_i32(*value);
            }
            Operand::Prototype(proto) => {
                self.write_u8(4);
                self.write_var_u32(proto.0);
            }
            Operand::Upvalue(index) => {
                self.write_u8(5);
                self.write_var_u32(u32::from(*index));
            }
            Operand::Boolean(value) => {
                self.write_u8(6);
                self.write_u8(u8::from(*value));
            }
        }
        Ok(())
    }

    pub fn checksum_of(bytes: &[u8]) -> u32 {
        fnv1a32(bytes)
    }
}
