//! Tokens de Dart 3.6 com intervalos em bytes e sem empréstimo da fonte.
//!
//! O contrato entre lexer e parser. Decisões que o parser depende:
//!
//! * Palavras **reservadas** (`class`, `if`, `is`…) viram [`Kind::Keyword`];
//!   identificadores embutidos (`abstract`, `dynamic`, `get`…) e contextuais
//!   (`async`, `show`, `when`…) viram [`Kind::Ident`] e o parser decide pelo
//!   texto, porque em Dart eles são identificadores válidos em quase todo lugar.
//! * `>` é sempre emitido **sozinho** ([`Op::Gt`]) com a marca [`Token::glued`]
//!   indicando que o próximo token começa no byte seguinte. É o parser que
//!   compõe `>>`, `>>>`, `>=`, `>>=` e `>>>=` em contexto de expressão; assim
//!   `List<List<int>>` nunca precisa de retrocesso.
//! * Strings interpoladas viram uma sequência: [`Kind::StrBegin`], os tokens da
//!   expressão, [`Kind::StrMid`]… e [`Kind::StrEnd`]. Cada trecho carrega
//!   [`Interp`] dizendo como a interpolação seguinte foi escrita — `$nome`
//!   (então o próximo token é exatamente um [`Kind::Ident`]) ou `${expr}` (o
//!   lexer devolve o trecho seguinte ao encontrar a `}` correspondente).
//!   Uma string sem interpolação é um único [`Kind::Str`].
//! * O texto de identificadores e literais **não** é copiado: o parser lê
//!   `&source[span]`; o lexer interna identificadores em `SymbolId` só quando
//!   o parser pede.
use dartforge_diagnostics::Span;

/// Palavras reservadas de Dart 3.6 (§17.1 da especificação), que nunca são
/// identificadores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Assert,
    Break,
    Case,
    Catch,
    Class,
    Const,
    Continue,
    Default,
    Do,
    Else,
    Enum,
    Extends,
    False,
    Final,
    Finally,
    For,
    If,
    In,
    Is,
    New,
    Null,
    Rethrow,
    Return,
    Super,
    Switch,
    This,
    Throw,
    True,
    Try,
    Var,
    Void,
    While,
    With,
}

impl Keyword {
    /// Reconhece uma palavra reservada pelo texto; `None` para qualquer outra.
    ///
    /// ```
    /// use dartforge_frontend::token::Keyword;
    /// assert_eq!(Keyword::from_text("class"), Some(Keyword::Class));
    /// assert_eq!(Keyword::from_text("abstract"), None);
    /// ```
    pub fn from_text(text: &str) -> Option<Keyword> {
        Some(match text {
            "assert" => Keyword::Assert,
            "break" => Keyword::Break,
            "case" => Keyword::Case,
            "catch" => Keyword::Catch,
            "class" => Keyword::Class,
            "const" => Keyword::Const,
            "continue" => Keyword::Continue,
            "default" => Keyword::Default,
            "do" => Keyword::Do,
            "else" => Keyword::Else,
            "enum" => Keyword::Enum,
            "extends" => Keyword::Extends,
            "false" => Keyword::False,
            "final" => Keyword::Final,
            "finally" => Keyword::Finally,
            "for" => Keyword::For,
            "if" => Keyword::If,
            "in" => Keyword::In,
            "is" => Keyword::Is,
            "new" => Keyword::New,
            "null" => Keyword::Null,
            "rethrow" => Keyword::Rethrow,
            "return" => Keyword::Return,
            "super" => Keyword::Super,
            "switch" => Keyword::Switch,
            "this" => Keyword::This,
            "throw" => Keyword::Throw,
            "true" => Keyword::True,
            "try" => Keyword::Try,
            "var" => Keyword::Var,
            "void" => Keyword::Void,
            "while" => Keyword::While,
            "with" => Keyword::With,
            _ => return None,
        })
    }

    /// Texto da palavra reservada, para mensagens.
    pub fn text(self) -> &'static str {
        match self {
            Keyword::Assert => "assert",
            Keyword::Break => "break",
            Keyword::Case => "case",
            Keyword::Catch => "catch",
            Keyword::Class => "class",
            Keyword::Const => "const",
            Keyword::Continue => "continue",
            Keyword::Default => "default",
            Keyword::Do => "do",
            Keyword::Else => "else",
            Keyword::Enum => "enum",
            Keyword::Extends => "extends",
            Keyword::False => "false",
            Keyword::Final => "final",
            Keyword::Finally => "finally",
            Keyword::For => "for",
            Keyword::If => "if",
            Keyword::In => "in",
            Keyword::Is => "is",
            Keyword::New => "new",
            Keyword::Null => "null",
            Keyword::Rethrow => "rethrow",
            Keyword::Return => "return",
            Keyword::Super => "super",
            Keyword::Switch => "switch",
            Keyword::This => "this",
            Keyword::Throw => "throw",
            Keyword::True => "true",
            Keyword::Try => "try",
            Keyword::Var => "var",
            Keyword::Void => "void",
            Keyword::While => "while",
            Keyword::With => "with",
        }
    }
}

/// Pontuação e operadores. `>` nunca se combina no lexer (ver módulo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `?..`
    QuestionDotDot,
    /// `?.`
    QuestionDot,
    /// `...`
    Ellipsis,
    /// `...?`
    EllipsisQuestion,
    /// `:`
    Colon,
    /// `?`
    Question,
    /// `!`
    Bang,
    /// `~`
    Tilde,
    /// `@`
    At,
    /// `#`
    Hash,
    /// `=>`
    Arrow,
    /// `=`
    Assign,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<`
    Lt,
    /// `>` — sempre isolado; ver [`Token::glued`].
    Gt,
    /// `<=`
    LtEq,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `~/`
    TildeSlash,
    /// `%`
    Percent,
    /// `++`
    PlusPlus,
    /// `--`
    MinusMinus,
    /// `<<`
    LtLt,
    /// `&`
    Amp,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `??`
    QuestionQuestion,
    /// `+=`
    PlusAssign,
    /// `-=`
    MinusAssign,
    /// `*=`
    StarAssign,
    /// `/=`
    SlashAssign,
    /// `~/=`
    TildeSlashAssign,
    /// `%=`
    PercentAssign,
    /// `<<=`
    LtLtAssign,
    /// `&=`
    AmpAssign,
    /// `|=`
    PipeAssign,
    /// `^=`
    CaretAssign,
    /// `??=`
    QuestionQuestionAssign,
}

impl Op {
    /// Texto do operador, para mensagens.
    pub fn text(self) -> &'static str {
        match self {
            Op::LParen => "(",
            Op::RParen => ")",
            Op::LBracket => "[",
            Op::RBracket => "]",
            Op::LBrace => "{",
            Op::RBrace => "}",
            Op::Comma => ",",
            Op::Semicolon => ";",
            Op::Dot => ".",
            Op::DotDot => "..",
            Op::QuestionDotDot => "?..",
            Op::QuestionDot => "?.",
            Op::Ellipsis => "...",
            Op::EllipsisQuestion => "...?",
            Op::Colon => ":",
            Op::Question => "?",
            Op::Bang => "!",
            Op::Tilde => "~",
            Op::At => "@",
            Op::Hash => "#",
            Op::Arrow => "=>",
            Op::Assign => "=",
            Op::EqEq => "==",
            Op::BangEq => "!=",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::LtEq => "<=",
            Op::Plus => "+",
            Op::Minus => "-",
            Op::Star => "*",
            Op::Slash => "/",
            Op::TildeSlash => "~/",
            Op::Percent => "%",
            Op::PlusPlus => "++",
            Op::MinusMinus => "--",
            Op::LtLt => "<<",
            Op::Amp => "&",
            Op::Pipe => "|",
            Op::Caret => "^",
            Op::AmpAmp => "&&",
            Op::PipePipe => "||",
            Op::QuestionQuestion => "??",
            Op::PlusAssign => "+=",
            Op::MinusAssign => "-=",
            Op::StarAssign => "*=",
            Op::SlashAssign => "/=",
            Op::TildeSlashAssign => "~/=",
            Op::PercentAssign => "%=",
            Op::LtLtAssign => "<<=",
            Op::AmpAssign => "&=",
            Op::PipeAssign => "|=",
            Op::CaretAssign => "^=",
            Op::QuestionQuestionAssign => "??=",
        }
    }
}

/// Como a interpolação que segue um trecho de string foi escrita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interp {
    /// `$nome`: o próximo token é um único [`Kind::Ident`] e depois vem o
    /// trecho seguinte da string.
    Ident,
    /// `${expr}`: os tokens seguintes são a expressão; o trecho seguinte da
    /// string chega quando o lexer encontra a `}` correspondente.
    Brace,
}

/// Forma do literal de string, necessária para decodificar o conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrFlags {
    /// `r'...'`: sem escapes nem interpolação.
    pub raw: bool,
    /// Aspas triplas: aceita quebras de linha e ignora a primeira quebra
    /// imediatamente após a abertura.
    pub triple: bool,
    /// Aspa usada: `'` ou `"`.
    pub quote: u8,
}

/// Classe do token. Texto de identificadores e literais vem de `&source[span]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Identificador ou identificador embutido/contextual (`abstract`, `async`…).
    Ident,
    /// Palavra reservada.
    Keyword(Keyword),
    /// Literal inteiro decimal ou hexadecimal (`0x1F`).
    Int,
    /// Literal double (`1.5`, `1e3`, `.5` **não** existe em Dart).
    Double,
    /// String inteira sem interpolação; o span inclui as aspas (e o `r`).
    Str(StrFlags),
    /// Abertura de string interpolada: da aspa inicial até o `$`/`${`.
    StrBegin(StrFlags, Interp),
    /// Trecho entre duas interpolações: da `}`/fim do nome até o `$` seguinte.
    StrMid(StrFlags, Interp),
    /// Trecho final: da `}`/fim do nome até a aspa de fechamento.
    StrEnd(StrFlags),
    /// Pontuação e operadores.
    Op(Op),
    /// Tag `#!...` na primeira linha do arquivo.
    ScriptTag,
    /// Fim do arquivo; sempre o último token, com span vazio.
    Eof,
}

/// Token com posição em bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
    /// O próximo token começa exatamente em `span.end` (sem espaço nem
    /// comentário entre eles). O parser usa isso para compor `>>`, `>=`,
    /// `>>>=` a partir de [`Op::Gt`] isolados.
    pub glued: bool,
}

impl Token {
    /// Texto do token na fonte de onde veio.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.start..self.span.end]
    }
}
