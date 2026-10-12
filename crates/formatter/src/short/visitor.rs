//! Porte de `short/source_visitor.dart` (`SourceVisitor`): percorre todos os
//! tokens da árvore e passa ao [`ChunkBuilder`] o texto, os espaços, as quebras
//! possíveis e os comentários.
//!
//! Cada `visitX` do original virou `visit_x`; os auxiliares mantêm os nomes
//! (`_visitCollectionLiteral` → `visit_collection_literal`). As correções
//! opcionais (`--fix`) não são portadas: o `dart format` só as aplica quando
//! pedidas.

use std::collections::{HashMap, HashSet};

use super::arena::{Arenas, ChunkId, RuleId, cost, recuo};
use super::builder::{SourceComment, ChunkBuilder, CommentKind};
use crate::syntax::tree::{Tree, Node, NodeId, ParameterKind};
use crate::syntax::token::{TokenKind, Tk, Tokens};
use dartforge_frontend::token::{Keyword, Op};

/// Ação entre (ou antes/depois de) nós de uma lista, como as funções passadas
/// a `visitNodes` no original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Space,
    Newline,
    Split,
    ZeroSplit,
    SplitOrNewline,
    SplitOrTwoNewlines,
    OneOrTwoNewlines,
}

/// Linhas da fonte (`LineInfo`), com a mesma regra do analyzer: `\n`, `\r\n`
/// e `\r` sozinho terminam linha.
pub struct LineInfo {
    starts: Vec<usize>,
}

impl LineInfo {
    pub fn from(source: &str) -> LineInfo {
        let b = source.as_bytes();
        let mut starts = vec![0];
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\r' {
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                starts.push(i + 1);
            } else if b[i] == b'\n' {
                starts.push(i + 1);
            }
            i += 1;
        }
        LineInfo { starts }
    }

    /// Linha (a partir de 1) da posição.
    pub fn line(&self, pos: usize) -> i32 {
        self.starts.partition_point(|&s| s <= pos) as i32
    }

    /// Coluna (a partir de 1) da posição.
    pub fn column(&self, pos: usize) -> usize {
        let l = self.line(pos) as usize;
        pos - self.starts[l - 1] + 1
    }

    /// Início das linhas (para a primeira quebra).
    pub fn starts(&self) -> &[usize] {
        &self.starts
    }
}

/// `SourceVisitor`.
pub struct SourceVisitor<'f> {
    pub source: &'f str,
    pub tk: &'f Tokens,
    pub tree_: &'f Tree,
    lines_: LineInfo,
    pub a: Arenas,
    stack: Vec<ChunkBuilder>,
    ultimo_token: Tk,
    collections_: Vec<bool>,
    block_rules: HashMap<Tk, RuleId>,
    block_chunks: HashMap<Tk, ChunkId>,
    suppressed: HashSet<Tk>,
}

impl<'f> SourceVisitor<'f> {
    /// `SourceVisitor(formatter, lineInfo, source)`.
    pub fn new(source: &'f str, tk: &'f Tokens, tree_: &'f Tree, largura: usize, recuo: i32, line_ending: String) -> SourceVisitor<'f> {
        let mut a = Arenas::new_arenas(largura, line_ending);
        let topo = ChunkBuilder::topo(&mut a, recuo);
        SourceVisitor {
            source,
            tk,
            tree_,
            lines_: LineInfo::from(source),
            a,
            stack: vec![topo],
            ultimo_token: 0,
            collections_: Vec::new(),
            block_rules: HashMap::new(),
            block_chunks: HashMap::new(),
            suppressed: HashSet::new(),
        }
    }

    /// `run(node)`: visita a unidade e devolve os chunks de topo.
    pub fn run(mut self, root: NodeId) -> (Arenas, Vec<ChunkId>) {
        self.visit(Some(root));
        // `node.endToken.next`: o EOF (numa unidade vazia, o próprio fim).
        let eof = self.tk.list.len() as Tk - 1;
        let after = (self.tree_.end(root) + 1).min(eof);
        self.write_preceding_comments_and_newlines(after);
        let topo = self.stack.pop().expect("construtor de topo");
        let chunks = topo.finish(&mut self.a);
        (self.a, chunks)
    }

    // ================================================================ tokens

    fn tipo(&self, t: Tk) -> TokenKind {
        self.tk.list[t as usize].tipo
    }

    fn lexeme(&self, t: Tk) -> &'f str {
        self.tk.texto(self.source, t)
    }

    fn is_op(&self, t: Tk, op: Op) -> bool {
        self.tipo(t) == TokenKind::Op(op)
    }

    fn has_comments(&self, t: Tk) -> bool {
        !self.tk.comments_of(t).is_empty()
    }

    fn start_line(&self, t: Tk) -> i32 {
        self.lines_.line(self.tk.list[t as usize].start)
    }

    fn end_line(&self, t: Tk) -> i32 {
        let k = &self.tk.list[t as usize];
        let mut end = k.end;
        if k.tipo == TokenKind::ScriptTag {
            // O lexema do analyzer inclui a quebra de linha.
            let b = self.source.as_bytes();
            if b.get(end) == Some(&b'\r') {
                end += 1;
            }
            if b.get(end) == Some(&b'\n') {
                end += 1;
            }
        }
        self.lines_.line(end)
    }

    /// `_linesBeforeNextToken`.
    fn lines_before_next(&self) -> i32 {
        let anterior = self.ultimo_token;
        let proximo = anterior + 1;
        let start = match self.tk.comments_of(proximo).first() {
            Some(c) => self.lines_.line(c.start),
            None => self.start_line(proximo),
        };
        start - self.end_line(anterior)
    }

    // ============================================================= construtor

    fn b(&mut self) -> (&mut ChunkBuilder, &mut Arenas) {
        (self.stack.last_mut().expect("construtor"), &mut self.a)
    }

    pub fn space(&mut self) {
        self.b().0.space();
    }

    pub fn newline(&mut self) {
        self.b().0.newline(false, false, false);
    }

    pub fn two_newlines(&mut self) {
        self.b().0.newline(true, false, false);
    }

    pub fn write_newline(&mut self, double: bool, flush_left: bool, nest: bool) {
        self.b().0.newline(double, flush_left, nest);
    }

    pub fn split_or_newline(&mut self) {
        if self.lines_before_next() > 0 {
            self.write_newline(false, false, true);
        } else {
            self.split();
        }
    }

    pub fn split_or_two_newlines(&mut self) {
        if self.lines_before_next() > 1 {
            self.two_newlines();
        } else {
            self.split();
        }
    }

    pub fn one_or_two_newlines(&mut self) {
        let double = self.lines_before_next() > 1;
        self.write_newline(double, false, false);
    }

    /// `split()`: quebra com espaço na regra atual.
    pub fn split(&mut self) -> ChunkId {
        let (b, a) = self.b();
        b.split(a, true, true)
    }

    /// `zeroSplit()`.
    pub fn zero_split(&mut self) -> ChunkId {
        let (b, a) = self.b();
        b.split(a, true, false)
    }

    /// `builder.split(nest:, space:)`.
    pub fn builder_split(&mut self, nest: bool, space: bool) -> ChunkId {
        let (b, a) = self.b();
        b.split(a, nest, space)
    }

    /// `soloSplit([cost])`.
    pub fn solo_split(&mut self, cost: i32) -> RuleId {
        let r = self.a.rule(cost);
        self.start_rule(Some(r));
        self.split();
        self.end_rule();
        r
    }

    /// `soloZeroSplit()`.
    pub fn solo_zero_split(&mut self) {
        self.start_rule(None);
        self.builder_split(true, false);
        self.end_rule();
    }

    pub fn start_span(&mut self, cost: i32) {
        self.b().0.start_span_(cost);
    }

    pub fn end_span(&mut self) {
        let (b, a) = self.b();
        b.end_span_(a);
    }

    pub fn start_rule(&mut self, r: Option<RuleId>) {
        let (b, a) = self.b();
        b.start_rule_(a, r);
    }

    pub fn start_lazy_rule(&mut self, r: Option<RuleId>) {
        let (b, a) = self.b();
        b.start_lazy_rule_(a, r);
    }

    pub fn end_rule(&mut self) {
        self.b().0.end_rule_();
    }

    pub fn current_rule(&self) -> RuleId {
        self.stack.last().expect("construtor").rule()
    }

    pub fn force_rules(&mut self) {
        let (b, a) = self.b();
        b.force_rules_(a);
    }

    pub fn nest_expression(&mut self, recuo: Option<i32>, agora: bool) {
        let (b, a) = self.b();
        b.nest_expression_at(a, recuo, agora);
    }

    /// `unnest()` (que por padrão confirma na hora).
    pub fn unnest(&mut self) {
        let (b, a) = self.b();
        b.unnest_(a, true);
    }

    pub fn unnest_later(&mut self) {
        let (b, a) = self.b();
        b.unnest_(a, false);
    }

    pub fn start_block_argument_nesting(&mut self) {
        self.b().0.start_argument_nesting();
    }

    pub fn end_block_argument_nesting(&mut self) {
        self.b().0.end_argument_nesting();
    }

    fn indent(&mut self, spaces: Option<i32>) {
        self.b().0.indent(spaces);
    }

    fn unindent(&mut self) {
        self.b().0.unindent_();
    }

    /// `builder = builder.startBlock(...)`.
    fn start_block(&mut self, argument: Option<ChunkId>, indent: bool, space: bool) {
        let (b, a) = self.b();
        let child = b.start_block(a, argument, indent, space);
        self.stack.push(child);
    }

    /// `builder = builder.endBlock(forceSplit:)`.
    fn end_block(&mut self, force: bool) {
        let child = self.stack.pop().expect("bloco");
        if child.end_block(&mut self.a, force) {
            self.force_rules();
        }
    }

    // ================================================================ escrita

    /// `token(token)`.
    pub fn token(&mut self, t: Tk) {
        self.write_preceding_comments_and_newlines(t);
        let texto = self.lexeme(t);
        self.write_text(texto, t);
    }

    /// `token(token)` com token opcional.
    pub fn token_opt(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.token(t);
        }
    }

    /// `token(token, after: space)` / `modifier(token)`.
    pub fn modifier(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.token(t);
            self.space();
        }
    }

    /// `token(token, before: space)`: o espaço vem depois dos comentários.
    fn token_space_before(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.write_preceding_comments_and_newlines(t);
            self.space();
            let texto = self.lexeme(t);
            self.write_text(texto, t);
        }
    }

    /// `_writeText(text, token)`.
    fn write_text(&mut self, texto: &str, t: Tk) {
        self.write_text_merge(texto, t, true);
    }

    fn write_text_merge(&mut self, texto: &str, t: Tk, join: bool) {
        let (b, a) = self.b();
        b.write(a, texto, join);
        self.ultimo_token = t;
    }

    /// `writePrecedingCommentsAndNewlines(token)`.
    pub fn write_preceding_comments_and_newlines(&mut self, t: Tk) -> bool {
        let comments = self.tk.comments_of(t);
        if comments.is_empty() {
            return false;
        }
        if self.suppressed.contains(&t) {
            return false;
        }
        let anterior = if t == 0 { None } else { Some(t - 1) };
        let mut previous_line = anterior.map(|p| self.end_line(p)).unwrap_or(1);
        let token_line = self.start_line(t);
        if anterior.is_some_and(|p| self.tipo(p) == TokenKind::ScriptTag) {
            previous_line = token_line;
        }
        let mut list = Vec::with_capacity(comments.len());
        for (i, c) in comments.iter().enumerate() {
            let line_c = self.lines_.line(c.start);
            if i == 0 && anterior.is_none() {
                previous_line = line_c;
            }
            let texto = self.source[c.start..c.end].trim();
            let mut lines_before = line_c - previous_line;
            let mut flush_left = self.lines_.column(c.start) == 1;
            let doc_line = texto.starts_with("///") && !texto.starts_with("////");
            if doc_line {
                flush_left = false;
                if i == 0 {
                    lines_before = 2;
                }
            }
            let tipo = if doc_line || (texto.starts_with("/**") && texto != "/**/") {
                CommentKind::Doc
            } else if c.is_line {
                CommentKind::Line
            } else if line_c == previous_line || line_c == token_line {
                CommentKind::InlineBlock
            } else {
                CommentKind::Block
            };
            list.push(SourceComment {
                texto: texto.to_string(),
                tipo,
                lines_before,
                flush_left,
            });
            previous_line = self.lines_.line(c.end);
        }
        let token = self.lexeme(t);
        let (b, a) = self.b();
        b.write_comments(a, &mut list, token_line - previous_line, token);
        list[0].lines_before > 0
    }

    // ============================================================== consultas

    pub fn node(&self, id: NodeId) -> &'f Node {
        self.tree_.node(id)
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.tree_.parent(id)
    }

    fn parent_and(&self, id: NodeId, f: fn(&Node) -> bool) -> bool {
        self.parent(id).is_some_and(|p| f(self.node(p)))
    }

    /// `commaAfter`.
    pub fn comma_after(&self, id: NodeId) -> Option<Tk> {
        let proximo = self.tree_.end(id) + 1;
        if self.is_op(proximo, Op::Comma) {
            return Some(proximo);
        }
        if self.is_op(proximo, Op::Question) && self.is_op(proximo + 1, Op::Comma) {
            return Some(proximo + 1);
        }
        None
    }

    pub fn has_comma_after(&self, id: NodeId) -> bool {
        self.comma_after(id).is_some()
    }

    pub fn list_has_comma_after(&self, nodes: &[NodeId]) -> bool {
        nodes.last().is_some_and(|&n| self.has_comma_after(n))
    }

    /// `canSplit(rightBracket)`.
    fn can_split(&self, nodes: &[NodeId], right_: Tk) -> bool {
        !nodes.is_empty() || self.has_comments(right_)
    }

    /// `hasLineCommentBefore` do token.
    fn has_line_comment_before(&self, t: Tk) -> bool {
        self.tk.comments_of(t).iter().any(|c| c.is_line)
    }

    /// `beginToken.hasLineCommentBefore` de um nó. Nós anotados com
    /// comentário de documentação começam no próprio comentário no analyzer,
    /// e um token de comentário não tem comentários antes.
    fn node_has_line_comment_before(&self, id: NodeId) -> bool {
        let start = self.tree_.start(id);
        if matches!(self.node(id), Node::EnumConstantDeclaration { .. }) && self.has_doc(start) {
            return false;
        }
        self.has_line_comment_before(start)
    }

    /// Se os comentários antes de `t` têm um comentário de documentação
    /// (`findDartDoc`: o último `/**`, ou uma sequência de `///`).
    fn has_doc(&self, t: Tk) -> bool {
        self.tk.comments_of(t).iter().any(|c| {
            let s = &self.source[c.start..c.end];
            s.starts_with("///") || s.starts_with("/**")
        })
    }

    /// `containsLineComments([rightBracket])`.
    fn contains_line_comments(&self, nodes: &[NodeId], right_: Option<Tk>) -> bool {
        nodes.iter().any(|&n| self.node_has_line_comment_before(n)) || right_.is_some_and(|t| self.has_line_comment_before(t))
    }

    /// `hasNonEmptyBody`.
    fn has_non_empty_body(&self, id: NodeId) -> bool {
        let corpo = match self.node(id) {
            Node::MethodDeclaration { body, .. } => Some(*body),
            Node::FunctionDeclarationStatement { function_declaration } => self.declaration_body(*function_declaration),
            Node::FunctionDeclaration { .. } => self.declaration_body(id),
            _ => None,
        };
        corpo.is_some_and(|c| self.non_empty_block(c))
    }

    fn declaration_body(&self, decl: NodeId) -> Option<NodeId> {
        if let Node::FunctionDeclaration { function_expression, .. } = self.node(decl)
            && let Node::FunctionExpression { body, .. } = self.node(*function_expression)
        {
            return Some(*body);
        }
        None
    }

    /// `body is BlockFunctionBody && body.block.statements.isNotEmpty`.
    fn non_empty_block(&self, corpo: NodeId) -> bool {
        if let Node::BlockFunctionBody { block, .. } = self.node(corpo)
            && let Node::Block { statements, .. } = self.node(*block)
        {
            return !statements.is_empty();
        }
        false
    }

    pub fn is_collection_literal(&self, id: NodeId) -> bool {
        matches!(self.node(id), Node::ListLiteral { .. } | Node::RecordLiteral { .. } | Node::SetOrMapLiteral { .. })
    }

    fn is_control_flow_element(&self, id: NodeId) -> bool {
        matches!(self.node(id), Node::IfElement { .. } | Node::ForElement { .. })
    }

    /// `isFunctionExpressionBody`.
    fn is_function_expression_body(&self, id: NodeId) -> bool {
        match self.parent(id) {
            Some(p) if matches!(self.node(p), Node::FunctionExpression { .. }) => !self.parent_and(p, |n| matches!(n, Node::FunctionDeclaration { .. })),
            _ => false,
        }
    }

    /// `spreadCollectionBracket`.
    fn spread_collection_bracket(&self, id: NodeId) -> Option<Tk> {
        if let Node::SpreadElement { expression, .. } = self.node(id) {
            match self.node(*expression) {
                Node::ListLiteral {
                    elements,
                    left_bracket,
                    right_bracket,
                    ..
                }
                | Node::SetOrMapLiteral {
                    elements,
                    left_bracket,
                    right_bracket,
                    ..
                } => {
                    if self.can_split(elements, *right_bracket) {
                        return Some(*left_bracket);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// `isTrailingCommaArgument`.
    fn is_trailing_comma_argument(&self, id: NodeId) -> bool {
        let mut parent = self.parent(id);
        if let Some(p) = parent
            && matches!(self.node(p), Node::NamedExpression { .. })
        {
            parent = self.parent(p);
        }
        match parent.map(|p| self.node(p)) {
            Some(Node::ArgumentList { arguments, .. }) => self.list_has_comma_after(arguments),
            _ => false,
        }
    }

    /// `looksLikeStaticCall`.
    pub fn looks_like_static_call(&self, id: NodeId) -> bool {
        let Node::MethodInvocation { target, method_name, .. } = self.node(id) else {
            return false;
        };
        let Some(mut alvo) = *target else {
            return false;
        };
        if matches!(self.node(alvo), Node::SimpleIdentifier { .. }) && looks_like_class_name(self.simple_name(*method_name)) {
            return true;
        }
        if let Node::PrefixedIdentifier { identifier, .. } = self.node(alvo) {
            alvo = *identifier;
        }
        matches!(self.node(alvo), Node::SimpleIdentifier { .. }) && looks_like_class_name(self.simple_name(alvo))
    }

    pub fn simple_name(&self, id: NodeId) -> &'f str {
        match self.node(id) {
            Node::SimpleIdentifier { token } => self.lexeme(*token),
            _ => "",
        }
    }

    /// `allowInline` da cascata.
    fn allow_inline(&self, target: NodeId, sections: usize) -> bool {
        if sections > 1 {
            return false;
        }
        !matches!(
            self.node(target),
            Node::ConditionalExpression { .. } | Node::BinaryExpression { .. } | Node::PrefixExpression { .. } | Node::AwaitExpression { .. }
        )
    }

    /// `isMultiline` de `SingleStringLiteral`.
    pub fn is_multiline_string(&self, id: NodeId) -> bool {
        let first = match self.node(id) {
            Node::SimpleStringLiteral { literal } => *literal,
            Node::StringInterpolation { elements } => match elements.first().map(|e| self.node(*e)) {
                Some(Node::InterpolationString { contents }) => *contents,
                _ => return false,
            },
            _ => return false,
        };
        let s = self.lexeme(first);
        let s = s.strip_prefix('r').unwrap_or(s);
        s.starts_with("'''") || s.starts_with("\"\"\"")
    }

    fn is_string_literal(&self, id: NodeId) -> bool {
        matches!(
            self.node(id),
            Node::SimpleStringLiteral { .. } | Node::StringInterpolation { .. } | Node::AdjacentStrings { .. }
        )
    }

    fn named_parts(&self, id: NodeId) -> (Tk, Tk, NodeId) {
        let Node::NamedExpression { name, expression } = self.node(id) else {
            panic!("NamedExpression esperado");
        };
        let Node::Label { label, colon } = self.node(*name) else {
            panic!("Label esperado");
        };
        let Node::SimpleIdentifier { token } = self.node(*label) else {
            panic!("SimpleIdentifier esperado");
        };
        (*token, *colon, *expression)
    }

    fn param_kind(&self, id: NodeId) -> ParameterKind {
        match self.node(id) {
            Node::SimpleFormalParameter { kind, .. }
            | Node::FieldFormalParameter { kind, .. }
            | Node::SuperFormalParameter { kind, .. }
            | Node::FunctionTypedFormalParameter { kind, .. }
            | Node::DefaultFormalParameter { kind, .. } => *kind,
            _ => ParameterKind::Required,
        }
    }

    // ================================================================ visita

    /// `visit(node)`.
    pub fn visit(&mut self, node: Option<NodeId>) {
        if let Some(id) = node {
            self.accept(id);
        }
    }

    fn visit_before(&mut self, node: Option<NodeId>, antes: Action) {
        if let Some(id) = node {
            self.action(antes);
            self.accept(id);
        }
    }

    fn visit_after(&mut self, node: Option<NodeId>, after: Action) {
        if let Some(id) = node {
            self.accept(id);
            self.action(after);
        }
    }

    fn action(&mut self, a: Action) {
        match a {
            Action::Nothing => {}
            Action::Space => self.space(),
            Action::Newline => self.newline(),
            Action::Split => {
                self.split();
            }
            Action::ZeroSplit => {
                self.zero_split();
            }
            Action::SplitOrNewline => self.split_or_newline(),
            Action::SplitOrTwoNewlines => self.split_or_two_newlines(),
            Action::OneOrTwoNewlines => self.one_or_two_newlines(),
        }
    }

    /// `visitNodes(nodes, before:, between:, after:)`.
    fn visit_nodes(&mut self, nodes: &[NodeId], antes: Action, between: Action, after: Action) {
        if nodes.is_empty() {
            return;
        }
        self.action(antes);
        self.accept(nodes[0]);
        for &n in &nodes[1..] {
            self.action(between);
            self.accept(n);
        }
        self.action(after);
    }

    /// `visitCommaSeparatedNodes(nodes, between:)`.
    fn visit_comma_separated_nodes(&mut self, nodes: &[NodeId], between: Action) {
        for (i, &n) in nodes.iter().enumerate() {
            if i > 0 {
                self.action(between);
            }
            self.accept(n);
            let proximo = self.tree_.end(n) + 1;
            if self.lexeme(proximo) == "," {
                self.token(proximo);
            }
        }
    }

    /// `visitMetadata(metadata)`.
    fn visit_metadata(&mut self, metadata: &[NodeId]) {
        self.visit_nodes(metadata, Action::Nothing, Action::Newline, Action::Newline);
    }

    /// `_writeCommaAfter(node)`.
    fn write_comma_after(&mut self, id: NodeId) {
        let c = self.comma_after(id);
        self.token_opt(c);
    }

    /// `node.accept(this)`.
    fn accept(&mut self, id: NodeId) {
        let n = self.node(id);
        match n {
            Node::CompilationUnit { .. } => self.visit_compilation_unit(id),
            Node::LibraryDirective { metadata, library_keyword, name2, semicolon } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*library_keyword);
                self.visit_before(*name2, Action::Space);
                self.token(*semicolon);
                self.unnest();
            }
            Node::ImportDirective { .. } => self.visit_import_directive(id),
            Node::ExportDirective {
                metadata,
                export_keyword,
                uri,
                configurations,
                combinators,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*export_keyword);
                self.space();
                self.visit(Some(*uri));
                self.visit_configurations(configurations);
                self.visit_combinators(combinators);
                self.token(*semicolon);
                self.unnest();
            }
            Node::PartDirective {
                metadata,
                part_keyword,
                uri,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*part_keyword);
                self.space();
                self.visit(Some(*uri));
                self.token(*semicolon);
                self.unnest();
            }
            Node::PartOfDirective {
                metadata,
                part_keyword,
                of_keyword,
                library_name,
                uri,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*part_keyword);
                self.space();
                self.token(*of_keyword);
                self.space();
                self.visit(*library_name);
                self.visit(*uri);
                self.token(*semicolon);
                self.unnest();
            }
            Node::Configuration {
                if_keyword,
                left_parenthesis,
                name,
                equal_token,
                value,
                right_parenthesis,
                uri,
            } => {
                self.token(*if_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.visit(Some(*name));
                if let Some(eq) = equal_token {
                    self.nest_expression(None, false);
                    self.space();
                    self.token(*eq);
                    self.solo_split(cost::NORMAL);
                    self.visit(*value);
                    self.unnest();
                }
                self.token(*right_parenthesis);
                self.space();
                self.visit(Some(*uri));
            }
            Node::DottedName { components } => {
                for (i, &c) in components.iter().enumerate() {
                    if i > 0 {
                        let dot = self.tree_.start(c) - 1;
                        self.token(dot);
                    }
                    self.visit(Some(c));
                }
            }
            Node::LibraryIdentifier { components } => {
                self.visit(Some(components[0]));
                for &c in &components[1..] {
                    let dot = self.tree_.start(c) - 1;
                    self.token(dot);
                    self.visit(Some(c));
                }
            }
            Node::ShowCombinator { keyword, shown_names } => self.visit_combinator(*keyword, shown_names),
            Node::HideCombinator { keyword, hidden_names } => self.visit_combinator(*keyword, hidden_names),
            Node::ClassDeclaration { .. } => self.visit_class_declaration(id),
            Node::ExtendsClause { extends_keyword, superclass } => {
                self.solo_split(cost::NORMAL);
                self.token(*extends_keyword);
                self.space();
                self.visit(Some(*superclass));
            }
            Node::WithClause { with_keyword, mixin_types } => self.visit_combinator(*with_keyword, mixin_types),
            Node::ImplementsClause {
                implements_keyword,
                interfaces,
            } => self.visit_combinator(*implements_keyword, interfaces),
            Node::MixinOnClause {
                on_keyword,
                superclass_constraints,
            } => self.visit_combinator(*on_keyword, superclass_constraints),
            Node::NativeClause { native_keyword, name } => {
                self.token(*native_keyword);
                self.visit_before(*name, Action::Space);
            }
            Node::MixinDeclaration { .. } => self.visit_mixin_declaration(id),
            Node::ExtensionDeclaration { .. } => self.visit_extension_declaration(id),
            Node::ExtensionOnClause { .. } => panic!("ExtensionOnClause é visitada pela extensão"),
            Node::ExtensionTypeDeclaration { .. } => self.visit_extension_type_declaration(id),
            Node::RepresentationDeclaration { .. } => self.visit_representation_declaration(id),
            Node::RepresentationConstructorName { period, name } => {
                self.token(*period);
                self.token(*name);
            }
            Node::EnumDeclaration { .. } => self.visit_enum_declaration(id),
            Node::EnumConstantDeclaration { metadata, name, arguments } => {
                self.visit_metadata(metadata);
                self.token(*name);
                if let Some(args) = arguments
                    && let Node::EnumConstantArguments {
                        type_arguments,
                        constructor_selector,
                        argument_list,
                    } = self.node(*args)
                {
                    self.nest_expression(None, false);
                    self.visit(*type_arguments);
                    if let Some(sel) = constructor_selector
                        && let Node::ConstructorSelector { period, name } = self.node(*sel)
                    {
                        self.token(*period);
                        self.visit(Some(*name));
                    }
                    self.visit_argument_list(*argument_list, false);
                    self.unnest();
                }
            }
            Node::EnumConstantArguments { .. } | Node::ConstructorSelector { .. } => {
                panic!("visitado pela constante do enum")
            }
            Node::ClassTypeAlias { .. } => self.visit_class_type_alias(id),
            Node::FunctionTypeAlias {
                metadata,
                typedef_keyword,
                return_type,
                name,
                type_parameters,
                parameters,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.token(*typedef_keyword);
                self.space();
                self.visit_after(*return_type, Action::Space);
                self.token(*name);
                self.visit(*type_parameters);
                self.visit(Some(*parameters));
                self.token(*semicolon);
                self.unnest();
            }
            Node::GenericTypeAlias {
                metadata,
                typedef_keyword,
                name,
                type_parameters,
                equals,
                type_,
                semicolon,
                ..
            } => {
                self.visit_nodes(metadata, Action::Nothing, Action::Newline, Action::Newline);
                self.nest_expression(None, false);
                self.visit_generic_type_alias_header(*typedef_keyword, *name, *type_parameters, *equals);
                self.space();
                self.visit(Some(*type_));
                self.token(*semicolon);
                self.unnest();
            }
            Node::FunctionDeclaration {
                metadata,
                external_keyword,
                return_type,
                property_keyword,
                name,
                function_expression,
                ..
            } => {
                let Node::FunctionExpression {
                    type_parameters,
                    parameters,
                    body,
                } = self.node(*function_expression)
                else {
                    panic!("FunctionExpression esperada");
                };
                self.visit_function_or_method_declaration(
                    metadata,
                    *external_keyword,
                    *property_keyword,
                    None,
                    None,
                    *name,
                    *return_type,
                    *type_parameters,
                    *parameters,
                    *body,
                );
            }
            Node::TopLevelVariableDeclaration {
                metadata,
                external_keyword,
                variables,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.modifier(*external_keyword);
                self.visit(Some(*variables));
                self.token(*semicolon);
                self.unnest();
            }
            Node::ConstructorDeclaration { .. } => self.visit_constructor_declaration(id),
            Node::MethodDeclaration {
                metadata,
                external_keyword,
                modifier_keyword,
                return_type,
                property_keyword,
                operator_keyword,
                name,
                type_parameters,
                parameters,
                body,
                ..
            } => self.visit_function_or_method_declaration(
                metadata,
                *external_keyword,
                *property_keyword,
                *modifier_keyword,
                *operator_keyword,
                *name,
                *return_type,
                *type_parameters,
                *parameters,
                *body,
            ),
            Node::FieldDeclaration {
                metadata,
                abstract_keyword,
                covariant_keyword,
                external_keyword,
                static_keyword,
                fields,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.modifier(*external_keyword);
                self.modifier(*static_keyword);
                self.modifier(*abstract_keyword);
                self.modifier(*covariant_keyword);
                self.visit(Some(*fields));
                self.token(*semicolon);
                self.unnest();
            }
            Node::ConstructorFieldInitializer {
                this_keyword,
                period,
                field_name,
                equals,
                expression,
            } => {
                self.nest_expression(None, false);
                self.token_opt(*this_keyword);
                self.token_opt(*period);
                self.visit(Some(*field_name));
                self.visit_assignment(*equals, *expression, false);
                self.unnest();
            }
            Node::SuperConstructorInvocation {
                super_keyword: kw,
                period,
                constructor_name,
                argument_list,
            }
            | Node::RedirectingConstructorInvocation {
                this_keyword: kw,
                period,
                constructor_name,
                argument_list,
            } => {
                self.start_span(cost::NORMAL);
                self.token(*kw);
                self.token_opt(*period);
                self.visit(*constructor_name);
                self.visit(Some(*argument_list));
                self.end_span();
            }
            Node::AssertInitializer {
                assert_keyword,
                left_parenthesis,
                condition,
                message,
                right_parenthesis,
                ..
            } => {
                self.token(*assert_keyword);
                let mut args = vec![*condition];
                if let Some(m) = message {
                    args.push(*m);
                }
                if self.list_has_comma_after(&args) {
                    self.visit_collection_literal(*left_parenthesis, &args, *right_parenthesis, None, None, None, false, false);
                    return;
                }
                self.nest_expression(None, false);
                super::arguments::ArgumentListVisitor::for_arguments(self, *left_parenthesis, *right_parenthesis, &args).visit(self);
                self.unnest();
            }
            Node::BlockFunctionBody { keyword, star, block } => {
                self.space();
                self.token_opt(*keyword);
                self.token_opt(*star);
                if keyword.is_some() {
                    self.space();
                }
                self.visit(Some(*block));
            }
            Node::ExpressionFunctionBody { .. } => self.visit_expression_function_body(id),
            Node::EmptyFunctionBody { semicolon } | Node::EmptyStatement { semicolon } => self.token(*semicolon),
            Node::NativeFunctionBody {
                native_keyword,
                string_literal,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.nest_expression(None, true);
                self.solo_split(cost::NORMAL);
                self.token(*native_keyword);
                self.visit_before(*string_literal, Action::Space);
                self.unnest();
                self.token(*semicolon);
                self.unnest();
            }
            Node::FormalParameterList { .. } => self.visit_formal_parameter_list(id, true),
            Node::SimpleFormalParameter {
                metadata,
                keyword,
                type_,
                name,
                ..
            } => {
                let (metadata, keyword, type_, name) = (metadata.clone(), *keyword, *type_, *name);
                self.visit_parameter_metadata(&metadata, |v| {
                    v.begin_formal_parameter(id);
                    v.modifier(keyword);
                    v.visit(type_);
                    if name.is_some() {
                        v.separator_between_type_and_variable(type_, false);
                    }
                    v.token_opt(name);
                    v.end_formal_parameter();
                });
            }
            Node::FieldFormalParameter {
                metadata,
                keyword,
                type_,
                this_keyword: kw,
                period,
                name,
                type_parameters,
                parameters,
                question,
                ..
            }
            | Node::SuperFormalParameter {
                metadata,
                keyword,
                type_,
                super_keyword: kw,
                period,
                name,
                type_parameters,
                parameters,
                question,
                ..
            } => {
                let is_super = matches!(n, Node::SuperFormalParameter { .. });
                let (keyword, type_, kw, period, name, tps, ps, q) =
                    (*keyword, *type_, *kw, *period, *name, *type_parameters, *parameters, *question);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.begin_formal_parameter(id);
                    if let Some(k) = keyword {
                        v.token(k);
                        v.space();
                    }
                    if is_super {
                        v.visit_after(type_, Action::Split);
                    } else {
                        v.visit(type_);
                        v.separator_between_type_and_variable(type_, false);
                    }
                    v.token(kw);
                    v.token(period);
                    v.token(name);
                    v.visit(tps);
                    v.visit(ps);
                    v.token_opt(q);
                    v.end_formal_parameter();
                });
            }
            Node::FunctionTypedFormalParameter {
                metadata,
                covariant_keyword,
                required_keyword,
                return_type,
                name,
                type_parameters,
                parameters,
                question,
                ..
            } => {
                let (covariant_, req, rt, name, tps, ps, q) = (
                    *covariant_keyword,
                    *required_keyword,
                    *return_type,
                    *name,
                    *type_parameters,
                    *parameters,
                    *question,
                );
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.modifier(req);
                    v.modifier(covariant_);
                    v.visit_after(rt, Action::Space);
                    v.start_span(cost::NORMAL);
                    v.token(name);
                    v.visit_parameter_signature(tps, Some(ps));
                    v.token_opt(q);
                    v.end_span();
                });
            }
            Node::DefaultFormalParameter {
                parameter,
                separator,
                default_value,
                ..
            } => {
                self.visit(Some(*parameter));
                if let Some(sep) = separator {
                    self.start_span(cost::NORMAL);
                    self.nest_expression(None, false);
                    if self.is_op(*sep, Op::Assign) {
                        self.space();
                    }
                    self.token(*sep);
                    let dv = default_value.expect("valor padrão");
                    let c = self.assignment_cost(dv);
                    self.solo_split(c);
                    self.visit(Some(dv));
                    self.unnest();
                    self.end_span();
                }
            }
            Node::NamedType {
                import_prefix,
                name2,
                type_arguments,
                question,
            } => {
                if let Some((prefix, dot)) = import_prefix {
                    self.start_span(cost::NORMAL);
                    self.token(*prefix);
                    self.solo_zero_split();
                    self.token(*dot);
                    self.token(*name2);
                    self.end_span();
                } else {
                    self.token(*name2);
                }
                self.visit(*type_arguments);
                self.token_opt(*question);
            }
            Node::GenericFunctionType {
                return_type,
                function_keyword,
                type_parameters,
                parameters,
                question,
            } => {
                self.visit_generic_function_type(*return_type, *function_keyword, *type_parameters, *parameters);
                self.token_opt(*question);
            }
            Node::RecordTypeAnnotation { .. } => self.visit_record_type_annotation(id),
            Node::RecordTypeAnnotationNamedFields { .. } => panic!("visitado pelo tipo de registro"),
            Node::RecordTypeAnnotationPositionalField { metadata, type_, name } => {
                let (t, nm) = (*type_, *name);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.visit(Some(t));
                    v.token_space_before(nm);
                });
            }
            Node::RecordTypeAnnotationNamedField { metadata, type_, name } => {
                let (t, nm) = (*type_, *name);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.visit(Some(t));
                    v.token_space_before(Some(nm));
                });
            }
            Node::TypeArgumentList {
                left_bracket,
                arguments,
                right_bracket,
            } => self.visit_generic_list(*left_bracket, *right_bracket, arguments),
            Node::TypeParameterList {
                left_bracket,
                type_parameters,
                right_bracket,
            } => self.visit_generic_list(*left_bracket, *right_bracket, type_parameters),
            Node::TypeParameter {
                metadata,
                name,
                extends_keyword,
                bound,
            } => {
                let (nm, ext, bd) = (*name, *extends_keyword, *bound);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.token(nm);
                    if let Some(e) = ext {
                        v.write_preceding_comments_and_newlines(e);
                        v.space();
                        let t = v.lexeme(e);
                        v.write_text(t, e);
                        v.space();
                    }
                    v.visit(bd);
                });
            }
            Node::Annotation {
                at_sign,
                name,
                type_arguments,
                period,
                constructor_name,
                arguments,
            } => {
                self.token(*at_sign);
                self.visit(Some(*name));
                self.nest_expression(None, false);
                self.visit(*type_arguments);
                self.token_opt(*period);
                self.visit(*constructor_name);
                if let Some(args) = arguments {
                    self.visit_argument_list(*args, false);
                }
                self.unnest();
            }
            Node::Block {
                left_bracket,
                statements,
                right_bracket,
            } => {
                if !self.can_split(statements, *right_bracket) {
                    self.token(*left_bracket);
                    if self.split_empty_block(id) {
                        self.newline();
                    }
                    self.token(*right_bracket);
                    return;
                }
                self.visit_body(*left_bracket, statements, *right_bracket);
            }
            Node::ExpressionStatement { expression, semicolon } => {
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.token_opt(*semicolon);
                self.unnest();
            }
            Node::VariableDeclarationStatement { variables, semicolon } => {
                self.nest_expression(None, false);
                self.visit(Some(*variables));
                self.token(*semicolon);
                self.unnest();
            }
            Node::VariableDeclarationList { .. } => self.visit_variable_declaration_list(id),
            Node::VariableDeclaration {
                name,
                equals,
                initializer,
            } => {
                self.token(*name);
                let Some(init) = initializer else {
                    return;
                };
                let multiple = match self.parent(id).map(|p| self.node(p)) {
                    Some(Node::VariableDeclarationList { variables, .. }) => variables.len() > 1,
                    _ => false,
                };
                self.visit_assignment(equals.expect("="), *init, multiple);
            }
            Node::FunctionDeclarationStatement { function_declaration } => self.visit(Some(*function_declaration)),
            Node::IfStatement { .. } => self.visit_if_statement(id),
            Node::CaseClause { .. } | Node::GuardedPattern { .. } | Node::WhenClause { .. } => {
                panic!("visitado pelo if/case")
            }
            Node::ForStatement {
                await_keyword,
                for_keyword,
                left_parenthesis,
                for_loop_parts,
                right_parenthesis,
                body,
            } => {
                self.nest_expression(None, false);
                self.modifier(*await_keyword);
                self.token(*for_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.start_rule(None);
                self.visit(Some(*for_loop_parts));
                self.token(*right_parenthesis);
                self.end_rule();
                self.unnest();
                self.visit_loop_body(*body);
            }
            Node::ForPartsWithDeclarations { .. } | Node::ForPartsWithExpression { .. } | Node::ForPartsWithPattern { .. } => {
                self.visit_for_parts(id)
            }
            Node::ForEachPartsWithDeclaration {
                loop_variable,
                in_keyword,
                iterable,
            } => {
                let metadata = match self.node(*loop_variable) {
                    Node::DeclaredIdentifier { metadata, .. } => metadata.clone(),
                    _ => Vec::new(),
                };
                self.visit_nodes(&metadata, Action::Nothing, Action::Split, Action::Split);
                self.visit(Some(*loop_variable));
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            Node::ForEachPartsWithIdentifier {
                identifier,
                in_keyword,
                iterable,
            } => {
                self.visit(Some(*identifier));
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            Node::ForEachPartsWithPattern {
                metadata,
                keyword,
                pattern,
                in_keyword,
                iterable,
            } => {
                self.start_block_argument_nesting();
                self.visit_nodes(metadata, Action::Nothing, Action::Split, Action::Split);
                self.token(*keyword);
                self.space();
                self.visit(Some(*pattern));
                self.end_block_argument_nesting();
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            Node::DeclaredIdentifier { keyword, type_, name, .. } => {
                self.modifier(*keyword);
                self.visit_after(*type_, Action::Space);
                self.token(*name);
            }
            Node::WhileStatement {
                while_keyword,
                left_parenthesis,
                condition,
                right_parenthesis,
                body,
            } => {
                self.nest_expression(None, false);
                self.token(*while_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.solo_zero_split();
                self.visit(Some(*condition));
                self.token(*right_parenthesis);
                self.unnest();
                self.visit_loop_body(*body);
            }
            Node::DoStatement {
                do_keyword,
                body,
                while_keyword,
                left_parenthesis,
                condition,
                right_parenthesis,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*do_keyword);
                self.space();
                self.unnest_later();
                self.visit(Some(*body));
                self.nest_expression(None, false);
                self.space();
                self.token(*while_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.solo_zero_split();
                self.visit(Some(*condition));
                self.token(*right_parenthesis);
                self.token(*semicolon);
                self.unnest();
            }
            Node::SwitchStatement { .. } => self.visit_switch_statement(id),
            Node::SwitchCase { .. } | Node::SwitchPatternCase { .. } | Node::SwitchDefault { .. } => {
                panic!("visitado pelo switch")
            }
            Node::TryStatement {
                try_keyword,
                body,
                catch_clauses,
                finally_keyword,
                finally_block,
            } => {
                self.token(*try_keyword);
                self.space();
                self.visit(Some(*body));
                self.visit_nodes(catch_clauses, Action::Space, Action::Space, Action::Nothing);
                if let Some(f) = finally_keyword {
                    self.write_preceding_comments_and_newlines(*f);
                    self.space();
                    let t = self.lexeme(*f);
                    self.write_text(t, *f);
                    self.space();
                }
                self.visit(*finally_block);
            }
            Node::CatchClause {
                on_keyword,
                exception_type,
                catch_keyword,
                left_parenthesis,
                exception_parameter,
                comma,
                stack_trace_parameter,
                right_parenthesis,
                body,
            } => {
                self.modifier(*on_keyword);
                self.visit(*exception_type);
                if let Some(ck) = catch_keyword {
                    if exception_type.is_some() {
                        self.space();
                    }
                    self.token(*ck);
                    self.space();
                    self.token_opt(*left_parenthesis);
                    self.visit(*exception_parameter);
                    self.modifier(*comma);
                    self.visit(*stack_trace_parameter);
                    self.token_opt(*right_parenthesis);
                    self.space();
                } else {
                    self.space();
                }
                self.visit(Some(*body));
            }
            Node::CatchClauseParameter { name } => self.token(*name),
            Node::ReturnStatement {
                return_keyword,
                expression,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*return_keyword);
                self.visit_before(*expression, Action::Space);
                self.token(*semicolon);
                self.unnest();
            }
            Node::BreakStatement {
                break_keyword: kw,
                label,
                semicolon,
            }
            | Node::ContinueStatement {
                continue_keyword: kw,
                label,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*kw);
                self.visit_before(*label, Action::Space);
                self.token(*semicolon);
                self.unnest();
            }
            Node::YieldStatement {
                yield_keyword,
                star,
                expression,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*yield_keyword);
                self.token_opt(*star);
                self.space();
                self.visit(Some(*expression));
                self.token(*semicolon);
                self.unnest();
            }
            Node::LabeledStatement { labels, statement } => {
                self.visit_nodes(labels, Action::Nothing, Action::Newline, Action::Newline);
                self.visit(Some(*statement));
            }
            Node::Label { label, colon } => {
                self.visit(Some(*label));
                self.token(*colon);
            }
            Node::AssertStatement {
                assert_keyword,
                left_parenthesis,
                condition,
                message,
                right_parenthesis,
                semicolon,
                ..
            } => {
                self.nest_expression(None, false);
                self.token(*assert_keyword);
                let mut args = vec![*condition];
                if let Some(m) = message {
                    args.push(*m);
                }
                if self.list_has_comma_after(&args) {
                    self.visit_collection_literal(*left_parenthesis, &args, *right_parenthesis, None, None, None, false, false);
                } else {
                    super::arguments::ArgumentListVisitor::for_arguments(self, *left_parenthesis, *right_parenthesis, &args).visit(self);
                }
                self.token(*semicolon);
                self.unnest();
            }
            Node::PatternVariableDeclarationStatement { declaration, semicolon } => {
                self.visit(Some(*declaration));
                self.token(*semicolon);
            }
            Node::PatternVariableDeclaration {
                metadata,
                keyword,
                pattern,
                equals,
                expression,
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.token(*keyword);
                self.space();
                self.visit(Some(*pattern));
                self.visit_assignment(*equals, *expression, false);
                self.unnest();
            }
            Node::AdjacentStrings { .. } => self.visit_adjacent_strings(id),
            Node::AsExpression {
                expression,
                as_operator: op,
                type_,
            }
            | Node::CastPattern {
                pattern: expression,
                as_token: op,
                type_,
            } => {
                self.start_span(cost::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.solo_split(cost::NORMAL);
                self.token(*op);
                self.space();
                self.visit(Some(*type_));
                self.unnest();
                self.end_span();
            }
            Node::AssignmentExpression {
                left_hand_side,
                operator,
                right_hand_side,
            } => {
                self.nest_expression(None, false);
                self.visit(Some(*left_hand_side));
                self.visit_assignment(*operator, *right_hand_side, false);
                self.unnest();
            }
            Node::AwaitExpression {
                await_keyword: kw,
                expression,
            }
            | Node::ThrowExpression {
                throw_keyword: kw,
                expression,
            } => {
                self.token(*kw);
                self.space();
                self.visit(Some(*expression));
            }
            Node::BinaryExpression { operator, .. } => {
                let nest = !self.parent_and(id, |p| matches!(p, Node::ExpressionFunctionBody { .. }));
                let prec = precedence(self.lexeme(*operator));
                self.visit_binary(id, Some(prec), nest);
            }
            Node::LogicalAndPattern { .. } | Node::LogicalOrPattern { .. } => self.visit_binary(id, None, true),
            Node::BooleanLiteral { literal }
            | Node::DoubleLiteral { literal }
            | Node::IntegerLiteral { literal }
            | Node::NullLiteral { literal } => self.token(*literal),
            Node::CascadeExpression { .. } => self.visit_cascade_expression(id),
            Node::ConditionalExpression { .. } => self.visit_conditional_expression(id),
            Node::FunctionExpression {
                type_parameters,
                parameters,
                body,
            } => self.visit_function_body(*type_parameters, *parameters, *body, |_| {}),
            Node::FunctionExpressionInvocation {
                function,
                type_arguments,
                argument_list,
            } => {
                self.start_span(cost::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*function));
                self.visit(*type_arguments);
                self.visit_argument_list(*argument_list, false);
                self.unnest();
                self.end_span();
            }
            Node::FunctionReference {
                function,
                type_arguments,
            } => {
                self.visit(Some(*function));
                self.visit(*type_arguments);
            }
            Node::IndexExpression { target, period, .. } => {
                self.nest_expression(None, false);
                if target.is_none() && period.is_some() {
                    self.token_opt(*period);
                } else {
                    self.visit(*target);
                }
                self.finish_index_expression(id);
                self.unnest();
            }
            Node::InstanceCreationExpression {
                keyword,
                constructor_name,
                argument_list,
            } => {
                self.start_span(cost::NORMAL);
                self.modifier(*keyword);
                self.start_span(cost::CONSTRUCTOR_NAME);
                self.nest_expression(None, false);
                self.visit(Some(*constructor_name));
                self.end_span();
                self.visit_argument_list(*argument_list, false);
                self.end_span();
                self.unnest();
            }
            Node::ConstructorName { type_, period, name } => {
                self.visit(Some(*type_));
                self.token_opt(*period);
                self.visit(*name);
            }
            Node::IsExpression {
                expression,
                is_operator,
                not_operator,
                type_,
            } => {
                self.start_span(cost::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.solo_split(cost::NORMAL);
                self.token(*is_operator);
                self.token_opt(*not_operator);
                self.space();
                self.visit(Some(*type_));
                self.unnest();
                self.end_span();
            }
            Node::ListLiteral {
                const_keyword,
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => {
                let c = if elements.len() <= 1 { cost::SINGLE_ELEMENT_LIST } else { cost::NORMAL };
                self.visit_collection_literal(*left_bracket, elements, *right_bracket, *const_keyword, *type_arguments, Some(c), true, false);
            }
            Node::SetOrMapLiteral {
                const_keyword,
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => self.visit_collection_literal(*left_bracket, elements, *right_bracket, *const_keyword, *type_arguments, None, true, false),
            Node::ListPattern {
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            }
            | Node::MapPattern {
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => self.visit_collection_literal(*left_bracket, elements, *right_bracket, None, *type_arguments, None, false, false),
            Node::MapLiteralEntry { key, separator, value } | Node::MapPatternEntry { key, separator, value } => {
                self.nest_expression(None, false);
                self.visit(Some(*key));
                self.token(*separator);
                self.solo_split(cost::NORMAL);
                self.visit(Some(*value));
                self.unnest();
            }
            Node::MethodInvocation { .. } => self.visit_method_invocation(id),
            Node::NamedExpression { .. } => {
                let (nome, colon_, expr) = self.named_parts(id);
                self.visit_named_node(nome, colon_, expr, None);
            }
            Node::ParenthesizedExpression {
                left_parenthesis,
                expression,
                right_parenthesis,
            }
            | Node::ParenthesizedPattern {
                left_parenthesis,
                pattern: expression,
                right_parenthesis,
            } => {
                self.nest_expression(None, false);
                self.token(*left_parenthesis);
                self.visit(Some(*expression));
                self.unnest();
                self.token(*right_parenthesis);
            }
            Node::PostfixExpression { operand, operator } => {
                self.visit(Some(*operand));
                self.token(*operator);
            }
            Node::PrefixExpression { operator, operand } => {
                self.token(*operator);
                if let Node::PrefixExpression { operator: op2, .. } = self.node(*operand) {
                    let l = self.lexeme(*op2);
                    if l == "-" || l == "--" {
                        self.space();
                    }
                }
                self.visit(Some(*operand));
            }
            Node::PrefixedIdentifier { .. } => super::chain::CallChainVisitor::new(self, id).visit(self),
            Node::PropertyAccess {
                target,
                operator,
                property_name,
            } => {
                if target.is_none() {
                    self.token(*operator);
                    self.visit(Some(*property_name));
                    return;
                }
                super::chain::CallChainVisitor::new(self, id).visit(self);
            }
            Node::RecordLiteral {
                const_keyword,
                left_parenthesis,
                fields,
                right_parenthesis,
            } => {
                self.modifier(*const_keyword);
                self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, true);
            }
            Node::RecordPattern {
                left_parenthesis,
                fields,
                right_parenthesis,
            } => self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, true),
            Node::RethrowExpression { rethrow_keyword } => self.token(*rethrow_keyword),
            Node::SimpleIdentifier { token } => self.token(*token),
            Node::SimpleStringLiteral { literal } => self.write_string_literal(*literal),
            Node::StringInterpolation { elements } => {
                for &e in elements {
                    self.visit(Some(e));
                }
            }
            Node::InterpolationExpression {
                left_bracket,
                expression,
                right_bracket,
            } => {
                self.b().0.prevent_splits();
                self.token(*left_bracket);
                self.start_span(cost::NORMAL);
                self.visit(Some(*expression));
                self.end_span();
                self.token_opt(*right_bracket);
                self.b().0.allow_splits();
            }
            Node::InterpolationString { contents } => self.write_string_literal(*contents),
            Node::SuperExpression { super_keyword: kw } | Node::ThisExpression { this_keyword: kw } => self.token(*kw),
            Node::SymbolLiteral { pound_sign, components } => {
                self.token(*pound_sign);
                for &c in components {
                    if c > 0 && self.lexeme(c - 1) == "." {
                        self.token(c - 1);
                    }
                    self.token(c);
                }
            }
            Node::SwitchExpression { .. } => self.visit_switch_expression(id),
            Node::SwitchExpressionCase { .. } => self.visit_switch_expression_case(id),
            Node::ArgumentList { .. } => self.visit_argument_list(id, true),
            Node::SpreadElement {
                spread_operator,
                expression,
            } => {
                self.token(*spread_operator);
                self.visit(Some(*expression));
            }
            Node::IfElement { .. } => self.visit_if_element(id),
            Node::ForElement { .. } => self.visit_for_element(id),
            Node::AssignedVariablePattern { name } => self.token(*name),
            Node::ConstantPattern {
                const_keyword,
                expression,
            } => {
                self.modifier(*const_keyword);
                self.visit(Some(*expression));
            }
            Node::DeclaredVariablePattern { keyword, type_, name } | Node::WildcardPattern { keyword, type_, name } => {
                self.modifier(*keyword);
                if let Some(t) = type_ {
                    self.visit(Some(*t));
                    self.solo_split(cost::NORMAL);
                }
                self.token(*name);
            }
            Node::NullAssertPattern { pattern, operator } | Node::NullCheckPattern { pattern, operator } => {
                self.visit(Some(*pattern));
                self.token(*operator);
            }
            Node::ObjectPattern {
                type_,
                left_parenthesis,
                fields,
                right_parenthesis,
            } => {
                self.visit(Some(*type_));
                self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, false);
            }
            Node::PatternField { name, pattern } => match name.map(|n| self.node(n)) {
                Some(Node::PatternFieldName { name: Some(nm), colon }) => {
                    self.visit_named_node(*nm, *colon, *pattern, None);
                }
                Some(Node::PatternFieldName { name: None, colon }) => {
                    self.token(*colon);
                    self.visit(Some(*pattern));
                }
                _ => self.visit(Some(*pattern)),
            },
            Node::PatternFieldName { .. } => panic!("visitado pelo campo"),
            Node::RelationalPattern { operator, operand } => {
                self.token(*operator);
                self.space();
                self.visit(Some(*operand));
            }
            Node::RestPatternElement { operator, pattern } => {
                self.token(*operator);
                self.visit(*pattern);
            }
            Node::PatternAssignment {
                pattern,
                equals,
                expression,
            } => {
                self.visit(Some(*pattern));
                self.visit_assignment(*equals, *expression, false);
            }
        }
    }

    // ======================================================== declarações

    fn visit_compilation_unit(&mut self, id: NodeId) {
        let Node::CompilationUnit {
            script_tag,
            directives,
            declarations,
        } = self.node(id)
        else {
            unreachable!()
        };
        if let Some(st) = script_tag {
            let t = self.lexeme(*st).trim();
            self.write_text(t, *st);
            self.two_newlines();
        }
        let mut directives_: &[NodeId] = directives;
        if let Some(&first_) = directives_.first()
            && matches!(self.node(first_), Node::LibraryDirective { .. })
        {
            self.visit(Some(first_));
            self.two_newlines();
            directives_ = &directives_[1..];
        }
        self.visit_nodes(directives_, Action::Nothing, Action::OneOrTwoNewlines, Action::Nothing);

        let mut needs_double = true;
        for &d in declarations {
            let has_body = matches!(
                self.node(d),
                Node::ClassDeclaration { .. } | Node::EnumDeclaration { .. } | Node::ExtensionDeclaration { .. }
            );
            if has_body {
                needs_double = true;
            }
            if needs_double {
                self.two_newlines();
            } else {
                self.one_or_two_newlines();
            }
            self.visit(Some(d));
            needs_double = false;
            if has_body {
                needs_double = true;
            } else if matches!(self.node(d), Node::FunctionDeclaration { .. })
                && let Some(corpo) = self.declaration_body(d)
                && matches!(self.node(corpo), Node::BlockFunctionBody { .. })
            {
                needs_double = self.non_empty_block(corpo);
            }
        }
    }

    /// `_visitDirectiveMetadata(directive)`.
    fn visit_directive_metadata(&mut self, id: NodeId, metadata: &[NodeId]) {
        let first_ = match self.parent(id).map(|p| self.node(p)) {
            Some(Node::CompilationUnit { directives, .. }) => directives.first() == Some(&id),
            _ => false,
        };
        let after = if first_ { Action::OneOrTwoNewlines } else { Action::Newline };
        self.visit_nodes(metadata, Action::Nothing, Action::Newline, after);
    }

    fn visit_import_directive(&mut self, id: NodeId) {
        let Node::ImportDirective {
            metadata,
            import_keyword,
            uri,
            configurations,
            deferred_keyword,
            as_keyword,
            prefix,
            combinators,
            semicolon,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_directive_metadata(id, metadata);
        self.nest_expression(None, false);
        self.token(*import_keyword);
        self.space();
        self.visit(Some(*uri));
        let mut wrote_config = false;
        if let Some(asw) = as_keyword
            && let Some(&c0) = configurations.first()
            && let Node::Configuration { if_keyword, .. } = self.node(c0)
            && self.tk.list[*if_keyword as usize].start < self.tk.list[*asw as usize].start
        {
            self.visit_configurations(configurations);
            wrote_config = true;
        }
        if let Some(asw) = as_keyword {
            self.solo_split(cost::NORMAL);
            self.modifier(*deferred_keyword);
            self.token(*asw);
            self.space();
            self.visit(*prefix);
        }
        if !wrote_config {
            self.visit_configurations(configurations);
        }
        self.visit_combinators(combinators);
        self.token(*semicolon);
        self.unnest();
    }

    fn visit_configurations(&mut self, configs: &[NodeId]) {
        if configs.is_empty() {
            return;
        }
        self.start_rule(None);
        for &c in configs {
            self.split();
            self.visit(Some(c));
        }
        self.end_rule();
    }

    fn visit_combinators(&mut self, combinators: &[NodeId]) {
        let r = self.a.combinator_rule();
        self.start_rule(Some(r));
        self.visit_nodes(combinators, Action::Nothing, Action::Nothing, Action::Nothing);
        self.end_rule();
    }

    /// `_visitCombinator(keyword, nodes)`.
    fn visit_combinator(&mut self, keyword: Tk, nodes: &[NodeId]) {
        let rule = self.current_rule();
        debug_assert!(self.a.is_combinator(rule));
        let c = self.split();
        self.a.add_combinator(rule, c);
        self.nest_expression(None, false);
        self.token(keyword);
        let c = self.split();
        self.a.add_name(rule, c);
        for (i, &n) in nodes.iter().enumerate() {
            if i > 0 {
                let c = self.split();
                self.a.add_name(rule, c);
            }
            self.accept(n);
            let proximo = self.tree_.end(n) + 1;
            if self.lexeme(proximo) == "," {
                self.token(proximo);
            }
        }
        self.unnest();
    }

    /// `_visitClauses(withClause, implementsClause)`.
    fn visit_clauses(&mut self, with: Option<NodeId>, implements: Option<NodeId>) {
        let r = self.a.combinator_rule();
        self.start_rule(Some(r));
        self.visit(with);
        self.visit(implements);
        self.end_rule();
    }

    fn visit_class_declaration(&mut self, id: NodeId) {
        let Node::ClassDeclaration {
            metadata,
            abstract_keyword,
            macro_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            class_keyword,
            name,
            type_parameters,
            extends_clause,
            with_clause,
            implements_clause,
            native_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*abstract_keyword);
        self.modifier(*base_keyword);
        self.modifier(*interface_keyword);
        self.modifier(*final_keyword);
        self.modifier(*sealed_keyword);
        self.modifier(*mixin_keyword);
        self.modifier(*macro_keyword);
        self.token(*class_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.visit(*extends_clause);
        self.visit_clauses(*with_clause, *implements_clause);
        self.visit_before(*native_clause, Action::Space);
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_class_type_alias(&mut self, id: NodeId) {
        let Node::ClassTypeAlias {
            metadata,
            abstract_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            typedef_keyword,
            name,
            type_parameters,
            equals,
            superclass,
            with_clause,
            implements_clause,
            semicolon,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*abstract_keyword);
        self.modifier(*base_keyword);
        self.modifier(*interface_keyword);
        self.modifier(*final_keyword);
        self.modifier(*sealed_keyword);
        self.modifier(*mixin_keyword);
        self.token(*typedef_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.space();
        self.token(*equals);
        self.space();
        self.visit(Some(*superclass));
        self.visit_clauses(*with_clause, *implements_clause);
        self.token(*semicolon);
        self.unnest();
    }

    fn visit_mixin_declaration(&mut self, id: NodeId) {
        let Node::MixinDeclaration {
            metadata,
            base_keyword,
            mixin_keyword,
            name,
            type_parameters,
            on_clause,
            implements_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*base_keyword);
        self.token(*mixin_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        let constraints: &[NodeId] = match on_clause.map(|o| self.node(o)) {
            Some(Node::MixinOnClause {
                superclass_constraints, ..
            }) => superclass_constraints,
            _ => &[],
        };
        if let Some(o) = on_clause
            && constraints.len() == 1
            && let Node::MixinOnClause { on_keyword, .. } = self.node(*o)
        {
            self.solo_split(cost::NORMAL);
            self.token(*on_keyword);
            self.space();
            self.visit(Some(constraints[0]));
        }
        let r = self.a.combinator_rule();
        self.start_rule(Some(r));
        if constraints.len() > 1 {
            self.visit(*on_clause);
        }
        self.visit(*implements_clause);
        self.end_rule();
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_extension_declaration(&mut self, id: NodeId) {
        let Node::ExtensionDeclaration {
            metadata,
            extension_keyword,
            name,
            type_parameters,
            on_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*extension_keyword);
        self.token_space_before(*name);
        self.visit(*type_parameters);
        if let Some(o) = on_clause
            && let Node::ExtensionOnClause { on_keyword, extended_type } = self.node(*o)
        {
            self.solo_split(cost::NORMAL);
            self.token(*on_keyword);
            self.space();
            self.visit(Some(*extended_type));
        }
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_extension_type_declaration(&mut self, id: NodeId) {
        let Node::ExtensionTypeDeclaration {
            metadata,
            extension_keyword,
            type_keyword,
            const_keyword,
            name,
            type_parameters,
            representation,
            implements_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*extension_keyword);
        self.space();
        self.token(*type_keyword);
        self.token_space_before(*const_keyword);
        self.space();
        self.token(*name);
        self.nest_expression(None, false);
        self.visit(*type_parameters);
        self.visit(Some(*representation));
        self.unnest();
        let r = self.a.combinator_rule();
        self.start_rule(Some(r));
        self.visit(*implements_clause);
        self.end_rule();
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_representation_declaration(&mut self, id: NodeId) {
        let Node::RepresentationDeclaration {
            constructor_name,
            left_parenthesis,
            field_metadata,
            field_type,
            field_name,
            right_parenthesis,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit(*constructor_name);
        self.token(*left_parenthesis);
        let rule = self.a.positional_rule(None, 1, 0, 0);
        self.start_rule(Some(rule));
        let c = self.zero_split();
        self.a.before_argument(rule, Some(c));
        self.start_block_argument_nesting();
        self.start_span(cost::NORMAL);
        let (ft, fname) = (*field_type, *field_name);
        self.visit_parameter_metadata(field_metadata, |v| {
            let r = v.a.rule(cost::PARAMETER_TYPE);
            v.start_lazy_rule(Some(r));
            v.nest_expression(None, false);
            v.visit(Some(ft));
            v.separator_between_type_and_variable(Some(ft), false);
            v.token(fname);
            v.unnest();
            v.end_rule();
        });
        self.end_block_argument_nesting();
        self.end_span();
        self.end_rule();
        self.token(*right_parenthesis);
    }

    fn visit_enum_declaration(&mut self, id: NodeId) {
        let Node::EnumDeclaration {
            metadata,
            enum_keyword,
            name,
            type_parameters,
            with_clause,
            implements_clause,
            left_bracket,
            constants,
            members,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*enum_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.visit_clauses(*with_clause, *implements_clause);
        self.space();
        self.unnest();
        self.begin_body(*left_bracket, true);
        self.visit_comma_separated_nodes(constants, Action::SplitOrTwoNewlines);
        let ultima = *constants.last().expect("enum com constantes");
        let virgula_final = self.comma_after(ultima);
        if virgula_final.is_some() {
            self.force_rules();
        }
        let after = self.tree_.end(ultima) + 1;
        let mut semicolon_ = None;
        if self.is_op(after, Op::Semicolon) {
            semicolon_ = Some(after);
        } else if let Some(v) = virgula_final
            && self.is_op(v + 1, Op::Semicolon)
        {
            semicolon_ = Some(after + 1);
        }
        if let Some(pv) = semicolon_ {
            if virgula_final.is_some() {
                self.newline();
            }
            self.token(pv);
            if !members.is_empty() {
                self.two_newlines();
            }
        }
        self.visit_body_contents(members);
        let force = semicolon_.is_some()
            || virgula_final.is_some()
            || !members.is_empty()
            || self.contains_line_comments(constants, None);
        self.end_body(*right_bracket, force);
    }

    fn visit_constructor_declaration(&mut self, id: NodeId) {
        let Node::ConstructorDeclaration {
            metadata,
            external_keyword,
            const_keyword,
            factory_keyword,
            return_type,
            period,
            name,
            parameters,
            initializers,
            redirected_constructor,
            body,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.modifier(*external_keyword);
        self.modifier(*const_keyword);
        self.modifier(*factory_keyword);
        self.visit(Some(*return_type));
        self.token_opt(*period);
        self.token_opt(*name);
        if !initializers.is_empty() {
            self.start_rule(None);
        }
        if redirected_constructor.is_some() {
            self.nest_expression(None, false);
        }
        let redir = *redirected_constructor;
        let has_initializers = !initializers.is_empty();
        self.visit_function_body(None, Some(*parameters), *body, |v| {
            if redir.is_some() {
                v.visit_constructor_redirects(id);
                v.unnest();
            } else if has_initializers {
                v.visit_constructor_initializers(id);
                v.end_rule();
            }
        });
    }

    fn visit_constructor_redirects(&mut self, id: NodeId) {
        let Node::ConstructorDeclaration {
            separator,
            initializers,
            redirected_constructor,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        self.token_space_before(*separator);
        self.solo_split(cost::NORMAL);
        self.visit_comma_separated_nodes(initializers, Action::Space);
        self.visit(*redirected_constructor);
    }

    fn visit_constructor_initializers(&mut self, id: NodeId) {
        let Node::ConstructorDeclaration {
            parameters,
            separator,
            initializers,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        let params: &[NodeId] = match self.node(*parameters) {
            Node::FormalParameterList { parameters, .. } => parameters,
            _ => &[],
        };
        let virgula_final = self.list_has_comma_after(params);
        let sep = separator.expect("separador dos inicializadores");
        if virgula_final {
            self.space();
            if initializers.len() > 1 {
                let ultimo = *params.last().expect("parâmetro");
                let k = self.param_kind(ultimo);
                let padding = if k == ParameterKind::Named || k == ParameterKind::OptionalPositional {
                    " "
                } else {
                    "  "
                };
                self.write_text(padding, sep);
            }
            self.token(sep);
            self.space();
            self.indent(Some(6));
        } else {
            self.indent(Some(recuo::CONSTRUCTOR_INITIALIZER));
            self.split();
            self.token(sep);
            self.space();
            self.indent(Some(2));
        }
        for (i, &start_) in initializers.iter().enumerate() {
            if i > 0 {
                let virgula = self.tree_.start(start_) - 1;
                self.token(virgula);
                self.newline();
            }
            self.accept(start_);
        }
        self.unindent();
        if !virgula_final {
            self.unindent();
        }
    }

    /// `_visitFunctionOrMethodDeclaration`.
    #[allow(clippy::too_many_arguments)]
    fn visit_function_or_method_declaration(
        &mut self,
        metadata: &[NodeId],
        external_keyword: Option<Tk>,
        property_keyword: Option<Tk>,
        modifier_keyword: Option<Tk>,
        operator_keyword: Option<Tk>,
        name: Tk,
        return_type: Option<NodeId>,
        type_parameters: Option<NodeId>,
        formal_parameters: Option<NodeId>,
        body: NodeId,
    ) {
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.start_span(cost::NORMAL);
        self.modifier(external_keyword);
        self.modifier(modifier_keyword);
        if let Some(rt) = return_type {
            self.visit(Some(rt));
            self.solo_split(cost::NORMAL);
        }
        self.modifier(property_keyword);
        self.modifier(operator_keyword);
        self.token(name);
        self.end_span();
        let expr = matches!(self.node(body), Node::ExpressionFunctionBody { .. });
        self.visit_function_body(type_parameters, formal_parameters, body, |v| {
            if !expr {
                v.unnest();
            }
        });
        if expr {
            self.unnest();
        }
    }

    /// `_visitFunctionBody(typeParameters, parameters, body, beforeBody)`.
    fn visit_function_body(&mut self, type_parameters: Option<NodeId>, parameters: Option<NodeId>, body: NodeId, before_body: impl FnOnce(&mut Self)) {
        let expr = matches!(self.node(body), Node::ExpressionFunctionBody { .. });
        if expr {
            self.nest_expression(None, false);
            let r = self.a.rule(cost::ARROW);
            self.start_lazy_rule(Some(r));
        }
        self.visit_parameter_signature(type_parameters, parameters);
        before_body(self);
        self.visit(Some(body));
        if expr {
            self.unnest();
        }
    }

    /// `_visitParameterSignature(typeParameters, parameters)`.
    fn visit_parameter_signature(&mut self, type_parameters: Option<NodeId>, parameters: Option<NodeId>) {
        self.nest_expression(None, false);
        self.visit(type_parameters);
        if let Some(p) = parameters {
            self.visit_formal_parameter_list(p, false);
        }
        self.unnest();
    }

    fn visit_expression_function_body(&mut self, id: NodeId) {
        let Node::ExpressionFunctionBody {
            keyword,
            star,
            function_definition,
            expression,
            semicolon,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.space();
        self.token_opt(*keyword);
        self.token_opt(*star);
        if keyword.is_some() || star.is_some() {
            self.space();
        }
        let function_body = self.is_function_expression_body(id);
        if function_body {
            self.start_span(cost::NORMAL);
        }
        self.token(*function_definition);
        self.split();
        let binary = matches!(self.node(*expression), Node::BinaryExpression { .. });
        if !binary {
            self.end_rule();
        }
        if function_body {
            self.end_span();
        }
        let mut arg_with_comma = false;
        if let Some(p) = self.parent(id)
            && matches!(self.node(p), Node::FunctionExpression { .. })
        {
            arg_with_comma = self.is_trailing_comma_argument(p);
        }
        if !arg_with_comma {
            self.start_block_argument_nesting();
        }
        self.start_span(cost::NORMAL);
        self.visit(Some(*expression));
        self.end_span();
        if !arg_with_comma {
            self.end_block_argument_nesting();
        }
        if binary {
            self.end_rule();
        }
        self.token_opt(*semicolon);
    }

    // ============================================================ parâmetros

    /// `visitFormalParameterList(node, nestExpression:)`.
    fn visit_formal_parameter_list(&mut self, id: NodeId, nest: bool) {
        let Node::FormalParameterList {
            left_parenthesis,
            parameters,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
        } = self.node(id)
        else {
            unreachable!()
        };
        if parameters.is_empty() {
            self.token(*left_parenthesis);
            if self.has_comments(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            return;
        }
        if self.list_has_comma_after(parameters) {
            self.visit_trailing_comma_parameter_list(id);
            return;
        }
        let required_count: Vec<NodeId> = parameters
            .iter()
            .copied()
            .filter(|&p| !matches!(self.node(p), Node::DefaultFormalParameter { .. }))
            .collect();
        let optionals: Vec<NodeId> = parameters
            .iter()
            .copied()
            .filter(|&p| matches!(self.node(p), Node::DefaultFormalParameter { .. }))
            .collect();
        if nest {
            self.nest_expression(None, false);
        }
        self.token(*left_parenthesis);
        let mut rule = None;
        if !required_count.is_empty() {
            let r = self.a.positional_rule(None, required_count.len(), 0, 0);
            rule = Some(r);
            self.start_rule(Some(r));
            if self.is_function_expression_body(id) {
                self.a.before_argument(r, None);
            } else {
                let c = self.zero_split();
                self.a.before_argument(r, Some(c));
            }
            self.start_block_argument_nesting();
            self.start_span(cost::NORMAL);
            for (i, &p) in required_count.iter().enumerate() {
                self.visit(Some(p));
                self.write_comma_after(p);
                if i + 1 != required_count.len() {
                    let c = self.split();
                    self.a.before_argument(r, Some(c));
                }
            }
            self.end_block_argument_nesting();
            self.end_span();
            self.end_rule();
        }
        if !optionals.is_empty() {
            let named_ = self.a.named_rule(None, 0, 0);
            if let Some(r) = rule {
                self.a.named_constraints(r, named_);
            }
            self.start_rule(Some(named_));
            self.start_block_argument_nesting();
            let c = self.builder_split(true, !required_count.is_empty());
            self.a.before_argument(named_, Some(c));
            self.token_opt(*left_delimiter);
            for (i, &p) in optionals.iter().enumerate() {
                self.visit(Some(p));
                self.write_comma_after(p);
                if i + 1 != optionals.len() {
                    let c = self.split();
                    self.a.before_argument(named_, Some(c));
                }
            }
            self.end_block_argument_nesting();
            self.end_rule();
            self.token_opt(*right_delimiter);
        }
        self.token(*right_parenthesis);
        if nest {
            self.unnest();
        }
    }

    /// `_visitTrailingCommaParameterList(parameters)`.
    fn visit_trailing_comma_parameter_list(&mut self, id: NodeId) {
        let Node::FormalParameterList {
            left_parenthesis,
            parameters,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
        } = self.node(id)
        else {
            unreachable!()
        };
        let hard = self.a.hard_rule();
        self.start_rule(Some(hard));
        self.token(*left_parenthesis);
        let mut last_required = None;
        for (i, &p) in parameters.iter().enumerate() {
            if matches!(self.node(p), Node::DefaultFormalParameter { .. }) {
                if i > 0 {
                    last_required = Some(parameters[i - 1]);
                }
                break;
            }
        }
        if matches!(self.node(parameters[0]), Node::DefaultFormalParameter { .. }) {
            self.token_opt(*left_delimiter);
        }
        self.start_block(None, true, false);
        for &p in parameters {
            self.write_newline(false, false, false);
            self.visit(Some(p));
            self.write_comma_after(p);
            if Some(p) == last_required {
                self.space();
                self.token_opt(*left_delimiter);
                last_required = None;
            }
        }
        let first_delim = right_delimiter.unwrap_or(*right_parenthesis);
        if self.has_comments(first_delim) {
            self.write_newline(false, false, false);
            self.write_preceding_comments_and_newlines(first_delim);
        }
        self.end_block(true);
        self.end_rule();
        let t = self.lexeme(first_delim);
        self.write_text(t, first_delim);
        if first_delim != *right_parenthesis {
            self.token(*right_parenthesis);
        }
    }

    /// `_beginFormalParameter(node)`.
    fn begin_formal_parameter(&mut self, id: NodeId) {
        let r = self.a.rule(cost::PARAMETER_TYPE);
        self.start_lazy_rule(Some(r));
        self.nest_expression(None, false);
        let (req, covariant_) = match self.node(id) {
            Node::SimpleFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | Node::FieldFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | Node::SuperFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | Node::FunctionTypedFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            } => (*required_keyword, *covariant_keyword),
            _ => (None, None),
        };
        self.modifier(req);
        self.modifier(covariant_);
    }

    /// `_endFormalParameter(node)`.
    fn end_formal_parameter(&mut self) {
        self.unnest();
        self.end_rule();
    }

    /// `visitParameterMetadata(metadata, visitParameter)`.
    fn visit_parameter_metadata(&mut self, metadata: &[NodeId], visit: impl FnOnce(&mut Self)) {
        if metadata.is_empty() {
            visit(self);
            return;
        }
        self.start_lazy_rule(None);
        self.visit_nodes(metadata, Action::Nothing, Action::Split, Action::Split);
        visit(self);
        self.end_rule();
    }

    /// `_separatorBetweenTypeAndVariable(type, isSolo:)`.
    fn separator_between_type_and_variable(&mut self, tipo: Option<NodeId>, solo: bool) {
        let Some(t) = tipo else {
            return;
        };
        let block_type = match self.node(t) {
            Node::GenericFunctionType { parameters, .. } => match self.node(*parameters) {
                Node::FormalParameterList { parameters, .. } => !parameters.is_empty() && self.list_has_comma_after(parameters),
                _ => false,
            },
            Node::RecordTypeAnnotation { .. } => true,
            _ => false,
        };
        if block_type {
            self.space();
        } else if solo {
            self.solo_split(cost::NORMAL);
        } else {
            self.split();
        }
    }

    /// `_visitGenericFunctionType`.
    fn visit_generic_function_type(&mut self, return_type: Option<NodeId>, function_keyword: Tk, type_parameters: Option<NodeId>, parameters: NodeId) {
        self.start_lazy_rule(None);
        self.nest_expression(None, false);
        self.visit_after(return_type, Action::Split);
        self.token(function_keyword);
        self.unnest();
        self.end_rule();
        self.visit_parameter_signature(type_parameters, Some(parameters));
    }

    /// `_visitGenericTypeAliasHeader`.
    fn visit_generic_type_alias_header(&mut self, typedef_keyword: Tk, name: Tk, type_parameters: Option<NodeId>, equals: Tk) {
        self.token(typedef_keyword);
        self.space();
        self.start_rule(None);
        self.token(name);
        self.visit(type_parameters);
        self.split();
        self.token(equals);
        self.end_rule();
    }

    fn visit_record_type_annotation(&mut self, id: NodeId) {
        let Node::RecordTypeAnnotation {
            left_parenthesis,
            positional_fields,
            named_fields,
            right_parenthesis,
            question,
        } = self.node(id)
        else {
            unreachable!()
        };
        if positional_fields.is_empty() && named_fields.is_none() {
            self.token(*left_parenthesis);
            if self.has_comments(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            self.token_opt(*question);
            return;
        }
        let named_args = named_fields.map(|n| match self.node(n) {
            Node::RecordTypeAnnotationNamedFields {
                left_bracket,
                fields,
                right_bracket,
            } => (*left_bracket, fields, *right_bracket),
            _ => unreachable!(),
        });
        self.token(*left_parenthesis);
        self.start_rule(None);
        if positional_fields.is_empty()
            && let Some((lb, _, _)) = named_args
        {
            self.token(lb);
        }
        self.start_block(None, true, false);
        for (i, &f) in positional_fields.iter().enumerate() {
            self.builder_split(false, i != 0);
            self.visit(Some(f));
            self.write_comma_after(f);
        }
        let mut first_closer = *right_parenthesis;
        if let Some((lb, fields_, rb)) = named_args {
            if !positional_fields.is_empty() {
                self.space();
                self.token(lb);
            }
            for (i, &f) in fields_.iter().enumerate() {
                self.builder_split(false, i != 0);
                self.visit(Some(f));
                self.write_comma_after(f);
            }
            first_closer = rb;
        }
        if self.has_comments(first_closer) {
            self.newline();
            self.write_preceding_comments_and_newlines(first_closer);
        }
        let force = match named_args {
            None => positional_fields.len() > 1 && self.has_comma_after(*positional_fields.last().expect("campo")),
            Some((_, fields_, _)) => fields_.last().is_some_and(|&c| self.has_comma_after(c)),
        };
        self.end_block(force);
        self.end_rule();
        let t = self.lexeme(first_closer);
        self.write_text(t, first_closer);
        if named_args.is_some() {
            self.token(*right_parenthesis);
        }
        self.token_opt(*question);
    }

    /// `_visitGenericList(leftBracket, rightBracket, nodes)`.
    fn visit_generic_list(&mut self, left_side: Tk, right_: Tk, nodes: &[NodeId]) {
        let rule = self.a.type_argument_rule();
        self.start_lazy_rule(Some(rule));
        self.start_span(cost::NORMAL);
        self.nest_expression(None, false);
        self.token(left_side);
        let c = self.zero_split();
        self.a.before_argument(rule, Some(c));
        self.start_block_argument_nesting();
        for (i, &n) in nodes.iter().enumerate() {
            self.visit(Some(n));
            if i + 1 != nodes.len() {
                let mut virgula = self.tree_.end(n) + 1;
                if self.lexeme(virgula) == "?" {
                    virgula += 1;
                }
                self.token(virgula);
                let c = self.split();
                self.a.before_argument(rule, Some(c));
            }
        }
        self.token(right_);
        self.end_block_argument_nesting();
        self.unnest();
        self.end_span();
        self.end_rule();
    }

    // ============================================================ instruções

    /// `_visitLoopBody(body)`.
    fn visit_loop_body(&mut self, corpo: NodeId) {
        match self.node(corpo) {
            Node::EmptyStatement { .. } => self.visit(Some(corpo)),
            Node::Block { .. } => {
                self.space();
                self.visit(Some(corpo));
            }
            _ => {
                self.indent(None);
                self.start_rule(None);
                self.builder_split(false, true);
                self.visit(Some(corpo));
                self.end_rule();
                self.unindent();
            }
        }
    }

    fn visit_for_parts(&mut self, id: NodeId) {
        let (left_separator, condition, right_separator, updaters) = match self.node(id) {
            Node::ForPartsWithDeclarations {
                variables,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.nest_expression(None, false);
                self.start_rule(None);
                if let Node::VariableDeclarationList {
                    metadata,
                    keyword,
                    type_,
                    variables: vars,
                    ..
                } = self.node(*variables)
                {
                    self.visit_nodes(metadata, Action::Nothing, Action::Split, Action::Split);
                    self.modifier(*keyword);
                    self.visit_after(*type_, Action::Space);
                    self.visit_comma_separated_nodes(vars, Action::Split);
                }
                self.end_rule();
                self.unnest();
                (*left_separator, *condition, *right_separator, updaters)
            }
            Node::ForPartsWithExpression {
                initialization,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.visit(*initialization);
                (*left_separator, *condition, *right_separator, updaters)
            }
            Node::ForPartsWithPattern {
                variables,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.start_block_argument_nesting();
                self.nest_expression(None, false);
                if let Node::PatternVariableDeclaration {
                    metadata,
                    keyword,
                    pattern,
                    equals,
                    expression,
                } = self.node(*variables)
                {
                    self.visit_nodes(metadata, Action::Nothing, Action::Split, Action::Split);
                    self.token(*keyword);
                    self.space();
                    self.visit(Some(*pattern));
                    self.visit_assignment(*equals, *expression, false);
                }
                self.unnest();
                self.end_block_argument_nesting();
                (*left_separator, *condition, *right_separator, updaters)
            }
            _ => unreachable!(),
        };
        self.token(left_separator);
        if condition.is_some() {
            self.split();
        }
        self.visit(condition);
        self.token(right_separator);
        if !updaters.is_empty() {
            self.split();
            self.start_rule(None);
            self.visit_comma_separated_nodes(updaters, Action::Split);
            self.end_rule();
        }
    }

    fn visit_for_each_parts_from_in(&mut self, in_keyword: Tk, iterable: NodeId) {
        self.solo_split(cost::NORMAL);
        self.token(in_keyword);
        self.space();
        self.visit(Some(iterable));
    }

    fn visit_if_statement(&mut self, id: NodeId) {
        let Node::IfStatement {
            if_keyword,
            left_parenthesis,
            expression,
            case_clause,
            right_parenthesis,
            then_statement,
            else_keyword,
            else_statement,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_if_condition(*if_keyword, *left_parenthesis, *expression, *case_clause, *right_parenthesis);
        let has_else = else_statement.is_some();
        let visit_clause = |v: &mut Self, clausula: NodeId| {
            if matches!(v.node(clausula), Node::Block { .. } | Node::IfStatement { .. }) {
                v.space();
                v.visit(Some(clausula));
            } else {
                v.indent(None);
                v.start_rule(None);
                if has_else {
                    v.write_newline(false, false, false);
                } else {
                    v.builder_split(false, true);
                }
                v.visit(Some(clausula));
                v.end_rule();
                v.unindent();
            }
        };
        visit_clause(self, *then_statement);
        if let Some(else_) = else_statement {
            if matches!(self.node(*then_statement), Node::Block { .. }) {
                self.space();
            } else {
                self.newline();
            }
            self.token_opt(*else_keyword);
            visit_clause(self, *else_);
        }
    }

    /// `_visitIfCondition`.
    fn visit_if_condition(&mut self, if_keyword: Tk, left_parenthesis: Tk, condition: NodeId, case_clause: Option<NodeId>, right_parenthesis: Tk) {
        self.nest_expression(None, false);
        self.token(if_keyword);
        self.space();
        self.token(left_parenthesis);
        match case_clause {
            None => self.visit(Some(condition)),
            Some(cc) => {
                let Node::CaseClause {
                    case_keyword,
                    guarded_pattern,
                } = self.node(cc)
                else {
                    unreachable!()
                };
                let Node::GuardedPattern { pattern, when_clause } = self.node(*guarded_pattern) else {
                    unreachable!()
                };
                let case_rule = self.a.rule(cost::NORMAL);
                self.start_rule(Some(case_rule));
                self.visit(Some(condition));
                self.split();
                self.token(*case_keyword);
                self.space();
                self.start_block_argument_nesting();
                self.nest_expression(None, true);
                self.visit(Some(*pattern));
                self.unnest();
                self.end_block_argument_nesting();
                self.end_rule();
                if let Some(w) = when_clause {
                    self.start_rule(None);
                    self.split();
                    self.start_block_argument_nesting();
                    self.nest_expression(None, false);
                    self.visit_when_clause(*w);
                    self.unnest();
                    self.end_block_argument_nesting();
                    self.end_rule();
                }
            }
        }
        self.token(right_parenthesis);
        self.unnest();
    }

    fn visit_when_clause(&mut self, w: NodeId) {
        let Node::WhenClause { when_keyword, expression } = self.node(w) else {
            unreachable!()
        };
        self.token(*when_keyword);
        self.space();
        self.visit(Some(*expression));
    }

    fn visit_switch_statement(&mut self, id: NodeId) {
        let Node::SwitchStatement {
            switch_keyword,
            left_parenthesis,
            expression,
            right_parenthesis,
            left_bracket,
            members,
            right_bracket,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
        self.begin_body(*left_bracket, false);
        for &m in members {
            let (labels, keyword, colon, statements) = match self.node(m) {
                Node::SwitchCase {
                    labels,
                    keyword,
                    colon,
                    statements,
                    ..
                }
                | Node::SwitchPatternCase {
                    labels,
                    keyword,
                    colon,
                    statements,
                    ..
                }
                | Node::SwitchDefault {
                    labels,
                    keyword,
                    colon,
                    statements,
                } => (labels, *keyword, *colon, statements),
                _ => unreachable!(),
            };
            self.visit_nodes(labels, Action::Nothing, Action::Newline, Action::Newline);
            self.token(keyword);
            match self.node(m) {
                Node::SwitchCase { expression, .. } => {
                    self.space();
                    self.visit(Some(*expression));
                }
                Node::SwitchPatternCase { guarded_pattern, .. } => {
                    self.space();
                    let Node::GuardedPattern { pattern, when_clause } = self.node(*guarded_pattern) else {
                        unreachable!()
                    };
                    match when_clause {
                        None => {
                            self.indent(None);
                            self.visit(Some(*pattern));
                            self.unindent();
                        }
                        Some(w) => {
                            self.start_rule(None);
                            self.nest_expression(None, false);
                            self.start_block_argument_nesting();
                            self.visit(Some(*pattern));
                            self.split();
                            self.visit_when_clause(*w);
                            self.end_block_argument_nesting();
                            self.unnest();
                            self.end_rule();
                        }
                    }
                }
                _ => {}
            }
            self.token(colon);
            if !statements.is_empty() {
                self.indent(None);
                self.newline();
                self.visit_nodes(statements, Action::Nothing, Action::OneOrTwoNewlines, Action::Nothing);
                self.unindent();
                self.one_or_two_newlines();
            } else {
                self.write_newline(false, false, false);
            }
        }
        if !members.is_empty() {
            self.newline();
        }
        self.end_body(*right_bracket, !members.is_empty());
    }

    /// `_visitSwitchValue`.
    fn visit_switch_value(&mut self, switch_keyword: Tk, left_parenthesis: Tk, value: NodeId, right_parenthesis: Tk) {
        self.nest_expression(None, false);
        self.token(switch_keyword);
        self.space();
        self.token(left_parenthesis);
        self.solo_zero_split();
        self.visit(Some(value));
        self.token(right_parenthesis);
        self.space();
        self.unnest();
    }

    fn visit_switch_expression(&mut self, id: NodeId) {
        let Node::SwitchExpression {
            switch_keyword,
            left_parenthesis,
            expression,
            right_parenthesis,
            left_bracket,
            cases,
            right_bracket,
        } = self.node(id)
        else {
            unreachable!()
        };
        if !self.can_split(cases, *right_bracket) {
            self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
            self.token(*left_bracket);
            self.token(*right_bracket);
            return;
        }
        self.start_rule(None);
        self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
        self.token(*left_bracket);
        self.start_block(None, true, !cases.is_empty());
        self.visit_comma_separated_nodes(cases, Action::Split);
        let virgula_final = cases.last().is_some_and(|&c| self.comma_after(c).is_some());
        let force = self.contains_line_comments(cases, Some(*right_bracket));
        self.end_body(*right_bracket, virgula_final || force);
    }

    fn visit_switch_expression_case(&mut self, id: NodeId) {
        let Node::SwitchExpressionCase {
            guarded_pattern,
            arrow,
            expression,
        } = self.node(id)
        else {
            unreachable!()
        };
        let Node::GuardedPattern { pattern, when_clause } = self.node(*guarded_pattern) else {
            unreachable!()
        };
        let mut branches = Vec::new();
        let mut ops = Vec::new();
        self.flatten_or(*pattern, &mut branches, &mut ops);
        self.start_lazy_rule(None);
        for i in 0..branches.len() - 1 {
            self.visit(Some(branches[i]));
            self.space();
            self.token(ops[i]);
            self.split();
        }
        self.nest_expression(Some(recuo::BLOCK), false);
        if when_clause.is_some() {
            self.start_lazy_rule(None);
            self.nest_expression(Some(recuo::BLOCK), false);
        }
        self.visit(branches.last().copied());
        if let Some(w) = when_clause {
            self.split();
            self.start_block_argument_nesting();
            self.visit_when_clause(*w);
            self.end_block_argument_nesting();
            self.unnest();
            self.end_rule();
        }
        self.space();
        self.token(*arrow);
        self.split();
        self.end_rule();
        self.start_block_argument_nesting();
        self.visit(Some(*expression));
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn flatten_or(&self, p: NodeId, branches: &mut Vec<NodeId>, ops: &mut Vec<Tk>) {
        if let Node::LogicalOrPattern {
            left_operand,
            operator,
            right_operand,
        } = self.node(p)
        {
            self.flatten_or(*left_operand, branches, ops);
            ops.push(*operator);
            self.flatten_or(*right_operand, branches, ops);
        } else {
            branches.push(p);
        }
    }

    /// `_splitEmptyBlock(node)`.
    fn split_empty_block(&self, id: NodeId) -> bool {
        let Some(p) = self.parent(id) else {
            return false;
        };
        if let Node::IfStatement {
            else_statement,
            then_statement,
            ..
        } = self.node(p)
        {
            return else_statement.is_some() && *then_statement == id;
        }
        if matches!(self.node(p), Node::CatchClause { .. })
            && let Some(avo) = self.parent(p)
            && let Node::TryStatement {
                finally_block,
                catch_clauses,
                ..
            } = self.node(avo)
        {
            let last_body = catch_clauses.last().and_then(|c| match self.node(*c) {
                Node::CatchClause { body, .. } => Some(*body),
                _ => None,
            });
            return finally_block.is_some() || Some(id) != last_body;
        }
        false
    }

    fn visit_variable_declaration_list(&mut self, id: NodeId) {
        let Node::VariableDeclarationList {
            metadata,
            late_keyword,
            keyword,
            type_,
            variables,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.start_span(cost::NORMAL);
        self.modifier(*late_keyword);
        self.modifier(*keyword);
        self.visit(*type_);
        self.separator_between_type_and_variable(*type_, true);
        self.end_span();
        self.start_rule(None);
        if variables.len() > 1 {
            self.start_block_argument_nesting();
        }
        self.visit_comma_separated_nodes(variables, Action::Split);
        if variables.len() > 1 {
            self.end_block_argument_nesting();
        }
        self.end_rule();
    }

    // ============================================================ expressões

    fn visit_adjacent_strings(&mut self, id: NodeId) {
        let Node::AdjacentStrings { strings } = self.node(id) else {
            unreachable!()
        };
        let mut nest = true;
        if let Some(p) = self.parent(id) {
            match self.node(p) {
                Node::ArgumentList { arguments, .. } => {
                    nest = false;
                    for &arg in arguments {
                        if arg == id {
                            continue;
                        }
                        if self.is_string_literal(arg) {
                            nest = true;
                            break;
                        }
                    }
                }
                Node::AssertInitializer { condition, message, .. } | Node::AssertStatement { condition, message, .. } => {
                    nest = false;
                    if *condition != id && self.is_string_literal(*condition) {
                        nest = true;
                    }
                    if let Some(m) = message
                        && *m != id
                        && self.is_string_literal(*m)
                    {
                        nest = true;
                    }
                }
                Node::VariableDeclaration { .. } => nest = false,
                Node::AssignmentExpression { right_hand_side, .. }
                    if *right_hand_side == id && self.parent_and(p, |n| matches!(n, Node::ExpressionStatement { .. })) =>
                {
                    nest = false
                }
                Node::NamedExpression { .. } | Node::ExpressionFunctionBody { .. } => nest = false,
                _ => {}
            }
        }
        self.start_span(cost::NORMAL);
        self.start_rule(None);
        if nest {
            self.nest_expression(None, false);
        }
        self.visit_nodes(strings, Action::Nothing, Action::SplitOrNewline, Action::Nothing);
        if nest {
            self.unnest();
        }
        self.end_rule();
        self.end_span();
    }

    /// `visitArgumentList(node, nestExpression:)`.
    pub fn visit_argument_list(&mut self, id: NodeId, nest: bool) {
        let Node::ArgumentList {
            left_parenthesis,
            arguments,
            right_parenthesis,
        } = self.node(id)
        else {
            unreachable!()
        };
        if arguments.is_empty() {
            self.token(*left_parenthesis);
            if self.has_comments(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            return;
        }
        if self.list_has_comma_after(arguments) {
            self.visit_collection_literal(*left_parenthesis, arguments, *right_parenthesis, None, None, None, false, false);
            return;
        }
        if nest {
            self.nest_expression(None, false);
        }
        super::arguments::ArgumentListVisitor::new(self, id).visit(self);
        if nest {
            self.unnest();
        }
    }

    fn visit_cascade_expression(&mut self, id: NodeId) {
        let Node::CascadeExpression {
            target,
            cascade_sections,
        } = self.node(id)
        else {
            unreachable!()
        };
        if cascade_sections.len() > 1 {
            self.visit_split_cascade(*target, cascade_sections);
            return;
        }
        let mut split_with_target = true;
        if self.is_collection_literal(*target) {
            split_with_target = false;
        } else {
            match self.node(*target) {
                Node::MethodInvocation { argument_list, .. }
                | Node::FunctionExpressionInvocation { argument_list, .. }
                | Node::InstanceCreationExpression { argument_list, .. } => {
                    if let Node::ArgumentList { arguments, .. } = self.node(*argument_list) {
                        split_with_target = !self.list_has_comma_after(arguments);
                    }
                }
                _ => {}
            }
        }
        let inline = self.allow_inline(*target, cascade_sections.len());
        if split_with_target {
            let r = if inline { self.a.rule(cost::NORMAL) } else { self.a.hard_rule() };
            self.start_lazy_rule(Some(r));
        }
        self.visit(Some(*target));
        self.nest_expression(Some(recuo::CASCADE), true);
        self.start_block_argument_nesting();
        if !split_with_target {
            let r = if inline { self.a.rule(cost::NORMAL) } else { self.a.hard_rule() };
            self.start_rule(Some(r));
        }
        self.zero_split();
        if !split_with_target {
            self.end_rule();
        }
        self.visit_nodes(cascade_sections, Action::Nothing, Action::ZeroSplit, Action::Nothing);
        if split_with_target {
            self.end_rule();
        }
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn visit_split_cascade(&mut self, target: NodeId, sections: &[NodeId]) {
        let hard = self.a.hard_rule();
        self.start_lazy_rule(Some(hard));
        self.visit(Some(target));
        self.nest_expression(Some(recuo::CASCADE), true);
        self.start_block_argument_nesting();
        let first = self.tree_.start(sections[0]);
        self.write_preceding_comments_and_newlines(first);
        self.suppressed.insert(first);
        self.start_block(None, false, false);
        for &s in &sections[..sections.len() - 1] {
            self.newline();
            self.visit(Some(s));
        }
        let ultimo = self.tree_.start(*sections.last().expect("seção"));
        self.write_preceding_comments_and_newlines(ultimo);
        self.suppressed.insert(ultimo);
        self.end_block(true);
        self.visit(sections.last().copied());
        self.end_rule();
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn visit_conditional_expression(&mut self, id: NodeId) {
        let Node::ConditionalExpression {
            condition,
            question,
            then_expression,
            colon,
            else_expression,
        } = self.node(id)
        else {
            unreachable!()
        };
        self.nest_expression(None, false);
        self.start_lazy_rule(None);
        self.visit(Some(*condition));
        self.nest_expression(Some(recuo::BLOCK), true);
        self.start_block_argument_nesting();
        self.unnest();
        self.start_span(cost::NORMAL);
        self.split();
        self.token(*question);
        self.space();
        self.nest_expression(None, false);
        self.visit(Some(*then_expression));
        self.unnest();
        self.split();
        self.token(*colon);
        self.space();
        self.visit(Some(*else_expression));
        if self.parent_and(id, |p| matches!(p, Node::ConditionalExpression { .. })) {
            self.force_rules();
        }
        self.end_rule();
        self.end_span();
        self.end_block_argument_nesting();
        self.unnest();
    }

    /// `finishIndexExpression(node)`.
    pub fn finish_index_expression(&mut self, id: NodeId) {
        let Node::IndexExpression {
            target,
            question,
            left_bracket,
            index,
            right_bracket,
            ..
        } = self.node(id)
        else {
            unreachable!()
        };
        if let Some(t) = target
            && matches!(self.node(*t), Node::IndexExpression { .. })
        {
            self.solo_zero_split();
        }
        self.start_span(cost::INDEX);
        self.token_opt(*question);
        self.token(*left_bracket);
        self.solo_zero_split();
        self.visit(Some(*index));
        self.token(*right_bracket);
        self.end_span();
    }

    fn visit_method_invocation(&mut self, id: NodeId) {
        let Node::MethodInvocation {
            target,
            operator,
            method_name,
            type_arguments,
            argument_list,
        } = self.node(id)
        else {
            unreachable!()
        };
        if target.is_none() || self.looks_like_static_call(id) {
            self.nest_expression(None, false);
            self.start_span(cost::NORMAL);
            if target.is_some() {
                self.start_span(cost::CONSTRUCTOR_NAME);
                self.visit(*target);
                self.solo_zero_split();
            }
            self.token_opt(*operator);
            self.visit(Some(*method_name));
            if target.is_some() {
                self.end_span();
            }
            self.nest_expression(None, false);
            self.visit(*type_arguments);
            self.visit_argument_list(*argument_list, false);
            self.unnest();
            self.end_span();
            self.unnest();
            return;
        }
        super::chain::CallChainVisitor::new(self, id).visit(self);
    }

    /// `visitNamedNode(name, colon, node, [rule])`.
    pub fn visit_named_node(&mut self, nome: Tk, colon_: Tk, node: NodeId, rule: Option<RuleId>) {
        self.nest_expression(None, false);
        self.start_span(cost::NORMAL);
        self.token(nome);
        self.token(colon_);
        if matches!(self.node(node), Node::ListLiteral { .. } | Node::SetOrMapLiteral { .. } | Node::RecordLiteral { .. }) {
            self.space();
        } else {
            let r = self.solo_split(cost::NORMAL);
            if let Some(named_) = rule {
                self.a.constrain_when_split(r, named_);
            }
        }
        self.visit(Some(node));
        self.end_span();
        self.unnest();
    }

    /// `_visitAssignment(equalsOperator, rightHandSide, nest:)`.
    fn visit_assignment(&mut self, equals_: Tk, right_: NodeId, nest: bool) {
        self.space();
        self.token(equals_);
        if nest {
            self.nest_expression(None, true);
        }
        let c = self.assignment_cost(right_);
        self.solo_split(c);
        self.start_span(cost::NORMAL);
        self.visit(Some(right_));
        self.end_span();
        if nest {
            self.unnest();
        }
    }

    /// `_assignmentCost(rightHandSide)`.
    fn assignment_cost(&self, right_: NodeId) -> i32 {
        match self.node(right_) {
            Node::ListLiteral { .. } | Node::SetOrMapLiteral { .. } | Node::CascadeExpression { .. } => cost::ASSIGN_BLOCK,
            _ => cost::ASSIGN,
        }
    }

    /// `_visitBinary(node, destructure, precedence:, nest:)`.
    fn visit_binary(&mut self, id: NodeId, target_precedence: Option<u8>, nest: bool) {
        self.start_span(cost::NORMAL);
        if nest {
            self.nest_expression(None, false);
        }
        self.start_lazy_rule(None);
        self.start_block_argument_nesting();
        let is_pattern_and = matches!(self.node(id), Node::LogicalAndPattern { .. });
        let is_pattern_or = matches!(self.node(id), Node::LogicalOrPattern { .. });
        self.walk_binary(id, target_precedence, is_pattern_and, is_pattern_or);
        self.end_block_argument_nesting();
        if nest {
            self.unnest();
        }
        self.end_span();
        self.end_rule();
    }

    fn walk_binary(&mut self, e: NodeId, prec: Option<u8>, pattern_and: bool, pattern_or: bool) {
        let parts_ = match self.node(e) {
            Node::BinaryExpression {
                left_operand,
                operator,
                right_operand,
            } if !pattern_and && !pattern_or => Some((*left_operand, *operator, *right_operand)),
            Node::LogicalAndPattern {
                left_operand,
                operator,
                right_operand,
            } if pattern_and => Some((*left_operand, *operator, *right_operand)),
            Node::LogicalOrPattern {
                left_operand,
                operator,
                right_operand,
            } if pattern_or => Some((*left_operand, *operator, *right_operand)),
            _ => None,
        };
        let Some((left, op, dir)) = parts_ else {
            self.visit(Some(e));
            return;
        };
        if let Some(p) = prec
            && precedence(self.lexeme(op)) != p
        {
            self.visit(Some(e));
            return;
        }
        self.walk_binary(left, prec, pattern_and, pattern_or);
        self.space();
        self.token(op);
        self.split();
        self.walk_binary(dir, prec, pattern_and, pattern_or);
    }

    fn visit_if_element(&mut self, id: NodeId) {
        let mut elements_ = Vec::new();
        let mut current = Some(id);
        while let Some(n) = current {
            if let Node::IfElement { else_element, .. } = self.node(n) {
                elements_.push(n);
                current = *else_element;
            } else {
                break;
            }
        }
        let spread_rule = self.a.rule(cost::NORMAL);
        let mut spreads: Vec<NodeId> = Vec::new();
        for &el in &elements_ {
            let Node::IfElement { then_element, .. } = self.node(el) else {
                unreachable!()
            };
            if let Some(bracket) = self.spread_collection_bracket(*then_element) {
                spreads.push(el);
                self.before_block(bracket, spread_rule, None);
            }
        }
        let ultimo = *elements_.last().expect("if");
        let ultimo_else = match self.node(ultimo) {
            Node::IfElement { else_element, .. } => *else_element,
            _ => None,
        };
        if let Some(le) = ultimo_else
            && let Some(bracket) = self.spread_collection_bracket(le)
        {
            spreads.push(le);
            self.before_block(bracket, spread_rule, None);
        }

        let visit_child = |v: &mut Self, element: NodeId, child: NodeId, spreads: &[NodeId]| {
            v.nest_expression(Some(2), true);
            let is_spread = spreads.contains(&element);
            if is_spread {
                v.space();
            } else {
                v.split();
                v.start_block_argument_nesting();
            }
            v.visit(Some(child));
            if !is_spread {
                v.end_block_argument_nesting();
            }
            v.unnest();
        };

        self.start_lazy_rule(None);
        let mut inner_flow = false;
        for &el in &elements_ {
            let Node::IfElement {
                if_keyword,
                left_parenthesis,
                expression,
                case_clause,
                right_parenthesis,
                then_element,
                else_keyword,
                else_element,
            } = self.node(el)
            else {
                unreachable!()
            };
            self.visit_if_condition(*if_keyword, *left_parenthesis, *expression, *case_clause, *right_parenthesis);
            visit_child(self, el, *then_element, &spreads);
            if self.is_control_flow_element(*then_element) {
                inner_flow = true;
            }
            if else_element.is_some() {
                if spreads.contains(&el) {
                    self.space();
                } else {
                    self.split();
                }
                self.token_opt(*else_keyword);
                if el != ultimo {
                    self.space();
                }
            }
        }
        if let Some(le) = ultimo_else {
            visit_child(self, le, le, &spreads);
            if self.is_control_flow_element(le) {
                inner_flow = true;
            }
        }
        if inner_flow {
            self.force_rules();
        }
        self.end_rule();
    }

    fn visit_for_element(&mut self, id: NodeId) {
        let Node::ForElement {
            await_keyword,
            for_keyword,
            left_parenthesis,
            for_loop_parts,
            right_parenthesis,
            body,
        } = self.node(id)
        else {
            unreachable!()
        };
        let spread_body = self.spread_collection_bracket(*body).is_some();
        self.nest_expression(None, false);
        self.modifier(*await_keyword);
        self.token(*for_keyword);
        self.space();
        self.token(*left_parenthesis);
        self.start_rule(None);
        self.start_rule(None);
        self.visit(Some(*for_loop_parts));
        self.token(*right_parenthesis);
        self.end_rule();
        self.unnest();
        self.nest_expression(Some(recuo::BLOCK), true);
        if spread_body {
            self.space();
        } else {
            self.split();
            self.start_block_argument_nesting();
        }
        self.visit(Some(*body));
        if !spread_body {
            self.end_block_argument_nesting();
        }
        self.unnest();
        if self.is_control_flow_element(*body) {
            self.force_rules();
        }
        self.end_rule();
    }

    // ============================================================= coleções

    /// `beforeBlock(token, rule, [previousChunk])`.
    pub fn before_block(&mut self, t: Tk, rule: RuleId, anterior: Option<ChunkId>) {
        self.block_rules.insert(t, rule);
        if let Some(c) = anterior {
            self.block_chunks.insert(t, c);
        }
    }

    /// `_visitCollectionLiteral`.
    #[allow(clippy::too_many_arguments)]
    pub fn visit_collection_literal(
        &mut self,
        left_side: Tk,
        elements_: &[NodeId],
        right_: Tk,
        const_keyword: Option<Tk>,
        type_arguments: Option<NodeId>,
        cost_opt: Option<i32>,
        split_outer: bool,
        is_record: bool,
    ) {
        let _ = cost_opt;
        self.modifier(const_keyword);
        if let Some(ta) = type_arguments
            && let Node::TypeArgumentList {
                left_bracket,
                arguments,
                right_bracket,
            } = self.node(ta)
        {
            self.start_span(cost::NORMAL);
            self.nest_expression(None, false);
            self.token(*left_bracket);
            let r = self.a.rule(cost::TYPE_ARGUMENT);
            self.start_rule(Some(r));
            for (i, &arg) in arguments.iter().enumerate() {
                self.visit(Some(arg));
                if i + 1 != arguments.len() {
                    let mut virgula = self.tree_.end(arg) + 1;
                    if self.lexeme(virgula) == "?" {
                        virgula += 1;
                    }
                    self.token(virgula);
                    self.split();
                }
            }
            self.token(*right_bracket);
            self.end_rule();
            self.unnest();
            self.end_span();
        }
        if elements_.is_empty() {
            self.visit_body(left_side, elements_, right_);
            return;
        }
        if split_outer {
            for c in self.collections_.iter_mut() {
                *c = true;
            }
            self.collections_.push(false);
        }
        self.begin_body(left_side, false);
        if self.contains_line_comments(elements_, Some(right_)) {
            let mut line_rule = self.a.type_argument_rule();
            self.start_lazy_rule(Some(line_rule));
            for &el in elements_ {
                let start = self.tree_.start(el);
                if self.end_line(start - 1) != self.start_line(start) {
                    self.one_or_two_newlines();
                    self.end_rule();
                    line_rule = self.a.type_argument_rule();
                    self.start_lazy_rule(Some(line_rule));
                } else {
                    let c = self.split();
                    self.a.before_argument(line_rule, Some(c));
                }
                self.visit(Some(el));
                self.write_comma_after(el);
            }
            self.end_rule();
        } else {
            for (i, &el) in elements_.iter().enumerate() {
                self.builder_split(false, i != 0);
                self.visit(Some(el));
                self.write_comma_after(el);
            }
        }
        let mut force = false;
        if split_outer {
            force = self.collections_.pop().expect("coleção");
        }
        let single_record = is_record && elements_.len() == 1;
        if self.list_has_comma_after(elements_) && !single_record {
            force = true;
        }
        self.end_body(right_, force);
    }

    /// `_beginBody(leftBracket, space:)`.
    fn begin_body(&mut self, left_side: Tk, space: bool) {
        self.token(left_side);
        let rule = self.block_rules.get(&left_side).copied();
        self.start_rule(rule);
        let anterior = self.block_chunks.get(&left_side).copied();
        self.start_block(anterior, true, space);
    }

    /// `_endBody(rightBracket, forceSplit:)`.
    fn end_body(&mut self, right_: Tk, force: bool) {
        let newline = self.write_preceding_comments_and_newlines(right_);
        self.end_block(newline || force);
        self.end_rule();
        let t = self.lexeme(right_);
        self.write_text(t, right_);
    }

    /// `_visitBody(leftBracket, nodes, rightBracket)`.
    fn visit_body(&mut self, left_side: Tk, nodes: &[NodeId], right_: Tk) {
        if !self.can_split(nodes, right_) {
            self.token(left_side);
            self.token(right_);
            return;
        }
        self.begin_body(left_side, false);
        self.visit_body_contents(nodes);
        self.end_body(right_, !nodes.is_empty());
    }

    /// `_visitBodyContents(nodes)`.
    fn visit_body_contents(&mut self, nodes: &[NodeId]) {
        for (i, &n) in nodes.iter().enumerate() {
            self.visit(Some(n));
            if i + 1 != nodes.len() {
                if self.has_non_empty_body(n) {
                    self.two_newlines();
                } else {
                    self.one_or_two_newlines();
                }
            }
        }
    }

    // =============================================================== strings

    /// `_writeStringLiteral(string)`.
    fn write_string_literal(&mut self, t: Tk) {
        self.write_preceding_comments_and_newlines(t);
        let lexeme = self.lexeme(t);
        let lines_ = split_lines(lexeme);
        if lines_.len() > 1 {
            self.write_string_first_line(lines_[0], t);
        } else {
            self.write_text(lines_[0], t);
        }
        for line in &lines_[1..] {
            self.write_newline(false, true, true);
            self.write_text_merge(line, t, false);
        }
    }

    /// `_writeStringFirstLine(line, string)`.
    fn write_string_first_line(&mut self, line: &str, t: Tk) {
        let b = line.as_bytes();
        let quote_start = usize::from(line.starts_with('r'));
        let quote_end = quote_start + 3;
        let mut barras = 0;
        if b.len() > quote_end && (line[quote_start..].starts_with("'''") || line[quote_start..].starts_with("\"\"\"")) {
            let mut cursor = quote_end;
            loop {
                let mut c = b[cursor];
                if c == b'\\' {
                    cursor += 1;
                    barras += 1;
                    if cursor >= b.len() {
                        break;
                    }
                    c = b[cursor];
                }
                if c != b' ' && c != b'\t' {
                    break;
                }
                cursor += 1;
                if cursor >= b.len() {
                    break;
                }
            }
            if cursor == b.len() {
                let mut texto = line[..quote_end].to_string();
                if barras > 0 {
                    texto.push('\\');
                    barras -= 1;
                    while barras > 0 {
                        texto.push_str(" \\");
                        barras -= 1;
                    }
                }
                self.write_text(&texto, t);
                return;
            }
        }
        self.write_text(line, t);
    }
}

/// Divide um lexema nas quebras `\r\n`, `\r` e `\n` (`_lineTerminatorRE`).
fn split_lines(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut v = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            v.push(&s[start..i]);
            if b.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            start = i + 1;
        } else if b[i] == b'\n' {
            v.push(&s[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    v.push(&s[start..]);
    v
}

/// `_looksLikeClassName(name)`.
pub fn looks_like_class_name(nome: &str) -> bool {
    if matches!(nome, "bool" | "double" | "int" | "num") {
        return true;
    }
    let b = nome.as_bytes();
    let mut i = 0;
    let Some(&first) = b.first() else {
        return false;
    };
    let mut c = first;
    i += 1;
    if c == b'_' {
        if b.len() == 1 {
            return false;
        }
        c = b[i];
        i += 1;
    }
    if !c.is_ascii_uppercase() {
        return false;
    }
    b[i..].iter().any(|c| c.is_ascii_lowercase())
}

/// Precedência de um operador binário (`TokenType.precedence`).
pub fn precedence(op: &str) -> u8 {
    match op {
        "??" => 4,
        "||" => 5,
        "&&" => 6,
        "==" | "!=" => 7,
        "<" | ">" | "<=" | ">=" => 8,
        "|" => 9,
        "^" => 10,
        "&" => 11,
        "<<" | ">>" | ">>>" => 12,
        "+" | "-" => 13,
        "*" | "/" | "~/" | "%" => 14,
        _ => 0,
    }
}

#[allow(dead_code)]
fn _uses(k: Keyword) -> Keyword {
    k
}
