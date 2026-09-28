//! Mapa de fontes (source map v3), como o `SourceMapBuffer` do dart-sass
//! (`lib/src/util/source_map_buffer.dart`) o monta e o `SingleMapping` do
//! pacote `source_maps` 0.10 o escreve.
//!
//! O serializador escreve num `Vec<u8>`; o [`BufferMapa`] acompanha esse
//! texto por trás (`sincronizar`), contando linhas e colunas em unidades
//! UTF-16 — as de uma `String` do Dart —, e guarda as entradas nos mesmos
//! pontos em que o `SourceMapBuffer` as guarda: no começo de cada `forSpan`
//! e no começo de cada linha escrita dentro de um.
use std::collections::HashMap;

use codemap::{CodeMap, Span};

use crate::sass_builder::{fonte_do_mapa, Ativo};

/// Um ponto do mapa: fonte (URL, linha, coluna) → alvo (linha, coluna,
/// deslocamento). Linhas e colunas começam em 0; colunas contam unidades
/// UTF-16. O deslocamento do alvo é em bytes (só serve para comparar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entrada {
    pub fonte: usize,
    pub fonte_linha: usize,
    pub fonte_coluna: usize,
    pub alvo_linha: usize,
    pub alvo_coluna: usize,
    pub alvo_offset: usize,
}

/// O acompanhamento do texto escrito e as entradas.
#[derive(Debug, Default)]
pub(crate) struct BufferMapa {
    entradas: Vec<Entrada>,
    urls: Vec<String>,
    indices: HashMap<String, usize>,
    linha: usize,
    coluna: usize,
    varrido: usize,
    em_span: bool,
}

impl BufferMapa {
    /// Processa o que o serializador escreveu desde a última vez: cada
    /// `\n` é o `_writeLine` do `SourceMapBuffer`.
    pub fn sincronizar(&mut self, buffer: &[u8]) {
        while self.varrido < buffer.len() {
            let b = buffer[self.varrido];
            self.varrido += 1;
            if b == b'\n' {
                self.escrever_linha();
            } else if b & 0xC0 != 0x80 {
                // Início de caractere UTF-8: 4 bytes são um par substituto
                // no UTF-16.
                self.coluna += if b >= 0xF0 { 2 } else { 1 };
            }
        }
    }

    /// `_writeLine`: descarta a entrada que ficou no fim da linha e, dentro
    /// de um `span`, abre a linha nova apontando para a mesma fonte.
    fn escrever_linha(&mut self) {
        if let Some(u) = self.entradas.last() {
            if u.alvo_linha == self.linha && u.alvo_coluna == self.coluna {
                self.entradas.pop();
            }
        }
        self.linha += 1;
        self.coluna = 0;
        if self.em_span {
            if let Some(u) = self.entradas.last().copied() {
                self.entradas.push(Entrada {
                    alvo_linha: self.linha,
                    alvo_coluna: 0,
                    alvo_offset: self.varrido,
                    ..u
                });
            }
        }
    }

    /// Começo de um `forSpan(span, …)`: `_addEntry(span.start, alvo)`.
    /// Devolve o `_inSpan` anterior, para [`BufferMapa::fim`].
    pub fn inicio(&mut self, buffer: &[u8], mapa: &CodeMap, span: Span) -> bool {
        self.sincronizar(buffer);
        let (fonte, fonte_linha, fonte_coluna) = self.local(mapa, span);
        let nova = Entrada {
            fonte,
            fonte_linha,
            fonte_coluna,
            alvo_linha: self.linha,
            alvo_coluna: self.coluna,
            alvo_offset: self.varrido,
        };
        let redundante = self.entradas.last().is_some_and(|u| {
            // Mesma linha de fonte e de alvo (o arquivo não entra na
            // comparação do dart-sass), ou o mesmo ponto do alvo.
            (u.fonte_linha == nova.fonte_linha && u.alvo_linha == nova.alvo_linha)
                || u.alvo_offset == nova.alvo_offset
        });
        if !redundante {
            self.entradas.push(nova);
        }
        std::mem::replace(&mut self.em_span, true)
    }

    /// Fim do `forSpan`: o texto escrito dentro dele é processado com
    /// `_inSpan` ligado, e o estado anterior volta.
    pub fn fim(&mut self, buffer: &[u8], anterior: bool) {
        self.sincronizar(buffer);
        self.em_span = anterior;
    }

    /// A URL, a linha e a coluna (UTF-16) do começo de `span`.
    fn local(&mut self, mapa: &CodeMap, span: Span) -> (usize, usize, usize) {
        let arquivo = mapa.find_file(span.low());
        let linha = arquivo.find_line(span.low());
        let inicio_linha = arquivo.line_span(linha).low();
        let texto = arquivo.source_slice(arquivo.span.subspan(
            inicio_linha - arquivo.span.low(),
            span.low() - arquivo.span.low(),
        ));
        let coluna = texto.encode_utf16().count();
        let nome = arquivo.name().to_owned();
        let n = self.urls.len();
        let i = *self.indices.entry(nome.clone()).or_insert(n);
        if i == n {
            self.urls.push(nome);
        }
        (i, linha, coluna)
    }

    /// O mapa com o prefixo (`@charset` ou BOM) aplicado, como o
    /// `buildSourceMap(prefix:)`.
    pub fn construir(mut self, buffer: &[u8], prefixo: &str) -> Mapa {
        self.sincronizar(buffer);
        let linhas = prefixo.matches('\n').count();
        let coluna = match prefixo.rfind('\n') {
            Some(i) => prefixo[i + 1..].encode_utf16().count(),
            None => prefixo.encode_utf16().count(),
        };
        let mut entradas = self.entradas;
        for e in &mut entradas {
            if e.alvo_linha == 0 {
                e.alvo_coluna += coluna;
            }
            e.alvo_linha += linhas;
        }
        Mapa {
            urls: self.urls,
            entradas,
        }
    }
}

/// O mapa pronto: as URLs canônicas das fontes e os pontos.
#[derive(Debug, Clone, Default)]
pub struct Mapa {
    pub(crate) urls: Vec<String>,
    pub(crate) entradas: Vec<Entrada>,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn vlq(saida: &mut String, valor: i64) {
    let mut v = if valor < 0 {
        ((-valor) << 1) | 1
    } else {
        valor << 1
    };
    loop {
        let mut digito = (v & 31) as usize;
        v >>= 5;
        if v > 0 {
            digito |= 32;
        }
        saida.push(BASE64[digito] as char);
        if v == 0 {
            break;
        }
    }
}

/// `json.encode` de uma `String` do Dart.
fn json_texto(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

impl Mapa {
    /// O JSON do mapa (`SingleMapping.fromEntries(...).toJson()`), com cada
    /// URL de fonte passada por `fonte`.
    pub fn json(&self, fonte: impl Fn(&str) -> String) -> String {
        // `fromEntries`: as entradas em ordem de alvo; as URLs na ordem em
        // que aparecem.
        let mut entradas = self.entradas.clone();
        entradas.sort_by_key(|e| e.alvo_offset);
        let mut ordem: Vec<usize> = Vec::new();
        let mut novo_indice: HashMap<usize, usize> = HashMap::new();
        for e in &entradas {
            if !novo_indice.contains_key(&e.fonte) {
                novo_indice.insert(e.fonte, ordem.len());
                ordem.push(e.fonte);
            }
        }
        let mut m = String::new();
        let (mut linha, mut coluna, mut fl, mut fc, mut fu) = (0usize, 0i64, 0i64, 0i64, 0i64);
        let mut primeiro = true;
        for e in &entradas {
            if e.alvo_linha > linha {
                for _ in linha..e.alvo_linha {
                    m.push(';');
                }
                linha = e.alvo_linha;
                coluna = 0;
                primeiro = true;
            }
            if !primeiro {
                m.push(',');
            }
            primeiro = false;
            vlq(&mut m, e.alvo_coluna as i64 - coluna);
            coluna = e.alvo_coluna as i64;
            let u = novo_indice[&e.fonte] as i64;
            vlq(&mut m, u - fu);
            fu = u;
            vlq(&mut m, e.fonte_linha as i64 - fl);
            fl = e.fonte_linha as i64;
            vlq(&mut m, e.fonte_coluna as i64 - fc);
            fc = e.fonte_coluna as i64;
        }
        let fontes: Vec<String> = ordem
            .iter()
            .map(|&i| json_texto(&fonte(&self.urls[i])))
            .collect();
        format!(
            "{{\"version\":3,\"sourceRoot\":\"\",\"sources\":[{}],\"names\":[],\"mappings\":{}}}",
            fontes.join(","),
            json_texto(&m)
        )
    }

    /// O JSON do `.css.map` que o `sass_builder` grava.
    pub fn json_sass_builder(&self, entrada: &Ativo) -> String {
        self.json(|u| fonte_do_mapa(u, entrada))
    }
}
