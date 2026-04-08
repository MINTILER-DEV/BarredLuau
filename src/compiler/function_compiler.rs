use crate::compiler::compile_context::{FunctionCompileContext, ParentBindings};
use crate::compiler::statement_compiler::compile_block;
use crate::config::CompileConfig;
use crate::error::CompileError;
use crate::ir::{FunctionProto, ProgramBlob, PrototypeId};
use crate::parser::{FunctionExpression, Program};

#[derive(Clone, Debug, Default)]
pub struct CompilerState {
    prototypes: Vec<FunctionProto>,
}

impl CompilerState {
    pub fn compile_root_program(
        mut self,
        program: &Program,
        config: &CompileConfig,
    ) -> Result<ProgramBlob, CompileError> {
        let entry = self.compile_block_as_proto(
            Some("main".to_string()),
            &FunctionExpression {
                params: Vec::new(),
                body: program.block.clone(),
            },
            ParentBindings::new(),
        )?;

        Ok(ProgramBlob {
            feature_flags: config.feature_flags(),
            entry_prototype: entry,
            prototypes: self.prototypes,
            ..ProgramBlob::default()
        })
    }

    pub fn compile_nested_function(
        &mut self,
        name: Option<String>,
        function: &FunctionExpression,
        parent_bindings: ParentBindings,
    ) -> Result<PrototypeId, CompileError> {
        self.compile_block_as_proto(name, function, parent_bindings)
    }

    fn compile_block_as_proto(
        &mut self,
        name: Option<String>,
        function: &FunctionExpression,
        parent_bindings: ParentBindings,
    ) -> Result<PrototypeId, CompileError> {
        let mut ctx = FunctionCompileContext::new(name, &function.params, parent_bindings);
        compile_block(&function.body.statements, &mut ctx, self, false)?;
        let proto = ctx.finalize()?;
        let id = PrototypeId(self.prototypes.len() as u32);
        self.prototypes.push(proto);
        Ok(id)
    }
}

pub fn compile_program_to_ir(
    program: &Program,
    config: &CompileConfig,
) -> Result<ProgramBlob, CompileError> {
    CompilerState::default().compile_root_program(program, config)
}
