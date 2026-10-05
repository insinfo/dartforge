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
    FormaMudou(&'static str),
    /// Uma das versões tem erro léxico: a assinatura não é comparável.
    Lexico,
    /// A assinatura de API das declarações mudou.
    Assinatura,
    /// As diretivas mudaram.
    Diretivas,
}

/// Um corpo da unidade, pelos ids das arenas (estáveis numa troca aceita).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorpoDaUnidade {
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
    mapa: HashMap<TypeId, TypeId>,
    alterados: Vec<CorpoDaUnidade>,
}

impl Anterior {
    /// A unidade trocada.
    pub fn unidade(&self) -> UnitId {
        self.unidade
    }

    /// Os corpos cujo texto mudou.
    pub fn corpos_alterados(&self) -> &[CorpoDaUnidade] {
        &self.alterados
    }

    /// `TypeId` da árvore velha → o da nova, para as anotações das
    /// assinaturas (as únicas que o esboço guarda).
    pub fn mapa_de_tipos(&self) -> &HashMap<TypeId, TypeId> {
        &self.mapa
    }

    /// O inverso de [`Anterior::mapa_de_tipos`] (o mapa é uma bijeção).
    pub fn mapa_inverso(&self) -> HashMap<TypeId, TypeId> {
        self.mapa.iter().map(|(&a, &b)| (b, a)).collect()
    }
}

/// Troca o texto da unidade `u` por `texto` quando a assinatura de API não
/// mudou; devolve o anterior para desfazer.
///
/// `inicializadores_como_corpo`: os inicializadores de variáveis e campos
/// contam todos como corpo (o completar, §16.7 regra 5: o tipo inferido de
/// uma variável não importa para as sugestões dentro do próprio
/// inicializador, e nada do que a troca especulativa produz é gravado).
pub fn substituir_unidade(
    programa: &mut Program,
    nomes: &mut Interner,
    u: UnitId,
    texto: &str,
    inicializadores_como_corpo: bool,
) -> Result<Anterior, Recusa> {
    let features = programa.unit(u).features;
    let novo = dartforge_frontend::parser::parse_com(texto, nomes, features);
    let (mapa, alterados) = {
        let velho = programa.unit(u);
        verificar_forma(&velho.ast, &velho.unit, &novo.ast, &novo.unit)?;
        let (Ok(tv), Ok(tn)) = (dartforge_frontend::lexer::lex(&velho.source), dartforge_frontend::lexer::lex(texto)) else {
            return Err(Recusa::Lexico);
        };
        let rv = regioes(&velho.ast, inicializadores_como_corpo);
        let rn = regioes(&novo.ast, inicializadores_como_corpo);
        if rv.len() != rn.len() || rv.iter().zip(rn.iter()).any(|(a, b)| a.dono != b.dono) {
            return Err(Recusa::Assinatura);
        }
        let (dir_v, sig_v) = separar(&velho.source, &tv, &spans_de_diretivas(&velho.unit), &rv);
        let (dir_n, sig_n) = separar(texto, &tn, &spans_de_diretivas(&novo.unit), &rn);
        if dir_v != dir_n {
            return Err(Recusa::Diretivas);
        }
        if sig_v != sig_n {
            return Err(Recusa::Assinatura);
        }
        let alterados: Vec<CorpoDaUnidade> = rv
            .iter()
            .zip(rn.iter())
            .filter(|(a, b)| velho.source.get(a.span.start..a.span.end) != texto.get(b.span.start..b.span.end))
            .map(|(a, _)| a.dono)
            .collect();
        let (lv, ln) = (tipos_da_assinatura(&velho.ast), tipos_da_assinatura(&novo.ast));
        if lv.len() != ln.len() {
            return Err(Recusa::FormaMudou("tipos das assinaturas"));
        }
        for (a, b) in lv.iter().zip(ln.iter()) {
            let (ta, tb) = (velho.ast.ty(*a), novo.ast.ty(*b));
            if ta.nullable != tb.nullable || std::mem::discriminant(&ta.kind) != std::mem::discriminant(&tb.kind) {
                return Err(Recusa::FormaMudou("tipos das assinaturas"));
            }
        }
        let mapa: HashMap<TypeId, TypeId> = lv.into_iter().zip(ln).collect();
        (mapa, alterados)
    };
    {
        let mut ps = pares(programa);
        if ps.iter().any(|p| p.0 == u && !mapa.contains_key(&p.1)) {
            return Err(Recusa::FormaMudou("tipos dos elementos"));
        }
        for p in ps.iter_mut() {
            if p.0 == u {
                p.1 = mapa[&p.1];
            }
        }
    }
    let un = &mut programa.units[u.0 as usize];
    Ok(Anterior {
        unidade: u,
        source: std::mem::replace(&mut un.source, texto.to_string()),
        ast: std::mem::replace(&mut un.ast, novo.ast),
        unit: std::mem::replace(&mut un.unit, novo.unit),
        pulados: std::mem::replace(&mut un.pulados, novo.pulados),
        referencia: std::mem::replace(&mut un.referencia, novo.referencia),
        mapa,
        alterados,
    })
}

/// Desfaz uma [`substituir_unidade`] (os pares dos elementos voltam aos ids
/// da árvore velha).
pub fn restaurar_unidade(programa: &mut Program, anterior: Anterior) {
    let u = anterior.unidade;
    let inverso = anterior.mapa_inverso();
    for p in pares(programa) {
        if p.0 == u
            && let Some(&t) = inverso.get(&p.1)
        {
            p.1 = t;
        }
    }
    let un = &mut programa.units[u.0 as usize];
    un.source = anterior.source;
    un.ast = anterior.ast;
    un.unit = anterior.unit;
    un.pulados = anterior.pulados;
    un.referencia = anterior.referencia;
}

/// A correspondência um a um das arenas que os elementos citam.
fn verificar_forma(velho: &Ast, unidade_velha: &CompilationUnit, a: &Ast, unidade_nova: &CompilationUnit) -> Result<(), Recusa> {
    if velho.decls.len() != a.decls.len() {
        return Err(Recusa::FormaMudou("declarações"));
    }
    if velho.members.len() != a.members.len() {
        return Err(Recusa::FormaMudou("membros"));
    }
    if velho.functions.len() != a.functions.len() {
        return Err(Recusa::FormaMudou("funções"));
    }
    for (d0, d1) in velho.decls.iter().zip(a.decls.iter()) {
        if std::mem::discriminant(&d0.kind) != std::mem::discriminant(&d1.kind) || nome_da_decl(&d0.kind) != nome_da_decl(&d1.kind) {
            return Err(Recusa::FormaMudou("declaração"));
        }
        if let (DeclKind::Variables(l0), DeclKind::Variables(l1)) = (&d0.kind, &d1.kind)
            && !mesmos_nomes(l0, l1)
        {
            return Err(Recusa::FormaMudou("variáveis"));
        }
    }
    for (m0, m1) in velho.members.iter().zip(a.members.iter()) {
        if std::mem::discriminant(&m0.kind) != std::mem::discriminant(&m1.kind) || nome_do_membro(&m0.kind) != nome_do_membro(&m1.kind) {
            return Err(Recusa::FormaMudou("membro"));
        }
        if let (MemberKind::Field(l0), MemberKind::Field(l1)) = (&m0.kind, &m1.kind)
            && !mesmos_nomes(l0, l1)
        {
            return Err(Recusa::FormaMudou("campos"));
        }
    }
    for (f0, f1) in velho.functions.iter().zip(a.functions.iter()) {
        if f0.name.map(|n| n.sym) != f1.name.map(|n| n.sym) {
            return Err(Recusa::FormaMudou("função"));
        }
    }
    if unidade_velha.declarations != unidade_nova.declarations {
        return Err(Recusa::FormaMudou("declarações da unidade"));
    }
    Ok(())
}

/// A lista inteira de nomes (o `index` de `VariableRef` aponta nela).
fn mesmos_nomes(a: &ast::VariableList, b: &ast::VariableList) -> bool {
    a.variables.len() == b.variables.len() && a.variables.iter().zip(b.variables.iter()).all(|(x, y)| x.name.sym == y.name.sym)
}

/// Um trecho de corpo: os tokens dentro dele ficam fora da assinatura.
struct Regiao {
    dono: CorpoDaUnidade,
    span: Span,
    /// Bloco: as chaves ficam na assinatura ("corpo vazio?"), só o
    /// interior sai.
    estrito: bool,
}

/// Os corpos da unidade, na ordem das arenas.
fn regioes(a: &Ast, inicializadores_como_corpo: bool) -> Vec<Regiao> {
    let mut v = Vec::new();
    let corpo = |v: &mut Vec<Regiao>, dono: CorpoDaUnidade, body: &FunctionBody| match body {
        FunctionBody::Block(s) => v.push(Regiao { dono, span: a.stmt(*s).span, estrito: true }),
        FunctionBody::Expression(e) => v.push(Regiao { dono, span: a.expr(*e).span, estrito: false }),
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    };
    let membros = |v: &mut Vec<Regiao>, membros: &[ast::MemberId], tem_const: bool| {
        for &m in membros {
            match &a.member(m).kind {
                MemberKind::Method(f) => corpo(v, CorpoDaUnidade::Funcao(*f), &a.function(*f).body),
                MemberKind::Constructor(k) => corpo(v, CorpoDaUnidade::Construtor(m), &k.body),
                MemberKind::Field(l) => {
                    let assinatura = l.ty.is_none() || l.const_ || (l.final_ && !l.static_ && tem_const);
                    if inicializadores_como_corpo || !assinatura {
                        for (k, x) in l.variables.iter().enumerate() {
                            if let Some(e) = x.initializer {
                                v.push(Regiao { dono: CorpoDaUnidade::Campo { member: m, index: k }, span: a.expr(e).span, estrito: false });
                            }
                        }
                    }
                }
            }
        }
    };
    let tem_construtor_const = |membros: &[ast::MemberId]| membros.iter().any(|&m| matches!(&a.member(m).kind, MemberKind::Constructor(k) if k.const_));
    for (i, d) in a.decls.iter().enumerate() {
        let decl = ast::DeclId(i as u32);
        match &d.kind {
            DeclKind::Function(f) => corpo(&mut v, CorpoDaUnidade::Funcao(*f), &a.function(*f).body),
            DeclKind::Variables(l) => {
                if inicializadores_como_corpo || (l.ty.is_some() && !l.const_) {
                    for (k, x) in l.variables.iter().enumerate() {
                        if let Some(e) = x.initializer {
                            v.push(Regiao { dono: CorpoDaUnidade::VariavelDeTopo { decl, index: k }, span: a.expr(e).span, estrito: false });
                        }
                    }
                }
            }
            DeclKind::Class(c) => membros(&mut v, &c.members, tem_construtor_const(&c.members)),
            DeclKind::Enum(e) => membros(&mut v, &e.members, true),
            DeclKind::ExtensionType(x) => membros(&mut v, &x.members, x.const_ || tem_construtor_const(&x.members)),
            DeclKind::Extension(x) => membros(&mut v, &x.members, false),
            DeclKind::Mixin(_) | DeclKind::Typedef(_) => {}
        }
    }
    v
}

fn spans_de_diretivas(unit: &CompilationUnit) -> Vec<(Span, bool)> {
    let mut v: Vec<(Span, bool)> = unit.directives.iter().map(|d| (d.span, false)).collect();
    v.sort_by_key(|(s, _)| s.start);
    v
}

/// O token está num dos trechos (ordenados pelo início, sem sobreposição).
fn dentro(trechos: &[(Span, bool)], t: Span) -> bool {
    let i = trechos.partition_point(|(s, _)| s.start <= t.start);
    if i == 0 {
        return false;
    }
    let (s, estrito) = trechos[i - 1];
    if estrito { t.start > s.start && t.end < s.end } else { t.start >= s.start && t.end <= s.end }
}

/// Um token da assinatura: espécie, texto e a cola de um `>`.
type Lexema<'s> = (Kind, &'s str, bool);

/// Os tokens das diretivas e os da assinatura das declarações.
fn separar<'s>(fonte: &'s str, tokens: &[Token], diretivas: &[(Span, bool)], corpos: &[Regiao]) -> (Vec<Lexema<'s>>, Vec<Lexema<'s>>) {
    let mut trechos: Vec<(Span, bool)> = corpos.iter().map(|r| (r.span, r.estrito)).collect();
    trechos.sort_by_key(|(s, _)| s.start);
    let (mut d, mut s) = (Vec::new(), Vec::new());
    for t in tokens {
        let lexema = (t.kind, fonte.get(t.span.start..t.span.end).unwrap_or(""), t.glued && matches!(t.kind, Kind::Op(Op::Gt)));
        if dentro(diretivas, t.span) {
            d.push(lexema);
        } else if !dentro(&trechos, t.span) {
            s.push(lexema);
        }
    }
    (d, s)
}

/// As anotações de tipo das assinaturas, numa ordem determinada pela forma
/// (a mesma nas duas árvores quando as assinaturas são iguais).
fn tipos_da_assinatura(a: &Ast) -> Vec<TypeId> {
    let mut v = Vec::new();
    let mut declaradas: Vec<ast::FunctionId> = Vec::new();
    for d in &a.decls {
        anotacoes(a, &d.metadata, &mut v);
        match &d.kind {
            DeclKind::Class(c) => {
                parametros_de_tipo(a, &c.type_params, &mut v);
                if let Some(t) = c.extends {
                    tipo(a, t, &mut v);
                }
                for t in c.with.iter().chain(c.implements.iter()) {
                    tipo(a, *t, &mut v);
                }
            }
            DeclKind::Mixin(m) => {
                parametros_de_tipo(a, &m.type_params, &mut v);
                for t in m.on.iter().chain(m.implements.iter()) {
                    tipo(a, *t, &mut v);
                }
            }
            DeclKind::Enum(e) => {
                parametros_de_tipo(a, &e.type_params, &mut v);
                for t in e.with.iter().chain(e.implements.iter()) {
                    tipo(a, *t, &mut v);
                }
                for k in &e.constants {
                    anotacoes(a, &k.metadata, &mut v);
                    for t in k.type_args.iter() {
                        tipo(a, *t, &mut v);
                    }
                }
            }
            DeclKind::Extension(x) => {
                parametros_de_tipo(a, &x.type_params, &mut v);
                tipo(a, x.on, &mut v);
            }
            DeclKind::ExtensionType(x) => {
                parametros_de_tipo(a, &x.type_params, &mut v);
                anotacoes(a, &x.representation_metadata, &mut v);
                tipo(a, x.representation_type, &mut v);
                for t in x.implements.iter() {
                    tipo(a, *t, &mut v);
                }
            }
            DeclKind::Typedef(t) => {
                parametros_de_tipo(a, &t.type_params, &mut v);
                match &t.kind {
                    ast::TypedefKind::Alias(x) => tipo(a, *x, &mut v),
                    ast::TypedefKind::Legacy { return_type, parameters } => {
                        if let Some(r) = return_type {
                            tipo(a, *r, &mut v);
                        }
                        parametros(a, parameters, &mut v);
                    }
                }
            }
            DeclKind::Function(f) => declaradas.push(*f),
            DeclKind::Variables(l) => {
                if let Some(t) = l.ty {
                    tipo(a, t, &mut v);
                }
            }
        }
    }
    for m in &a.members {
        anotacoes(a, &m.metadata, &mut v);
        match &m.kind {
            MemberKind::Field(l) => {
                if let Some(t) = l.ty {
                    tipo(a, t, &mut v);
                }
            }
            MemberKind::Method(f) => declaradas.push(*f),
            MemberKind::Constructor(k) => {
                parametros(a, &k.parameters, &mut v);
                if let Some(r) = &k.redirect {
                    tipo(a, r.ty, &mut v);
                }
            }
        }
    }
    for f in declaradas {
        let f = a.function(f);
        if let Some(r) = f.return_type {
            tipo(a, r, &mut v);
        }
        parametros_de_tipo(a, &f.type_params, &mut v);
        if let Some(ps) = &f.parameters {
            parametros(a, ps, &mut v);
        }
    }
    v
}

fn tipo(a: &Ast, t: TypeId, v: &mut Vec<TypeId>) {
    v.push(t);
    match &a.ty(t).kind {
        ast::TypeKind::Named { args, .. } => {
            for x in args.iter() {
                tipo(a, *x, v);
            }
        }
        ast::TypeKind::Void => {}
        ast::TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type {
                tipo(a, *r, v);
            }
            parametros_de_tipo(a, type_params, v);
            parametros(a, parameters, v);
        }
        ast::TypeKind::Record { positional, named } => {
            for x in positional.iter() {
                tipo(a, *x, v);
            }
            for (_, x) in named.iter() {
                tipo(a, *x, v);
            }
        }
    }
}

fn parametros_de_tipo(a: &Ast, ps: &[ast::TypeParameter], v: &mut Vec<TypeId>) {
    for p in ps {
        anotacoes(a, &p.metadata, v);
        if let Some(b) = p.bound {
            tipo(a, b, v);
        }
    }
}

fn parametros(a: &Ast, ps: &[ast::Parameter], v: &mut Vec<TypeId>) {
    for p in ps {
        anotacoes(a, &p.metadata, v);
        if let Some(t) = p.ty {
            tipo(a, t, v);
        }
        parametros_de_tipo(a, &p.function_type_params, v);
        if let Some(fp) = &p.function_parameters {
            parametros(a, fp, v);
        }
    }
}

fn anotacoes(a: &Ast, lista: &[ast::Annotation], v: &mut Vec<TypeId>) {
    for x in lista {
        for t in &x.type_args {
            tipo(a, *t, v);
        }
    }
}

/// Os pares `(UnitId, ast::TypeId)` guardados nos elementos.
fn pares(programa: &mut Program) -> Vec<&mut (UnitId, TypeId)> {
    let mut v: Vec<&mut (UnitId, TypeId)> = Vec::new();
    for c in programa.classes.iter_mut() {
        let crate::model::ClassElement { type_params, supertype, mixins, interfaces, on, .. } = c;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.extend(supertype.as_mut());
        v.extend(mixins.iter_mut());
        v.extend(interfaces.iter_mut());
        v.extend(on.iter_mut());
    }
    for x in programa.extensions.iter_mut() {
        let crate::model::ExtensionElement { type_params, on, .. } = x;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.push(on);
    }
    for t in programa.typedefs.iter_mut() {
        v.extend(t.type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
    }
    v
}

fn nome_da_decl(k: &DeclKind) -> Option<dartforge_intern::SymbolId> {
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

fn nome_do_membro(k: &MemberKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        MemberKind::Constructor(c) => c.name.map(|n| n.sym),
        MemberKind::Field(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}
