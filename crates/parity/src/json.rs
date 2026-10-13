//! The JSON v1 format of `dart analyze --format=json`, read from the oracle
//! and written by us field by field in the same order.
//!
//! Measured on SDK 3.6.2 (7-error probe): a compact object
//! `{"version":1,"diagnostics":[...]}`; each diagnostic has `code` (the
//! `ErrorCode.name` in lower case), `severity`, `type`, `location` with the
//! file's absolute path and the range (`offset` in **UTF-16 code units**,
//! `line` and `column` starting at 1), `problemMessage`, and — only when they
//! exist — `correctionMessage` and `documentation` (the `hasPublishedDocs`
//! URL). The order is dartdev's `DiagnosticWithPath.compareTo`: severity
//! (errors first), path, line, column, message.

use dartforge_diagnostics::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub version: u32,
    pub diagnostics: Vec<DiagJson>,
}

/// A diagnostic in JSON v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagJson {
    pub code: String,
    pub severity: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub location: Local,
    pub problem_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction_message: Option<String>,
    /// The context messages; the key only exists when there is one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_messages: Vec<ContextJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// A context message in JSON v1 (`analyze.dart:409-422`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextJson {
    pub location: Local,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Local {
    pub file: String,
    pub range: Faixa,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faixa {
    pub start: Ponto,
    pub end: Ponto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ponto {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

/// Converts UTF-8 byte offsets to (UTF-16 offset, line, column), like the
/// analyzer's `LineInfo`: line breaks are `\n`, `\r\n` and a lone `\r`.
pub struct Linhas<'a> {
    text: &'a str,
    /// The first byte of each line.
    starts: Vec<usize>,
    /// The UTF-16 offset of the start of each line.
    starts16: Vec<usize>,
}

impl<'a> Linhas<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        let mut starts16 = vec![0];
        let b = text.as_bytes();
        let mut u16s = 0usize;
        let mut i = 0;
        for (pos, ch) in text.char_indices() {
            if pos < i {
                continue;
            }
            u16s += ch.len_utf16();
            i = pos + ch.len_utf8();
            let is_break = match ch {
                '\n' => true,
                '\r' => {
                    if b.get(i) == Some(&b'\n') {
                        i += 1;
                        u16s += 1;
                    }
                    true
                }
                _ => false,
            };
            if is_break {
                starts.push(i);
                starts16.push(u16s);
            }
        }
        Self { text, starts, starts16 }
    }

    /// The point of byte `byte` (clamped to the end of the text and to a character boundary).
    pub fn ponto(&self, byte: usize) -> Ponto {
        // Past the end (the fasta scanner reports the missing closer at the
        // end of the file with length 1): the last line continues.
        if byte > self.text.len() {
            let p = self.ponto(self.text.len());
            let extra = byte - self.text.len();
            return Ponto { offset: p.offset + extra, line: p.line, column: p.column + extra };
        }
        let mut byte = byte;
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        let line = self.starts.partition_point(|&s| s <= byte) - 1;
        let col16: usize = self.text[self.starts[line]..byte].chars().map(char::len_utf16).sum();
        Ponto { offset: self.starts16[line] + col16, line: line + 1, column: col16 + 1 }
    }
}

/// Converts one of our diagnostics to JSON v1. Without a code, the `code`
/// field is `dartforge_sem_codigo` (it never matches the oracle: it counts as
/// a false positive in the scoreboard).
pub fn to_json(file: &str, lines: &Linhas<'_>, d: &Diagnostic, syntax: bool) -> DiagJson {
    let (code, kind, doc) = match d.code {
        Some(c) => {
            let i = c.info();
            (i.name.to_string(), i.error_type.name().to_string(), i.url())
        }
        None => {
            let kind = if syntax { "SYNTACTIC_ERROR" } else { "COMPILE_TIME_ERROR" };
            ("dartforge_sem_codigo".to_string(), kind.to_string(), None)
        }
    };
    DiagJson {
        code,
        severity: d.severity.name().to_string(),
        tipo: kind,
        location: Local {
            file: file.to_string(),
            range: Faixa { start: lines.ponto(d.span.start), end: lines.ponto(d.span.end.max(d.span.start)) },
        },
        problem_message: d.message.clone(),
        correction_message: d.correction(),
        // Line and column come from the `lineInfo` of the ERROR's file, even
        // when the message points at another file (`protocol_server.dart:185-191`).
        context_messages: d
            .context
            .iter()
            .map(|c| ContextJson {
                location: Local {
                    file: c.file.as_deref().unwrap_or(file).to_string(),
                    range: Faixa { start: lines.ponto(c.span.start), end: lines.ponto(c.span.end.max(c.span.start)) },
                },
                message: c.message.to_string(),
            })
            .collect(),
        documentation: doc,
    }
}

fn priority(sev: &str) -> u8 {
    match Severity::by_name(sev) {
        Some(Severity::Error) => 0,
        Some(Severity::Warning) => 1,
        Some(Severity::Info) => 3,
        None => 4,
    }
}

/// dartdev's order (`DiagnosticWithPath.compareTo`).
pub fn ordenar(v: &mut [DiagJson]) {
    v.sort_by(|a, b| {
        priority(&a.severity)
            .cmp(&priority(&b.severity))
            .then_with(|| a.location.file.cmp(&b.location.file))
            .then_with(|| a.location.range.start.line.cmp(&b.location.range.start.line))
            .then_with(|| a.location.range.start.column.cmp(&b.location.range.start.column))
            .then_with(|| a.problem_message.cmp(&b.problem_message))
    });
}

/// Serializes like `dart analyze` (compact JSON, no trailing newline).
pub fn escrever(diags: Vec<DiagJson>) -> String {
    serde_json::to_string(&Report { version: 1, diagnostics: diags }).expect("JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_utf16_and_breaks() {
        let t = "a\r\nb\u{1F600}c\rd\n";
        let l = Linhas::new(t);
        assert_eq!(l.ponto(0), Ponto { offset: 0, line: 1, column: 1 });
        assert_eq!(l.ponto(3), Ponto { offset: 3, line: 2, column: 1 });
        // after the emoji (4 bytes, 2 UTF-16 code units)
        assert_eq!(l.ponto(8), Ponto { offset: 6, line: 2, column: 4 });
        assert_eq!(l.ponto(10), Ponto { offset: 8, line: 3, column: 1 });
    }
}
