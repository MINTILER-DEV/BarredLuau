use crate::parser::ast_loader::{
    Assignable, Block, Expression, FunctionExpression, IfClause, Program, Statement, TableField,
};

pub trait AstVisitor {
    fn visit_program(&mut self, program: &Program) {
        walk_program(self, program);
    }

    fn visit_block(&mut self, block: &Block) {
        walk_block(self, block);
    }

    fn visit_statement(&mut self, statement: &Statement) {
        walk_statement(self, statement);
    }

    fn visit_expression(&mut self, expression: &Expression) {
        walk_expression(self, expression);
    }

    fn visit_assignable(&mut self, assignable: &Assignable) {
        walk_assignable(self, assignable);
    }
}

pub fn walk_program<V: AstVisitor + ?Sized>(visitor: &mut V, program: &Program) {
    visitor.visit_block(&program.block);
}

pub fn walk_block<V: AstVisitor + ?Sized>(visitor: &mut V, block: &Block) {
    for statement in &block.statements {
        visitor.visit_statement(statement);
    }
}

pub fn walk_statement<V: AstVisitor + ?Sized>(visitor: &mut V, statement: &Statement) {
    match statement {
        Statement::LocalDeclaration { values, .. } => {
            for value in values {
                visitor.visit_expression(value);
            }
        }
        Statement::Assignment { targets, values } => {
            for target in targets {
                visitor.visit_assignable(target);
            }
            for value in values {
                visitor.visit_expression(value);
            }
        }
        Statement::FunctionDeclaration { function, .. } => walk_function(visitor, function),
        Statement::If {
            clauses,
            else_block,
        } => {
            for IfClause { condition, block } in clauses {
                visitor.visit_expression(condition);
                visitor.visit_block(block);
            }
            if let Some(block) = else_block {
                visitor.visit_block(block);
            }
        }
        Statement::While { condition, body } => {
            visitor.visit_expression(condition);
            visitor.visit_block(body);
        }
        Statement::RepeatUntil { body, condition } => {
            visitor.visit_block(body);
            visitor.visit_expression(condition);
        }
        Statement::NumericFor {
            start,
            end,
            step,
            body,
            ..
        } => {
            visitor.visit_expression(start);
            visitor.visit_expression(end);
            if let Some(step) = step {
                visitor.visit_expression(step);
            }
            visitor.visit_block(body);
        }
        Statement::GenericFor {
            iterators, body, ..
        } => {
            for iterator in iterators {
                visitor.visit_expression(iterator);
            }
            visitor.visit_block(body);
        }
        Statement::Return(values) => {
            for value in values {
                visitor.visit_expression(value);
            }
        }
        Statement::Expression(expression) => visitor.visit_expression(expression),
    }
}

pub fn walk_assignable<V: AstVisitor + ?Sized>(visitor: &mut V, assignable: &Assignable) {
    match assignable {
        Assignable::Identifier(_) => {}
        Assignable::Index { table, index } => {
            visitor.visit_expression(table);
            visitor.visit_expression(index);
        }
        Assignable::Member { table, .. } => visitor.visit_expression(table),
    }
}

pub fn walk_expression<V: AstVisitor + ?Sized>(visitor: &mut V, expression: &Expression) {
    match expression {
        Expression::Nil
        | Expression::Boolean(_)
        | Expression::Number(_)
        | Expression::String(_)
        | Expression::Identifier(_)
        | Expression::VarArg => {}
        Expression::Binary { left, right, .. } => {
            visitor.visit_expression(left);
            visitor.visit_expression(right);
        }
        Expression::Unary { expression, .. } => visitor.visit_expression(expression),
        Expression::TableConstructor(fields) => {
            for field in fields {
                walk_table_field(visitor, field);
            }
        }
        Expression::Index { table, index } => {
            visitor.visit_expression(table);
            visitor.visit_expression(index);
        }
        Expression::Member { table, .. } => visitor.visit_expression(table),
        Expression::FunctionCall { callee, args } => {
            visitor.visit_expression(callee);
            for argument in args {
                visitor.visit_expression(argument);
            }
        }
        Expression::AnonymousFunction(function) => walk_function(visitor, function),
    }
}

pub fn walk_function<V: AstVisitor + ?Sized>(visitor: &mut V, function: &FunctionExpression) {
    visitor.visit_block(&function.body);
}

pub fn walk_table_field<V: AstVisitor + ?Sized>(visitor: &mut V, field: &TableField) {
    if let Some(key) = &field.key {
        visitor.visit_expression(key);
    }
    visitor.visit_expression(&field.value);
}
