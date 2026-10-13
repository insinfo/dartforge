//! As strings do espaço unificado (P1, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_OneByteString` (cid 6) e `_TwoByteString` (cid 7) são blocos `BRUTO`: o
//! comprimento em `b+16`, as unidades a partir de `b+24` (completadas com zeros
//! até a palavra) e o hash em `map` (`layout::desl`). O coletor não percorre o
//! corpo; o bloco não se move enquanto vive, então as unidades têm endereço fixo.
//!
//! Três tipos:
//!
//! * [`crate::textos::TextoRef`] é a vista emprestada das unidades (de um bloco do heap ou de um
//!   [`crate::textos::Texto`] de construção); os algoritmos de busca, comparação e codificação
//!   moram nela;
//! * [`crate::textos::Texto`] é o dono das unidades antes de alocar (a saída de `toString`, os
//!   pedaços que o runtime monta); a forma é canônica, como na VM;
//! * [`crate::textos::StringMut`] junta unidades sem passar pelo `String` do Rust (que perderia os
//!   surrogates soltos).
//!
//! As funções de `impl Heap` usam só a API de `heap.rs` (§3.2); o endereço do
//! bloco sai do handle (`h - 2`), conferido vivo por [`crate::heap::Heap::header`].
#![allow(unsafe_code)]

use crate::heap::Heap;
use crate::layout::{self, Ref, cid, desl, estado, flags};

// ---------------------------------------------------------------------------
// Texto: o dono das unidades antes de alocar.
// ---------------------------------------------------------------------------

/// Texto Dart: uma sequência de unidades de código UTF-16, na forma da VM
/// (decisão 5, docs/NATIVO-PLANO.md §7.1).
///
/// * `Um` é o `_OneByteString`: toda unidade cabe em um byte (Latin-1,
///   U+0000–U+00FF), guardada em um byte;
/// * `Dois` é o `_TwoByteString`: alguma unidade passa de 0xFF, e todas são
///   guardadas em dois bytes.
///
/// A forma é **canônica**, como na VM (`String::New`/`String::SubString`,
/// `runtime/vm/object.cc`, escolhem a classe pelo conteúdo): `Dois` só existe
/// quando alguma unidade passa de 0xFF. Os construtores abaixo garantem isso;
/// a igualdade compara unidades de todo modo, então um `Dois` Latin-1 criado à
/// mão só custa espaço, nunca muda o resultado.
///
/// Um *surrogate* solto (U+D800–U+DFFF sem par) é uma unidade como outra
/// qualquer: `length`, índices, `codeUnitAt`, `substring` no meio de um par e
/// `runes` (que devolve o próprio valor do surrogate solto) seguem a
/// semântica do Dart. Só na saída (`print`) ele vira U+FFFD, como a VM
/// escreve (`Utf8::Encode`, via `Dart_CopyUTF8EncodingOfString`).
#[derive(Clone)]
pub enum Texto {
    Um(Vec<u8>),
    Dois(Vec<u16>),
}

impl Texto {
    /// A string vazia.
    pub fn empty() -> Self {
        Texto::Um(Vec::new())
    }

    /// Texto a partir de unidades UTF-16, na forma canônica.
    pub fn from_units(unidades: Vec<u16>) -> Self {
        if unidades.iter().all(|&u| u <= 0xFF) {
            Texto::Um(unidades.into_iter().map(|u| u as u8).collect())
        } else {
            Texto::Dois(unidades)
        }
    }

    /// Texto a partir de uma fatia de unidades UTF-16, na forma canônica.
    pub fn from_slice(unidades: &[u16]) -> Self {
        if unidades.iter().all(|&u| u <= 0xFF) {
            Texto::Um(unidades.iter().map(|&u| u as u8).collect())
        } else {
            Texto::Dois(unidades.to_vec())
        }
    }

    /// Texto a partir de UTF-8 válido (texto que o próprio runtime formatou).
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

    /// Texto a partir de WTF-8: UTF-8 generalizado em que um surrogate solto
    /// é codificado em três bytes como qualquer ponto de código (é a forma em
    /// que o front-end guarda o texto dos literais, `frontend/src/text.rs`).
    /// UTF-8 válido é WTF-8. Um byte malformado vira U+FFFD — não ocorre
    /// para bytes emitidos pelo compilador.
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

    /// A vista das unidades deste texto de construção.
    pub fn vista(&self) -> TextoRef<'_> {
        match self {
            Texto::Um(b) => TextoRef::Um(b),
            Texto::Dois(u) => TextoRef::Dois(u),
        }
    }

    /// Quantidade de unidades UTF-16 (`String.length`).
    pub fn len(&self) -> usize {
        self.vista().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// É um `_OneByteString` (Latin-1)?
    pub fn e_um_byte(&self) -> bool {
        matches!(self, Texto::Um(_))
    }

    /// A unidade no índice (`codeUnitAt`); o índice é verificado por quem chama.
    pub fn unit(&self, i: usize) -> u16 {
        self.vista().unit(i)
    }

    /// As unidades, em ordem.
    pub fn unidades(&self) -> UnitsIter<'_> {
        self.vista().unidades()
    }

    /// As unidades num vetor.
    pub fn para_vec(&self) -> Vec<u16> {
        match self {
            Texto::Um(b) => b.iter().map(|&x| u16::from(x)).collect(),
            Texto::Dois(u) => u.clone(),
        }
    }

    /// `[start, end)` em unidades, na forma canônica (o `_substringUnchecked`
    /// da VM: um pedaço Latin-1 de um `_TwoByteString` volta a ser `Um`).
    pub fn piece(&self, start: usize, end: usize) -> Texto {
        self.vista().piece(start, end).to_string_fn()
    }

    /// Os pontos de código (`runes`): um par de surrogates vira o escalar; um
    /// surrogate solto sai como ele mesmo (o `RuneIterator` do `dart:core`).
    pub fn points(&self) -> Vec<u32> {
        self.vista().points()
    }

    /// WTF-8 (surrogate solto em três bytes): a forma sem perda, inversa de
    /// [`Texto::de_wtf8`].
    pub fn to_wtf8(&self) -> Vec<u8> {
        self.vista().to_wtf8()
    }

    /// UTF-8 como a VM escreve na saída (`print`): o surrogate solto vira
    /// U+FFFD (`Utf8::Encode`, `runtime/vm/unicode.cc`).
    pub fn to_vm_utf8(&self) -> Vec<u8> {
        self.vista().to_vm_utf8()
    }

    /// Texto legível para Rust: surrogate solto vira U+FFFD. Só para
    /// mensagens e formatação interna; o valor Dart nunca passa por aqui.
    pub fn para_string(&self) -> String {
        self.vista().para_string()
    }

    /// Concatenação (`+`), na forma canônica.
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

    /// Primeira ocorrência de `pattern` a partir de `since` (unidades).
    pub fn search_for(&self, pattern: &Texto, since: usize) -> Option<usize> {
        self.vista().search_for(pattern.vista(), since)
    }

    /// Última ocorrência de `pattern` que começa em `until` ou antes.
    pub fn find_last(&self, pattern: &Texto, until: usize) -> Option<usize> {
        self.vista().find_last(pattern.vista(), until)
    }

    /// `pattern` aparece inteiro a partir da unidade `i`.
    pub fn matches_at(&self, pattern: &Texto, i: usize) -> bool {
        self.vista().matches_at(pattern.vista(), i)
    }

    /// Ordem lexicográfica por unidades (`String.compareTo`).
    pub fn compare(&self, other_one: &Texto) -> std::cmp::Ordering {
        self.vista().compare(other_one.vista())
    }

    /// `String.hashCode` da VM ([`layout::hash_de_texto`]), como `i64` (a
    /// forma que a API velha usa).
    pub fn hash_vm(&self) -> i64 {
        i64::from(self.vista().hash_vm())
    }

}

/// Uma unidade (ponto ≤ 0xFFFF, inclusive surrogate) ou um par de surrogates.
pub fn push_point(unidades: &mut Vec<u16>, point: u32) {
    if point <= 0xFFFF {
        unidades.push(point as u16);
    } else {
        let p = point - 0x10000;
        unidades.push(0xD800 + (p >> 10) as u16);
        unidades.push(0xDC00 + (p & 0x3FF) as u16);
    }
}

/// Um ponto de código em WTF-8 (surrogate em três bytes, como qualquer ponto).
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

/// Construtor de texto por unidades: a saída de `toString`/interpolação, que
/// não pode passar por `String` do Rust sem perder surrogates soltos.
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
    /// Acrescenta as unidades de uma vista (de um bloco do heap ou de um `Texto`).
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
// TextoRef: a vista emprestada.
// ---------------------------------------------------------------------------

/// A vista das unidades de uma string: um byte por unidade (`_OneByteString`) ou
/// dois (`_TwoByteString`).
///
/// Emprestada de um bloco do heap ([`Heap::text`]) ela vale enquanto o
/// empréstimo do heap durar (nenhuma alocação no meio); de um [`Texto`],
/// enquanto ele viver.
#[derive(Clone, Copy)]
pub enum TextoRef<'a> {
    Um(&'a [u8]),
    Dois(&'a [u16]),
}

impl<'a> TextoRef<'a> {
    /// Quantas unidades.
    pub fn len(self) -> usize {
        match self {
            TextoRef::Um(b) => b.len(),
            TextoRef::Dois(u) => u.len(),
        }
    }
    /// Sem unidades?
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// A forma de um byte por unidade?
    pub fn e_um_byte(self) -> bool {
        matches!(self, TextoRef::Um(_))
    }
    /// Toda unidade cabe num byte (a forma canônica de uma cópia é `Um`)?
    pub fn fits_in_one_byte(self) -> bool {
        match self {
            TextoRef::Um(_) => true,
            TextoRef::Dois(u) => u.iter().all(|&x| x <= 0xFF),
        }
    }
    /// A unidade `i`.
    pub fn unit(self, i: usize) -> u16 {
        match self {
            TextoRef::Um(b) => u16::from(b[i]),
            TextoRef::Dois(u) => u[i],
        }
    }
    /// As unidades, em ordem.
    pub fn unidades(self) -> UnitsIter<'a> {
        UnitsIter { text: self, i: 0, end: self.len() }
    }
    /// As unidades `start..end`.
    pub fn piece(self, start: usize, end: usize) -> TextoRef<'a> {
        match self {
            TextoRef::Um(b) => TextoRef::Um(&b[start..end]),
            TextoRef::Dois(u) => TextoRef::Dois(&u[start..end]),
        }
    }
    /// Uma cópia de construção, na forma canônica.
    pub fn to_string_fn(self) -> Texto {
        match self {
            TextoRef::Um(b) => Texto::Um(b.to_vec()),
            TextoRef::Dois(u) => Texto::from_slice(u),
        }
    }
    /// O texto em UTF-8, com os surrogates soltos trocados por U+FFFD.
    pub fn para_string(self) -> String {
        match self {
            TextoRef::Um(b) => b.iter().map(|&x| char::from(x)).collect(),
            TextoRef::Dois(u) => String::from_utf16_lossy(u),
        }
    }
    /// O texto em WTF-8 (surrogates soltos preservados), inverso de
    /// [`Texto::de_wtf8`].
    pub fn to_wtf8(self) -> Vec<u8> {
        self.encode(false)
    }
    /// O UTF-8 que a VM escreve (`Utf8::Encode`): o surrogate solto vira U+FFFD.
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
    /// Os pontos de código (`runes`): um par de surrogates vira o escalar; um
    /// surrogate solto sai como ele mesmo (o `RuneIterator` do `dart:core`).
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
    /// A primeira ocorrência de `pattern` a partir de `since`.
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
        // Latin-1 nos dois: o primeiro byte filtra as posições.
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
    /// A última ocorrência de `pattern` que começa até `until`.
    pub fn find_last(self, pattern: TextoRef<'_>, until: usize) -> Option<usize> {
        let (n, m) = (self.len(), pattern.len());
        if m > n {
            return None;
        }
        let start = until.min(n - m);
        (0..=start).rev().find(|&i| self.matches_at(pattern, i))
    }
    /// `pattern` ocorre na posição `i`?
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
    /// A ordem de `compareTo` (unidade a unidade, depois o comprimento).
    pub fn compare(self, other_one: TextoRef<'_>) -> std::cmp::Ordering {
        match (self, other_one) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a.cmp(b),
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a.cmp(b),
            _ => self.unidades().cmp(other_one.unidades()),
        }
    }
    /// O `hashCode` da VM ([`layout::hash_de_texto`]): 30 bits, nunca 0.
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

/// Iterador das unidades de uma [`TextoRef`] (e de um [`Texto`]).
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

/// Desliga a vista do empréstimo de onde ela saiu.
///
/// # Safety
/// As unidades continuam válidas enquanto a vista for usada: vêm de um bloco do
/// heap que está vivo (enraizado) e nenhuma coleta acontece no meio (o coletor
/// não move, então só a morte do bloco as invalida).
unsafe fn disable<'b>(t: TextoRef<'_>) -> TextoRef<'b> {
    match t {
        // SAFETY: contrato acima.
        TextoRef::Um(b) => TextoRef::Um(unsafe { std::slice::from_raw_parts(b.as_ptr(), b.len()) }),
        TextoRef::Dois(u) => TextoRef::Dois(unsafe { std::slice::from_raw_parts(u.as_ptr(), u.len()) }),
    }
}

// ---------------------------------------------------------------------------
// As strings no heap.
// ---------------------------------------------------------------------------

/// O cid da string na forma pedida.
const fn string_cid(dois: bool) -> i32 {
    if dois { cid::TWO_BYTE_STRING } else { cid::ONE_BYTE_STRING }
}

impl Heap {
    /// O endereço do bloco e o cid da string `h`; `None` se `h` não é string.
    /// Pânico N4 (de [`Heap::header`]) para objeto morto ou inválido.
    fn string_block(&self, h: Ref) -> Option<(usize, i32)> {
        if !layout::is_object(h) {
            return None;
        }
        let c = self.header(h).class_id;
        cid::is_string(c).then_some(((h - layout::DESLOCAMENTO_DO_HANDLE) as usize, c))
    }

    /// As unidades da string `h`; `None` se `h` não é string.
    pub fn text(&self, h: Ref) -> Option<TextoRef<'_>> {
        let (b, c) = self.string_block(h)?;
        // SAFETY: bloco vivo (ou estático) de string: o comprimento em `b+16` e
        // as unidades a partir de `b+24`, alinhadas a 8, dentro do corpo. O
        // empréstimo de `self` impede coleta enquanto a vista vive.
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

    /// `h` é string (cid 6 ou 7)?
    pub fn is_string(&self, h: Ref) -> bool {
        self.string_block(h).is_some()
    }

    /// Uma string nova de `len` unidades zeradas (a forma pedida). Pode coletar:
    /// as referências que o chamador segura precisam de raiz.
    pub fn new_string(&mut self, len: usize, dois: bool) -> Ref {
        let w = layout::palavras_de_texto(len, dois);
        let h = self.allocate(string_cid(dois), w, flags::BRUTO);
        self.words_mut(h)[0] = len as i64;
        h
    }

    /// As unidades de uma `_OneByteString` ainda não publicada.
    ///
    /// # Panics
    /// Se `h` não é `_OneByteString`.
    pub fn one_units_mut(&mut self, h: Ref) -> &mut [u8] {
        let (_, c) = self.string_block(h).expect("bug do compilador: _OneByteString esperada");
        assert_eq!(c, cid::ONE_BYTE_STRING, "bug do compilador: _OneByteString esperada");
        let len = self.words(h)[0] as usize;
        &mut self.bytes_mut(h)[8..8 + len]
    }

    /// As unidades de uma `_TwoByteString` ainda não publicada.
    ///
    /// # Panics
    /// Se `h` não é `_TwoByteString`.
    pub fn two_units_mut(&mut self, h: Ref) -> &mut [u16] {
        let (_, c) = self.string_block(h).expect("bug do compilador: _TwoByteString esperada");
        assert_eq!(c, cid::TWO_BYTE_STRING, "bug do compilador: _TwoByteString esperada");
        let len = self.words(h)[0] as usize;
        let bytes = &mut self.bytes_mut(h)[8..8 + 2 * len];
        // SAFETY: os bytes das unidades começam em `b+24` (alinhado a 8) e têm
        // `2·len` bytes; a fatia de `u16` cobre exatamente eles.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<u16>(), len) }
    }

    /// Grava as unidades de `t` a partir da unidade `pos` da string `dest`
    /// (ainda não publicada). Numa `_OneByteString` cada unidade de `t` precisa
    /// caber num byte (a forma do destino foi escolhida por quem alocou).
    pub fn write_string(&mut self, dest: Ref, pos: usize, t: TextoRef<'_>) {
        let (_, c) = self.string_block(dest).expect("bug do compilador: string esperada");
        let n = t.len();
        if c == cid::ONE_BYTE_STRING {
            let d = &mut self.one_units_mut(dest)[pos..pos + n];
            match t {
                TextoRef::Um(b) => d.copy_from_slice(b),
                TextoRef::Dois(u) => {
                    for (x, &y) in d.iter_mut().zip(u) {
                        debug_assert!(y <= 0xFF, "unidade {y:#x} numa _OneByteString");
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

    /// Copia as unidades `start..end` da string `source` para a posição `pos` da
    /// string `dest` (outra, ainda não publicada), sem cópia intermediária.
    ///
    /// # Panics
    /// Se `source` não é string ou a faixa está fora dela.
    pub fn copy_string(&mut self, dest: Ref, pos: usize, source: Ref, start: usize, end: usize) {
        let t = self.text(source).expect("bug do compilador: string esperada").piece(start, end);
        // SAFETY: `source` está viva (o chamador a tem) e `write_string` não
        // aloca nem coleta; `dest` é outro bloco, então as fatias não se
        // sobrepõem.
        let t = unsafe { disable(t) };
        debug_assert_ne!(source, dest);
        self.write_string(dest, pos, t);
    }

    /// Uma string com as unidades de `t`, na forma canônica (um byte se cabe).
    /// `t` não pode vir deste heap (a alocação pode coletar): para uma fatia de
    /// uma string viva use [`Heap::string_slice`].
    pub fn allocate_string(&mut self, t: TextoRef<'_>) -> Ref {
        let h = self.new_string(t.len(), !t.fits_in_one_byte());
        self.write_string(h, 0, t);
        h
    }

    /// As unidades `start..end` da string `h` (enraizada pelo chamador) numa
    /// string nova, na forma canônica (o `_substringUnchecked` da VM).
    pub fn string_slice(&mut self, h: Ref, start: usize, end: usize) -> Ref {
        let one = self.text(h).expect("bug do compilador: string esperada").piece(start, end).fits_in_one_byte();
        let r = self.new_string(end - start, !one);
        self.copy_string(r, 0, h, start, end);
        r
    }

    /// A concatenação das strings `part_list` (enraizadas pelo chamador), numa
    /// alocação só, na forma canônica. Um elemento que não é string é bug do
    /// compilador (pânico).
    pub fn join_strings(&mut self, part_list: &[Ref]) -> Ref {
        let (mut total, mut one) = (0usize, true);
        for &p in part_list {
            let t = self.text(p).expect("bug do compilador: string esperada");
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

    /// Uma string com o texto de `s`.
    pub fn allocate_str(&mut self, s: &str) -> Ref {
        if s.is_ascii() {
            return self.allocate_string(TextoRef::Um(s.as_bytes()));
        }
        let t = Texto::de_str(s);
        self.allocate_string(t.vista())
    }

    /// Uma string a partir de WTF-8.
    pub fn string_from_wtf8(&mut self, bytes: &[u8]) -> Ref {
        if bytes.is_ascii() {
            return self.allocate_string(TextoRef::Um(bytes));
        }
        let t = Texto::de_wtf8(bytes);
        self.allocate_string(t.vista())
    }

    /// O `hashCode` da string `h`, calculado na primeira consulta e gravado em
    /// `map`; `None` se `h` não é string. O estático (`PERMANENTE`) já vem com
    /// o hash do emissor e nunca é gravado (mora em memória só de leitura).
    pub fn hash_de_texto(&self, h: Ref) -> Option<u32> {
        let (b, _) = self.string_block(h)?;
        let p = (b + desl::HASH_DO_TEXTO) as *mut u32;
        // SAFETY: `map` do cabeçalho de um bloco vivo; o heap é de uma thread
        // só e nenhuma referência ao cabeçalho está viva aqui. Pela `AtomicU32`
        // a gravação por `&self` é um cache sem referência compartilhada ao
        // mesmo `u32`.
        let cell = unsafe { std::sync::atomic::AtomicU32::from_ptr(p) };
        let current = cell.load(std::sync::atomic::Ordering::Relaxed);
        if current != 0 {
            return Some(current);
        }
        let x = self.text(h)?.hash_vm();
        // SAFETY: `b` é o bloco (endereço do cabeçalho).
        if unsafe { *(b as *const u8) } != estado::PERMANENTE {
            cell.store(x, std::sync::atomic::Ordering::Relaxed);
        }
        Some(x)
    }

    /// As duas strings têm as mesmas unidades? `false` se uma delas não é string.
    pub fn strings_equal(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return layout::is_object(a) && self.is_string(a);
        }
        let (Some(ta), Some(tb)) = (self.text(a), self.text(b)) else { return false };
        if ta.len() != tb.len() {
            return false;
        }
        // Os hashes já calculados diferem: unidades diferentes.
        let hash = |h: Ref| -> u32 {
            // SAFETY: `map` de um bloco de string vivo, só lido.
            unsafe { *((h - layout::DESLOCAMENTO_DO_HANDLE) as usize as *const u32).byte_add(desl::HASH_DO_TEXTO) }
        };
        let (ha, hb) = (hash(a), hash(b));
        if ha != 0 && hb != 0 && ha != hb {
            return false;
        }
        ta == tb
    }

    /// A string canônica com as unidades de `t` (internada em `literals`, raiz
    /// permanente: o literal do JIT e do runtime). `t` não vem deste heap.
    pub fn string_literal(&mut self, t: TextoRef<'_>) -> Ref {
        // O literal estático de uma imagem com o mesmo conteúdo é o canônico
        // (§2.11): `identical(s[0], 'a')` e a `const` interpolada.
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
// O índice dos literais estáticos das imagens.
// ---------------------------------------------------------------------------

/// Os literais estáticos (`@df.s.*`, §2.11) das imagens registradas, pelo hash
/// gravado no cabeçalho: `(images já percorridas, hash → blocks)`. Montado sob
/// demanda (a primeira canonicalização depois de uma imagem nova), só lendo as
/// palavras de cabeçalho — o hash vem calculado do emissor.
static STRING_STATICS: std::sync::RwLock<Option<(usize, std::collections::HashMap<u32, Vec<usize>>)>> =
    std::sync::RwLock::new(None);

/// A vista de um bloco de string estático (`PERMANENTE`, de uma imagem).
///
/// # Safety
/// `b` é o bloco de uma string estática válida.
unsafe fn static_view<'a>(b: usize) -> TextoRef<'a> {
    // SAFETY: contrato acima; a imagem não é descarregada.
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

/// Percorre a seção de estáticos `[start, end)`: palavras zero (os marcadores,
/// o enchimento entre as contribuições dos objetos) são puladas; uma palavra
/// que não é o cabeçalho de uma string estática coerente encerra a travessia.
fn index_image(start: usize, end: usize, ix: &mut std::collections::HashMap<u32, Vec<usize>>) {
    let mut b = start.next_multiple_of(8);
    while b + layout::TAMANHO_DO_CABECALHO + 8 <= end {
        // SAFETY: `b` está dentro da faixa registrada da imagem.
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
        // SAFETY: cabeçalho de string estática: o hash e o comprimento seguem.
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

/// O literal estático com as unidades de `t`, se alguma imagem registrada o
/// tem. Entre imagens com o mesmo literal (o executável e a DLL do SDK na
/// ligação de desenvolvimento, §4.10 item 52), vale a registrada por último — o
/// programa, que se registra depois do SDK.
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
        // SAFETY: bloco indexado de uma string estática.
        (unsafe { static_view(b) } == t).then_some(b as Ref + layout::DESLOCAMENTO_DO_HANDLE)
    })
}

impl Heap {
    /// A forma canônica de uma constante (o getter de `const`, `constantes.rs`):
    /// uma string vira o literal canônico com o mesmo conteúdo (o estático da
    /// imagem ou o internado); o resto volta como está.
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
