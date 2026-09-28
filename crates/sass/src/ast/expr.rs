use std::{iter::Iterator, sync::Arc};

use codemap::{Span, Spanned};

use crate::{
    color::Color,
    common::{BinaryOp, Brackets, Identifier, ListSeparator, QuoteKind, UnaryOp},
    unit::Unit,
    value::Number,
};

use super::{ArgumentInvocation, AstSupportsCondition, Interpolation, InterpolationPart};

/// Represented by the `if` function
#[derive(Debug, Clone)]
pub struct Ternary(pub ArgumentInvocation);

#[derive(Debug, Clone)]
pub struct ListExpr {
    pub elems: Vec<Spanned<AstExpr>>,
    pub separator: ListSeparator,
    pub brackets: Brackets,
}

#[derive(Debug, Clone)]
pub struct FunctionCallExpr {
    pub namespace: Option<Spanned<Identifier>>,
    pub name: Identifier,
    pub arguments: Arc<ArgumentInvocation>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InterpolatedFunction {
    pub name: Interpolation,
    pub arguments: ArgumentInvocation,
    pub span: Span,
}

#[derive(Debug, Clone, Default)]
pub struct AstSassMap(pub Vec<(Spanned<AstExpr>, AstExpr)>);

#[derive(Debug, Clone)]
pub struct BinaryOpExpr {
    pub lhs: AstExpr,
    pub op: BinaryOp,
    pub rhs: AstExpr,
    pub allows_slash: bool,
    pub span: Span,
    /// Os `span`s dos operandos: num cálculo, `+`/`-` precisam de espaço dos
    /// dois lados (`_checkWhitespaceAroundCalculationOperator`).
    pub lhs_span: Span,
    pub rhs_span: Span,
}

/// Uma condição do `if()` do CSS (`IfConditionExpression` do dart-sass).
#[derive(Debug, Clone)]
pub enum IfCondition {
    Paren(Box<IfCondition>),
    Not(Box<IfCondition>),
    /// Grupos ligados por `and` (`true`) ou `or` (`false`).
    Op(Vec<IfCondition>, bool),
    Function {
        name: Interpolation,
        args: Interpolation,
    },
    Sass(AstExpr, Span),
    Raw(Interpolation),
}

impl IfCondition {
    /// `isArbitrarySubstitution`.
    pub fn is_arbitrary_substitution(&self) -> bool {
        match self {
            IfCondition::Raw(..) => true,
            IfCondition::Function { name, .. } => {
                match name.as_plain().map(str::to_ascii_lowercase) {
                    Some(n) => matches!(n.as_str(), "if" | "var" | "attr") || n.starts_with("--"),
                    None => false,
                }
            }
            _ => false,
        }
    }

    /// `toInterpolation`.
    pub fn to_interpolation(&self) -> Result<Interpolation, Span> {
        Ok(match self {
            IfCondition::Paren(e) => {
                let mut b = Interpolation::new_plain("(".to_owned());
                b.add_interpolation(e.to_interpolation()?);
                b.add_char(')');
                b
            }
            IfCondition::Not(e) => {
                let mut b = Interpolation::new_plain("not ".to_owned());
                b.add_interpolation(e.to_interpolation()?);
                b
            }
            IfCondition::Op(es, and) => {
                let mut b = Interpolation::new();
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        b.add_string(if *and { " and " } else { " or " }.to_owned());
                    }
                    b.add_interpolation(e.to_interpolation()?);
                }
                b
            }
            IfCondition::Function { name, args } => {
                let mut b = name.clone();
                b.add_char('(');
                b.add_interpolation(args.clone());
                b.add_char(')');
                b
            }
            IfCondition::Sass(_, span) => return Err(*span),
            IfCondition::Raw(t) => t.clone(),
        })
    }
}

/// O `if()` do CSS (`IfExpression`): ramos com condição (ou `else`).
#[derive(Debug, Clone)]
pub struct CssIfExpr {
    pub branches: Vec<(Option<IfCondition>, AstExpr)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AstExpr {
    CssIf(Arc<CssIfExpr>),
    BinaryOp(Arc<BinaryOpExpr>),
    True,
    False,
    Color(Arc<Color>),
    FunctionCall(FunctionCallExpr),
    If(Arc<Ternary>),
    InterpolatedFunction(Arc<InterpolatedFunction>),
    List(ListExpr),
    Map(AstSassMap),
    Null,
    Number {
        n: Number,
        unit: Unit,
    },
    Paren(Arc<Self>),
    ParentSelector,
    String(StringExpr, Span),
    Supports(Arc<AstSupportsCondition>),
    /// A `CalculationInterpolation` do dart-sass 1.66 (modo 1.66): o texto
    /// cru, com as interpolações, de um argumento de cálculo ou de um grupo
    /// entre parênteses dentro dele que tem `#{}` no nível de cima
    /// (`_tryCalculationInterpolation`). Como operando sai entre parênteses.
    CalcInterp166(Interpolation, Span),
    UnaryOp(UnaryOp, Arc<Self>, Span),
    Variable {
        name: Spanned<Identifier>,
        namespace: Option<Spanned<Identifier>>,
    },
}

// todo: make quotes bool
// todo: track span inside
#[derive(Debug, Clone)]
pub struct StringExpr(pub Interpolation, pub QuoteKind);

impl StringExpr {
    fn quote_inner_text(
        text: &str,
        quote: char,
        buffer: &mut Interpolation,
        // default=false
        is_static: bool,
    ) {
        let mut chars = text.chars().peekable();
        while let Some(char) = chars.next() {
            if char == '\n' || char == '\r' {
                buffer.add_char('\\');
                buffer.add_char('a');
                if let Some(next) = chars.peek() {
                    if next.is_ascii_whitespace() || next.is_ascii_hexdigit() {
                        buffer.add_char(' ');
                    }
                }
            } else {
                if char == quote
                    || char == '\\'
                    || (is_static && char == '#' && chars.peek() == Some(&'{'))
                {
                    buffer.add_char('\\');
                }
                buffer.add_char(char);
            }
        }
    }

    fn best_quote<'a>(strings: impl Iterator<Item = &'a str>) -> char {
        let mut contains_double_quote = false;
        for s in strings {
            for c in s.chars() {
                if c == '\'' {
                    return '"';
                }
                if c == '"' {
                    contains_double_quote = true;
                }
            }
        }
        if contains_double_quote {
            '\''
        } else {
            '"'
        }
    }

    pub fn as_interpolation(self, is_static: bool) -> Interpolation {
        if self.1 == QuoteKind::None {
            return self.0;
        }

        let quote = Self::best_quote(self.0.contents.iter().filter_map(|c| match c {
            InterpolationPart::Expr(..) => None,
            InterpolationPart::String(text) => Some(text.as_str()),
        }));

        let mut buffer = Interpolation::new();
        buffer.add_char(quote);

        for value in self.0.contents {
            match value {
                InterpolationPart::Expr(e) => buffer.add_expr(e),
                InterpolationPart::String(text) => {
                    Self::quote_inner_text(&text, quote, &mut buffer, is_static);
                }
            }
        }

        buffer.add_char(quote);

        buffer
    }
}

impl AstExpr {
    pub fn is_variable(&self) -> bool {
        matches!(self, Self::Variable { .. })
    }

    pub fn is_slash_operand(&self) -> bool {
        match self {
            // dart-sass `_isSlashOperand`: número, qualquer chamada de função
            // (os cálculos também são chamadas) ou outra barra.
            Self::Number { .. } | Self::FunctionCall(..) => true,
            Self::BinaryOp(binop) => binop.allows_slash,
            _ => false,
        }
    }

    pub fn slash(left: Self, right: Self, span: Span) -> Self {
        Self::BinaryOp(Arc::new(BinaryOpExpr {
            lhs: left,
            op: BinaryOp::Div,
            rhs: right,
            allows_slash: true,
            span,
            lhs_span: span,
            rhs_span: span,
        }))
    }

    pub const fn span(self, span: Span) -> Spanned<Self> {
        Spanned { node: self, span }
    }
}
