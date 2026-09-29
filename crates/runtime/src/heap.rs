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

/// A forma dos elementos de uma lista do runtime (N14), lida pelo código
/// gerado no cabeçalho ([`CabecalhoDeLista`]).
///
/// `Geral` guarda cada elemento como um [`TaggedValue`] de 16 bytes. As
/// outras são as listas compactas: o `E` reificado é exatamente `int`,
/// `double` ou `bool` (não anulável), e cada elemento são só os 8 bytes dos
/// bits, sem tag nem referência — o coletor não os percorre. O código de
/// cada forma é o mesmo das gravações diretas (`codigo` de
/// `dartforge_lista_len_gravavel`: 1 `int`, 2 `double`, 3 `bool`).
///
/// A forma é invisível para o programa: `is`/`as`, a covariância e os erros
/// vêm do `E` reificado (o metadado do slot) e das conferências do SDK, que
/// não mudam. Um valor que a forma não guarda (só o código do runtime
/// poderia gravá-lo, sem a conferência de tipo) devolve a lista à forma
/// geral ([`Elementos::descompactar`]) antes da gravação.
///
/// **Vagas.** O SDK grava `null` em posições de uma lista de `E` não
/// anulável em dois momentos, e nunca as lê antes de gravar de novo: o
/// `_List(n)` recém-alocado (que `List.filled`/`List.generate` preenchem em
/// seguida) e o `null` que o `length =` da `_GrowableList` grava nas
/// posições que vai cortar (logo depois cortadas). A forma compacta guarda
/// essas vagas como 0 (`false`, `0.0`), como o `_List` da VM guarda o null
/// que ninguém lê.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FormaDeLista {
    Geral = 0,
    Int = 1,
    Double = 2,
    Bool = 3,
}

impl FormaDeLista {
    /// A forma compacta do código de gravação (1 `int`, 2 `double`, 3 `bool`).
    pub fn do_codigo(codigo: i64) -> Option<Self> {
        match codigo {
            1 => Some(Self::Int),
            2 => Some(Self::Double),
            3 => Some(Self::Bool),
            _ => None,
        }
    }

    fn tag(self) -> ValueTag {
        match self {
            Self::Int => ValueTag::Int,
            Self::Double => ValueTag::Double,
            Self::Bool => ValueTag::Bool,
            Self::Geral => ValueTag::Ref,
        }
    }

    /// Os bits de `v` nesta forma compacta, ou `None` se ela não o guarda.
    /// O null é uma vaga (ver o tipo): 0.
    fn bits_de(self, v: TaggedValue) -> Option<i64> {
        if self == Self::Geral {
            return None;
        }
        if v.is_ref {
            return (v.bits == 0).then_some(0);
        }
        (v.tag == self.tag()).then_some(v.bits)
    }

    /// O elemento de bits `bits` nesta forma compacta.
    fn valor(self, bits: i64) -> TaggedValue {
        TaggedValue { bits, is_ref: false, tag: self.tag() }
    }
}

/// Os elementos de uma `Value::List` atrás de um cabeçalho de endereço fixo.
///
/// O código gerado lê o cabeçalho em linha (`lower/tipados.rs`): o endereço
/// dos elementos no deslocamento 0, o comprimento lógico no 8, no 16 as
/// gravações diretas já conferidas e no 24 a forma ([`FormaDeLista`]: o
/// tamanho e a interpretação de cada elemento). O cabeçalho mora num `Box`
/// e não muda de endereço enquanto a lista vive (o slot pode mover o
/// `Value`, o `Box` não): `dartforge_lista_cabecalho` é uma função pura do
/// handle, que o LLVM tira dos laços. Toda mudança de estrutura ou de forma
/// ressincroniza o cabeçalho; gravar um elemento que a forma guarda não o
/// muda.
pub struct Elementos(Box<CabecalhoDeLista>);

/// O cabeçalho lido pelo código gerado; ver [`Elementos`].
#[repr(C)]
pub struct CabecalhoDeLista {
    /// `vetor.as_mut_ptr()` na forma geral, `compacto.as_mut_ptr()` nas
    /// compactas.
    dados: *mut u8,
    /// O comprimento lógico: `logico`, ou o do vetor da forma.
    len: i64,
    /// Bit `1 << codigo` (1 `int`, 2 `double`, 3 `bool`): a lista é
    /// modificável, compacta da forma `codigo` (então o `E` reificado é o
    /// escalar), conferido por `dartforge_lista_len_gravavel`; o código
    /// gerado grava os 8 bytes direto. Bit `1 << (codigo + 4)`: a lista
    /// cresce e aceita o escalar (`dartforge_lista_add_escalar`). Zerado
    /// quando o `E`, a imutabilidade ou a forma mudam
    /// ([`Elementos::esquecer_gravacoes`]).
    gravavel: i64,
    /// A [`FormaDeLista`], como `i64`.
    forma: i64,
    /// Os elementos na forma geral (vazio nas compactas).
    vetor: Vec<TaggedValue>,
    /// Os bits dos elementos nas formas compactas (vazio na geral).
    compacto: Vec<i64>,
    /// `_GrowableList._withData(data)` (P5c): o vetor tem os elementos de
    /// `data` (a reserva) e o tamanho lógico é este, até o primeiro
    /// `_setLength`/`_setData` — na VM a lista aponta para o `_List` e o
    /// tamanho é outro campo.
    logico: Option<usize>,
}

const _: () = {
    assert!(std::mem::offset_of!(CabecalhoDeLista, dados) == 0);
    assert!(std::mem::offset_of!(CabecalhoDeLista, len) == 8);
    assert!(std::mem::offset_of!(CabecalhoDeLista, gravavel) == 16);
    assert!(std::mem::offset_of!(CabecalhoDeLista, forma) == 24);
};

/// O cabeçalho de quem não é lista do runtime: comprimento 0 (nenhum índice
/// passa no teste de limites), nenhuma gravação conferida, forma geral.
pub static CABECALHO_VAZIO: CabecalhoDeLista = CabecalhoDeLista {
    dados: std::ptr::null_mut(),
    len: 0,
    gravavel: 0,
    forma: 0,
    vetor: Vec::new(),
    compacto: Vec::new(),
    logico: None,
};

// SAFETY: `dados` aponta para o buffer de um dos vetores do próprio
// cabeçalho (ou é nulo no `CABECALHO_VAZIO`, que nunca é gravado): mover ou
// compartilhar o cabeçalho entre threads é mover os `Vec`, que são
// `Send`/`Sync`.
#[allow(unsafe_code)]
unsafe impl Send for CabecalhoDeLista {}
#[allow(unsafe_code)]
unsafe impl Sync for CabecalhoDeLista {}

impl Elementos {
    /// Uma lista geral com os elementos `vetor`.
    pub fn new(vetor: Vec<TaggedValue>) -> Self {
        let mut e = Self(Box::new(CabecalhoDeLista {
            dados: std::ptr::null_mut(),
            len: 0,
            gravavel: 0,
            forma: 0,
            vetor,
            compacto: Vec::new(),
            logico: None,
        }));
        e.sincronizar();
        e
    }

    fn sincronizar(&mut self) {
        let c = &mut *self.0;
        let fisico = if c.forma == 0 {
            c.dados = c.vetor.as_mut_ptr().cast();
            c.vetor.len()
        } else {
            c.dados = c.compacto.as_mut_ptr().cast();
            c.compacto.len()
        };
        c.len = c.logico.unwrap_or(fisico) as i64;
    }

    /// A forma dos elementos.
    pub fn forma(&self) -> FormaDeLista {
        match self.0.forma {
            1 => FormaDeLista::Int,
            2 => FormaDeLista::Double,
            3 => FormaDeLista::Bool,
            _ => FormaDeLista::Geral,
        }
    }

    /// Passa à forma compacta `forma` se todo elemento (a reserva de
    /// `_withData` inclusive) cabe nela — o escalar da forma, ou uma vaga
    /// null; senão fica como está e devolve falso. Esquece as gravações
    /// conferidas.
    pub fn compactar(&mut self, forma: FormaDeLista) -> bool {
        if forma == FormaDeLista::Geral || self.forma() == forma {
            return self.forma() == forma;
        }
        if self.forma() != FormaDeLista::Geral {
            self.descompactar();
        }
        let c = &mut *self.0;
        let mut bits = Vec::with_capacity(c.vetor.capacity());
        for v in &c.vetor {
            match forma.bits_de(*v) {
                Some(b) => bits.push(b),
                None => return false,
            }
        }
        c.vetor = Vec::new();
        c.compacto = bits;
        c.forma = forma as i64;
        c.gravavel = 0;
        self.sincronizar();
        true
    }

    /// Volta à forma geral (os escalares com a tag), com a mesma
    /// capacidade. Esquece as gravações conferidas.
    pub fn descompactar(&mut self) {
        let forma = self.forma();
        if forma == FormaDeLista::Geral {
            return;
        }
        let c = &mut *self.0;
        let mut vetor = Vec::with_capacity(c.compacto.capacity());
        vetor.extend(c.compacto.iter().map(|&b| forma.valor(b)));
        c.compacto = Vec::new();
        c.vetor = vetor;
        c.forma = 0;
        c.gravavel = 0;
        self.sincronizar();
    }

    /// Quantos elementos o vetor da forma tem (a reserva de `_withData`
    /// inclusive; o comprimento do Dart é [`Elementos::len_logico`]).
    pub fn len(&self) -> usize {
        if self.0.forma == 0 { self.0.vetor.len() } else { self.0.compacto.len() }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A capacidade do vetor da forma.
    pub fn capacity(&self) -> usize {
        if self.0.forma == 0 { self.0.vetor.capacity() } else { self.0.compacto.capacity() }
    }

    /// Os bytes do buffer de elementos (para a contabilidade do coletor).
    pub fn bytes_reservados(&self) -> usize {
        if self.0.forma == 0 {
            self.0.vetor.capacity() * std::mem::size_of::<TaggedValue>()
        } else {
            self.0.compacto.capacity() * std::mem::size_of::<i64>()
        }
    }

    /// O elemento `i`, se existe.
    pub fn get(&self, i: usize) -> Option<TaggedValue> {
        match self.forma() {
            FormaDeLista::Geral => self.0.vetor.get(i).copied(),
            f => self.0.compacto.get(i).map(|&b| f.valor(b)),
        }
    }

    /// O elemento `i`.
    ///
    /// # Panics
    /// Se `i` passa do vetor da forma.
    pub fn valor(&self, i: usize) -> TaggedValue {
        match self.forma() {
            FormaDeLista::Geral => self.0.vetor[i],
            f => f.valor(self.0.compacto[i]),
        }
    }

    /// Os elementos, com a tag.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = TaggedValue> + ExactSizeIterator + '_ {
        (0..self.len()).map(move |i| self.valor(i))
    }

    pub fn first(&self) -> Option<TaggedValue> {
        self.get(0)
    }

    pub fn last(&self) -> Option<TaggedValue> {
        self.len().checked_sub(1).and_then(|i| self.get(i))
    }

    /// As referências dos elementos (nenhuma numa forma compacta).
    pub fn referencias(&self) -> impl Iterator<Item = i64> + '_ {
        self.0.vetor.iter().filter_map(|v| v.is_ref.then_some(v.bits))
    }

    /// Os elementos, com a tag, num vetor novo.
    pub fn to_vec(&self) -> Vec<TaggedValue> {
        self.iter().collect()
    }

    /// Os elementos `[inicio, fim)` numa lista nova da mesma forma.
    ///
    /// # Panics
    /// Se a faixa passa do vetor da forma.
    pub fn fatia(&self, inicio: usize, fim: usize) -> Self {
        let mut e = Self::new(Vec::new());
        match self.forma() {
            FormaDeLista::Geral => e.0.vetor = self.0.vetor[inicio..fim].to_vec(),
            f => {
                e.0.compacto = self.0.compacto[inicio..fim].to_vec();
                e.0.forma = f as i64;
            }
        }
        e.sincronizar();
        e
    }

    /// Grava `v` (normalizado, R8) no elemento `i` existente; um valor que
    /// a forma compacta não guarda a devolve à forma geral antes.
    ///
    /// # Panics
    /// Se `i` passa do vetor da forma.
    pub fn definir(&mut self, i: usize, v: TaggedValue) {
        let forma = self.forma();
        if forma != FormaDeLista::Geral {
            if let Some(b) = forma.bits_de(v) {
                self.0.compacto[i] = b;
                return;
            }
            self.descompactar();
        }
        self.0.vetor[i] = v;
    }

    /// Grava `v` nos `n` primeiros elementos.
    pub fn preencher(&mut self, n: usize, v: TaggedValue) {
        let forma = self.forma();
        if forma != FormaDeLista::Geral {
            if let Some(b) = forma.bits_de(v) {
                self.0.compacto[..n].fill(b);
                return;
            }
            self.descompactar();
        }
        self.0.vetor[..n].fill(v);
    }

    /// Acrescenta `v` (normalizado) no fim do vetor.
    pub fn push(&mut self, v: TaggedValue) {
        let forma = self.forma();
        if forma != FormaDeLista::Geral {
            if let Some(b) = forma.bits_de(v) {
                self.0.compacto.push(b);
                self.sincronizar();
                return;
            }
            self.descompactar();
        }
        self.0.vetor.push(v);
        self.sincronizar();
    }

    /// O vetor passa a ter `n` elementos; os novos são `v`.
    pub fn redimensionar(&mut self, n: usize, v: TaggedValue) {
        let forma = self.forma();
        if forma != FormaDeLista::Geral {
            if let Some(b) = forma.bits_de(v) {
                self.0.compacto.resize(n, b);
                self.sincronizar();
                return;
            }
            self.descompactar();
        }
        self.0.vetor.resize(n, v);
        self.sincronizar();
    }

    /// Corta o vetor em `n` elementos.
    pub fn truncar(&mut self, n: usize) {
        self.0.vetor.truncate(n);
        self.0.compacto.truncate(n);
        self.sincronizar();
    }

    /// Reserva capacidade para `adicional` elementos além dos que há.
    pub fn reservar_exato(&mut self, adicional: usize) {
        if self.0.forma == 0 {
            self.0.vetor.reserve_exact(adicional);
        } else {
            self.0.compacto.reserve_exact(adicional);
        }
        self.sincronizar();
    }

    /// Troca os elementos por `novos` (normalizados), na forma corrente se
    /// todos cabem nela, senão na geral. A capacidade é a de `novos`.
    pub fn substituir(&mut self, novos: Vec<TaggedValue>) {
        let forma = self.forma();
        if forma != FormaDeLista::Geral {
            let bits: Option<Vec<i64>> = {
                let mut b = Vec::with_capacity(novos.capacity());
                novos.iter().try_for_each(|v| forma.bits_de(*v).map(|x| b.push(x))).map(|()| b)
            };
            if let Some(b) = bits {
                self.0.compacto = b;
                self.sincronizar();
                return;
            }
            self.0.compacto = Vec::new();
            self.0.forma = 0;
            self.0.gravavel = 0;
        }
        self.0.vetor = novos;
        self.sincronizar();
    }

    /// Muda a estrutura (comprimento, capacidade) na forma geral: devolve
    /// a lista compacta à geral, e a guarda ressincroniza o cabeçalho quando
    /// sai de escopo. Os caminhos comuns têm métodos próprios, que mantêm a
    /// forma ([`Elementos::push`], [`Elementos::redimensionar`]…).
    pub fn vetor_mut(&mut self) -> GuardaDeElementos<'_> {
        self.descompactar();
        GuardaDeElementos(self)
    }

    /// O tamanho lógico de `_withData`, se ainda pendente.
    pub fn logico(&self) -> Option<usize> {
        self.0.logico
    }

    pub fn definir_logico(&mut self, n: Option<usize>) {
        self.0.logico = n;
        self.sincronizar();
    }

    /// O comprimento que o Dart vê.
    pub fn len_logico(&self) -> usize {
        self.0.logico.unwrap_or(self.len())
    }

    pub fn cabecalho(&self) -> *const CabecalhoDeLista {
        &*self.0
    }

    pub fn gravacao_conferida(&self, codigo: i64) -> bool {
        self.0.gravavel & (1 << codigo) != 0
    }

    pub fn conferir_gravacao(&mut self, codigo: i64) {
        self.0.gravavel |= 1 << codigo;
    }

    pub fn esquecer_gravacoes(&mut self) {
        self.0.gravavel = 0;
    }

    /// Os elementos, com a tag (a forma geral).
    pub fn into_vec(mut self) -> Vec<TaggedValue> {
        self.descompactar();
        std::mem::take(&mut self.0.vetor)
    }
}

impl FromIterator<TaggedValue> for Elementos {
    fn from_iter<I: IntoIterator<Item = TaggedValue>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl From<Vec<TaggedValue>> for Elementos {
    fn from(vetor: Vec<TaggedValue>) -> Self {
        Self::new(vetor)
    }
}

impl Clone for Elementos {
    /// Outra lista, da mesma forma: cabeçalho próprio, gravações a conferir
    /// de novo.
    fn clone(&self) -> Self {
        let mut e = Self::new(self.0.vetor.clone());
        e.0.compacto = self.0.compacto.clone();
        e.0.forma = self.0.forma;
        e.0.logico = self.0.logico;
        e.sincronizar();
        e
    }
}

impl std::fmt::Debug for Elementos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Acesso de estrutura a [`Elementos`] na forma geral; ressincroniza o
/// cabeçalho ao sair.
pub struct GuardaDeElementos<'a>(&'a mut Elementos);

impl std::ops::Deref for GuardaDeElementos<'_> {
    type Target = Vec<TaggedValue>;
    fn deref(&self) -> &Vec<TaggedValue> {
        &self.0.0.vetor
    }
}

impl std::ops::DerefMut for GuardaDeElementos<'_> {
    fn deref_mut(&mut self) -> &mut Vec<TaggedValue> {
        &mut self.0.0.vetor
    }
}

impl Drop for GuardaDeElementos<'_> {
    fn drop(&mut self) {
        self.0.sincronizar();
    }
}

/// Uma closure: imutável depois de criada. Layout C fixo, lido em linha
/// pela chamada tipada (`lower/closures.rs`) a partir de
/// `dartforge_closure_cabecalho`.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CabecalhoDeClosure {
    pub code_id: i64,
    pub environment: i64,
    /// O corpo com a ABI tipada, ou 0: chamado direto quando a ABI de quem
    /// chama é `abi`.
    pub tipado: i64,
    pub abi: i64,
}

const _: () = {
    assert!(std::mem::offset_of!(CabecalhoDeClosure, environment) == 8);
    assert!(std::mem::offset_of!(CabecalhoDeClosure, tipado) == 16);
    assert!(std::mem::offset_of!(CabecalhoDeClosure, abi) == 24);
};

/// O cabeçalho de quem não é closure: `abi` 0 não casa com nenhuma chamada
/// tipada.
pub static CLOSURE_VAZIA: CabecalhoDeClosure = CabecalhoDeClosure { code_id: 0, environment: 0, tipado: 0, abi: 0 };

/// Um campo de objeto: os bits e se são referência (o que o coletor segue).
pub type Campo = (i64, bool);

/// Maior índice de campo que o emissor lê em linha (acima dele, a chamada
/// `dartforge_object_get`/`_set`).
pub const CAMPOS_EM_LINHA: usize = 4096;

/// O cabeçalho de um objeto do usuário no espaço de objetos
/// ([`EspacoDeObjetos`]), seguido dos campos (8 bytes cada, os bits) e, com
/// mais de 32 campos, das palavras que estendem o mapa de referências.
/// Layout C fixo: contrato com o emissor (`llvm/mod.rs`).
///
/// O handle do objeto é o endereço do cabeçalho com o bit 1 ligado
/// (`bloco + 2`, [`DESLOCAMENTO_DO_HANDLE`]); os handles da tabela de slots
/// são múltiplos de 4 e o `Smi` é ímpar (R10), então `h & 3 == 2`
/// distingue o objeto sem consulta.
///
/// Como na VM, o campo não carrega a marca de referência: o **mapa**
/// (`mapa`, um bit por campo, e as palavras de extensão) diz quais campos o
/// coletor segue — a VM tira isso da classe (o *unboxed fields bitmap*);
/// aqui cada gravação acende ou apaga o bit do campo, então um campo que a
/// representação faz às vezes `Ref` e às vezes escalar continua preciso.
///
/// Um objeto cujos campos mudaram de número depois de criado (o erro que
/// ganha o rastro, a recarga do JIT que muda o layout da classe, J03) não
/// pode mudar de endereço: os campos vão para um **corpo de fora** (bit
/// [`FORA`] em `flags`), com o mesmo layout (cabeçalho + campos + extensão),
/// cujo endereço fica no lugar do primeiro campo. O código gerado lê o
/// corpo de fora com uma seleção sem desvio (`llvm/mod.rs`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cabecalho {
    /// [`LIVRE`], [`JOVEM`], [`MARCADO`] (pela coleta em curso), [`VELHO`]
    /// ou [`LEMBRADO`] (velho que ganhou referência depois da última coleta).
    pub estado: u8,
    /// [`FORA`]: os campos moram num corpo de fora.
    pub flags: u8,
    /// O número de campos (no corpo de fora, o dele).
    pub n: u16,
    /// A classe (o `class_id` do código gerado; cabe em 32 bits).
    pub class_id: i32,
    /// O mapa de referências dos 32 primeiros campos.
    pub mapa: u32,
    /// O metadado (RTI) do objeto, `id + 1` (0 = nenhum).
    pub metadado: u32,
}

const _: () = {
    assert!(std::mem::size_of::<Cabecalho>() == 16);
    assert!(std::mem::offset_of!(Cabecalho, estado) == 0);
    assert!(std::mem::offset_of!(Cabecalho, flags) == 1);
    assert!(std::mem::offset_of!(Cabecalho, n) == 2);
    assert!(std::mem::offset_of!(Cabecalho, class_id) == 4);
    assert!(std::mem::offset_of!(Cabecalho, mapa) == 8);
    assert!(std::mem::offset_of!(Cabecalho, metadado) == 12);
};

/// Estados de um bloco ([`Cabecalho::estado`]). O coletor é geracional sem
/// mover objetos (marcas "pegajosas"): um objeto nasce [`JOVEM`] (a
/// alocação em linha grava 1), e o que sobrevive a uma coleta vira
/// [`VELHO`]. A coleta menor só marca e varre os jovens; os velhos contam
/// como vivos, e as referências de velho para jovem são achadas pelos
/// [`LEMBRADO`]s — a barreira de escrita (`Heap::get_mut` e as gravações de
/// campo do runtime, e a do código gerado depois de gravar um `Ref` num
/// campo, `llvm/mod.rs`) marca o velho que recebe uma referência, como o
/// *store buffer* da VM.
pub const LIVRE: u8 = 0;
pub const JOVEM: u8 = 1;
pub const MARCADO: u8 = 2;
pub const VELHO: u8 = 3;
pub const LEMBRADO: u8 = 4;
/// Bit de [`Cabecalho::flags`]: os campos moram num corpo de fora.
pub const FORA: u8 = 1;

/// O handle de um objeto do espaço é `bloco + DESLOCAMENTO_DO_HANDLE`.
pub const DESLOCAMENTO_DO_HANDLE: i64 = 2;
/// Tamanho da página do espaço de objetos (e o alinhamento dela: a página
/// de um handle é `h & !(PAGINA - 1)`).
pub const PAGINA: usize = 64 * 1024;
/// Maior número de campos das classes de tamanho do espaço; um objeto
/// maior tem uma página só dele.
pub const MAIOR_CLASSE: usize = 64;
/// Os números de campos com alocação em linha (TLAB): `0..=TLAB_N`.
pub const TLAB_N: usize = 16;
/// Até quantos blocos cada reabastecimento da TLAB entrega (uma faixa
/// contígua da lista livre: menos, se a contiguidade acaba antes).
const TLAB_BLOCOS: usize = 256;

/// A coleta menor vem a cada `LIMITE_JOVEM` bytes alocados (o tamanho do
/// *new space*; o semiespaço da VM começa menor e cresce até 8 MiB em 64
/// bits — aqui fixo, entre o custo por coleta e o pico de memória)…
const LIMITE_JOVEM: usize = 4 * 1024 * 1024;
/// … ou a cada `CONTAGEM_JOVEM` alocações (valores pequenos do runtime).
const CONTAGEM_JOVEM: usize = 256 * 1024;
/// No `--gc-stress`, uma coleta completa a cada tantas (as demais, menores:
/// é nelas que uma barreira faltando apareceria).
const MENORES_POR_COMPLETA_NO_ESTRESSE: u64 = 8;

/// Palavras de extensão do mapa de referências para `n` campos (os 32
/// primeiros bits estão no cabeçalho).
#[inline]
pub const fn palavras_do_mapa(n: usize) -> usize {
    if n > 32 { (n - 32).div_ceil(64) } else { 0 }
}

/// Campos que o bloco guarda: pelo menos um (o lugar do encadeamento da
/// lista livre e do endereço do corpo de fora).
#[inline]
pub const fn capacidade(n: usize) -> usize {
    if n == 0 { 1 } else { n }
}

/// Bytes de um bloco (ou corpo de fora) de `n` campos.
#[inline]
pub const fn tamanho_do_bloco(n: usize) -> usize {
    std::mem::size_of::<Cabecalho>() + 8 * (capacidade(n) + palavras_do_mapa(n))
}

/// O objeto de ninguém: cabeçalho zerado e campos zerados. A leitura em
/// linha de um campo de algo que não é objeto do espaço lê daqui (o que
/// `dartforge_object_get` devolvia); nunca é gravado — está em memória só
/// de leitura, e gravar nela é erro do compilador que termina o processo.
#[repr(C)]
pub struct ObjetoVazio {
    pub cabecalho: Cabecalho,
    pub campos: [i64; CAMPOS_EM_LINHA],
}
pub static OBJETO_VAZIO: ObjetoVazio = ObjetoVazio {
    cabecalho: Cabecalho { estado: LIVRE, flags: 0, n: 0, class_id: 0, mapa: 0, metadado: 0 },
    campos: [0; CAMPOS_EM_LINHA],
};

/// O corpo de um objeto (o próprio bloco, ou o corpo de fora).
///
/// # Safety
/// `b` é o cabeçalho de um bloco vivo do espaço.
#[inline]
#[allow(unsafe_code)]
unsafe fn corpo(b: *mut Cabecalho) -> *mut Cabecalho {
    // SAFETY: o contrato da função; com FORA, o primeiro campo é o endereço.
    unsafe {
        if (*b).flags & FORA != 0 { *campos_de(b).cast::<*mut Cabecalho>() } else { b }
    }
}

/// Os campos depois de um cabeçalho (bloco ou corpo).
#[inline]
fn campos_de(c: *mut Cabecalho) -> *mut i64 {
    c.wrapping_add(1).cast()
}

/// O campo `i` de um corpo é referência?
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
#[allow(unsafe_code)]
unsafe fn e_referencia(c: *const Cabecalho, i: usize) -> bool {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            (*c).mapa >> i & 1 != 0
        } else {
            let ext = campos_de(c.cast_mut()).add(capacidade(usize::from((*c).n))).cast::<u64>();
            *ext.add((i - 32) / 64) >> ((i - 32) % 64) & 1 != 0
        }
    }
}

/// Acende ou apaga o bit de referência do campo `i` de um corpo.
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
#[allow(unsafe_code)]
unsafe fn marcar_referencia(c: *mut Cabecalho, i: usize, e_ref: bool) {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            if e_ref { (*c).mapa |= 1 << i } else { (*c).mapa &= !(1 << i) }
        } else {
            let w = campos_de(c).add(capacidade(usize::from((*c).n))).cast::<u64>().add((i - 32) / 64);
            let bit = 1u64 << ((i - 32) % 64);
            if e_ref { *w |= bit } else { *w &= !bit }
        }
    }
}

/// Empilha os campos-referência de um corpo; devolve quantos campos tem.
///
/// # Safety
/// `c` é um corpo válido.
#[inline]
#[allow(unsafe_code)]
unsafe fn empilhar_referencias(c: *const Cabecalho, pilha: &mut Vec<i64>) -> usize {
    // SAFETY: o contrato da função.
    unsafe {
        let n = usize::from((*c).n);
        let campos = campos_de(c.cast_mut());
        let mut m = (*c).mapa;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            pilha.push(*campos.add(i));
            m &= m - 1;
        }
        if n > 32 {
            let ext = campos.add(capacidade(n)).cast::<u64>();
            for w in 0..palavras_do_mapa(n) {
                let mut m = *ext.add(w);
                while m != 0 {
                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                    pilha.push(*campos.add(i));
                    m &= m - 1;
                }
            }
        }
        n
    }
}

/// Um corpo de fora de `n` campos zerados (o alocador do sistema).
fn novo_corpo_de_fora(n: usize) -> *mut Cabecalho {
    let layout = std::alloc::Layout::from_size_align(tamanho_do_bloco(n), 8).expect("layout do corpo");
    // SAFETY: layout de tamanho não nulo.
    #[allow(unsafe_code)]
    let p = unsafe { std::alloc::alloc_zeroed(layout) }.cast::<Cabecalho>();
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    // SAFETY: memória nova, zerada, do tamanho do corpo.
    #[allow(unsafe_code)]
    unsafe {
        (*p).n = u16::try_from(n).expect("objeto com campos demais");
    }
    p
}

/// Solta um corpo de fora.
///
/// # Safety
/// `c` veio de [`novo_corpo_de_fora`] e ninguém mais o usa.
#[allow(unsafe_code)]
unsafe fn soltar_corpo_de_fora(c: *mut Cabecalho) {
    // SAFETY: o contrato da função.
    unsafe {
        let n = usize::from((*c).n);
        std::alloc::dealloc(c.cast(), std::alloc::Layout::from_size_align(tamanho_do_bloco(n), 8).expect("layout do corpo"));
    }
}

/// A vista de um objeto do espaço para o runtime ([`Heap::objeto`]): a
/// classe e os campos, `(bits, é referência)`.
#[derive(Clone, Copy)]
pub struct Obj<'a> {
    corpo: *const Cabecalho,
    /// A classe do objeto.
    pub class_id: i64,
    _heap: std::marker::PhantomData<&'a Heap>,
}

#[allow(unsafe_code)]
impl Obj<'_> {
    /// Quantos campos.
    pub fn len(&self) -> usize {
        // SAFETY: corpo vivo enquanto o empréstimo do heap dura.
        usize::from(unsafe { (*self.corpo).n })
    }
    /// Sem campos?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// O campo `i`: os bits e se é referência.
    ///
    /// # Panics
    /// Com `i` fora do objeto.
    pub fn campo(&self, i: usize) -> Campo {
        assert!(i < self.len(), "campo {i} fora do objeto de {} campos", self.len());
        // SAFETY: `i < n`; corpo vivo.
        unsafe { (*campos_de(self.corpo.cast_mut()).add(i), e_referencia(self.corpo, i)) }
    }
    /// O campo `i`, se existe.
    pub fn get(&self, i: usize) -> Option<Campo> {
        (i < self.len()).then(|| self.campo(i))
    }
    /// O primeiro campo, se existe.
    pub fn first(&self) -> Option<Campo> {
        self.get(0)
    }
    /// Os campos, em ordem.
    pub fn iter(&self) -> impl Iterator<Item = Campo> + '_ {
        (0..self.len()).map(|i| self.campo(i))
    }
    /// Os campos copiados.
    pub fn to_vec(&self) -> Vec<Campo> {
        self.iter().collect()
    }
}

impl std::fmt::Debug for Obj<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Obj").field("class_id", &self.class_id).field("campos", &self.to_vec()).finish()
    }
}

/// Uma página do espaço de objetos: blocos de um só tamanho.
#[derive(Debug)]
struct Pagina {
    base: *mut u8,
    /// Bytes alocados (`PAGINA`, ou mais para um objeto grande).
    bytes: usize,
    /// Número de campos dos blocos.
    n: usize,
    blocos: usize,
    /// O inverso de `tamanho_do_bloco(n) / 8` (ímpar, ou potência de 2
    /// vezes ímpar: ver `indice`) módulo 2³²: o índice de um bloco sai de uma
    /// multiplicação, e um deslocamento que não é múltiplo do tamanho dá um
    /// índice além de `blocos` (o teste de divisibilidade exata, *Hacker's
    /// Delight* 10-17).
    inverso: u32,
    /// `tamanho_do_bloco(n) / 8` = `impar << deslocamento`.
    deslocamento: u32,
    /// Página sem objeto vivo guardada para reuso (em
    /// [`EspacoDeObjetos::vazias`]): o conteúdo é lixo dos mortos até ela
    /// ser zerada e formatada de novo, para qualquer número de campos.
    vazia: bool,
}

impl Pagina {
    /// A página de blocos de `n` campos em `base` (`bytes` bytes).
    fn formatada(base: *mut u8, bytes: usize, n: usize) -> Self {
        let tamanho = tamanho_do_bloco(n);
        let blocos = if n > MAIOR_CLASSE { 1 } else { PAGINA / tamanho };
        let palavras = u32::try_from(tamanho / 8).unwrap_or(1);
        let deslocamento = palavras.trailing_zeros();
        let inverso = inverso_impar(palavras >> deslocamento);
        Pagina { base, bytes, n, blocos, inverso, deslocamento, vazia: false }
    }
    fn layout(bytes: usize) -> std::alloc::Layout {
        std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout da página")
    }
    fn bloco(&self, i: usize) -> *mut Cabecalho {
        // SAFETY (dos chamadores): `i < blocos`, dentro da página.
        #[allow(unsafe_code)]
        unsafe {
            self.base.add(i * tamanho_do_bloco(self.n)).cast()
        }
    }
    /// O índice do bloco que começa `desl` bytes depois da base, se algum
    /// começa ali.
    #[inline]
    fn indice(&self, desl: usize) -> Option<usize> {
        if desl & 7 != 0 || desl >= self.bytes || self.vazia {
            return None;
        }
        let palavras = (desl >> 3) as u32;
        let mascara = (1u32 << self.deslocamento) - 1;
        if palavras & mascara != 0 {
            return None;
        }
        let i = (palavras >> self.deslocamento).wrapping_mul(self.inverso) as usize;
        (i < self.blocos).then_some(i)
    }
}

/// O inverso de `a` (ímpar) módulo 2³² (Newton: cada passo dobra os bits
/// certos).
const fn inverso_impar(a: u32) -> u32 {
    let mut x = a;
    let mut i = 0;
    while i < 5 {
        x = x.wrapping_mul(2u32.wrapping_sub(a.wrapping_mul(x)));
        i += 1;
    }
    x
}

/// Base da página (`endereço >> 16`) → índice + 1 em `paginas`: tabelas de
/// 2¹⁶ posições por região de 4 GiB (quase sempre uma só), como o
/// *pagemap* de um alocador — a validação de um handle sem hash.
#[derive(Default)]
struct MapaDePaginas {
    regioes: Vec<(usize, Box<[u32]>)>,
}

impl MapaDePaginas {
    const BITS: usize = 16;
    #[inline]
    fn chave(base: usize) -> (usize, usize) {
        let k = base >> PAGINA.trailing_zeros();
        (k >> Self::BITS, k & ((1 << Self::BITS) - 1))
    }
    #[inline]
    fn get(&self, base: usize) -> Option<usize> {
        let (r, i) = Self::chave(base);
        let (_, t) = self.regioes.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }
    fn definir(&mut self, base: usize, valor: u32) {
        let (r, i) = Self::chave(base);
        let pos = match self.regioes.iter().position(|(x, _)| *x == r) {
            Some(p) => p,
            None => {
                self.regioes.push((r, vec![0u32; 1 << Self::BITS].into_boxed_slice()));
                self.regioes.len() - 1
            }
        };
        self.regioes[pos].1[i] = valor;
    }
    fn inserir(&mut self, base: usize, indice: usize) {
        self.definir(base, u32::try_from(indice + 1).expect("páginas demais"));
    }
    fn remover(&mut self, base: usize) {
        self.definir(base, 0);
    }
}

/// Zera `n` palavras em `p`: os tamanhos pequenos (quase todos os objetos)
/// com gravações de tamanho fixo, sem a chamada ao `memset`.
///
/// # Safety
/// `p` aponta `n` palavras graváveis.
#[inline]
#[allow(unsafe_code)]
unsafe fn zerar(p: *mut i64, n: usize) {
    // SAFETY: o contrato da função.
    unsafe {
        match n {
            0 => {}
            1 => p.write(0),
            2 => p.cast::<[i64; 2]>().write([0; 2]),
            3 => p.cast::<[i64; 3]>().write([0; 3]),
            4 => p.cast::<[i64; 4]>().write([0; 4]),
            _ => std::ptr::write_bytes(p, 0, n),
        }
    }
}

/// O espaço dos objetos do usuário: páginas de [`PAGINA`] bytes, cada uma
/// com blocos ([`Cabecalho`] + campos) de um só número de campos, e uma
/// lista livre por número de campos (encadeada pelo primeiro campo),
/// refeita a cada coleta completa (em ordem de endereço).
///
/// É o par, sem mover objetos, do *new space* da VM
/// (`runtime/vm/heap/scavenger.cc` e a alocação em linha do
/// `stub_code_compiler.cc`): alocar um objeto é tirar o primeiro bloco da
/// lista — pelo código gerado, da TLAB ([`Contexto::tlab`]) — sem `malloc`
/// nem entrada na tabela de slots.
pub struct EspacoDeObjetos {
    paginas: Vec<Pagina>,
    /// A página de cada base (a validação de um handle).
    mapa: MapaDePaginas,
    /// Cabeça da lista livre de cada número de campos (`0..=MAIOR_CLASSE`).
    livres: Vec<*mut Cabecalho>,
    /// Blocos vivos (entregues e não devolvidos).
    pub vivos: usize,
    /// Blocos em todas as páginas.
    pub blocos: usize,
    /// Os blocos entregues um a um pelo runtime desde a última coleta: com
    /// [`EspacoDeObjetos::faixas`], o que a coleta menor varre.
    jovens: Vec<*mut Cabecalho>,
    /// As faixas `(início, fim, n)` entregues às TLABs desde a última coleta
    /// (os blocos que o código gerado alocou; o resto não usado já voltou à
    /// lista livre e está livre).
    faixas: Vec<(*mut u8, *mut u8, usize)>,
    /// A região ainda não usada `[cursor, fim)` da página em uso de cada
    /// número de campos: blocos zerados (cabeçalho inclusive), que saem em
    /// ordem, sem lista livre (o *bump pointer*). Uma página nova (ou
    /// vazia reusada) entra inteira aqui; a coleta completa a desfaz.
    regiao: Vec<(*mut u8, *mut u8)>,
    /// As páginas sem objeto vivo guardadas pela coleta completa (índices
    /// em `paginas`), por zerar: a próxima região de qualquer número de
    /// campos sai delas antes de pedir memória ao sistema.
    vazias: Vec<usize>,
    /// Os velhos que a barreira de escrita marcou ([`LEMBRADO`]).
    lembrados: Vec<*mut Cabecalho>,
    /// Blocos entregues desde a última coleta, por número de campos: a
    /// demanda que a varredura reserva em páginas vazias.
    demanda: Vec<usize>,
    /// O pico recente de blocos ocupados (vivos e mortos numa coleta
    /// completa) de cada número de campos, que decai a cada coleta completa:
    /// um programa que refaz uma estrutura grande (a lista morre e volta)
    /// reusa as páginas em vez de devolvê-las e pedi-las de novo — cada
    /// página nova custa as faltas de página e a formatação.
    pico: Vec<usize>,
    /// Algum objeto já ganhou corpo de fora (senão o `Drop` não percorre os
    /// blocos atrás deles).
    tem_fora: bool,
}

impl std::fmt::Debug for EspacoDeObjetos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EspacoDeObjetos").field("paginas", &self.paginas.len()).field("vivos", &self.vivos).finish()
    }
}

#[allow(unsafe_code)]
impl EspacoDeObjetos {
    fn new() -> Self {
        Self {
            paginas: Vec::new(),
            mapa: MapaDePaginas::default(),
            livres: vec![std::ptr::null_mut(); MAIOR_CLASSE + 1],
            vivos: 0,
            blocos: 0,
            jovens: Vec::new(),
            faixas: Vec::new(),
            regiao: vec![(std::ptr::null_mut(), std::ptr::null_mut()); MAIOR_CLASSE + 1],
            vazias: Vec::new(),
            lembrados: Vec::new(),
            demanda: vec![0; MAIOR_CLASSE + 1],
            pico: vec![0; MAIOR_CLASSE + 1],
            tem_fora: false,
        }
    }

    /// O próximo da lista livre (no primeiro campo do bloco livre).
    ///
    /// # Safety
    /// `b` é bloco livre do espaço.
    #[inline]
    unsafe fn proximo(b: *mut Cabecalho) -> *mut Cabecalho {
        // SAFETY: o contrato da função.
        unsafe { *campos_de(b) as *mut Cabecalho }
    }

    /// Grava o encadeamento da lista livre.
    ///
    /// # Safety
    /// `b` é bloco livre do espaço.
    #[inline]
    unsafe fn encadear(b: *mut Cabecalho, proximo: *mut Cabecalho) {
        // SAFETY: o contrato da função.
        unsafe { *campos_de(b) = proximo as i64 };
    }

    /// Formata um bloco livre de `n` campos: cabeçalho zerado (menos `n`),
    /// campos e extensão do mapa zerados, `proximo` no primeiro campo.
    ///
    /// # Safety
    /// `b` aponta um bloco de `n` campos de uma página deste espaço, sem
    /// objeto vivo.
    unsafe fn formatar(b: *mut Cabecalho, n: usize, proximo: *mut Cabecalho) {
        // SAFETY: o contrato da função.
        unsafe {
            b.write(Cabecalho { estado: LIVRE, flags: 0, n: u16::try_from(n).unwrap_or(u16::MAX), class_id: 0, mapa: 0, metadado: 0 });
            zerar(campos_de(b), capacidade(n) + palavras_do_mapa(n));
            Self::encadear(b, proximo);
        }
    }

    /// Uma página zerada para blocos de `n` campos: uma vazia reusada
    /// (zerada e formatada agora) ou uma nova do sistema. Devolve o índice.
    fn pagina_zerada(&mut self, n: usize) -> usize {
        if n <= MAIOR_CLASSE
            && let Some(i) = self.vazias.pop()
        {
            let p = &mut self.paginas[i];
            // SAFETY: a página inteira é deste espaço e não tem objeto vivo.
            unsafe { std::ptr::write_bytes(p.base, 0, p.bytes) };
            self.blocos -= p.blocos;
            *p = Pagina::formatada(p.base, p.bytes, n);
            self.blocos += p.blocos;
            return i;
        }
        let tamanho = tamanho_do_bloco(n);
        let bytes = if n > MAIOR_CLASSE { tamanho.div_ceil(PAGINA) * PAGINA } else { PAGINA };
        let layout = Pagina::layout(bytes);
        // SAFETY: layout de tamanho não nulo.
        let base = unsafe { std::alloc::alloc_zeroed(layout) };
        if base.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        let pagina = Pagina::formatada(base, bytes, n);
        self.blocos += pagina.blocos;
        self.mapa.inserir(base as usize, self.paginas.len());
        self.paginas.push(pagina);
        self.paginas.len() - 1
    }

    /// Até `k` blocos zerados e contíguos de `n ≤ MAIOR_CLASSE` campos da
    /// região da classe (uma página zerada nova quando ela acaba): o início
    /// e quantos. O cabeçalho sai zerado, sem `n` (quem entrega o grava).
    fn tirar_da_regiao(&mut self, n: usize, k: usize) -> (*mut u8, usize) {
        let tamanho = tamanho_do_bloco(n);
        let (mut cursor, mut fim) = self.regiao[n];
        if (fim as usize).saturating_sub(cursor as usize) < tamanho {
            let i = self.pagina_zerada(n);
            let p = &self.paginas[i];
            cursor = p.base;
            fim = p.base.wrapping_add(p.blocos * tamanho);
        }
        let j = k.min((fim as usize - cursor as usize) / tamanho);
        self.regiao[n] = (cursor.wrapping_add(j * tamanho), fim);
        (cursor, j)
    }

    /// Tira um bloco livre de `n` campos (estado ainda 0) — da lista livre,
    /// ou da região — com o primeiro campo zerado.
    #[inline]
    fn tirar(&mut self, n: usize) -> *mut Cabecalho {
        if n > MAIOR_CLASSE {
            let i = self.pagina_zerada(n);
            let b = self.paginas[i].base.cast::<Cabecalho>();
            // SAFETY: o bloco da página nova.
            unsafe { (*b).n = u16::try_from(n).unwrap_or(u16::MAX) };
            return b;
        }
        let b = self.livres[n];
        if b.is_null() {
            let (inicio, _) = self.tirar_da_regiao(n, 1);
            let b = inicio.cast::<Cabecalho>();
            // SAFETY: bloco zerado da região.
            unsafe { (*b).n = n as u16 };
            return b;
        }
        // SAFETY: `b` é o primeiro bloco livre da lista de `n`.
        unsafe {
            self.livres[n] = Self::proximo(b);
            Self::encadear(b, std::ptr::null_mut());
        }
        b
    }

    /// Uma faixa de até `k` blocos livres contíguos de `n ≤ TLAB_N` campos
    /// para a TLAB: do começo da lista livre (em ordem de endereço dentro de
    /// cada página) enquanto contígua, ou da região. Devolve o início e
    /// quantos; os blocos têm campos zerados, menos talvez o primeiro (o
    /// encadeamento da lista), e o cabeçalho sem estado.
    fn tirar_faixa(&mut self, n: usize, k: usize) -> (*mut u8, usize) {
        debug_assert!(n <= MAIOR_CLASSE && k > 0);
        let tamanho = tamanho_do_bloco(n);
        let (inicio, j) = if self.livres[n].is_null() {
            self.tirar_da_regiao(n, k)
        } else {
            let inicio = self.livres[n];
            let mut fim = inicio;
            let mut j = 1;
            // SAFETY: blocos livres da lista de `n`, encadeados pelo
            // primeiro campo.
            unsafe {
                let mut proximo = Self::proximo(fim);
                while j < k && proximo == fim.cast::<u8>().wrapping_add(tamanho).cast() {
                    fim = proximo;
                    proximo = Self::proximo(fim);
                    j += 1;
                }
                self.livres[n] = proximo;
            }
            (inicio.cast::<u8>(), j)
        };
        self.faixas.push((inicio, inicio.wrapping_add(j * tamanho), n));
        self.demanda[n] += j;
        (inicio, j)
    }

    /// Devolve à frente da lista livre de `n` os blocos `[inicio, fim)`
    /// (o resto não usado de uma faixa), encadeados em ordem, com o `n` no
    /// cabeçalho.
    fn devolver_faixa(&mut self, n: usize, inicio: *mut u8, fim: *mut u8) {
        let tamanho = tamanho_do_bloco(n);
        let mut proximo = self.livres[n];
        let mut p = fim;
        while p > inicio {
            p = p.wrapping_sub(tamanho);
            let b = p.cast::<Cabecalho>();
            // SAFETY: bloco livre da faixa, sem objeto.
            unsafe {
                (*b).n = n as u16;
                Self::encadear(b, proximo);
            }
            proximo = b;
        }
        self.livres[n] = proximo;
    }

    /// Marca o velho `b` como [`LEMBRADO`] (a barreira de escrita).
    #[inline]
    fn lembrar(&mut self, b: *mut Cabecalho) {
        // SAFETY: bloco do espaço.
        unsafe {
            if (*b).estado == VELHO {
                (*b).estado = LEMBRADO;
                self.lembrados.push(b);
            }
        }
    }

    /// Bytes que o objeto do bloco `b` ocupa (com o corpo de fora).
    ///
    /// # Safety
    /// `b` é bloco com objeto.
    unsafe fn bytes_do_objeto(b: *mut Cabecalho, n: usize) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            let fora = if (*b).flags & FORA != 0 { tamanho_do_bloco(usize::from((*corpo(b)).n)) } else { 0 };
            tamanho_do_bloco(n) + fora
        }
    }

    /// A varredura da coleta menor: os jovens não marcados voltam à lista
    /// livre (os marcados a marcação já fez velhos); os lembrados voltam a
    /// velhos. Devolve (mortos, bytes promovidos, bytes soltos).
    fn varrer_jovens(&mut self) -> (usize, usize, usize) {
        let (mut mortos, mut promovidos, mut soltos) = (0, 0, 0);
        let jovens = std::mem::take(&mut self.jovens);
        for &b in &jovens {
            // SAFETY: bloco entregue desde a última coleta, de uma página
            // ainda viva (as páginas só saem na coleta completa).
            unsafe {
                let classe = usize::from((*b).n);
                let n = if classe > MAIOR_CLASSE { self.pagina_do_grande(b) } else { classe };
                match (*b).estado {
                    VELHO => {
                        // Promovido pela marcação.
                        promovidos += Self::bytes_do_objeto(b, n);
                    }
                    JOVEM => {
                        mortos += 1;
                        soltos += Self::bytes_do_objeto(b, n);
                        Self::soltar_objeto(b, n);
                        if n > MAIOR_CLASSE {
                            // O objeto grande fica livre na página dele, que a
                            // coleta completa solta.
                            Self::encadear(b, std::ptr::null_mut());
                        } else {
                            Self::encadear(b, self.livres[n]);
                            self.livres[n] = b;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.jovens = jovens;
        self.jovens.clear();
        // As faixas das TLABs: o que o código gerado alocou é jovem (morto)
        // ou já velho (a marcação da menor promove); o livre é o resto que
        // `devolver_tlabs` já pôs de volta na lista. Os mortos voltam em
        // ordem de endereço, na frente da lista (a próxima faixa sai
        // contígua).
        for (inicio, fim, n) in std::mem::take(&mut self.faixas) {
            let tamanho = tamanho_do_bloco(n);
            let (mut cabeca, mut cauda): (*mut Cabecalho, *mut Cabecalho) = (std::ptr::null_mut(), std::ptr::null_mut());
            let mut p = inicio;
            while p < fim {
                let b = p.cast::<Cabecalho>();
                // SAFETY: bloco da faixa, numa página viva.
                unsafe {
                    if (*b).estado == JOVEM {
                        mortos += 1;
                        soltos += Self::bytes_do_objeto(b, n);
                        Self::soltar_objeto(b, n);
                        if cauda.is_null() {
                            cabeca = b;
                        } else {
                            Self::encadear(cauda, b);
                        }
                        cauda = b;
                    }
                }
                p = p.wrapping_add(tamanho);
            }
            if !cauda.is_null() {
                // SAFETY: a cauda é bloco livre da classe.
                unsafe { Self::encadear(cauda, self.livres[n]) };
                self.livres[n] = cabeca;
            }
        }
        for b in std::mem::take(&mut self.lembrados) {
            // SAFETY: bloco lembrado, velho.
            unsafe { (*b).estado = VELHO };
        }
        self.vivos -= mortos;
        for d in self.demanda.iter_mut() {
            *d = 0;
        }
        (mortos, promovidos, soltos)
    }

    /// O número de campos do bloco grande `b` (a página dele sabe).
    fn pagina_do_grande(&self, b: *mut Cabecalho) -> usize {
        let base = b as usize & !(PAGINA - 1);
        self.mapa.get(base).map_or(0, |i| self.paginas[i].n)
    }

    /// Devolve o bloco `b` (de `n` campos, objeto morto) ao molde livre:
    /// solta o corpo de fora, zera campos, mapa e cabeçalho (menos o
    /// encadeamento, que fica com quem chama).
    ///
    /// # Safety
    /// `b` é bloco de `n` campos deste espaço cujo objeto morreu.
    unsafe fn soltar_objeto(b: *mut Cabecalho, n: usize) {
        // SAFETY: o contrato da função.
        unsafe {
            if (*b).flags & FORA != 0 {
                soltar_corpo_de_fora(corpo(b));
            }
            Self::formatar(b, n, std::ptr::null_mut());
        }
    }

    /// Um objeto novo de `n` campos zerados e classe `class_id`: o handle.
    #[inline]
    fn alocar(&mut self, class_id: i64, n: usize) -> i64 {
        let b = self.tirar(n);
        self.jovens.push(b);
        if n <= MAIOR_CLASSE {
            self.demanda[n] += 1;
        }
        // SAFETY: bloco livre tirado agora, formatado para `n` campos.
        unsafe {
            (*b).estado = JOVEM;
            (*b).class_id = i32::try_from(class_id).expect("id de classe além de 32 bits");
        }
        self.vivos += 1;
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    /// O bloco do handle `h`, se `h` é o de um bloco de alguma página deste
    /// espaço (vivo ou não: o estado fica com quem pergunta).
    #[inline]
    pub fn bloco_de(&self, h: i64) -> Option<*mut Cabecalho> {
        if !e_objeto(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        let base = b & !(PAGINA - 1);
        let p = &self.paginas[self.mapa.get(base)?];
        p.indice(b - base).map(|i| p.bloco(i))
    }

    /// A varredura da coleta completa: os marcados voltam a velhos; os demais
    /// morrem e voltam às listas, refeitas em ordem de endereço. A página
    /// sem vivo não é percorrida bloco a bloco: fica inteira como vazia
    /// (zerada só quando for reusada, para qualquer número de campos)
    /// enquanto os blocos livres do tamanho dela não passam dos vivos, da
    /// demanda do ciclo e do pico recente (e de duas páginas); as demais vão
    /// ao sistema. As regiões por usar das classes se desfazem (os blocos
    /// delas entram nas listas). Devolve (mortos, vivos, bytes vivos).
    fn varrer(&mut self) -> (usize, usize, usize) {
        let classes = MAIOR_CLASSE + 1;
        let (mut mortos, mut vivos, mut bytes_vivos) = (0, 0, 0);
        for r in self.regiao.iter_mut() {
            *r = (std::ptr::null_mut(), std::ptr::null_mut());
        }
        // Por página: o trecho da lista livre (cabeça, cauda) e os vivos.
        let mut trechos: Vec<(*mut Cabecalho, *mut Cabecalho, usize)> = Vec::with_capacity(self.paginas.len());
        let mut vivos_da_classe = vec![0usize; classes];
        let mut livres_da_classe = vec![0usize; classes];
        // Os blocos ocupados (vivos e mortos desta coleta) por classe: o
        // tamanho que o heap teve neste ciclo.
        let mut ocupados_da_classe = vec![0usize; classes];
        for p in &self.paginas {
            let n = p.n;
            if p.vazia {
                trechos.push((std::ptr::null_mut(), std::ptr::null_mut(), 0));
                continue;
            }
            let tamanho = tamanho_do_bloco(n);
            // Primeiro só se contam os marcados (um byte por bloco): a página
            // sem vivo sai inteira.
            let mut vivos_na_pagina = 0;
            let mut q = p.base;
            for _ in 0..p.blocos {
                // SAFETY: bloco da página.
                vivos_na_pagina += usize::from(unsafe { (*q.cast::<Cabecalho>()).estado } == MARCADO);
                q = q.wrapping_add(tamanho);
            }
            let (mut cabeca, mut cauda): (*mut Cabecalho, *mut Cabecalho) = (std::ptr::null_mut(), std::ptr::null_mut());
            if vivos_na_pagina == 0 {
                let mut q = p.base;
                for _ in 0..p.blocos {
                    let b = q.cast::<Cabecalho>();
                    // SAFETY: bloco da página; o estado diz se há objeto.
                    unsafe {
                        if (*b).estado != LIVRE {
                            mortos += 1;
                            if n <= MAIOR_CLASSE {
                                ocupados_da_classe[n] += 1;
                            }
                            if self.tem_fora && (*b).flags & FORA != 0 {
                                soltar_corpo_de_fora(corpo(b));
                            }
                        }
                    }
                    q = q.wrapping_add(tamanho);
                }
            } else {
                let mut q = p.base;
                for _ in 0..p.blocos {
                    let b = q.cast::<Cabecalho>();
                    q = q.wrapping_add(tamanho);
                    // SAFETY: bloco da página; o estado diz se há objeto.
                    unsafe {
                        match (*b).estado {
                            MARCADO => {
                                (*b).estado = VELHO;
                                bytes_vivos += Self::bytes_do_objeto(b, n);
                                continue;
                            }
                            JOVEM | VELHO | LEMBRADO => {
                                mortos += 1;
                                if n <= MAIOR_CLASSE {
                                    ocupados_da_classe[n] += 1;
                                }
                                Self::soltar_objeto(b, n);
                            }
                            // Livre: formatado, ou zerado de uma região (sem
                            // o `n`).
                            _ => (*b).n = u16::try_from(n).unwrap_or(u16::MAX),
                        }
                        if cauda.is_null() {
                            cabeca = b;
                        } else {
                            Self::encadear(cauda, b);
                        }
                        cauda = b;
                    }
                }
                // SAFETY: a cauda é bloco livre da página.
                if !cauda.is_null() {
                    unsafe { Self::encadear(cauda, std::ptr::null_mut()) };
                }
            }
            vivos += vivos_na_pagina;
            if n <= MAIOR_CLASSE {
                vivos_da_classe[n] += vivos_na_pagina;
                livres_da_classe[n] += p.blocos - vivos_na_pagina;
            }
            trechos.push((cabeca, cauda, vivos_na_pagina));
        }
        for (n, p) in self.pico.iter_mut().enumerate() {
            *p = (*p - *p / 8).max(vivos_da_classe[n] + ocupados_da_classe[n]);
        }
        // As páginas vazias que passam da folga vão ao sistema (as últimas
        // primeiro: as listas preferem os endereços baixos); as já vazias
        // de antes contam como folga de todas as classes.
        let mut solta = vec![false; self.paginas.len()];
        let mut guardadas = 0usize;
        for i in (0..self.paginas.len()).rev() {
            let p = &self.paginas[i];
            if trechos[i].2 != 0 {
                continue;
            }
            if p.vazia {
                // Vazia desde a coleta anterior e não reusada: sobra.
                if guardadas >= 2 {
                    solta[i] = true;
                } else {
                    guardadas += 1;
                }
                continue;
            }
            if p.n > MAIOR_CLASSE {
                solta[i] = true;
                continue;
            }
            // A folga: o que sobreviveu, o que se alocou desde a última
            // coleta (a demanda do próximo ciclo), o pico recente e duas
            // páginas. Soltar e pedir de novo a cada coleta custava as
            // faltas de página e a formatação.
            let folga = vivos_da_classe[p.n].max(self.demanda[p.n]).max(self.pico[p.n]).max(2 * p.blocos);
            if livres_da_classe[p.n] >= folga + p.blocos {
                livres_da_classe[p.n] -= p.blocos;
                solta[i] = true;
            }
        }
        for l in self.livres.iter_mut() {
            *l = std::ptr::null_mut();
        }
        self.vazias.clear();
        let mut caudas: Vec<*mut Cabecalho> = vec![std::ptr::null_mut(); classes];
        let paginas = std::mem::take(&mut self.paginas);
        for (i, mut p) in paginas.into_iter().enumerate() {
            if solta[i] {
                self.mapa.remover(p.base as usize);
                self.blocos -= p.blocos;
                // SAFETY: a página veio de `alloc` com este layout, e nenhum
                // bloco dela está vivo (os corpos de fora já foram soltos).
                unsafe { std::alloc::dealloc(p.base, Pagina::layout(p.bytes)) };
                continue;
            }
            let (cabeca, cauda, vivos_na_pagina) = trechos[i];
            if vivos_na_pagina == 0 {
                p.vazia = true;
                self.vazias.push(self.paginas.len());
            } else if p.n <= MAIOR_CLASSE && !cabeca.is_null() {
                if caudas[p.n].is_null() {
                    self.livres[p.n] = cabeca;
                } else {
                    // SAFETY: a cauda é um bloco livre de outra página da classe.
                    unsafe { Self::encadear(caudas[p.n], cabeca) };
                }
                caudas[p.n] = cauda;
            }
            self.mapa.inserir(p.base as usize, self.paginas.len());
            self.paginas.push(p);
        }
        // As vazias de endereço baixo saem primeiro (`pop`).
        self.vazias.reverse();
        self.vivos = vivos;
        self.jovens.clear();
        self.faixas.clear();
        self.lembrados.clear();
        for d in self.demanda.iter_mut() {
            *d = 0;
        }
        (mortos, vivos, bytes_vivos)
    }

    /// Visita o bloco de cada objeto vivo.
    fn para_cada_vivo(&mut self, mut f: impl FnMut(*mut Cabecalho)) {
        for p in self.paginas.iter().filter(|p| !p.vazia) {
            for j in 0..p.blocos {
                let b = p.bloco(j);
                // SAFETY: bloco da página.
                if unsafe { (*b).estado } != LIVRE {
                    f(b);
                }
            }
        }
    }
}

impl Drop for EspacoDeObjetos {
    fn drop(&mut self) {
        for p in &self.paginas {
            for j in 0..if self.tem_fora && !p.vazia { p.blocos } else { 0 } {
                let b = p.bloco(j);
                // SAFETY: bloco da página; os corpos de fora são do alocador
                // do sistema.
                #[allow(unsafe_code)]
                unsafe {
                    if (*b).estado != LIVRE && (*b).flags & FORA != 0 {
                        soltar_corpo_de_fora(corpo(b));
                    }
                }
            }
            // SAFETY: a página veio de `alloc` com este layout.
            #[allow(unsafe_code)]
            unsafe {
                std::alloc::dealloc(p.base, Pagina::layout(p.bytes))
            };
        }
    }
}

/// Valor gerenciado da tabela de slots; somente campos marcados como
/// referência participam do tracing. Os objetos do usuário moram fora dela,
/// no espaço de objetos ([`Cabecalho`], [`Heap::objeto`]).
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
    /// O que `Heap::get` devolve para um objeto do espaço de objetos: só
    /// o marcador (a classe e os campos vêm de [`Heap::objeto`]). Nunca
    /// mora num slot.
    Objeto,
    /// Local capturado mutável compartilhado por ambientes distintos.
    Cell(TaggedValue),
    /// Capturas ordenadas imutáveis; mutabilidade compartilhada usa Cell.
    Environment(Vec<TaggedValue>),
    /// Identidade própria, código simbólico e ambiente; não executa código
    /// Rust/Dart. O cabeçalho mora num `Box`: endereço fixo enquanto a
    /// closure vive, lido em linha pelo código gerado ([`CabecalhoDeClosure`]).
    Closure(Box<CabecalhoDeClosure>),
    /// Lista expansível de payloads tipados para tracing, sem generics Dart ainda.
    List(Elementos),
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
    TypedData { tipo: u8, class_id: i64, bytes: Armazenamento },
    /// Visão sobre uma lista tipada interna (`_Uint8ArrayView`,
    /// `_ByteDataView`, …): a classe, o tipo do elemento, a lista de base,
    /// o deslocamento em bytes, o comprimento em elementos e se é não
    /// modificável (`_UnmodifiableXArrayView`, que rejeita escrita).
    TypedView { tipo: u8, imutavel: bool, class_id: i64, base: i64, deslocamento: usize, comprimento: usize },
}
/// Os bytes de uma lista tipada interna: próprios (do heap do runtime) ou
/// externos — a memória nativa de `Pointer.asTypedList`, que o Dart só vê
/// (o `ExternalTypedData` da VM): nem copiada nem liberada pelo coletor.
///
/// Os bytes moram atrás de um cabeçalho de endereço fixo
/// ([`CabecalhoTipado`], como o [`CabecalhoDeLista`] das listas, N13): o
/// código gerado lê dele, em linha, o endereço do primeiro byte e o
/// tamanho (`dartforge_typed_cabecalho`, `lower/tipados.rs`), sem
/// `dartforge_typed_len`/`dartforge_typed_ptr` a cada acesso. O `Box` não
/// muda de endereço quando o `Value` se move, e os bytes não mudam de
/// tamanho: nenhum `&mut Vec` sai daqui, só a fatia.
#[derive(Debug)]
pub struct Armazenamento(Box<CabecalhoTipado>);

/// O cabeçalho de uma lista tipada interna; ver [`Armazenamento`].
#[repr(C)]
#[derive(Debug)]
pub struct CabecalhoTipado {
    /// O primeiro byte: o buffer de `proprio`, ou a memória externa.
    dados: *mut u8,
    /// O tamanho em bytes.
    bytes: usize,
    /// Os bytes próprios (vazio numa lista externa).
    proprio: Vec<u8>,
    /// A memória é externa (`asTypedList`).
    externo: bool,
}

const _: () = {
    assert!(std::mem::offset_of!(CabecalhoTipado, dados) == 0);
    assert!(std::mem::offset_of!(CabecalhoTipado, bytes) == 8);
};

/// O cabeçalho de quem não é lista tipada interna do tipo pedido (uma visão,
/// outra classe): tamanho 0 e nenhum endereço — o código gerado volta ao
/// caminho de antes.
pub static CABECALHO_TIPADO_VAZIO: CabecalhoTipado =
    CabecalhoTipado { dados: std::ptr::null_mut(), bytes: 0, proprio: Vec::new(), externo: false };

// SAFETY: `dados` aponta para o buffer de `proprio` (mover o cabeçalho é
// mover o `Vec`, que é `Send`/`Sync`), para a memória externa que o
// programa garante viva (o contrato de `asTypedList`, como antes, quando o
// endereço era um `usize`) ou é nulo no `CABECALHO_TIPADO_VAZIO`, que nunca
// é gravado.
#[allow(unsafe_code)]
unsafe impl Send for CabecalhoTipado {}
#[allow(unsafe_code)]
unsafe impl Sync for CabecalhoTipado {}

impl Armazenamento {
    /// Os bytes `v`, do heap do runtime.
    pub fn proprio(mut v: Vec<u8>) -> Self {
        let dados = v.as_mut_ptr();
        let bytes = v.len();
        Armazenamento(Box::new(CabecalhoTipado { dados, bytes, proprio: v, externo: false }))
    }

    /// `tamanho` bytes de memória nativa em `endereco`.
    pub fn externo(endereco: usize, tamanho: usize) -> Self {
        Armazenamento(Box::new(CabecalhoTipado {
            dados: endereco as *mut u8,
            bytes: tamanho,
            proprio: Vec::new(),
            externo: true,
        }))
    }

    /// A memória é externa (`asTypedList`).
    pub fn e_externo(&self) -> bool {
        self.0.externo
    }

    /// O endereço fixo do cabeçalho, para o código gerado.
    pub fn cabecalho(&self) -> *const CabecalhoTipado {
        &*self.0
    }

    /// Bytes que o heap do runtime ocupa: os próprios e o cabeçalho (a
    /// memória externa não conta).
    pub fn capacity(&self) -> usize {
        self.0.proprio.capacity() + std::mem::size_of::<CabecalhoTipado>()
    }
}

impl Default for Armazenamento {
    fn default() -> Self {
        Armazenamento::proprio(Vec::new())
    }
}

impl Clone for Armazenamento {
    /// Uma cópia de uma lista externa continua sobre a mesma memória nativa.
    fn clone(&self) -> Self {
        if self.0.externo {
            Armazenamento::externo(self.0.dados as usize, self.0.bytes)
        } else {
            Armazenamento::proprio(self.0.proprio.clone())
        }
    }
}

impl From<Vec<u8>> for Armazenamento {
    fn from(v: Vec<u8>) -> Self {
        Armazenamento::proprio(v)
    }
}

// A única leitura de memória nativa do heap: a de uma lista externa.
#[allow(unsafe_code)]
impl std::ops::Deref for Armazenamento {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        if !self.0.externo {
            return &self.0.proprio;
        }
        // SAFETY: a memória nativa de `asTypedList`, que o programa garante
        // viva e com `bytes` bytes enquanto usa a lista (o mesmo contrato da
        // VM).
        unsafe { std::slice::from_raw_parts(self.0.dados, self.0.bytes) }
    }
}

#[allow(unsafe_code)]
impl std::ops::DerefMut for Armazenamento {
    fn deref_mut(&mut self) -> &mut [u8] {
        if !self.0.externo {
            return &mut self.0.proprio;
        }
        // SAFETY: como em `deref`.
        unsafe { std::slice::from_raw_parts_mut(self.0.dados, self.0.bytes) }
    }
}

impl Value {
    /// Estima armazenamento próprio usando capacidades efetivas, com overflow explícito.
    fn estimated_bytes(&self) -> usize {
        let payload = match self {
            Self::String(text) | Self::RegExp(text) | Self::Match(text) => text.capacidade_bytes(),
            Self::StringBuffer(v) => v.capacity().checked_mul(2).expect("payload excede usize"),
            Self::Objeto | Self::Cell(_) | Self::BoxedInt(_) | Self::BoxedDouble(_) | Self::BoxedBool(_) => 0,
            Self::Closure(_) => std::mem::size_of::<CabecalhoDeClosure>(),
            Self::TypedData { bytes, .. } => bytes.capacity(),
            Self::TypedView { .. } => 0,
            Self::List(values) => values
                .bytes_reservados()
                .checked_add(std::mem::size_of::<CabecalhoDeLista>())
                .expect("payload excede usize"),
            Self::Environment(values) | Self::Set(values) | Self::Record(values) => values
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
    /// Empilha as arestas e devolve quantas posições percorreu: o trabalho
    /// da marcação neste objeto, que não depende de quantas são referências
    /// (uma lista de um milhão de `int` custa um milhão de passos).
    fn trace(&self, pending: &mut Vec<i64>) -> usize {
        match self {
            Self::String(_)
            | Self::StringBuffer(_)
            | Self::RegExp(_)
            | Self::Match(_)
            | Self::BoxedInt(_)
            | Self::BoxedDouble(_)
            | Self::BoxedBool(_)
            | Self::TypedData { .. }
            | Self::Objeto => 0,
            Self::TypedView { base, .. } => {
                pending.push(*base);
                1
            }
            Self::Cell(value) => {
                if value.is_ref {
                    pending.push(value.bits);
                }
                1
            }
            // Uma lista compacta (N14) não tem referências: nada a percorrer.
            Self::List(values) if values.forma() != FormaDeLista::Geral => 1,
            Self::List(values) => {
                pending.extend(values.referencias());
                values.len()
            }
            Self::Environment(values) | Self::Set(values) | Self::Record(values) => {
                pending.extend(
                    values
                        .iter()
                        .filter_map(|value| value.is_ref.then_some(value.bits)),
                );
                values.len()
            }
            Self::Map(entries) => {
                pending.extend(entries.iter().flat_map(|(key, value)| {
                    [key, value]
                        .into_iter()
                        .filter_map(|part| part.is_ref.then_some(part.bits))
                }));
                entries.len() * 2
            }
            Self::Closure(c) => {
                pending.push(c.environment);
                1
            }
        }
    }
}

/// O handle `h` foi marcado pela coleta em curso (vivo)? Null, `Smi` e
/// escalar qualquer: não.
///
/// Na coleta menor, o velho conta como vivo.
fn marcado_na_coleta(marks: &[bool], idade: &[u8], objetos: &EspacoDeObjetos, menor: bool, h: i64) -> bool {
    if !smi::e_handle(h) || h < 0 {
        return false;
    }
    if e_objeto(h) {
        // SAFETY: bloco de uma página do espaço.
        #[allow(unsafe_code)]
        return objetos.bloco_de(h).is_some_and(|b| {
            let e = unsafe { (*b).estado };
            e == MARCADO || (menor && (e == VELHO || e == LEMBRADO))
        });
    }
    let i = Heap::indice_de(h);
    marks.get(i).copied().unwrap_or(false) || (menor && idade.get(i).is_some_and(|&x| x != 0))
}

/// O que [`Heap::get`] devolve para um objeto do espaço.
struct Marcador(Value);
// SAFETY: `Value::Objeto` não tem dados.
#[allow(unsafe_code)]
unsafe impl Sync for Marcador {}
static VALOR_OBJETO: Marcador = Marcador(Value::Objeto);

/// O `Ref` é o handle de um objeto do espaço de objetos ([`Bloco`]): o bit
/// 1 ligado (os da tabela de slots são múltiplos de 4; o `Smi`, ímpar).
/// Negativo nunca é objeto (é escalar usado como handle, N4).
#[inline]
pub fn e_objeto(r: i64) -> bool {
    r & (3 | i64::MIN) == 2
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

/// O contexto da thread que o código gerado lê e grava direto, sem
/// chamada (`llvm/mod.rs`): a exceção pendente (espelho de
/// `excecoes::EXCEPTION`, conferido depois de cada chamada) e o topo da
/// pilha-sombra desta thread (do isolado dela). O endereço vem de
/// `dartforge_contexto`, uma vez por ativação; não muda enquanto a
/// thread vive. Os deslocamentos são contrato com o emissor.
#[repr(C)]
pub struct Contexto {
    /// 1 com exceção pendente (deslocamento 0).
    pub pendente: std::cell::Cell<u8>,
    /// O quadro do topo da pilha-sombra (deslocamento 8).
    pub topo: std::cell::Cell<*const QuadroDeRaizes>,
    /// A área de globais de cada módulo neste isolado, pelo id do módulo
    /// (`@df.area_id`; 0 = sem id): o caminho rápido do prólogo das
    /// funções (`@df.obter_area`). Endereço dos elementos (deslocamento 16)
    /// e comprimento (24); uma entrada nula vai ao runtime.
    pub areas: std::cell::Cell<*const *mut i64>,
    pub n_areas: std::cell::Cell<usize>,
    /// O pedido de interrupção do isolado (deslocamento 32): a porta de
    /// controle o liga, de outra thread, quando chega uma mensagem de controle
    /// (`portas.rs`). O código gerado o lê, atômico, no começo de cada volta
    /// de laço (o ponto seguro da J01).
    pub interrupcao: std::sync::atomic::AtomicU8,
    /// O cabeçalho de [`OBJETO_VAZIO`] (deslocamento 40): a leitura em
    /// linha de um campo de algo que não é objeto do espaço usa este objeto
    /// no lugar do cabeçalho em `h - 2` (`llvm/mod.rs`), e lê zeros, como
    /// `dartforge_object_get` dava.
    pub vazios: *const Cabecalho,
    /// As classes com tabela de métodos registrada (deslocamento 48; um
    /// byte 0/1 por id, `seletores.rs`) e quantas (56): a alocação em linha
    /// de uma classe que registra a tabela na primeira alocação
    /// (`dartforge_object_new_t`) só pula o runtime depois do registro.
    pub registradas: std::cell::Cell<*const u8>,
    pub n_registradas: std::cell::Cell<usize>,
    /// A TLAB do isolado (deslocamento 64): para cada número de campos
    /// `n ≤ TLAB_N`, uma faixa `[cursor, fim)` de blocos livres contíguos do
    /// espaço de objetos já contados como alocação
    /// (`Heap::reabastecer_tlab`): o cursor em `64 + 16n`, o fim em
    /// `72 + 16n`. O código gerado aloca avançando o cursor um bloco (o
    /// *bump pointer* da VM, `llvm/mod.rs`); faixa esgotada vai ao runtime,
    /// que reabastece.
    pub tlab: [[std::cell::Cell<*mut u8>; 2]; TLAB_N + 1],
}

const _: () = {
    assert!(std::mem::offset_of!(Contexto, pendente) == 0);
    assert!(std::mem::offset_of!(Contexto, topo) == 8);
    assert!(std::mem::offset_of!(Contexto, areas) == 16);
    assert!(std::mem::offset_of!(Contexto, n_areas) == 24);
    assert!(std::mem::offset_of!(Contexto, interrupcao) == 32);
    assert!(std::mem::offset_of!(Contexto, vazios) == 40);
    assert!(std::mem::offset_of!(Contexto, registradas) == 48);
    assert!(std::mem::offset_of!(Contexto, n_registradas) == 56);
    assert!(std::mem::offset_of!(Contexto, tlab) == 64);
};

thread_local! {
    pub static CONTEXTO: Contexto = const {
        Contexto {
            pendente: std::cell::Cell::new(0),
            topo: std::cell::Cell::new(std::ptr::null()),
            areas: std::cell::Cell::new(std::ptr::null()),
            n_areas: std::cell::Cell::new(0),
            interrupcao: std::sync::atomic::AtomicU8::new(0),
            vazios: &raw const OBJETO_VAZIO.cabecalho,
            registradas: std::cell::Cell::new(std::ptr::null()),
            n_registradas: std::cell::Cell::new(0),
            tlab: [const { [std::cell::Cell::new(std::ptr::null_mut()), std::cell::Cell::new(std::ptr::null_mut())] }; TLAB_N + 1],
        }
    };
}


/// Encadeia `q` no topo da pilha-sombra. `q.n` e os slots (zerados) já
/// foram escritos pelo código gerado.
///
/// # Safety
/// `q` aponta um quadro válido que vive até o [`desempilhar_quadro`]
/// correspondente.
#[allow(unsafe_code)]
pub unsafe fn empilhar_quadro(q: *mut QuadroDeRaizes) {
    CONTEXTO.with(|c| {
        // SAFETY: `q` é o quadro no stack da função que chama, vivo até o
        // `desempilhar_quadro` antes de cada retorno dela.
        unsafe { (*q).anterior = c.topo.get() };
        c.topo.set(q);
    });
}

/// Desencadeia `q`, que tem de ser o topo (os retornos fecham os quadros
/// em ordem, G3).
///
/// # Safety
/// `q` é o quadro válido passado ao último [`empilhar_quadro`].
#[allow(unsafe_code)]
pub unsafe fn desempilhar_quadro(q: *const QuadroDeRaizes) {
    CONTEXTO.with(|c| {
        assert!(std::ptr::eq(c.topo.get(), q), "bug do compilador: quadro de raízes fechado fora de ordem");
        // SAFETY: `q` é o topo, ainda no stack de quem chama.
        c.topo.set(unsafe { (*q).anterior });
    });
}

/// Visita as raízes de todos os quadros da pilha-sombra desta thread.
#[allow(unsafe_code)]
fn visitar_quadros(mut f: impl FnMut(i64)) {
    let mut q = CONTEXTO.with(|c| c.topo.get());
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
    stress: bool,
    stats: HeapStats,
    marks: Vec<bool>,
    /// Posições percorridas pela última marcação (`Value::trace`).
    trabalho_da_marcacao: usize,
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
    /// A época de layout (`EPOCA_DE_LAYOUT`) dos objetos deste heap: a de
    /// quando ele nasceu, e a de cada migração aplicada depois (J03).
    pub epoca_de_layout: u64,
    /// Listas de tamanho fixo (`_List` do SDK da fonte, P5c).
    pub fixas: crate::hash::HashSet<i64>,
    /// `_GrowableList` criada por `_withData(data)` (P5c): o vetor tem os
    /// elementos de `data` (a reserva) e o tamanho lógico ainda é este, até
    /// o primeiro `_setLength`/`_setData` — na VM a lista aponta para o
    /// `_List` e o tamanho é outro campo.
    /// Onde moram os objetos do usuário ([`EspacoDeObjetos`]); os slots
    /// guardam os demais valores do runtime.
    objetos: EspacoDeObjetos,
    /// O heap do isolado da thread (não um de teste): reabastece a TLAB do
    /// [`Contexto`] dela.
    publica: bool,
    /// O que `get_mut` devolve para um objeto do espaço (os campos se
    /// gravam por [`Heap::definir_campo`]).
    marcador: Value,
    /// A idade de cada slot: 0 jovem, 1 velho, 2 velho lembrado pela
    /// barreira de escrita (`get_mut`), como os estados dos blocos.
    idade: Vec<u8>,
    /// Os slots ocupados desde a última coleta (o que a coleta menor varre)
    /// e os velhos lembrados.
    slots_jovens: Vec<usize>,
    slots_lembrados: Vec<usize>,
    /// Bytes alocados desde a última coleta (o gatilho da coleta menor).
    bytes_jovens: usize,
    /// O `hashCode` de cada slot que é `String`, calculado na primeira
    /// consulta (0 = ainda não): a `String` é imutável, e a VM também guarda
    /// o hash no cabeçalho dela. Zerado quando o slot é reusado ou entregue
    /// para escrita (`get_mut`).
    hashes_de_texto: Vec<std::cell::Cell<u32>>,
    /// A coleta em curso é menor (só os jovens).
    coleta_menor: bool,
    /// `DARTFORGE_GC_VERIFICAR=1`: toda coleta menor confere, por uma
    /// travessia completa, que nenhum jovem alcançável ficou sem marca (uma
    /// barreira de escrita faltando).
    verificar: bool,
    /// `DARTFORGE_GC_RASTRO=1`: uma linha por coleta no stderr (o tipo, os
    /// marcados, a estimativa e o gatilho).
    rastrear: bool,
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
            stress,
            stats: HeapStats::default(),
            marks: Vec::new(),
            trabalho_da_marcacao: 0,
            pending: Vec::new(),
            byte_threshold: 2 * LIMITE_JOVEM,
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
            epoca_de_layout: EPOCA_DE_LAYOUT.load(std::sync::atomic::Ordering::Acquire),
            fixas: crate::hash::HashSet::default(),
            objetos: EspacoDeObjetos::new(),
            publica: false,
            marcador: Value::Objeto,
            idade: Vec::new(),
            slots_jovens: Vec::new(),
            slots_lembrados: Vec::new(),
            bytes_jovens: 0,
            hashes_de_texto: Vec::new(),
            coleta_menor: false,
            verificar: std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1"),
            rastrear: std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1"),
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
        let object = self.novo_objeto(class_id, &[(index, false), (text, true)]);
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
    ///
    /// Os valores do runtime vão a um slot; os objetos do usuário, ao espaço
    /// de objetos ([`Heap::novo_objeto`], [`Heap::alocar_objeto`]).
    ///
    /// # Panics
    /// Com [`Value::Objeto`], que não é valor de slot.
    pub fn allocate(&mut self, value: Value) -> i64 {
        assert!(!matches!(value, Value::Objeto), "objeto do espaço alocado como slot");
        let bytes = value.estimated_bytes();
        self.antes_de_alocar(bytes);
        self.guardar(value)
    }
    /// Coleta se algum gatilho pede, confere o teto e conta a alocação de
    /// `bytes`.
    fn antes_de_alocar(&mut self, bytes: usize) {
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
        if !self.gc_desligado && self.precisa_coletar(bytes) {
            self.coletar_automatico(bytes);
        }
        // Depois da coleta: se ainda passa do teto, nao ha o que recuperar.
        self.verificar_teto(bytes);
        self.contar_alocacao(1, bytes);
    }
    /// Um objeto do usuário novo com os campos `campos` (os que o runtime
    /// monta à mão: erros, valores de enum, a classe `Type`…). Coleta antes
    /// como [`Heap::allocate`]: as referências em `campos` precisam de raiz
    /// de quem chama.
    pub fn novo_objeto(&mut self, class_id: i64, campos: &[Campo]) -> i64 {
        let h = self.alocar_objeto(class_id, campos.len());
        for (i, &(bits, e_ref)) in campos.iter().enumerate() {
            self.definir_campo(h, i, bits, e_ref);
        }
        h
    }
    /// Conta `k` alocações de `bytes` no total para os gatilhos e os
    /// contadores.
    #[inline]
    fn contar_alocacao(&mut self, k: usize, bytes: usize) {
        self.stats.estimated_bytes = self.stats.estimated_bytes.checked_add(bytes).expect("heap excede usize");
        self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
        self.allocations += k;
        self.stats.allocations += k as u64;
        self.bytes_jovens += bytes;
    }
    /// Põe `value` num slot (um livre, ou um novo no fim da tabela);
    /// devolve o handle.
    #[inline]
    fn guardar(&mut self, value: Value) -> i64 {
        let index = if let Some(index) = self.free.pop() {
            self.slots[index] = Some(value);
            self.metadados[index] = 0;
            self.idade[index] = 0;
            self.hashes_de_texto[index].set(0);
            index
        } else {
            self.slots.push(Some(value));
            self.metadados.push(0);
            self.idade.push(0);
            self.hashes_de_texto.push(std::cell::Cell::new(0));
            self.slots.len() - 1
        };
        self.slots_jovens.push(index);
        Self::handle_de_indice(index)
    }
    /// O heap do isolado da thread: reabastece a TLAB do [`Contexto`] (a
    /// alocação em linha do código gerado).
    pub fn do_isolado(stress: bool) -> Self {
        let mut h = Self::new(stress);
        h.publica = true;
        h
    }
    /// Aloca um objeto do usuário de `n` campos zerados (o caminho do
    /// `dartforge_object_new`), no espaço de objetos.
    ///
    /// O caminho rápido confere os gatilhos de [`Heap::allocate`] numa
    /// comparação cada; se algum dispara (coleta, teto, `--gc-stress`), vai
    /// ao caminho geral.
    #[inline]
    pub fn alocar_objeto(&mut self, class_id: i64, n: usize) -> i64 {
        let bytes = tamanho_do_bloco(n);
        if self.precisa_coletar(bytes) {
            return self.alocar_objeto_lento(class_id, n);
        }
        self.contar_alocacao(1, bytes);
        self.objetos.alocar(class_id, n)
    }
    /// Uma alocação de `bytes` agora passaria de algum gatilho de coleta?
    /// A menor vem a cada [`LIMITE_JOVEM`] bytes (ou [`CONTAGEM_JOVEM`]
    /// alocações) desde a última coleta; a completa, quando o total passa do
    /// gatilho que a última completa calculou (`byte_threshold`) ou do teto.
    #[inline]
    fn precisa_coletar(&self, bytes: usize) -> bool {
        self.stress
            || self.allocations >= CONTAGEM_JOVEM
            || self.bytes_jovens + bytes > LIMITE_JOVEM
            || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.limite_bytes != usize::MAX && self.bytes_totais(bytes) > self.limite_bytes)
    }
    /// A coleta que os gatilhos pedem antes de alocar `bytes`: a completa se
    /// o total passou do gatilho dela (ou do teto, ou a cada
    /// [`MENORES_POR_COMPLETA_NO_ESTRESSE`] no `--gc-stress`); a menor senão.
    fn coletar_automatico(&mut self, bytes: usize) {
        let completa = self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.limite_bytes != usize::MAX && self.bytes_totais(bytes) > self.limite_bytes)
            || (self.stress && self.stats.collections.is_multiple_of(MENORES_POR_COMPLETA_NO_ESTRESSE));
        self.coletar(!completa);
    }
    #[cold]
    #[inline(never)]
    fn alocar_objeto_lento(&mut self, class_id: i64, n: usize) -> i64 {
        self.antes_de_alocar(tamanho_do_bloco(n));
        self.objetos.alocar(class_id, n)
    }
    /// Reabastece a TLAB de `n` campos do [`Contexto`] (se esgotada) com
    /// uma faixa contígua de até [`TLAB_BLOCOS`] blocos livres, contados
    /// como alocados agora — sem coletar: só o que cabe antes do próximo
    /// gatilho (o que passa dele fica para o runtime, que coleta). Sem TLAB
    /// no `--gc-stress` (toda alocação passa pelo runtime, que coleta antes)
    /// e nos heaps de teste.
    pub fn reabastecer_tlab(&mut self, n: usize) {
        if !self.publica || self.stress || n > TLAB_N {
            return;
        }
        let tamanho = tamanho_do_bloco(n);
        if CONTEXTO.with(|c| (c.tlab[n][1].get() as usize).saturating_sub(c.tlab[n][0].get() as usize) >= tamanho) {
            return;
        }
        let mut k = TLAB_BLOCOS
            .min(CONTAGEM_JOVEM.saturating_sub(self.allocations))
            .min(LIMITE_JOVEM.saturating_sub(self.bytes_jovens) / tamanho)
            .min(self.byte_threshold.saturating_sub(self.stats.estimated_bytes) / tamanho);
        if self.limite_bytes != usize::MAX {
            k = k.min(self.limite_bytes.saturating_sub(self.bytes_totais(0)) / tamanho);
        }
        if k == 0 {
            return;
        }
        let (inicio, k) = self.objetos.tirar_faixa(n, k);
        self.objetos.vivos += k;
        self.contar_alocacao(k, k * tamanho);
        CONTEXTO.with(|c| {
            c.tlab[n][0].set(inicio);
            c.tlab[n][1].set(inicio.wrapping_add(k * tamanho));
        });
    }
    /// Devolve as TLABs do [`Contexto`] (o que o código gerado não usou) antes
    /// de uma coleta: o resto de cada faixa volta à frente da lista livre
    /// e sai da contagem de alocações.
    fn devolver_tlabs(&mut self) {
        if !self.publica {
            return;
        }
        for n in 0..=TLAB_N {
            let (cursor, fim) = CONTEXTO.with(|c| (c.tlab[n][0].replace(std::ptr::null_mut()), c.tlab[n][1].replace(std::ptr::null_mut())));
            let tamanho = tamanho_do_bloco(n);
            let k = (fim as usize).saturating_sub(cursor as usize) / tamanho;
            if k > 0 {
                self.objetos.devolver_faixa(n, cursor, fim);
                let bytes = k * tamanho;
                self.objetos.vivos -= k;
                self.allocations = self.allocations.saturating_sub(k);
                self.stats.allocations = self.stats.allocations.saturating_sub(k as u64);
                self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(bytes);
                self.bytes_jovens = self.bytes_jovens.saturating_sub(bytes);
            }
        }
    }
    /// O bloco vivo do objeto `h` (`h & 3 == 2`), ou `panic` como
    /// [`Heap::indice_vivo`] (N4).
    #[inline]
    fn bloco_vivo(&self, handle: i64) -> *mut Cabecalho {
        match self.objetos.bloco_de(handle) {
            // SAFETY: bloco de uma página do espaço.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).estado } != 0 => b,
            Some(_) => panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}"),
            None => panic!("bug do compilador: handle além da tabela (escalar usado como handle)\nhandle {handle}"),
        }
    }
    /// O `hashCode` de identidade do objeto `h` do espaço: tirado do
    /// endereço, que não muda enquanto o objeto vive (o coletor não move) e
    /// é único entre os vivos — 30 bits espalhados, nunca 0 (a VM sorteia e
    /// guarda no cabeçalho; aqui o cabeçalho não tem lugar e não precisa).
    /// `None` se `h` não é objeto vivo do espaço.
    pub fn hash_de_identidade(&mut self, h: i64) -> Option<i64> {
        let b = self.objetos.bloco_de(h)?;
        // SAFETY: bloco de uma página do espaço.
        #[allow(unsafe_code)]
        if unsafe { (*b).estado } == LIVRE {
            return None;
        }
        let x = ((b as u64 >> 3).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 34) as i64;
        Some(x.max(1))
    }
    /// A vista do objeto `h` do espaço (classe e campos), ou `None` se `h`
    /// não é objeto vivo do espaço (null, `Smi`, valor do runtime, escalar).
    pub fn objeto(&self, h: i64) -> Option<Obj<'_>> {
        let b = self.objetos.bloco_de(h)?;
        // SAFETY: bloco de uma página do espaço, conferido vivo.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return None;
            }
            Some(Obj { corpo: corpo(b), class_id: i64::from((*b).class_id), _heap: std::marker::PhantomData })
        }
    }
    /// A classe do objeto `h` do espaço (`None` se não é objeto vivo).
    #[inline]
    pub fn classe_do_objeto(&self, h: i64) -> Option<i64> {
        self.objeto(h).map(|o| o.class_id)
    }
    /// Grava o campo `i` do objeto `h` (com a marca de referência e a
    /// barreira de escrita).
    ///
    /// # Panics
    /// Se `h` não é objeto vivo ou `i` está fora dele.
    pub fn definir_campo(&mut self, h: i64, i: usize, bits: i64, e_ref: bool) {
        if e_ref && smi::e_handle(bits) {
            self.conferir_vivo(bits);
        }
        let b = self.bloco_vivo(h);
        self.objetos.lembrar(b);
        // SAFETY: bloco vivo; `i` conferido contra o número de campos.
        #[allow(unsafe_code)]
        unsafe {
            let c = corpo(b);
            assert!(i < usize::from((*c).n), "campo {i} fora do objeto de {} campos", (*c).n);
            *campos_de(c).add(i) = bits;
            marcar_referencia(c, i, e_ref);
        }
    }
    /// Pânico (N4) se `h` não é handle vivo.
    fn conferir_vivo(&self, h: i64) {
        if e_objeto(h) {
            self.bloco_vivo(h);
        } else {
            self.indice_vivo(h);
        }
    }
    /// A barreira de escrita do código gerado (`dartforge_lembrar`): o
    /// objeto `h`, velho, recebeu um `Ref` num campo.
    pub fn lembrar_objeto(&mut self, h: i64) {
        if let Some(b) = self.objetos.bloco_de(h) {
            self.objetos.lembrar(b);
        }
    }
    /// O `hashCode` da `String` `h` ([`Texto::hash_vm`]), calculado uma vez
    /// por valor; `None` se `h` não é `String` viva.
    pub fn hash_de_texto(&self, h: i64) -> Option<i64> {
        if !smi::e_handle(h) || h < 0 || e_objeto(h) {
            return None;
        }
        let i = Self::indice_de(h);
        let Some(Some(Value::String(t))) = self.slots.get(i) else { return None };
        let cache = &self.hashes_de_texto[i];
        if cache.get() != 0 {
            return Some(i64::from(cache.get()));
        }
        let x = t.hash_vm();
        cache.set(x as u32);
        Some(x)
    }
    /// O metadado de um handle vivo (0 = nenhum).
    pub fn metadado(&self, handle: i64) -> i64 {
        if e_objeto(handle) {
            // SAFETY: bloco vivo.
            #[allow(unsafe_code)]
            return i64::from(unsafe { (*self.bloco_vivo(handle)).metadado });
        }
        let i = self.indice_vivo(handle);
        self.metadados[i]
    }
    /// O metadado e o valor de um handle vivo, numa consulta só.
    pub fn metadado_e_valor(&self, handle: i64) -> (i64, &Value) {
        if e_objeto(handle) {
            let b = self.bloco_vivo(handle);
            // SAFETY: bloco vivo.
            #[allow(unsafe_code)]
            return (i64::from(unsafe { (*b).metadado }), &VALOR_OBJETO.0);
        }
        let i = self.indice_vivo(handle);
        (self.metadados[i], self.slots[i].as_ref().expect("slot vivo verificado"))
    }
    /// Grava o metadado de um handle vivo.
    pub fn set_metadado(&mut self, handle: i64, valor: i64) {
        if e_objeto(handle) {
            // SAFETY: bloco vivo.
            #[allow(unsafe_code)]
            unsafe {
                (*self.bloco_vivo(handle)).metadado = u32::try_from(valor).expect("metadado além de 32 bits")
            };
            return;
        }
        let i = self.indice_vivo(handle);
        self.metadados[i] = valor;
        // O `E` de uma lista mudou: as gravações diretas se conferem de novo.
        if let Some(Value::List(e)) = self.slots[i].as_mut() {
            e.esquecer_gravacoes();
        }
    }
    /// Marca `handle` como não modificável; uma lista esquece as gravações
    /// diretas conferidas (`CabecalhoDeLista::gravavel`).
    pub fn marcar_imutavel(&mut self, handle: i64) {
        self.imutaveis.insert(handle);
        if e_objeto(handle) {
            return;
        }
        if let Some(Value::List(e)) = smi::e_handle(handle)
            .then(|| self.slots.get_mut(Self::indice_de(handle)))
            .flatten()
            .and_then(Option::as_mut)
        {
            e.esquecer_gravacoes();
        }
    }
    /// O handle (par) do slot `index` (R10).
    ///
    /// Múltiplo de 4: o bit 1 ligado é o dos objetos do espaço ([`Cabecalho`]).
    fn handle_de_indice(index: usize) -> i64 {
        i64::try_from(index + 1)
            .ok()
            .and_then(|h| h.checked_shl(2).filter(|x| x >> 2 == h))
            .expect("handles esgotados")
    }
    /// O slot de um handle da tabela, sem verificar se está vivo.
    pub fn indice_de(handle: i64) -> usize {
        usize::try_from((handle >> 2) - 1).expect("handle positivo")
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
        debug_assert!(!e_objeto(handle), "objeto do espaço no caminho da tabela");
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
    /// Migra os objetos das classes de `plano` para o layout novo (J03):
    /// cada posição nova recebe o valor da antiga indicada (`-1`: zero, que
    /// é `null` para o campo anulável e "não inicializado" para o `late`), e
    /// as marcas de `late` inicializado seguem o campo.
    pub fn migrar_instancias(&mut self, plano: &crate::hash::HashMap<i64, Vec<i64>>) {
        let mut blocos = Vec::new();
        self.objetos.para_cada_vivo(|b| blocos.push(b));
        for b in blocos {
            // SAFETY: bloco vivo do espaço.
            #[allow(unsafe_code)]
            let class_id = i64::from(unsafe { (*b).class_id });
            let Some(origem) = plano.get(&class_id) else { continue };
            let h = b as i64 + DESLOCAMENTO_DO_HANDLE;
            let antigos = self.objeto(h).expect("objeto vivo").to_vec();
            let valores: Vec<Campo> = origem
                .iter()
                .map(|&o| usize::try_from(o).ok().and_then(|o| antigos.get(o).copied()).unwrap_or((0, false)))
                .collect();
            self.trocar_campos(b, &valores);
        }
        let marcas: Vec<(i64, i64)> = self.campos_late_inicializados.iter().copied().filter(|&(_, i)| i >= 0).collect();
        for (h, i) in marcas {
            let Some(class_id) = self.classe_do_objeto(h) else { continue };
            let Some(origem) = plano.get(&class_id) else { continue };
            let nova = origem.iter().position(|&o| o == i);
            self.campos_late_inicializados.remove(&(h, i));
            if let Some(j) = nova {
                self.campos_late_inicializados.insert((h, j as i64));
            }
        }
    }

    /// Troca os campos do objeto do bloco `b` por `valores`: no próprio bloco
    /// se o número de campos é o dele, senão num corpo de fora (o objeto não
    /// muda de endereço; o código gerado segue o corpo, [`FORA`]).
    fn trocar_campos(&mut self, b: *mut Cabecalho, valores: &[Campo]) {
        self.objetos.lembrar(b);
        // SAFETY: bloco vivo do espaço; o corpo de fora é novo e do tamanho
        // de `valores`.
        #[allow(unsafe_code)]
        unsafe {
            let atual = corpo(b);
            let c = if usize::from((*atual).n) == valores.len() {
                atual
            } else {
                let c = novo_corpo_de_fora(valores.len());
                self.objetos.tem_fora = true;
                (*c).class_id = (*b).class_id;
                if (*b).flags & FORA != 0 {
                    soltar_corpo_de_fora(atual);
                }
                *campos_de(b).cast::<*mut Cabecalho>() = c;
                (*b).flags |= FORA;
                c
            };
            for (i, &(bits, e_ref)) in valores.iter().enumerate() {
                *campos_de(c).add(i) = bits;
                marcar_referencia(c, i, e_ref);
            }
        }
    }

    /// Obtém valor vivo; o protocolo ABI não permite handles obsoletos. De
    /// um objeto do espaço, o marcador [`Value::Objeto`] (os dados vêm de
    /// [`Heap::objeto`]).
    pub fn get(&self, handle: i64) -> &Value {
        if e_objeto(handle) {
            self.bloco_vivo(handle);
            return &VALOR_OBJETO.0;
        }
        let index = self.indice_vivo(handle);
        self.slots[index].as_ref().expect("slot vivo verificado")
    }
    /// Garante ao objeto `handle` pelo menos `n` campos (os novos,
    /// zerados): num corpo de fora ([`Heap::trocar_campos`]).
    pub fn garantir_campos(&mut self, handle: i64, n: usize) {
        let Some(o) = self.objeto(handle) else { return };
        if o.len() >= n {
            return;
        }
        let mut novos = o.to_vec();
        novos.resize(n, (0, false));
        let b = self.bloco_vivo(handle);
        self.trocar_campos(b, &novos);
    }
    /// O endereço dos campos do objeto `handle` (palavras de 8 bytes; o
    /// caminho de `dartforge_object_campos`); `None` se não é objeto vivo.
    /// Quem pede o ponteiro pode gravar por ele: conta como barreira de
    /// escrita, mas o mapa de referências fica com quem grava.
    #[inline]
    pub fn campos_de_objeto(&mut self, handle: i64) -> Option<*mut i64> {
        let b = self.objetos.bloco_de(handle)?;
        // SAFETY: bloco do espaço.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return None;
            }
            self.objetos.lembrar(b);
            Some(campos_de(corpo(b)))
        }
    }
    /// Obtém valor vivo se o handle for válido, ou None se inválido/destruído.
    /// Null, `Smi` e escalar qualquer dão `None`; um objeto vivo do espaço,
    /// o marcador [`Value::Objeto`].
    pub fn try_get(&self, handle: i64) -> Option<&Value> {
        if !smi::e_handle(handle) || handle < 0 {
            return None;
        }
        if e_objeto(handle) {
            return self.objeto(handle).map(|_| &VALOR_OBJETO.0);
        }
        self.slots.get(Self::indice_de(handle)).and_then(Option::as_ref)
    }
    /// Atualiza campo com tag explícita; valores escalares jamais são raízes.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        self.definir_campo(handle, usize::try_from(index).expect("índice inválido"), bits, is_ref);
    }
    /// Aloca protegendo as referências do payload contra a coleta anterior à alocação.
    fn allocate_linked(&mut self, value: Value) -> i64 {
        // Sem coleta à vista, nada a proteger: o caminho enraizado (rastrear
        // o valor, abrir um quadro, conferir cada referência) só quando a
        // alocação vai coletar.
        if self.gc_desligado || !self.precisa_coletar(value.estimated_bytes()) {
            return self.allocate(value);
        }
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
        self.allocate_linked(Value::Closure(Box::new(CabecalhoDeClosure { code_id, environment, tipado: 0, abi: 0 })))
    }
    /// Retorna código simbólico e ambiente, preservando a identidade do handle.
    pub fn closure_parts(&self, handle: i64) -> (i64, i64) {
        match self.get(handle) {
            Value::Closure(c) => (c.code_id, c.environment),
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
            Value::List(values) => values.valor(index),
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
        values.definir(index, value);
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
    ///
    /// É a barreira de escrita do runtime: um velho entregue para escrita
    /// é lembrado (o que se gravar nele pode ser jovem), sem olhar o que
    /// vai ser gravado — conservador, como toda mutação do runtime passa
    /// por aqui.
    pub fn get_mut(&mut self, handle: i64) -> &mut Value {
        if e_objeto(handle) {
            let b = self.bloco_vivo(handle);
            self.objetos.lembrar(b);
            // Os campos de um objeto se gravam por `definir_campo`; aqui só
            // o marcador (gravar nele não muda nada).
            self.marcador = Value::Objeto;
            return &mut self.marcador;
        }
        let slot = self.indice_vivo(handle);
        self.hashes_de_texto[slot].set(0);
        if self.idade[slot] == 1 {
            self.idade[slot] = 2;
            self.slots_lembrados.push(slot);
        }
        self.slots[slot].as_mut().expect("slot vivo verificado")
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    /// Marca tudo o que é alcançável a partir de `pending`; devolve quantos
    /// objetos marcou.
    fn marcar_pendentes(&mut self) -> usize {
        // Uma cópia da marcação para cada tipo de coleta: o teste do estado
        // de cada objeto (jovem na menor, não marcado na completa) sai sem
        // desvio pelo tipo.
        if self.coleta_menor { self.marcar::<true>() } else { self.marcar::<false>() }
    }

    fn marcar<const MENOR: bool>(&mut self) -> usize {
        let menor = MENOR;
        let mut live = 0_usize;
        while let Some(handle) = self.pending.pop() {
            // null e `Smi` (R10) não são arestas: o coletor nunca segue um
            // `Smi`, que não aponta para o heap.
            if !smi::e_handle(handle) {
                continue;
            }
            if e_objeto(handle) {
                // As arestas vêm de raízes e de campos `is_ref`: a validação
                // completa do handle (a página e o início do bloco) fica para
                // o `--gc-stress`, o modo que caça raiz faltando (N4).
                let validar = self.stress;
                let mut atual = handle;
                // Segue direto o primeiro filho ainda por marcar (a lista
                // ligada, o ramo de uma árvore) sem passar pela pilha.
                while atual != 0 {
                    let b = if validar {
                        self.bloco_vivo(atual)
                    } else {
                        debug_assert!(self.objetos.bloco_de(atual).is_some(), "aresta inválida {atual}");
                        (atual - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho
                    };
                    let mut proximo = 0;
                    // SAFETY: bloco vivo do espaço.
                    #[allow(unsafe_code)]
                    unsafe {
                        // Na coleta menor, um velho conta como vivo e não é
                        // percorrido (as referências dele a jovens estão nos
                        // lembrados); na completa, só o marcado é pulado.
                        let e = (*b).estado;
                        if if menor { e != JOVEM } else { e == MARCADO } {
                            break;
                        }
                        // A menor promove já na marcação (o jovem alcançado
                        // fica velho: a varredura só procura os mortos).
                        (*b).estado = if menor { VELHO } else { MARCADO };
                        live += 1;
                        // Só os campos que o mapa diz referência: o que
                        // esta coleta não percorre (velho na menor, já
                        // marcado) nem entra na pilha; o primeiro que falta
                        // marcar é seguido direto.
                        let c = corpo(b);
                        let n = usize::from((*c).n);
                        self.trabalho_da_marcacao += n;
                        let campos = campos_de(c);
                        let visitar = |bits: i64, proximo: &mut i64, pilha: &mut Vec<i64>| {
                            if !smi::e_handle(bits) {
                                return;
                            }
                            if e_objeto(bits) {
                                if !validar {
                                    let e = (*((bits - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).estado;
                                    if if menor { e != JOVEM } else { e == MARCADO } {
                                        return;
                                    }
                                }
                                if *proximo == 0 {
                                    *proximo = bits;
                                    return;
                                }
                            }
                            pilha.push(bits);
                        };
                        let mut m = (*c).mapa;
                        while m != 0 {
                            let i = m.trailing_zeros() as usize;
                            visitar(*campos.add(i), &mut proximo, &mut self.pending);
                            m &= m - 1;
                        }
                        if n > 32 {
                            let ext = campos.add(capacidade(n)).cast::<u64>();
                            for w in 0..palavras_do_mapa(n) {
                                let mut m = *ext.add(w);
                                while m != 0 {
                                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                                    visitar(*campos.add(i), &mut proximo, &mut self.pending);
                                    m &= m - 1;
                                }
                            }
                        }
                    }
                    atual = proximo;
                }
                continue;
            }
            // Raiz ou aresta que não é um handle vivo: mesmas quatro
            // mensagens de `get`, porque a causa é a mesma (N4).
            let index = self.indice_vivo(handle);
            if self.marks[index] || (menor && self.idade[index] != 0) {
                continue;
            }
            self.marks[index] = true;
            live += 1;
            self.trabalho_da_marcacao += self.slots[index]
                .as_ref()
                .expect("slot vivo verificado")
                .trace(&mut self.pending);
        }
        live
    }

    /// A coleta completa (o `dartforge_gc_collect` e os testes): marca a
    /// partir das raízes e varre tudo; o que sobrevive fica velho.
    pub fn collect(&mut self) {
        self.coletar(false);
    }

    /// Empilha em `destino` as raízes: as permanentes do runtime, os
    /// globais, os anexos de finalizador e os quadros (os do runtime e a
    /// pilha-sombra do código gerado).
    fn raizes(&self, destino: &mut Vec<i64>) {
        destino.extend(self.enum_values.values().copied());
        destino.extend(self.tearoffs.values().copied());
        destino.extend(self.literais.values().copied());
        destino.extend(self.globais.values().copied());
        destino.extend(self.caixas_bool.iter().copied().filter(|&h| h != 0));
        destino.extend(self.raizes_do_runtime.iter().copied().filter(|&h| h != 0));
        destino.extend(self.finalizacoes_prontas.iter().copied());
        for a in &self.anexos {
            destino.push(a.dono);
            if let AcaoDeFinalizador::Dart(acao) = a.acao {
                destino.push(acao);
            }
        }
        destino.extend(self.frames.iter().flat_map(|(_, roots)| roots.iter().copied()));
        visitar_quadros(|h| destino.push(h));
    }

    /// `DARTFORGE_GC_VERIFICAR=1`, depois da marcação de uma coleta menor:
    /// uma travessia completa a partir das raízes, sem gerações, não pode
    /// achar jovem sem marca — seria uma referência de velho para jovem que
    /// a barreira de escrita não lembrou.
    ///
    /// # Panics
    /// Com a primeira referência assim.
    fn verificar_coleta_menor(&self) {
        let mut pilha = Vec::new();
        self.raizes(&mut pilha);
        let mut visto = crate::hash::HashSet::default();
        let mut origem: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
        while let Some(h) = pilha.pop() {
            if !smi::e_handle(h) || h < 0 || !visto.insert(h) {
                continue;
            }
            let antes = pilha.len();
            if e_objeto(h) {
                let b = self.bloco_vivo(h);
                // SAFETY: bloco vivo do espaço.
                #[allow(unsafe_code)]
                let estado = unsafe { (*b).estado };
                if estado == JOVEM {
                    panic!(
                        "bug do coletor: jovem alcançável sem marca na coleta menor (barreira de escrita faltando)\nobjeto {h}, alcançado de {:?}",
                        origem.get(&h)
                    );
                }
                // SAFETY: bloco vivo do espaço.
                #[allow(unsafe_code)]
                unsafe {
                    empilhar_referencias(corpo(b), &mut pilha)
                };
            } else {
                let i = self.indice_vivo(h);
                if self.idade[i] == 0 && !self.marks[i] {
                    panic!(
                        "bug do coletor: jovem alcançável sem marca na coleta menor (barreira de escrita faltando)\nslot {h}, alcançado de {:?}",
                        origem.get(&h)
                    );
                }
                self.slots[i].as_ref().expect("slot vivo verificado").trace(&mut pilha);
            }
            for &f in &pilha[antes..] {
                origem.entry(f).or_insert(h);
            }
        }
    }

    /// Uma coleta: `menor` marca e varre só os jovens (os velhos contam como
    /// vivos, e os lembrados pela barreira de escrita entram como raízes);
    /// a completa, tudo. Em ambas o que sobrevive fica velho.
    fn coletar(&mut self, menor: bool) {
        // O que a TLAB não usou volta a ser livre.
        self.devolver_tlabs();
        self.coleta_menor = menor;
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self.literais.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots)| roots.len() as u64)
            .sum::<u64>();
        self.marks.resize(self.slots.len(), false);
        if !menor {
            self.stats.slots_scanned += self.slots.len() as u64;
            self.marks.fill(false);
        }
        self.trabalho_da_marcacao = 0;
        let mut pendentes = std::mem::take(&mut self.pending);
        pendentes.clear();
        self.raizes(&mut pendentes);
        if menor {
            // Os velhos lembrados: as referências deles podem ser jovens.
            for &b in &self.objetos.lembrados {
                // SAFETY: bloco lembrado, vivo.
                #[allow(unsafe_code)]
                unsafe { empilhar_referencias(corpo(b), &mut pendentes) };
            }
            for &i in &self.slots_lembrados {
                if let Some(v) = &self.slots[i] {
                    v.trace(&mut pendentes);
                }
            }
        }
        self.pending = pendentes;
        let mut live = self.marcar_pendentes();
        // Os efêmeros: o valor de um portador vivo é alcançado quando a
        // chave é — o que pode tornar vivas outras chaves, até o ponto fixo.
        if !self.efemeros.is_empty() {
            loop {
                let (marks, idade, objetos) = (&self.marks, &self.idade, &self.objetos);
                let marcado = |h: i64| marcado_na_coleta(marks, idade, objetos, menor, h);
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
        if menor && self.verificar {
            self.verificar_coleta_menor();
        }
        let (marks, idade, objetos) = (&self.marks, &self.idade, &self.objetos);
        let vivo = |h: &i64| marcado_na_coleta(marks, idade, objetos, menor, *h);
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
        if menor {
            // Só os jovens: o marcado fica velho; o resto sai. A estimativa
            // perde o que saiu (a completa a refaz dos vivos).
            let mut soltos = 0usize;
            for &index in &self.slots_jovens {
                let Some(valor) = self.slots[index].as_ref() else { continue };
                if self.marks[index] {
                    self.marks[index] = false;
                    self.idade[index] = 1;
                } else {
                    soltos = soltos.saturating_add(valor.estimated_bytes());
                    self.slots[index] = None;
                    self.free.push(index);
                    self.stats.reclaimed += 1;
                }
            }
            for &index in &self.slots_lembrados {
                self.idade[index] = 1;
            }
            let (mortos, _, bytes_soltos) = self.objetos.varrer_jovens();
            self.stats.reclaimed += mortos as u64;
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(soltos.saturating_add(bytes_soltos));
        } else {
            let mut vivos_em_bytes = 0usize;
            for (index, slot) in self.slots.iter_mut().enumerate() {
                let Some(valor) = slot.as_ref() else { continue };
                if self.marks[index] {
                    vivos_em_bytes = vivos_em_bytes.saturating_add(valor.estimated_bytes());
                    self.idade[index] = 1;
                } else {
                    *slot = None;
                    self.free.push(index);
                    self.stats.reclaimed += 1;
                }
            }
            let (mortos, _, bytes_de_objetos) = self.objetos.varrer();
            self.stats.reclaimed += mortos as u64;
            vivos_em_bytes = vivos_em_bytes.saturating_add(bytes_de_objetos);
            self.stats.estimated_bytes = vivos_em_bytes;
        }
        self.slots_jovens.clear();
        self.slots_lembrados.clear();
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
        self.bytes_jovens = 0;
        self.coleta_menor = false;
        if !menor {
            self.recalcular_gatilhos(live);
        }
        if self.rastrear {
            eprintln!(
                "[gc] {} marcados={live} trabalho={} estimados={} gatilho={} paginas={} vivos={}",
                if menor { "menor" } else { "completa" },
                self.trabalho_da_marcacao,
                self.stats.estimated_bytes,
                self.byte_threshold,
                self.objetos.paginas.len(),
                self.objetos.vivos
            );
        }
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
        // O dobro do que sobreviveu, e não menos que três quartos do gatilho
        // anterior (a histerese: uma estrutura grande que morre e volta — a
        // lista refeita a cada rodada — não faz o gatilho despencar e voltar
        // a subir com uma coleta completa a cada dobra).
        // E pelo menos dois semiespaços jovens acima do que sobreviveu: com
        // pouco dado vivo, o dobro dele vinha antes do gatilho da coleta
        // menor, e toda coleta era completa (a árvore longa de
        // `objetos_escapam/arvores` remarcada 250 vezes).
        let crescimento = self
            .stats
            .estimated_bytes
            .saturating_mul(2)
            .max(self.stats.estimated_bytes.saturating_add(2 * LIMITE_JOVEM))
            .max(self.byte_threshold - self.byte_threshold / 4);
        self.byte_threshold = if self.limite_bytes == usize::MAX {
            crescimento
        } else {
            let folga = self.limite_bytes.saturating_sub(self.bytes_totais(0));
            crescimento.min(self.stats.estimated_bytes.saturating_add((folga / 2).max(PASSO_MINIMO)))
        };
        // Não há mais gatilho por contagem para a coleta completa: ele
        // existia porque toda coleta varria a tabela de slots inteira, e com
        // poucos vivos coletava a cada 256 alocações (um `sort` de 100 mil
        // `int` com alguns `_Mint` percorria a lista toda a cada vez). A
        // coleta menor varre só os jovens e vem por bytes ou por
        // `CONTAGEM_JOVEM` alocações; a completa, pelos bytes.
        let _ = vivos;
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.slots.len() - self.free.len() + self.objetos.vivos,
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
        let fields = heap.objeto(first).expect("enum deve ser objeto");
        assert_eq!(fields.class_id, 1);
        assert_eq!(fields.campo(0), (0, false));
        assert!(matches!(heap.get(fields.campo(1).0), Value::String(name) if name == "red"));
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Um único objeto raiz mantém transitivamente ciclos e strings de seus campos.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.novo_objeto(1, &[(0, true)]);
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
        let a = heap.novo_objeto(1, &[(0, true)]);
        heap.root(frame, a);
        let b = heap.novo_objeto(2, &[(a, true)]);
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // Os blocos dos mortos voltam à lista livre do tamanho deles.
        let c = heap.novo_objeto(3, &[(0, false)]);
        assert!(c == a || c == b);
        assert_eq!(heap.stats().live_objects, 1);
    }
    /// Bits escalares coincidentes com handles não retêm objetos.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.allocate(Value::String("descartável".into()));
        let object = heap.novo_objeto(1, &[(text, false)]);
        heap.root(frame, object);
        heap.collect();
        assert!(heap.slots[Heap::indice_de(text)].is_none());
        assert!(matches!(heap.get(object), Value::Objeto));
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
        // O temporário da volta anterior ficou velho na coleta explícita
        // (completa): a coleta menor antes da alocação seguinte não o solta,
        // a completa da volta sim — no máximo três slots.
        assert!(heap.slots.len() <= 3);
    }
}

#[cfg(test)]
mod espaco_de_objetos {
    //! O espaço de objetos: blocos por número de campos, handles com o bit
    //! 1, listas livres refeitas na varredura e páginas soltas.
    use super::*;


    #[test]
    fn handles_de_objeto_e_de_slot_nao_se_confundem() {
        let mut heap = Heap::new(false);
        let o = heap.alocar_objeto(7, 3);
        let s = heap.allocate(Value::String("x".into()));
        assert!(e_objeto(o) && !e_objeto(s) && s % 4 == 0);
        assert!(matches!(heap.objeto(o), Some(f) if f.class_id == 7 && f.len() == 3));
        // Escalares quaisquer não são objetos (nem derrubam o `try_get`).
        for x in [2, 6, 18, o + 4, o - 4, o + 56, -2, i64::MAX - 1] {
            assert!(heap.try_get(x).is_none() || x == o, "{x}");
        }
        heap.set(o, 1, s, true);
        assert_eq!(heap.objeto(o).expect("objeto").campo(1), (s, true));
    }

    #[test]
    fn coleta_devolve_blocos_e_mantem_os_alcancaveis() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        // Uma lista ligada de 10 mil nós alcançável e 10 mil mortos.
        let mut cabeca = 0;
        for i in 0..10_000 {
            let lixo = heap.alocar_objeto(1, 2);
            heap.set(lixo, 0, i, false);
            let no = heap.alocar_objeto(2, 2);
            heap.set(no, 0, i, false);
            heap.set(no, 1, cabeca, true);
            cabeca = no;
            heap.set_root(frame, 0, cabeca);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 10_000);
        let mut soma = 0;
        let mut no = cabeca;
        while no != 0 {
            let fields = heap.objeto(no).expect("objeto");
            soma += fields.campo(0).0;
            no = fields.campo(1).0;
        }
        assert_eq!(soma, (0..10_000).sum::<i64>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // As páginas ficam enquanto o pico recente de vivos as pede (a
        // estrutura pode voltar); o pico decai a cada coleta completa, e as
        // vazias além da folga voltam ao sistema.
        let retidas = heap.objetos.paginas.len();
        assert!(retidas > 2);
        for _ in 0..20 {
            heap.collect();
        }
        assert!(heap.objetos.paginas.len() <= 2, "{} páginas de {retidas}", heap.objetos.paginas.len());
    }

    #[test]
    fn objeto_que_cresce_segue_pelo_ponteiro_dos_campos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let o = heap.alocar_objeto(3, 1);
        heap.root(frame, o);
        heap.set(o, 0, 41, false);
        heap.garantir_campos(o, 5);
        heap.set(o, 4, 42, false);
        heap.collect();
        let fields = heap.objeto(o).expect("objeto");
        assert_eq!((fields.len(), fields.campo(0).0, fields.campo(4).0), (5, 41, 42));
        heap.pop_frame(frame);
        heap.collect();
        // O bloco voltou ao molde: um objeto novo de 1 campo o reusa zerado.
        let novo = heap.alocar_objeto(4, 1);
        assert_eq!(novo, o);
        assert!(matches!(heap.objeto(novo), Some(f) if f.class_id == 4 && f.len() == 1 && f.campo(0) == (0, false)));
    }

    #[test]
    fn hash_de_identidade_estavel_e_distinto() {
        let mut heap = Heap::new(false);
        let a = heap.alocar_objeto(1, 0);
        let b = heap.alocar_objeto(1, 0);
        let ha = heap.hash_de_identidade(a).expect("objeto");
        assert_eq!(heap.hash_de_identidade(a), Some(ha));
        assert_ne!(heap.hash_de_identidade(b), Some(ha));
        assert!(ha > 0 && ha <= 0x3fff_ffff);
    }

    /// A alocação em linha do código gerado: avança o cursor da faixa da
    /// TLAB e grava o cabeçalho (`llvm/mod.rs`).
    #[allow(unsafe_code)]
    fn alocar_como_o_codigo_gerado(heap: &mut Heap, class_id: i64, n: usize) -> i64 {
        let tamanho = tamanho_do_bloco(n);
        let b = CONTEXTO.with(|c| {
            let (cursor, fim) = (c.tlab[n][0].get(), c.tlab[n][1].get());
            if (fim as usize).saturating_sub(cursor as usize) < tamanho {
                return None;
            }
            c.tlab[n][0].set(cursor.wrapping_add(tamanho));
            Some(cursor)
        });
        let Some(b) = b else {
            let h = heap.alocar_objeto(class_id, n);
            heap.reabastecer_tlab(n);
            return h;
        };
        // SAFETY: bloco livre da faixa.
        unsafe {
            *b.add(16).cast::<i64>() = 0;
            *b.cast::<u64>() = 1 | (n as u64) << 16 | (class_id as u64) << 32;
        }
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    #[test]
    fn faixas_da_tlab_sobrevivem_e_voltam_a_lista() {
        // Numa thread nova: o `Contexto` dela começa sem TLAB.
        std::thread::spawn(|| {
            let mut heap = Heap::do_isolado(false);
            let frame = heap.push_frame_with_slots(1);
            let mut cabeca = 0;
            let mut lixo = Vec::new();
            for i in 0..10_000 {
                let elo = alocar_como_o_codigo_gerado(&mut heap, 5, 2);
                heap.set(elo, 0, smi::de(i).unwrap(), false);
                heap.set(elo, 1, cabeca, true);
                if i % 3 == 0 {
                    cabeca = elo;
                    heap.set_root(frame, 0, cabeca);
                } else {
                    lixo.push(elo);
                }
                if i % 2_500 == 1_249 {
                    heap.coletar(i % 5_000 != 1_249);
                }
            }
            heap.coletar(true);
            let mut n = 0;
            let mut h = cabeca;
            while h != 0 {
                let o = heap.objeto(h).expect("elo vivo");
                assert_eq!(o.class_id, 5);
                n += 1;
                h = o.campo(1).0;
            }
            assert_eq!(n, 3_334);
            assert_eq!(heap.stats().live_objects, 3_334);
            // Os mortos voltaram à lista: alocar de novo não cresce o heap.
            let paginas = heap.objetos.paginas.len();
            for _ in 0..2_000 {
                alocar_como_o_codigo_gerado(&mut heap, 6, 2);
            }
            assert_eq!(heap.objetos.paginas.len(), paginas);
            heap.set_root(frame, 0, 0);
            heap.coletar(false);
            assert_eq!(heap.stats().live_objects, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn coleta_menor_segue_os_lembrados_e_solta_os_jovens_mortos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let velho = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, velho);
        heap.coletar(false);
        // Jovem só alcançável pelo velho (a barreira de `set` o lembra).
        let jovem = heap.alocar_objeto(2, 1);
        heap.set(jovem, 0, 7, false);
        heap.set(velho, 0, jovem, true);
        let lixo = heap.alocar_objeto(3, 1);
        heap.coletar(true);
        assert!(matches!(heap.objeto(jovem), Some(f) if f.class_id == 2 && f.campo(0).0 == 7));
        assert!(heap.try_get(lixo).is_none());
        assert_eq!(heap.stats().live_objects, 2);
        // Depois dela, o jovem ficou velho: sem barreira nova, outra menor
        // não o percorre, e ele continua vivo.
        heap.coletar(true);
        assert!(heap.try_get(jovem).is_some());
        // A completa solta o que só os velhos mortos alcançavam.
        heap.set_root(frame, 0, 0);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn slots_velhos_gravados_pelo_runtime_sao_lembrados() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let lista = heap.create_list(Vec::new());
        heap.set_root(frame, 0, lista);
        heap.coletar(false);
        let jovem = heap.alocar_objeto(5, 0);
        heap.list_push(lista, TaggedValue::reference(jovem));
        heap.coletar(true);
        assert!(heap.try_get(jovem).is_some());
        assert_eq!(heap.list_get(lista, 0).bits, jovem);
    }

    #[test]
    #[should_panic(expected = "barreira de escrita faltando")]
    fn verificacao_acha_barreira_faltando() {
        let mut heap = Heap::new(false);
        heap.verificar = true;
        let frame = heap.push_frame_with_slots(1);
        let velho = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, velho);
        heap.coletar(false);
        let jovem = heap.alocar_objeto(2, 0);
        // Grava sem barreira (como faria código gerado sem ela).
        let b = heap.objetos.bloco_de(velho).expect("objeto");
        #[allow(unsafe_code)]
        // SAFETY: bloco vivo de 1 campo.
        unsafe {
            *campos_de(b) = jovem;
            marcar_referencia(b, 0, true);
        }
        heap.coletar(true);
    }

    /// A carga de `bench/desempenho/objetos_escapam.dart` (`lista_ligada`)
    /// direto no heap, para medir o coletor sem compilar Dart:
    /// `cargo test --release -p dartforge-runtime lista_ligada_no_heap --
    /// --ignored --nocapture` (com `DARTFORGE_GC_RASTRO=1`, as coletas).
    #[test]
    #[ignore = "medição"]
    fn lista_ligada_no_heap() {
        let mut heap = Heap::new(false);
        heap.rastrear = std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1");
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..6 {
            let t = std::time::Instant::now();
            let mut cab = 0;
            for i in 0..1_000_000 {
                let no = heap.alocar_objeto(1, 2);
                heap.set(no, 0, i, false);
                heap.set(no, 1, cab, true);
                cab = no;
                heap.set_root(frame, 0, cab);
            }
            eprintln!("rodada: {:?} coletas={}", t.elapsed(), heap.stats().collections);
        }
    }

    #[test]
    fn mapa_de_referencias_alem_de_32_campos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.alocar_objeto(1, 40);
        heap.set_root(frame, 0, o);
        let mut filhos = Vec::new();
        for i in [0usize, 31, 32, 39] {
            let f = heap.alocar_objeto(2, 0);
            heap.set(o, i as i64, f, true);
            filhos.push(f);
        }
        // Um escalar que parece handle não segura nada.
        let lixo = heap.alocar_objeto(3, 0);
        heap.set(o, 33, lixo, false);
        heap.coletar(false);
        for f in &filhos {
            assert!(heap.objeto(*f).is_some());
        }
        assert!(heap.objeto(lixo).is_none());
        let obj = heap.objeto(o).expect("objeto");
        assert_eq!(obj.campo(32), (filhos[2], true));
        assert!(!obj.campo(33).1);
        // Trocar a referência por escalar apaga o bit.
        heap.set(o, 39, 5, false);
        heap.coletar(false);
        assert!(heap.objeto(filhos[3]).is_none());
    }

    #[test]
    fn corpo_de_fora_segue_na_coleta_e_na_barreira() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, o);
        heap.coletar(false);
        // O objeto velho cresce (corpo de fora) e recebe um jovem no campo
        // novo: a coleta menor o acha pelo lembrado.
        heap.garantir_campos(o, 3);
        let jovem = heap.alocar_objeto(2, 0);
        heap.set(o, 2, jovem, true);
        heap.coletar(true);
        assert!(heap.objeto(jovem).is_some());
        assert_eq!(heap.objeto(o).expect("objeto").len(), 3);
        heap.pop_frame(frame);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn objeto_grande_tem_pagina_propria() {
        let mut heap = Heap::new(false);
        let o = heap.alocar_objeto(9, MAIOR_CLASSE + 100);
        heap.set(o, (MAIOR_CLASSE + 99) as i64, 5, false);
        assert!(matches!(heap.objeto(o), Some(f) if f.campo(MAIOR_CLASSE + 99).0 == 5));
        heap.collect();
        assert!(heap.try_get(o).is_none());
        assert!(heap.objetos.paginas.is_empty());
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
        let obj = heap.novo_objeto(1, &[(s, true)]);
        heap.set_root(frame, 1, obj);
        heap.set(obj, 0, smi::de(-5).unwrap(), true);
        let lista = heap.create_list(vec![TaggedValue::reference(s)]);
        // A lista normaliza o `Smi` para o escalar (R8).
        assert_eq!(heap.list_get(lista, 0), TaggedValue::scalar(3));
        heap.collect();
        assert!(matches!(heap.get(obj), Value::Objeto));
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

    /// N14: a forma compacta guarda só os bits, com a mesma leitura; uma
    /// vaga null vira 0; um valor de outra forma devolve a lista à geral.
    #[test]
    fn lista_compacta_mantem_valores_e_volta_a_geral() {
        let mut e = Elementos::new(vec![TaggedValue::scalar(7), TaggedValue::reference(0), TaggedValue::scalar(-1)]);
        assert!(e.compactar(FormaDeLista::Int));
        assert_eq!(e.forma(), FormaDeLista::Int);
        assert_eq!(e.to_vec(), vec![TaggedValue::scalar(7), TaggedValue::scalar(0), TaggedValue::scalar(-1)]);
        assert_eq!((e.0.forma, e.0.len), (1, 3));
        assert_eq!(e.0.dados, e.0.compacto.as_ptr() as *mut u8);
        e.push(TaggedValue::scalar(9));
        e.redimensionar(6, TaggedValue::reference(0));
        let f = e.fatia(3, 5);
        assert_eq!((f.forma(), f.to_vec()), (FormaDeLista::Int, vec![TaggedValue::scalar(9), TaggedValue::scalar(0)]));
        assert!(!e.clone().compactar(FormaDeLista::Double));
        e.conferir_gravacao(1);
        e.definir(1, TaggedValue::boolean(true));
        assert_eq!(e.forma(), FormaDeLista::Geral);
        assert!(!e.gravacao_conferida(1));
        assert_eq!(e.valor(1), TaggedValue::boolean(true));
        assert_eq!(e.valor(0), TaggedValue::scalar(7));
        assert!(!e.compactar(FormaDeLista::Int));
    }

    /// O coletor não segue os bits de uma lista compacta (um `int` par
    /// pareceria um handle).
    #[test]
    fn lista_compacta_nao_tem_referencias() {
        let mut heap = Heap::new(false);
        let s = heap.allocate(Value::String(Texto::de_str("x")));
        let lista = heap.create_list(vec![TaggedValue::scalar(s)]);
        if let Value::List(e) = heap.get_mut(lista) {
            assert!(e.compactar(FormaDeLista::Int));
        }
        let quadro = heap.push_frame();
        heap.root(quadro, lista);
        heap.collect();
        assert!(heap.try_get(s).is_none(), "o `int` com os bits do handle não mantém a string viva");
        assert_eq!(heap.list_get(lista, 0), TaggedValue::scalar(s));
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
        // O portão antigo só coletava com frame; agora o limiar basta (a
        // coleta menor, por contagem de alocações).
        let mut heap = Heap::new(false);
        let n = CONTAGEM_JOVEM + 1000;
        for _ in 0..n {
            heap.allocate(Value::String("lixo".into()));
        }
        assert!(heap.stats().collections > 0);
        assert!(heap.stats().live_objects < n);
    }

    #[test]
    fn tabela_lateral_purgada_quando_o_slot_e_reutilizado() {
        let mut heap = Heap::new(false);
        let lista = heap.create_list(Vec::new());
        heap.marcar_imutavel(lista);
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
        let a = heap.novo_objeto(7, &[]);
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.novo_objeto(7, &[]);
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
        let first = heap.novo_objeto(1, &[(0, true)]);
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.novo_objeto(1, &[(first, true)]);
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
                let handle = heap.novo_objeto(1, &[(index, false)]);
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
        // O pico: o jovem entre coletas menores e o lixo velho (cada payload
        // enraizado vira velho e morre na volta seguinte) até a completa,
        // que vem dois semiespaços jovens acima do que sobreviveu.
        assert!(stats.peak_estimated_bytes < 2 * LIMITE_JOVEM + 3 * 2 * 1024 * 1024, "{}", stats.peak_estimated_bytes);
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

/// A época dos layouts de objeto do processo (J03): cresce a cada migração
/// que uma recarga do JIT define (`dartforge_definir_migracao`).
pub static EPOCA_DE_LAYOUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
