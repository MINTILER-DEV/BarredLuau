use std::iter::Peekable;
use std::str::Chars;

use crate::error::CompileError;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    LocalDeclaration {
        names: Vec<String>,
        values: Vec<Expression>,
    },
    Assignment {
        targets: Vec<Assignable>,
        values: Vec<Expression>,
    },
    FunctionDeclaration {
        name: String,
        function: FunctionExpression,
        is_local: bool,
    },
    If {
        clauses: Vec<IfClause>,
        else_block: Option<Block>,
    },
    While {
        condition: Expression,
        body: Block,
    },
    RepeatUntil {
        body: Block,
        condition: Expression,
    },
    NumericFor {
        name: String,
        start: Expression,
        end: Expression,
        step: Option<Expression>,
        body: Block,
    },
    GenericFor {
        names: Vec<String>,
        iterators: Vec<Expression>,
        body: Block,
    },
    Return(Vec<Expression>),
    Expression(Expression),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfClause {
    pub condition: Expression,
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Assignable {
    Identifier(String),
    Index {
        table: Box<Expression>,
        index: Box<Expression>,
    },
    Member {
        table: Box<Expression>,
        member: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionExpression {
    pub params: Vec<String>,
    pub is_vararg: bool,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableField {
    pub key: Option<Expression>,
    pub value: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
    Identifier(String),
    VarArg,
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        expression: Box<Expression>,
    },
    TableConstructor(Vec<TableField>),
    Index {
        table: Box<Expression>,
        index: Box<Expression>,
    },
    Member {
        table: Box<Expression>,
        member: String,
    },
    FunctionCall {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },
    AnonymousFunction(FunctionExpression),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Not,
    Length,
}

pub trait AstBackend {
    fn parse(&self, source: &str) -> Result<Program, CompileError>;
    fn backend_name(&self) -> &'static str;
}

#[derive(Clone, Debug, Default)]
pub struct MockLuauBackend;

impl AstBackend for MockLuauBackend {
    fn parse(&self, source: &str) -> Result<Program, CompileError> {
        let lexer = Lexer::new(source);
        let tokens = lexer.lex()?;
        Parser::new(tokens).parse_program()
    }

    fn backend_name(&self) -> &'static str {
        "mock-luau"
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Identifier(String),
    Number(f64),
    String(String),
    Symbol(&'static str),
    Keyword(Keyword),
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Keyword {
    Local,
    Function,
    End,
    If,
    Then,
    ElseIf,
    Else,
    While,
    Do,
    Repeat,
    Until,
    For,
    In,
    Return,
    True,
    False,
    Nil,
    Not,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

impl Token {
    fn display(&self) -> String {
        match &self.kind {
            TokenKind::Identifier(value) => value.clone(),
            TokenKind::Number(value) => value.to_string(),
            TokenKind::String(_) => "<string>".to_string(),
            TokenKind::Symbol(symbol) => symbol.to_string(),
            TokenKind::Keyword(keyword) => format!("{keyword:?}"),
            TokenKind::Eof => "<eof>".to_string(),
        }
    }
}

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().peekable(),
            line: 1,
            column: 1,
        }
    }

    fn lex(mut self) -> Result<Vec<Token>, CompileError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let line = self.line;
            let column = self.column;
            let Some(ch) = self.peek_char() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    line,
                    column,
                });
                break;
            };

            let kind = if ch.is_ascii_alphabetic() || ch == '_' {
                self.lex_identifier_or_keyword()?
            } else if ch.is_ascii_digit() {
                self.lex_number()?
            } else if ch == '\'' || ch == '"' {
                self.lex_string()?
            } else {
                self.lex_symbol()?
            };

            tokens.push(Token { kind, line, column });
        }
        Ok(tokens)
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
                self.next_char();
            }

            let mut clone = self.chars.clone();
            if matches!(clone.next(), Some('-')) && matches!(clone.next(), Some('-')) {
                self.next_char();
                self.next_char();
                while !matches!(self.peek_char(), None | Some('\n')) {
                    self.next_char();
                }
                continue;
            }

            break;
        }
    }

    fn lex_identifier_or_keyword(&mut self) -> Result<TokenKind, CompileError> {
        let mut ident = String::new();
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_alphanumeric() || ch == '_') {
            ident.push(
                self.next_char()
                    .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?,
            );
        }

        let kind = match ident.as_str() {
            "local" => TokenKind::Keyword(Keyword::Local),
            "function" => TokenKind::Keyword(Keyword::Function),
            "end" => TokenKind::Keyword(Keyword::End),
            "if" => TokenKind::Keyword(Keyword::If),
            "then" => TokenKind::Keyword(Keyword::Then),
            "elseif" => TokenKind::Keyword(Keyword::ElseIf),
            "else" => TokenKind::Keyword(Keyword::Else),
            "while" => TokenKind::Keyword(Keyword::While),
            "do" => TokenKind::Keyword(Keyword::Do),
            "repeat" => TokenKind::Keyword(Keyword::Repeat),
            "until" => TokenKind::Keyword(Keyword::Until),
            "for" => TokenKind::Keyword(Keyword::For),
            "in" => TokenKind::Keyword(Keyword::In),
            "return" => TokenKind::Keyword(Keyword::Return),
            "true" => TokenKind::Keyword(Keyword::True),
            "false" => TokenKind::Keyword(Keyword::False),
            "nil" => TokenKind::Keyword(Keyword::Nil),
            "not" => TokenKind::Keyword(Keyword::Not),
            "and" => TokenKind::Keyword(Keyword::And),
            "or" => TokenKind::Keyword(Keyword::Or),
            _ => TokenKind::Identifier(ident),
        };
        Ok(kind)
    }

    fn lex_number(&mut self) -> Result<TokenKind, CompileError> {
        if matches!(self.peek_char(), Some('0')) {
            let mut clone = self.chars.clone();
            clone.next();
            match clone.next() {
                Some('x') | Some('X') => return self.lex_prefixed_number(16, 2, "hex"),
                Some('b') | Some('B') => return self.lex_prefixed_number(2, 2, "binary"),
                _ => {}
            }
        }

        let mut number = String::new();
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit() || ch == '.') {
            let Some(ch) = self.peek_char() else {
                break;
            };
            if ch == '.' {
                let mut clone = self.chars.clone();
                clone.next();
                if matches!(clone.next(), Some('.')) {
                    break;
                }
            }
            number.push(
                self.next_char()
                    .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?,
            );
        }
        let parsed = number.parse::<f64>().map_err(|error| {
            CompileError::Parse(format!("Invalid numeric literal `{number}`: {error}"))
        })?;
        Ok(TokenKind::Number(parsed))
    }

    fn lex_prefixed_number(
        &mut self,
        radix: u32,
        prefix_len: usize,
        kind: &str,
    ) -> Result<TokenKind, CompileError> {
        let mut literal = String::new();
        for _ in 0..prefix_len {
            literal.push(
                self.next_char()
                    .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?,
            );
        }

        let mut digits = String::new();
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_alphanumeric() || ch == '_') {
            let ch = self
                .next_char()
                .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?;
            literal.push(ch);
            if ch != '_' {
                digits.push(ch);
            }
        }

        if digits.is_empty() {
            return Err(CompileError::Parse(format!(
                "Invalid {kind} numeric literal `{literal}`"
            )));
        }

        let parsed = u64::from_str_radix(&digits, radix).map_err(|error| {
            CompileError::Parse(format!("Invalid numeric literal `{literal}`: {error}"))
        })?;
        Ok(TokenKind::Number(parsed as f64))
    }

    fn lex_string(&mut self) -> Result<TokenKind, CompileError> {
        let delimiter = self
            .next_char()
            .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?;
        let mut content = String::new();
        loop {
            let Some(ch) = self.next_char() else {
                return Err(CompileError::Parse(
                    "Unterminated string literal".to_string(),
                ));
            };
            if ch == delimiter {
                break;
            }
            if ch == '\\' {
                let escaped = self
                    .next_char()
                    .ok_or_else(|| CompileError::Parse("Unterminated string escape".to_string()))?;
                let mapped = match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '\'' => '\'',
                    '"' => '"',
                    other => other,
                };
                content.push(mapped);
            } else {
                content.push(ch);
            }
        }
        Ok(TokenKind::String(content))
    }

    fn lex_symbol(&mut self) -> Result<TokenKind, CompileError> {
        let current = self
            .next_char()
            .ok_or_else(|| CompileError::Parse("Unexpected EOF".to_string()))?;
        let next = self.peek_char();
        let symbol = match (current, next) {
            ('.', Some('.')) => {
                let mut clone = self.chars.clone();
                clone.next();
                if matches!(clone.next(), Some('.')) {
                    self.next_char();
                    self.next_char();
                    "..."
                } else if matches!(clone.next(), Some('=')) {
                    self.next_char();
                    self.next_char();
                    "..="
                } else {
                    self.next_char();
                    ".."
                }
            }
            ('=', Some('=')) => {
                self.next_char();
                "=="
            }
            ('~', Some('=')) => {
                self.next_char();
                "~="
            }
            ('+', Some('=')) => {
                self.next_char();
                "+="
            }
            ('-', Some('=')) => {
                self.next_char();
                "-="
            }
            ('*', Some('=')) => {
                self.next_char();
                "*="
            }
            ('/', Some('=')) => {
                self.next_char();
                "/="
            }
            ('%', Some('=')) => {
                self.next_char();
                "%="
            }
            ('^', Some('=')) => {
                self.next_char();
                "^="
            }
            ('<', Some('=')) => {
                self.next_char();
                "<="
            }
            ('>', Some('=')) => {
                self.next_char();
                ">="
            }
            ('(', _) => "(",
            (')', _) => ")",
            ('{', _) => "{",
            ('}', _) => "}",
            ('[', _) => "[",
            (']', _) => "]",
            (',', _) => ",",
            (';', _) => ";",
            ('.', _) => ".",
            ('=', _) => "=",
            ('+', _) => "+",
            ('-', _) => "-",
            ('*', _) => "*",
            ('/', _) => "/",
            ('%', _) => "%",
            ('^', _) => "^",
            ('#', _) => "#",
            ('<', _) => "<",
            ('>', _) => ">",
            (':', _) => ":",
            _ => {
                return Err(CompileError::Parse(format!(
                    "Unexpected symbol `{current}` at {}:{}",
                    self.line, self.column
                )));
            }
        };
        Ok(TokenKind::Symbol(symbol))
    }
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_program(mut self) -> Result<Program, CompileError> {
        let block = self.parse_block(&[])?;
        self.expect_eof()?;
        Ok(Program { block })
    }

    fn parse_block(&mut self, terminators: &[Keyword]) -> Result<Block, CompileError> {
        let mut statements = Vec::new();
        while !self.is_terminated_by(terminators) && !self.is_eof() {
            statements.push(self.parse_statement()?);
            self.consume_symbol(";");
        }
        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, CompileError> {
        if self.consume_keyword(Keyword::Local) {
            if self.consume_keyword(Keyword::Function) {
                return self.parse_function_declaration(true);
            }
            return self.parse_local_declaration();
        }
        if self.consume_keyword(Keyword::Function) {
            return self.parse_function_declaration(false);
        }
        if self.consume_keyword(Keyword::If) {
            return self.parse_if_statement();
        }
        if self.consume_keyword(Keyword::While) {
            return self.parse_while_statement();
        }
        if self.consume_keyword(Keyword::Repeat) {
            return self.parse_repeat_until_statement();
        }
        if self.consume_keyword(Keyword::For) {
            return self.parse_for_statement();
        }
        if self.consume_keyword(Keyword::Return) {
            let values = if self.is_terminated_by(&[
                Keyword::End,
                Keyword::Else,
                Keyword::ElseIf,
                Keyword::Until,
            ]) || self.matches_symbol(";")
                || self.is_eof()
            {
                Vec::new()
            } else {
                self.parse_expression_list()?
            };
            return Ok(Statement::Return(values));
        }

        self.parse_assignment_or_expression_statement()
    }

    fn parse_local_declaration(&mut self) -> Result<Statement, CompileError> {
        let names = self.parse_name_list()?;
        let values = if self.consume_symbol("=") {
            self.parse_expression_list()?
        } else {
            Vec::new()
        };
        Ok(Statement::LocalDeclaration { names, values })
    }

    fn parse_function_declaration(&mut self, is_local: bool) -> Result<Statement, CompileError> {
        let name = self.expect_identifier()?;
        if self.matches_symbol(".") || self.matches_symbol(":") {
            return Err(CompileError::UnsupportedSyntax {
                node: "function declaration".to_string(),
                detail: "dotted and method declarations are reserved for a later parser backend"
                    .to_string(),
            });
        }
        let function = self.parse_function_body()?;
        Ok(Statement::FunctionDeclaration {
            name,
            function,
            is_local,
        })
    }

    fn parse_function_body(&mut self) -> Result<FunctionExpression, CompileError> {
        self.expect_symbol("(")?;
        let mut params = Vec::new();
        let mut is_vararg = false;
        if !self.consume_symbol(")") {
            loop {
                if self.consume_symbol("...") {
                    is_vararg = true;
                    self.expect_symbol(")")?;
                    break;
                }
                params.push(self.expect_identifier()?);
                if self.consume_symbol(")") {
                    break;
                }
                self.expect_symbol(",")?;
            }
        }

        let body = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok(FunctionExpression {
            params,
            is_vararg,
            body,
        })
    }

    fn parse_if_statement(&mut self) -> Result<Statement, CompileError> {
        let condition = self.parse_expression(0)?;
        self.expect_keyword(Keyword::Then)?;
        let first_block = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::End])?;
        let mut clauses = vec![IfClause {
            condition,
            block: first_block,
        }];

        while self.consume_keyword(Keyword::ElseIf) {
            let condition = self.parse_expression(0)?;
            self.expect_keyword(Keyword::Then)?;
            let block = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::End])?;
            clauses.push(IfClause { condition, block });
        }

        let else_block = if self.consume_keyword(Keyword::Else) {
            Some(self.parse_block(&[Keyword::End])?)
        } else {
            None
        };
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::If {
            clauses,
            else_block,
        })
    }

    fn parse_while_statement(&mut self) -> Result<Statement, CompileError> {
        let condition = self.parse_expression(0)?;
        self.expect_keyword(Keyword::Do)?;
        let body = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::While { condition, body })
    }

    fn parse_repeat_until_statement(&mut self) -> Result<Statement, CompileError> {
        let body = self.parse_block(&[Keyword::Until])?;
        self.expect_keyword(Keyword::Until)?;
        let condition = self.parse_expression(0)?;
        Ok(Statement::RepeatUntil { body, condition })
    }

    fn parse_for_statement(&mut self) -> Result<Statement, CompileError> {
        let first_name = self.expect_identifier()?;
        if self.consume_symbol("=") {
            let start = self.parse_expression(0)?;
            self.expect_symbol(",")?;
            let end = self.parse_expression(0)?;
            let step = if self.consume_symbol(",") {
                Some(self.parse_expression(0)?)
            } else {
                None
            };
            self.expect_keyword(Keyword::Do)?;
            let body = self.parse_block(&[Keyword::End])?;
            self.expect_keyword(Keyword::End)?;
            return Ok(Statement::NumericFor {
                name: first_name,
                start,
                end,
                step,
                body,
            });
        }

        let mut names = vec![first_name];
        while self.consume_symbol(",") {
            names.push(self.expect_identifier()?);
        }
        self.expect_keyword(Keyword::In)?;
        let iterators = self.parse_expression_list()?;
        self.expect_keyword(Keyword::Do)?;
        let body = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok(Statement::GenericFor {
            names,
            iterators,
            body,
        })
    }

    fn parse_assignment_or_expression_statement(&mut self) -> Result<Statement, CompileError> {
        let expression = self.parse_prefix_expression()?;
        if self.matches_symbol("=") || self.matches_symbol(",") {
            let mut targets = vec![self.into_assignable(expression)?];
            while self.consume_symbol(",") {
                let target_expression = self.parse_prefix_expression()?;
                targets.push(self.into_assignable(target_expression)?);
            }
            self.expect_symbol("=")?;
            let values = self.parse_expression_list()?;
            return Ok(Statement::Assignment { targets, values });
        }
        if let Some(operator) = self.consume_compound_assignment_operator() {
            let target = self.into_assignable(expression.clone())?;
            let right = self.parse_expression(0)?;
            let left = self.assignable_to_expression(&target);
            return Ok(Statement::Assignment {
                targets: vec![target],
                values: vec![Expression::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                }],
            });
        }

        match expression {
            Expression::FunctionCall { .. } => Ok(Statement::Expression(expression)),
            _ => Err(CompileError::Parse(format!(
                "Expected assignment or function call, found {}",
                self.current().display()
            ))),
        }
    }

    fn parse_expression_list(&mut self) -> Result<Vec<Expression>, CompileError> {
        let mut values = vec![self.parse_expression(0)?];
        while self.consume_symbol(",") {
            values.push(self.parse_expression(0)?);
        }
        Ok(values)
    }

    fn parse_name_list(&mut self) -> Result<Vec<String>, CompileError> {
        let mut names = vec![self.expect_identifier()?];
        while self.consume_symbol(",") {
            names.push(self.expect_identifier()?);
        }
        Ok(names)
    }

    fn parse_expression(&mut self, min_precedence: u8) -> Result<Expression, CompileError> {
        let mut left = self.parse_unary_expression()?;
        while let Some((operator, precedence, right_associative)) = self.current_binary_operator() {
            if precedence < min_precedence {
                break;
            }
            self.advance();
            let next_precedence = if right_associative {
                precedence
            } else {
                precedence + 1
            };
            let right = self.parse_expression(next_precedence)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary_expression(&mut self) -> Result<Expression, CompileError> {
        if self.consume_keyword(Keyword::Not) {
            let expression = self.parse_expression(7)?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::Not,
                expression: Box::new(expression),
            });
        }
        if self.consume_symbol("-") {
            let expression = self.parse_expression(7)?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::Negate,
                expression: Box::new(expression),
            });
        }
        if self.consume_symbol("#") {
            let expression = self.parse_expression(7)?;
            return Ok(Expression::Unary {
                operator: UnaryOperator::Length,
                expression: Box::new(expression),
            });
        }
        self.parse_prefix_expression()
    }

    fn parse_prefix_expression(&mut self) -> Result<Expression, CompileError> {
        let mut expression = self.parse_primary_expression()?;
        loop {
            if self.consume_symbol(".") {
                let member = self.expect_identifier()?;
                expression = Expression::Member {
                    table: Box::new(expression),
                    member,
                };
                continue;
            }
            if self.consume_symbol("[") {
                let index = self.parse_expression(0)?;
                self.expect_symbol("]")?;
                expression = Expression::Index {
                    table: Box::new(expression),
                    index: Box::new(index),
                };
                continue;
            }
            if self.consume_symbol(":") {
                let receiver = expression;
                let member = self.expect_identifier()?;
                let mut args = self.parse_argument_list()?;
                let callee = Expression::Member {
                    table: Box::new(receiver.clone()),
                    member,
                };
                args.insert(0, receiver);
                expression = Expression::FunctionCall {
                    callee: Box::new(callee),
                    args,
                };
                continue;
            }
            if self.matches_symbol("(") {
                let args = self.parse_argument_list()?;
                expression = Expression::FunctionCall {
                    callee: Box::new(expression),
                    args,
                };
                continue;
            }
            break;
        }
        Ok(expression)
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expression>, CompileError> {
        self.expect_symbol("(")?;
        if self.consume_symbol(")") {
            return Ok(Vec::new());
        }
        let args = self.parse_expression_list()?;
        self.expect_symbol(")")?;
        Ok(args)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, CompileError> {
        match &self.current().kind {
            TokenKind::Keyword(Keyword::Nil) => {
                self.advance();
                Ok(Expression::Nil)
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression::Boolean(true))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression::Boolean(false))
            }
            TokenKind::Number(value) => {
                let value = *value;
                self.advance();
                Ok(Expression::Number(value))
            }
            TokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::String(value))
            }
            TokenKind::Identifier(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::Identifier(value))
            }
            TokenKind::Symbol("...") => {
                self.advance();
                Ok(Expression::VarArg)
            }
            TokenKind::Symbol("(") => {
                self.advance();
                let expression = self.parse_expression(0)?;
                self.expect_symbol(")")?;
                Ok(expression)
            }
            TokenKind::Symbol("{") => self.parse_table_constructor(),
            TokenKind::Keyword(Keyword::Function) => {
                self.advance();
                Ok(Expression::AnonymousFunction(self.parse_function_body()?))
            }
            _ => Err(CompileError::Parse(format!(
                "Unexpected token {} at {}:{}",
                self.current().display(),
                self.current().line,
                self.current().column
            ))),
        }
    }

    fn parse_table_constructor(&mut self) -> Result<Expression, CompileError> {
        self.expect_symbol("{")?;
        let mut fields = Vec::new();
        if self.consume_symbol("}") {
            return Ok(Expression::TableConstructor(fields));
        }

        loop {
            let field = if self.consume_symbol("[") {
                let key = self.parse_expression(0)?;
                self.expect_symbol("]")?;
                self.expect_symbol("=")?;
                let value = self.parse_expression(0)?;
                TableField {
                    key: Some(key),
                    value,
                }
            } else if matches!(&self.current().kind, TokenKind::Identifier(_))
                && self.peek_symbol("=")
            {
                let key = Expression::String(self.expect_identifier()?);
                self.expect_symbol("=")?;
                let value = self.parse_expression(0)?;
                TableField {
                    key: Some(key),
                    value,
                }
            } else {
                TableField {
                    key: None,
                    value: self.parse_expression(0)?,
                }
            };

            fields.push(field);
            if self.consume_symbol("}") {
                break;
            }
            if !self.consume_symbol(",") && !self.consume_symbol(";") {
                self.expect_symbol("}")?;
                break;
            }
            if self.consume_symbol("}") {
                break;
            }
        }

        Ok(Expression::TableConstructor(fields))
    }

    fn current_binary_operator(&self) -> Option<(BinaryOperator, u8, bool)> {
        match &self.current().kind {
            TokenKind::Symbol("+") => Some((BinaryOperator::Add, 5, false)),
            TokenKind::Symbol("-") => Some((BinaryOperator::Sub, 5, false)),
            TokenKind::Symbol("*") => Some((BinaryOperator::Mul, 6, false)),
            TokenKind::Symbol("/") => Some((BinaryOperator::Div, 6, false)),
            TokenKind::Symbol("%") => Some((BinaryOperator::Mod, 6, false)),
            TokenKind::Symbol("^") => Some((BinaryOperator::Pow, 8, true)),
            TokenKind::Symbol("..") => Some((BinaryOperator::Concat, 4, true)),
            TokenKind::Symbol("==") => Some((BinaryOperator::Eq, 3, false)),
            TokenKind::Symbol("~=") => Some((BinaryOperator::Ne, 3, false)),
            TokenKind::Symbol("<") => Some((BinaryOperator::Lt, 3, false)),
            TokenKind::Symbol("<=") => Some((BinaryOperator::Le, 3, false)),
            TokenKind::Symbol(">") => Some((BinaryOperator::Gt, 3, false)),
            TokenKind::Symbol(">=") => Some((BinaryOperator::Ge, 3, false)),
            TokenKind::Keyword(Keyword::And) => Some((BinaryOperator::And, 2, false)),
            TokenKind::Keyword(Keyword::Or) => Some((BinaryOperator::Or, 1, false)),
            _ => None,
        }
    }

    fn into_assignable(&self, expression: Expression) -> Result<Assignable, CompileError> {
        match expression {
            Expression::Identifier(name) => Ok(Assignable::Identifier(name)),
            Expression::Index { table, index } => Ok(Assignable::Index { table, index }),
            Expression::Member { table, member } => Ok(Assignable::Member { table, member }),
            _ => Err(CompileError::Parse(
                "Only identifiers and indexing expressions are assignable".to_string(),
            )),
        }
    }

    fn assignable_to_expression(&self, assignable: &Assignable) -> Expression {
        match assignable {
            Assignable::Identifier(name) => Expression::Identifier(name.clone()),
            Assignable::Index { table, index } => Expression::Index {
                table: table.clone(),
                index: index.clone(),
            },
            Assignable::Member { table, member } => Expression::Member {
                table: table.clone(),
                member: member.clone(),
            },
        }
    }

    fn consume_compound_assignment_operator(&mut self) -> Option<BinaryOperator> {
        let operator = match self.current().kind {
            TokenKind::Symbol("+=") => BinaryOperator::Add,
            TokenKind::Symbol("-=") => BinaryOperator::Sub,
            TokenKind::Symbol("*=") => BinaryOperator::Mul,
            TokenKind::Symbol("/=") => BinaryOperator::Div,
            TokenKind::Symbol("%=") => BinaryOperator::Mod,
            TokenKind::Symbol("^=") => BinaryOperator::Pow,
            TokenKind::Symbol("..=") => BinaryOperator::Concat,
            _ => return None,
        };
        self.advance();
        Some(operator)
    }

    fn peek_symbol(&self, symbol: &str) -> bool {
        matches!(
            self.tokens.get(self.position + 1).map(|token| &token.kind),
            Some(TokenKind::Symbol(value)) if *value == symbol
        )
    }

    fn expect_identifier(&mut self) -> Result<String, CompileError> {
        match &self.current().kind {
            TokenKind::Identifier(value) => {
                let value = value.clone();
                self.advance();
                Ok(value)
            }
            _ => Err(self.error_expected("identifier")),
        }
    }

    fn expect_symbol(&mut self, symbol: &str) -> Result<(), CompileError> {
        if self.consume_symbol(symbol) {
            Ok(())
        } else {
            Err(self.error_expected(symbol))
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), CompileError> {
        if self.consume_keyword(keyword.clone()) {
            Ok(())
        } else {
            Err(self.error_expected(&format!("{keyword:?}")))
        }
    }

    fn expect_eof(&self) -> Result<(), CompileError> {
        if self.is_eof() {
            Ok(())
        } else {
            Err(self.error_expected("end of file"))
        }
    }

    fn consume_symbol(&mut self, symbol: &str) -> bool {
        if self.matches_symbol(symbol) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, keyword: Keyword) -> bool {
        if self.matches_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn matches_symbol(&self, symbol: &str) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Symbol(value) if *value == symbol
        )
    }

    fn matches_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(value) if *value == keyword)
    }

    fn is_terminated_by(&self, keywords: &[Keyword]) -> bool {
        keywords
            .iter()
            .any(|keyword| self.matches_keyword(keyword.clone()))
    }

    fn is_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn advance(&mut self) {
        if !self.is_eof() {
            self.position += 1;
        }
    }

    fn error_expected(&self, expected: &str) -> CompileError {
        CompileError::Parse(format!(
            "Expected {expected}, found {} at {}:{}",
            self.current().display(),
            self.current().line,
            self.current().column
        ))
    }
}
