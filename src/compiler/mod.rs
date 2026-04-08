pub mod compile_context;
pub mod expression_compiler;
pub mod function_compiler;
pub mod label_resolver;
pub mod statement_compiler;

pub use function_compiler::compile_program_to_ir;
