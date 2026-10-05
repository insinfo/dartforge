//! Os rótulos de fechamento (`dart/textDocument/publishClosingLabels`), como
//! o `DartUnitClosingLabelsComputer` do servidor do Dart 3.6.2
//! (`AS:src/computer/computer_closing_labels.dart:22-133`;
//! docs/LSP-ESPECIFICACAO.md §3.4): um rótulo `Tipo` ou `Tipo.nome` para
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

use crate::projeto::Projeto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StringPart, TypeKind};
use dartforge_types::Resolved;

/// Um candidato a rótulo.
struct Candidato {
    /// O nó inteiro.
    span: Span,
    /// O trecho cuja primeira e última linha decidem se é de várias linhas.
    medida: Span,
    rotulo: String,
}

/// O nome qualificado escrito no alvo de uma chamada: `A`, `p.A`, `A.nome`.
fn nome_do_alvo(projeto: &Projeto, a: &ast::Ast, e: ExprId) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(projeto.consulta.nomes.resolve(n.sym).to_string()),
        ExprKind::Property { target, name, .. } => {
            Some(format!("{}.{}", nome_do_alvo(projeto, a, *target)?, projeto.consulta.nomes.resolve(name.sym)))
        }
        ExprKind::TypeArguments { target, .. } => nome_do_alvo(projeto, a, *target),
        _ => None,
    }
}

impl Projeto {
    /// Os rótulos de fechamento da unidade: o intervalo do nó e o texto, em
    /// pré-ordem.
    pub(crate) fn rotulos_de_fechamento(&self, unidade: UnitId) -> Vec<(Span, String)> {
        let u = self.consulta.programa.unit(unidade);
        let a = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let nomes = &self.consulta.nomes;
        // As expressões interpoladas: nada dentro delas ganha rótulo.
        let interpoladas: Vec<Span> = a
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
        let em_interpolacao = |s: Span| interpoladas.iter().any(|i| i.start <= s.start && s.end <= i.end);
        let mut candidatos: Vec<Candidato> = Vec::new();
        for (i, e) in a.exprs.iter().enumerate() {
            if em_interpolacao(e.span) {
                continue;
            }
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    let TypeKind::Named { name, .. } = &a.types[ty.0 as usize].kind else { continue };
                    let mut rotulo = name.iter().map(|n| nomes.resolve(n.sym)).collect::<Vec<_>>().join(".");
                    if let Some(k) = constructor {
                        rotulo.push('.');
                        rotulo.push_str(nomes.resolve(k.sym));
                    }
                    candidatos.push(Candidato { span: e.span, medida: arguments.span, rotulo });
                }
                // `A(...)` sem `new`: criação quando resolve para construtor.
                ExprKind::Call { target, arguments } => {
                    let construtor = matches!(corpos.get_resolved(id), Some(Resolved::Constructor(_)))
                        || matches!(corpos.get_resolved(*target), Some(Resolved::Constructor(_)));
                    if construtor && let Some(rotulo) = nome_do_alvo(self, a, *target) {
                        candidatos.push(Candidato { span: e.span, medida: arguments.span, rotulo });
                    }
                }
                ExprKind::List { type_args, .. } => {
                    if let Some(t) = type_args.first() {
                        let s = a.types[t.0 as usize].span;
                        let texto = fonte.get(s.start..s.end).unwrap_or("");
                        candidatos.push(Candidato { span: e.span, medida: e.span, rotulo: format!("<{texto}>[]") });
                    }
                }
                _ => {}
            }
        }
        // Pré-ordem: pelo início; no mesmo início, o de fora primeiro.
        candidatos.sort_by(|x, y| x.span.start.cmp(&y.span.start).then(y.span.end.cmp(&x.span.end)));
        let linha = |offset: usize| fonte.as_bytes()[..offset.min(fonte.len())].iter().filter(|b| **b == b'\n').count();
        let contem = |fora: Span, dentro: Span| fora.start <= dentro.start && dentro.end <= fora.end && fora != dentro;
        candidatos
            .iter()
            .filter(|k| {
                let varias_linhas = linha(k.medida.start) != linha(k.medida.end.saturating_sub(1));
                let aninhado = candidatos.iter().any(|o| contem(o.span, k.span) || contem(k.span, o.span));
                varias_linhas && aninhado
            })
            .map(|k| (k.span, k.rotulo.clone()))
            .collect()
    }
}
