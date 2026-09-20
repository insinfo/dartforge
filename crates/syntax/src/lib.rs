//! Syntax for the explicitly supported Dart 3.6.2 subset.
use dartforge_diagnostics::Span;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind<'a> {
    Word(&'a str),
    String(&'a str),
    Number(&'a str),
    Symbol(char),
    Operator(&'a str),
}
#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Void,
    Int,
    String,
    Bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}
#[derive(Debug)]
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}
#[derive(Debug)]
pub enum ExprKind<'a> {
    Int(i32),
    String(&'a str),
    Bool(bool),
    Identifier(&'a str),
    Call {
        name: &'a str,
        arguments: Vec<Expr<'a>>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr<'a>>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr<'a>>,
        right: Box<Expr<'a>>,
    },
}
#[derive(Debug)]
pub struct Statement<'a> {
    pub kind: StatementKind<'a>,
    pub span: Span,
}
#[derive(Debug)]
pub enum StatementKind<'a> {
    Variable {
        name: &'a str,
        annotation: Option<Type>,
        is_final: bool,
        initializer: Expr<'a>,
    },
    Assign {
        name: &'a str,
        value: Expr<'a>,
    },
    Print(Expr<'a>),
    Return(Option<Expr<'a>>),
    Expression(Expr<'a>),
    If {
        condition: Expr<'a>,
        then_body: Vec<Statement<'a>>,
        else_body: Option<Vec<Statement<'a>>>,
    },
    Block(Vec<Statement<'a>>),
}
#[derive(Debug)]
pub struct Parameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug)]
pub struct Function<'a> {
    pub name: &'a str,
    pub return_type: Type,
    pub parameters: Vec<Parameter<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Program<'a> {
    pub functions: Vec<Function<'a>>,
    pub statements: Vec<Statement<'a>>,
}
