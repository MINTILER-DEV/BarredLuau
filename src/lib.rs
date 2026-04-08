pub mod compiler;
pub mod config;
pub mod error;
pub mod ir;
pub mod obfuscation;
pub mod parser;
pub mod pipeline;
pub mod serializer;
pub mod vmgen;

pub use config::{BuildMode, CompileConfig};
pub use error::CompileError;
pub use pipeline::compile_pipeline::{PipelineArtifacts, compile, compile_with_artifacts};
