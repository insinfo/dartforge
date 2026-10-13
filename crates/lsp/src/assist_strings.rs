//! Assistências de literais de texto, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to single quoted string` | `refactor.convert.toSingleQuotedString` | `ConvertToSingleQuotes` |
//! | `Convert to double quoted string` | `refactor.convert.toDoubleQuotedString` | `ConvertToDoubleQuotes` |
//! | `Convert to multiline string` | `refactor.convert.toMultilineString` | `ConvertToMultilineString` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::refactorings_exec::Change;
use dartforge_diagnostics::Span;

/// O texto de um literal: cru (`r`), de várias linhas e com aspas simples.
struct Quotes {
    raw: bool,
    multiline: bool,
    simple: bool,
}

fn quotes_of(text: &str) -> Quotes {
    let raw = text.starts_with('r') || text.starts_with('R');
    let rest = if raw { &text[1..] } else { text };
    Quotes { raw, multiline: rest.starts_with("'''") || rest.starts_with("\"\"\""), simple: rest.starts_with('\'') }
}

impl Context<'_> {
    /// `_ConvertQuotes.compute`: `of_double` converte de aspas duplas para
    /// simples (`ConvertToSingleQuotes`); senão, de simples para duplas.
    fn convert_quotes(&self, uri: &str, start: usize, end: usize, of_double: bool) -> Option<Change> {
        let no = self.tree.localizar(start, end)?;
        let mut m = Change::default();
        // O `token` do produtor (`_tokenAt(node, selectionOffset)`).
        let token: Option<Span>;
        match self.kind(no) {
            "SimpleStringLiteral" => {
                let span = self.tree.span(no);
                token = Some(span);
                let text = &self.source[span.start..span.end];
                let a = quotes_of(text);
                if if of_double { !a.simple } else { a.simple } {
                    let new = match (a.multiline, of_double) {
                        (true, true) => "'''",
                        (true, false) => "\"\"\"",
                        (false, true) => "'",
                        (false, false) => "\"",
                    };
                    let size = if a.multiline { 3 } else { 1 };
                    // `_addBackslash`: antes de cada aspa nova não escapada
                    // (o laço começa no índice 1 do texto do token).
                    let quote = if of_double { b'\'' } else { b'"' };
                    let b = text.as_bytes();
                    let mut i = 1;
                    while i + 1 < b.len() {
                        if b[i + 1] == quote && b[i] != b'\\' {
                            m.add(uri, Span { start: span.start + 1 + i, end: span.start + 1 + i }, String::from("\\"));
                        }
                        i += 1;
                    }
                    let opens = span.start + usize::from(a.raw);
                    m.add(uri, Span { start: opens, end: opens + size }, new.to_string());
                    m.add(uri, Span { start: span.end - size, end: span.end }, new.to_string());
                }
            }
            "StringInterpolation" | "InterpolationString" => {
                let interpolation = if self.kind(no) == "InterpolationString" { self.parent(no)? } else { no };
                let span = self.tree.span(interpolation);
                let a = quotes_of(&self.source[span.start..span.end]);
                token = if self.kind(no) == "InterpolationString" {
                    Some(self.tree.span(no))
                } else {
                    self.children(no).iter().map(|&k| self.tree.span(k)).find(|s| s.start <= start && start <= s.end)
                };
                if if of_double { !a.simple } else { a.simple } {
                    let new = match (a.multiline, of_double) {
                        (true, true) => "'''",
                        (true, false) => "\"\"\"",
                        (false, true) => "'",
                        (false, false) => "\"",
                    };
                    let size = if a.multiline { 3 } else { 1 };
                    let some_contains = self
                        .children(interpolation)
                        .iter()
                        .filter(|&&k| self.kind(k) == "InterpolationString")
                        .any(|&k| self.node_text(k).contains(new));
                    if !some_contains {
                        let opens = span.start + usize::from(a.raw);
                        m.add(uri, Span { start: opens, end: opens + size }, new.to_string());
                        m.add(uri, Span { start: span.end - size, end: span.end }, new.to_string());
                    }
                }
            }
            _ => return None,
        }
        // `_removeBackslash(token)`: o `\` antes da aspa antiga.
        if let Some(t) = token {
            let quote = if of_double { b'"' } else { b'\'' };
            let b = &self.source.as_bytes()[t.start..t.end];
            let mut i = 0;
            while i + 1 < b.len() {
                if b[i] == b'\\' && b[i + 1] == quote {
                    m.add(uri, Span { start: t.start + i, end: t.start + i + 1 }, String::new());
                    i += 1;
                }
                i += 1;
            }
        }
        (m.conflict.is_none() && !m.files.is_empty()).then_some(m)
    }

    /// `ConvertToSingleQuotes` e `ConvertToDoubleQuotes` (convert_quotes.dart).
    pub(crate) fn convert_text_quotes(&self, uri: &str, start: usize, end: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        for (of_double, title, kind) in [
            (false, "Convert to double quoted string", "refactor.convert.toDoubleQuotedString"),
            (true, "Convert to single quoted string", "refactor.convert.toSingleQuotedString"),
        ] {
            if let Some(m) = self.convert_quotes(uri, start, end, of_double) {
                output.push(CodeAction {
                    title: title.into(),
                    kind: kind.into(),
                    edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
                    diagnostic: None,
                    create_file: None, linked: Default::default(),
                });
            }
        }
        output
    }

    /// `ConvertToMultilineString` (convert_to_multiline_string.dart): o
    /// literal de uma linha ganha as aspas triplas, com uma quebra de linha
    /// depois das de abertura (`writeln`).
    pub(crate) fn convert_to_multiline(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let mut no = self.tree.localizar(start, end)?;
        if matches!(self.kind(no), "InterpolationString" | "InterpolationExpression") {
            no = self.parent(no)?;
        }
        if !matches!(self.kind(no), "SimpleStringLiteral" | "StringInterpolation") {
            return None;
        }
        let span = self.tree.span(no);
        let a = quotes_of(&self.source[span.start..span.end]);
        if a.multiline || span.end <= span.start + 1 {
            return None;
        }
        let new = if a.simple { "\'\'\'" } else { "\"\"\"" };
        let eol = crate::refactorings_exec::Text::new(self.source).eol();
        let opens = span.start + usize::from(a.raw);
        let mut m = Change::default();
        m.add(uri, Span { start: opens, end: opens + 1 }, format!("{new}{eol}"));
        m.add(uri, Span { start: span.end - 1, end: span.end }, new.to_string());
        Some(CodeAction {
            title: "Convert to multiline string".into(),
            kind: "refactor.convert.toMultilineString".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
