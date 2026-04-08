pub mod ast_loader;
pub mod ast_walker;
pub mod scope_analyzer;

pub use ast_loader::{
    Assignable, AstBackend, BinaryOperator, Block, Expression, FunctionExpression, IfClause,
    MockLuauBackend, Program, Statement, TableField, UnaryOperator,
};
pub use scope_analyzer::{FunctionScope, ScopeSummary};
