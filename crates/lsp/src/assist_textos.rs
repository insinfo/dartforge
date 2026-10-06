//! Assistências de literais de texto, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to single quoted string` | `refactor.convert.toSingleQuotedString` | `ConvertToSingleQuotes` |
//! | `Convert to double quoted string` | `refactor.convert.toDoubleQuotedString` | `ConvertToDoubleQuotes` |
//! | `Convert to multiline string` | `refactor.convert.toMultilineString` | `ConvertToMultilineString` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::Mudanca;
use dartforge_diagnostics::Span;

/// O texto de um literal: cru (`r`), de várias linhas e com aspas simples.
struct Aspas {
    cru: bool,
    multilinha: bool,
    simples: bool,
}

fn aspas_de(texto: &str) -> Aspas {
    let cru = texto.starts_with('r') || texto.starts_with('R');
    let resto = if cru { &texto[1..] } else { texto };
    Aspas { cru, multilinha: resto.starts_with("'''") || resto.starts_with("\"\"\""), simples: resto.starts_with('\'') }
}

impl Contexto<'_> {
    /// `_ConvertQuotes.compute`: `de_duplas` converte de aspas duplas para
    /// simples (`ConvertToSingleQuotes`); senão, de simples para duplas.
    fn converter_aspas(&self, uri: &str, inicio: usize, fim: usize, de_duplas: bool) -> Option<Mudanca> {
        let no = self.arvore.localizar(inicio, fim)?;
        let mut m = Mudanca::default();
        // O `token` do produtor (`_tokenAt(node, selectionOffset)`).
        let mut token: Option<Span> = None;
        match self.especie(no) {
            "SimpleStringLiteral" => {
                let span = self.arvore.span(no);
                token = Some(span);
                let texto = &self.fonte[span.start..span.end];
                let a = aspas_de(texto);
                if if de_duplas { !a.simples } else { a.simples } {
                    let nova = match (a.multilinha, de_duplas) {
                        (true, true) => "'''",
                        (true, false) => "\"\"\"",
                        (false, true) => "'",
                        (false, false) => "\"",
                    };
                    let tamanho = if a.multilinha { 3 } else { 1 };
                    // `_addBackslash`: antes de cada aspa nova não escapada
                    // (o laço começa no índice 1 do texto do token).
                    let aspa = if de_duplas { b'\'' } else { b'"' };
                    let b = texto.as_bytes();
                    let mut i = 1;
                    while i + 1 < b.len() {
                        if b[i + 1] == aspa && b[i] != b'\\' {
                            m.adicionar(uri, Span { start: span.start + 1 + i, end: span.start + 1 + i }, String::from("\\"));
                        }
                        i += 1;
                    }
                    let abre = span.start + usize::from(a.cru);
                    m.adicionar(uri, Span { start: abre, end: abre + tamanho }, nova.to_string());
                    m.adicionar(uri, Span { start: span.end - tamanho, end: span.end }, nova.to_string());
                }
            }
            "StringInterpolation" | "InterpolationString" => {
                let interpolacao = if self.especie(no) == "InterpolationString" { self.pai(no)? } else { no };
                let span = self.arvore.span(interpolacao);
                let a = aspas_de(&self.fonte[span.start..span.end]);
                token = if self.especie(no) == "InterpolationString" {
                    Some(self.arvore.span(no))
                } else {
                    self.filhos(no).iter().map(|&k| self.arvore.span(k)).find(|s| s.start <= inicio && inicio <= s.end)
                };
                if if de_duplas { !a.simples } else { a.simples } {
                    let nova = match (a.multilinha, de_duplas) {
                        (true, true) => "'''",
                        (true, false) => "\"\"\"",
                        (false, true) => "'",
                        (false, false) => "\"",
                    };
                    let tamanho = if a.multilinha { 3 } else { 1 };
                    let algum_contem = self
                        .filhos(interpolacao)
                        .iter()
                        .filter(|&&k| self.especie(k) == "InterpolationString")
                        .any(|&k| self.texto_do_no(k).contains(nova));
                    if !algum_contem {
                        let abre = span.start + usize::from(a.cru);
                        m.adicionar(uri, Span { start: abre, end: abre + tamanho }, nova.to_string());
                        m.adicionar(uri, Span { start: span.end - tamanho, end: span.end }, nova.to_string());
                    }
                }
            }
            _ => return None,
        }
        // `_removeBackslash(token)`: o `\` antes da aspa antiga.
        if let Some(t) = token {
            let aspa = if de_duplas { b'"' } else { b'\'' };
            let b = &self.fonte.as_bytes()[t.start..t.end];
            let mut i = 0;
            while i + 1 < b.len() {
                if b[i] == b'\\' && b[i + 1] == aspa {
                    m.adicionar(uri, Span { start: t.start + i, end: t.start + i + 1 }, String::new());
                    i += 1;
                }
                i += 1;
            }
        }
        (m.conflito.is_none() && !m.arquivos.is_empty()).then_some(m)
    }

    /// `ConvertToSingleQuotes` e `ConvertToDoubleQuotes` (convert_quotes.dart).
    pub(crate) fn converter_aspas_de_texto(&self, uri: &str, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        for (de_duplas, titulo, especie) in [
            (false, "Convert to double quoted string", "refactor.convert.toDoubleQuotedString"),
            (true, "Convert to single quoted string", "refactor.convert.toSingleQuotedString"),
        ] {
            if let Some(m) = self.converter_aspas(uri, inicio, fim, de_duplas) {
                saida.push(AcaoDeCodigo {
                    titulo: titulo.into(),
                    especie: especie.into(),
                    edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
                    diagnostico: None,
                    criar_arquivo: None,
                });
            }
        }
        saida
    }

    /// `ConvertToMultilineString` (convert_to_multiline_string.dart): o
    /// literal de uma linha ganha as aspas triplas, com uma quebra de linha
    /// depois das de abertura (`writeln`).
    pub(crate) fn converter_em_multilinha(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let mut no = self.arvore.localizar(inicio, fim)?;
        if matches!(self.especie(no), "InterpolationString" | "InterpolationExpression") {
            no = self.pai(no)?;
        }
        if !matches!(self.especie(no), "SimpleStringLiteral" | "StringInterpolation") {
            return None;
        }
        let span = self.arvore.span(no);
        let a = aspas_de(&self.fonte[span.start..span.end]);
        if a.multilinha || span.end <= span.start + 1 {
            return None;
        }
        let nova = if a.simples { "\'\'\'" } else { "\"\"\"" };
        let eol = crate::refatoracoes_exec::Texto::novo(self.fonte).eol();
        let abre = span.start + usize::from(a.cru);
        let mut m = Mudanca::default();
        m.adicionar(uri, Span { start: abre, end: abre + 1 }, format!("{nova}{eol}"));
        m.adicionar(uri, Span { start: span.end - 1, end: span.end }, nova.to_string());
        Some(AcaoDeCodigo {
            titulo: "Convert to multiline string".into(),
            especie: "refactor.convert.toMultilineString".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
