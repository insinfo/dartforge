//! The twenty-first batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), through the
//! unit's semantics:
//!
//! * `omit_obvious_local_variable_types`: the written type of a list of
//!   locals (also a `for`'s) whose initializers have an obvious type
//!   (`super::obvious`) equal to it, and that of a `for-in` variable equal to
//!   the element type of an obvious iterable.
//! * `prefer_constructors_over_static_methods`: a static method (without type
//!   parameters) that returns the `thisType` of a non-generic class and whose
//!   last visited creation in the body is of that type.
//! * `invalid_case_patterns`: old `case` expressions (library before 3.0) that
//!   become something else as a pattern.
//! * `avoid_null_checks_in_equality_operators`: `== null`, `??` and `?.` on
//!   the nullable parameter of `operator ==`.
//! * `sized_box_shrink_expand`: `SizedBox(width: 0, height: 0)` and with
//!   `double.infinity`.
//! * `implicit_reopen`: a class that reopens, without `@reopen`, a `final`/
//!   `interface` superclass of the library.
//! * `noop_primitive_operations`: the empty literal in the middle of adjacent
//!   strings, the interpolated `toString()` or one passed to `print`, and the
//!   conversions that do not change the value.
//! * `unawaited_futures`: a `Future` discarded in an async body (statement,
//!   cascade section, interpolation).
//! * `avoid_returning_this`: a method that only returns `this` (the class's
//!   own type).
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
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

fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn is_non_nullable(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => is_non_nullable(table, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && is_non_nullable(table, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && is_non_nullable(table, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        Type::Never => true,
    }
}

fn library_name(s: &super::Semantics<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
}

/// The type is the class `name` of `dart:core`/`dart:async` (the element).
fn is_class_named(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, name: &str, lib: &str) -> bool {
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let k = s.program.class(*class);
            interner.resolve(k.name) == name && library_name(s, interner, k.library).as_deref() == Some(lib)
        }
        _ => false,
    }
}

/// The innermost function body that contains `sp` is async.
fn in_async_body(a: &Ast, sp: Span) -> bool {
    let mut best: Option<(usize, bool)> = None;
    for f in a.functions.iter() {
        let body = match &f.body {
            FunctionBody::Block(b) => a.stmt(*b).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if body.start <= sp.start && sp.end <= body.end {
            let len = body.end - body.start;
            if best.is_none_or(|(t, _)| len < t) {
                best = Some((len, matches!(f.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar)));
            }
        }
    }
    // An innermost constructor body is not async.
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let FunctionBody::Block(b) = &k.body
        {
            let body = a.stmt(*b).span;
            if body.start <= sp.start && sp.end <= body.end {
                let len = body.end - body.start;
                if best.is_none_or(|(t, _)| len < t) {
                    best = Some((len, false));
                }
            }
        }
    }
    best.is_some_and(|(_, x)| x)
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let source = u.fonte;
    let program = s.program;
    let table = s.table;
    let lib = program.unit(s.unit).library;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `omit_obvious_local_variable_types`.
    if enabled("omit_obvious_local_variable_types") {
        let mut lists: Vec<&dartforge_frontend::ast::VariableList> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) => lists.push(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => lists.push(l),
                StmtKind::ForIn { target: ForInTarget::Declared { ty: Some(ty), .. }, iterable, .. } => {
                    let Some(&tl) = s.body.tipos_de_anotacoes.get(ty) else { continue };
                    if matches!(table.get(tl), Type::Dynamic) || !super::obvious::has_obvious_type(s, interner, a, *iterable) {
                        continue;
                    }
                    if super::obvious::iterable_element_type(s, interner, s.body.get_type(*iterable)) == Some(tl) {
                        report(&c::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES, a.ty(*ty).span, &[]);
                    }
                }
                _ => {}
            }
        }
        for l in lists {
            let Some(ty) = l.ty else { continue };
            let Some(&tl) = s.body.tipos_de_anotacoes.get(&ty) else { continue };
            if matches!(table.get(tl), Type::Null) {
                continue;
            }
            let all_obvious = l.variables.iter().all(|v| match v.initializer {
                Some(i) => super::obvious::has_obvious_type(s, interner, a, i) && s.body.get_type(i) == Some(tl),
                None => false,
            });
            if all_obvious {
                report(&c::OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES, a.ty(ty).span, &[]);
            }
        }
    }

    // `prefer_constructors_over_static_methods`.
    if enabled("prefer_constructors_over_static_methods") {
        for (k, d) in a.decls.iter().enumerate() {
            let (members, no_type_params) = match &d.kind {
                DeclKind::Class(x) => (&x.members, x.type_params.is_empty()),
                DeclKind::ExtensionType(x) => (&x.members, x.type_params.is_empty()),
                _ => continue,
            };
            if !no_type_params {
                continue;
            }
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            for &mid in members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if !f.static_ || !f.type_params.is_empty() {
                    continue;
                }
                let Some(rt) = f.return_type else { continue };
                let Some(&tr) = s.outline.tipos_escritos.get(&(s.unit, rt)) else { continue };
                let is_this = match table.get(tr) {
                    Type::Interface { class, nullable: false, .. } => *class == class_id,
                    Type::ExtensionType { decl, nullable: false, .. } => *decl == class_id,
                    _ => false,
                };
                if !is_this {
                    continue;
                }
                let body = match &f.body {
                    FunctionBody::Block(b) => a.stmt(*b).span,
                    FunctionBody::Expression(e) => a.expr(*e).span,
                    _ => continue,
                };
                // The body's creations in pre-order; the matching one does not
                // let the ones inside it be visited; the last visited counts.
                let mut creations: Vec<ExprId> = (0..a.exprs.len())
                    .map(|i| ExprId(i as u32))
                    .filter(|e| {
                        let sp = a.expr(*e).span;
                        sp.start >= body.start && sp.end <= body.end && matches!(s.body.get_resolved(*e), Some(Resolved::Constructor(_)))
                    })
                    .collect();
                creations.sort_by_key(|e| (a.expr(*e).span.start, usize::MAX - a.expr(*e).span.end));
                let mut found = false;
                let mut skip: Option<Span> = None;
                for e in creations {
                    let sp = a.expr(e).span;
                    if skip.is_some_and(|p| sp.start >= p.start && sp.end <= p.end) {
                        continue;
                    }
                    found = s.body.get_type(e) == Some(tr);
                    if found {
                        skip = Some(sp);
                    }
                }
                if found && let Some(n) = f.name {
                    report(&c::PREFER_CONSTRUCTORS_OVER_STATIC_METHODS, n.span, &[]);
                }
            }
        }
    }

    // `invalid_case_patterns`: only before 3.0 (without `patterns`).
    if enabled("invalid_case_patterns") && program.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(3, 0) {
        for st in a.stmts.iter() {
            let StmtKind::Switch { cases, .. } = &st.kind else { continue };
            for case in cases.iter() {
                let Some(p) = case.pattern else { continue };
                let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
                let x = unparenthesized(a, *e);
                let reported = match &a.expr(x).kind {
                    ExprKind::SetOrMap { const_, .. } | ExprKind::List { const_, .. } => !*const_,
                    ExprKind::Call { target, .. } => {
                        // A creation without `const` (constant by context) or
                        // `dart:core`'s `identical`.
                        if matches!(s.body.get_resolved(x), Some(Resolved::Constructor(_))) {
                            true
                        } else {
                            matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if interner.resolve(n.sym) == "identical")
                                && matches!(s.body.get_resolved(*target), Some(Resolved::Element(Element::Function(f))) if program.library(program.function(*f).library).uri == "dart:core")
                        }
                    }
                    ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot, operand } => !matches!(a.expr(*operand).kind, ExprKind::Int(_)),
                    ExprKind::Binary { .. } | ExprKind::Conditional { .. } | ExprKind::Is { .. } => true,
                    ExprKind::Property { target, name, .. } => {
                        // `PropertyAccess` (not `a.b` with `a` an identifier).
                        !matches!(a.expr(*target).kind, ExprKind::Identifier(_))
                            && interner.resolve(name.sym) == "length"
                            && match s.body.get_resolved(x) {
                                Some(Resolved::Member { member: MemberRef::Function(f), .. }) => program.library(program.function(*f).library).uri == "dart:core",
                                _ => false,
                            }
                    }
                    ExprKind::InstanceCreation { keyword, .. } => *keyword != Some(dartforge_frontend::ast::CreationKeyword::Const),
                    ExprKind::Identifier(n) => interner.resolve(n.sym) == "_",
                    _ => false,
                };
                if reported {
                    report(&c::INVALID_CASE_PATTERNS, a.expr(x).span, &[]);
                }
            }
        }
    }

    // `avoid_null_checks_in_equality_operators`.
    if enabled("avoid_null_checks_in_equality_operators") {
        for m in a.members.iter() {
            let MemberKind::Method(fid) = &m.kind else { continue };
            let f = a.function(*fid);
            if f.name.is_none_or(|n| interner.resolve(n.sym) != "==") {
                continue;
            }
            let Some(ps) = &f.parameters else { continue };
            let [p] = &ps[..] else { continue };
            let Some(pn) = p.name else { continue };
            // The parameter's declared type must have `?`.
            let is_nullable = match p.ty {
                Some(t) => a.ty(t).nullable,
                None => true,
            };
            if !is_nullable {
                continue;
            }
            let body = match &f.body {
                FunctionBody::Block(b) => a.stmt(*b).span,
                FunctionBody::Expression(e) => a.expr(*e).span,
                _ => continue,
            };
            let is_the_param = |e: ExprId| {
                let e = unparenthesized(a, e);
                matches!(&a.expr(e).kind, ExprKind::Identifier(n) if n.sym == pn.sym) && matches!(s.body.get_resolved(e), Some(Resolved::Parameter { .. }))
            };
            let is_null = |e: ExprId| matches!(a.expr(unparenthesized(a, e)).kind, ExprKind::Null);
            for e in a.exprs.iter() {
                if e.span.start < body.start || e.span.end > body.end {
                    continue;
                }
                let reported = match &e.kind {
                    ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, left, right } => (is_null(*left) && is_the_param(*right)) || (is_null(*right) && is_the_param(*left)),
                    ExprKind::Binary { op: BinaryOp::IfNull, left, .. } => is_the_param(*left),
                    ExprKind::Property { target, null_aware: true, .. } => is_the_param(*target),
                    _ => false,
                };
                if reported {
                    // `x?.m()`: the node is the whole invocation.
                    let mut sp = e.span;
                    if let ExprKind::Property { .. } = &e.kind
                        && let Some(parent) = a.exprs.iter().find(|y| matches!(&y.kind, ExprKind::Call { target, .. } if a.expr(*target).span == e.span))
                    {
                        sp = parent.span;
                    }
                    report(&c::AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS, sp, &[]);
                }
            }
        }
    }

    // `sized_box_shrink_expand`.
    if enabled("sized_box_shrink_expand") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let Some(Resolved::Constructor(f)) = s.body.get_resolved(id) else { continue };
            if !interner.resolve(program.function(*f).name).is_empty() {
                continue;
            }
            if !s.body.get_type(id).is_some_and(|t| super::flutter::is_sized_box(s, interner, t)) {
                continue;
            }
            let args = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            if args.args.iter().any(|x| x.name.is_none()) {
                continue;
            }
            let value_of = |x: ExprId| -> Option<f64> {
                match &a.expr(x).kind {
                    ExprKind::Int(sp) => {
                        let t: String = source[sp.start..sp.end].chars().filter(|c| *c != '_').collect();
                        if t.starts_with('-') {
                            return None;
                        }
                        match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                            Some(h) => u64::from_str_radix(h, 16).ok().map(|v| v as i64 as f64),
                            None => t.parse::<i64>().ok().map(|v| v as f64),
                        }
                    }
                    ExprKind::Double(sp) => {
                        let t: String = source[sp.start..sp.end].chars().filter(|c| *c != '_').collect();
                        if t.starts_with('-') { None } else { t.parse::<f64>().ok() }
                    }
                    ExprKind::Property { target, name, .. } => match &a.expr(*target).kind {
                        ExprKind::Identifier(p) if interner.resolve(p.sym) == "double" && interner.resolve(name.sym) == "infinity" => Some(f64::INFINITY),
                        _ => None,
                    },
                    _ => None,
                }
            };
            let (mut width, mut height): (Option<f64>, Option<f64>) = (None, None);
            for x in args.args.iter() {
                match interner.resolve(x.name.expect("nomeado").sym) {
                    "width" => width = value_of(x.value),
                    "height" => height = value_of(x.value),
                    _ => {}
                }
            }
            let name = match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => a.ty(*ty).span,
                ExprKind::Call { target, .. } => a.expr(*target).span,
                _ => e.span,
            };
            if width == Some(0.0) && height == Some(0.0) {
                report(&c::SIZED_BOX_SHRINK_EXPAND, name, &["shrink"]);
            } else if width == Some(f64::INFINITY) && height == Some(f64::INFINITY) {
                report(&c::SIZED_BOX_SHRINK_EXPAND, name, &["expand"]);
            }
        }
    }

    // `implicit_reopen`.
    if enabled("implicit_reopen") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            if d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "reopen")) {
                continue;
            }
            let m = &x.modifiers;
            if m.sealed || m.mixin {
                continue;
            }
            let Some(sup) = program.class(class_id).supertype_class else { continue };
            let ks = program.class(sup);
            if ks.kind != ClassKind::Class || ks.library != lib {
                continue;
            }
            let reason = if m.base {
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
            if let Some(r) = reason {
                report(&c::IMPLICIT_REOPEN, x.name.span, &["class", interner.resolve(x.name.sym), interner.resolve(ks.name), r]);
            }
        }
    }

    // `noop_primitive_operations`.
    if enabled("noop_primitive_operations") {
        let empty_to_string = |e: ExprId| -> Option<Span> {
            let ExprKind::Call { target, arguments } = &a.expr(e).kind else { return None };
            let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind else { return None };
            if matches!(a.expr(*recv).kind, ExprKind::Super) || interner.resolve(name.sym) != "toString" || !arguments.args.is_empty() {
                return None;
            }
            Some(name.span)
        };
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::String(l) => {
                    // Adjacent strings: the empty literals in the middle.
                    let literals = super::strings::literals(source, l.span);
                    if literals.len() > 2 {
                        for lit in &literals[1..literals.len() - 1] {
                            if !lit.is_interpolated() && lit.text(source).is_empty() {
                                report(&c::NOOP_PRIMITIVE_OPERATIONS, lit.span, &[]);
                            }
                        }
                    }
                    for p in l.parts.iter() {
                        if let StringPart::Interpolation(x) = p
                            && let Some(sp) = empty_to_string(*x)
                        {
                            report(&c::NOOP_PRIMITIVE_OPERATIONS, sp, &[]);
                        }
                    }
                }
                ExprKind::Call { target, arguments } => {
                    let (recv, name) = match &a.expr(*target).kind {
                        ExprKind::Property { target: t, name, .. } => (Some(*t), *name),
                        ExprKind::Identifier(n) => (None, *n),
                        _ => continue,
                    };
                    let method = interner.resolve(name.sym);
                    let ty = recv.filter(|t| !matches!(a.expr(*t).kind, ExprKind::CascadeTarget)).and_then(|t| s.body.get_type(t));
                    match ty {
                        None => {
                            // `print(x.toString())`.
                            let is_print = matches!(s.body.get_resolved(*target), Some(Resolved::Element(Element::Function(f))) if interner.resolve(program.function(*f).name) == "print" && program.library(program.function(*f).library).uri == "dart:core");
                            if recv.is_none()
                                && is_print
                                && let [x] = &arguments.args[..]
                                && let Some(sp) = empty_to_string(x.value)
                            {
                                report(&c::NOOP_PRIMITIVE_OPERATIONS, sp, &[]);
                            }
                        }
                        Some(t) => {
                            if is_class_named(s, interner, t, "String", "dart.core") && method == "toString" && is_non_nullable(table, t) {
                                report(&c::NOOP_PRIMITIVE_OPERATIONS, name.span, &[]);
                            } else if is_class_named(s, interner, t, "int", "dart.core") && matches!(method, "toInt" | "round" | "ceil" | "floor" | "truncate") {
                                report(&c::NOOP_PRIMITIVE_OPERATIONS, name.span, &[]);
                            } else if is_class_named(s, interner, t, "double", "dart.core") && method == "toDouble" {
                                report(&c::NOOP_PRIMITIVE_OPERATIONS, name.span, &[]);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // `unawaited_futures`.
    if enabled("unawaited_futures") {
        let is_future = |t: TypeId| is_class_named(s, interner, t, "Future", "dart.async");
        let implements_future = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } => {
                let f = |k: ClassId| {
                    let x = program.class(k);
                    interner.resolve(x.name) == "Future" && library_name(s, interner, x.library).as_deref() == Some("dart.async")
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
            let Some(t) = s.body.get_type(*e) else { continue };
            if !implements_future(t) {
                continue;
            }
            // `Future.delayed(d, f)` and `Map`'s `putIfAbsent`.
            let delayed = matches!(s.body.get_resolved(*e), Some(Resolved::Constructor(f)) if interner.resolve(program.function(*f).name) == "delayed")
                && is_future(t)
                && matches!(&a.expr(*e).kind, ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } if arguments.args.len() == 2);
            let put_if_absent = match &a.expr(*e).kind {
                ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } if interner.resolve(name.sym) == "putIfAbsent" => match s.body.get_resolved(*target) {
                        Some(Resolved::Member { class, .. }) => {
                            let k = program.class(*class);
                            interner.resolve(k.name) == "Map" && library_name(s, interner, k.library).as_deref() == Some("dart.core")
                        }
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            };
            if delayed || put_if_absent {
                continue;
            }
            if in_async_body(a, st.span) {
                report(&c::UNAWAITED_FUTURES, st.span, &[]);
            }
        }
        let visit = |x: ExprId, report: &mut dyn FnMut(Span)| {
            if matches!(a.expr(x).kind, ExprKind::Assign { .. }) {
                return;
            }
            if s.body.get_type(x).is_some_and(is_future) && in_async_body(a, a.expr(x).span) {
                report(a.expr(x).span);
            }
        };
        let mut findings: Vec<Span> = Vec::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Cascade { sections, .. } => {
                    for &sec in sections.iter() {
                        visit(sec, &mut |sp| findings.push(sp));
                    }
                }
                ExprKind::String(l) => {
                    for p in l.parts.iter() {
                        if let StringPart::Interpolation(x) = p {
                            visit(*x, &mut |sp| findings.push(sp));
                        }
                    }
                }
                _ => {}
            }
        }
        for sp in findings {
            report(&c::UNAWAITED_FUTURES, sp, &[]);
        }
    }

    // `avoid_returning_this`.
    if enabled("avoid_returning_this") {
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                _ => continue,
            };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            for &mid in members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.kind == dartforge_frontend::ast::FunctionKind::Operator {
                    continue;
                }
                let Some(n) = f.name else { continue };
                // `isOverride`: some supertype has a member of the same kind.
                let key = if f.kind == dartforge_frontend::ast::FunctionKind::Setter { interner.lookup(&format!("{}_=", interner.resolve(n.sym))) } else { Some(n.sym) };
                let overrides = key.is_some_and(|ch| {
                    s.outline.hierarchy.get(class_id).is_some_and(|h| {
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
                if overrides {
                    continue;
                }
                // The element's return type is the class itself.
                let ret = (0..program.functions.len()).find_map(|i| match program.functions[i].node {
                    dartforge_elements::model::FunctionRef::Function { unit, function } if unit == s.unit && function == *fid => s.outline.functions.get(i).map(|d| d.return_type),
                    _ => None,
                });
                let is_own_class = ret.is_some_and(|t| matches!(table.get(t), Type::Interface { class, .. } if *class == class_id));
                if !is_own_class {
                    continue;
                }
                match &f.body {
                    FunctionBody::Block(b) => {
                        let body = a.stmt(*b).span;
                        // Nested functions do not count.
                        let nested: Vec<Span> = a.functions.iter().filter(|g| g.span.start >= body.start && g.span.end <= body.end).map(|g| g.span).collect();
                        let mut returns: Vec<Span> = Vec::new();
                        let mut other = false;
                        let mut stmts: Vec<&dartforge_frontend::ast::Stmt> = a.stmts.iter().filter(|x| x.span.start >= body.start && x.span.end <= body.end).collect();
                        stmts.sort_by_key(|x| x.span.start);
                        for x in stmts {
                            let StmtKind::Return(v) = &x.kind else { continue };
                            if nested.iter().any(|g| x.span.start >= g.start && x.span.end <= g.end) {
                                continue;
                            }
                            match v {
                                Some(e) if matches!(a.expr(*e).kind, ExprKind::This) => returns.push(a.expr(*e).span),
                                _ => {
                                    other = true;
                                    break;
                                }
                            }
                        }
                        if !other && let Some(&first) = returns.first() {
                            report(&c::AVOID_RETURNING_THIS, first, &[]);
                        }
                    }
                    FunctionBody::Expression(e) => {
                        if matches!(a.expr(*e).kind, ExprKind::This) {
                            report(&c::AVOID_RETURNING_THIS, n.span, &[]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    out
}
