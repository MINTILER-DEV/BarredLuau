use crate::compiler::compile_context::{FunctionCompileContext, VariableResolution};
use crate::compiler::expression_compiler::compile_expression;
use crate::compiler::function_compiler::CompilerState;
use crate::error::CompileError;
use crate::ir::{ConstantValue, Instruction, InstructionExt, Opcode, Operand, RegisterId};
use crate::parser::{Assignable, Expression, Statement};

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
            state.hydrate_parent_captures(ctx, proto_id);
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
        Statement::NumericFor {
            name,
            start,
            end,
            step,
            body,
        } => compile_numeric_for(name, start, end, step.as_ref(), body, ctx, state)?,
        Statement::GenericFor {
            names,
            iterators,
            body,
        } => compile_generic_for(names, iterators, body, ctx, state)?,
        Statement::Return(values) => {
            if values.len() == 1 {
                if let Some(spread) = extract_unpack_argument(&values[0]) {
                    let spread_reg = compile_expression(spread, ctx, state)?;
                    ctx.emit(Instruction::new(
                        Opcode::ReturnSpread,
                        Operand::Register(spread_reg),
                        Operand::None,
                        Operand::None,
                    ));
                    return Ok(());
                }
            }
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

fn extract_unpack_argument(expression: &Expression) -> Option<&Expression> {
    let Expression::FunctionCall { callee, args } = expression else {
        return None;
    };
    if args.len() != 1 {
        return None;
    }
    match callee.as_ref() {
        Expression::Member { table, member }
            if matches!(table.as_ref(), Expression::Identifier(name) if name == "table")
                && member == "unpack" =>
        {
            Some(&args[0])
        }
        _ => None,
    }
}

fn compile_numeric_for(
    name: &str,
    start: &Expression,
    end: &Expression,
    step: Option<&Expression>,
    body: &crate::parser::Block,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<(), CompileError> {
    let start_reg = compile_expression(start, ctx, state)?;
    let limit_reg = compile_expression(end, ctx, state)?;
    let step_reg = if let Some(step) = step {
        compile_expression(step, ctx, state)?
    } else {
        let one = ctx.alloc_temp();
        let constant = ctx.intern_constant(ConstantValue::Number(1.0));
        ctx.emit(Instruction::new(
            Opcode::LoadNumber,
            Operand::Register(one),
            Operand::Constant(constant),
            Operand::None,
        ));
        one
    };

    ctx.enter_scope();
    let loop_reg = ctx.declare_local(name.to_string());
    if loop_reg != start_reg {
        ctx.emit_move(loop_reg, start_reg);
    }

    let zero = ctx.alloc_temp();
    let zero_constant = ctx.intern_constant(ConstantValue::Number(0.0));
    ctx.emit(Instruction::new(
        Opcode::LoadNumber,
        Operand::Register(zero),
        Operand::Constant(zero_constant),
        Operand::None,
    ));

    let start_label = ctx.new_label();
    let positive_label = ctx.new_label();
    let end_label = ctx.new_label();
    ctx.mark_label(start_label);

    let step_positive = ctx.alloc_temp();
    ctx.emit(Instruction::binary(
        Opcode::Lt,
        step_positive,
        Operand::Register(zero),
        Operand::Register(step_reg),
    ));
    ctx.emit_jump(Opcode::JumpIf, Some(step_positive), positive_label);

    let negative_ok = ctx.alloc_temp();
    ctx.emit(Instruction::binary(
        Opcode::Le,
        negative_ok,
        Operand::Register(limit_reg),
        Operand::Register(loop_reg),
    ));
    ctx.emit_jump(Opcode::JumpIfNot, Some(negative_ok), end_label);

    let body_label = ctx.new_label();
    ctx.emit_jump(Opcode::Jump, None, body_label);

    ctx.mark_label(positive_label);
    let positive_ok = ctx.alloc_temp();
    ctx.emit(Instruction::binary(
        Opcode::Le,
        positive_ok,
        Operand::Register(loop_reg),
        Operand::Register(limit_reg),
    ));
    ctx.emit_jump(Opcode::JumpIfNot, Some(positive_ok), end_label);

    ctx.mark_label(body_label);
    compile_block(&body.statements, ctx, state, true)?;
    ctx.emit(Instruction::binary(
        Opcode::Add,
        loop_reg,
        Operand::Register(loop_reg),
        Operand::Register(step_reg),
    ));
    ctx.emit_jump(Opcode::Jump, None, start_label);
    ctx.mark_label(end_label);
    ctx.exit_scope();
    Ok(())
}

fn compile_generic_for(
    names: &[String],
    iterators: &[Expression],
    body: &crate::parser::Block,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<(), CompileError> {
    let [iterator] = iterators else {
        return Err(CompileError::UnsupportedSyntax {
            node: "generic for".to_string(),
            detail: "only single-iterator generic for loops are supported".to_string(),
        });
    };
    let Expression::FunctionCall { callee, args } = iterator else {
        return Err(CompileError::UnsupportedSyntax {
            node: "generic for".to_string(),
            detail: "only ipairs-based generic for loops are supported".to_string(),
        });
    };
    if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "ipairs")
        || args.len() != 1
    {
        return Err(CompileError::UnsupportedSyntax {
            node: "generic for".to_string(),
            detail: "only ipairs-based generic for loops are supported".to_string(),
        });
    }

    let source_reg = compile_expression(&args[0], ctx, state)?;
    let counter_reg = ctx.alloc_temp();
    let one_constant = ctx.intern_constant(ConstantValue::Number(1.0));
    ctx.emit(Instruction::new(
        Opcode::LoadNumber,
        Operand::Register(counter_reg),
        Operand::Constant(one_constant),
        Operand::None,
    ));

    ctx.enter_scope();
    let mut loop_locals = Vec::with_capacity(names.len());
    for name in names {
        loop_locals.push(ctx.declare_local(name.clone()));
    }

    let start_label = ctx.new_label();
    let end_label = ctx.new_label();
    ctx.mark_label(start_label);

    let len_reg = ctx.alloc_temp();
    ctx.emit(Instruction::unary(
        Opcode::Len,
        len_reg,
        Operand::Register(source_reg),
    ));
    let continue_reg = ctx.alloc_temp();
    ctx.emit(Instruction::binary(
        Opcode::Le,
        continue_reg,
        Operand::Register(counter_reg),
        Operand::Register(len_reg),
    ));
    ctx.emit_jump(Opcode::JumpIfNot, Some(continue_reg), end_label);

    if let Some(index_local) = loop_locals.first().copied() {
        if index_local != counter_reg {
            ctx.emit_move(index_local, counter_reg);
        }
    }
    if let Some(value_local) = loop_locals.get(1).copied() {
        ctx.emit(Instruction::new(
            Opcode::GetTable,
            Operand::Register(value_local),
            Operand::Register(source_reg),
            Operand::Register(counter_reg),
        ));
    }
    for local in loop_locals.iter().skip(2) {
        ctx.emit_load_nil(*local);
    }

    compile_block(&body.statements, ctx, state, true)?;
    ctx.emit(Instruction::binary(
        Opcode::Add,
        counter_reg,
        Operand::Register(counter_reg),
        Operand::Constant(one_constant),
    ));
    ctx.emit_jump(Opcode::Jump, None, start_label);
    ctx.mark_label(end_label);
    ctx.exit_scope();
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
