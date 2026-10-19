//! Complete Dart 3.6 parser, recursive descent, without resolution.
//!
//! The parser is split into modules by grammar area, all as `impl Parser`
//! over the same cursor:
//!
//! * [`types`] — type annotations, type arguments and parameters, formal
//!   parameter lists and the speculative *lookaheads* (`skip_type`);
//! * [`declarations`] — unit, directives, metadata, classes, mixins, enums,
//!   extensions, extension types, typedefs, members, constructors and
//!   function bodies;
//! * [`expressions`] — expressions, literals (strings with interpolation and
//!   escape decoding, collections, records), arguments, cascades, function
//!   expressions and `switch` as an expression;
//! * [`statements`] — statements, local declarations and `for` headers;
//! * [`patterns`] — patterns (Dart 3).
//!
//! Rules shared by every module:
//!
//! * `>` always arrives isolated from the lexer; in expression context use
//!   `Parser::composed_gt` to read `>>`, `>>>`, `>=`, `>>=` and `>>>=`.
//! * Built-in and contextual identifiers (`abstract`, `async`, `show`,
//!   `when`, `on`…) are [`Kind::Ident`]; compare the text with
//!   `Parser::at_ident`. Only reserved words are [`Kind::Keyword`].
//! * An error produces a [`Diagnostic`] in `Parser::diagnostics` and returns
//!   [`ParseError`]; the caller propagates it with `?`. Recovery, where
//!   there is any, sits at top-level and member declaration boundaries.
//! * The parser accepts a **superset** of the valid programs where the
//!   distinction would need resolution (for example, any unary expression as
//!   a constant pattern): rejecting invalid programs is the job of later
//!   phases, and accepting every valid program is the acceptance criterion
//!   here.
pub mod declarations;
pub(crate) mod fasta;
pub(crate) mod modifiers;
pub mod expressions;
pub mod patterns;
pub mod statements;
pub mod types;

use crate::ast::{Ast, CompilationUnit, ExprId, ForInTarget, ForInit, Name};
use crate::features::{Feature, LanguageVersion, LibraryFeatures, AnalyzerReference};
use crate::lexer;
use crate::token::{Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Code, Diagnostic, Span, codes};
use dartforge_intern::Interner;

/// Failure marker: the diagnostic was already recorded in
/// `Parser::diagnostics`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

/// Result of the parser functions.
pub type PResult<T> = Result<T, ParseError>;

/// Composition of isolated `>` in expression context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposedGt {
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `>>`
    Shr,
    /// `>>>`
    UShr,
    /// `>>=`
    ShrAssign,
    /// `>>>=`
    UShrAssign,
}

impl ComposedGt {
    /// How many tokens the composition consumes (never zero: a lone `>` is 1).
    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        match self {
            ComposedGt::Gt => 1,
            ComposedGt::GtEq | ComposedGt::Shr => 2,
            ComposedGt::UShr | ComposedGt::ShrAssign => 3,
            ComposedGt::UShrAssign => 4,
        }
    }
}

/// A `for (...)` header already consumed, including the parentheses.
#[derive(Debug)]
pub enum ForHeader {
    Classic {
        init: Option<ForInit>,
        condition: Option<ExprId>,
        updates: Vec<ExprId>,
    },
    In {
        target: ForInTarget,
        iterable: ExprId,
    },
}

/// Result of [`parse`].
#[derive(Debug)]
pub struct Parsed {
    pub unit: CompilationUnit,
    pub ast: Ast,
    pub diagnostics: Vec<Diagnostic>,
    /// The source ranges whose tokens error recovery discarded (from the first
    /// to the last skipped token): the declaration, member or statement that did
    /// not close, and what `ensureCloseParen` jumped over. A name mentioned in
    /// one of these ranges may have a use or declaration the tree does not have
    /// (docs/ANALYZER-ESPECIFICACAO.md §G, T5, structure (b)).
    pub skipped: Vec<Span>,
    /// Which analyzer is the reference for this unit's code names and texts:
    /// 3.13.4 if the library is above 3.6 or if the unit uses the syntax of a
    /// disabled feature that 3.6.2 does not know
    /// ([`Feature::UNKNOWN_IN_3_6`]); otherwise 3.6.2
    /// (docs/ANALYZER-ESPECIFICACAO.md, T2).
    pub reference: AnalyzerReference,
}

/// Parses a whole compilation unit at the current language version
/// ([`LibraryFeatures::current`]); see [`parse_with`].
///
/// Scanner errors do not stop reading (the lexer goes on like fasta's); all
/// of them end up in [`Parsed::diagnostics`], and the returned unit holds
/// what was read up to the latest recovery point. An empty list means the
/// whole unit was accepted by the grammar.
///
/// ```
/// let mut names = dartforge_intern::Interner::new();
/// let output = dartforge_frontend::parser::parse("void main() { print('oi'); }", &mut names);
/// assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
/// assert_eq!(output.unit.declarations.len(), 1);
/// ```
pub fn parse(source: &str, interner: &mut Interner) -> Parsed {
    parse_with(source, interner, LibraryFeatures::current())
}

/// Like [`parse`], with the features of the library the unit belongs to
/// (`docs/VERSOES-LINGUAGEM.md`). The accepted grammar is always the
/// superset; a disabled feature becomes a diagnostic, and where the grammar
/// **diverges** between versions (`factory() {}`, `var`/`final` on a
/// parameter) the version decides.
pub fn parse_with(source: &str, interner: &mut Interner, features: LibraryFeatures) -> Parsed {
    parse_lexed_with(source, lexer::lex(source), interner, features)
}

/// The fasta scanner errors that do not stop reading (the synthetic token
/// goes on to the parser), taken from the token lexemes:
/// `UNSUPPORTED_OPERATOR` at the start of `===`/`!==` (with the lexeme),
/// `MISSING_DIGIT` at the last character of a `double` whose exponent has
/// no digit and `MISSING_HEX_DIGIT` at the last character of a `0x` without
/// a hexadecimal digit (`fe:scanner/errors.dart`, `translateErrorToken`).
/// All of length 1.
fn recoverable_scanner_errors(source: &str, tokens: &[Token]) -> Vec<Diagnostic> {
    use crate::token::{Kind, Op};
    let mut output = Vec::new();
    let one = |start_offset: usize| Span { start: start_offset, end: start_offset + 1 };
    for t in tokens {
        let lexeme = &source[t.span.start..t.span.end];
        match t.kind {
            Kind::Op(Op::EqEq | Op::BangEq) if lexeme.len() == 3 => {
                output.push(Diagnostic::with_code(dartforge_diagnostics::codes::scanner::UNSUPPORTED_OPERATOR, one(t.span.start), [lexeme]));
            }
            Kind::Double => {
                if let Some(i) = lexeme.find(['e', 'E']) {
                    let exponent = &lexeme[i + 1..];
                    if !exponent.bytes().any(|b| b.is_ascii_digit()) {
                        output.push(Diagnostic::with_code(dartforge_diagnostics::codes::scanner::MISSING_DIGIT, one(t.span.end - 1), Vec::<&str>::new()));
                    }
                }
            }
            // `translateErrorToken` (`errors.dart:36`): at the character before the
            // `endOffset`, which is the line break or the end of the file.
            Kind::Str(f) | Kind::StrEnd(f) if f.unterminated => {
                let mut last = t.span.end.saturating_sub(1);
                while last > 0 && !source.is_char_boundary(last) {
                    last -= 1;
                }
                output.push(Diagnostic::with_code(dartforge_diagnostics::codes::scanner::UNTERMINATED_STRING_LITERAL, one(last), Vec::<&str>::new()));
            }
            // `NonAsciiIdentifierToken`: each character outside the ASCII identifier,
            // with its code point.
            Kind::Ident if !lexeme.chars().all(crate::lexer::identifier_char) => {
                for (i, ch) in lexeme.char_indices() {
                    if !crate::lexer::identifier_char(ch) {
                        let start_offset = t.span.start + i;
                        let n = (ch as u32).to_string();
                        output.push(Diagnostic::with_code(
                            dartforge_diagnostics::codes::scanner::ILLEGAL_CHARACTER,
                            Span { start: start_offset, end: start_offset + ch.len_utf8() },
                            [n.as_str()],
                        ));
                    }
                }
            }
            Kind::Int
                if (lexeme.starts_with("0x") || lexeme.starts_with("0X")) && !lexeme[2..].bytes().any(|b| b.is_ascii_hexdigit()) =>
            {
                output.push(Diagnostic::with_code(dartforge_diagnostics::codes::scanner::MISSING_HEX_DIGIT, one(t.span.end - 1), Vec::<&str>::new()));
            }
            _ => {}
        }
    }
    // Between tokens there are only whitespace and comments, plus what the
    // scanner skipped (ASCII control, non-ASCII whitespace) and the
    // unterminated block comment (`UNTERMINATED_MULTI_LINE_COMMENT` at the last
    // character, `errors.dart:39-44`).
    let mut previous_end = 0usize;
    for t in tokens {
        gap(source, previous_end, t.span.start, &mut output);
        previous_end = previous_end.max(t.span.end);
    }
    output
}

/// The scanner errors in the gap `[from, until)` between two tokens.
fn gap(source: &str, from: usize, until: usize, output: &mut Vec<Diagnostic>) {
    if from >= until {
        return;
    }
    let bytes = source.as_bytes();
    let mut i = from;
    while i < until {
        if bytes[i..].starts_with(b"//") {
            while i < until && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            let mut prof = 1usize;
            i += 2;
            while i < bytes.len() && prof != 0 {
                if bytes[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if prof != 0 {
                let end_offset = bytes.len().saturating_sub(1);
                output.push(Diagnostic::with_code(
                    dartforge_diagnostics::codes::scanner::UNTERMINATED_MULTI_LINE_COMMENT,
                    Span { start: end_offset, end: end_offset + 1 },
                    Vec::<&str>::new(),
                ));
            }
            continue;
        }
        let Some(ch) = source[i..].chars().next() else { break };
        if !ch.is_ascii_whitespace() && crate::lexer::skipped_char(ch) {
            let n = (ch as u32).to_string();
            output.push(Diagnostic::with_code(
                dartforge_diagnostics::codes::scanner::ILLEGAL_CHARACTER,
                Span { start: i, end: i + ch.len_utf8() },
                [n.as_str()],
            ));
        }
        i += ch.len_utf8();
    }
}

/// Like [`parse`], with the source already lexed. The lexer is pure (it does
/// not intern names), so it can run on another thread; only parsing needs
/// the `Interner` and stays sequential.
pub fn parse_lexed(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
) -> Parsed {
    parse_lexed_with(source, tokens, interner, LibraryFeatures::current())
}

/// [`parse_lexed`] with the library's features; see [`parse_with`].
pub fn parse_lexed_with(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
    features: LibraryFeatures,
) -> Parsed {
    let tokens = match tokens {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return Parsed {
                unit: CompilationUnit::default(),
                ast: Ast::default(),
                diagnostics: vec![diagnostic],
                // Lexical error: nothing was read, the whole source was left out.
                skipped: vec![Span { start: 0, end: source.len() }],
                reference: if features.version() > LanguageVersion::FLOOR { AnalyzerReference::V3_13 } else { AnalyzerReference::V3_6 },
            };
        }
    };
    let mut from_scanner = recoverable_scanner_errors(source, &tokens);
    // The missing closers (and the error of each), like fasta's scanner.
    let (tokens, group_of, mut closers) = crate::grouping::group_tokens(source, tokens);
    for f in &mut closers {
        f.error += from_scanner.len();
    }
    from_scanner.extend(group_of);
    // The read, in 3.13.4 mode or not. The tree of some constructs diverges
    // between the references (`factory new()`: a constructor in 3.13.4, a
    // `factory` field in 3.6.2), and the reference is only known at the end of
    // the unit (the new syntax may come later): a unit that diverged and ended
    // up with the 3.13 reference is read again in 3.13 mode.
    let read = |interner: &mut Interner, mode_313: bool| {
        let mut parser = Parser::new(source, tokens.clone(), interner);
        parser.synthetic_closers = closers.clone();
        parser.features = features;
        parser.mode_313 = mode_313;
        parser.diagnostics.extend(from_scanner.iter().cloned());
        let unit = parser.parse_compilation_unit();
        let redo = !mode_313 && parser.diverged_313 && parser.new_syntax;
        (unit, parser.ast, parser.diagnostics, parser.skipped, parser.new_syntax, parser.only_3_6, parser.only_3_13, redo)
    };
    let mut reading = read(interner, features.version() > LanguageVersion::FLOOR);
    if reading.7 {
        reading = read(interner, true);
    }
    let (unit, mut ast, diags, skipped, new_syntax, only_3_6, only_3_13, _) = reading;
    // The returned tree lives long (an editor keeps it per open file) and the
    // arenas grew in powers of two: in `new_sali` the slack was 29 MiB out of
    // 132 MiB. One reallocation per arena here is cheaper than keeping the
    // slack for the unit's whole life.
    ast.shrink_to_fit();
    let reference =
        if features.version() > LanguageVersion::FLOOR || new_syntax { AnalyzerReference::V3_13 } else { AnalyzerReference::V3_6 };
    // The diagnostics only the other reference reports come out now, when this
    // unit's reference is known.
    let mut diagnostics = diags;
    let outside = if reference == AnalyzerReference::V3_13 { &only_3_6 } else { &only_3_13 };
    if !outside.is_empty() {
        diagnostics.retain(|d| !d.code.is_some_and(|c| outside.contains(&(c, d.span))));
    }
    Parsed {
        unit,
        ast,
        diagnostics,
        skipped,
        reference,
    }
}

/// A rewrite of the token stream, for speculative attempts to undo.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Splice {
    /// `n` tokens inserted at `at`.
    Insertion { at: usize, n: usize },
    /// The synthetic closer `closer` moved from `from` (where it was `token`, at
    /// `at`) to `to`; its error had `error_span`.
    Move { from: usize, to: usize, token: Token, closer: usize, at: usize, error_span: Option<Span> },
}

/// Cursor over a unit's tokens plus the arenas under construction.
pub struct Parser<'s, 'i> {
    pub(crate) source: &'s str,
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) ast: Ast,
    pub(crate) interner: &'i mut Interner,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Ver [`Parsed::skipped`].
    pub(crate) skipped: Vec<Span>,
    /// Nesting depth of expressions/statements, to reject pathological input
    /// before blowing the stack.
    pub(crate) depth: u32,
    /// Inside an `async`/`async*` body: `await` and `yield` are keywords.
    pub(crate) in_async: bool,
    /// Inside a `sync*`/`async*` body: `yield` is a keyword.
    pub(crate) in_generator: bool,
    /// Inside a type's `<...>`: `>` never composes.
    pub(crate) in_type_args: u32,
    /// Scratch space for call arguments, shared by every call (nested ones push
    /// on top of the previous); see [`Parser::parse_arguments`].
    pub(crate) scratch_args: Vec<crate::ast::Argument>,
    /// Scratch space for string literal pieces; see
    /// [`Parser::parse_string_literal`].
    pub(crate) scratch_parts: Vec<crate::ast::StringPart>,
    /// Library features (language version and experiments).
    pub(crate) features: LibraryFeatures,
    /// Reading a primary constructor's parameter list (Dart 3.13): `var`/`final`
    /// declare a field and a declaring private named parameter is allowed.
    pub(crate) in_primary_constructor: bool,
    /// Reading an extension type's representation (which reports the
    /// representation's `var` separately, in `parse_representation`).
    pub(crate) in_representation: bool,
    /// The token that opens the body (`{` or `=>`) of each `this` part of a
    /// primary constructor, by the start of the `this`: elaboration, which
    /// knows whether the constructor is `const`, reports the analyzer's body
    /// errors there.
    pub(crate) primary_bodies: std::collections::HashMap<usize, Span>,
    /// The token reinsertions in the stream (fasta's `rewriter.insertToken`), as
    /// `(index, count)`, in the order they were made: speculative attempts undo
    /// them when backtracking ([`Parser::undo_splices`]).
    pub(crate) splices: Vec<Splice>,
    /// The scanner's synthetic closers ([`crate::grouping::group_tokens`]); the
    /// `error` of each is the index in `diagnostics`.
    pub(crate) synthetic_closers: Vec<crate::grouping::SyntheticCloser>,
    /// Inside a `spelled_out_operator` attempt (fasta's `_currentlyRecovering`):
    /// no other attempt is made inside it.
    pub(crate) recovering_operator: bool,
    /// The declaration whose members are being read (fasta's
    /// `DeclarationKind`): decides the errors specific to extensions and mixins.
    pub(crate) owner: MemberOwner,
    /// The owner of the next formal parameter list (fasta's `MemberKind`), set
    /// by whoever reads it right before; the list consumes it (nested
    /// function-parameter lists are `Other`).
    pub(crate) params_owner: modifiers::ParameterOwner,
    /// The name of the declaration whose member is being read (`None` at top
    /// level): a field, getter or setter with that name is
    /// `MEMBER_WITH_CLASS_NAME`.
    pub(crate) enclosing_name: Option<&'s str>,
    /// fasta's `PatternContext` of the pattern being read.
    pub(crate) default_context: patterns::PatternContext,
    /// O `DirectiveState` do fasta (`directive_context.dart`).
    pub(crate) directive_state: DirectiveState,
    /// In a speculative `recover_member` attempt: the missing `;` is not
    /// inserted before just any token (the attempt only succeeds with a
    /// well-terminated member).
    pub(crate) speculating: bool,
    /// The place of the synthetic name of the class/mixin being read (recovery
    /// of `declaration_name`), where the missing-body error goes.
    pub(crate) synthetic_name: Option<dartforge_diagnostics::Span>,
    /// The unit uses the syntax of a disabled feature that the 3.6.2 analyzer
    /// does not know: its reference is 3.13.4 ([`Parsed::reference`]).
    pub(crate) new_syntax: bool,
    /// Reads the constructs where the 3.6.2 and 3.13.4 trees diverge the 3.13.4
    /// way (`factory new()`): a unit with a version above the floor, or the
    /// second read of a unit that ended up with the 3.13 reference
    /// ([`Self::diverged_313`]).
    pub(crate) mode_313: bool,
    /// The read outside [`Self::mode_313`] went through a construct where the
    /// trees diverge: if the unit ends up with the 3.13 reference, it is read
    /// again in 3.13 mode.
    pub(crate) diverged_313: bool,
    /// Reading the elements of a list literal: a plain element does not read
    /// `: value` (`parseLiteralListSuffix` reads the entries with
    /// `parseExpression`); only the null-aware `?k: v` becomes an entry.
    pub(crate) in_list: bool,
    /// The diagnostics (code and place) only the 3.6.2 analyzer reports; they
    /// leave the list if the unit's reference ends up being 3.13.4. Stored by
    /// code and place, not by index: speculative attempts truncate the
    /// diagnostic list.
    pub(crate) only_3_6: Vec<(Code, Span)>,
    /// The inverse: those only 3.13.4 reports.
    pub(crate) only_3_13: Vec<(Code, Span)>,
    /// fasta's `!mayParseFunctionExpressions`: reading a constructor
    /// initializer or the pattern and guard of a `switch` expression case,
    /// where `name(…) {` and `name(…) =>` are not a named function literal (the
    /// `{` is the constructor body and the `=>` is the case's).
    pub(crate) without_named_function: bool,
}

/// `DirectiveState` (`directive_context.dart`), in the order of fasta's
/// enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DirectiveState {
    NoneFound,
    Script,
    Library,
    ImportExport,
    Part,
    PartOf,
    Declarations,
}

/// fasta's `DeclarationKind` for the members being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberOwner {
    Class,
    Mixin,
    Enum,
    Extension,
    ExtensionType,
}

/// Nesting limit before a depth diagnostic.
pub const MAX_DEPTH: u32 = 512;

impl<'s, 'i> Parser<'s, 'i> {
    pub(crate) fn new(source: &'s str, tokens: Vec<Token>, interner: &'i mut Interner) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            ast: Ast::default(),
            interner,
            diagnostics: Vec::new(),
            skipped: Vec::new(),
            depth: 0,
            in_async: false,
            in_generator: false,
            in_type_args: 0,
            scratch_args: Vec::new(),
            scratch_parts: Vec::new(),
            features: LibraryFeatures::current(),
            in_primary_constructor: false,
            in_representation: false,
            primary_bodies: std::collections::HashMap::new(),
            splices: Vec::new(),
            synthetic_closers: Vec::new(),
            recovering_operator: false,
            owner: MemberOwner::Class,
            params_owner: modifiers::ParameterOwner::Other,
            enclosing_name: None,
            default_context: patterns::PatternContext::Matching,
            directive_state: DirectiveState::NoneFound,
            speculating: false,
            synthetic_name: None,
            new_syntax: false,
            mode_313: false,
            diverged_313: false,
            in_list: false,
            only_3_6: Vec::new(),
            only_3_13: Vec::new(),
            without_named_function: false,
        }
    }

    // -- Cursor ---------------------------------------------------------

    /// Current token (the last one is always [`Kind::Eof`]).
    pub(crate) fn peek(&self) -> Token {
        self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    /// Token `n` positions ahead, saturating at `Eof`.
    pub(crate) fn peek_at(&self, n: usize) -> Token {
        self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
    }

    pub(crate) fn kind(&self) -> Kind {
        self.peek().kind
    }

    pub(crate) fn kind_at(&self, n: usize) -> Kind {
        self.peek_at(n).kind
    }

    pub(crate) fn span(&self) -> Span {
        self.peek().span
    }

    /// Text of the current token.
    pub(crate) fn text(&self) -> &'s str {
        let token = self.peek();
        &self.source[token.span.start..token.span.end]
    }

    pub(crate) fn text_at(&self, n: usize) -> &'s str {
        let token = self.peek_at(n);
        &self.source[token.span.start..token.span.end]
    }

    /// Text of any token.
    pub(crate) fn token_text(&self, token: Token) -> &'s str {
        &self.source[token.span.start..token.span.end]
    }

    /// Advances and returns the consumed token.
    pub(crate) fn advance(&mut self) -> Token {
        let token = self.peek();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.kind() == Kind::Eof
    }

    pub(crate) fn at_op(&self, op: Op) -> bool {
        self.kind() == Kind::Op(op)
    }

    pub(crate) fn at_op_at(&self, n: usize, op: Op) -> bool {
        self.kind_at(n) == Kind::Op(op)
    }

    pub(crate) fn at_kw(&self, kw: Keyword) -> bool {
        self.kind() == Kind::Keyword(kw)
    }

    pub(crate) fn at_kw_at(&self, n: usize, kw: Keyword) -> bool {
        self.kind_at(n) == Kind::Keyword(kw)
    }

    /// The current token is an identifier (any, including built-in ones).
    pub(crate) fn at_identifier(&self) -> bool {
        self.kind() == Kind::Ident
    }

    pub(crate) fn at_identifier_at(&self, n: usize) -> bool {
        self.kind_at(n) == Kind::Ident
    }

    /// The current token is the contextual identifier `text`.
    pub(crate) fn at_ident(&self, text: &str) -> bool {
        self.kind() == Kind::Ident && self.text() == text
    }

    pub(crate) fn at_ident_at(&self, n: usize, text: &str) -> bool {
        self.kind_at(n) == Kind::Ident && self.text_at(n) == text
    }

    /// Consumes the operator if it is the current one.
    pub(crate) fn eat_op(&mut self, op: Op) -> bool {
        if self.at_op(op) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, kw: Keyword) -> bool {
        if self.at_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consumes the contextual identifier if it is the current one.
    pub(crate) fn eat_ident(&mut self, text: &str) -> bool {
        if self.at_ident(text) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_op(&mut self, op: Op) -> PResult<Token> {
        if self.at_op(op) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(op.text()))
        }
    }

    pub(crate) fn expect_kw(&mut self, kw: Keyword) -> PResult<Token> {
        if self.at_kw(kw) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(kw.text()))
        }
    }

    /// Requires the contextual identifier `text`.
    pub(crate) fn expect_ident(&mut self, text: &str) -> PResult<Token> {
        if self.at_ident(text) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(text))
        }
    }

    /// Requires any identifier and returns its [`Name`].
    pub(crate) fn expect_identifier(&mut self) -> PResult<Name> {
        if self.at_identifier() {
            Ok(self.identifier())
        } else {
            Err(self.identifier_error())
        }
    }

    /// Consumes the current identifier (precondition:
    /// [`Parser::at_identifier`]).
    pub(crate) fn identifier(&mut self) -> Name {
        let token = self.advance();
        let sym = self
            .interner
            .intern(&self.source[token.span.start..token.span.end]);
        Name {
            sym,
            span: token.span,
        }
    }

    /// Interns arbitrary text as a name (operators, `unary-`, `[]=`).
    pub(crate) fn name_from(&mut self, text: &str, span: Span) -> Name {
        Name {
            sym: self.interner.intern(text),
            span,
        }
    }

    /// Span from the given start up to the end of the last consumed token.
    /// Reinserts copies of the tokens of `indices` into the cursor, in that
    /// order (fasta's `rewriter.insertToken`): they are the next to be read.
    pub(crate) fn reinject(&mut self, indices: &[usize]) {
        let copies: Vec<Token> = indices.iter().map(|&i| Token { glued: false, ..self.tokens[i] }).collect();
        let n = copies.len();
        self.tokens.splice(self.pos..self.pos, copies);
        self.splices.push(Splice::Insertion { at: self.pos, n });
    }

    /// `rewriter.moveSynthetic` (`token_stream_rewriter.dart:80`): if the closer
    /// of the opener at `opener` is synthetic, it (and its scanner error) moves to
    /// the cursor, at the current token's position, and is the next to be read.
    pub(crate) fn move_synthetic_closer(&mut self, opener: usize) -> bool {
        let Some(start_offset) = self.tokens.get(opener).map(|t| t.span.start) else { return false };
        let Some(k) = self.synthetic_closers.iter().position(|f| f.opener == start_offset) else { return false };
        let f = self.synthetic_closers[k];
        let Some(j) = (self.pos..self.tokens.len())
            .find(|&j| self.tokens[j].span.start == f.at && self.tokens[j].span.end == f.at && self.tokens[j].kind != Kind::Eof)
        else {
            return false;
        };
        if j == self.pos {
            return true;
        }
        let token = self.tokens.remove(j);
        let new_pos = self.tokens[self.pos].span.start;
        self.tokens.insert(self.pos, Token { span: Span { start: new_pos, end: new_pos }, ..token });
        let error_span = self.diagnostics.get(f.error).map(|d| d.span);
        if let Some(d) = self.diagnostics.get_mut(f.error) {
            d.span = Span { start: new_pos, end: new_pos + 1 };
        }
        self.synthetic_closers[k].at = new_pos;
        self.splices.push(Splice::Move { from: j, to: self.pos, token, closer: k, at: f.at, error_span });
        true
    }

    /// Undoes the splices made after `mark` of them.
    pub(crate) fn undo_splices(&mut self, mark: usize) {
        while self.splices.len() > mark {
            match self.splices.pop().expect("splice") {
                Splice::Insertion { at, n } => {
                    self.tokens.drain(at..at + n);
                }
                Splice::Move { from, to, token, closer, at, error_span } => {
                    self.tokens.remove(to);
                    self.tokens.insert(from, token);
                    self.synthetic_closers[closer].at = at;
                    let error = self.synthetic_closers[closer].error;
                    if let (Some(d), Some(s)) = (self.diagnostics.get_mut(error), error_span) {
                        d.span = s;
                    }
                }
            }
        }
    }

    pub(crate) fn span_from(&self, start: Span) -> Span {
        let end = if self.pos == 0 {
            start.end
        } else {
            self.tokens[self.pos - 1].span.end
        };
        Span {
            start: start.start,
            end: end.max(start.start),
        }
    }

    /// End of the last consumed token.
    pub(crate) fn last_end(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.tokens[self.pos - 1].span.end
        }
    }

    // -- Diagnostics -------------------------------------------------------

    /// Records the diagnostic `code` (from the analyzer's
    /// `ParserErrorCode`/`ScannerErrorCode`, official English message) at
    /// `span` and returns the marker.
    pub(crate) fn error_at(&mut self, code: Code, span: Span, args: &[&str]) -> ParseError {
        self.diagnostics.push(Diagnostic::with_code(code, span, args.iter().copied()));
        ParseError
    }

    /// `code` at the current token — where fasta reports what is missing or
    /// extra.
    pub(crate) fn error(&mut self, code: Code, args: &[&str]) -> ParseError {
        // `findNonZeroLengthToken`: an error at a synthetic closer goes to the
        // first real token after it.
        let i = (self.pos..self.tokens.len())
            .find(|&i| self.tokens[i].span.start < self.tokens[i].span.end || self.tokens[i].kind == Kind::Eof)
            .unwrap_or(self.pos);
        let span = if i == self.pos { self.span() } else { self.tokens[i].span };
        self.error_at(code, span, args)
    }

    /// `EXPECTED_TOKEN`, where the analyzer puts it: a missing `;` goes at the
    /// **previous** token (fasta's `ensureSemicolon`); a closer `)`/`]`/`}`
    /// missing at end of file is a scanner error, at the end, with length 1
    /// (`translateErrorToken`); the rest, at the current token.
    pub(crate) fn expected_error(&mut self, text: &str) -> ParseError {
        if text == ";" && self.pos > 0 {
            // `findPreviousNonZeroLengthToken`: synthetic tokens do not count.
            let previous = (0..self.pos).rev().find(|&i| self.tokens[i].span.start < self.tokens[i].span.end).unwrap_or(self.pos - 1);
            let span = self.tokens[previous].span;
            return self.error_at(codes::parser::EXPECTED_TOKEN, span, &[";"]);
        }
        if matches!(text, ")" | "]" | "}") && self.kind() == Kind::Eof {
            let s = self.span().start;
            return self.error_at(codes::scanner::EXPECTED_TOKEN, Span { start: s, end: s + 1 }, &[text]);
        }
        // A `>` that fasta's scanner reads together with the next one (`>=`, `>>`,
        // `>>=`…) is a single token there: the error covers the whole composition.
        if self.in_type_args == 0
            && let Some(c) = self.composed_gt()
            && c.len() > 1
        {
            let s = self.span().start;
            let end_offset = self.peek_at(c.len() - 1).span.end;
            return self.error_at(codes::parser::EXPECTED_TOKEN, Span { start: s, end: end_offset }, &[text]);
        }
        self.error(codes::parser::EXPECTED_TOKEN, &[text])
    }

    /// Like [`Parser::error_at`], without repeating an identical diagnostic
    /// (code and range) already recorded: the analyzer does not report the same
    /// error at the same place twice (fasta sometimes asks for it twice, like
    /// the `INVALID_CONSTANT_CONST_PREFIX` of `const 1 + 2`).
    pub(crate) fn single_error(&mut self, code: Code, span: Span, args: &[&str]) {
        if !self.diagnostics.iter().any(|d| d.code == Some(code) && d.span == span) {
            self.error_at(code, span, args);
        }
    }

    /// fasta's `ensureSemicolon`: without `;`, `EXPECTED_TOKEN` (`;`) at the
    /// last token read and parsing goes on as if it were there (fasta inserts a
    /// synthetic `;`), without skipping anything.
    pub(crate) fn ensure_semicolon(&mut self) -> PResult<()> {
        if !self.eat_op(Op::Semicolon) {
            self.expected_error(";");
        }
        Ok(())
    }

    /// `ensureSemicolon` without the caution of [`Parser::ensure_semicolon`]:
    /// for where the construct ended at a definite token (name, URI, prefix),
    /// the missing `;` is reported at the last token read and parsing goes on at
    /// the current token, whatever it is.
    pub(crate) fn ensure_semicolon_forced(&mut self) {
        if !self.eat_op(Op::Semicolon) {
            self.expected_error(";");
        }
    }

    /// Records as skipped the tokens from `from` up to the cursor (what a recovery
    /// just discarded), merging with the previous range when they are contiguous
    /// or overlap.
    pub(crate) fn record_skipped(&mut self, from: usize) {
        if self.pos <= from || from >= self.tokens.len() {
            return;
        }
        let start_offset = self.tokens[from].span.start;
        let end_offset = self.tokens[self.pos - 1].span.end;
        if end_offset <= start_offset {
            return;
        }
        if let Some(last) = self.skipped.last_mut()
            && start_offset <= last.end && last.start <= end_offset
        {
            last.start = last.start.min(start_offset);
            last.end = last.end.max(end_offset);
            return;
        }
        self.skipped.push(Span { start: start_offset, end: end_offset });
    }

    /// fasta's `ensureCloseParen`: the `)` that closes the `(` at absolute
    /// position `opener`. If the current token is not it, `EXPECTED_TOKEN` (`)`)
    /// at the current token and the cursor jumps past the matching `)` — what
    /// is in between is not read, as in fasta. Without a matching `)` (end of
    /// file), the usual error.
    pub(crate) fn ensure_close_paren(&mut self, opener: usize) -> PResult<()> {
        if self.eat_op(Op::RParen) {
            return Ok(());
        }
        // `ensureCloseParen`: the misplaced synthetic `)` comes here (the error is
        // the scanner's).
        if self.move_synthetic_closer(opener) {
            self.advance();
            return Ok(());
        }
        match self.matching_close(opener) {
            Some(closes) if closes >= self.pos && self.kind_of(closes) == Kind::Op(Op::RParen) && !self.at_eof() => {
                self.expected_error(")");
                let from = self.pos;
                self.pos = closes;
                self.record_skipped(from);
                self.pos = closes + 1;
                Ok(())
            }
            _ => Err(self.expected_error(")")),
        }
    }

    /// A statement was missing: `MISSING_STATEMENT` at the current token,
    /// preceded by `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` if it is a reserved
    /// word (fasta tries the expression statement and trips over the
    /// identifier).
    pub(crate) fn statement_error(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let text = self.text().to_string();
            let span = self.span();
            self.error_at(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, span, &[&text]);
        }
        self.error(codes::parser::MISSING_STATEMENT, &[])
    }

    /// An identifier was missing: `MISSING_IDENTIFIER` at the current token, or
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` if it is a reserved word.
    pub(crate) fn identifier_error(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let text = self.text().to_string();
            return self.error(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&text]);
        }
        self.error(codes::parser::MISSING_IDENTIFIER, &[])
    }
    /// Records that `span` uses feature `f`: a diagnostic if the library's
    /// version does not enable it. Parsing goes on (the superset is accepted).
    ///
    /// The arguments are `[feature, version 3.6.2 writes, version 3.13.4
    /// writes]`: the template uses the second, and the code's 3.13 variant
    /// swaps it for the third (`Diagnostic::for_reference`).
    pub(crate) fn require(&mut self, f: Feature, span: Span) {
        if !self.features.has(f) {
            if Feature::UNKNOWN_IN_3_6.contains(&f) {
                self.new_syntax = true;
            }
            // `EXPERIMENT_NOT_ENABLED` (with the version in which the feature turns
            // on) or `EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT` (an experiment without a
            // version).
            match f.enabled_in() {
                Some(v) => {
                    let new_ = format!("{v}.0");
                    let old = f.version_reported_in_3_6().map_or_else(|| new_.clone(), |a| format!("{a}.0"));
                    self.error_at(codes::parser::EXPERIMENT_NOT_ENABLED, span, &[f.name(), &old, &new_]);
                }
                None => {
                    self.error_at(codes::parser::EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT, span, &[f.name()]);
                }
            }
        }
    }

    /// Like [`Parser::require`], for the features the analyzer checks while
    /// building the tree (`AstBuilder`), not in fasta's parser: the fix carries
    /// the version as `x.y` (`3.13`), not `x.y.0`. These are the `;` body, the
    /// `this`/`new` members and the declaring `final` (measured on the 3.13.4
    /// oracle).
    pub(crate) fn require_in_ast(&mut self, f: Feature, span: Span) {
        if self.features.has(f) {
            return;
        }
        match f.enabled_in() {
            Some(v) => {
                if Feature::UNKNOWN_IN_3_6.contains(&f) {
                    self.new_syntax = true;
                }
                let new_ = v.to_string();
                let old = f.version_reported_in_3_6().map_or_else(|| new_.clone(), |a| a.to_string());
                self.error_at(codes::parser::EXPERIMENT_NOT_ENABLED, span, &[f.name(), &old, &new_]);
            }
            None => self.require(f, span),
        }
    }

    /// Enters a nesting level; fails beyond [`MAX_DEPTH`].
    pub(crate) fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_DEPTH {
            return Err(self.error(codes::parser::STACK_OVERFLOW, &[]));
        }
        self.depth += 1;
        Ok(())
    }

    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }

    // -- Composing `>` -----------------------------------------------------

    /// Reads, without consuming, the `>` composition starting at the current
    /// token.
    ///
    /// It only composes **glued** tokens, so `a > >b` never becomes a shift.
    /// Returns `None` if the current token is not `>`.
    pub(crate) fn composed_gt(&self) -> Option<ComposedGt> {
        if !self.at_op(Op::Gt) {
            return None;
        }
        let t0 = self.peek();
        let t1 = self.peek_at(1);
        let t2 = self.peek_at(2);
        let t3 = self.peek_at(3);
        let g1 = t0.glued && t1.kind == Kind::Op(Op::Gt);
        let g1_eq = t0.glued && t1.kind == Kind::Op(Op::Assign);
        if !g1 {
            return Some(if g1_eq {
                ComposedGt::GtEq
            } else {
                ComposedGt::Gt
            });
        }
        let g2 = t1.glued && t2.kind == Kind::Op(Op::Gt);
        let g2_eq = t1.glued && t2.kind == Kind::Op(Op::Assign);
        if !g2 {
            return Some(if g2_eq {
                ComposedGt::ShrAssign
            } else {
                ComposedGt::Shr
            });
        }
        let g3_eq = t2.glued && t3.kind == Kind::Op(Op::Assign);
        Some(if g3_eq {
            ComposedGt::UShrAssign
        } else {
            ComposedGt::UShr
        })
    }

    /// Consumes the composition returned by `Parser::composed_gt`.
    pub(crate) fn eat_composed_gt(&mut self, composed: ComposedGt) {
        for _ in 0..composed.len() {
            self.advance();
        }
    }

    // -- Lookahead helpers -------------------------------------------------

    /// Index of the token that closes the delimiter opened at `open_pos`, or
    /// `None` if there is none. Considers `( ) [ ] { }` and does **not** treat
    /// `<`/`>` as delimiters. Used by lookaheads that need to skip a group.
    pub(crate) fn matching_close(&self, open_pos: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = open_pos;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => depth += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Text of the token at absolute position `pos`.
    pub(crate) fn text_of(&self, pos: usize) -> &'s str {
        let token = self.tokens[pos.min(self.tokens.len() - 1)];
        &self.source[token.span.start..token.span.end]
    }

    /// Kind of the token at absolute position `pos`.
    pub(crate) fn kind_of(&self, pos: usize) -> Kind {
        self.tokens[pos.min(self.tokens.len() - 1)].kind
    }
}
