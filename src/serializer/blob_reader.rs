use crate::error::CompileError;
use crate::ir::{
    ConstantId, ConstantPool, ConstantValue, FunctionProto, Instruction, Operand, ProgramBlob,
    PrototypeId, RegisterId, opcode_registry::OpcodeRegistry,
};
use crate::serializer::checksum::seal_metadata;

#[derive(Clone, Debug)]
pub struct BlobReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BlobReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn deserialize_program(
        bytes: &'a [u8],
        registry: &OpcodeRegistry,
    ) -> Result<ProgramBlob, CompileError> {
        if bytes.len() < 10 {
            return Err(CompileError::Deserialize(
                "Blob is too short to contain a valid header".to_string(),
            ));
        }

        let payload_len = bytes.len().saturating_sub(4);
        let expected_checksum = u32::from_le_bytes(
            bytes[payload_len..]
                .try_into()
                .map_err(|_| CompileError::Deserialize("Missing checksum footer".to_string()))?,
        );
        let feature_flags = u32::from_le_bytes(bytes[6..10].try_into().map_err(|_| {
            CompileError::Deserialize("Missing feature flags in blob header".to_string())
        })?);
        let actual_checksum = seal_metadata(&bytes[..4], &bytes[4..payload_len], feature_flags);
        if expected_checksum != actual_checksum {
            return Err(CompileError::Integrity(
                "Program blob checksum validation failed".to_string(),
            ));
        }

        let mut reader = Self::new(&bytes[..payload_len]);
        let magic: [u8; 4] = reader.read_bytes(4)?.try_into().map_err(|_| {
            CompileError::Deserialize("Header did not contain a 4-byte magic".to_string())
        })?;
        let version = reader.read_u16()?;
        let feature_flags = reader.read_u32()?;
        let entry_prototype = PrototypeId(reader.read_var_u32()?);
        let prototype_count = reader.read_var_u32()? as usize;
        let mut prototypes = Vec::with_capacity(prototype_count);
        for _ in 0..prototype_count {
            let has_name = reader.read_u8()? != 0;
            let name = if has_name {
                Some(reader.read_string()?)
            } else {
                None
            };
            let parameter_count = reader.read_var_u32()? as usize;
            let mut parameters = Vec::with_capacity(parameter_count);
            for _ in 0..parameter_count {
                parameters.push(reader.read_string()?);
            }
            let is_vararg = reader.read_u8()? != 0;
            let vararg_register = match reader.read_u16()? {
                u16::MAX => None,
                value => Some(value),
            };
            let max_registers = reader.read_u16()?;
            let return_arity = reader.read_u8()?;
            let upvalue_count = reader.read_var_u32()? as usize;
            let mut upvalues = Vec::with_capacity(upvalue_count);
            for _ in 0..upvalue_count {
                upvalues.push(reader.read_string()?);
            }
            let child_count = reader.read_var_u32()? as usize;
            let mut child_prototypes = Vec::with_capacity(child_count);
            for _ in 0..child_count {
                child_prototypes.push(PrototypeId(reader.read_var_u32()?));
            }
            let local_name_count = reader.read_var_u32()? as usize;
            let mut local_names = Vec::with_capacity(local_name_count);
            for _ in 0..local_name_count {
                let has_name = reader.read_u8()? != 0;
                local_names.push(if has_name {
                    Some(reader.read_string()?)
                } else {
                    None
                });
            }
            let constant_count = reader.read_var_u32()? as usize;
            let mut constants = ConstantPool::default();
            for _ in 0..constant_count {
                let tag = reader.read_u8()?;
                let value = match tag {
                    0 => ConstantValue::Nil,
                    1 => ConstantValue::Boolean(reader.read_u8()? != 0),
                    2 => ConstantValue::Number(reader.read_f64()?),
                    3 => ConstantValue::String(reader.read_string()?),
                    _ => {
                        return Err(CompileError::Deserialize(format!(
                            "Unknown constant tag `{tag}`"
                        )));
                    }
                };
                constants.values.push(value);
            }
            let instruction_count = reader.read_var_u32()? as usize;
            let mut instructions = Vec::with_capacity(instruction_count);
            for _ in 0..instruction_count {
                let opcode = registry.decode(reader.read_u16()?)?;
                let a = reader.read_operand()?;
                let b = reader.read_operand()?;
                let c = reader.read_operand()?;
                instructions.push(Instruction { opcode, a, b, c });
            }

            prototypes.push(FunctionProto {
                name,
                parameters,
                is_vararg,
                vararg_register,
                max_registers,
                upvalues,
                constants,
                instructions,
                child_prototypes,
                local_names,
                return_arity,
            });
        }

        Ok(ProgramBlob {
            magic,
            version,
            feature_flags,
            entry_prototype,
            prototypes,
            checksum: expected_checksum,
        })
    }

    pub fn read_u8(&mut self) -> Result<u8, CompileError> {
        let byte = *self
            .bytes
            .get(self.cursor)
            .ok_or_else(|| CompileError::Deserialize("Unexpected end of blob".to_string()))?;
        self.cursor += 1;
        Ok(byte)
    }

    pub fn read_u16(&mut self) -> Result<u16, CompileError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32(&mut self) -> Result<u32, CompileError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32(&mut self) -> Result<i32, CompileError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_f64(&mut self) -> Result<f64, CompileError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes(
            bytes.try_into().expect("f64 is 8 bytes"),
        ))
    }

    pub fn read_var_u32(&mut self) -> Result<u32, CompileError> {
        let mut value = 0u32;
        let mut shift = 0;
        loop {
            let byte = self.read_u8()?;
            value |= u32::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                return Err(CompileError::Deserialize(
                    "Invalid varint encoding".to_string(),
                ));
            }
        }
        Ok(value)
    }

    pub fn read_string(&mut self) -> Result<String, CompileError> {
        let length = self.read_var_u32()? as usize;
        let bytes = self.read_bytes(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|error| {
            CompileError::Deserialize(format!("String payload was not valid UTF-8: {error}"))
        })
    }

    fn read_operand(&mut self) -> Result<Operand, CompileError> {
        let tag = self.read_u8()?;
        match tag {
            0 => Ok(Operand::None),
            1 => Ok(Operand::Register(RegisterId(self.read_var_u32()? as u16))),
            2 => Ok(Operand::Constant(ConstantId(self.read_var_u32()?))),
            3 => Ok(Operand::Immediate(self.read_i32()?)),
            4 => Ok(Operand::Prototype(PrototypeId(self.read_var_u32()?))),
            5 => Ok(Operand::Upvalue(self.read_var_u32()? as u16)),
            6 => Ok(Operand::Boolean(self.read_u8()? != 0)),
            _ => Err(CompileError::Deserialize(format!(
                "Unknown operand tag `{tag}`"
            ))),
        }
    }

    fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], CompileError> {
        let end = self.cursor + count;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            CompileError::Deserialize("Unexpected end of blob while reading bytes".to_string())
        })?;
        self.cursor = end;
        Ok(bytes)
    }
}
