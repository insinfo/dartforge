//! As strings do espaço unificado (P1, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_OneByteString` (cid 6) e `_TwoByteString` (cid 7) são blocos `BRUTO`: o
//! comprimento em `b+16`, as unidades a partir de `b+24` (completadas com zeros
//! até a palavra) e o hash em `mapa` (`layout::desl`). O coletor não percorre o
//! corpo; o bloco não se move enquanto vive, então as unidades têm endereço fixo.
//!
//! Três tipos:
//!
//! * [`TextoRef`] é a vista emprestada das unidades (de um bloco do heap ou de um
//!   [`Texto`] de construção); os algoritmos de busca, comparação e codificação
//!   moram nela;
//! * [`Texto`] é o dono das unidades antes de alocar (a saída de `toString`, os
//!   pedaços que o runtime monta); a forma é canônica, como na VM;
//! * [`TextoMut`] junta unidades sem passar pelo `String` do Rust (que perderia os
//!   surrogates soltos).
//!
//! As funções de `impl Heap` usam só a API de `heap.rs` (§3.2); o endereço do
//! bloco sai do handle (`h - 2`), conferido vivo por [`Heap::cabecalho`].
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
    pub fn vazio() -> Self {
        Texto::Um(Vec::new())
    }

    /// Texto a partir de unidades UTF-16, na forma canônica.
    pub fn de_unidades(unidades: Vec<u16>) -> Self {
        if unidades.iter().all(|&u| u <= 0xFF) {
            Texto::Um(unidades.into_iter().map(|u| u as u8).collect())
        } else {
            Texto::Dois(unidades)
        }
    }

    /// Texto a partir de uma fatia de unidades UTF-16, na forma canônica.
    pub fn de_fatia(unidades: &[u16]) -> Self {
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
        let mut um = Vec::with_capacity(s.len());
        for c in s.chars() {
            if (c as u32) <= 0xFF {
                um.push(c as u32 as u8);
            } else {
                return Texto::Dois(s.encode_utf16().collect());
            }
        }
        Texto::Um(um)
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
            let (ponto, n) = if b0 < 0x80 {
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
            match ponto {
                Some(p) => {
                    empurrar_ponto(&mut unidades, p);
                    i += n;
                }
                None => {
                    unidades.push(0xFFFD);
                    i += 1;
                }
            }
        }
        Self::de_unidades(unidades)
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
    pub fn unidade(&self, i: usize) -> u16 {
        self.vista().unidade(i)
    }

    /// As unidades, em ordem.
    pub fn unidades(&self) -> IterUnidades<'_> {
        self.vista().unidades()
    }

    /// As unidades num vetor.
    pub fn para_vec(&self) -> Vec<u16> {
        match self {
            Texto::Um(b) => b.iter().map(|&x| u16::from(x)).collect(),
            Texto::Dois(u) => u.clone(),
        }
    }

    /// `[inicio, fim)` em unidades, na forma canônica (o `_substringUnchecked`
    /// da VM: um pedaço Latin-1 de um `_TwoByteString` volta a ser `Um`).
    pub fn fatia(&self, inicio: usize, fim: usize) -> Texto {
        self.vista().fatia(inicio, fim).para_texto()
    }

    /// Os pontos de código (`runes`): um par de surrogates vira o escalar; um
    /// surrogate solto sai como ele mesmo (o `RuneIterator` do `dart:core`).
    pub fn pontos(&self) -> Vec<u32> {
        self.vista().pontos()
    }

    /// WTF-8 (surrogate solto em três bytes): a forma sem perda, inversa de
    /// [`Texto::de_wtf8`].
    pub fn para_wtf8(&self) -> Vec<u8> {
        self.vista().para_wtf8()
    }

    /// UTF-8 como a VM escreve na saída (`print`): o surrogate solto vira
    /// U+FFFD (`Utf8::Encode`, `runtime/vm/unicode.cc`).
    pub fn para_utf8_da_vm(&self) -> Vec<u8> {
        self.vista().para_utf8_da_vm()
    }

    /// Texto legível para Rust: surrogate solto vira U+FFFD. Só para
    /// mensagens e formatação interna; o valor Dart nunca passa por aqui.
    pub fn para_string(&self) -> String {
        self.vista().para_string()
    }

    /// Concatenação (`+`), na forma canônica.
    pub fn concatenar(&self, outro: &Texto) -> Texto {
        match (self, outro) {
            (Texto::Um(a), Texto::Um(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(a);
                v.extend_from_slice(b);
                Texto::Um(v)
            }
            _ => {
                let mut v = Vec::with_capacity(self.len() + outro.len());
                v.extend(self.unidades());
                v.extend(outro.unidades());
                Texto::Dois(v)
            }
        }
    }

    /// Primeira ocorrência de `padrao` a partir de `desde` (unidades).
    pub fn procurar(&self, padrao: &Texto, desde: usize) -> Option<usize> {
        self.vista().procurar(padrao.vista(), desde)
    }

    /// Última ocorrência de `padrao` que começa em `ate` ou antes.
    pub fn procurar_ultimo(&self, padrao: &Texto, ate: usize) -> Option<usize> {
        self.vista().procurar_ultimo(padrao.vista(), ate)
    }

    /// `padrao` aparece inteiro a partir da unidade `i`.
    pub fn coincide_em(&self, padrao: &Texto, i: usize) -> bool {
        self.vista().coincide_em(padrao.vista(), i)
    }

    /// Ordem lexicográfica por unidades (`String.compareTo`).
    pub fn comparar(&self, outro: &Texto) -> std::cmp::Ordering {
        self.vista().comparar(outro.vista())
    }

    /// `String.hashCode` da VM ([`layout::hash_de_texto`]), como `i64` (a
    /// forma que a API velha usa).
    pub fn hash_vm(&self) -> i64 {
        i64::from(self.vista().hash_vm())
    }

}

/// Uma unidade (ponto ≤ 0xFFFF, inclusive surrogate) ou um par de surrogates.
pub fn empurrar_ponto(unidades: &mut Vec<u16>, ponto: u32) {
    if ponto <= 0xFFFF {
        unidades.push(ponto as u16);
    } else {
        let p = ponto - 0x10000;
        unidades.push(0xD800 + (p >> 10) as u16);
        unidades.push(0xDC00 + (p & 0x3FF) as u16);
    }
}

/// Um ponto de código em WTF-8 (surrogate em três bytes, como qualquer ponto).
fn codificar_wtf8(saida: &mut Vec<u8>, p: u32) {
    if p < 0x80 {
        saida.push(p as u8);
    } else if p < 0x800 {
        saida.extend_from_slice(&[0xC0 | (p >> 6) as u8, 0x80 | (p & 0x3F) as u8]);
    } else if p < 0x10000 {
        saida.extend_from_slice(&[0xE0 | (p >> 12) as u8, 0x80 | ((p >> 6) & 0x3F) as u8, 0x80 | (p & 0x3F) as u8]);
    } else {
        saida.extend_from_slice(&[
            0xF0 | (p >> 18) as u8,
            0x80 | ((p >> 12) & 0x3F) as u8,
            0x80 | ((p >> 6) & 0x3F) as u8,
            0x80 | (p & 0x3F) as u8,
        ]);
    }
}

impl PartialEq for Texto {
    fn eq(&self, outro: &Texto) -> bool {
        self.vista() == outro.vista()
    }
}

impl Eq for Texto {}

impl Default for Texto {
    fn default() -> Self {
        Texto::vazio()
    }
}

impl PartialEq<str> for Texto {
    fn eq(&self, outro: &str) -> bool {
        self.vista() == *outro
    }
}

impl PartialEq<&str> for Texto {
    fn eq(&self, outro: &&str) -> bool {
        self.vista() == **outro
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
pub struct TextoMut(pub Vec<u16>);

impl TextoMut {
    pub fn new() -> Self {
        TextoMut(Vec::new())
    }
    pub fn push_str(&mut self, s: &str) {
        self.0.extend(s.encode_utf16());
    }
    pub fn push(&mut self, c: char) {
        let mut buf = [0u16; 2];
        self.0.extend_from_slice(c.encode_utf16(&mut buf));
    }
    pub fn push_texto(&mut self, t: &Texto) {
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
    pub fn fim(self) -> Texto {
        Texto::de_unidades(self.0)
    }
}

// ---------------------------------------------------------------------------
// TextoRef: a vista emprestada.
// ---------------------------------------------------------------------------

/// A vista das unidades de uma string: um byte por unidade (`_OneByteString`) ou
/// dois (`_TwoByteString`).
///
/// Emprestada de um bloco do heap ([`Heap::texto`]) ela vale enquanto o
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
    pub fn cabe_em_um_byte(self) -> bool {
        match self {
            TextoRef::Um(_) => true,
            TextoRef::Dois(u) => u.iter().all(|&x| x <= 0xFF),
        }
    }
    /// A unidade `i`.
    pub fn unidade(self, i: usize) -> u16 {
        match self {
            TextoRef::Um(b) => u16::from(b[i]),
            TextoRef::Dois(u) => u[i],
        }
    }
    /// As unidades, em ordem.
    pub fn unidades(self) -> IterUnidades<'a> {
        IterUnidades { texto: self, i: 0, fim: self.len() }
    }
    /// As unidades `inicio..fim`.
    pub fn fatia(self, inicio: usize, fim: usize) -> TextoRef<'a> {
        match self {
            TextoRef::Um(b) => TextoRef::Um(&b[inicio..fim]),
            TextoRef::Dois(u) => TextoRef::Dois(&u[inicio..fim]),
        }
    }
    /// Uma cópia de construção, na forma canônica.
    pub fn para_texto(self) -> Texto {
        match self {
            TextoRef::Um(b) => Texto::Um(b.to_vec()),
            TextoRef::Dois(u) => Texto::de_fatia(u),
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
    pub fn para_wtf8(self) -> Vec<u8> {
        self.codificar(false)
    }
    /// O UTF-8 que a VM escreve (`Utf8::Encode`): o surrogate solto vira U+FFFD.
    pub fn para_utf8_da_vm(self) -> Vec<u8> {
        self.codificar(true)
    }
    fn codificar(self, trocar_soltos: bool) -> Vec<u8> {
        if let TextoRef::Um(b) = self
            && b.is_ascii()
        {
            return b.to_vec();
        }
        let mut saida = Vec::with_capacity(self.len() + 8);
        for p in self.pontos() {
            let p = if trocar_soltos && (0xD800..0xE000).contains(&p) { 0xFFFD } else { p };
            codificar_wtf8(&mut saida, p);
        }
        saida
    }
    /// Os pontos de código (`runes`): um par de surrogates vira o escalar; um
    /// surrogate solto sai como ele mesmo (o `RuneIterator` do `dart:core`).
    pub fn pontos(self) -> Vec<u32> {
        if let TextoRef::Um(b) = self {
            return b.iter().map(|&x| u32::from(x)).collect();
        }
        let n = self.len();
        let mut saida = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let u = self.unidade(i);
            if (0xD800..0xDC00).contains(&u) && i + 1 < n {
                let v = self.unidade(i + 1);
                if (0xDC00..0xE000).contains(&v) {
                    saida.push(0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(v) - 0xDC00));
                    i += 2;
                    continue;
                }
            }
            saida.push(u32::from(u));
            i += 1;
        }
        saida
    }
    /// A primeira ocorrência de `padrao` a partir de `desde`.
    pub fn procurar(self, padrao: TextoRef<'_>, desde: usize) -> Option<usize> {
        let (n, m) = (self.len(), padrao.len());
        if desde > n {
            return None;
        }
        if m == 0 {
            return Some(desde);
        }
        if m > n {
            return None;
        }
        let maximo = n - m;
        // Latin-1 nos dois: o primeiro byte filtra as posições.
        if let (TextoRef::Um(a), TextoRef::Um(b)) = (self, padrao) {
            let primeiro = b[0];
            let mut i = desde;
            while i <= maximo {
                match a[i..=maximo].iter().position(|&x| x == primeiro) {
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
        (desde..=maximo).find(|&i| self.coincide_em(padrao, i))
    }
    /// A última ocorrência de `padrao` que começa até `ate`.
    pub fn procurar_ultimo(self, padrao: TextoRef<'_>, ate: usize) -> Option<usize> {
        let (n, m) = (self.len(), padrao.len());
        if m > n {
            return None;
        }
        let inicio = ate.min(n - m);
        (0..=inicio).rev().find(|&i| self.coincide_em(padrao, i))
    }
    /// `padrao` ocorre na posição `i`?
    pub fn coincide_em(self, padrao: TextoRef<'_>, i: usize) -> bool {
        let m = padrao.len();
        if i.checked_add(m).is_none_or(|f| f > self.len()) {
            return false;
        }
        match (self, padrao) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a[i..i + m] == *b,
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a[i..i + m] == *b,
            _ => (0..m).all(|k| self.unidade(i + k) == padrao.unidade(k)),
        }
    }
    /// A ordem de `compareTo` (unidade a unidade, depois o comprimento).
    pub fn comparar(self, outro: TextoRef<'_>) -> std::cmp::Ordering {
        match (self, outro) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a.cmp(b),
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a.cmp(b),
            _ => self.unidades().cmp(outro.unidades()),
        }
    }
    /// O `hashCode` da VM ([`layout::hash_de_texto`]): 30 bits, nunca 0.
    pub fn hash_vm(self) -> u32 {
        layout::hash_de_texto(self.unidades())
    }
}

impl PartialEq for TextoRef<'_> {
    fn eq(&self, outro: &TextoRef<'_>) -> bool {
        match (*self, *outro) {
            (TextoRef::Um(a), TextoRef::Um(b)) => a == b,
            (TextoRef::Dois(a), TextoRef::Dois(b)) => a == b,
            (a, b) => a.len() == b.len() && a.unidades().eq(b.unidades()),
        }
    }
}

impl Eq for TextoRef<'_> {}

impl PartialEq<Texto> for TextoRef<'_> {
    fn eq(&self, outro: &Texto) -> bool {
        *self == outro.vista()
    }
}

impl PartialEq<str> for TextoRef<'_> {
    fn eq(&self, outro: &str) -> bool {
        if let TextoRef::Um(b) = *self
            && outro.is_ascii()
        {
            return b == outro.as_bytes();
        }
        self.unidades().eq(outro.encode_utf16())
    }
}

impl PartialEq<&str> for TextoRef<'_> {
    fn eq(&self, outro: &&str) -> bool {
        *self == **outro
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
pub struct IterUnidades<'a> {
    texto: TextoRef<'a>,
    i: usize,
    fim: usize,
}

impl Iterator for IterUnidades<'_> {
    type Item = u16;
    fn next(&mut self) -> Option<u16> {
        if self.i >= self.fim {
            return None;
        }
        let u = self.texto.unidade(self.i);
        self.i += 1;
        Some(u)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.fim - self.i;
        (n, Some(n))
    }
}

impl DoubleEndedIterator for IterUnidades<'_> {
    fn next_back(&mut self) -> Option<u16> {
        if self.i >= self.fim {
            return None;
        }
        self.fim -= 1;
        Some(self.texto.unidade(self.fim))
    }
}

impl ExactSizeIterator for IterUnidades<'_> {}

/// Desliga a vista do empréstimo de onde ela saiu.
///
/// # Safety
/// As unidades continuam válidas enquanto a vista for usada: vêm de um bloco do
/// heap que está vivo (enraizado) e nenhuma coleta acontece no meio (o coletor
/// não move, então só a morte do bloco as invalida).
unsafe fn desligar<'b>(t: TextoRef<'_>) -> TextoRef<'b> {
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
const fn cid_de_texto(dois: bool) -> i32 {
    if dois { cid::TWO_BYTE_STRING } else { cid::ONE_BYTE_STRING }
}

impl Heap {
    /// O endereço do bloco e o cid da string `h`; `None` se `h` não é string.
    /// Pânico N4 (de [`Heap::cabecalho`]) para objeto morto ou inválido.
    fn bloco_de_texto(&self, h: Ref) -> Option<(usize, i32)> {
        if !layout::e_objeto(h) {
            return None;
        }
        let c = self.cabecalho(h).class_id;
        cid::e_texto(c).then_some(((h - layout::DESLOCAMENTO_DO_HANDLE) as usize, c))
    }

    /// As unidades da string `h`; `None` se `h` não é string.
    pub fn texto(&self, h: Ref) -> Option<TextoRef<'_>> {
        let (b, c) = self.bloco_de_texto(h)?;
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
    pub fn e_texto(&self, h: Ref) -> bool {
        self.bloco_de_texto(h).is_some()
    }

    /// Uma string nova de `len` unidades zeradas (a forma pedida). Pode coletar:
    /// as referências que o chamador segura precisam de raiz.
    pub fn novo_texto(&mut self, len: usize, dois: bool) -> Ref {
        let w = layout::palavras_de_texto(len, dois);
        let h = self.alocar(cid_de_texto(dois), w, flags::BRUTO);
        self.palavras_mut(h)[0] = len as i64;
        h
    }

    /// As unidades de uma `_OneByteString` ainda não publicada.
    ///
    /// # Panics
    /// Se `h` não é `_OneByteString`.
    pub fn unidades_um_mut(&mut self, h: Ref) -> &mut [u8] {
        let (_, c) = self.bloco_de_texto(h).expect("bug do compilador: _OneByteString esperada");
        assert_eq!(c, cid::ONE_BYTE_STRING, "bug do compilador: _OneByteString esperada");
        let len = self.palavras(h)[0] as usize;
        &mut self.bytes_mut(h)[8..8 + len]
    }

    /// As unidades de uma `_TwoByteString` ainda não publicada.
    ///
    /// # Panics
    /// Se `h` não é `_TwoByteString`.
    pub fn unidades_dois_mut(&mut self, h: Ref) -> &mut [u16] {
        let (_, c) = self.bloco_de_texto(h).expect("bug do compilador: _TwoByteString esperada");
        assert_eq!(c, cid::TWO_BYTE_STRING, "bug do compilador: _TwoByteString esperada");
        let len = self.palavras(h)[0] as usize;
        let bytes = &mut self.bytes_mut(h)[8..8 + 2 * len];
        // SAFETY: os bytes das unidades começam em `b+24` (alinhado a 8) e têm
        // `2·len` bytes; a fatia de `u16` cobre exatamente eles.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<u16>(), len) }
    }

    /// Grava as unidades de `t` a partir da unidade `pos` da string `destino`
    /// (ainda não publicada). Numa `_OneByteString` cada unidade de `t` precisa
    /// caber num byte (a forma do destino foi escolhida por quem alocou).
    pub fn escrever_texto(&mut self, destino: Ref, pos: usize, t: TextoRef<'_>) {
        let (_, c) = self.bloco_de_texto(destino).expect("bug do compilador: string esperada");
        let n = t.len();
        if c == cid::ONE_BYTE_STRING {
            let d = &mut self.unidades_um_mut(destino)[pos..pos + n];
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
            let d = &mut self.unidades_dois_mut(destino)[pos..pos + n];
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

    /// Copia as unidades `inicio..fim` da string `fonte` para a posição `pos` da
    /// string `destino` (outra, ainda não publicada), sem cópia intermediária.
    ///
    /// # Panics
    /// Se `fonte` não é string ou a faixa está fora dela.
    pub fn copiar_texto(&mut self, destino: Ref, pos: usize, fonte: Ref, inicio: usize, fim: usize) {
        let t = self.texto(fonte).expect("bug do compilador: string esperada").fatia(inicio, fim);
        // SAFETY: `fonte` está viva (o chamador a tem) e `escrever_texto` não
        // aloca nem coleta; `destino` é outro bloco, então as fatias não se
        // sobrepõem.
        let t = unsafe { desligar(t) };
        debug_assert_ne!(fonte, destino);
        self.escrever_texto(destino, pos, t);
    }

    /// Uma string com as unidades de `t`, na forma canônica (um byte se cabe).
    /// `t` não pode vir deste heap (a alocação pode coletar): para uma fatia de
    /// uma string viva use [`Heap::fatia_de_texto`].
    pub fn alocar_texto(&mut self, t: TextoRef<'_>) -> Ref {
        let h = self.novo_texto(t.len(), !t.cabe_em_um_byte());
        self.escrever_texto(h, 0, t);
        h
    }

    /// As unidades `inicio..fim` da string `h` (enraizada pelo chamador) numa
    /// string nova, na forma canônica (o `_substringUnchecked` da VM).
    pub fn fatia_de_texto(&mut self, h: Ref, inicio: usize, fim: usize) -> Ref {
        let um = self.texto(h).expect("bug do compilador: string esperada").fatia(inicio, fim).cabe_em_um_byte();
        let r = self.novo_texto(fim - inicio, !um);
        self.copiar_texto(r, 0, h, inicio, fim);
        r
    }

    /// A concatenação das strings `partes` (enraizadas pelo chamador), numa
    /// alocação só, na forma canônica. Um elemento que não é string é bug do
    /// compilador (pânico).
    pub fn juntar_textos(&mut self, partes: &[Ref]) -> Ref {
        let (mut total, mut um) = (0usize, true);
        for &p in partes {
            let t = self.texto(p).expect("bug do compilador: string esperada");
            total += t.len();
            um &= t.cabe_em_um_byte();
        }
        let r = self.novo_texto(total, !um);
        let mut pos = 0;
        for &p in partes {
            let n = self.texto(p).map_or(0, TextoRef::len);
            self.copiar_texto(r, pos, p, 0, n);
            pos += n;
        }
        r
    }

    /// Uma string com o texto de `s`.
    pub fn alocar_str(&mut self, s: &str) -> Ref {
        if s.is_ascii() {
            return self.alocar_texto(TextoRef::Um(s.as_bytes()));
        }
        let t = Texto::de_str(s);
        self.alocar_texto(t.vista())
    }

    /// Uma string a partir de WTF-8.
    pub fn texto_de_wtf8(&mut self, bytes: &[u8]) -> Ref {
        if bytes.is_ascii() {
            return self.alocar_texto(TextoRef::Um(bytes));
        }
        let t = Texto::de_wtf8(bytes);
        self.alocar_texto(t.vista())
    }

    /// O `hashCode` da string `h`, calculado na primeira consulta e gravado em
    /// `mapa`; `None` se `h` não é string. O estático (`PERMANENTE`) já vem com
    /// o hash do emissor e nunca é gravado (mora em memória só de leitura).
    pub fn hash_de_texto(&self, h: Ref) -> Option<u32> {
        let (b, _) = self.bloco_de_texto(h)?;
        let p = (b + desl::HASH_DO_TEXTO) as *mut u32;
        // SAFETY: `mapa` do cabeçalho de um bloco vivo; o heap é de uma thread
        // só e nenhuma referência ao cabeçalho está viva aqui. Pela `AtomicU32`
        // a gravação por `&self` é um cache sem referência compartilhada ao
        // mesmo `u32`.
        let celula = unsafe { std::sync::atomic::AtomicU32::from_ptr(p) };
        let atual = celula.load(std::sync::atomic::Ordering::Relaxed);
        if atual != 0 {
            return Some(atual);
        }
        let x = self.texto(h)?.hash_vm();
        // SAFETY: `b` é o bloco (endereço do cabeçalho).
        if unsafe { *(b as *const u8) } != estado::PERMANENTE {
            celula.store(x, std::sync::atomic::Ordering::Relaxed);
        }
        Some(x)
    }

    /// As duas strings têm as mesmas unidades? `false` se uma delas não é string.
    pub fn textos_iguais(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return layout::e_objeto(a) && self.e_texto(a);
        }
        let (Some(ta), Some(tb)) = (self.texto(a), self.texto(b)) else { return false };
        if ta.len() != tb.len() {
            return false;
        }
        // Os hashes já calculados diferem: unidades diferentes.
        let hash = |h: Ref| -> u32 {
            // SAFETY: `mapa` de um bloco de string vivo, só lido.
            unsafe { *((h - layout::DESLOCAMENTO_DO_HANDLE) as usize as *const u32).byte_add(desl::HASH_DO_TEXTO) }
        };
        let (ha, hb) = (hash(a), hash(b));
        if ha != 0 && hb != 0 && ha != hb {
            return false;
        }
        ta == tb
    }

    /// A string canônica com as unidades de `t` (internada em `literais`, raiz
    /// permanente: o literal do JIT e do runtime). `t` não vem deste heap.
    pub fn string_literal(&mut self, t: TextoRef<'_>) -> Ref {
        // O literal estático de uma imagem com o mesmo conteúdo é o canônico
        // (§2.11): `identical(s[0], 'a')` e a `const` interpolada.
        if let Some(h) = estatico_de_texto(t) {
            return h;
        }
        let chave: Vec<u16> = t.unidades().collect();
        if let Some(h) = self.literal(&chave) {
            return h;
        }
        let h = self.alocar_texto(t);
        self.guardar_literal(chave, h);
        h
    }
}

// ---------------------------------------------------------------------------
// O índice dos literais estáticos das imagens.
// ---------------------------------------------------------------------------

/// Os literais estáticos (`@df.s.*`, §2.11) das imagens registradas, pelo hash
/// gravado no cabeçalho: `(imagens já percorridas, hash → blocos)`. Montado sob
/// demanda (a primeira canonicalização depois de uma imagem nova), só lendo as
/// palavras de cabeçalho — o hash vem calculado do emissor.
static ESTATICOS_DE_TEXTO: std::sync::RwLock<Option<(usize, std::collections::HashMap<u32, Vec<usize>>)>> =
    std::sync::RwLock::new(None);

/// A vista de um bloco de string estático (`PERMANENTE`, de uma imagem).
///
/// # Safety
/// `b` é o bloco de uma string estática válida.
unsafe fn vista_de_estatico<'a>(b: usize) -> TextoRef<'a> {
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

/// Percorre a seção de estáticos `[inicio, fim)`: palavras zero (os marcadores,
/// o enchimento entre as contribuições dos objetos) são puladas; uma palavra
/// que não é o cabeçalho de uma string estática coerente encerra a travessia.
fn indexar_imagem(inicio: usize, fim: usize, indice: &mut std::collections::HashMap<u32, Vec<usize>>) {
    let mut b = inicio.next_multiple_of(8);
    while b + layout::TAMANHO_DO_CABECALHO + 8 <= fim {
        // SAFETY: `b` está dentro da faixa registrada da imagem.
        let palavra = unsafe { *(b as *const u64) };
        if palavra == 0 {
            b += 8;
            continue;
        }
        let est = palavra as u8;
        let fl = (palavra >> 8) as u8;
        let c = (palavra >> 32) as i32;
        if est != estado::PERMANENTE || fl & flags::FORMA != flags::BRUTO || !cid::e_texto(c) {
            break;
        }
        // SAFETY: cabeçalho de string estática: o hash e o comprimento seguem.
        let (hash, len) = unsafe { (*((b + desl::HASH_DO_TEXTO) as *const u32), *((b + desl::COMPRIMENTO) as *const i64)) };
        let w = layout::palavras_de_texto(len as usize, c == cid::TWO_BYTE_STRING);
        let tamanho = layout::TAMANHO_DO_CABECALHO + 8 * w;
        if len < 0 || b + tamanho > fim {
            break;
        }
        indice.entry(hash).or_default().push(b);
        b += tamanho;
    }
}

/// O literal estático com as unidades de `t`, se alguma imagem registrada o
/// tem. Entre imagens com o mesmo literal (o executável e a DLL do SDK na
/// ligação de desenvolvimento, §4.10 item 52), vale a registrada por último — o
/// programa, que se registra depois do SDK.
pub fn estatico_de_texto(t: TextoRef<'_>) -> Option<Ref> {
    let imagens = crate::heap::imagens();
    let pronto = ESTATICOS_DE_TEXTO
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|(n, _)| *n == imagens.len());
    if !pronto {
        let mut g = ESTATICOS_DE_TEXTO.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (n, indice) = g.get_or_insert_with(|| (0, std::collections::HashMap::new()));
        for &(i, f) in &imagens[*n..] {
            indexar_imagem(i, f, indice);
        }
        *n = imagens.len();
    }
    let g = ESTATICOS_DE_TEXTO.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_, indice) = g.as_ref()?;
    let hash = t.hash_vm();
    indice.get(&hash)?.iter().rev().find_map(|&b| {
        // SAFETY: bloco indexado de uma string estática.
        (unsafe { vista_de_estatico(b) } == t).then_some(b as Ref + layout::DESLOCAMENTO_DO_HANDLE)
    })
}

impl Heap {
    /// A forma canônica de uma constante (o getter de `const`, `constantes.rs`):
    /// uma string vira o literal canônico com o mesmo conteúdo (o estático da
    /// imagem ou o internado); o resto volta como está.
    pub fn constante_canonica(&mut self, h: Ref) -> Ref {
        if !self.e_texto(h) || crate::heap::e_estatico(h) {
            return h;
        }
        let t = self.texto(h).expect("conferido").para_texto();
        self.string_literal(t.vista())
    }
}

#[cfg(test)]
mod testes_textos {
    use super::*;

    #[test]
    fn textos_forma_canonica_e_fatia() {
        assert!(Texto::de_str("abc").e_um_byte());
        assert!(Texto::de_str("ç").e_um_byte());
        assert!(!Texto::de_str("€").e_um_byte());
        let t = Texto::de_str("a€b");
        assert!(t.fatia(2, 3).e_um_byte());
        assert_eq!(t.vista().fatia(0, 1), "a");
        assert!(!TextoRef::Dois(&[0x61, 0x100]).cabe_em_um_byte());
        assert!(TextoRef::Dois(&[0x61, 0xFF]).cabe_em_um_byte());
        assert!(TextoRef::Dois(&[0x61, 0xFF]).para_texto().e_um_byte());
    }

    #[test]
    fn textos_igualdade_entre_formas() {
        let um = TextoRef::Um(b"ab");
        let dois = TextoRef::Dois(&[0x61, 0x62]);
        assert_eq!(um, dois);
        assert_eq!(dois, "ab");
        assert_ne!(um, TextoRef::Um(b"abc"));
        assert_eq!(Texto::de_str("ab"), Texto::Dois(vec![0x61, 0x62]));
        assert_eq!(um.comparar(dois), std::cmp::Ordering::Equal);
        assert_eq!(um.comparar(TextoRef::Dois(&[0x61, 0x63])), std::cmp::Ordering::Less);
        assert_eq!(TextoRef::Um(b"b").comparar(TextoRef::Um(b"ab")), std::cmp::Ordering::Greater);
    }

    #[test]
    fn textos_busca() {
        let t = TextoRef::Um(b"abcabc");
        assert_eq!(t.procurar(TextoRef::Um(b"bc"), 0), Some(1));
        assert_eq!(t.procurar(TextoRef::Um(b"bc"), 2), Some(4));
        assert_eq!(t.procurar(TextoRef::Um(b"bd"), 0), None);
        assert_eq!(t.procurar(TextoRef::Um(b""), 6), Some(6));
        assert_eq!(t.procurar(TextoRef::Um(b""), 7), None);
        assert_eq!(t.procurar(TextoRef::Dois(&[0x63, 0x61]), 0), Some(2));
        assert_eq!(t.procurar_ultimo(TextoRef::Um(b"abc"), 6), Some(3));
        assert_eq!(t.procurar_ultimo(TextoRef::Um(b"abc"), 2), Some(0));
        assert!(t.coincide_em(TextoRef::Um(b"ca"), 2));
        assert!(!t.coincide_em(TextoRef::Um(b"ca"), 5));
        assert!(!t.coincide_em(TextoRef::Um(b"c"), usize::MAX));
    }

    #[test]
    fn textos_pontos_e_codificacao() {
        let t = Texto::de_str("a😀");
        assert_eq!(t.len(), 3);
        assert_eq!(t.pontos(), vec![0x61, 0x1F600]);
        let solto = TextoRef::Dois(&[0xD800, 0x61]);
        assert_eq!(solto.pontos(), vec![0xD800, 0x61]);
        assert_eq!(solto.para_utf8_da_vm(), "\u{FFFD}a".as_bytes());
        let wtf8 = solto.para_wtf8();
        assert_eq!(Texto::de_wtf8(&wtf8).vista(), solto);
        assert_eq!(TextoRef::Um(&[0xE9]).para_wtf8(), "é".as_bytes());
        assert_eq!(TextoRef::Um(&[0xE9]).para_string(), "é");
    }

    #[test]
    fn textos_hash_igual_ao_do_layout() {
        for s in ["", "a", "héllo", "€uro", "😀"] {
            let t = Texto::de_str(s);
            assert_eq!(t.vista().hash_vm(), layout::hash_de_texto(s.encode_utf16()));
            assert_eq!(t.hash_vm(), i64::from(t.vista().hash_vm()));
        }
    }

    #[test]
    fn textos_iterador_dos_dois_lados() {
        let t = TextoRef::Dois(&[1, 2, 3]);
        assert_eq!(t.unidades().rev().collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(t.unidades().len(), 3);
        let mut m = TextoMut::new();
        m.push_vista(TextoRef::Um(b"x"));
        m.push('€');
        assert_eq!(m.fim(), Texto::de_str("x€"));
    }
}
