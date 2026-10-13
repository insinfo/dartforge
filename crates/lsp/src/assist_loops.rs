//! Assistências de laços, portadas dos produtores do `analysis_server`
//! 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to for-index loop` | `refactor.convert.forEachToForIndex` | `ConvertIntoForIndex` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::refactorings::Context;
use crate::refactorings_exec::{Text, ONE_INDENT};
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_types::{Resolved, Type};

/// O `toSource` de um `DeclaredIdentifier` (metadados, palavra-chave, tipo e
/// nome separados por um espaço), a partir do texto: os brancos viram um
/// espaço, e os de dentro dos argumentos de tipo somem como no
/// `ToSourceVisitor` (`<`, `>` e `, `).
pub(crate) fn as_source(text: &str) -> String {
    let mut s = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for (de, para) in [("< ", "<"), (" <", "<"), (" >", ">"), (" ,", ","), (" ?", "?")] {
        while s.contains(de) {
            s = s.replace(de, para);
        }
    }
    s
}

impl Context<'_> {
    /// `ConvertIntoForIndex` (convert_into_for_index.dart): `for (final x in
    /// lista) { … }` sobre uma variável local or pairâmetro de ty `List`
    /// vira `for (int i = 0; i < list.length; i++)` com `final x =
    /// lista[i];` no começo do bloco.
    pub(crate) fn convert_to_for_index(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let loop_ = self.with_parents(no).find(|&k| self.kind(k) == "ForStatement")?;
        let parts = self.children(loop_).iter().copied().find(|&k| self.kind(k).starts_with("ForEachParts"))?;
        let body = *self.children(loop_).last()?;
        // `selectionOffset < forStatement.offset || rightParenthesis.end < selectionOffset`.
        let closes = self.previous_token(self.tree.nos[body].inicio)?;
        if start < self.tree.nos[loop_].inicio || closes.end < start {
            return None;
        }
        if self.kind(parts) != "ForEachPartsWithDeclaration" {
            return None;
        }
        let children = self.children(parts);
        let variable = *children.iter().find(|&&k| self.kind(k) == "DeclaredIdentifier")?;
        let iterable = *children.last()?;
        if self.kind(iterable) != "SimpleIdentifier" {
            return None;
        }
        // `iterable.element is VariableElement2`: local ou parâmetro (o
        // campo e a variável de topo resolvem para o getter).
        let Marca::Expr(x) = self.tree.nos[iterable].marca else { return None };
        if !matches!(self.bodies.get_resolved(x), Some(Resolved::Local(_) | Resolved::Parameter { .. })) {
            return None;
        }
        let list = self.p.query.core.list_class?;
        let ty = *self.bodies.static_types.get(x.0 as usize)?;
        if !matches!(self.p.query.table.get(ty), Type::Interface { class, .. } if *class == list) {
            return None;
        }
        if self.kind(body) != "Block" {
            return None;
        }
        let conflicts = self.local_conflicts(self.tree.nos[loop_].inicio);
        let index = ["i", "j", "k"].into_iter().find(|n| !conflicts.contains(*n))?;
        let list_name = self.node_text(iterable).to_string();
        let tx = Text::new(self.source);
        let prefix = self.node_prefix(loop_);
        // `getLineContentEnd(body.leftBracket.end)`.
        let first_line = tx.content_end(self.tree.nos[body].inicio + 1);
        let variable_source = as_source(self.node_text(variable));
        let eol = tx.eol();
        Some(CodeAction {
            title: "Convert to for-index loop".into(),
            kind: "refactor.convert.forEachToForIndex".into(),
            edits: vec![
                Edit {
                    uri: uri.to_string(),
                    span: Span { start: self.tree.nos[loop_].inicio, end: closes.end },
                    text: format!("for (int {index} = 0; {index} < {list_name}.length; {index}++)"),
                },
                Edit {
                    uri: uri.to_string(),
                    span: Span { start: first_line, end: first_line },
                    text: format!("{prefix}{ONE_INDENT}{variable_source} = {list_name}[{index}];{eol}"),
                },
            ],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
