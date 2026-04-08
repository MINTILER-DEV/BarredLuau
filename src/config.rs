#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    Debug,
    Release,
}

#[derive(Clone, Debug)]
pub enum ParserBackendKind {
    MockLuau,
}

#[derive(Clone, Debug)]
pub struct EncoderConfig {
    pub rounds: usize,
    pub alphabet: String,
    pub chunk_size: usize,
    pub include_checksum: bool,
    pub interleave: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            rounds: 2,
            alphabet: "q7Wm!xP2r#Dk9L@cT5nYh$Jb%Vf&gK*QsZ+a?E".to_string(),
            chunk_size: 48,
            include_checksum: true,
            interleave: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AntiTamperConfig {
    pub enabled: bool,
    pub verify_header: bool,
    pub verify_checksum: bool,
    pub verify_opcode_table: bool,
}

impl Default for AntiTamperConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            verify_header: true,
            verify_checksum: true,
            verify_opcode_table: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObfuscationConfig {
    pub mangle_runtime_identifiers: bool,
    pub pool_runtime_strings: bool,
    pub randomize_opcodes: bool,
    pub handler_indirection: bool,
    pub constant_pool_shuffle: bool,
    pub emit_decoy_metadata: bool,
    pub selective_virtualization: bool,
}

impl Default for ObfuscationConfig {
    fn default() -> Self {
        Self {
            mangle_runtime_identifiers: true,
            pool_runtime_strings: true,
            randomize_opcodes: false,
            handler_indirection: false,
            constant_pool_shuffle: false,
            emit_decoy_metadata: false,
            selective_virtualization: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompileConfig {
    pub mode: BuildMode,
    pub parser_backend: ParserBackendKind,
    pub anti_tamper: AntiTamperConfig,
    pub obfuscation: ObfuscationConfig,
    pub encoder: EncoderConfig,
    pub target: String,
    pub seed: u32,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            mode: BuildMode::Release,
            parser_backend: ParserBackendKind::MockLuau,
            anti_tamper: AntiTamperConfig::default(),
            obfuscation: ObfuscationConfig::default(),
            encoder: EncoderConfig::default(),
            target: "roblox-luau".to_string(),
            seed: 0x0BAD_5EED,
        }
    }
}

impl CompileConfig {
    pub fn feature_flags(&self) -> u32 {
        let mut flags = 0u32;
        if self.anti_tamper.enabled {
            flags |= 1 << 0;
        }
        if self.obfuscation.randomize_opcodes {
            flags |= 1 << 1;
        }
        if self.obfuscation.mangle_runtime_identifiers {
            flags |= 1 << 2;
        }
        if self.obfuscation.pool_runtime_strings {
            flags |= 1 << 3;
        }
        if self.obfuscation.constant_pool_shuffle {
            flags |= 1 << 4;
        }
        if self.obfuscation.emit_decoy_metadata {
            flags |= 1 << 5;
        }
        if self.encoder.interleave {
            flags |= 1 << 6;
        }
        if self.encoder.include_checksum {
            flags |= 1 << 7;
        }
        flags
    }
}
