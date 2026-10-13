//! The analysis server 3.6.2 `InScopeCompletionPass`
//! (`AS:src/services/completion/dart/in_scope_completion_pass.dart`, read in
//! full; docs/LSP-ESPECIFICACAO.md §14.13 and §14.14).
//!
//! The context is decided over the tree of the **real text** in the
//! analyzer's shape ([`crate::arvore_analyzer`]): the `coveringNode` of
//! `unit.select` (§14.13.4, with the two boundary exceptions), the
//! `_completionNode` (walking up while the parent starts at the same token
//! and the child is not in a list) and one `visit*` per node kind
//! (`isp_visits`), which records the `completionLocation` and calls the
//! helpers (keywords, declarations, identifiers, labels, overrides, URIs) in
//! Dart's order: the order in which the candidates reach the collector is
//! the order of the response among equals.
//!
//! The semantics (the static type of a target, the element of an
//! identifier, the declared elements) come from the [`Query`], which may
//! have analyzed the text with the sentinel in place of the word under the
//! cursor: the offsets of the real text are carried to its own through the
//! sentinel's shift ([`Isp::map`]); what comes before the cursor has the
//! same offset.
//!
//! What the DartForge parser's recovery does not reproduce from fasta
//! (zero-length synthetic tokens, the `droppedToken`) is read as "absent": a
//! token fasta would create as synthetic is looked for in the text and, if
//! it is not there, the position of the next real token counts (where fasta
//! would put it).

use super::*;
use crate::arvore_analyzer::{Arvore, Ligacao};
use dartforge_elements::model::UnitId;
use dartforge_frontend::token::{Kind, Op, Token};

/// The lexemes the fasta scanner reads as keywords (`KeywordToken`:
/// reserved, built-in and pseudo-keywords): for them `type == IDENTIFIER` is
/// false and `isKeyword` is true.
pub(super) const ANALYZER_WORDS: &[&str] = &[
    "abstract", "as", "assert", "async", "augment", "await", "base", "break", "case", "catch", "class", "const", "continue", "covariant", "default",
    "deferred", "do", "dynamic", "else", "enum", "export", "extends", "extension", "external", "factory", "false", "final", "finally", "for",
    "Function", "get", "hide", "if", "implements", "import", "in", "inout", "interface", "is", "late", "library", "mixin", "native", "new", "null",
    "of", "on", "operator", "out", "part", "patch", "required", "rethrow", "return", "sealed", "set", "show", "source", "static", "super", "switch",
    "sync", "this", "throw", "true", "try", "typedef", "var", "void", "when", "while", "with", "yield",
];

/// The node kinds that are an `Expression` in the analyzer.
pub(super) const EXPRESSION_KINDS: &[&str] = &[
    "AdjacentStrings", "AsExpression", "AssignmentExpression", "AwaitExpression", "BinaryExpression", "BooleanLiteral", "CascadeExpression",
    "ConditionalExpression", "ConstructorReference", "DotShorthand", "DoubleLiteral", "ExtensionOverride", "FunctionExpression", "FunctionExpressionInvocation",
    "FunctionReference", "IndexExpression", "InstanceCreationExpression", "IntegerLiteral", "IsExpression", "ListLiteral", "MethodInvocation",
    "NamedExpression", "NullLiteral", "ParenthesizedExpression", "PatternAssignment", "PostfixExpression", "PrefixExpression",
    "PrefixedIdentifier", "PropertyAccess", "RecordLiteral", "RethrowExpression", "SetOrMapLiteral", "SimpleIdentifier",
    "SimpleStringLiteral", "StringInterpolation", "SuperExpression", "SwitchExpression", "SymbolLiteral", "ThisExpression",
    "ThrowExpression", "TypeLiteral",
];

/// The kind is a `DartPattern`.
pub(super) fn is_pattern(e: &str) -> bool {
    matches!(
        e,
        "CastPattern" | "ConstantPattern" | "DeclaredVariablePattern" | "ListPattern" | "LogicalAndPattern" | "LogicalOrPattern" | "MapPattern"
            | "NullAssertPattern" | "NullCheckPattern" | "ObjectPattern" | "ParenthesizedPattern" | "RecordPattern" | "RelationalPattern"
            | "WildcardPattern" | "AssignedVariablePattern"
    )
}

/// The kind is a declaration with a member body (a `Declaration` where an
/// `@override` annotation applies).
pub(super) fn is_declaration(e: &str) -> bool {
    matches!(e, "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration")
}

/// The kind is a `Statement`.
pub(super) fn is_command(e: &str) -> bool {
    matches!(
        e,
        "AssertStatement" | "Block" | "BreakStatement" | "ContinueStatement" | "DoStatement" | "EmptyStatement" | "ExpressionStatement"
            | "ForStatement" | "FunctionDeclarationStatement" | "IfStatement" | "LabeledStatement" | "PatternVariableDeclarationStatement"
            | "ReturnStatement" | "SwitchStatement" | "TryStatement" | "VariableDeclarationStatement" | "WhileStatement" | "YieldStatement"
    )
}

/// The kind is a `FormalParameter`.
pub(super) fn is_parameter(e: &str) -> bool {
    matches!(e, "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter")
}

/// A language feature that completion consults (`featureSet`), by the
/// version it came in.
#[derive(Debug, Clone, Copy)]
pub(super) enum Feature {
    ExtensionMethods,
    EnhancedEnums,
    SuperParameters,
    Patterns,
    ClassModifiers,
    SealedClass,
    InlineClass,
}

/// The options of the `DeclarationHelper` (the first call to
/// `declarationHelper` fixes them; the following ones ignore them).
#[derive(Debug, Clone, Default)]
pub(super) struct Flags {
    pub assignable: bool,
    pub constant: bool,
    pub extensible: bool,
    pub implementable: bool,
    pub mixable: bool,
    pub non_void: bool,
    pub is_static: bool,
    pub ty: bool,
    pub exclude_type_names: bool,
    pub object_pattern: bool,
    pub prefer_without_invocation: bool,
    pub unnamed_as_new: bool,
    /// The excluded nodes (`excludedNodes`): phantom type parameter lists from
    /// recovery.
    pub excluded: Vec<usize>,
}

/// A `NotImportedCompletionPass` operation registered by the
/// `DeclarationHelper`.
#[derive(Debug, Clone)]
pub(super) enum Operation {
    /// `StaticMembersOperation`.
    StaticMembers,
    /// `ConstructorsOperation`.
    Constructors,
    /// `InstanceExtensionMembersOperation`.
    ExtensionMembers { ty: TypeId, excluded: Vec<String>, methods: bool, setters: bool },
}

/// The result of the pass.
pub(super) struct ResultIsp {
    /// The candidates in collector order and each one's `matcherScore`.
    pub items: Vec<(f64, CompletionItem)>,
    pub local: Option<String>,
    pub incomplete: bool,
    pub prefers_constants: bool,
    pub operations: Vec<Operation>,
    /// The options of the `DeclarationHelper` used (those of the second pass).
    pub flags: Option<Flags>,
    /// The names seen by the `VisibilityTracker` (with the import prefix).
    pub visible: HashSet<String>,
    /// `isCommentText`: the cursor is in an ordinary comment (empty list).
    pub comment: bool,
    /// `replacementRange`: `(start, length)`.
    pub range: (usize, usize),
    /// The `targetPrefix` (the handler's filter and truncation).
    pub request_prefix: String,
    /// The matcher's prefix (`TokenData.fromSelection`).
    pub matcher_prefix: String,
    /// `request.contextType`.
    pub context_type: Option<TypeId>,
    /// `request.inConstantContext`.
    pub in_constant_context: bool,
    /// The not-imported libraries the query did not load (their
    /// `NotImportedCompletionPass` goes through the name summary):
    /// `(path, import URI)`.
    pub summarized: Vec<(PathBuf, String)>,
}

/// The `beginToken` of a node: a documentation comment, a text token, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Initial {
    Comment,
    Tok(usize),
    None,
}

pub(super) struct Isp<'a, 'c> {
    /// The real text, its parser tree (with the names) and the analyzer tree.
    pub source: &'a str,
    pub a: &'a ast::Ast,
    pub cu: &'a ast::CompilationUnit,
    pub nr: &'a Interner,
    pub tree: &'a Arvore,
    pub toks: Vec<Token>,
    pub offset: usize,
    pub features: LibraryFeatures,
    /// The semantics.
    pub query: &'c mut Query,
    pub unit: UnitId,
    /// The span replaced by the sentinel in the query's text: `(start, end in
    /// the real text, sentinel length, shift after the span)`.
    pub swap: Option<(usize, usize, usize, isize)>,
    /// The query's expressions by range.
    pub exprs: HashMap<(usize, usize), ExprId>,
    /// The item collector (the completion item builders).
    pub collector: Collector,
    pub scores: Vec<f64>,
    pub local: Option<String>,
    pub prefers_constants: bool,
    pub incomplete: bool,
    pub flags: Option<Flags>,
    /// `VisibilityTracker._declaredNames`.
    pub visible: HashSet<String>,
    /// `_variableDistance`.
    pub distance: u32,
    /// `IdentifierHelper.includePrivateIdentifiers` (the first call).
    pub private: Option<bool>,
    pub matcher: Option<crate::fuzzy_matcher::Matcher>,
    pub operations: Vec<Operation>,
    /// The file path (the suggested name for the top-level declaration).
    pub path: Option<PathBuf>,
    /// `state.contextType`, when the request computed it.
    pub context_type: Option<TypeId>,
    /// `_containingMemberName`: the name of the method containing a `super.▮`.
    pub super_method: Option<String>,
}

/// Runs the pass: builds the tree of the real text, finds the target (the
/// `CompletionTarget`), the range, the prefixes and the context type, and
/// visits the completion node.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    query: &mut Query,
    unit: UnitId,
    text: &str,
    features: LibraryFeatures,
    offset: usize,
    swap: Option<(usize, usize, usize, isize)>,
    before_3: bool,
    not_imported: Option<&mut dyn FnMut() -> Vec<(PathBuf, String)>>,
) -> ResultIsp {
    let mut names = Interner::new();
    let analyzed = dartforge_frontend::parser::parse_with(text, &mut names, features);
    let tree = crate::arvore_analyzer::construir(text, &analyzed.ast, &analyzed.unit, before_3);
    let toks = dartforge_frontend::lexer::lex(text).unwrap_or_default();
    let mut exprs = HashMap::new();
    for (i, e) in query.program.unit(unit).ast.exprs.iter().enumerate() {
        exprs.entry((e.span.start, e.span.end)).or_insert(ExprId(i as u32));
    }
    let library = query.program.unit(unit).library;
    let path = query.program.unit(unit).path.clone();
    let mut isp = Isp {
        source: text,
        a: &analyzed.ast,
        cu: &analyzed.unit,
        nr: &names,
        tree: &tree,
        toks,
        offset,
        features,
        query,
        unit,
        swap,
        exprs,
        collector: Collector { items: Vec::new(), library, receiver: None },
        scores: Vec::new(),
        local: None,
        prefers_constants: false,
        incomplete: false,
        flags: None,
        visible: HashSet::new(),
        distance: 0,
        private: None,
        matcher: None,
        operations: Vec::new(),
        path,
        context_type: None,
        super_method: None,
    };
    let target = isp.target();
    if target.comment_text {
        return ResultIsp {
            items: Vec::new(),
            local: None,
            incomplete: false,
            prefers_constants: false,
            operations: Vec::new(),
            flags: None,
            visible: HashSet::new(),
            comment: true,
            range: (offset, 0),
            request_prefix: String::new(),
            matcher_prefix: String::new(),
            context_type: None,
            in_constant_context: false,
            summarized: Vec::new(),
        };
    }
    let range = isp.replacement_range(&target);
    let request_prefix = isp.request_prefix(&target);
    let matcher_prefix = isp.matcher_prefix();
    let in_constant_context = isp.request_in_constant_context(&target);
    let context = isp.context_type_in(target.node, offset);
    isp.context_type = context;
    isp.matcher = (!matcher_prefix.is_empty()).then(|| crate::fuzzy_matcher::Matcher::new(&matcher_prefix, crate::fuzzy_matcher::Style::Text));
    if let Some(node) = isp.completion_node() {
        isp.visit(node);
    }
    // The `NotImportedCompletionPass`: the registered operations, over each
    // candidate library (the loaded ones through the elements; the others
    // through the summary, outside of here).
    let mut summarized = Vec::new();
    if !isp.operations.is_empty()
        && !request_prefix.is_empty()
        && let Some(candidates) = not_imported
    {
        let key = |c: &std::path::Path| dartforge_elements::generated::key(c);
        let mut by_path: HashMap<PathBuf, LibraryId> = HashMap::new();
        for (i, l) in isp.query.program.libraries.iter().enumerate() {
            if let Some(&u) = l.units.first()
                && let Some(c) = isp.query.program.unit(u).path.as_deref()
            {
                by_path.insert(key(c), LibraryId(i as u32));
            }
        }
        for (path, uri) in candidates() {
            match by_path.get(&key(&path)) {
                Some(&l) => {
                    let before = isp.collector.items.len();
                    isp.operations_not_imported(l, &uri);
                    let _ = before;
                }
                None => summarized.push((path, uri)),
            }
        }
    }
    let Isp { collector, scores, local, incomplete, prefers_constants, operations, flags, visible, .. } = isp;
    let items = collector.items.into_iter().zip(scores).map(|(i, s)| (s, i)).collect();
    ResultIsp {
        items,
        local,
        incomplete,
        prefers_constants,
        operations,
        flags,
        visible,
        comment: false,
        range,
        request_prefix,
        matcher_prefix,
        context_type: context,
        in_constant_context,
        summarized,
    }
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- The collector -------------------------------------------------------------

    /// `featureSet.isEnabled(feature)`.
    pub(super) fn has_feature(&self, r: Feature) -> bool {
        let v = match r {
            Feature::ExtensionMethods => (2, 6),
            Feature::EnhancedEnums | Feature::SuperParameters => (2, 17),
            Feature::Patterns | Feature::ClassModifiers | Feature::SealedClass => (3, 0),
            Feature::InlineClass => (3, 3),
        };
        let current = self.features.version();
        (current.major, current.minor) >= v
    }

    /// `state.matcher.score(text)`: 0 without a prefix (`NoPrefixMatcher`), −1
    /// when it does not match.
    pub(super) fn score(&mut self, text: &str) -> f64 {
        match self.matcher.as_mut() {
            Some(c) => c.score(text),
            None => 0.0,
        }
    }

    /// Records the items the collector gained since `before` with the score.
    pub(super) fn register(&mut self, before: usize, score: f64) {
        while self.scores.len() < self.collector.items.len() {
            self.scores.push(score);
        }
        let _ = before;
    }

    /// The `completionLocation` (plain assignment).
    pub(super) fn local(&mut self, l: &str) {
        self.local = Some(l.to_string());
    }

    /// `completionLocation ??= l`.
    pub(super) fn local_if_empty(&mut self, l: &str) {
        if self.local.is_none() {
            self.local = Some(l.to_string());
        }
    }

    // -- Nodes ---------------------------------------------------------------------

    pub(super) fn kind(&self, n: usize) -> &'static str {
        self.tree.nos[n].especie
    }

    pub(super) fn parent(&self, n: usize) -> Option<usize> {
        self.tree.nos[n].pai
    }

    pub(super) fn children(&self, n: usize) -> &[usize] {
        &self.tree.nos[n].filhos
    }

    pub(super) fn start(&self, n: usize) -> usize {
        self.tree.nos[n].inicio
    }

    pub(super) fn end(&self, n: usize) -> usize {
        self.tree.nos[n].fim
    }

    pub(super) fn link(&self, n: usize) -> Ligacao {
        self.tree.nos[n].ligacao
    }

    /// The first child of the kind.
    pub(super) fn child(&self, n: usize, kind: &str) -> Option<usize> {
        self.children(n).iter().copied().find(|&f| self.kind(f) == kind)
    }

    /// The children of one of the kinds.
    pub(super) fn children_of(&self, n: usize, kinds: &[&str]) -> Vec<usize> {
        self.children(n).iter().copied().filter(|&f| kinds.contains(&self.kind(f))).collect()
    }

    /// The closest ancestor (itself included) of one of the kinds.
    pub(super) fn ancestor(&self, n: usize, kinds: &[&str]) -> Option<usize> {
        let mut current = Some(n);
        while let Some(k) = current {
            if kinds.contains(&self.kind(k)) {
                return Some(k);
            }
            current = self.parent(k);
        }
        None
    }

    /// `coversOffset`: `start <= offset <= end`.
    pub(super) fn covers(&self, start: usize, end: usize) -> bool {
        start <= self.offset && self.offset <= end
    }

    pub(super) fn covers_node(&self, n: usize) -> bool {
        self.covers(self.start(n), self.end(n))
    }

    /// `elementBefore(offset)`: the last one with `end <= offset`.
    pub(super) fn element_before(&self, nodes: &[usize]) -> Option<usize> {
        nodes.iter().copied().filter(|&k| self.end(k) <= self.offset).last()
    }

    // -- Tokens ------------------------------------------------------------------

    /// The index of the first token that starts at `off` or after.
    pub(super) fn tok_since(&self, off: usize) -> usize {
        self.toks.partition_point(|t| t.span.start < off)
    }

    /// The token that starts exactly at `off`.
    pub(super) fn tok_at(&self, off: usize) -> Option<usize> {
        let i = self.tok_since(off);
        self.toks.get(i).filter(|t| t.span.start == off && t.kind != Kind::Eof).map(|_| i)
    }

    /// The last token that ends at or before `off`.
    pub(super) fn tok_before(&self, off: usize) -> Option<usize> {
        self.toks.partition_point(|t| t.span.end <= off).checked_sub(1)
    }

    /// The first token that starts at `off` or after (excluding EOF).
    pub(super) fn tok_after(&self, off: usize) -> Option<usize> {
        let i = self.tok_since(off);
        self.toks.get(i).filter(|t| t.kind != Kind::Eof).map(|_| i)
    }

    pub(super) fn span_tok(&self, i: usize) -> Span {
        self.toks[i].span
    }

    pub(super) fn text_tok(&self, i: usize) -> &'a str {
        self.toks[i].text(self.source)
    }

    /// `isKeywordOrIdentifier`.
    pub(super) fn word(&self, i: usize) -> bool {
        matches!(self.toks.get(i).map(|t| t.kind), Some(Kind::Ident | Kind::Keyword(_)))
    }

    /// `type.isKeyword` (fasta's `KeywordToken`).
    pub(super) fn is_keyword(&self, i: usize) -> bool {
        self.word(i) && ANALYZER_WORDS.contains(&self.text_tok(i))
    }

    /// `type == TokenType.IDENTIFIER`.
    pub(super) fn is_identifier(&self, i: usize) -> bool {
        self.word(i) && !self.is_keyword(i)
    }

    pub(super) fn is_op(&self, i: usize, op: Op) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Kind::Op(op))
    }

    /// The first `op` token in `[from, until)`, outside parentheses and brackets
    /// opened after `from`.
    pub(super) fn op_in(&self, op: Op, from: usize, until: usize) -> Option<usize> {
        let mut i = self.tok_since(from);
        let mut bottom = 0i32;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= until || t.kind == Kind::Eof {
                return None;
            }
            if bottom == 0 && t.kind == Kind::Op(op) {
                return Some(i);
            }
            match t.kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => bottom += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => bottom -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// The first token with the text in `[from, until)` (at the outer level).
    pub(super) fn word_in(&self, text: &str, from: usize, until: usize) -> Option<usize> {
        let mut i = self.tok_since(from);
        let mut bottom = 0i32;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= until || t.kind == Kind::Eof {
                return None;
            }
            if bottom == 0 && matches!(t.kind, Kind::Ident | Kind::Keyword(_)) && t.text(self.source) == text {
                return Some(i);
            }
            match t.kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => bottom += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => bottom -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// The pair of a `(`/`[`/`{` (or the reverse).
    pub(super) fn pair(&self, i: usize) -> Option<usize> {
        let (opens, closes, to_front) = match self.toks.get(i)?.kind {
            Kind::Op(Op::LParen) => (Op::LParen, Op::RParen, true),
            Kind::Op(Op::LBracket) => (Op::LBracket, Op::RBracket, true),
            Kind::Op(Op::LBrace) => (Op::LBrace, Op::RBrace, true),
            Kind::Op(Op::RParen) => (Op::LParen, Op::RParen, false),
            Kind::Op(Op::RBracket) => (Op::LBracket, Op::RBracket, false),
            Kind::Op(Op::RBrace) => (Op::LBrace, Op::RBrace, false),
            _ => return None,
        };
        let mut bottom = 0i32;
        if to_front {
            let mut j = i;
            while let Some(t) = self.toks.get(j) {
                if t.kind == Kind::Op(opens) {
                    bottom += 1;
                } else if t.kind == Kind::Op(closes) {
                    bottom -= 1;
                    if bottom == 0 {
                        return Some(j);
                    }
                }
                j += 1;
            }
            None
        } else {
            let mut j = i as isize;
            while j >= 0 {
                let t = self.toks[j as usize];
                if t.kind == Kind::Op(closes) {
                    bottom += 1;
                } else if t.kind == Kind::Op(opens) {
                    bottom -= 1;
                    if bottom == 0 {
                        return Some(j as usize);
                    }
                }
                j -= 1;
            }
            None
        }
    }

    /// The `offset` of a token fasta would create as synthetic when it is not in
    /// the text: that of the next real token after `after`.
    pub(super) fn synthetic(&self, after: usize) -> Span {
        let p = self.tok_after(after).map_or(self.source.len(), |i| self.span_tok(i).start);
        Span { start: p, end: p }
    }

    /// The node's `beginToken`.
    pub(super) fn initial(&self, n: usize) -> Initial {
        let start = self.start(n);
        if n == 0 {
            return self.tok_after(0).map_or(Initial::None, Initial::Tok);
        }
        let rest = &self.source[start.min(self.source.len())..];
        if rest.starts_with("//") || rest.starts_with("/*") {
            return Initial::Comment;
        }
        match self.tok_at(start) {
            Some(i) => Initial::Tok(i),
            None => self.tok_after(start).map_or(Initial::None, Initial::Tok),
        }
    }

    /// The node's first token (after comments and annotations, when
    /// `after_annotations`): `firstTokenAfterCommentAndMetadata`.
    pub(super) fn first_after_annotations(&self, n: usize) -> Option<usize> {
        let mut after = self.start(n);
        for &f in self.children(n) {
            if matches!(self.kind(f), "Comment" | "Annotation") {
                after = after.max(self.end(f));
            }
        }
        self.tok_after(after).filter(|&i| self.span_tok(i).start < self.end(n).max(after + 1))
    }

    /// `isFullySynthetic` here: the node has no token in the text.
    pub(super) fn all_synthetic(&self, n: usize) -> bool {
        self.start(n) >= self.end(n)
    }

    // -- The completion node (§14.13.4) -------------------------------------------

    /// `contains(node)` of `nodeCovering`, with the two boundary exceptions.
    fn contains(&self, n: usize) -> bool {
        let (start, end) = (self.start(n), self.end(n));
        if self.offset == start
            && let Some(i) = self.tok_before(start)
            && self.span_tok(i).end == self.offset
            && self.is_identifier(i)
        {
            return false;
        }
        if self.offset == end
            && let Some(i) = self.tok_after(end)
            && self.span_tok(i).start == self.offset
            && self.is_identifier(i)
        {
            return false;
        }
        start <= self.offset && self.offset <= end
    }

    /// `unit.select(offset, 0).coveringNode`.
    pub(super) fn covered_node(&self) -> Option<usize> {
        if self.offset > self.source.len() {
            return None;
        }
        let mut n = 0usize;
        loop {
            match self.children(n).iter().copied().find(|&f| self.contains(f)) {
                Some(f) => n = f,
                None => return Some(n),
            }
        }
    }

    /// `isChildInList` (`in_scope_completion_pass.dart:3964-4048`).
    pub(super) fn child_in_list(&self, parent: usize, child: usize) -> bool {
        let f = self.kind(child);
        let annotation = f == "Annotation";
        match self.kind(parent) {
            "AdjacentStrings" | "ArgumentList" | "Block" | "FormalParameterList" | "RecordLiteral" | "HideCombinator" | "ShowCombinator"
            | "DottedName" | "LibraryIdentifier" | "TypeArgumentList" | "TypeParameterList" | "ImplementsClause" | "WithClause"
            | "MixinOnClause" | "RecordTypeAnnotationNamedFields" => true,
            "CascadeExpression" => self.children(parent).first() != Some(&child),
            "ClassDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration" => {
                annotation || matches!(f, "FieldDeclaration" | "MethodDeclaration" | "ConstructorDeclaration")
            }
            "EnumDeclaration" => annotation || matches!(f, "EnumConstantDeclaration" | "FieldDeclaration" | "MethodDeclaration" | "ConstructorDeclaration"),
            "Comment" => f == "CommentReference",
            "CompilationUnit" => f != "ScriptTag",
            "ConstructorDeclaration" => {
                annotation || matches!(f, "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer")
            }
            "ExportDirective" | "ImportDirective" => annotation || matches!(f, "ShowCombinator" | "HideCombinator" | "Configuration"),
            "ClassTypeAlias" | "DeclaredIdentifier" | "EnumConstantDeclaration" | "FieldDeclaration" | "ForEachPartsWithPattern"
            | "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter"
            | "DefaultFormalParameter" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "LibraryDirective"
            | "MethodDeclaration" | "PartDirective" | "PartOfDirective" | "PatternVariableDeclaration" | "RecordTypeAnnotationNamedField"
            | "RecordTypeAnnotationPositionalField" | "RepresentationDeclaration" | "TopLevelVariableDeclaration" | "TypeParameter"
            | "VariableDeclaration" => annotation,
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                self.tree.partes_de_for.get(&parent).is_some_and(|p| p.atualizacoes.contains(&child))
            }
            "LabeledStatement" => f == "Label",
            "ListLiteral" | "SetOrMapLiteral" | "ListPattern" | "MapPattern" => f != "TypeArgumentList",
            "ObjectPattern" | "RecordPattern" => f == "PatternField",
            "RecordTypeAnnotation" => f == "RecordTypeAnnotationPositionalField",
            "SwitchExpression" => f == "SwitchExpressionCase",
            "SwitchDefault" => true,
            "SwitchPatternCase" => f != "GuardedPattern",
            // Old `case e:`: the labels and the statements; the expression (the first
            // child that is not a label) is not.
            "SwitchCase" => self.children(parent).iter().copied().find(|&k| self.kind(k) != "Label") != Some(child),
            "SwitchStatement" => matches!(f, "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"),
            "TryStatement" => f == "CatchClause",
            "VariableDeclarationList" => annotation || f == "VariableDeclaration",
            _ => false,
        }
    }

    /// `_completionNode` (`in_scope_completion_pass.dart:123-153`).
    pub(super) fn completion_node(&self) -> Option<usize> {
        let c = self.covered_node()?;
        let b = self.initial(c);
        let b_word = matches!(b, Initial::Tok(i) if self.word(i));
        if !b_word && self.kind(c) != "SimpleIdentifier" {
            return Some(c);
        }
        // `isCoveredByToken(beginToken)`.
        let covered = match b {
            Initial::Tok(i) => self.covers(self.span_tok(i).start, self.span_tok(i).end),
            _ => false,
        };
        if !covered {
            return Some(c);
        }
        let mut child = c;
        let mut parent = self.parent(c);
        while let Some(p) = parent {
            if self.initial(p) != b || (self.kind(child) != "SimpleIdentifier" && self.child_in_list(p, child)) {
                break;
            }
            child = p;
            parent = self.parent(child);
        }
        if let Some(p) = parent
            && !(self.kind(child) != "SimpleIdentifier" && self.child_in_list(p, child))
        {
            return Some(p);
        }
        Some(child)
    }

    // -- Binding to the real AST ---------------------------------------------------

    pub(super) fn real_expr(&self, n: usize) -> Option<ExprId> {
        match self.link(n) {
            Ligacao::Expr(e) => Some(e),
            _ => None,
        }
    }

    pub(super) fn real_stmt(&self, n: usize) -> Option<&'a ast::Stmt> {
        match self.link(n) {
            Ligacao::Stmt(s) => Some(self.a.stmt(s)),
            _ => None,
        }
    }

    pub(super) fn real_decl(&self, n: usize) -> Option<&'a ast::Decl> {
        match self.link(n) {
            Ligacao::Decl(d) => Some(self.a.decl(d)),
            _ => None,
        }
    }

    pub(super) fn real_member(&self, n: usize) -> Option<&'a ast::Member> {
        match self.link(n) {
            Ligacao::Membro(m) => Some(self.a.member(m)),
            _ => None,
        }
    }

    /// The function of a `MethodDeclaration`, `FunctionDeclaration` or
    /// `FunctionExpression`.
    pub(super) fn real_function(&self, n: usize) -> Option<&'a ast::Function> {
        match self.link(n) {
            Ligacao::Funcao(f) => Some(self.a.function(f)),
            Ligacao::Decl(d) => match &self.a.decl(d).kind {
                ast::DeclKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Membro(m) => match &self.a.member(m).kind {
                ast::MemberKind::Method(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Expr(e) => match &self.a.expr(e).kind {
                ast::ExprKind::FunctionExpression(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                ast::StmtKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The constructor of a `ConstructorDeclaration`.
    pub(super) fn real_constructor(&self, n: usize) -> Option<&'a ast::Constructor> {
        match &self.real_member(n)?.kind {
            ast::MemberKind::Constructor(k) => Some(k),
            _ => None,
        }
    }

    /// The variable list of a `VariableDeclarationList` (through the parent).
    pub(super) fn real_list(&self, n: usize) -> Option<&'a ast::VariableList> {
        let p = if self.kind(n) == "VariableDeclarationList" { self.parent(n)? } else { n };
        match self.link(p) {
            Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                ast::StmtKind::Variables(l) => Some(l),
                ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                _ => None,
            },
            Ligacao::Decl(d) => match &self.a.decl(d).kind {
                ast::DeclKind::Variables(l) => Some(l),
                _ => None,
            },
            Ligacao::Membro(m) => match &self.a.member(m).kind {
                ast::MemberKind::Field(l) => Some(l),
                _ => None,
            },
            _ => {
                // `ForPartsWithDeclarations` (of a statement or an element).
                let grandparent = self.parent(p)?;
                match self.link(grandparent) {
                    Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                        ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                        _ => None,
                    },
                    Ligacao::Elemento(_) => self.real_element(grandparent).and_then(|el| match el {
                        ast::CollectionElement::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                        _ => None,
                    }),
                    _ => None,
                }
            }
        }
    }

    /// The variable of a `VariableDeclaration` (by name).
    pub(super) fn real_variable(&self, n: usize) -> Option<&'a ast::Variable> {
        let list = self.real_list(self.parent(n)?)?;
        list.variables.iter().find(|v| v.name.span.start == self.start(n))
    }

    /// The `ast::Parameter` of a parameter node.
    pub(super) fn real_parameter(&self, n: usize) -> Option<&'a ast::Parameter> {
        let Ligacao::Parametro(end) = self.link(n) else { return None };
        let find = |ps: &'a [ast::Parameter]| ps.iter().find(|p| crate::arvore_analyzer::endereco(*p) == end);
        for f in self.a.functions.iter() {
            if let Some(ps) = &f.parameters {
                if let Some(p) = find(ps) {
                    return Some(p);
                }
                for p in ps.iter() {
                    if let Some(fps) = &p.function_parameters
                        && let Some(q) = find(fps)
                    {
                        return Some(q);
                    }
                }
            }
        }
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind
                && let Some(p) = find(&k.parameters)
            {
                return Some(p);
            }
        }
        for t in self.a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind
                && let Some(p) = find(parameters)
            {
                return Some(p);
            }
        }
        for d in self.a.decls.iter() {
            if let ast::DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
                && let Some(p) = find(parameters)
            {
                return Some(p);
            }
        }
        None
    }

    /// The `ast::Initializer` of an initializer node.
    pub(super) fn real_initializer(&self, n: usize) -> Option<&'a ast::Initializer> {
        let Ligacao::Inicializador(end) = self.link(n) else { return None };
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind
                && let Some(i) = k.initializers.iter().find(|i| crate::arvore_analyzer::endereco(*i) == end)
            {
                return Some(i);
            }
        }
        None
    }

    /// The `ast::SwitchCase` of a `switch` member.
    pub(super) fn real_case(&self, n: usize) -> Option<&'a ast::SwitchCase> {
        let Ligacao::Caso(end) = self.link(n) else { return None };
        for s in self.a.stmts.iter() {
            if let ast::StmtKind::Switch { cases, .. } = &s.kind
                && let Some(c) = cases.iter().find(|c| crate::arvore_analyzer::endereco(*c) == end)
            {
                return Some(c);
            }
        }
        None
    }

    /// The `ast::Directive`.
    pub(super) fn real_directive(&self, n: usize) -> Option<&'a ast::Directive> {
        let Ligacao::Diretiva(end) = self.link(n) else { return None };
        self.cu.directives.iter().find(|d| crate::arvore_analyzer::endereco(*d) == end)
    }

    /// The `ast::CatchClause`.
    pub(super) fn real_catch(&self, n: usize) -> Option<&'a ast::CatchClause> {
        let Ligacao::Catch(end) = self.link(n) else { return None };
        for s in self.a.stmts.iter() {
            if let ast::StmtKind::Try { catches, .. } = &s.kind
                && let Some(c) = catches.iter().find(|c| crate::arvore_analyzer::endereco(*c) == end)
            {
                return Some(c);
            }
        }
        None
    }

    /// The `ast::CollectionElement`.
    pub(super) fn real_element(&self, n: usize) -> Option<&'a ast::CollectionElement> {
        let Ligacao::Elemento(end) = self.link(n) else { return None };
        fn at<'x>(els: &'x [ast::CollectionElement], end: usize) -> Option<&'x ast::CollectionElement> {
            for el in els {
                if crate::arvore_analyzer::endereco(el) == end {
                    return Some(el);
                }
                let inside: Vec<&ast::CollectionElement> = match el {
                    ast::CollectionElement::If { then, else_, .. } => std::iter::once(&**then).chain(else_.as_deref()).collect(),
                    ast::CollectionElement::For { body, .. } | ast::CollectionElement::ForIn { body, .. } => vec![&**body],
                    _ => Vec::new(),
                };
                for d in inside {
                    if let Some(x) = at(std::slice::from_ref(d), end) {
                        return Some(x);
                    }
                }
            }
            None
        }
        for e in self.a.exprs.iter() {
            if let ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } = &e.kind
                && let Some(x) = at(elements, end)
            {
                return Some(x);
            }
        }
        None
    }

    /// The `ast::Arguments` of an `ArgumentList`.
    pub(super) fn real_arguments(&self, n: usize) -> Option<&'a ast::Arguments> {
        let Ligacao::Argumentos(end) = self.link(n) else { return None };
        let e = |a: &'a ast::Arguments| (crate::arvore_analyzer::endereco(a) == end).then_some(a);
        for x in self.a.exprs.iter() {
            match &x.kind {
                ast::ExprKind::Call { arguments, .. } | ast::ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(a) = e(arguments) {
                        return Some(a);
                    }
                }
                _ => {}
            }
        }
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } = i
                        && let Some(a) = e(arguments)
                    {
                        return Some(a);
                    }
                }
            }
        }
        for d in self.a.decls.iter() {
            if let ast::DeclKind::Enum(x) = &d.kind {
                for k in x.constants.iter() {
                    if let Some(a) = k.arguments.as_ref().and_then(e) {
                        return Some(a);
                    }
                }
            }
        }
        let annot = |ms: &'a [ast::Annotation]| ms.iter().find_map(|m| m.arguments.as_ref().and_then(e));
        for d in self.a.decls.iter() {
            if let Some(a) = annot(&d.metadata) {
                return Some(a);
            }
        }
        for m in self.a.members.iter() {
            if let Some(a) = annot(&m.metadata) {
                return Some(a);
            }
        }
        None
    }

    /// The name of an `ast::Name` of the real text.
    pub(super) fn real_name(&self, n: ast::Name) -> &'a str {
        self.nr.resolve(n.sym)
    }

    // -- Semantics (the query) --------------------------------------------------

    /// The offset of a span **start** of the real text in the query's text:
    /// before the replaced word, the same; inside it, the start of the
    /// sentinel; from its end on, shifted (with an empty word, what starts at
    /// the cursor is the next real token).
    pub(super) fn map(&self, off: usize) -> usize {
        match self.swap {
            None => off,
            Some((start, end, _, delta)) => {
                if off < start || (off == start && start < end) {
                    off
                } else if off >= end {
                    (off as isize + delta) as usize
                } else {
                    start
                }
            }
        }
    }

    /// The offset of a span **end**: what ends inside or at the end of the
    /// replaced word ends at the end of the sentinel.
    pub(super) fn map_end(&self, off: usize) -> usize {
        match self.swap {
            None => off,
            Some((start, end, size, delta)) => {
                if off <= start {
                    off
                } else if off <= end && start < end {
                    start + size
                } else {
                    (off as isize + delta) as usize
                }
            }
        }
    }

    /// The query's expression for the real expression `e`.
    pub(super) fn query_expr(&self, e: ExprId) -> Option<ExprId> {
        let s = self.a.expr(e).span;
        self.exprs.get(&(self.map(s.start), self.map_end(s.end))).copied()
    }

    /// `staticType` of the real expression `e`.
    pub(super) fn static_type(&self, e: ExprId) -> Option<TypeId> {
        let c = self.query_expr(e)?;
        self.query.bodies.units[self.unit.0 as usize].get_type(c)
    }

    /// `staticElement` of the real expression `e`.
    pub(super) fn resolved(&self, e: ExprId) -> Option<Resolved> {
        let c = self.query_expr(e)?;
        self.query.bodies.units[self.unit.0 as usize].get_resolved(c).cloned()
    }

    /// The type of a local or a local function parameter by the name offset.
    pub(super) fn local_type(&self, name: usize) -> Option<TypeId> {
        self.query.bodies.units[self.unit.0 as usize].tipos_de_locais.get(&self.map(name)).copied()
    }

    /// The class (or mixin, enum, extension type) declared with the name at
    /// `name` (real offset).
    pub(super) fn class_in(&self, name: usize) -> Option<ClassId> {
        let target = self.map(name);
        let p = &self.query.program;
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| {
            p.class(c).decl.is_some_and(|d| {
                d.unit == self.unit
                    && match &p.unit(d.unit).ast.decl(d.decl).kind {
                        ast::DeclKind::Class(x) => x.name.span.start == target,
                        ast::DeclKind::Mixin(x) => x.name.span.start == target,
                        ast::DeclKind::Enum(x) => x.name.span.start == target,
                        ast::DeclKind::ExtensionType(x) => x.name.span.start == target,
                        _ => false,
                    }
            })
        })
    }

    /// The extension declared with the name at `name` (or, unnamed, starting at
    /// `start`).
    pub(super) fn extension_in(&self, start: usize, name: Option<usize>) -> Option<dartforge_elements::model::ExtensionId> {
        let p = &self.query.program;
        (0..p.extensions.len()).map(|i| dartforge_elements::model::ExtensionId(i as u32)).find(|&x| {
            let ex = p.extension(x);
            ex.decl.unit == self.unit
                && match (&p.unit(ex.decl.unit).ast.decl(ex.decl.decl).kind, name) {
                    (ast::DeclKind::Extension(e), Some(n)) => e.name.is_some_and(|k| k.span.start == self.map(n)),
                    (ast::DeclKind::Extension(_), None) => p.unit(ex.decl.unit).ast.decl(ex.decl.decl).span.start >= self.map(start),
                    _ => false,
                }
        })
    }

    /// The class whose declaration contains the node (the
    /// `enclosingInterfaceElement` with the declared element).
    pub(super) fn node_class(&self, n: usize) -> Option<ClassId> {
        let d = self.real_decl(n)?;
        let name = match &d.kind {
            ast::DeclKind::Class(x) => x.name,
            ast::DeclKind::Mixin(x) => x.name,
            ast::DeclKind::Enum(x) => x.name,
            ast::DeclKind::ExtensionType(x) => x.name,
            _ => return None,
        };
        self.class_in(name.span.start)
    }

    /// The element of the constructor declared in member `n`.
    pub(super) fn declared_constructor(&self, n: usize) -> Option<FunctionElementId> {
        let k = self.real_constructor(n)?;
        let target = self.map(k.name.unwrap_or(k.class_name).span.start);
        let p = &self.query.program;
        (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| match p.function(f).node {
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => {
                unit == self.unit
                    && match &p.unit(unit).ast.member(member).kind {
                        ast::MemberKind::Constructor(c) => c.name.unwrap_or(c.class_name).span.start == target,
                        _ => false,
                    }
            }
            _ => false,
        })
    }

    /// The element of the function (top-level, method, local) declared with the
    /// name at `name`.
    pub(super) fn declared_function(&self, name: usize) -> Option<FunctionElementId> {
        let target = self.map(name);
        let p = &self.query.program;
        (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                unit == self.unit && p.unit(unit).ast.function(function).name.is_some_and(|n| n.span.start == target)
            }
            _ => false,
        })
    }

    /// The request's library.
    pub(super) fn library(&self) -> LibraryId {
        self.query.program.unit(self.unit).library
    }

    // -- Context properties ------------------------------------------------

    /// `inStaticContext` (`:3935-3947`): walks up the parents; the first method
    /// says whether it is static; a constructor's body is instance; the root is
    /// static.
    pub(super) fn static_context(&self, n: usize) -> bool {
        let mut current = self.parent(n);
        while let Some(k) = current {
            match self.kind(k) {
                "MethodDeclaration" => {
                    return self.real_function(k).is_some_and(|f| f.static_);
                }
                "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"
                    if self.parent(k).is_some_and(|p| self.kind(p) == "ConstructorDeclaration") =>
                {
                    return false;
                }
                _ => {}
            }
            current = self.parent(k);
        }
        true
    }

    /// The node starts with the `const` keyword.
    pub(super) fn starts_with_const(&self, n: usize) -> bool {
        matches!(self.initial(n), Initial::Tok(i) if self.text_tok(i) == "const")
    }

    /// `inConstantContext` (`constantContext(includeSelf: false)`,
    /// `AN:src/dart/ast/ast.dart:6208-6262`): walks up the parents; an
    /// annotation, enum constant arguments and an old `case` are a constant
    /// context; a creation, a record and a typed literal with `const` too; a
    /// constant pattern only with `const`, and the variable list decides (with
    /// `const` or not); an argument, an expression, a collection element and a
    /// variable declaration pass it on; any other node stops.
    pub(super) fn constant_context(&self, n: usize) -> bool {
        let mut current = self.parent(n);
        while let Some(k) = current {
            let e = self.kind(k);
            match e {
                "Annotation" | "EnumConstantArguments" | "SwitchCase" => return true,
                "ConstantPattern" => return self.starts_with_const(k),
                "InstanceCreationExpression" | "RecordLiteral" | "ListLiteral" | "SetOrMapLiteral" => {
                    if self.starts_with_const(k) {
                        return true;
                    }
                }
                "VariableDeclarationList" => return self.real_list(k).is_some_and(|l| l.const_),
                "ArgumentList" | "IfElement" | "ForElement" | "MapLiteralEntry" | "SpreadElement" | "VariableDeclaration" => {}
                _ if EXPRESSION_KINDS.contains(&e) => {}
                _ => return false,
            }
            current = self.parent(k);
        }
        false
    }

    /// The `FunctionBody` that contains the node (itself included:
    /// `thisOrAncestorOfType<FunctionBody>`).
    pub(super) fn containing_body(&self, n: usize) -> Option<usize> {
        self.ancestor(n, &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"])
    }

    /// The modifier of the body containing the node (`body.keyword`, `body.star`).
    fn modifier(&self, n: usize) -> Option<ast::AsyncModifier> {
        let body = self.containing_body(n)?;
        let owner = self.parent(body)?;
        match self.kind(owner) {
            "FunctionExpression" | "MethodDeclaration" | "FunctionDeclaration" => self.real_function(owner).map(|f| f.modifier),
            _ => Some(ast::AsyncModifier::None),
        }
    }

    /// `inAsyncMethodOrFunction`: the body is `async` without `*`.
    pub(super) fn in_async(&self, n: usize) -> bool {
        self.modifier(n) == Some(ast::AsyncModifier::Async)
    }

    /// `inAsyncStarOrSyncStarMethodOrFunction`.
    pub(super) fn in_generator(&self, n: usize) -> bool {
        matches!(self.modifier(n), Some(ast::AsyncModifier::AsyncStar | ast::AsyncModifier::SyncStar))
    }

    /// `inLoop` (`inDoLoop || inForLoop || inWhileLoop`): some ancestor (itself
    /// included) is a loop; does not stop at functions.
    pub(super) fn in_loop(&self, n: usize) -> bool {
        self.ancestor(n, &["ForStatement", "WhileStatement", "DoStatement"]).is_some()
    }

    /// `inSwitch`: some ancestor is a `switch` statement.
    pub(super) fn in_switch(&self, n: usize) -> bool {
        self.ancestor(n, &["SwitchStatement"]).is_some()
    }

    /// `inCatchClause`.
    pub(super) fn in_catch(&self, n: usize) -> bool {
        self.ancestor(n, &["CatchClause"]).is_some()
    }

    /// The previous statement in the same `Block` (`precedingStatement`).
    pub(super) fn previous_command(&self, n: usize) -> Option<usize> {
        let p = self.parent(n)?;
        if self.kind(p) != "Block" {
            return None;
        }
        let fs = self.children(p);
        let i = fs.iter().position(|&k| k == n)?;
        i.checked_sub(1).map(|j| fs[j])
    }

    /// The previous member in the class body (`precedingMember`).
    pub(super) fn previous_member(&self, n: usize) -> Option<usize> {
        let p = self.parent(n)?;
        if !matches!(self.kind(p), "ClassDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration") {
            return None;
        }
        let members = self.children_of(p, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
        let i = members.iter().position(|&k| k == n)?;
        i.checked_sub(1).map(|j| members[j])
    }

    /// `sortedDirectivesAndDeclarations` of the unit.
    pub(super) fn unit_members(&self) -> Vec<usize> {
        self.children(0).iter().copied().filter(|&k| self.kind(k) != "ScriptTag").collect()
    }

    /// `membersBeforeAndAfterMember`.
    pub(super) fn member_neighbors(&self, m: usize) -> (Option<usize>, Option<usize>) {
        let ms = self.unit_members();
        let Some(i) = ms.iter().position(|&k| k == m) else { return (None, None) };
        (i.checked_sub(1).map(|j| ms[j]), ms.get(i + 1).copied())
    }

    /// `membersBeforeAndAfterOffset`.
    pub(super) fn offset_neighbors(&self) -> (Option<usize>, Option<usize>) {
        let mut previous = None;
        for k in self.unit_members() {
            if self.offset < self.start(k) {
                return (previous, Some(k));
            }
            previous = Some(k);
        }
        (previous, None)
    }

    pub(super) fn is_directive(&self, n: usize) -> bool {
        self.kind(n).ends_with("Directive")
    }
}
