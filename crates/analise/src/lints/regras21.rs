//! O vigésimo primeiro lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `omit_obvious_local_variable_types`: o tipo escrito de uma lista de
//!   locais (também a do `for`) com inicializadores de tipo óbvio
//!   (`super::obvio`) iguais a ele, e o da variável do `for-in` igual ao tipo
//!   dos elementos de um iterável óbvio.
//! * `prefer_constructors_over_static_methods`: o método estático (sem
//!   parâmetros de tipo) que devolve o `thisType` de uma classe não
//!   genérica e cuja última criação visitada no corpo é desse tipo.
//! * `invalid_case_patterns`: as expressões de `case` antigas (biblioteca
//!   antes da 3.0) que viram outra coisa como padrão.
//! * `avoid_null_checks_in_equality_operators`: `== null`, `??` e `?.` sobre
//!   o parâmetro anulável do `operator ==`.
//! * `sized_box_shrink_expand`: `SizedBox(width: 0, height: 0)` e com
//!   `double.infinity`.
//! * `implicit_reopen`: a classe que reabre sem `@reopen` uma superclasse
//!   `final`/`interface` da biblioteca.
//! * `noop_primitive_operations`: o literal vazio no meio de strings
//!   adjacentes, o `toString()` interpolado ou passado ao `print`, e as
//!   conversões que não mudam o valor.
//! * `unawaited_futures`: o `Future` descartado num corpo assíncrono
//!   (instrução, seção de cascata, interpolação).
//! * `avoid_returning_this`: o método que só devolve `this` (o tipo da
//!   própria classe).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId};
use dartforge_frontend::ast::{
    AsyncModifier, Ast, BinaryOp, DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, MemberKind, PatternKind, StmtKind, StringPart,
    UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId, TypeTable};

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn nao_anulavel(table: &TypeTable, t: TypeId) -> bool {
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

fn nome_da_biblioteca(s: &super::Semantica<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// O tipo é a classe `nome` do `dart:core`/`dart:async` (o elemento).
fn e_classe(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, nome: &str, lib: &str) -> bool {
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let k = s.program.class(*class);
            interner.resolve(k.name) == nome && nome_da_biblioteca(s, interner, k.library).as_deref() == Some(lib)
        }
        _ => false,
    }
}

/// O corpo de função mais interno que contém `sp` é assíncrono.
fn em_corpo_assincrono(a: &Ast, sp: Span) -> bool {
    let mut melhor: Option<(usize, bool)> = None;
    for f in a.functions.iter() {
        let corpo = match &f.body {
            FunctionBody::Block(b) => a.stmt(*b).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if corpo.start <= sp.start && sp.end <= corpo.end {
            let tam = corpo.end - corpo.start;
            if melhor.is_none_or(|(t, _)| tam < t) {
                melhor = Some((tam, matches!(f.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar)));
            }
        }
    }
    // Um corpo de construtor mais interno não é assíncrono.
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let FunctionBody::Block(b) = &k.body
        {
            let corpo = a.stmt(*b).span;
            if corpo.start <= sp.start && sp.end <= corpo.end {
                let tam = corpo.end - corpo.start;
                if melhor.is_none_or(|(t, _)| tam < t) {
                    melhor = Some((tam, false));
                }
            }
        }
    }
    melhor.is_some_and(|(_, x)| x)
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

    // `omit_obvious_local_variable_types`.
    if ligada("omit_obvious_local_variable_types") {
        let mut listas: Vec<&dartforge_frontend::ast::VariableList> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) => listas.push(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => listas.push(l),
                StmtKind::ForIn { target: ForInTarget::Declared { ty: Some(ty), .. }, iterable, .. } => {
                    let Some(&tl) = s.corpo.tipos_de_anotacoes.get(ty) else { continue };
                    if matches!(table.get(tl), Type::Dynamic) || !super::obvio::tem_tipo_obvio(s, interner, a, *iterable) {
                        continue;
                    }
                    if super::obvio::tipo_do_elemento_do_iteravel(s, interner, s.corpo.get_type(*iterable)) == Some(tl) {
                        relatar(&c::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES, a.ty(*ty).span, &[]);
                    }
                }
                _ => {}
            }
        }
        for l in listas {
            let Some(ty) = l.ty else { continue };
            let Some(&tl) = s.corpo.tipos_de_anotacoes.get(&ty) else { continue };
            if matches!(table.get(tl), Type::Null) {
                continue;
            }
            let todas = l.variables.iter().all(|v| match v.initializer {
                Some(i) => super::obvio::tem_tipo_obvio(s, interner, a, i) && s.corpo.get_type(i) == Some(tl),
                None => false,
            });
            if todas {
                relatar(&c::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES, a.ty(ty).span, &[]);
            }
        }
    }

    // `prefer_constructors_over_static_methods`.
    if ligada("prefer_constructors_over_static_methods") {
        for (k, d) in a.decls.iter().enumerate() {
            let (membros, sem_tps) = match &d.kind {
                DeclKind::Class(x) => (&x.members, x.type_params.is_empty()),
                DeclKind::ExtensionType(x) => (&x.members, x.type_params.is_empty()),
                _ => continue,
            };
            if !sem_tps {
                continue;
            }
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            for &mid in membros {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if !f.static_ || !f.type_params.is_empty() {
                    continue;
                }
                let Some(rt) = f.return_type else { continue };
                let Some(&tr) = s.outline.tipos_escritos.get(&(s.unidade, rt)) else { continue };
                let e_this = match table.get(tr) {
                    Type::Interface { class, nullable: false, .. } => *class == classe,
                    Type::ExtensionType { decl, nullable: false, .. } => *decl == classe,
                    _ => false,
                };
                if !e_this {
                    continue;
                }
                let corpo = match &f.body {
                    FunctionBody::Block(b) => a.stmt(*b).span,
                    FunctionBody::Expression(e) => a.expr(*e).span,
                    _ => continue,
                };
                // As criações do corpo em pré-ordem; a que casa não deixa
                // visitar as de dentro dela; vale a última visitada.
                let mut criacoes: Vec<ExprId> = (0..a.exprs.len())
                    .map(|i| ExprId(i as u32))
                    .filter(|e| {
                        let sp = a.expr(*e).span;
                        sp.start >= corpo.start && sp.end <= corpo.end && matches!(s.corpo.get_resolved(*e), Some(Resolved::Constructor(_)))
                    })
                    .collect();
                criacoes.sort_by_key(|e| (a.expr(*e).span.start, usize::MAX - a.expr(*e).span.end));
                let mut achou = false;
                let mut pular: Option<Span> = None;
                for e in criacoes {
                    let sp = a.expr(e).span;
                    if pular.is_some_and(|p| sp.start >= p.start && sp.end <= p.end) {
                        continue;
                    }
                    achou = s.corpo.get_type(e) == Some(tr);
                    if achou {
                        pular = Some(sp);
                    }
                }
                if achou && let Some(n) = f.name {
                    relatar(&c::PREFER_CONSTRUCTORS_OVER_STATIC_METHODS, n.span, &[]);
                }
            }
        }
    }

    // `invalid_case_patterns`: só antes da 3.0 (sem `patterns`).
    if ligada("invalid_case_patterns") && program.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(3, 0) {
        for st in a.stmts.iter() {
            let StmtKind::Switch { cases, .. } = &st.kind else { continue };
            for caso in cases.iter() {
                let Some(p) = caso.pattern else { continue };
                let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
                let x = sem_parenteses(a, *e);
                let relata = match &a.expr(x).kind {
                    ExprKind::SetOrMap { const_, .. } | ExprKind::List { const_, .. } => !*const_,
                    ExprKind::Call { target, .. } => {
                        // A criação sem `const` (constante pelo contexto) ou
                        // o `identical` do `dart:core`.
                        if matches!(s.corpo.get_resolved(x), Some(Resolved::Constructor(_))) {
                            true
                        } else {
                            matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if interner.resolve(n.sym) == "identical")
                                && matches!(s.corpo.get_resolved(*target), Some(Resolved::Element(Element::Function(f))) if program.library(program.function(*f).library).uri == "dart:core")
                        }
                    }
                    ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot, operand } => !matches!(a.expr(*operand).kind, ExprKind::Int(_)),
                    ExprKind::Binary { .. } | ExprKind::Conditional { .. } | ExprKind::Is { .. } => true,
                    ExprKind::Property { target, name, .. } => {
                        // `PropertyAccess` (não `a.b` com `a` identificador).
                        !matches!(a.expr(*target).kind, ExprKind::Identifier(_))
                            && interner.resolve(name.sym) == "length"
                            && match s.corpo.get_resolved(x) {
                                Some(Resolved::Member { member: MemberRef::Function(f), .. }) => program.library(program.function(*f).library).uri == "dart:core",
                                _ => false,
                            }
                    }
                    ExprKind::InstanceCreation { keyword, .. } => *keyword != Some(dartforge_frontend::ast::CreationKeyword::Const),
                    ExprKind::Identifier(n) => interner.resolve(n.sym) == "_",
                    _ => false,
                };
                if relata {
                    relatar(&c::INVALID_CASE_PATTERNS, a.expr(x).span, &[]);
                }
            }
        }
    }

    // `avoid_null_checks_in_equality_operators`.
    if ligada("avoid_null_checks_in_equality_operators") {
        for m in a.members.iter() {
            let MemberKind::Method(fid) = &m.kind else { continue };
            let f = a.function(*fid);
            if f.name.is_none_or(|n| interner.resolve(n.sym) != "==") {
                continue;
            }
            let Some(ps) = &f.parameters else { continue };
            let [p] = &ps[..] else { continue };
            let Some(pn) = p.name else { continue };
            // O tipo declarado do parâmetro precisa ter `?`.
            let anulavel = match p.ty {
                Some(t) => a.ty(t).nullable,
                None => true,
            };
            if !anulavel {
                continue;
            }
            let corpo = match &f.body {
                FunctionBody::Block(b) => a.stmt(*b).span,
                FunctionBody::Expression(e) => a.expr(*e).span,
                _ => continue,
            };
            let e_o_parametro = |e: ExprId| {
                let e = sem_parenteses(a, e);
                matches!(&a.expr(e).kind, ExprKind::Identifier(n) if n.sym == pn.sym) && matches!(s.corpo.get_resolved(e), Some(Resolved::Parameter { .. }))
            };
            let e_nulo = |e: ExprId| matches!(a.expr(sem_parenteses(a, e)).kind, ExprKind::Null);
            for e in a.exprs.iter() {
                if e.span.start < corpo.start || e.span.end > corpo.end {
                    continue;
                }
                let relata = match &e.kind {
                    ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, left, right } => (e_nulo(*left) && e_o_parametro(*right)) || (e_nulo(*right) && e_o_parametro(*left)),
                    ExprKind::Binary { op: BinaryOp::IfNull, left, .. } => e_o_parametro(*left),
                    ExprKind::Property { target, null_aware: true, .. } => e_o_parametro(*target),
                    _ => false,
                };
                if relata {
                    // `x?.m()`: o nó é a invocação inteira.
                    let mut sp = e.span;
                    if let ExprKind::Property { .. } = &e.kind
                        && let Some(pai) = a.exprs.iter().find(|y| matches!(&y.kind, ExprKind::Call { target, .. } if a.expr(*target).span == e.span))
                    {
                        sp = pai.span;
                    }
                    relatar(&c::AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS, sp, &[]);
                }
            }
        }
    }

    // `sized_box_shrink_expand`.
    if ligada("sized_box_shrink_expand") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(id) else { continue };
            if !interner.resolve(program.function(*f).name).is_empty() {
                continue;
            }
            if !s.corpo.get_type(id).is_some_and(|t| super::flutter::e_sized_box(s, interner, t)) {
                continue;
            }
            let args = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            if args.args.iter().any(|x| x.name.is_none()) {
                continue;
            }
            let valor = |x: ExprId| -> Option<f64> {
                match &a.expr(x).kind {
                    ExprKind::Int(sp) => {
                        let t: String = fonte[sp.start..sp.end].chars().filter(|c| *c != '_').collect();
                        if t.starts_with('-') {
                            return None;
                        }
                        match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                            Some(h) => u64::from_str_radix(h, 16).ok().map(|v| v as i64 as f64),
                            None => t.parse::<i64>().ok().map(|v| v as f64),
                        }
                    }
                    ExprKind::Double(sp) => {
                        let t: String = fonte[sp.start..sp.end].chars().filter(|c| *c != '_').collect();
                        if t.starts_with('-') { None } else { t.parse::<f64>().ok() }
                    }
                    ExprKind::Property { target, name, .. } => match &a.expr(*target).kind {
                        ExprKind::Identifier(p) if interner.resolve(p.sym) == "double" && interner.resolve(name.sym) == "infinity" => Some(f64::INFINITY),
                        _ => None,
                    },
                    _ => None,
                }
            };
            let (mut largura, mut altura): (Option<f64>, Option<f64>) = (None, None);
            for x in args.args.iter() {
                match interner.resolve(x.name.expect("nomeado").sym) {
                    "width" => largura = valor(x.value),
                    "height" => altura = valor(x.value),
                    _ => {}
                }
            }
            let nome = match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => a.ty(*ty).span,
                ExprKind::Call { target, .. } => a.expr(*target).span,
                _ => e.span,
            };
            if largura == Some(0.0) && altura == Some(0.0) {
                relatar(&c::SIZED_BOX_SHRINK_EXPAND, nome, &["shrink"]);
            } else if largura == Some(f64::INFINITY) && altura == Some(f64::INFINITY) {
                relatar(&c::SIZED_BOX_SHRINK_EXPAND, nome, &["expand"]);
            }
        }
    }

    // `implicit_reopen`.
    if ligada("implicit_reopen") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "reopen")) {
                continue;
            }
            let m = &x.modifiers;
            if m.sealed || m.mixin {
                continue;
            }
            let Some(sup) = program.class(classe).supertype_class else { continue };
            let ks = program.class(sup);
            if ks.kind != ClassKind::Class || ks.library != lib {
                continue;
            }
            let razao = if m.base {
                if ks.modifiers.final_ {
                    Some("final")
                } else if ks.modifiers.interface {
                    Some("interface")
                } else {
                    None
                }
            } else if !m.interface && !m.base && !m.sealed && !m.final_ {
                ks.modifiers.interface.then_some("interface")
            } else {
                None
            };
            if let Some(r) = razao {
                relatar(&c::IMPLICIT_REOPEN, x.name.span, &["class", interner.resolve(x.name.sym), interner.resolve(ks.name), r]);
            }
        }
    }

    // `noop_primitive_operations`.
    if ligada("noop_primitive_operations") {
        let to_string_vazio = |e: ExprId| -> Option<Span> {
            let ExprKind::Call { target, arguments } = &a.expr(e).kind else { return None };
            let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { return None };
            if matches!(a.expr(*alvo).kind, ExprKind::Super) || interner.resolve(name.sym) != "toString" || !arguments.args.is_empty() {
                return None;
            }
            Some(name.span)
        };
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::String(l) => {
                    // As strings adjacentes: os literais do meio vazios.
                    let literais = super::cordas::literais(fonte, l.span);
                    if literais.len() > 2 {
                        for lit in &literais[1..literais.len() - 1] {
                            if !lit.interpolado() && lit.texto(fonte).is_empty() {
                                relatar(&c::NOOP_PRIMITIVE_OPERATIONS, lit.span, &[]);
                            }
                        }
                    }
                    for p in l.parts.iter() {
                        if let StringPart::Interpolation(x) = p
                            && let Some(sp) = to_string_vazio(*x)
                        {
                            relatar(&c::NOOP_PRIMITIVE_OPERATIONS, sp, &[]);
                        }
                    }
                }
                ExprKind::Call { target, arguments } => {
                    let (alvo, nome) = match &a.expr(*target).kind {
                        ExprKind::Property { target: t, name, .. } => (Some(*t), *name),
                        ExprKind::Identifier(n) => (None, *n),
                        _ => continue,
                    };
                    let metodo = interner.resolve(nome.sym);
                    let tipo = alvo.filter(|t| !matches!(a.expr(*t).kind, ExprKind::CascadeTarget)).and_then(|t| s.corpo.get_type(t));
                    match tipo {
                        None => {
                            // `print(x.toString())`.
                            let e_print = matches!(s.corpo.get_resolved(*target), Some(Resolved::Element(Element::Function(f))) if interner.resolve(program.function(*f).name) == "print" && program.library(program.function(*f).library).uri == "dart:core");
                            if alvo.is_none()
                                && e_print
                                && let [x] = &arguments.args[..]
                                && let Some(sp) = to_string_vazio(x.value)
                            {
                                relatar(&c::NOOP_PRIMITIVE_OPERATIONS, sp, &[]);
                            }
                        }
                        Some(t) => {
                            if e_classe(s, interner, t, "String", "dart.core") && metodo == "toString" && nao_anulavel(table, t) {
                                relatar(&c::NOOP_PRIMITIVE_OPERATIONS, nome.span, &[]);
                            } else if e_classe(s, interner, t, "int", "dart.core") && matches!(metodo, "toInt" | "round" | "ceil" | "floor" | "truncate") {
                                relatar(&c::NOOP_PRIMITIVE_OPERATIONS, nome.span, &[]);
                            } else if e_classe(s, interner, t, "double", "dart.core") && metodo == "toDouble" {
                                relatar(&c::NOOP_PRIMITIVE_OPERATIONS, nome.span, &[]);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // `unawaited_futures`.
    if ligada("unawaited_futures") {
        let e_future = |t: TypeId| e_classe(s, interner, t, "Future", "dart.async");
        let implementa_future = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } => {
                let f = |k: ClassId| {
                    let x = program.class(k);
                    interner.resolve(x.name) == "Future" && nome_da_biblioteca(s, interner, x.library).as_deref() == Some("dart.async")
                };
                f(*class) || (program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| f(*k))))
            }
            _ => false,
        };
        for st in a.stmts.iter() {
            let StmtKind::Expression(e) = &st.kind else { continue };
            if matches!(a.expr(*e).kind, ExprKind::Assign { .. }) {
                continue;
            }
            let Some(t) = s.corpo.get_type(*e) else { continue };
            if !implementa_future(t) {
                continue;
            }
            // `Future.delayed(d, f)` e o `putIfAbsent` de `Map`.
            let atrasado = matches!(s.corpo.get_resolved(*e), Some(Resolved::Constructor(f)) if interner.resolve(program.function(*f).name) == "delayed")
                && e_future(t)
                && matches!(&a.expr(*e).kind, ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } if arguments.args.len() == 2);
            let put_if_absent = match &a.expr(*e).kind {
                ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } if interner.resolve(name.sym) == "putIfAbsent" => match s.corpo.get_resolved(*target) {
                        Some(Resolved::Member { class, .. }) => {
                            let k = program.class(*class);
                            interner.resolve(k.name) == "Map" && nome_da_biblioteca(s, interner, k.library).as_deref() == Some("dart.core")
                        }
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            };
            if atrasado || put_if_absent {
                continue;
            }
            if em_corpo_assincrono(a, st.span) {
                relatar(&c::UNAWAITED_FUTURES, st.span, &[]);
            }
        }
        let visitar = |x: ExprId, relatar: &mut dyn FnMut(Span)| {
            if matches!(a.expr(x).kind, ExprKind::Assign { .. }) {
                return;
            }
            if s.corpo.get_type(x).is_some_and(e_future) && em_corpo_assincrono(a, a.expr(x).span) {
                relatar(a.expr(x).span);
            }
        };
        let mut achados: Vec<Span> = Vec::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Cascade { sections, .. } => {
                    for &sec in sections.iter() {
                        visitar(sec, &mut |sp| achados.push(sp));
                    }
                }
                ExprKind::String(l) => {
                    for p in l.parts.iter() {
                        if let StringPart::Interpolation(x) = p {
                            visitar(*x, &mut |sp| achados.push(sp));
                        }
                    }
                }
                _ => {}
            }
        }
        for sp in achados {
            relatar(&c::UNAWAITED_FUTURES, sp, &[]);
        }
    }

    // `avoid_returning_this`.
    if ligada("avoid_returning_this") {
        for (k, d) in a.decls.iter().enumerate() {
            let membros = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                _ => continue,
            };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            for &mid in membros {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.kind == dartforge_frontend::ast::FunctionKind::Operator {
                    continue;
                }
                let Some(n) = f.name else { continue };
                // `isOverride`: algum supertipo tem membro da mesma espécie.
                let chave = if f.kind == dartforge_frontend::ast::FunctionKind::Setter { interner.lookup(&format!("{}_=", interner.resolve(n.sym))) } else { Some(n.sym) };
                let sobrescreve = chave.is_some_and(|ch| {
                    s.outline.hierarchy.get(classe).is_some_and(|h| {
                        h.supertypes.keys().any(|sc| {
                            program.class(*sc).instance_members.get(&ch).is_some_and(|g| {
                                let gk = program.function(*g).kind;
                                match f.kind {
                                    dartforge_frontend::ast::FunctionKind::Getter => gk == FunctionKind::Getter,
                                    dartforge_frontend::ast::FunctionKind::Setter => gk == FunctionKind::Setter,
                                    _ => matches!(gk, FunctionKind::Function | FunctionKind::Operator),
                                }
                            })
                        })
                    })
                });
                if sobrescreve {
                    continue;
                }
                // O tipo de retorno do elemento é a própria classe.
                let retorno = (0..program.functions.len()).find_map(|i| match program.functions[i].node {
                    dartforge_elements::model::FunctionRef::Function { unit, function } if unit == s.unidade && function == *fid => s.outline.functions.get(i).map(|d| d.return_type),
                    _ => None,
                });
                let e_propria = retorno.is_some_and(|t| matches!(table.get(t), Type::Interface { class, .. } if *class == classe));
                if !e_propria {
                    continue;
                }
                match &f.body {
                    FunctionBody::Block(b) => {
                        let corpo = a.stmt(*b).span;
                        // As funções aninhadas não contam.
                        let aninhadas: Vec<Span> = a.functions.iter().filter(|g| g.span.start >= corpo.start && g.span.end <= corpo.end).map(|g| g.span).collect();
                        let mut retornos: Vec<Span> = Vec::new();
                        let mut outro = false;
                        let mut stmts: Vec<&dartforge_frontend::ast::Stmt> = a.stmts.iter().filter(|x| x.span.start >= corpo.start && x.span.end <= corpo.end).collect();
                        stmts.sort_by_key(|x| x.span.start);
                        for x in stmts {
                            let StmtKind::Return(v) = &x.kind else { continue };
                            if aninhadas.iter().any(|g| x.span.start >= g.start && x.span.end <= g.end) {
                                continue;
                            }
                            match v {
                                Some(e) if matches!(a.expr(*e).kind, ExprKind::This) => retornos.push(a.expr(*e).span),
                                _ => {
                                    outro = true;
                                    break;
                                }
                            }
                        }
                        if !outro && let Some(&primeiro) = retornos.first() {
                            relatar(&c::AVOID_RETURNING_THIS, primeiro, &[]);
                        }
                    }
                    FunctionBody::Expression(e) => {
                        if matches!(a.expr(*e).kind, ExprKind::This) {
                            relatar(&c::AVOID_RETURNING_THIS, n.span, &[]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    out
}
