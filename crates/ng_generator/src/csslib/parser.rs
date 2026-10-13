//! Port of the `_Parser` of `csslib/parser.dart` (csslib 1.0.2), with the
//! `ExpressionsProcessor` and `_escapeString`.
//!
//! Each function cites the official one. The parser does not stop on errors:
//! the official one records the message in `messages` and goes on, and the shim
//! only shows them as warnings. The messages themselves do not change the
//! output; what changes it is the count, which the lookahead for nested
//! selectors (`_nestedSelector`) checks — so here they are counted, without
//! keeping the text.
//!
//! What throws in Dart (`!` on null, `as` with the wrong type, index out of the
//! list, `int.parse` out of range) returns [`Exception`]: the official builder
//! would fail there.
//!
//! The styles for Dart (`_styleForDart`, `DartStyleExpression`) are never
//! printed or visited; of them, only the exceptions their construction throws
//! are reproduced (`exprs.expressions[0]` on an empty list, `value as int?`,
//! `value as num`).
use super::Exception;
use super::token_kind as tk;
use super::tokenizer::{Token, Tokenizer, TokenizerState};
use super::tree::*;

type R<T> = Result<T, Exception>;

/// `_legacyPseudoElements`.
const LEGACY_PSEUDO_ELEMENTS: [&str; 4] = ["after", "before", "first-letter", "first-line"];

/// The nesting depth (open recursive productions: rule, term, selector, simple
/// selector, `@supports` condition) beyond which the official
/// `StackOverflowError` is returned. The csslib parser is recursive and the VM
/// stack runs out at a point that depends on the VM (main isolate or not, code
/// already compiled or not). Measured with `shimShadowCss` on the main isolate:
/// up to 1500 levels of any shape pass; nested `:host(` (two productions per
/// level) overflows from 2000 on, `:not(` from 4000 on, and parentheses,
/// blocks, `@media` and `@supports` between 4000 and 8000. With 4000
/// productions the port agrees at all those points, except for 4000–6000 levels
/// of the shapes with one production per level. What always matters is
/// recursion without consuming input — `@supports` at the end of the file
/// recurses forever and the official one always overflows.
const LIMIT: usize = 4000;

/// `_Parser.MAX_UNICODE`.
const MAX_UNICODE: i64 = 0x10FFFF;

/// `processTerm` returns an expression or, for the Less `@name`, the list of
/// expressions (`List<Expression>`).
enum TermOrList {
    One(Expr),
    List(Vec<Expr>),
}

/// `processVariableOrDirective` returns a definition or the directive kind.
enum VarOrDirective {
    VarDefinitionDirective(VarDef),
    VarDefinition,
    Kind(i32),
}

/// `ParserState`.
struct ParserState {
    peeked: Token,
    previous: Option<Token>,
    tokenizer: TokenizerState,
}

/// `_Parser`.
pub(crate) struct Parser<'a> {
    source: &'a [u16],
    tokenizer: Tokenizer<'a>,
    /// `_previousToken`.
    previous: Option<Token>,
    /// `_peekToken`.
    peeked: Token,
    /// `messages.messages.length` of the current `Messages`.
    pub messages: usize,
    /// How many recursive productions are open (see [`LIMIT`]).
    depth: usize,
}

impl<'a> Parser<'a> {
    /// `_Parser(file, text)`.
    pub fn new(source: &'a [u16]) -> R<Self> {
        let mut tokenizer = Tokenizer::new(source, true);
        let peeked = tokenizer.next(false)?;
        Ok(Parser {
            source,
            tokenizer,
            previous: None,
            peeked,
            messages: 0,
            depth: 0,
        })
    }

    /// `parse`.
    pub fn parse(&mut self) -> R<StyleSheet> {
        let mut productions = Vec::new();
        while !self.maybe_eat(tk::END_OF_FILE)? && !self.peek_kind(tk::RBRACE) {
            match self.process_rule(None)? {
                Some(rule) => productions.push(rule),
                None => break,
            }
        }
        self.check_end_of_file();
        Ok(StyleSheet {
            top_level: productions,
        })
    }

    /// `checkEndOfFile`.
    fn check_end_of_file(&mut self) {
        if !(self.peek_kind(tk::END_OF_FILE) || self.peek_kind(tk::INCOMPLETE_COMMENT)) {
            self.error();
        }
    }

    /// `isPrematureEndOfFile`.
    fn is_premature_end_of_file(&mut self) -> R<bool> {
        if self.maybe_eat(tk::END_OF_FILE)? {
            self.error();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ---------------------------------------------------------------
    // Basic support
    // ---------------------------------------------------------------

    /// Enters a recursive production: beyond [`LIMIT`], the official one would
    /// already have overflowed the stack.
    fn enter(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > LIMIT {
            return Err(Exception("StackOverflowError"));
        }
        Ok(())
    }

    /// `_peek`.
    fn peek(&self) -> i32 {
        self.peeked.kind
    }

    /// `_next`.
    fn next(&mut self) -> R<Token> {
        self.next_ur(false)
    }

    /// `_next(unicodeRange: …)`.
    fn next_ur(&mut self, unicode_range: bool) -> R<Token> {
        let new = self.tokenizer.next(unicode_range)?;
        let next_token = std::mem::replace(&mut self.peeked, new);
        self.previous = Some(next_token.clone());
        Ok(next_token)
    }

    /// `_peekKind`.
    fn peek_kind(&self, kind: i32) -> bool {
        self.peeked.kind == kind
    }

    /// `_peekIdentifier`.
    fn peek_identifier(&self) -> bool {
        tk::is_identifier(self.peeked.kind)
    }

    /// `_mark`.
    fn mark(&self) -> ParserState {
        ParserState {
            peeked: self.peeked.clone(),
            previous: self.previous.clone(),
            tokenizer: self.tokenizer.mark(),
        }
    }

    /// `_restore`.
    fn restore(&mut self, m: ParserState) {
        self.tokenizer.restore(m.tokenizer);
        self.peeked = m.peeked;
        self.previous = m.previous;
    }

    /// `_maybeEat`.
    fn maybe_eat(&mut self, kind: i32) -> R<bool> {
        self.maybe_eat_ur(kind, false)
    }

    /// `_maybeEat(kind, unicodeRange: …)`.
    fn maybe_eat_ur(&mut self, kind: i32, unicode_range: bool) -> R<bool> {
        if self.peeked.kind == kind {
            let new = self.tokenizer.next(unicode_range)?;
            self.previous = Some(std::mem::replace(&mut self.peeked, new));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// `_eat`.
    fn eat(&mut self, kind: i32) -> R<()> {
        self.eat_ur(kind, false)
    }

    /// `_eat(kind, unicodeRange: …)`.
    fn eat_ur(&mut self, kind: i32, unicode_range: bool) -> R<()> {
        if !self.maybe_eat_ur(kind, unicode_range)? {
            self.error_expected()?;
        }
        Ok(())
    }

    /// `_errorExpected`: consumes the token and records the error (building the
    /// message, which may fail in `kindToString`, is inside a `try`).
    fn error_expected(&mut self) -> R<()> {
        self.next()?;
        self.error();
        Ok(())
    }

    /// `_error`.
    fn error(&mut self) {
        self.messages += 1;
    }

    /// `_warning` (without `warningsAsErrors`, the warning also goes into
    /// `messages.messages`).
    fn warning(&mut self) {
        self.messages += 1;
    }

    /// `_peekToken.text`.
    fn peeked_text(&self) -> String {
        self.peeked.text(self.source)
    }

    /// `_previousToken!.end != _peekToken.start` and the like.
    fn previous_end(&self) -> Option<usize> {
        self.previous.as_ref().map(|t| t.end)
    }

    // ---------------------------------------------------------------
    // Top-level productions
    // ---------------------------------------------------------------

    /// `processMediaQueryList`.
    fn process_media_query_list(&mut self) -> R<Vec<MediaQuery>> {
        let mut queries = Vec::new();
        while let Some(c) = self.process_media_query()? {
            queries.push(c);
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        Ok(queries)
    }

    /// `processMediaQuery`.
    fn process_media_query(&mut self) -> R<Option<MediaQuery>> {
        let op: Vec<u16> = self.peeked_text().encode_utf16().collect();
        let unary_op = tk::match_media_operator(&op);
        if unary_op != -1 {
            self.next()?;
        }
        let mut kind = None;
        if self.peek_identifier() {
            kind = Some(self.identifier()?);
        }
        let mut exprs = Vec::new();
        loop {
            let and_op = !exprs.is_empty() || kind.is_some();
            if and_op {
                let op: Vec<u16> = self.peeked_text().encode_utf16().collect();
                if tk::match_media_operator(&op) != tk::MEDIA_OP_AND {
                    break;
                }
                self.next()?;
            }
            match self.process_media_expression(and_op)? {
                Some(e) => exprs.push(e),
                None => break,
            }
        }
        if unary_op != -1 || kind.is_some() || !exprs.is_empty() {
            return Ok(Some(MediaQuery {
                unary: unary_op,
                kind,
                expressions: exprs,
            }));
        }
        Ok(None)
    }

    /// `processMediaExpression`.
    fn process_media_expression(&mut self, and_operator: bool) -> R<Option<MediaExpr>> {
        if self.maybe_eat(tk::LPAREN)? && self.peek_identifier() {
            let feature = self.identifier()?;
            let exprs = if self.maybe_eat(tk::COLON)? {
                self.process_expr(false)?
            } else {
                Expressions::default()
            };
            if self.maybe_eat(tk::RPAREN)? {
                return Ok(Some(MediaExpr {
                    and_operator,
                    feature,
                    exprs,
                }));
            }
        }
        Ok(None)
    }

    /// `processDirective`.
    fn process_directive(&mut self) -> R<Option<Node>> {
        let tok_id = match self.process_variable_or_directive(false)? {
            VarOrDirective::VarDefinitionDirective(d) => return Ok(Some(Node::VarDefDirective(d))),
            // Only with `mixinParameter`.
            VarOrDirective::VarDefinition => return Err(Exception("TypeError")),
            VarOrDirective::Kind(t) => t,
        };
        match tok_id {
            tk::DIRECTIVE_IMPORT => {
                self.next()?;
                // `@import "uri"` or `@import url("uri")`.
                let mut import_str = None;
                if self.peek_identifier() {
                    let name = self.identifier()?;
                    if let Expr::Term(Term {
                        kind: TermKind::Uri,
                        text,
                        ..
                    }) = self.process_function(name)?
                    {
                        import_str = Some(text);
                    }
                } else {
                    import_str = Some(self.process_quoted_string(false)?);
                }
                let media_queries = self.process_media_query_list()?;
                if import_str.is_none() {
                    self.error();
                }
                let import = import_str.ok_or(Exception("TypeError"))?;
                Ok(Some(Node::Import {
                    import: super::tokenizer::dart_trim(&import).to_string(),
                    media_queries,
                }))
            }
            tk::DIRECTIVE_MEDIA => {
                self.next()?;
                let queries = self.process_media_query_list()?;
                let mut rules = Vec::new();
                if self.maybe_eat(tk::LBRACE)? {
                    while !self.maybe_eat(tk::END_OF_FILE)? {
                        match self.process_rule(None)? {
                            Some(r) => rules.push(r),
                            None => break,
                        }
                    }
                    if !self.maybe_eat(tk::RBRACE)? {
                        self.error();
                    }
                } else {
                    self.error();
                }
                Ok(Some(Node::Media { queries, rules }))
            }
            tk::DIRECTIVE_HOST => {
                self.next()?;
                let mut rules = Vec::new();
                if self.maybe_eat(tk::LBRACE)? {
                    while !self.maybe_eat(tk::END_OF_FILE)? {
                        match self.process_rule(None)? {
                            Some(r) => rules.push(r),
                            None => break,
                        }
                    }
                    if !self.maybe_eat(tk::RBRACE)? {
                        self.error();
                    }
                } else {
                    self.error();
                }
                Ok(Some(Node::Host { rules }))
            }
            tk::DIRECTIVE_PAGE => {
                // @page S* IDENT? pseudo_page? S* '{' … '}'
                self.next()?;
                let mut name = None;
                if self.peek_identifier() {
                    name = Some(self.identifier()?);
                }
                let mut pseudo_page = None;
                if self.maybe_eat(tk::COLON)? && self.peek_identifier() {
                    pseudo_page = Some(self.identifier()?);
                }
                let pseudo = pseudo_page.map(|p| p.name).unwrap_or_default();
                let ident = name.map(|n| n.name).unwrap_or_default();
                let groups = self.process_margins_declarations()?;
                Ok(Some(Node::Page {
                    ident,
                    pseudo,
                    groups,
                }))
            }
            tk::DIRECTIVE_CHARSET => {
                // @charset S* STRING S* ';'
                self.next()?;
                let encoding = self.process_quoted_string(false)?;
                Ok(Some(Node::Charset(encoding)))
            }
            tk::DIRECTIVE_KEYFRAMES
            | tk::DIRECTIVE_WEB_KIT_KEYFRAMES
            | tk::DIRECTIVE_MOZ_KEYFRAMES
            | tk::DIRECTIVE_O_KEYFRAMES
            | tk::DIRECTIVE_MS_KEYFRAMES => {
                // @[browser]? keyframes [IDENT|STRING] '{' keyframes-blocks '}'
                self.next()?;
                let mut name = None;
                if self.peek_identifier() {
                    name = Some(self.identifier()?);
                }
                self.eat(tk::LBRACE)?;
                let mut blocks = Vec::new();
                loop {
                    let mut selectors = Expressions::default();
                    loop {
                        // `processTerm() as Expression`: null or a list throws.
                        match self.process_term(false)? {
                            Some(TermOrList::One(e)) => selectors.expressions.push(e),
                            _ => return Err(Exception("TypeError")),
                        }
                        if !self.maybe_eat(tk::COMMA)? {
                            break;
                        }
                    }
                    let declarations = self.process_declarations(true)?;
                    blocks.push(KeyframeBlock {
                        selectors,
                        declarations,
                    });
                    if self.maybe_eat(tk::RBRACE)? || self.is_premature_end_of_file()? {
                        break;
                    }
                }
                Ok(Some(Node::Keyframes {
                    kind: tok_id,
                    name,
                    blocks,
                }))
            }
            tk::DIRECTIVE_FONTFACE => {
                self.next()?;
                Ok(Some(Node::FontFace(self.process_declarations(true)?)))
            }
            tk::DIRECTIVE_STYLET => {
                // @stylet IDENT '{' ruleset '}'
                self.next()?;
                if self.peek_identifier() {
                    self.identifier()?;
                }
                self.eat(tk::LBRACE)?;
                while !self.maybe_eat(tk::END_OF_FILE)? {
                    if self.process_rule(None)?.is_none() {
                        break;
                    }
                }
                self.eat(tk::RBRACE)?;
                // `StyletDirective(name as String, …)`: the name is an
                // `Identifier` or null, and the `as String` always throws.
                Err(Exception("TypeError"))
            }
            tk::DIRECTIVE_NAMESPACE => {
                // @namespace S* [namespace_prefix S*]? [STRING|URI] S* ';' S*
                self.next()?;
                let mut prefix = None;
                if self.peek_identifier() {
                    prefix = Some(self.identifier()?);
                }
                let mut uri = None;
                if self.peek_identifier() {
                    let name = self.identifier()?;
                    if let Expr::Term(Term {
                        kind: TermKind::Uri,
                        text,
                        ..
                    }) = self.process_function(name)?
                    {
                        uri = Some(text);
                    }
                } else if prefix.as_ref().is_some_and(|p| p.name == "url") {
                    let name = prefix.clone().ok_or(Exception("TypeError"))?;
                    if let Expr::Term(Term {
                        kind: TermKind::Uri,
                        text,
                        ..
                    }) = self.process_function(name)?
                    {
                        // @namespace url("");
                        uri = Some(text);
                        prefix = None;
                    }
                } else {
                    uri = Some(self.process_quoted_string(false)?);
                }
                Ok(Some(Node::Namespace {
                    prefix: prefix.map(|p| p.name).unwrap_or_default(),
                    uri,
                }))
            }
            tk::DIRECTIVE_MIXIN => self.process_mixin(),
            tk::DIRECTIVE_INCLUDE => Ok(Some(Node::Include(self.process_include(true)?))),
            tk::DIRECTIVE_CONTENT => {
                // `@content not implemented.`
                self.warning();
                Ok(None)
            }
            tk::DIRECTIVE_MOZ_DOCUMENT => Ok(Some(self.process_document_directive()?)),
            tk::DIRECTIVE_SUPPORTS => Ok(Some(self.process_supports_directive()?)),
            tk::DIRECTIVE_VIEWPORT | tk::DIRECTIVE_MS_VIEWPORT => {
                Ok(Some(self.process_viewport_directive()?))
            }
            _ => Ok(None),
        }
    }

    /// `processMixin`.
    fn process_mixin(&mut self) -> R<Option<Node>> {
        self.next()?;
        let name = self.identifier()?;
        // Neither the visitor nor the printer reads the parameters
        // (`definedArgs`); what matters is what the loop consumes.
        if self.maybe_eat(tk::LPAREN)? {
            let mut must_have_param = false;
            let mut keep_going = true;
            while keep_going {
                let before = (
                    self.peeked.start,
                    self.peeked.kind,
                    self.tokenizer.position(),
                );
                let var_def = self.process_variable_or_directive(true)?;
                let is_param = matches!(
                    var_def,
                    VarOrDirective::VarDefinitionDirective(_) | VarOrDirective::VarDefinition
                );
                if !is_param && must_have_param {
                    self.warning();
                    keep_going = false;
                }
                if self.maybe_eat(tk::COMMA)? {
                    must_have_param = true;
                    continue;
                }
                keep_going = !self.maybe_eat(tk::RPAREN)?;
                // The official one does not terminate when nothing more is
                // consumed (for example `@mixin m(` at the end of the file):
                // the builder hangs.
                if keep_going
                    && before
                        == (
                            self.peeked.start,
                            self.peeked.kind,
                            self.tokenizer.position(),
                        )
                {
                    return Err(Exception("endless loop in processMixin"));
                }
            }
        }
        self.eat(tk::LBRACE)?;

        let mut productions: Vec<Node> = Vec::new();
        // `mixinDirective`; `rules` when the loop exits holding the
        // `MixinRulesetDirective(productions)` — the same list that the check
        // after the loop uses.
        let mut mixin: Option<Node> = None;
        let mut rules = false;
        while !self.maybe_eat(tk::END_OF_FILE)? {
            if let Some(directive) = self.process_directive()? {
                productions.push(directive);
                continue;
            }
            let mut group = self.process_declarations(false)?;
            let any_declaration = group.declarations.iter().any(|d| {
                matches!(
                    d,
                    Node::Declaration(
                        Declaration::Plain { .. } | Declaration::VarDef(_) | Declaration::Extend(_)
                    )
                )
            });
            if any_declaration {
                // With declarations, the mixin is a declaration mixin: the
                // `@include`s before them go into them, the rest is lost with a
                // warning.
                let mut new_declarations = Vec::new();
                for include in std::mem::take(&mut productions) {
                    match include {
                        Node::Include(i) => {
                            new_declarations.push(Node::Declaration(Declaration::Include(i)))
                        }
                        _ => self.warning(),
                    }
                }
                new_declarations.append(&mut group.declarations);
                group.declarations = new_declarations;
            } else {
                // Only `@include`: becomes a list of productions.
                for d in std::mem::take(&mut group.declarations) {
                    productions.push(match d {
                        Node::Declaration(Declaration::Include(i)) => Node::Include(i),
                        other => other,
                    });
                }
            }
            if !group.declarations.is_empty() {
                if productions.is_empty() {
                    mixin = Some(Node::MixinDeclarations {
                        name: name.name.clone(),
                        declarations: group,
                    });
                    break;
                }
                for d in std::mem::take(&mut group.declarations) {
                    productions.push(match d {
                        Node::Declaration(Declaration::Include(i)) => Node::Include(i),
                        other => other,
                    });
                }
            } else {
                rules = true;
                break;
            }
        }
        if rules || !productions.is_empty() {
            mixin = Some(Node::MixinRules {
                name: name.name.clone(),
                rules: productions,
            });
        }
        self.eat(tk::RBRACE)?;
        Ok(mixin)
    }

    /// `processVariableOrDirective`.
    fn process_variable_or_directive(&mut self, mixin_parameter: bool) -> R<VarOrDirective> {
        let mut tok_id = self.peek();
        // `@ name`, with whitespace between the at sign and the name.
        if tok_id == tk::AT {
            self.next()?;
            tok_id = self.peek();
            if self.peek_identifier() {
                let directive: Vec<u16> = self.peeked_text().encode_utf16().collect();
                tok_id = tk::match_directives(&directive);
                if tok_id == -1 {
                    tok_id = tk::match_margin_directives(&directive);
                }
            }
            if tok_id == -1 {
                // Less (`lessSupport` holds by default):
                //    @name: value;  =>  var-name: value;  (VarDefinition)
                let mut name = None;
                if self.peek_identifier() {
                    name = Some(self.identifier()?);
                }
                let mut exprs = None;
                if mixin_parameter && self.maybe_eat(tk::COLON)? {
                    exprs = Some(self.process_expr(false)?);
                } else if !mixin_parameter {
                    self.eat(tk::COLON)?;
                    exprs = Some(self.process_expr(false)?);
                }
                return Ok(VarOrDirective::VarDefinitionDirective(VarDef {
                    name,
                    expression: exprs,
                }));
            }
        } else if mixin_parameter && self.peeked.kind == tk::VAR_DEFINITION {
            self.next()?;
            if self.peek_identifier() {
                self.identifier()?;
            }
            if self.maybe_eat(tk::COLON)? {
                self.process_expr(false)?;
            }
            return Ok(VarOrDirective::VarDefinition);
        }
        Ok(VarOrDirective::Kind(tok_id))
    }

    /// `processInclude`.
    fn process_include(&mut self, eat_semi_colon: bool) -> R<Include> {
        // @include IDENT [(args,...)];
        self.next()?;
        let mut name = None;
        if self.peek_identifier() {
            name = Some(self.identifier()?);
        }
        let mut params: Vec<Vec<Expr>> = Vec::new();
        if self.maybe_eat(tk::LPAREN)? {
            let mut terms: Vec<Expr> = Vec::new();
            let mut keep_going = true;
            while keep_going {
                let Some(expr) = self.process_term(false)? else {
                    break;
                };
                // `(expr is List ? expr[0] : expr) as Expression`.
                terms.push(match expr {
                    TermOrList::One(e) => e,
                    TermOrList::List(mut l) => {
                        if l.is_empty() {
                            return Err(Exception("RangeError"));
                        }
                        l.swap_remove(0)
                    }
                });
                keep_going = !self.peek_kind(tk::RPAREN);
                if keep_going && self.maybe_eat(tk::COMMA)? {
                    params.push(std::mem::take(&mut terms));
                }
            }
            params.push(terms);
            self.maybe_eat(tk::RPAREN)?;
        }
        if eat_semi_colon {
            self.eat(tk::SEMICOLON)?;
        }
        let name = name.ok_or(Exception("TypeError"))?;
        Ok(Include {
            name: name.name,
            args: params,
        })
    }

    /// `processDocumentDirective`.
    fn process_document_directive(&mut self) -> R<Node> {
        self.next()?; // '@-moz-document'
        let mut functions = Vec::new();
        loop {
            // IDENT '('
            let ident = self.identifier()?;
            self.eat(tk::LPAREN)?;
            let function = if ident.name == "url-prefix" || ident.name == "domain" {
                // Accepts the argument without quotes and puts them back in the
                // output.
                let value = self.process_quoted_string(true)?;
                let argument = if value.is_empty() {
                    String::new()
                } else {
                    format!("\"{value}\"")
                };
                self.eat(tk::RPAREN)?;
                let arguments = Expressions {
                    expressions: vec![Expr::Term(Term::new(
                        TermKind::Literal,
                        Value::Str(argument.clone()),
                        argument,
                    ))],
                };
                Term::new(
                    TermKind::Function(arguments),
                    Value::Str(ident.name.clone()),
                    ident.name.clone(),
                )
            } else {
                // `processFunction(ident) as LiteralTerm`.
                match self.process_function(ident)? {
                    Expr::Term(t) => t,
                    _ => return Err(Exception("TypeError")),
                }
            };
            functions.push(function);
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        self.eat(tk::LBRACE)?;
        let body = self.process_group_rule_body()?;
        self.eat(tk::RBRACE)?;
        Ok(Node::Document { functions, body })
    }

    /// `processSupportsDirective`.
    fn process_supports_directive(&mut self) -> R<Node> {
        self.next()?; // '@supports'
        let condition = self.process_supports_condition()?;
        self.eat(tk::LBRACE)?;
        let body = self.process_group_rule_body()?;
        self.eat(tk::RBRACE)?;
        Ok(Node::Supports { condition, body })
    }

    /// `processSupportsCondition`.
    fn process_supports_condition(&mut self) -> R<Option<SupportsCondition>> {
        self.enter()?;
        let r = self.process_supports_condition_();
        self.depth -= 1;
        r
    }

    /// The body of [`Self::process_supports_condition`].
    fn process_supports_condition_(&mut self) -> R<Option<SupportsCondition>> {
        if self.peek_kind(tk::IDENTIFIER) {
            return self.process_supports_negation();
        }
        // 0: none; 1: conjunction; 2: disjunction.
        let mut conditions = Vec::new();
        let mut clause_type = 0;
        loop {
            conditions.push(self.process_supports_condition_in_parens()?);
            let text = self.peeked_text().to_lowercase();
            let kind = if text == "and" {
                1
            } else if text == "or" {
                2
            } else {
                break;
            };
            if clause_type == 0 {
                clause_type = kind;
            } else if clause_type != kind {
                self.error();
                break;
            }
            self.next()?;
        }
        Ok(Some(match clause_type {
            1 => SupportsCondition::Conjunction(conditions),
            2 => SupportsCondition::Disjunction(conditions),
            _ => conditions.swap_remove(0),
        }))
    }

    /// `processSupportsNegation`.
    fn process_supports_negation(&mut self) -> R<Option<SupportsCondition>> {
        if self.peeked_text().to_lowercase() != "not" {
            return Ok(None);
        }
        self.next()?; // 'not'
        let condition = self.process_supports_condition_in_parens()?;
        Ok(Some(SupportsCondition::Negation(Box::new(condition))))
    }

    /// `processSupportsConditionInParens`.
    fn process_supports_condition_in_parens(&mut self) -> R<SupportsCondition> {
        self.eat(tk::LPAREN)?;
        if let Some(condition) = self.process_supports_condition()? {
            self.eat(tk::RPAREN)?;
            return Ok(SupportsCondition::InParens(Some(Box::new(
                InsideParens::Condition(condition),
            ))));
        }
        let declaration = self.process_declaration()?;
        self.eat(tk::RPAREN)?;
        Ok(SupportsCondition::InParens(
            declaration.map(|d| Box::new(InsideParens::Declaration(d))),
        ))
    }

    /// `processViewportDirective`.
    fn process_viewport_directive(&mut self) -> R<Node> {
        let name = self.next()?.text(self.source);
        let declarations = self.process_declarations(true)?;
        Ok(Node::Viewport { name, declarations })
    }

    /// `processRule`.
    fn process_rule(&mut self, group: Option<SelectorGroup>) -> R<Option<Node>> {
        self.enter()?;
        let r = self.process_rule_(group);
        self.depth -= 1;
        r
    }

    /// The body of [`Self::process_rule`].
    fn process_rule_(&mut self, group: Option<SelectorGroup>) -> R<Option<Node>> {
        let group = match group {
            Some(g) => Some(g),
            None => {
                if let Some(directive) = self.process_directive()? {
                    self.maybe_eat(tk::SEMICOLON)?;
                    return Ok(Some(directive));
                }
                self.process_selector_group()?
            }
        };
        match group {
            Some(group) => {
                let declarations = self.process_declarations(true)?;
                Ok(Some(Node::Rule(RuleSet {
                    group,
                    declarations,
                })))
            }
            None => Ok(None),
        }
    }

    /// `processGroupRuleBody`.
    fn process_group_rule_body(&mut self) -> R<Vec<Node>> {
        let mut nodes = Vec::new();
        while !(self.peek_kind(tk::RBRACE) || self.peek_kind(tk::END_OF_FILE)) {
            match self.process_rule(None)? {
                Some(r) => nodes.push(r),
                None => break,
            }
        }
        Ok(nodes)
    }

    /// `_nestedSelector`: reads a selector group ahead; if it is not a nested
    /// selector (no `{` after it, or with some message), it backtracks.
    fn nested_selector(&mut self) -> R<Option<SelectorGroup>> {
        let old_count = self.messages;
        self.messages = 0;
        let mark_state = self.mark();
        let group = self.process_selector_group()?;
        let nested = group.is_some() && self.peek_kind(tk::LBRACE) && self.messages == 0;
        if !nested {
            self.restore(mark_state);
            self.messages = old_count;
            Ok(None)
        } else {
            self.messages += old_count;
            Ok(group)
        }
    }

    /// `processDeclarations`.
    fn process_declarations(&mut self, check_brace: bool) -> R<DeclarationGroup> {
        if check_brace {
            self.eat(tk::LBRACE)?;
        }
        let mut decls = Vec::new();
        loop {
            let mut group = self.nested_selector()?;
            while let Some(g) = group {
                // Nested selector: a rule.
                let rule = self.process_rule(Some(g))?.ok_or(Exception("TypeError"))?;
                decls.push(rule);
                group = self.nested_selector()?;
            }
            if let Some(decl) = self.process_declaration()? {
                decls.push(Node::Declaration(decl));
            }
            if !self.maybe_eat(tk::SEMICOLON)? {
                break;
            }
        }
        if check_brace {
            self.eat(tk::RBRACE)?;
        }
        Ok(DeclarationGroup {
            declarations: decls,
            margin: None,
        })
    }

    /// `processMarginsDeclarations`.
    fn process_margins_declarations(&mut self) -> R<Vec<DeclarationGroup>> {
        let mut groups = Vec::new();
        self.eat(tk::LBRACE)?;
        let mut decls = Vec::new();
        loop {
            let before = (
                self.peeked.start,
                self.peeked.kind,
                self.tokenizer.position(),
            );
            match self.peek() {
                tk::MARGIN_DIRECTIVE_TOPLEFTCORNER
                | tk::MARGIN_DIRECTIVE_TOPLEFT
                | tk::MARGIN_DIRECTIVE_TOPCENTER
                | tk::MARGIN_DIRECTIVE_TOPRIGHT
                | tk::MARGIN_DIRECTIVE_TOPRIGHTCORNER
                | tk::MARGIN_DIRECTIVE_BOTTOMLEFTCORNER
                | tk::MARGIN_DIRECTIVE_BOTTOMLEFT
                | tk::MARGIN_DIRECTIVE_BOTTOMCENTER
                | tk::MARGIN_DIRECTIVE_BOTTOMRIGHT
                | tk::MARGIN_DIRECTIVE_BOTTOMRIGHTCORNER
                | tk::MARGIN_DIRECTIVE_LEFTTOP
                | tk::MARGIN_DIRECTIVE_LEFTMIDDLE
                | tk::MARGIN_DIRECTIVE_LEFTBOTTOM
                | tk::MARGIN_DIRECTIVE_RIGHTTOP
                | tk::MARGIN_DIRECTIVE_RIGHTMIDDLE
                | tk::MARGIN_DIRECTIVE_RIGHTBOTTOM => {
                    // margin_sym S* '{' declaration [ ';' S* declaration? ]* '}' S*
                    let margin = self.peek();
                    self.next()?;
                    let group = self.process_declarations(true)?;
                    groups.push(DeclarationGroup {
                        declarations: group.declarations,
                        margin: Some(margin),
                    });
                }
                _ => {
                    if let Some(decl) = self.process_declaration()? {
                        decls.push(Node::Declaration(decl));
                    }
                    self.maybe_eat(tk::SEMICOLON)?;
                }
            }
            if self.maybe_eat(tk::RBRACE)? || self.is_premature_end_of_file()? {
                break;
            }
            // The official one does not terminate when nothing is consumed (for
            // example a `.x` inside `@page { }`): the builder hangs.
            if before
                == (
                    self.peeked.start,
                    self.peeked.kind,
                    self.tokenizer.position(),
                )
            {
                return Err(Exception("endless loop in processMarginsDeclarations"));
            }
        }
        if !decls.is_empty() {
            groups.push(DeclarationGroup {
                declarations: decls,
                margin: None,
            });
        }
        Ok(groups)
    }

    /// `processSelectorGroup`.
    pub fn process_selector_group(&mut self) -> R<Option<SelectorGroup>> {
        let mut selectors = Vec::new();
        self.tokenizer.in_selector = true;
        loop {
            if let Some(s) = self.process_selector()? {
                selectors.push(s);
            }
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        self.tokenizer.in_selector = false;
        if !selectors.is_empty() {
            return Ok(Some(SelectorGroup { selectors }));
        }
        Ok(None)
    }

    /// `processSelector`.
    fn process_selector(&mut self) -> R<Option<Selector>> {
        self.enter()?;
        let r = self.process_selector_();
        self.depth -= 1;
        r
    }

    /// The body of [`Self::process_selector`].
    fn process_selector_(&mut self) -> R<Option<Selector>> {
        let mut sequences = Vec::new();
        while let Some(s) = self.simple_selector_sequence(sequences.is_empty())? {
            sequences.push(s);
        }
        if sequences.is_empty() {
            return Ok(None);
        }
        Ok(Some(Selector { sequences }))
    }

    /// `processCompoundSelector`: one error for each combinator.
    fn process_compound_selector(&mut self) -> R<Option<Selector>> {
        let selector = self.process_selector()?;
        if let Some(s) = &selector {
            for seq in &s.sequences {
                if !seq.is_combinator_none() {
                    self.error();
                }
            }
        }
        Ok(selector)
    }

    /// `simpleSelectorSequence`.
    fn simple_selector_sequence(
        &mut self,
        force_combinator_none: bool,
    ) -> R<Option<std::rc::Rc<Sequence>>> {
        let mut combinator = tk::COMBINATOR_NONE;
        let mut this_operator = false;
        match self.peek() {
            tk::PLUS => {
                self.eat(tk::PLUS)?;
                combinator = tk::COMBINATOR_PLUS;
            }
            tk::GREATER => {
                self.eat(tk::GREATER)?;
                combinator = tk::COMBINATOR_GREATER;
            }
            tk::TILDE => {
                self.eat(tk::TILDE)?;
                combinator = tk::COMBINATOR_TILDE;
            }
            tk::AMPERSAND => {
                self.eat(tk::AMPERSAND)?;
                this_operator = true;
            }
            _ => {}
        }
        // Whitespace between the tokens: descendant.
        if combinator == tk::COMBINATOR_NONE
            && !force_combinator_none
            && self.previous_end().is_some_and(|f| f != self.peeked.start)
        {
            combinator = tk::COMBINATOR_DESCENDANT;
        }
        let mut selector = if this_operator {
            Some(std::rc::Rc::new(SimpleSelector {
                kind: SimpleKind::Element,
                name: SimpleName::This,
                has_span: true,
            }))
        } else {
            self.simple_selector()?
        };
        if selector.is_none()
            && (combinator == tk::COMBINATOR_PLUS
                || combinator == tk::COMBINATOR_GREATER
                || combinator == tk::COMBINATOR_TILDE)
        {
            // `+ &`, `~ &`, `> &`: a sequence without a name to hold the
            // combinator.
            selector = Some(std::rc::Rc::new(SimpleSelector {
                kind: SimpleKind::Element,
                name: SimpleName::Ident(Identifier {
                    name: String::new(),
                    text: None,
                }),
                has_span: true,
            }));
        }
        Ok(selector.map(|s| Sequence::new(s, combinator)))
    }

    /// `simpleSelector`.
    fn simple_selector(&mut self) -> R<Option<std::rc::Rc<SimpleSelector>>> {
        self.enter()?;
        let r = self.simple_selector_();
        self.depth -= 1;
        r
    }

    /// The body of [`Self::simple_selector`].
    fn simple_selector_(&mut self) -> R<Option<std::rc::Rc<SimpleSelector>>> {
        let mut first: Option<SimpleName> = None;
        match self.peek() {
            tk::ASTERISK => {
                self.next()?;
                first = Some(SimpleName::Wildcard);
            }
            tk::IDENTIFIER => first = Some(SimpleName::Ident(self.identifier()?)),
            _ => {
                if tk::is_kind_identifier(self.peek()) {
                    first = Some(SimpleName::Ident(self.identifier()?));
                } else if self.peek_kind(tk::SEMICOLON) {
                    // It is not a selector if it found `;`.
                    return Ok(None);
                }
            }
        }
        if self.maybe_eat(tk::NAMESPACE)? {
            let element = match self.peek() {
                tk::ASTERISK => {
                    self.next()?;
                    SimpleName::Wildcard
                }
                tk::IDENTIFIER => SimpleName::Ident(self.identifier()?),
                // The error builds the message with `$_peekToken`, whose
                // `kindToString` throws `StateError` for a kind without a name;
                // otherwise, `element!.span!` throws with a null element.
                k => {
                    return Err(Exception(if tk::kind_to_string(k).is_none() {
                        "StateError"
                    } else {
                        "TypeError"
                    }));
                }
            };
            let element = std::rc::Rc::new(SimpleSelector {
                kind: SimpleKind::Element,
                name: element,
                has_span: true,
            });
            return Ok(Some(std::rc::Rc::new(SimpleSelector {
                kind: SimpleKind::Namespace { namespace: first },
                name: SimpleName::Selector(element),
                has_span: true,
            })));
        }
        if let Some(first) = first {
            return Ok(Some(std::rc::Rc::new(SimpleSelector {
                kind: SimpleKind::Element,
                name: first,
                has_span: true,
            })));
        }
        // HASH | class | attrib | pseudo | negation
        self.simple_selector_tail()
    }

    /// `_anyWhiteSpaceBeforePeekToken`.
    fn any_white_space_before_peek_token(&self, kind: i32) -> bool {
        match &self.previous {
            Some(t) if t.kind == kind => t.end != self.peeked.start,
            _ => false,
        }
    }

    /// `simpleSelectorTail`.
    fn simple_selector_tail(&mut self) -> R<Option<std::rc::Rc<SimpleSelector>>> {
        match self.peek() {
            tk::HASH => {
                self.eat(tk::HASH)?;
                if self.any_white_space_before_peek_token(tk::HASH) {
                    self.error();
                    return Ok(None);
                }
                let id = self.identifier()?;
                Ok(Some(simple(SimpleKind::Id, id)))
            }
            tk::DOT => {
                self.eat(tk::DOT)?;
                if self.any_white_space_before_peek_token(tk::DOT) {
                    self.error();
                    return Ok(None);
                }
                let class = self.identifier()?;
                Ok(Some(simple(SimpleKind::Class, class)))
            }
            tk::COLON => self.process_pseudo_selector(),
            tk::LBRACK => self.process_attribute(),
            tk::DOUBLE => {
                self.error();
                self.next()?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// `processPseudoSelector`.
    fn process_pseudo_selector(&mut self) -> R<Option<std::rc::Rc<SimpleSelector>>> {
        self.eat(tk::COLON)?;
        let pseudo_element = self.maybe_eat(tk::COLON)?;
        if !self.peek_identifier() {
            return Ok(None);
        }
        let pseudo_name = self.identifier()?;
        let name = pseudo_name.name.to_lowercase();

        // Pseudo with a function?
        if self.peeked.kind == tk::LPAREN {
            if !pseudo_element && name == "not" {
                self.eat(tk::LPAREN)?;
                // Negation: ':NOT(' S* negation_arg S* ')'
                let neg_arg = self.simple_selector()?;
                self.eat(tk::RPAREN)?;
                return Ok(Some(std::rc::Rc::new(SimpleSelector {
                    kind: SimpleKind::Negation(neg_arg),
                    name: SimpleName::Negation,
                    has_span: true,
                })));
            } else if !pseudo_element
                && (name == "host"
                    || name == "host-context"
                    || name == "global-context"
                    || name == "-acx-global-context")
            {
                self.eat(tk::LPAREN)?;
                let Some(selector) = self.process_compound_selector()? else {
                    self.error_expected()?;
                    return Ok(None);
                };
                self.eat(tk::RPAREN)?;
                return Ok(Some(simple(
                    SimpleKind::PseudoClassFunction(PseudoArgument::Selector(std::rc::Rc::new(
                        selector,
                    ))),
                    pseudo_name,
                )));
            } else {
                // In a pseudo expression the `-` is an operator; the mode
                // changes before eating the `(`, which already fetches the next
                // token.
                self.tokenizer.in_selector_expression = true;
                self.eat(tk::LPAREN)?;
                let expr = self.process_selector_expression()?;
                self.tokenizer.in_selector_expression = false;
                return match expr {
                    Some(expr) => {
                        self.eat(tk::RPAREN)?;
                        Ok(Some(if pseudo_element {
                            simple(SimpleKind::PseudoElementFunction(expr), pseudo_name)
                        } else {
                            simple(
                                SimpleKind::PseudoClassFunction(PseudoArgument::Expression(expr)),
                                pseudo_name,
                            )
                        }))
                    }
                    None => {
                        self.error_expected()?;
                        Ok(None)
                    }
                };
            }
        }
        // The CSS2.1 pseudo-elements with a single `:`.
        if pseudo_element || LEGACY_PSEUDO_ELEMENTS.contains(&name.as_str()) {
            Ok(Some(simple(
                SimpleKind::PseudoElement {
                    legacy: !pseudo_element,
                },
                pseudo_name,
            )))
        } else {
            Ok(Some(simple(SimpleKind::PseudoClass, pseudo_name)))
        }
    }

    /// `processSelectorExpression`: the `SelectorExpression`, or `None` where
    /// the official one returns a `LiteralTerm` (quoted argument).
    fn process_selector_expression(&mut self) -> R<Option<Vec<Expr>>> {
        let mut expressions = Vec::new();
        let mut term_token: Option<Token> = None;
        let mut value: Option<Value> = None;
        let mut keep_parsing = true;
        while keep_parsing {
            match self.peek() {
                tk::PLUS => {
                    term_token = Some(self.next()?);
                    expressions.push(Expr::Plus);
                }
                tk::MINUS => {
                    term_token = Some(self.next()?);
                    expressions.push(Expr::Minus);
                }
                tk::INTEGER => {
                    let t = self.next()?;
                    value = Some(Value::Int(int_parse(&t.text(self.source))?));
                    term_token = Some(t);
                }
                tk::DOUBLE => {
                    let t = self.next()?;
                    value = Some(Value::Double(double_parse(&t.text(self.source))?));
                    term_token = Some(t);
                }
                tk::SINGLE_QUOTE | tk::DOUBLE_QUOTE => {
                    self.process_quoted_string(false)?;
                    return Ok(None);
                }
                tk::IDENTIFIER => {
                    value = Some(Value::Ident(self.identifier()?));
                }
                _ => keep_parsing = false,
            }
            if keep_parsing && let Some(v) = value.take() {
                let term = self.process_dimension(term_token.as_ref(), v)?;
                expressions.push(Expr::Term(term));
            }
        }
        Ok(Some(expressions))
    }

    /// `processAttribute`.
    fn process_attribute(&mut self) -> R<Option<std::rc::Rc<SimpleSelector>>> {
        if !self.maybe_eat(tk::LBRACK)? {
            return Ok(None);
        }
        let attr_name = self.identifier()?;
        let op = match self.peek() {
            tk::EQUALS
            | tk::INCLUDES
            | tk::DASH_MATCH
            | tk::PREFIX_MATCH
            | tk::SUFFIX_MATCH
            | tk::SUBSTRING_MATCH => {
                let op = self.peek();
                self.next()?;
                op
            }
            _ => tk::NO_MATCH,
        };
        let mut value = None;
        if op != tk::NO_MATCH {
            // With an operator, a value is required.
            value = Some(if self.peek_identifier() {
                AttributeValue::Ident(self.identifier()?)
            } else {
                AttributeValue::Str(self.process_quoted_string(false)?)
            });
        }
        self.eat(tk::RBRACK)?;
        Ok(Some(simple(SimpleKind::Attribute { op, value }, attr_name)))
    }

    /// `processDeclaration`.
    fn process_declaration(&mut self) -> R<Option<Declaration>> {
        // `*IDENT`: the IE7 hack.
        let ie7 = self.peek_kind(tk::ASTERISK);
        if ie7 {
            self.next()?;
        }
        if tk::is_identifier(self.peeked.kind) {
            // IDENT ':' expr '!important'?
            let property = self.identifier()?;
            let ie_filter_property = property.name.to_lowercase() == "filter";
            self.eat(tk::COLON)?;
            let exprs = self.process_expr(ie_filter_property)?;
            style_for_dart(&property, &exprs)?;
            let important = self.maybe_eat(tk::IMPORTANT)?;
            return Ok(Some(Declaration::Plain {
                property,
                expression: exprs,
                important,
                ie7,
            }));
        } else if self.peeked.kind == tk::VAR_DEFINITION {
            self.next()?;
            let mut name = None;
            if self.peek_identifier() {
                name = Some(self.identifier()?);
            }
            self.eat(tk::COLON)?;
            let exprs = self.process_expr(false)?;
            return Ok(Some(Declaration::VarDef(VarDef {
                name,
                expression: Some(exprs),
            })));
        } else if self.peeked.kind == tk::DIRECTIVE_INCLUDE {
            // @include in the middle of the declarations.
            let include = self.process_include(false)?;
            return Ok(Some(Declaration::Include(include)));
        } else if self.peeked.kind == tk::DIRECTIVE_EXTEND {
            let mut selectors = Vec::new();
            self.next()?;
            match self.simple_selector()? {
                None => self.warning(),
                Some(s) => selectors.push(s),
            }
            if self.peek_kind(tk::COLON) {
                match self.process_pseudo_selector()? {
                    Some(p) if p.is_pseudo_element() || p.is_pseudo_class() => selectors.push(p),
                    _ => self.warning(),
                }
            }
            return Ok(Some(Declaration::Extend(selectors)));
        }
        Ok(None)
    }

    /// `processExpr`.
    fn process_expr(&mut self, ie_filter: bool) -> R<Expressions> {
        let mut expressions = Expressions::default();
        let mut keep_going = true;
        while keep_going {
            let Some(expr) = self.process_term(ie_filter)? else {
                break;
            };
            let mut op: Option<Expr> = None;
            let mut ie8 = false;
            match self.peek() {
                tk::SLASH => op = Some(Expr::Slash),
                tk::COMMA => op = Some(Expr::Comma),
                tk::BACKSLASH => {
                    // `\9` at the end of the expression: IE8 or earlier.
                    self.next()?;
                    if self.peek_kind(tk::INTEGER) {
                        let num = self.next()?;
                        let value = int_parse(&num.text(self.source))?;
                        if value == 9 {
                            op = Some(Expr::Term(Term::new(
                                TermKind::Ie8,
                                Value::Str("\\9".into()),
                                "\\9".into(),
                            )));
                            ie8 = true;
                        }
                    }
                }
                _ => {}
            }
            match expr {
                TermOrList::List(l) => expressions.expressions.extend(l),
                TermOrList::One(e) => expressions.expressions.push(e),
            }
            if let Some(op) = op {
                expressions.expressions.push(op);
                if ie8 {
                    keep_going = false;
                } else {
                    self.next()?;
                }
            }
        }
        Ok(expressions)
    }

    /// `processTerm`.
    fn process_term(&mut self, ie_filter: bool) -> R<Option<TermOrList>> {
        self.enter()?;
        let r = self.process_term_(ie_filter);
        self.depth -= 1;
        r
    }

    /// The body of [`Self::process_term`].
    fn process_term_(&mut self, ie_filter: bool) -> R<Option<TermOrList>> {
        let start = self.peeked.start;
        let one = |t: Term| Ok(Some(TermOrList::One(Expr::Term(t))));
        match self.peek() {
            tk::HASH => {
                self.eat(tk::HASH)?;
                if !self.any_white_space_before_peek_token(tk::HASH) {
                    let mut hex_text = None;
                    if self.peek_kind(tk::INTEGER) {
                        let hex_text1 = self.peeked_text();
                        self.next()?;
                        // Joins the identifier only if there is no whitespace.
                        if self.peek_identifier() && self.previous_end() == Some(self.peeked.start)
                        {
                            let rest = self.identifier()?.name;
                            hex_text = Some(format!("{hex_text1}{rest}"));
                        } else {
                            hex_text = Some(hex_text1);
                        }
                    } else if self.peek_identifier() {
                        hex_text = Some(self.identifier()?.name);
                    }
                    if let Some(h) = hex_text {
                        return one(self.parse_hex(h));
                    }
                }
                // `#<space>number`: `(processTerm() as LiteralTerm).text`.
                let text = match self.process_term(false)? {
                    Some(TermOrList::One(Expr::Term(t))) => t.text,
                    _ => return Err(Exception("TypeError")),
                };
                one(self.parse_hex(format!(" {text}")))
            }
            tk::INTEGER => {
                let t = self.next()?;
                let value = int_parse(&t.text(self.source))?;
                one(self.process_dimension(Some(&t), Value::Int(value))?)
            }
            tk::DOUBLE => {
                let t = self.next()?;
                let value = double_parse(&t.text(self.source))?;
                one(self.process_dimension(Some(&t), Value::Double(value))?)
            }
            tk::SINGLE_QUOTE => {
                let value = self.process_quoted_string(false)?;
                let value = format!("'{}'", escape_string(&value, true));
                one(Term::new(
                    TermKind::Literal,
                    Value::Str(value.clone()),
                    value,
                ))
            }
            tk::DOUBLE_QUOTE => {
                let value = self.process_quoted_string(false)?;
                let value = format!("\"{}\"", escape_string(&value, false));
                one(Term::new(
                    TermKind::Literal,
                    Value::Str(value.clone()),
                    value,
                ))
            }
            tk::LPAREN => {
                self.next()?;
                let mut group = Vec::new();
                loop {
                    let term = self.process_term(false)?;
                    let null = term.is_none();
                    if let Some(TermOrList::One(Expr::Term(t))) = term {
                        group.push(t);
                    }
                    if null || self.maybe_eat(tk::RPAREN)? || self.is_premature_end_of_file()? {
                        break;
                    }
                }
                Ok(Some(TermOrList::One(Expr::Group(group))))
            }
            tk::LBRACK => {
                self.next()?;
                // `processTerm() as LiteralTerm`.
                let term = match self.process_term(false)? {
                    Some(TermOrList::One(Expr::Term(t))) => t,
                    _ => return Err(Exception("TypeError")),
                };
                if !term.is_number() {
                    self.error();
                }
                self.eat(tk::RBRACK)?;
                one(Term::new(TermKind::Item, term.value, term.text))
            }
            tk::IDENTIFIER => self.process_identifier(ie_filter, start),
            tk::UNICODE_RANGE => {
                let mut first = None;
                let mut second = None;
                self.eat_ur(tk::UNICODE_RANGE, true)?;
                if self.maybe_eat_ur(tk::HEX_INTEGER, true)? {
                    let f = self.previous_text();
                    let first_number = hex_parse(&f)?;
                    if first_number > MAX_UNICODE {
                        self.error();
                    }
                    first = Some(f);
                    if self.maybe_eat_ur(tk::MINUS, true)?
                        && self.maybe_eat_ur(tk::HEX_INTEGER, true)?
                    {
                        let s = self.previous_text();
                        let second_number = hex_parse(&s)?;
                        if second_number > MAX_UNICODE {
                            self.error();
                        }
                        if first_number > second_number {
                            self.error();
                        }
                        second = Some(s);
                    }
                } else if self.maybe_eat_ur(tk::HEX_RANGE, true)? {
                    first = Some(self.previous_text());
                }
                Ok(Some(TermOrList::One(Expr::UnicodeRange { first, second })))
            }
            tk::AT => {
                // Less (`lessSupport`): `property: @name` => `var(name)`.
                self.next()?;
                let mut expr = self.process_expr(false)?;
                let Some(param) = expr.expressions.first() else {
                    return Err(Exception("RangeError"));
                };
                let Expr::Term(param) = param else {
                    return Err(Exception("TypeError"));
                };
                let name = param.text.clone();
                expr.expressions[0] = Expr::VarUsage {
                    name,
                    defaults: Vec::new(),
                };
                Ok(Some(TermOrList::List(expr.expressions)))
            }
            _ => {
                // Synthesized identifier tokens (`PT`, `PX`…).
                if tk::is_kind_identifier(self.peek()) {
                    self.process_identifier(ie_filter, start)
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// `_previousToken!.text`.
    fn previous_text(&self) -> String {
        self.previous
            .as_ref()
            .map(|t| t.text(self.source))
            .unwrap_or_default()
    }

    /// The local `processIdentifier` of `processTerm`.
    fn process_identifier(&mut self, ie_filter: bool, start: usize) -> R<Option<TermOrList>> {
        let name = self.identifier()?;
        if !ie_filter && self.maybe_eat(tk::LPAREN)? {
            if let Some(calc) = self.process_calc(&name)? {
                return Ok(Some(TermOrList::One(Expr::Term(calc))));
            }
            // FUNCTION
            return Ok(Some(TermOrList::One(self.process_function(name)?)));
        }
        if ie_filter {
            // `filter:progid:…` or `filter:<name>`: both official paths end up
            // in the same `processIEFilter` (the `_maybeEat(COLON)` comes
            // before the `&&`).
            self.maybe_eat(tk::COLON)?;
            return Ok(self
                .process_ie_filter(start)?
                .map(|t| TermOrList::One(Expr::Term(t))));
        }
        let text = name.name.clone();
        if text == "from" {
            return Ok(Some(TermOrList::One(Expr::Term(Term::new(
                TermKind::Literal,
                Value::Ident(name),
                text,
            )))));
        }
        // Color name?
        match tk::match_color_name(&text) {
            None => Ok(Some(TermOrList::One(Expr::Term(Term::new(
                TermKind::Literal,
                Value::Ident(name),
                text,
            ))))),
            Some(value) => {
                // The color as an RGB value.
                let rgb = tk::decimal_to_hex(value, 6);
                Ok(Some(TermOrList::One(Expr::Term(self.parse_hex(rgb)))))
            }
        }
    }

    /// `processDimension`.
    fn process_dimension(&mut self, t: Option<&Token>, value: Value) -> R<Term> {
        let unit = self.peek();
        let token_text =
            |p: &Self| -> R<String> { t.map(|t| t.text(p.source)).ok_or(Exception("TypeError")) };
        let kind = match unit {
            tk::UNIT_EM => TermKind::Em,
            tk::UNIT_EX => TermKind::Ex,
            tk::UNIT_LENGTH_PX
            | tk::UNIT_LENGTH_CM
            | tk::UNIT_LENGTH_MM
            | tk::UNIT_LENGTH_IN
            | tk::UNIT_LENGTH_PT
            | tk::UNIT_LENGTH_PC => TermKind::Unit {
                unit,
                is_length: true,
            },
            tk::UNIT_ANGLE_DEG
            | tk::UNIT_ANGLE_RAD
            | tk::UNIT_ANGLE_GRAD
            | tk::UNIT_ANGLE_TURN
            | tk::UNIT_TIME_MS
            | tk::UNIT_TIME_S
            | tk::UNIT_FREQ_HZ
            | tk::UNIT_FREQ_KHZ
            | tk::UNIT_RESOLUTION_DPI
            | tk::UNIT_RESOLUTION_DPCM
            | tk::UNIT_RESOLUTION_DPPX
            | tk::UNIT_CH
            | tk::UNIT_REM
            | tk::UNIT_VIEWPORT_VW
            | tk::UNIT_VIEWPORT_VH
            | tk::UNIT_VIEWPORT_VMIN
            | tk::UNIT_VIEWPORT_VMAX
            | tk::UNIT_LH
            | tk::UNIT_RLH => TermKind::Unit {
                unit,
                is_length: false,
            },
            tk::PERCENT => TermKind::Percentage,
            tk::UNIT_FRACTION => TermKind::Fraction,
            _ => {
                return match value {
                    Value::Ident(i) => {
                        let text = i.name.clone();
                        Ok(Term::new(TermKind::Literal, Value::Ident(i), text))
                    }
                    v => Ok(Term::new(TermKind::Number, v, token_text(self)?)),
                };
            }
        };
        // `span.expand(_next().span)`, then `t!.text`.
        self.next()?;
        Ok(Term::new(kind, value, token_text(self)?))
    }

    /// `processQuotedString`.
    fn process_quoted_string(&mut self, url_string: bool) -> R<String> {
        // The URI swallows everything between quotes or between parentheses.
        let mut stop_token = if url_string { tk::RPAREN } else { -1 };
        // Without skipping whitespace inside the string.
        let in_string = self.tokenizer.in_string;
        self.tokenizer.in_string = false;
        match self.peek() {
            tk::SINGLE_QUOTE => {
                stop_token = tk::SINGLE_QUOTE;
                self.next()?;
            }
            tk::DOUBLE_QUOTE => {
                stop_token = tk::DOUBLE_QUOTE;
                self.next()?;
            }
            _ => {
                if url_string {
                    if self.peek() == tk::LPAREN {
                        self.next()?;
                    }
                    stop_token = tk::RPAREN;
                } else {
                    self.error();
                }
            }
        }
        // Everything up to the stop token.
        let mut value = String::new();
        while self.peek() != stop_token && self.peek() != tk::END_OF_FILE {
            let t = self.next()?;
            value.push_str(&t.text(self.source));
        }
        self.tokenizer.in_string = in_string;
        if stop_token != tk::RPAREN {
            self.next()?; // the quote
        }
        Ok(value)
    }

    /// `processIEFilter`: the raw text of the filter, from the start of the
    /// term up to `;`/`}` or until the parenthesis closes; null at the end of
    /// the file.
    fn process_ie_filter(&mut self, start: usize) -> R<Option<Term>> {
        let literal = |p: &Self| {
            let text = p.tokenizer.text_between(start, p.peeked.start);
            Term::new(TermKind::Literal, Value::Str(text.clone()), text)
        };
        let kind = self.peek();
        if kind == tk::SEMICOLON || kind == tk::RBRACE {
            return Ok(Some(literal(self)));
        }
        let mut parens = 0i64;
        while self.peek() != tk::END_OF_FILE {
            match self.peek() {
                tk::LPAREN => {
                    self.eat(tk::LPAREN)?;
                    parens += 1;
                }
                tk::RPAREN => {
                    self.eat(tk::RPAREN)?;
                    parens -= 1;
                    if parens == 0 {
                        return Ok(Some(literal(self)));
                    }
                }
                k => self.eat(k)?,
            }
        }
        Ok(None)
    }

    /// `processCalcExpression`: the raw text up to the closing `)`.
    fn process_calc_expression(&mut self) -> R<String> {
        let in_string = self.tokenizer.in_string;
        self.tokenizer.in_string = false;
        let mut value = String::new();
        let mut left = 1;
        let mut matching_parens = false;
        while self.peek() != tk::END_OF_FILE && !matching_parens {
            let token = self.peek();
            if token == tk::LPAREN {
                left += 1;
            } else if token == tk::RPAREN {
                left -= 1;
            }
            matching_parens = left == 0;
            if !matching_parens {
                let t = self.next()?;
                value.push_str(&t.text(self.source));
            }
        }
        if !matching_parens {
            self.error();
        }
        self.tokenizer.in_string = in_string;
        Ok(value)
    }

    /// `processCalc`.
    fn process_calc(&mut self, func: &Identifier) -> R<Option<Term>> {
        let name = func.name.as_str();
        if ["calc", "-webkit-calc", "-moz-calc", "min", "max", "clamp"].contains(&name) {
            let expression = self.process_calc_expression()?;
            let calc_expr = Term::new(
                TermKind::Literal,
                Value::Str(expression.clone()),
                expression,
            );
            if !self.maybe_eat(tk::RPAREN)? {
                self.error();
            }
            return Ok(Some(Term::new(
                TermKind::Calc(Box::new(calc_expr)),
                Value::Str(name.to_string()),
                name.to_string(),
            )));
        }
        Ok(None)
    }

    /// `processFunction`: `UriTerm`, `VarUsage` or `FunctionTerm`.
    fn process_function(&mut self, func: Identifier) -> R<Expr> {
        let name = func.name;
        match name.as_str() {
            "url" => {
                let url_param = self.process_quoted_string(true)?;
                if self.peek() == tk::END_OF_FILE {
                    self.error();
                }
                if self.peek() == tk::RPAREN {
                    self.next()?;
                }
                Ok(Expr::Term(Term::new(
                    TermKind::Uri,
                    Value::Str(url_param.clone()),
                    url_param,
                )))
            }
            "var" => {
                let mut expr = self.process_expr(false)?;
                if !self.maybe_eat(tk::RPAREN)? {
                    self.error();
                }
                // `(expr.expressions[0] as LiteralTerm).text`.
                let param_name = match expr.expressions.first() {
                    None => return Err(Exception("RangeError")),
                    Some(Expr::Term(t)) => t.text.clone(),
                    Some(_) => return Err(Exception("TypeError")),
                };
                // [0] - name, [1] - comma, [2..] - default value.
                let defaults = if expr.expressions.len() >= 3 {
                    expr.expressions.split_off(2)
                } else {
                    Vec::new()
                };
                Ok(Expr::VarUsage {
                    name: param_name,
                    defaults,
                })
            }
            _ => {
                let expr = self.process_expr(false)?;
                if !self.maybe_eat(tk::RPAREN)? {
                    self.error();
                }
                Ok(Expr::Term(Term::new(
                    TermKind::Function(expr),
                    Value::Str(name.clone()),
                    name,
                )))
            }
        }
    }

    /// `identifier`: the next token as an identifier (empty if it is not an
    /// identifier; the text is always the token's).
    fn identifier(&mut self) -> R<Identifier> {
        let tok = self.next()?;
        let text = Some(String::from_utf16_lossy(&self.source[tok.start..tok.end]));
        if !tk::is_identifier(tok.kind) && !tk::is_kind_identifier(tok.kind) {
            return Ok(Identifier {
                name: String::new(),
                text,
            });
        }
        Ok(Identifier {
            name: tok.text(self.source),
            text,
        })
    }

    /// `_parseHex`.
    fn parse_hex(&mut self, hex_text: String) -> Term {
        let mut hex_value: i64 = 0;
        for c in hex_text.encode_utf16() {
            let digit = hex_digit(c);
            if digit < 0 {
                self.warning();
                return Term::new(TermKind::Hex, Value::BadHex, hex_text);
            }
            hex_value = hex_value.wrapping_shl(4).wrapping_add(digit);
        }
        // #RRGGBB => #RGB when each pair repeats the digit.
        let b = hex_text.as_bytes();
        let text = if b.len() == 6 && b[0] == b[1] && b[2] == b[3] && b[4] == b[5] {
            format!("{}{}{}", b[0] as char, b[2] as char, b[4] as char)
        } else if b.len() == 4 && b[0] == b[1] && b[2] == b[3] {
            format!("{}{}", b[0] as char, b[2] as char)
        } else if b.len() == 2 && b[0] == b[1] {
            (b[0] as char).to_string()
        } else {
            hex_text
        };
        Term::new(TermKind::Hex, Value::Int(hex_value), text)
    }
}

/// A simple selector named by an `Identifier`, with a position.
fn simple(kind: SimpleKind, name: Identifier) -> std::rc::Rc<SimpleSelector> {
    std::rc::Rc::new(SimpleSelector {
        kind,
        name: SimpleName::Ident(name),
        has_span: true,
    })
}

/// `_Parser._hexDigit`.
fn hex_digit(c: u16) -> i64 {
    match c {
        48..=57 => i64::from(c) - 48,
        97..=102 => i64::from(c) - 87,
        65..=70 => i64::from(c) - 55,
        _ => -1,
    }
}

/// `int.parse` of the text of a numeric token (sign and digits): outside the
/// 64-bit range Dart throws `FormatException`.
fn int_parse(text: &str) -> R<i64> {
    text.parse::<i64>()
        .map_err(|_| Exception("FormatException"))
}

/// `double.parse` of the text of a numeric token.
fn double_parse(text: &str) -> R<f64> {
    text.parse::<f64>()
        .map_err(|_| Exception("FormatException"))
}

/// `int.parse('0x…')`: up to 64 bits (with the sign bit, as in Dart).
fn hex_parse(text: &str) -> R<i64> {
    u64::from_str_radix(text, 16)
        .map(|v| v as i64)
        .map_err(|_| Exception("FormatException"))
}

/// `_escapeString`: escapes the quote that delimits the string.
pub(crate) fn escape_string(text: &str, single: bool) -> String {
    let mut output = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' if !single => output.push_str("\\\""),
            '\'' if single => output.push_str("\\'"),
            _ => output.push(c),
        }
    }
    output
}

/// `_styleForDart` and `buildDartStyleNode`, only where they throw: the styles
/// they build are read by nobody.
fn style_for_dart(property: &Identifier, exprs: &Expressions) -> R<()> {
    let e = &exprs.expressions;
    let first = || e.first().ok_or(Exception("RangeError"));
    match property.name.to_lowercase().as_str() {
        // `font`, `font-family`, `font-size`: the `ExpressionsProcessor` only
        // reads `LengthTerm`, whose value is always a number.
        "font" | "font-family" | "font-size" | "font-style" | "font-variant" => {}
        "font-weight" => {
            // `expr.value as int?` on a `NumberTerm`.
            if let Expr::Term(t) = first()?
                && t.is_number()
                && !matches!(t.value, Value::Int(_))
            {
                return Err(Exception("TypeError"));
            }
        }
        "line-height" => {
            if e.len() == 1
                && let Expr::Term(t) = &e[0]
            {
                let px_or_pt = matches!(
                    t.kind,
                    TermKind::Unit {
                        unit: tk::UNIT_LENGTH_PX | tk::UNIT_LENGTH_PT,
                        ..
                    }
                );
                if px_or_pt || t.is_number() {
                    as_num(&t.value)?;
                }
            }
        }
        "margin" | "padding" => {
            // `processFourNums`.
            if (1..=4).contains(&e.len()) {
                for x in e {
                    margin_value(x)?;
                }
            }
        }
        "border" => {
            for x in e {
                if margin_value(x)?.is_some() {
                    break;
                }
            }
        }
        "border-width" => {
            margin_value(first()?)?;
        }
        "margin-left"
        | "margin-right"
        | "margin-top"
        | "margin-bottom"
        | "border-left"
        | "border-right"
        | "border-top"
        | "border-bottom"
        | "border-left-width"
        | "border-top-width"
        | "border-right-width"
        | "border-bottom-width"
        | "height"
        | "width"
        | "padding-left"
        | "padding-top"
        | "padding-right"
        | "padding-bottom" => {
            if let Some(x) = e.first() {
                margin_value(x)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `value as num`.
fn as_num(value: &Value) -> R<()> {
    match value {
        Value::Int(_) | Value::Double(_) => Ok(()),
        _ => Err(Exception("TypeError")),
    }
}

/// `marginValue`: the number of a `UnitTerm` or `NumberTerm`.
fn margin_value(expr: &Expr) -> R<Option<()>> {
    match expr {
        Expr::Term(t) if matches!(t.kind, TermKind::Unit { .. }) || t.is_number() => {
            as_num(&t.value)?;
            Ok(Some(()))
        }
        _ => Ok(None),
    }
}
