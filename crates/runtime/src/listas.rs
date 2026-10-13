//! As listas do núcleo no espaço unificado (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_List` (cid 8) e `_ImmutableList` (9) são `REFS` (o comprimento na palavra 0,
//! os elementos `Ref` depois; cartões nas grandes) ou `BRUTO` com a forma compacta
//! em `flags` (`ELEMENTO_INT`/`_DOUBLE`/`_BOOL`); `_GrowableList` (10) é
//! `INSTANCIA` de dois campos: o comprimento (bruto) e o armazenamento (`_List`,
//! `Ref`), como o `GrowableObjectArray` da VM (`raw_object.h:3561-3572`).
//!
//! Regras desta vista:
//!
//! * **A classe diz** fixa (`_List`), imutável (`_ImmutableList`) ou expansível
//!   (`_GrowableList`); o cid nunca muda depois de publicado (§2.16). Fixar ou
//!   tornar imutável é copiar para um objeto novo ([`Heap::fixed_list_of`],
//!   [`Heap::unmodifiable_list_of`]).
//! * **A forma é do armazenamento** (a `_List` de uma `_GrowableList`): só
//!   representação, nunca semântica. Uma forma compacta que não guarda o valor
//!   gravado descompacta (§2.16): monta um armazenamento geral enraizado com as
//!   caixas e só então troca, sem alocação entre a mudança dos dados e a do
//!   `flags`.
//! * **Toda gravação de `Ref`** num armazenamento publicado passa por
//!   [`Heap::write_ref`]/[`Heap::write_refs`] (barreira e cartão); o armazenamento
//!   recém-alocado, ainda jovem e sem outra referência, é preenchido por
//!   [`Heap::words_mut`].
//! * **Coleta no meio:** os métodos que alocam (`nova_*`, `list_get_ref`,
//!   `list_set`, `list_push`, `list_reserve`, `list_adjust_shape`,
//!   `list_copy`, `lista_*_de`) enraízam o que seguram entre duas alocações; os
//!   handles que o chamador segura depois da chamada são dele.
//! * **A compactação aceita null.** [`Heap::list_adjust_shape`] é chamado depois
//!   de o `E` reificado (`int`, `double` ou `bool`, não anuláveis) ser gravado; um
//!   null dentro do comprimento só existe antes de o SDK preencher a lista
//!   (`List<int>.filled`, `_GrowableList<int>(n)`) e vira 0/0.0/false. Pelo mesmo
//!   motivo, gravar null numa lista compacta **de tipo gravado** (metadado ≠ 0: o
//!   `length =` que encolhe limpa as posições cortadas com null) grava zero em vez
//!   de descompactar; sem tipo gravado, descompacta.

use crate::heap::{Heap, Value};
use crate::layout::{self, cid, flags, Ref};

/// A forma dos elementos de uma lista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// `Ref` (`REFS`).
    General,
    /// `i64` sem caixa.
    Int,
    /// Bits de `f64`.
    Double,
    /// 0/1.
    Bool,
}

impl Element {
    /// Os bits de `flags` (`FORMA` e `ELEMENTO`) de um armazenamento nesta forma.
    pub const fn flags(self) -> u8 {
        match self {
            Element::General => flags::REFS,
            Element::Int => flags::BRUTO | flags::ELEMENTO_INT,
            Element::Double => flags::BRUTO | flags::ELEMENTO_DOUBLE,
            Element::Bool => flags::BRUTO | flags::ELEMENTO_BOOL,
        }
    }

    /// A forma de um armazenamento pelos bits de `flags`.
    pub const fn from_flags(f: u8) -> Element {
        if f & flags::FORMA != flags::BRUTO {
            return Element::General;
        }
        match f & flags::ELEMENTO {
            flags::ELEMENTO_INT => Element::Int,
            flags::ELEMENTO_DOUBLE => Element::Double,
            flags::ELEMENTO_BOOL => Element::Bool,
            _ => Element::General,
        }
    }

    /// O código da ABI de `dartforge_lista_nova` (0 geral, 1 int, 2 double, 3
    /// bool: a ordem das variantes).
    pub const fn code(self) -> i64 {
        match self {
            Element::General => 0,
            Element::Int => 1,
            Element::Double => 2,
            Element::Bool => 3,
        }
    }

    /// A forma do código da ABI; `None` para outro valor.
    pub const fn from_code(c: i64) -> Option<Element> {
        match c {
            0 => Some(Element::General),
            1 => Some(Element::Int),
            2 => Some(Element::Double),
            3 => Some(Element::Bool),
            _ => None,
        }
    }

    /// Os bits de `v` nesta forma compacta, se ela o guarda sem caixa.
    fn bits_of(self, v: Value) -> Option<i64> {
        match (self, v) {
            (Element::Int, Value::Int(x)) => Some(x),
            (Element::Double, Value::Double(d)) => Some(d.to_bits() as i64),
            (Element::Bool, Value::Bool(b)) => Some(i64::from(b)),
            _ => None,
        }
    }

    /// O valor dos bits `w` de um elemento nesta forma.
    fn value_of(self, w: i64) -> Value {
        match self {
            Element::General => Value::Ref(w),
            Element::Int => Value::Int(w),
            Element::Double => Value::Double(f64::from_bits(w as u64)),
            Element::Bool => Value::Bool(w != 0),
        }
    }
}

/// Os elementos de uma lista, cortados no comprimento, na forma do armazenamento.
#[derive(Clone, Copy)]
pub enum RefElements<'a> {
    General(&'a [i64]),
    Int(&'a [i64]),
    Double(&'a [f64]),
    Bool(&'a [i64]),
}

impl RefElements<'_> {
    /// Quantos elementos.
    pub fn len(&self) -> usize {
        match self {
            RefElements::General(p) | RefElements::Int(p) | RefElements::Bool(p) => p.len(),
            RefElements::Double(p) => p.len(),
        }
    }

    /// Sem elementos?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// O elemento `i`, sem caixa.
    pub fn get(&self, i: usize) -> Value {
        match self {
            RefElements::General(p) => Value::Ref(p[i]),
            RefElements::Int(p) => Value::Int(p[i]),
            RefElements::Double(p) => Value::Double(p[i]),
            RefElements::Bool(p) => Value::Bool(p[i] != 0),
        }
    }
}

impl Heap {
    /// Uma `_List` ou `_ImmutableList` (`cid`) de `len` elementos zerados (null,
    /// 0, 0.0 ou false, pela forma).
    ///
    /// O corpo tem sempre as palavras de cartões de uma lista grande
    /// (`layout::list_words`), também na forma compacta: descompactar no
    /// lugar (§2.16) precisa delas.
    pub fn new_list(&mut self, cid: i32, len: usize, e: Element) -> Ref {
        assert!(cid::is_fixed_list(cid), "bug do runtime: nova_lista com o cid {cid}");
        let mut f = e.flags();
        if layout::has_cards(len) {
            f |= flags::CARTOES;
        }
        let h = self.allocate(cid, layout::list_words(len), f);
        self.words_mut(h)[0] = len as i64;
        h
    }

    /// Uma `_GrowableList` de `len` elementos zerados e armazenamento de
    /// `capacidade` (no mínimo `len`).
    pub fn new_growable(&mut self, len: usize, capacidade: usize, e: Element) -> Ref {
        let data = self.new_list(cid::LIST, capacidade.max(len), e);
        let h = self.list_with_roots(&[data], |heap| heap.allocate_instance(cid::GROWABLE_LIST, 2));
        self.set_field(h, 0, len as i64, false);
        self.set_field(h, 1, data, true);
        h
    }

    /// `h` é lista do núcleo (cid 8, 9 ou 10)? Sem pânico para null, `Smi` ou
    /// objeto morto.
    pub fn e_lista(&self, h: Ref) -> bool {
        layout::is_object(h) && self.is_live_object(h) && cid::e_lista(self.header(h).class_id)
    }

    /// O comprimento.
    pub fn list_len(&self, h: Ref) -> usize {
        match self.list_cid(h) {
            cid::GROWABLE_LIST => self.list_growable_field(h, 0) as usize,
            _ => self.words(h)[0] as usize,
        }
    }

    /// A capacidade (o comprimento do armazenamento).
    pub fn list_capacity(&self, h: Ref) -> usize {
        let a = self.list_data(h);
        self.words(a)[0] as usize
    }

    /// A forma dos elementos (do armazenamento, numa `_GrowableList`).
    pub fn list_shape(&self, h: Ref) -> Element {
        let a = self.list_data(h);
        Element::from_flags(self.header(a).flags)
    }

    /// Os elementos, cortados no comprimento.
    #[allow(unsafe_code)]
    pub fn list_elements(&self, h: Ref) -> RefElements<'_> {
        let n = self.list_len(h);
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        let p = &self.words(a)[1..1 + n];
        match shape {
            Element::General => RefElements::General(p),
            Element::Int => RefElements::Int(p),
            Element::Bool => RefElements::Bool(p),
            // SAFETY: `i64` e `f64` têm o mesmo tamanho e alinhamento, e todo
            // padrão de bits é um `f64`; a fatia é a mesma memória, só lida.
            Element::Double => RefElements::Double(unsafe { std::slice::from_raw_parts(p.as_ptr().cast::<f64>(), n) }),
        }
    }

    /// O elemento `i`, sem caixa. Pânico se `i` está fora do comprimento.
    pub fn list_get(&self, h: Ref, i: usize) -> Value {
        let n = self.list_len(h);
        assert!(i < n, "bug do runtime: elemento {i} de uma lista de {n}");
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        shape.value_of(self.words(a)[1 + i])
    }

    /// O elemento `i` em posição `Ref` (encaixota o compacto; pode coletar).
    pub fn list_get_ref(&mut self, h: Ref, i: usize) -> Ref {
        match self.list_get(h, i) {
            Value::Ref(r) => r,
            v => self.to_ref(v),
        }
    }

    /// Grava o elemento `i` (descompacta se a forma não guarda `v`; com barreira).
    /// Pânico se `i` está fora do comprimento. Pode coletar (caixa, descompactação).
    pub fn list_set(&mut self, h: Ref, i: usize, v: Value) {
        let n = self.list_len(h);
        assert!(i < n, "bug do runtime: gravação no elemento {i} de uma lista de {n}");
        self.list_write_to_storage(h, i, v);
    }

    /// Acrescenta ao fim de uma `_GrowableList` (cresce se preciso: a capacidade
    /// seguinte é `(capacidade * 2) | 3`, o `_nextCapacity` da VM,
    /// `growable_array.dart:385`). Pode coletar.
    pub fn list_push(&mut self, h: Ref, v: Value) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "bug do runtime: lista_push numa lista fixa");
        let len = self.list_len(h);
        let cap = self.list_capacity(h);
        if len == cap {
            let r = match v {
                Value::Ref(r) => r,
                _ => 0,
            };
            self.list_with_roots(&[h, r], |heap| heap.list_reserve(h, (cap * 2) | 3));
        }
        self.set_field(h, 0, (len + 1) as i64, false);
        self.list_write_to_storage(h, len, v);
    }

    /// Muda o comprimento de uma `_GrowableList` (sem barreira; o armazenamento
    /// precisa comportar `len`).
    pub fn list_set_len(&mut self, h: Ref, len: usize) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "bug do runtime: comprimento de uma lista fixa");
        debug_assert!(len <= self.list_capacity(h), "comprimento {len} além da capacidade");
        self.set_field(h, 0, len as i64, false);
    }

    /// O armazenamento de uma `_GrowableList`; a própria lista, se é fixa ou
    /// imutável (o `@df.lista_armazenamento` do código gerado).
    pub fn list_data(&self, h: Ref) -> Ref {
        match self.list_cid(h) {
            cid::GROWABLE_LIST => self.list_growable_field(h, 1),
            _ => h,
        }
    }

    /// Troca o armazenamento de uma `_GrowableList` (com barreira). `data` é uma
    /// `_List`.
    pub fn list_set_data(&mut self, h: Ref, data: Ref) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "bug do runtime: armazenamento de uma lista fixa");
        debug_assert!(cid::is_fixed_list(self.class(data)), "o armazenamento é uma _List");
        self.set_field(h, 1, data, true);
    }

    /// Garante armazenamento para `capacidade` elementos numa `_GrowableList`:
    /// um armazenamento novo, da mesma forma, com os elementos até o comprimento.
    /// Nada se a capacidade já basta. Pode coletar.
    pub fn list_reserve(&mut self, h: Ref, capacidade: usize) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "bug do runtime: reserva numa lista fixa");
        if capacidade <= self.list_capacity(h) {
            return;
        }
        let shape = self.list_shape(h);
        let new = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, capacidade, shape));
        let n = self.list_len(h);
        let old_gen = self.list_data(h);
        // O novo é jovem e ninguém mais o vê: cópia crua, sem barreira.
        let copy: Vec<i64> = self.words(old_gen)[1..1 + n].to_vec();
        self.words_mut(new)[1..1 + n].copy_from_slice(&copy);
        self.list_set_data(h, new);
    }

    /// Troca a forma do armazenamento, convertendo os elementos no lugar (§2.15,
    /// §2.16). Para a forma compacta, só se todo elemento até o comprimento cabe
    /// nela (null conta como 0/0.0/false: ver o cabeçalho do módulo); para a
    /// geral, descompacta (aloca as caixas). A `_GrowableList` de armazenamento
    /// vazio ganha um armazenamento vazio próprio da forma (o vazio é
    /// compartilhado, o `_emptyList` do SDK). Pode coletar.
    pub fn list_adjust_shape(&mut self, h: Ref, e: Element) {
        let current = self.list_shape(h);
        if current == e {
            return;
        }
        let a = self.list_data(h);
        let cap = self.words(a)[0] as usize;
        if self.list_cid(h) == cid::GROWABLE_LIST && cap == 0 {
            let new = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, 0, e));
            self.list_set_data(h, new);
            return;
        }
        if current != Element::General {
            self.list_decompact(h);
            if e == Element::General {
                return;
            }
        }
        self.list_compact(h, e);
    }

    /// Copia `n` elementos de `de[start..]` para `to[dest..]` (com barreira;
    /// `to` descompacta se um elemento não cabe na forma dela). As faixas estão
    /// dentro dos comprimentos; `de` e `to` podem ser a mesma lista (a cópia é a
    /// de `memmove`). Pode coletar.
    pub fn list_copy(&mut self, de: Ref, start: usize, to: Ref, dest: usize, n: usize) {
        if n == 0 {
            return;
        }
        let (nd, np) = (self.list_len(de), self.list_len(to));
        assert!(start + n <= nd && dest + n <= np, "bug do runtime: cópia fora das listas");
        let (fd, fp) = (self.list_shape(de), self.list_shape(to));
        let (ad, ap) = (self.list_data(de), self.list_data(to));
        if fd == fp {
            let source: Vec<i64> = self.words(ad)[1 + start..1 + start + n].to_vec();
            if fp == Element::General {
                self.write_refs(ap, 1 + dest, &source);
            } else {
                self.words_mut(ap)[1 + dest..1 + dest + n].copy_from_slice(&source);
            }
            return;
        }
        // Formas diferentes: elemento a elemento, com as caixas e a
        // descompactação do destino que forem precisas. Os valores saem antes
        // (a fonte pode ser o próprio destino, que a descompactação troca).
        let value_list: Vec<Value> = (0..n).map(|k| self.list_get(de, start + k)).collect();
        self.list_with_roots(&[de, to], |heap| {
            for (k, v) in value_list.into_iter().enumerate() {
                heap.list_write_to_storage(to, dest + k, v);
            }
        });
    }

    /// Uma `_List` nova com os elementos de `h` (na mesma forma). Pode coletar.
    pub fn fixed_list_of(&mut self, h: Ref) -> Ref {
        self.list_copy_of(h, cid::LIST)
    }

    /// Uma `_ImmutableList` nova com os elementos de `h` (na mesma forma). Pode
    /// coletar.
    pub fn unmodifiable_list_of(&mut self, h: Ref) -> Ref {
        self.list_copy_of(h, cid::IMMUTABLE_LIST)
    }

    /// Uma `_List`/`_ImmutableList` (`cid`) geral com os `Ref` de `value_list` (as
    /// listas que o runtime monta: argumentos, resultados, literais). Os `Ref`
    /// precisam estar vivos até a alocação (quem chama os enraíza, se preciso).
    pub fn ref_list(&mut self, cid: i32, value_list: &[Ref]) -> Ref {
        let h = self.new_list(cid, value_list.len(), Element::General);
        // Recém-alocada, jovem: sem barreira.
        self.words_mut(h)[1..1 + value_list.len()].copy_from_slice(value_list);
        h
    }

    /// Grava `v` em todos os elementos de `h` (o `List.filled`): uma gravação
    /// normal no primeiro (a caixa, se a forma geral pedir, ou a descompactação,
    /// se a compacta não guardar `v`), e as mesmas palavras nos demais — o mesmo
    /// objeto em toda posição, como na VM. Pode coletar.
    pub fn list_fill(&mut self, h: Ref, v: Value) {
        let n = self.list_len(h);
        if n == 0 {
            return;
        }
        self.list_set(h, 0, v);
        let a = self.list_data(h);
        let w = self.words(a)[1];
        if self.list_shape(h) == Element::General {
            self.write_refs(a, 1, &vec![w; n]);
        } else {
            self.words_mut(a)[1..1 + n].fill(w);
        }
    }

    // ── Auxiliares ───────────────────────────────────────────────────────────

    /// O cid de uma lista do núcleo; pânico (N4) se `h` não é uma.
    fn list_cid(&self, h: Ref) -> i32 {
        let c = self.class(h);
        assert!(cid::e_lista(c), "bug do runtime: lista do núcleo esperada (cid {c}, handle {h})");
        c
    }

    /// O campo `i` (0 comprimento, 1 armazenamento) de uma `_GrowableList`.
    fn list_growable_field(&self, h: Ref, i: usize) -> i64 {
        self.object(h).expect("_GrowableList é INSTANCIA").field(i).0
    }

    /// `f` com cópias proprietárias de `hs` num quadro temporário do runtime.
    /// Null e `Smi` não contam RC. O resultado deve ser publicado sem coleta
    /// entre o fechamento deste quadro e a gravação da referência pelo chamador.
    pub(crate) fn list_with_roots<R>(&mut self, hs: &[Ref], f: impl FnOnce(&mut Heap) -> R) -> R {
        let frame = self.push_owning_frame(hs.len());
        for (i, &x) in hs.iter().enumerate() {
            self.set_root(frame, i, x);
        }
        let r = f(self);
        self.pop_frame(frame);
        r
    }

    /// Grava `v` no elemento `i` do armazenamento de `h` (o comprimento já foi
    /// conferido por quem chama).
    fn list_write_to_storage(&mut self, h: Ref, i: usize, v: Value) {
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        if shape == Element::General {
            let r = match v {
                Value::Ref(r) => r,
                // A caixa pode coletar: `h` fica enraizado (o armazenamento é
                // relido depois).
                other_one => self.list_with_roots(&[h], |heap| heap.to_ref(other_one)),
            };
            let a = self.list_data(h);
            self.write_ref(a, 1 + i, r);
            return;
        }
        // Forma compacta: o valor sem caixa, se couber.
        let v = match v {
            Value::Ref(r) => self.value(r),
            other_one => other_one,
        };
        if let Some(bits) = shape.bits_of(v) {
            self.words_mut(a)[1 + i] = bits;
            return;
        }
        if v == Value::Ref(0) && self.metadata(h) != 0 {
            // A limpeza de uma posição cortada de uma lista de `E` escalar não
            // anulável (o `length =` que encolhe): zero, sem descompactar.
            self.words_mut(a)[1 + i] = 0;
            return;
        }
        let r = match v {
            Value::Ref(r) => r,
            other_one => self.list_with_roots(&[h], |heap| heap.to_ref(other_one)),
        };
        self.list_with_roots(&[h, r], |heap| heap.list_decompact(h));
        let a = self.list_data(h);
        self.write_ref(a, 1 + i, r);
    }

    /// Passa o armazenamento de `h` à forma geral, com as caixas dos elementos
    /// até o comprimento (§2.16). Numa `_GrowableList`, um armazenamento geral
    /// novo (o velho pode ser compartilhado); numa `_List`/`_ImmutableList`, uma
    /// lista geral temporária com as caixas, copiada para o corpo de `h` depois
    /// de trocar o `flags` — sem alocação entre as duas coisas. Pode coletar.
    fn list_decompact(&mut self, h: Ref) {
        let shape = self.list_shape(h);
        if shape == Element::General {
            return;
        }
        let n = self.list_len(h);
        let growable = self.list_cid(h) == cid::GROWABLE_LIST;
        let size = if growable { self.list_capacity(h) } else { n };
        let general = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, size, Element::General));
        self.list_with_roots(&[h, general], |heap| {
            for i in 0..n {
                let bits = heap.words(heap.list_data(h))[1 + i];
                let r = heap.to_ref(shape.value_of(bits));
                // `general` pode ter envelhecido numa coleta da caixa seguinte.
                heap.write_ref(general, 1 + i, r);
            }
        });
        if growable {
            self.list_set_data(h, general);
            return;
        }
        // Daqui ao fim, nenhuma alocação: o corpo de `h` troca de formato.
        let refs: Vec<i64> = self.words(general)[1..1 + n].to_vec();
        self.set_flags(h, flags::FORMA | flags::ELEMENTO, Element::General.flags());
        self.write_refs(h, 1, &refs);
    }

    /// Passa o armazenamento geral de `h` à forma compacta `e`, no lugar, se todo
    /// elemento até o comprimento cabe nela (null conta como zero). Não aloca.
    fn list_compact(&mut self, h: Ref, e: Element) {
        debug_assert!(e != Element::General && self.list_shape(h) == Element::General);
        let n = self.list_len(h);
        let a = self.list_data(h);
        let cap = self.words(a)[0] as usize;
        let mut bits = Vec::with_capacity(cap);
        for &r in &self.words(a)[1..1 + cap] {
            if r == 0 {
                bits.push(0);
                continue;
            }
            match e.bits_of(self.value(r)) {
                Some(b) => bits.push(b),
                // Um elemento que não cabe dentro do comprimento: fica geral.
                // Além dele (o resto da capacidade), o valor não é
                // observável e vira zero.
                None if bits.len() < n => return,
                None => bits.push(0),
            }
        }
        self.set_flags(a, flags::FORMA | flags::ELEMENTO, e.flags());
        self.words_mut(a)[1..1 + cap].copy_from_slice(&bits);
    }

    /// A cópia de `h` numa lista fixa (`cid`) nova, na mesma forma. Pode coletar.
    fn list_copy_of(&mut self, h: Ref, cid: i32) -> Ref {
        let n = self.list_len(h);
        let shape = self.list_shape(h);
        let new = self.list_with_roots(&[h], |heap| heap.new_list(cid, n, shape));
        let a = self.list_data(h);
        let source: Vec<i64> = self.words(a)[1..1 + n].to_vec();
        // Recém-alocada, jovem: sem barreira.
        self.words_mut(new)[1..1 + n].copy_from_slice(&source);
        new
    }
}

#[cfg(test)]
mod tests_lists {
    use super::*;

    #[test]
    fn lists_flags_and_codes_round_trip() {
        for e in [Element::General, Element::Int, Element::Double, Element::Bool] {
            assert_eq!(Element::from_flags(e.flags()), e);
            assert_eq!(Element::from_flags(e.flags() | flags::CARTOES), e);
            assert_eq!(Element::from_code(e.code()), Some(e));
        }
        assert_eq!(Element::from_code(4), None);
        // O `@df.list_shape` compara `flags & 0x66` com estes valores.
        assert_eq!(Element::Int.flags(), 0x22);
        assert_eq!(Element::Double.flags(), 0x42);
        assert_eq!(Element::Bool.flags(), 0x62);
        assert_eq!(Element::General.flags(), 0x04);
    }

    #[test]
    fn lists_bits_of_each_shape() {
        assert_eq!(Element::Int.bits_of(Value::Int(-3)), Some(-3));
        assert_eq!(Element::Double.bits_of(Value::Double(1.5)), Some(1.5f64.to_bits() as i64));
        assert_eq!(Element::Bool.bits_of(Value::Bool(true)), Some(1));
        assert_eq!(Element::Int.bits_of(Value::Double(1.0)), None);
        assert_eq!(Element::Int.bits_of(Value::Ref(0)), None);
        assert_eq!(Element::Double.value_of(2.5f64.to_bits() as i64), Value::Double(2.5));
        assert_eq!(Element::Bool.value_of(0), Value::Bool(false));
    }
}
