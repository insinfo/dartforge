//! Heap preciso com tracing iterativo, raízes explícitas e contadores observáveis.
//! Células, ambientes, closures e listas são infraestrutura: ainda não implicam
//! lowering Dart para LLVM. O contrato detalhado está em CONTRACT.md nesta crate.

/// Contadores cumulativos de trabalho; não representam bytes físicos do alocador.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeapStats {
    pub allocations: u64,
    pub collections: u64,
    pub reclaimed: u64,
    pub roots_scanned: u64,
    pub slots_scanned: u64,
    pub live_objects: usize,
    pub reserved_slots: usize,
    /// Valores enum canônicos mantidos vivos até encerrar o runtime.
    pub permanent_roots: usize,
    pub live_roots: usize,
    pub peak_roots: usize,
    pub root_slots: usize,
    pub peak_root_slots: usize,
    /// Cabeçalhos vivos e capacidades dos payloads, sem metadados auxiliares/RSS.
    pub estimated_bytes: usize,
    pub peak_estimated_bytes: usize,
    /// `int` em posição `Ref` que viraram `Smi` (R10) em vez de caixa: sem o
    /// `Smi`, cada um seria uma alocação. `allocations + caixas_evitadas` é a
    /// contagem de antes do R10, exata.
    pub caixas_evitadas: u64,
}

/// Marca escalar precisa; bits coincidentes entre int e bool não são iguais.
///
/// A distinção existe porque chaves de `Map` seguem `==` de Dart: `0` e `false`
/// são chaves diferentes, e zero como handle null só vale para referências.
/// A invariante é `is_ref ⟺ tag == Ref`; os construtores abaixo a garantem.
///
/// Discriminantes fixos: o código gerado lê e grava a tag dos elementos de
/// uma lista em linha (`lower/tipados.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueTag {
    Int = 0,
    Bool = 1,
    Double = 2,
    Ref = 3,
}

/// Payload com tag precisa; bits escalares jamais são interpretados como handles.
///
/// Layout C fixo (16 bytes: `bits` no deslocamento 0, `is_ref` no 8, `tag`
/// no 9): o código gerado lê e grava elementos de `Value::List` em linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TaggedValue {
    pub bits: i64,
    pub is_ref: bool,
    pub tag: ValueTag,
}
const _: () = {
    assert!(std::mem::size_of::<TaggedValue>() == 16);
    assert!(std::mem::offset_of!(TaggedValue, is_ref) == 8);
    assert!(std::mem::offset_of!(TaggedValue, tag) == 9);
};
impl TaggedValue {
    /// Representa inteiro; booleanos usam [`TaggedValue::boolean`].
    pub fn scalar(bits: i64) -> Self {
        Self {
            bits,
            is_ref: false,
            tag: ValueTag::Int,
        }
    }
    /// Representa booleano sem confundir `false` (bits 0) com inteiro zero.
    pub fn boolean(value: bool) -> Self {
        Self {
            bits: i64::from(value),
            is_ref: false,
            tag: ValueTag::Bool,
        }
    }
    /// Representa ponto flutuante de 64 bits.
    pub fn double(val: f64) -> Self {
        Self {
            bits: val.to_bits() as i64,
            is_ref: false,
            tag: ValueTag::Double,
        }
    }
    /// Representa handle gerenciado; zero representa referência null.
    pub fn reference(handle: i64) -> Self {
        Self {
            bits: handle,
            is_ref: true,
            tag: ValueTag::Ref,
        }
    }
}

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

    /// Quantidade de unidades UTF-16 (`String.length`).
    pub fn len(&self) -> usize {
        match self {
            Texto::Um(b) => b.len(),
            Texto::Dois(u) => u.len(),
        }
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
        match self {
            Texto::Um(b) => u16::from(b[i]),
            Texto::Dois(u) => u[i],
        }
    }

    /// As unidades, em ordem.
    pub fn unidades(&self) -> IterUnidades<'_> {
        IterUnidades { texto: self, i: 0, fim: self.len() }
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
        match self {
            Texto::Um(b) => Texto::Um(b[inicio..fim].to_vec()),
            Texto::Dois(u) => Self::de_fatia(&u[inicio..fim]),
        }
    }

    /// Os pontos de código (`runes`): um par de surrogates vira o escalar; um
    /// surrogate solto sai como ele mesmo (o `RuneIterator` do `dart:core`).
    pub fn pontos(&self) -> Vec<u32> {
        let mut saida = Vec::with_capacity(self.len());
        let n = self.len();
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

    /// WTF-8 (surrogate solto em três bytes): a forma sem perda, inversa de
    /// [`Texto::de_wtf8`].
    pub fn para_wtf8(&self) -> Vec<u8> {
        self.codificar(false)
    }

    /// UTF-8 como a VM escreve na saída (`print`): o surrogate solto vira
    /// U+FFFD — `Utf8::Encode` (`runtime/vm/unicode.cc`): "Encode unpaired
    /// surrogates as replacement characters to ensure the output is valid
    /// UTF-8".
    pub fn para_utf8_da_vm(&self) -> Vec<u8> {
        self.codificar(true)
    }

    fn codificar(&self, trocar_soltos: bool) -> Vec<u8> {
        match self {
            Texto::Um(b) if b.is_ascii() => b.clone(),
            _ => {
                let mut saida = Vec::with_capacity(self.len() + 8);
                for p in self.pontos() {
                    let p = if trocar_soltos && (0xD800..0xE000).contains(&p) { 0xFFFD } else { p };
                    codificar_wtf8(&mut saida, p);
                }
                saida
            }
        }
    }

    /// Texto legível para Rust: surrogate solto vira U+FFFD. Só para
    /// mensagens e formatação interna; o valor Dart nunca passa por aqui.
    pub fn para_string(&self) -> String {
        match self {
            Texto::Um(b) => b.iter().map(|&x| char::from(x)).collect(),
            Texto::Dois(u) => String::from_utf16_lossy(u),
        }
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
        (desde..=n - m).find(|&i| self.coincide_em(padrao, i))
    }

    /// Última ocorrência de `padrao` que começa em `ate` ou antes.
    pub fn procurar_ultimo(&self, padrao: &Texto, ate: usize) -> Option<usize> {
        let (n, m) = (self.len(), padrao.len());
        if m > n {
            return None;
        }
        let inicio = ate.min(n - m);
        (0..=inicio).rev().find(|&i| self.coincide_em(padrao, i))
    }

    /// `padrao` aparece inteiro a partir da unidade `i`.
    pub fn coincide_em(&self, padrao: &Texto, i: usize) -> bool {
        let m = padrao.len();
        if i + m > self.len() {
            return false;
        }
        match (self, padrao) {
            (Texto::Um(a), Texto::Um(b)) => &a[i..i + m] == b.as_slice(),
            (Texto::Dois(a), Texto::Dois(b)) => &a[i..i + m] == b.as_slice(),
            _ => (0..m).all(|k| self.unidade(i + k) == padrao.unidade(k)),
        }
    }

    /// Ordem lexicográfica por unidades (`String.compareTo`).
    pub fn comparar(&self, outro: &Texto) -> std::cmp::Ordering {
        match (self, outro) {
            (Texto::Um(a), Texto::Um(b)) => a.cmp(b),
            _ => self.unidades().cmp(outro.unidades()),
        }
    }

    /// `String.hashCode` da VM (`String_getHashCode`): o `StringHasher` de
    /// `runtime/vm/object.h` — `CombineHashes` (Jenkins *one-at-a-time*) por
    /// unidade de código e `FinalizeHash` com `String::kHashBits = 30`
    /// (`runtime/vm/hash.h`); 0 vira 1. A ordem de um `HashMap`/`HashSet` do
    /// `dart:collection` depende disto, então tem de ser o mesmo número.
    pub fn hash_vm(&self) -> i64 {
        let mut h: u32 = 0;
        for u in self.unidades() {
            h = h.wrapping_add(u32::from(u));
            h = h.wrapping_add(h << 10);
            h ^= h >> 6;
        }
        h = h.wrapping_add(h << 3);
        h ^= h >> 11;
        h = h.wrapping_add(h << 15);
        h &= (1u32 << 30) - 1;
        i64::from(if h == 0 { 1 } else { h })
    }

    /// Bytes do conteúdo (para os contadores do coletor).
    fn capacidade_bytes(&self) -> usize {
        match self {
            Texto::Um(b) => b.capacity(),
            Texto::Dois(u) => u.capacity().checked_mul(2).expect("payload excede usize"),
        }
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
        saida.extend_from_slice(&[
            0xE0 | (p >> 12) as u8,
            0x80 | ((p >> 6) & 0x3F) as u8,
            0x80 | (p & 0x3F) as u8,
        ]);
    } else {
        saida.extend_from_slice(&[
            0xF0 | (p >> 18) as u8,
            0x80 | ((p >> 12) & 0x3F) as u8,
            0x80 | ((p >> 6) & 0x3F) as u8,
            0x80 | (p & 0x3F) as u8,
        ]);
    }
}

/// Iterador das unidades de um [`Texto`].
pub struct IterUnidades<'a> {
    texto: &'a Texto,
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

impl ExactSizeIterator for IterUnidades<'_> {}

impl PartialEq for Texto {
    fn eq(&self, outro: &Texto) -> bool {
        match (self, outro) {
            (Texto::Um(a), Texto::Um(b)) => a == b,
            (Texto::Dois(a), Texto::Dois(b)) => a == b,
            _ => self.len() == outro.len() && self.unidades().eq(outro.unidades()),
        }
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
        *self == Texto::de_str(outro)
    }
}

impl PartialEq<&str> for Texto {
    fn eq(&self, outro: &&str) -> bool {
        *self == Texto::de_str(outro)
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
        self.0.extend(t.unidades());
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn fim(self) -> Texto {
        Texto::de_unidades(self.0)
    }
}

/// Valor gerenciado; somente campos marcados como referência participam do tracing.
#[derive(Debug)]
pub enum Value {
    /// `String` do Dart (`_OneByteString`/`_TwoByteString`, ver [`Texto`]).
    String(Texto),
    /// `StringBuffer`: as unidades acumuladas.
    StringBuffer(Vec<u16>),
    /// `RegExp`: o texto do padrão.
    RegExp(Texto),
    /// `Match`: o texto casado.
    Match(Texto),
    Object {
        class_id: i64,
        fields: Vec<(i64, bool)>,
    },
    /// Local capturado mutável compartilhado por ambientes distintos.
    Cell(TaggedValue),
    /// Capturas ordenadas imutáveis; mutabilidade compartilhada usa Cell.
    Environment(Vec<TaggedValue>),
    /// Identidade própria, código simbólico e ambiente; não executa código Rust/Dart.
    Closure {
        code_id: i64,
        environment: i64,
    },
    /// Lista expansível de payloads tipados para tracing, sem generics Dart ainda.
    List(Vec<TaggedValue>),
    /// Mapa de inserção ordenada, como o `LinkedHashMap` padrão de Dart.
    ///
    /// Chaves seguem `==` observável: escalares distinguem int de bool pelos
    /// bits e pela tag, e referências usam identidade, exceto strings, que
    /// comparam conteúdo. A busca é linear; adequada ao subconjunto, não a
    /// mapas grandes de produção.
    Map(Vec<(TaggedValue, TaggedValue)>),
    /// Conjunto de inserção ordenada, como o `LinkedHashSet` padrão de Dart.
    Set(Vec<TaggedValue>),
    /// Record do Dart: `(1, 'b')`
    Record(Vec<TaggedValue>),
    /// `int` numa posição `Ref` (R3 do contrato, docs/NATIVO-PLANO.md §6.2).
    /// Coleções nunca guardam a caixa: a entrada normaliza para o escalar.
    BoxedInt(i64),
    /// `double` numa posição `Ref`.
    BoxedDouble(f64),
    /// `bool` numa posição `Ref`: só existem dois, permanentes.
    BoxedBool(bool),
    /// Lista tipada interna (`_Uint8List`, `_Float64List`, … do
    /// `typed_data_patch.dart` da VM): a classe, o tipo do elemento
    /// (`typed_data.rs`, `TIPO_*`) e os bytes, no endian do hospedeiro.
    TypedData { class_id: i64, tipo: u8, bytes: Armazenamento },
    /// Visão sobre uma lista tipada interna (`_Uint8ArrayView`,
    /// `_ByteDataView`, …): a classe, o tipo do elemento, a lista de base,
    /// o deslocamento em bytes, o comprimento em elementos e se é não
    /// modificável (`_UnmodifiableXArrayView`, que rejeita escrita).
    TypedView { class_id: i64, tipo: u8, base: i64, deslocamento: usize, comprimento: usize, imutavel: bool },
}
/// Os bytes de uma lista tipada interna: próprios (do heap do runtime) ou
/// externos — a memória nativa de `Pointer.asTypedList`, que o Dart só vê
/// (o `ExternalTypedData` da VM): nem copiada nem liberada pelo coletor.
#[derive(Debug)]
pub enum Armazenamento {
    Proprio(Vec<u8>),
    Externo { endereco: usize, tamanho: usize },
}

impl Default for Armazenamento {
    fn default() -> Self {
        Armazenamento::Proprio(Vec::new())
    }
}

impl Clone for Armazenamento {
    /// Uma cópia de uma lista externa continua sobre a mesma memória nativa.
    fn clone(&self) -> Self {
        match self {
            Armazenamento::Proprio(v) => Armazenamento::Proprio(v.clone()),
            Armazenamento::Externo { endereco, tamanho } => Armazenamento::Externo { endereco: *endereco, tamanho: *tamanho },
        }
    }
}

impl From<Vec<u8>> for Armazenamento {
    fn from(v: Vec<u8>) -> Self {
        Armazenamento::Proprio(v)
    }
}

// A única leitura de memória nativa do heap: a de uma lista externa.
#[allow(unsafe_code)]
impl std::ops::Deref for Armazenamento {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Armazenamento::Proprio(v) => v,
            // SAFETY: a memória nativa de `asTypedList`, que o programa
            // garante viva e com `tamanho` bytes enquanto usa a lista (o
            // mesmo contrato da VM).
            Armazenamento::Externo { endereco, tamanho } => unsafe {
                std::slice::from_raw_parts(*endereco as *const u8, *tamanho)
            },
        }
    }
}

#[allow(unsafe_code)]
impl std::ops::DerefMut for Armazenamento {
    fn deref_mut(&mut self) -> &mut [u8] {
        match self {
            Armazenamento::Proprio(v) => v,
            // SAFETY: como em `deref`.
            Armazenamento::Externo { endereco, tamanho } => unsafe {
                std::slice::from_raw_parts_mut(*endereco as *mut u8, *tamanho)
            },
        }
    }
}

impl Armazenamento {
    /// Bytes que o heap do runtime ocupa (a memória externa não conta).
    pub fn capacity(&self) -> usize {
        match self {
            Armazenamento::Proprio(v) => v.capacity(),
            Armazenamento::Externo { .. } => 0,
        }
    }
}

impl Value {
    /// Estima armazenamento próprio usando capacidades efetivas, com overflow explícito.
    fn estimated_bytes(&self) -> usize {
        let payload = match self {
            Self::String(text) | Self::RegExp(text) | Self::Match(text) => text.capacidade_bytes(),
            Self::StringBuffer(v) => v.capacity().checked_mul(2).expect("payload excede usize"),
            Self::Object { fields, .. } => fields
                .capacity()
                .checked_mul(std::mem::size_of::<(i64, bool)>())
                .expect("payload excede usize"),
            Self::Cell(_) | Self::Closure { .. } | Self::BoxedInt(_) | Self::BoxedDouble(_) | Self::BoxedBool(_) => 0,
            Self::TypedData { bytes, .. } => bytes.capacity(),
            Self::TypedView { .. } => 0,
            Self::Environment(values) | Self::List(values) | Self::Set(values) | Self::Record(values) => values
                .capacity()
                .checked_mul(std::mem::size_of::<TaggedValue>())
                .expect("payload excede usize"),
            Self::Map(entries) => entries
                .capacity()
                .checked_mul(std::mem::size_of::<(TaggedValue, TaggedValue)>())
                .expect("payload excede usize"),
        };
        std::mem::size_of::<Self>()
            .checked_add(payload)
            .expect("payload excede usize")
    }
    /// Igualdade de chaves de `Map`/`Set` segundo `==` observável de Dart.
    fn trace(&self, pending: &mut Vec<i64>) {
        match self {
            Self::String(_)
            | Self::StringBuffer(_)
            | Self::RegExp(_)
            | Self::Match(_)
            | Self::BoxedInt(_)
            | Self::BoxedDouble(_)
            | Self::BoxedBool(_)
            | Self::TypedData { .. } => {}
            Self::TypedView { base, .. } => pending.push(*base),
            Self::Object { fields, .. } => pending.extend(
                fields
                    .iter()
                    .filter_map(|(bits, is_ref)| is_ref.then_some(*bits)),
            ),
            Self::Cell(value) => {
                if value.is_ref {
                    pending.push(value.bits);
                }
            }
            Self::Environment(values) | Self::List(values) | Self::Set(values) | Self::Record(values) => pending.extend(
                values
                    .iter()
                    .filter_map(|value| value.is_ref.then_some(value.bits)),
            ),
            Self::Map(entries) => pending.extend(entries.iter().flat_map(|(key, value)| {
                [key, value]
                    .into_iter()
                    .filter_map(|part| part.is_ref.then_some(part.bits))
            })),
            Self::Closure { environment, .. } => pending.push(*environment),
        }
    }
}

/// `Ref` com `Smi` etiquetado (R10, docs/NATIVO-PLANO.md §6.2 e §7.1).
///
/// Um `Ref` do código gerado é um `i64` com a etiqueta no bit baixo, como o
/// `Smi` da VM (`runtime/vm/object.h`; lá `kSmiTag = 0` e o ponteiro do heap
/// tem o bit 1 — aqui o bit está invertido para o `0` continuar sendo null
/// sem mudar o código gerado):
///
/// * `0` — null;
/// * **par** e positivo — um handle do heap: `(índice + 1) << 1`;
/// * **ímpar** — um `int` pequeno, `(v << 1) | 1`, para `v` em
///   `[-2^62, 2^62)`: o `Smi`. Não aloca, não é raiz, e o coletor nunca o
///   segue (as arestas ímpares são puladas na marcação).
///
/// Um `int` fora dessa faixa numa posição `Ref` vai para o heap como
/// `Value::BoxedInt` (o `_Mint` da VM). A forma é canônica: um valor que cabe
/// no `Smi` nunca é encaixotado, então dois `int` iguais em posição `Ref`
/// ou são o mesmo `Smi` (mesmos bits) ou são dois `_Mint` de mesmo valor.
pub mod smi {
    /// Menor e maior `int` que cabem num `Smi` (63 bits com sinal).
    pub const MIN: i64 = -(1 << 62);
    pub const MAX: i64 = (1 << 62) - 1;

    /// O `Ref` de um `int` que cabe num `Smi`.
    #[inline]
    pub fn de(v: i64) -> Option<i64> {
        (MIN..=MAX).contains(&v).then_some((v << 1) | 1)
    }

    /// O `Ref` é um `Smi`?
    #[inline]
    pub fn e_smi(r: i64) -> bool {
        r & 1 == 1
    }

    /// O valor de um `Smi` (deslocamento aritmético).
    #[inline]
    pub fn valor(r: i64) -> i64 {
        r >> 1
    }

    /// O `Ref` aponta para o heap (não é null nem `Smi`)?
    #[inline]
    pub fn e_handle(r: i64) -> bool {
        r != 0 && r & 1 == 0
    }
}

/// Um finalizador nativo e o dado dele (`Dart_NewFinalizableHandle`).
pub type Finalizador = (fn(usize), usize);

/// O quadro de raízes de uma função gerada (G1), no stack dela: o
/// anterior, o número de slots e os slots, que o código gerado grava com um
/// `store` comum (a pilha-sombra do LLVM, `ShadowStackGC`). O runtime só
/// encadeia e desencadeia o quadro ([`empilhar_quadro`]) e o percorre na
/// coleta; os quadros do próprio runtime continuam em `Heap::frames`.
#[repr(C)]
pub struct QuadroDeRaizes {
    anterior: *const QuadroDeRaizes,
    n: i64,
    slots: [i64; 0],
}

thread_local! {
    /// O quadro do topo da pilha-sombra desta thread (do isolado dela).
    static TOPO_DOS_QUADROS: std::cell::Cell<*const QuadroDeRaizes> = const { std::cell::Cell::new(std::ptr::null()) };
}

/// Encadeia `q` no topo da pilha-sombra. `q.n` e os slots (zerados) já
/// foram escritos pelo código gerado.
#[allow(unsafe_code)]
pub fn empilhar_quadro(q: *mut QuadroDeRaizes) {
    TOPO_DOS_QUADROS.with(|t| {
        // SAFETY: `q` é o quadro no stack da função que chama, vivo até o
        // `desempilhar_quadro` antes de cada retorno dela.
        unsafe { (*q).anterior = t.get() };
        t.set(q);
    });
}

/// Desencadeia `q`, que tem de ser o topo (os retornos fecham os quadros
/// em ordem, G3).
#[allow(unsafe_code)]
pub fn desempilhar_quadro(q: *const QuadroDeRaizes) {
    TOPO_DOS_QUADROS.with(|t| {
        assert!(std::ptr::eq(t.get(), q), "bug do compilador: quadro de raízes fechado fora de ordem");
        // SAFETY: `q` é o topo, ainda no stack de quem chama.
        t.set(unsafe { (*q).anterior });
    });
}

/// Visita as raízes de todos os quadros da pilha-sombra desta thread.
#[allow(unsafe_code)]
fn visitar_quadros(mut f: impl FnMut(i64)) {
    let mut q = TOPO_DOS_QUADROS.with(|t| t.get());
    while !q.is_null() {
        // SAFETY: cada quadro encadeado está no stack de uma função ainda
        // ativa desta thread, com `n` slots depois do cabeçalho.
        unsafe {
            let n = (*q).n as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in 0..n {
                f(*slots.add(i));
            }
            q = (*q).anterior;
        }
    }
}

/// Heap preciso sem compactação; handles pares indexam slots reutilizáveis.
#[derive(Debug)]
pub struct Heap {
    slots: Vec<Option<Value>>,
    /// Metadado de cada slot (o `metadata_ptr` do cabeçalho, NATIVO.md §2):
    /// o tipo em tempo de execução de um objeto genérico (P6/RTI,
    /// `tipos.rs`), `id + 1`; 0 = nenhum. Zerado a cada alocação do slot.
    metadados: Vec<i64>,
    free: Vec<usize>,
    frames: Vec<(i64, Vec<i64>)>,
    next_frame: i64,
    allocations: usize,
    threshold: usize,
    stress: bool,
    stats: HeapStats,
    marks: Vec<bool>,
    pending: Vec<i64>,
    byte_threshold: usize,
    /// Teto DURO do heap, em bytes.
    ///
    /// Nao confundir com `byte_threshold`, que e o gatilho de COLETA. Este e o
    /// limite do processo: um programa em laco infinito que aloca strings come
    /// a memoria da maquina inteira antes de qualquer tempo-limite do harness
    /// disparar (foi o que travou a maquina rodando o corpus nativo em
    /// paralelo). Ao estourar, o processo sai com mensagem legivel e codigo
    /// 255, que e o que a VM usa para excecao nao capturada — vira uma falha
    /// no placar em vez de um travamento.
    ///
    /// Padrao 256 MiB; `DARTFORGE_HEAP_MAX_MB` ajusta, e 0 desliga o teto.
    limite_bytes: usize,
    enum_values: crate::hash::HashMap<(i64, i64), i64>,
    /// Tear-offs canônicos de funções top-level, por ID de código.
    ///
    /// O oráculo Dart 3.6.2 exige `identical(f, f)` verdadeiro para dois
    /// tear-offs da mesma função top-level; cada `code_id` tem um único handle,
    /// mantido vivo como raiz permanente, como os singletons de enum.
    tearoffs: crate::hash::HashMap<i64, i64>,
    /// Literais do compilador, canônicos por unidades UTF-16. Permanecem
    /// enraizados pelo isolate; strings criadas em execução não entram aqui.
    literais: crate::hash::HashMap<Vec<u16>, i64>,
    /// Objetos permanentes e imutáveis que uma mensagem entre portas do
    /// mesmo isolado passa pela identidade, sem copiar (a VM compartilha os
    /// profundamente imutáveis): constantes canônicas, valores de enum e
    /// globais `const` (o código gerado os marca, `dartforge_marcar_permanente`),
    /// tear-offs de topo e literais.
    permanentes: crate::hash::HashSet<i64>,
    /// Constante canônica → o getter gerado que a produz (o mesmo endereço
    /// em todos os isolados): uma mensagem para outro isolado leva o getter,
    /// e o destino recebe a própria instância canônica (`identical` entre
    /// isolados, como os objetos compartilhados do grupo da VM).
    constantes: crate::hash::HashMap<i64, usize>,
    /// Tear-off canônico → o código dele (o inverso de `tearoffs`).
    codigo_do_tearoff: crate::hash::HashMap<i64, i64>,
    /// `DARTFORGE_GC_OFF=1`: nunca coleta. Instrumento de diagnóstico
    /// (docs/NATIVO-PLANO.md §6): um programa que morre com "handle já
    /// coletado" e passa com a coleta desligada tem raiz faltando; um que
    /// morre igual nos dois modos tem escalar usado como handle.
    gc_desligado: bool,
    /// Valor corrente de cada global `Ref` do programa (variável de topo ou
    /// campo estático), por id: raízes permanentes (N6/G6 do contrato).
    globais: crate::hash::HashMap<i64, i64>,
    /// As caixas de `false` e `true` (0 = ainda não alocada), permanentes.
    caixas_bool: [i64; 2],
    /// Raízes que moram no runtime e não num frame (G6): a exceção pendente
    /// e o rastro corrente. 0 = nenhuma.
    raizes_do_runtime: [i64; 2],
    /// Tabelas laterais indexadas por handle. Moram aqui, e não em
    /// `thread_local`s do runtime, porque são purgadas a cada coleta (G6):
    /// um slot reutilizado herdaria a marca "imutável" ou "em iteração" de
    /// outro objeto.
    pub imutaveis: crate::hash::HashSet<i64>,
    /// Campos `late` já escritos, por handle e índice físico. A marca fica
    /// fora do valor: zero e null são atribuições válidas do programa.
    pub campos_late_inicializados: crate::hash::HashSet<(i64, i64)>,
    /// Listas de tamanho fixo (`_List` do SDK da fonte, P5c).
    pub fixas: crate::hash::HashSet<i64>,
    /// `_GrowableList` criada por `_withData(data)` (P5c): o vetor tem os
    /// elementos de `data` (a reserva) e o tamanho lógico ainda é este, até
    /// o primeiro `_setLength`/`_setData` — na VM a lista aponta para o
    /// `_List` e o tamanho é outro campo.
    pub pendentes: crate::hash::HashMap<i64, usize>,
    pub iteracoes_ativas: crate::hash::HashSet<i64>,
    /// Lista de chaves → mapa de origem (para acusar modificação do mapa
    /// durante a iteração das chaves).
    pub origens: crate::hash::HashMap<i64, i64>,
    /// Pares nativos finalizáveis (o `Dart_NewFinalizableHandle` da VM):
    /// objeto → (finalizador, par). Quando o objeto morre, a coleta tira a
    /// entrada e chama `finalizador(par)` — que só libera recursos do
    /// sistema (fecha um arquivo, solta uma contagem de referências) e nunca
    /// toca o heap.
    pub finalizaveis: crate::hash::HashMap<i64, Finalizador>,
    /// As referências fracas (`WeakReference`, o `WeakReference_*` da VM):
    /// objeto portador → alvo. O alvo NÃO é seguido pela marcação; se não
    /// sobreviver por outro caminho, a coleta o troca por 0 (null).
    pub fracas: crate::hash::HashMap<i64, i64>,
    /// Os efêmeros (`_WeakProperty`, a base do `Expando`): portador →
    /// (chave, valor). O valor só é alcançado se a chave for (ponto fixo na
    /// marcação); chave morta zera os dois.
    pub efemeros: crate::hash::HashMap<i64, (i64, i64)>,
    /// Os anexos de `Finalizer`/`NativeFinalizer` (o `FinalizerEntry` da
    /// VM): o valor e a chave de `detach` são fracos; o dono e a ação,
    /// fortes. Valor morto: a ação de um `Finalizer` vai para
    /// [`Heap::finalizacoes_prontas`] (o laço de eventos a chama); a de um
    /// `NativeFinalizer` roda logo depois da coleta.
    pub anexos: Vec<AnexoDeFinalizador>,
    /// As ações de `Finalizer` cujo valor morreu, à espera do laço de
    /// eventos (raízes até lá).
    pub finalizacoes_prontas: std::collections::VecDeque<i64>,
}

/// Um anexo de finalizador (ver [`Heap::anexos`]).
#[derive(Debug, Clone, Copy)]
pub struct AnexoDeFinalizador {
    /// O `Finalizer`/`NativeFinalizer` (identidade do `detach`).
    pub dono: i64,
    pub valor: i64,
    /// A chave de `detach` (0 = nenhuma).
    pub desanexo: i64,
    pub acao: AcaoDeFinalizador,
}

/// O que um finalizador faz quando o valor morre.
#[derive(Debug, Clone, Copy)]
pub enum AcaoDeFinalizador {
    /// Uma closure Dart sem argumentos (`callback(token)` já aplicada).
    Dart(i64),
    /// `funcao(token)`, uma função C (`NativeFinalizerFunction`).
    Nativa(usize, usize),
}

#[allow(unsafe_code)]
impl Heap {
    /// Roda as ações nativas de todos os anexos ainda vivos (o isolado
    /// terminou: a VM garante os `NativeFinalizer` no encerramento) e
    /// descarta os de `Finalizer`.
    pub fn encerrar_finalizadores(&mut self) {
        let anexos = std::mem::take(&mut self.anexos);
        self.finalizacoes_prontas.clear();
        for a in anexos {
            if let AcaoDeFinalizador::Nativa(f, token) = a.acao {
                // SAFETY: `f` é a `NativeFinalizerFunction` que o programa
                // registrou, `void f(void* token)`.
                let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
                f(token);
            }
        }
    }
}
impl Heap {
    /// Inicializa heap; stress força coleta antes de cada alocação.
    pub fn new(stress: bool) -> Self {
        Self {
            slots: Vec::new(),
            metadados: Vec::new(),
            free: Vec::new(),
            frames: Vec::new(),
            next_frame: 1,
            allocations: 0,
            threshold: 256,
            stress,
            stats: HeapStats::default(),
            marks: Vec::new(),
            pending: Vec::new(),
            byte_threshold: 1024 * 1024,
            limite_bytes: Self::limite_do_ambiente(),
            enum_values: crate::hash::HashMap::default(),
            tearoffs: crate::hash::HashMap::default(),
            literais: crate::hash::HashMap::default(),
            permanentes: crate::hash::HashSet::default(),
            constantes: crate::hash::HashMap::default(),
            codigo_do_tearoff: crate::hash::HashMap::default(),
            gc_desligado: std::env::var("DARTFORGE_GC_OFF").as_deref() == Ok("1"),
            globais: crate::hash::HashMap::default(),
            caixas_bool: [0, 0],
            raizes_do_runtime: [0, 0],
            imutaveis: crate::hash::HashSet::default(),
            campos_late_inicializados: crate::hash::HashSet::default(),
            fixas: crate::hash::HashSet::default(),
            pendentes: crate::hash::HashMap::default(),
            iteracoes_ativas: crate::hash::HashSet::default(),
            origens: crate::hash::HashMap::default(),
            finalizaveis: crate::hash::HashMap::default(),
            fracas: crate::hash::HashMap::default(),
            efemeros: crate::hash::HashMap::default(),
            anexos: Vec::new(),
            finalizacoes_prontas: std::collections::VecDeque::new(),
        }
    }
    /// Raiz mantida pelo runtime: 0 = exceção pendente, 1 = rastro corrente.
    pub fn set_raiz_do_runtime(&mut self, qual: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.get(handle);
        }
        self.raizes_do_runtime[qual] = handle;
    }
    /// Caixa de um `bool` (R3): um dos dois singletons permanentes.
    pub fn caixa_bool(&mut self, valor: bool) -> i64 {
        let i = usize::from(valor);
        if self.caixas_bool[i] == 0 {
            self.caixas_bool[i] = self.allocate(Value::BoxedBool(valor));
        }
        self.caixas_bool[i]
    }
    /// Retorna o mesmo objeto para literais de mesmo conteúdo, inclusive
    /// quando vieram de módulos LLVM diferentes.
    pub fn string_literal(&mut self, texto: Texto) -> i64 {
        let chave: Vec<u16> = texto.unidades().collect();
        if let Some(&handle) = self.literais.get(&chave) {
            return handle;
        }
        let handle = self.allocate(Value::String(texto));
        self.literais.insert(chave, handle);
        self.permanentes.insert(handle);
        handle
    }
    /// Valor como referência: escalar vira caixa; referência passa direto.
    /// Um `int` que cabe no `Smi` (R10) não aloca. Quem chama enraíza o
    /// resultado antes da próxima alocação.
    pub fn como_ref(&mut self, v: TaggedValue) -> i64 {
        match v.tag {
            ValueTag::Ref => v.bits,
            ValueTag::Int => self.caixa_int(v.bits),
            ValueTag::Double => self.allocate(Value::BoxedDouble(f64::from_bits(v.bits as u64))),
            ValueTag::Bool => self.caixa_bool(v.bits != 0),
        }
    }
    /// `int` numa posição `Ref` (R3/R10): `Smi` quando cabe, senão `_Mint`.
    pub fn caixa_int(&mut self, v: i64) -> i64 {
        match smi::de(v) {
            Some(r) => {
                self.stats.caixas_evitadas += 1;
                r
            }
            None => self.allocate(Value::BoxedInt(v)),
        }
    }
    /// O `int` de um `Ref`: `Smi` ou `_Mint`; outro valor dá `None`.
    pub fn int_de_ref(&self, r: i64) -> Option<i64> {
        if smi::e_smi(r) {
            return Some(smi::valor(r));
        }
        match self.try_get(r) {
            Some(Value::BoxedInt(i)) => Some(*i),
            _ => None,
        }
    }
    /// Referência para uma caixa vira o escalar (R8): coleções, células e
    /// ambientes nunca guardam caixas, então `[1]` e `<Object>[1]` têm o
    /// mesmo conteúdo e a chave `1` é a mesma encaixotada ou não. Um `Smi`
    /// também vira o escalar.
    pub fn normalizar(&self, v: TaggedValue) -> TaggedValue {
        if !v.is_ref || v.bits == 0 {
            return v;
        }
        if smi::e_smi(v.bits) {
            return TaggedValue::scalar(smi::valor(v.bits));
        }
        match self.try_get(v.bits) {
            Some(Value::BoxedInt(i)) => TaggedValue::scalar(*i),
            Some(Value::BoxedDouble(d)) => TaggedValue::double(*d),
            Some(Value::BoxedBool(b)) => TaggedValue::boolean(*b),
            _ => v,
        }
    }
    /// Registra o valor corrente de um global `Ref`; 0 (null) solta a raiz.
    pub fn set_global_root(&mut self, id: i64, handle: i64) {
        if !smi::e_handle(handle) {
            // null ou `Smi`: nada no heap a manter vivo.
            self.globais.remove(&id);
        } else {
            self.get(handle);
            self.globais.insert(id, handle);
        }
    }

    /// Move a raiz de um global de um endereço de slot para outro (a área de
    /// globais de um módulo recarregado, `gc_raizes.rs`).
    pub fn mover_raiz_global(&mut self, de: i64, para: i64) {
        if let Some(h) = self.globais.remove(&de) {
            self.globais.insert(para, h);
        }
    }

    /// Solta a raiz de um global que deixou de existir.
    pub fn soltar_raiz_global(&mut self, id: i64) {
        self.globais.remove(&id);
    }

    /// Marca `handle` como permanente (ver `permanentes`).
    pub fn marcar_permanente(&mut self, handle: i64) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
        }
    }

    /// Marca `handle` como a constante canônica que `getter` produz.
    pub fn marcar_constante(&mut self, handle: i64, getter: usize) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
            if getter != 0 {
                self.constantes.insert(handle, getter);
            }
        }
    }

    /// O getter da constante canônica `handle`, se for uma.
    pub fn getter_da_constante(&self, handle: i64) -> Option<usize> {
        self.constantes.get(&handle).copied()
    }

    /// O código do tear-off canônico `handle`, se for um.
    pub fn codigo_do_tearoff(&self, handle: i64) -> Option<i64> {
        self.codigo_do_tearoff.get(&handle).copied()
    }

    /// Se `handle` é permanente e imutável (ver `permanentes`): uma
    /// mensagem no mesmo isolado o passa sem copiar.
    pub fn e_permanente(&self, handle: i64) -> bool {
        self.permanentes.contains(&handle) || self.caixas_bool.contains(&handle)
    }
    /// Le o teto do heap do ambiente uma vez, na criacao.
    fn limite_do_ambiente() -> usize {
        const PADRAO: usize = 256 * 1024 * 1024;
        match std::env::var("DARTFORGE_HEAP_MAX_MB") {
            Ok(v) => match v.trim().parse::<usize>() {
                Ok(0) => usize::MAX,
                Ok(mb) => mb.saturating_mul(1024 * 1024),
                Err(_) => PADRAO,
            },
            Err(_) => PADRAO,
        }
    }

    /// Bytes que o heap ocupa, contando tambem a tabela de slots.
    ///
    /// `stats.estimated_bytes` so mede o conteudo dos valores; um programa que
    /// aloca milhoes de objetos minusculos cresce pela tabela, nao pelo
    /// conteudo, e passaria pelo teto sem ele.
    fn bytes_totais(&self, adicional: usize) -> usize {
        let tabela = self
            .slots
            .len()
            .saturating_mul(std::mem::size_of::<Option<Value>>());
        self.stats
            .estimated_bytes
            .saturating_add(tabela)
            .saturating_add(adicional)
    }

    /// Para o processo quando o heap passa do teto.
    fn verificar_teto(&self, adicional: usize) {
        if self.limite_bytes == usize::MAX {
            return;
        }
        let total = self.bytes_totais(adicional);
        if total > self.limite_bytes {
            Self::abortar_por_memoria(total, self.limite_bytes);
        }
    }

    #[cold]
    #[inline(never)]
    fn abortar_por_memoria(total: usize, limite: usize) -> ! {
        // Sem `panic!`: o runtime e chamado por `extern "C"` a partir do codigo
        // gerado, e um panico atravessando essa fronteira aborta com um despejo
        // de pilha ilegivel. Uma linha e o codigo 255 (o mesmo da VM para
        // excecao nao capturada) sao o que o harness precisa.
        let mib = 1024 * 1024;
        eprintln!(
            "Out of memory: heap do DartForge chegou a {} MiB, acima do teto de {} MiB (ajuste com DARTFORGE_HEAP_MAX_MB, 0 desliga).",
            total / mib,
            limite / mib
        );
        std::process::exit(255);
    }

    /// Obtém o singleton de um valor enum, protegendo as alocações internas.
    pub fn enum_value(&mut self, class_id: i64, index: i64, name: &str) -> i64 {
        assert!(class_id >= 0 && index >= 0, "identidade enum inválida");
        if let Some(&handle) = self.enum_values.get(&(class_id, index)) {
            return handle;
        }
        let frame = self.push_frame_with_slots(1);
        let text = self.allocate(Value::String(Texto::de_str(name)));
        self.set_root(frame, 0, text);
        let object = self.allocate(Value::Object {
            class_id,
            fields: vec![(index, false), (text, true)],
        });
        self.enum_values.insert((class_id, index), object);
        self.permanentes.insert(object);
        self.pop_frame(frame);
        object
    }
    /// Obtém o tear-off canônico de uma função top-level, criando-o uma vez.
    ///
    /// O ID de código é o índice da função no módulo; o ambiente é vazio e o
    /// handle devolvido é estável entre chamadas, preservando `identical`.
    /// Tear-offs de métodos (com receptor capturado) não passam por aqui:
    /// cada avaliação cria uma closure nova.
    pub fn tearoff(&mut self, code_id: i64) -> i64 {
        assert!(code_id >= 0, "ID de código inválido");
        if let Some(&handle) = self.tearoffs.get(&code_id) {
            return handle;
        }
        let frame = self.push_frame_with_slots(1);
        let env = self.create_environment(Vec::new());
        self.set_root(frame, 0, env);
        let closure = self.create_closure(code_id, env);
        self.tearoffs.insert(code_id, closure);
        self.codigo_do_tearoff.insert(closure, code_id);
        self.permanentes.insert(closure);
        self.pop_frame(frame);
        closure
    }
    /// Abre frame de raízes com identificador monotônico.
    pub fn push_frame(&mut self) -> i64 {
        self.push_frame_with_slots(0)
    }
    /// Reserva slots fixos inicialmente null, reutilizados por todas as iterações.
    pub fn push_frame_with_slots(&mut self, slots: usize) -> i64 {
        let id = self.next_frame;
        self.next_frame = id.checked_add(1).expect("frames esgotados");
        self.stats.root_slots = self
            .stats
            .root_slots
            .checked_add(slots)
            .expect("slots excedem usize");
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
        self.frames.push((id, vec![0; slots]));
        id
    }
    /// Substitui a raiz do slot; zero libera a referência anteriormente retida.
    /// Um `Smi` ocupa o slot como qualquer `Ref`, mas o coletor não o segue.
    pub fn set_root(&mut self, frame: i64, slot: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.get(handle);
        }
        let roots = &mut self
            .frames
            .iter_mut()
            .rev()
            .find(|(id, _)| *id == frame)
            .expect("frame inexistente")
            .1;
        let previous = roots.get_mut(slot).expect("slot de raiz inválido");
        self.stats.live_roots -= usize::from(*previous != 0);
        self.stats.live_roots += usize::from(handle != 0);
        *previous = handle;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
    }
    /// Protege handle até o retorno da função; null não ocupa uma raiz.
    pub fn root(&mut self, frame: i64, handle: i64) {
        if !smi::e_handle(handle) {
            return;
        }
        self.get(handle);
        self.frames
            .iter_mut()
            .rev()
            .find(|(id, _)| *id == frame)
            .expect("frame inexistente")
            .1
            .push(handle);
        self.stats.live_roots += 1;
        self.stats.root_slots += 1;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
    }
    /// Fecha exatamente o frame do topo, sem coletar entre retorno e raiz do chamador.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _)| *id), Some(frame));
        let (_, roots) = self.frames.pop().unwrap();
        self.stats.root_slots -= roots.len();
        self.stats.live_roots -= roots.iter().filter(|handle| **handle != 0).count();
    }
    /// Aloca após coleta; o chamador deve proteger o resultado antes de outra alocação.
    pub fn allocate(&mut self, value: Value) -> i64 {
        let bytes = value.estimated_bytes();
        // Dois limites diferentes (G6):
        // * o GATILHO (`threshold`, `byte_threshold`): quanto se aloca desde
        //   a última coleta, recalculado no fim de cada uma
        //   (`recalcular_gatilhos`) a partir dos sobreviventes e da folga até
        //   o teto;
        // * o TETO (`limite_bytes`): só a alocação que passaria dele força
        //   uma última coleta; se ainda passa, falta memória.
        // O portão antigo (`!self.frames.is_empty()`) existia porque o código
        // gerado não registrava raízes; com o frame de cada função (G1),
        // coletar sem frame aberto é só coletar com as raízes permanentes.
        let passa_do_teto = self.limite_bytes != usize::MAX && self.bytes_totais(bytes) > self.limite_bytes;
        if !self.gc_desligado
            && (self.stress
                || self.allocations >= self.threshold
                || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
                || passa_do_teto)
        {
            self.collect();
        }
        // Depois da coleta: se ainda passa do teto, nao ha o que recuperar.
        self.verificar_teto(bytes);
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(bytes)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
        self.allocations += 1;
        self.stats.allocations += 1;
        let index = if let Some(index) = self.free.pop() {
            self.slots[index] = Some(value);
            self.metadados[index] = 0;
            index
        } else {
            self.slots.push(Some(value));
            self.metadados.push(0);
            self.slots.len() - 1
        };
        Self::handle_de_indice(index)
    }
    /// O metadado do slot de um handle vivo (0 = nenhum).
    pub fn metadado(&self, handle: i64) -> i64 {
        let i = self.indice_vivo(handle);
        self.metadados[i]
    }
    /// Grava o metadado do slot de um handle vivo.
    pub fn set_metadado(&mut self, handle: i64, valor: i64) {
        let i = self.indice_vivo(handle);
        self.metadados[i] = valor;
    }
    /// O handle (par) do slot `index` (R10).
    fn handle_de_indice(index: usize) -> i64 {
        i64::try_from(index + 1)
            .ok()
            .and_then(|h| h.checked_shl(1).filter(|x| x >> 1 == h))
            .expect("handles esgotados")
    }
    /// O slot de um handle par, sem verificar se está vivo.
    pub fn indice_de(handle: i64) -> usize {
        usize::try_from((handle >> 1) - 1).expect("handle positivo")
    }
    /// Índice do slot de um handle vivo, ou `panic` que diz QUAL contrato
    /// foi quebrado (docs/NATIVO-PLANO.md §6.4, N4).
    ///
    /// As quatro falhas têm causas diferentes e a mensagem é a chave de
    /// agrupamento do harness, então cada uma tem texto fixo na primeira
    /// linha (sem o número do handle, que separaria o grupo) e o detalhe
    /// na segunda:
    /// * `0` — null desreferenciado: o lowering devia ter testado antes;
    /// * negativo — escalar (ou -2 de "classe String") usado como handle;
    /// * além da tabela — escalar positivo usado como handle;
    /// * slot vazio — o objeto foi coletado: faltou raiz.
    fn indice_vivo(&self, handle: i64) -> usize {
        if handle == 0 {
            panic!("bug do compilador: handle null (0) desreferenciado");
        }
        if smi::e_smi(handle) {
            panic!(
                "bug do compilador: Smi usado como handle (int em posição Ref lido como objeto)\nSmi {}",
                smi::valor(handle)
            );
        }
        if handle < 0 {
            panic!("bug do compilador: handle negativo (escalar usado como handle)\nhandle {handle}");
        }
        let index = Self::indice_de(handle);
        match self.slots.get(index) {
            None => panic!(
                "bug do compilador: handle além da tabela (escalar usado como handle)\nhandle {handle}, {} slots",
                self.slots.len()
            ),
            Some(None) => panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}"),
            Some(Some(_)) => index,
        }
    }
    /// Obtém valor vivo; o protocolo ABI não permite handles obsoletos.
    pub fn get(&self, handle: i64) -> &Value {
        let index = self.indice_vivo(handle);
        self.slots[index].as_ref().expect("slot vivo verificado")
    }
    /// O endereço dos campos do objeto `handle`, numa busca só (o caminho
    /// quente de `dartforge_object_campos`); `None` se não é objeto vivo.
    #[inline]
    pub fn campos_de_objeto(&mut self, handle: i64) -> Option<*mut (i64, bool)> {
        if !smi::e_handle(handle) || handle < 0 {
            return None;
        }
        match self.slots.get_mut(Self::indice_de(handle)) {
            Some(Some(Value::Object { fields, .. })) => Some(fields.as_mut_ptr()),
            _ => None,
        }
    }
    /// Obtém valor vivo se o handle for válido, ou None se inválido/destruído.
    /// Null, `Smi` e escalar qualquer dão `None`.
    pub fn try_get(&self, handle: i64) -> Option<&Value> {
        if !smi::e_handle(handle) || handle < 0 {
            return None;
        }
        self.slots.get(Self::indice_de(handle)).and_then(Option::as_ref)
    }
    /// Atualiza campo com tag explícita; valores escalares jamais são raízes.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        if is_ref && smi::e_handle(bits) {
            self.get(bits);
        }
        let slot = self.indice_vivo(handle);
        let Value::Object { fields, .. } = self.slots[slot].as_mut().expect("slot vivo verificado") else {
            panic!("objeto esperado")
        };
        fields[usize::try_from(index).expect("índice inválido")] = (bits, is_ref);
    }
    /// Aloca protegendo as referências do payload contra a coleta anterior à alocação.
    fn allocate_linked(&mut self, value: Value) -> i64 {
        let mut references = Vec::new();
        value.trace(&mut references);
        references.retain(|&h| smi::e_handle(h));
        for &handle in &references {
            self.get(handle);
        }
        let frame = self.push_frame_with_slots(references.len());
        for (slot, handle) in references.into_iter().enumerate() {
            self.set_root(frame, slot, handle);
        }
        let handle = self.allocate(value);
        self.pop_frame(frame);
        handle
    }
    /// Igualdade de chaves de `Map`/`Set` segundo `==` observável de Dart.
    ///
    /// Escalares comparam tag e bits (`0` int difere de `false`); referências
    /// null só igualam null; strings comparam conteúdo e demais referências,
    /// identidade de handle.
    pub fn key_equal(&self, left: &TaggedValue, right: &TaggedValue) -> bool {
        match (left.tag, right.tag) {
            (ValueTag::Int, ValueTag::Int)
            | (ValueTag::Bool, ValueTag::Bool)
            | (ValueTag::Double, ValueTag::Double) => {
                left.bits == right.bits
            }
            (ValueTag::Ref, ValueTag::Ref) => {
                if left.bits == 0 || right.bits == 0 {
                    return left.bits == right.bits;
                }
                if left.bits == right.bits {
                    return true;
                }
                self.string_equal(left.bits, right.bits)
            }
            _ => false,
        }
    }
    /// Cria célula compartilhável; proteja o resultado antes da próxima alocação.
    pub fn create_cell(&mut self, value: TaggedValue) -> i64 {
        let value = self.normalizar(value);
        self.allocate_linked(Value::Cell(value))
    }
    /// Lê captura mutável sem copiar o objeto apontado por uma referência.
    pub fn cell_get(&self, handle: i64) -> TaggedValue {
        match self.get(handle) {
            Value::Cell(value) => *value,
            _ => panic!("célula esperada"),
        }
    }
    /// Muda a captura observada por todos os ambientes que compartilham esta célula.
    pub fn cell_set(&mut self, handle: i64, value: TaggedValue) {
        let value = self.normalizar(value);
        self.validate_tag(value);
        let Value::Cell(current) = self.get_mut(handle) else {
            panic!("célula esperada")
        };
        *current = value;
    }
    /// Cria ambiente imutável; cada captura mutável deve apontar para uma célula.
    pub fn create_environment(&mut self, captures: Vec<TaggedValue>) -> i64 {
        let captures = captures.into_iter().map(|v| self.normalizar(v)).collect();
        self.allocate_linked(Value::Environment(captures))
    }
    /// Obtém captura por índice; índice inválido provoca panic, sem acesso inseguro.
    pub fn environment_get(&self, handle: i64, index: usize) -> TaggedValue {
        match self.get(handle) {
            Value::Environment(captures) => captures[index],
            _ => panic!("ambiente esperado"),
        }
    }
    /// Cria nova identidade de closure mesmo para o mesmo código e ambiente.
    /// O ID de código é simbólico: esta API não realiza despacho nem execução Dart.
    pub fn create_closure(&mut self, code_id: i64, environment: i64) -> i64 {
        assert!(code_id >= 0, "ID de código inválido");
        assert!(
            matches!(self.get(environment), Value::Environment(_)),
            "ambiente esperado"
        );
        self.allocate_linked(Value::Closure {
            code_id,
            environment,
        })
    }
    /// Retorna código simbólico e ambiente, preservando a identidade do handle.
    pub fn closure_parts(&self, handle: i64) -> (i64, i64) {
        match self.get(handle) {
            Value::Closure {
                code_id,
                environment,
            } => (*code_id, *environment),
            _ => panic!("closure esperada"),
        }
    }
    /// Cria lista expansível com tracing preciso de seus elementos gerenciados.
    pub fn create_list(&mut self, values: Vec<TaggedValue>) -> i64 {
        let values = values.into_iter().map(|v| self.normalizar(v)).collect();
        self.allocate_linked(Value::List(values))
    }
    /// Quantidade de elementos inicializados; capacidade interna não é comprimento.
    pub fn list_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::List(values) => values.len(),
            _ => panic!("lista esperada"),
        }
    }
    /// Lê o elemento sem alterar sua tag ou identidade.
    pub fn list_get(&self, handle: i64, index: usize) -> TaggedValue {
        match self.get(handle) {
            Value::List(values) => values[index],
            _ => panic!("lista esperada"),
        }
    }
    /// Substitui elemento existente; referências removidas deixam de ser rastreadas.
    pub fn list_set(&mut self, handle: i64, index: usize, value: TaggedValue) {
        let value = self.normalizar(value);
        self.validate_tag(value);
        let Value::List(values) = self.get_mut(handle) else {
            panic!("lista esperada")
        };
        values[index] = value;
    }
    /// Acrescenta elemento e contabiliza capacidade real do buffer na política de GC.
    /// A lista permanece protegida durante eventual coleta causada pelo crescimento.
    pub fn list_push(&mut self, handle: i64, value: TaggedValue) {
        let value = self.normalizar(value);
        self.validate_tag(value);
        let previous = self.get(handle).estimated_bytes();
        let Value::List(values) = self.get_mut(handle) else {
            panic!("lista esperada")
        };
        values.push(value);
        let added = self.get(handle).estimated_bytes() - previous;
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(added)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
        self.verificar_teto(0);
        if !self.gc_desligado && (self.stress || self.stats.estimated_bytes > self.byte_threshold) {
            let frame = self.push_frame_with_slots(1);
            self.set_root(frame, 0, handle);
            self.collect();
            self.pop_frame(frame);
        }
    }
    /// Cria mapa com ordem de inserção; chaves duplicadas conservam a última.
    ///
    /// A entrada duplicada mantém a posição da primeira ocorrência e o valor da
    /// última, como o literal de mapa do SDK 3.6.2. O chamador enraíza o
    /// resultado antes da próxima operação que possa coletar.
    pub fn create_map(&mut self, entries: Vec<(TaggedValue, TaggedValue)>) -> i64 {
        let mut unique: Vec<(TaggedValue, TaggedValue)> = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let key = self.normalizar(key);
            let value = self.normalizar(value);
            self.validate_tag(key);
            self.validate_tag(value);
            if let Some(slot) = unique.iter_mut().find(|(existing, _)| {
                // `find` não tem acesso a `self` sem conflito de empréstimo;
                // a comparação repete `key_equal` sobre chaves já validadas.
                self.key_equal(existing, &key)
            }) {
                slot.1 = value;
            } else {
                unique.push((key, value));
            }
        }
        self.allocate_linked(Value::Map(unique))
    }
    /// Quantidade de pares; capacidade interna não é comprimento.
    pub fn map_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::Map(entries) => entries.len(),
            _ => panic!("mapa esperado"),
        }
    }
    /// Diz se a chave existe, sem distinguir valor null de ausência pelo valor.
    pub fn map_contains(&self, handle: i64, key: TaggedValue) -> bool {
        let key = self.normalizar(key);
        match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .any(|(existing, _)| self.key_equal(existing, &key)),
            _ => panic!("mapa esperado"),
        }
    }
    /// Obtém o valor da chave; ausência provoca panic, sem acesso inseguro.
    ///
    /// O protocolo LLVM consulta `map_contains` antes; esta API não devolve
    /// "null por ausência" para não confundir valor null armazenado com falta.
    pub fn map_get(&self, handle: i64, key: TaggedValue) -> TaggedValue {
        let key = self.normalizar(key);
        match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .find(|(existing, _)| self.key_equal(existing, &key))
                .map(|(_, value)| *value)
                .expect("chave ausente"),
            _ => panic!("mapa esperado"),
        }
    }
    /// Insere ou substitui, preservando a posição da primeira ocorrência.
    pub fn map_set(&mut self, handle: i64, key: TaggedValue, value: TaggedValue) {
        let key = self.normalizar(key);
        let value = self.normalizar(value);
        self.validate_tag(key);
        self.validate_tag(value);
        let index = match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .position(|(existing, _)| self.key_equal(existing, &key)),
            _ => panic!("mapa esperado"),
        };
        let Value::Map(entries) = self.get_mut(handle) else {
            panic!("mapa esperado")
        };
        if let Some(index) = index {
            entries[index].1 = value;
        } else {
            entries.push((key, value));
        }
    }
    /// Cria conjunto com ordem de inserção; duplicadas conservam a primeira.
    pub fn create_set(&mut self, values: Vec<TaggedValue>) -> i64 {
        let mut unique = Vec::with_capacity(values.len());
        for value in values {
            let value = self.normalizar(value);
            self.validate_tag(value);
            if !unique
                .iter()
                .any(|existing| self.key_equal(existing, &value))
            {
                unique.push(value);
            }
        }
        self.allocate_linked(Value::Set(unique))
    }
    /// Quantidade de elementos distintos.
    pub fn set_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::Set(values) => values.len(),
            _ => panic!("conjunto esperado"),
        }
    }
    /// Pertinência segundo `==` observável de chaves.
    pub fn set_contains(&self, handle: i64, value: TaggedValue) -> bool {
        let value = self.normalizar(value);
        match self.get(handle) {
            Value::Set(values) => values
                .iter()
                .any(|existing| self.key_equal(existing, &value)),
            _ => panic!("conjunto esperado"),
        }
    }
    /// Insere quando ausente; devolve se houve inserção (`Set.add` de Dart).
    pub fn set_add(&mut self, handle: i64, value: TaggedValue) -> bool {
        let value = self.normalizar(value);
        self.validate_tag(value);
        let present = match self.get(handle) {
            Value::Set(values) => values
                .iter()
                .any(|existing| self.key_equal(existing, &value)),
            _ => panic!("conjunto esperado"),
        };
        if present {
            return false;
        }
        let previous = self.get(handle).estimated_bytes();
        let Value::Set(values) = self.get_mut(handle) else {
            panic!("conjunto esperado")
        };
        values.push(value);
        let added = self.get(handle).estimated_bytes() - previous;
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(added)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
        self.verificar_teto(0);
        true
    }
    /// Valida referência antes de modificar o grafo; null não exige objeto vivo.
    fn validate_tag(&self, value: TaggedValue) {
        assert_eq!(
            value.is_ref,
            value.tag == ValueTag::Ref,
            "tag inconsistente com is_ref"
        );
        if value.is_ref && smi::e_handle(value.bits) {
            self.get(value.bits);
        }
    }
    /// Obtém armazenamento mutável; não oferece acesso a slots já coletados.
    pub fn get_mut(&mut self, handle: i64) -> &mut Value {
        let slot = self.indice_vivo(handle);
        self.slots[slot].as_mut().expect("slot vivo verificado")
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    /// Marca tudo o que é alcançável a partir de `pending`; devolve quantos
    /// objetos marcou.
    fn marcar_pendentes(&mut self) -> usize {
        let mut live = 0_usize;
        while let Some(handle) = self.pending.pop() {
            // null e `Smi` (R10) não são arestas: o coletor nunca segue um
            // `Smi`, que não aponta para o heap.
            if !smi::e_handle(handle) {
                continue;
            }
            // Raiz ou aresta que não é um handle vivo: mesmas quatro
            // mensagens de `get`, porque a causa é a mesma (N4).
            let index = self.indice_vivo(handle);
            if self.marks[index] {
                continue;
            }
            self.marks[index] = true;
            live += 1;
            self.slots[index]
                .as_ref()
                .expect("slot vivo verificado")
                .trace(&mut self.pending);
        }
        live
    }

    pub fn collect(&mut self) {
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self.literais.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots)| roots.len() as u64)
            .sum::<u64>();
        self.stats.slots_scanned += self.slots.len() as u64;
        self.marks.resize(self.slots.len(), false);
        self.marks.fill(false);
        self.pending.clear();
        self.pending.extend(self.enum_values.values().copied());
        self.pending.extend(self.tearoffs.values().copied());
        self.pending.extend(self.literais.values().copied());
        self.pending.extend(self.globais.values().copied());
        self.pending.extend(self.caixas_bool.iter().copied().filter(|&h| h != 0));
        self.pending.extend(self.raizes_do_runtime.iter().copied().filter(|&h| h != 0));
        self.pending.extend(self.finalizacoes_prontas.iter().copied());
        for a in &self.anexos {
            self.pending.push(a.dono);
            if let AcaoDeFinalizador::Dart(acao) = a.acao {
                self.pending.push(acao);
            }
        }
        self.pending.extend(
            self.frames
                .iter()
                .flat_map(|(_, roots)| roots.iter().copied()),
        );
        let (pending, stats) = (&mut self.pending, &mut self.stats);
        visitar_quadros(|h| {
            stats.roots_scanned += 1;
            pending.push(h);
        });
        let mut live = self.marcar_pendentes();
        // Os efêmeros: o valor de um portador vivo é alcançado quando a
        // chave é — o que pode tornar vivas outras chaves, até o ponto fixo.
        if !self.efemeros.is_empty() {
            loop {
                let marks = &self.marks;
                let marcado = |h: i64| smi::e_handle(h) && h > 0 && marks.get(Self::indice_de(h)).copied().unwrap_or(false);
                let antes = self.pending.len();
                for (&portador, &(chave, valor)) in &self.efemeros {
                    if marcado(portador) && (marcado(chave) || !smi::e_handle(chave)) && smi::e_handle(valor) && !marcado(valor) {
                        self.pending.push(valor);
                    }
                }
                if self.pending.len() == antes {
                    break;
                }
                live += self.marcar_pendentes();
            }
        }
        // Tabelas laterais: só ficam os handles que sobreviveram (G6).
        let marks = &self.marks;
        let vivo = |h: &i64| {
            smi::e_handle(*h) && *h > 0 && marks.get(Self::indice_de(*h)).copied().unwrap_or(false)
        };
        self.fracas.retain(|portador, alvo| {
            if smi::e_handle(*alvo) && !vivo(alvo) {
                *alvo = 0;
            }
            vivo(portador)
        });
        self.efemeros.retain(|portador, (chave, valor)| {
            if smi::e_handle(*chave) && !vivo(chave) {
                *chave = 0;
                *valor = 0;
            }
            vivo(portador)
        });
        self.imutaveis.retain(|h| vivo(h));
        self.campos_late_inicializados.retain(|(h, _)| vivo(h));
        self.fixas.retain(|h| vivo(h));
        self.pendentes.retain(|h, _| vivo(h));
        self.iteracoes_ativas.retain(|h| vivo(h));
        self.origens.retain(|k, v| vivo(k) && vivo(v));
        let mut prontas = Vec::new();
        let mut nativas = Vec::new();
        self.anexos.retain_mut(|a| {
            if smi::e_handle(a.desanexo) && !vivo(&a.desanexo) {
                a.desanexo = 0;
            }
            if vivo(&a.valor) {
                return true;
            }
            match a.acao {
                AcaoDeFinalizador::Dart(acao) => prontas.push(acao),
                AcaoDeFinalizador::Nativa(f, token) => nativas.push((f, token)),
            }
            false
        });
        self.finalizacoes_prontas.extend(prontas);
        let mut finalizar = Vec::new();
        self.finalizaveis.retain(|h, &mut par| {
            let fica = vivo(h);
            if !fica {
                finalizar.push(par);
            }
            fica
        });
        // A estimativa é refeita dos vivos a cada coleta: os natives que
        // trocam o armazenamento de um valor no lugar (`_setLength`,
        // `_setData`, as listas tipadas…) não passam pela contabilidade, e
        // subtrair a capacidade de agora do que foi somado na alocação
        // estouraria o contador.
        let mut vivos_em_bytes = 0usize;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let Some(valor) = slot.as_ref() else { continue };
            if self.marks[index] {
                vivos_em_bytes = vivos_em_bytes.saturating_add(valor.estimated_bytes());
            } else {
                *slot = None;
                self.free.push(index);
                self.stats.reclaimed += 1;
            }
        }
        self.stats.estimated_bytes = vivos_em_bytes;
        for (finalizador, par) in finalizar {
            finalizador(par);
        }
        for (f, token) in nativas {
            #[allow(unsafe_code)]
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
        self.allocations = 0;
        self.recalcular_gatilhos(live);
    }

    /// Os gatilhos da próxima coleta, a partir do que sobreviveu a esta.
    ///
    /// Sem teto, o heap pode dobrar (o geométrico de sempre). Com teto, a
    /// próxima coleta vem quando se gastar metade da folga que resta — a
    /// histerese: uma coleta que achou quase tudo vivo não se repete na
    /// alocação seguinte (antes, passar da metade do teto coletava em TODA
    /// alocação, e um programa com muito dado vivo parava de andar). O passo
    /// mínimo garante progresso mesmo com a folga no fim; o teto em si é
    /// conferido à parte (`allocate`).
    fn recalcular_gatilhos(&mut self, vivos: usize) {
        const PASSO_MINIMO: usize = 256 * 1024;
        let crescimento = self.stats.estimated_bytes.saturating_mul(2).max(1024 * 1024);
        self.byte_threshold = if self.limite_bytes == usize::MAX {
            crescimento
        } else {
            let folga = self.limite_bytes.saturating_sub(self.bytes_totais(0));
            crescimento.min(self.stats.estimated_bytes.saturating_add((folga / 2).max(PASSO_MINIMO)))
        };
        // O gatilho por contagem acompanha: com teto, no máximo tantos
        // objetos quanto a folga comporta pelo tamanho mínimo de um slot.
        let por_contagem = vivos.saturating_mul(2).max(256);
        self.threshold = if self.limite_bytes == usize::MAX {
            por_contagem
        } else {
            let folga = self.limite_bytes.saturating_sub(self.bytes_totais(0));
            let slot = std::mem::size_of::<Option<Value>>().max(1);
            por_contagem.min(vivos.saturating_add((folga / 2 / slot).max(PASSO_MINIMO / slot)))
        };
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.slots.len() - self.free.len(),
            reserved_slots: self.slots.len(),
            permanent_roots: self.enum_values.len()
                + self.tearoffs.len()
                + self.literais.len()
                + self.caixas_bool.iter().filter(|&&h| h != 0).count(),
            ..self.stats
        }
    }

    /// Compara conteúdo por unidades UTF-16 (`String.==`): NUL, surrogates
    /// soltos e sequências Unicode diferentes (`á` × `a` + acento) contam.
    pub fn string_equal(&self, a: i64, b: i64) -> bool {
        if a == 0 || b == 0 {
            return a == b;
        }
        match (self.try_get(a), self.try_get(b)) {
            (Some(Value::String(left)), Some(Value::String(right))) => left == right,
            _ => false,
        }
    }

    /// O texto de uma string viva; outro valor é bug do compilador.
    pub fn texto(&self, handle: i64) -> &Texto {
        match self.get(handle) {
            Value::String(t) => t,
            _ => panic!("bug do compilador: string esperada"),
        }
    }

    /// Concatena conteúdo antes da possível coleta; os operandos seguem o protocolo de raízes.
    pub fn string_concat(&mut self, a: i64, b: i64) -> i64 {
        let junto = self.texto(a).concatenar(self.texto(b));
        self.allocate(Value::String(junto))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Enums preservam identidade e nomes sem frames externos, mesmo em stress.
    #[test]
    fn enum_singletons_survive_collection_and_keep_nominal_identity() {
        let mut heap = Heap::new(true);
        let first = heap.enum_value(1, 0, "red");
        let second = heap.enum_value(1, 1, "blue");
        let other_class = heap.enum_value(2, 0, "red");
        heap.collect();
        assert_eq!(heap.enum_value(1, 0, "red"), first);
        assert_ne!(first, second);
        assert_ne!(first, other_class);
        let Value::Object { class_id, fields } = heap.get(first) else {
            panic!("enum deve ser objeto")
        };
        assert_eq!(*class_id, 1);
        assert_eq!(fields[0], (0, false));
        assert!(matches!(heap.get(fields[1].0), Value::String(name) if name == "red"));
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Um único objeto raiz mantém transitivamente ciclos e strings de seus campos.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(outer, parent);
        let inner = heap.push_frame();
        let text = heap.allocate(Value::String("ação 🦀".into()));
        heap.root(inner, text);
        heap.set(parent, 0, text, true);
        heap.pop_frame(inner);
        heap.collect();
        assert!(matches!(heap.get(text), Value::String(s) if s == "ação 🦀"));
        heap.set(parent, 0, 0, true);
        heap.collect();
        assert!(heap.slots[Heap::indice_de(text)].is_none());
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 0);
    }
    /// Ciclos sobrevivem por raiz, depois são coletados e seus slots reutilizados.
    #[test]
    fn cycles_and_slot_reuse() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(frame, a);
        let b = heap.allocate(Value::Object {
            class_id: 2,
            fields: vec![(a, true)],
        });
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 0);
        let c = heap.allocate(Value::String("novo".into()));
        assert!(c == a || c == b);
        assert_eq!(heap.slots.len(), 2);
    }
    /// Bits escalares coincidentes com handles não retêm objetos.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.allocate(Value::String("descartável".into()));
        let object = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(text, false)],
        });
        heap.root(frame, object);
        heap.collect();
        assert!(heap.slots[Heap::indice_de(text)].is_none());
        assert!(matches!(heap.get(object), Value::Object { .. }));
    }
    /// Frames aninhados preservam argumentos; stress coleta durante milhares de alocações.
    #[test]
    fn nested_frames_stress() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let permanent = heap.allocate(Value::String("vivo".into()));
        heap.root(outer, permanent);
        for _ in 0..1000 {
            let inner = heap.push_frame();
            let temporary = heap.allocate(Value::String("temporário".into()));
            heap.root(inner, temporary);
            heap.collect();
            assert!(matches!(heap.get(permanent), Value::String(s) if s == "vivo"));
            heap.pop_frame(inner);
        }
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 1);
        assert!(heap.slots.len() <= 2);
    }
}

#[cfg(test)]
mod falhas_de_handle {
    //! N4: cada contrato quebrado tem mensagem própria.
    use super::*;

    #[test]
    #[should_panic(expected = "handle null (0) desreferenciado")]
    fn null_desreferenciado() {
        Heap::new(false).get(0);
    }

    #[test]
    #[should_panic(expected = "handle negativo")]
    fn handle_negativo() {
        Heap::new(false).get(-2);
    }

    #[test]
    #[should_panic(expected = "handle além da tabela")]
    fn handle_alem_da_tabela() {
        let mut heap = Heap::new(false);
        heap.allocate(Value::String("x".into()));
        heap.get(42);
    }

    #[test]
    #[should_panic(expected = "handle já coletado")]
    fn handle_coletado() {
        let mut heap = Heap::new(false);
        let h = heap.allocate(Value::String("x".into()));
        heap.collect();
        heap.get(h);
    }
}

#[cfg(test)]
mod texto_utf16 {
    //! Decisão 5: strings com a semântica de unidades UTF-16 do Dart, na
    //! forma da VM. Os casos são os do programa 04 do corpus
    //! (`corpus/js/04_strings_surrogates.dart`), com os valores da VM.
    use super::*;

    #[test]
    fn literais_iguais_sao_identicos_apos_coleta_mas_texto_dinamico_nao() {
        let mut heap = Heap::new(true);
        let primeiro = heap.string_literal(Texto::de_wtf8("ação".as_bytes()));
        heap.collect();
        let segundo = heap.string_literal(Texto::de_wtf8("ação".as_bytes()));
        assert_eq!(primeiro, segundo);
        let dinamico = heap.allocate(Value::String(Texto::de_str("ação")));
        assert_ne!(primeiro, dinamico);
        assert!(heap.string_equal(primeiro, dinamico));
    }

    #[test]
    fn forma_canonica_um_e_dois_bytes() {
        assert!(matches!(Texto::de_str("abc"), Texto::Um(_)));
        assert!(matches!(Texto::de_str("ação"), Texto::Um(_)), "Latin-1 é um byte");
        assert!(matches!(Texto::de_str("ÿ"), Texto::Um(_)));
        assert!(matches!(Texto::de_str("Ÿ"), Texto::Dois(_)));
        assert!(matches!(Texto::de_str("😀"), Texto::Dois(_)));
        // Um pedaço Latin-1 de um texto de dois bytes volta a ser um byte.
        assert!(matches!(Texto::de_str("a😀b").fatia(0, 1), Texto::Um(_)));
        // A igualdade é por unidades, qualquer que seja a forma.
        assert_eq!(Texto::Dois(vec![0x61]), Texto::de_str("a"));
    }

    #[test]
    fn comprimento_e_unidades_de_um_emoji() {
        let emoji = Texto::de_str("😀");
        assert_eq!(emoji.len(), 2);
        assert_eq!(emoji.para_vec(), vec![0xD83D, 0xDE00]);
        assert_eq!(emoji.pontos(), vec![0x1F600]);
        let misto = Texto::de_str("a😀b");
        assert_eq!(misto.len(), 4);
        assert_eq!(misto.pontos(), vec![97, 0x1F600, 98]);
        assert_eq!(misto.fatia(1, 3), emoji);
        assert_eq!(misto.fatia(3, 4), "b");
        assert_eq!(misto.fatia(1, 2).len(), 1);
        assert_eq!(misto.fatia(1, 2).unidade(0), 0xD83D);
        assert_eq!(Texto::de_str("\u{1D11E}").len(), 2);
        assert_eq!(Texto::de_str("👍🏽").pontos().len(), 2);
        assert_eq!(Texto::de_str("👍🏽").len(), 4);
        assert_eq!(Texto::de_str("e\u{301}").len(), 2);
    }

    #[test]
    fn surrogate_solto_e_preservado() {
        let solto = Texto::de_unidades(vec![0xD83D]);
        assert_eq!(solto.len(), 1);
        assert_eq!(solto.unidade(0), 0xD83D);
        // `runes` devolve o próprio surrogate solto, não U+FFFD.
        assert_eq!(solto.pontos(), vec![0xD83D]);
        // Juntar as duas metades forma o emoji.
        let par = solto.concatenar(&Texto::de_unidades(vec![0xDE00]));
        assert_eq!(par, Texto::de_str("😀"));
        // WTF-8 guarda o surrogate solto sem perda (três bytes)...
        assert_eq!(solto.para_wtf8(), vec![0xED, 0xA0, 0xBD]);
        assert_eq!(Texto::de_wtf8(&[0xED, 0xA0, 0xBD]), solto);
        assert_eq!(par.para_wtf8(), "😀".as_bytes());
        // ...mas `print` escreve U+FFFD no lugar dele, como a VM (medido:
        // `print(String.fromCharCode(0xD83D))` sai `EF BF BD`).
        assert_eq!(solto.para_utf8_da_vm(), "\u{FFFD}".as_bytes());
        assert_eq!(par.para_utf8_da_vm(), "😀".as_bytes());
    }

    #[test]
    fn busca_por_unidades() {
        let tres = Texto::de_str("😀😀😀");
        let emoji = Texto::de_str("😀");
        assert_eq!(tres.procurar(&emoji, 1), Some(2));
        assert_eq!(tres.procurar_ultimo(&emoji, tres.len()), Some(4));
        assert_eq!(Texto::de_str("a😀b").procurar(&emoji, 0), Some(1));
        assert_eq!(tres.procurar(&Texto::vazio(), 3), Some(3));
        assert!(Texto::de_str("abc").comparar(&Texto::de_str("abd")).is_lt());
        // Comparação por unidades: U+FFFF > U+1F600 (0xD83D...).
        assert!(Texto::de_str("\u{FFFF}").comparar(&emoji).is_gt());
    }

    /// Os valores são os da VM 3.6.2 (`print(s.hashCode)`, medidos).
    #[test]
    fn hash_code_e_o_da_vm() {
        let casos: [(&[u16], i64); 8] = [
            (&[], 1),
            (&[97], 170824770),
            (&[97, 98, 99], 756227931),
            (&[97, 231, 227, 111], 927877670),
            (&[0xD83D, 0xDE00], 472421242),
            (&[97, 0xD83D, 0xDE00, 98], 661772406),
            (&[0xD83D], 471726753),
            (&[72, 101, 108, 108, 111, 44, 32, 87, 111, 114, 108, 100, 33], 847757641),
        ];
        for (unidades, esperado) in casos {
            assert_eq!(Texto::de_fatia(unidades).hash_vm(), esperado, "{unidades:?}");
        }
    }

    #[test]
    fn construtor_por_unidades() {
        let mut m = TextoMut::new();
        m.push_str("x");
        m.push_texto(&Texto::de_unidades(vec![0xDE00]));
        m.push('y');
        let t = m.fim();
        assert_eq!(t.len(), 3);
        assert_eq!(t.unidade(1), 0xDE00);
    }
}

#[cfg(test)]
mod smi_r10 {
    //! R10: `int` pequeno etiquetado dentro do `Ref`, sem alocação, que o
    //! coletor nunca segue.
    use super::*;

    #[test]
    fn faixa_e_codificacao() {
        assert_eq!(smi::de(0), Some(1));
        assert_eq!(smi::de(-1), Some(-1));
        assert_eq!(smi::valor(smi::de(-1).unwrap()), -1);
        for v in [smi::MIN, smi::MAX, 7, -7, 1 << 40] {
            let r = smi::de(v).unwrap();
            assert!(smi::e_smi(r) && !smi::e_handle(r));
            assert_eq!(smi::valor(r), v);
        }
        assert_eq!(smi::de(smi::MAX + 1), None);
        assert_eq!(smi::de(smi::MIN - 1), None);
        assert_eq!(smi::de(i64::MAX), None);
        assert!(!smi::e_handle(0));
    }

    #[test]
    fn caixa_de_int_pequeno_nao_aloca() {
        let mut heap = Heap::new(true);
        let antes = heap.stats().allocations;
        let r = heap.caixa_int(42);
        assert_eq!(heap.stats().allocations, antes, "Smi não aloca");
        assert_eq!(heap.int_de_ref(r), Some(42));
        // Dois `int` iguais em posição `Ref` são o mesmo `Ref` (identical).
        assert_eq!(heap.caixa_int(42), r);
        // Fora da faixa: `_Mint` no heap, com handle par.
        let grande = heap.caixa_int(i64::MAX);
        assert!(smi::e_handle(grande));
        assert_eq!(heap.stats().allocations, antes + 1);
        assert_eq!(heap.int_de_ref(grande), Some(i64::MAX));
        assert!(matches!(heap.get(grande), Value::BoxedInt(i64::MAX)));
    }

    #[test]
    fn handles_sao_pares() {
        let mut heap = Heap::new(false);
        for _ in 0..10 {
            let h = heap.allocate(Value::String("x".into()));
            assert!(smi::e_handle(h), "handle {h} deveria ser par");
        }
    }

    #[test]
    fn coletor_nunca_segue_um_smi() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let s = smi::de(3).unwrap();
        // Um `Smi` como raiz, como campo `Ref` e como elemento: nada disso é
        // aresta; a coleta sob estresse não o toma por handle.
        heap.set_root(frame, 0, s);
        let obj = heap.allocate(Value::Object { class_id: 1, fields: vec![(s, true)] });
        heap.set_root(frame, 1, obj);
        heap.set(obj, 0, smi::de(-5).unwrap(), true);
        let lista = heap.create_list(vec![TaggedValue::reference(s)]);
        // A lista normaliza o `Smi` para o escalar (R8).
        assert_eq!(heap.list_get(lista, 0), TaggedValue::scalar(3));
        heap.collect();
        assert!(matches!(heap.get(obj), Value::Object { .. }));
        heap.set_global_root(1, s);
        heap.set_raiz_do_runtime(0, s);
        heap.collect();
        heap.pop_frame(frame);
        heap.set_raiz_do_runtime(0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn normalizar_e_como_ref_sao_inversos() {
        let mut heap = Heap::new(false);
        for v in [0, 1, -1, smi::MAX, smi::MIN, i64::MAX, i64::MIN] {
            let r = heap.como_ref(TaggedValue::scalar(v));
            assert_eq!(heap.normalizar(TaggedValue::reference(r)), TaggedValue::scalar(v));
        }
    }

    #[test]
    #[should_panic(expected = "Smi usado como handle")]
    fn smi_desreferenciado_e_bug_com_mensagem_propria() {
        Heap::new(false).get(smi::de(9).unwrap());
    }
}

#[cfg(test)]
mod caixas {
    //! R3/R8/R9: caixas de escalares e a normalização nas coleções.
    use super::*;

    #[test]
    fn caixa_de_int_e_a_mesma_chave_que_o_int() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let caixa = heap.allocate(Value::BoxedInt(7));
        heap.set_root(frame, 0, caixa);
        let mapa = heap.create_map(vec![(TaggedValue::reference(caixa), TaggedValue::scalar(1))]);
        heap.set_root(frame, 1, mapa);
        // A chave entrou como escalar: a caixa não é aresta do mapa.
        assert!(heap.map_contains(mapa, TaggedValue::scalar(7)));
        assert!(heap.map_contains(mapa, TaggedValue::reference(caixa)));
        heap.set_root(frame, 0, 0);
        heap.collect();
        assert!(heap.map_contains(mapa, TaggedValue::scalar(7)));
    }

    #[test]
    fn lista_nao_guarda_caixa_e_devolve_caixa_nova() {
        let mut heap = Heap::new(false);
        let caixa = heap.allocate(Value::BoxedDouble(2.5));
        let lista = heap.create_list(vec![TaggedValue::reference(caixa)]);
        assert_eq!(heap.list_get(lista, 0), TaggedValue::double(2.5));
        let de_volta = heap.como_ref(heap.list_get(lista, 0));
        assert!(matches!(heap.get(de_volta), Value::BoxedDouble(d) if *d == 2.5));
    }

    #[test]
    fn caixas_de_bool_sao_dois_singletons_permanentes() {
        let mut heap = Heap::new(true);
        let t1 = heap.caixa_bool(true);
        let t2 = heap.caixa_bool(true);
        let f = heap.caixa_bool(false);
        assert_eq!(t1, t2);
        assert_ne!(t1, f);
        heap.collect();
        assert!(matches!(heap.get(t1), Value::BoxedBool(true)));
        assert_eq!(heap.stats().permanent_roots, 2);
    }
}

#[cfg(test)]
mod raizes_do_runtime {
    //! G6: raízes que não moram num frame, e as tabelas laterais.
    use super::*;

    #[test]
    fn excecao_pendente_sobrevive_a_coleta_sem_frame() {
        let mut heap = Heap::new(true);
        let erro = heap.allocate(Value::String("falhou".into()));
        heap.set_raiz_do_runtime(0, erro);
        heap.allocate(Value::String("outra".into()));
        heap.collect();
        assert!(matches!(heap.get(erro), Value::String(s) if s == "falhou"));
        heap.set_raiz_do_runtime(0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn coleta_sem_frame_aberto() {
        // O portão antigo só coletava com frame; agora o limiar basta.
        let mut heap = Heap::new(false);
        for _ in 0..1000 {
            heap.allocate(Value::String("lixo".into()));
        }
        assert!(heap.stats().collections > 0);
        assert!(heap.stats().live_objects < 1000);
    }

    #[test]
    fn tabela_lateral_purgada_quando_o_slot_e_reutilizado() {
        let mut heap = Heap::new(false);
        let lista = heap.create_list(Vec::new());
        heap.imutaveis.insert(lista);
        heap.iteracoes_ativas.insert(lista);
        heap.campos_late_inicializados.insert((lista, 0));
        heap.campos_late_inicializados.insert((lista, -1));
        heap.campos_late_inicializados.insert((lista, -2));
        heap.collect();
        assert!(heap.imutaveis.is_empty());
        assert!(heap.iteracoes_ativas.is_empty());
        assert!(heap.campos_late_inicializados.is_empty());
        let nova = heap.create_list(Vec::new());
        assert_eq!(nova, lista, "o slot é reutilizado");
        assert!(!heap.imutaveis.contains(&nova));
        assert!(!heap.campos_late_inicializados.contains(&(nova, 0)));
        assert!(!heap.campos_late_inicializados.contains(&(nova, -1)));
        assert!(!heap.campos_late_inicializados.contains(&(nova, -2)));
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    /// Igualdade preserva Unicode, NUL embutido e nulidade sem normalização implícita.
    #[test]
    fn unicode_nul_nullable_and_concat() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate(Value::String("á\0🦀".into()));
        heap.root(frame, a);
        let b = heap.allocate(Value::String("á\0🦀".into()));
        heap.root(frame, b);
        assert_ne!(a, b);
        assert!(heap.string_equal(a, b));
        assert!(heap.string_equal(0, 0));
        assert!(!heap.string_equal(0, a));
        assert!(!heap.string_equal(a, 0));
        let c = heap.allocate(Value::String("a\u{301}\0🦀".into()));
        heap.root(frame, c);
        assert!(!heap.string_equal(a, c));
        let joined = heap.string_concat(a, b);
        heap.root(frame, joined);
        assert!(matches!(heap.get(joined), Value::String(s) if s == "á\0🦀á\0🦀"));
    }
    /// Retorno transfere raiz sem coleta entre pop e registro no chamador.
    #[test]
    fn returned_handles_and_identity() {
        let mut heap = Heap::new(true);
        let caller = heap.push_frame();
        let callee = heap.push_frame();
        let a = heap.allocate(Value::Object {
            class_id: 7,
            fields: vec![],
        });
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.allocate(Value::Object {
            class_id: 7,
            fields: vec![],
        });
        heap.root(caller, b);
        assert_ne!(a, b);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }
    /// Tracing e destruição de um ciclo grande não dependem da pilha de chamadas.
    #[test]
    fn large_cycle_is_iterative() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let first = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.allocate(Value::Object {
                class_id: 1,
                fields: vec![(first, true)],
            });
            heap.set(previous, 0, next, true);
            previous = next;
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 20_000);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        assert_eq!(heap.stats().reclaimed, 20_000);
    }
    /// Microbenchmark reproduzível: tempos incluem raízes e coletas automáticas declaradas.
    #[test]
    #[ignore = "microbenchmark; execute em release com --ignored --nocapture"]
    fn gc_microbenchmark() {
        for (count, stress) in [(100_000, false), (2_000, true)] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame();
            let started = std::time::Instant::now();
            for index in 0..count {
                let handle = heap.allocate(Value::Object {
                    class_id: 1,
                    fields: vec![(index, false)],
                });
                heap.root(frame, handle);
                std::hint::black_box(handle);
            }
            let allocation_ns = started.elapsed().as_nanos();
            let started = std::time::Instant::now();
            heap.collect();
            let collect_live_ns = started.elapsed().as_nanos();
            heap.pop_frame(frame);
            let started = std::time::Instant::now();
            heap.collect();
            let collect_dead_ns = started.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "count={count} stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_live_ns={collect_live_ns} collect_dead_ns={collect_dead_ns} stats={stats:?}"
            );
            assert_eq!(stats.live_objects, 0);
            assert_eq!(stats.reclaimed, count as u64);
        }
    }
}

#[cfg(test)]
mod fixed_root_tests {
    use super::*;
    /// Slots substituídos não crescem com iterações; cópias locais têm raízes independentes.
    #[test]
    fn fixed_slots_preserve_copies_and_nested_returns() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let kept = heap.allocate(Value::String("keep".into()));
        heap.set_root(frame, 0, kept);
        heap.set_root(frame, 1, kept);
        for _ in 0..1000 {
            let inner = heap.push_frame_with_slots(1);
            let temporary = heap.allocate(Value::String("next".into()));
            heap.set_root(inner, 0, temporary);
            heap.pop_frame(inner);
            heap.set_root(frame, 0, temporary);
            heap.collect();
            assert!(matches!(heap.get(kept), Value::String(s) if s == "keep"));
        }
        assert_eq!(heap.stats().peak_root_slots, 3);
        assert_eq!(heap.stats().live_roots, 2);
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        assert_eq!(heap.stats().live_roots, 0);
        assert_eq!(heap.stats().estimated_bytes, 0);
        assert_eq!(heap.stats().live_objects, 0);
        assert!(heap.stats().reserved_slots <= 3);
        heap.pop_frame(frame);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Payload grande dispara coleta antes do limiar por quantidade de objetos.
    #[test]
    fn byte_trigger_collects_large_payloads() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..8 {
            let handle = heap.allocate(Value::String(Texto::Um(Vec::with_capacity(2 * 1024 * 1024))));
            heap.set_root(frame, 0, handle);
        }
        let stats = heap.stats();
        assert!(stats.collections >= 3);
        assert!(stats.reclaimed >= 5);
        assert!(stats.peak_estimated_bytes < 7 * 1024 * 1024);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
    /// Mede alocações transientes com uma única raiz sobrescrita, sem alegar superioridade.
    #[test]
    #[ignore = "microbenchmark de slots fixos; execute release --ignored --nocapture"]
    fn fixed_slot_microbenchmark() {
        for stress in [false, true] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame_with_slots(1);
            let start = std::time::Instant::now();
            for _ in 0..100_000 {
                let handle = heap.allocate(Value::String("temporary string".into()));
                heap.set_root(frame, 0, handle);
            }
            let allocation_ns = start.elapsed().as_nanos();
            let start = std::time::Instant::now();
            heap.collect();
            let collection_ns = start.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "fixed count=100000 stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_ns={collection_ns} stats={stats:?}"
            );
            assert_eq!(stats.peak_root_slots, 1);
            assert_eq!(stats.live_objects, 1);
            assert_eq!(stats.reclaimed, 99_999);
            assert!(stats.reserved_slots <= 257);
        }
    }
}
#[cfg(test)]
mod maps_sets_tearoffs {
    use super::*;

    /// Chaves int e bool com os mesmos bits são entradas distintas, como no SDK.
    #[test]
    fn int_and_bool_keys_are_distinct_and_strings_compare_by_content() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let first = heap.allocate(Value::String("chave".into()));
        heap.set_root(frame, 0, first);
        let second = heap.allocate(Value::String("chave".into()));
        heap.set_root(frame, 1, second);
        assert_ne!(first, second);
        let map = heap.create_map(vec![
            (TaggedValue::scalar(0), TaggedValue::scalar(1)),
            (TaggedValue::boolean(false), TaggedValue::scalar(2)),
            (TaggedValue::reference(first), TaggedValue::scalar(3)),
        ]);
        heap.set_root(frame, 2, map);
        assert_eq!(heap.map_len(map), 3);
        assert!(heap.map_contains(map, TaggedValue::scalar(0)));
        assert!(heap.map_contains(map, TaggedValue::boolean(false)));
        // Conteúdo igual, handle diferente: mesma chave.
        assert!(heap.map_contains(map, TaggedValue::reference(second)));
        assert_eq!(
            heap.map_get(map, TaggedValue::reference(second)),
            TaggedValue::scalar(3)
        );
        assert!(!heap.map_contains(map, TaggedValue::scalar(7)));
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        // A chave original sobrevive pelo mapa; a duplicata é coletada.
        assert_eq!(heap.stats().live_objects, 2);
        assert_eq!(
            heap.map_get(map, TaggedValue::reference(first)),
            TaggedValue::scalar(3)
        );
        // Substituição preserva a posição; inserção acrescenta no fim.
        heap.map_set(map, TaggedValue::scalar(0), TaggedValue::scalar(10));
        assert_eq!(heap.map_len(map), 3);
        assert_eq!(
            heap.map_get(map, TaggedValue::scalar(0)),
            TaggedValue::scalar(10)
        );
    }

    /// Literais com chaves duplicadas conservam o último valor, como Dart.
    #[test]
    fn duplicate_literal_keys_keep_the_last_value() {
        let mut heap = Heap::new(false);
        let map = heap.create_map(vec![
            (TaggedValue::scalar(1), TaggedValue::scalar(100)),
            (TaggedValue::scalar(1), TaggedValue::scalar(200)),
        ]);
        assert_eq!(heap.map_len(map), 1);
        assert_eq!(
            heap.map_get(map, TaggedValue::scalar(1)),
            TaggedValue::scalar(200)
        );
    }

    /// Conjuntos removem duplicadas por conteúdo e `add` informa a inserção.
    #[test]
    fn sets_deduplicate_and_report_insertion() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let text = heap.allocate(Value::String("dup".into()));
        heap.set_root(frame, 0, text);
        let set = heap.create_set(vec![
            TaggedValue::scalar(1),
            TaggedValue::scalar(1),
            TaggedValue::boolean(true),
            TaggedValue::reference(text),
            TaggedValue::reference(text),
        ]);
        heap.set_root(frame, 1, set);
        // `true` (bits 1) difere de `1` int pela tag.
        assert_eq!(heap.set_len(set), 3);
        assert!(heap.set_contains(set, TaggedValue::boolean(true)));
        assert!(!heap.set_add(set, TaggedValue::scalar(1)));
        assert!(heap.set_add(set, TaggedValue::scalar(9)));
        assert_eq!(heap.set_len(set), 4);
        heap.set_root(frame, 0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }

    /// Tear-offs da mesma função são canônicos e sobrevivem sem frames externos.
    #[test]
    fn top_level_tearoffs_are_canonical_permanent_roots() {
        let mut heap = Heap::new(true);
        let first = heap.tearoff(4);
        let second = heap.tearoff(4);
        let other = heap.tearoff(9);
        assert_eq!(first, second);
        assert_ne!(first, other);
        heap.collect();
        assert_eq!(heap.tearoff(4), first);
        let (code, _) = heap.closure_parts(first);
        assert_eq!(code, 4);
        // Cada tear-off tem closure e ambiente próprios, ambos permanentes.
        assert_eq!(heap.stats().live_objects, 4);
        assert_eq!(heap.stats().permanent_roots, 2);
    }
}
#[cfg(test)]
mod captures_and_lists {
    use super::*;

    /// A closure escapada conserva ambiente e celula depois de fechar o frame criador.
    #[test]
    fn escaping_closure_preserves_mutable_capture() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame_with_slots(1);
        let creator = heap.push_frame_with_slots(2);
        let cell = heap.create_cell(TaggedValue::scalar(10));
        heap.set_root(creator, 0, cell);
        let env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        heap.set_root(creator, 1, env);
        let closure = heap.create_closure(7, env);
        heap.set_root(outer, 0, closure);
        heap.pop_frame(creator);
        heap.allocate(Value::String("coleta forcada".into()));
        heap.collect();
        let (code, escaped) = heap.closure_parts(closure);
        assert_eq!(code, 7);
        let captured = heap.environment_get(escaped, 0).bits;
        assert_eq!(heap.cell_get(captured), TaggedValue::scalar(10));
        heap.cell_set(captured, TaggedValue::scalar(11));
        assert_eq!(heap.cell_get(cell), TaggedValue::scalar(11));
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    /// Ambientes diferentes compartilham celulas, mas closures conservam identidade propria.
    #[test]
    fn aliases_share_cells_and_closure_identity_is_not_code_identity() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let cell = heap.create_cell(TaggedValue::scalar(1));
        heap.set_root(frame, 0, cell);
        let first_env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        let first = heap.create_closure(42, first_env);
        heap.set_root(frame, 1, first);
        let second_env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        let second = heap.create_closure(42, second_env);
        heap.set_root(frame, 2, second);
        assert_ne!(first, second);
        let (_, first_env) = heap.closure_parts(first);
        let (_, second_env) = heap.closure_parts(second);
        let alias = heap.environment_get(second_env, 0).bits;
        heap.cell_set(alias, TaggedValue::scalar(8));
        assert_eq!(
            heap.cell_get(heap.environment_get(first_env, 0).bits).bits,
            8
        );
        heap.collect();
        assert_eq!(heap.stats().live_objects, 5);
    }

    /// Tracing iterativo recupera o ciclo closure -> ambiente -> celula -> closure.
    #[test]
    fn closure_capture_cycle_is_collected() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let cell = heap.create_cell(TaggedValue::reference(0));
        heap.set_root(frame, 0, cell);
        let env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        heap.set_root(frame, 1, env);
        let closure = heap.create_closure(0, env);
        heap.cell_set(cell, TaggedValue::reference(closure));
        heap.collect();
        assert_eq!(heap.stats().live_objects, 3);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        assert_eq!(heap.stats().reclaimed, 3);
    }

    /// Lista distingue handles reais de inteiros coincidentes e libera referencias removidas.
    #[test]
    fn lists_trace_references_not_scalar_bits() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let kept = heap.allocate(Value::String("mantido".into()));
        heap.set_root(frame, 0, kept);
        let scalar_bits = heap.allocate(Value::String("descartado".into()));
        heap.set_root(frame, 1, scalar_bits);
        let list = heap.create_list(vec![
            TaggedValue::reference(kept),
            TaggedValue::scalar(scalar_bits),
        ]);
        heap.set_root(frame, 2, list);
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        assert_eq!(heap.list_get(list, 0), TaggedValue::reference(kept));
        heap.list_set(list, 0, TaggedValue::reference(0));
        heap.collect();
        assert_eq!(heap.stats().live_objects, 1);
        heap.list_push(list, TaggedValue::reference(list));
        assert_eq!(heap.list_len(list), 3);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    /// Crescimento de capacidade entra nos contadores e nos limites de coleta.
    #[test]
    fn growable_list_accounts_capacity_and_keeps_new_reference() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(1);
        let list = heap.create_list(vec![]);
        heap.set_root(frame, 0, list);
        let before = heap.stats().estimated_bytes;
        for n in 0..128 {
            heap.list_push(list, TaggedValue::scalar(n));
        }
        let text = heap.allocate(Value::String("novo".into()));
        heap.list_push(list, TaggedValue::reference(text));
        assert!(matches!(heap.get(text),Value::String(s) if s=="novo"));
        assert_eq!(heap.list_len(list), 129);
        assert!(heap.stats().estimated_bytes >= before + 129 * std::mem::size_of::<TaggedValue>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
}
