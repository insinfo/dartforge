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

use crate::utf16::TabelaLinhas;

/// Uma região de dobra em linhas (0-based), com a espécie LSP
/// (`comment`, `imports` ou nenhuma).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dobra {
    pub linha_inicio: u32,
    pub coluna_inicio: u32,
    pub linha_fim: u32,
    pub coluna_fim: u32,
    pub especie: Option<&'static str>,
}

/// Espécie de região do analyzer, reduzida ao que o LSP distingue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Especie {
    Comentario,
    Diretivas,
    Outra,
}

/// Uma região candidata, com a chave de visita (início e fim do nó que a
/// produz; o pai antes do filho, a ordem de produção entre iguais).
struct Candidata {
    no: (usize, Reverse<usize>, usize),
    inicio: usize,
    fim: usize,
    especie: Especie,
    alternativo: Option<usize>,
}

/// Tokens do texto com o par de cada parêntese, colchete e chave.
struct Tokens<'a> {
    fonte: &'a str,
    lista: Vec<Token>,
    par: HashMap<usize, usize>,
}

impl<'a> Tokens<'a> {
    fn novos(fonte: &'a str) -> Option<Self> {
        let lista = dartforge_frontend::lexer::lex(fonte).ok()?;
        let mut par = HashMap::new();
        let mut pilha: Vec<(Op, usize)> = Vec::new();
        for (i, t) in lista.iter().enumerate() {
            if let Kind::Op(op) = t.kind {
                match op {
                    Op::LParen | Op::LBracket | Op::LBrace => pilha.push((op, i)),
                    Op::RParen | Op::RBracket | Op::RBrace => {
                        let abre = match op {
                            Op::RParen => Op::LParen,
                            Op::RBracket => Op::LBracket,
                            _ => Op::LBrace,
                        };
                        if let Some(pos) = pilha.iter().rposition(|(o, _)| *o == abre) {
                            let (_, j) = pilha[pos];
                            pilha.truncate(pos);
                            par.insert(j, i);
                            par.insert(i, j);
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(Self { fonte, lista, par })
    }

    /// Índice do primeiro token que começa em `offset` ou depois.
    fn indice(&self, offset: usize) -> usize {
        self.lista.partition_point(|t| t.span.start < offset)
    }

    /// O primeiro token `op` em `[de, ate)`.
    fn primeiro(&self, op: Op, de: usize, ate: usize) -> Option<usize> {
        let mut i = self.indice(de);
        while let Some(t) = self.lista.get(i) {
            if t.span.start >= ate {
                return None;
            }
            if t.kind == Kind::Op(op) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// O último token `op` em `[de, ate)`.
    fn ultimo(&self, op: Op, de: usize, ate: usize) -> Option<usize> {
        let i0 = self.indice(de);
        let i1 = self.indice(ate);
        (i0..i1).rev().find(|i| self.lista[*i].kind == Kind::Op(op))
    }

    fn span(&self, i: usize) -> Span {
        self.lista[i].span
    }

    /// `(abre, fecha)` do par que começa no token `i`.
    fn par_de(&self, i: usize) -> Option<(Span, Span)> {
        let j = *self.par.get(&i)?;
        Some((self.span(i), self.span(j)))
    }

    /// Fim do último comentário entre o token anterior a `offset` e
    /// `offset` (os comentários que precedem o token em `offset`).
    fn fim_de_comentario_antes(&self, offset: usize) -> Option<usize> {
        let i = self.indice(offset);
        let de = if i == 0 { 0 } else { self.lista[i - 1].span.end };
        comentarios(self.fonte, de, offset).last().map(|c| c.fim)
    }
}

/// Um comentário no texto.
#[derive(Debug, Clone, Copy)]
struct Comentario {
    inicio: usize,
    fim: usize,
    bloco: bool,
    tripla: bool,
}

/// Os comentários de uma lacuna entre tokens.
fn comentarios(fonte: &str, de: usize, ate: usize) -> Vec<Comentario> {
    let b = fonte.as_bytes();
    let mut saida = Vec::new();
    let mut i = de;
    let ate = ate.min(b.len());
    while i < ate {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            let fim = fonte[i..ate].find(['\n', '\r']).map_or(ate, |k| i + k);
            saida.push(Comentario { inicio: i, fim, bloco: false, tripla: fonte[i..].starts_with("///") });
            i = fim;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            // Comentários de bloco aninham em Dart.
            let mut nivel = 0usize;
            let mut j = i;
            while j + 1 < ate {
                if b[j] == b'/' && b[j + 1] == b'*' {
                    nivel += 1;
                    j += 2;
                } else if b[j] == b'*' && b[j + 1] == b'/' {
                    nivel -= 1;
                    j += 2;
                    if nivel == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            let fim = j.min(ate);
            saida.push(Comentario { inicio: i, fim, bloco: true, tripla: false });
            i = fim;
        } else {
            i += 1;
        }
    }
    saida
}

/// As regiões de dobra de `texto`.
pub(crate) fn dobras(texto: &str, features: LibraryFeatures, so_linhas: bool) -> Vec<Dobra> {
    let mut nomes = Interner::new();
    let analisado = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let Some(tokens) = Tokens::novos(texto) else { return Vec::new() };
    let linhas = TabelaLinhas::construir(texto);
    let mut c = Coletor { ast: &analisado.ast, t: &tokens, candidatas: Vec::new() };
    c.coletar(&analisado.unit);
    let mut candidatas = c.candidatas;
    // A ordem de visita da árvore (pré-ordem): o nó de início menor; entre
    // iguais, o maior (pai) e então a ordem de produção.
    candidatas.sort_by_key(|r| r.no);

    let linha = |o: usize| linhas.linha_de(texto, o.min(texto.len()));
    let mut ocupadas: HashSet<usize> = HashSet::new();
    let mut regioes: Vec<(usize, usize, Especie)> = Vec::new();
    let mut adicionar = |inicio: usize, fim: usize, especie: Especie, alternativo: Option<usize>, ocupadas: &mut HashSet<usize>| {
        let mut inicio = inicio;
        if ocupadas.contains(&linha(inicio)) {
            let Some(a) = alternativo else { return };
            inicio = a;
            if ocupadas.contains(&linha(inicio)) {
                return;
            }
        }
        if fim >= inicio && linha(fim) > linha(inicio) {
            regioes.push((inicio, fim, especie));
            ocupadas.insert(linha(inicio));
        }
    };
    for r in &candidatas {
        adicionar(r.inicio, r.fim, r.especie, r.alternativo, &mut ocupadas);
    }
    // Diretivas: da palavra-chave da primeira ao fim da última.
    let diretivas: Vec<&ast::Directive> = analisado
        .unit
        .directives
        .iter()
        .filter(|d| !matches!(d.kind, ast::DirectiveKind::ImportAugment { .. } | ast::DirectiveKind::AugmentLibrary { .. }))
        .collect();
    if diretivas.len() > 1 {
        let primeira = diretivas[0];
        let depois_dos_metadados = primeira.metadata.last().map_or(primeira.span.start, |a| a.span.end);
        let i = tokens.indice(depois_dos_metadados);
        if let Some(t) = tokens.lista.get(i) {
            adicionar(t.span.end, diretivas[diretivas.len() - 1].span.end, Especie::Diretivas, None, &mut ocupadas);
        }
    }
    // Comentários, lacuna por lacuna (a cadeia que precede cada token).
    let mut anterior = analisado.unit.script_tag.map_or(0, |s| s.end);
    for t in &tokens.lista {
        if t.kind == Kind::ScriptTag {
            anterior = t.span.end;
            continue;
        }
        let cadeia = comentarios(texto, anterior, t.span.start);
        let mut k = 0;
        while k < cadeia.len() {
            let atual = cadeia[k];
            let (inicio, fim, proximo) = if atual.bloco {
                let eol = texto[atual.inicio..atual.fim].find(['\r', '\n']).unwrap_or(0);
                (atual.inicio + eol, atual.fim, k + 1)
            } else {
                let mut ultimo = k;
                while ultimo + 1 < cadeia.len()
                    && !cadeia[ultimo + 1].bloco
                    && cadeia[ultimo + 1].tripla == atual.tripla
                    && linha(cadeia[ultimo + 1].inicio) - linha(cadeia[ultimo].fim) <= 1
                {
                    ultimo += 1;
                }
                (atual.fim, cadeia[ultimo].fim, ultimo + 1)
            };
            adicionar(inicio, fim, Especie::Comentario, None, &mut ocupadas);
            k = proximo;
        }
        anterior = t.span.end;
    }
    regioes.sort_by_key(|r| r.0);
    let mut saida: Vec<Dobra> = regioes
        .into_iter()
        .map(|(inicio, fim, especie)| {
            let (l0, c0) = linhas.posicao_de_offset(texto, inicio);
            let (l1, c1) = linhas.posicao_de_offset(texto, fim);
            Dobra {
                linha_inicio: l0,
                coluna_inicio: c0,
                linha_fim: l1,
                coluna_fim: c1,
                especie: match especie {
                    Especie::Comentario => Some("comment"),
                    Especie::Diretivas => Some("imports"),
                    Especie::Outra => None,
                },
            }
        })
        .collect();
    if so_linhas {
        // A que termina na linha em que a seguinte começa (sem contê-la)
        // é encurtada até a linha anterior.
        let mut i = 0;
        while i + 1 < saida.len() {
            let (atual, prox) = (&saida[i], &saida[i + 1]);
            if atual.linha_fim >= prox.linha_inicio && atual.linha_fim <= prox.linha_fim {
                let novo_fim = prox.linha_inicio.saturating_sub(1);
                if novo_fim <= atual.linha_inicio {
                    saida.remove(i);
                    i = i.saturating_sub(1);
                    continue;
                }
                saida[i].linha_fim = novo_fim;
            }
            i += 1;
        }
    }
    saida
}

/// Percorre a árvore produzindo as regiões candidatas.
struct Coletor<'a> {
    ast: &'a Ast,
    t: &'a Tokens<'a>,
    candidatas: Vec<Candidata>,
}

impl Coletor<'_> {
    fn regiao(&mut self, no: Span, inicio: usize, fim: usize, alternativo: Option<usize>) {
        let seq = self.candidatas.len();
        self.candidatas.push(Candidata {
            no: (no.start, Reverse(no.end), seq),
            inicio,
            fim,
            especie: Especie::Outra,
            alternativo,
        });
    }

    fn anotacoes(&mut self, no: Span, anotacoes: &[Annotation]) {
        let (Some(primeira), Some(ultima)) = (anotacoes.first(), anotacoes.last()) else { return };
        let Some(nome) = primeira.name.get(primeira.name.len().min(2).saturating_sub(1)) else { return };
        self.regiao(no, nome.span.end, ultima.span.end, None);
        for a in anotacoes {
            if let Some(args) = &a.arguments {
                self.argumentos(args);
            }
        }
    }

    /// Lista de argumentos: entre os parênteses; alternativo, o primeiro
    /// argumento.
    fn argumentos(&mut self, a: &Arguments) {
        let (ast, toks) = (self.ast, self.t);
        let s = a.span;
        if s.end <= s.start || toks.fonte.as_bytes().get(s.end - 1) != Some(&b')') {
            return;
        }
        let primeiro = a.args.first().map(|x| x.name.map_or(ast.expr(x.value).span.start, |n| n.span.start));
        self.regiao(s, s.start + 1, s.end - 1, primeiro);
    }

    /// Lista de parâmetros a partir do primeiro `(` em `[ancora, limite)`.
    fn parametros(&mut self, ancora: usize, limite: usize, ps: &[Parameter]) {
        let toks = self.t;
        let Some(i) = toks.primeiro(Op::LParen, ancora, limite) else { return };
        let Some((abre, fecha)) = toks.par_de(i) else { return };
        self.regiao(Span { start: abre.start, end: fecha.end }, abre.end, fecha.start, ps.first().map(|p| p.span.start));
        for p in ps {
            self.parametro(p);
        }
    }

    fn parametro(&mut self, p: &Parameter) {
        if let Some(fps) = &p.function_parameters {
            let ancora = p.name.map_or(p.span.start, |n| n.span.end);
            self.parametros(ancora, p.span.end, fps);
        }
        if let Some(ty) = p.ty {
            self.tipo(ty);
        }
    }

    fn tipo(&mut self, ty: ast::TypeId) {
        let (ast, toks) = (self.ast, self.t);
        let t = ast.ty(ty);
        if let TypeKind::Function { parameters, .. } = &t.kind {
            // A lista vem depois da palavra `Function` (e dos parâmetros de tipo).
            let mut i = toks.indice(t.span.start);
            while let Some(tok) = toks.lista.get(i) {
                if tok.span.start >= t.span.end {
                    return;
                }
                if tok.text(toks.fonte) == "Function" {
                    break;
                }
                i += 1;
            }
            let Some(tok) = toks.lista.get(i) else { return };
            self.parametros(tok.span.end, t.span.end, parameters);
        }
    }

    /// Bloco de `if`/`while`/`do`: do fim do `{` ao fim do último
    /// comentário antes do `}`, senão ao fim do último comando.
    fn bloco_condicional(&mut self, no: Span, bloco: ast::StmtId) {
        let (ast, toks) = (self.ast, self.t);
        let b = ast.stmt(bloco);
        let StmtKind::Block(cmds) = &b.kind else { return };
        let s = b.span;
        if s.end <= s.start {
            return;
        }
        let inicio = s.start + 1;
        if let Some(fim) = toks.fim_de_comentario_antes(s.end - 1).filter(|f| *f > inicio) {
            self.regiao(no, inicio, fim, None);
        } else if let Some(ultimo) = cmds.last() {
            self.regiao(no, inicio, ast.stmt(*ultimo).span.end, None);
        }
    }

    /// Corpo de `{` a `}` de uma expressão de função ou de um `for`.
    fn bloco_inteiro(&mut self, no: Span, bloco: ast::StmtId) {
        let ast = self.ast;
        let b = ast.stmt(bloco);
        if matches!(b.kind, StmtKind::Block(_)) {
            self.regiao(no, b.span.start, b.span.end, None);
        }
    }

    fn coletar(&mut self, unidade: &ast::CompilationUnit) {
        let ast = self.ast;
        let toks = self.t;
        for d in &unidade.directives {
            for a in &d.metadata {
                if let Some(args) = &a.arguments {
                    self.argumentos(args);
                }
            }
        }
        // Funções que são `FunctionExpression` no analyzer (todas menos os
        // métodos) e as que são declarações (topo e locais).
        let mut metodos: HashSet<u32> = HashSet::new();
        for m in &ast.members {
            if let MemberKind::Method(f) = m.kind {
                metodos.insert(f.0);
            }
        }
        for d in &ast.decls {
            self.anotacoes(d.span, &d.metadata);
            match &d.kind {
                DeclKind::Class(k) if !k.mixin_application => self.regiao(d.span, k.name.span.end, d.span.end, None),
                DeclKind::Mixin(k) => self.regiao(d.span, k.name.span.end, d.span.end, None),
                DeclKind::Enum(k) => {
                    self.corpo_entre_chaves(d.span, k.name.span.end);
                    for c in &k.constants {
                        self.anotacoes(c.span, &c.metadata);
                        if let Some(a) = &c.arguments {
                            self.argumentos(a);
                        }
                    }
                }
                DeclKind::Extension(k) => {
                    let ancora = ast.ty(k.on).span.end;
                    self.corpo_entre_chaves(d.span, ancora);
                }
                DeclKind::ExtensionType(k) => {
                    let ancora = k.representation_span.end;
                    self.corpo_entre_chaves(d.span, ancora);
                }
                DeclKind::Typedef(k) => match &k.kind {
                    ast::TypedefKind::Alias(t) => self.tipo(*t),
                    ast::TypedefKind::Legacy { parameters, .. } => self.parametros(k.name.span.end, d.span.end, parameters),
                },
                DeclKind::Function(f) => {
                    let func = ast.function(*f);
                    if let Some(n) = func.name {
                        self.regiao(d.span, n.span.end, d.span.end, None);
                    }
                }
                DeclKind::Variables(v) => {
                    if let Some(t) = v.ty {
                        self.tipo(t);
                    }
                }
                _ => {}
            }
        }
        for m in &ast.members {
            self.anotacoes(m.span, &m.metadata);
            match &m.kind {
                MemberKind::Method(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.regiao(m.span, n.span.end, m.span.end, None);
                    }
                }
                MemberKind::Constructor(k) => {
                    let ancora = k.name.unwrap_or(k.class_name).span.end;
                    self.regiao(m.span, ancora, m.span.end, None);
                    self.parametros(ancora, m.span.end, &k.parameters);
                    for ini in k.initializers.iter() {
                        match ini {
                            Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } => self.argumentos(arguments),
                            Initializer::Assert { span, condition, .. } => {
                                if let Some(i) = toks.primeiro(Op::LParen, span.start, span.end)
                                    && let Some((abre, fecha)) = toks.par_de(i)
                                {
                                    let c = ast.expr(*condition).span.start;
                                    self.regiao(*span, abre.end, fecha.start, Some(c));
                                }
                            }
                            Initializer::Field { .. } => {}
                        }
                    }
                }
                MemberKind::Field(v) => {
                    if let Some(t) = v.ty {
                        self.tipo(t);
                    }
                }
            }
        }
        for (i, f) in ast.functions.iter().enumerate() {
            let ancora = f.name.map_or(f.span.start, |n| n.span.end);
            if !metodos.contains(&(i as u32))
                && let FunctionBody::Block(b) = f.body
            {
                // O nó da expressão de função começa nos parâmetros.
                let no_inicio = toks.primeiro(Op::LParen, ancora, f.span.end).map_or(f.span.start, |k| toks.span(k).start);
                self.bloco_inteiro(Span { start: no_inicio, end: f.span.end }, b);
            }
            if let Some(ps) = &f.parameters {
                self.parametros(ancora, f.span.end, ps);
            }
            if let Some(t) = f.return_type {
                self.tipo(t);
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::If { then, else_, .. } => {
                    self.bloco_condicional(s.span, *then);
                    if let Some(e) = else_ {
                        self.bloco_condicional(s.span, *e);
                    }
                }
                StmtKind::While { body, .. } | StmtKind::DoWhile { body, .. } => self.bloco_condicional(s.span, *body),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } => self.bloco_inteiro(s.span, *body),
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.regiao(s.span, n.span.end, s.span.end, None);
                    }
                }
                StmtKind::Switch { cases, .. } => {
                    if let Some(i) = toks.primeiro(Op::LBrace, s.span.start, s.span.end)
                        && let Some((abre, fecha)) = toks.par_de(i)
                    {
                        self.regiao(s.span, abre.end, fecha.end, None);
                    }
                    for caso in cases.iter() {
                        let ate = caso.body.first().map_or(caso.span.end, |b| ast.stmt(*b).span.start);
                        if let Some(i) = toks.ultimo(Op::Colon, caso.span.start, ate) {
                            self.regiao(caso.span, toks.span(i).end, caso.span.end, None);
                        }
                    }
                }
                StmtKind::Assert { .. } => {
                    if let Some(i) = toks.primeiro(Op::LParen, s.span.start, s.span.end)
                        && let Some((abre, fecha)) = toks.par_de(i)
                    {
                        self.regiao(s.span, abre.end, fecha.start, None);
                    }
                }
                StmtKind::Variables(v) => {
                    if let Some(t) = v.ty {
                        self.tipo(t);
                    }
                }
                _ => {}
            }
        }
        for e in &ast.exprs {
            let s = e.span;
            match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => self.argumentos(arguments),
                ExprKind::List { elements, .. } => self.literal(s, Op::LBracket, elements),
                ExprKind::SetOrMap { elements, .. } => self.literal(s, Op::LBrace, elements),
                ExprKind::Record { .. } => {
                    if let Some(i) = toks.primeiro(Op::LParen, s.start, s.end)
                        && let Some((abre, fecha)) = toks.par_de(i)
                    {
                        self.regiao(s, abre.end, fecha.start, None);
                    }
                }
                ExprKind::Switch { cases, .. } => {
                    if let Some(i) = toks.primeiro(Op::LBrace, s.start, s.end)
                        && let Some((abre, fecha)) = toks.par_de(i)
                    {
                        self.regiao(s, abre.end, fecha.end, None);
                    }
                    for caso in cases.iter() {
                        let corpo = ast.expr(caso.body).span.start;
                        if let Some(i) = toks.ultimo(Op::Arrow, caso.span.start, corpo) {
                            self.regiao(caso.span, toks.span(i).end, caso.span.end, None);
                        }
                    }
                }
                ExprKind::As { ty, .. } | ExprKind::Is { ty, .. } => self.tipo(*ty),
                _ => {}
            }
        }
        // Strings de várias linhas (simples ou interpoladas), cada literal
        // adjacente por si.
        let mut pilha: Vec<usize> = Vec::new();
        for tok in &toks.lista {
            match tok.kind {
                Kind::Str(_) => self.regiao(tok.span, tok.span.start, tok.span.end, None),
                Kind::StrBegin(..) => pilha.push(tok.span.start),
                Kind::StrEnd(_) => {
                    if let Some(ini) = pilha.pop() {
                        self.regiao(Span { start: ini, end: tok.span.end }, ini, tok.span.end, None);
                    }
                }
                _ => {}
            }
        }
    }

    /// Corpo de enum, extensão ou extension type: do fim do `{` ao início
    /// do `}`.
    fn corpo_entre_chaves(&mut self, no: Span, ancora: usize) {
        let toks = self.t;
        if let Some(i) = toks.primeiro(Op::LBrace, ancora, no.end)
            && let Some((abre, fecha)) = toks.par_de(i)
        {
            self.regiao(no, abre.end, fecha.start, None);
        }
    }

    fn literal(&mut self, s: Span, abre: Op, _elementos: &[CollectionElement]) {
        let toks = self.t;
        if let Some(i) = toks.primeiro(abre, s.start, s.end)
            && let Some((a, f)) = toks.par_de(i)
        {
            self.regiao(s, a.end, f.start, None);
        }
    }
}

/// As faixas de seleção em `offset`: os nós da árvore que o contêm, do mais
/// interno ao mais externo, sem repetir intervalos iguais
/// (`DartSelectionRangeComputer`).
pub(crate) fn selecoes(texto: &str, features: LibraryFeatures, offset: usize) -> Vec<Span> {
    // A árvore no formato do analyzer e o `NodeLocator` (§7.2 a §7.6).
    crate::arvore_analyzer::selecoes(texto, features, offset)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn linhas_de(texto: &str) -> Vec<(u32, u32, Option<&'static str>)> {
        dobras(texto, LibraryFeatures::new(dartforge_frontend::LanguageVersion::ATUAL, &[]), true)
            .into_iter()
            .map(|d| (d.linha_inicio, d.linha_fim, d.especie))
            .collect()
    }

    #[test]
    fn classe_metodo_comentario_e_diretivas() {
        let texto = "import 'a.dart';\nimport 'b.dart';\n\n/// Doc\n/// mais\nclass A {\n  void m() {\n    print(1);\n  }\n}\n";
        assert_eq!(
            linhas_de(texto),
            vec![(0, 1, Some("imports")), (3, 4, Some("comment")), (5, 9, None), (6, 8, None)]
        );
    }

    #[test]
    fn argumentos_em_varias_linhas() {
        let texto = "void main() {\n  f(\n    1,\n    2,\n  );\n}\n";
        assert_eq!(linhas_de(texto), vec![(0, 5, None), (1, 4, None)]);
    }
}
