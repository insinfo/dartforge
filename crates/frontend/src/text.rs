//! Dart string text: UTF-16 units, stored as WTF-8.
//!
//! A Dart `String` is a sequence of UTF-16 code units, and the language
//! allows writing units that do not form a Unicode scalar: `'\uD800'` is a
//! valid literal of length 1. Rust's `str` requires valid UTF-8 and has no
//! way to store that. The way out is WTF-8 — generalized UTF-8 in which a
//! lone *surrogate* (U+D800–U+DFFF) is encoded like any other code point, in
//! three bytes. Text without lone surrogates is plain UTF-8, and
//! [`DartStr::as_str`] returns the `&str` without copying.
//!
//! Preserved semantics: the surrogate escape pair D83D+DC6D and
//! `'\u{1F46D}'` are the same string in Dart (two code units, one code
//! point), and here too — the builder merges a high+low pair into a scalar.
//! A high and a low surrogate in adjacent literals also form the pair, by
//! concatenation of units.
use std::fmt;

/// Dart string text in WTF-8. Immutable; built by the lexer.
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DartStr(Box<[u8]>);

/// Accumulates UTF-16 units and scalars in WTF-8, merging surrogate pairs
/// when they arrive in sequence.
#[derive(Default)]
pub struct DartStrBuilder {
    bytes: Vec<u8>,
    /// High surrogate still without a pair; decided when the next unit arrives.
    pending_high: Option<u16>,
}

impl DartStrBuilder {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            pending_high: None,
        }
    }

    /// Appends a Unicode scalar (`char`).
    pub fn push_char(&mut self, c: char) {
        self.flush_pending();
        let mut buf = [0u8; 4];
        self.bytes
            .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }

    /// Appends already valid UTF-8 text.
    pub fn push_str(&mut self, s: &str) {
        self.flush_pending();
        self.bytes.extend_from_slice(s.as_bytes());
    }

    /// Appends a code point from a `\u`/`\x` escape, which may be a surrogate.
    /// Outside the Unicode range returns false and changes nothing.
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

    /// Appends a UTF-16 unit, merging it with the pending high surrogate when
    /// they form a pair.
    pub fn push_unit(&mut self, unit: u16) {
        match (self.pending_high.take(), unit) {
            (Some(high), low @ 0xDC00..=0xDFFF) => {
                let scalar =
                    0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                let c = char::from_u32(scalar).expect("a surrogate pair forms a valid scalar");
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

/// Encodes a lone surrogate in three bytes, as WTF-8 prescribes.
fn push_wtf8_surrogate(bytes: &mut Vec<u8>, unit: u16) {
    let v = u32::from(unit);
    bytes.push(0xE0 | (v >> 12) as u8);
    bytes.push(0x80 | ((v >> 6) & 0x3F) as u8);
    bytes.push(0x80 | (v & 0x3F) as u8);
}

impl DartStr {
    /// Empty text.
    pub fn empty() -> Self {
        DartStr(Box::default())
    }

    /// WTF-8 bytes; valid UTF-8 when there are no lone surrogates.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The `&str` when the text has no lone surrogates — the common case,
    /// without copying. `None` when it does: the caller needs UTF-16.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// True when there are no units.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Length as Dart measures it: UTF-16 units (`'\u{1F46D}'.length == 2`).
    pub fn utf16_len(&self) -> usize {
        self.code_units().count()
    }

    /// UTF-16 units, in order — the observable value of a Dart `String`.
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

    /// Readable text: each lone surrogate becomes U+FFFD. Only for messages.
    pub fn to_string_lossy(&self) -> String {
        match self.as_str() {
            Some(s) => s.to_owned(),
            None => String::from_utf16_lossy(&self.code_units().collect::<Vec<_>>()),
        }
    }
}

/// Continuation byte at `i`, or 0 when absent (does not happen in
/// well-formed WTF-8, the only kind the builder produces).
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
    fn plain_utf8_comes_out_without_copy() {
        let s = DartStr::from("café");
        assert_eq!(s.as_str(), Some("café"));
        assert_eq!(s.utf16_len(), 3);
    }

    #[test]
    fn surrogate_pair_becomes_scalar() {
        let mut b = DartStrBuilder::default();
        assert!(b.push_code_point(0xD83D));
        assert!(b.push_code_point(0xDC6D));
        let s = b.finish();
        assert_eq!(s.as_str(), Some("👭"));
        assert_eq!(s.utf16_len(), 2);
        assert_eq!(s, DartStr::from("\u{1F46D}"));
    }

    #[test]
    fn lone_surrogate_is_kept_and_measured() {
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
    fn high_followed_by_text_does_not_merge() {
        let mut b = DartStrBuilder::default();
        assert!(b.push_code_point(0xD800));
        b.push_str("x");
        let s = b.finish();
        assert_eq!(s.code_units().collect::<Vec<_>>(), vec![0xD800, 0x78]);
    }

    #[test]
    fn outside_unicode_is_rejected() {
        let mut b = DartStrBuilder::default();
        assert!(!b.push_code_point(0x110000));
        assert!(b.finish().is_empty());
    }
}
