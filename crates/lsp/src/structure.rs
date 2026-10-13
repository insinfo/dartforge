//! Estrutura sintática do documento: regiões de dobra
//! (`textDocument/foldingRange`) e faixas de seleção
//! (`textDocument/selectionRange`), só da árvore do parser e dos tokens do
//! texto vigente (nenhum programa carregado).
//!
//! As dobras seguem o `DartUnitFoldingComputer` do Dart 3.6.2
//! (`pkg/analysis_server/lib/src/computer/computer_folding.dart`) e o
//! `FoldingHandler` (`lsp/handlers/handler_folding.dart`): cada nó contribui
//! com uma região (corpo de classe a partir do fim do nome, corpo de função
//! a partir do fim do nome, listas de argumentos e de parâmetros entre os
//! parênteses, literais entre os colchetes, blocos de `if`/`while`/`do` até
//! o último comando, `for` e *closures* do `{` ao `}`, casos de `switch`,
//! strings de várias linhas, anotações), na ordem de visita da árvore; duas
//! regiões não começam na mesma linha (a segunda usa o início alternativo,
//! quando há, ou é descartada); depois vêm as diretivas (da palavra-chave
//! da primeira ao fim da última) e os comentários (`/* */` a partir do fim
//! da primeira linha; `//` e `///` consecutivos, sem linha em branco, do fim
//! do primeiro ao fim do último). Ordenadas pelo início; com
//! `lineFoldingOnly`, a região que termina na linha em que a seguinte começa
//! é encurtada até a linha anterior (e some se ficar com uma linha só).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Annotation, Arguments, Ast, CollectionElement, DeclKind, ExprKind, FunctionBody,
    Initializer, MemberKind, Parameter, StmtKind, TypeKind,
};
use dartforge_frontend::token::{Kind, Op, Token};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use crate::utf16::LineTable;

/// Uma região de dobra em linhas (0-based), com a espécie LSP
/// (`comment`, `imports` ou nenhuma).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold {
    pub line_begin: u32,
    pub column_start: u32,
    pub line_end: u32,
    pub column_end: u32,
    pub kind: Option<&'static str>,
}

/// Espécie de região do analyzer, reduzida ao que o LSP distingue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoldKind {
    Comment,
    Directives,
    Other,
}

/// Uma região candidata, com a chave de visita (início e fim do nó que a
/// produz; o pai antes do filho, a ordem de produção entre iguais).
struct Candidate {
    no: (usize, Reverse<usize>, usize),
    start: usize,
    end: usize,
    kind: FoldKind,
    alternative: Option<usize>,
}

/// Tokens do texto com o par de cada parêntese, colchete e chave.
struct Tokens<'a> {
    source: &'a str,
    list: Vec<Token>,
    pair: HashMap<usize, usize>,
}

impl<'a> Tokens<'a> {
    fn new(source: &'a str) -> Option<Self> {
        let list = dartforge_frontend::lexer::lex(source).ok()?;
        let mut pair = HashMap::new();
        let mut stack: Vec<(Op, usize)> = Vec::new();
        for (i, t) in list.iter().enumerate() {
            if let Kind::Op(op) = t.kind {
                match op {
                    Op::LParen | Op::LBracket | Op::LBrace => stack.push((op, i)),
                    Op::RParen | Op::RBracket | Op::RBrace => {
                        let opens = match op {
                            Op::RParen => Op::LParen,
                            Op::RBracket => Op::LBracket,
                            _ => Op::LBrace,
                        };
                        if let Some(pos) = stack.iter().rposition(|(o, _)| *o == opens) {
                            let (_, j) = stack[pos];
                            stack.truncate(pos);
                            pair.insert(j, i);
                            pair.insert(i, j);
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(Self { source, list, pair })
    }

    /// Índice do primeiro token que começa em `offset` ou depois.
    fn index(&self, offset: usize) -> usize {
        self.list.partition_point(|t| t.span.start < offset)
    }

    /// O primeiro token `op` em `[de, until)`.
    fn first(&self, op: Op, de: usize, until: usize) -> Option<usize> {
        let mut i = self.index(de);
        while let Some(t) = self.list.get(i) {
            if t.span.start >= until {
                return None;
            }
            if t.kind == Kind::Op(op) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// O último token `op` em `[de, until)`.
    fn last(&self, op: Op, de: usize, until: usize) -> Option<usize> {
        let i0 = self.index(de);
        let i1 = self.index(until);
        (i0..i1).rev().find(|i| self.list[*i].kind == Kind::Op(op))
    }

    fn span(&self, i: usize) -> Span {
        self.list[i].span
    }

    /// `(opens, closes)` do par que começa no token `i`.
    fn pair_of(&self, i: usize) -> Option<(Span, Span)> {
        let j = *self.pair.get(&i)?;
        Some((self.span(i), self.span(j)))
    }

    /// Fim do último comentário entre o token anterior a `offset` e
    /// `offset` (os comentários que precedem o token em `offset`).
    fn comment_end_before(&self, offset: usize) -> Option<usize> {
        let i = self.index(offset);
        let de = if i == 0 { 0 } else { self.list[i - 1].span.end };
        comments(self.source, de, offset).last().map(|c| c.end)
    }
}

/// Um comentário no texto.
#[derive(Debug, Clone, Copy)]
struct Comment {
    start: usize,
    end: usize,
    block: bool,
    triple: bool,
}

/// Os comentários de uma lacuna entre tokens.
fn comments(source: &str, de: usize, until: usize) -> Vec<Comment> {
    let b = source.as_bytes();
    let mut output = Vec::new();
    let mut i = de;
    let until = until.min(b.len());
    while i < until {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = source[i..until].find(['\n', '\r']).map_or(until, |k| i + k);
            output.push(Comment { start: i, end, block: false, triple: source[i..].starts_with("///") });
            i = end;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            // Comentários de bloco aninham em Dart.
            let mut level = 0usize;
            let mut j = i;
            while j + 1 < until {
                if b[j] == b'/' && b[j + 1] == b'*' {
                    level += 1;
                    j += 2;
                } else if b[j] == b'*' && b[j + 1] == b'/' {
                    level -= 1;
                    j += 2;
                    if level == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            let end = j.min(until);
            output.push(Comment { start: i, end, block: true, triple: false });
            i = end;
        } else {
            i += 1;
        }
    }
    output
}

/// As regiões de dobra de `text`.
pub(crate) fn folds(text: &str, features: LibraryFeatures, lines_only: bool) -> Vec<Fold> {
    let mut names = Interner::new();
    let analyzed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    let Some(tokens) = Tokens::new(text) else { return Vec::new() };
    let lines = LineTable::build(text);
    let mut c = Collector { ast: &analyzed.ast, t: &tokens, candidates: Vec::new() };
    c.collect(&analyzed.unit);
    let mut candidates = c.candidates;
    // A ordem de visita da árvore (pré-ordem): o nó de início menor; entre
    // iguais, o maior (pai) e então a ordem de produção.
    candidates.sort_by_key(|r| r.no);

    let line = |o: usize| lines.line_of(text, o.min(text.len()));
    let mut occupied: HashSet<usize> = HashSet::new();
    let mut regions: Vec<(usize, usize, FoldKind)> = Vec::new();
    let mut add = |start: usize, end: usize, kind: FoldKind, alternative: Option<usize>, occupied: &mut HashSet<usize>| {
        let mut start = start;
        if occupied.contains(&line(start)) {
            let Some(a) = alternative else { return };
            start = a;
            if occupied.contains(&line(start)) {
                return;
            }
        }
        if end >= start && line(end) > line(start) {
            regions.push((start, end, kind));
            occupied.insert(line(start));
        }
    };
    for r in &candidates {
        add(r.start, r.end, r.kind, r.alternative, &mut occupied);
    }
    // Diretivas: da palavra-chave da primeira ao fim da última.
    let directives: Vec<&ast::Directive> = analyzed
        .unit
        .directives
        .iter()
        .filter(|d| !matches!(d.kind, ast::DirectiveKind::ImportAugment { .. } | ast::DirectiveKind::AugmentLibrary { .. }))
        .collect();
    if directives.len() > 1 {
        let first = directives[0];
        let after_metadata = first.metadata.last().map_or(first.span.start, |a| a.span.end);
        let i = tokens.index(after_metadata);
        if let Some(t) = tokens.list.get(i) {
            add(t.span.end, directives[directives.len() - 1].span.end, FoldKind::Directives, None, &mut occupied);
        }
    }
    // Comentários, lacuna por lacuna (a cadeia que precede cada token).
    let mut previous = analyzed.unit.script_tag.map_or(0, |s| s.end);
    for t in &tokens.list {
        if t.kind == Kind::ScriptTag {
            previous = t.span.end;
            continue;
        }
        let chain = comments(text, previous, t.span.start);
        let mut k = 0;
        while k < chain.len() {
            let current = chain[k];
            let (start, end, next_) = if current.block {
                let eol = text[current.start..current.end].find(['\r', '\n']).unwrap_or(0);
                (current.start + eol, current.end, k + 1)
            } else {
                let mut last = k;
                while last + 1 < chain.len()
                    && !chain[last + 1].block
                    && chain[last + 1].triple == current.triple
                    && line(chain[last + 1].start) - line(chain[last].end) <= 1
                {
                    last += 1;
                }
                (current.end, chain[last].end, last + 1)
            };
            add(start, end, FoldKind::Comment, None, &mut occupied);
            k = next_;
        }
        previous = t.span.end;
    }
    regions.sort_by_key(|r| r.0);
    let mut output: Vec<Fold> = regions
        .into_iter()
        .map(|(start, end, kind)| {
            let (l0, c0) = lines.offset_position(text, start);
            let (l1, c1) = lines.offset_position(text, end);
            Fold {
                line_begin: l0,
                column_start: c0,
                line_end: l1,
                column_end: c1,
                kind: match kind {
                    FoldKind::Comment => Some("comment"),
                    FoldKind::Directives => Some("imports"),
                    FoldKind::Other => None,
                },
            }
        })
        .collect();
    if lines_only {
        // A que termina na linha em que a seguinte começa (sem contê-la)
        // é encurtada até a linha anterior.
        let mut i = 0;
        while i + 1 < output.len() {
            let (current, next) = (&output[i], &output[i + 1]);
            if current.line_end >= next.line_begin && current.line_end <= next.line_end {
                let new_end = next.line_begin.saturating_sub(1);
                if new_end <= current.line_begin {
                    output.remove(i);
                    i = i.saturating_sub(1);
                    continue;
                }
                output[i].line_end = new_end;
            }
            i += 1;
        }
    }
    output
}

/// Percorre a árvore produzindo as regiões candidatas.
struct Collector<'a> {
    ast: &'a Ast,
    t: &'a Tokens<'a>,
    candidates: Vec<Candidate>,
}

impl Collector<'_> {
    fn region(&mut self, no: Span, start: usize, end: usize, alternative: Option<usize>) {
        let seq = self.candidates.len();
        self.candidates.push(Candidate {
            no: (no.start, Reverse(no.end), seq),
            start,
            end,
            kind: FoldKind::Other,
            alternative,
        });
    }

    fn annotations(&mut self, no: Span, annotations: &[Annotation]) {
        let (Some(first), Some(last)) = (annotations.first(), annotations.last()) else { return };
        let Some(name) = first.name.get(first.name.len().min(2).saturating_sub(1)) else { return };
        self.region(no, name.span.end, last.span.end, None);
        for a in annotations {
            if let Some(args) = &a.arguments {
                self.arguments(args);
            }
        }
    }

    /// Lista de argumentos: entre os parênteses; alternativo, o primeiro
    /// argumento.
    fn arguments(&mut self, a: &Arguments) {
        let (ast, toks) = (self.ast, self.t);
        let s = a.span;
        if s.end <= s.start || toks.source.as_bytes().get(s.end - 1) != Some(&b')') {
            return;
        }
        let first = a.args.first().map(|x| x.name.map_or(ast.expr(x.value).span.start, |n| n.span.start));
        self.region(s, s.start + 1, s.end - 1, first);
    }

    /// Lista de parâmetros a partir do primeiro `(` em `[anchor, limit)`.
    fn parameters(&mut self, anchor: usize, limit: usize, ps: &[Parameter]) {
        let toks = self.t;
        let Some(i) = toks.first(Op::LParen, anchor, limit) else { return };
        let Some((opens, closes)) = toks.pair_of(i) else { return };
        self.region(Span { start: opens.start, end: closes.end }, opens.end, closes.start, ps.first().map(|p| p.span.start));
        for p in ps {
            self.parameter(p);
        }
    }

    fn parameter(&mut self, p: &Parameter) {
        if let Some(fps) = &p.function_parameters {
            let anchor = p.name.map_or(p.span.start, |n| n.span.end);
            self.parameters(anchor, p.span.end, fps);
        }
        if let Some(ty) = p.ty {
            self.ty(ty);
        }
    }

    fn ty(&mut self, ty: ast::TypeId) {
        let (ast, toks) = (self.ast, self.t);
        let t = ast.ty(ty);
        if let TypeKind::Function { parameters, .. } = &t.kind {
            // A lista vem depois da palavra `Function` (e dos parâmetros de tipo).
            let mut i = toks.index(t.span.start);
            while let Some(tok) = toks.list.get(i) {
                if tok.span.start >= t.span.end {
                    return;
                }
                if tok.text(toks.source) == "Function" {
                    break;
                }
                i += 1;
            }
            let Some(tok) = toks.list.get(i) else { return };
            self.parameters(tok.span.end, t.span.end, parameters);
        }
    }

    /// Bloco de `if`/`while`/`do`: do fim do `{` ao fim do último
    /// comentário antes do `}`, senão ao fim do último comando.
    fn conditional_block(&mut self, no: Span, block: ast::StmtId) {
        let (ast, toks) = (self.ast, self.t);
        let b = ast.stmt(block);
        let StmtKind::Block(cmds) = &b.kind else { return };
        let s = b.span;
        if s.end <= s.start {
            return;
        }
        let start = s.start + 1;
        if let Some(end) = toks.comment_end_before(s.end - 1).filter(|f| *f > start) {
            self.region(no, start, end, None);
        } else if let Some(last) = cmds.last() {
            self.region(no, start, ast.stmt(*last).span.end, None);
        }
    }

    /// Corpo de `{` a `}` de uma expressão de função ou de um `for`.
    fn whole_block(&mut self, no: Span, block: ast::StmtId) {
        let ast = self.ast;
        let b = ast.stmt(block);
        if matches!(b.kind, StmtKind::Block(_)) {
            self.region(no, b.span.start, b.span.end, None);
        }
    }

    fn collect(&mut self, unit: &ast::CompilationUnit) {
        let ast = self.ast;
        let toks = self.t;
        for d in &unit.directives {
            for a in &d.metadata {
                if let Some(args) = &a.arguments {
                    self.arguments(args);
                }
            }
        }
        // Funções que são `FunctionExpression` no analyzer (todas menos os
        // métodos) e as que são declarações (topo e locais).
        let mut methods: HashSet<u32> = HashSet::new();
        for m in &ast.members {
            if let MemberKind::Method(f) = m.kind {
                methods.insert(f.0);
            }
        }
        for d in &ast.decls {
            self.annotations(d.span, &d.metadata);
            match &d.kind {
                DeclKind::Class(k) if !k.mixin_application => self.region(d.span, k.name.span.end, d.span.end, None),
                DeclKind::Mixin(k) => self.region(d.span, k.name.span.end, d.span.end, None),
                DeclKind::Enum(k) => {
                    self.body_in_braces(d.span, k.name.span.end);
                    for c in &k.constants {
                        self.annotations(c.span, &c.metadata);
                        if let Some(a) = &c.arguments {
                            self.arguments(a);
                        }
                    }
                }
                DeclKind::Extension(k) => {
                    let anchor = ast.ty(k.on).span.end;
                    self.body_in_braces(d.span, anchor);
                }
                DeclKind::ExtensionType(k) => {
                    let anchor = k.representation_span.end;
                    self.body_in_braces(d.span, anchor);
                }
                DeclKind::Typedef(k) => match &k.kind {
                    ast::TypedefKind::Alias(t) => self.ty(*t),
                    ast::TypedefKind::Legacy { parameters, .. } => self.parameters(k.name.span.end, d.span.end, parameters),
                },
                DeclKind::Function(f) => {
                    let func = ast.function(*f);
                    if let Some(n) = func.name {
                        self.region(d.span, n.span.end, d.span.end, None);
                    }
                }
                DeclKind::Variables(v) => {
                    if let Some(t) = v.ty {
                        self.ty(t);
                    }
                }
                _ => {}
            }
        }
        for m in &ast.members {
            self.annotations(m.span, &m.metadata);
            match &m.kind {
                MemberKind::Method(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.region(m.span, n.span.end, m.span.end, None);
                    }
                }
                MemberKind::Constructor(k) => {
                    let anchor = k.name.unwrap_or(k.class_name).span.end;
                    self.region(m.span, anchor, m.span.end, None);
                    self.parameters(anchor, m.span.end, &k.parameters);
                    for start in k.initializers.iter() {
                        match start {
                            Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } => self.arguments(arguments),
                            Initializer::Assert { span, condition, .. } => {
                                if let Some(i) = toks.first(Op::LParen, span.start, span.end)
                                    && let Some((opens, closes)) = toks.pair_of(i)
                                {
                                    let c = ast.expr(*condition).span.start;
                                    self.region(*span, opens.end, closes.start, Some(c));
                                }
                            }
                            Initializer::Field { .. } => {}
                        }
                    }
                }
                MemberKind::Field(v) => {
                    if let Some(t) = v.ty {
                        self.ty(t);
                    }
                }
            }
        }
        for (i, f) in ast.functions.iter().enumerate() {
            let anchor = f.name.map_or(f.span.start, |n| n.span.end);
            if !methods.contains(&(i as u32))
                && let FunctionBody::Block(b) = f.body
            {
                // O nó da expressão de função começa nos parâmetros.
                let at_start = toks.first(Op::LParen, anchor, f.span.end).map_or(f.span.start, |k| toks.span(k).start);
                self.whole_block(Span { start: at_start, end: f.span.end }, b);
            }
            if let Some(ps) = &f.parameters {
                self.parameters(anchor, f.span.end, ps);
            }
            if let Some(t) = f.return_type {
                self.ty(t);
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::If { then, else_, .. } => {
                    self.conditional_block(s.span, *then);
                    if let Some(e) = else_ {
                        self.conditional_block(s.span, *e);
                    }
                }
                StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => self.conditional_block(s.span, *body),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } => self.whole_block(s.span, *body),
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.region(s.span, n.span.end, s.span.end, None);
                    }
                }
                StmtKind::Switch { cases, .. } => {
                    if let Some(i) = toks.first(Op::LBrace, s.span.start, s.span.end)
                        && let Some((opens, closes)) = toks.pair_of(i)
                    {
                        self.region(s.span, opens.end, closes.end, None);
                    }
                    for case in cases.iter() {
                        let until = case.body.first().map_or(case.span.end, |b| ast.stmt(*b).span.start);
                        if let Some(i) = toks.last(Op::Colon, case.span.start, until) {
                            self.region(case.span, toks.span(i).end, case.span.end, None);
                        }
                    }
                }
                StmtKind::Assert { .. } => {
                    if let Some(i) = toks.first(Op::LParen, s.span.start, s.span.end)
                        && let Some((opens, closes)) = toks.pair_of(i)
                    {
                        self.region(s.span, opens.end, closes.start, None);
                    }
                }
                StmtKind::Variables(v) => {
                    if let Some(t) = v.ty {
                        self.ty(t);
                    }
                }
                _ => {}
            }
        }
        for e in &ast.exprs {
            let s = e.span;
            match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => self.arguments(arguments),
                ExprKind::List { elements, .. } => self.literal(s, Op::LBracket, elements),
                ExprKind::SetOrMap { elements, .. } => self.literal(s, Op::LBrace, elements),
                ExprKind::Record { .. } => {
                    if let Some(i) = toks.first(Op::LParen, s.start, s.end)
                        && let Some((opens, closes)) = toks.pair_of(i)
                    {
                        self.region(s, opens.end, closes.start, None);
                    }
                }
                ExprKind::Switch { cases, .. } => {
                    if let Some(i) = toks.first(Op::LBrace, s.start, s.end)
                        && let Some((opens, closes)) = toks.pair_of(i)
                    {
                        self.region(s, opens.end, closes.end, None);
                    }
                    for case in cases.iter() {
                        let body_ = ast.expr(case.body).span.start;
                        if let Some(i) = toks.last(Op::Arrow, case.span.start, body_) {
                            self.region(case.span, toks.span(i).end, case.span.end, None);
                        }
                    }
                }
                ExprKind::As { ty, .. } | ExprKind::Is { ty, .. } => self.ty(*ty),
                _ => {}
            }
        }
        // Strings de várias linhas (simples ou interpoladas), cada literal
        // adjacente por si.
        let mut stack: Vec<usize> = Vec::new();
        for tok in &toks.list {
            match tok.kind {
                Kind::Str(_) => self.region(tok.span, tok.span.start, tok.span.end, None),
                Kind::StrBegin(..) => stack.push(tok.span.start),
                Kind::StrEnd(_) => {
                    if let Some(start) = stack.pop() {
                        self.region(Span { start: start, end: tok.span.end }, start, tok.span.end, None);
                    }
                }
                _ => {}
            }
        }
    }

    /// Corpo de enum, extensão ou extension type: do fim do `{` ao início
    /// do `}`.
    fn body_in_braces(&mut self, no: Span, anchor: usize) {
        let toks = self.t;
        if let Some(i) = toks.first(Op::LBrace, anchor, no.end)
            && let Some((opens, closes)) = toks.pair_of(i)
        {
            self.region(no, opens.end, closes.start, None);
        }
    }

    fn literal(&mut self, s: Span, opens: Op, _elements: &[CollectionElement]) {
        let toks = self.t;
        if let Some(i) = toks.first(opens, s.start, s.end)
            && let Some((a, f)) = toks.pair_of(i)
        {
            self.region(s, a.end, f.start, None);
        }
    }
}

/// As faixas de seleção em `offset`: os nós da árvore que o contêm, do mais
/// interno ao mais externo, sem repetir intervalos iguais
/// (`DartSelectionRangeComputer`).
pub(crate) fn selections(text: &str, features: LibraryFeatures, offset: usize) -> Vec<Span> {
    // A árvore no formato do analyzer e o `NodeLocator` (§7.2 a §7.6).
    crate::arvore_analyzer::selecoes(text, features, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(text: &str) -> Vec<(u32, u32, Option<&'static str>)> {
        folds(text, LibraryFeatures::new(dartforge_frontend::LanguageVersion::ATUAL, &[]), true)
            .into_iter()
            .map(|d| (d.line_begin, d.line_end, d.kind))
            .collect()
    }

    #[test]
    fn class_method_comment_and_directives() {
        let text = "import 'a.dart';\nimport 'b.dart';\n\n/// Doc\n/// mais\nclass A {\n  void m() {\n    print(1);\n  }\n}\n";
        assert_eq!(
            lines_of(text),
            vec![(0, 1, Some("imports")), (3, 4, Some("comment")), (5, 9, None), (6, 8, None)]
        );
    }

    #[test]
    fn arguments_on_several_lines() {
        let text = "void main() {\n  f(\n    1,\n    2,\n  );\n}\n";
        assert_eq!(lines_of(text), vec![(0, 5, None), (1, 4, None)]);
    }
}
