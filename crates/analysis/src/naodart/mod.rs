//! Os validadores de arquivos não-Dart do analyzer
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §7.2 e lote II.10): o
//! `pubspec.yaml` ([`pubspec`]) e o `analysis_options.yaml` ([`opcoes`])
//! sobre o YAML com posições ([`yaml`]). Os
//! códigos são os das tabelas geradas ([`codigos_g`]): ainda não estão no
//! catálogo de `crates/diagnostics`, por isso um relato daqui é um
//! [`Relato`], não um `Diagnostic` com código.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::{Severidade, Span, TipoErro};

pub mod codigos_g;
pub mod manifesto;
pub mod nomes_g;
pub mod opcoes;
pub mod pubspec;
pub mod yaml;

/// Um código de diagnóstico de arquivo não-Dart (`AnalysisOptions*Code`,
/// `PubspecWarningCode`, `ManifestWarningCode`).
#[derive(Debug, PartialEq, Eq)]
pub struct CodigoNaoDart {
    /// O nome relatado (`ErrorCode.name` em minúsculas).
    pub nome: &'static str,
    /// `ErrorCode.uniqueName`.
    pub unico: &'static str,
    /// O molde da mensagem, com `{0}`, `{1}`...
    pub mensagem: &'static str,
    /// O molde da correção.
    pub correcao: Option<&'static str>,
    pub tipo: TipoErro,
    pub severidade: Severidade,
    pub documentado: bool,
}

/// Um diagnóstico de arquivo não-Dart.
#[derive(Debug, PartialEq, Eq)]
pub struct Relato {
    pub codigo: &'static CodigoNaoDart,
    pub span: Span,
    pub args: Vec<String>,
}

/// Um molde com os `{n}` trocados pelos argumentos.
fn preencher(molde: &str, args: &[String]) -> String {
    let mut saida = molde.to_string();
    for (i, a) in args.iter().enumerate() {
        saida = saida.replace(&format!("{{{i}}}"), a);
    }
    saida
}

impl Relato {
    pub fn novo(codigo: &'static CodigoNaoDart, span: Span, args: &[&str]) -> Relato {
        Relato { codigo, span, args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// A mensagem do problema.
    pub fn mensagem(&self) -> String {
        preencher(self.codigo.mensagem, &self.args)
    }

    /// A mensagem de correção, se o código tem.
    pub fn correcao(&self) -> Option<String> {
        self.codigo.correcao.map(|c| preencher(c, &self.args))
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
