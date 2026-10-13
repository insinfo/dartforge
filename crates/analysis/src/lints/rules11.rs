//! The eleventh batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `no_adjacent_strings_in_list`, `missing_whitespace_between_adjacent_strings`,
//! `flutter_style_todos`, `unnecessary_library_name`,
//! `prefer_null_aware_method_calls`, `avoid_print`,
//! `avoid_catches_without_on_clauses` and `avoid_setters_without_getters`.
//!
//! With the same data as the original (those that need the element only
//! report with the unit's semantics):
//! - `avoid_print`: `dart:core`'s `print` and `package:flutter`'s
//!   `kDebugMode` by element; the tear-off in positional arguments.
//! - `avoid_catches_without_on_clauses`: the `_ValidUseVisitor` in text order
//!   (a `rethrow` before an inner `catch`, the `throw`, a `Never` call,
//!   `Future.error`, `FlutterError.reportError`, a `Completer`'s
//!   `completeError`), the exception by element.
//! - `avoid_setters_without_getters`: `lookUpGetter` and
//!   `lookUpInheritedConcreteSetter` along the implementation chain; without
//!   the semantics, only a declaration without a superclass or mixin.
//! - `missing_whitespace_between_adjacent_strings`: the decoded values; the
//!   exemption only for the direct positional argument of `RegExp` (by the
//!   constructor's class) and of `matches`.
//! - `no_adjacent_strings_in_list`: the map by static type, and the list
//!   patterns of a `switch` (with patterns).
//! - `unnecessary_library_name`: only from 2.19 on.
//! - `prefer_null_aware_method_calls`: the `toSource` of both sides.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::strings;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionKind, MemberKind, StmtKind,
    UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};

fn within(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// The expression without the outer parentheses.
fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// Adjacent strings (`'a' 'b'`): a `String` written with more than one literal.
fn is_adjacent_strings(a: &Ast, source: &str, e: ExprId) -> bool {
    let e = a.expr(e);
    matches!(e.kind, ExprKind::String(_)) && strings::literals(source, e.span).len() > 1
}

/// `flutter_style_todos`: `//+\s*TODO\b` (case-insensitive) that does not
/// start with `// TODO\([a-zA-Z0-9][-a-zA-Z0-9\.]*\): `.
fn todo_out_of_style(text: &str) -> bool {
    let b = text.as_bytes();
    let slashes = b.iter().take_while(|&&x| x == b'/').count();
    if slashes < 2 {
        return false;
    }
    let mut i = slashes;
    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    if b.len() < i + 4 || !b[i..i + 4].eq_ignore_ascii_case(b"todo") {
        return false;
    }
    if b.get(i + 4).is_some_and(|&x| x == b'_' || x.is_ascii_alphanumeric()) {
        return false;
    }
    // The expected format.
    let Some(rest) = text.strip_prefix("// TODO(") else { return true };
    let r = rest.as_bytes();
    if r.first().is_none_or(|x| !x.is_ascii_alphanumeric()) {
        return true;
    }
    let mut k = 1;
    while k < r.len() && (r[k].is_ascii_alphanumeric() || r[k] == b'-' || r[k] == b'.') {
        k += 1;
    }
    !rest[k..].starts_with("): ")
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `no_adjacent_strings_in_list`.
    if enabled("no_adjacent_strings_in_list") {
        fn collect_elements(a: &Ast, source: &str, el: &CollectionElement, direct: bool, out: &mut Vec<Span>) {
            match el {
                CollectionElement::Expression(e) if direct && is_adjacent_strings(a, source, *e) => out.push(a.expr(*e).span),
                // `visitForElement`: the adjacent body.
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    if let CollectionElement::Expression(e) = body.as_ref()
                        && is_adjacent_strings(a, source, *e)
                    {
                        out.push(a.expr(*e).span);
                    }
                    collect_elements(a, source, body, false, out);
                }
                // `visitIfElement`: without `else`, the adjacent `then`; with
                // `else`, only the `else`.
                CollectionElement::If { then, else_, .. } => {
                    match else_ {
                        None => {
                            if let CollectionElement::Expression(e) = then.as_ref()
                                && is_adjacent_strings(a, source, *e)
                            {
                                out.push(a.expr(*e).span);
                            }
                        }
                        Some(x) => {
                            if let CollectionElement::Expression(e) = x.as_ref()
                                && is_adjacent_strings(a, source, *e)
                            {
                                out.push(a.expr(*e).span);
                            }
                        }
                    }
                    collect_elements(a, source, then, false, out);
                    if let Some(x) = else_ {
                        collect_elements(a, source, x, false, out);
                    }
                }
                _ => {}
            }
        }
        let mut findings = Vec::new();
        for (k, e) in a.exprs.iter().enumerate() {
            match &e.kind {
                ExprKind::List { elements, .. } => {
                    for el in elements.iter() {
                        collect_elements(a, source, el, true, &mut findings);
                    }
                }
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    // `isMap`: by the static type, with the semantics; without
                    // it, by the type arguments and the elements.
                    let by_type = sem.and_then(|s| {
                        let t = s.body.get_type(ExprId(k as u32))?;
                        match s.table.get(t) {
                            dartforge_types::table::Type::Interface { class, .. } => Some(Some(*class) == s.core.map_class),
                            _ => None,
                        }
                    });
                    let is_map = by_type.unwrap_or_else(|| {
                        type_args.len() == 2
                            || (type_args.is_empty()
                                && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))))
                    });
                    for el in elements.iter() {
                        collect_elements(a, source, el, !is_map, &mut findings);
                    }
                }
                _ => {}
            }
        }
        // `visitSwitchPatternCase` (with patterns, 3.0 or later): the constant
        // elements of a list pattern.
        if sem.is_some_and(|s| super::version_at_least(s, 3, 0)) {
            for st in a.stmts.iter() {
                let StmtKind::Switch { cases, .. } = &st.kind else { continue };
                for k in cases.iter() {
                    let Some(mut pattern) = k.pattern else { continue };
                    while let ast::PatternKind::Parenthesized(x) = &a.pattern(pattern).kind {
                        pattern = *x;
                    }
                    let ast::PatternKind::List { elements, .. } = &a.pattern(pattern).kind else { continue };
                    for el in elements.iter() {
                        if let ast::ListPatternElement::Pattern(x) = el
                            && let ast::PatternKind::Constant(e) = &a.pattern(*x).kind
                        {
                            let e = unparenthesized(a, *e);
                            if is_adjacent_strings(a, source, e) {
                                findings.push(a.expr(e).span);
                            }
                        }
                    }
                }
            }
        }
        findings.sort_by_key(|s| (s.start, s.end));
        findings.dedup();
        for s in findings {
            report(&c::NO_ADJACENT_STRINGS_IN_LIST, s, &[]);
        }
    }

    // `missing_whitespace_between_adjacent_strings`.
    if enabled("missing_whitespace_between_adjacent_strings") {
        // The arguments of `RegExp(…)` and of a target-less `matches(…)`.
        let mut exempt: Vec<ExprId> = Vec::new();
        for (k, e) in a.exprs.iter().enumerate() {
            let args = match &e.kind {
                ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                    ExprKind::Identifier(n) if matches!(interner.resolve(n.sym), "RegExp" | "matches") => Some(arguments),
                    _ => None,
                },
                // The creation: the constructor's class is called `RegExp` (by
                // element, with the semantics; without it, by the written name).
                ExprKind::InstanceCreation { ty, arguments, .. } => {
                    let by_element = sem.and_then(|s| {
                        match s.body.get_resolved(ExprId(k as u32))? {
                            dartforge_types::resolved::Resolved::Constructor(f) => {
                                s.program.function(*f).class.map(|c| interner.resolve(s.program.class(c).name) == "RegExp")
                            }
                            _ => None,
                        }
                    });
                    let text = source.get(a.ty(*ty).span.start..a.ty(*ty).span.end).unwrap_or("");
                    by_element.unwrap_or(text == "RegExp" || text.ends_with(".RegExp")).then_some(arguments)
                }
                _ => None,
            };
            if let Some(args) = args {
                // `parent is ArgumentList`: a named argument has the
                // `NamedExpression` in between.
                exempt.extend(args.args.iter().filter(|x| x.name.is_none()).map(|x| x.value));
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            if !matches!(e.kind, ExprKind::String(_)) || exempt.contains(&ExprId(k as u32)) {
                continue;
            }
            let lits = strings::literals(source, e.span);
            for pair in lits.windows(2) {
                let (current, next) = (&pair[0], &pair[1]);
                // The values (with the escapes decoded) of the current one's
                // last part, of the next one's first and of the whole current one.
                let value_of = |l: &strings::Literal, t: Option<&Span>| -> String {
                    let text = t.and_then(|x| source.get(x.start..x.end)).unwrap_or("");
                    if l.raw { text.to_string() } else { strings::decode(text) }
                };
                let end = value_of(current, current.parts.last());
                let start = value_of(next, next.parts.first());
                let blanks = [' ', '\n', '\r', '\t'];
                let ends_blank = (current.is_interpolated() && end.is_empty()) || end.ends_with(blanks);
                let starts_blank = (next.is_interpolated() && start.is_empty()) || start.starts_with(blanks);
                if ends_blank || starts_blank {
                    continue;
                }
                let all_text = current.parts.iter().map(|t| value_of(current, Some(t))).collect::<String>();
                if !all_text.contains(blanks) {
                    continue;
                }
                report(&c::MISSING_WHITESPACE_BETWEEN_ADJACENT_STRINGS, current.span, &[]);
            }
        }
    }

    // `flutter_style_todos`.
    if enabled("flutter_style_todos") {
        for s in strings::comments(source) {
            if todo_out_of_style(source.get(s.start..s.end).unwrap_or("")) {
                report(&c::FLUTTER_STYLE_TODOS, s, &[]);
            }
        }
    }

    // `unnecessary_library_name`.
    if enabled("unnecessary_library_name") && sem.is_some_and(|s| super::version_at_least(s, 2, 19)) {
        for d in &u.unit.directives {
            if let DirectiveKind::Library { name } = &d.kind
                && let (Some(p), Some(f)) = (name.first(), name.last())
            {
                report(&c::UNNECESSARY_LIBRARY_NAME, Span { start: p.span.start, end: f.span.end }, &[]);
            }
        }
    }

    // `prefer_null_aware_method_calls`.
    if enabled("prefer_null_aware_method_calls") {
        // `x != null`: the left operand.
        let compared_to_null = |cond: ExprId| -> Option<ExprId> {
            match &a.expr(cond).kind {
                ExprKind::Binary { op: BinaryOp::NotEq, left, right } if matches!(a.expr(*right).kind, ExprKind::Null) => Some(*left),
                _ => None,
            }
        };
        // `x!()` with the same `x`.
        let invocation = |e: ExprId, left: ExprId| -> Option<Span> {
            let ExprKind::Call { target, .. } = &a.expr(e).kind else { return None };
            let ExprKind::Unary { op: UnaryOp::NullAssert, operand } = &a.expr(*target).kind else { return None };
            // `toSource()` of both.
            let text = |x: ExprId| dartforge_frontend::source::from_expr(a, source, interner, x);
            (text(*operand) == text(left)).then_some(a.expr(e).span)
        };
        let mut findings = Vec::new();
        for e in a.exprs.iter() {
            if let ExprKind::Conditional { condition, then, else_ } = &e.kind
                && matches!(a.expr(*else_).kind, ExprKind::Null)
                && let Some(left_operand) = compared_to_null(*condition)
                && let Some(s) = invocation(*then, left_operand)
            {
                findings.push(s);
            }
        }
        for s in a.stmts.iter() {
            let StmtKind::If { condition, then, else_: None, .. } = &s.kind else { continue };
            let Some(left_operand) = compared_to_null(*condition) else { continue };
            let mut body = *then;
            if let StmtKind::Block(b) = &a.stmt(body).kind {
                let [single] = &b[..] else { continue };
                body = *single;
            }
            if let StmtKind::Expression(x) = &a.stmt(body).kind
                && let Some(sp) = invocation(*x, left_operand)
            {
                findings.push(sp);
            }
        }
        findings.sort_by_key(|s| (s.start, s.end));
        for s in findings {
            report(&c::PREFER_NULL_AWARE_METHOD_CALLS, s, &[]);
        }
    }

    // `avoid_print`: the invocation whose `methodName` resolves to
    // `dart:core`'s `print` function, outside the `then` branch of an
    // `if (kDebugMode)` (from `package:flutter/src/foundation/constants.dart`)
    // in the same function body; and the positional argument that is the
    // identifier `print` (the tear-off) of any method invocation. Needs the
    // unit's semantics.
    if enabled("avoid_print")
        && let Some(s) = sem
    {
        use dartforge_elements::model::Element;
        use dartforge_types::resolved::Resolved;
        let p = s.program;
        let is_print = |e: ExprId| match s.body.get_resolved(e) {
            Some(Resolved::Element(Element::Function(f))) => {
                let g = p.function(*f);
                g.class.is_none() && g.extension.is_none() && interner.resolve(g.name) == "print" && p.library(g.library).uri == "dart:core"
            }
            _ => false,
        };
        let is_kdebug = |e: ExprId| {
            let is_library = |l: dartforge_elements::model::LibraryId| p.library(l).uri == "package:flutter/src/foundation/constants.dart";
            match s.body.get_resolved(e) {
                Some(Resolved::Element(Element::Variable(v))) => interner.resolve(p.variable(*v).name) == "kDebugMode" && is_library(p.variable(*v).library),
                Some(Resolved::Element(Element::Function(f))) => interner.resolve(p.function(*f).name) == "kDebugMode" && is_library(p.function(*f).library),
                _ => false,
            }
        };
        // The `then` branches of `if (kDebugMode)`.
        let debug_branches: Vec<Span> = a
            .stmts
            .iter()
            .filter_map(|st| match &st.kind {
                StmtKind::If { condition, then, .. } if matches!(a.expr(*condition).kind, ExprKind::Identifier(_)) && is_kdebug(*condition) => {
                    Some(a.stmt(*then).span)
                }
                _ => None,
            })
            .collect();
        // No function body between the branch and the call.
        let only_in_debug = |x: Span| {
            debug_branches.iter().any(|&r| {
                within(x, r)
                    && !a.functions.iter().any(|f| {
                        let body = match &f.body {
                            FunctionBody::Block(b) => a.stmt(*b).span,
                            FunctionBody::Expression(e) => a.expr(*e).span,
                            _ => return false,
                        };
                        within(x, body) && within(body, r)
                    })
            })
        };
        for (k, e) in a.exprs.iter().enumerate() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            // `MethodInvocation`: the target is the name (with or without a
            // prefix), and not an instance creation.
            if matches!(s.body.get_resolved(ExprId(k as u32)), Some(Resolved::Constructor(_))) {
                continue;
            }
            let name = match &a.expr(*target).kind {
                ExprKind::Identifier(n) => *n,
                ExprKind::Property { name, .. } => *name,
                _ => continue,
            };
            if is_print(*target) && !only_in_debug(e.span) {
                report(&c::AVOID_PRINT, name.span, &[]);
            }
            for arg in arguments.args.iter().filter(|x| x.name.is_none()) {
                if let ExprKind::Identifier(n) = &a.expr(arg.value).kind
                    && is_print(arg.value)
                {
                    report(&c::AVOID_PRINT, n.span, &[]);
                }
            }
        }
    }

    // `avoid_catches_without_on_clauses`: a `catch` without `on`, with the
    // exception parameter, whose body makes no valid use of it (the
    // `_ValidUseVisitor`, in text order): a `rethrow` counts while no inner
    // `catch` has been visited; a `throw` that mentions the exception; a call
    // of type `Never`, `Future.error(…)`, `FlutterError.reportError(…)` and a
    // `Completer`'s `completeError(…)` that mention it in the arguments (an
    // expression invocation of type `Never` is not visited inside). Needs the
    // unit's semantics.
    if enabled("avoid_catches_without_on_clauses")
        && let Some(s) = sem
    {
        use dartforge_elements::model::{ClassKind, Element};
        use dartforge_types::resolved::Resolved;
        use dartforge_types::table::Type;
        let p = s.program;
        let class_named = |name: &str, uri: &str| {
            p.classes.iter().position(|c| interner.resolve(c.name) == name && p.library(c.library).uri == uri).map(|i| dartforge_elements::model::ClassId(i as u32))
        };
        let (future, completer) = (class_named("Future", "dart:async"), class_named("Completer", "dart:async"));
        let is_never = |e: ExprId| s.body.get_type(e).is_some_and(|t| matches!(s.table.get(t), Type::Never));
        // `extendsClass('Completer', 'dart.async')`: the class or a
        // superclass.
        let extends_completer = |e: ExprId| {
            let Some(Type::Interface { class, .. }) = s.body.get_type(e).map(|t| s.table.get(t)) else { return false };
            let mut current = Some(*class);
            let mut steps = 0;
            while let Some(k) = current {
                if Some(k) == completer {
                    return true;
                }
                steps += 1;
                if steps > 64 {
                    break;
                }
                current = p.class(k).supertype_class;
            }
            false
        };
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for cc in catches.iter() {
                if cc.on_type.is_some() {
                    continue;
                }
                let Some(ex) = cc.exception else { continue };
                let body = a.stmt(cc.body).span;
                let mentions = |region: Span| {
                    a.exprs.iter().enumerate().any(|(j, e)| {
                        matches!(e.kind, ExprKind::Identifier(_)) && within(e.span, region) && s.body.local_declaration(ExprId(j as u32)) == Some(ex.span.start)
                    })
                };
                // The visitor's events, by position: (position, order,
                // event). 0: an inner `catch`; 1: `rethrow`; 2: a valid
                // use.
                let mut events: Vec<(usize, u8)> = Vec::new();
                let mut outside: Vec<Span> = Vec::new();
                for t in a.stmts.iter() {
                    if let StmtKind::Try { catches: cs, .. } = &t.kind {
                        for k in cs.iter().filter(|k| within(k.span, body)) {
                            events.push((k.span.start, 0));
                        }
                    }
                }
                for (j, e) in a.exprs.iter().enumerate().filter(|(_, e)| within(e.span, body)) {
                    let id = ExprId(j as u32);
                    match &e.kind {
                        ExprKind::Rethrow => events.push((e.span.start, 1)),
                        ExprKind::Throw(x) => {
                            if mentions(a.expr(*x).span) {
                                events.push((e.span.start, 2));
                            }
                        }
                        ExprKind::InstanceCreation { constructor: Some(name), arguments, .. } => {
                            let is_future = matches!(s.body.get_type(id).map(|t| s.table.get(t)), Some(Type::Interface { class, .. }) if Some(*class) == future);
                            if interner.resolve(name.sym) == "error" && is_future && mentions(arguments.span) {
                                events.push((e.span.start, 2));
                            }
                        }
                        ExprKind::Call { target, arguments } => {
                            let target_expr = a.expr(*target);
                            // The creation without `new`: `Future.error(…)`.
                            if let Some(Resolved::Constructor(f)) = s.body.get_resolved(id) {
                                let g = p.function(*f);
                                let is_future = matches!(s.body.get_type(id).map(|t| s.table.get(t)), Some(Type::Interface { class, .. }) if Some(*class) == future);
                                if interner.resolve(g.name) == "error" && is_future && mentions(arguments.span) {
                                    events.push((e.span.start, 2));
                                }
                                continue;
                            }
                            match &target_expr.kind {
                                // `MethodInvocation`.
                                ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                                    let (name, receiver) = match &target_expr.kind {
                                        ExprKind::Property { target: r, name, .. } => (interner.resolve(name.sym), Some(*r)),
                                        ExprKind::Identifier(n) => (interner.resolve(n.sym), None),
                                        _ => unreachable!(),
                                    };
                                    let delivers = if is_never(id) {
                                        true
                                    } else if name == "reportError" {
                                        receiver.is_some_and(|r| {
                                            matches!(a.expr(r).kind, ExprKind::Identifier(_))
                                                && matches!(s.body.get_resolved(r), Some(Resolved::Element(Element::Class(c)))
                                                    if p.class(*c).kind == ClassKind::Class && interner.resolve(p.class(*c).name) == "FlutterError")
                                        })
                                    } else if name == "completeError" {
                                        receiver.is_some_and(extends_completer)
                                    } else {
                                        false
                                    };
                                    if delivers && mentions(arguments.span) {
                                        events.push((e.span.start, 2));
                                    }
                                }
                                // A `FunctionExpressionInvocation` of type `Never`.
                                _ => {
                                    if is_never(id) {
                                        if mentions(arguments.span) {
                                            events.push((e.span.start, 2));
                                        }
                                        outside.push(Span { start: e.span.start + 1, end: e.span.end });
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                events.retain(|(x, _)| !outside.iter().any(|f| f.start <= *x && *x < f.end));
                events.sort();
                let mut can_rethrow = true;
                let mut valid = false;
                for (_, ev) in events {
                    match ev {
                        0 => can_rethrow = false,
                        1 => valid = can_rethrow,
                        _ => valid = true,
                    }
                }
                if !valid {
                    report(&c::AVOID_CATCHES_WITHOUT_ON_CLAUSES, Span { start: cc.span.start, end: cc.span.start + "catch".len() }, &[]);
                }
            }
        }
    }

    // `avoid_setters_without_getters`: the setter of a class, enum or
    // extension type without an inherited concrete setter
    // (`lookUpInheritedConcreteSetter`) and without a getter along the
    // implementation chain (`lookUpGetter`: the class, the mixins from last
    // to first, the superclass…). Without the semantics, only a declaration
    // without a superclass or mixin.
    if enabled("avoid_setters_without_getters") {
        for d in a.decls.iter() {
            let (members, no_superclass, is_enum, representation): (&[_], bool, bool, Option<SymbolId>) = match &d.kind {
                DeclKind::Class(x) => (x.members.as_slice(), x.extends.is_none() && x.with.is_empty(), false, None),
                DeclKind::Enum(x) => (x.members.as_slice(), x.with.is_empty(), true, None),
                DeclKind::ExtensionType(x) => (x.members.as_slice(), true, false, Some(x.representation_name.sym)),
                _ => continue,
            };
            let class_id = sem.and_then(|s| members.first().and_then(|&m| super::member_class(s, m)).map(|c| (s, c)));
            if class_id.is_none() && !no_superclass {
                continue;
            }
            for &m in members {
                let MemberKind::Method(f) = &a.member(m).kind else { continue };
                let f = a.function(*f);
                if f.kind != FunctionKind::Setter {
                    continue;
                }
                let Some(n) = f.name else { continue };
                let has_getter = match class_id {
                    Some((s, c)) => super::lookup_in_chain(s, interner, c, n.sym, false),
                    None => {
                        Some(n.sym) == representation
                            || matches!(interner.resolve(n.sym), "hashCode" | "runtimeType")
                            || (is_enum && interner.resolve(n.sym) == "index")
                            || members.iter().any(|&mm| match &a.member(mm).kind {
                                MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == n.sym),
                                MemberKind::Method(g) => {
                                    let g = a.function(*g);
                                    g.kind == FunctionKind::Getter && g.name.is_some_and(|x| x.sym == n.sym)
                                }
                                MemberKind::Constructor(_) => false,
                            })
                    }
                };
                let has_inherited_setter = class_id.is_some_and(|(s, c)| super::lookup_in_chain(s, interner, c, n.sym, true));
                if !has_getter && !has_inherited_setter {
                    report(&c::AVOID_SETTERS_WITHOUT_GETTERS, n.span, &[]);
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flutter_style_todo() {
        assert!(todo_out_of_style("// TODO: x"));
        assert!(todo_out_of_style("//todo(a): x"));
        assert!(todo_out_of_style("/// TODO(a): x"));
        assert!(!todo_out_of_style("// TODO(user1): do it"));
        assert!(!todo_out_of_style("// TODOS all cases"));
        assert!(!todo_out_of_style("// nothing"));
        assert!(!todo_out_of_style("/* TODO */"));
    }
}
