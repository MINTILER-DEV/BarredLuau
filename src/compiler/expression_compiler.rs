use crate::compiler::compile_context::{FunctionCompileContext, VariableResolution};
use crate::compiler::function_compiler::CompilerState;
use crate::error::CompileError;
use crate::ir::{ConstantValue, Instruction, InstructionExt, Opcode, Operand, RegisterId};
use crate::parser::{BinaryOperator, Expression, TableField, UnaryOperator};

pub fn compile_expression(
    expression: &Expression,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<RegisterId, CompileError> {
    match expression {
        Expression::Nil => {
            let dst = ctx.alloc_temp();
            ctx.emit_load_nil(dst);
            Ok(dst)
        }
        Expression::Boolean(value) => {
            let dst = ctx.alloc_temp();
            ctx.emit(Instruction::new(
                Opcode::LoadBool,
                Operand::Register(dst),
                Operand::Boolean(*value),
                Operand::None,
            ));
            Ok(dst)
        }
        Expression::Number(value) => {
            let dst = ctx.alloc_temp();
            let constant = ctx.intern_constant(ConstantValue::Number(*value));
            ctx.emit(Instruction::new(
                Opcode::LoadNumber,
                Operand::Register(dst),
                Operand::Constant(constant),
                Operand::None,
            ));
            Ok(dst)
        }
        Expression::String(value) => {
            let dst = ctx.alloc_temp();
            let constant = ctx.intern_constant(ConstantValue::String(value.clone()));
            ctx.emit(Instruction::new(
                Opcode::LoadString,
                Operand::Register(dst),
                Operand::Constant(constant),
                Operand::None,
            ));
            Ok(dst)
        }
        Expression::Identifier(name) => match ctx.resolve_variable(name) {
            VariableResolution::Local(register) => Ok(register),
            VariableResolution::Upvalue(index) => {
                let dst = ctx.alloc_temp();
                ctx.emit(Instruction::new(
                    Opcode::GetUpvalue,
                    Operand::Register(dst),
                    Operand::Upvalue(index),
                    Operand::None,
                ));
                Ok(dst)
            }
            VariableResolution::Global => {
                let dst = ctx.alloc_temp();
                let constant = ctx.intern_constant(ConstantValue::String(name.clone()));
                ctx.emit(Instruction::new(
                    Opcode::GetGlobal,
                    Operand::Register(dst),
                    Operand::Constant(constant),
                    Operand::None,
                ));
                Ok(dst)
            }
        },
        Expression::VarArg => {
            let register =
                ctx.proto
                    .vararg_register
                    .ok_or_else(|| CompileError::UnsupportedSyntax {
                        node: "...".to_string(),
                        detail: "vararg expressions are only available inside vararg functions"
                            .to_string(),
                    })?;
            Ok(RegisterId(register))
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => compile_binary_expression(left, *operator, right, ctx, state),
        Expression::Unary {
            operator,
            expression,
        } => compile_unary_expression(*operator, expression, ctx, state),
        Expression::TableConstructor(fields) => compile_table_constructor(fields, ctx, state),
        Expression::Index { table, index } => {
            let table_reg = compile_expression(table, ctx, state)?;
            let key_reg = compile_expression(index, ctx, state)?;
            let dst = ctx.alloc_temp();
            ctx.emit(Instruction::new(
                Opcode::GetTable,
                Operand::Register(dst),
                Operand::Register(table_reg),
                Operand::Register(key_reg),
            ));
            Ok(dst)
        }
        Expression::Member { table, member } => {
            let table_reg = compile_expression(table, ctx, state)?;
            let key = ctx.intern_constant(ConstantValue::String(member.clone()));
            let dst = ctx.alloc_temp();
            ctx.emit(Instruction::new(
                Opcode::GetTable,
                Operand::Register(dst),
                Operand::Register(table_reg),
                Operand::Constant(key),
            ));
            Ok(dst)
        }
        Expression::FunctionCall { callee, args } => {
            if let Some(spread) = args.last().and_then(extract_unpack_argument) {
                let fixed_count = args.len().saturating_sub(1);
                let base = ctx.alloc_block(fixed_count as u16 + 2);
                let callee_reg = compile_expression(callee, ctx, state)?;
                if base != callee_reg {
                    ctx.emit_move(base, callee_reg);
                }
                for (offset, arg) in args[..fixed_count].iter().enumerate() {
                    let source = compile_expression(arg, ctx, state)?;
                    let dst = RegisterId(base.0 + offset as u16 + 1);
                    if dst != source {
                        ctx.emit_move(dst, source);
                    }
                }
                let spread_reg = compile_expression(spread, ctx, state)?;
                let spread_dst = RegisterId(base.0 + fixed_count as u16 + 1);
                if spread_dst != spread_reg {
                    ctx.emit_move(spread_dst, spread_reg);
                }
                let result = ctx.alloc_temp();
                ctx.emit(Instruction::new(
                    Opcode::CallSpread,
                    Operand::Register(base),
                    Operand::Immediate(fixed_count as i32),
                    Operand::Register(result),
                ));
                return Ok(result);
            }
            let base = ctx.alloc_block((args.len() as u16) + 1);
            let callee_reg = compile_expression(callee, ctx, state)?;
            if base != callee_reg {
                ctx.emit_move(base, callee_reg);
            }
            for (offset, arg) in args.iter().enumerate() {
                let source = compile_expression(arg, ctx, state)?;
                let dst = RegisterId(base.0 + offset as u16 + 1);
                if dst != source {
                    ctx.emit_move(dst, source);
                }
            }
            let result = ctx.alloc_temp();
            ctx.emit(Instruction::new(
                Opcode::Call,
                Operand::Register(base),
                Operand::Immediate(args.len() as i32),
                Operand::Register(result),
            ));
            Ok(result)
        }
        Expression::AnonymousFunction(function) => {
            let proto_id = state.compile_nested_function(None, function, ctx.export_bindings())?;
            state.hydrate_parent_captures(ctx, proto_id);
            let dst = ctx.alloc_temp();
            ctx.emit_closure(dst, proto_id);
            Ok(dst)
        }
    }
}

fn compile_table_constructor(
    fields: &[TableField],
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<RegisterId, CompileError> {
    if fields.len() == 1 && fields[0].key.is_none() && matches!(fields[0].value, Expression::VarArg)
    {
        return compile_expression(&fields[0].value, ctx, state);
    }

    let dst = ctx.alloc_temp();
    ctx.emit(Instruction::new(
        Opcode::NewTable,
        Operand::Register(dst),
        Operand::Immediate(fields.len() as i32),
        Operand::None,
    ));
    for (index, field) in fields.iter().enumerate() {
        let key = if let Some(key) = &field.key {
            compile_expression(key, ctx, state)?
        } else {
            let tmp = ctx.alloc_temp();
            let constant = ctx.intern_constant(ConstantValue::Number((index + 1) as f64));
            ctx.emit(Instruction::new(
                Opcode::LoadNumber,
                Operand::Register(tmp),
                Operand::Constant(constant),
                Operand::None,
            ));
            tmp
        };
        let value = compile_expression(&field.value, ctx, state)?;
        ctx.emit(Instruction::new(
            Opcode::SetTable,
            Operand::Register(dst),
            Operand::Register(key),
            Operand::Register(value),
        ));
    }
    Ok(dst)
}

fn compile_unary_expression(
    operator: UnaryOperator,
    expression: &Expression,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<RegisterId, CompileError> {
    let source = compile_expression(expression, ctx, state)?;
    let dst = ctx.alloc_temp();
    match operator {
        UnaryOperator::Not => {
            ctx.emit(Instruction::unary(
                Opcode::Not,
                dst,
                Operand::Register(source),
            ));
        }
        UnaryOperator::Length => {
            ctx.emit(Instruction::unary(
                Opcode::Len,
                dst,
                Operand::Register(source),
            ));
        }
        UnaryOperator::Negate => {
            let zero = ctx.alloc_temp();
            let constant = ctx.intern_constant(ConstantValue::Number(0.0));
            ctx.emit(Instruction::new(
                Opcode::LoadNumber,
                Operand::Register(zero),
                Operand::Constant(constant),
                Operand::None,
            ));
            ctx.emit(Instruction::binary(
                Opcode::Sub,
                dst,
                Operand::Register(zero),
                Operand::Register(source),
            ));
        }
    }
    Ok(dst)
}

fn compile_binary_expression(
    left: &Expression,
    operator: BinaryOperator,
    right: &Expression,
    ctx: &mut FunctionCompileContext,
    state: &mut CompilerState,
) -> Result<RegisterId, CompileError> {
    if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
        let left_reg = compile_expression(left, ctx, state)?;
        let dst = ctx.alloc_temp();
        if dst != left_reg {
            ctx.emit_move(dst, left_reg);
        }
        let end_label = ctx.new_label();
        match operator {
            BinaryOperator::And => ctx.emit_jump(Opcode::JumpIfNot, Some(dst), end_label),
            BinaryOperator::Or => ctx.emit_jump(Opcode::JumpIf, Some(dst), end_label),
            _ => unreachable!(),
        }
        let right_reg = compile_expression(right, ctx, state)?;
        if dst != right_reg {
            ctx.emit_move(dst, right_reg);
        }
        ctx.mark_label(end_label);
        return Ok(dst);
    }

    let left_reg = compile_expression(left, ctx, state)?;
    let right_reg = compile_expression(right, ctx, state)?;
    let dst = ctx.alloc_temp();
    let opcode = match operator {
        BinaryOperator::Add => Opcode::Add,
        BinaryOperator::Sub => Opcode::Sub,
        BinaryOperator::Mul => Opcode::Mul,
        BinaryOperator::Div => Opcode::Div,
        BinaryOperator::Mod => Opcode::Mod,
        BinaryOperator::Pow => Opcode::Pow,
        BinaryOperator::Concat => Opcode::Concat,
        BinaryOperator::Eq => Opcode::Eq,
        BinaryOperator::Lt => Opcode::Lt,
        BinaryOperator::Le => Opcode::Le,
        BinaryOperator::Gt => Opcode::Lt,
        BinaryOperator::Ge => Opcode::Le,
        BinaryOperator::Ne => Opcode::Eq,
        BinaryOperator::And | BinaryOperator::Or => unreachable!(),
    };
    ctx.emit(Instruction::binary(
        opcode,
        dst,
        Operand::Register(left_reg),
        Operand::Register(right_reg),
    ));
    if matches!(
        operator,
        BinaryOperator::Gt | BinaryOperator::Ge | BinaryOperator::Ne
    ) {
        let inverted = ctx.alloc_temp();
        ctx.emit(Instruction::unary(
            Opcode::Not,
            inverted,
            Operand::Register(dst),
        ));
        return Ok(inverted);
    }
    Ok(dst)
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
