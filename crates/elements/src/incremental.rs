//! Troca do texto de uma unidade sem recarregar o programa
//! (docs/LSP-ESPECIFICACAO.md §16.6, etapa I1): reanalisa a unidade, compara
//! a **assinatura de API** da versão velha com a da nova e, quando só o
//! interior de corpos mudou (classe **Corpo**), troca a árvore e o texto no
//! lugar.
//!
//! A assinatura é a sequência de tokens do arquivo fora dos corpos (o
//! `apiSignature` do analyzer, que também é feito de tokens): ficam de fora o
//! interior dos blocos e das expressões `=>` de funções de topo, métodos e
//! construtores, e os inicializadores de variáveis e campos com tipo escrito
//! que não são `const` nem `final` de instância numa classe com construtor
//! `const`. Os tokens das diretivas formam uma assinatura à parte (classe
//! **Diretivas**: o grafo de bibliotecas pode mudar). Comentários e espaços
//! não contam; a cola de `>` (que compõe `>>`) conta.
//!
//! Os membros de `mixin` não têm corpo fora da assinatura: os nomes chamados
//! por `super` dentro deles pertencem à assinatura no analyzer, e a edição
//! dentro de um deles recusa a troca (recarga), o que é conservador.
//!
//! Os elementos guardam ids das arenas de declarações, membros e funções
//! (`DeclRef`, `FunctionRef`, `VariableRef`), conferidos um a um; e pares
//! `(UnitId, ast::TypeId)` de cláusulas e limites, que são **remapeados** pelo
//! percurso paralelo das anotações de tipo das assinaturas (a arena de tipos
//! é preenchida em ordem de parse, por assinaturas e corpos: um tipo novo num
//! corpo desloca os ids do que vem depois). O esboço dos tipos guarda pares
//! iguais (`OutlineTypes::tipos_escritos`); quem o retém aplica
//! [`Anterior::mapa_de_tipos`] (e o inverso, ao desfazer).

use crate::model::{Program, UnitId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, Ast, CompilationUnit, DeclKind, FunctionBody, MemberKind, TypeId};
use dartforge_frontend::token::{Kind, Op, Token};
use dartforge_intern::Interner;
use std::collections::HashMap;

/// Por que a troca foi recusada (o chamador recarrega o programa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recusa {
    /// As arenas de declarações, membros ou funções não correspondem uma a
    /// uma, ou as anotações de tipo das assinaturas não se alinham.
    ShapeChanged(&'static str),
    /// Uma das versões tem erro léxico: a assinatura não é comparável.
    Lexico,
    /// A assinatura de API das declarações mudou.
    Assinatura,
    /// As diretivas mudaram.
    Diretivas,
}

/// Um corpo da unidade, pelos ids das arenas (estáveis numa troca aceita).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitBody {
    /// Função de topo, método, getter, setter ou operador.
    Funcao(ast::FunctionId),
    /// Corpo de construtor (os inicializadores são assinatura).
    Construtor(ast::MemberId),
    /// Inicializador de variável de topo.
    VariavelDeTopo { decl: ast::DeclId, index: usize },
    /// Inicializador de campo.
    Campo { member: ast::MemberId, index: usize },
}

/// O que a troca substituiu, para desfazer com [`restaurar_unidade`].
pub struct Anterior {
    unidade: UnitId,
    source: String,
    ast: Ast,
    unit: CompilationUnit,
    pulados: Vec<Span>,
    referencia: dartforge_diagnostics::Referencia,
    /// `TypeId` velho → novo das anotações das assinaturas.
    map: HashMap<TypeId, TypeId>,
    alterados: Vec<UnitBody>,
}

impl Anterior {
    /// A unidade trocada.
    pub fn unidade(&self) -> UnitId {
        self.unidade
    }

    /// Os corpos cujo texto mudou.
    pub fn changed_bodies(&self) -> &[UnitBody] {
        &self.alterados
    }

    /// `TypeId` da árvore velha → o da nova, para as anotações das
    /// assinaturas (as únicas que o esboço guarda).
    pub fn mapa_de_tipos(&self) -> &HashMap<TypeId, TypeId> {
        &self.map
    }

    /// O inverso de [`Anterior::mapa_de_tipos`] (o mapa é uma bijeção).
    pub fn mapa_inverso(&self) -> HashMap<TypeId, TypeId> {
        self.map.iter().map(|(&a, &b)| (b, a)).collect()
    }
}

/// Troca o texto da unidade `u` por `text` quando a assinatura de API não
/// mudou; devolve o anterior para desfazer.
///
/// `initializers_as_body`: os inicializadores de variáveis e campos
/// contam todos como corpo (o completar, §16.7 regra 5: o tipo inferido de
/// uma variável não importa para as sugestões dentro do próprio
/// inicializador, e nada do que a troca especulativa produz é gravado).
pub fn substituir_unidade(
    program: &mut Program,
    names_: &mut Interner,
    u: UnitId,
    text: &str,
    initializers_as_body: bool,
) -> Result<Anterior, Recusa> {
    let features = program.unit(u).features;
    let novo = dartforge_frontend::parser::parse_com(text, names_, features);
    let (map, alterados) = {
        let old_ = program.unit(u);
        verify_shape(&old_.ast, &old_.unit, &novo.ast, &novo.unit)?;
        let (Ok(tv), Ok(tn)) = (dartforge_frontend::lexer::lex(&old_.source), dartforge_frontend::lexer::lex(text)) else {
            return Err(Recusa::Lexico);
        };
        let rv = regions(&old_.ast, initializers_as_body);
        let rn = regions(&novo.ast, initializers_as_body);
        if rv.len() != rn.len() || rv.iter().zip(rn.iter()).any(|(a, b)| a.owner != b.owner) {
            return Err(Recusa::Assinatura);
        }
        let (dir_v, sig_v) = split(&old_.source, &tv, &directive_spans(&old_.unit), &rv);
        let (dir_n, sig_n) = split(text, &tn, &directive_spans(&novo.unit), &rn);
        if dir_v != dir_n {
            return Err(Recusa::Diretivas);
        }
        if sig_v != sig_n {
            return Err(Recusa::Assinatura);
        }
        let alterados: Vec<UnitBody> = rv
            .iter()
            .zip(rn.iter())
            .filter(|(a, b)| old_.source.get(a.span.start..a.span.end) != text.get(b.span.start..b.span.end))
            .map(|(a, _)| a.owner)
            .collect();
        let (lv, ln) = (signature_types(&old_.ast), signature_types(&novo.ast));
        if lv.len() != ln.len() {
            return Err(Recusa::ShapeChanged("tipos das assinaturas"));
        }
        for (a, b) in lv.iter().zip(ln.iter()) {
            let (ta, tb) = (old_.ast.ty(*a), novo.ast.ty(*b));
            if ta.nullable != tb.nullable || std::mem::discriminant(&ta.kind) != std::mem::discriminant(&tb.kind) {
                return Err(Recusa::ShapeChanged("tipos das assinaturas"));
            }
        }
        let map: HashMap<TypeId, TypeId> = lv.into_iter().zip(ln).collect();
        (map, alterados)
    };
    {
        let mut ps = pairs(program);
        if ps.iter().any(|p| p.0 == u && !map.contains_key(&p.1)) {
            return Err(Recusa::ShapeChanged("tipos dos elementos"));
        }
        for p in ps.iter_mut() {
            if p.0 == u {
                p.1 = map[&p.1];
            }
        }
    }
    let un = &mut program.units[u.0 as usize];
    Ok(Anterior {
        unidade: u,
        source: std::mem::replace(&mut un.source, text.to_string()),
        ast: std::mem::replace(&mut un.ast, novo.ast),
        unit: std::mem::replace(&mut un.unit, novo.unit),
        pulados: std::mem::replace(&mut un.pulados, novo.pulados),
        referencia: std::mem::replace(&mut un.referencia, novo.referencia),
        map,
        alterados,
    })
}

/// Desfaz uma [`substituir_unidade`] (os pares dos elementos voltam aos ids
/// da árvore velha).
pub fn restaurar_unidade(program: &mut Program, anterior: Anterior) {
    let u = anterior.unidade;
    let inverse = anterior.mapa_inverso();
    for p in pairs(program) {
        if p.0 == u
            && let Some(&t) = inverse.get(&p.1)
        {
            p.1 = t;
        }
    }
    let un = &mut program.units[u.0 as usize];
    un.source = anterior.source;
    un.ast = anterior.ast;
    un.unit = anterior.unit;
    un.pulados = anterior.pulados;
    un.referencia = anterior.referencia;
}

/// A correspondência um a um das arenas que os elementos citam.
fn verify_shape(old_: &Ast, old_unit: &CompilationUnit, a: &Ast, new_unit: &CompilationUnit) -> Result<(), Recusa> {
    if old_.decls.len() != a.decls.len() {
        return Err(Recusa::ShapeChanged("declarações"));
    }
    if old_.members.len() != a.members.len() {
        return Err(Recusa::ShapeChanged("membros"));
    }
    if old_.functions.len() != a.functions.len() {
        return Err(Recusa::ShapeChanged("funções"));
    }
    for (d0, d1) in old_.decls.iter().zip(a.decls.iter()) {
        if std::mem::discriminant(&d0.kind) != std::mem::discriminant(&d1.kind) || decl_name(&d0.kind) != decl_name(&d1.kind) {
            return Err(Recusa::ShapeChanged("declaração"));
        }
        if let (DeclKind::Variables(l0), DeclKind::Variables(l1)) = (&d0.kind, &d1.kind)
            && !same_names(l0, l1)
        {
            return Err(Recusa::ShapeChanged("variáveis"));
        }
    }
    for (m0, m1) in old_.members.iter().zip(a.members.iter()) {
        if std::mem::discriminant(&m0.kind) != std::mem::discriminant(&m1.kind) || member_name(&m0.kind) != member_name(&m1.kind) {
            return Err(Recusa::ShapeChanged("membro"));
        }
        if let (MemberKind::Field(l0), MemberKind::Field(l1)) = (&m0.kind, &m1.kind)
            && !same_names(l0, l1)
        {
            return Err(Recusa::ShapeChanged("campos"));
        }
    }
    for (f0, f1) in old_.functions.iter().zip(a.functions.iter()) {
        if f0.name.map(|n| n.sym) != f1.name.map(|n| n.sym) {
            return Err(Recusa::ShapeChanged("função"));
        }
    }
    if old_unit.declarations != new_unit.declarations {
        return Err(Recusa::ShapeChanged("declarações da unidade"));
    }
    Ok(())
}

/// A lista inteira de nomes (o `index` de `VariableRef` aponta nela).
fn same_names(a: &ast::VariableList, b: &ast::VariableList) -> bool {
    a.variables.len() == b.variables.len() && a.variables.iter().zip(b.variables.iter()).all(|(x, y)| x.name.sym == y.name.sym)
}

/// Um trecho de corpo: os tokens dentro dele ficam fora da assinatura.
struct Region {
    owner: UnitBody,
    span: Span,
    /// Bloco: as chaves ficam na assinatura ("corpo vazio?"), só o
    /// interior sai.
    strict: bool,
}

/// Os corpos da unidade, na ordem das arenas.
fn regions(a: &Ast, initializers_as_body: bool) -> Vec<Region> {
    let mut v = Vec::new();
    let body_node = |v: &mut Vec<Region>, owner: UnitBody, body: &FunctionBody| match body {
        FunctionBody::Block(s) => v.push(Region { owner, span: a.stmt(*s).span, strict: true }),
        FunctionBody::Expression(e) => v.push(Region { owner, span: a.expr(*e).span, strict: false }),
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    };
    let members_ = |v: &mut Vec<Region>, members_: &[ast::MemberId], has_const: bool| {
        for &m in members_ {
            match &a.member(m).kind {
                MemberKind::Method(f) => body_node(v, UnitBody::Funcao(*f), &a.function(*f).body),
                MemberKind::Constructor(k) => body_node(v, UnitBody::Construtor(m), &k.body),
                MemberKind::Field(l) => {
                    let signature = l.ty.is_none() || l.const_ || (l.final_ && !l.static_ && has_const);
                    if initializers_as_body || !signature {
                        for (k, x) in l.variables.iter().enumerate() {
                            if let Some(e) = x.initializer {
                                v.push(Region { owner: UnitBody::Campo { member: m, index: k }, span: a.expr(e).span, strict: false });
                            }
                        }
                    }
                }
            }
        }
    };
    let has_const_constructor = |members_: &[ast::MemberId]| members_.iter().any(|&m| matches!(&a.member(m).kind, MemberKind::Constructor(k) if k.const_));
    for (i, d) in a.decls.iter().enumerate() {
        let decl = ast::DeclId(i as u32);
        match &d.kind {
            DeclKind::Function(f) => body_node(&mut v, UnitBody::Funcao(*f), &a.function(*f).body),
            DeclKind::Variables(l) => {
                if initializers_as_body || (l.ty.is_some() && !l.const_) {
                    for (k, x) in l.variables.iter().enumerate() {
                        if let Some(e) = x.initializer {
                            v.push(Region { owner: UnitBody::VariavelDeTopo { decl, index: k }, span: a.expr(e).span, strict: false });
                        }
                    }
                }
            }
            DeclKind::Class(c) => members_(&mut v, &c.members, has_const_constructor(&c.members)),
            DeclKind::Enum(e) => members_(&mut v, &e.members, true),
            DeclKind::ExtensionType(x) => members_(&mut v, &x.members, x.const_ || has_const_constructor(&x.members)),
            DeclKind::Extension(x) => members_(&mut v, &x.members, false),
            DeclKind::Mixin(_) | DeclKind::Typedef(_) => {}
        }
    }
    v
}

fn directive_spans(unit: &CompilationUnit) -> Vec<(Span, bool)> {
    let mut v: Vec<(Span, bool)> = unit.directives.iter().map(|d| (d.span, false)).collect();
    v.sort_by_key(|(s, _)| s.start);
    v
}

/// O token está num dos trechos (ordenados pelo início, sem sobreposição).
fn inside(excerpts: &[(Span, bool)], t: Span) -> bool {
    let i = excerpts.partition_point(|(s, _)| s.start <= t.start);
    if i == 0 {
        return false;
    }
    let (s, strict) = excerpts[i - 1];
    if strict { t.start > s.start && t.end < s.end } else { t.start >= s.start && t.end <= s.end }
}

/// Um token da assinatura: espécie, texto e a cola de um `>`.
type Lexeme<'s> = (Kind, &'s str, bool);

/// Os tokens das diretivas e os da assinatura das declarações.
fn split<'s>(source: &'s str, tokens: &[Token], diretivas: &[(Span, bool)], bodies: &[Region]) -> (Vec<Lexeme<'s>>, Vec<Lexeme<'s>>) {
    let mut excerpts: Vec<(Span, bool)> = bodies.iter().map(|r| (r.span, r.strict)).collect();
    excerpts.sort_by_key(|(s, _)| s.start);
    let (mut d, mut s) = (Vec::new(), Vec::new());
    for t in tokens {
        let lexeme = (t.kind, source.get(t.span.start..t.span.end).unwrap_or(""), t.glued && matches!(t.kind, Kind::Op(Op::Gt)));
        if inside(diretivas, t.span) {
            d.push(lexeme);
        } else if !inside(&excerpts, t.span) {
            s.push(lexeme);
        }
    }
    (d, s)
}

/// As anotações de tipo das assinaturas, numa ordem determinada pela forma
/// (a mesma nas duas árvores quando as assinaturas são iguais).
fn signature_types(a: &Ast) -> Vec<TypeId> {
    let mut v = Vec::new();
    let mut declared: Vec<ast::FunctionId> = Vec::new();
    for d in &a.decls {
        annotations(a, &d.metadata, &mut v);
        match &d.kind {
            DeclKind::Class(c) => {
                type_parameters(a, &c.type_params, &mut v);
                if let Some(t) = c.extends {
                    kind(a, t, &mut v);
                }
                for t in c.with.iter().chain(c.implements.iter()) {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Mixin(m) => {
                type_parameters(a, &m.type_params, &mut v);
                for t in m.on.iter().chain(m.implements.iter()) {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Enum(e) => {
                type_parameters(a, &e.type_params, &mut v);
                for t in e.with.iter().chain(e.implements.iter()) {
                    kind(a, *t, &mut v);
                }
                for k in &e.constants {
                    annotations(a, &k.metadata, &mut v);
                    for t in k.type_args.iter() {
                        kind(a, *t, &mut v);
                    }
                }
            }
            DeclKind::Extension(x) => {
                type_parameters(a, &x.type_params, &mut v);
                kind(a, x.on, &mut v);
            }
            DeclKind::ExtensionType(x) => {
                type_parameters(a, &x.type_params, &mut v);
                annotations(a, &x.representation_metadata, &mut v);
                kind(a, x.representation_type, &mut v);
                for t in x.implements.iter() {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Typedef(t) => {
                type_parameters(a, &t.type_params, &mut v);
                match &t.kind {
                    ast::TypedefKind::Alias(x) => kind(a, *x, &mut v),
                    ast::TypedefKind::Legacy { return_type, parameters } => {
                        if let Some(r) = return_type {
                            kind(a, *r, &mut v);
                        }
                        parameters_(a, parameters, &mut v);
                    }
                }
            }
            DeclKind::Function(f) => declared.push(*f),
            DeclKind::Variables(l) => {
                if let Some(t) = l.ty {
                    kind(a, t, &mut v);
                }
            }
        }
    }
    for m in &a.members {
        annotations(a, &m.metadata, &mut v);
        match &m.kind {
            MemberKind::Field(l) => {
                if let Some(t) = l.ty {
                    kind(a, t, &mut v);
                }
            }
            MemberKind::Method(f) => declared.push(*f),
            MemberKind::Constructor(k) => {
                parameters_(a, &k.parameters, &mut v);
                if let Some(r) = &k.redirect {
                    kind(a, r.ty, &mut v);
                }
            }
        }
    }
    for f in declared {
        let f = a.function(f);
        if let Some(r) = f.return_type {
            kind(a, r, &mut v);
        }
        type_parameters(a, &f.type_params, &mut v);
        if let Some(ps) = &f.parameters {
            parameters_(a, ps, &mut v);
        }
    }
    v
}

fn kind(a: &Ast, t: TypeId, v: &mut Vec<TypeId>) {
    v.push(t);
    match &a.ty(t).kind {
        ast::TypeKind::Named { args, .. } => {
            for x in args.iter() {
                kind(a, *x, v);
            }
        }
        ast::TypeKind::Void => {}
        ast::TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type {
                kind(a, *r, v);
            }
            type_parameters(a, type_params, v);
            parameters_(a, parameters, v);
        }
        ast::TypeKind::Record { positional, named } => {
            for x in positional.iter() {
                kind(a, *x, v);
            }
            for (_, x) in named.iter() {
                kind(a, *x, v);
            }
        }
    }
}

fn type_parameters(a: &Ast, ps: &[ast::TypeParameter], v: &mut Vec<TypeId>) {
    for p in ps {
        annotations(a, &p.metadata, v);
        if let Some(b) = p.bound {
            kind(a, b, v);
        }
    }
}

fn parameters_(a: &Ast, ps: &[ast::Parameter], v: &mut Vec<TypeId>) {
    for p in ps {
        annotations(a, &p.metadata, v);
        if let Some(t) = p.ty {
            kind(a, t, v);
        }
        type_parameters(a, &p.function_type_params, v);
        if let Some(fp) = &p.function_parameters {
            parameters_(a, fp, v);
        }
    }
}

fn annotations(a: &Ast, list: &[ast::Annotation], v: &mut Vec<TypeId>) {
    for x in list {
        for t in &x.type_args {
            kind(a, *t, v);
        }
    }
}

/// Os pares `(UnitId, ast::TypeId)` guardados nos elementos.
fn pairs(program: &mut Program) -> Vec<&mut (UnitId, TypeId)> {
    let mut v: Vec<&mut (UnitId, TypeId)> = Vec::new();
    for c in program.classes.iter_mut() {
        let crate::model::ClassElement { type_params, supertype, mixins, interfaces, on, .. } = c;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.extend(supertype.as_mut());
        v.extend(mixins.iter_mut());
        v.extend(interfaces.iter_mut());
        v.extend(on.iter_mut());
    }
    for x in program.extensions.iter_mut() {
        let crate::model::ExtensionElement { type_params, on, .. } = x;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.push(on);
    }
    for t in program.typedefs.iter_mut() {
        v.extend(t.type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
    }
    v
}

fn decl_name(k: &DeclKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        DeclKind::Class(x) => Some(x.name.sym),
        DeclKind::Mixin(x) => Some(x.name.sym),
        DeclKind::Enum(x) => Some(x.name.sym),
        DeclKind::Extension(x) => x.name.map(|n| n.sym),
        DeclKind::ExtensionType(x) => Some(x.name.sym),
        DeclKind::Typedef(x) => Some(x.name.sym),
        DeclKind::Variables(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}

fn member_name(k: &MemberKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        MemberKind::Constructor(c) => c.name.map(|n| n.sym),
        MemberKind::Field(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}
