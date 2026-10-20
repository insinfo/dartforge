//! Dart 3.6 parser that builds the [`Tree`] with the same shapes as the
//! analyzer 3.6 `parseString` (fasta's parser plus the `AstBuilder`, without
//! resolution or `AstRewriter`).
//!
//! It is a recursive descent parser that ports fasta's decisions
//! (`_fe_analyzer_shared` 76, `parser_impl.dart` and `type_info*.dart`) point
//! by point, because the formatter tells apart the nodes the analyzer
//! produces: the same text `a.b()` becomes `MethodInvocation`, `a.b()()`
//! becomes `FunctionExpressionInvocation`, `A<T>.b()` becomes
//! `InstanceCreationExpression` and so on. The modules follow the grammar's
//! layout:
//!
//! * `lookahead` — side-effect-free *lookahead*: `computeType`,
//!   `computeTypeParamOrArg`, `skipOuterPattern` and the like;
//! * `types` — type annotations, type parameters and arguments, formal
//!   parameters and metadata;
//! * `expressions` — expressions, literals, cascades and collections;
//! * `statements` — statements and local declarations;
//! * `patterns` — patterns (Dart 3);
//! * `declarations` — unit, directives and top-level and member declarations;
//! * [`verify`] — checks on the tree (token coverage, bounds) and the text
//!   dump used to compare against the analyzer.
//!
//! The parser does not change the token list while it works: the `>` that
//! form `>>`, `>=`, `>>>`, `>>=` and `>>>=` in expression context, and the
//! `[` `]` (`=`) of the operator names `[]`/`[]=`, are recorded in
//! `Parser::merges` and only merged at the end, remapping the nodes' token
//! indices. That way the *lookahead* and the speculation
//! (`canParseAsConditional`) work on stable indices.
//!
//! Errors: any error the analyzer would report and that can be detected
//! without resolution becomes an [`SyntaxError`]; the formatter rejects the
//! file, like `dart format`. There is no error recovery.

mod declarations;
mod expressions;
mod lookahead;
mod patterns;
mod statements;
mod types;
pub mod verify;

use dartforge_frontend::token::{Keyword, Op};

use super::token::{SyntaxError, Tk, Token, TokenKind, Tokens};
use super::tree::{Node, NodeId, Tree};

/// Result of the parser functions.
type R<T> = Result<T, SyntaxError>;

/// Maximum nesting depth of expressions and statements, like fasta's 500
/// (`expressionDepth`/`statementDepth`), which reports `StackOverflow`
/// beyond that.
const MAX_DEPTH: u32 = 500;

/// fasta precedences (`token.dart`).
mod prec {
    pub const ASSIGNMENT: u8 = 1;
    pub const CASCADE: u8 = 2;
    pub const CONDITIONAL: u8 = 3;
    pub const IF_NULL: u8 = 4;
    pub const OR: u8 = 5;
    pub const E: u8 = 6;
    pub const EQUALITY: u8 = 7;
    pub const RELATIONAL: u8 = 8;
    pub const BIT_OR: u8 = 9;
    pub const BIT_XOR: u8 = 10;
    pub const BIT_AND: u8 = 11;
    pub const SHIFT: u8 = 12;
    pub const ADDITIVE: u8 = 13;
    pub const MULTIPLICATIVE: u8 = 14;
    pub const PREFIX: u8 = 15;
    pub const POSTFIX: u8 = 16;
    pub const SELECTOR: u8 = 17;
}

/// `async` state of the body being parsed (fasta's `AsyncModifier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncKind {
    SyncKind,
    Plain,
    SyncStar,
    Star,
}

/// Where we are with respect to loops (fasta's `LoopState`), for `break` and
/// `continue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopState {
    Outside,
    Switch,
    Repetition,
}

/// A token merge deferred to the end of parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Merge {
    /// Composite `>` ([`Tokens::merge_greater`]).
    Greater(&'static str),
    /// Glued `[` `]` or `[` `]` `=`: the analyzer scanner's `INDEX`/`INDEX_EQ`
    /// token, made of `n` of our tokens.
    Brackets(usize),
}

/// Parse options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Language version (major, minor). Before 3.0 there are no patterns: the
    /// `switch` `case`s are expressions ([`Node::SwitchCase`]). A
    /// `// @dart=X.Y` comment before the first declaration takes precedence, as
    /// in the analyzer's scanner.
    pub version: (u32, u32),
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions { version: (3, 6) }
    }
}

/// Reads the whole `source` as a compilation unit, at language version 3.6.
///
/// ```
/// use dartforge_formatter::syntax::tree::Node;
/// use dartforge_formatter::syntax::parser::parse;
/// let (_tokens, tree) = parse("main() => a.b();").unwrap();
/// let root = tree.root.unwrap();
/// assert!(matches!(tree.node(root), Node::CompilationUnit { .. }));
/// ```
///
/// # Errors
/// The first syntax error found.
pub fn parse(source: &str) -> Result<(Tokens, Tree), SyntaxError> {
    parse_with(source, ParseOptions::default())
}

/// Like [`parse`], with an explicit language version.
///
/// ```
/// use dartforge_formatter::syntax::parser::{parse_with, ParseOptions};
/// let source = "f(x) { switch (x) { case 1: break; } }";
/// assert!(parse_with(source, ParseOptions { version: (2, 19) }).is_ok());
/// ```
///
/// # Errors
/// The first syntax error found.
pub fn parse_with(source: &str, options: ParseOptions) -> Result<(Tokens, Tree), SyntaxError> {
    let tokens = Tokens::read(source)?;
    let version = comment_version(source, &tokens).unwrap_or(options.version);
    let mut p = Parser::new(source, tokens)?;
    p.patterns_ = version >= (3, 0);
    p.version = version;
    let root = p.unit()?;
    if let Some(e) = p.version_error.take() {
        return Err(e);
    }
    p.tree_.root = Some(root);
    let Parser {
        mut toks,
        mut tree_,
        merges,
        ..
    } = p;
    apply_merges(&mut toks, &mut tree_, &merges);
    tree_.link_parents();
    Ok((toks, tree_))
}

/// The `// @dart = X.Y` before the first token (the scanner's
/// `LanguageVersionToken`).
fn comment_version(source: &str, tokens: &Tokens) -> Option<(u32, u32)> {
    let first = tokens.list.first()?;
    let target_ = if first.kind_ == TokenKind::ScriptTag {
        tokens.list.get(1)?
    } else {
        first
    };
    let (a, b) = target_.comments;
    for c in &tokens.comments[a as usize..b as usize] {
        if !c.is_line {
            continue;
        }
        let text = &source[c.start + 2..c.end];
        let t = text.trim_start();
        let Some(rest) = t.strip_prefix("@dart") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim();
        let mut parts_ = rest.splitn(2, '.');
        let greater = parts_.next()?.parse().ok()?;
        let smaller = parts_.next()?.parse().ok()?;
        return Some((greater, smaller));
    }
    None
}

/// Applies the deferred merges and remaps the tree's token indices.
fn apply_merges(toks: &mut Tokens, tree_: &mut Tree, merges: &[(usize, Merge)]) {
    if merges.is_empty() {
        return;
    }
    let n = toks.list.len();
    // `removed[i]` = how many tokens before `i` (exclusive) disappear.
    // `owner[i]`: for an absorbed token, the first token of the merge.
    let mut owner: Vec<Option<usize>> = vec![None; n];
    for &(i, f) in merges {
        let k = match f {
            Merge::Greater(a) => a.len(),
            Merge::Brackets(k) => k,
        };
        for d in owner.iter_mut().skip(i + 1).take(k - 1) {
            *d = Some(i);
        }
    }
    let mut new = vec![0u32; n];
    let mut removed = 0u32;
    for i in 0..n {
        match owner[i] {
            // A node that ends at an absorbed token ends at the merged token.
            Some(d) => {
                new[i] = new[d];
                removed += 1;
            }
            None => new[i] = i as u32 - removed,
        }
    }
    let mut sorted: Vec<(usize, Merge)> = merges.to_vec();
    sorted.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (i, f) in sorted {
        match f {
            Merge::Greater(a) => toks.merge_greater(i as Tk, a),
            Merge::Brackets(k) => {
                let end = toks.list[i + k - 1].end;
                let glued = toks.list[i + k - 1].glued;
                toks.list.drain(i + 1..i + k);
                toks.list[i].end = end;
                toks.list[i].glued = glued;
            }
        }
    }
    let map = |t: &mut Tk| *t = new[*t as usize];
    for d in &mut tree_.nodes {
        d.start = new[d.start as usize];
        d.end = new[d.end as usize];
        verify::tokens_mut(&mut d.node, &mut |t| map(t));
    }
}

/// fasta's word class for built-in identifiers and pseudo-keywords
/// (`KeywordStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordStyle {
    BuiltIn,
    Pseudo,
    NoneKind,
}

fn word_style(text: &str) -> WordStyle {
    match text {
        "abstract" | "as" | "covariant" | "deferred" | "dynamic" | "export" | "extension"
        | "external" | "factory" | "Function" | "get" | "implements" | "import" | "interface"
        | "late" | "library" | "mixin" | "operator" | "part" | "required" | "set" | "static"
        | "typedef" => WordStyle::BuiltIn,
        "async" | "await" | "base" | "hide" | "inout" | "native" | "of" | "on" | "out"
        | "patch" | "sealed" | "show" | "source" | "sync" | "when" | "yield" => WordStyle::Pseudo,
        _ => WordStyle::NoneKind,
    }
}

/// O cursor e o estado do parser.
pub(crate) struct Parser<'f> {
    source: &'f str,
    toks: Tokens,
    /// For each `(`, `[`, `{` and `${`, the index of the matching closer.
    pairs: Vec<u32>,
    pos: usize,
    tree_: Tree,
    merges: Vec<(usize, Merge)>,
    async_kind: AsyncKind,
    loop_: LoopState,
    /// `mayParseFunctionExpressions` do fasta.
    functions: bool,
    /// fasta's `allowPatterns` (version >= 3.0).
    patterns_: bool,
    /// Language version in effect.
    version: (u32, u32),
    /// First use of a feature above the version in effect.
    version_error: Option<SyntaxError>,
    depth: u32,
    /// `isLastPatternAllowedInsideUnaryPattern` do fasta.
    unary_pattern_ok: bool,
}

impl<'f> Parser<'f> {
    fn new(source: &'f str, toks: Tokens) -> R<Parser<'f>> {
        let n = toks.list.len();
        let mut pairs = vec![u32::MAX; n];
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in toks.list.iter().enumerate() {
            match t.kind_ {
                TokenKind::Op(Op::LParen | Op::LBracket | Op::LBrace)
                | TokenKind::ExpressionInterpolation => stack.push(i),
                TokenKind::Op(o @ (Op::RParen | Op::RBracket | Op::RBrace)) => {
                    let Some(a) = stack.pop() else {
                        return Err(token_error(
                            source,
                            t,
                            &format!("Unexpected '{}'.", o.text()),
                        ));
                    };
                    let expected = match toks.list[a].kind_ {
                        TokenKind::Op(Op::LParen) => Op::RParen,
                        TokenKind::Op(Op::LBracket) => Op::RBracket,
                        _ => Op::RBrace,
                    };
                    if expected != o {
                        return Err(token_error(
                            source,
                            &toks.list[a],
                            &format!("Expected '{}' to match.", expected.text()),
                        ));
                    }
                    pairs[a] = i as u32;
                    pairs[i] = a as u32;
                }
                _ => {}
            }
        }
        if let Some(&a) = stack.last() {
            return Err(token_error(source, &toks.list[a], "Unbalanced bracket."));
        }
        check_gaps(source, &toks)?;
        Ok(Parser {
            source,
            toks,
            pairs,
            pos: 0,
            tree_: Tree::default(),
            merges: Vec::new(),
            async_kind: AsyncKind::SyncKind,
            loop_: LoopState::Outside,
            functions: true,
            patterns_: true,
            version: (3, 6),
            version_error: None,
            depth: 0,
            unary_pattern_ok: false,
        })
    }

    // ------------------------------------------------------------ tokens

    fn tk(&self, i: usize) -> &Token {
        let n = self.toks.list.len();
        &self.toks.list[i.min(n - 1)]
    }

    fn kind_(&self, i: usize) -> TokenKind {
        self.tk(i).kind_
    }

    fn txt(&self, i: usize) -> &'f str {
        let t = self.tk(i);
        &self.source[t.start..t.end]
    }

    fn op(&self, i: usize, o: Op) -> bool {
        self.kind_(i) == TokenKind::Op(o)
    }

    fn kw(&self, i: usize, k: Keyword) -> bool {
        self.kind_(i) == TokenKind::Reserved(k)
    }

    /// fasta's `isIdentifier`: a plain, built-in or pseudo identifier.
    fn ident(&self, i: usize) -> bool {
        self.kind_(i) == TokenKind::Identifier
    }

    /// `kind == IDENTIFIER_TOKEN`: an identifier that is neither built-in nor a
    /// pseudo-keyword.
    fn plain_ident(&self, i: usize) -> bool {
        self.ident(i) && self.word_style_at(i) == WordStyle::NoneKind
    }

    fn built_in(&self, i: usize) -> bool {
        self.ident(i) && self.word_style_at(i) == WordStyle::BuiltIn
    }

    fn pseudo(&self, i: usize) -> bool {
        self.ident(i) && self.word_style_at(i) == WordStyle::Pseudo
    }

    /// Identifier with text `s` (contextual words).
    fn word(&self, i: usize, s: &str) -> bool {
        self.ident(i) && self.txt(i) == s
    }

    /// `isKeywordOrIdentifier`.
    fn word_(&self, i: usize) -> bool {
        matches!(
            self.kind_(i),
            TokenKind::Identifier | TokenKind::Reserved(_)
        )
    }

    fn eof(&self, i: usize) -> bool {
        self.kind_(i) == TokenKind::Eof
    }

    fn text(&self, i: usize) -> bool {
        self.kind_(i) == TokenKind::Text
    }

    /// Closer of the group opened at `i`.
    fn pair(&self, i: usize) -> usize {
        let p = self.pairs.get(i).copied().unwrap_or(u32::MAX);
        if p == u32::MAX {
            self.toks.list.len() - 1
        } else {
            p as usize
        }
    }

    /// fasta's `isModifier` (`modifier_context.dart`): built-ins are only
    /// modifiers before another word (or before a record type followed by a
    /// name).
    fn modifier(&self, i: usize) -> bool {
        let t = self.txt(i);
        // Before null safety these contextual words remain ordinary names,
        // including types named `late` and named parameters called `required`.
        if self.version < (2, 12) && matches!(t, "late" | "required") {
            return false;
        }
        match self.kind_(i) {
            TokenKind::Reserved(Keyword::Const | Keyword::Final | Keyword::Var) => true,
            TokenKind::Identifier
                if matches!(
                    t,
                    "abstract" | "covariant" | "external" | "late" | "required" | "static"
                ) =>
            {
                let n = i + 1;
                if !self.word_(n) || self.kw(n, Keyword::In) {
                    if self.op(n, Op::LParen) {
                        let after = self.pair(n) + 1;
                        let this_super = |p: &Self, k: usize| {
                            (p.kw(k, Keyword::This) || p.kw(k, Keyword::Super))
                                && p.op(k + 1, Op::Dot)
                        };
                        if self.ident(after) || this_super(self, after) {
                            return true;
                        }
                        if self.op(after, Op::Question)
                            && (self.ident(after + 1) || this_super(self, after + 1))
                        {
                            return true;
                        }
                    }
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// User-definable operator starting at `i` and how many of our tokens it
    /// takes (`isUserDefinableOperator`).
    fn user_definable_operator(&self, i: usize) -> Option<usize> {
        match self.kind_(i) {
            TokenKind::Op(
                Op::EqEq
                | Op::Lt
                | Op::LtEq
                | Op::Minus
                | Op::Plus
                | Op::Slash
                | Op::TildeSlash
                | Op::Star
                | Op::Percent
                | Op::Pipe
                | Op::Caret
                | Op::Amp
                | Op::LtLt
                | Op::Tilde,
            ) => Some(1),
            TokenKind::Op(Op::Gt) => match self.toks.composed_greater(i as Tk) {
                None => Some(1),
                Some(">>") => Some(2),
                Some(">=") => Some(2),
                Some(">>>") => Some(3),
                Some(_) => None,
            },
            TokenKind::Op(Op::LBracket) if self.op(i + 1, Op::RBracket) && self.tk(i).glued => {
                if self.op(i + 2, Op::Assign) && self.tk(i + 1).glued {
                    Some(3)
                } else {
                    Some(2)
                }
            }
            _ => None,
        }
    }

    // --------------------------------------------------------------- errors

    fn error_at(&self, i: usize, msg: &str) -> SyntaxError {
        token_error(self.source, self.tk(i), msg)
    }

    fn error(&self, msg: &str) -> SyntaxError {
        self.error_at(self.pos, msg)
    }

    fn expected(&self, o: &str) -> SyntaxError {
        let found = self.txt(self.pos);
        self.error(&format!("Expected '{o}' before '{found}'."))
    }

    // ------------------------------------------------------------ cursor

    fn advance(&mut self) -> usize {
        let i = self.pos;
        self.pos += 1;
        i
    }

    fn expect_op(&mut self, o: Op) -> R<usize> {
        if self.op(self.pos, o) {
            Ok(self.advance())
        } else {
            Err(self.expected(o.text()))
        }
    }

    fn expect_kw(&mut self, k: Keyword) -> R<usize> {
        if self.kw(self.pos, k) {
            Ok(self.advance())
        } else {
            Err(self.expected(k.text()))
        }
    }

    fn expect_word(&mut self, s: &str) -> R<usize> {
        if self.word(self.pos, s) {
            Ok(self.advance())
        } else {
            Err(self.expected(s))
        }
    }

    fn optional_op(&mut self, o: Op) -> Option<usize> {
        if self.op(self.pos, o) {
            Some(self.advance())
        } else {
            None
        }
    }

    /// `ensureIdentifier` for names: an identifier (any style).
    fn expect_ident(&mut self) -> R<usize> {
        if self.ident(self.pos) {
            Ok(self.advance())
        } else {
            Err(self.error(&format!(
                "Expected an identifier, but got '{}'.",
                self.txt(self.pos)
            )))
        }
    }

    /// Identifier or `new` (in the continuations where fasta rewrites `new` as
    /// an identifier: `A.new`).
    fn expect_ident_or_new(&mut self) -> R<usize> {
        if self.kw(self.pos, Keyword::New) {
            self.feature(self.pos, (2, 15), "constructor-tearoffs");
            return Ok(self.advance());
        }
        self.expect_ident()
    }

    /// Declared name (variable, parameter, local function): besides being an
    /// identifier, `await`/`yield` are not allowed outside a plain synchronous
    /// body (`checkAsyncAwaitYieldAsIdentifier`).
    fn expect_declared_name(&mut self) -> R<usize> {
        let i = self.expect_ident()?;
        self.check_await_yield(i)?;
        Ok(i)
    }

    fn check_await_yield(&self, i: usize) -> R<()> {
        if self.async_kind != AsyncKind::SyncKind && matches!(self.txt(i), "await" | "yield") {
            return Err(self.error_at(i, "'await' and 'yield' can't be used as identifiers here."));
        }
        Ok(())
    }

    /// Records the merge of the `>` at `i` with the following ones, forming
    /// `target_`.
    fn merge_greater_at(&mut self, i: usize, target_: &'static str) {
        if target_.starts_with(">>>") {
            self.feature(i, (2, 14), "triple-shift");
        }
        self.merges.push((i, Merge::Greater(target_)));
    }

    /// Consumes a user-definable operator (an `operator` name or a `#` symbol),
    /// recording the needed merges; returns the token.
    fn consume_user_definable_operator(&mut self) -> R<usize> {
        let i = self.pos;
        let Some(n) = self.user_definable_operator(i) else {
            return Err(self.error("Expected an operator."));
        };
        if self.op(i, Op::Gt) && n > 1 {
            let target_ = self.toks.composed_greater(i as Tk).unwrap_or(">");
            self.merge_greater_at(i, target_);
        } else if self.op(i, Op::LBracket) {
            self.merges.push((i, Merge::Brackets(n)));
        }
        self.pos += n;
        Ok(i)
    }

    /// Language feature that requires version `minimum`: before it the analyzer
    /// reports `EXPERIMENT_NOT_ENABLED`, which is a syntax error. The first
    /// error is stored and returned at the end of parsing.
    fn feature(&mut self, i: usize, minimum: (u32, u32), name_: &str) {
        if self.version < minimum && self.version_error.is_none() {
            self.version_error = Some(self.error_at(
                i,
                &format!("This requires the '{name_}' language feature to be enabled."),
            ));
        }
    }

    // ---------------------------------------------------------------- nodes

    /// Creates the node `node` from `start_` up to the last consumed token.
    fn node(&mut self, node: Node, start_: usize) -> NodeId {
        let end = self.pos.saturating_sub(1).max(start_);
        self.tree_.push(node, start_ as Tk, end as Tk)
    }

    fn node_between(&mut self, node: Node, start_: usize, end: usize) -> NodeId {
        self.tree_.push(node, start_ as Tk, end as Tk)
    }

    fn start_(&self, id: NodeId) -> usize {
        self.tree_.start(id) as usize
    }

    fn end(&self, id: NodeId) -> usize {
        self.tree_.end(id) as usize
    }

    fn simple(&mut self, i: usize) -> NodeId {
        self.node_between(Node::SimpleIdentifier { token: i as Tk }, i, i)
    }

    fn is_simple(&self, id: NodeId) -> bool {
        matches!(self.tree_.node(id), Node::SimpleIdentifier { .. })
    }

    /// Runs `f` as a probe: returns the position reached if `f` succeeded and
    /// undoes every effect (nodes, merges, cursor and state).
    fn probe<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> Option<usize> {
        let pos = self.pos;
        let nodes = self.tree_.nodes.len();
        let merges = self.merges.len();
        let (async_kind, loop_, functions, prof, unary) = (
            self.async_kind,
            self.loop_,
            self.functions,
            self.depth,
            self.unary_pattern_ok,
        );
        let r = f(self);
        let reached = self.pos;
        self.pos = pos;
        self.tree_.nodes.truncate(nodes);
        self.merges.truncate(merges);
        self.async_kind = async_kind;
        self.loop_ = loop_;
        self.functions = functions;
        self.depth = prof;
        self.unary_pattern_ok = unary;
        r.ok().map(|_| reached)
    }

    fn enter_(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("The file has too many nested expressions or statements."));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }
}

/// Only whitespace and comments may appear between two tokens; the
/// analyzer's scanner reports any other character (`Illegal character`),
/// like the non-breaking space.
fn check_gaps(source: &str, toks: &Tokens) -> R<()> {
    let b = source.as_bytes();
    let mut ant = 0usize;
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        ant = 3;
    }
    for t in &toks.list {
        let (c0, c1) = t.comments;
        let mut p = ant;
        let mut cs = toks.comments[c0 as usize..c1 as usize].iter().peekable();
        while p < t.start {
            if let Some(c) = cs.peek()
                && c.start == p
            {
                p = c.end;
                cs.next();
                continue;
            }
            if !matches!(b[p], b' ' | b'\t' | b'\n' | b'\r' | 0x0C) {
                let ch = source[p..].chars().next().unwrap_or(' ');
                return Err(SyntaxError {
                    offset: p,
                    length: ch.len_utf8(),
                    message: format!("Illegal character '{}'.", ch as u32),
                });
            }
            p += 1;
        }
        if t.kind_ == TokenKind::ScriptTag {
            ant = t.end;
        } else {
            ant = ant.max(t.end);
        }
    }
    Ok(())
}

fn token_error(_source: &str, t: &Token, msg: &str) -> SyntaxError {
    SyntaxError {
        offset: t.start,
        length: (t.end - t.start).max(1),
        message: msg.to_string(),
    }
}
