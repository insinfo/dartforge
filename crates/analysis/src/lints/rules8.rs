//! The eighth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_conditional_assignment`, `join_return_with_assignment`,
//! `literal_only_boolean_expressions`,
//! `avoid_unused_constructor_parameters`, `unnecessary_const`,
//! `avoid_init_to_null`, `type_init_formals`,
//! `always_put_control_body_on_new_line` and `directives_ordering`.
//!
//! With the same data as the original (those that need the element or the
//! type only report with the unit's semantics):
//! - `prefer_conditional_assignment` and `join_return_with_assignment`:
//!   `canonicalElementsFromIdentifiersAreEqual` (`super::same_elements`).
//! - `avoid_unused_constructor_parameters`: the use through the parameter's
//!   element, and the resolved `@deprecated`/`@Deprecated(…)`.
//! - `avoid_init_to_null`: the `isNullable` of the element's type (of the
//!   body's local, of the written one, of the field, of the superclass
//!   parameter), and `super.x` only with the superclass parameter's default
//!   `null` or absent.
//! - `type_init_formals`: the resolved type of `this.x` against the field's
//!   and of `super.x` against the superclass parameter's.
//! - `unnecessary_const`: `inConstantContext` (`dartforge_frontend::pais`).
//! - `directives_ordering`: also the `@docImport`s of the `library`
//!   directive's documentation, with the URIs' `stringValue`.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::strings::{literals, string_value};
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, CreationKeyword, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, MemberId, MemberKind,
    StmtId, StmtKind, UnaryOp, VariableList,
};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn is_null(a: &Ast, e: ExprId) -> bool {
    matches!(a.expr(unparenthesized(a, e)).kind, ExprKind::Null)
}

/// `_onlyLiterals`.
fn only_literals(a: &Ast, e: Option<ExprId>) -> bool {
    let Some(e) = e else { return false };
    match &a.expr(unparenthesized(a, e)).kind {
        ExprKind::Int(_)
        | ExprKind::Double(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::String(_)
        | ExprKind::Symbol(_)
        | ExprKind::List { .. }
        | ExprKind::SetOrMap { .. }
        | ExprKind::Record { .. } => true,
        ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::PrefixInc | UnaryOp::PrefixDec, operand } => {
            only_literals(a, Some(*operand))
        }
        ExprKind::Binary { op: BinaryOp::IfNull, left, .. } => only_literals(a, Some(*left)),
        ExprKind::Binary { left, right, .. } => only_literals(a, Some(*left)) && only_literals(a, Some(*right)),
        _ => false,
    }
}

/// The `when` conditions of collections (the `if (x case p when c)` element).
fn element_guards(el: &CollectionElement, out: &mut Vec<ExprId>) {
    match el {
        CollectionElement::If { guard, then, else_, .. } => {
            out.extend(guard.iter().copied());
            element_guards(then, out);
            if let Some(x) = else_ {
                element_guards(x, out);
            }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => element_guards(body, out),
        _ => {}
    }
}

/// The first token from `start` on (a statement's `beginToken`).
fn first_token(source: &str, start: usize) -> Span {
    let b = source.as_bytes();
    let is_word_char = |c: u8| c == b'_' || c == b'$' || c.is_ascii_alphanumeric();
    let mut end = start;
    match b.get(start).copied() {
        Some(c) if is_word_char(c) => {
            while b.get(end).copied().is_some_and(is_word_char) {
                end += 1;
            }
        }
        Some(b'\'' | b'"') => {
            end = literals(source, Span { start: start, end: source.len() }).first().map_or(start + 1, |l| l.span.end);
        }
        Some(c @ (b'+' | b'-')) if b.get(start + 1) == Some(&c) => end = start + 2,
        Some(_) => end = start + source[start..].chars().next().map_or(1, char::len_utf8),
        None => {}
    }
    Span { start: start, end: end }
}

/// The `@docImport`s of the `library` directive's documentation
/// (`directives_ordering`'s `docImportDirectives`): each documentation line
/// (outside a fenced or indented code block) that starts with `@docImport `,
/// with the range of the synthetic `ImportDirective` (from the `import `
/// imagined before the URI to the end of the last token) and the URI's
/// `stringValue`.
fn doc_imports(source: &str, u: Unidade<'_>) -> Vec<(Span, Option<String>)> {
    let mut v = Vec::new();
    let Some(library) = u.unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. })) else { return v };
    let comments = dartforge_frontend::comentarios::Comentarios::de(source);
    let after = match library.metadata.last() {
        Some(m) => dartforge_frontend::fonte::pular_brancos(source.as_bytes(), m.span.end),
        None => library.span.start,
    };
    let doc = comments
        .dart_doc(source, after)
        .or_else(|| library.metadata.iter().rev().find_map(|m| comments.dart_doc(source, m.span.start)));
    let Some(doc) = doc else { return v };
    // The documentation lines: the run of `///` from `doc`, or the lines of
    // the `/** … */`; (content position, content).
    let mut lines: Vec<(usize, &str)> = Vec::new();
    if source[doc.start..doc.end].starts_with("///") {
        for c in comments.antes_de(source, after).into_iter().filter(|c| c.start >= doc.start) {
            let text = &source[c.start..c.end];
            let Some(content) = text.strip_prefix("///") else { break };
            lines.push((c.start + 3, content));
        }
    } else {
        let body = &source[doc.start + 3..doc.end.saturating_sub(2).max(doc.start + 3)];
        let mut base = doc.start + 3;
        for line in body.split('\n') {
            let trimmed = line.trim_start();
            let (offset, content) = match trimmed.strip_prefix('*') {
                Some(r) => (line.len() - r.len(), r),
                None => (line.len() - trimmed.len(), trimmed),
            };
            lines.push((base + offset, content.trim_end_matches('\r')));
            base += line.len() + 1;
        }
    }
    let mut fenced = false;
    let mut previous_blank = true;
    for (pos, content) in lines {
        let indent = content.len() - content.trim_start().len();
        let rest = content.trim_start();
        if rest.starts_with("```") {
            fenced = !fenced;
            previous_blank = false;
            continue;
        }
        if fenced || (previous_blank && indent >= 4) {
            previous_blank = content.trim().is_empty();
            continue;
        }
        previous_blank = content.trim().is_empty();
        let Some(after_tag) = rest.strip_prefix("@docImport ") else { continue };
        let uri_rel = after_tag.len() - after_tag.trim_start().len();
        let uri_start = pos + indent + "@docImport ".len() + uri_rel;
        let text = after_tag.trim_start();
        // The end: the `;`, or the end of the text without whitespace.
        let end_rel = text.find(';').map(|k| k + 1).unwrap_or_else(|| text.trim_end().len());
        let uri = literals(source, Span { start: uri_start, end: uri_start + end_rel }).first().map(|l| l.span);
        let value = uri.and_then(|s| string_value(source, s));
        v.push((Span { start: uri_start.saturating_sub("import ".len()), end: uri_start + end_rel }, value));
    }
    v
}

/// `compareDirectives`: by package (up to the first `/`) and then by the rest.
fn compare_directives(x: &str, y: &str) -> std::cmp::Ordering {
    if (!x.starts_with("package:") || !y.starts_with("package:")) && !x.starts_with('/') && !y.starts_with('/') {
        return x.cmp(y);
    }
    let (Some(i), Some(j)) = (x.find('/'), y.find('/')) else { return x.cmp(y) };
    x[..i].cmp(&y[..j]).then_with(|| x[i + 1..].cmp(&y[j + 1..]))
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // The (0-based) line number of a place.
    // `LineInfo`: `\n`, `\r\n` and a lone `\r` end a line.
    let bs = source.as_bytes();
    let mut line_starts: Vec<usize> = vec![0];
    line_starts.extend(
        bs.iter()
            .enumerate()
            .filter(|(i, b)| **b == b'\n' || (**b == b'\r' && bs.get(i + 1) != Some(&b'\n')))
            .map(|(i, _)| i + 1),
    );
    let line_of = |place: usize| line_starts.partition_point(|&x| x <= place);

    // `prefer_conditional_assignment`: `if (x == null) x = v;`, with the left
    // side and the tested one having the same canonical elements (needs the
    // unit's semantics).
    if enabled("prefer_conditional_assignment")
        && let Some(s) = sem
    {
        fn assigns(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, st: StmtId, tested: ExprId) -> bool {
            match &a.stmt(st).kind {
                StmtKind::Expression(e) => {
                    matches!(&a.expr(*e).kind, ExprKind::Assign { target, .. } if super::same_elements(s, interner, a, Some(*target), Some(tested)))
                }
                StmtKind::Block(ss) if ss.len() == 1 => assigns(s, interner, a, ss[0], tested),
                _ => false,
            }
        }
        for st in a.stmts.iter() {
            let StmtKind::If { condition, then, else_: None, .. } = &st.kind else { continue };
            let ExprKind::Binary { op: BinaryOp::Eq, left, right } = &a.expr(unparenthesized(a, *condition)).kind else { continue };
            let tested = if is_null(a, *right) {
                *left
            } else if is_null(a, *left) {
                *right
            } else {
                continue;
            };
            if assigns(s, interner, a, *then, tested) {
                report(&c::PREFER_CONDITIONAL_ASSIGNMENT, st.span, &[]);
            }
        }
    }
    // `join_return_with_assignment`: `x = v; return x;` at the end of a
    // block, by canonical elements (needs the unit's semantics).
    if enabled("join_return_with_assignment")
        && let Some(s) = sem
    {
        // The target of the assignment, or the operand of an `x++`/`x!`/`-x`,
        // of an expression statement.
        let assigned_target = |st: StmtId| -> Option<ExprId> {
            let StmtKind::Expression(e) = &a.stmt(st).kind else { return None };
            match &a.expr(unparenthesized(a, *e)).kind {
                ExprKind::Assign { target, .. } => Some(*target),
                ExprKind::Unary { operand, .. } => Some(*operand),
                _ => None,
            }
        };
        for st in a.stmts.iter() {
            let StmtKind::Block(ss) = &st.kind else { continue };
            let n = ss.len();
            if n < 2 {
                continue;
            }
            let StmtKind::Return(Some(returned)) = &a.stmt(ss[n - 1]).kind else { continue };
            let Some(second_to_last) = assigned_target(ss[n - 2]) else { continue };
            let third_to_last = if n >= 3 { assigned_target(ss[n - 3]) } else { None };
            if !super::same_elements(s, interner, a, Some(second_to_last), third_to_last)
                && super::same_elements(s, interner, a, Some(*returned), Some(second_to_last))
            {
                report(&c::JOIN_RETURN_WITH_ASSIGNMENT, a.stmt(ss[n - 2]).span, &[]);
            }
        }
    }
    // `literal_only_boolean_expressions`: `do`, `for`, `if`, `while` (except
    // `while (true)`) and the `when` clauses.
    if enabled("literal_only_boolean_expressions") {
        let mut guards: Vec<ExprId> = Vec::new();
        for s in a.stmts.iter() {
            let reported = match &s.kind {
                StmtKind::DoWhile { condition, .. } => only_literals(a, Some(*condition)),
                StmtKind::For { condition, .. } => only_literals(a, *condition),
                StmtKind::If { condition, case_pattern, guard, .. } => {
                    guards.extend(guard.iter().copied());
                    case_pattern.is_none() && only_literals(a, Some(*condition))
                }
                StmtKind::While { condition, .. } => !matches!(a.expr(*condition).kind, ExprKind::Bool(true)) && only_literals(a, Some(*condition)),
                StmtKind::Switch { cases, .. } => {
                    guards.extend(cases.iter().filter_map(|k| k.guard));
                    false
                }
                _ => false,
            };
            if reported {
                report(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, s.span, &[]);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Switch { cases, .. } => guards.extend(cases.iter().filter_map(|k| k.guard)),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                    for el in elements.iter() {
                        element_guards(el, &mut guards);
                    }
                }
                _ => {}
            }
        }
        for g in guards {
            if only_literals(a, Some(g)) {
                let s = a.expr(g).span;
                let start = source.get(..s.start).and_then(|t| t.rfind("when")).unwrap_or(s.start);
                report(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, Span { start: start, end: s.end }, &[]);
            }
        }
    }


    // `type_init_formals`: the written type of `this.x` equal to the field's,
    // and that of `super.x` equal to the superclass constructor parameter's
    // (the resolved types; needs the unit's semantics).
    if enabled("type_init_formals")
        && let Some(s) = sem
    {
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for (pi, p) in k.parameters.iter().enumerate() {
                let Some(t) = p.ty else { continue };
                let Some(written) = super::written_type(s, t) else { continue };
                let target = if p.this_ {
                    p.name.and_then(|n| super::class_field(s, MemberId(mi as u32), n.sym)).and_then(|v| super::variable_type(s, v))
                } else if p.super_ {
                    super::super_parameter(s, interner, MemberId(mi as u32), pi)
                        .and_then(|(f, j)| s.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.get(j)).map(|x| x.ty))
                } else {
                    continue;
                };
                if target.is_some_and(|x| s.table.canonico(x) == s.table.canonico(written)) {
                    report(&c::TYPE_INIT_FORMALS, a.ty(t).span, &[]);
                }
            }
        }
    }
    // `avoid_init_to_null`: a non-`final`/`const` variable and a parameter
    // with a `null` default whose element's type is nullable (`isNullable`);
    // `super.x` only when the superclass parameter's default is `null` or
    // missing. Needs the unit's semantics.
    if enabled("avoid_init_to_null")
        && let Some(s) = sem
    {
        // The elements of the unit's top-level variables and fields.
        let mut top_level: HashMap<(usize, usize), dartforge_elements::model::VariableId> = HashMap::new();
        let mut fields: HashMap<(usize, usize), dartforge_elements::model::VariableId> = HashMap::new();
        for (i, v) in s.program.variables.iter().enumerate() {
            let id = dartforge_elements::model::VariableId(i as u32);
            match v.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } if unit == s.unit => {
                    top_level.insert((decl.0 as usize, index), id);
                }
                dartforge_elements::model::VariableRef::Field { unit, member, index } if unit == s.unit => {
                    fields.insert((member.0 as usize, index), id);
                }
                _ => {}
            }
        }
        let mut findings: Vec<Span> = Vec::new();
        let from_list = |l: &VariableList, kind: &dyn Fn(usize, ast::Name) -> Option<dartforge_types::table::TypeId>, findings: &mut Vec<Span>| {
            if l.final_ || l.const_ {
                return;
            }
            for (k, v) in l.variables.iter().enumerate() {
                if let Some(i) = v.initializer
                    && is_null(a, i)
                    && kind(k, v.name).is_some_and(|t| super::is_nullable(s, t))
                {
                    findings.push(Span { start: v.name.span.start, end: a.expr(i).span.end });
                }
            }
        };
        for (di, d) in a.decls.iter().enumerate() {
            if let DeclKind::Variables(l) = &d.kind {
                from_list(l, &|k, _| top_level.get(&(di, k)).and_then(|v| super::variable_type(s, *v)), &mut findings);
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            if let MemberKind::Field(l) = &m.kind {
                from_list(l, &|k, _| fields.get(&(mi, k)).and_then(|v| super::variable_type(s, *v)), &mut findings);
            }
        }
        let local = |_: usize, n: ast::Name| s.body.tipo_local(n.span.start);
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => from_list(l, &local, &mut findings),
                _ => {}
            }
        }
        // The parameters with a `null` default: the element's type (that of
        // the body's local; without a body, the written one, the field's or
        // the superclass parameter's).
        let param_type = |mi: Option<usize>, pi: usize, p: &ast::Parameter| -> Option<dartforge_types::table::TypeId> {
            let n = p.name?;
            if let Some(t) = s.body.tipo_local(n.span.start) {
                return Some(t);
            }
            if let Some(t) = p.ty {
                return super::written_type(s, t);
            }
            if p.this_ {
                return super::class_field(s, MemberId(mi? as u32), n.sym).and_then(|v| super::variable_type(s, v));
            }
            if p.super_ {
                return super::super_parameter(s, interner, MemberId(mi? as u32), pi)
                    .and_then(|(f, j)| s.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.get(j)).map(|x| x.ty));
            }
            if p.function_parameters.is_some() {
                // The old form without a return: a function type, nullable
                // only with `?`.
                return Some(if p.function_nullable { s.core.dynamic_ } else { s.core.function });
            }
            Some(s.core.dynamic_)
        };
        // The superclass parameter's default is `null` (or missing).
        let super_default_is_null = |mi: usize, pi: usize| -> bool {
            let Some((f, j)) = super::super_parameter(s, interner, MemberId(mi as u32), pi) else { return false };
            let dartforge_elements::model::FunctionRef::Constructor { unit, member } = s.program.function(f).node else { return true };
            let au = &s.program.unit(unit).ast;
            let MemberKind::Constructor(kk) = &au.member(member).kind else { return true };
            match kk.parameters.get(j).and_then(|x| x.default_value) {
                None => true,
                Some(d) => {
                    let sp = au.expr(d).span;
                    s.program.unit(unit).source.get(sp.start..sp.end) == Some("null")
                }
            }
        };
        fn collect_lists<'x>(ps: &'x [ast::Parameter], out: &mut Vec<&'x [ast::Parameter]>) {
            out.push(ps);
            for p in ps {
                if let Some(inner) = &p.function_parameters {
                    collect_lists(inner, out);
                }
            }
        }
        let mut ownerless: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                collect_lists(ps, &mut ownerless);
            }
        }
        for list in ownerless {
            for (pi, p) in list.iter().enumerate() {
                if p.default_value.is_some_and(|d| is_null(a, d)) && param_type(None, pi, p).is_some_and(|t| super::is_nullable(s, t)) {
                    findings.push(p.span);
                }
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for (pi, p) in k.parameters.iter().enumerate() {
                if let Some(inner) = &p.function_parameters {
                    let mut ls = Vec::new();
                    collect_lists(inner, &mut ls);
                    for list in ls {
                        for (qi, q) in list.iter().enumerate() {
                            if q.default_value.is_some_and(|d| is_null(a, d)) && param_type(None, qi, q).is_some_and(|t| super::is_nullable(s, t)) {
                                findings.push(q.span);
                            }
                        }
                    }
                }
                if !p.default_value.is_some_and(|d| is_null(a, d)) {
                    continue;
                }
                if p.super_ && !super_default_is_null(mi, pi) {
                    continue;
                }
                if param_type(Some(mi), pi, p).is_some_and(|t| super::is_nullable(s, t)) {
                    findings.push(p.span);
                }
            }
        }
        findings.sort_by_key(|x| (x.start, x.end));
        findings.dedup();
        for x in findings {
            report(&c::AVOID_INIT_TO_NULL, x, &[]);
        }
    }
    // `avoid_unused_constructor_parameters`: the parameter (not `this.x`,
    // `super.x`, deprecated or only underscores) that no identifier of the
    // body and the initializers reads (by element; needs the unit's
    // semantics). Excluded: augmentation, redirecting factory, `external`.
    if enabled("avoid_unused_constructor_parameters")
        && let Some(s) = sem
    {
        let primaries: HashSet<MemberId> = a
            .decls
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Class(x) => x.primary_constructor,
                DeclKind::Enum(x) => x.primary_constructor,
                _ => None,
            })
            .collect();
        for (i, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            if m.augment || k.redirect.is_some() || k.external || k.parte_primaria || primaries.contains(&MemberId(i as u32)) {
                continue;
            }
            // The visited regions: the body and each initializer.
            let mut regions: Vec<Span> = k
                .initializers
                .iter()
                .map(|x| match x {
                    ast::Initializer::Field { span, .. }
                    | ast::Initializer::Super { span, .. }
                    | ast::Initializer::Redirect { span, .. }
                    | ast::Initializer::Assert { span, .. } => *span,
                })
                .collect();
            match &k.body {
                ast::FunctionBody::Block(b) => regions.push(a.stmt(*b).span),
                ast::FunctionBody::Expression(e) => regions.push(a.expr(*e).span),
                _ => {}
            }
            let used: HashSet<usize> = a
                .exprs
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e.kind, ExprKind::Identifier(_)) && regions.iter().any(|r| r.start <= e.span.start && e.span.end <= r.end))
                .filter_map(|(k, _)| s.body.declaracao_local(ExprId(k as u32)))
                .collect();
            for p in k.parameters.iter().filter(|p| !p.this_ && !p.super_) {
                let Some(n) = p.name else { continue };
                let text = interner.resolve(n.sym);
                let deprecated = p.metadata.iter().any(|x| dartforge_types::anotacoes::e_deprecated(s.program, interner, s.unit, x));
                if deprecated || text.bytes().all(|b| b == b'_') || used.contains(&n.span.start) {
                    continue;
                }
                report(&c::AVOID_UNUSED_CONSTRUCTOR_PARAMETERS, p.span, &[text]);
            }
        }
    }
    // `always_put_control_body_on_new_line`.
    if enabled("always_put_control_body_on_new_line") {
        // The body does not start on the line where the control ends.
        let mut check_body = |body: StmtId, control_end: usize| {
            let n = a.stmt(body);
            let first = match &n.kind {
                StmtKind::Block(ss) => match ss.first() {
                    Some(&x) => a.stmt(x).span.start,
                    None => return,
                },
                _ => n.span.start,
            };
            if line_of(control_end) == line_of(first) {
                report(&c::ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE, first_token(source, n.span.start), &[]);
            }
        };
        // The end of the `)` that closes the header, before the body.
        let closing_paren = |start: usize, body: StmtId| -> Option<usize> {
            let until = a.stmt(body).span.start;
            source.get(start..until).and_then(|t| t.rfind(')')).map(|i| start + i + 1)
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::DoWhile { body, .. } => check_body(*body, s.span.start + 2),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } => {
                    if let Some(f) = closing_paren(s.span.start, *body) {
                        check_body(*body, f);
                    }
                }
                StmtKind::If { then, else_, .. } => {
                    if let Some(f) = closing_paren(s.span.start, *then) {
                        check_body(*then, f);
                    }
                    if let Some(else_stmt) = else_
                        && !matches!(a.stmt(*else_stmt).kind, StmtKind::If { .. })
                        && let Some(word) = source.get(a.stmt(*then).span.end..a.stmt(*else_stmt).span.start).and_then(|t| t.rfind("else"))
                    {
                        check_body(*else_stmt, a.stmt(*then).span.end + word + 4);
                    }
                }
                _ => {}
            }
        }
    }
    // `directives_ordering`: the imports, the exports and the `@docImport`s
    // of the `library` directive's documentation, with the URIs' `stringValue`.
    if enabled("directives_ordering") {
        #[derive(Clone, Copy, PartialEq)]
        enum Kind {
            Import,
            Export,
            DocImport,
        }
        // (node range, kind, `stringValue`).
        let mut directives: Vec<(Span, Kind, Option<String>)> = Vec::new();
        for d in u.unit.directives.iter() {
            let (uri, kind) = match &d.kind {
                DirectiveKind::Import { uri, .. } => (uri, Kind::Import),
                DirectiveKind::Export { uri, .. } => (uri, Kind::Export),
                _ => continue,
            };
            directives.push((d.span, kind, string_value(source, uri.span)));
        }
        for (span, uri) in doc_imports(source, u) {
            directives.push((span, Kind::DocImport, uri));
        }
        let mut reported_spans: HashSet<(usize, usize)> = HashSet::new();
        let mut report_once = |span: Span, code: &'static CodigoLint, args: &[&str]| {
            if reported_spans.insert((span.start, span.end)) {
                report(code, span, args);
            }
        };
        let uri_starts_with = |x: &(Span, Kind, Option<String>), p: &str| x.2.as_deref().is_some_and(|u| u.starts_with(p));
        let is_dart = |x: &&(Span, Kind, Option<String>)| uri_starts_with(x, "dart:");
        let is_package = |x: &&(Span, Kind, Option<String>)| uri_starts_with(x, "package:");
        let is_absolute = |x: &&(Span, Kind, Option<String>)| x.2.as_deref().is_some_and(|u| u.contains(':'));
        let is_relative = |x: &&(Span, Kind, Option<String>)| x.2.as_deref().is_some_and(|u| !u.contains(':'));
        let words = [(Kind::Import, "import"), (Kind::Export, "export"), (Kind::DocImport, "@docImport")];
        // `_checkDartDirectiveGoFirst`.
        for (kind, word) in words {
            for x in directives.iter().filter(|x| x.1 == kind).skip_while(is_dart).filter(is_dart) {
                report_once(x.0, &c::DIRECTIVES_ORDERING_DART, &[word]);
            }
        }
        // `_checkPackageDirectiveBeforeRelative`.
        for (kind, word) in words {
            for x in directives.iter().filter(|x| x.1 == kind && !is_dart(x)).skip_while(is_absolute).filter(is_package) {
                report_once(x.0, &c::DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE, &[word]);
            }
        }
        // `_checkExportDirectiveAfterImportDirective`.
        let trailing_exports: Vec<usize> = (0..u.unit.directives.len())
            .rev()
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Part { .. }))
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .filter(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .collect();
        for i in trailing_exports {
            report_once(u.unit.directives[i].span, &c::DIRECTIVES_ORDERING_EXPORTS, &[]);
        }
        // `_checkDirectiveSectionOrderedAlphabetically`: each section in
        // order, comparing with the section's previous URI.
        let sections: [&dyn Fn(&&(Span, Kind, Option<String>)) -> bool; 3] = [&is_dart, &is_relative, &is_package];
        for section in sections {
            for kind in [Kind::Import, Kind::Export, Kind::DocImport] {
                let in_section: Vec<&(Span, Kind, Option<String>)> = directives.iter().filter(|x| x.1 == kind).filter(|x| section(x)).collect();
                for pair in in_section.windows(2) {
                    if let (Some(previous), Some(current)) = (&pair[0].2, &pair[1].2)
                        && compare_directives(previous, current) == std::cmp::Ordering::Greater
                    {
                        report_once(pair[1].0, &c::DIRECTIVES_ORDERING_ALPHABETICAL, &[]);
                    }
                }
            }
        }
    }
    // `unnecessary_const`: the `const` written on a creation, collection or
    // record in a constant context (`inConstantContext`); a collection that
    // is directly the expression of a constant pattern is excluded.
    if enabled("unnecessary_const") {
        let old_version = sem.is_some_and(|s| !super::version_at_least(s, 3, 0));
        let parents = dartforge_frontend::pais::Pais::novo(a, u.unit, source, old_version);
        for (k, n) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let candidate = match &n.kind {
                ExprKind::InstanceCreation { keyword: Some(CreationKeyword::Const), .. } | ExprKind::Record { const_: true, .. } => true,
                ExprKind::List { const_: true, .. } | ExprKind::SetOrMap { const_: true, .. } => {
                    !matches!(parents.pai(id), dartforge_frontend::pais::Pai::PadraoConstante { .. })
                }
                _ => false,
            };
            if candidate && source.get(n.span.start..).is_some_and(|t| t.starts_with("const")) && parents.em_contexto_constante(a, id) {
                report(&c::UNNECESSARY_CONST, Span { start: n.span.start, end: n.span.start + 5 }, &[]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(rule: &str, source: &str) -> Vec<String> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        let mut reports = run(u, &names, &|r| r == rule, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| source[r.span.start..r.span.end].to_string()).collect()
    }

    #[test]
    fn assignments() {
        // The canonical elements need the unit's semantics.
        assert!(only("prefer_conditional_assignment", "void f(int? x) {\n  if (x == null) x = 1;\n}\n").is_empty());
        assert!(only("join_return_with_assignment", "int f(int x) {\n  x = 1;\n  return x;\n}\n").is_empty());
    }

    #[test]
    fn literals_in_conditions() {
        let source = "void f(int x) {\n  if (true) {}\n  while (true) {}\n  while (1 > 2) {}\n  for (; 1 == 1;) {}\n  if (x > 0) {}\n  switch (x) {\n    case 1 when true:\n      break;\n  }\n}\n";
        assert_eq!(
            only("literal_only_boolean_expressions", source),
            vec!["if (true) {}".to_string(), "while (1 > 2) {}".to_string(), "for (; 1 == 1;) {}".to_string(), "when true".to_string()]
        );
    }

    #[test]
    fn constructors() {
        let source = "class A {\n  final int? a;\n  int b;\n  A(int? this.a, this.b, int c, int d, int _) : assert(d > 0);\n  A.n(int e) : a = null, b = 0;\n}\n";
        // The resolved types and the uses by element need the semantics.
        assert!(only("type_init_formals", source).is_empty());
        assert!(only("avoid_unused_constructor_parameters", source).is_empty());
    }

    #[test]
    fn null_initializers() {
        let source = "int? a = null;\nvar b = null;\nfinal int? c = null;\nint d = 0;\nclass A {\n  int? e;\n  A([this.e = null, int? f = null, int g = 0]);\n}\n";
        // The element's type needs the unit's semantics.
        assert!(only("avoid_init_to_null", source).is_empty());
    }

    #[test]
    fn unnecessary_const() {
        let source = "class A {\n  const A([Object? x]);\n}\nconst a = const A();\nconst b = [const A(), const [1]];\nfinal c = const A(const A());\nfinal d = const A();\n@A(const A())\nvoid f([x = const A()]) {}\n";
        assert_eq!(only("unnecessary_const", source).len(), 5);
    }

    #[test]
    fn body_on_same_line() {
        let source = "void f(bool x) {\n  if (x) return;\n  if (x) {\n    return;\n  } else return;\n  while (x) { f(x); }\n  do f(x); while (x);\n  if (x)\n    return;\n}\n";
        assert_eq!(
            only("always_put_control_body_on_new_line", source),
            vec!["return".to_string(), "return".to_string(), "{".to_string(), "f".to_string()]
        );
    }

    #[test]
    fn directive_order() {
        assert_eq!(compare_directives("package:a/z.dart", "package:b/a.dart"), std::cmp::Ordering::Less);
        assert_eq!(compare_directives("b.dart", "a.dart"), std::cmp::Ordering::Greater);
        let source = "import 'package:b/b.dart';\nimport 'dart:io';\nimport 'package:a/a.dart';\nexport 'x.dart';\nimport 'z.dart';\nimport 'y.dart';\n";
        assert_eq!(
            only("directives_ordering", source),
            vec![
                "import 'dart:io';".to_string(),
                "import 'package:a/a.dart';".to_string(),
                "export 'x.dart';".to_string(),
                "import 'y.dart';".to_string(),
            ]
        );
    }
}
