//! O décimo sexto lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), do conjunto
//! `recommended`, pela semântica da unidade:
//!
//! * `avoid_function_literals_in_foreach_calls`: `alvo.forEach((…) {…})` com
//!   o alvo `Iterable`, sem `?.` na cadeia (`containsNullAwareInvocationInChain`),
//!   sem invocação de método no alvo (`_hasMethodChaining`) e fora de cascata
//!   (o ancestral mais próximo que é instrução ou cascata).
//! * `avoid_renaming_method_parameters`: o `lookUpInheritedMethod` (a cadeia
//!   de superclasses com os mixins, sem a própria classe) e os posicionais.
//! * `implementation_imports`: a URI da biblioteca importada com `src` no
//!   segundo segmento, importada de uma unidade `package:` de outro pacote.
//! * `null_closures`: as tabelas do emissor (construtores, métodos estáticos
//!   e de instância), com o `extendsClass` pela cadeia de superclasses e o
//!   índice posicional contado sobre todos os argumentos.
//! * `prefer_collection_literals`: `Map()`, `LinkedHashMap()`, `Set()`,
//!   `LinkedHashSet()`, `Set.from([…])`/`Set.of([…])` e `[…].toSet()`, com o
//!   `approximateContextType` do linter.
//! * `prefer_interpolation_to_compose_strings`: a cadeia de `+`
//!   (`chainedAdditions`), cada nó binário visitado.
//! * `unnecessary_nullable_for_final_variable_declarations`: campos privados
//!   ou estáticos, variáveis de topo e locais `final`/`const`, e os padrões
//!   de declaração `final` de record e de lista.
//! * `unnecessary_this`: o `this.` de inicializador de campo, e o `this.x`
//!   cujo nome o escopo (`super::escopo`, o `resolveNameInScope`) resolve ao
//!   mesmo elemento, ou a nada, ou ao outro acessor de uma classe.
//! * `prefer_final_fields`: os campos privados que nenhuma escrita da
//!   biblioteca muda, pelo elemento.
//! * `overridden_fields`: a ordem do `_findAllSupertypesAndMixins` (a
//!   superclasse antes dos mixins da classe).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::escopo::{Escopos, Especie};
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, VariableId, VariableRef};
use dartforge_frontend::ast::{
    Ast, BinaryOp, DeclId, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, FunctionBody, Initializer, MemberKind, ParameterKind,
    PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::{HashMap, HashSet};

fn nome_da_biblioteca(s: &super::Semantica<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

/// `isSameAs(nome, biblioteca)`: o elemento do tipo de interface.
fn classe_e(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, nome: &str, biblioteca: &str) -> bool {
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let k = s.program.class(*class);
            interner.resolve(k.name) == nome && nome_da_biblioteca(s, interner, k.library).as_deref() == Some(biblioteca)
        }
        _ => false,
    }
}

/// `implementsInterface(nome, biblioteca)`.
fn implementa(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, nome: &str, biblioteca: &str) -> bool {
    let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = s.table.get(t) else { return false };
    let e = |k: ClassId| {
        let x = s.program.class(k);
        interner.resolve(x.name) == nome && nome_da_biblioteca(s, interner, x.library).as_deref() == Some(biblioteca)
    };
    e(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e(*k))))
}

/// A chamada é `MethodInvocation` no analyzer (o nome resolvido a variável
/// ou a getter vira `FunctionExpressionInvocation`).
pub(super) fn e_invocacao_de_metodo(s: &super::Semantica<'_>, a: &Ast, alvo: ExprId) -> bool {
    if !matches!(a.expr(alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return false;
    }
    let nao_getter = |f: FunctionElementId| s.program.function(f).kind != FunctionKind::Getter;
    match s.corpo.get_resolved(alvo) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => nao_getter(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(Element::Function(f))) => nao_getter(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => nao_getter(*member),
        Some(Resolved::Local(_)) => s.corpo.declaracao_local(alvo).is_some_and(|d| {
            a.stmts.iter().any(|x| matches!(&x.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
        }),
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// O `TypeSystemImpl.isNullable`/`isNonNullable` sintáticos.
pub(super) fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Never => false,
    }
}

pub(super) fn nao_anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => nao_anulavel(table, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && nao_anulavel(table, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && nao_anulavel(table, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        Type::Never => true,
    }
}

/// O `ClassId` de uma declaração da unidade.
fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// Os segmentos do caminho de uma URI (`Uri.pathSegments`).
fn segmentos(uri: &str) -> Vec<&str> {
    let Some((_, resto)) = uri.split_once(':') else { return uri.split('/').filter(|x| !x.is_empty()).collect() };
    let resto = resto.split(['?', '#']).next().unwrap_or("");
    let caminho = match resto.strip_prefix("//") {
        Some(r) => r.find('/').map_or("", |i| &r[i..]),
        None => resto,
    };
    let caminho = caminho.strip_prefix('/').unwrap_or(caminho);
    if caminho.is_empty() {
        return Vec::new();
    }
    caminho.split('/').collect()
}

/// `approximateContextType`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Contexto {
    Nenhum,
    Invalido,
    Tipo(TypeId),
}

/// Os pais das expressões e das funções (pelo passeio).
pub(super) struct Pais {
    pub(super) expr: HashMap<ExprId, No>,
    pub(super) funcao: HashMap<dartforge_frontend::ast::FunctionId, No>,
    pub(super) ancestrais: HashMap<ExprId, Vec<No>>,
}

pub(super) fn pais_da_unidade(u: Unidade<'_>, guardar: &dyn Fn(ExprId) -> bool) -> Pais {
    let mut p = Pais { expr: HashMap::new(), funcao: HashMap::new(), ancestrais: HashMap::new() };
    andar(u, &mut |no, ancestrais| {
        match no {
            No::Expr(e) => {
                if let Some(x) = ancestrais.last() {
                    p.expr.insert(e, *x);
                }
                if guardar(e) {
                    p.ancestrais.insert(e, ancestrais.to_vec());
                }
            }
            No::Funcao(f) => {
                if let Some(x) = ancestrais.last() {
                    p.funcao.insert(f, *x);
                }
            }
            _ => {}
        }
    });
    p
}

/// O tipo de retorno escrito de uma função (`returnType?.type`).
fn retorno_escrito(s: &super::Semantica<'_>, a: &Ast, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    let t = a.function(f).return_type?;
    s.corpo.tipos_de_anotacoes.get(&t).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unidade, t)).copied())
}

/// O tipo de retorno do elemento de uma função declarada (método, de topo
/// ou local).
fn retorno_do_elemento(s: &super::Semantica<'_>, a: &Ast, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    for (k, fe) in s.program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = fe.node
            && unit == s.unidade
            && function == f
        {
            return s.outline.functions.get(k).map(|d| d.return_type);
        }
    }
    let n = a.function(f).name?;
    match s.table.get(s.corpo.tipo_local(n.span.start)?) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }
}

/// `_expectedReturnableOrYieldableType`.
fn retornavel(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, f: dartforge_frontend::ast::FunctionId, ret: Option<TypeId>) -> Option<TypeId> {
    let ret = ret?;
    if !matches!(s.table.get(ret), Type::Interface { .. }) {
        return None;
    }
    let primeiro = || match s.table.get(ret) {
        Type::Interface { args, .. } => args.first().copied(),
        _ => None,
    };
    use dartforge_frontend::ast::AsyncModifier as M;
    match a.function(f).modifier {
        M::Async if classe_e(s, interner, ret, "Future", "dart.async") => return primeiro(),
        M::AsyncStar if classe_e(s, interner, ret, "Stream", "dart.async") => return primeiro(),
        M::SyncStar if classe_e(s, interner, ret, "Iterable", "dart.core") => return primeiro(),
        _ => {}
    }
    Some(ret)
}

/// `FunctionBody.expectedReturnType` do corpo da função `f`.
fn retorno_esperado(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, pais: &Pais, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    let e_metodo = a.members.iter().any(|m| matches!(m.kind, MemberKind::Method(g) if g == f));
    let literal = a.exprs.iter().position(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f)).map(|k| ExprId(k as u32));
    if e_metodo {
        return retornavel(s, interner, a, f, retorno_do_elemento(s, a, f));
    }
    match literal {
        // Uma `FunctionExpression` solta: o contexto aproximado dela.
        Some(e) => match contexto_aproximado(s, interner, a, pais, e) {
            Contexto::Tipo(t) => match s.table.get(t) {
                Type::Function { ret, .. } => retornavel(s, interner, a, f, Some(*ret)),
                _ => None,
            },
            _ => None,
        },
        // A de uma `FunctionDeclaration` (de topo ou local).
        None => retornavel(s, interner, a, f, retorno_do_elemento(s, a, f)),
    }
}

/// `approximateContextType` (`extensions.dart:377`).
pub(super) fn contexto_aproximado(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, pais: &Pais, eu: ExprId) -> Contexto {
    let mut filho = eu;
    let mut pai = pais.expr.get(&eu).copied();
    while let Some(No::Expr(p)) = pai {
        match &a.expr(p).kind {
            ExprKind::Parenthesized(_) => {}
            ExprKind::Cascade { target, .. } if *target == filho => {}
            _ => break,
        }
        filho = p;
        pai = pais.expr.get(&p).copied();
    }
    let tipo = |e: ExprId| s.corpo.get_type(e).map_or(Contexto::Nenhum, Contexto::Tipo);
    match pai {
        Some(No::Expr(p)) => match &a.expr(p).kind {
            // Argumento (posicional ou nomeado).
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                let Some(arg) = arguments.args.iter().find(|x| x.value == filho) else { return Contexto::Nenhum };
                // O `staticParameterElement` do próprio nó (posicional) ou o
                // da `NamedExpression`.
                let chave = if arg.name.is_some() { filho } else { eu };
                if arg.name.is_none() && filho != eu {
                    return Contexto::Invalido;
                }
                s.corpo.tipos_de_parametros.get(&chave).copied().map_or(Contexto::Invalido, Contexto::Tipo)
            }
            ExprKind::Assign { .. } | ExprKind::Conditional { .. } => tipo(p),
            _ => Contexto::Nenhum,
        },
        // Corpo de expressão.
        Some(No::Funcao(f)) if matches!(a.function(f).body, FunctionBody::Expression(x) if x == filho) => {
            let literal = a.exprs.iter().position(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f)).map(|k| ExprId(k as u32));
            match literal {
                Some(fe) => match contexto_aproximado(s, interner, a, pais, fe) {
                    Contexto::Tipo(t) => match s.table.get(t) {
                        Type::Function { ret, .. } => Contexto::Tipo(*ret),
                        _ => Contexto::Nenhum,
                    },
                    _ => Contexto::Nenhum,
                },
                None => retorno_escrito(s, a, f).map_or(Contexto::Nenhum, Contexto::Tipo),
            }
        }
        // Inicializador de campo de construtor: o tipo do campo.
        Some(No::Membro(m)) => {
            let MemberKind::Constructor(k) = &a.member(m).kind else {
                return variavel_de_lista(s, a, Some(m), None, filho);
            };
            let Some(nome) = k.initializers.iter().find_map(|i| match i {
                Initializer::Field { name, value, .. } if *value == filho => Some(*name),
                _ => None,
            }) else {
                return Contexto::Nenhum;
            };
            // O campo da mesma classe: o que tem o nome entre os membros da
            // declaração que contém o construtor.
            let classe = a.decls.iter().find_map(|d| {
                let membros = match &d.kind {
                    DeclKind::Class(x) => &x.members,
                    DeclKind::Enum(x) => &x.members,
                    DeclKind::Mixin(x) => &x.members,
                    DeclKind::ExtensionType(x) => &x.members,
                    _ => return None,
                };
                membros.contains(&m).then_some(membros)
            });
            let Some(membros) = classe else { return Contexto::Nenhum };
            for &mm in membros {
                if let MemberKind::Field(l) = &a.member(mm).kind
                    && let Some(index) = l.variables.iter().position(|x| x.name.sym == nome.sym)
                {
                    return tipo_de_variavel(s, VariableRef::Field { unit: s.unidade, member: mm, index }).map_or(Contexto::Nenhum, Contexto::Tipo);
                }
            }
            Contexto::Nenhum
        }
        Some(No::Stmt(st)) => match &a.stmt(st).kind {
            StmtKind::Return(Some(_)) | StmtKind::Yield { .. } => {
                let Some(anc) = pais.ancestrais.get(&eu) else { return Contexto::Nenhum };
                // O corpo de função mais próximo.
                let f = anc.iter().rev().find_map(|n| match n {
                    No::Funcao(f) => Some(*f),
                    _ => None,
                });
                match f.and_then(|f| retorno_esperado(s, interner, a, pais, f)) {
                    Some(t) => Contexto::Tipo(t),
                    None => Contexto::Nenhum,
                }
            }
            StmtKind::Variables(l) => {
                if l.ty.is_none() {
                    return Contexto::Nenhum;
                }
                let t = l.ty.and_then(|t| s.corpo.tipos_de_anotacoes.get(&t).copied());
                t.map_or(Contexto::Nenhum, Contexto::Tipo)
            }
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                l.ty.and_then(|t| s.corpo.tipos_de_anotacoes.get(&t).copied()).map_or(Contexto::Nenhum, Contexto::Tipo)
            }
            _ => Contexto::Nenhum,
        },
        Some(No::Decl(d)) => variavel_de_lista(s, a, None, Some(d), filho),
        _ => Contexto::Nenhum,
    }
}

/// O tipo escrito da lista de variáveis (campo ou de topo) que inicializa
/// `filho`.
fn variavel_de_lista(s: &super::Semantica<'_>, a: &Ast, m: Option<dartforge_frontend::ast::MemberId>, d: Option<DeclId>, filho: ExprId) -> Contexto {
    let lista = match (m, d) {
        (Some(m), _) => match &a.member(m).kind {
            MemberKind::Field(l) => l,
            _ => return Contexto::Nenhum,
        },
        (_, Some(d)) => match &a.decl(d).kind {
            DeclKind::Variables(l) => l,
            _ => return Contexto::Nenhum,
        },
        _ => return Contexto::Nenhum,
    };
    if !lista.variables.iter().any(|v| v.initializer == Some(filho)) {
        return Contexto::Nenhum;
    }
    let Some(t) = lista.ty else { return Contexto::Nenhum };
    s.outline.tipos_escritos.get(&(s.unidade, t)).copied().map_or(Contexto::Nenhum, Contexto::Tipo)
}

/// O tipo do elemento de uma variável de topo ou campo.
pub(super) fn tipo_de_variavel(s: &super::Semantica<'_>, r: VariableRef) -> Option<TypeId> {
    let v = s.program.variables.iter().position(|x| x.node == r)?;
    let d = s.outline.variables.get(v)?;
    d.declared_type.or(d.inferred)
}

/// O `VariableId` do campo escrito por `e` (`writeElement.canonicalElement`).
fn campo_escrito(s: &super::Semantica<'_>, corpo: &dartforge_types::resolved::UnitBodyTypes, e: ExprId) -> Option<VariableId> {
    match corpo.get_resolved(e)? {
        Resolved::Member { member: MemberRef::Variable(v), .. } => Some(*v),
        Resolved::Member { member: MemberRef::Function(f), .. } => s.program.function(*f).variable,
        Resolved::Element(Element::Variable(v)) => Some(*v),
        Resolved::Element(Element::Function(f)) => s.program.function(*f).variable,
        _ => None,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let fonte = u.fonte;
    let program = s.program;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let regras = [
        "avoid_function_literals_in_foreach_calls",
        "avoid_renaming_method_parameters",
        "implementation_imports",
        "null_closures",
        "prefer_collection_literals",
        "prefer_interpolation_to_compose_strings",
        "unnecessary_nullable_for_final_variable_declarations",
        "unnecessary_this",
        "prefer_final_fields",
        "overridden_fields",
    ];
    if !regras.iter().any(|r| ligada(r)) {
        return out;
    }
    let pais = pais_da_unidade(u, &|e| {
        matches!(a.expr(e).kind, ExprKind::Call { .. } | ExprKind::InstanceCreation { .. } | ExprKind::This)
    });
    let lib = program.unit(s.unidade).library;
    let doc = dartforge_frontend::comentarios::Comentarios::de(fonte);
    let tem_doc = |metadata: &[dartforge_frontend::ast::Annotation], inicio: usize| {
        let depois = match metadata.last() {
            Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
            None => inicio,
        };
        doc.dart_doc(fonte, depois).is_some() || metadata.iter().rev().any(|m| doc.dart_doc(fonte, m.span.start).is_some())
    };

    // `avoid_function_literals_in_foreach_calls`.
    if ligada("avoid_function_literals_in_foreach_calls") {
        // `containsNullAwareInvocationInChain`.
        fn null_aware_na_cadeia(a: &Ast, e: ExprId) -> bool {
            match &a.expr(e).kind {
                ExprKind::Property { target, null_aware, .. } => {
                    if *null_aware {
                        return true;
                    }
                    // `a.b` com `a` identificador é `PrefixedIdentifier`.
                    if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                        return false;
                    }
                    null_aware_na_cadeia(a, *target)
                }
                ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                    ExprKind::Property { target: t, null_aware, .. } => *null_aware || null_aware_na_cadeia(a, *t),
                    _ => false,
                },
                ExprKind::Index { target, null_aware, .. } => *null_aware || null_aware_na_cadeia(a, *target),
                _ => false,
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: alvo, name, null_aware } = &a.expr(*target).kind else { continue };
            if interner.resolve(name.sym) != "forEach" || matches!(a.expr(*alvo).kind, ExprKind::CascadeTarget) || !e_invocacao_de_metodo(s, a, *target) {
                continue;
            }
            let Some(primeiro) = arguments.args.first() else { continue };
            if primeiro.name.is_some() || !matches!(a.expr(primeiro.value).kind, ExprKind::FunctionExpression(_)) {
                continue;
            }
            if !s.corpo.get_type(*alvo).is_some_and(|t| implementa(s, interner, t, "Iterable", "dart.core")) {
                continue;
            }
            if *null_aware || null_aware_na_cadeia(a, *alvo) {
                continue;
            }
            // `_hasMethodChaining`.
            let mut x = *alvo;
            let encadeado = loop {
                match &a.expr(x).kind {
                    ExprKind::Property { target, .. } => x = *target,
                    ExprKind::Call { target, .. } if e_invocacao_de_metodo(s, a, *target) => break true,
                    _ => break false,
                }
            };
            if encadeado {
                continue;
            }
            // `_isInsideCascade`.
            let em_cascata = pais.ancestrais.get(&id).is_some_and(|anc| {
                for n in anc.iter().rev() {
                    match n {
                        No::Stmt(_) => return false,
                        No::Expr(p) if matches!(a.expr(*p).kind, ExprKind::Cascade { .. }) => return true,
                        _ => {}
                    }
                }
                false
            });
            if !em_cascata {
                relatar(&c::AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS, name.span, &[]);
            }
        }
    }

    // `avoid_renaming_method_parameters`.
    if ligada("avoid_renaming_method_parameters") {
        for (k, d) in a.decls.iter().enumerate() {
            let (nome, membros) = match &d.kind {
                DeclKind::Class(x) => (x.name, &x.members),
                DeclKind::Mixin(x) => (x.name, &x.members),
                DeclKind::Enum(x) => (x.name, &x.members),
                DeclKind::ExtensionType(x) => (x.name, &x.members),
                _ => continue,
            };
            if interner.resolve(nome.sym).starts_with('_') {
                continue;
            }
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            for &mid in membros {
                let m = a.member(mid);
                let MemberKind::Method(fid) = &m.kind else { continue };
                let f = a.function(*fid);
                if f.static_ || tem_doc(&m.metadata, m.span.start) {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                let texto = interner.resolve(n.sym);
                // `lookUpInheritedMethod`: a cadeia, sem a própria classe.
                let mut pai: Option<FunctionElementId> = None;
                let mut vistos: HashSet<ClassId> = HashSet::new();
                let mut atual = Some(classe);
                'busca: while let Some(cl) = atual {
                    if !vistos.insert(cl) {
                        break;
                    }
                    let e = program.class(cl);
                    let candidatas = std::iter::once(cl).chain(e.mixin_classes.iter().rev().copied());
                    for cc in candidatas {
                        if cc == classe {
                            continue;
                        }
                        let k2 = program.class(cc);
                        if let Some(&g) = k2.instance_members.get(&n.sym) {
                            let ge = program.function(g);
                            let metodo = matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator);
                            if metodo && !ge.static_ && (!texto.starts_with('_') || k2.library == lib) {
                                pai = Some(g);
                                break 'busca;
                            }
                        }
                    }
                    atual = e.supertype_class;
                }
                let Some(pai) = pai else { continue };
                let FunctionRef::Function { unit, function } = program.function(pai).node else { continue };
                let a2 = &program.unit(unit).ast;
                let Some(ps_pai) = &a2.function(function).parameters else { continue };
                let posicionais: Vec<_> = ps.iter().filter(|p| p.kind != ParameterKind::Named).collect();
                let do_pai: Vec<_> = ps_pai.iter().filter(|p| p.kind != ParameterKind::Named).collect();
                for (p, q) in posicionais.iter().zip(do_pai.iter()) {
                    let (Some(pn), Some(qn)) = (p.name, q.name) else { continue };
                    let (pt, qt) = (interner.resolve(pn.sym), interner.resolve(qn.sym));
                    if pt != qt {
                        relatar(&c::AVOID_RENAMING_METHOD_PARAMETERS, pn.span, &[pt, qt]);
                    }
                }
            }
        }
    }

    // `implementation_imports`.
    if ligada("implementation_imports") {
        let fonte_uri = program.unit(s.unidade).uri.clone();
        if fonte_uri.starts_with("package:") {
            for imp in program.library(lib).imports.iter().filter(|i| i.unit == s.unidade) {
                let Some(d) = u.unit.directives.get(imp.directive) else { continue };
                let DirectiveKind::Import { uri, .. } = &d.kind else { continue };
                let importada = &program.library(imp.library).uri;
                let si = segmentos(importada);
                let implementacao = si.len() > 2 && si[1] == "src";
                let sf = segmentos(&fonte_uri);
                let mesmo = !si.is_empty() && !sf.is_empty() && si[0] == sf[0];
                if implementacao && !mesmo {
                    relatar(&c::IMPLEMENTATION_IMPORTS, uri.span, &[]);
                }
            }
        }
    }

    // `null_closures`.
    if ligada("null_closures") {
        // (biblioteca, tipo, nome, posicionais, nomeados)
        type Def = (&'static str, Option<&'static str>, Option<&'static str>, &'static [usize], &'static [&'static str]);
        const CONSTRUTORES: [Def; 6] = [
            ("dart.async", Some("Future"), None, &[0], &[]),
            ("dart.async", Some("Future"), Some("microtask"), &[0], &[]),
            ("dart.async", Some("Future"), Some("sync"), &[0], &[]),
            ("dart.async", Some("Timer"), None, &[1], &[]),
            ("dart.async", Some("Timer"), Some("periodic"), &[1], &[]),
            ("dart.core", Some("List"), Some("generate"), &[1], &[]),
        ];
        const ESTATICOS: [Def; 5] = [
            ("dart.async", None, Some("scheduleMicrotask"), &[0], &[]),
            ("dart.async", Some("Future"), Some("doWhile"), &[0], &[]),
            ("dart.async", Some("Future"), Some("forEach"), &[1], &[]),
            ("dart.async", Some("Future"), Some("wait"), &[], &["cleanUp"]),
            ("dart.async", Some("Timer"), Some("run"), &[0], &[]),
        ];
        const DE_INSTANCIA: [Def; 26] = [
            ("dart.core", Some("Iterable"), Some("any"), &[0], &[]),
            ("dart.async", Some("Future"), Some("complete"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("every"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("expand"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("firstWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("forEach"), &[0], &[]),
            ("dart.core", Some("Map"), Some("forEach"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("fold"), &[1], &[]),
            ("dart.core", Some("Iterable"), Some("lastWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("map"), &[0], &[]),
            ("dart.core", Some("Map"), Some("putIfAbsent"), &[1], &[]),
            ("dart.core", Some("Iterable"), Some("reduce"), &[0], &[]),
            ("dart.collection", Some("Queue"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("List"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("Set"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("String"), Some("replaceAllMapped"), &[1], &[]),
            ("dart.core", Some("String"), Some("replaceFirstMapped"), &[1], &[]),
            ("dart.collection", Some("Queue"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("List"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("Set"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("singleWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("skipWhile"), &[0], &[]),
            ("dart.core", Some("String"), Some("splitMapJoin"), &[], &["onMatch", "onNonMatch"]),
            ("dart.core", Some("Iterable"), Some("takeWhile"), &[0], &[]),
            ("dart.async", Some("Future"), Some("then"), &[0], &["onError"]),
            ("dart.core", Some("Iterable"), Some("where"), &[0], &[]),
        ];
        let checar = |argumentos: &dartforge_frontend::ast::Arguments, posicoes: &[usize], nomes: &[&str], relatar: &mut dyn FnMut(Span)| {
            for (i, arg) in argumentos.args.iter().enumerate() {
                let nulo = matches!(a.expr(arg.value).kind, ExprKind::Null);
                match arg.name {
                    Some(n) => {
                        if nulo && nomes.contains(&interner.resolve(n.sym)) {
                            relatar(Span { start: n.span.start, end: a.expr(arg.value).span.end });
                        }
                    }
                    None => {
                        if nulo && posicoes.contains(&i) {
                            relatar(a.expr(arg.value).span);
                        }
                    }
                }
            }
        };
        let mut achados: Vec<Span> = Vec::new();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            // Criação de instância.
            if let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(id)
                && let ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } = &e.kind
            {
                let nome_do_construtor = interner.resolve(program.function(*f).name);
                let nome_do_construtor = (!nome_do_construtor.is_empty()).then_some(nome_do_construtor);
                let Some(t) = s.corpo.get_type(id) else { continue };
                for (biblioteca, tipo, nome, posicoes, nomes) in CONSTRUTORES {
                    if nome != nome_do_construtor {
                        continue;
                    }
                    // `extendsClass`: a cadeia de superclasses.
                    let estende = match table.get(t) {
                        Type::Interface { class, .. } => {
                            let mut vistos = HashSet::new();
                            let mut atual = Some(*class);
                            let mut sim = false;
                            while let Some(cl) = atual {
                                if !vistos.insert(cl) {
                                    break;
                                }
                                let x = program.class(cl);
                                if Some(interner.resolve(x.name)) == tipo && nome_da_biblioteca(s, interner, x.library).as_deref() == Some(biblioteca) {
                                    sim = true;
                                    break;
                                }
                                atual = x.supertype_class;
                            }
                            sim
                        }
                        _ => false,
                    };
                    if estende {
                        checar(arguments, posicoes, nomes, &mut |sp| achados.push(sp));
                    }
                }
                continue;
            }
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
            if !e_invocacao_de_metodo(s, a, *target) {
                continue;
            }
            let metodo = interner.resolve(name.sym);
            // Alvo que é uma classe: os estáticos.
            let classe_alvo = match (&a.expr(*alvo).kind, s.corpo.get_resolved(*alvo)) {
                (ExprKind::Identifier(_) | ExprKind::Property { .. }, Some(Resolved::Element(Element::Class(cl)))) if program.class(*cl).kind == ClassKind::Class => Some(*cl),
                _ => None,
            };
            if let Some(cl) = classe_alvo {
                let nome_da_classe = interner.resolve(program.class(cl).name);
                for (_, tipo, nome, posicoes, nomes) in ESTATICOS {
                    if nome == Some(metodo) && tipo == Some(nome_da_classe) {
                        checar(arguments, posicoes, nomes, &mut |sp| achados.push(sp));
                    }
                }
                continue;
            }
            let Some(t) = s.corpo.get_type(*alvo) else { continue };
            let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = table.get(t) else { continue };
            if program.class(*class).decl.is_none() && program.class(*class).kind == ClassKind::MixinApplication {
                continue;
            }
            let mut classes = vec![*class];
            if let Some(d) = s.outline.hierarchy.get(*class) {
                let mut sup: Vec<ClassId> = d.supertypes.keys().copied().collect();
                sup.sort();
                classes.extend(sup);
            }
            let def = classes.iter().find_map(|cl| {
                let x = program.class(*cl);
                let b = nome_da_biblioteca(s, interner, x.library)?;
                DE_INSTANCIA.iter().find(|(bib, tipo, nome, _, _)| *bib == b && *tipo == Some(interner.resolve(x.name)) && *nome == Some(metodo))
            });
            if let Some((_, _, _, posicoes, nomes)) = def {
                checar(arguments, posicoes, nomes, &mut |sp| achados.push(sp));
            }
        }
        for sp in achados {
            relatar(&c::NULL_CLOSURES, sp, &[]);
        }
    }

    // `prefer_collection_literals`.
    if ligada("prefer_collection_literals") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            match &e.kind {
                ExprKind::InstanceCreation { .. } | ExprKind::Call { .. } => {
                    let argumentos = match &e.kind {
                        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                        _ => continue,
                    };
                    if let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(id) {
                        // O tipo escrito que é um `typedef`.
                        let escrito_alias = match &e.kind {
                            ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
                                dartforge_frontend::ast::TypeKind::Named { name, .. } => {
                                    let elemento = match &name[..] {
                                        [n] => program.lookup_na_unidade(s.unidade, n.sym),
                                        [p, n] => program.lookup_prefixed_na_unidade(s.unidade, p.sym, n.sym),
                                        _ => None,
                                    };
                                    matches!(elemento.and_then(|b| b.getter), Some(Element::Typedef(_)))
                                }
                                _ => false,
                            },
                            ExprKind::Call { target, .. } => {
                                let raiz = match &a.expr(*target).kind {
                                    ExprKind::Identifier(n) => Some(*n),
                                    ExprKind::Property { target: t2, .. } => match &a.expr(*t2).kind {
                                        ExprKind::Identifier(n) => Some(*n),
                                        _ => None,
                                    },
                                    _ => None,
                                };
                                raiz.is_some_and(|n| matches!(program.lookup_na_unidade(s.unidade, n.sym).and_then(|b| b.getter), Some(Element::Typedef(_))))
                            }
                            _ => false,
                        };
                        if escrito_alias {
                            continue;
                        }
                        let Some(t) = s.corpo.get_type(id) else { continue };
                        let nome_do_construtor = interner.resolve(program.function(*f).name);
                        let sem_nome = nome_do_construtor.is_empty();
                        let e_hash_map = classe_e(s, interner, t, "LinkedHashMap", "dart.collection");
                        let e_hash_set = classe_e(s, interner, t, "LinkedHashSet", "dart.collection");
                        let e_map = classe_e(s, interner, t, "Map", "dart.core");
                        let e_set = classe_e(s, interner, t, "Set", "dart.core");
                        if e_hash_map || e_hash_set {
                            let ctx = contexto_aproximado(s, interner, a, &pais, id);
                            if ctx == Contexto::Invalido {
                                continue;
                            }
                            if let Contexto::Tipo(c2) = ctx
                                && classe_e(s, interner, c2, if e_hash_map { "LinkedHashMap" } else { "LinkedHashSet" }, "dart.collection")
                            {
                                continue;
                            }
                        }
                        if e_map || e_hash_map {
                            if sem_nome && argumentos.args.is_empty() {
                                relatar(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                            }
                            continue;
                        }
                        if e_set || e_hash_set {
                            if sem_nome {
                                if argumentos.args.is_empty() {
                                    relatar(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                                }
                            } else if (nome_do_construtor == "from" || nome_do_construtor == "of")
                                && argumentos.args.len() == 1
                                && argumentos.args[0].name.is_none()
                                && matches!(a.expr(argumentos.args[0].value).kind, ExprKind::List { .. })
                            {
                                relatar(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                            }
                        }
                        continue;
                    }
                    // `[…].toSet()`.
                    if let ExprKind::Call { target, .. } = &e.kind
                        && let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind
                        && interner.resolve(name.sym) == "toSet"
                        && matches!(a.expr(*alvo).kind, ExprKind::List { .. })
                        && e_invocacao_de_metodo(s, a, *target)
                    {
                        relatar(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }

    // `prefer_interpolation_to_compose_strings`.
    if ligada("prefer_interpolation_to_compose_strings") {
        fn adicoes(a: &Ast, e: ExprId) -> Vec<ExprId> {
            match &a.expr(e).kind {
                ExprKind::Binary { op: BinaryOp::Add, left, right } => {
                    let mut v = adicoes(a, *left);
                    v.push(*right);
                    v
                }
                ExprKind::Binary { .. } => Vec::new(),
                _ => vec![e],
            }
        }
        let e_literal = |e: ExprId| matches!(a.expr(e).kind, ExprKind::String(_));
        let cru_simples = |e: ExprId| match &a.expr(e).kind {
            ExprKind::String(l) => {
                let lits = super::cordas::literais(fonte, l.span);
                lits.len() == 1 && !lits[0].interpolado() && lits[0].crua
            }
            _ => false,
        };
        let to_string_com_args = |e: ExprId| match &a.expr(e).kind {
            ExprKind::Call { target, arguments } => {
                let nome = match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } | ExprKind::Identifier(name) => Some(*name),
                    _ => None,
                };
                nome.is_some_and(|n| interner.resolve(n.sym) == "toString") && !arguments.args.is_empty() && e_invocacao_de_metodo(s, a, *target)
            }
            _ => false,
        };
        let e_string = |e: ExprId| s.corpo.get_type(e).is_some_and(|t| classe_e(s, interner, t, "String", "dart.core"));
        for (k, e) in a.exprs.iter().enumerate() {
            if !matches!(e.kind, ExprKind::Binary { op: BinaryOp::Add, .. }) {
                continue;
            }
            let cadeia = adicoes(a, ExprId(k as u32));
            let mut i = 0;
            while i + 1 < cadeia.len() {
                let (l, r) = (cadeia[i], cadeia[i + 1]);
                let pular = (!e_literal(l) && !e_literal(r))
                    || cru_simples(l)
                    || cru_simples(r)
                    || (e_literal(l) && e_literal(r))
                    || to_string_com_args(l)
                    || to_string_com_args(r);
                if !pular && e_string(l) {
                    relatar(&c::PREFER_INTERPOLATION_TO_COMPOSE_STRINGS, Span { start: a.expr(l).span.start, end: a.expr(r).span.end }, &[]);
                    i += 1;
                }
                i += 1;
            }
        }
    }

    // `unnecessary_nullable_for_final_variable_declarations`.
    if ligada("unnecessary_nullable_for_final_variable_declarations") {
        let mut conferir = |final_ou_const: bool, nome: dartforge_frontend::ast::Name, init: Option<ExprId>, declarado: Option<TypeId>| {
            if !final_ou_const {
                return;
            }
            let (Some(i), Some(d)) = (init.and_then(|i| s.corpo.get_type(i)), declarado) else { return };
            if matches!(table.get(d), Type::Dynamic) {
                return;
            }
            if anulavel(table, d) && nao_anulavel(table, i) {
                relatar(&c::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS, nome.span, &[]);
            }
        };
        for (k, d) in a.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Variables(l) => {
                    for (index, v) in l.variables.iter().enumerate() {
                        let t = tipo_de_variavel(s, VariableRef::TopLevel { unit: s.unidade, decl: DeclId(k as u32), index });
                        conferir(l.final_ || l.const_, v.name, v.initializer, t);
                    }
                }
                DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) => {}
                _ => {}
            }
        }
        for (k, m) in a.members.iter().enumerate() {
            let MemberKind::Field(l) = &m.kind else { continue };
            for (index, v) in l.variables.iter().enumerate() {
                if interner.resolve(v.name.sym).starts_with('_') || l.static_ {
                    let t = tipo_de_variavel(s, VariableRef::Field { unit: s.unidade, member: dartforge_frontend::ast::MemberId(k as u32), index });
                    conferir(l.final_ || l.const_, v.name, v.initializer, t);
                }
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Variables(l) = &st.kind {
                for v in l.variables.iter() {
                    conferir(l.final_ || l.const_, v.name, v.initializer, s.corpo.tipo_local(v.name.span.start));
                }
            }
        }
        // Os padrões de declaração `final` de record e de lista.
        let mut padroes: Vec<dartforge_frontend::ast::PatternId> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::PatternVariables { final_: true, pattern, .. } => padroes.push(*pattern),
                StmtKind::For { init: Some(ForInit::Pattern { final_: true, pattern, .. }), .. } => padroes.push(*pattern),
                _ => {}
            }
        }
        for p in padroes {
            let filhos: Vec<dartforge_frontend::ast::PatternId> = match &a.pattern(p).kind {
                PatternKind::Record { fields } => fields.iter().map(|f| f.pattern).collect(),
                PatternKind::List { elements, .. } => elements
                    .iter()
                    .filter_map(|el| match el {
                        dartforge_frontend::ast::ListPatternElement::Pattern(x) => Some(*x),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            for f in filhos {
                let PatternKind::Variable { name, .. } = &a.pattern(f).kind else { continue };
                let (Some(t), Some(&valor)) = (s.corpo.tipo_local(name.span.start), s.corpo.tipos_casados.get(&f)) else { continue };
                if matches!(table.get(t), Type::Dynamic) {
                    continue;
                }
                if anulavel(table, t) && nao_anulavel(table, valor) {
                    relatar(&c::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS, name.span, &[]);
                }
            }
        }
    }

    // `unnecessary_this`.
    if ligada("unnecessary_this") {
        let mut achados: Vec<Span> = Vec::new();
        // O `this.` do inicializador de campo.
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Field { span, this_: true, .. } = i {
                        achados.push(Span { start: span.start, end: span.start + 4 });
                    }
                }
            }
        }
        let escopos = Escopos::de(a);
        // As escritas (o `writeElement`).
        let mut escritos: HashSet<ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    escritos.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    escritos.insert(*operand);
                }
                _ => {}
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !matches!(e.kind, ExprKind::This) {
                continue;
            }
            let Some(No::Expr(p)) = pais.expr.get(&id).copied() else { continue };
            let ExprKind::Property { target, name, null_aware: false } = &a.expr(p).kind else { continue };
            if *target != id {
                continue;
            }
            // `PropertyAccess` (o `propertyName`) ou `MethodInvocation` (o
            // `methodName`): o elemento da propriedade.
            let setter = escritos.contains(&p);
            let elemento = s.corpo.get_resolved(p);
            let dono_do_elemento: Option<(u32, DeclId)> = match elemento {
                Some(Resolved::Member { class, .. }) => program.class(*class).decl.map(|r| (r.unit.0, r.decl)),
                Some(Resolved::ExtensionMember { extension, .. }) => {
                    let x = &program.extensions[extension.0 as usize];
                    Some((x.decl.unit.0, x.decl.decl))
                }
                _ => None,
            };
            if elemento.is_none() || matches!(elemento, Some(Resolved::Dynamic)) {
                continue;
            }
            let achadas = escopos.procurar(name.sym, e.span.start);
            let mut pedido = Vec::new();
            let mut outro = Vec::new();
            for r in achadas.iter() {
                let casa = match r.especie {
                    Especie::Membro { setter: st, .. } => st == setter,
                    _ => !setter,
                };
                if casa {
                    pedido.push(r);
                } else {
                    outro.push(r);
                }
            }
            let relata = if achadas.is_empty() {
                // O escopo da unidade.
                match program.lookup_na_unidade(s.unidade, name.sym) {
                    // Um elemento de topo (o pedido ou o outro acessor) não é o
                    // membro, nem é de classe.
                    None => true,
                    Some(_) => false,
                }
            } else if let Some(r) = pedido.first() {
                match r.especie {
                    Especie::Membro { classe, .. } => dono_do_elemento == Some((s.unidade.0, classe)),
                    _ => false,
                }
            } else if let Some(r) = outro.first() {
                match r.especie {
                    Especie::Membro { classe, .. } => matches!(a.decl(classe).kind, DeclKind::Class(_)),
                    _ => false,
                }
            } else {
                true
            };
            if relata {
                achados.push(e.span);
            }
        }
        achados.sort_by_key(|s| s.start);
        for sp in achados {
            relatar(&c::UNNECESSARY_THIS, sp, &[]);
        }
    }

    // `prefer_final_fields`.
    if ligada("prefer_final_fields") {
        // Os campos candidatos, na ordem da fonte: (campo, membro, índice,
        // classe da declaração, é `ClassDeclaration`).
        let mut campos: Vec<(VariableId, dartforge_frontend::ast::MemberId, usize, Option<DeclId>)> = Vec::new();
        for (k, d) in a.decls.iter().enumerate() {
            let (membros, e_classe, ext_type) = match &d.kind {
                DeclKind::Class(x) => (&x.members, true, false),
                DeclKind::Mixin(x) => (&x.members, false, false),
                DeclKind::Extension(x) => (&x.members, false, false),
                DeclKind::ExtensionType(x) => (&x.members, false, true),
                _ => continue,
            };
            let classe = classe_da_decl(s, DeclId(k as u32));
            for &mid in membros {
                let MemberKind::Field(l) = &a.member(mid).kind else { continue };
                if (ext_type && !l.static_) || l.final_ || l.const_ {
                    continue;
                }
                for (index, v) in l.variables.iter().enumerate() {
                    let texto = interner.resolve(v.name.sym);
                    if !texto.starts_with('_') {
                        continue;
                    }
                    let Some(vid) = program.variables.iter().position(|x| x.node == VariableRef::Field { unit: s.unidade, member: mid, index }) else { continue };
                    // `overridesField`: um setter concreto herdado.
                    let sobrescreve = classe.is_some_and(|cl| {
                        let chave = interner.lookup(&format!("{texto}_="));
                        let Some(chave) = chave else { return false };
                        let mut vistos: HashSet<ClassId> = HashSet::new();
                        let e0 = program.class(cl);
                        let mut fila: Vec<ClassId> = e0.mixin_classes.iter().rev().copied().collect();
                        fila.extend(e0.supertype_class);
                        fila.extend(e0.on_classes.iter().copied());
                        let mut i = 0;
                        while i < fila.len() {
                            let x = fila[i];
                            i += 1;
                            if !vistos.insert(x) {
                                continue;
                            }
                            let kx = program.class(x);
                            if kx.library == lib
                                && let Some(&g) = kx.instance_members.get(&chave)
                                && !program.function(g).abstract_
                            {
                                return true;
                            }
                            fila.extend(kx.mixin_classes.iter().rev().copied());
                            fila.extend(kx.supertype_class);
                        }
                        false
                    });
                    if sobrescreve {
                        continue;
                    }
                    campos.push((VariableId(vid as u32), mid, index, e_classe.then_some(DeclId(k as u32))));
                }
            }
        }
        if !campos.is_empty() {
            // As escritas de toda a biblioteca.
            let mut mutados: HashSet<VariableId> = HashSet::new();
            for &un in &program.library(lib).units {
                let Some(corpo) = s.corpos.units.get(un.0 as usize) else { continue };
                let a2 = &program.unit(un).ast;
                for e in a2.exprs.iter() {
                    let alvo = match &e.kind {
                        ExprKind::Assign { target, .. } => *target,
                        ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => *operand,
                        _ => continue,
                    };
                    if let Some(v) = campo_escrito(s, corpo, alvo) {
                        mutados.insert(v);
                    }
                }
            }
            for (vid, mid, index, classe) in campos {
                if mutados.contains(&vid) {
                    continue;
                }
                let MemberKind::Field(l) = &a.member(mid).kind else { continue };
                let v = &l.variables[index];
                let construtores: Vec<&dartforge_frontend::ast::Constructor> = match classe.map(|d| &a.decl(d).kind) {
                    Some(DeclKind::Class(x)) => x
                        .members
                        .iter()
                        .filter_map(|m| match &a.member(*m).kind {
                            MemberKind::Constructor(k) => Some(k),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let definido_em = |k: &dartforge_frontend::ast::Constructor| {
                    k.initializers.iter().any(|i| matches!(i, Initializer::Field { name, .. } if name.sym == v.name.sym))
                        || k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == v.name.sym))
                };
                let em_algum = construtores.iter().any(|k| definido_em(k));
                let relata = if em_algum { construtores.iter().all(|k| definido_em(k)) } else { v.initializer.is_some() };
                if relata {
                    let fim = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                    let texto = interner.resolve(v.name.sym);
                    relatar(&c::PREFER_FINAL_FIELDS, Span { start: v.name.span.start, end: fim }, &[texto]);
                }
            }
        }
    }

    // `overridden_fields`.
    if ligada("overridden_fields") {
        for (k, d) in a.decls.iter().enumerate() {
            let membros = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            if d.augment {
                continue;
            }
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            // `_findAllSupertypesAndMixins` / `_findAllSupertypesInMixin`.
            let ordem = supertipos_e_mixins(s, classe);
            for &mid in membros {
                let m = a.member(mid);
                let MemberKind::Field(l) = &m.kind else { continue };
                if l.static_ || m.augment {
                    continue;
                }
                for v in l.variables.iter() {
                    let texto = interner.resolve(v.name.sym);
                    let achado = ordem.iter().find_map(|cl| {
                        let x = program.class(*cl);
                        let g = *x.instance_members.get(&v.name.sym)?;
                        let ge = program.function(g);
                        // O getter sintético de um campo.
                        let campo = ge.variable?;
                        if !matches!(ge.node, FunctionRef::None) || ge.static_ || ge.kind != FunctionKind::Getter {
                            return None;
                        }
                        if texto.starts_with('_') && x.library != lib {
                            return None;
                        }
                        Some((*cl, campo))
                    });
                    let Some((dona, campo)) = achado else { continue };
                    // `isAbstract` do acessor: o campo `abstract`.
                    let abstrato = match program.variable(campo).node {
                        VariableRef::Field { unit, member, .. } => matches!(&program.unit(unit).ast.member(member).kind, MemberKind::Field(l2) if l2.abstract_),
                        _ => false,
                    };
                    if !abstrato {
                        let nome_dona = interner.resolve(program.class(dona).name);
                        relatar(&c::OVERRIDDEN_FIELDS, v.name.span, &[nome_dona]);
                    }
                }
            }
        }
    }

    out
}

/// A ordem do `_findAllSupertypesAndMixins` (classe) ou do
/// `_findAllSupertypesInMixin` (mixin), sem repetição e sem a própria classe.
fn supertipos_e_mixins(s: &super::Semantica<'_>, classe: ClassId) -> Vec<ClassId> {
    let program = s.program;
    let object = s.core.object_class;
    // `f(I)`: [superclasse, mixins de I, f(superclasse)…].
    fn f(program: &dartforge_elements::model::Program, object: Option<ClassId>, i: ClassId, acc: &mut Vec<ClassId>, saida: &mut Vec<ClassId>) {
        if Some(i) == object || acc.contains(&i) {
            return;
        }
        acc.push(i);
        let e = program.class(i);
        if let Some(sup) = e.supertype_class {
            saida.push(sup);
        }
        saida.extend(e.mixin_classes.iter().copied());
        if let Some(sup) = e.supertype_class {
            f(program, object, sup, acc, saida);
        }
    }
    let mut saida: Vec<ClassId> = Vec::new();
    let mut acc: Vec<ClassId> = Vec::new();
    let e = program.class(classe);
    if e.kind == ClassKind::Mixin {
        for &t in &e.on_classes {
            saida.push(t);
            f(program, object, t, &mut acc, &mut saida);
        }
    } else {
        f(program, object, classe, &mut acc, &mut saida);
    }
    let mut vistos: HashSet<ClassId> = HashSet::new();
    saida.into_iter().filter(|c| *c != classe && vistos.insert(*c)).collect()
}
