#[path = "support/sample_programs.rs"]
mod sample_programs;

use barred_luau::compile_with_artifacts;
use barred_luau::config::{BuildMode, CompileConfig};

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
    assert!(artifacts.emitted_luau.contains("barinteg"));
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
    assert!(artifacts.emitted_luau.contains("barinteg"));
    assert!(artifacts.emitted_luau.contains("barfault"));
    assert!(!artifacts.emitted_luau.contains("\"BRLU\""));
    assert!(!artifacts.emitted_luau.contains("LoadNil"));
    assert!(!artifacts.emitted_luau.contains("if op=="));
    assert!(!artifacts.emitted_luau.contains("elseif op=="));
    assert!(!artifacts.emitted_luau.contains("decodeRuntimeString("));
}
