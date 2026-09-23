//! O formato JSON v1 do `dart analyze --format=json`, lido do oráculo e
//! emitido por nós campo a campo na mesma ordem.
//!
//! Medido no SDK 3.6.2 (sonda de 7 erros): objeto compacto
//! `{"version":1,"diagnostics":[...]}`; cada diagnóstico tem `code` (o
//! `ErrorCode.name` em minúsculas), `severity`, `type`, `location` com o
//! caminho absoluto do arquivo e o intervalo (`offset` em **unidades UTF-16**,
//! `line` e `column` a partir de 1), `problemMessage`, e — só quando existem —
//! `correctionMessage` e `documentation` (a URL de `hasPublishedDocs`).
//! A ordem é a do `DiagnosticWithPath.compareTo` do dartdev: severidade
//! (erros primeiro), caminho, linha, coluna, mensagem.

use dartforge_diagnostics::{Diagnostic, Severidade};
use serde::{Deserialize, Serialize};

/// O documento inteiro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relatorio {
    pub version: u32,
    pub diagnostics: Vec<DiagJson>,
}

/// Um diagnóstico no JSON v1.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
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

/// Converte offsets de bytes UTF-8 em (offset UTF-16, linha, coluna), como o
/// `LineInfo` do analyzer: quebras são `\n`, `\r\n` e `\r` sozinho.
pub struct Linhas<'a> {
    texto: &'a str,
    /// Byte inicial de cada linha.
    inicios: Vec<usize>,
    /// Offset UTF-16 do início de cada linha.
    inicios16: Vec<usize>,
}

impl<'a> Linhas<'a> {
    pub fn new(texto: &'a str) -> Self {
        let mut inicios = vec![0];
        let mut inicios16 = vec![0];
        let b = texto.as_bytes();
        let mut u16s = 0usize;
        let mut i = 0;
        for (pos, ch) in texto.char_indices() {
            if pos < i {
                continue;
            }
            u16s += ch.len_utf16();
            i = pos + ch.len_utf8();
            let quebra = match ch {
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
            if quebra {
                inicios.push(i);
                inicios16.push(u16s);
            }
        }
        Self { texto, inicios, inicios16 }
    }

    /// Ponto do byte `byte` (limitado ao fim do texto e a uma fronteira de caractere).
    pub fn ponto(&self, byte: usize) -> Ponto {
        let mut byte = byte.min(self.texto.len());
        while !self.texto.is_char_boundary(byte) {
            byte -= 1;
        }
        let linha = self.inicios.partition_point(|&s| s <= byte) - 1;
        let col16: usize = self.texto[self.inicios[linha]..byte].chars().map(char::len_utf16).sum();
        Ponto { offset: self.inicios16[linha] + col16, line: linha + 1, column: col16 + 1 }
    }
}

/// Converte um diagnóstico nosso para o JSON v1. Sem código, o campo `code`
/// sai `dartforge_sem_codigo` (nunca coincide com o oráculo: conta como falso
/// positivo no placar).
pub fn para_json(arquivo: &str, linhas: &Linhas<'_>, d: &Diagnostic, sintaxe: bool) -> DiagJson {
    let (code, tipo, doc) = match d.code {
        Some(c) => {
            let i = c.info();
            (i.nome.to_string(), i.tipo.nome().to_string(), i.url())
        }
        None => {
            let tipo = if sintaxe { "SYNTACTIC_ERROR" } else { "COMPILE_TIME_ERROR" };
            ("dartforge_sem_codigo".to_string(), tipo.to_string(), None)
        }
    };
    DiagJson {
        code,
        severity: d.severity.nome().to_string(),
        tipo,
        location: Local {
            file: arquivo.to_string(),
            range: Faixa { start: linhas.ponto(d.span.start), end: linhas.ponto(d.span.end.max(d.span.start)) },
        },
        problem_message: d.message.clone(),
        correction_message: d.correcao(),
        documentation: doc,
    }
}

fn prioridade(sev: &str) -> u8 {
    match Severidade::por_nome(sev) {
        Some(Severidade::Error) => 0,
        Some(Severidade::Warning) => 1,
        Some(Severidade::Info) => 3,
        None => 4,
    }
}

/// A ordem do dartdev (`DiagnosticWithPath.compareTo`).
pub fn ordenar(v: &mut [DiagJson]) {
    v.sort_by(|a, b| {
        prioridade(&a.severity)
            .cmp(&prioridade(&b.severity))
            .then_with(|| a.location.file.cmp(&b.location.file))
            .then_with(|| a.location.range.start.line.cmp(&b.location.range.start.line))
            .then_with(|| a.location.range.start.column.cmp(&b.location.range.start.column))
            .then_with(|| a.problem_message.cmp(&b.problem_message))
    });
}

/// Serializa como o `dart analyze` (JSON compacto, sem quebra no fim).
pub fn escrever(diags: Vec<DiagJson>) -> String {
    serde_json::to_string(&Relatorio { version: 1, diagnostics: diags }).expect("JSON")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn linhas_utf16_e_quebras() {
        let t = "a\r\nb\u{1F600}c\rd\n";
        let l = Linhas::new(t);
        assert_eq!(l.ponto(0), Ponto { offset: 0, line: 1, column: 1 });
        assert_eq!(l.ponto(3), Ponto { offset: 3, line: 2, column: 1 });
        // depois do emoji (4 bytes, 2 unidades UTF-16)
        assert_eq!(l.ponto(8), Ponto { offset: 6, line: 2, column: 4 });
        assert_eq!(l.ponto(10), Ponto { offset: 8, line: 3, column: 1 });
    }
}
