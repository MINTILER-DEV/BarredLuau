use crate::compiler::compile_context::{FunctionCompileContext, VariableResolution};
use crate::compiler::expression_compiler::compile_expression;
use crate::compiler::function_compiler::CompilerState;
use crate::error::CompileError;
use crate::ir::{ConstantValue, Instruction, InstructionExt, Opcode, Operand, RegisterId};
use crate::parser::{Assignable, Statement};

pub fn compile_block(
    statements: &[Statement],
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
    scoped: bool,
) -> Result<(), CompileError> {
    if scoped {
        ctx.enter_scope();
    }
    for statement in statements {
        compile_statement(statement, ctx, state)?;
    }
    if scoped {
        ctx.exit_scope();
    }
    Ok(())
}

pub fn compile_statement(
    statement: &Statement,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<(), CompileError> {
    match statement {
        Statement::LocalDeclaration { names, values } => {
            let mut compiled_values = Vec::with_capacity(values.len());
            for value in values {
                compiled_values.push(compile_expression(value, ctx, state)?);
            }

            for (index, name) in names.iter().enumerate() {
                let local = ctx.declare_local(name.clone());
                if let Some(value_register) = compiled_values.get(index) {
                    if *value_register != local {
                        ctx.emit_move(local, *value_register);
                    }
                } else {
                    ctx.emit_load_nil(local);
                }
            }
        }
        Statement::Assignment { targets, values } => {
            let mut compiled_values = Vec::with_capacity(values.len());
            for value in values {
                compiled_values.push(compile_expression(value, ctx, state)?);
            }

            for (index, target) in targets.iter().enumerate() {
                let source = compiled_values
                    .get(index)
                    .copied()
                    .or_else(|| compiled_values.last().copied())
                    .ok_or_else(|| CompileError::UnsupportedSyntax {
                        node: "assignment".to_string(),
                        detail: "assignment without at least one value is not supported"
                            .to_string(),
                    })?;
                assign_target(target, source, ctx, state)?;
            }
        }
        Statement::FunctionDeclaration {
            name,
            function,
            is_local,
        } => {
            let destination = if *is_local {
                ctx.declare_local(name.clone())
            } else {
                ctx.alloc_temp()
            };
            let proto_id = state.compile_nested_function(
                Some(name.clone()),
                function,
                ctx.export_bindings(),
            )?;
            ctx.emit_closure(destination, proto_id);
            if !*is_local {
                let constant = ctx.intern_constant(ConstantValue::String(name.clone()));
                ctx.emit(Instruction::new(
                    Opcode::SetGlobal,
                    Operand::Constant(constant),
                    Operand::Register(destination),
                    Operand::None,
                ));
            }
        }
        Statement::If {
            clauses,
            else_block,
        } => {
            let end_label = ctx.new_label();
            for clause in clauses {
                let else_label = ctx.new_label();
                let condition = compile_expression(&clause.condition, ctx, state)?;
                ctx.emit_jump(Opcode::JumpIfNot, Some(condition), else_label);
                compile_block(&clause.block.statements, ctx, state, true)?;
                ctx.emit_jump(Opcode::Jump, None, end_label);
                ctx.mark_label(else_label);
            }
            if let Some(block) = else_block {
                compile_block(&block.statements, ctx, state, true)?;
            }
            ctx.mark_label(end_label);
        }
        Statement::While { condition, body } => {
            let start_label = ctx.new_label();
            let end_label = ctx.new_label();
            ctx.mark_label(start_label);
            let condition = compile_expression(condition, ctx, state)?;
            ctx.emit_jump(Opcode::JumpIfNot, Some(condition), end_label);
            compile_block(&body.statements, ctx, state, true)?;
            ctx.emit_jump(Opcode::Jump, None, start_label);
            ctx.mark_label(end_label);
        }
        Statement::RepeatUntil { body, condition } => {
            let start_label = ctx.new_label();
            ctx.mark_label(start_label);
            compile_block(&body.statements, ctx, state, true)?;
            let condition = compile_expression(condition, ctx, state)?;
            ctx.emit_jump(Opcode::JumpIfNot, Some(condition), start_label);
        }
        Statement::NumericFor { .. } => {
            return Err(CompileError::UnsupportedSyntax {
                node: "numeric for".to_string(),
                detail: "numeric for loops are parsed but not yet lowered into VM IR".to_string(),
            });
        }
        Statement::GenericFor { .. } => {
            return Err(CompileError::UnsupportedSyntax {
                node: "generic for".to_string(),
                detail: "generic for loops are parsed but not yet lowered into VM IR".to_string(),
            });
        }
        Statement::Return(values) => {
            if values.is_empty() {
                ctx.emit(Instruction::new(
                    Opcode::Return,
                    Operand::Register(RegisterId(0)),
                    Operand::Immediate(0),
                    Operand::None,
                ));
            } else {
                let base = ctx.alloc_block(values.len() as u16);
                for (offset, value) in values.iter().enumerate() {
                    let source = compile_expression(value, ctx, state)?;
                    let dst = RegisterId(base.0 + offset as u16);
                    if dst != source {
                        ctx.emit_move(dst, source);
                    }
                }
                ctx.emit(Instruction::new(
                    Opcode::Return,
                    Operand::Register(base),
                    Operand::Immediate(values.len() as i32),
                    Operand::None,
                ));
            }
        }
        Statement::Expression(expression) => {
            let _ = compile_expression(expression, ctx, state)?;
        }
    }
    Ok(())
}

fn assign_target(
    target: &Assignable,
    source: RegisterId,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<(), CompileError> {
    match target {
        Assignable::Identifier(name) => match ctx.resolve_variable(name) {
            VariableResolution::Local(register) => {
                if register != source {
                    ctx.emit_move(register, source);
                }
            }
            VariableResolution::Upvalue(index) => {
                ctx.emit(Instruction::new(
                    Opcode::SetUpvalue,
                    Operand::Upvalue(index),
                    Operand::Register(source),
                    Operand::None,
                ));
            }
            VariableResolution::Global => {
                let constant = ctx.intern_constant(ConstantValue::String(name.clone()));
                ctx.emit(Instruction::new(
                    Opcode::SetGlobal,
                    Operand::Constant(constant),
                    Operand::Register(source),
                    Operand::None,
                ));
            }
        },
        Assignable::Index { table, index } => {
            let table_reg = compile_expression(table, ctx, state)?;
            let key_reg = compile_expression(index, ctx, state)?;
            ctx.emit(Instruction::new(
                Opcode::SetTable,
                Operand::Register(table_reg),
                Operand::Register(key_reg),
                Operand::Register(source),
            ));
        }
        Assignable::Member { table, member } => {
            let table_reg = compile_expression(table, ctx, state)?;
            let key = ctx.intern_constant(ConstantValue::String(member.clone()));
            ctx.emit(Instruction::new(
                Opcode::SetTable,
                Operand::Register(table_reg),
                Operand::Constant(key),
                Operand::Register(source),
            ));
        }
    }
    Ok(())
}
