//! Closing labels (`dart/textDocument/publishClosingLabels`), like the Dart
//! 3.6.2 server's `DartUnitClosingLabelsComputer`
//! (`AS:src/computer/computer_closing_labels.dart:22-133`;
//! docs/LSP-ESPECIFICACAO.md §3.4): a `Type` or `Type.name` label for each
//! instance creation and `<T>[]` for each list with a type argument, when
//! the node spans more than one line **and** is nested with another label
//! (inside one, or with one inside it).
//!
//! A call without `new` is an instance creation when resolution says the
//! target is a constructor. The line count of a creation uses the argument
//! list; that of a list, the literal. Nothing inside string interpolation.
//! The type argument text is the source's, without the normalization of the
//! original's `toString`.
//! Written without compiling or running (2026-10-05).

use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StringPart, TypeKind};
use dartforge_types::Resolved;

/// A label candidate.
struct Candidate {
    /// The whole node.
    span: Span,
    /// The span whose first and last lines decide whether it is multi-line.
    measured: Span,
    label: String,
}

/// The qualified name written in a call target: `A`, `p.A`, `A.name`.
fn target_name(project: &Project, a: &ast::Ast, e: ExprId) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(project.query.names.resolve(n.sym).to_string()),
        ExprKind::Property { target, name, .. } => {
            Some(format!("{}.{}", target_name(project, a, *target)?, project.query.names.resolve(name.sym)))
        }
        ExprKind::TypeArguments { target, .. } => target_name(project, a, *target),
        _ => None,
    }
}

impl Project {
    /// The unit's closing labels: the node range and the text, in pre-order.
    pub(crate) fn closing_labels(&self, unit: UnitId) -> Vec<(Span, String)> {
        let u = self.query.program.unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let names = &self.query.names;
        // The interpolated expressions: nothing inside them gets a label.
        let interpolated: Vec<Span> = a
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::String(lit) => Some(lit),
                _ => None,
            })
            .flat_map(|lit| lit.parts.iter())
            .filter_map(|p| match p {
                StringPart::Interpolation(x) => Some(a.expr(*x).span),
                _ => None,
            })
            .collect();
        let in_interpolation = |s: Span| interpolated.iter().any(|i| i.start <= s.start && s.end <= i.end);
        let mut candidates: Vec<Candidate> = Vec::new();
        for (i, e) in a.exprs.iter().enumerate() {
            if in_interpolation(e.span) {
                continue;
            }
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    let TypeKind::Named { name, .. } = &a.types[ty.0 as usize].kind else { continue };
                    let mut label = name.iter().map(|n| names.resolve(n.sym)).collect::<Vec<_>>().join(".");
                    if let Some(k) = constructor {
                        label.push('.');
                        label.push_str(names.resolve(k.sym));
                    }
                    candidates.push(Candidate { span: e.span, measured: arguments.span, label });
                }
                // `A(...)` without `new`: a creation when it resolves to a constructor.
                ExprKind::Call { target, arguments } => {
                    let constructor_ = matches!(bodies.get_resolved(id), Some(Resolved::Constructor(_)))
                        || matches!(bodies.get_resolved(*target), Some(Resolved::Constructor(_)));
                    if constructor_ && let Some(label) = target_name(self, a, *target) {
                        candidates.push(Candidate { span: e.span, measured: arguments.span, label });
                    }
                }
                ExprKind::List { type_args, .. } => {
                    if let Some(t) = type_args.first() {
                        let s = a.types[t.0 as usize].span;
                        let text = source.get(s.start..s.end).unwrap_or("");
                        candidates.push(Candidate { span: e.span, measured: e.span, label: format!("<{text}>[]") });
                    }
                }
                _ => {}
            }
        }
        // Pre-order: by start; at the same start, the outer one first.
        candidates.sort_by(|x, y| x.span.start.cmp(&y.span.start).then(y.span.end.cmp(&x.span.end)));
        let line = |offset: usize| source.as_bytes()[..offset.min(source.len())].iter().filter(|b| **b == b'\n').count();
        let contains = |outside: Span, inside: Span| outside.start <= inside.start && inside.end <= outside.end && outside != inside;
        candidates
            .iter()
            .filter(|k| {
                let several_lines = line(k.measured.start) != line(k.measured.end.saturating_sub(1));
                let nested = candidates.iter().any(|o| contains(o.span, k.span) || contains(k.span, o.span));
                several_lines && nested
            })
            .map(|k| (k.span, k.label.clone()))
            .collect()
    }
}
