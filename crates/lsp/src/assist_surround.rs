//! `SurroundWith` (surround_with.dart) do `analysis_server` 3.6.2: os
//! comandos selecionados, todos dentro de um bloco, envolvidos por um bloco,
//! um laço, um `if`, um `try` ou um `setState`. As posições ligadas do
//! produtor (`condition`, `item`, `iterable`…) saem como o texto delas.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Surround with block` | `refactor.surround.block` |
//! | `Surround with 'do-while'` | `refactor.surround.doWhile` |
//! | `Surround with 'for'` | `refactor.surround.forEach` |
//! | `Surround with 'for-in'` | `refactor.surround.forIn` |
//! | `Surround with 'if'` | `refactor.surround.if` |
//! | `Surround with 'setState'` | `refactor.surround.setState` |
//! | `Surround with 'try-catch'` | `refactor.surround.tryCatch` |
//! | `Surround with 'try-finally'` | `refactor.surround.tryFinally` |
//! | `Surround with 'while'` | `refactor.surround.while` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::refactorings::Context;
use crate::refactorings_exec::{Text, ONE_INDENT};
use crate::Edit;

impl Context<'_> {
    /// `ClassElement.isState`: a classe estende, direta ou indiretamente, o
    /// `State` do Flutter (`package:flutter/src/widgets/framework.dart`).
    fn class_and_state(&self, n: usize) -> bool {
        let Some(declaration) = self.with_parents(n).find(|&k| self.kind(k) == "ClassDeclaration") else { return false };
        let Marca::Decl(d) = self.tree.nos[declaration].marca else { return false };
        let Some(class) = self.declaration_class(self.unit, d) else { return false };
        let prog = self.p.program();
        let mut current = prog.class(class).supertype_class;
        let mut seen = std::collections::HashSet::new();
        while let Some(c) = current {
            if !seen.insert(c) {
                break;
            }
            let ce = prog.class(c);
            if self.p.name(ce.name) == "State" && prog.library(ce.library).uri == "package:flutter/src/widgets/framework.dart" {
                return true;
            }
            current = ce.supertype_class;
        }
        false
    }

    /// As assistências de `SurroundWith` para a seleção `[start, end)`.
    pub(crate) fn wrap_commands(&self, uri: &str, start: usize, end: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        // O nó coberto não pode ser a unidade (declarações de topo).
        let Some(no) = self.tree.localizar(start, end) else { return output };
        if self.kind(no) == "CompilationUnit" {
            return output;
        }
        // `StatementAnalyzer`: os nós selecionados, todos comandos de bloco.
        let analysis = self.analyze(start, end);
        let selected = &analysis.selected;
        if selected.is_empty()
            || !selected.iter().all(|&s| self.is_command(s) && self.parent(s).is_some_and(|p| self.kind(p) == "Block"))
        {
            return output;
        }
        let (first, last) = (selected[0], *selected.last().unwrap_or(&selected[0]));
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let range = tx.line_range(self.tree.nos[first].inicio, self.tree.nos[last].fim);
        let old = self.node_prefix(first);
        let new = format!("{old}{ONE_INDENT}");
        let indented = tx.swap_indent(&self.source[range.start..range.end], &old, &new, true, true);
        // `_SurroundWithBlock`: três edições, como o Dart — `{` inserido
        // antes, o código recuado no lugar e `}` inserido depois.
        output.push(CodeAction {
            title: "Surround with block".into(),
            kind: "refactor.surround.block".into(),
            edits: vec![
                Edit { uri: uri.to_string(), span: dartforge_diagnostics::Span { start: range.start, end: range.start }, text: format!("{old}{{{eol}") },
                Edit { uri: uri.to_string(), span: range, text: indented.clone() },
                Edit { uri: uri.to_string(), span: dartforge_diagnostics::Span { start: range.end, end: range.end }, text: format!("{old}}}{eol}") },
            ],
            diagnostic: None,
            create_file: None,
            linked: Default::default(),
        });
        let mut action = |title: &str, kind: &str, e: &crate::snippets::Write| {
            let a = CodeAction {
                title: title.into(),
                kind: kind.into(),
                edits: vec![Edit { uri: uri.to_string(), span: range, text: e.text.clone() }],
                diagnostic: None,
                create_file: None,
                linked: Default::default(),
            };
            output.push(a.with_write(0, e));
        };
        // Os demais escrevem a substituição inteira com os grupos ligados e o
        // `selectHere` de `surround_with.dart:135-424`.
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}do {{{eol}{indented}{old}}} while ("))
            .linked("CONDITION", "condition", &[])
            .write(");")
            .here()
            .write(eol);
        action("Surround with 'do-while'", "refactor.surround.doWhile", &e);
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}for (var "))
            .linked("VAR", "v", &[])
            .write(" = ")
            .linked("INIT", "init", &[])
            .write("; ")
            .linked("CONDITION", "condition", &[])
            .write("; ")
            .linked("INCREMENT", "increment", &[])
            .write(&format!(") {{{eol}{indented}{old}}}"))
            .here()
            .write(eol);
        action("Surround with 'for'", "refactor.surround.forEach", &e);
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}for (var "))
            .linked("NAME", "item", &[])
            .write(" in ")
            .linked("ITERABLE", "iterable", &[])
            .write(&format!(") {{{eol}{indented}{old}}}"))
            .here()
            .write(eol);
        action("Surround with 'for-in'", "refactor.surround.forIn", &e);
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}if ("))
            .linked("CONDITION", "condition", &[])
            .write(&format!(") {{{eol}{indented}{old}}}"))
            .here()
            .write(eol);
        action("Surround with 'if'", "refactor.surround.if", &e);
        if self.parent(no).is_some_and(|p| self.class_and_state(p)) {
            let mut e = crate::snippets::Write::default();
            e.write(&format!("{old}setState(() {{{eol}{indented}{old}"))
                .here()
                .write(&format!("}});{eol}"));
            action("Surround with 'setState'", "refactor.surround.setState", &e);
        }
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}try {{{eol}{indented}{old}}} on "))
            .linked("EXCEPTION_TYPE", "Exception", &[])
            .write(" catch (")
            .linked("EXCEPTION_VAR", "e", &[])
            .write(&format!(") {{{eol}{new}"))
            .linked("CATCH", "// TODO", &[])
            .here()
            .write(&format!("{eol}{old}}}{eol}"));
        action("Surround with 'try-catch'", "refactor.surround.tryCatch", &e);
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}try {{{eol}{indented}{old}}} finally {{{eol}{new}"))
            .linked("FINALLY", "// TODO", &[])
            .here()
            .write(&format!("{eol}{old}}}{eol}"));
        action("Surround with 'try-finally'", "refactor.surround.tryFinally", &e);
        let mut e = crate::snippets::Write::default();
        e.write(&format!("{old}while ("))
            .linked("CONDITION", "condition", &[])
            .write(&format!(") {{{eol}{indented}{old}}}"))
            .here()
            .write(eol);
        action("Surround with 'while'", "refactor.surround.while", &e);
        output
    }
}
