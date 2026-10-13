//! Os rótulos de fechamento (`dart/textDocument/publishClosingLabels`), como
//! o `DartUnitClosingLabelsComputer` do servidor do Dart 3.6.2
//! (`AS:src/computer/computer_closing_labels.dart:22-133`;
//! docs/LSP-ESPECIFICACAO.md §3.4): um rótulo `Type` ou `Type.name` para
//! cada criação de instância e `<T>[]` para cada lista com argumento de
//! tipo, quando o nó ocupa mais de uma linha **e** está aninhado com outro
//! rótulo (dentro de um, ou com um dentro dele).
//!
//! Uma chamada sem `new` é criação de instância quando a resolução diz que
//! o alvo é um construtor. A conta de linhas de uma criação usa a lista de
//! argumentos; a de uma lista, o literal. Nada dentro de interpolação de
//! string. O texto do argumento de tipo é o da fonte, sem a normalização do
//! `toString` do original.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StringPart, TypeKind};
use dartforge_types::Resolved;

/// Um candidato a rótulo.
struct Candidate {
    /// O nó inteiro.
    span: Span,
    /// O trecho cuja primeira e última linha decidem se é de várias linhas.
    measured: Span,
    label: String,
}

/// O nome qualificado escrito no alvo de uma chamada: `A`, `p.A`, `A.name`.
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
    /// Os rótulos de fechamento da unidade: o intervalo do nó e o texto, em
    /// pré-ordem.
    pub(crate) fn closing_labels(&self, unit: UnitId) -> Vec<(Span, String)> {
        let u = self.query.program.unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let names = &self.query.names;
        // As expressões interpoladas: nada dentro delas ganha rótulo.
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
                // `A(...)` sem `new`: criação quando resolve para construtor.
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
        // Pré-ordem: pelo início; no mesmo início, o de fora primeiro.
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
