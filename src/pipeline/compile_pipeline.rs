use crate::compiler::compile_program_to_ir;
use crate::config::{CompileConfig, ParserBackendKind};
use crate::error::CompileError;
use crate::ir::{OpcodeRegistry, ProgramBlob};
use crate::obfuscation::anti_tamper::{RuntimeIntegrity, build_runtime_integrity};
use crate::parser::scope_analyzer::{ScopeSummary, analyze_program};
use crate::parser::{AstBackend, MockLuauBackend};
use crate::serializer::{BlobReader, BlobWriter, EncoderKey, encode};
use crate::vmgen::emit_luau_output;

#[derive(Clone, Debug)]
pub struct PipelineArtifacts {
    pub program: ProgramBlob,
    pub opcode_registry: OpcodeRegistry,
    pub serialized_blob: Vec<u8>,
    pub encoded_blob: String,
    pub emitted_luau: String,
    pub scope_summary: ScopeSummary,
    pub integrity: RuntimeIntegrity,
}

pub fn compile(source: &str, config: &CompileConfig) -> Result<String, CompileError> {
    Ok(compile_with_artifacts(source, config)?.emitted_luau)
}

pub fn compile_with_artifacts(
    source: &str,
    config: &CompileConfig,
) -> Result<PipelineArtifacts, CompileError> {
    let backend: Box<dyn AstBackend> = match config.parser_backend {
        ParserBackendKind::MockLuau => Box::<MockLuauBackend>::default(),
    };

    let program = backend.parse(source)?;
    let scope_summary = analyze_program(&program)?;
    let program_ir = compile_program_to_ir(&program, config)?;
    let opcode_registry = if config.obfuscation.randomize_opcodes {
        OpcodeRegistry::randomized(config.seed)
    } else {
        OpcodeRegistry::sequential()
    };
    let serialized_blob = BlobWriter::serialize_program(&program_ir, &opcode_registry)?;
    let _roundtrip = BlobReader::deserialize_program(&serialized_blob, &opcode_registry)?;
    let encoder_key = EncoderKey {
        seed: config.seed,
        nonce: config.seed.rotate_left(13) ^ 0xA5A5_5A5A,
    };
    let encoded_blob = encode(&serialized_blob, &encoder_key, &config.encoder)?;
    let integrity = build_runtime_integrity(&program_ir, &opcode_registry, config);
    let emitted_luau = emit_luau_output(
        &program_ir,
        &encoded_blob,
        &encoder_key,
        config,
        &opcode_registry,
        &integrity,
    );

    Ok(PipelineArtifacts {
        program: program_ir,
        opcode_registry,
        serialized_blob,
        encoded_blob,
        emitted_luau,
        scope_summary,
        integrity,
    })
}
