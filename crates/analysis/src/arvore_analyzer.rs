//! The tree in analyzer 6.11's shape (the merely parsed unit) and the
//! `NodeLocator` (`AN:src/dart/ast/utilities.dart:1733-1870`), built over the
//! DartForge parser's tree and the text's tokens
//! (docs/LSP-ESPECIFICACAO.md §7.2 to §7.6).
//!
//! Each node has the analyzer's range (`beginToken.offset` to `endToken.end`)
//! and the children in `visitChildren` order (the one that decides "the first
//! that covers"). Declared names are tokens, not nodes; the references and the
//! identifiers the analyzer keeps as nodes (a constructor's `returnType`, the
//! import prefix, `show`/`hide` names, an annotation's name, labels) are
//! `SimpleIdentifier`. The nodes the parser's tree lacks (`FormalParameterList`,
//! `VariableDeclarationList`, `TypeArgumentList`, function bodies, clauses,
//! `for` parts, collection elements, combinators, `Comment`…) are synthesized
//! from the tokens. The shape is the unresolved one: `A()` is a
//! `MethodInvocation`. Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Annotation, Arguments, Ast, AsyncModifier, CollectionElement, Combinator, CompilationUnit, DeclKind, DirectiveKind, ExprId,
    ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, ListPatternElement, MemberKind, Parameter, ParameterKind, PatternId,
    PatternKind, StmtId, StmtKind, StringLit, TypeId, TypeKind, TypeParameter, UnaryOp,
};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_frontend::token::{Kind, Op, Token};
use std::collections::HashMap;

/// Where the node came from in the parser's tree (to find its element).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marca {
    Nenhuma,
    /// A `SimpleIdentifier` whose element is the one resolved in this
    /// expression (the identifier itself, the property it names, the target of
    /// the invocation whose `methodName` it is).
    Expr(ExprId),
    /// The `FunctionDeclaration`, `MethodDeclaration` or `FunctionExpression`
    /// of this function.
    Funcao(ast::FunctionId),
    /// The `VariableDeclaration` of a local variable.
    VariavelLocal,
    /// A top-level declaration.
    Decl(ast::DeclId),
    /// A `SimpleIdentifier` in a declaration context (`inDeclarationContext`:
    /// the `as p` prefix and a statement's label).
    ContextoDeDeclaracao,
    /// The `PrefixExpression` of this expression (the `staticElement` is the operator).
    Operador(ExprId),
}

/// The DartForge AST entity the node represents (completion reads its parts).
/// Those without an id in the parser's tree go by address (only compared,
/// never followed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ligacao {
    Nenhuma,
    Expr(ExprId),
    Stmt(StmtId),
    Decl(ast::DeclId),
    Membro(ast::MemberId),
    Funcao(ast::FunctionId),
    Tipo(TypeId),
    Padrao(PatternId),
    /// `ast::Parameter`.
    Parametro(usize),
    /// `ast::Initializer`.
    Inicializador(usize),
    /// `ast::EnumConstant`.
    ConstanteDeEnum(usize),
    /// `ast::SwitchCase`.
    Caso(usize),
    /// `ast::Directive`.
    Diretiva(usize),
    /// `ast::CatchClause`.
    Catch(usize),
    /// `ast::Arguments`.
    Argumentos(usize),
    /// `ast::CollectionElement`.
    Elemento(usize),
    /// `ast::Combinator`.
    Combinador(usize),
    /// `ast::Annotation`.
    Anotacao(usize),
    /// `ast::TypeParameter`.
    ParametroDeTipo(usize),
    /// The `SwitchExpressionCase` with index `.1` of the `switch` expression `.0`.
    CasoDeSwitchExpr(ExprId, usize),
}

/// The address of an AST entity, for the [`Ligacao`].
pub fn endereco<T>(x: &T) -> usize {
    x as *const T as usize
}

/// An analyzer node.
#[derive(Debug, Clone)]
pub struct No {
    pub especie: &'static str,
    pub inicio: usize,
    pub fim: usize,
    pub filhos: Vec<usize>,
    pub pai: Option<usize>,
    /// The position of the end of the name that the `NodeLocator` overrides
    /// (class, function, method, constructor).
    pub sobrescrita: Option<usize>,
    pub marca: Marca,
    pub ligacao: Ligacao,
}

/// The parts of a `ForParts` (`initialization`/`variables`, `condition`,
/// `updaters`), by node.
#[derive(Debug, Clone, Default)]
pub struct PartesDeFor {
    pub inicio: Option<usize>,
    pub condicao: Option<usize>,
    pub atualizacoes: Vec<usize>,
}

/// The tree; node 0 is the `CompilationUnit`.
pub struct Arvore {
    pub nos: Vec<No>,
    /// The parts of each `ForPartsWith*`, by node.
    pub partes_de_for: HashMap<usize, PartesDeFor>,
}

impl Arvore {
    /// `NodeLocator(ini, fim).searchWithin(unit)`.
    pub fn localizar(&self, start: usize, end: usize) -> Option<usize> {
        let mut found = None;
        self.visit(0, start, end, &mut found);
        found
    }

    fn visit(&self, n: usize, start: usize, end: usize, found: &mut Option<usize>) {
        if found.is_some() {
            return;
        }
        let node = &self.nos[n];
        if let Some(s) = node.sobrescrita
            && start == end
            && start == s
        {
            *found = Some(n);
            return;
        }
        if node.fim < start || node.inicio > end {
            return;
        }
        for &f in &node.filhos {
            self.visit(f, start, end, found);
            if found.is_some() {
                return;
            }
        }
        if node.inicio <= start && end <= node.fim {
            *found = Some(n);
        }
    }

    /// `NodeLocator2` (exclusive end): the deepest node with
    /// `start <= pos < end`, the first child that covers, with the `name.end`
    /// overrides.
    pub fn localizar_exclusivo(&self, pos: usize) -> Option<usize> {
        let mut found = None;
        self.visit_exclusive(0, pos, &mut found);
        found
    }

    fn visit_exclusive(&self, n: usize, pos: usize, found: &mut Option<usize>) {
        if found.is_some() {
            return;
        }
        let node = &self.nos[n];
        if let Some(s) = node.sobrescrita
            && pos == s
        {
            *found = Some(n);
            return;
        }
        if node.fim <= pos || node.inicio > pos {
            return;
        }
        for &f in &node.filhos {
            self.visit_exclusive(f, pos, found);
            if found.is_some() {
                return;
            }
        }
        if node.inicio <= pos && pos < node.fim {
            *found = Some(n);
        }
    }

    /// `NodeLocator2(start, end)`: the deepest node with `start <= ini` and
    /// `end < node.end` (the fixes' `coveringNode`).
    pub fn localizar2(&self, start: usize, end: usize) -> Option<usize> {
        let mut found = None;
        self.visit2(0, start, end, &mut found);
        found
    }

    fn visit2(&self, n: usize, start: usize, end: usize, found: &mut Option<usize>) {
        if found.is_some() {
            return;
        }
        let node = &self.nos[n];
        if node.fim <= start || node.inicio > end {
            return;
        }
        for &f in &node.filhos {
            self.visit2(f, start, end, found);
            if found.is_some() {
                return;
            }
        }
        if node.inicio <= start && end < node.fim {
            *found = Some(n);
        }
    }

    /// The node and its ancestors, without the `CompilationUnit`.
    pub fn cadeia(&self, n: usize) -> Vec<usize> {
        let mut v = Vec::new();
        let mut current = Some(n);
        while let Some(k) = current {
            if k == 0 {
                break;
            }
            v.push(k);
            current = self.nos[k].pai;
        }
        v
    }

    pub fn span(&self, n: usize) -> Span {
        Span { start: self.nos[n].inicio, end: self.nos[n].fim }
    }
}

/// The text's tokens, with the pair of each `( ) [ ] { }`.
struct Tokens {
    list: Vec<Token>,
    pairs: HashMap<usize, usize>,
}

impl Tokens {
    fn new(source: &str) -> Tokens {
        let list = dartforge_frontend::lexer::lex(source).unwrap_or_default();
        let mut pairs = HashMap::new();
        let mut stack: Vec<(Op, usize)> = Vec::new();
        for (i, t) in list.iter().enumerate() {
            if let Kind::Op(op) = t.kind {
                match op {
                    Op::LParen | Op::LBracket | Op::LBrace => stack.push((op, i)),
                    Op::RParen | Op::RBracket | Op::RBrace => {
                        let open = match op {
                            Op::RParen => Op::LParen,
                            Op::RBracket => Op::LBracket,
                            _ => Op::LBrace,
                        };
                        if let Some(pos) = stack.iter().rposition(|(o, _)| *o == open) {
                            let (_, j) = stack[pos];
                            stack.truncate(pos);
                            pairs.insert(j, i);
                            pairs.insert(i, j);
                        }
                    }
                    _ => {}
                }
            }
        }
        Tokens { list, pairs }
    }

    /// The first token starting at `off` or later.
    fn index(&self, off: usize) -> usize {
        self.list.partition_point(|t| t.span.start < off)
    }

    /// The last token ending at or before `off`.
    fn before(&self, off: usize) -> Option<usize> {
        let k = self.list.partition_point(|t| t.span.end <= off);
        k.checked_sub(1)
    }

    /// The first token starting at `off` or later (except EOF).
    fn after(&self, off: usize) -> Option<usize> {
        let i = self.index(off);
        self.list.get(i).filter(|t| t.kind != Kind::Eof).map(|_| i)
    }

    fn span(&self, i: usize) -> Span {
        self.list[i].span
    }

    fn is_op(&self, i: usize, op: Op) -> bool {
        self.list.get(i).is_some_and(|t| t.kind == Kind::Op(op))
    }

    fn is_word(&self, i: usize, source: &str, word: &str) -> bool {
        self.list.get(i).is_some_and(|t| matches!(t.kind, Kind::Ident | Kind::Keyword(_)) && t.text(source) == word)
    }

    /// The token immediately before `off`, if it is `op`.
    fn op_before(&self, off: usize, op: Op) -> Option<usize> {
        self.before(off).filter(|&i| self.is_op(i, op))
    }

    /// The token immediately after `off`, if it is `op`.
    fn op_after(&self, off: usize, op: Op) -> Option<usize> {
        self.after(off).filter(|&i| self.is_op(i, op))
    }

    /// The last `op` token ending at or before `until` and starting at `from` or later.
    fn last_op(&self, op: Op, from: usize, until: usize) -> Option<usize> {
        let i0 = self.index(from);
        let i1 = self.list.partition_point(|t| t.span.end <= until);
        (i0..i1).rev().find(|&i| self.is_op(i, op))
    }

    /// The first `op` token in `[from, until)`.
    fn first_op(&self, op: Op, from: usize, until: usize) -> Option<usize> {
        let mut i = self.index(from);
        while let Some(t) = self.list.get(i) {
            if t.span.start >= until || t.kind == Kind::Eof {
                return None;
            }
            if t.kind == Kind::Op(op) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// The pair of token `i`.
    fn pair_of(&self, i: usize) -> Option<usize> {
        self.pairs.get(&i).copied()
    }
}

struct Builder<'a> {
    a: &'a Ast,
    source: &'a str,
    t: Tokens,
    comments: Comentarios,
    nos: Vec<No>,
    /// The language version is before 3.0 (`case e:` is a `SwitchCase`).
    before_3_0: bool,
    /// The metadata of the statements (local variables and functions).
    local_metadata: HashMap<u32, &'a [Annotation]>,
    /// The resolved shape (the refactorings'): the calls without `new` that
    /// invoke a constructor (`A()`, `A.n()`, `p.A()`, `p.A.n()`) become
    /// `InstanceCreationExpression`, as in `AstRewriter.methodInvocation`.
    constructor_calls: Option<&'a std::collections::HashSet<ExprId>>,
    /// The identifiers that are an import prefix (resolved shape).
    prefix_idents: Option<&'a std::collections::HashSet<ExprId>>,
    partes_de_for: HashMap<usize, PartesDeFor>,
}

/// Builds the tree of the already parsed text.
pub fn construir(source: &str, a: &Ast, unit: &CompilationUnit, before_3_0: bool) -> Arvore {
    build_with(source, a, unit, before_3_0, None, None)
}

/// The tree in the resolved shape: `construtores` are the calls (`Call` or
/// `InstanceCreation` without a keyword) that invoke a constructor, and
/// `prefixos` the identifiers that denote an import prefix.
pub fn construir_resolvida(
    source: &str,
    a: &Ast,
    unit: &CompilationUnit,
    before_3_0: bool,
    constructor_calls: &std::collections::HashSet<ExprId>,
    prefix_idents: &std::collections::HashSet<ExprId>,
) -> Arvore {
    build_with(source, a, unit, before_3_0, Some(constructor_calls), Some(prefix_idents))
}

fn build_with<'a>(
    source: &'a str,
    a: &'a Ast,
    unit: &'a CompilationUnit,
    before_3_0: bool,
    constructor_calls: Option<&'a std::collections::HashSet<ExprId>>,
    prefix_idents: Option<&'a std::collections::HashSet<ExprId>>,
) -> Arvore {
    let mut c = Builder {
        a,
        source,
        t: Tokens::new(source),
        comments: Comentarios::de(source),
        nos: Vec::new(),
        before_3_0,
        local_metadata: a.metadados_locais.iter().map(|(s, m)| (s.0, &m[..])).collect(),
        constructor_calls,
        prefix_idents,
        partes_de_for: HashMap::new(),
    };
    // The root first (index 0), the children afterwards.
    c.nos.push(No { especie: "CompilationUnit", inicio: 0, fim: source.len(), filhos: Vec::new(), pai: None, sobrescrita: None, marca: Marca::Nenhuma, ligacao: Ligacao::Nenhuma });
    let mut children = Vec::new();
    if let Some(s) = unit.script_tag {
        children.extend(c.leaf("ScriptTag", s));
    }
    // Directives and declarations in source order.
    let mut items: Vec<(usize, bool, usize)> = Vec::new();
    for (i, d) in unit.directives.iter().enumerate() {
        items.push((d.span.start, true, i));
    }
    for (i, d) in unit.declarations.iter().enumerate() {
        items.push((a.decl(*d).span.start, false, i));
    }
    items.sort();
    for (_, directive, i) in items {
        let n = if directive { c.directive(&unit.directives[i]) } else { c.decl(unit.declarations[i]) };
        children.extend(n);
    }
    c.link(0, children);
    Arvore { nos: c.nos, partes_de_for: c.partes_de_for }
}

impl<'a> Builder<'a> {
    // -- Infraestrutura --------------------------------------------------------

    /// A node with the given children; never an empty node.
    fn node(&mut self, kind: &'static str, start: usize, end: usize, children: Vec<usize>) -> Option<usize> {
        if end <= start {
            return None;
        }
        let id = self.nos.len();
        self.nos.push(No { especie: kind, inicio: start, fim: end, filhos: Vec::new(), pai: None, sobrescrita: None, marca: Marca::Nenhuma, ligacao: Ligacao::Nenhuma });
        self.link(id, children);
        Some(id)
    }

    fn link(&mut self, parent: usize, children: Vec<usize>) {
        for &f in &children {
            self.nos[f].pai = Some(parent);
        }
        self.nos[parent].filhos = children;
    }

    fn leaf(&mut self, kind: &'static str, s: Span) -> Option<usize> {
        self.node(kind, s.start, s.end, Vec::new())
    }

    fn identifier(&mut self, n: ast::Name) -> Option<usize> {
        // The recovery's synthetic identifier (`b.` before `}`) stays in the
        // tree with length zero, as in the analyzer.
        if n.span.start == n.span.end {
            let id = self.nos.len();
            self.nos.push(No {
                especie: "SimpleIdentifier",
                inicio: n.span.start,
                fim: n.span.end,
                filhos: Vec::new(),
                pai: None,
                sobrescrita: None,
                marca: Marca::Nenhuma,
                ligacao: Ligacao::Nenhuma,
            });
            return Some(id);
        }
        self.leaf("SimpleIdentifier", n.span)
    }

    fn bind_to(&mut self, n: Option<usize>, l: Ligacao) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].ligacao = l;
        }
        n
    }

    fn mark(&mut self, n: Option<usize>, m: Marca) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].marca = m;
        }
        n
    }

    fn with_override(&mut self, n: Option<usize>, pos: usize) -> Option<usize> {
        if let Some(k) = n {
            self.nos[k].sobrescrita = Some(pos);
        }
        n
    }

    // -- Documentation comment and annotations ----------------------------------

    /// The documentation `Comment` before `pos` (`findDartDoc`): the `/**`, or
    /// the `///`s of the chain from the first one (the ordinary comments in
    /// between do not count: `DocCommentBuilder.build`), with the chain's
    /// tokens from it on.
    fn doc(&mut self, pos: usize) -> Option<(usize, usize, Vec<Span>)> {
        let start = self.comments.dart_doc(self.source, pos)?;
        let cadeia: Vec<Span> = self.comments.antes_de(self.source, pos).into_iter().filter(|s| s.start >= start.start).collect();
        let text = &self.source[start.start..start.end];
        let mut end = start.end;
        if text.starts_with("///") {
            for s in &cadeia {
                if self.source[s.start..s.end].starts_with("///") {
                    end = s.end;
                }
            }
        }
        Some((start.start, end, cadeia))
    }

    /// The `Comment` node with the `CommentReference`s (the `DocCommentBuilder`).
    fn comment_node(&mut self, start: usize, end: usize, cadeia: &[Span]) -> Option<usize> {
        let mut refs = Vec::new();
        for r in dartforge_frontend::doc_referencias::referencias(self.source, cadeia) {
            let names: Vec<Span> = r.partes.iter().map(|(s, _)| *s).collect();
            let (Some(first), Some(last)) = (names.first().copied(), names.last().copied()) else { continue };
            // The `CommentReference` goes from the `new` (when present) to the end of the name.
            let a = if r.novo {
                self.source[..first.start].trim_end().strip_suffix("new").map_or(first.start, str::len)
            } else {
                first.start
            };
            let b = last.end;
            let expr = match names.as_slice() {
                [x] => self.leaf("SimpleIdentifier", *x),
                [x, y] => {
                    let fx = self.leaf("SimpleIdentifier", *x);
                    let fy = self.leaf("SimpleIdentifier", *y);
                    self.node("PrefixedIdentifier", x.start, y.end, fx.into_iter().chain(fy).collect())
                }
                [x, y, z] => {
                    let fx = self.leaf("SimpleIdentifier", *x);
                    let fy = self.leaf("SimpleIdentifier", *y);
                    let p = self.node("PrefixedIdentifier", x.start, y.end, fx.into_iter().chain(fy).collect());
                    let fz = self.leaf("SimpleIdentifier", *z);
                    self.node("PropertyAccess", x.start, z.end, p.into_iter().chain(fz).collect())
                }
                _ => None,
            };
            refs.extend(self.node("CommentReference", a, b, expr.into_iter().collect()));
        }
        self.node("Comment", start, end, refs)
    }

    /// The `[doc/annot]` children (in offset order) and the node's start: the
    /// `AstBuilder`'s `_findComment` (before the token after the annotations;
    /// otherwise before each annotation, from the last to the first).
    fn doc_and_annotations(&mut self, start: usize, metadata: &'a [Annotation]) -> (usize, Vec<usize>) {
        // The token after the annotations (the declaration's).
        let apos = match metadata.last() {
            Some(m) => self.t.after(m.span.end).map_or(start, |i| self.t.span(i).start),
            None => start,
        };
        let mut doc = self.doc(apos);
        if doc.is_none() {
            for m in metadata.iter().rev() {
                doc = self.doc(m.span.start);
                if doc.is_some() {
                    break;
                }
            }
        }
        let mut comeco = start;
        if let Some(m) = metadata.first() {
            comeco = comeco.min(m.span.start);
        }
        let mut items: Vec<(usize, Option<usize>)> = Vec::new();
        if let Some((a, b, cadeia)) = doc {
            comeco = comeco.min(a);
            items.push((a, self.comment_node(a, b, &cadeia)));
        }
        for m in metadata {
            items.push((m.span.start, self.annotation(m)));
        }
        items.sort_by_key(|(p, _)| *p);
        (comeco, items.into_iter().filter_map(|(_, n)| n).collect())
    }

    /// `Annotation`: `@` → arguments ?? constructor name ?? name.
    fn annotation(&mut self, m: &'a Annotation) -> Option<usize> {
        let n = self.annotation_raw(m);
        self.bind_to(n, Ligacao::Anotacao(endereco(m)))
    }

    fn annotation_raw(&mut self, m: &'a Annotation) -> Option<usize> {
        let mut children = Vec::new();
        // `@A<T>.b(…)`: the name is `A` and the constructor `b`; `@p.A<T>(…)`:
        // the name is `p.A`.
        let types_before_second = m.name.len() == 2 && m.type_args.first().is_some_and(|t| self.a.ty(*t).span.start < m.name[1].span.start);
        let (name, constructor): (&[ast::Name], Option<ast::Name>) = match m.name.len() {
            3 => (&m.name[..2], Some(m.name[2])),
            2 if types_before_second => (&m.name[..1], Some(m.name[1])),
            _ => (&m.name[..], None),
        };
        match name {
            [x] => children.extend(self.identifier(*x)),
            [x, y] => {
                let fx = self.identifier(*x);
                let fy = self.identifier(*y);
                children.extend(self.node("PrefixedIdentifier", x.span.start, y.span.end, fx.into_iter().chain(fy).collect()));
            }
            _ => {}
        }
        children.extend(self.type_args(&m.type_args));
        if let Some(k) = constructor {
            children.extend(self.identifier(k));
        }
        if let Some(args) = &m.arguments {
            children.extend(self.arg_list(args));
        }
        self.node("Annotation", m.span.start, m.span.end, children)
    }

    // -- Diretivas -------------------------------------------------------------

    fn uri_literal(&mut self, s: &'a StringLit) -> Option<usize> {
        self.string(s)
    }

    fn dotted_names(&mut self, kind: &'static str, names: &[ast::Name]) -> Option<usize> {
        let (Some(p), Some(u)) = (names.first(), names.last()) else { return None };
        let children: Vec<usize> = names.iter().filter_map(|n| self.identifier(*n)).collect();
        self.node(kind, p.span.start, u.span.end, children)
    }

    fn configuration(&mut self, c: &'a ast::Configuration) -> Option<usize> {
        let mut children = Vec::new();
        children.extend(self.dotted_names("DottedName", &c.test));
        if let Some(v) = &c.value {
            children.extend(self.string(v));
        }
        children.extend(self.string(&c.uri));
        self.node("Configuration", c.span.start, c.span.end, children)
    }

    fn combinator(&mut self, c: &'a Combinator) -> Option<usize> {
        let n = self.combinator_raw(c);
        self.bind_to(n, Ligacao::Combinador(endereco(c)))
    }

    fn combinator_raw(&mut self, c: &'a Combinator) -> Option<usize> {
        let (kind, names) = match c {
            Combinator::Show(n) => ("ShowCombinator", n),
            Combinator::Hide(n) => ("HideCombinator", n),
        };
        let (Some(p), Some(u)) = (names.first(), names.last()) else { return None };
        let word = self.t.before(p.span.start).map_or(p.span.start, |i| self.t.span(i).start);
        let children: Vec<usize> = names.iter().filter_map(|n| self.identifier(*n)).collect();
        self.node(kind, word, u.span.end, children)
    }

    fn directive(&mut self, d: &'a ast::Directive) -> Option<usize> {
        let n = self.directive_raw(d);
        self.bind_to(n, Ligacao::Diretiva(endereco(d)))
    }

    fn directive_raw(&mut self, d: &'a ast::Directive) -> Option<usize> {
        let (start, doc) = self.doc_and_annotations(d.span.start, &d.metadata);
        let mut children = Vec::new();
        let kind = match &d.kind {
            DirectiveKind::Library { name } => {
                children.extend(doc);
                children.extend(self.dotted_names("LibraryIdentifier", name));
                "LibraryDirective"
            }
            DirectiveKind::Import { uri, configurations, prefix, combinators, .. } => {
                children.extend(doc);
                children.extend(self.uri_literal(uri));
                for c in configurations {
                    children.extend(self.configuration(c));
                }
                if let Some(p) = prefix {
                    let id = self.identifier(*p);
                    children.extend(self.mark(id, Marca::ContextoDeDeclaracao));
                }
                for c in combinators {
                    children.extend(self.combinator(c));
                }
                "ImportDirective"
            }
            DirectiveKind::Export { uri, configurations, combinators } => {
                for c in configurations {
                    children.extend(self.configuration(c));
                }
                children.extend(doc);
                children.extend(self.uri_literal(uri));
                for c in combinators {
                    children.extend(self.combinator(c));
                }
                "ExportDirective"
            }
            DirectiveKind::Part { uri } => {
                children.extend(doc);
                children.extend(self.uri_literal(uri));
                "PartDirective"
            }
            DirectiveKind::PartOf { uri, name } => {
                children.extend(doc);
                children.extend(self.dotted_names("LibraryIdentifier", name));
                if let Some(u) = uri {
                    children.extend(self.uri_literal(u));
                }
                "PartOfDirective"
            }
            DirectiveKind::ImportAugment { uri } | DirectiveKind::AugmentLibrary { uri } => {
                children.extend(doc);
                children.extend(self.uri_literal(uri));
                "AugmentationImportDirective"
            }
        };
        self.node(kind, start, d.span.end, children)
    }

    // -- Declarations ------------------------------------------------------------

    /// The keyword token before the first type of a clause.
    fn clause(&mut self, kind: &'static str, types: &[TypeId]) -> Option<usize> {
        let (Some(p), Some(u)) = (types.first(), types.last()) else { return None };
        let ps = self.a.ty(*p).span;
        let us = self.a.ty(*u).span;
        let word = self.t.before(ps.start).map_or(ps.start, |i| self.t.span(i).start);
        let children: Vec<usize> = types.iter().filter_map(|t| self.ty(*t)).collect();
        self.node(kind, word, us.end, children)
    }

    fn decl(&mut self, did: ast::DeclId) -> Option<usize> {
        let n = self.decl_unmarked(did);
        let n = self.bind_to(n, Ligacao::Decl(did));
        // The top-level function keeps the function's mark.
        match n {
            Some(k) if self.nos[k].marca == Marca::Nenhuma => self.mark(n, Marca::Decl(did)),
            _ => n,
        }
    }

    fn decl_unmarked(&mut self, did: ast::DeclId) -> Option<usize> {
        let a = self.a;
        let d = a.decl(did);
        let (start, mut children) = self.doc_and_annotations(d.span.start, &d.metadata);
        match &d.kind {
            DeclKind::Class(x) => {
                children.extend(self.type_params(&x.type_params));
                if x.mixin_application {
                    if let Some(s) = x.extends {
                        children.extend(self.ty(s));
                    }
                    children.extend(self.clause("WithClause", &x.with));
                    children.extend(self.clause("ImplementsClause", &x.implements));
                    return self.node("ClassTypeAlias", start, d.span.end, children);
                }
                if let Some(s) = x.extends {
                    children.extend(self.clause("ExtendsClause", &[s]));
                }
                children.extend(self.clause("WithClause", &x.with));
                children.extend(self.clause("ImplementsClause", &x.implements));
                for &m in &x.members {
                    children.extend(self.member(m));
                }
                let n = self.node("ClassDeclaration", start, d.span.end, children);
                self.with_override(n, x.name.span.end)
            }
            DeclKind::Mixin(x) => {
                children.extend(self.type_params(&x.type_params));
                children.extend(self.clause("MixinOnClause", &x.on));
                children.extend(self.clause("ImplementsClause", &x.implements));
                for &m in &x.members {
                    children.extend(self.member(m));
                }
                self.node("MixinDeclaration", start, d.span.end, children)
            }
            DeclKind::Enum(x) => {
                children.extend(self.type_params(&x.type_params));
                children.extend(self.clause("WithClause", &x.with));
                children.extend(self.clause("ImplementsClause", &x.implements));
                for k in &x.constants {
                    children.extend(self.enum_constant(k));
                }
                for &m in &x.members {
                    children.extend(self.member(m));
                }
                self.node("EnumDeclaration", start, d.span.end, children)
            }
            DeclKind::Extension(x) => {
                children.extend(self.type_params(&x.type_params));
                children.extend(self.clause("ExtensionOnClause", &[x.on]));
                for &m in &x.members {
                    children.extend(self.member(m));
                }
                self.node("ExtensionDeclaration", start, d.span.end, children)
            }
            DeclKind::ExtensionType(x) => {
                children.extend(self.type_params(&x.type_params));
                // `RepresentationDeclaration`: `.nome` ?? `(` → `)`.
                let mut rep = Vec::new();
                if let Some(c) = x.constructor {
                    let dot = self.t.op_before(c.span.start, Op::Dot).map_or(c.span.start, |i| self.t.span(i).start);
                    rep.extend(self.node("RepresentationConstructorName", dot, c.span.end, Vec::new()));
                }
                for m in x.representation_metadata.iter() {
                    rep.extend(self.annotation(m));
                }
                rep.extend(self.ty(x.representation_type));
                children.extend(self.node("RepresentationDeclaration", x.representation_span.start, x.representation_span.end, rep));
                children.extend(self.clause("ImplementsClause", &x.implements));
                for &m in &x.members {
                    children.extend(self.member(m));
                }
                self.node("ExtensionTypeDeclaration", start, d.span.end, children)
            }
            DeclKind::Typedef(x) => match &x.kind {
                ast::TypedefKind::Alias(t) => {
                    children.extend(self.type_params(&x.type_params));
                    children.extend(self.ty(*t));
                    self.node("GenericTypeAlias", start, d.span.end, children)
                }
                ast::TypedefKind::Legacy { return_type, parameters } => {
                    if let Some(r) = return_type {
                        children.extend(self.ty(*r));
                    }
                    children.extend(self.type_params(&x.type_params));
                    children.extend(self.param_list(parameters, x.name.span.end));
                    self.node("FunctionTypeAlias", start, d.span.end, children)
                }
            },
            DeclKind::Function(f) => {
                let n = self.function_decl(*f, start, children, true);
                let name = a.function(*f).name.map(|n| n.span.end);
                match name {
                    Some(p) => self.with_override(n, p),
                    None => n,
                }
            }
            DeclKind::Variables(l) => {
                children.extend(self.variable_list(l, d.span.start, &[]));
                self.node("TopLevelVariableDeclaration", start, d.span.end, children)
            }
        }
    }

    /// `FunctionDeclaration` (top-level or local): `returnType`,
    /// `FunctionExpression`.
    fn function_decl(&mut self, fid: ast::FunctionId, start: usize, mut children: Vec<usize>, from_decl: bool) -> Option<usize> {
        let f = self.a.function(fid);
        if let Some(r) = f.return_type {
            children.extend(self.ty(r));
        }
        children.extend(self.function_expr(fid, from_decl));
        let n = self.node("FunctionDeclaration", start, f.span.end, children);
        let n = self.bind_to(n, Ligacao::Funcao(fid));
        self.mark(n, Marca::Funcao(fid))
    }

    /// `FunctionExpression`: type parameters ?? parameters ?? body → end of
    /// the body.
    fn function_expr(&mut self, fid: ast::FunctionId, from_decl: bool) -> Option<usize> {
        let f = self.a.function(fid);
        let after_name = f.name.map_or(f.span.start, |n| n.span.end);
        let mut children = Vec::new();
        let tps = self.type_params(&f.type_params);
        let after_types = tps.map_or(after_name, |n| self.nos[n].fim);
        children.extend(tps);
        let params = f.parameters.as_ref().and_then(|ps| self.param_list(ps, after_types));
        children.extend(params);
        let body = self.body(&f.body, f.modifier, f.span.end, from_decl);
        children.extend(body);
        let start = children.first().map_or(f.span.start, |&n| self.nos[n].inicio);
        let end = body.map_or(f.span.end, |n| self.nos[n].fim);
        let n = self.node("FunctionExpression", start, end, children);
        let n = self.bind_to(n, Ligacao::Funcao(fid));
        self.mark(n, Marca::Funcao(fid))
    }

    /// A function's body.
    fn body(&mut self, b: &'a FunctionBody, modifier: AsyncModifier, function_end: usize, from_decl: bool) -> Option<usize> {
        match b {
            FunctionBody::Block(s) => {
                let block = self.stmt(*s);
                let bs = self.a.stmt(*s).span;
                let start = if modifier == AsyncModifier::None { bs.start } else { self.modifier_start(bs.start) };
                self.node("BlockFunctionBody", start, bs.end, block.into_iter().collect())
            }
            FunctionBody::Expression(e) => {
                let es = self.a.expr(*e).span;
                let seta = self.t.last_op(Op::Arrow, 0, es.start);
                let start = match (modifier, seta) {
                    (AsyncModifier::None, Some(i)) => self.t.span(i).start,
                    (_, Some(i)) => self.modifier_start(self.t.span(i).start),
                    (_, None) => es.start,
                };
                let end = if from_decl { self.t.op_after(es.end, Op::Semicolon).map_or(es.end, |i| self.t.span(i).end) } else { es.end };
                let expr = self.expr(*e);
                self.node("ExpressionFunctionBody", start, end, expr.into_iter().collect())
            }
            FunctionBody::Empty => {
                let i = self.t.before(function_end).filter(|&i| self.t.is_op(i, Op::Semicolon))?;
                self.leaf("EmptyFunctionBody", self.t.span(i))
            }
            FunctionBody::Native(s) => {
                let end = self.t.before(function_end).filter(|&i| self.t.is_op(i, Op::Semicolon)).map_or(function_end, |i| self.t.span(i).end);
                let lit = s.as_ref().and_then(|s| self.string(s));
                let native = match s {
                    Some(l) => self.t.before(l.span.start).map_or(l.span.start, |i| self.t.span(i).start),
                    None => self.t.before(end).and_then(|i| self.t.before(self.t.span(i).start)).map_or(end, |i| self.t.span(i).start),
                };
                self.node("NativeFunctionBody", native, end, lit.into_iter().collect())
            }
        }
    }

    /// The `async`/`sync` before `pos` (`async`, `async*`, `sync*`).
    fn modifier_start(&self, pos: usize) -> usize {
        let mut i = match self.t.before(pos) {
            Some(i) => i,
            None => return pos,
        };
        if self.t.is_op(i, Op::Star)
            && let Some(j) = self.t.before(self.t.span(i).start)
        {
            i = j;
        }
        if self.t.is_word(i, self.source, "async") || self.t.is_word(i, self.source, "sync") { self.t.span(i).start } else { pos }
    }

    fn enum_constant(&mut self, k: &'a ast::EnumConstant) -> Option<usize> {
        let n = self.enum_constant_raw(k);
        self.bind_to(n, Ligacao::ConstanteDeEnum(endereco(k)))
    }

    fn enum_constant_raw(&mut self, k: &'a ast::EnumConstant) -> Option<usize> {
        let (start, mut children) = self.doc_and_annotations(k.span.start, &k.metadata);
        let mut args = Vec::new();
        let tas = self.type_args(&k.type_args);
        let mut args_start = tas.map(|n| self.nos[n].inicio);
        args.extend(tas);
        if let Some(c) = k.constructor {
            let dot = self.t.op_before(c.span.start, Op::Dot).map_or(c.span.start, |i| self.t.span(i).start);
            args_start = args_start.or(Some(dot));
            args.extend(self.node("ConstructorSelector", dot, c.span.end, Vec::new()));
        }
        if let Some(a) = &k.arguments {
            args_start = args_start.or(Some(a.span.start));
            args.extend(self.arg_list(a));
            children.extend(self.node("EnumConstantArguments", args_start.unwrap_or(a.span.start), a.span.end, args));
        }
        self.node("EnumConstantDeclaration", start, k.span.end, children)
    }

    fn member(&mut self, mid: ast::MemberId) -> Option<usize> {
        let n = self.member_raw(mid);
        self.bind_to(n, Ligacao::Membro(mid))
    }

    fn member_raw(&mut self, mid: ast::MemberId) -> Option<usize> {
        let a = self.a;
        let m = a.member(mid);
        let (start, mut children) = self.doc_and_annotations(m.span.start, &m.metadata);
        match &m.kind {
            MemberKind::Field(l) => {
                children.extend(self.variable_list(l, m.span.start, &[]));
                self.node("FieldDeclaration", start, m.span.end, children)
            }
            MemberKind::Method(fid) => {
                let f = a.function(*fid);
                if let Some(r) = f.return_type {
                    children.extend(self.ty(r));
                }
                let after_name = f.name.map_or(f.span.start, |n| n.span.end);
                let tps = self.type_params(&f.type_params);
                let after_types = tps.map_or(after_name, |n| self.nos[n].fim);
                children.extend(tps);
                if let Some(ps) = &f.parameters {
                    children.extend(self.param_list(ps, after_types));
                }
                children.extend(self.body(&f.body, f.modifier, m.span.end, true));
                let n = self.node("MethodDeclaration", start, m.span.end, children);
                let n = self.mark(n, Marca::Funcao(*fid));
                match f.name {
                    Some(name) => self.with_override(n, name.span.end),
                    None => n,
                }
            }
            MemberKind::Constructor(k) => {
                children.extend(self.identifier(k.class_name));
                let after_name = k.name.unwrap_or(k.class_name).span.end;
                children.extend(self.param_list(&k.parameters, after_name));
                for i in k.initializers.iter() {
                    children.extend(self.initializer(i));
                }
                if let Some(r) = &k.redirect {
                    let ty = self.ty(r.ty);
                    let ts = a.ty(r.ty).span;
                    let mut f = ty.into_iter().collect::<Vec<_>>();
                    let end = match r.constructor {
                        Some(n) => {
                            f.extend(self.identifier(n));
                            n.span.end
                        }
                        None => ts.end,
                    };
                    children.extend(self.node("ConstructorName", ts.start, end, f));
                }
                children.extend(self.body(&k.body, AsyncModifier::None, m.span.end, true));
                let n = self.node("ConstructorDeclaration", start, m.span.end, children);
                self.with_override(n, after_name)
            }
        }
    }

    fn initializer(&mut self, i: &'a Initializer) -> Option<usize> {
        let n = self.initializer_raw(i);
        self.bind_to(n, Ligacao::Inicializador(endereco(i)))
    }

    fn initializer_raw(&mut self, i: &'a Initializer) -> Option<usize> {
        match i {
            Initializer::Field { span, name, value, .. } => {
                let mut f = Vec::new();
                f.extend(self.identifier(*name));
                f.extend(self.expr(*value));
                self.node("ConstructorFieldInitializer", span.start, span.end, f)
            }
            Initializer::Super { span, constructor, arguments } => {
                let mut f = Vec::new();
                if let Some(n) = constructor {
                    f.extend(self.identifier(*n));
                }
                f.extend(self.arg_list(arguments));
                self.node("SuperConstructorInvocation", span.start, span.end, f)
            }
            Initializer::Redirect { span, constructor, arguments } => {
                let mut f = Vec::new();
                if let Some(n) = constructor {
                    f.extend(self.identifier(*n));
                }
                f.extend(self.arg_list(arguments));
                self.node("RedirectingConstructorInvocation", span.start, span.end, f)
            }
            Initializer::Assert { span, condition, message } => {
                let mut f = Vec::new();
                f.extend(self.expr(*condition));
                if let Some(m) = message {
                    f.extend(self.expr(*m));
                }
                self.node("AssertInitializer", span.start, span.end, f)
            }
        }
    }

    // -- Variables and parameters -------------------------------------------------

    /// `VariableDeclarationList`: from the first of `late`/`final`/`const`/`var`
    /// ?? type ?? 1st variable (after `static`/`covariant`/`abstract`/
    /// `external`) to the end of the last variable.
    fn variable_list(&mut self, l: &'a ast::VariableList, container_start: usize, metadata: &'a [Annotation]) -> Option<usize> {
        self.variable_list_with(l, container_start, metadata, false)
    }

    fn local_variable_list(&mut self, l: &'a ast::VariableList, container_start: usize, metadata: &'a [Annotation]) -> Option<usize> {
        self.variable_list_with(l, container_start, metadata, true)
    }

    fn variable_list_with(&mut self, l: &'a ast::VariableList, container_start: usize, metadata: &'a [Annotation], locals: bool) -> Option<usize> {
        let first = l.variables.first()?;
        let last = l.variables.last()?;
        let end = last.initializer.map_or(last.name.span.end, |e| self.a.expr(e).span.end);
        // The list's first token: the list's first modifier between the start
        // of the container and the type (or the 1st variable).
        let bound = l.ty.map_or(first.name.span.start, |t| self.a.ty(t).span.start);
        let mut start = bound;
        let mut i = self.t.index(container_start);
        while let Some(tk) = self.t.list.get(i) {
            if tk.span.start >= bound {
                break;
            }
            let text = tk.text(self.source);
            if matches!(text, "late" | "final" | "const" | "var") {
                start = tk.span.start;
                break;
            }
            i += 1;
        }
        let (start, mut children) = if metadata.is_empty() { (start, Vec::new()) } else { self.doc_and_annotations(start, metadata) };
        if let Some(t) = l.ty {
            children.extend(self.ty(t));
        }
        for v in l.variables.iter() {
            let end_v = v.initializer.map_or(v.name.span.end, |e| self.a.expr(e).span.end);
            let start = v.initializer.and_then(|e| self.expr(e));
            let n = self.node("VariableDeclaration", v.name.span.start, end_v, start.into_iter().collect());
            children.extend(if locals { self.mark(n, Marca::VariavelLocal) } else { n });
        }
        self.node("VariableDeclarationList", start, end, children)
    }

    /// `FormalParameterList`: `(` → `)`; `after` is where to look for the `(`
    /// of an empty list.
    fn param_list(&mut self, ps: &'a [Parameter], after: usize) -> Option<usize> {
        let open = match ps.first() {
            Some(p) => {
                let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                self.t.last_op(Op::LParen, 0, start)?
            }
            None => self.t.first_op(Op::LParen, after, usize::MAX)?,
        };
        let close = self.t.pair_of(open)?;
        let children: Vec<usize> = ps.iter().filter_map(|p| self.param(p)).collect();
        self.node("FormalParameterList", self.t.span(open).start, self.t.span(close).end, children)
    }

    fn param(&mut self, p: &'a Parameter) -> Option<usize> {
        let n = self.param_raw(p);
        self.bind_to(n, Ligacao::Parametro(endereco(p)))
    }

    fn param_raw(&mut self, p: &'a Parameter) -> Option<usize> {
        let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
        let mut children = Vec::new();
        for m in p.metadata.iter() {
            children.extend(self.annotation(m));
        }
        let (kind, end) = if let Some(fps) = &p.function_parameters {
            // `void f(int x)`: `returnType`, type parameters, parameters.
            if let Some(r) = p.ty {
                children.extend(self.ty(r));
            }
            let after = p.name.map_or(start, |n| n.span.end);
            let tps = self.type_params(&p.function_type_params);
            let after = tps.map_or(after, |n| self.nos[n].fim);
            children.extend(tps);
            let list = self.param_list(fps, after);
            let mut end = list.map_or(after, |n| self.nos[n].fim);
            children.extend(list);
            if p.function_nullable
                && let Some(i) = self.t.op_after(end, Op::Question)
            {
                end = self.t.span(i).end;
            }
            let kind = if p.this_ {
                "FieldFormalParameter"
            } else if p.super_ {
                "SuperFormalParameter"
            } else {
                "FunctionTypedFormalParameter"
            };
            (kind, end)
        } else {
            if let Some(t) = p.ty {
                children.extend(self.ty(t));
            }
            let end = p.name.map(|n| n.span.end).or_else(|| p.ty.map(|t| self.a.ty(t).span.end)).unwrap_or(p.span.end);
            let kind = if p.this_ {
                "FieldFormalParameter"
            } else if p.super_ {
                "SuperFormalParameter"
            } else {
                "SimpleFormalParameter"
            };
            (kind, end)
        };
        let inner = self.node(kind, start, end, children);
        let inner = self.bind_to(inner, Ligacao::Parametro(endereco(p)));
        if p.kind == ParameterKind::Required {
            return inner;
        }
        // Every optional or named one is wrapped.
        let mut f = inner.into_iter().collect::<Vec<_>>();
        let mut end_d = end;
        if let Some(d) = p.default_value {
            end_d = self.a.expr(d).span.end;
            f.extend(self.expr(d));
        }
        self.node("DefaultFormalParameter", start, end_d, f)
    }

    /// `TypeParameterList`: `<` → `>`.
    fn type_params(&mut self, tps: &'a [TypeParameter]) -> Option<usize> {
        let (Some(p), Some(u)) = (tps.first(), tps.last()) else { return None };
        let p_start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
        let lt = self.t.op_before(p_start, Op::Lt)?;
        let gt = self.t.op_after(u.span.end, Op::Gt)?;
        let mut children = Vec::new();
        for tp in tps.iter() {
            let (start, mut f) = self.doc_and_annotations(tp.span.start, &tp.metadata);
            if let Some(b) = tp.bound {
                f.extend(self.ty(b));
            }
            let n = self.node("TypeParameter", start, tp.span.end, f);
            children.extend(self.bind_to(n, Ligacao::ParametroDeTipo(endereco(tp))));
        }
        self.node("TypeParameterList", self.t.span(lt).start, self.t.span(gt).end, children)
    }

    /// `TypeArgumentList`: `<` → `>` around the types.
    fn type_args(&mut self, types: &[TypeId]) -> Option<usize> {
        let (Some(p), Some(u)) = (types.first(), types.last()) else { return None };
        let ps = self.a.ty(*p).span;
        let us = self.a.ty(*u).span;
        let lt = self.t.op_before(ps.start, Op::Lt)?;
        let gt = self.t.op_after(us.end, Op::Gt)?;
        let children: Vec<usize> = types.iter().filter_map(|t| self.ty(*t)).collect();
        self.node("TypeArgumentList", self.t.span(lt).start, self.t.span(gt).end, children)
    }

    /// `ArgumentList` with the parentheses; `name: e` is a `NamedExpression`
    /// with the `Label`.
    fn arg_list(&mut self, args: &'a Arguments) -> Option<usize> {
        let n = self.arg_list_raw(args);
        self.bind_to(n, Ligacao::Argumentos(endereco(args)))
    }

    fn arg_list_raw(&mut self, args: &'a Arguments) -> Option<usize> {
        let mut children = Vec::new();
        for x in args.args.iter() {
            children.extend(self.argument(x.name, x.value));
        }
        self.node("ArgumentList", args.span.start, args.span.end, children)
    }

    fn argument(&mut self, name: Option<ast::Name>, value: ExprId) -> Option<usize> {
        match name {
            None => self.expr(value),
            Some(n) => {
                let colon = self.t.op_after(n.span.end, Op::Colon).map_or(n.span.end, |i| self.t.span(i).end);
                let id = self.identifier(n);
                let label = self.node("Label", n.span.start, colon, id.into_iter().collect());
                let e = self.expr(value);
                let end = self.a.expr(value).span.end;
                self.node("NamedExpression", n.span.start, end, label.into_iter().chain(e).collect())
            }
        }
    }

    // -- Tipos ---------------------------------------------------------------

    fn ty(&mut self, t: TypeId) -> Option<usize> {
        let n = self.ty_raw(t);
        self.bind_to(n, Ligacao::Tipo(t))
    }

    fn ty_raw(&mut self, t: TypeId) -> Option<usize> {
        let ty = self.a.ty(t);
        match &ty.kind {
            TypeKind::Named { name, args } => {
                let mut children = Vec::new();
                if name.len() == 2 {
                    // `ImportPrefixReference`: the prefix with the dot.
                    let dot = self.t.op_after(name[0].span.end, Op::Dot).map_or(name[0].span.end, |i| self.t.span(i).end);
                    children.extend(self.node("ImportPrefixReference", name[0].span.start, dot, Vec::new()));
                }
                children.extend(self.type_args(args));
                self.node("NamedType", ty.span.start, ty.span.end, children)
            }
            TypeKind::Void => self.leaf("NamedType", ty.span),
            TypeKind::Function { return_type, type_params, parameters } => {
                let mut children = Vec::new();
                if let Some(r) = return_type {
                    children.extend(self.ty(*r));
                }
                let tps = self.type_params(type_params);
                let after = tps.map_or_else(|| self.function_keyword_pos(ty.span), |n| self.nos[n].fim);
                children.extend(tps);
                children.extend(self.param_list(parameters, after));
                self.node("GenericFunctionType", ty.span.start, ty.span.end, children)
            }
            TypeKind::Record { positional, named } => {
                let mut children = Vec::new();
                for p in positional.iter() {
                    let f = self.ty(*p);
                    let s = self.a.ty(*p).span;
                    children.extend(self.node("RecordTypeAnnotationPositionalField", s.start, s.end, f.into_iter().collect()));
                }
                if let (Some(p), Some(u)) = (named.first(), named.last()) {
                    let start = self.a.ty(p.1).span.start;
                    let open = self.t.last_op(Op::LBrace, 0, start);
                    let close = open.and_then(|i| self.t.pair_of(i));
                    let mut fields = Vec::new();
                    for (n, t) in named.iter() {
                        let f = self.ty(*t);
                        let s = self.a.ty(*t).span;
                        fields.extend(self.node("RecordTypeAnnotationNamedField", s.start, n.span.end, f.into_iter().collect()));
                    }
                    let (a, b) = match (open, close) {
                        (Some(a), Some(b)) => (self.t.span(a).start, self.t.span(b).end),
                        _ => (start, u.0.span.end),
                    };
                    children.extend(self.node("RecordTypeAnnotationNamedFields", a, b, fields));
                }
                self.node("RecordTypeAnnotation", ty.span.start, ty.span.end, children)
            }
        }
    }

    /// Where to look for the `(` of a `Function(...)` without type parameters:
    /// the `Function` token inside the type.
    fn function_keyword_pos(&self, s: Span) -> usize {
        let mut i = self.t.index(s.start);
        while let Some(tk) = self.t.list.get(i) {
            if tk.span.start >= s.end {
                break;
            }
            if tk.text(self.source) == "Function" {
                return tk.span.end;
            }
            i += 1;
        }
        s.start
    }

    // -- Comandos --------------------------------------------------------------

    fn stmt(&mut self, sid: StmtId) -> Option<usize> {
        let n = self.stmt_raw(sid);
        self.bind_to(n, Ligacao::Stmt(sid))
    }

    fn stmt_raw(&mut self, sid: StmtId) -> Option<usize> {
        let a = self.a;
        let s = a.stmt(sid);
        match &s.kind {
            StmtKind::Block(ss) => {
                let children: Vec<usize> = ss.iter().filter_map(|x| self.stmt(*x)).collect();
                self.node("Block", s.span.start, s.span.end, children)
            }
            StmtKind::Variables(l) => {
                let meta = self.local_metadata.get(&sid.0).copied().unwrap_or(&[]);
                let list = self.local_variable_list(l, s.span.start, meta);
                let start = list.map_or(s.span.start, |n| self.nos[n].inicio.min(s.span.start));
                self.node("VariableDeclarationStatement", start, s.span.end, list.into_iter().collect())
            }
            StmtKind::PatternVariables { pattern, value, .. } => {
                let end = a.expr(*value).span.end;
                let p = self.pattern(*pattern);
                let e = self.expr(*value);
                let decl = self.node("PatternVariableDeclaration", s.span.start, end, p.into_iter().chain(e).collect());
                self.node("PatternVariableDeclarationStatement", s.span.start, s.span.end, decl.into_iter().collect())
            }
            StmtKind::Function(fid) => {
                let meta = self.local_metadata.get(&sid.0).copied().unwrap_or(&[]);
                let (start, children) = self.doc_and_annotations(s.span.start, meta);
                let d = self.function_decl(*fid, start, children, true);
                let name = a.function(*fid).name.map(|n| n.span.end);
                let d = match name {
                    Some(p) => self.with_override(d, p),
                    None => d,
                };
                let start = d.map_or(s.span.start, |n| self.nos[n].inicio);
                let end = d.map_or(s.span.end, |n| self.nos[n].fim);
                self.node("FunctionDeclarationStatement", start, end, d.into_iter().collect())
            }
            StmtKind::Expression(e) => {
                let x = self.expr(*e);
                self.node("ExpressionStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                let mut children = Vec::new();
                children.extend(self.expr(*condition));
                children.extend(self.case_clause(*case_pattern, *guard));
                children.extend(self.stmt(*then));
                if let Some(e) = else_ {
                    children.extend(self.stmt(*e));
                }
                self.node("IfStatement", s.span.start, s.span.end, children)
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                let parts = self.for_parts(s.span.start, init.as_ref(), *condition, updates);
                let body = self.stmt(*body);
                self.node("ForStatement", s.span.start, s.span.end, parts.into_iter().chain(body).collect())
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                let parts = self.for_in_parts(s.span.start, target, *iterable);
                let body = self.stmt(*body);
                self.node("ForStatement", s.span.start, s.span.end, parts.into_iter().chain(body).collect())
            }
            StmtKind::While { condition, body } => {
                let c = self.expr(*condition);
                let b = self.stmt(*body);
                self.node("WhileStatement", s.span.start, s.span.end, c.into_iter().chain(b).collect())
            }
            StmtKind::DoWhile { body, condition } => {
                let b = self.stmt(*body);
                let c = self.expr(*condition);
                self.node("DoStatement", s.span.start, s.span.end, b.into_iter().chain(c).collect())
            }
            StmtKind::Switch { value, cases } => {
                let mut children = Vec::new();
                children.extend(self.expr(*value));
                for c in cases.iter() {
                    children.extend(self.case(c));
                }
                self.node("SwitchStatement", s.span.start, s.span.end, children)
            }
            StmtKind::Break(l) | StmtKind::Continue(l) => {
                let kind = if matches!(s.kind, StmtKind::Break(_)) { "BreakStatement" } else { "ContinueStatement" };
                let r = l.and_then(|n| self.identifier(n));
                self.node(kind, s.span.start, s.span.end, r.into_iter().collect())
            }
            StmtKind::Return(e) => {
                let x = e.and_then(|e| self.expr(e));
                self.node("ReturnStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::Yield { value, .. } => {
                let x = self.expr(*value);
                self.node("YieldStatement", s.span.start, s.span.end, x.into_iter().collect())
            }
            StmtKind::Try { body, catches, finally_ } => {
                let mut children = Vec::new();
                children.extend(self.stmt(*body));
                for c in catches.iter() {
                    let mut f = Vec::new();
                    if let Some(t) = c.on_type {
                        f.extend(self.ty(t));
                    }
                    if let Some(e) = c.exception {
                        f.extend(self.leaf("CatchClauseParameter", e.span));
                    }
                    if let Some(st) = c.stack_trace {
                        f.extend(self.leaf("CatchClauseParameter", st.span));
                    }
                    f.extend(self.stmt(c.body));
                    let cc = self.node("CatchClause", c.span.start, c.span.end, f);
                    children.extend(self.bind_to(cc, Ligacao::Catch(endereco(c))));
                }
                if let Some(fi) = finally_ {
                    children.extend(self.stmt(*fi));
                }
                self.node("TryStatement", s.span.start, s.span.end, children)
            }
            StmtKind::Labeled { labels, body } => {
                let mut children = Vec::new();
                for l in labels.iter() {
                    children.extend(self.label(*l));
                }
                children.extend(self.stmt(*body));
                self.node("LabeledStatement", s.span.start, s.span.end, children)
            }
            StmtKind::Assert { condition, message } => {
                let mut children = Vec::new();
                children.extend(self.expr(*condition));
                if let Some(m) = message {
                    children.extend(self.expr(*m));
                }
                self.node("AssertStatement", s.span.start, s.span.end, children)
            }
            StmtKind::Empty => self.leaf("EmptyStatement", s.span),
        }
    }

    /// A statement `Label`: name → `:`.
    fn label(&mut self, n: ast::Name) -> Option<usize> {
        let end = self.t.op_after(n.span.end, Op::Colon).map_or(n.span.end, |i| self.t.span(i).end);
        let id = self.identifier(n);
        let id = self.mark(id, Marca::ContextoDeDeclaracao);
        self.node("Label", n.span.start, end, id.into_iter().collect())
    }

    /// The `CaseClause` of `if (e case p when g)`.
    fn case_clause(&mut self, pattern: Option<PatternId>, guard: Option<ExprId>) -> Option<usize> {
        let p = pattern?;
        let ps = self.a.pattern(p).span;
        let word = self.t.before(ps.start).map_or(ps.start, |i| self.t.span(i).start);
        let gp = self.guarded_pattern(p, guard);
        let end = gp.map_or(ps.end, |n| self.nos[n].fim);
        self.node("CaseClause", word, end, gp.into_iter().collect())
    }

    /// `GuardedPattern` (pattern → guard) with the `WhenClause`.
    fn guarded_pattern(&mut self, p: PatternId, guard: Option<ExprId>) -> Option<usize> {
        let ps = self.a.pattern(p).span;
        let mut children = Vec::new();
        children.extend(self.pattern(p));
        let mut end = ps.end;
        if let Some(g) = guard {
            let gs = self.a.expr(g).span;
            let when = self.t.before(gs.start).map_or(gs.start, |i| self.t.span(i).start);
            let e = self.expr(g);
            children.extend(self.node("WhenClause", when, gs.end, e.into_iter().collect()));
            end = gs.end;
        }
        self.node("GuardedPattern", ps.start, end, children)
    }

    fn case(&mut self, c: &'a ast::SwitchCase) -> Option<usize> {
        let n = self.case_raw(c);
        self.bind_to(n, Ligacao::Caso(endereco(c)))
    }

    fn case_raw(&mut self, c: &'a ast::SwitchCase) -> Option<usize> {
        let mut children = Vec::new();
        for l in c.labels.iter() {
            children.extend(self.label(*l));
        }
        let kind = match c.pattern {
            None => "SwitchDefault",
            Some(p) => {
                if self.before_3_0 {
                    match &self.a.pattern(p).kind {
                        PatternKind::Constant(e) => children.extend(self.expr(*e)),
                        _ => children.extend(self.pattern(p)),
                    }
                    "SwitchCase"
                } else {
                    children.extend(self.guarded_pattern(p, c.guard));
                    "SwitchPatternCase"
                }
            }
        };
        for s in c.body.iter() {
            children.extend(self.stmt(*s));
        }
        self.node(kind, c.span.start, c.span.end, children)
    }

    /// The two depth-zero `;`s inside the `(` of a `for`.
    fn for_semicolons(&self, for_start: usize) -> (Option<usize>, Option<usize>, Option<usize>) {
        let Some(open) = self.t.first_op(Op::LParen, for_start, usize::MAX) else { return (None, None, None) };
        let Some(close) = self.t.pair_of(open) else { return (Some(open), None, None) };
        let mut first = None;
        let mut segundo = None;
        let mut i = open + 1;
        while i < close {
            if let Some(j) = self.t.pair_of(i)
                && j > i
            {
                i = j + 1;
                continue;
            }
            if self.t.is_op(i, Op::Semicolon) {
                if first.is_none() {
                    first = Some(i);
                } else {
                    segundo = Some(i);
                    break;
                }
            }
            i += 1;
        }
        (Some(open), first, segundo)
    }

    fn for_parts(&mut self, for_start: usize, init: Option<&'a ForInit>, cond: Option<ExprId>, updates: &'a [ExprId]) -> Option<usize> {
        let (_, p1, p2) = self.for_semicolons(for_start);
        let end = match updates.last() {
            Some(u) => self.a.expr(*u).span.end,
            None => p2.map(|i| self.t.span(i).end)?,
        };
        let mut children = Vec::new();
        let mut parts = PartesDeFor::default();
        let (kind, start) = match init {
            Some(ForInit::Variables(l)) => {
                // The list starts right after the `(` of the `for`.
                let after_paren = self.t.first_op(Op::LParen, for_start, usize::MAX).map_or(for_start, |i| self.t.span(i).end);
                let list = self.local_variable_list(l, after_paren, &[]);
                let start = list.map_or(after_paren, |n| self.nos[n].inicio);
                children.extend(list);
                ("ForPartsWithDeclarations", start)
            }
            Some(ForInit::Expression(e)) => {
                let s = self.a.expr(*e).span;
                children.extend(self.expr(*e));
                ("ForPartsWithExpression", s.start)
            }
            Some(ForInit::Pattern { pattern, value, .. }) => {
                let open = self.t.first_op(Op::LParen, for_start, usize::MAX).map_or(0, |i| self.t.span(i).end);
                let word = self.t.after(open).map_or(open, |i| self.t.span(i).start);
                let end_v = self.a.expr(*value).span.end;
                let p = self.pattern(*pattern);
                let e = self.expr(*value);
                children.extend(self.node("PatternVariableDeclaration", word, end_v, p.into_iter().chain(e).collect()));
                ("ForPartsWithPattern", word)
            }
            None => ("ForPartsWithExpression", p1.map(|i| self.t.span(i).start)?),
        };
        parts.inicio = if init.is_some() { children.first().copied() } else { None };
        if let Some(c) = cond {
            let n = self.expr(c);
            parts.condicao = n;
            children.extend(n);
        }
        for u in updates.iter() {
            let n = self.expr(*u);
            parts.atualizacoes.extend(n);
            children.extend(n);
        }
        let n = self.node(kind, start, end, children);
        if let Some(k) = n {
            self.partes_de_for.insert(k, parts);
        }
        n
    }

    fn for_in_parts(&mut self, for_start: usize, callee: &'a ForInTarget, iterable: ExprId) -> Option<usize> {
        let end = self.a.expr(iterable).span.end;
        let open = self.t.first_op(Op::LParen, for_start, usize::MAX).map_or(for_start, |i| self.t.span(i).end);
        let first = self.t.after(open).map_or(open, |i| self.t.span(i).start);
        let mut children = Vec::new();
        let (kind, start) = match callee {
            ForInTarget::Declared { metadata, ty, name, .. } => {
                let (start, mut f) = self.doc_and_annotations(first, metadata);
                if let Some(t) = ty {
                    f.extend(self.ty(*t));
                }
                let d = self.node("DeclaredIdentifier", start, name.span.end, f);
                let start = d.map_or(start, |n| self.nos[n].inicio);
                children.extend(d);
                ("ForEachPartsWithDeclaration", start)
            }
            ForInTarget::Pattern { pattern, .. } => {
                children.extend(self.pattern(*pattern));
                ("ForEachPartsWithPattern", first)
            }
            ForInTarget::Expression(e) => {
                let s = self.a.expr(*e).span;
                children.extend(self.expr(*e));
                ("ForEachPartsWithIdentifier", s.start)
            }
        };
        children.extend(self.expr(iterable));
        self.node(kind, start, end, children)
    }

    // -- Expressions --------------------------------------------------------------

    /// The leftmost node of the expression is the cascade's implicit target.
    fn in_cascade(&self, e: ExprId) -> bool {
        let mut current = e;
        for _ in 0..256 {
            current = match &self.a.expr(current).kind {
                ExprKind::CascadeTarget => return true,
                ExprKind::Property { target, .. }
                | ExprKind::Index { target, .. }
                | ExprKind::Call { target, .. }
                | ExprKind::TypeArguments { target, .. }
                | ExprKind::Assign { target, .. } => *target,
                ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert, operand } => *operand,
                _ => return false,
            };
        }
        false
    }

    /// The start of a cascade section: the `..`/`?..` before it.
    fn expr_start(&self, e: ExprId) -> usize {
        let s = self.a.expr(e).span;
        if self.in_cascade(e) {
            let i = self.t.before(s.start).filter(|&i| self.t.is_op(i, Op::DotDot) || self.t.is_op(i, Op::QuestionDotDot));
            if let Some(i) = i {
                return self.t.span(i).start;
            }
            // The `..` may be inside the span (the parser includes it).
            if let Some(i) = self.t.after(s.start)
                && (self.t.is_op(i, Op::DotDot) || self.t.is_op(i, Op::QuestionDotDot))
            {
                return self.t.span(i).start;
            }
        }
        s.start
    }

    fn expr(&mut self, e: ExprId) -> Option<usize> {
        let n = self.expr_raw(e);
        self.bind_to(n, Ligacao::Expr(e))
    }

    fn expr_raw(&mut self, e: ExprId) -> Option<usize> {
        let a = self.a;
        let x = a.expr(e);
        let s = Span { start: self.expr_start(e), end: x.span.end };
        match &x.kind {
            ExprKind::Int(_) => self.leaf("IntegerLiteral", s),
            ExprKind::Double(_) => self.leaf("DoubleLiteral", s),
            ExprKind::Bool(_) => self.leaf("BooleanLiteral", s),
            ExprKind::Null => self.leaf("NullLiteral", s),
            ExprKind::String(lit) => self.string(lit),
            ExprKind::Symbol(_) => self.leaf("SymbolLiteral", s),
            ExprKind::Identifier(_) => {
                let n = self.leaf("SimpleIdentifier", s);
                self.mark(n, Marca::Expr(e))
            }
            ExprKind::This => self.leaf("ThisExpression", s),
            ExprKind::Super => self.leaf("SuperExpression", s),
            ExprKind::Rethrow => self.leaf("RethrowExpression", s),
            ExprKind::CascadeTarget => None,
            ExprKind::DotShorthand { .. } => self.leaf("DotShorthand", s),
            ExprKind::Parenthesized(i) => {
                let f = self.expr(*i);
                self.node("ParenthesizedExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } => {
                let kind = if matches!(x.kind, ExprKind::List { .. }) { "ListLiteral" } else { "SetOrMapLiteral" };
                let mut children = Vec::new();
                children.extend(self.type_args(type_args));
                for el in elements.iter() {
                    children.extend(self.element(el));
                }
                self.node(kind, s.start, s.end, children)
            }
            ExprKind::Record { positional, named, .. } => {
                let mut items: Vec<(usize, Option<ast::Name>, ExprId)> = Vec::new();
                for p in positional.iter() {
                    items.push((a.expr(*p).span.start, None, *p));
                }
                for (n, v) in named.iter() {
                    items.push((n.span.start, Some(*n), *v));
                }
                items.sort_by_key(|(p, _, _)| *p);
                let mut children = Vec::new();
                for (_, n, v) in items {
                    children.extend(self.argument(n, v));
                }
                self.node("RecordLiteral", s.start, s.end, children)
            }
            ExprKind::InstanceCreation { keyword, ty, constructor, arguments } => {
                let ts = a.ty(*ty).span;
                if keyword.is_none() && constructor.is_none() && !self.constructor_calls.is_some_and(|c| c.contains(&e)) {
                    // `A<T>()` without a keyword: `MethodInvocation`.
                    return self.type_invocation(s, *ty, arguments);
                }
                let mut f = Vec::new();
                f.extend(self.ty(*ty));
                let end = match constructor {
                    Some(n) => {
                        f.extend(self.identifier(*n));
                        n.span.end
                    }
                    None => ts.end,
                };
                let name = self.node("ConstructorName", ts.start, end, f);
                let mut children: Vec<usize> = name.into_iter().collect();
                children.extend(self.type_args(&arguments.type_args));
                children.extend(self.arg_list(arguments));
                self.node("InstanceCreationExpression", s.start, s.end, children)
            }
            ExprKind::FunctionExpression(fid) => {
                let n = self.function_expr(*fid, false)?;
                // The closure starts at the type parameters or at the `(`.
                Some(n)
            }
            ExprKind::Property { target, name, null_aware } => {
                let mut children = Vec::new();
                children.extend(self.expr(*target));
                let id = self.identifier(*name);
                children.extend(self.mark(id, Marca::Expr(e)));
                let prefixed = !*null_aware && matches!(a.expr(*target).kind, ExprKind::Identifier(_)) && !self.in_cascade(e);
                self.node(if prefixed { "PrefixedIdentifier" } else { "PropertyAccess" }, s.start, s.end, children)
            }
            ExprKind::Index { target, index, .. } => {
                let mut children = Vec::new();
                children.extend(self.expr(*target));
                children.extend(self.expr(*index));
                self.node("IndexExpression", s.start, s.end, children)
            }
            ExprKind::Call { target, arguments } => self.call(e, s, *target, arguments),
            ExprKind::TypeArguments { target, type_args } => {
                let mut children = Vec::new();
                children.extend(self.expr(*target));
                children.extend(self.type_args(type_args));
                self.node("FunctionReference", s.start, s.end, children)
            }
            ExprKind::Unary { op, operand } => {
                let kind = match op {
                    UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert => "PostfixExpression",
                    _ => "PrefixExpression",
                };
                let f = self.expr(*operand);
                let n = self.node(kind, s.start, s.end, f.into_iter().collect());
                if kind == "PrefixExpression" { self.mark(n, Marca::Operador(e)) } else { n }
            }
            ExprKind::Binary { left, right, .. } => {
                let l = self.expr(*left);
                let r = self.expr(*right);
                self.node("BinaryExpression", s.start, s.end, l.into_iter().chain(r).collect())
            }
            ExprKind::Conditional { condition, then, else_ } => {
                let mut f = Vec::new();
                f.extend(self.expr(*condition));
                f.extend(self.expr(*then));
                f.extend(self.expr(*else_));
                self.node("ConditionalExpression", s.start, s.end, f)
            }
            ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
                let kind = if matches!(x.kind, ExprKind::Is { .. }) { "IsExpression" } else { "AsExpression" };
                let v = self.expr(*value);
                let t = self.ty(*ty);
                self.node(kind, s.start, s.end, v.into_iter().chain(t).collect())
            }
            ExprKind::Assign { target, value, .. } => {
                let l = self.expr(*target);
                let r = self.expr(*value);
                self.node("AssignmentExpression", s.start, s.end, l.into_iter().chain(r).collect())
            }
            ExprKind::PatternAssign { pattern, value } => {
                let p = self.pattern(*pattern);
                let v = self.expr(*value);
                self.node("PatternAssignment", s.start, s.end, p.into_iter().chain(v).collect())
            }
            ExprKind::Cascade { target, sections, .. } => {
                let mut f = Vec::new();
                f.extend(self.expr(*target));
                for sec in sections.iter() {
                    f.extend(self.expr(*sec));
                }
                self.node("CascadeExpression", s.start, s.end, f)
            }
            ExprKind::Await(i) => {
                let f = self.expr(*i);
                self.node("AwaitExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::Throw(i) => {
                let f = self.expr(*i);
                self.node("ThrowExpression", s.start, s.end, f.into_iter().collect())
            }
            ExprKind::Switch { value, cases } => {
                let mut f = Vec::new();
                f.extend(self.expr(*value));
                for (k, c) in cases.iter().enumerate() {
                    let gp = self.guarded_pattern(c.pattern, c.guard);
                    let b = self.expr(c.body);
                    let start = self.a.pattern(c.pattern).span.start;
                    let end = self.a.expr(c.body).span.end;
                    let n = self.node("SwitchExpressionCase", start, end, gp.into_iter().chain(b).collect());
                    f.extend(self.bind_to(n, Ligacao::CasoDeSwitchExpr(e, k)));
                }
                self.node("SwitchExpression", s.start, s.end, f)
            }
        }
    }

    /// `A<T>()` without `new`/`const`: the parser's `MethodInvocation`.
    fn type_invocation(&mut self, s: Span, ty: TypeId, arguments: &'a Arguments) -> Option<usize> {
        let mut children = Vec::new();
        if let TypeKind::Named { name, args } = &self.a.ty(ty).kind {
            match &name[..] {
                [n] => children.extend(self.identifier(*n)),
                [p, n] => {
                    children.extend(self.identifier(*p));
                    children.extend(self.identifier(*n));
                }
                _ => {}
            }
            children.extend(self.type_args(args));
        }
        children.extend(self.arg_list(arguments));
        self.node("MethodInvocation", s.start, s.end, children)
    }

    /// `MethodInvocation` (an identifier target or `target.name`) or
    /// `FunctionExpressionInvocation`.
    fn call(&mut self, e: ExprId, s: Span, callee: ExprId, arguments: &'a Arguments) -> Option<usize> {
        let a = self.a;
        // `f<int>(x)`: the type arguments may come on the target.
        let (callee, target_types): (ExprId, Option<&'a [TypeId]>) = match &a.expr(callee).kind {
            ExprKind::TypeArguments { target, type_args }
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) =>
            {
                (*target, Some(&type_args[..]))
            }
            _ => (callee, None),
        };
        if self.constructor_calls.is_some_and(|c| c.contains(&e))
            && let Some(n) = self.creation_without_new(s, callee, target_types.unwrap_or(&arguments.type_args), arguments)
        {
            return Some(n);
        }
        let mut children = Vec::new();
        let kind = match &a.expr(callee).kind {
            // `assert(…)` as an expression (`AstBuilder.endAssert`).
            ExprKind::Identifier(_) if a.invocacoes_de_assert.contains(&e) => {
                children.extend(self.expr(callee));
                "FunctionExpressionInvocation"
            }
            ExprKind::Identifier(n) => {
                let id = self.identifier(*n);
                children.extend(self.mark(id, Marca::Expr(callee)));
                "MethodInvocation"
            }
            ExprKind::Property { target, name, .. } => {
                children.extend(self.expr(*target));
                let id = self.identifier(*name);
                children.extend(self.mark(id, Marca::Expr(callee)));
                "MethodInvocation"
            }
            _ => {
                children.extend(self.expr(callee));
                "FunctionExpressionInvocation"
            }
        };
        match target_types {
            Some(t) => children.extend(self.type_args(t)),
            None => children.extend(self.type_args(&arguments.type_args)),
        }
        children.extend(self.arg_list(arguments));
        self.node(kind, s.start, s.end, children)
    }

    /// `AstRewriter.methodInvocation`: `A()`, `A.n()`, `p.A()` and `p.A.n()`
    /// that invoke a constructor become `InstanceCreationExpression` with
    /// `ConstructorName(NamedType, name?)`; the invocation's type arguments go
    /// to the `NamedType`.
    fn creation_without_new(&mut self, s: Span, callee: ExprId, types: &'a [TypeId], arguments: &'a Arguments) -> Option<usize> {
        let a = self.a;
        let e_prefixo = |x: ExprId| self.prefix_idents.is_some_and(|p| p.contains(&x));
        // `A<T>.n()` and `p.A<T>.n()` (`parseImplicitCreationExpression`): the
        // type arguments come between the type and the constructor name and
        // stay on the `NamedType`.
        let mut type_types: Option<&'a [TypeId]> = None;
        // (prefixo, tipo, construtor nomeado)
        let (prefix, ty, name): (Option<ast::Name>, ast::Name, Option<(ast::Name, ExprId)>) = match &a.expr(callee).kind {
            ExprKind::Identifier(n) => (None, *n, None),
            ExprKind::Property { target, name, .. } => match &a.expr(*target).kind {
                ExprKind::Identifier(p) if e_prefixo(*target) => (Some(*p), *name, None),
                ExprKind::Identifier(c) => (None, *c, Some((*name, callee))),
                ExprKind::Property { target: t2, name: c, .. } => match &a.expr(*t2).kind {
                    ExprKind::Identifier(p) if e_prefixo(*t2) => (Some(*p), *c, Some((*name, callee))),
                    _ => return None,
                },
                ExprKind::TypeArguments { target: inner, type_args } => {
                    type_types = Some(&type_args[..]);
                    match &a.expr(*inner).kind {
                        ExprKind::Identifier(c) => (None, *c, Some((*name, callee))),
                        ExprKind::Property { target: t2, name: c, .. } => match &a.expr(*t2).kind {
                            ExprKind::Identifier(p) if e_prefixo(*t2) => (Some(*p), *c, Some((*name, callee))),
                            _ => return None,
                        },
                        _ => return None,
                    }
                }
                _ => return None,
            },
            _ => return None,
        };
        let mut f = Vec::new();
        let type_start = prefix.map_or(ty.span.start, |p| p.span.start);
        if let Some(p) = prefix {
            let dot = self.t.op_after(p.span.end, Op::Dot).map_or(p.span.end, |i| self.t.span(i).end);
            f.extend(self.node("ImportPrefixReference", p.span.start, dot, Vec::new()));
        }
        // The type arguments stay on the type when they come right after it
        // (`A<T>()`, `p.A<T>()`); after the constructor name (`A.n<T>()`, an
        // error) they stay on the creation.
        let args_on_type = (name.is_none() && !types.is_empty()) || type_types.is_some();
        let mut type_end = ty.span.end;
        if args_on_type {
            let n = self.type_args(type_types.unwrap_or(types));
            if let Some(k) = n {
                type_end = self.nos[k].fim;
            }
            f.extend(n);
        }
        let named = self.node("NamedType", type_start, type_end, f);
        let mut cn = Vec::new();
        cn.extend(named);
        let mut end_cn = type_end;
        if let Some((n, from)) = name {
            let id = self.identifier(n);
            cn.extend(self.mark(id, Marca::Expr(from)));
            end_cn = n.span.end;
        }
        let constructor_name = self.node("ConstructorName", type_start, end_cn, cn);
        let mut children: Vec<usize> = constructor_name.into_iter().collect();
        if !args_on_type || (type_types.is_some() && !types.is_empty()) {
            children.extend(self.type_args(types));
        }
        children.extend(self.arg_list(arguments));
        self.node("InstanceCreationExpression", s.start, s.end, children)
    }

    /// A collection element.
    fn element(&mut self, el: &'a CollectionElement) -> Option<usize> {
        let n = self.element_raw(el);
        self.bind_to(n, Ligacao::Elemento(endereco(el)))
    }

    fn element_raw(&mut self, el: &'a CollectionElement) -> Option<usize> {
        match el {
            CollectionElement::Expression(e) => self.expr(*e),
            CollectionElement::NullAwareExpression(e) => {
                let s = self.a.expr(*e).span;
                let q = self.t.op_before(s.start, Op::Question).map_or(s.start, |i| self.t.span(i).start);
                let f = self.expr(*e);
                self.node("NullAwareElement", q, s.end, f.into_iter().collect())
            }
            CollectionElement::MapEntry { key, value, null_aware_key, .. } => {
                let ks = self.a.expr(*key).span;
                let start = if *null_aware_key { self.t.op_before(ks.start, Op::Question).map_or(ks.start, |i| self.t.span(i).start) } else { ks.start };
                let k = self.expr(*key);
                let v = self.expr(*value);
                let end = self.a.expr(*value).span.end;
                self.node("MapLiteralEntry", start, end, k.into_iter().chain(v).collect())
            }
            CollectionElement::Spread { value, .. } => {
                let s = self.a.expr(*value).span;
                let three = self
                    .t
                    .before(s.start)
                    .filter(|&i| self.t.is_op(i, Op::Ellipsis) || self.t.is_op(i, Op::EllipsisQuestion))
                    .map_or(s.start, |i| self.t.span(i).start);
                let f = self.expr(*value);
                self.node("SpreadElement", three, s.end, f.into_iter().collect())
            }
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                let cs = self.a.expr(*condition).span;
                // `if (`: the `if` before the `(` that opens the condition.
                let open = self.t.last_op(Op::LParen, 0, cs.start);
                let word = open.and_then(|i| self.t.before(self.t.span(i).start)).map_or(cs.start, |i| self.t.span(i).start);
                let mut f = Vec::new();
                f.extend(self.expr(*condition));
                f.extend(self.case_clause(*case_pattern, *guard));
                let t = self.element(then);
                let mut end = t.map_or(cs.end, |n| self.nos[n].fim);
                f.extend(t);
                if let Some(e) = else_ {
                    let x = self.element(e);
                    end = x.map_or(end, |n| self.nos[n].fim);
                    f.extend(x);
                }
                self.node("IfElement", word, end, f)
            }
            CollectionElement::For { await_, init, condition, updates, body } => {
                let body_start = self.element_start(body);
                let (word, open) = self.for_before(body_start, *await_);
                let parts = self.for_parts(open, init.as_ref(), *condition, updates);
                let b = self.element(body);
                let end = b.map_or(body_start, |n| self.nos[n].fim);
                self.node("ForElement", word, end, parts.into_iter().chain(b).collect())
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                let body_start = self.element_start(body);
                let (word, open) = self.for_before(body_start, *await_);
                let parts = self.for_in_parts(open, target, *iterable);
                let b = self.element(body);
                let end = b.map_or(body_start, |n| self.nos[n].fim);
                self.node("ForElement", word, end, parts.into_iter().chain(b).collect())
            }
        }
    }

    /// The first offset of an element (without building nodes).
    fn element_start(&self, el: &CollectionElement) -> usize {
        match el {
            CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) | CollectionElement::Spread { value: e, .. } => {
                self.t.before(self.a.expr(*e).span.start).map_or(self.a.expr(*e).span.start, |i| self.t.span(i).start).min(self.a.expr(*e).span.start)
            }
            CollectionElement::MapEntry { key, .. } => self.a.expr(*key).span.start,
            CollectionElement::If { condition, .. } => self.a.expr(*condition).span.start,
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => self.element_start(body),
        }
    }

    /// The `for` (or `await`) and the `(` of a `for` element whose body starts
    /// at `body`: the `)` before the body closes the `for`'s `(`.
    fn for_before(&self, body: usize, await_: bool) -> (usize, usize) {
        let close = self.t.last_op(Op::RParen, 0, body + 1);
        let open = close.and_then(|i| self.t.pair_of(i));
        let word = open.and_then(|i| self.t.before(self.t.span(i).start));
        let mut start = word.map_or(body, |i| self.t.span(i).start);
        if await_
            && let Some(p) = word
            && let Some(i) = self.t.before(self.t.span(p).start)
            && self.t.is_word(i, self.source, "await")
        {
            start = self.t.span(i).start;
        }
        (start, word.map_or(body, |i| self.t.span(i).start))
    }

    // -- Strings ---------------------------------------------------------------

    /// `SimpleStringLiteral`, `StringInterpolation` or `AdjacentStrings`, from
    /// the string's tokens.
    fn string(&mut self, lit: &'a StringLit) -> Option<usize> {
        let interpolations: Vec<ExprId> = lit
            .parts
            .iter()
            .filter_map(|p| match p {
                ast::StringPart::Interpolation(e) => Some(*e),
                ast::StringPart::Text(_) => None,
            })
            .collect();
        let mut next = 0usize;
        let mut groups: Vec<usize> = Vec::new();
        let mut i = self.t.index(lit.span.start);
        while let Some(tk) = self.t.list.get(i).copied() {
            if tk.span.start >= lit.span.end {
                break;
            }
            match tk.kind {
                Kind::Str(_) => {
                    groups.extend(self.leaf("SimpleStringLiteral", tk.span));
                    i += 1;
                }
                Kind::StrBegin(_, interp) => {
                    // `StringInterpolation`: from the start to the `StrEnd`.
                    let start = tk.span.start;
                    let mut elements = Vec::new();
                    let mut fragment = tk.span;
                    let mut mode = interp;
                    let mut fechou = false;
                    let mut end = tk.span.end;
                    loop {
                        // The text without the trailing `$`/`${`.
                        let cut = match mode {
                            dartforge_frontend::token::Interp::Brace => 2,
                            dartforge_frontend::token::Interp::Ident => 1,
                        };
                        let text_end = fragment.end.saturating_sub(cut).max(fragment.start);
                        elements.extend(self.leaf("InterpolationString", Span { start: fragment.start, end: text_end }));
                        // The interpolated expression.
                        let e = interpolations.get(next).copied();
                        next += 1;
                        let (expr_node, expr_end) = match e {
                            Some(e) => (self.expr(e), self.a.expr(e).span.end),
                            None => (None, fragment.end),
                        };
                        // The next fragment of the string (`StrMid`/`StrEnd`).
                        let mut j = self.t.index(expr_end);
                        while let Some(t2) = self.t.list.get(j) {
                            if matches!(t2.kind, Kind::StrMid(..) | Kind::StrEnd(_)) {
                                break;
                            }
                            j += 1;
                        }
                        let Some(t2) = self.t.list.get(j).copied() else { break };
                        // `${e}`: the `}` is the 1st character of the next fragment.
                        let interp_end = match mode {
                            dartforge_frontend::token::Interp::Brace => (t2.span.start + 1).min(t2.span.end),
                            dartforge_frontend::token::Interp::Ident => expr_end,
                        };
                        elements.extend(self.node("InterpolationExpression", text_end, interp_end, expr_node.into_iter().collect()));
                        let text_start = interp_end.max(t2.span.start);
                        match t2.kind {
                            Kind::StrMid(_, m) => {
                                fragment = Span { start: text_start, end: t2.span.end };
                                mode = m;
                                i = j + 1;
                            }
                            _ => {
                                elements.extend(self.leaf("InterpolationString", Span { start: text_start, end: t2.span.end }));
                                end = t2.span.end;
                                i = j + 1;
                                fechou = true;
                                break;
                            }
                        }
                    }
                    if !fechou {
                        i += 1;
                    }
                    groups.extend(self.node("StringInterpolation", start, end, elements));
                }
                _ => i += 1,
            }
        }
        match groups.len() {
            0 => None,
            1 => Some(groups[0]),
            _ => {
                let start = self.nos[groups[0]].inicio;
                let end = self.nos[*groups.last().expect("não vazio")].fim;
                self.node("AdjacentStrings", start, end, groups)
            }
        }
    }

    // -- Patterns -----------------------------------------------------------------

    fn pattern(&mut self, p: PatternId) -> Option<usize> {
        let n = self.pattern_raw(p);
        self.bind_to(n, Ligacao::Padrao(p))
    }

    fn pattern_raw(&mut self, p: PatternId) -> Option<usize> {
        let a = self.a;
        let x = a.pattern(p);
        let s = x.span;
        match &x.kind {
            PatternKind::Wildcard { ty } => {
                let t = ty.and_then(|t| self.ty(t));
                self.node("WildcardPattern", s.start, s.end, t.into_iter().collect())
            }
            PatternKind::Variable { ty, final_, var_, .. } => {
                if !final_ && !var_ && ty.is_none() {
                    // A loose `x`: in an assignment it is an
                    // `AssignedVariablePattern`; in a declaration, a `DeclaredVariablePattern`.
                    return self.leaf("DeclaredVariablePattern", s);
                }
                let t = ty.and_then(|t| self.ty(t));
                self.node("DeclaredVariablePattern", s.start, s.end, t.into_iter().collect())
            }
            PatternKind::Constant(e) => {
                let f = self.expr(*e);
                self.node("ConstantPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Relational { value, .. } => {
                let f = self.expr(*value);
                self.node("RelationalPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Or(l, r) | PatternKind::And(l, r) => {
                let kind = if matches!(x.kind, PatternKind::Or(..)) { "LogicalOrPattern" } else { "LogicalAndPattern" };
                let a1 = self.pattern(*l);
                let b1 = self.pattern(*r);
                self.node(kind, s.start, s.end, a1.into_iter().chain(b1).collect())
            }
            PatternKind::NullCheck(i) | PatternKind::NullAssert(i) => {
                let kind = if matches!(x.kind, PatternKind::NullCheck(_)) { "NullCheckPattern" } else { "NullAssertPattern" };
                let f = self.pattern(*i);
                self.node(kind, s.start, s.end, f.into_iter().collect())
            }
            PatternKind::Cast { pattern, ty } => {
                let f = self.pattern(*pattern);
                let t = self.ty(*ty);
                self.node("CastPattern", s.start, s.end, f.into_iter().chain(t).collect())
            }
            PatternKind::Parenthesized(i) => {
                let f = self.pattern(*i);
                self.node("ParenthesizedPattern", s.start, s.end, f.into_iter().collect())
            }
            PatternKind::List { type_args, elements } => {
                let mut f = Vec::new();
                f.extend(self.type_args(type_args));
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(q) => f.extend(self.pattern(*q)),
                        ListPatternElement::Rest(q) => {
                            let end = q.map(|q| self.a.pattern(q).span.end);
                            let start = q.map(|q| self.a.pattern(q).span.start);
                            let three = start
                                .and_then(|i| self.t.op_before(i, Op::Ellipsis))
                                .map(|i| self.t.span(i));
                            let inner = q.and_then(|q| self.pattern(q));
                            if let Some(ts) = three {
                                f.extend(self.node("RestPatternElement", ts.start, end.unwrap_or(ts.end), inner.into_iter().collect()));
                            }
                        }
                    }
                }
                self.node("ListPattern", s.start, s.end, f)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let mut f = Vec::new();
                f.extend(self.type_args(type_args));
                for en in entries.iter() {
                    let k = self.expr(en.key);
                    let v = self.pattern(en.value);
                    let start = self.a.expr(en.key).span.start;
                    let end = self.a.pattern(en.value).span.end;
                    f.extend(self.node("MapPatternEntry", start, end, k.into_iter().chain(v).collect()));
                }
                self.node("MapPattern", s.start, s.end, f)
            }
            PatternKind::Record { fields } => {
                let f: Vec<usize> = fields.iter().filter_map(|c| self.pattern_field(c)).collect();
                self.node("RecordPattern", s.start, s.end, f)
            }
            PatternKind::Object { ty, fields } => {
                let mut f = Vec::new();
                f.extend(self.ty(*ty));
                for c in fields.iter() {
                    f.extend(self.pattern_field(c));
                }
                self.node("ObjectPattern", s.start, s.end, f)
            }
        }
    }

    fn pattern_field(&mut self, c: &'a ast::PatternField) -> Option<usize> {
        let ps = self.a.pattern(c.pattern).span;
        let mut f = Vec::new();
        // `PatternFieldName`: `name:` or `:` (implicit name).
        let colon = match c.name {
            Some(n) => self.t.op_after(n.span.end, Op::Colon).map(|i| (n.span.start, self.t.span(i).end)),
            None => self.t.op_before(ps.start, Op::Colon).map(|i| (self.t.span(i).start, self.t.span(i).end)),
        };
        let has_name = c.name.is_some() || c.span.start < ps.start;
        if has_name && let Some((a, b)) = colon {
            f.extend(self.node("PatternFieldName", a, b, Vec::new()));
        }
        f.extend(self.pattern(c.pattern));
        self.node("PatternField", c.span.start, c.span.end, f)
    }
}

/// The selection ranges at `offset`: the `NodeLocator` node and its ancestors,
/// without the unit, without repeating the previous range
/// (`DartSelectionRangeComputer`).
pub fn selecoes(text: &str, features: dartforge_frontend::LibraryFeatures, offset: usize) -> Vec<Span> {
    let before_3_0 = features.versao().major < 3;
    let mut names = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    let arvore = construir(text, &parsed.ast, &parsed.unit, before_3_0);
    let Some(node) = arvore.localizar(offset, offset) else { return Vec::new() };
    let mut v: Vec<Span> = Vec::new();
    for n in arvore.cadeia(node) {
        let s = arvore.span(n);
        if v.last() != Some(&s) {
            v.push(s);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(text: &str, cursor: &str) -> Vec<String> {
        let offset = text.find(cursor).expect("cursor") + cursor.len() / 2;
        let features = dartforge_frontend::LibraryFeatures::new(dartforge_frontend::LanguageVersion::ATUAL, &[]);
        selecoes(text, features, offset).into_iter().map(|s| text[s.start..s.end].to_string()).collect()
    }

    #[test]
    fn param_list_with_parens() {
        let t = "int soma(int a, {int b = 0}) => a + b;\n";
        let c = chain(t, "int a");
        assert_eq!(c[0], "int");
        assert!(c.contains(&"int a".to_string()));
        assert!(c.contains(&"(int a, {int b = 0})".to_string()));
    }

    #[test]
    fn field_variable_list() {
        let t = "class P {\n  final double y = 1;\n}\n";
        let c = chain(t, "double");
        assert!(c.contains(&"final double y = 1".to_string()));
        assert!(c.contains(&"final double y = 1;".to_string()));
    }

    #[test]
    fn expression_body_with_semicolon() {
        let t = "int f() => 1 + 2;\n";
        let c = chain(t, "1 +");
        assert!(c.contains(&"=> 1 + 2;".to_string()));
    }

    #[test]
    fn annotation_name_and_annotation() {
        let t = "class A {\n  @override\n  String toString() => '';\n}\n";
        let c = chain(t, "override");
        assert_eq!(c[0], "override");
        assert_eq!(c[1], "@override");
    }
}

/// The `Expression` subclasses the tree produces.
pub const EXPRESSOES: &[&str] = &[
    "SimpleIdentifier",
    "PrefixedIdentifier",
    "MethodInvocation",
    "PropertyAccess",
    "IntegerLiteral",
    "DoubleLiteral",
    "BooleanLiteral",
    "NullLiteral",
    "SimpleStringLiteral",
    "StringInterpolation",
    "AdjacentStrings",
    "SymbolLiteral",
    "ListLiteral",
    "SetOrMapLiteral",
    "RecordLiteral",
    "InstanceCreationExpression",
    "FunctionExpression",
    "FunctionExpressionInvocation",
    "FunctionReference",
    "IndexExpression",
    "PrefixExpression",
    "PostfixExpression",
    "BinaryExpression",
    "ConditionalExpression",
    "IsExpression",
    "AsExpression",
    "AssignmentExpression",
    "PatternAssignment",
    "CascadeExpression",
    "AwaitExpression",
    "ThrowExpression",
    "RethrowExpression",
    "ThisExpression",
    "SuperExpression",
    "SwitchExpression",
    "ParenthesizedExpression",
    "NamedExpression",
    "DotShorthand",
];

/// The `Statement` subclasses.
pub const COMANDOS: &[&str] = &[
    "Block",
    "ExpressionStatement",
    "VariableDeclarationStatement",
    "PatternVariableDeclarationStatement",
    "ReturnStatement",
    "IfStatement",
    "ForStatement",
    "WhileStatement",
    "DoStatement",
    "SwitchStatement",
    "TryStatement",
    "BreakStatement",
    "ContinueStatement",
    "YieldStatement",
    "LabeledStatement",
    "EmptyStatement",
    "AssertStatement",
    "FunctionDeclarationStatement",
];

/// The kind is a subclass of `Statement`.
pub fn e_comando(kind: &str) -> bool {
    COMANDOS.contains(&kind)
}

/// The kind is a subclass of `Expression`.
pub fn e_expressao(kind: &str) -> bool {
    EXPRESSOES.contains(&kind)
}
