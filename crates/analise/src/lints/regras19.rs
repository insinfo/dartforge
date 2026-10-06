//! O décimo nono lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela semântica
//! da unidade:
//!
//! * `prefer_const_constructors`: a criação que pode ser `const`
//!   (`Semantica::pode_ser_const`), fora o tipo de prefixo adiado, o
//!   construtor `@literal`, o de `Object` e a classe genérica sem argumentos
//!   de tipo cujo contexto aproximado tem parâmetro de tipo.
//! * `prefer_const_literals_to_create_immutables`: o literal que pode ser
//!   `const`, argumento (através de parênteses, literais, entradas e
//!   nomeados) de uma criação de classe `@immutable` (nela ou na cadeia de
//!   superclasses).
//! * `prefer_const_constructors_in_immutables`: o construtor de corpo vazio
//!   de classe `@immutable` sem mixins que pode ser `const`, ou a fábrica
//!   que redireciona a um `const`; e o tipo de extensão `@immutable` sem
//!   `const`.
//! * `use_to_and_as_if_applicable`: o método sem parâmetros que devolve
//!   `C(this)`.
//! * `use_decorated_box`, `use_colored_box` e `sized_box_for_whitespace`:
//!   os argumentos de um `Container`.
//! * `no_logic_in_create_state`: o `createState` de um `StatefulWidget` que
//!   não devolve só uma criação sem argumentos.
//! * `use_setters_to_change_properties`: o método `void` de um parâmetro que
//!   só atribui o parâmetro a um campo.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::No;
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::{self, AssignOp, Ast, CollectionElement, DeclId, DeclKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, StmtKind, TypeKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::Type;
use std::collections::HashSet;

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// A classe tem `@immutable` (`hasImmutable` do elemento).
fn imutavel(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    let Some(r) = s.program.class(c).decl else { return false };
    s.program.unit(r.unit).ast.decl(r.decl).metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(s.program, interner, r.unit, m, "meta", "immutable"))
}

/// A classe ou a cadeia de superclasses tem `@immutable`.
fn imutavel_na_cadeia(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    let mut vistos = HashSet::new();
    let mut atual = Some(c);
    while let Some(x) = atual {
        if !vistos.insert(x) {
            break;
        }
        if imutavel(s, interner, x) {
            return true;
        }
        atual = s.program.class(x).supertype_class;
    }
    false
}

fn token_em(fonte: &str, pos: usize) -> Span {
    let b = fonte.as_bytes();
    let mut fim = pos;
    while fim < b.len() && (b[fim].is_ascii_alphanumeric() || b[fim] == b'_' || b[fim] == b'$') {
        fim += 1;
    }
    if fim == pos {
        fim = (pos + 1).min(b.len());
    }
    Span { start: pos, end: fim }
}

fn nome_do_construtor(a: &Ast, e: ExprId) -> Span {
    match &a.expr(e).kind {
        ExprKind::InstanceCreation { ty, constructor, .. } => {
            let t = a.ty(*ty).span;
            Span { start: t.start, end: constructor.map_or(t.end, |n| n.span.end) }
        }
        ExprKind::Call { target, .. } => a.expr(*target).span,
        _ => a.expr(e).span,
    }
}

/// `lookUpInheritedMethod`: um método (não estático, acessível) na cadeia
/// de superclasses com os mixins, sem a própria classe.
fn metodo_herdado(s: &super::Semantica<'_>, interner: &Interner, classe: ClassId, nome: dartforge_intern::SymbolId) -> bool {
    let program = s.program;
    let lib = program.unit(s.unidade).library;
    let privado = interner.resolve(nome).starts_with('_');
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
                if matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator) && !ge.static_ && (!privado || k.library == lib) {
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
    let lib = program.unit(s.unidade).library;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let vazio = dartforge_types::lints_tipados6::PodemSerConst::default();
    let pode = s.pode_ser_const.unwrap_or(&vazio);

    // `prefer_const_constructors`.
    if ligada("prefer_const_constructors") {
        let pais = super::regras16::pais_da_unidade(u, &|e| pode.criacoes.contains(&e));
        let mut criacoes: Vec<ExprId> = pode.criacoes.iter().copied().collect();
        criacoes.sort_by_key(|e| a.expr(*e).span.start);
        for e in criacoes {
            let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(e) else { continue };
            let g = program.function(*f);
            // O tipo com prefixo adiado.
            let prefixo = match &a.expr(e).kind {
                ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
                    TypeKind::Named { name, .. } if name.len() == 2 => Some(name[0].sym),
                    _ => None,
                },
                ExprKind::Call { target, .. } => {
                    let mut t = *target;
                    if let ExprKind::TypeArguments { target: t2, .. } = &a.expr(t).kind {
                        t = *t2;
                    }
                    // `p.C()` ou `p.C.nome()`.
                    match &a.expr(t).kind {
                        ExprKind::Property { target: t3, .. } => match &a.expr(*t3).kind {
                            ExprKind::Identifier(p) if matches!(program.lookup_na_unidade(s.unidade, p.sym).and_then(|b| b.getter), Some(Element::Prefix(..))) => Some(p.sym),
                            ExprKind::Property { target: t4, .. } => match &a.expr(*t4).kind {
                                ExprKind::Identifier(p) => Some(p.sym),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    }
                }
                _ => None,
            };
            let adiado = prefixo.is_some_and(|p| program.library(lib).imports.iter().any(|i| i.unit == s.unidade && i.prefix == Some(p) && i.deferred));
            if adiado {
                continue;
            }
            // `@literal` no construtor.
            let literal = match g.node {
                FunctionRef::Constructor { unit, member } => program
                    .unit(unit)
                    .ast
                    .member(member)
                    .metadata
                    .iter()
                    .any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, unit, m, "meta", "literal")),
                _ => false,
            };
            if literal {
                continue;
            }
            let Some(classe) = g.class else { continue };
            if Some(classe) == s.core.object_class {
                continue;
            }
            // A classe genérica sem argumentos de tipo escritos: o contexto.
            let tem_argumentos = match &a.expr(e).kind {
                ExprKind::InstanceCreation { ty, .. } => matches!(&a.ty(*ty).kind, TypeKind::Named { args, .. } if !args.is_empty()),
                ExprKind::Call { target, .. } => matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. }),
                _ => false,
            };
            let params = s.outline.classes.get(classe.0 as usize).map(|x| x.type_params.len()).unwrap_or(0);
            if params > 0 && !tem_argumentos {
                let ctx = super::regras16::contexto_aproximado(s, interner, a, &pais, e);
                if let super::regras16::Contexto::Tipo(t) = ctx
                    && let Type::Interface { class: cc, args: cargs, .. } = table.get(t)
                {
                    // `asInstanceOf(classe)`: os argumentos vistos na classe.
                    let argumentos: Option<Vec<dartforge_types::table::TypeId>> = if *cc == classe {
                        Some(cargs.to_vec())
                    } else {
                        s.outline.hierarchy.get(*cc).and_then(|d| {
                            let molde = *d.supertypes.get(&classe)?;
                            let Type::Interface { args: margs, .. } = table.get(molde) else { return None };
                            Some(
                                margs
                                    .iter()
                                    .map(|m| match table.get(*m) {
                                        Type::TypeParameter { param, .. } => d.type_params.iter().position(|q| q == param).and_then(|j| cargs.get(j).copied()).unwrap_or(*m),
                                        _ => *m,
                                    })
                                    .collect(),
                            )
                        })
                    };
                    if argumentos.is_some_and(|v| v.iter().any(|x| matches!(table.get(*x), Type::TypeParameter { .. } | Type::Intersection { .. }))) {
                        continue;
                    }
                }
            }
            relatar(&c::PREFER_CONST_CONSTRUCTORS, a.expr(e).span, &[]);
        }
    }

    // `prefer_const_literals_to_create_immutables`.
    if ligada("prefer_const_literals_to_create_immutables") {
        let pais = super::regras16::pais_da_unidade(u, &|_| false);
        let mut literais: Vec<ExprId> = pode.literais.iter().copied().collect();
        literais.sort_by_key(|e| a.expr(*e).span.start);
        for lit in literais {
            // Sobe por parênteses, argumentos, literais, entradas e
            // nomeados até uma criação.
            let mut no = lit;
            let criacao = loop {
                let Some(No::Expr(p)) = pais.expr.get(&no).copied() else { break None };
                if matches!(s.corpo.get_resolved(p), Some(Resolved::Constructor(_))) {
                    // Argumento da criação (a `ArgumentList` dela).
                    let e_argumento = match &a.expr(p).kind {
                        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments.args.iter().any(|x| x.value == no),
                        _ => false,
                    };
                    break if e_argumento { Some(p) } else { None };
                }
                let passa = match &a.expr(p).kind {
                    ExprKind::Parenthesized(_) => true,
                    ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().any(|el| match el {
                        CollectionElement::Expression(x) => *x == no,
                        CollectionElement::MapEntry { key, value, .. } => *key == no || *value == no,
                        _ => false,
                    }),
                    _ => false,
                };
                if !passa {
                    break None;
                }
                no = p;
            };
            let Some(cr) = criacao else { continue };
            let Some(t) = s.corpo.get_type(cr) else { continue };
            let Type::Interface { class, .. } = table.get(t) else { continue };
            if imutavel_na_cadeia(s, interner, *class) {
                relatar(&c::PREFER_CONST_LITERALS_TO_CREATE_IMMUTABLES, a.expr(lit).span, &[]);
            }
        }
    }

    // `prefer_const_constructors_in_immutables`.
    if ligada("prefer_const_constructors_in_immutables") {
        for (k, d) in a.decls.iter().enumerate() {
            let (membros, com_mixins) = match &d.kind {
                DeclKind::Class(x) => (&x.members, !x.with.is_empty() || x.modifiers.macro_),
                DeclKind::Enum(x) => (&x.members, !x.with.is_empty()),
                DeclKind::ExtensionType(x) => {
                    // O tipo de extensão `@immutable` sem `const`.
                    if !x.const_
                        && let Some(cl) = classe_da_decl(s, DeclId(k as u32))
                        && imutavel(s, interner, cl)
                    {
                        relatar(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, x.name.span, &[]);
                    }
                    (&x.members, false)
                }
                _ => continue,
            };
            if com_mixins {
                continue;
            }
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if !imutavel_na_cadeia(s, interner, classe) {
                continue;
            }
            let construtor_const = |cl: ClassId, nome: Option<dartforge_intern::SymbolId>| -> bool {
                let chave = nome.or_else(|| interner.lookup(""));
                chave.and_then(|ch| program.class(cl).constructors.get(&ch)).is_some_and(|f| program.function(*f).const_)
            };
            for &mid in membros {
                let m = a.member(mid);
                let MemberKind::Constructor(kc) = &m.kind else { continue };
                if kc.const_ || !matches!(kc.body, FunctionBody::Empty) {
                    continue;
                }
                let primeiro = match m.metadata.last() {
                    Some(x) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), x.span.end),
                    None => m.span.start,
                };
                // A fábrica que redireciona.
                if kc.factory
                    && let Some(r) = &kc.redirect
                {
                    let alvo_const = s.outline.tipos_escritos.get(&(s.unidade, r.ty)).and_then(|t| match table.get(*t) {
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
                        _ => None,
                    });
                    if alvo_const.is_some_and(|cl| construtor_const(cl, r.constructor.map(|n| n.sym))) {
                        relatar(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, token_em(fonte, primeiro), &[]);
                    }
                    continue;
                }
                // `_hasConstConstructorInvocation`.
                let mut invoca_const = None;
                for i in kc.initializers.iter() {
                    match i {
                        Initializer::Super { constructor, .. } if invoca_const.is_none() => {
                            invoca_const = Some(program.class(classe).supertype_class.is_some_and(|sc| construtor_const(sc, constructor.map(|n| n.sym))));
                        }
                        _ => {}
                    }
                }
                if invoca_const.is_none() {
                    for i in kc.initializers.iter() {
                        if let Initializer::Redirect { constructor, .. } = i {
                            invoca_const = Some(construtor_const(classe, constructor.map(|n| n.sym)));
                            break;
                        }
                    }
                }
                let invoca_const = invoca_const.unwrap_or_else(|| match &d.kind {
                    DeclKind::ExtensionType(x) => x.const_,
                    _ => program.class(classe).supertype_class.is_some_and(|sc| construtor_const(sc, None)),
                });
                if invoca_const && pode.construtores.contains(&mid) {
                    relatar(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, token_em(fonte, primeiro), &[]);
                }
            }
        }
    }

    // `use_to_and_as_if_applicable`.
    if ligada("use_to_and_as_if_applicable") {
        let prefixo_ok = |n: &str| {
            ["to", "as", "_to", "_as"].iter().any(|p| n.strip_prefix(p).and_then(|r| r.chars().next()).is_some_and(|c| c.is_ascii_uppercase()))
        };
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
            for &mid in membros {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.kind == ast::FunctionKind::Getter {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                if !ps.is_empty() {
                    continue;
                }
                if f.return_type.is_some_and(|t| matches!(a.ty(t).kind, TypeKind::Void)) {
                    continue;
                }
                let texto = interner.resolve(n.sym);
                if prefixo_ok(texto) {
                    continue;
                }
                if classe.is_some_and(|cl| metodo_herdado(s, interner, cl, n.sym)) {
                    continue;
                }
                let expressao = match &f.body {
                    FunctionBody::Expression(e) => Some(*e),
                    FunctionBody::Block(b) => match &a.stmt(*b).kind {
                        StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                            StmtKind::Return(v) => *v,
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let Some(e) = expressao.map(|e| sem_parenteses(a, e)) else { continue };
                if !matches!(s.corpo.get_resolved(e), Some(Resolved::Constructor(_))) {
                    continue;
                }
                let args = match &a.expr(e).kind {
                    ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                    _ => continue,
                };
                if let [x] = &args.args[..]
                    && x.name.is_none()
                    && matches!(a.expr(x.value).kind, ExprKind::This)
                {
                    relatar(&c::USE_TO_AND_AS_IF_APPLICABLE, n.span, &[]);
                }
            }
        }
    }

    // `use_decorated_box`, `use_colored_box` e `sized_box_for_whitespace`.
    let caixas = ligada("use_decorated_box") || ligada("use_colored_box") || ligada("sized_box_for_whitespace");
    if caixas {
        for (k, _) in a.exprs.iter().enumerate() {
            let e = ExprId(k as u32);
            if !matches!(s.corpo.get_resolved(e), Some(Resolved::Constructor(_))) {
                continue;
            }
            if !s.corpo.get_type(e).is_some_and(|t| super::flutter::e_container(s, interner, t)) {
                continue;
            }
            let args = match &a.expr(e).kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            // Todos nomeados.
            if args.args.iter().any(|x| x.name.is_none()) {
                continue;
            }
            let nomes: Vec<(&str, ExprId)> = args.args.iter().map(|x| (interner.resolve(x.name.expect("nomeado").sym), x.value)).collect();
            let so = |permitidos: &[&str]| nomes.iter().all(|(n, _)| permitidos.contains(n));
            let tem = |n: &str| nomes.iter().any(|(x, _)| *x == n);
            if ligada("use_decorated_box") && so(&["child", "decoration", "key"]) && tem("child") && tem("decoration") {
                relatar(&c::USE_DECORATED_BOX, nome_do_construtor(a, e), &[]);
            }
            if ligada("use_colored_box") {
                // `color` só conta com tipo não anulável; com tipo anulável,
                // o argumento cai no `case _` e cala a regra.
                let mut ok = true;
                let mut tem_cor = false;
                for (n, v) in &nomes {
                    match *n {
                        "child" | "key" => {}
                        "color" => {
                            let anulavel = s.corpo.get_type(*v).is_some_and(|t| match table.get(t) {
                                Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
                                Type::Function { nullable, .. } | Type::Record { nullable, .. } | Type::FutureOr { nullable, .. } => *nullable,
                                Type::Null | Type::Dynamic | Type::Void => false,
                                _ => false,
                            });
                            if anulavel {
                                ok = false;
                            } else {
                                tem_cor = true;
                            }
                        }
                        _ => ok = false,
                    }
                }
                if ok && tem_cor && tem("child") {
                    relatar(&c::USE_COLORED_BOX, nome_do_construtor(a, e), &[]);
                }
            }
            if ligada("sized_box_for_whitespace") && so(&["child", "height", "width", "key"]) {
                let (filho, altura, largura) = (tem("child"), tem("height"), tem("width"));
                if (filho && (largura || altura)) || (largura && altura) {
                    relatar(&c::SIZED_BOX_FOR_WHITESPACE, nome_do_construtor(a, e), &[]);
                }
            }
        }
    }

    // `no_logic_in_create_state`.
    if ligada("no_logic_in_create_state") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if !super::flutter::e_stateful_widget(s, interner, classe) {
                continue;
            }
            for &mid in &x.members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.name.is_none_or(|n| interner.resolve(n.sym) != "createState") {
                    continue;
                }
                let (expressao, corpo_span) = match &f.body {
                    FunctionBody::Block(b) => {
                        let e = match &a.stmt(*b).kind {
                            StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                                StmtKind::Return(v) => *v,
                                _ => None,
                            },
                            _ => None,
                        };
                        (e, Some(a.stmt(*b).span))
                    }
                    FunctionBody::Expression(e) => (Some(*e), None),
                    FunctionBody::Empty => continue,
                    _ => (None, None),
                };
                if let Some(e) = expressao
                    && matches!(s.corpo.get_resolved(e), Some(Resolved::Constructor(_)))
                    && matches!(&a.expr(e).kind, ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } if arguments.args.is_empty())
                {
                    continue;
                }
                let alvo = match (expressao, corpo_span) {
                    (Some(e), _) => a.expr(e).span,
                    (None, Some(sp)) => sp,
                    _ => continue,
                };
                relatar(&c::NO_LOGIC_IN_CREATE_STATE, alvo, &[]);
            }
        }
    }

    // `use_setters_to_change_properties`.
    if ligada("use_setters_to_change_properties") {
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
            for &mid in membros {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if matches!(f.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter) {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                let [p] = &ps[..] else { continue };
                if !f.return_type.is_some_and(|t| matches!(a.ty(t).kind, TypeKind::Void)) {
                    continue;
                }
                // `isOverride`: um supertipo tem um método de mesmo nome.
                let sobrescreve = classe.is_some_and(|cl| {
                    s.outline.hierarchy.get(cl).is_some_and(|h| {
                        h.supertypes.keys().any(|sc| {
                            program.class(*sc).instance_members.get(&n.sym).is_some_and(|g| matches!(program.function(*g).kind, FunctionKind::Function | FunctionKind::Operator))
                        })
                    })
                });
                if sobrescreve {
                    continue;
                }
                let expressao = match &f.body {
                    FunctionBody::Expression(e) => Some(*e),
                    FunctionBody::Block(b) => match &a.stmt(*b).kind {
                        StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                            StmtKind::Expression(e) => Some(*e),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let Some(e) = expressao else { continue };
                let ExprKind::Assign { op: AssignOp::Assign, target, value } = &a.expr(e).kind else { continue };
                // O lado direito é o parâmetro.
                let v = sem_parenteses(a, *value);
                let e_param = matches!(&a.expr(v).kind, ExprKind::Identifier(m) if Some(m.sym) == p.name.map(|q| q.sym))
                    && matches!(s.corpo.get_resolved(v), Some(Resolved::Parameter { .. }));
                if !e_param {
                    continue;
                }
                // O lado esquerdo escreve um campo.
                let campo = match s.corpo.get_resolved(*target) {
                    Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => true,
                    Some(Resolved::Member { member: MemberRef::Function(g), .. }) => {
                        let ge = program.function(*g);
                        ge.variable.is_some() && matches!(ge.node, FunctionRef::None)
                    }
                    _ => false,
                };
                if campo {
                    relatar(&c::USE_SETTERS_TO_CHANGE_PROPERTIES, n.span, &[]);
                }
            }
        }
    }

    out
}
