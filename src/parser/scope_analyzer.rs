use std::collections::{BTreeSet, HashMap};

use crate::error::CompileError;
use crate::parser::ast_loader::{
    Assignable, Block, Expression, FunctionExpression, Program, Statement,
};

#[derive(Clone, Debug, Default)]
pub struct FunctionScope {
    pub name: Option<String>,
    pub locals: BTreeSet<String>,
    pub globals: BTreeSet<String>,
    pub upvalues: BTreeSet<String>,
    pub parameters: Vec<String>,
    pub returns: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ScopeSummary {
    pub functions: Vec<FunctionScope>,
}

#[derive(Clone, Debug, Default)]
struct LexicalScope {
    names: BTreeSet<String>,
}

#[derive(Debug)]
struct ScopeAnalyzer {
    functions: Vec<FunctionScope>,
    lexical_scopes: Vec<LexicalScope>,
    function_stack: Vec<usize>,
    scope_to_function: Vec<usize>,
}

impl ScopeAnalyzer {
    fn new() -> Self {
        let mut analyzer = Self {
            functions: vec![FunctionScope::default()],
            lexical_scopes: Vec::new(),
            function_stack: vec![0],
            scope_to_function: Vec::new(),
        };
        analyzer.enter_scope();
        analyzer
    }

    fn analyze_program(mut self, program: &Program) -> Result<ScopeSummary, CompileError> {
        self.visit_block(&program.block)?;
        Ok(ScopeSummary {
            functions: self.functions,
        })
    }

    fn current_function_index(&self) -> usize {
        *self
            .function_stack
            .last()
            .expect("function stack is never empty")
    }

    fn enter_scope(&mut self) {
        self.lexical_scopes.push(LexicalScope::default());
        self.scope_to_function.push(self.current_function_index());
    }

    fn exit_scope(&mut self) {
        self.lexical_scopes.pop();
        self.scope_to_function.pop();
    }

    fn declare_local(&mut self, name: &str) {
        if let Some(scope) = self.lexical_scopes.last_mut() {
            scope.names.insert(name.to_string());
        }
        let function_index = self.current_function_index();
        self.functions[function_index]
            .locals
            .insert(name.to_string());
    }

    fn record_reference(&mut self, name: &str) {
        for (scope_index, scope) in self.lexical_scopes.iter().enumerate().rev() {
            if scope.names.contains(name) {
                let owner_function = self.scope_to_function[scope_index];
                let current_function = self.current_function_index();
                if owner_function != current_function {
                    self.functions[current_function]
                        .upvalues
                        .insert(name.to_string());
                }
                return;
            }
        }

        let function_index = self.current_function_index();
        self.functions[function_index]
            .globals
            .insert(name.to_string());
    }

    fn visit_block(&mut self, block: &Block) -> Result<(), CompileError> {
        for statement in &block.statements {
            self.visit_statement(statement)?;
        }
        Ok(())
    }

    fn visit_statement(&mut self, statement: &Statement) -> Result<(), CompileError> {
        match statement {
            Statement::LocalDeclaration { names, values } => {
                for value in values {
                    self.visit_expression(value)?;
                }
                for name in names {
                    self.declare_local(name);
                }
            }
            Statement::Assignment { targets, values } => {
                for target in targets {
                    self.visit_assignable(target)?;
                }
                for value in values {
                    self.visit_expression(value)?;
                }
            }
            Statement::FunctionDeclaration {
                name,
                function,
                is_local,
            } => {
                if *is_local {
                    self.declare_local(name);
                } else {
                    self.record_reference(name);
                }
                self.visit_function(Some(name.clone()), function)?;
            }
            Statement::If {
                clauses,
                else_block,
            } => {
                for clause in clauses {
                    self.visit_expression(&clause.condition)?;
                    self.enter_scope();
                    self.visit_block(&clause.block)?;
                    self.exit_scope();
                }
                if let Some(block) = else_block {
                    self.enter_scope();
                    self.visit_block(block)?;
                    self.exit_scope();
                }
            }
            Statement::While { condition, body } => {
                self.visit_expression(condition)?;
                self.enter_scope();
                self.visit_block(body)?;
                self.exit_scope();
            }
            Statement::RepeatUntil { body, condition } => {
                self.enter_scope();
                self.visit_block(body)?;
                self.visit_expression(condition)?;
                self.exit_scope();
            }
            Statement::NumericFor {
                name,
                start,
                end,
                step,
                body,
            } => {
                self.visit_expression(start)?;
                self.visit_expression(end)?;
                if let Some(step) = step {
                    self.visit_expression(step)?;
                }
                self.enter_scope();
                self.declare_local(name);
                self.visit_block(body)?;
                self.exit_scope();
            }
            Statement::GenericFor {
                names,
                iterators,
                body,
            } => {
                for iterator in iterators {
                    self.visit_expression(iterator)?;
                }
                self.enter_scope();
                for name in names {
                    self.declare_local(name);
                }
                self.visit_block(body)?;
                self.exit_scope();
            }
            Statement::Return(values) => {
                let function_index = self.current_function_index();
                self.functions[function_index].returns += 1;
                for value in values {
                    self.visit_expression(value)?;
                }
            }
            Statement::Expression(expression) => self.visit_expression(expression)?,
        }
        Ok(())
    }

    fn visit_assignable(&mut self, assignable: &Assignable) -> Result<(), CompileError> {
        match assignable {
            Assignable::Identifier(name) => self.record_reference(name),
            Assignable::Index { table, index } => {
                self.visit_expression(table)?;
                self.visit_expression(index)?;
            }
            Assignable::Member { table, .. } => {
                self.visit_expression(table)?;
            }
        }
        Ok(())
    }

    fn visit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Nil
            | Expression::Boolean(_)
            | Expression::Number(_)
            | Expression::String(_)
            | Expression::VarArg => {}
            Expression::Identifier(name) => self.record_reference(name),
            Expression::Binary { left, right, .. } => {
                self.visit_expression(left)?;
                self.visit_expression(right)?;
            }
            Expression::Unary { expression, .. } => self.visit_expression(expression)?,
            Expression::TableConstructor(fields) => {
                for field in fields {
                    if let Some(key) = &field.key {
                        self.visit_expression(key)?;
                    }
                    self.visit_expression(&field.value)?;
                }
            }
            Expression::Index { table, index } => {
                self.visit_expression(table)?;
                self.visit_expression(index)?;
            }
            Expression::Member { table, .. } => self.visit_expression(table)?,
            Expression::FunctionCall { callee, args } => {
                self.visit_expression(callee)?;
                for argument in args {
                    self.visit_expression(argument)?;
                }
            }
            Expression::AnonymousFunction(function) => self.visit_function(None, function)?,
        }
        Ok(())
    }

    fn visit_function(
        &mut self,
        name: Option<String>,
        function: &FunctionExpression,
    ) -> Result<(), CompileError> {
        let index = self.functions.len();
        self.functions.push(FunctionScope {
            name,
            parameters: function.params.clone(),
            ..FunctionScope::default()
        });
        self.function_stack.push(index);
        self.enter_scope();
        for parameter in &function.params {
            self.declare_local(parameter);
        }
        self.visit_block(&function.body)?;
        self.exit_scope();
        self.function_stack.pop();
        Ok(())
    }
}

pub fn analyze_program(program: &Program) -> Result<ScopeSummary, CompileError> {
    ScopeAnalyzer::new().analyze_program(program)
}

pub fn summarize_globals(summary: &ScopeSummary) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for function in &summary.functions {
        for global in &function.globals {
            *counts.entry(global.clone()).or_insert(0) += 1;
        }
    }
    counts
}
