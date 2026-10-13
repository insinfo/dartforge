//! The strings of the unified space (P1, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_OneByteString` (cid 6) and `_TwoByteString` (cid 7) are `BRUTO` blocks: the
//! length at `b+16`, the units from `b+24` on (padded with zeros up to the word)
//! and the hash in `map` (`layout::desl`). The collector does not walk the body;
//! the block does not move while it lives, so the units have a fixed address.
//!
//! Three types:
//!
//! * [`crate::textos::TextoRef`] is the borrowed view of the units (of a heap block
//!   or of a construction [`crate::textos::Texto`]); the search, comparison and
//!   encoding algorithms live on it;
//! * [`crate::textos::Texto`] owns the units before allocating (the output of
//!   `toString`, the pieces the runtime builds); the form is canonical, as in the VM;
//! * [`crate::textos::StringMut`] joins units without going through Rust's `String`
//!   (which would lose lone surrogates).
//!
//! The `impl Heap` functions use only the `heap.rs` API (§3.2); the block's address
//! comes from the handle (`h - 2`), checked live by [`crate::heap::Heap::header`].
#![allow(unsafe_code)]

use crate::heap::Heap;
use crate::layout::{self, Ref, cid, desl, estado, flags};

// ---------------------------------------------------------------------------
// Texto: the owner of the units before allocating.
// ---------------------------------------------------------------------------

/// Dart text: a sequence of UTF-16 code units, in the VM's form (decision 5,
/// docs/NATIVO-PLANO.md §7.1).
///
/// * `Um` is the `_OneByteString`: every unit fits in one byte (Latin-1,
///   U+0000–U+00FF), stored in one byte;
/// * `Dois` is the `_TwoByteString`: some unit exceeds 0xFF, and all are stored in
///   two bytes.
///
/// The form is **canonical**, as in the VM (`String::New`/`String::SubString`,
/// `runtime/vm/object.cc`, choose the class by the content): `Dois` only exists
/// when some unit exceeds 0xFF. The constructors below guarantee it; equality
/// compares units anyway, so a Latin-1 `Dois` created by hand only costs space,
/// never changes the result.
///
/// A lone *surrogate* (U+D800–U+DFFF without a pair) is a unit like any other:
/// `length`, indices, `codeUnitAt`, `substring` in the middle of a pair and `runes`
/// (which returns the lone surrogate's own value) follow Dart semantics. Only on
/// output (`print`) does it become U+FFFD, as the VM writes it (`Utf8::Encode`,
/// through `Dart_CopyUTF8EncodingOfString`).
#[derive(Clone)]
pub enum Texto {
    Um(Vec<u8>),
    Dois(Vec<u16>),
}

impl Texto {
    /// The empty string.
    pub fn empty() -> Self {
        Texto::Um(Vec::new())
    }

    /// Text from UTF-16 units, in canonical form.
    pub fn from_units(unidades: Vec<u16>) -> Self {
        if unidades.iter().all(|&u| u <= 0xFF) {
            Texto::Um(unidades.into_iter().map(|u| u as u8).collect())
        } else {
            Texto::Dois(unidades)
        }
    }

    /// Text from a slice of UTF-16 units, in canonical form.
    pub fn from_slice(unidades: &[u16]) -> Self {
        if unidades.iter().all(|&u| u <= 0xFF) {
            Texto::Um(unidades.iter().map(|&u| u as u8).collect())
        } else {
            Texto::Dois(unidades.to_vec())
        }
    }

    /// Text from valid UTF-8 (text the runtime itself formatted).
    pub fn de_str(s: &str) -> Self {
        if s.is_ascii() {
            return Texto::Um(s.as_bytes().to_vec());
        }
        let mut one = Vec::with_capacity(s.len());
        for c in s.chars() {
            if (c as u32) <= 0xFF {
                one.push(c as u32 as u8);
            } else {
                return Texto::Dois(s.encode_utf16().collect());
            }
        }
        Texto::Um(one)
    }

    /// Text from WTF-8: generalized UTF-8 in which a lone surrogate is encoded in
    /// three bytes like any code point (the form in which the front end keeps the
    /// text of literals, `frontend/src/text.rs`). Valid UTF-8 is WTF-8. A malformed
    /// byte becomes U+FFFD — it does not happen for bytes emitted by the compiler.
    pub fn de_wtf8(bytes: &[u8]) -> Self {
        if bytes.is_ascii() {
            return Texto::Um(bytes.to_vec());
        }
        let mut unidades: Vec<u16> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let b0 = u32::from(bytes[i]);
            let cont = |k: usize| -> Option<u32> {
                bytes.get(i + k).filter(|&&b| b & 0xC0 == 0x80).map(|&b| u32::from(b & 0x3F))
            };
            let (point, n) = if b0 < 0x80 {
                (Some(b0), 1)
            } else if b0 & 0xE0 == 0xC0 {
                (cont(1).map(|c1| ((b0 & 0x1F) << 6) | c1).filter(|&p| p >= 0x80), 2)
            } else if b0 & 0xF0 == 0xE0 {
                (
                    cont(1).zip(cont(2)).map(|(c1, c2)| ((b0 & 0x0F) << 12) | (c1 << 6) | c2).filter(|&p| p >= 0x800),
                    3,
                )
            } else if b0 & 0xF8 == 0xF0 {
                (
                    cont(1)
                        .zip(cont(2))
                        .zip(cont(3))
                        .map(|((c1, c2), c3)| ((b0 & 0x07) << 18) | (c1 << 12) | (c2 << 6) | c3)
                        .filter(|&p| (0x10000..=0x10FFFF).contains(&p)),
                    4,
                )
            } else {
                (None, 1)
            };
            match point {
                Some(p) => {
                    push_point(&mut unidades, p);
                    i += n;
                }
                None => {
                    unidades.push(0xFFFD);
                    i += 1;
                }
            }
        }
        Self::from_units(unidades)
    }

    /// The view of the units of this construction text.
    pub fn vista(&self) -> TextoRef<'_> {
        match self {
            Texto::Um(b) => TextoRef::Um(b),
            Texto::Dois(u) => TextoRef::Dois(u),
        }
    }

    /// Number of UTF-16 units (`String.length`).
    pub fn len(&self) -> usize {
        self.vista().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Is it a `_OneByteString` (Latin-1)?
    pub fn e_um_byte(&self) -> bool {
        matches!(self, Texto::Um(_))
    }

    /// The unit at the index (`codeUnitAt`); the caller checks the index.
    pub fn unit(&self, i: usize) -> u16 {
        self.vista().unit(i)
    }

    /// The units, in order.
    pub fn unidades(&self) -> UnitsIter<'_> {
        self.vista().unidades()
    }

    /// The units in a vector.
    pub fn para_vec(&self) -> Vec<u16> {
        match self {
            Texto::Um(b) => b.iter().map(|&x| u16::from(x)).collect(),
            Texto::Dois(u) => u.clone(),
        }
    }

    /// `[start, end)` in units, in canonical form (the VM's `_substringUnchecked`:
    /// a Latin-1 piece of a `_TwoByteString` goes back to being `Um`).
    pub fn piece(&self, start: usize, end: usize) -> Texto {
        self.vista().piece(start, end).to_string_fn()
    }

    /// The code points (`runes`): a surrogate pair becomes the scalar; a lone
    /// surrogate comes out as itself (`dart:core`'s `RuneIterator`).
    pub fn points(&self) -> Vec<u32> {
        self.vista().points()
    }

    /// WTF-8 (a lone surrogate in three bytes): the lossless form, the inverse of
    /// [`Texto::de_wtf8`].
    pub fn to_wtf8(&self) -> Vec<u8> {
        self.vista().to_wtf8()
    }

    /// UTF-8 as the VM writes it on output (`print`): a lone surrogate becomes
    /// U+FFFD (`Utf8::Encode`, `runtime/vm/unicode.cc`).
    pub fn to_vm_utf8(&self) -> Vec<u8> {
        self.vista().to_vm_utf8()
    }

    /// Text readable by Rust: a lone surrogate becomes U+FFFD. Only for messages and
    /// internal formatting; the Dart value never goes through here.
    pub fn para_string(&self) -> String {
        self.vista().para_string()
    }

    /// Concatenation (`+`), in canonical form.
    pub fn concatenate(&self, other_one: &Texto) -> Texto {
        match (self, other_one) {
            (Texto::Um(a), Texto::Um(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(a);
                v.extend_from_slice(b);
                Texto::Um(v)
            }
            _ => {
                let mut v = Vec::with_capacity(self.len() + other_one.len());
                v.extend(self.unidades());
                v.extend(other_one.unidades());
                Texto::Dois(v)
            }
        }
    }

    /// First occurrence of `pattern` from `since` on (units).
    pub fn search_for(&self, pattern: &Texto, since: usize) -> Option<usize> {
        self.vista().search_for(pattern.vista(), since)
    }

    /// Last occurrence of `pattern` that starts at `until` or before.
    pub fn find_last(&self, pattern: &Texto, until: usize) -> Option<usize> {
        self.vista().find_last(pattern.vista(), until)
    }

    /// `pattern` appears whole from unit `i` on.
    pub fn matches_at(&self, pattern: &Texto, i: usize) -> bool {
        self.vista().matches_at(pattern.vista(), i)
    }

    /// Lexicographic order by units (`String.compareTo`).
    pub fn compare(&self, other_one: &Texto) -> std::cmp::Ordering {
        self.vista().compare(other_one.vista())
    }

    /// The VM's `String.hashCode` ([`layout::hash_de_texto`]), as an `i64` (the form
    /// the old API uses).
    pub fn hash_vm(&self) -> i64 {
        i64::from(self.vista().hash_vm())
    }

}

/// One unit (a point ≤ 0xFFFF, surrogates included) or a surrogate pair.
pub fn push_point(unidades: &mut Vec<u16>, point: u32) {
    if point <= 0xFFFF {
        unidades.push(point as u16);
    } else {
        let p = point - 0x10000;
        unidades.push(0xD800 + (p >> 10) as u16);
        unidades.push(0xDC00 + (p & 0x3FF) as u16);
    }
}

/// A code point in WTF-8 (a surrogate in three bytes, like any point).
fn encode_wtf8(out: &mut Vec<u8>, p: u32) {
    if p < 0x80 {
        out.push(p as u8);
    } else if p < 0x800 {
        out.extend_from_slice(&[0xC0 | (p >> 6) as u8, 0x80 | (p & 0x3F) as u8]);
    } else if p < 0x10000 {
        out.extend_from_slice(&[0xE0 | (p >> 12) as u8, 0x80 | ((p >> 6) & 0x3F) as u8, 0x80 | (p & 0x3F) as u8]);
    } else {
        out.extend_from_slice(&[
            0xF0 | (p >> 18) as u8,
            0x80 | ((p >> 12) & 0x3F) as u8,
            0x80 | ((p >> 6) & 0x3F) as u8,
            0x80 | (p & 0x3F) as u8,
        ]);
    }
}

impl PartialEq for Texto {
    fn eq(&self, other_one: &Texto) -> bool {
        self.vista() == other_one.vista()
    }
}

impl Eq for Texto {}

impl Default for Texto {
    fn default() -> Self {
        Texto::empty()
    }
}

impl PartialEq<str> for Texto {
    fn eq(&self, other_one: &str) -> bool {
        self.vista() == *other_one
    }
}

impl PartialEq<&str> for Texto {
    fn eq(&self, other_one: &&str) -> bool {
        self.vista() == **other_one
    }
}

impl From<&str> for Texto {
    fn from(s: &str) -> Self {
        Texto::de_str(s)
    }
}

impl From<String> for Texto {
    fn from(s: String) -> Self {
        Texto::de_str(&s)
    }
}

impl std::fmt::Display for Texto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.para_string())
    }
}

impl std::fmt::Debug for Texto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.para_string())
    }
}

/// Text builder by units: the output of `toString`/interpolation, which cannot go
/// through Rust's `String` without losing lone surrogates.
#[derive(Default)]
pub struct StringMut(pub Vec<u16>);

impl StringMut {
    pub fn new() -> Self {
        StringMut(Vec::new())
    }
    pub fn push_str(&mut self, s: &str) {
        self.0.extend(s.encode_utf16());
    }
    pub fn push(&mut self, c: char) {
        let mut buf = [0u16; 2];
        self.0.extend_from_slice(c.encode_utf16(&mut buf));
    }
    pub fn push_string(&mut self, t: &Texto) {
        self.push_vista(t.vista());
    }
    /// Appends the units of a view (of a heap block or of a `Texto`).
    pub fn push_vista(&mut self, t: TextoRef<'_>) {
        match t {
            TextoRef::Um(b) => self.0.extend(b.iter().map(|&x| u16::from(x))),
            TextoRef::Dois(u) => self.0.extend_from_slice(u),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn end(self) -> Texto {
        Texto::from_units(self.0)
    }
}

// ---------------------------------------------------------------------------
// TextoRef: the borrowed view.
// ---------------------------------------------------------------------------

/// The view of a string's units: one byte per unit (`_OneByteString`) or two
/// (`_TwoByteString`).
///
/// Borrowed from a heap block ([`Heap::text`]) it is valid while the heap borrow
/// lasts (no allocation in the middle); from a [`Texto`], while it lives.
#[derive(Clone, Copy)]
pub enum TextoRef<'a> {
    Um(&'a [u8]),
    Dois(&'a [u16]),
}

impl<'a> TextoRef<'a> {
    /// How many units.
    pub fn len(self) -> usize {
        match self {
            TextoRef::Um(b) => b.len(),
            TextoRef::Dois(u) => u.len(),
        }
    }
    /// No units?
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// The one-byte-per-unit form?
    pub fn e_um_byte(self) -> bool {
        matches!(self, TextoRef::Um(_))
    }
    /// Does every unit fit in one byte (the canonical form of a copy is `Um`)?
    pub fn fits_in_one_byte(self) -> bool {
        match self {
            TextoRef::Um(_) => true,
            TextoRef::Dois(u) => u.iter().all(|&x| x <= 0xFF),
        }
    }
    /// Unit `i`.
    pub fn unit(self, i: usize) -> u16 {
        match self {
            TextoRef::Um(b) => u16::from(b[i]),
            TextoRef::Dois(u) => u[i],
        }
    }
    /// The units, in order.
    pub fn unidades(self) -> UnitsIter<'a> {
        UnitsIter { text: self, i: 0, end: self.len() }
    }
    /// The units `start..end`.
    pub fn piece(self, start: usize, end: usize) -> TextoRef<'a> {
        match self {
            TextoRef::Um(b) => TextoRef::Um(&b[start..end]),
            TextoRef::Dois(u) => TextoRef::Dois(&u[start..end]),
        }
    }
    /// A construction copy, in canonical form.
    pub fn to_string_fn(self) -> Texto {
        match self {
            TextoRef::Um(b) => Texto::Um(b.to_vec()),
            TextoRef::Dois(u) => Texto::from_slice(u),
        }
    }
    /// The text in UTF-8, with lone surrogates replaced by U+FFFD.
    pub fn para_string(self) -> String {
        match self {
            TextoRef::Um(b) => b.iter().map(|&x| char::from(x)).collect(),
            TextoRef::Dois(u) => String::from_utf16_lossy(u),
        }
    }
    /// The text in WTF-8 (lone surrogates preserved), the inverse of
    /// [`Texto::de_wtf8`].
    pub fn to_wtf8(self) -> Vec<u8> {
        self.encode(false)
    }
    /// The UTF-8 that the VM writes (`Utf8::Encode`): a lone surrogate becomes U+FFFD.
    pub fn to_vm_utf8(self) -> Vec<u8> {
        self.encode(true)
    }
    fn encode(self, swap_released: bool) -> Vec<u8> {
        if let TextoRef::Um(b) = self
            && b.is_ascii()
        {
            return b.to_vec();
        }
        let mut out = Vec::with_capacity(self.len() + 8);
        for p in self.points() {
            let p = if swap_released && (0xD800..0xE000).contains(&p) { 0xFFFD } else { p };
            encode_wtf8(&mut out, p);
        }
        out
    }
    /// The code points (`runes`): a surrogate pair becomes the scalar; a lone
    /// surrogate comes out as itself (`dart:core`'s `RuneIterator`).
    pub fn points(self) -> Vec<u32> {
        if let TextoRef::Um(b) = self {
            return b.iter().map(|&x| u32::from(x)).collect();
        }
        let n = self.len();
        let mut out = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let u = self.unit(i);
            if (0xD800..0xDC00).contains(&u) && i + 1 < n {
                let v = self.unit(i + 1);
                if (0xDC00..0xE000).contains(&v) {
                    out.push(0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(v) - 0xDC00));
                    i += 2;
                    continue;
                }
            }
            out.push(u32::from(u));
            i += 1;
        }
        out
    }
    /// The first occurrence of `pattern` from `since` on.
    pub fn search_for(self, pattern: TextoRef<'_>, since: usize) -> Option<usize> {
        let (n, m) = (self.len(), pattern.len());
        if since > n {
            return None;
        }
        if m == 0 {
            return Some(since);
        }
        if m > n {
            return None;
        }
        let maximum = n - m;
        // Latin-1 on both: the first byte filters the positions.
        if let (TextoRef::Um(a), TextoRef::Um(b)) = (self, pattern) {
            let first = b[0];
            let mut i = since;
            while i <= maximum {
                match a[i..=maximum].iter().position(|&x| x == first) {
                    None => return None,
                    Some(k) => i += k,
                }
                if a[i..i + m] == *b {
                    return Some(i);
                }
                i += 1;
            }
            return None;
        }
        (since..=maximum).find(|&i| self.matches_at(pattern, i))
    }
    /// The last occurrence of `pattern` that starts up to `until`.
    pub fn find_last(self, pattern: TextoRef<'_>, until: usize) -> Option<usize> {
        let (n, m) = (self.len(), pattern.len());
        if m > n {
            return None;
        }
        let start = until.min(n - m);
        (0..=start).rev().find(|&i| self.matches_at(pattern, i))
    }
    /// Does `pattern` occur at position `i`?
    pub fn matches_at(self, pattern: TextoRef<'_>, i: usize) -> bool {
        let m = pattern.len();
        if i.checked_add(m).is_none_or(|f| f > self.len()) {
            return false;
        }
        match (self, pattern) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a[i..i + m] == *b,
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a[i..i + m] == *b,
            _ => (0..m).all(|k| self.unit(i + k) == pattern.unit(k)),
        }
    }
    /// The `compareTo` order (unit by unit, then the length).
    pub fn compare(self, other_one: TextoRef<'_>) -> std::cmp::Ordering {
        match (self, other_one) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a.cmp(b),
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a.cmp(b),
            _ => self.unidades().cmp(other_one.unidades()),
        }
    }
    /// The VM's `hashCode` ([`layout::hash_de_texto`]): 30 bits, never 0.
    pub fn hash_vm(self) -> u32 {
        layout::hash_de_texto(self.unidades())
    }
}

impl PartialEq for TextoRef<'_> {
    fn eq(&self, other_one: &TextoRef<'_>) -> bool {
        match (*self, *other_one) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a == b,
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a == b,
            (a, b) => a.len() == b.len() && a.unidades().eq(b.unidades()),
        }
    }
}

impl Eq for TextoRef<'_> {}

impl PartialEq<Texto> for TextoRef<'_> {
    fn eq(&self, other_one: &Texto) -> bool {
        *self == other_one.vista()
    }
}

impl PartialEq<str> for TextoRef<'_> {
    fn eq(&self, other_one: &str) -> bool {
        if let TextoRef::Um(b) = *self
            && other_one.is_ascii()
        {
            return b == other_one.as_bytes();
        }
        self.unidades().eq(other_one.encode_utf16())
    }
}

impl PartialEq<&str> for TextoRef<'_> {
    fn eq(&self, other_one: &&str) -> bool {
        *self == **other_one
    }
}

impl std::fmt::Display for TextoRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.para_string())
    }
}

impl std::fmt::Debug for TextoRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.para_string())
    }
}

/// Iterator over the units of a [`TextoRef`] (and of a [`Texto`]).
#[derive(Clone)]
pub struct UnitsIter<'a> {
    text: TextoRef<'a>,
    i: usize,
    end: usize,
}

impl Iterator for UnitsIter<'_> {
    type Item = u16;
    fn next(&mut self) -> Option<u16> {
        if self.i >= self.end {
            return None;
        }
        let u = self.text.unit(self.i);
        self.i += 1;
        Some(u)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.i;
        (n, Some(n))
    }
}

impl DoubleEndedIterator for UnitsIter<'_> {
    fn next_back(&mut self) -> Option<u16> {
        if self.i >= self.end {
            return None;
        }
        self.end -= 1;
        Some(self.text.unit(self.end))
    }
}

impl ExactSizeIterator for UnitsIter<'_> {}

/// Detaches the view from the borrow it came from.
///
/// # Safety
/// The units stay valid while the view is used: they come from a heap block that is
/// alive (rooted) and no collection happens in the middle (the collector does not
/// move, so only the block's death invalidates them).
unsafe fn disable<'b>(t: TextoRef<'_>) -> TextoRef<'b> {
    match t {
        // SAFETY: contract above.
        TextoRef::Um(b) => TextoRef::Um(unsafe { std::slice::from_raw_parts(b.as_ptr(), b.len()) }),
        TextoRef::Dois(u) => TextoRef::Dois(unsafe { std::slice::from_raw_parts(u.as_ptr(), u.len()) }),
    }
}

// ---------------------------------------------------------------------------
// The strings on the heap.
// ---------------------------------------------------------------------------

/// The cid of the string in the requested form.
const fn string_cid(dois: bool) -> i32 {
    if dois { cid::TWO_BYTE_STRING } else { cid::ONE_BYTE_STRING }
}

impl Heap {
    /// The address of the block and the cid of string `h`; `None` if `h` is not a
    /// string. N4 panic (from [`Heap::header`]) for a dead or invalid object.
    fn string_block(&self, h: Ref) -> Option<(usize, i32)> {
        if !layout::is_object(h) {
            return None;
        }
        let c = self.header(h).class_id;
        cid::is_string(c).then_some(((h - layout::DESLOCAMENTO_DO_HANDLE) as usize, c))
    }

    /// The units of string `h`; `None` if `h` is not a string.
    pub fn text(&self, h: Ref) -> Option<TextoRef<'_>> {
        let (b, c) = self.string_block(h)?;
        // SAFETY: a live (or static) string block: the length at `b+16` and the
        // units from `b+24` on, aligned to 8, within the body. The borrow of `self`
        // prevents collection while the view lives.
        unsafe {
            let len = *((b + desl::COMPRIMENTO) as *const i64) as usize;
            let p = (b + desl::UNIDADES) as *const u8;
            Some(if c == cid::ONE_BYTE_STRING {
                TextoRef::Um(std::slice::from_raw_parts(p, len))
            } else {
                TextoRef::Dois(std::slice::from_raw_parts(p.cast::<u16>(), len))
            })
        }
    }

    /// Is `h` a string (cid 6 or 7)?
    pub fn is_string(&self, h: Ref) -> bool {
        self.string_block(h).is_some()
    }

    /// A new string of `len` zeroed units (in the requested form). May collect: the
    /// references the caller holds need roots.
    pub fn new_string(&mut self, len: usize, dois: bool) -> Ref {
        let w = layout::palavras_de_texto(len, dois);
        let h = self.allocate(string_cid(dois), w, flags::BRUTO);
        self.words_mut(h)[0] = len as i64;
        h
    }

    /// The units of a `_OneByteString` not yet published.
    ///
    /// # Panics
    /// If `h` is not a `_OneByteString`.
    pub fn one_units_mut(&mut self, h: Ref) -> &mut [u8] {
        let (_, c) = self.string_block(h).expect("compiler bug: _OneByteString expected");
        assert_eq!(c, cid::ONE_BYTE_STRING, "compiler bug: _OneByteString expected");
        let len = self.words(h)[0] as usize;
        &mut self.bytes_mut(h)[8..8 + len]
    }

    /// The units of a `_TwoByteString` not yet published.
    ///
    /// # Panics
    /// If `h` is not a `_TwoByteString`.
    pub fn two_units_mut(&mut self, h: Ref) -> &mut [u16] {
        let (_, c) = self.string_block(h).expect("compiler bug: _TwoByteString expected");
        assert_eq!(c, cid::TWO_BYTE_STRING, "compiler bug: _TwoByteString expected");
        let len = self.words(h)[0] as usize;
        let bytes = &mut self.bytes_mut(h)[8..8 + 2 * len];
        // SAFETY: the bytes of the units start at `b+24` (aligned to 8) and are
        // `2·len` bytes long; the `u16` slice covers exactly them.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<u16>(), len) }
    }

    /// Writes the units of `t` from unit `pos` of string `dest` on (not yet
    /// published). In a `_OneByteString` each unit of `t` must fit in one byte (the
    /// destination's form was chosen by whoever allocated it).
    pub fn write_string(&mut self, dest: Ref, pos: usize, t: TextoRef<'_>) {
        let (_, c) = self.string_block(dest).expect("compiler bug: string expected");
        let n = t.len();
        if c == cid::ONE_BYTE_STRING {
            let d = &mut self.one_units_mut(dest)[pos..pos + n];
            match t {
                TextoRef::Um(b) => d.copy_from_slice(b),
                TextoRef::Dois(u) => {
                    for (x, &y) in d.iter_mut().zip(u) {
                        debug_assert!(y <= 0xFF, "unit {y:#x} in a _OneByteString");
                        *x = y as u8;
                    }
                }
            }
        } else {
            let d = &mut self.two_units_mut(dest)[pos..pos + n];
            match t {
                TextoRef::Um(b) => {
                    for (x, &y) in d.iter_mut().zip(b) {
                        *x = u16::from(y);
                    }
                }
                TextoRef::Dois(u) => d.copy_from_slice(u),
            }
        }
    }

    /// Copies the units `start..end` of string `source` to position `pos` of string
    /// `dest` (another one, not yet published), without an intermediate copy.
    ///
    /// # Panics
    /// If `source` is not a string or the range is outside it.
    pub fn copy_string(&mut self, dest: Ref, pos: usize, source: Ref, start: usize, end: usize) {
        let t = self.text(source).expect("compiler bug: string expected").piece(start, end);
        // SAFETY: `source` is alive (the caller has it) and `write_string` neither
        // allocates nor collects; `dest` is another block, so the slices do not
        // overlap.
        let t = unsafe { disable(t) };
        debug_assert_ne!(source, dest);
        self.write_string(dest, pos, t);
    }

    /// A string with the units of `t`, in canonical form (one byte if it fits). `t`
    /// must not come from this heap (the allocation may collect): for a slice of a
    /// live string use [`Heap::string_slice`].
    pub fn allocate_string(&mut self, t: TextoRef<'_>) -> Ref {
        let h = self.new_string(t.len(), !t.fits_in_one_byte());
        self.write_string(h, 0, t);
        h
    }

    /// The units `start..end` of string `h` (rooted by the caller) in a new string,
    /// in canonical form (the VM's `_substringUnchecked`).
    pub fn string_slice(&mut self, h: Ref, start: usize, end: usize) -> Ref {
        let one = self.text(h).expect("compiler bug: string expected").piece(start, end).fits_in_one_byte();
        let r = self.new_string(end - start, !one);
        self.copy_string(r, 0, h, start, end);
        r
    }

    /// The concatenation of the strings `part_list` (rooted by the caller), in a
    /// single allocation, in canonical form. An element that is not a string is a
    /// compiler bug (panic).
    pub fn join_strings(&mut self, part_list: &[Ref]) -> Ref {
        let (mut total, mut one) = (0usize, true);
        for &p in part_list {
            let t = self.text(p).expect("compiler bug: string expected");
            total += t.len();
            one &= t.fits_in_one_byte();
        }
        let r = self.new_string(total, !one);
        let mut pos = 0;
        for &p in part_list {
            let n = self.text(p).map_or(0, TextoRef::len);
            self.copy_string(r, pos, p, 0, n);
            pos += n;
        }
        r
    }

    /// A string with the text of `s`.
    pub fn allocate_str(&mut self, s: &str) -> Ref {
        if s.is_ascii() {
            return self.allocate_string(TextoRef::Um(s.as_bytes()));
        }
        let t = Texto::de_str(s);
        self.allocate_string(t.vista())
    }

    /// A string from WTF-8.
    pub fn string_from_wtf8(&mut self, bytes: &[u8]) -> Ref {
        if bytes.is_ascii() {
            return self.allocate_string(TextoRef::Um(bytes));
        }
        let t = Texto::de_wtf8(bytes);
        self.allocate_string(t.vista())
    }

    /// The `hashCode` of string `h`, computed on the first query and written into
    /// `map`; `None` if `h` is not a string. A static one (`PERMANENTE`) already comes
    /// with the emitter's hash and is never written (it lives in read-only memory).
    pub fn hash_de_texto(&self, h: Ref) -> Option<u32> {
        let (b, _) = self.string_block(h)?;
        let p = (b + desl::HASH_DO_TEXTO) as *mut u32;
        // SAFETY: `map` of a live block's header; the heap belongs to a single
        // thread and no reference to the header is alive here. Through the
        // `AtomicU32` the write through `&self` is a cache without a shared
        // reference to the same `u32`.
        let cell = unsafe { std::sync::atomic::AtomicU32::from_ptr(p) };
        let current = cell.load(std::sync::atomic::Ordering::Relaxed);
        if current != 0 {
            return Some(current);
        }
        let x = self.text(h)?.hash_vm();
        // SAFETY: `b` is the block (the header's address).
        if unsafe { *(b as *const u8) } != estado::PERMANENTE {
            cell.store(x, std::sync::atomic::Ordering::Relaxed);
        }
        Some(x)
    }

    /// Do the two strings have the same units? `false` if either is not a string.
    pub fn strings_equal(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return layout::is_object(a) && self.is_string(a);
        }
        let (Some(ta), Some(tb)) = (self.text(a), self.text(b)) else { return false };
        if ta.len() != tb.len() {
            return false;
        }
        // The already computed hashes differ: different units.
        let hash = |h: Ref| -> u32 {
            // SAFETY: `map` of a live string block, only read.
            unsafe { *((h - layout::DESLOCAMENTO_DO_HANDLE) as usize as *const u32).byte_add(desl::HASH_DO_TEXTO) }
        };
        let (ha, hb) = (hash(a), hash(b));
        if ha != 0 && hb != 0 && ha != hb {
            return false;
        }
        ta == tb
    }

    /// The canonical string with the units of `t` (interned in `literals`, a
    /// permanent root: the literal of the JIT and of the runtime). `t` does not come
    /// from this heap.
    pub fn string_literal(&mut self, t: TextoRef<'_>) -> Ref {
        // An image's static literal with the same content is the canonical one
        // (§2.11): `identical(s[0], 'a')` and the interpolated `const`.
        if let Some(h) = string_static(t) {
            return h;
        }
        let key: Vec<u16> = t.unidades().collect();
        if let Some(h) = self.literal(&key) {
            return h;
        }
        let h = self.allocate_string(t);
        self.store_literal(key, h);
        h
    }
}

// ---------------------------------------------------------------------------
// The index of the images' static literals.
// ---------------------------------------------------------------------------

/// The static literals (`@df.s.*`, §2.11) of the registered images, by the hash
/// written in the header: `(images already walked, hash → blocks)`. Built on demand
/// (the first canonicalization after a new image), reading only the header words —
/// the hash comes precomputed by the emitter.
static STRING_STATICS: std::sync::RwLock<Option<(usize, std::collections::HashMap<u32, Vec<usize>>)>> =
    std::sync::RwLock::new(None);

/// The view of a static string block (`PERMANENTE`, from an image).
///
/// # Safety
/// `b` is the block of a valid static string.
unsafe fn static_view<'a>(b: usize) -> TextoRef<'a> {
    // SAFETY: contract above; the image is not unloaded.
    unsafe {
        let c = *((b + desl::CLASSE) as *const i32);
        let len = *((b + desl::COMPRIMENTO) as *const i64) as usize;
        let p = (b + desl::UNIDADES) as *const u8;
        if c == cid::ONE_BYTE_STRING {
            TextoRef::Um(std::slice::from_raw_parts(p, len))
        } else {
            TextoRef::Dois(std::slice::from_raw_parts(p.cast::<u16>(), len))
        }
    }
}

/// Walks the static section `[start, end)`: zero words (the markers, the padding
/// between the objects' contributions) are skipped; a word that is not the header of
/// a coherent static string ends the walk.
fn index_image(start: usize, end: usize, ix: &mut std::collections::HashMap<u32, Vec<usize>>) {
    let mut b = start.next_multiple_of(8);
    while b + layout::TAMANHO_DO_CABECALHO + 8 <= end {
        // SAFETY: `b` is within the image's registered range.
        let word = unsafe { *(b as *const u64) };
        if word == 0 {
            b += 8;
            continue;
        }
        let est = word as u8;
        let fl = (word >> 8) as u8;
        let c = (word >> 32) as i32;
        if est != estado::PERMANENTE || fl & flags::FORMA != flags::BRUTO || !cid::is_string(c) {
            break;
        }
        // SAFETY: a static string header: the hash and the length follow.
        let (hash, len) = unsafe { (*((b + desl::HASH_DO_TEXTO) as *const u32), *((b + desl::COMPRIMENTO) as *const i64)) };
        let w = layout::palavras_de_texto(len as usize, c == cid::TWO_BYTE_STRING);
        let size = layout::TAMANHO_DO_CABECALHO + 8 * w;
        if len < 0 || b + size > end {
            break;
        }
        ix.entry(hash).or_default().push(b);
        b += size;
    }
}

/// The static literal with the units of `t`, if some registered image has it.
/// Between images with the same literal (the executable and the SDK DLL in the
/// development link, §4.10 item 52), the one registered last wins — the program,
/// which registers after the SDK.
pub fn string_static(t: TextoRef<'_>) -> Option<Ref> {
    let images = crate::heap::images();
    let ready_flag = STRING_STATICS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|(n, _)| *n == images.len());
    if !ready_flag {
        let mut g = STRING_STATICS.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (n, ix) = g.get_or_insert_with(|| (0, std::collections::HashMap::new()));
        for &(i, f) in &images[*n..] {
            index_image(i, f, ix);
        }
        *n = images.len();
    }
    let g = STRING_STATICS.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_, ix) = g.as_ref()?;
    let hash = t.hash_vm();
    ix.get(&hash)?.iter().rev().find_map(|&b| {
        // SAFETY: an indexed block of a static string.
        (unsafe { static_view(b) } == t).then_some(b as Ref + layout::DESLOCAMENTO_DO_HANDLE)
    })
}

impl Heap {
    /// The canonical form of a constant (the `const` getter, `constantes.rs`): a
    /// string becomes the canonical literal with the same content (the image's static
    /// one or the interned one); the rest comes back as it is.
    pub fn canonical_constant(&mut self, h: Ref) -> Ref {
        if !self.is_string(h) || crate::heap::is_static(h) {
            return h;
        }
        let t = self.text(h).expect("conferido").to_string_fn();
        self.string_literal(t.vista())
    }
}

#[cfg(test)]
mod tests_strings {
    use super::*;

    #[test]
    fn strings_canonical_form_and_slice() {
        assert!(Texto::de_str("abc").e_um_byte());
        assert!(Texto::de_str("ç").e_um_byte());
        assert!(!Texto::de_str("€").e_um_byte());
        let t = Texto::de_str("a€b");
        assert!(t.piece(2, 3).e_um_byte());
        assert_eq!(t.vista().piece(0, 1), "a");
        assert!(!TextoRef::Dois(&[0x61, 0x100]).fits_in_one_byte());
        assert!(TextoRef::Dois(&[0x61, 0xFF]).fits_in_one_byte());
        assert!(TextoRef::Dois(&[0x61, 0xFF]).to_string_fn().e_um_byte());
    }

    #[test]
    fn strings_equality_across_forms() {
        let one = TextoRef::Um(b"ab");
        let dois = TextoRef::Dois(&[0x61, 0x62]);
        assert_eq!(one, dois);
        assert_eq!(dois, "ab");
        assert_ne!(one, TextoRef::Um(b"abc"));
        assert_eq!(Texto::de_str("ab"), Texto::Dois(vec![0x61, 0x62]));
        assert_eq!(one.compare(dois), std::cmp::Ordering::Equal);
        assert_eq!(one.compare(TextoRef::Dois(&[0x61, 0x63])), std::cmp::Ordering::Less);
        assert_eq!(TextoRef::Um(b"b").compare(TextoRef::Um(b"ab")), std::cmp::Ordering::Greater);
    }

    #[test]
    fn strings_search() {
        let t = TextoRef::Um(b"abcabc");
        assert_eq!(t.search_for(TextoRef::Um(b"bc"), 0), Some(1));
        assert_eq!(t.search_for(TextoRef::Um(b"bc"), 2), Some(4));
        assert_eq!(t.search_for(TextoRef::Um(b"bd"), 0), None);
        assert_eq!(t.search_for(TextoRef::Um(b""), 6), Some(6));
        assert_eq!(t.search_for(TextoRef::Um(b""), 7), None);
        assert_eq!(t.search_for(TextoRef::Dois(&[0x63, 0x61]), 0), Some(2));
        assert_eq!(t.find_last(TextoRef::Um(b"abc"), 6), Some(3));
        assert_eq!(t.find_last(TextoRef::Um(b"abc"), 2), Some(0));
        assert!(t.matches_at(TextoRef::Um(b"ca"), 2));
        assert!(!t.matches_at(TextoRef::Um(b"ca"), 5));
        assert!(!t.matches_at(TextoRef::Um(b"c"), usize::MAX));
    }

    #[test]
    fn strings_points_and_encoding() {
        let t = Texto::de_str("a😀");
        assert_eq!(t.len(), 3);
        assert_eq!(t.points(), vec![0x61, 0x1F600]);
        let released = TextoRef::Dois(&[0xD800, 0x61]);
        assert_eq!(released.points(), vec![0xD800, 0x61]);
        assert_eq!(released.to_vm_utf8(), "\u{FFFD}a".as_bytes());
        let wtf8 = released.to_wtf8();
        assert_eq!(Texto::de_wtf8(&wtf8).vista(), released);
        assert_eq!(TextoRef::Um(&[0xE9]).to_wtf8(), "é".as_bytes());
        assert_eq!(TextoRef::Um(&[0xE9]).para_string(), "é");
    }

    #[test]
    fn strings_hash_matches_layout() {
        for s in ["", "a", "héllo", "€uro", "😀"] {
            let t = Texto::de_str(s);
            assert_eq!(t.vista().hash_vm(), layout::hash_de_texto(s.encode_utf16()));
            assert_eq!(t.hash_vm(), i64::from(t.vista().hash_vm()));
        }
    }

    #[test]
    fn strings_double_ended_iterator() {
        let t = TextoRef::Dois(&[1, 2, 3]);
        assert_eq!(t.unidades().rev().collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(t.unidades().len(), 3);
        let mut m = StringMut::new();
        m.push_vista(TextoRef::Um(b"x"));
        m.push('€');
        assert_eq!(m.end(), Texto::de_str("x€"));
    }
}
