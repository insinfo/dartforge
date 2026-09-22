//! Texto de string Dart: unidades UTF-16, guardadas em WTF-8.
//!
//! Uma `String` do Dart é uma sequência de unidades de código UTF-16, e a
//! linguagem permite escrever unidades que não formam um escalar Unicode:
//! `'\uD800'` é um literal válido de comprimento 1. `str` do Rust exige UTF-8
//! válido e não tem como guardar isso. A saída é WTF-8 — UTF-8 generalizado
//! em que um *surrogate* solto (U+D800–U+DFFF) é codificado como qualquer
//! outro ponto de código, em três bytes. Texto sem surrogates soltos é UTF-8
//! comum, e [`DartStr::as_str`] devolve o `&str` sem cópia.
//!
//! Semântica preservada: o par de escapes de surrogate D83D+DC6D e
//! `'\u{1F46D}'` são a mesma string em Dart (dois code units, um ponto de
//! código), e aqui também — o construtor funde par alto+baixo num escalar.
//! Um alto e um baixo em literais adjacentes também formam o par, por
//! concatenação de unidades.
use std::fmt;

/// Texto de string Dart em WTF-8. Imutável; construído pelo lexer.
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DartStr(Box<[u8]>);

/// Acumula unidades UTF-16 e escalares em WTF-8, fundindo pares de
/// surrogates quando eles chegam em sequência.
#[derive(Default)]
pub struct DartStrBuilder {
    bytes: Vec<u8>,
    /// Surrogate alto ainda sem par; decide-se ao chegar a próxima unidade.
    pending_high: Option<u16>,
}

impl DartStrBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            pending_high: None,
        }
    }

    /// Acrescenta um escalar Unicode (`char`).
    pub fn push_char(&mut self, c: char) {
        self.flush_pending();
        let mut buf = [0u8; 4];
        self.bytes
            .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }

    /// Acrescenta texto UTF-8 já válido.
    pub fn push_str(&mut self, s: &str) {
        self.flush_pending();
        self.bytes.extend_from_slice(s.as_bytes());
    }

    /// Acrescenta um ponto de código de escape `\u`/`\x`, que pode ser um
    /// surrogate. Fora do intervalo Unicode devolve falso e não altera nada.
    pub fn push_code_point(&mut self, value: u32) -> bool {
        if let Some(c) = char::from_u32(value) {
            self.push_char(c);
            return true;
        }
        let Ok(unit) = u16::try_from(value) else {
            return false;
        };
        if !(0xD800..=0xDFFF).contains(&unit) {
            return false;
        }
        self.push_unit(unit);
        true
    }

    /// Acrescenta uma unidade UTF-16, fundindo com o surrogate alto pendente
    /// quando formam par.
    pub fn push_unit(&mut self, unit: u16) {
        match (self.pending_high.take(), unit) {
            (Some(high), low @ 0xDC00..=0xDFFF) => {
                let scalar =
                    0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                let c = char::from_u32(scalar).expect("par de surrogates forma escalar válido");
                self.push_char(c);
            }
            (high, unit) => {
                if let Some(high) = high {
                    push_wtf8_surrogate(&mut self.bytes, high);
                }
                if (0xD800..=0xDBFF).contains(&unit) {
                    self.pending_high = Some(unit);
                } else if (0xDC00..=0xDFFF).contains(&unit) {
                    push_wtf8_surrogate(&mut self.bytes, unit);
                } else if let Some(c) = char::from_u32(u32::from(unit)) {
                    self.push_char(c);
                }
            }
        }
    }

    fn flush_pending(&mut self) {
        if let Some(high) = self.pending_high.take() {
            push_wtf8_surrogate(&mut self.bytes, high);
        }
    }

    pub fn finish(mut self) -> DartStr {
        self.flush_pending();
        DartStr(self.bytes.into_boxed_slice())
    }
}

/// Codifica um surrogate solto em três bytes, como WTF-8 manda.
fn push_wtf8_surrogate(bytes: &mut Vec<u8>, unit: u16) {
    let v = u32::from(unit);
    bytes.push(0xE0 | (v >> 12) as u8);
    bytes.push(0x80 | ((v >> 6) & 0x3F) as u8);
    bytes.push(0x80 | (v & 0x3F) as u8);
}

impl DartStr {
    /// Texto vazio.
    pub fn empty() -> Self {
        DartStr(Box::default())
    }

    /// Bytes WTF-8; UTF-8 válido quando não há surrogates soltos.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// `&str` quando o texto não contém surrogates soltos — o caso comum,
    /// sem cópia. `None` quando contém: o chamador precisa de UTF-16.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// Verdadeiro sem unidades.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Comprimento como o Dart mede: unidades UTF-16 (`'\u{1F46D}'.length == 2`).
    pub fn utf16_len(&self) -> usize {
        self.code_units().count()
    }

    /// Unidades UTF-16, na ordem — o valor observável de uma `String` Dart.
    pub fn code_units(&self) -> impl Iterator<Item = u16> + '_ {
        let mut i = 0;
        std::iter::from_fn(move || {
            let b = *self.0.get(i)?;
            let (scalar, len) = match b {
                0x00..=0x7F => (u32::from(b), 1),
                0xC0..=0xDF => ((u32::from(b) & 0x1F) << 6 | cont(&self.0, i + 1), 2),
                0xE0..=0xEF => (
                    (u32::from(b) & 0x0F) << 12 | cont(&self.0, i + 1) << 6 | cont(&self.0, i + 2),
                    3,
                ),
                _ => (
                    (u32::from(b) & 0x07) << 18
                        | cont(&self.0, i + 1) << 12
                        | cont(&self.0, i + 2) << 6
                        | cont(&self.0, i + 3),
                    4,
                ),
            };
            i += len;
            Some(scalar)
        })
        .flat_map(|scalar| {
            if scalar >= 0x10000 {
                let v = scalar - 0x10000;
                [
                    Some(0xD800 + (v >> 10) as u16),
                    Some(0xDC00 + (v & 0x3FF) as u16),
                ]
            } else {
                [Some(scalar as u16), None]
            }
        })
        .flatten()
    }

    /// Texto legível: cada surrogate solto vira um U+FFFD. Só para mensagens.
    pub fn to_string_lossy(&self) -> String {
        match self.as_str() {
            Some(s) => s.to_owned(),
            None => String::from_utf16_lossy(&self.code_units().collect::<Vec<_>>()),
        }
    }
}

/// Byte de continuação em `i`, ou 0 quando ausente (não ocorre em WTF-8
/// bem formado, que é o único que o construtor produz).
fn cont(bytes: &[u8], i: usize) -> u32 {
    u32::from(bytes.get(i).copied().unwrap_or(0) & 0x3F)
}

impl From<&str> for DartStr {
    fn from(s: &str) -> Self {
        DartStr(s.as_bytes().into())
    }
}

impl From<String> for DartStr {
    fn from(s: String) -> Self {
        DartStr(s.into_bytes().into_boxed_slice())
    }
}

impl PartialEq<str> for DartStr {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other.as_bytes()
    }
}

impl PartialEq<&str> for DartStr {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == other.as_bytes()
    }
}

impl fmt::Debug for DartStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) => fmt::Debug::fmt(s, f),
            None => write!(f, "DartStr({:?})", self.code_units().collect::<Vec<_>>()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_comum_sai_sem_copia() {
        let s = DartStr::from("olá");
        assert_eq!(s.as_str(), Some("olá"));
        assert_eq!(s.utf16_len(), 3);
    }

    #[test]
    fn par_de_surrogates_vira_escalar() {
        let mut b = DartStrBuilder::default();
        assert!(b.push_code_point(0xD83D));
        assert!(b.push_code_point(0xDC6D));
        let s = b.finish();
        assert_eq!(s.as_str(), Some("👭"));
        assert_eq!(s.utf16_len(), 2);
        assert_eq!(s, DartStr::from("\u{1F46D}"));
    }

    #[test]
    fn surrogate_solto_e_guardado_e_medido() {
        let mut b = DartStrBuilder::default();
        assert!(b.push_code_point(0xD800));
        b.push_char('-');
        assert!(b.push_code_point(0xDBFF));
        let s = b.finish();
        assert_eq!(s.as_str(), None);
        assert_eq!(
            s.code_units().collect::<Vec<_>>(),
            vec![0xD800, 0x2D, 0xDBFF]
        );
        assert_eq!(s.utf16_len(), 3);
        assert_eq!(s.to_string_lossy(), "\u{FFFD}-\u{FFFD}");
    }

    #[test]
    fn alto_seguido_de_texto_nao_funde() {
        let mut b = DartStrBuilder::default();
        assert!(b.push_code_point(0xD800));
        b.push_str("x");
        let s = b.finish();
        assert_eq!(s.code_units().collect::<Vec<_>>(), vec![0xD800, 0x78]);
    }

    #[test]
    fn fora_do_unicode_e_recusado() {
        let mut b = DartStrBuilder::default();
        assert!(!b.push_code_point(0x110000));
        assert!(b.finish().is_empty());
    }
}
