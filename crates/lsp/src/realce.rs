//! Tokens semânticos (`textDocument/semanticTokens/full` e `/range`) no
//! formato do servidor do Dart 3.6.2: o `DartUnitHighlightsComputer`
//! (`pkg/analysis_server/lib/src/computer/computer_highlights.dart`) com o
//! mapa de `lsp/semantic_tokens/mapping.dart` e o `SemanticTokenEncoder`
//! (`lsp/semantic_tokens/encoder.dart`).
//!
//! * palavras reservadas e embutidas (`keyword`; `control` nas de fluxo:
//!   `if`, `for`, `return`, `await`, `async`…), `true`/`false` (`boolean`),
//!   `void` (`keyword` + `void`), números, strings (cada trecho de uma
//!   interpolada; `${…}` como `source` + `interpolation`; escapes como
//!   `string` + `escape`), comentários (`documentation` nos `///`/`/**`);
//! * cada nome pela identidade da inferência comum (`get_resolved`,
//!   namespaces): classe, enum, extensão e extension type (`class`/`enum`;
//!   `constructor` quando é o nome de uma criação), typedef (`type`),
//!   construtor nomeado (`method` + `constructor`), método (`method`),
//!   getter, setter e referência a campo ou variável de topo (`property`),
//!   declaração de campo ou variável (`variable` + `declaration`), local,
//!   parâmetro (`parameter`; rótulo `nome:` com `label`), parâmetro de tipo,
//!   constante de enum, prefixo de import (`variable` + `importPrefix`),
//!   função; `instance`/`static` e `declaration` como no Dart;
//! * anotações (`annotation` do `@` ao `(`, e o `)`), com `annotation` nos
//!   nomes dentro delas;
//! * sobreposições divididas pelo de cima (o mais interno), como o
//!   `splitOverlappingTokens`, e tokens de várias linhas quebrados por linha
//!   quando o cliente não aceita `multilineTokenSupport`.

use crate::projeto::{Projeto, declaracao_de_parametro_de_tipo, eh_prefixo};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, StmtKind, TypeKind};
use dartforge_frontend::token::{Kind, Op};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashMap;

/// Tipos de token na ordem da legenda do Dart 3.6.2 (o índice é o número
/// enviado ao cliente).
pub(crate) const TIPOS: &[&str] = &[
    "annotation", "keyword", "class", "comment", "method", "variable", "parameter", "enum", "enumMember", "type",
    "source", "property", "namespace", "boolean", "number", "string", "function", "typeParameter",
];

/// Modificadores na ordem da legenda do Dart 3.6.2 (o índice é o bit).
pub(crate) const MODIFICADORES: &[&str] = &[
    "documentation", "constructor", "declaration", "importPrefix", "instance", "static", "escape", "annotation",
    "control", "label", "interpolation", "void", "wildcard",
];

/// Um token antes da codificação: offset e fim em bytes, tipo e
/// modificadores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Realce {
    pub inicio: usize,
    pub fim: usize,
    pub tipo: &'static str,
    pub modificadores: Vec<&'static str>,
}

/// Palavras embutidas e contextuais que, fora de um nome da árvore, são
/// palavras-chave (`BUILT_IN` do analyzer).
const EMBUTIDAS: &[&str] = &[
    "abstract", "as", "augment", "async", "await", "base", "covariant", "deferred", "export", "extension", "external",
    "factory", "Function", "get", "hide", "implements", "import", "interface", "late", "library", "macro", "mixin",
    "native", "of", "on", "operator", "part", "required", "sealed", "set", "show", "static", "sync", "type", "typedef",
    "when", "yield",
];

/// Palavras de fluxo de controle (modificador `control`).
const CONTROLE: &[&str] = &[
    "assert", "await", "break", "case", "catch", "continue", "default", "do", "else", "finally", "for", "if", "in",
    "rethrow", "return", "switch", "throw", "try", "when", "while", "yield", "async", "sync",
];

struct Coletor<'a> {
    projeto: &'a Projeto,
    unidade: UnitId,
    /// Nomes classificados pela árvore, por início.
    nomes: HashMap<usize, Realce>,
    outros: Vec<Realce>,
}

impl Coletor<'_> {
    fn nome(&mut self, s: Span, tipo: &'static str, modificadores: &[&'static str]) {
        if s.end > s.start {
            self.nomes.entry(s.start).or_insert(Realce { inicio: s.start, fim: s.end, tipo, modificadores: modificadores.to_vec() });
        }
    }

    fn regiao(&mut self, inicio: usize, fim: usize, tipo: &'static str, modificadores: &[&'static str]) {
        if fim > inicio {
            self.outros.push(Realce { inicio, fim, tipo, modificadores: modificadores.to_vec() });
        }
    }

    /// Nome de tipo escrito (`A`, `p.A`, `T`, `dynamic`).
    fn tipo_escrito(&mut self, ty: ast::TypeId, extra: &[&'static str]) {
        let p = self.projeto.programa();
        let u = p.unit(self.unidade);
        let t = u.ast.ty(ty);
        let TypeKind::Named { name, .. } = &t.kind else { return };
        let lib = u.library;
        let texto = |n: &ast::Name| &u.source[n.span.start..n.span.end];
        match &name[..] {
            [n] => {
                if texto(n) == "dynamic" || texto(n) == "Never" {
                    self.nome(n.span, "type", &[]);
                    return;
                }
                if declaracao_de_parametro_de_tipo(&u.ast, n.span.start, n.sym).is_some() {
                    self.nome(n.span, "typeParameter", &[]);
                    return;
                }
                if let Some(el) = p.lookup(lib, n.sym).and_then(|b| b.getter) {
                    self.elemento(n.span, el, extra);
                }
            }
            [pf, n] if eh_prefixo(p, lib, pf.sym) => {
                self.nome(pf.span, "variable", &["importPrefix"]);
                if let Some(el) = p.lookup_prefixed(lib, pf.sym, n.sym).and_then(|b| b.getter) {
                    self.elemento(n.span, el, extra);
                }
            }
            [c, k] => {
                // `A.nome` como tipo: a classe e o construtor.
                if let Some(el) = p.lookup(lib, c.sym).and_then(|b| b.getter) {
                    self.elemento(c.span, el, extra);
                }
                let mut m = vec!["constructor"];
                m.extend_from_slice(extra);
                self.nome(k.span, "method", &m);
            }
            _ => {}
        }
    }

    /// Referência a um elemento de topo.
    fn elemento(&mut self, s: Span, el: Element, extra: &[&'static str]) {
        let p = self.projeto.programa();
        let (tipo, mods): (&'static str, Vec<&'static str>) = match el {
            Element::Class(c) => {
                let enum_ = p.class(c).decl.is_some_and(|d| matches!(p.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Enum(_)));
                (if enum_ { "enum" } else { "class" }, Vec::new())
            }
            Element::Extension(_) => ("class", Vec::new()),
            Element::Typedef(_) => ("type", Vec::new()),
            Element::Function(f) => match p.function(f).kind {
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => ("property", Vec::new()),
                _ => ("function", Vec::new()),
            },
            Element::Variable(_) => ("property", Vec::new()),
            Element::Prefix(..) => ("variable", vec!["importPrefix"]),
        };
        let mut m = mods;
        m.extend_from_slice(extra);
        self.nome(s, tipo, &m);
    }

    /// Referência resolvida numa expressão.
    fn resolvido(&mut self, s: Span, r: Option<&Resolved>, construtor: bool, extra: &[&'static str], dinamico: bool) {
        let p = self.projeto.programa();
        let com = |base: &[&'static str]| {
            let mut m = base.to_vec();
            m.extend_from_slice(extra);
            m
        };
        match r {
            Some(Resolved::Local(_)) => self.nome(s, "variable", &com(&[])),
            Some(Resolved::Parameter { .. }) => self.nome(s, "parameter", &com(&[])),
            Some(Resolved::TypeParameter(_)) => self.nome(s, "typeParameter", &com(&[])),
            Some(Resolved::Prefix(_)) => self.nome(s, "variable", &com(&["importPrefix"])),
            Some(Resolved::Element(Element::Class(c))) if construtor => {
                let _ = c;
                self.nome(s, "class", &com(&["constructor"]))
            }
            Some(Resolved::Element(el)) => self.elemento(s, *el, extra),
            Some(Resolved::Constructor(f)) => {
                let classe = p.function(*f).class.map(|c| p.class(c).name);
                let texto = &p.unit(self.unidade).source[s.start..s.end];
                if classe.is_some_and(|c| self.projeto.nome(c) == texto) {
                    self.nome(s, "class", &com(&["constructor"]));
                } else {
                    self.nome(s, "method", &com(&["constructor"]));
                }
            }
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let fe = p.function(*f);
                let escopo = if fe.static_ { "static" } else { "instance" };
                let eh_constante = fe.variable.is_some_and(|v| {
                    let ve = p.variable(v);
                    ve.class.is_some_and(|c| p.class(c).enum_constants.contains(&v))
                });
                match fe.kind {
                    _ if eh_constante => self.nome(s, "enumMember", &com(&[])),
                    FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => self.nome(s, "property", &com(&[escopo])),
                    _ => self.nome(s, "method", &com(&[escopo])),
                }
            }
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                let ve = p.variable(*v);
                if ve.class.is_some_and(|c| p.class(c).enum_constants.contains(v)) {
                    self.nome(s, "enumMember", &com(&[]));
                } else {
                    self.nome(s, "property", &com(&[if ve.static_ { "static" } else { "instance" }]));
                }
            }
            Some(Resolved::Dynamic) | None if dinamico => self.nome(s, "source", &com(&[])),
            _ => {}
        }
    }

    fn anotacoes(&mut self, anotacoes: &[ast::Annotation]) {
        let p = self.projeto.programa();
        let u = p.unit(self.unidade);
        for a in anotacoes {
            match &a.arguments {
                None => self.regiao(a.span.start, a.span.end, "annotation", &[]),
                Some(args) => {
                    self.regiao(a.span.start, args.span.start + 1, "annotation", &[]);
                    if args.span.end > args.span.start && u.source.as_bytes().get(args.span.end - 1) == Some(&b')') {
                        self.regiao(args.span.end - 1, args.span.end, "annotation", &[]);
                    }
                }
            }
            let lib = u.library;
            match &a.name[..] {
                [n] => {
                    if let Some(el) = p.lookup(lib, n.sym).and_then(|b| b.getter) {
                        self.elemento(n.span, el, &["annotation"]);
                    }
                }
                [pf, n, ..] if eh_prefixo(p, lib, pf.sym) => {
                    self.nome(pf.span, "variable", &["importPrefix"]);
                    if let Some(el) = p.lookup_prefixed(lib, pf.sym, n.sym).and_then(|b| b.getter) {
                        self.elemento(n.span, el, &["annotation"]);
                    }
                    if let Some(k) = a.name.get(2) {
                        self.nome(k.span, "method", &["constructor", "annotation"]);
                    }
                }
                [c, k] => {
                    if let Some(el) = p.lookup(lib, c.sym).and_then(|b| b.getter) {
                        self.elemento(c.span, el, &["annotation"]);
                    }
                    self.nome(k.span, "method", &["constructor", "annotation"]);
                }
                _ => {}
            }
        }
    }

    fn parametros(&mut self, ps: &[ast::Parameter], corpos: &dartforge_types::UnitBodyTypes) {
        for prm in ps {
            self.anotacoes(&prm.metadata);
            if let Some(n) = prm.name {
                if prm.this_ {
                    self.nome(n.span, "variable", &["instance"]);
                } else {
                    let _ = corpos;
                    self.nome(n.span, "parameter", &["declaration"]);
                }
            }
            if let Some(t) = prm.ty {
                self.tipo_escrito(t, &[]);
            }
            if let Some(fps) = &prm.function_parameters {
                self.parametros(fps, corpos);
            }
        }
    }
}

impl Projeto {
    /// Os tokens semânticos de `unidade`, já sem sobreposição, em ordem.
    pub(crate) fn realces(&self, unidade: UnitId) -> Vec<Realce> {
        let p = self.programa();
        let u = p.unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let mut c = Coletor { projeto: self, unidade, nomes: HashMap::new(), outros: Vec::new() };

        // Declarações de topo.
        for d in &ast.decls {
            c.anotacoes(&d.metadata);
            match &d.kind {
                DeclKind::Class(k) => {
                    c.nome(k.name.span, "class", &["declaration"]);
                    for t in k.extends.iter().chain(k.with.iter()).chain(k.implements.iter()) {
                        c.tipo_escrito(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Mixin(k) => {
                    c.nome(k.name.span, "class", &[]);
                    for t in k.on.iter().chain(k.implements.iter()) {
                        c.tipo_escrito(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Enum(k) => {
                    c.nome(k.name.span, "enum", &[]);
                    for k2 in &k.constants {
                        c.anotacoes(&k2.metadata);
                        c.nome(k2.name.span, "enumMember", &[]);
                    }
                    for t in k.with.iter().chain(k.implements.iter()) {
                        c.tipo_escrito(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Extension(k) => {
                    if let Some(n) = k.name {
                        c.nome(n.span, "class", &[]);
                    }
                    c.tipo_escrito(k.on, &[]);
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::ExtensionType(k) => {
                    c.nome(k.name.span, "class", &["declaration"]);
                    if let Some(n) = k.constructor {
                        c.nome(n.span, "method", &["constructor", "declaration"]);
                    }
                    c.nome(k.representation_name.span, "variable", &["declaration", "instance"]);
                    c.tipo_escrito(k.representation_type, &[]);
                    for t in k.implements.iter() {
                        c.tipo_escrito(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Typedef(k) => {
                    c.nome(k.name.span, "type", &[]);
                    for tp in k.type_params.iter() {
                        c.nome(tp.name.span, "typeParameter", &[]);
                    }
                    match &k.kind {
                        ast::TypedefKind::Alias(t) => c.tipo_escrito(*t, &[]),
                        ast::TypedefKind::Legacy { return_type, parameters } => {
                            if let Some(t) = return_type {
                                c.tipo_escrito(*t, &[]);
                            }
                            c.parametros(parameters, corpos);
                        }
                    }
                }
                DeclKind::Function(f) => {
                    let func = ast.function(*f);
                    if let Some(n) = func.name {
                        match func.kind {
                            ast::FunctionKind::Getter | ast::FunctionKind::Setter => c.nome(n.span, "property", &["declaration"]),
                            _ => c.nome(n.span, "function", &["declaration", "static"]),
                        }
                    }
                }
                DeclKind::Variables(v) => {
                    for var in v.variables.iter() {
                        c.nome(var.name.span, "variable", &["declaration"]);
                    }
                    if let Some(t) = v.ty {
                        c.tipo_escrito(t, &[]);
                    }
                }
            }
        }
        // Membros.
        for m in &ast.members {
            c.anotacoes(&m.metadata);
            match &m.kind {
                MemberKind::Field(v) => {
                    let escopo = if v.static_ { "static" } else { "instance" };
                    for var in v.variables.iter() {
                        c.nome(var.name.span, "variable", &["declaration", escopo]);
                    }
                    if let Some(t) = v.ty {
                        c.tipo_escrito(t, &[]);
                    }
                }
                MemberKind::Method(f) => {
                    let func = ast.function(*f);
                    let escopo = if func.static_ { "static" } else { "instance" };
                    if let Some(n) = func.name {
                        match func.kind {
                            ast::FunctionKind::Getter | ast::FunctionKind::Setter => c.nome(n.span, "property", &["declaration", escopo]),
                            _ => c.nome(n.span, "method", &["declaration", escopo]),
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    c.nome(k.class_name.span, "class", &["constructor", "declaration"]);
                    if let Some(n) = k.name {
                        c.nome(n.span, "method", &["constructor", "declaration"]);
                    }
                    c.parametros(&k.parameters, corpos);
                    for ini in k.initializers.iter() {
                        match ini {
                            ast::Initializer::Field { name, .. } => c.nome(name.span, "variable", &["instance"]),
                            ast::Initializer::Super { constructor: Some(n), .. } | ast::Initializer::Redirect { constructor: Some(n), .. } => {
                                c.nome(n.span, "method", &["constructor"])
                            }
                            _ => {}
                        }
                    }
                    if let Some(r) = &k.redirect {
                        c.tipo_escrito(r.ty, &["constructor"]);
                        if let Some(n) = r.constructor {
                            c.nome(n.span, "method", &["constructor"]);
                        }
                    }
                }
            }
        }
        // Funções: parâmetros, parâmetros de tipo e retorno; locais.
        for f in &ast.functions {
            c.parametros(f.parameters.as_deref().unwrap_or(&[]), corpos);
            for tp in f.type_params.iter() {
                c.nome(tp.name.span, "typeParameter", &[]);
            }
            if let Some(t) = f.return_type {
                c.tipo_escrito(t, &[]);
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::Variables(v) => {
                    for var in v.variables.iter() {
                        c.nome(var.name.span, "variable", &["declaration"]);
                    }
                }
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        c.nome(n.span, "function", &["declaration"]);
                    }
                }
                StmtKind::ForIn { target: ast::ForInTarget::Declared { name, metadata, .. }, .. } => {
                    c.anotacoes(metadata);
                    c.nome(name.span, "variable", &["declaration"]);
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            c.nome(n.span, "variable", &["declaration"]);
                        }
                    }
                }
                _ => {}
            }
        }
        for pt in &ast.patterns {
            if let ast::PatternKind::Variable { name, .. } = &pt.kind {
                c.nome(name.span, "variable", &["declaration"]);
            }
        }
        // Anotações de tipo (as que não vieram pelas declarações acima).
        for i in 0..ast.types.len() {
            c.tipo_escrito(ast::TypeId(i as u32), &[]);
        }
        // Prefixos das diretivas.
        for d in &u.unit.directives {
            c.anotacoes(&d.metadata);
            if let ast::DirectiveKind::Import { prefix: Some(n), .. } = &d.kind {
                c.nome(n.span, "variable", &["importPrefix"]);
            }
            if let ast::DirectiveKind::Library { name } | ast::DirectiveKind::PartOf { name, .. } = &d.kind
                && let (Some(a), Some(b)) = (name.first(), name.last())
            {
                c.regiao(a.span.start, b.span.end, "namespace", &[]);
            }
        }
        // Expressões: referências, criações e rótulos de argumentos.
        let construtoras: std::collections::HashSet<u32> = ast
            .exprs
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                ExprKind::Call { target, .. } if matches!(corpos.get_resolved(ast::ExprId(i as u32)), Some(Resolved::Constructor(_))) => Some(target.0),
                _ => None,
            })
            .collect();
        // Os nomes de parâmetros declarados (a inferência resolve o uso de
        // um parâmetro como local).
        let parametros: std::collections::HashSet<usize> = ast
            .functions
            .iter()
            .flat_map(|f| f.parameters.iter().flatten())
            .chain(ast.members.iter().flat_map(|m| match &m.kind {
                MemberKind::Constructor(k) => k.parameters.iter(),
                _ => [].iter(),
            }))
            .filter_map(|p| p.name.map(|n| n.span.start))
            .collect();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Identifier(n) => {
                    let construtor = construtoras.contains(&(i as u32));
                    let r = corpos.get_resolved(id);
                    if matches!(r, Some(Resolved::Local(_))) && corpos.declaracao_local(id).is_some_and(|d| parametros.contains(&d)) {
                        c.nome(n.span, "parameter", &[]);
                    } else {
                        c.resolvido(n.span, r, construtor, &[], true);
                    }
                }
                ExprKind::Property { target, name, .. } => {
                    let construtor = construtoras.contains(&(i as u32));
                    // `A.nome(…)`: o alvo é a classe da criação.
                    if construtor && let ExprKind::Identifier(cn) = &ast.expr(*target).kind {
                        c.nome(cn.span, "class", &["constructor"]);
                    }
                    let dinamico = corpos.get_type(*target).is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Dynamic));
                    let r = corpos.get_resolved(id);
                    if construtor && !matches!(r, Some(Resolved::Constructor(_))) {
                        c.nome(name.span, "method", &["constructor"]);
                    } else if r.is_none()
                        && let Some(t) = corpos.get_type(*target)
                        && matches!(self.consulta.tabela.get(t), Type::Record { .. })
                    {
                        c.nome(name.span, "property", &["instance"]);
                    } else {
                        c.resolvido(name.span, r, false, &[], dinamico || r.is_none());
                    }
                }
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    c.tipo_escrito(*ty, &["constructor"]);
                    if let Some(n) = constructor {
                        c.nome(n.span, "method", &["constructor"]);
                    }
                    let _ = arguments;
                }
                _ => {}
            }
            if let ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } = &e.kind {
                for a in arguments.args.iter() {
                    if let Some(n) = a.name {
                        c.nome(n.span, "parameter", &["label"]);
                    }
                }
            }
            if let ExprKind::Record { named, .. } = &e.kind {
                for (n, _) in named.iter() {
                    c.nome(n.span, "parameter", &[]);
                }
            }
        }

        // Nomes de `show`/`hide`: o elemento que a biblioteca alvo exporta.
        for d in &u.unit.directives {
            if let ast::DirectiveKind::Import { combinators, .. } | ast::DirectiveKind::Export { combinators, .. } = &d.kind {
                for comb in combinators.iter() {
                    let (ast::Combinator::Show(nomes) | ast::Combinator::Hide(nomes)) = comb;
                    for n in nomes.iter() {
                        if let Ok(Some(den)) = self.identificar(unidade, n.span.start)
                            && let Some((tipo, mods)) = classificar(self, &den.alvo, den.concreto, &parametros)
                        {
                            c.nome(n.span, tipo, &mods);
                        }
                    }
                }
            }
        }
        // Referências `[nome]` dos comentários de documentação, por cima do
        // comentário (que fica dividido em volta delas).
        let mut referencias: Vec<Realce> = Vec::new();
        for com in crate::dartdoc::comentarios(fonte) {
            for r in &com.referencias {
                for pos in 0..r.len() {
                    let Some((alvo, concreto)) = self.resolver_referencia_doc(unidade, com.span, &r[..=pos]) else { continue };
                    if let Some((tipo, mods)) = classificar(self, &alvo, concreto, &parametros) {
                        referencias.push(Realce { inicio: r[pos].0.start, fim: r[pos].0.end, tipo, modificadores: mods });
                    }
                }
            }
        }

        // Tokens do texto: palavras, literais, strings e comentários.
        let mut regioes = std::mem::take(&mut c.outros);
        regioes.extend(referencias);
        let nomes = std::mem::take(&mut c.nomes);
        let Ok(tokens) = dartforge_frontend::lexer::lex(fonte) else { return Vec::new() };
        let mut anterior = 0usize;
        // Interpolação aberta por `${` (início do `$`).
        let mut interpolacoes: Vec<usize> = Vec::new();
        let mut ultimo_ident_interp: Option<usize> = None;
        for (k, t) in tokens.iter().enumerate() {
            // Comentários na lacuna.
            for (ini, fim) in comentarios(fonte, anterior, t.span.start) {
                let texto = &fonte[ini..fim];
                let doc = texto.starts_with("///") || texto.starts_with("/**");
                regioes.push(Realce { inicio: ini, fim, tipo: "comment", modificadores: if doc { vec!["documentation"] } else { Vec::new() } });
            }
            anterior = t.span.end;
            let texto = &fonte[t.span.start..t.span.end];
            let r = |tipo: &'static str, m: &[&'static str]| Realce { inicio: t.span.start, fim: t.span.end, tipo, modificadores: m.to_vec() };
            match t.kind {
                Kind::Keyword(_) => {
                    let realce = match texto {
                        "true" | "false" => r("boolean", &[]),
                        "void" => r("keyword", &["void"]),
                        "switch" if !eh_comando_switch(ast, t.span.start) => r("keyword", &[]),
                        _ if CONTROLE.contains(&texto) => r("keyword", &["control"]),
                        _ => r("keyword", &[]),
                    };
                    regioes.push(realce);
                }
                Kind::Ident => {
                    if let Some(n) = nomes.get(&t.span.start) {
                        regioes.push(n.clone());
                        if ultimo_ident_interp == Some(k) {
                            regioes.push(Realce { inicio: t.span.start - 1, fim: t.span.end, tipo: "source", modificadores: vec!["interpolation"] });
                        }
                        continue;
                    }
                    if ultimo_ident_interp == Some(k) {
                        regioes.push(Realce { inicio: t.span.start - 1, fim: t.span.end, tipo: "source", modificadores: vec!["interpolation"] });
                        continue;
                    }
                    if texto == "dynamic" {
                        regioes.push(r("type", &[]));
                    } else if EMBUTIDAS.contains(&texto) {
                        // `async*`/`sync*`/`yield*`: um token só com o `*`.
                        let mut fim = t.span.end;
                        if matches!(texto, "async" | "sync" | "yield")
                            && let Some(prox) = tokens.get(k + 1)
                            && prox.kind == Kind::Op(Op::Star)
                        {
                            fim = prox.span.end;
                        }
                        // `part of`: um token só.
                        if texto == "part"
                            && let Some(prox) = tokens.get(k + 1)
                            && prox.text(fonte) == "of"
                        {
                            fim = prox.span.end;
                        }
                        let m: &[&'static str] = if CONTROLE.contains(&texto) || (texto == "on" && eh_on_de_catch(fonte, t.span.start)) {
                            &["control"]
                        } else {
                            &[]
                        };
                        regioes.push(Realce { inicio: t.span.start, fim, tipo: "keyword", modificadores: m.to_vec() });
                    } else {
                        regioes.push(r("source", &[]));
                    }
                }
                Kind::Int | Kind::Double => regioes.push(r("number", &[])),
                Kind::Str(flags) => {
                    regioes.push(r("string", &[]));
                    if !flags.raw {
                        escapes(fonte, t.span, &mut regioes);
                    }
                }
                Kind::StrBegin(_, interp) | Kind::StrMid(_, interp) => {
                    // O trecho de texto vai até antes do `$`/`${`.
                    let mut ini = t.span.start;
                    if matches!(t.kind, Kind::StrMid(..)) && fonte.as_bytes().get(ini) == Some(&b'}') {
                        if let Some(abre) = interpolacoes.pop() {
                            regioes.push(Realce { inicio: abre, fim: ini + 1, tipo: "source", modificadores: vec!["interpolation"] });
                        }
                        ini += 1;
                    }
                    let corte = match interp {
                        dartforge_frontend::token::Interp::Brace => t.span.end - 2,
                        dartforge_frontend::token::Interp::Ident => t.span.end - 1,
                    };
                    regioes.push(Realce { inicio: ini, fim: corte, tipo: "string", modificadores: Vec::new() });
                    match interp {
                        dartforge_frontend::token::Interp::Brace => interpolacoes.push(corte),
                        dartforge_frontend::token::Interp::Ident => ultimo_ident_interp = Some(k + 1),
                    }
                }
                Kind::StrEnd(_) => {
                    let mut ini = t.span.start;
                    if fonte.as_bytes().get(ini) == Some(&b'}') {
                        if let Some(abre) = interpolacoes.pop() {
                            regioes.push(Realce { inicio: abre, fim: ini + 1, tipo: "source", modificadores: vec!["interpolation"] });
                        }
                        ini += 1;
                    }
                    regioes.push(Realce { inicio: ini, fim: t.span.end, tipo: "string", modificadores: Vec::new() });
                }
                _ => {}
            }
        }
        dividir_sobreposicoes(regioes)
    }
}

/// O token de um nome pelo que ele denota (referências de documentação e
/// nomes de `show`/`hide`, como o `_addIdentifierRegion` do analyzer).
fn classificar(
    projeto: &Projeto,
    alvo: &crate::projeto::Alvo,
    concreto: Option<crate::projeto::Concreto>,
    parametros: &std::collections::HashSet<usize>,
) -> Option<(&'static str, Vec<&'static str>)> {
    use crate::projeto::{Alvo, Concreto};
    let p = projeto.programa();
    Some(match (alvo, concreto) {
        (Alvo::Local { declaracao, .. }, _) => (if parametros.contains(declaracao) { "parameter" } else { "variable" }, Vec::new()),
        (Alvo::ParametroDeTipo { .. }, _) => ("typeParameter", Vec::new()),
        (Alvo::Prefixo { .. }, _) => ("variable", vec!["importPrefix"]),
        (Alvo::Construtor(_), _) => ("method", vec!["constructor"]),
        (Alvo::Topo(Element::Class(c)), _) => {
            let enum_ = p.class(*c).decl.is_some_and(|d| matches!(p.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Enum(_)));
            (if enum_ { "enum" } else { "class" }, Vec::new())
        }
        (Alvo::Topo(Element::Typedef(_)), _) => ("type", Vec::new()),
        (Alvo::Topo(Element::Extension(_)), _) => ("class", Vec::new()),
        (_, Some(Concreto::Funcao(f))) => {
            let fe = p.function(f);
            let membro = fe.class.is_some() || fe.extension.is_some();
            let escopo: Vec<&'static str> = if !membro {
                Vec::new()
            } else if fe.static_ {
                vec!["static"]
            } else {
                vec!["instance"]
            };
            match fe.kind {
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => ("property", escopo),
                _ if membro => ("method", escopo),
                _ => ("function", Vec::new()),
            }
        }
        (_, Some(Concreto::Variavel(v))) => {
            let ve = p.variable(v);
            match ve.class {
                Some(c) if p.class(c).enum_constants.contains(&v) => ("enumMember", Vec::new()),
                Some(_) => ("property", vec![if ve.static_ { "static" } else { "instance" }]),
                None => ("property", Vec::new()),
            }
        }
        _ => return None,
    })
}

/// O `switch` em `offset` é um comando (o de expressão não leva `control`).
fn eh_comando_switch(ast: &ast::Ast, offset: usize) -> bool {
    ast.stmts.iter().any(|s| s.span.start == offset && matches!(s.kind, StmtKind::Switch { .. }))
        || !ast.exprs.iter().any(|e| e.span.start == offset && matches!(e.kind, ExprKind::Switch { .. }))
}

/// O `on` em `offset` é o de uma cláusula `catch` (`} on T catch`).
fn eh_on_de_catch(fonte: &str, offset: usize) -> bool {
    fonte[..offset].trim_end().ends_with('}')
}

/// Os escapes válidos de uma string simples (`\n`, `\x41`, `\u{1F600}`).
fn escapes(fonte: &str, s: Span, saida: &mut Vec<Realce>) {
    let b = fonte.as_bytes();
    let hex = |i: usize, min: usize, max: usize| {
        let mut n = 0;
        while n < max && b.get(i + n).is_some_and(u8::is_ascii_hexdigit) {
            n += 1;
        }
        (n >= min).then_some(n)
    };
    let mut i = s.start;
    while i < s.end {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        let ini = i;
        i += 1;
        let Some(&c) = b.get(i) else { break };
        let fim = match c {
            b'x' => hex(i + 1, 2, 2).map(|n| i + 1 + n),
            b'u' if b.get(i + 1) == Some(&b'{') => hex(i + 2, 1, 6).filter(|n| b.get(i + 2 + n) == Some(&b'}')).map(|n| i + 3 + n),
            b'u' => hex(i + 1, 4, 4).map(|n| i + 1 + n),
            _ => Some(i + fonte[i..].chars().next().map_or(1, char::len_utf8)),
        };
        match fim {
            Some(f) if f <= s.end => {
                saida.push(Realce { inicio: ini, fim: f, tipo: "string", modificadores: vec!["escape"] });
                i = f;
            }
            _ => i += 1,
        }
    }
}

/// Os comentários (início, fim) da lacuna `[de, ate)`.
fn comentarios(fonte: &str, de: usize, ate: usize) -> Vec<(usize, usize)> {
    let b = fonte.as_bytes();
    let ate = ate.min(b.len());
    let mut saida = Vec::new();
    let mut i = de;
    while i < ate {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            let fim = fonte[i..ate].find(['\n', '\r']).map_or(ate, |k| i + k);
            saida.push((i, fim));
            i = fim;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
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
            saida.push((i, j.min(ate)));
            i = j.max(i + 2);
        } else {
            i += 1;
        }
    }
    saida
}

/// Ordena (início; o maior primeiro; `boolean` acima de `keyword`) e divide
/// as sobreposições pelo token de cima, como o `splitOverlappingTokens`.
fn dividir_sobreposicoes(mut regioes: Vec<Realce>) -> Vec<Realce> {
    regioes.sort_by(|a, b| {
        let prioridade = |r: &Realce| u8::from(r.tipo == "boolean");
        (a.inicio, std::cmp::Reverse(a.fim - a.inicio), prioridade(a)).cmp(&(b.inicio, std::cmp::Reverse(b.fim - b.inicio), prioridade(b)))
    });
    regioes.dedup_by(|b, a| a.inicio == b.inicio && a.fim == b.fim && a.tipo == b.tipo);
    let mut saida = Vec::new();
    let mut pilha: Vec<Realce> = Vec::new();
    let processar = |pilha: &mut Vec<Realce>, mut de: usize, ate: usize, saida: &mut Vec<Realce>| {
        while let Some(ultimo) = pilha.last() {
            let fim = ultimo.fim.min(ate);
            if fim > de {
                saida.push(Realce { inicio: de, fim, tipo: ultimo.tipo, modificadores: ultimo.modificadores.clone() });
                de = fim;
            }
            if ultimo.fim <= ate {
                pilha.pop();
            } else {
                return;
            }
        }
    };
    let mut pos = regioes.first().map_or(0, |r| r.inicio);
    for r in regioes {
        processar(&mut pilha, pos, r.inicio, &mut saida);
        pos = r.inicio;
        pilha.push(r);
    }
    if let Some(primeiro) = pilha.first() {
        let fim = primeiro.fim;
        processar(&mut pilha, pos, fim, &mut saida);
    }
    saida
}

/// Codifica para o protocolo: linha e coluna relativas, comprimento (UTF-16),
/// índice do tipo e bits dos modificadores. Sem `multilineTokenSupport`,
/// cada token de várias linhas vira um por linha. `faixa` (bytes) filtra
/// os tokens fora dela.
pub(crate) fn codificar(
    texto: &str,
    tabela: &crate::utf16::TabelaLinhas,
    realces: &[Realce],
    multilinha: bool,
    faixa: Option<(usize, usize)>,
) -> Vec<u32> {
    let mut dados = Vec::new();
    let (mut ultima_linha, mut ultima_coluna) = (0u32, 0u32);
    let mut emitir = |ini: usize, fim: usize, r: &Realce, dados: &mut Vec<u32>| {
        let (l0, c0) = tabela.posicao_de_offset(texto, ini);
        let (l1, c1) = tabela.posicao_de_offset(texto, fim);
        let comprimento = if l1 == l0 {
            c1 - c0
        } else {
            texto[ini..fim].encode_utf16().count() as u32
        };
        if comprimento == 0 {
            return;
        }
        let tipo = TIPOS.iter().position(|t| *t == r.tipo).unwrap_or(0) as u32;
        let bits = r
            .modificadores
            .iter()
            .filter_map(|m| MODIFICADORES.iter().position(|x| x == m))
            .fold(0u32, |b, i| b | (1 << i));
        let dl = l0 - ultima_linha;
        let dc = if dl == 0 { c0 - ultima_coluna } else { c0 };
        dados.extend_from_slice(&[dl, dc, comprimento, tipo, bits]);
        ultima_linha = l0;
        ultima_coluna = c0;
    };
    for r in realces {
        if let Some((de, ate)) = faixa
            && (r.fim < de || r.inicio > ate)
        {
            continue;
        }
        let l0 = tabela.linha_de(texto, r.inicio);
        let l1 = tabela.linha_de(texto, r.fim);
        if multilinha || l0 == l1 {
            emitir(r.inicio, r.fim, r, &mut dados);
            continue;
        }
        // Um por linha, com a quebra e a indentação incluídas.
        for l in l0..=l1 {
            let ini = if l == l0 { r.inicio } else { tabela.inicio_da_linha(l) };
            let fim = if l == l1 { r.fim } else { tabela.inicio_da_linha(l + 1) };
            let fim_sem_quebra = if l == l1 { fim } else { tabela.fim_da_linha(texto, l) };
            emitir(ini, fim_sem_quebra.max(ini), r, &mut dados);
            let _ = fim;
        }
    }
    dados
}
