//! Tokens e árvore sintática do subconjunto suportado de Dart 3.6.2.
use dartforge_diagnostics::Span;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Categoria de token que empresta lexemas do texto original.
pub enum TokenKind<'a> {
    Word(&'a str),
    String(&'a str),
    /// Conteúdo de uma string raw, sem interpretação de escapes.
    RawString(&'a str),
    Number(&'a str),
    Symbol(char),
    Operator(&'a str),
}
#[derive(Debug, Clone, Copy)]
/// Token com localização em bytes no arquivo de origem.
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Tipo primitivo ou ausência de valor reconhecido neste subconjunto.
pub enum Type {
    Void,
    /// Tipo nominal identificado pela posição da declaração de classe.
    Class(u32),
    /// Tipo nominal que também aceita null.
    NullableClass(u32),
    Null,
    NullableInt,
    NullableString,
    NullableBool,
    Int,
    String,
    Bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Operação binária preservada na árvore de expressões.
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
    /// Seleciona o operando direito somente quando o esquerdo é null.
    IfNull,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Operação prefixa ou asserção pós-fixa com um único operando.
pub enum UnaryOp {
    Negate,
    Not,
    /// Asserção pós-fixa de valor não nulo.
    NullAssert,
}
#[derive(Debug)]
/// Expressão acompanhada do intervalo de origem.
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}
#[derive(Debug)]
/// Forma sintática de uma expressão.
pub enum ExprKind<'a> {
    This,
    Construct {
        class_id: u32,
    },
    Member {
        receiver: Box<Expr<'a>>,
        name: &'a str,
    },
    MethodCall {
        receiver: Box<Expr<'a>>,
        name: &'a str,
        arguments: Vec<Expr<'a>>,
    },
    Null,
    Int(i32),
    String(&'a str),
    /// String que precisou de alocação para decodificar escapes Unicode.
    OwnedString(String),
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
/// Instrução acompanhada do intervalo de origem.
pub struct Statement<'a> {
    pub kind: StatementKind<'a>,
    pub span: Span,
}
#[derive(Debug)]
/// Forma sintática de uma instrução.
pub enum StatementKind<'a> {
    FieldAssign {
        receiver: Expr<'a>,
        name: &'a str,
        value: Expr<'a>,
    },
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
    /// Repete o corpo enquanto a condição booleana for verdadeira.
    While {
        condition: Expr<'a>,
        body: Vec<Statement<'a>>,
    },
    /// Executa o corpo antes de testar a condição booleana.
    DoWhile {
        body: Vec<Statement<'a>>,
        condition: Expr<'a>,
    },
    /// Laço clássico com inicialização, condição e atualização opcionais.
    For {
        initializer: Option<Box<Statement<'a>>>,
        condition: Option<Expr<'a>>,
        update: Option<Box<Statement<'a>>>,
        body: Vec<Statement<'a>>,
    },
    /// Encerra o laço mais próximo; rótulos ainda não são suportados.
    Break,
    /// Inicia a próxima iteração do laço mais próximo.
    Continue,
    Block(Vec<Statement<'a>>),
}
#[derive(Debug)]
/// Parâmetro posicional obrigatório com tipo explícito.
pub struct Parameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug)]
/// Função top-level com assinatura e corpo.
pub struct Function<'a> {
    pub name: &'a str,
    pub return_type: Type,
    pub parameters: Vec<Parameter<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}
#[derive(Debug)]
/// Programa com funções auxiliares e o corpo da entrada main.
pub struct Program<'a> {
    pub classes: Vec<Class<'a>>,
    pub functions: Vec<Function<'a>>,
    pub statements: Vec<Statement<'a>>,
}

/// Classe nominal com construtor implícito e herança simples.
#[derive(Debug)]
pub struct Class<'a> {
    pub id: u32,
    pub name: &'a str,
    pub superclass: Option<u32>,
    pub fields: Vec<Field<'a>>,
    pub methods: Vec<Function<'a>>,
    pub span: Span,
}
/// Campo tipado com inicialização obrigatória.
#[derive(Debug)]
pub struct Field<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub is_final: bool,
    pub initializer: Expr<'a>,
    pub span: Span,
}
