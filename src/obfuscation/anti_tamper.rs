use crate::config::CompileConfig;
use crate::ir::{OpcodeRegistry, ProgramBlob};
use crate::obfuscation::integrity::checksum_label;

#[derive(Clone, Debug)]
pub struct RuntimeIntegrity {
    pub verify_header: bool,
    pub verify_checksum: bool,
    pub verify_opcode_table: bool,
    pub header_checksum: u32,
    pub opcode_checksum: u32,
}

pub fn build_runtime_integrity(
    program: &ProgramBlob,
    registry: &OpcodeRegistry,
    config: &CompileConfig,
) -> RuntimeIntegrity {
    RuntimeIntegrity {
        verify_header: config.anti_tamper.verify_header,
        verify_checksum: config.anti_tamper.verify_checksum,
        verify_opcode_table: config.anti_tamper.verify_opcode_table,
        header_checksum: checksum_label("header", &program.magic),
        opcode_checksum: registry.checksum(),
    }
}
