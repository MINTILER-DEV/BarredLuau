#[path = "support/sample_programs.rs"]
mod sample_programs;

use barred_luau::compile_with_artifacts;
use barred_luau::config::{BuildMode, CompileConfig};
use barred_luau::ir::OpcodeRegistry;
use barred_luau::serializer::{BlobReader, EncoderKey, decode};
use std::fs;

fn extract_encoded_blob(source: &str) -> &str {
    let marker = "local _lepazse=\"";
    let start = source.find(marker).expect("encoded blob marker") + marker.len();
    let rest = &source[start..];
    let end = rest.find('"').expect("encoded blob terminator");
    &rest[..end]
}

#[test]
fn pipeline_emits_runtime_scaffold() {
    let mut config = CompileConfig::default();
    config.mode = BuildMode::Debug;
    config.anti_tamper.enabled = true;
    let artifacts = compile_with_artifacts(sample_programs::conditionals_and_loop(), &config)
        .expect("pipeline should succeed");

    assert!(
        artifacts
            .emitted_luau
            .contains("-- generated with BarredLuau")
    );
    assert!(artifacts.emitted_luau.contains("decodePayload"));
    assert!(artifacts.emitted_luau.contains("deserializeProgram"));
    assert!(artifacts.emitted_luau.contains("executeProto"));
    assert!(artifacts.emitted_luau.contains("local executeProto"));
    assert!(!artifacts.emitted_luau.contains("function executeProto("));
    assert!(artifacts.emitted_luau.contains("barx1"));
    assert!(!artifacts.encoded_blob.is_empty());
    assert!(!artifacts.serialized_blob.is_empty());
}

#[test]
fn release_pipeline_minifies_and_hides_bootstrap_strings() {
    let mut config = CompileConfig::default();
    config.mode = BuildMode::Release;
    config.anti_tamper.enabled = true;
    let artifacts = compile_with_artifacts(sample_programs::conditionals_and_loop(), &config)
        .expect("release pipeline should succeed");

    assert!(
        artifacts
            .emitted_luau
            .contains("-- generated with BarredLuau")
    );
    assert!(!artifacts.emitted_luau.contains("decodePayload"));
    assert!(!artifacts.emitted_luau.contains("bootstrap"));
    assert!(artifacts.emitted_luau.contains("barx1"));
    assert!(artifacts.emitted_luau.contains("barx2"));
    assert!(!artifacts.emitted_luau.contains("\"BRLU\""));
    assert!(!artifacts.emitted_luau.contains("LoadNil"));
    assert!(!artifacts.emitted_luau.contains("if op=="));
    assert!(!artifacts.emitted_luau.contains("elseif op=="));
    assert!(!artifacts.emitted_luau.contains("decodeRuntimeString("));
    assert!(!artifacts.emitted_luau.contains("instruction"));
    assert!(!artifacts.emitted_luau.contains("runtime"));
    assert!(!artifacts.emitted_luau.contains("tableValues"));
    assert!(!artifacts.emitted_luau.contains("bytes"));
    assert!(!artifacts.emitted_luau.contains("operand"));
    assert!(artifacts.emitted_luau.contains("getfenv"));
    assert!(artifacts.emitted_luau.contains("string.byte"));
}

#[test]
fn pipeline_can_reobfuscate_generated_bootstrap() {
    let mut config = CompileConfig::default();
    config.mode = BuildMode::Release;
    config.anti_tamper.enabled = true;
    let input = fs::read_to_string("examples/sample_output.protected.luau")
        .expect("should read generated bootstrap example");
    let artifacts =
        compile_with_artifacts(&input, &config).expect("re-obfuscating generated bootstrap");

    assert!(
        artifacts
            .emitted_luau
            .contains("-- generated with BarredLuau")
    );
    assert!(artifacts.emitted_luau.contains("getfenv"));
    assert!(!artifacts.serialized_blob.is_empty());
    assert!(!artifacts.encoded_blob.is_empty());
}

#[test]
fn checked_in_reobfuscated_example_decodes_to_valid_blob() {
    let emitted = fs::read_to_string("examples/reobfuscated.protected.luau")
        .expect("should read checked-in re-obfuscated example");
    let encoded_blob = extract_encoded_blob(&emitted);

    let input = fs::read_to_string("examples/sample_output.protected.luau")
        .expect("should read generated bootstrap example");
    let mut config = CompileConfig::default();
    config.mode = BuildMode::Release;
    config.anti_tamper.enabled = true;
    config.seed = 1337;
    config.encoder.rounds = 3;
    config.target = "roblox-luau".to_string();
    let artifacts =
        compile_with_artifacts(&input, &config).expect("rebuild checked-in re-obfuscated input");

    let key = EncoderKey {
        seed: config.seed,
        nonce: config.seed.rotate_left(13) ^ 0xA5A5_5A5A,
    };
    let decoded =
        decode(encoded_blob, &key, &config.encoder).expect("decode checked-in example blob");

    assert_eq!(decoded, artifacts.serialized_blob);
    BlobReader::deserialize_program(&decoded, &OpcodeRegistry::sequential())
        .expect("deserialize checked-in example blob");
}
