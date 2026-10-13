//! Port of the csslib 1.0.2 tree (`src/tree.dart`, `src/tree_base.dart`): the
//! nodes that the parser builds and that the visitor and the printer walk.
//!
//! Dart's class hierarchies become enums; the official `is` tests become the
//! `is_*` methods. What Dart shares by reference and the shim modifies — the
//! `SimpleSelectorSequence`, whose combinator the `_CompoundSelector` rewrites
//! — is `Rc` with `Cell`, so that the same sequence seen from two places
//! changes in both, as in the official code. Simple selectors are immutable and
//! shared as well (`SimpleSelectorSequence.clone` does not copy them).
//!
//! Of the positions (`span`), only what changes the output is kept: the text of
//! the `Identifier` (its `toString` returns the original lexeme, with escapes)
//! and whether a simple selector has a position (the shim reads `span!` when it
//! warns).
use std::cell::Cell;
use std::rc::Rc;

use super::token_kind as tk;

/// `Identifier`: the name (without escapes) and the text of the span it came
/// from.
#[derive(Clone, Debug)]
pub struct Identifier {
    pub name: String,
    /// `span?.text`; `None` for the ones created by the shim.
    pub text: Option<String>,
}

impl Identifier {
    /// `Identifier(name, null)`.
    pub fn without_span(name: &str) -> Self {
        Identifier {
            name: name.to_string(),
            text: None,
        }
    }

    /// `Identifier.toString`: the lexeme, or the name.
    pub fn as_text(&self) -> &str {
        self.text.as_deref().unwrap_or(&self.name)
    }
}

/// The dynamic `_name` of a `SimpleSelector`: `Identifier`, `Wildcard`,
/// `ThisOperator`, `Negation` or, in the `NamespaceSelector`, the
/// `ElementSelector`.
#[derive(Debug)]
pub enum SimpleName {
    Ident(Identifier),
    /// `Wildcard`.
    Wildcard,
    /// `ThisOperator`.
    This,
    /// `Negation`.
    Negation,
    Selector(Rc<SimpleSelector>),
}

impl SimpleName {
    /// `_name.name`.
    pub fn name(&self) -> &str {
        match self {
            SimpleName::Ident(i) => &i.name,
            SimpleName::Wildcard => "*",
            SimpleName::This => "&",
            SimpleName::Negation => "not",
            SimpleName::Selector(s) => s.name(),
        }
    }
}

/// The value of an `AttributeSelector`: `Identifier` or `String`.
#[derive(Debug)]
pub enum AttributeValue {
    Ident(Identifier),
    Str(String),
}

/// The argument of `PseudoClassFunctionSelector`: `Selector` or
/// `SelectorExpression`.
#[derive(Debug)]
pub enum PseudoArgument {
    Selector(Rc<Selector>),
    Expression(Vec<Expr>),
}

/// The subclasses of `SimpleSelector`.
#[derive(Debug)]
pub enum SimpleKind {
    /// `ElementSelector`.
    Element,
    /// `NamespaceSelector`: `_namespace` (`null`, `Wildcard` or `Identifier`);
    /// the element goes in `_name`.
    Namespace { namespace: Option<SimpleName> },
    /// `AttributeSelector`.
    Attribute {
        op: i32,
        value: Option<AttributeValue>,
    },
    /// `IdSelector`.
    Id,
    /// `ClassSelector`.
    Class,
    /// `PseudoClassSelector`.
    PseudoClass,
    /// `PseudoElementSelector`.
    PseudoElement { legacy: bool },
    /// `PseudoClassFunctionSelector`.
    PseudoClassFunction(PseudoArgument),
    /// `PseudoElementFunctionSelector`.
    PseudoElementFunction(Vec<Expr>),
    /// `NegationSelector`.
    Negation(Option<Rc<SimpleSelector>>),
}

/// `SimpleSelector`.
#[derive(Debug)]
pub struct SimpleSelector {
    pub kind: SimpleKind,
    pub name: SimpleName,
    /// `span != null`.
    pub has_span: bool,
}

impl SimpleSelector {
    /// `SimpleSelector.name`.
    pub fn name(&self) -> &str {
        self.name.name()
    }

    /// `is ElementSelector`.
    pub fn is_element(&self) -> bool {
        matches!(self.kind, SimpleKind::Element)
    }

    /// `is NamespaceSelector`.
    pub fn is_namespace(&self) -> bool {
        matches!(self.kind, SimpleKind::Namespace { .. })
    }

    /// `is PseudoClassSelector` (includes the function form).
    pub fn is_pseudo_class(&self) -> bool {
        matches!(
            self.kind,
            SimpleKind::PseudoClass | SimpleKind::PseudoClassFunction(_)
        )
    }

    /// `is PseudoClassFunctionSelector`.
    pub fn is_pseudo_class_function(&self) -> bool {
        matches!(self.kind, SimpleKind::PseudoClassFunction(_))
    }

    /// `is PseudoElementSelector` (includes the function form).
    pub fn is_pseudo_element(&self) -> bool {
        matches!(
            self.kind,
            SimpleKind::PseudoElement { .. } | SimpleKind::PseudoElementFunction(_)
        )
    }

    /// `ElementSelector(Identifier(name, null), null)` and the like: a selector
    /// created by the shim, without a position.
    pub fn created(kind: SimpleKind, name: &str) -> Rc<SimpleSelector> {
        Rc::new(SimpleSelector {
            kind,
            name: SimpleName::Ident(Identifier::without_span(name)),
            has_span: false,
        })
    }
}

/// `SimpleSelectorSequence`: the combinator is mutable and the sequence is
/// shared.
#[derive(Debug)]
pub struct Sequence {
    pub combinator: Cell<i32>,
    pub selector: Rc<SimpleSelector>,
}

impl Sequence {
    /// `SimpleSelectorSequence(simpleSelector, span, combinator)`.
    pub fn new(selector: Rc<SimpleSelector>, combinator: i32) -> Rc<Sequence> {
        Rc::new(Sequence {
            combinator: Cell::new(combinator),
            selector,
        })
    }

    /// `SimpleSelectorSequence.clone`: another sequence, the same selector.
    pub fn duplicate(&self) -> Rc<Sequence> {
        Sequence::new(Rc::clone(&self.selector), self.combinator.get())
    }

    /// `isCombinatorNone`.
    pub fn is_combinator_none(&self) -> bool {
        self.combinator.get() == tk::COMBINATOR_NONE
    }

    /// `_combinatorToString`.
    pub fn combinator_as_str(&self) -> &'static str {
        match self.combinator.get() {
            tk::COMBINATOR_DESCENDANT => " ",
            tk::COMBINATOR_GREATER => " > ",
            tk::COMBINATOR_PLUS => " + ",
            tk::COMBINATOR_TILDE => " ~ ",
            _ => "",
        }
    }
}

/// `Selector`.
#[derive(Debug, Default)]
pub struct Selector {
    pub sequences: Vec<Rc<Sequence>>,
}

/// `SelectorGroup`.
#[derive(Debug)]
pub struct SelectorGroup {
    pub selectors: Vec<Selector>,
}

/// The value (`Object value`) of a `LiteralTerm`.
#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Double(f64),
    Ident(Identifier),
    Str(String),
    /// `BAD_HEX_VALUE`.
    BadHex,
}

/// The subclasses of `LiteralTerm`.
#[derive(Clone, Debug)]
pub enum TermKind {
    /// `LiteralTerm`.
    Literal,
    /// `NumberTerm`.
    Number,
    /// `ItemTerm` (a `NumberTerm`).
    Item,
    /// `UnitTerm` and subclasses: `LengthTerm` (`length`), `AngleTerm`,
    /// `TimeTerm`, `FreqTerm`, `ResolutionTerm`, `ChTerm`, `RemTerm`,
    /// `ViewportTerm`, `LineHeightTerm`.
    Unit { unit: i32, is_length: bool },
    /// `PercentageTerm`.
    Percentage,
    /// `EmTerm`.
    Em,
    /// `ExTerm`.
    Ex,
    /// `FractionTerm`.
    Fraction,
    /// `UriTerm`.
    Uri,
    /// `HexColorTerm`.
    Hex,
    /// `IE8Term`.
    Ie8,
    /// `FunctionTerm`.
    Function(Expressions),
    /// `CalcTerm`.
    Calc(Box<Term>),
}

/// `LiteralTerm` (and subclasses).
#[derive(Clone, Debug)]
pub struct Term {
    pub kind: TermKind,
    pub value: Value,
    pub text: String,
}

impl Term {
    pub fn new(kind: TermKind, value: Value, text: String) -> Self {
        Term { kind, value, text }
    }

    /// `is NumberTerm`.
    pub fn is_number(&self) -> bool {
        matches!(self.kind, TermKind::Number | TermKind::Item)
    }
}

/// `Expression` (except `KeyFrameBlock`, which lives in the
/// `KeyFrameDirective`).
#[derive(Clone, Debug)]
pub enum Expr {
    Term(Term),
    /// `UnicodeRangeTerm`.
    UnicodeRange {
        first: Option<String>,
        second: Option<String>,
    },
    /// `VarUsage`.
    VarUsage {
        name: String,
        defaults: Vec<Expr>,
    },
    /// `OperatorSlash`.
    Slash,
    /// `OperatorComma`.
    Comma,
    /// `OperatorPlus`.
    Plus,
    /// `OperatorMinus`.
    Minus,
    /// `GroupTerm`.
    Group(Vec<Term>),
}

/// `Expressions`.
#[derive(Clone, Debug, Default)]
pub struct Expressions {
    pub expressions: Vec<Expr>,
}

/// `VarDefinition`.
#[derive(Debug)]
pub struct VarDef {
    pub name: Option<Identifier>,
    pub expression: Option<Expressions>,
}

/// `IncludeDirective`.
#[derive(Debug)]
pub struct Include {
    pub name: String,
    pub args: Vec<Vec<Expr>>,
}

/// `Declaration` and subclasses.
#[derive(Debug)]
pub enum Declaration {
    /// `Declaration`.
    Plain {
        property: Identifier,
        expression: Expressions,
        important: bool,
        ie7: bool,
    },
    /// `VarDefinition`.
    VarDef(VarDef),
    /// `IncludeMixinAtDeclaration`.
    Include(Include),
    /// `ExtendDeclaration`.
    Extend(Vec<Rc<SimpleSelector>>),
}

/// `DeclarationGroup`; with `margin`, the `MarginGroup` of a `@page`.
#[derive(Debug, Default)]
pub struct DeclarationGroup {
    pub declarations: Vec<Node>,
    pub margin: Option<i32>,
}

/// `RuleSet`.
#[derive(Debug)]
pub struct RuleSet {
    pub group: SelectorGroup,
    pub declarations: DeclarationGroup,
}

/// `MediaExpression`.
#[derive(Debug)]
pub struct MediaExpr {
    pub and_operator: bool,
    pub feature: Identifier,
    pub exprs: Expressions,
}

/// `MediaQuery`.
#[derive(Debug)]
pub struct MediaQuery {
    pub unary: i32,
    pub kind: Option<Identifier>,
    pub expressions: Vec<MediaExpr>,
}

/// `KeyFrameBlock`.
#[derive(Debug)]
pub struct KeyframeBlock {
    pub selectors: Expressions,
    pub declarations: DeclarationGroup,
}

/// `SupportsCondition` and subclasses.
#[derive(Debug)]
pub enum SupportsCondition {
    /// `SupportsConditionInParens`: a `Declaration` or a condition.
    InParens(Option<Box<InsideParens>>),
    /// `SupportsNegation`.
    Negation(Box<SupportsCondition>),
    /// `SupportsConjunction`.
    Conjunction(Vec<SupportsCondition>),
    /// `SupportsDisjunction`.
    Disjunction(Vec<SupportsCondition>),
}

/// The `condition` of `SupportsConditionInParens`.
#[derive(Debug)]
pub enum InsideParens {
    Declaration(Declaration),
    Condition(SupportsCondition),
}

/// The `TreeNode`s that live in lists: rules, directives and declarations.
#[derive(Debug)]
pub enum Node {
    Rule(RuleSet),
    Declaration(Declaration),
    /// `ImportDirective`.
    Import {
        import: String,
        media_queries: Vec<MediaQuery>,
    },
    /// `MediaDirective`.
    Media {
        queries: Vec<MediaQuery>,
        rules: Vec<Node>,
    },
    /// `HostDirective`.
    Host {
        rules: Vec<Node>,
    },
    /// `PageDirective`.
    Page {
        ident: String,
        pseudo: String,
        groups: Vec<DeclarationGroup>,
    },
    /// `CharsetDirective`.
    Charset(String),
    /// `KeyFrameDirective`.
    Keyframes {
        kind: i32,
        name: Option<Identifier>,
        blocks: Vec<KeyframeBlock>,
    },
    /// `FontFaceDirective`.
    FontFace(DeclarationGroup),
    /// `NamespaceDirective`.
    Namespace {
        prefix: String,
        uri: Option<String>,
    },
    /// `VarDefinitionDirective`.
    VarDefDirective(VarDef),
    /// `MixinRulesetDirective`.
    MixinRules {
        name: String,
        rules: Vec<Node>,
    },
    /// `MixinDeclarationDirective`.
    MixinDeclarations {
        name: String,
        declarations: DeclarationGroup,
    },
    /// `IncludeDirective`.
    Include(Include),
    /// `DocumentDirective`.
    Document {
        functions: Vec<Term>,
        body: Vec<Node>,
    },
    /// `SupportsDirective`.
    Supports {
        condition: Option<SupportsCondition>,
        body: Vec<Node>,
    },
    /// `ViewportDirective`.
    Viewport {
        name: String,
        declarations: DeclarationGroup,
    },
}

/// `StyleSheet`.
#[derive(Debug, Default)]
pub struct StyleSheet {
    pub top_level: Vec<Node>,
}
