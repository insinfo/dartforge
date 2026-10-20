//! The analyzer's validators of non-Dart files
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §7.2 and batch II.10): the
//! `pubspec.yaml` ([`pubspec`]) and the `analysis_options.yaml` ([`options`])
//! over YAML with positions ([`yaml`]). The codes are those of the generated
//! tables ([`codes_g`]): they are not in the `crates/diagnostics` catalog
//! yet, so a report from here is a [`Report`], not a `Diagnostic` with a code.
//!
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::{ErrorType, Severity, Span};

pub mod codes_g;
pub mod manifest;
pub mod names_g;
pub mod options;
pub mod pubspec;
pub mod yaml;

/// A diagnostic code of a non-Dart file (`AnalysisOptions*Code`,
/// `PubspecWarningCode`, `ManifestWarningCode`).
#[derive(Debug, PartialEq, Eq)]
pub struct NonDartCode {
    /// The reported name (`ErrorCode.name` in lower case).
    pub name: &'static str,
    /// `ErrorCode.uniqueName`.
    pub unique: &'static str,
    /// The message template, with `{0}`, `{1}`...
    pub message: &'static str,
    /// The correction template.
    pub correction: Option<&'static str>,
    pub error_type: ErrorType,
    pub severity: Severity,
    pub documented: bool,
}

/// A diagnostic of a non-Dart file.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub code: &'static NonDartCode,
    pub span: Span,
    pub args: Vec<String>,
}

/// A template with the `{n}`s replaced by the arguments.
fn fill(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

impl Report {
    pub fn new(code: &'static NonDartCode, span: Span, args: &[&str]) -> Report {
        Report {
            code,
            span,
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    /// The message of the problem.
    pub fn message(&self) -> String {
        fill(self.code.message, &self.args)
    }

    /// The correction message, if the code has one.
    pub fn correction(&self) -> Option<String> {
        self.code.correction.map(|c| fill(c, &self.args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_with_arguments() {
        let r = Report::new(
            &codes_g::pubspec::DEPRECATED_FIELD,
            Span { start: 0, end: 6 },
            &["author"],
        );
        assert!(r.message().contains("author"));
    }
}
