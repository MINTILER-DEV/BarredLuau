use crate::ir::OpcodeRegistry;

pub fn emit_opcode_constants(registry: &OpcodeRegistry) -> String {
    let mut buffer = String::new();
    buffer.push_str("local OPCODES = {\n");
    for (opcode, encoded) in registry.pairs() {
        buffer.push_str(&format!("    {} = {},\n", opcode.as_str(), encoded));
    }
    buffer.push_str("}\n");
    buffer
}
