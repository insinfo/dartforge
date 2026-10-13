//! Loop assists, ported from the `analysis_server` 3.6.2 producers over the
//! analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Convert to for-index loop` | `refactor.convert.forEachToForIndex` | `ConvertIntoForIndex` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::refactorings::Context;
use crate::refactorings_exec::{Text, ONE_INDENT};
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_types::{Resolved, Type};

/// The `toSource` of a `DeclaredIdentifier` (metadata, keyword, type and
/// name separated by a space), from the text: whitespace runs become one
/// space, and those inside type arguments disappear as in the
/// `ToSourceVisitor` (`<`, `>` and `, `).
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
    /// list) { … }` over a local variable or parameter of type `List` becomes
    /// `for (int i = 0; i < list.length; i++)` with `final x = list[i];` at the
    /// start of the block.
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
        // `iterable.element is VariableElement2`: a local or a parameter (a field
        // and a top-level variable resolve to the getter).
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
