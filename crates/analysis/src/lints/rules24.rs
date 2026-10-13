//! The twenty-fourth batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`),
//! through the unit's semantics:
//!
//! * `specify_nonobvious_local_variable_types`: the untyped local variables
//!   (also those of `for` and `for-in`, of pattern declarations and of
//!   `case`) whose initializer has no obvious type (`super::obvious`).
//! * `use_string_buffers`: `s += …` (of a variable that is not a local of the
//!   body) and `s = s + …` of a `String` directly in a loop body. As in the
//!   analyzer, the identifier on the left side of an assignment has no
//!   element, so the second case only matches an unresolved identifier.
//! * `use_enums`: a class that is a hand-written enum.
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{
    AssignOp, Ast, BinaryOp, DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, PatternId, PatternKind, StmtId, StmtKind, StringPart,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::Type;
use std::collections::HashSet;

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
}

/// The declared variable patterns without a type (or `dynamic`/`Null`) inside
/// `p`.
fn untyped_patterns(s: &super::Semantics<'_>, a: &Ast, p: PatternId, out: &mut Vec<Span>) {
    let mut vs = Vec::new();
    super::rules9::pattern_variables(a, p, &mut vs);
    for x in vs {
        let PatternKind::Variable { ty, .. } = &a.pattern(x).kind else { continue };
        let untyped = match ty {
            None => true,
            Some(t) => s.body.annotation_types.get(t).is_none_or(|t| matches!(s.table.get(*t), Type::Dynamic | Type::Null)),
        };
        if untyped {
            out.push(a.pattern(x).span);
        }
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `specify_nonobvious_local_variable_types`.
    if enabled("specify_nonobvious_local_variable_types") {
        let mut findings: Vec<Span> = Vec::new();
        let is_obvious = |e: ExprId| super::obvious::has_obvious_type(s, interner, a, e);
        let list = |l: &dartforge_frontend::ast::VariableList, start: usize, findings: &mut Vec<Span>| {
            if let Some(t) = l.ty
                && s.body.annotation_types.get(&t).is_some_and(|x| !matches!(table.get(*x), Type::Null))
            {
                return;
            }
            let needing: Vec<&dartforge_frontend::ast::Variable> = l.variables.iter().filter(|v| v.initializer.is_none_or(|i| !is_obvious(i))).collect();
            if needing.is_empty() {
                return;
            }
            if l.variables.len() == 1 {
                let v = &l.variables[0];
                let end = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                findings.push(Span { start, end });
            } else {
                for v in needing {
                    let end = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                    findings.push(Span { start: v.name.span.start, end });
                }
            }
        };
        let metadata_of = |k: usize| a.local_metadata.iter().find(|(x, _)| x.0 as usize == k).map_or(&[][..], |(_, m)| &m[..]);
        let list_start = |stmt_start: usize, meta: &[dartforge_frontend::ast::Annotation]| match meta.last() {
            Some(m) => dartforge_frontend::source::skip_whitespace(u.fonte.as_bytes(), m.span.end),
            None => stmt_start,
        };
        for (k, st) in a.stmts.iter().enumerate() {
            match &st.kind {
                StmtKind::Variables(l) => list(l, list_start(st.span.start, metadata_of(k)), &mut findings),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                    // The list starts after the `(`.
                    let start_pos = u.fonte[st.span.start..].find('(').map_or(st.span.start, |i| dartforge_frontend::source::skip_whitespace(u.fonte.as_bytes(), st.span.start + i + 1));
                    list(l, start_pos, &mut findings);
                }
                StmtKind::ForIn { target: ForInTarget::Declared { ty, name, .. }, iterable, .. } => {
                    let typed = ty.and_then(|t| s.body.annotation_types.get(&t)).is_some_and(|x| !matches!(table.get(*x), Type::Dynamic));
                    if typed || is_obvious(*iterable) {
                        continue;
                    }
                    let start_pos = u.fonte[st.span.start..].find('(').map_or(name.span.start, |i| dartforge_frontend::source::skip_whitespace(u.fonte.as_bytes(), st.span.start + i + 1));
                    findings.push(Span { start: start_pos, end: name.span.end });
                }
                StmtKind::PatternVariables { pattern, value, .. } => {
                    if !is_obvious(*value) {
                        untyped_patterns(s, a, *pattern, &mut findings);
                    }
                }
                StmtKind::Switch { value, cases } => {
                    if is_obvious(*value) {
                        continue;
                    }
                    for case in cases.iter() {
                        let Some(p) = case.pattern else { continue };
                        // The declaration patterns of the whole case (the pattern
                        // and the body's pattern declarations).
                        untyped_patterns(s, a, p, &mut findings);
                        for (k2, st2) in a.stmts.iter().enumerate() {
                            let _ = k2;
                            if st2.span.start >= case.span.start
                                && st2.span.end <= case.span.end
                                && let StmtKind::PatternVariables { pattern, .. } = &st2.kind
                            {
                                untyped_patterns(s, a, *pattern, &mut findings);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        findings.sort_by_key(|x| (x.start, x.end));
        findings.dedup();
        for sp in findings {
            report(&c::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES, sp, &[]);
        }
    }

    // `use_string_buffers`.
    if enabled("use_string_buffers") {
        let is_string = |t: dartforge_types::table::TypeId| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.string_class);
        // The loop bodies (`do`, `for`, `for-in`, `while`).
        let bodies: Vec<StmtId> = a
            .stmts
            .iter()
            .filter_map(|st| match &st.kind {
                StmtKind::DoWhile { body, .. } | StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } => Some(*body),
                _ => None,
            })
            .collect();
        let mut findings: Vec<Span> = Vec::new();
        for body in bodies {
            // The visited statements: those of the block (in order) or the only one.
            let stmts: Vec<StmtId> = match &a.stmt(body).kind {
                StmtKind::Block(cmds) => cmds.to_vec(),
                _ => vec![body],
            };
            let mut locals: HashSet<usize> = HashSet::new();
            for st in stmts {
                match &a.stmt(st).kind {
                    StmtKind::Variables(l) => {
                        for v in l.variables.iter() {
                            locals.insert(v.name.span.start);
                        }
                    }
                    StmtKind::Expression(e) => {
                        let mut x = *e;
                        while let ExprKind::Parenthesized(y) = &a.expr(x).kind {
                            x = *y;
                        }
                        let ExprKind::Assign { op, target, value } = &a.expr(x).kind else { continue };
                        if !matches!(op, AssignOp::Assign | AssignOp::Compound(BinaryOp::Add)) {
                            continue;
                        }
                        if !matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                            continue;
                        }
                        let Some(&write_type) = s.body.write_types.get(&x) else { continue };
                        if !is_string(write_type) {
                            continue;
                        }
                        if matches!(op, AssignOp::Compound(_)) {
                            // Is the written element a local of the body?
                            let local = match s.body.get_resolved(*target) {
                                Some(Resolved::Local(_)) => s.body.local_declaration(*target).is_some_and(|d| locals.contains(&d)),
                                _ => false,
                            };
                            if !local {
                                findings.push(a.expr(x).span);
                            }
                        } else {
                            // `_IdentifierIsPrefixVisitor`: the element of the left
                            // side is null.
                            let mut y = *value;
                            loop {
                                match &a.expr(y).kind {
                                    ExprKind::Binary { op: BinaryOp::Add, left, .. } => y = *left,
                                    ExprKind::Parenthesized(z) => y = *z,
                                    ExprKind::String(l) if l.parts.len() >= 2 => match (&l.parts[0], &l.parts[1]) {
                                        (StringPart::Text(t), StringPart::Interpolation(z)) if t.code_units().next().is_none() => y = *z,
                                        _ => break,
                                    },
                                    ExprKind::Identifier(_) => {
                                        if s.body.get_resolved(y).is_none() {
                                            findings.push(a.expr(*target).span);
                                        }
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        findings.sort_by_key(|x| x.start);
        for sp in findings {
            report(&c::USE_STRING_BUFFERS, sp, &[]);
        }
    }

    // `use_enums` (with `enhanced-enums`, 2.17).
    if enabled("use_enums") && program.library(program.unit(s.unit).library).features.version() >= dartforge_frontend::features::LanguageVersion::new(2, 17) {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            if d.augment || x.modifiers.abstract_ || x.mixin_application {
                continue;
            }
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            if program.class(class_id).supertype_class != s.core.object_class {
                continue;
            }
            let public = !interner.resolve(x.name.sym).starts_with('_');
            let mut candidates: Vec<ExprId> = Vec::new();
            let mut invalid = false;
            for &mid in &x.members {
                let m = a.member(mid);
                let named = |n: &str| match &m.kind {
                    MemberKind::Method(f) => a.function(*f).name.is_some_and(|q| interner.resolve(q.sym) == n),
                    MemberKind::Field(l) => l.variables.iter().any(|v| interner.resolve(v.name.sym) == n),
                    MemberKind::Constructor(_) => false,
                };
                if named("hashCode") || named("index") || named("==") || named("values") {
                    invalid = true;
                    break;
                }
                match &m.kind {
                    MemberKind::Field(l) if l.static_ => {
                        for v in l.variables.iter() {
                            if !l.const_ {
                                continue;
                            }
                            let Some(i) = v.initializer else { continue };
                            let Some(Resolved::Constructor(f)) = s.body.get_resolved(i) else { continue };
                            if !matches!(a.expr(i).kind, ExprKind::InstanceCreation { .. } | ExprKind::Call { .. }) {
                                continue;
                            }
                            let g = program.function(*f);
                            if g.factory || g.class != Some(class_id) {
                                continue;
                            }
                            candidates.push(i);
                        }
                    }
                    MemberKind::Constructor(kc) => {
                        if !kc.factory && !kc.const_ {
                            invalid = true;
                            break;
                        }
                        if public && kc.name.is_none_or(|n| !interner.resolve(n.sym).starts_with('_')) {
                            invalid = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if invalid || candidates.len() < 2 {
                continue;
            }
            // No other generative creation of the class (in the unit, except
            // those of the constants), nor a subclass that extends, implements
            // or mixes it in.
            let is_generative = |e: ExprId| match s.body.get_resolved(e) {
                Some(Resolved::Constructor(f)) => {
                    let g = program.function(*f);
                    !g.factory && g.class == Some(class_id)
                }
                _ => false,
            };
            let mut others = false;
            for (i, _) in a.exprs.iter().enumerate() {
                let e = ExprId(i as u32);
                if is_generative(e) && !candidates.contains(&e) {
                    others = true;
                    break;
                }
            }
            if others {
                continue;
            }
            let used = (0..program.classes.len()).map(|i| ClassId(i as u32)).any(|cl| {
                let kx = program.class(cl);
                cl != class_id
                    && kx.decl.is_some_and(|r| r.unit == s.unit)
                    && (kx.supertype_class == Some(class_id) || kx.interface_classes.contains(&class_id) || kx.mixin_classes.contains(&class_id))
            });
            if used {
                continue;
            }
            report(&c::USE_ENUMS, x.name.span, &[]);
        }
    }

    out
}
