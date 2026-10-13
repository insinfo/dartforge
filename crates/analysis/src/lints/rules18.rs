//! The eighteenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), through the
//! unit's semantics:
//!
//! * `avoid_equals_and_hash_code_on_mutable_classes`: `==`/`hashCode` declared
//!   in a class without `@immutable` (on it or its supertypes), at the first
//!   token after the annotations.
//! * `package_api_docs`: the 3.6.2 emitter reports nothing (`check` returns
//!   right away).
//! * `unnecessary_null_checks`: the `!` whose expected type
//!   (`getExpectedType`, `super::rules15`) is nullable, and the `p!` pattern
//!   with a nullable matched type.
//! * `avoid_unnecessary_containers`: the `Container` whose only argument is
//!   `child:` with a widget.
//! * `avoid_implementing_value_types`: the `implements` of a class whose
//!   concrete `==` is from a class (other than `Object`).
//! * `use_full_hex_values_for_flutter_colors`: `dart.ui`'s `Color(…)` with an
//!   integer literal that is not `0x` with eight digits.
//! * `use_test_throws_matchers`: `test_api`'s `fail()` at the end of a `try`
//!   with one `catch` and no `finally`.
//! * `do_not_use_environment`: `bool/int/String.fromEnvironment` and
//!   `bool.hasEnvironment`.
//! * `use_if_null_to_convert_nulls_to_bools`: `b == true` and `b != false`
//!   with `b` of type `bool?`.
//! * `annotate_redeclares`: an extension type member without `@redeclare`
//!   whose name is that of a member of the implemented interfaces.
//! * `avoid_types_on_closure_parameters`: the types written on the parameters
//!   of a closure with a function type context
//!   (`super::rules16::approximate_context`).
//! * `deprecated_consistency`: the constructor of a deprecated class and the
//!   `this.x` parameter that do not agree with the field.
//!
//! Written without compiling or running (2026-10-05).

use super::walk::{walk, Node};
use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId};
use dartforge_frontend::ast::{self, Annotation, Ast, BinaryOp, DeclId, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind, StmtKind, TypeKind, UnaryOp};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::{HashMap, HashSet};

fn is_nullable(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => is_nullable(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || is_nullable(table, *arg),
        Type::Never => false,
    }
}

fn library_name(s: &super::Semantics<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
}

/// `hasDeprecated`: `dart:core`'s `@deprecated` or `@Deprecated(…)`.
fn is_deprecated(s: &super::Semantics<'_>, interner: &Interner, u: dartforge_elements::model::UnitId, metadata: &[Annotation]) -> bool {
    metadata.iter().any(|m| {
        dartforge_types::annotations::is_getter_of(s.program, interner, u, m, "dart.core", "deprecated")
            || dartforge_types::annotations::is_constructor_of(s.program, interner, u, m, "dart.core", "Deprecated")
    })
}

/// The token that starts at `pos` (an identifier or one character).
fn token_at(source: &str, pos: usize) -> Span {
    let b = source.as_bytes();
    let mut end = pos;
    while end < b.len() && (b[end].is_ascii_alphanumeric() || b[end] == b'_' || b[end] == b'$') {
        end += 1;
    }
    if end == pos {
        end = (pos + source[pos..].chars().next().map_or(1, char::len_utf8)).min(b.len());
    }
    Span { start: pos, end }
}

/// The constructor's name as written (`ConstructorName`): the type and the
/// `.name`.
fn constructor_name_span(a: &Ast, e: ExprId) -> Span {
    match &a.expr(e).kind {
        ExprKind::InstanceCreation { ty, constructor, .. } => {
            let t = a.ty(*ty).span;
            Span { start: t.start, end: constructor.map_or(t.end, |n| n.span.end) }
        }
        ExprKind::Call { target, .. } => a.expr(*target).span,
        _ => a.expr(e).span,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    // `package_api_docs`: the 3.6.2 emitter returns without reporting.
    let _ = enabled("package_api_docs");
    let Some(s) = sem else { return out };
    let a = u.ast;
    let source = u.source;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let lib = program.unit(s.unit).library;
    let creations: Vec<(ExprId, dartforge_elements::model::FunctionElementId)> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, _)| match s.body.get_resolved(ExprId(k as u32)) {
            Some(Resolved::Constructor(f)) => Some((ExprId(k as u32), *f)),
            _ => None,
        })
        .collect();
    let arguments_of = |e: ExprId| -> Option<&ast::Arguments> {
        match &a.expr(e).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => Some(&**arguments),
            _ => None,
        }
    };

    // `avoid_equals_and_hash_code_on_mutable_classes`.
    if enabled("avoid_equals_and_hash_code_on_mutable_classes") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            let is_immutable = |c: ClassId| -> bool {
                let Some(r) = program.class(c).decl else { return false };
                program.unit(r.unit).ast.decl(r.decl).metadata.iter().any(|m| dartforge_types::annotations::is_getter_of(program, interner, r.unit, m, "meta", "immutable"))
            };
            let has = is_immutable(class_id) || s.outline.hierarchy.get(class_id).is_some_and(|h| h.supertypes.keys().any(|k| is_immutable(*k)));
            if has {
                continue;
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Method(f) = &m.kind else { continue };
                if m.augment {
                    continue;
                }
                let Some(n) = a.function(*f).name else { continue };
                let text = interner.resolve(n.sym);
                if text != "==" && text != "hashCode" {
                    continue;
                }
                let pos = match m.metadata.last() {
                    Some(x) => dartforge_frontend::source::skip_whitespace(source.as_bytes(), x.span.end),
                    None => {
                        // After the documentation comment.
                        dartforge_frontend::source::skip_whitespace(source.as_bytes(), m.span.start)
                    }
                };
                report(&c::AVOID_EQUALS_AND_HASH_CODE_ON_MUTABLE_CLASSES, token_at(source, pos), &[text]);
            }
        }
    }

    // `unnecessary_null_checks`.
    if enabled("unnecessary_null_checks") {
        for (k, p) in a.patterns.iter().enumerate() {
            if let PatternKind::NullAssert(_) = &p.kind
                && let Some(&t) = s.body.matched_types.get(&PatternId(k as u32))
                && is_nullable(table, t)
            {
                report(&c::UNNECESSARY_NULL_CHECKS, Span { start: p.span.end - 1, end: p.span.end }, &[]);
            }
        }
        let mut parent: HashMap<ExprId, Node> = HashMap::new();
        let mut bangs: Vec<(ExprId, Vec<Node>)> = Vec::new();
        walk(u, &mut |node, ancestors| {
            if let Node::Expr(e) = node {
                if let Some(p) = ancestors.last() {
                    parent.insert(e, *p);
                }
                if matches!(a.expr(e).kind, ExprKind::Unary { op: UnaryOp::NullAssert, .. }) {
                    bangs.push((e, ancestors.to_vec()));
                }
            }
        });
        let mut cascades: HashMap<ExprId, ExprId> = HashMap::new();
        for e in a.exprs.iter() {
            let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
            for &sec in sections.iter() {
                let mut x = sec;
                loop {
                    let next = match &a.expr(x).kind {
                        ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::Assign { target, .. } => *target,
                        ExprKind::CascadeTarget => {
                            cascades.insert(x, *target);
                            break;
                        }
                        _ => break,
                    };
                    x = next;
                }
            }
        }
        let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascades.get(&x).copied() } else { Some(x) };
        let functions = super::rules15::FunctionIndex::new(s, a);
        bangs.sort_by_key(|(e, _)| a.expr(*e).span.start);
        for (node, ancestors) in bangs {
            let expected = super::rules15::expected_type(s, a, interner, &functions, &parent, &ancestors, node, &real);
            if expected.is_some_and(|t| is_nullable(table, t)) {
                let sp = a.expr(node).span;
                report(&c::UNNECESSARY_NULL_CHECKS, Span { start: sp.end - 1, end: sp.end }, &[]);
            }
        }
    }

    // `avoid_unnecessary_containers`.
    if enabled("avoid_unnecessary_containers") {
        let parents = super::rules16::unit_parents(u, &|e| matches!(s.body.get_resolved(e), Some(Resolved::Constructor(_))));
        for &(e, _) in &creations {
            if !s.body.get_type(e).is_some_and(|t| super::flutter::is_widget_type(s, interner, t)) {
                continue;
            }
            let Some(Node::Expr(p)) = parents.expr.get(&e).copied() else { continue };
            let Some(args) = arguments_of(p) else { continue };
            let Some(arg) = args.args.iter().find(|x| x.value == e) else { continue };
            if arg.name.is_none_or(|n| interner.resolve(n.sym) != "child") || args.args.len() != 1 {
                continue;
            }
            // The nearest creation that contains the argument.
            let owner = if matches!(s.body.get_resolved(p), Some(Resolved::Constructor(_))) {
                Some(p)
            } else {
                parents.ancestors.get(&e).and_then(|anc| {
                    anc.iter().rev().find_map(|n| match n {
                        Node::Expr(x) if matches!(s.body.get_resolved(*x), Some(Resolved::Constructor(_))) => Some(*x),
                        _ => None,
                    })
                })
            };
            if let Some(d) = owner
                && s.body.get_type(d).is_some_and(|t| super::flutter::is_container(s, interner, t))
            {
                report(&c::AVOID_UNNECESSARY_CONTAINERS, constructor_name_span(a, d), &[]);
            }
        }
    }

    // `avoid_implementing_value_types`.
    if enabled("avoid_implementing_value_types") {
        let object = s.core.object_class;
        let overrides_equals = |c: ClassId| -> bool {
            let Some(key) = interner.lookup("==") else { return false };
            let mut seen: HashSet<ClassId> = HashSet::new();
            let mut current = Some(c);
            while let Some(x) = current {
                if !seen.insert(x) {
                    break;
                }
                let k = program.class(x);
                let candidates = std::iter::once(x).chain(k.mixin_classes.iter().rev().copied());
                for cc in candidates {
                    if let Some(&f) = program.class(cc).instance_members.get(&key)
                        && !program.function(f).abstract_
                    {
                        let owner = program.class(cc);
                        return owner.kind == ClassKind::Class && Some(cc) != object;
                    }
                }
                current = k.supertype_class;
            }
            false
        };
        for d in a.decls.iter() {
            let DeclKind::Class(x) = &d.kind else { continue };
            for &t in x.implements.iter() {
                if let Some(&ty_id) = s.outline.written_types.get(&(s.unit, t))
                    && let Type::Interface { class, .. } = table.get(ty_id)
                    && overrides_equals(*class)
                {
                    report(&c::AVOID_IMPLEMENTING_VALUE_TYPES, a.ty(t).span, &[]);
                }
            }
        }
    }

    // `use_full_hex_values_for_flutter_colors`.
    if enabled("use_full_hex_values_for_flutter_colors") {
        for &(e, f) in &creations {
            let g = program.function(f);
            let Some(cl) = g.class else { continue };
            let k = program.class(cl);
            if interner.resolve(k.name) != "Color" || !interner.resolve(g.name).is_empty() || library_name(s, interner, k.library).as_deref() != Some("dart.ui") {
                continue;
            }
            let Some(args) = arguments_of(e) else { continue };
            let Some(first) = args.args.first() else { continue };
            if first.name.is_some() {
                continue;
            }
            let ExprKind::Int(sp) = &a.expr(first.value).kind else { continue };
            let lexeme = &source[sp.start..sp.end];
            if lexeme.starts_with('-') {
                continue;
            }
            let hex_text: String = lexeme.to_lowercase().chars().filter(|c| *c != '_').collect();
            if !hex_text.starts_with("0x") || hex_text.chars().count() != 10 {
                report(&c::USE_FULL_HEX_VALUES_FOR_FLUTTER_COLORS, a.expr(first.value).span, &[]);
            }
        }
    }

    // `use_test_throws_matchers`.
    if enabled("use_test_throws_matchers") {
        for st in a.stmts.iter() {
            let StmtKind::Try { body, catches, finally_ } = &st.kind else { continue };
            if catches.len() != 1 || finally_.is_some() {
                continue;
            }
            let StmtKind::Block(cmds) = &a.stmt(*body).kind else { continue };
            let Some(&last) = cmds.last() else { continue };
            let StmtKind::Expression(e) = &a.stmt(last).kind else { continue };
            let ExprKind::Call { target, .. } = &a.expr(*e).kind else { continue };
            let is_fail = match s.body.get_resolved(*target) {
                Some(Resolved::Element(Element::Function(f))) => {
                    let g = program.function(*f);
                    interner.resolve(g.name) == "fail" && g.class.is_none() && program.library(g.library).uri == "package:test_api/src/frontend/expect.dart"
                }
                _ => false,
            };
            if is_fail {
                report(&c::USE_TEST_THROWS_MATCHERS, a.stmt(last).span, &[]);
            }
        }
    }

    // `do_not_use_environment`.
    if enabled("do_not_use_environment") {
        for &(e, f) in &creations {
            let g = program.function(f);
            if !g.factory {
                continue;
            }
            let name = interner.resolve(g.name);
            let Some(t) = s.body.get_type(e) else { continue };
            let Type::Interface { class, .. } = table.get(t) else { continue };
            let cl = program.class(*class);
            if program.library(cl.library).uri != "dart:core" {
                continue;
            }
            let class_id = interner.resolve(cl.name);
            let reported = (matches!(class_id, "bool" | "int" | "String") && name == "fromEnvironment") || (class_id == "bool" && name == "hasEnvironment");
            if reported {
                report(&c::DO_NOT_USE_ENVIRONMENT, constructor_name_span(a, e), &[]);
            }
        }
    }

    // `use_if_null_to_convert_nulls_to_bools`.
    if enabled("use_if_null_to_convert_nulls_to_bools") {
        let is_nullable_bool = |t: TypeId| matches!(table.get(t), Type::Interface { class, nullable: true, .. } if Some(*class) == s.core.bool_class);
        for e in a.exprs.iter() {
            let ExprKind::Binary { op, left, right } = &e.kind else { continue };
            let Some(t) = s.body.get_type(*left) else { continue };
            if !is_nullable_bool(t) {
                continue;
            }
            let reported = match (op, &a.expr(*right).kind) {
                (BinaryOp::Eq, ExprKind::Bool(true)) => true,
                (BinaryOp::NotEq, ExprKind::Bool(false)) => true,
                _ => false,
            };
            if reported {
                report(&c::USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS, e.span, &[]);
            }
        }
    }

    // `annotate_redeclares`.
    if enabled("annotate_redeclares") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::ExtensionType(x) = &d.kind else { continue };
            let _ = k;
            // The names of the members of the implemented interfaces (with the
            // inherited ones).
            let mut names: HashSet<dartforge_intern::SymbolId> = HashSet::new();
            for &t in x.implements.iter() {
                let Some(&ty_id) = s.outline.written_types.get(&(s.unit, t)) else { continue };
                let (Type::Interface { class: ic, .. } | Type::ExtensionType { decl: ic, .. }) = table.get(ty_id) else { continue };
                let mut all_classes = vec![*ic];
                if let Some(h) = s.outline.hierarchy.get(*ic) {
                    all_classes.extend(h.supertypes.keys().copied());
                }
                if let Some(o) = s.core.object_class {
                    all_classes.push(o);
                }
                for cc in all_classes {
                    let kc = program.class(cc);
                    for (name, f) in kc.instance_members.iter() {
                        let is_private = interner.resolve(*name).starts_with('_');
                        if !is_private || program.function(*f).library == lib {
                            names.insert(*name);
                        }
                    }
                }
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Method(f) = &m.kind else { continue };
                let f = a.function(*f);
                if f.static_ {
                    continue;
                }
                let Some(n) = f.name else { continue };
                if m.metadata.iter().any(|x| dartforge_types::annotations::is_getter_of(program, interner, s.unit, x, "meta", "redeclare")) {
                    continue;
                }
                let text = interner.resolve(n.sym);
                let key = if f.kind == ast::FunctionKind::Setter { interner.lookup(&format!("{text}_=")) } else { Some(n.sym) };
                if key.is_some_and(|ch| names.contains(&ch)) {
                    report(&c::ANNOTATE_REDECLARES, n.span, &[text]);
                }
            }
        }
    }

    // `avoid_types_on_closure_parameters`.
    if enabled("avoid_types_on_closure_parameters") {
        let parents = super::rules16::unit_parents(u, &|e| matches!(a.expr(e).kind, ExprKind::FunctionExpression(_) | ExprKind::Call { .. } | ExprKind::InstanceCreation { .. }));
        for (k, e) in a.exprs.iter().enumerate() {
            let ExprKind::FunctionExpression(f) = &e.kind else { continue };
            let ctx = super::rules16::approximate_context(s, interner, a, &parents, ExprId(k as u32));
            let super::rules16::ApproxContext::Typed(t) = ctx else { continue };
            if !matches!(table.get(t), Type::Function { .. }) {
                continue;
            }
            let Some(ps) = &a.function(*f).parameters else { continue };
            for p in ps.iter() {
                if p.function_parameters.is_some() {
                    report(&c::AVOID_TYPES_ON_CLOSURE_PARAMETERS, p.span, &[]);
                    continue;
                }
                let Some(ty) = p.ty else { continue };
                if !matches!(a.ty(ty).kind, TypeKind::Named { .. }) {
                    continue;
                }
                let is_dynamic = s.body.annotation_types.get(&ty).is_some_and(|x| matches!(table.get(*x), Type::Dynamic));
                if !is_dynamic {
                    report(&c::AVOID_TYPES_ON_CLOSURE_PARAMETERS, a.ty(ty).span, &[]);
                }
            }
        }
    }

    // `deprecated_consistency`.
    if enabled("deprecated_consistency") {
        for d in a.decls.iter() {
            let (members, type_name) = match &d.kind {
                DeclKind::Class(x) => (&x.members, x.name),
                DeclKind::Enum(x) => (&x.members, x.name),
                DeclKind::Mixin(x) => (&x.members, x.name),
                DeclKind::ExtensionType(x) => (&x.members, x.name),
                _ => continue,
            };
            let deprecated_class = is_deprecated(s, interner, s.unit, &d.metadata);
            for &mid in members {
                let m = a.member(mid);
                let MemberKind::Constructor(k) = &m.kind else { continue };
                if deprecated_class && !is_deprecated(s, interner, s.unit, &m.metadata) {
                    let ctor_span = k.name.unwrap_or(k.class_name).span;
                    report(&c::DEPRECATED_CONSISTENCY_CONSTRUCTOR, ctor_span, &[]);
                }
                for p in k.parameters.iter() {
                    if !p.this_ {
                        continue;
                    }
                    let Some(n) = p.name else { continue };
                    // The declaration's field with the same name.
                    let field = members.iter().find_map(|mm| match &a.member(*mm).kind {
                        MemberKind::Field(l) => l.variables.iter().find(|v| v.name.sym == n.sym).map(|v| (*mm, v.name)),
                        _ => None,
                    });
                    let Some((mc, field_name)) = field else { continue };
                    let deprecated_field = is_deprecated(s, interner, s.unit, &a.member(mc).metadata);
                    let deprecated_param = is_deprecated(s, interner, s.unit, &p.metadata);
                    if deprecated_field && !deprecated_param {
                        report(&c::DEPRECATED_CONSISTENCY_FIELD, p.span, &[]);
                    }
                    if !deprecated_field && deprecated_param {
                        report(&c::DEPRECATED_CONSISTENCY_PARAMETER, field_name.span, &[]);
                    }
                }
            }
            let _ = type_name;
        }
    }

    out
}
