//! The thirtieth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `use_late_for_private_fields_and_variables`: in the whole library, the
//!   private top-level variables and the private fields (or those of a
//!   private extension), not `late`, of a nullable type, that no nullable
//!   access reaches. Outside classes with a `const` constructor and `enum`s
//!   (the visitor does not descend into them), every identifier,
//!   `PrefixedIdentifier` and `PropertyAccess` with an element counts as a
//!   nullable access, except the simple identifier right before a `!` (the
//!   property name inside `a.b` has `a.b` as its parent and always counts);
//!   an assignment counts through the write element, except `=` with a
//!   non-nullable right side; the left side and the operand of `++`/`--` have
//!   no element.
//!
//!   Recorded divergence: documentation comment references (`[name]`) do not
//!   count.
//!
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::rules::LintReport;
use super::rules16::{declared_variable_type, is_non_nullable};
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{
    AssignOp, Ast, DeclId, DeclKind, ExprId, ExprKind, MemberId, MemberKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved, UnitBodyTypes};
use std::collections::HashSet;

/// The model variable that the resolved element canonicalizes to (an
/// accessor becomes the variable).
fn variable_of(s: &super::Semantics<'_>, body: &UnitBodyTypes, e: ExprId) -> Option<VariableId> {
    match body.get_resolved(e)? {
        Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        }
        | Resolved::Element(Element::Variable(v)) => Some(*v),
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        }
        | Resolved::Element(Element::Function(f)) => s.program.function(*f).variable,
        Resolved::ExtensionMember { member, .. } => s.program.function(*member).variable,
        _ => None,
    }
}

/// The regions the visitor does not visit: classes with a `const`
/// constructor and `enum`s.
fn excluded_regions(a: &Ast) -> Vec<Span> {
    let mut v = Vec::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Class(x)
                if x.members.iter().any(
                    |m| matches!(&a.member(*m).kind, MemberKind::Constructor(k) if k.const_),
                ) =>
            {
                v.push(d.span)
            }
            DeclKind::Enum(_) => v.push(d.span),
            _ => {}
        }
    }
    v
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if !enabled("use_late_for_private_fields_and_variables") {
        return out;
    }
    let program = s.program;
    let table = s.table;
    let is_private = |n: dartforge_intern::SymbolId| interner.resolve(n).starts_with('_');
    let library = program.unit(s.unit).library;
    let units: Vec<UnitId> = program.library(library).units.clone();
    // The library's nullable accesses.
    let mut nullable_accesses: HashSet<VariableId> = HashSet::new();
    for &x in units.iter() {
        let a = &program.unit(x).ast;
        let body = &s.bodies.units[x.0 as usize];
        let excluded = excluded_regions(a);
        let visited = |sp: Span| {
            !excluded
                .iter()
                .any(|f| f.start <= sp.start && sp.end <= f.end)
        };
        let mut written: HashSet<ExprId> = HashSet::new();
        // The immediate operand of a `!` (without parentheses).
        let mut checked: HashSet<ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    written.insert(*target);
                }
                ExprKind::Unary {
                    op:
                        UnaryOp::PrefixInc
                        | UnaryOp::PrefixDec
                        | UnaryOp::PostfixInc
                        | UnaryOp::PostfixDec,
                    operand,
                } => {
                    written.insert(*operand);
                }
                ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    operand,
                } => {
                    checked.insert(*operand);
                }
                _ => {}
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !visited(e.span) {
                continue;
            }
            match &e.kind {
                ExprKind::Identifier(_) if !written.contains(&id) => {
                    if let Some(v) = variable_of(s, body, id)
                        && !checked.contains(&id)
                    {
                        nullable_accesses.insert(v);
                    }
                }
                ExprKind::Property { .. } if !written.contains(&id) => {
                    // The name inside `a.b` has `a.b` itself as its parent.
                    if let Some(v) = variable_of(s, body, id) {
                        nullable_accesses.insert(v);
                    }
                }
                ExprKind::Assign { op, target, value } => {
                    if let Some(v) = variable_of(s, body, *target) {
                        let non_null = *op == AssignOp::Assign
                            && s.bodies.units[x.0 as usize]
                                .get_type(*value)
                                .is_some_and(|t| is_non_nullable(table, t));
                        if !non_null {
                            nullable_accesses.insert(v);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // The unit's candidates.
    let a = u.ast;
    let excluded = excluded_regions(a);
    let visited = |sp: Span| {
        !excluded
            .iter()
            .any(|f| f.start <= sp.start && sp.end <= f.end)
    };
    let mut findings: Vec<Span> = Vec::new();
    let candidate = |r: VariableRef,
                     l: &dartforge_frontend::ast::VariableList,
                     i: usize,
                     findings: &mut Vec<Span>| {
        if l.late {
            return;
        }
        let Some(v) = program.variable_of_node(r) else {
            return;
        };
        let Some(t) = declared_variable_type(s, r) else {
            return;
        };
        if is_non_nullable(table, t) {
            return;
        }
        if !nullable_accesses.contains(&v) {
            let x = &l.variables[i];
            let end = x
                .initializer
                .map_or(x.name.span.end, |e| a.expr(e).span.end);
            findings.push(Span {
                start: x.name.span.start,
                end,
            });
        }
    };
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        if !visited(d.span) {
            continue;
        }
        match &d.kind {
            DeclKind::Variables(l) => {
                for (i, x) in l.variables.iter().enumerate() {
                    if is_private(x.name.sym) {
                        candidate(
                            VariableRef::TopLevel {
                                unit: s.unit,
                                decl: id,
                                index: i,
                            },
                            l,
                            i,
                            &mut findings,
                        );
                    }
                }
            }
            _ => {
                let (members, private_extension, is_extension_type): (&[MemberId], bool, bool) =
                    match &d.kind {
                        DeclKind::Class(x) => (&x.members, false, false),
                        DeclKind::Mixin(x) => (&x.members, false, false),
                        DeclKind::Extension(x) => {
                            (&x.members, x.name.is_none_or(|n| is_private(n.sym)), false)
                        }
                        DeclKind::ExtensionType(x) => (&x.members, false, true),
                        _ => continue,
                    };
                for &m in members {
                    let MemberKind::Field(l) = &a.member(m).kind else {
                        continue;
                    };
                    if is_extension_type && !l.static_ {
                        continue;
                    }
                    for (i, x) in l.variables.iter().enumerate() {
                        if private_extension || is_private(x.name.sym) {
                            candidate(
                                VariableRef::Field {
                                    unit: s.unit,
                                    member: m,
                                    index: i,
                                },
                                l,
                                i,
                                &mut findings,
                            );
                        }
                    }
                }
            }
        }
    }
    for sp in findings {
        out.push(LintReport {
            code: &c::USE_LATE_FOR_PRIVATE_FIELDS_AND_VARIABLES,
            span: sp,
            args: Vec::new(),
        });
    }
    out
}
