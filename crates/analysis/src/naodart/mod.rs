//! The analyzer's validators of non-Dart files
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §7.2 and batch II.10): the
//! `pubspec.yaml` ([`pubspec`]) and the `analysis_options.yaml` ([`options`])
//! over YAML with positions ([`yaml`]). The codes are those of the generated
//! tables ([`codigos_g`]): they are not in the `crates/diagnostics` catalog
//! yet, so a report from here is a [`Relato`], not a `Diagnostic` with a code.
//!
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::{Severidade, Span, TipoErro};

pub mod codigos_g;
pub mod manifest;
pub mod nomes_g;
pub mod options;
pub mod pubspec;
pub mod yaml;

/// A diagnostic code of a non-Dart file (`AnalysisOptions*Code`,
/// `PubspecWarningCode`, `ManifestWarningCode`).
#[derive(Debug, PartialEq, Eq)]
pub struct CodigoNaoDart {
    /// The reported name (`ErrorCode.name` in lower case).
    pub nome: &'static str,
    /// `ErrorCode.uniqueName`.
    pub unico: &'static str,
    /// The message template, with `{0}`, `{1}`...
    pub mensagem: &'static str,
    /// The correction template.
    pub correcao: Option<&'static str>,
    pub tipo: TipoErro,
    pub severidade: Severidade,
    pub documentado: bool,
}

/// A diagnostic of a non-Dart file.
#[derive(Debug, PartialEq, Eq)]
pub struct Relato {
    pub codigo: &'static CodigoNaoDart,
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

impl Relato {
    pub fn novo(codigo: &'static CodigoNaoDart, span: Span, args: &[&str]) -> Relato {
        Relato { codigo, span, args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// The message of the problem.
    pub fn mensagem(&self) -> String {
        fill(self.codigo.mensagem, &self.args)
    }

    /// The correction message, if the code has one.
    pub fn correcao(&self) -> Option<String> {
        self.codigo.correcao.map(|c| fill(c, &self.args))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mensagem_com_argumentos() {
        let r = Relato::novo(&codigos_g::pubspec::DEPRECATED_FIELD, Span { start: 0, end: 6 }, &["author"]);
        assert!(r.mensagem().contains("author"));
    }
}
