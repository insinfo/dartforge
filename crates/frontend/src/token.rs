//! Dart 3.6 tokens with byte ranges and no borrowing from the source.
//!
//! The contract between lexer and parser. Decisions the parser relies on:
//!
//! * **Reserved** words (`class`, `if`, `is`…) become [`Kind::Keyword`];
//!   built-in identifiers (`abstract`, `dynamic`, `get`…) and contextual ones
//!   (`async`, `show`, `when`…) become [`Kind::Ident`] and the parser decides
//!   by the text, because in Dart they are valid identifiers almost
//!   everywhere.
//! * `>` is always emitted **alone** ([`Op::Gt`]) with the [`Token::glued`]
//!   flag saying the next token starts at the following byte. The parser
//!   composes `>>`, `>>>`, `>=`, `>>=` and `>>>=` in expression context;
//!   that way `List<List<int>>` never needs backtracking.
//! * Interpolated strings become a sequence: [`Kind::StrBegin`], the
//!   expression tokens, [`Kind::StrMid`]… and [`Kind::StrEnd`]. Each piece
//!   carries an [`Interp`] saying how the following interpolation was
//!   written — `$name` (then the next token is exactly one [`Kind::Ident`])
//!   or `${expr}` (the lexer returns the following piece when it meets the
//!   matching `}`). A string without interpolation is a single [`Kind::Str`].
//! * The text of identifiers and literals is **not** copied: the parser
//!   reads `&source[span]`; the lexer interns identifiers into `SymbolId`
//!   only when the parser asks.
use dartforge_diagnostics::Span;

/// Dart 3.6 reserved words (§17.1 of the specification), which are never
/// identifiers.
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
    /// Recognizes a reserved word by its text; `None` for anything else.
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

    /// Text of the reserved word, for messages.
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

/// Punctuation and operators. `>` never combines in the lexer (see the
/// module docs).
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
    /// `>` — always isolated; see [`Token::glued`].
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
    /// Text of the operator, for messages.
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

/// How the interpolation that follows a string piece was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interp {
    /// `$name`: the next token is a single [`Kind::Ident`] and then comes the
    /// next piece of the string.
    Ident,
    /// `${expr}`: the following tokens are the expression; the next piece of
    /// the string arrives when the lexer meets the matching `}`.
    Brace,
}

/// Shape of the string literal, needed to decode its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrFlags {
    /// `r'...'`: no escapes or interpolation.
    pub raw: bool,
    /// Triple quotes: accepts line breaks and ignores the first line break
    /// right after the opening.
    pub triple: bool,
    /// Quote used: `'` or `"`.
    pub quote: u8,
    /// The piece ends without the closing quote (a line break in a simple
    /// string, end of file): fasta scanner's `unterminatedString` inserts the
    /// synthetic closer and goes on, and the error
    /// (`UNTERMINATED_STRING_LITERAL`) comes from this token
    /// (`parser::recoverable_scanner_errors`). Only the last piece has it.
    pub aberta: bool,
}

/// Token class. The text of identifiers and literals comes from
/// `&source[span]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Identifier or built-in/contextual identifier (`abstract`, `async`…).
    Ident,
    /// Reserved word.
    Keyword(Keyword),
    /// Decimal or hexadecimal integer literal (`0x1F`).
    Int,
    /// Double literal (`1.5`, `1e3`; `.5` does **not** exist in Dart).
    Double,
    /// Whole string without interpolation; the span includes the quotes (and
    /// the `r`).
    Str(StrFlags),
    /// Opening of an interpolated string: from the opening quote up to the
    /// `$`/`${`.
    StrBegin(StrFlags, Interp),
    /// Piece between two interpolations: from the `}`/end of the name up to the
    /// next `$`.
    StrMid(StrFlags, Interp),
    /// Final piece: from the `}`/end of the name up to the closing quote.
    StrEnd(StrFlags),
    /// Punctuation and operators.
    Op(Op),
    /// `#!...` tag on the first line of the file.
    ScriptTag,
    /// End of file; always the last token, with an empty span.
    Eof,
}

/// Token with a byte position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
    /// The next token starts exactly at `span.end` (no whitespace or comment
    /// between them). The parser uses this to compose `>>`, `>=`, `>>>=` from
    /// isolated [`Op::Gt`].
    pub glued: bool,
}

impl Token {
    /// Text of the token in the source it came from.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.start..self.span.end]
    }
}
