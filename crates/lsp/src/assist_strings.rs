//! String literal assists, ported from the `analysis_server` 3.6.2
//! producers over the analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Convert to single quoted string` | `refactor.convert.toSingleQuotedString` | `ConvertToSingleQuotes` |
//! | `Convert to double quoted string` | `refactor.convert.toDoubleQuotedString` | `ConvertToDoubleQuotes` |
//! | `Convert to multiline string` | `refactor.convert.toMultilineString` | `ConvertToMultilineString` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::refactorings_exec::Change;
use dartforge_diagnostics::Span;

/// The text of a literal: raw (`r`), multi-line and single-quoted.
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
    /// `_ConvertQuotes.compute`: `from_double_quotes` converts from double to single
    /// quotes (`ConvertToSingleQuotes`); otherwise from single to double.
    fn convert_quotes(&self, uri: &str, start: usize, end: usize, from_double_quotes: bool) -> Option<Change> {
        let node = self.tree.locate(start, end)?;
        let mut m = Change::default();
        // The producer's `token` (`_tokenAt(node, selectionOffset)`).
        let token: Option<Span>;
        match self.kind(node) {
            "SimpleStringLiteral" => {
                let span = self.tree.span(node);
                token = Some(span);
                let text = &self.source[span.start..span.end];
                let a = quotes_of(text);
                if if from_double_quotes { !a.simple } else { a.simple } {
                    let new = match (a.multiline, from_double_quotes) {
                        (true, true) => "'''",
                        (true, false) => "\"\"\"",
                        (false, true) => "'",
                        (false, false) => "\"",
                    };
                    let size = if a.multiline { 3 } else { 1 };
                    // `_addBackslash`: before each unescaped new quote (the loop starts at
                    // index 1 of the token text).
                    let quote = if from_double_quotes { b'\'' } else { b'"' };
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
                let interpolation = if self.kind(node) == "InterpolationString" { self.parent(node)? } else { node };
                let span = self.tree.span(interpolation);
                let a = quotes_of(&self.source[span.start..span.end]);
                token = if self.kind(node) == "InterpolationString" {
                    Some(self.tree.span(node))
                } else {
                    self.children(node).iter().map(|&k| self.tree.span(k)).find(|s| s.start <= start && start <= s.end)
                };
                if if from_double_quotes { !a.simple } else { a.simple } {
                    let new = match (a.multiline, from_double_quotes) {
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
        // `_removeBackslash(token)`: the `\` before the old quote.
        if let Some(t) = token {
            let quote = if from_double_quotes { b'"' } else { b'\'' };
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

    /// `ConvertToSingleQuotes` and `ConvertToDoubleQuotes` (convert_quotes.dart).
    pub(crate) fn convert_text_quotes(&self, uri: &str, start: usize, end: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        for (from_double_quotes, title, kind) in [
            (false, "Convert to double quoted string", "refactor.convert.toDoubleQuotedString"),
            (true, "Convert to single quoted string", "refactor.convert.toSingleQuotedString"),
        ] {
            if let Some(m) = self.convert_quotes(uri, start, end, from_double_quotes) {
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

    /// `ConvertToMultilineString` (convert_to_multiline_string.dart): the
    /// single-line literal gets triple quotes, with a line break after the
    /// opening ones (`writeln`).
    pub(crate) fn convert_to_multiline(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let mut node = self.tree.locate(start, end)?;
        if matches!(self.kind(node), "InterpolationString" | "InterpolationExpression") {
            node = self.parent(node)?;
        }
        if !matches!(self.kind(node), "SimpleStringLiteral" | "StringInterpolation") {
            return None;
        }
        let span = self.tree.span(node);
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
