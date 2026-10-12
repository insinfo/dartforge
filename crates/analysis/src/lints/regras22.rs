//! O vigésimo segundo lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `avoid_positional_boolean_parameters`: o primeiro posicional `bool` de
//!   construtores, funções e métodos públicos que não sobrescrevem, e de
//!   tipos `Function`.
//! * `omit_local_variable_types`: o tipo escrito igual ao do inicializador
//!   (fora literal inteiro em tipo que não é `int` e a chamada genérica que
//!   depende dele), e o da variável do `for-in` igual ao dos elementos.
//! * `use_key_in_widget_constructors`: o widget público sem construtor, e o
//!   construtor público que não repassa a `key`.
//! * `avoid_type_to_string`: `toString` do `Object` sobre um `Type`, com o
//!   `thisType` da última classe, mixin ou extensão visitada.
//! * `prefer_int_literals`: o literal `double` inteiro onde o tipo `double`
//!   está escrito.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::{
    DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, MemberKind, Parameter, ParameterKind, StmtKind, TypeKind,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{HashMap, HashSet};

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

fn privado(interner: &Interner, s: dartforge_intern::SymbolId) -> bool {
    interner.resolve(s).starts_with('_')
}

/// O tipo é a classe `bool` (com ou sem `?`).
fn e_bool(s: &super::Semantica<'_>, t: TypeId) -> bool {
    matches!(s.table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.bool_class)
}

fn e_double(s: &super::Semantica<'_>, t: TypeId) -> bool {
    matches!(s.table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.double_class)
}

/// O tipo escrito de uma anotação.
fn escrito(s: &super::Semantica<'_>, t: dartforge_frontend::ast::TypeId) -> Option<TypeId> {
    s.corpo.tipos_de_anotacoes.get(&t).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unidade, t)).copied())
}

/// O primeiro posicional `bool` (`_isBoolean`).
fn primeiro_bool<'a>(s: &super::Semantica<'_>, ps: &'a [Parameter]) -> Option<&'a Parameter> {
    ps.iter().find(|p| p.kind != ParameterKind::Named && p.function_parameters.is_none() && p.ty.and_then(|t| escrito(s, t)).is_some_and(|t| e_bool(s, t)))
}

/// `lookUpInheritedMethod` (a cadeia de superclasses com os mixins, sem a
/// própria classe).
fn metodo_herdado(s: &super::Semantica<'_>, interner: &Interner, classe: ClassId, nome: dartforge_intern::SymbolId) -> bool {
    let program = s.program;
    let lib = program.unit(s.unidade).library;
    let mut vistos: HashSet<ClassId> = HashSet::new();
    let mut atual = Some(classe);
    while let Some(cl) = atual {
        if !vistos.insert(cl) {
            break;
        }
        let e = program.class(cl);
        for cc in std::iter::once(cl).chain(e.mixin_classes.iter().rev().copied()) {
            if cc == classe {
                continue;
            }
            let k = program.class(cc);
            if let Some(&g) = k.instance_members.get(&nome) {
                let ge = program.function(g);
                if matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator) && !ge.static_ && (!privado(interner, nome) || k.library == lib) {
                    return true;
                }
            }
        }
        atual = e.supertype_class;
    }
    false
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

    // `avoid_positional_boolean_parameters`.
    if ligada("avoid_positional_boolean_parameters") {
        let mut achados: Vec<Span> = Vec::new();
        // Os métodos e construtores das declarações.
        let mut metodos: HashSet<dartforge_frontend::ast::FunctionId> = HashSet::new();
        for (k, d) in a.decls.iter().enumerate() {
            let membros = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Extension(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let classe = if matches!(d.kind, DeclKind::Extension(_)) { None } else { classe_da_decl(s, DeclId(k as u32)) };
            let e_classe = matches!(d.kind, DeclKind::Class(_));
            for &mid in membros {
                let m = a.member(mid);
                if m.augment {
                    continue;
                }
                match &m.kind {
                    MemberKind::Constructor(kc) => {
                        if kc.name.is_some_and(|n| privado(interner, n.sym)) {
                            continue;
                        }
                        if let Some(p) = primeiro_bool(s, &kc.parameters) {
                            achados.push(p.span);
                        }
                    }
                    MemberKind::Method(fid) => {
                        metodos.insert(*fid);
                        let f = a.function(*fid);
                        let Some(n) = f.name else { continue };
                        if matches!(f.kind, dartforge_frontend::ast::FunctionKind::Setter | dartforge_frontend::ast::FunctionKind::Operator) || privado(interner, n.sym) {
                            continue;
                        }
                        if classe.is_some_and(|cl| metodo_herdado(s, interner, cl, n.sym)) {
                            continue;
                        }
                        // `_isOverridingMember`: um membro herdado de mesmo
                        // nome (só em classes).
                        let sobrescreve = e_classe
                            && classe.is_some_and(|cl| {
                                s.outline.hierarchy.get(cl).is_some_and(|h| h.supertypes.keys().any(|sc| program.class(*sc).instance_members.contains_key(&n.sym)))
                                    || s.core.object_class.is_some_and(|o| program.class(o).instance_members.contains_key(&n.sym))
                            });
                        if sobrescreve {
                            continue;
                        }
                        if let Some(ps) = &f.parameters
                            && let Some(p) = primeiro_bool(s, ps)
                        {
                            achados.push(p.span);
                        }
                    }
                    MemberKind::Field(_) => {}
                }
            }
        }
        // As funções declaradas (de topo e locais).
        let mut declaradas: Vec<dartforge_frontend::ast::FunctionId> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind
                && !d.augment
            {
                declaradas.push(*f);
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(f) = &st.kind {
                declaradas.push(*f);
            }
        }
        for fid in declaradas {
            let f = a.function(fid);
            if f.name.is_some_and(|n| privado(interner, n.sym)) {
                continue;
            }
            if let Some(ps) = &f.parameters
                && let Some(p) = primeiro_bool(s, ps)
            {
                achados.push(p.span);
            }
        }
        // Os tipos `Function`.
        for t in a.types.iter() {
            if let TypeKind::Function { parameters, .. } = &t.kind
                && let Some(p) = primeiro_bool(s, parameters)
            {
                achados.push(p.span);
            }
        }
        let _ = metodos;
        achados.sort_by_key(|x| x.start);
        for sp in achados {
            relatar(&c::AVOID_POSITIONAL_BOOLEAN_PARAMETERS, sp, &[]);
        }
    }

    // `omit_local_variable_types`.
    if ligada("omit_local_variable_types") {
        let mut listas: Vec<&dartforge_frontend::ast::VariableList> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) => listas.push(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => listas.push(l),
                StmtKind::ForIn { target: ForInTarget::Declared { ty: Some(ty), .. }, iterable, .. } => {
                    let Some(tl) = escrito(s, *ty) else { continue };
                    if matches!(table.get(tl), Type::Dynamic) {
                        continue;
                    }
                    let Some(ti) = s.corpo.get_type(*iterable) else { continue };
                    if !matches!(table.get(ti), Type::Interface { .. }) {
                        continue;
                    }
                    let Some(iteravel) = s.core.iterable_class else { continue };
                    if super::obvio::argumento_como_instancia(s, ti, iteravel, 0) == Some(tl) {
                        relatar(&c::OMIT_LOCAL_VARIABLE_TYPES, a.ty(*ty).span, &[]);
                    }
                }
                _ => {}
            }
        }
        for l in listas {
            let Some(ty) = l.ty else { continue };
            let Some(tl) = escrito(s, ty) else { continue };
            if matches!(table.get(tl), Type::Dynamic | Type::Null) {
                continue;
            }
            let e_int = matches!(table.get(tl), Type::Interface { class, .. } if Some(*class) == s.core.int_class);
            let todas = l.variables.iter().all(|v| {
                let Some(i) = v.initializer else { return false };
                if s.corpo.get_type(i) != Some(tl) {
                    return false;
                }
                if matches!(a.expr(i).kind, ExprKind::Int(_)) && !e_int {
                    return false;
                }
                // `dependsOnDeclaredTypeForInference`: a chamada de função
                // genérica sem argumentos de tipo que devolve um parâmetro
                // de tipo.
                if let ExprKind::Call { target, .. } = &a.expr(i).kind
                    && !matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. })
                {
                    let funcao = match s.corpo.get_resolved(*target) {
                        Some(Resolved::Element(Element::Function(f))) => Some(*f),
                        _ => None,
                    };
                    if let Some(f) = funcao
                        && program.function(f).class.is_none()
                        && let Some(d) = s.outline.functions.get(f.0 as usize)
                        && matches!(table.get(d.return_type), Type::TypeParameter { .. })
                    {
                        return false;
                    }
                    // A função local genérica.
                    if matches!(s.corpo.get_resolved(*target), Some(Resolved::Local(_)))
                        && let Some(decl) = s.corpo.declaracao_local(*target)
                        && let Some(g) = a.stmts.iter().find_map(|x| match &x.kind {
                            StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == decl) => Some(*g),
                            _ => None,
                        })
                        && let Some(rt) = a.function(g).return_type
                        && escrito(s, rt).is_some_and(|t| matches!(table.get(t), Type::TypeParameter { .. }))
                    {
                        return false;
                    }
                }
                true
            });
            if todas {
                relatar(&c::OMIT_LOCAL_VARIABLE_TYPES, a.ty(ty).span, &[]);
            }
        }
    }

    // `use_key_in_widget_constructors`.
    if ligada("use_key_in_widget_constructors") {
        let e_key = |t: TypeId| -> bool {
            let f = |k: ClassId| {
                let x = program.class(k);
                interner.resolve(x.name) == "Key" && program.library(x.library).name.as_ref().is_none_or(|n| n.is_empty())
            };
            match table.get(t) {
                Type::Interface { class, .. } => f(*class) || (program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| f(*k)))),
                _ => false,
            }
        };
        // O construtor tem um parâmetro `key` de tipo `Key`.
        let define_key = |f: dartforge_elements::model::FunctionElementId| -> bool {
            let FunctionRef::Constructor { unit, member } = program.function(f).node else { return false };
            let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return false };
            let Some(dados) = s.outline.functions.get(f.0 as usize) else { return false };
            let Type::Function { positional, optional, named, .. } = table.get(dados.signature) else { return false };
            let mut posicao = 0usize;
            for p in k.parameters.iter() {
                let t = match p.kind {
                    ParameterKind::Named => p.name.and_then(|n| named.iter().find(|(x, _, _)| *x == n.sym).map(|(_, t, _)| *t)),
                    _ => {
                        posicao += 1;
                        positional.iter().chain(optional.iter()).nth(posicao - 1).copied()
                    }
                };
                if p.name.is_some_and(|n| interner.resolve(n.sym) == "key") && t.is_some_and(e_key) {
                    return true;
                }
            }
            false
        };
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if privado(interner, x.name.sym) || !super::flutter::tem_widget_ascendente(s, interner, classe) {
                continue;
            }
            let construtores: Vec<&dartforge_frontend::ast::Constructor> = x
                .members
                .iter()
                .filter_map(|m| match &a.member(*m).kind {
                    MemberKind::Constructor(kc) => Some(kc),
                    _ => None,
                })
                .collect();
            if construtores.is_empty() {
                relatar(&c::USE_KEY_IN_WIDGET_CONSTRUCTORS, x.name.span, &[]);
                continue;
            }
            if super::flutter::e_widget_exato(s, interner, classe) {
                continue;
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Constructor(kc) = &m.kind else { continue };
                if m.augment || kc.factory || kc.name.is_some_and(|n| privado(interner, n.sym)) {
                    continue;
                }
                if kc.parameters.iter().any(|p| p.super_ && p.name.is_some_and(|n| interner.resolve(n.sym) == "key")) {
                    continue;
                }
                let sup = program.class(classe).supertype_class;
                let cala = kc.initializers.iter().any(|i| {
                    let (dono, nome, args) = match i {
                        Initializer::Super { constructor, arguments, .. } => (sup, *constructor, arguments),
                        Initializer::Redirect { constructor, arguments, .. } => (Some(classe), *constructor, arguments),
                        _ => return false,
                    };
                    let Some(dono) = dono else { return false };
                    let chave = nome.map(|n| n.sym).or_else(|| interner.lookup(""));
                    let Some(&f) = chave.and_then(|ch| program.class(dono).constructors.get(&ch)) else { return false };
                    // `_defineKeyArgument`: um argumento de parâmetro `key`.
                    let passa_key = args.args.iter().any(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == "key"));
                    !define_key(f) || passa_key
                });
                if cala {
                    continue;
                }
                let alvo = kc.name.unwrap_or(kc.class_name).span;
                relatar(&c::USE_KEY_IN_WIDGET_CONSTRUCTORS, alvo, &[]);
            }
        }
    }

    // `avoid_type_to_string`.
    if ligada("avoid_type_to_string") {
        let tipo_type = match table.get(s.core.type_) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let e_subtipo_de_type = |t: TypeId| match table.get(t) {
            Type::Interface { class, nullable: false, .. } => {
                Some(*class) == tipo_type || tipo_type.is_some_and(|ty| s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&ty)))
            }
            _ => false,
        };
        // O `thisType` vigente em cada posição: o da última classe, mixin
        // ou extensão (pela ordem da fonte).
        let mut tipos_this: Vec<(usize, Option<TypeId>)> = Vec::new();
        for (k, d) in a.decls.iter().enumerate() {
            let t = match &d.kind {
                DeclKind::Class(_) | DeclKind::Mixin(_) => classe_da_decl(s, DeclId(k as u32)).and_then(|cl| {
                    let x = Type::Interface {
                        class: cl,
                        args: s.outline.classes.get(cl.0 as usize).map(|c| c.type_params.iter().filter_map(|p| table.intern_lookup(Type::TypeParameter { param: *p, nullable: false })).collect()).unwrap_or_default(),
                        nullable: false,
                    };
                    table.intern_lookup(x)
                }),
                DeclKind::Extension(x) => s.outline.tipos_escritos.get(&(s.unidade, x.on)).copied().filter(|t| matches!(table.get(*t), Type::Interface { .. })),
                _ => continue,
            };
            tipos_this.push((d.span.start, t));
        }
        tipos_this.sort_by_key(|x| x.0);
        let this_em = |pos: usize| tipos_this.iter().rev().find(|(p, _)| *p <= pos).and_then(|(_, t)| *t);
        let do_object = |e: ExprId| -> bool {
            match s.corpo.get_resolved(e) {
                Some(Resolved::Member { member: MemberRef::Function(f), .. }) => program.function(*f).class == s.core.object_class,
                _ => false,
            }
        };
        let checar = |alvo: Option<TypeId>, nome: dartforge_frontend::ast::Name, e: ExprId, achados: &mut Vec<Span>| {
            if interner.resolve(nome.sym) == "toString" && do_object(e) && alvo.is_some_and(e_subtipo_de_type) {
                achados.push(nome.span);
            }
        };
        let mut achados: Vec<Span> = Vec::new();
        let tipo_interface = |e: ExprId| s.corpo.get_type(e).filter(|t| matches!(table.get(*t), Type::Interface { .. }));
        for e in a.exprs.iter() {
            let args = match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => &**arguments,
                _ => continue,
            };
            let validar = |x: ExprId, achados: &mut Vec<Span>| match &a.expr(x).kind {
                ExprKind::Property { target, name, .. } => {
                    let alvo = tipo_interface(*target).or_else(|| this_em(a.expr(x).span.start));
                    checar(alvo, *name, x, achados);
                }
                ExprKind::Identifier(n) => checar(this_em(a.expr(x).span.start), *n, x, achados),
                _ => {}
            };
            for x in args.args.iter().filter(|x| x.name.is_none()) {
                validar(x.value, &mut achados);
            }
            if let ExprKind::Call { target, .. } = &e.kind {
                match &a.expr(*target).kind {
                    ExprKind::Property { target: t, name, .. } => {
                        let alvo = tipo_interface(*t).or_else(|| this_em(e.span.start));
                        checar(alvo, *name, *target, &mut achados);
                    }
                    ExprKind::Identifier(n) => checar(this_em(e.span.start), *n, *target, &mut achados),
                    _ => {}
                }
            }
        }
        achados.sort_by_key(|x| x.start);
        achados.dedup();
        for sp in achados {
            relatar(&c::AVOID_TYPE_TO_STRING, sp, &[]);
        }
    }

    // `prefer_int_literals`.
    if ligada("prefer_int_literals") {
        let mut pai: HashMap<ExprId, No> = HashMap::new();
        let mut ancestrais_de: HashMap<ExprId, Vec<No>> = HashMap::new();
        andar(u, &mut |no, anc| {
            if let No::Expr(e) = no {
                if let Some(p) = anc.last() {
                    pai.insert(e, *p);
                }
                if matches!(a.expr(e).kind, ExprKind::Double(_)) {
                    ancestrais_de.insert(e, anc.to_vec());
                }
            }
        });
        // O tipo de retorno escrito é `double` (função declarada ou método).
        let retorno_double = |f: dartforge_frontend::ast::FunctionId| -> bool {
            let e_literal = a.exprs.iter().any(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f));
            if e_literal {
                return false;
            }
            a.function(f).return_type.and_then(|t| escrito(s, t)).is_some_and(|t| e_double(s, t))
        };
        let mut lits: Vec<(&ExprId, &Vec<No>)> = ancestrais_de.iter().collect();
        lits.sort_by_key(|(e, _)| a.expr(**e).span.start);
        for (&e, anc) in lits {
            let ExprKind::Double(sp) = &a.expr(e).kind else { continue };
            let texto: String = fonte[sp.start..sp.end].chars().filter(|c| *c != '_').collect();
            let negativo = texto.starts_with('-');
            let corpo = texto.trim_start_matches('-').trim_start();
            let Ok(v) = corpo.parse::<f64>() else { continue };
            if !v.is_finite() || v != v.trunc() {
                continue;
            }
            let inicio = if negativo { sp.start + (fonte[sp.start..sp.end].len() - fonte[sp.start..sp.end].trim_start_matches('-').trim_start().len()) } else { sp.start };
            // `hasTypeDouble` do nó (o literal, ou o `-` que o contém).
            let tipo_double = match pai.get(&e).copied() {
                Some(No::Expr(p)) => match &a.expr(p).kind {
                    ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                        arguments.args.iter().any(|x| x.value == e) && s.corpo.tipos_de_parametros.get(&e).is_some_and(|t| e_double(s, *t))
                    }
                    ExprKind::List { type_args, elements, .. } => {
                        elements.iter().any(|el| matches!(el, dartforge_frontend::ast::CollectionElement::Expression(x) if *x == e))
                            && type_args.len() == 1
                            && escrito(s, type_args[0]).is_some_and(|t| e_double(s, t))
                    }
                    _ => false,
                },
                Some(No::Funcao(f)) => matches!(a.function(f).body, FunctionBody::Expression(x) if x == e) && retorno_double(f),
                Some(No::Stmt(st)) => match &a.stmt(st).kind {
                    StmtKind::Return(Some(_)) => anc.iter().rev().find_map(|n| match n {
                        No::Funcao(f) => Some(*f),
                        _ => None,
                    })
                    .is_some_and(|f| matches!(a.function(f).body, FunctionBody::Block(_)) && retorno_double(f)),
                    StmtKind::Variables(l) => l.ty.and_then(|t| escrito(s, t)).is_some_and(|t| e_double(s, t)),
                    StmtKind::For { init: Some(ForInit::Variables(l)), .. } => l.ty.and_then(|t| escrito(s, t)).is_some_and(|t| e_double(s, t)),
                    _ => false,
                },
                Some(No::Membro(m)) => match &a.member(m).kind {
                    MemberKind::Field(l) => l.ty.and_then(|t| escrito(s, t)).is_some_and(|t| e_double(s, t)),
                    _ => false,
                },
                Some(No::Decl(d)) => match &a.decl(d).kind {
                    DeclKind::Variables(l) => l.ty.and_then(|t| escrito(s, t)).is_some_and(|t| e_double(s, t)),
                    _ => false,
                },
                _ => false,
            };
            if tipo_double {
                relatar(&c::PREFER_INT_LITERALS, Span { start: inicio, end: sp.end }, &[]);
            }
        }
    }

    out
}
