//! The twentieth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `document_ignores`: the `// ignore:`/`// ignore_for_file:` comment (the
//!   `ignoreComments`) with elements and no free text after them
//!   (`IgnoredDiagnosticComment`), without a `//` comment on the line above.
//! * `sort_child_properties_last`: the `child:`/`children:` of a widget with
//!   named arguments that are not closures after it.
//! * `test_types_in_equals`: `other as T` in `operator ==` with the parameter.
//! * `matching_super_parameters`: the positional `super.x`s with a name
//!   different from the superclass constructor's parameter.
//! * `unnecessary_null_aware_operator_on_extension_on_nullable`: `?.`/`?[`
//!   that calls an extension member on a nullable type.
//! * `avoid_slow_async_io`: the async methods of `dart:io`'s `File`,
//!   `Directory` and `FileSystemEntity`.
//! * `tighten_type_of_initializing_formals`: the `assert(x != null)` of a
//!   nullable `this.x`/`super.x`.
//!
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionRef, LibraryId};
use dartforge_frontend::ast::{BinaryOp, DeclKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};

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

/// `isSameAs`.
fn is_same_class(s: &super::Semantics<'_>, interner: &Interner, c: ClassId, name: &str, lib: &str) -> bool {
    let k = s.program.class(c);
    interner.resolve(k.name) == name && library_name(s, interner, k.library).as_deref() == Some(lib)
}

/// `extendsClass`: the superclass chain.
fn extends_class(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, name: &str, lib: &str) -> bool {
    let Type::Interface { class, .. } = s.table.get(t) else { return false };
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(*class);
    while let Some(x) = current {
        if !seen.insert(x) {
            return false;
        }
        if is_same_class(s, interner, x, name, lib) {
            return true;
        }
        current = s.program.class(x).supertype_class;
    }
    false
}

/// `isWidgetProperty`.
pub(super) fn is_widget_property(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    if super::flutter::is_widget_type(s, interner, t) {
        return true;
    }
    let mut x = t;
    if let Type::TypeParameter { param, .. } | Type::Intersection { param, .. } = s.table.get(x) {
        x = s.table.param(*param).bound;
    }
    let Type::Interface { class, args, .. } = s.table.get(x) else { return false };
    let collections = [("List", "dart.core"), ("Map", "dart.core"), ("LinkedHashMap", "dart.collection"), ("Set", "dart.core"), ("LinkedHashSet", "dart.collection")];
    let is_collection = |c: ClassId| collections.iter().any(|(n, l)| is_same_class(s, interner, c, n, l));
    let implements = is_collection(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| is_collection(*k))));
    if !implements {
        return false;
    }
    let params = s.outline.classes.get(class.0 as usize).map(|d| d.type_params.len()).unwrap_or(0);
    params == 1 && args.first().is_some_and(|a| is_widget_property(s, interner, *a, depth + 1))
}

/// The elements of an `ignore` comment (`ignoredElements`): `(there are
/// elements, the last one is free text)`.
fn ignore_elements(lexeme: &str) -> (bool, bool) {
    let b = lexeme.as_bytes();
    let n = b.len();
    let Some(dp) = lexeme.find(':') else { return (false, false) };
    let mut i = dp + 1;
    let is_blank = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r');
    let is_space = |c: u8| matches!(c, b' ' | b'\t');
    let word = |i: &mut usize| {
        if *i < n && b[*i].is_ascii_alphabetic() {
            *i += 1;
            while *i < n && (b[*i].is_ascii_alphanumeric() || b[*i] == b'_') {
                *i += 1;
            }
        }
    };
    let mut has = false;
    loop {
        while i < n && is_blank(b[i]) {
            i += 1;
        }
        if i == n {
            return (has, false);
        }
        let start = i;
        word(&mut i);
        if start == i {
            return (has, has);
        }
        if lexeme[start..i].eq_ignore_ascii_case("type") {
            while i < n && is_blank(b[i]) {
                i += 1;
            }
            if i == n || b[i] != b'=' {
                return (has, false);
            }
            i += 1;
            while i < n && is_blank(b[i]) {
                i += 1;
            }
            if i == n {
                return (has, false);
            }
            let text_start = i;
            word(&mut i);
            if text_start == i {
                return (has, has);
            }
            if i < n && !is_space(b[i]) && b[i] != b',' {
                return (has, has);
            }
            has = true;
        } else {
            if i < n && !is_space(b[i]) && b[i] != b',' {
                return (has, has);
            }
            has = true;
        }
        if i == n {
            return (has, false);
        }
        while i < n && is_blank(b[i]) {
            i += 1;
        }
        if i == n {
            return (has, false);
        }
        if b[i] != b',' {
            return (has, true);
        }
        i += 1;
        if i == n {
            return (has, false);
        }
    }
}

/// The `?.` or `?` token after `target_end`.
fn token_null_aware(source: &str, target_end: usize, len: usize) -> Span {
    let i = dartforge_frontend::source::skip_whitespace(source.as_bytes(), target_end);
    Span { start: i, end: (i + len).min(source.len()) }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.source;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `document_ignores`.
    if enabled("document_ignores") {
        let comments = dartforge_frontend::comments::Comments::scan(source);
        let mut line_starts = vec![0usize];
        let bs = source.as_bytes();
        let mut i = 0;
        while i < bs.len() {
            match bs[i] {
                b'\r' => {
                    if bs.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    line_starts.push(i + 1);
                }
                b'\n' => line_starts.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        for sp in comments.all() {
            let lexeme = &source[sp.start..sp.end];
            // `//+[ ]*ignore:` or `//[ ]*ignore_for_file:` at the start.
            let after = lexeme.trim_start_matches('/');
            let slashes = lexeme.len() - after.len();
            let body = after.trim_start_matches(' ');
            let is_ignore = slashes >= 2 && body.starts_with("ignore:");
            let is_file_ignore = slashes == 2 && body.starts_with("ignore_for_file:");
            if !is_ignore && !is_file_ignore {
                continue;
            }
            let (has, last_is_free_text) = ignore_elements(lexeme);
            if !has || last_is_free_text {
                continue;
            }
            let line = line_starts.partition_point(|&x| x <= sp.start);
            if line > 1 {
                let previous = line_starts[line - 2];
                let mut o = previous;
                while o < bs.len() && matches!(bs[o], b' ' | b'\t') {
                    o += 1;
                }
                if o + 1 < bs.len() && bs[o] == b'/' && bs[o + 1] == b'/' {
                    continue;
                }
            }
            report(&c::DOCUMENT_IGNORES, *sp, &[]);
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;
    let table = s.table;

    // `sort_child_properties_last`.
    if enabled("sort_child_properties_last") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !matches!(s.body.get_resolved(id), Some(Resolved::Constructor(_))) {
                continue;
            }
            if !s.body.get_type(id).is_some_and(|t| super::flutter::is_widget_type(s, interner, t)) {
                continue;
            }
            let args = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            let is_child = |x: &dartforge_frontend::ast::Argument| {
                x.name.is_some_and(|n| matches!(interner.resolve(n.sym), "child" | "children"))
                    && s.body.get_type(x.value).is_some_and(|t| is_widget_property(s, interner, t, 0))
            };
            let n = args.args.len();
            if n < 2 || is_child(&args.args[n - 1]) || args.args.iter().filter(|x| is_child(x)).count() != 1 {
                continue;
            }
            let after: Vec<&dartforge_frontend::ast::Argument> = args.args.iter().rev().take_while(|x| !is_child(x)).collect();
            let only_closures = after.iter().all(|x| x.name.is_none() || matches!(a.expr(x.value).kind, ExprKind::FunctionExpression(_)));
            if !only_closures {
                let child = args.args.iter().find(|x| is_child(x)).expect("a child");
                let name = child.name.expect("named");
                report(&c::SORT_CHILD_PROPERTIES_LAST, Span { start: name.span.start, end: a.expr(child.value).span.end }, &[interner.resolve(name.sym)]);
            }
        }
    }

    // `test_types_in_equals`.
    if enabled("test_types_in_equals") {
        // The one-parameter `operator ==`s, with the declaration's type name.
        let mut equals_ops: Vec<(Span, dartforge_intern::SymbolId, String)> = Vec::new();
        for d in a.decls.iter() {
            let (members, type_name) = match &d.kind {
                DeclKind::Class(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Enum(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Mixin(x) => (&x.members, interner.resolve(x.name.sym).to_string()),
                DeclKind::Extension(x) => (&x.members, dartforge_frontend::source::from_type(a, source, interner, x.on)),
                DeclKind::ExtensionType(x) => (&x.members, {
                    let _ = x;
                    "unknown".to_string()
                }),
                _ => continue,
            };
            for &mid in members {
                let MemberKind::Method(f) = &a.member(mid).kind else { continue };
                let f = a.function(*f);
                if f.kind != dartforge_frontend::ast::FunctionKind::Operator || f.name.is_none_or(|n| interner.resolve(n.sym) != "==") {
                    continue;
                }
                let Some(ps) = &f.parameters else { continue };
                let [p] = &ps[..] else { continue };
                let Some(pn) = p.name else { continue };
                equals_ops.push((f.span, pn.sym, type_name.clone()));
            }
        }
        for e in a.exprs.iter() {
            let ExprKind::As { value, .. } = &e.kind else { continue };
            let ExprKind::Identifier(n) = &a.expr(*value).kind else { continue };
            // The nearest method that contains the expression.
            let method = a
                .members
                .iter()
                .filter_map(|m| match &m.kind {
                    MemberKind::Method(f) => Some(a.function(*f).span),
                    _ => None,
                })
                .filter(|sp| sp.start <= e.span.start && e.span.end <= sp.end)
                .min_by_key(|sp| sp.end - sp.start);
            let Some(method) = method else { continue };
            if let Some((_, p, name)) = equals_ops.iter().find(|(sp, _, _)| *sp == method)
                && *p == n.sym
            {
                report(&c::TEST_TYPES_IN_EQUALS, e.span, &[name.as_str()]);
            }
        }
    }

    // `matching_super_parameters`.
    if enabled("matching_super_parameters") {
        for (k, d) in a.decls.iter().enumerate() {
            let (members, is_class) = match &d.kind {
                DeclKind::Class(x) => (&x.members, true),
                DeclKind::Enum(x) => (&x.members, false),
                DeclKind::ExtensionType(x) => (&x.members, false),
                _ => continue,
            };
            let class_id = (0..program.classes.len())
                .map(|i| ClassId(i as u32))
                .find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl.0 as usize == k));
            for &mid in members {
                let MemberKind::Constructor(kc) = &a.member(mid).kind else { continue };
                let positional: Vec<&dartforge_frontend::ast::Parameter> = kc.parameters.iter().filter(|p| p.super_ && p.kind != ParameterKind::Named).collect();
                if positional.is_empty() {
                    continue;
                }
                let super_call = kc.initializers.iter().find_map(|i| match i {
                    Initializer::Super { constructor, .. } => Some(*constructor),
                    _ => None,
                });
                let Some(sup) = class_id.and_then(|c| program.class(c).supertype_class) else { continue };
                let constructor = match super_call {
                    Some(name) => name.map(|n| n.sym).or_else(|| interner.lookup("")).and_then(|ch| program.class(sup).constructors.get(&ch).copied()),
                    None if is_class => interner.lookup("").and_then(|ch| program.class(sup).constructors.get(&ch).copied()),
                    None => None,
                };
                let Some(fsup) = constructor else { continue };
                let FunctionRef::Constructor { unit, member } = program.function(fsup).node else { continue };
                let MemberKind::Constructor(ks) = &program.unit(unit).ast.member(member).kind else { continue };
                let super_names: Vec<String> = ks.parameters.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| interner.resolve(n.sym).to_string())).collect();
                if super_names.len() < positional.len() {
                    continue;
                }
                for (p, super_name) in positional.iter().zip(super_names.iter()) {
                    let Some(n) = p.name else { continue };
                    let name = interner.resolve(n.sym);
                    if name != super_name {
                        report(&c::MATCHING_SUPER_PARAMETERS, p.span, &[name, super_name.as_str()]);
                    }
                }
            }
        }
    }

    // `unnecessary_null_aware_operator_on_extension_on_nullable`.
    if enabled("unnecessary_null_aware_operator_on_extension_on_nullable") {
        let is_nullable_extension = |e: ExprId| -> bool {
            match s.body.get_resolved(e) {
                Some(Resolved::ExtensionMember { extension, .. }) => s.outline.extensions.get(extension.0 as usize).is_some_and(|x| is_nullable(table, x.on)),
                _ => false,
            }
        };
        let mut assignment_targets: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
        for (k, e) in a.exprs.iter().enumerate() {
            if let ExprKind::Assign { target, .. } = &e.kind {
                let mut t = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(t).kind {
                    t = *x;
                }
                assignment_targets.insert(t, ExprId(k as u32));
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            match &e.kind {
                ExprKind::Index { target, null_aware: true, .. } => {
                    let element = assignment_targets.get(&id).copied().unwrap_or(id);
                    if is_nullable_extension(element) || is_nullable_extension(id) {
                        report(&c::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, token_null_aware(source, a.expr(*target).span.end, 1), &[]);
                    }
                }
                ExprKind::Property { target, null_aware: true, .. } => {
                    let element = assignment_targets.get(&id).copied().unwrap_or(id);
                    if is_nullable_extension(element) || is_nullable_extension(id) {
                        report(&c::UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE, token_null_aware(source, a.expr(*target).span.end, 2), &[]);
                    }
                }
                _ => {}
            }
        }
    }

    // `avoid_slow_async_io`.
    if enabled("avoid_slow_async_io") {
        for e in a.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind else { continue };
            let method = interner.resolve(name.sym);
            if arguments.args.is_empty() {
                let Some(t) = s.body.get_type(*recv) else { continue };
                let is_file = extends_class(s, interner, t, "File", "dart.io") && matches!(method, "lastModified" | "exists" | "stat");
                let is_directory = extends_class(s, interner, t, "Directory", "dart.io") && matches!(method, "exists" | "stat");
                if is_file {
                    report(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
                }
                if is_directory {
                    report(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
                }
            } else if matches!(a.expr(*recv).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                && let Some(Resolved::Element(Element::Class(cl))) = s.body.get_resolved(*recv)
                && interner.resolve(program.class(*cl).name) == "FileSystemEntity"
                && matches!(method, "isDirectory" | "isFile" | "isLink" | "type")
            {
                report(&c::AVOID_SLOW_ASYNC_IO, e.span, &[]);
            }
        }
    }

    // `tighten_type_of_initializing_formals`.
    if enabled("tighten_type_of_initializing_formals") {
        for m in a.members.iter() {
            let MemberKind::Constructor(kc) = &m.kind else { continue };
            for i in kc.initializers.iter() {
                let Initializer::Assert { condition, .. } = i else { continue };
                let ExprKind::Binary { op: BinaryOp::NotEq, left, right } = &a.expr(*condition).kind else { continue };
                let ident = if matches!(a.expr(*right).kind, ExprKind::Null) {
                    Some(*left)
                } else if matches!(a.expr(*left).kind, ExprKind::Null) {
                    Some(*right)
                } else {
                    None
                };
                let Some(x) = ident else { continue };
                let ExprKind::Identifier(n) = &a.expr(x).kind else { continue };
                if !s.body.get_type(x).is_some_and(|t| is_nullable(table, t)) {
                    continue;
                }
                if !matches!(s.body.get_resolved(x), Some(Resolved::Parameter { .. })) {
                    continue;
                }
                if let Some(p) = kc.parameters.iter().find(|p| p.name.is_some_and(|q| q.sym == n.sym))
                    && (p.this_ || p.super_)
                {
                    report(&c::TIGHTEN_TYPE_OF_INITIALIZING_FORMALS, p.span, &[]);
                }
            }
        }
    }

    out
}
