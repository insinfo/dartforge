//! O espaço de objetos (docs/NATIVO-ESPACO-UNIFICADO.md §2.6–§2.8 e §3.2).
//!
//! Todo bloco do heap mora aqui: um [`crate::layout::Header`] de 16 bytes seguido do corpo,
//! em palavras de 8 bytes. As páginas de [`crate::layout::PAGINA`] bytes guardam blocos de uma
//! só **classe de tamanho** (`layout::size_class`: as exatas de 1 a 64
//! palavras e 24 médias até 2014 palavras, cada uma enchendo a página); um corpo
//! maior tem uma **região grande** própria, arredondada a 4 KiB e alinhada a
//! [`crate::layout::PAGINA`] (`VirtualAlloc`/`mmap`), solta já na coleta menor que a acha
//! morta, para um cache por tamanho com teto. As páginas saem de pedaços
//! reservados do sistema e só ocupam memória onde se escreve; as soltas voltam
//! ao sistema salvo um cache quente pequeno (docs/NATIVO-PLANO.md §14).
//!
//! O começo de cada página (e de cada região) é o mapa de marcas: um bit por
//! palavra. A marcação acende o bit do objeto alcançado; as varreduras leem só os
//! bits, sem tocar os blocos, vivos ou mortos (a VM também marca fora do objeto
//! no *old space*, `marking.cc`; aqui o mapa fica junto, por página, como o do
//! Immix). O coletor não move: o endereço de um objeto é fixo enquanto ele vive.
//!
//! O coletor percorre o corpo pelo **formato** em `flags` (§2.3), sem olhar a
//! classe: `INSTANCIA` pelo mapa de referências, `BRUTO` nada, `REFS` as palavras
//! `1..=palavra 0`. As `REFS` grandes têm **cartões** (1 bit por 32 elementos,
//! depois do último elemento): a coleta menor percorre de um lembrado só os
//! elementos dos cartões sujos. Um bloco com **anexo** (`ATTACHMENT`) guarda em `b+16`
//! um ponteiro nativo, solto quando o dono morre.
//!
//! É o par, sem mover objetos, do *new space* da VM
//! (`runtime/vm/heap/scavenger.cc` e a alocação em linha do
//! `stub_code_compiler.cc`): alocar é tirar o próximo bloco da região da classe —
//! pelo código gerado, da TLAB (`Context::tlab`) — sem `malloc`.
#![allow(unsafe_code)]

use crate::layout::{
    PAGE_HEAD, Header, DESLOCAMENTO_DO_HANDLE, ELEMENTOS_POR_CARTAO, ELEMENTOS_POR_PALAVRA_DE_CARTAO,
    LARGEST_CLASS, N_CLASSES, PAGINA, bytes_do_bloco, capacidade, size_class, is_object, class_words,
    palavras_do_mapa,
};

const LIVRE: u8 = crate::layout::estado::LIVRE;
const JOVEM: u8 = crate::layout::estado::JOVEM;
const VELHO: u8 = crate::layout::estado::VELHO;
const REMEMBERED: u8 = crate::layout::estado::REMEMBERED;
const FORA: u8 = crate::layout::flags::FORA;
const FORMA: u8 = crate::layout::flags::FORMA;
const INSTANCIA: u8 = crate::layout::flags::INSTANCIA;
const REFS: u8 = crate::layout::flags::REFS;
const CARTOES: u8 = crate::layout::flags::CARTOES;

/// A classe das regiões grandes nos vetores por classe (o índice 0, que nenhuma
/// classe de tamanho usa).
pub(crate) const LARGE: usize = 0;

/// Granularidade das regiões grandes (a página do sistema).
const REGION_GRANULARITY: usize = 4096;

/// Até quantos blocos cada reabastecimento da TLAB entrega (uma faixa contígua da
/// lista livre: menos, se a contiguidade acaba antes).
pub(crate) const TLAB_BLOCKS: usize = 256;

// ─── Veneno e quarentena (`DARTFORGE_GC_VENENO`) ───────────────────────────

/// O estado de um bloco envenenado: o primeiro byte do padrão de veneno.
/// Nenhum estado válido o usa; a validação de handle o recusa.
pub(crate) const POISON_STATE: u8 = 0xDE;
/// A palavra com que o bloco morto é preenchido: par, e com o primeiro byte
/// (o estado) em [`POISON_STATE`].
const POISON_WORD: u64 = 0xDFDF_DFDF_DFDF_DFDE;

/// A quarentena dos blocos mortos (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §7.5): com `DARTFORGE_GC_VENENO=1` (ou `=<n>` blocos), cada bloco que
/// morre é preenchido com o padrão de veneno e só volta à lista livre depois
/// que outros `n` (1 000 por padrão) morreram depois dele. Um uso depois de
/// liberar lê então o veneno e falha na validação de handle, em vez de ler um
/// objeto novo posto no mesmo lugar. O coletor não move objetos: é o
/// equivalente da quarentena do *from-space* de um coletor de cópia.
struct Quarantine {
    /// (classe, bloco), na ordem em que entraram.
    ring: std::collections::VecDeque<(usize, *mut u8)>,
    /// Os blocos do anel, para a varredura completa não os devolver.
    pinned_set: crate::hash::HashSet<usize>,
    capacidade: usize,
}

impl Quarantine {
    fn from_env() -> Option<Self> {
        let v = std::env::var("DARTFORGE_GC_VENENO").ok()?;
        let capacidade = match v.trim() {
            "" | "0" => return None,
            "1" => 1000,
            n => n.parse::<usize>().ok().filter(|&n| n > 0)?,
        };
        Some(Quarantine { ring: std::collections::VecDeque::new(), pinned_set: crate::hash::HashSet::default(), capacidade })
    }

    /// Envenena o bloco `b` de `size` bytes e o põe no anel.
    ///
    /// # Safety
    /// `b` é bloco sem objeto vivo de uma página do espaço.
    unsafe fn pin(&mut self, class: usize, b: *mut u8, size: usize) {
        let words = b.cast::<u64>();
        for i in 0..size / 8 {
            // SAFETY: o contrato da função; o bloco é alinhado a 8.
            unsafe { words.add(i).write(POISON_WORD) };
        }
        self.ring.push_back((class, b));
        self.pinned_set.insert(b as usize);
    }

    /// Na varredura completa: o bloco `b` não marcado fica (ou entra) na
    /// quarentena? Já no anel, sim; com o estado de um objeto que acabou de
    /// morrer, entra; livre (zerado ou já envenenado e solto), não.
    ///
    /// # Safety
    /// `b` é bloco sem marca de uma página do espaço.
    unsafe fn retain_ref(&mut self, class: usize, b: *mut u8, size: usize) -> bool {
        if self.pinned_set.contains(&(b as usize)) {
            return true;
        }
        // SAFETY: o contrato da função.
        let estado = unsafe { *b };
        if estado == LIVRE || estado == POISON_STATE {
            return false;
        }
        // SAFETY: o contrato da função.
        unsafe { self.pin(class, b, size) };
        true
    }

    /// Os blocos que passaram da capacidade, os mais antigos primeiro: saem
    /// do anel (continuam envenenados até a entrega, que os zera).
    fn released_set(&mut self) -> Vec<(usize, *mut u8)> {
        let mut v = Vec::new();
        while self.ring.len() > self.capacidade {
            let (class, b) = self.ring.pop_front().expect("anel além da capacidade");
            self.pinned_set.remove(&(b as usize));
            v.push((class, b));
        }
        v
    }
}

// ─── Mapa de marcas ────────────────────────────────────────────────────────

/// A palavra e o bit do mapa de marcas do bloco `b`.
#[inline]
pub(crate) fn mark_bit(b: *const Header) -> (*mut u64, u64) {
    let a = b as usize;
    let base = a & !(PAGINA - 1);
    let desl = a - base;
    ((base + (desl >> 9) * 8) as *mut u64, 1u64 << ((desl >> 3) & 63))
}

/// O bloco `b` foi marcado (a coleta completa em curso, ou a menor que o
/// promoveu)?
///
/// # Safety
/// `b` é bloco de uma página do espaço (nunca um estático: o mapa dele cairia
/// fora de uma página).
#[inline]
pub(crate) unsafe fn marked(b: *const Header) -> bool {
    let (w, m) = mark_bit(b);
    // SAFETY: o contrato da função; o mapa está no começo da página.
    unsafe { *w & m != 0 }
}

/// Acende o bit de marca do bloco `b`.
///
/// # Safety
/// `b` é bloco de uma página do espaço.
#[inline]
pub(crate) unsafe fn mark_block(b: *const Header) {
    let (w, m) = mark_bit(b);
    // SAFETY: o contrato da função.
    unsafe { *w |= m };
}

/// Apaga o bit de marca do bloco `b` (o bloco que o ARC puro solta: ao ser
/// entregue de novo como jovem, a marca velha o daria por vivo).
///
/// # Safety
/// `b` é bloco de uma página do espaço.
#[inline]
pub(crate) unsafe fn unmark_block(b: *const Header) {
    let (w, m) = mark_bit(b);
    // SAFETY: o contrato da função.
    unsafe { *w &= !m };
}

// ─── O corpo de um bloco ───────────────────────────────────────────────────

/// Bytes de um bloco (ou corpo de fora) `INSTANCIA` de `n` campos.
#[inline]
pub const fn block_size(n: usize) -> usize {
    bytes_do_bloco(capacidade(n) + palavras_do_mapa(n))
}

/// O formato do corpo do bloco `b` (`flags & FORMA`).
///
/// # Safety
/// `b` aponta um cabeçalho legível.
#[inline]
pub(crate) unsafe fn shape(b: *const Header) -> u8 {
    // SAFETY: o contrato da função.
    unsafe { (*b).flags & FORMA }
}

/// O corpo de um objeto `INSTANCIA` (o próprio bloco, ou o corpo de fora).
///
/// # Safety
/// `b` é o cabeçalho de um bloco vivo.
#[inline]
pub(crate) unsafe fn body(b: *mut Header) -> *mut Header {
    // SAFETY: o contrato da função; com FORA, o primeiro campo é o endereço.
    unsafe {
        if (*b).flags & FORA != 0 { *fields_of(b).cast::<*mut Header>() } else { b }
    }
}

/// As palavras depois de um cabeçalho (bloco ou corpo).
#[inline]
pub(crate) fn fields_of(c: *mut Header) -> *mut i64 {
    c.wrapping_add(1).cast()
}

/// O campo `i` de um corpo `INSTANCIA` é referência?
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
pub(crate) unsafe fn is_reference(c: *const Header, i: usize) -> bool {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            (*c).map >> i & 1 != 0
        } else {
            let ext = fields_of(c.cast_mut()).add(capacidade(usize::from((*c).n))).cast::<u64>();
            *ext.add((i - 32) / 64) >> ((i - 32) % 64) & 1 != 0
        }
    }
}

/// Acende ou apaga o bit de referência do campo `i` de um corpo `INSTANCIA`.
///
/// # Safety
/// `c` é um corpo válido e `i < n`.
#[inline]
pub(crate) unsafe fn mark_reference(c: *mut Header, i: usize, is_ref_kind: bool) {
    // SAFETY: o contrato da função.
    unsafe {
        if i < 32 {
            if is_ref_kind { (*c).map |= 1 << i } else { (*c).map &= !(1 << i) }
        } else {
            let w = fields_of(c).add(capacidade(usize::from((*c).n))).cast::<u64>().add((i - 32) / 64);
            let bit = 1u64 << ((i - 32) % 64);
            if is_ref_kind { *w |= bit } else { *w &= !bit }
        }
    }
}

/// Empilha os campos-referência de um corpo `INSTANCIA`; devolve quantos campos
/// tem. O `map` só é mapa de referências em `INSTANCIA` (§2.3).
///
/// # Safety
/// `c` é um corpo `INSTANCIA` válido.
#[inline]
pub(crate) unsafe fn push_references(c: *const Header, stack: &mut Vec<i64>) -> usize {
    // SAFETY: o contrato da função.
    unsafe {
        debug_assert!(shape(c) == INSTANCIA, "mapa lido como mapa de referências num corpo que não é INSTANCIA");
        let n = usize::from((*c).n);
        let fields = fields_of(c.cast_mut());
        let mut m = (*c).map;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            stack.push(*fields.add(i));
            m &= m - 1;
        }
        if n > 32 {
            let ext = fields.add(capacidade(n)).cast::<u64>();
            for w in 0..palavras_do_mapa(n) {
                let mut m = *ext.add(w);
                while m != 0 {
                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                    stack.push(*fields.add(i));
                    m &= m - 1;
                }
            }
        }
        n
    }
}

/// Quantos `Ref` um corpo `REFS` de `words` palavras guarda: a palavra 0 (o
/// comprimento da `_List`, o número de campos do `_Record`), limitada ao corpo.
///
/// # Safety
/// `b` é um bloco `REFS` válido de `words` palavras de corpo.
#[inline]
pub(crate) unsafe fn body_refs(b: *const Header, words: usize) -> usize {
    // SAFETY: o contrato da função; a palavra 0 existe (`words ≥ 1`).
    let n = unsafe { *fields_of(b.cast_mut()) };
    usize::try_from(n).unwrap_or(0).min(words.saturating_sub(1))
}

/// As palavras de cartões de um corpo `REFS` com `CARTOES` de `len` elementos
/// (depois do último elemento: `b + 24 + 8·len`).
#[inline]
pub(crate) fn cards_of(b: *mut Header, len: usize) -> *mut u64 {
    fields_of(b).wrapping_add(1 + len).cast()
}

/// Suja o cartão do elemento `i` (índice do elemento, a partir de 0) de uma lista
/// com `CARTOES` de `len` elementos.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` e `i < len`.
#[inline]
pub(crate) unsafe fn dirty_card(b: *mut Header, len: usize, i: usize) {
    let p = cards_of(b, len).wrapping_add(i / ELEMENTOS_POR_PALAVRA_DE_CARTAO);
    // SAFETY: o contrato da função.
    unsafe { *p |= 1u64 << ((i / ELEMENTOS_POR_CARTAO) & 63) };
}

/// Suja todos os cartões (a barreira explícita de quem gravou por fora).
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn dirty_all_cards(b: *mut Header, len: usize) {
    let c = cards_of(b, len);
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: o contrato da função.
        unsafe { *c.add(w) = u64::MAX };
    }
}

/// Empilha os `Ref` das palavras `1..=n` de um corpo `REFS`, pulando null e `Smi`
/// (o chamador filtra o resto); devolve `n`.
///
/// # Safety
/// `b` é um bloco `REFS` válido com pelo menos `n + 1` palavras de corpo.
#[inline]
pub(crate) unsafe fn push_refs(b: *const Header, n: usize, stack: &mut Vec<i64>) -> usize {
    let p = fields_of(b.cast_mut());
    for i in 1..=n {
        // SAFETY: o contrato da função.
        let r = unsafe { *p.add(i) };
        if r != 0 && r & 1 == 0 {
            stack.push(r);
        }
    }
    n
}

/// Empilha os `Ref` dos elementos dos cartões sujos de uma lista `REFS` com
/// `CARTOES` de `len` elementos e zera os cartões; devolve quantos elementos
/// percorreu.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn push_dirty_cards(b: *mut Header, len: usize, stack: &mut Vec<i64>) -> usize {
    let c = cards_of(b, len);
    let elements = fields_of(b).wrapping_add(1);
    let mut work = 0;
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: o contrato da função.
        let mut m = unsafe { std::ptr::replace(c.add(w), 0) };
        while m != 0 {
            let k = w * 64 + m.trailing_zeros() as usize;
            let (de, until) = (k * ELEMENTOS_POR_CARTAO, ((k + 1) * ELEMENTOS_POR_CARTAO).min(len));
            for i in de..until {
                // SAFETY: `i < len`.
                let r = unsafe { *elements.add(i) };
                if r != 0 && r & 1 == 0 {
                    stack.push(r);
                }
            }
            work += until.saturating_sub(de);
            m &= m - 1;
        }
    }
    work
}

/// Zera os cartões de uma lista `REFS` com `CARTOES` de `len` elementos.
///
/// # Safety
/// `b` é um bloco `REFS` com `CARTOES` de `len` elementos.
pub(crate) unsafe fn clear_cards(b: *mut Header, len: usize) {
    // SAFETY: o contrato da função.
    unsafe { std::ptr::write_bytes(cards_of(b, len), 0, len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO)) };
}

/// Um corpo de fora de `n` campos zerados (o alocador do sistema).
pub(crate) fn new_out_of_line_body(n: usize) -> *mut Header {
    let layout = std::alloc::Layout::from_size_align(block_size(n), 8).expect("layout do corpo");
    // SAFETY: layout de tamanho não nulo.
    let p = unsafe { std::alloc::alloc_zeroed(layout) }.cast::<Header>();
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    // SAFETY: memória nova, zerada, do tamanho do corpo.
    unsafe {
        (*p).n = u16::try_from(n).expect("objeto com campos demais");
    }
    p
}

/// Solta um corpo de fora.
///
/// # Safety
/// `c` veio de [`new_out_of_line_body`] e ninguém mais o usa.
pub(crate) unsafe fn release_out_of_line_body(c: *mut Header) {
    // SAFETY: o contrato da função.
    unsafe {
        let n = usize::from((*c).n);
        std::alloc::dealloc(c.cast(), std::alloc::Layout::from_size_align(block_size(n), 8).expect("layout do corpo"));
    }
}

/// Bytes de uma região grande de corpo de `words` palavras: o mapa de marcas
/// e o bloco, arredondados a [`REGION_GRANULARITY`].
#[inline]
pub(crate) const fn region_bytes(words: usize) -> usize {
    (PAGE_HEAD + bytes_do_bloco(words)).next_multiple_of(REGION_GRANULARITY)
}

/// Bytes que uma alocação de corpo de `words` palavras ocupa: o bloco da
/// classe, ou a região grande.
#[inline]
pub(crate) const fn allocation_bytes(words: usize) -> usize {
    match size_class(words) {
        Some(c) => bytes_do_bloco(class_words(c)),
        None => region_bytes(words),
    }
}

// ─── Páginas ───────────────────────────────────────────────────────────────

/// Uma página do espaço de objetos (blocos de uma só classe de tamanho) ou uma
/// região grande (um bloco só).
#[derive(Debug)]
pub(crate) struct Page {
    base: *mut u8,
    /// Bytes alocados (`PAGINA`, ou os da região grande).
    bytes: usize,
    /// A classe de tamanho, ou [`LARGE`].
    class: usize,
    /// Palavras do corpo de cada bloco (a da classe; na região grande, a real).
    words: usize,
    blocks: usize,
    /// O inverso de `bytes_do_bloco(palavras) / 8` (ímpar, ou potência de 2
    /// vezes ímpar: ver `ix`) módulo 2³²: o índice de um bloco sai de uma
    /// multiplicação, e um deslocamento que não é múltiplo do tamanho dá um
    /// índice além de `blocks` (o teste de divisibilidade exata, *Hacker's
    /// Delight* 10-17).
    inverse: u32,
    /// `bytes_do_bloco(palavras) / 8` = `impar << deslocamento`.
    offset: u32,
    /// Página sem objeto vivo guardada para reuso (em
    /// [`ObjectSpace::empties_list`]): o conteúdo é lixo dos mortos até ela ser
    /// zerada e formatada de novo, para qualquer classe.
    empty: bool,
}

impl Page {
    /// A página de blocos da classe `class` (corpo de `words` palavras) em
    /// `base` (`bytes` bytes).
    fn formatted(base: *mut u8, bytes: usize, class: usize, words: usize) -> Self {
        let size = bytes_do_bloco(words);
        let blocks = if class == LARGE { 1 } else { (PAGINA - PAGE_HEAD) / size };
        let in_words = u32::try_from(size / 8).unwrap_or(1);
        let offset = in_words.trailing_zeros();
        let inverse = odd_inverse(in_words >> offset);
        Page { base, bytes, class, words, blocks, inverse, offset, empty: false }
    }
    /// O primeiro bloco (depois do mapa de marcas).
    fn start(&self) -> *mut u8 {
        self.base.wrapping_add(PAGE_HEAD)
    }
    fn size(&self) -> usize {
        bytes_do_bloco(self.words)
    }
    fn block(&self, i: usize) -> *mut Header {
        self.start().wrapping_add(i * self.size()).cast()
    }
    /// O índice do bloco que começa `desl` bytes depois da base, se algum
    /// começa ali.
    #[inline]
    fn ix(&self, desl: usize) -> Option<usize> {
        if desl & 7 != 0 || desl >= self.bytes || desl < PAGE_HEAD || self.empty {
            return None;
        }
        let desl = desl - PAGE_HEAD;
        let words = (desl >> 3) as u32;
        let mask = (1u32 << self.offset) - 1;
        if words & mask != 0 {
            return None;
        }
        let i = (words >> self.offset).wrapping_mul(self.inverse) as usize;
        (i < self.blocks).then_some(i)
    }
}

/// O inverso de `a` (ímpar) módulo 2³² (Newton: cada passo dobra os bits certos).
const fn odd_inverse(a: u32) -> u32 {
    let mut x = a;
    let mut i = 0;
    while i < 5 {
        x = x.wrapping_mul(2u32.wrapping_sub(a.wrapping_mul(x)));
        i += 1;
    }
    x
}

/// Base da página (`endereço >> 16`) → índice + 1 em `pages`: tabelas de 2¹⁶
/// posições por região de 4 GiB (quase sempre uma só), como o *pagemap* de um
/// alocador — a validação de um handle sem hash.
#[derive(Default)]
struct PageMap {
    regions: Vec<(usize, Box<[u32]>)>,
}

impl PageMap {
    const BITS: usize = 16;
    #[inline]
    fn key(base: usize) -> (usize, usize) {
        let k = base >> PAGINA.trailing_zeros();
        (k >> Self::BITS, k & ((1 << Self::BITS) - 1))
    }
    #[inline]
    fn get(&self, base: usize) -> Option<usize> {
        let (r, i) = Self::key(base);
        let (_, t) = self.regions.iter().find(|(x, _)| *x == r)?;
        let v = t[i];
        (v != 0).then(|| v as usize - 1)
    }
    fn define(&mut self, base: usize, value: u32) {
        let (r, i) = Self::key(base);
        let pos = match self.regions.iter().position(|(x, _)| *x == r) {
            Some(p) => p,
            None => {
                self.regions.push((r, vec![0u32; 1 << Self::BITS].into_boxed_slice()));
                self.regions.len() - 1
            }
        };
        self.regions[pos].1[i] = value;
    }
    fn insert_entry(&mut self, base: usize, ix: usize) {
        self.define(base, u32::try_from(ix + 1).expect("páginas demais"));
    }
    fn remove_it(&mut self, base: usize) {
        self.define(base, 0);
    }
}

// ─── Memória do sistema ────────────────────────────────────────────────────

/// A reserva das páginas de [`PAGINA`] bytes do espaço de objetos
/// (docs/NATIVO-PLANO.md §14): pedaços de [`PAGES_PER_CHUNK`] páginas
/// reservados do sistema de uma vez, em todas as plataformas, e as páginas
/// soltas pela coleta. A página nova vem zerada do sistema e só entra na
/// memória residente onde o programa escreve (antes, fora do Linux, cada
/// página era um `alloc_zeroed` alinhado a 64 KiB, que no Windows o `HeapAlloc`
/// atende com o dobro dos bytes, todos zerados: 128 KiB residentes por página).
/// A página solta fica suja num cache quente pequeno e o resto volta ao sistema
/// ([`system::discard`]) sem desfazer a reserva — o `gc_free_pages` da Julia.
#[derive(Default)]
struct PageReserve {
    /// Os pedaços reservados `(início, bytes)`.
    chunks: Vec<(*mut u8, usize)>,
    /// Páginas zeradas por usar (nunca usadas, ou devolvidas ao sistema): no
    /// Windows, confirmadas só quando saem ([`system::confirm`]).
    free_list: Vec<*mut u8>,
    /// Páginas soltas pela coleta, ainda com o lixo dos mortos (e na memória
    /// residente): a próxima página sai delas, zerada à mão. A coleta completa
    /// guarda no máximo [`HOT_PAGES`] (as soltas mais recentes) e devolve
    /// ao sistema as demais e as que ficaram sem uso desde a completa anterior
    /// ([`PageReserve::trim_free`]).
    dirty: Vec<*mut u8>,
    /// Quantas das `dirty` (as do fundo) ficaram sem uso desde o último
    /// [`PageReserve::trim_free`].
    stopped: usize,
}

/// Páginas de cada pedaço que a [`PageReserve`] reserva (2 MiB).
const PAGES_PER_CHUNK: usize = 32;

/// Quantas páginas sujas a reserva guarda de uma coleta completa à outra: as
/// que cabem no semiespaço jovem (`YOUNG_LIMIT`, 2 MiB), o que a próxima
/// coleta menor pode pedir. A Julia guarda o intervalo de coleta
/// (`gc_free_pages`, `default_collect_interval`).
const HOT_PAGES: usize = 32;

/// Teto dos bytes do cache das regiões grandes soltas ([`LargeRegions`]).
const LARGE_CACHE_CEILING: usize = 4 * 1024 * 1024;

#[cfg(unix)]
mod mapping {
    unsafe extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
        fn munmap(addr: *mut u8, len: usize) -> i32;
        #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
        fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    }
    /// `MAP_ANONYMOUS`: 0x20 no Linux, 0x1000 no macOS e nos BSD.
    #[cfg(target_os = "linux")]
    const MAP_ANON: i32 = 0x20;
    #[cfg(not(target_os = "linux"))]
    const MAP_ANON: i32 = 0x1000;
    const PROT_RW: i32 = 1 | 2;
    const MAP_PRIVATE: i32 = 2;
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    const MAP_FIXED: i32 = 0x10;
    /// `bytes` novos, zerados, alinhados a `alignment` (potência de 2).
    pub fn map_pages(bytes: usize, alignment: usize) -> *mut u8 {
        let total = bytes + alignment;
        // SAFETY: mapeamento anônimo novo.
        let p = unsafe { mmap(std::ptr::null_mut(), total, PROT_RW, MAP_PRIVATE | MAP_ANON, -1, 0) };
        if p as isize == -1 || p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, alignment).expect("layout"));
        }
        let start = (p as usize).next_multiple_of(alignment);
        let before = start - p as usize;
        let after = total - before - bytes;
        // SAFETY: as sobras do próprio mapeamento, fora de `[start, start + bytes)`.
        unsafe {
            if before > 0 {
                munmap(p, before);
            }
            if after > 0 {
                munmap((start + bytes) as *mut u8, after);
            }
        }
        start as *mut u8
    }
    /// Desfaz o mapeamento de `[p, p + bytes)`.
    ///
    /// # Safety
    /// `[p, p + bytes)` veio de [`map_pages`] e ninguém mais o usa.
    pub unsafe fn unmap(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { munmap(p, bytes) };
    }
    /// Devolve ao sistema a memória de `[p, p + bytes)`, que volta zerada: no
    /// Linux, `madvise(MADV_DONTNEED)`; nos demais Unix (onde o
    /// `MADV_DONTNEED` não garante zero), um mapeamento anônimo novo por cima.
    ///
    /// # Safety
    /// `[p, p + bytes)` é mapeamento anônimo privado sem nada vivo.
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        #[cfg(target_os = "linux")]
        {
            const MADV_DONTNEED: i32 = 4;
            // SAFETY: o contrato da função.
            if unsafe { madvise(p, bytes, MADV_DONTNEED) } == 0 {
                return;
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // SAFETY: o contrato da função; o mapeamento novo substitui o
            // trecho no mesmo endereço.
            let q = unsafe { mmap(p, bytes, PROT_RW, MAP_PRIVATE | MAP_ANON | MAP_FIXED, -1, 0) };
            if q == p {
                return;
            }
        }
        // Sem o descarte a memória não volta zerada: zera à mão.
        // SAFETY: o contrato da função.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

/// A memória do sistema: as regiões grandes (reservadas e confirmadas de uma
/// vez, zeradas, alinhadas a [`PAGINA`], com a granularidade de página do
/// sistema — um objeto de 20 KiB ocupa 24 KiB; o resto do endereço alinhado
/// não é confirmado) e os pedaços de páginas da [`PageReserve`]
/// (reservados, confirmados página a página no Windows).
mod system {
    use super::PAGINA;

    #[cfg(windows)]
    unsafe extern "system" {
        fn VirtualAlloc(address: *mut u8, bytes: usize, tipo: u32, protection: u32) -> *mut u8;
        fn VirtualFree(address: *mut u8, bytes: usize, tipo: u32) -> i32;
    }
    #[cfg(windows)]
    const MEM_COMMIT: u32 = 0x1000;
    #[cfg(windows)]
    const MEM_RESERVE: u32 = 0x2000;
    #[cfg(windows)]
    const MEM_DECOMMIT: u32 = 0x4000;
    #[cfg(windows)]
    const MEM_RELEASE: u32 = 0x8000;
    #[cfg(windows)]
    const PAGE_READWRITE: u32 = 0x04;

    /// `VirtualAlloc` de `bytes` com `tipo`; aborta sem memória.
    #[cfg(windows)]
    fn virtual_alloc(address: *mut u8, bytes: usize, tipo: u32) -> *mut u8 {
        // SAFETY: reserva nova (`address` nulo) ou confirmação dentro de uma
        // reserva deste módulo.
        let p = unsafe { VirtualAlloc(address, bytes, tipo, PAGE_READWRITE) };
        if p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout"));
        }
        p
    }

    /// `bytes` zerados alinhados a [`PAGINA`], confirmados.
    #[cfg(windows)]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        // A granularidade de alocação do Windows (64 KiB) dá o alinhamento
        // de uma página do espaço.
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE | MEM_COMMIT);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc sem o alinhamento de 64 KiB");
        p
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reserve_space`] ou de [`reserve_chunk`] e ninguém mais o usa.
    #[cfg(windows)]
    pub unsafe fn free(p: *mut u8, _bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { VirtualFree(p, 0, MEM_RELEASE) };
    }
    /// Um pedaço de `bytes` alinhado a [`PAGINA`] para páginas: no Windows só
    /// reservado ([`confirm`] cada página antes do uso).
    #[cfg(windows)]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc sem o alinhamento de 64 KiB");
        p
    }
    /// Confirma `[p, p + bytes)` de um pedaço (zerado se não estava
    /// confirmado; confirmar o já confirmado não muda nada).
    #[cfg(windows)]
    pub fn confirm(p: *mut u8, bytes: usize) {
        virtual_alloc(p, bytes, MEM_COMMIT);
    }
    /// Devolve ao sistema a memória de `[p, p + bytes)` de um pedaço (sai da
    /// residente e da confirmada; volta zerada no próximo [`confirm`]).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(windows)]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { VirtualFree(p, bytes, MEM_DECOMMIT) };
    }

    /// `bytes` zerados alinhados a [`PAGINA`].
    #[cfg(unix)]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        super::mapping::map_pages(bytes, PAGINA)
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reserve_space`] ou de [`reserve_chunk`] e ninguém mais o usa.
    #[cfg(unix)]
    pub unsafe fn free(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { super::mapping::unmap(p, bytes) };
    }
    /// Um pedaço de `bytes` alinhado a [`PAGINA`] para páginas (o `mmap` só
    /// ocupa memória onde se escreve).
    #[cfg(unix)]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        super::mapping::map_pages(bytes, PAGINA)
    }
    /// Nada a confirmar no Unix.
    #[cfg(unix)]
    pub fn confirm(_p: *mut u8, _bytes: usize) {}
    /// Devolve ao sistema a memória de `[p, p + bytes)` de um pedaço (volta
    /// zerada).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(unix)]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { super::mapping::discard(p, bytes) };
    }

    /// `bytes` zerados alinhados a [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout da região");
        // SAFETY: layout de tamanho não nulo.
        let p = unsafe { std::alloc::alloc_zeroed(layout) };
        if p.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        p
    }
    /// Devolve `[p, p + bytes)` ao sistema.
    ///
    /// # Safety
    /// Veio de [`reserve_space`] ou de [`reserve_chunk`] e ninguém mais o usa.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn free(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { std::alloc::dealloc(p, std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout da região")) };
    }
    /// Um pedaço de `bytes` zerados alinhado a [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        reserve_space(bytes)
    }
    /// Nada a confirmar.
    #[cfg(not(any(windows, unix)))]
    pub fn confirm(_p: *mut u8, _bytes: usize) {}
    /// Zera `[p, p + bytes)` (sem como devolver ao sistema).
    ///
    /// # Safety
    /// `[p, p + bytes)` é de um pedaço, sem nada vivo.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: o contrato da função.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

impl PageReserve {
    /// Uma página zerada de [`PAGINA`] bytes, alinhada a [`PAGINA`].
    fn request_fn(&mut self) -> *mut u8 {
        if let Some(p) = self.dirty.pop() {
            self.stopped = self.stopped.min(self.dirty.len());
            // SAFETY: página da reserva, sem uso.
            unsafe { std::ptr::write_bytes(p, 0, PAGINA) };
            return p;
        }
        if self.free_list.is_empty() {
            let bytes = PAGINA * PAGES_PER_CHUNK;
            let start = system::reserve_chunk(bytes);
            self.chunks.push((start, bytes));
            // As de endereço baixo saem primeiro (`pop`).
            for i in (0..PAGES_PER_CHUNK).rev() {
                self.free_list.push(start.wrapping_add(i * PAGINA));
            }
        }
        let p = self.free_list.pop().expect("pedaço novo sem páginas");
        system::confirm(p, PAGINA);
        p
    }
    /// Devolve a página `p` (sem objeto vivo) à reserva, suja.
    fn give_back(&mut self, p: *mut u8) {
        self.dirty.push(p);
    }

    /// A cada coleta completa: as sujas paradas desde a chamada anterior, e as
    /// que passam de [`HOT_PAGES`] (as soltas há mais tempo), voltam ao
    /// sistema ([`system::discard`]: saem da memória residente e voltam
    /// zeradas).
    fn trim_free(&mut self) {
        let k = self.stopped.max(self.dirty.len().saturating_sub(HOT_PAGES)).min(self.dirty.len());
        for p in self.dirty.drain(..k) {
            // SAFETY: página de um pedaço, sem uso.
            unsafe { system::discard(p, PAGINA) };
            self.free_list.push(p);
        }
        self.stopped = self.dirty.len();
    }
}

impl Drop for PageReserve {
    fn drop(&mut self) {
        for &(p, bytes) in &self.chunks {
            // SAFETY: o pedaço veio de `reserve_chunk`; o espaço que o usava
            // acabou.
            unsafe { system::free(p, bytes) };
        }
    }
}

/// O cache das regiões grandes soltas, por tamanho: um programa que refaz um
/// objeto grande a cada rodada (o buffer, a lista) reusa a região em vez de
/// pedi-la e devolvê-la ao sistema a cada vez. Tem teto
/// ([`LARGE_CACHE_CEILING`]; as mais antigas saem primeiro), e as que
/// ficam sem uso de uma coleta completa à outra voltam ao sistema: a VM não
/// guarda página grande nenhuma (`CanUseCache`, `page.cc`), e a região morta
/// na coleta menor vem para cá na hora (docs/NATIVO-PLANO.md §14).
#[derive(Default)]
struct LargeRegions {
    /// `(bytes, início)`, as mais antigas primeiro.
    released_list: Vec<(usize, *mut u8)>,
    /// Quantas das `released_list` (as do começo) ficaram sem uso desde o último
    /// [`LargeRegions::trim_free`].
    stopped: usize,
    /// Bytes das `released_list`.
    bytes: usize,
}

impl LargeRegions {
    /// Uma região zerada de `bytes` bytes, alinhada a [`PAGINA`].
    fn request_fn(&mut self, bytes: usize) -> *mut u8 {
        if let Some(i) = self.released_list.iter().rposition(|&(b, _)| b == bytes) {
            let (_, p) = self.released_list.remove(i);
            if i < self.stopped {
                self.stopped -= 1;
            }
            self.bytes -= bytes;
            // SAFETY: região do cache, sem uso.
            unsafe { std::ptr::write_bytes(p, 0, bytes) };
            return p;
        }
        system::reserve_space(bytes)
    }
    /// Guarda a região `p` de `bytes` bytes (sem objeto vivo); acima do teto,
    /// as mais antigas voltam ao sistema (a que sozinha passa dele, direto).
    fn give_back(&mut self, p: *mut u8, bytes: usize) {
        if bytes > LARGE_CACHE_CEILING {
            // SAFETY: região sem objeto vivo.
            unsafe { system::free(p, bytes) };
            return;
        }
        self.released_list.push((bytes, p));
        self.bytes += bytes;
        let mut k = 0;
        while self.bytes > LARGE_CACHE_CEILING {
            let (b, q) = self.released_list[k];
            self.bytes -= b;
            // SAFETY: região do cache, sem uso.
            unsafe { system::free(q, b) };
            k += 1;
        }
        if k > 0 {
            self.released_list.drain(..k);
            self.stopped = self.stopped.saturating_sub(k);
        }
    }
    /// As soltas sem uso desde a chamada anterior voltam ao sistema.
    fn trim_free(&mut self) {
        let k = self.stopped.min(self.released_list.len());
        for (bytes, p) in self.released_list.drain(..k) {
            self.bytes -= bytes;
            // SAFETY: região do cache, sem uso.
            unsafe { system::free(p, bytes) };
        }
        self.stopped = self.released_list.len();
    }
}

impl Drop for LargeRegions {
    fn drop(&mut self) {
        for &(bytes, p) in &self.released_list {
            // SAFETY: região do cache, sem uso; o espaço acabou.
            unsafe { system::free(p, bytes) };
        }
    }
}

/// Um anexo nativo (`ATTACHMENT`, §2.8): o dono, a função que solta o ponteiro, o
/// ponteiro e os bytes que ele ocupa fora do heap.
#[derive(Clone, Copy)]
pub(crate) struct NativeAttachment {
    pub block: *mut Header,
    pub release: unsafe fn(*mut u8),
    pub ptr: *mut u8,
    pub bytes: usize,
}

// ─── O espaço ──────────────────────────────────────────────────────────────

/// O espaço de objetos: páginas de blocos por classe de tamanho, regiões grandes
/// e as faixas livres (trechos contíguos de blocos zerados) por classe, refeitas
/// a cada coleta completa (em ordem de endereço).
pub struct ObjectSpace {
    pub(crate) pages: Vec<Page>,
    /// A página de cada base (a validação de um handle).
    map: PageMap,
    /// As faixas `[início, end)` de blocos livres de cada classe, zeradas
    /// (cabeçalho inclusive) ou com o lixo dos mortos (zeradas ao sair).
    free_list: Vec<Vec<(*mut u8, *mut u8)>>,
    /// Blocos vivos (entregues e não devolvidos).
    pub live_set: usize,
    /// Blocos em todas as páginas.
    pub blocks: usize,
    /// Os blocos entregues um a um pelo runtime desde a última coleta, com a
    /// classe ([`LARGE`] nas regiões grandes): com
    /// [`ObjectSpace::ranges`], o que a coleta menor varre.
    young_set: Vec<(*mut Header, usize)>,
    /// As faixas `(início, end, class)` entregues às TLABs desde a última
    /// coleta.
    ranges: Vec<(*mut u8, *mut u8, usize)>,
    /// A região ainda não usada `[cursor, end)` da página em uso de cada classe
    /// (o *bump pointer*).
    region: Vec<(*mut u8, *mut u8)>,
    /// As páginas sem objeto vivo guardadas pela coleta completa (índices em
    /// `pages`), por zerar.
    empties_list: Vec<usize>,
    /// Os velhos que a barreira de escrita marcou ([`REMEMBERED`]).
    pub(crate) remembered: Vec<*mut Header>,
    /// Blocos entregues desde a última coleta, por classe.
    demand: Vec<usize>,
    /// O pico recente de blocos ocupados de cada classe, que decai a cada coleta
    /// completa.
    peak: Vec<usize>,
    /// Os objetos com corpo de fora ([`FORA`]).
    pub(crate) with_out_of_line: Vec<*mut Header>,
    /// A região de cada classe veio de uma faixa livre (lixo dos mortos) ou de
    /// uma página zerada.
    dirty_region: Vec<bool>,
    /// Blocos entregues e não soltos de cada classe.
    in_use: Vec<usize>,
    /// Zerar os mortos já na varredura (`--gc-stress` e a verificação).
    pub(crate) clear_dead: bool,
    /// Blocos entregues desde a última coleta (menos os devolvidos).
    handed_out: usize,
    /// Objetos do espaço marcados pela coleta em curso.
    pub(crate) marked_set: usize,
    /// A quarentena dos mortos (`DARTFORGE_GC_VENENO`); `None` sem ela.
    quarantine: Option<Quarantine>,
    /// De onde vêm as páginas de [`PAGINA`] bytes.
    reserve: PageReserve,
    /// O cache das regiões grandes soltas.
    large_ones: LargeRegions,
    /// Os anexos nativos dos blocos vivos (ou mortos desde a última coleta),
    /// pelo endereço do bloco.
    attachments: crate::hash::HashMap<usize, NativeAttachment>,
    /// Bytes dos anexos.
    pub(crate) attachment_bytes: usize,
}

impl std::fmt::Debug for ObjectSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EspacoDeObjetos").field("paginas", &self.pages.len()).field("vivos", &self.live_set).finish()
    }
}

impl ObjectSpace {
    pub(crate) fn new(clear_dead: bool) -> Self {
        Self {
            pages: Vec::new(),
            map: PageMap::default(),
            free_list: vec![Vec::new(); N_CLASSES],
            live_set: 0,
            blocks: 0,
            young_set: Vec::new(),
            ranges: Vec::new(),
            region: vec![(std::ptr::null_mut(), std::ptr::null_mut()); N_CLASSES],
            empties_list: Vec::new(),
            remembered: Vec::new(),
            demand: vec![0; N_CLASSES],
            peak: vec![0; N_CLASSES],
            with_out_of_line: Vec::new(),
            dirty_region: vec![false; N_CLASSES],
            in_use: vec![0; N_CLASSES],
            clear_dead,
            handed_out: 0,
            marked_set: 0,
            quarantine: Quarantine::from_env(),
            reserve: PageReserve::default(),
            large_ones: LargeRegions::default(),
            attachments: crate::hash::HashMap::default(),
            attachment_bytes: 0,
        }
    }

    /// Uma página zerada para blocos da classe `class` (de uma página): uma
    /// vazia reusada (zerada e formatada agora) ou uma nova. Devolve o índice.
    fn zeroed_page(&mut self, class: usize) -> usize {
        debug_assert!(class != LARGE);
        let words = class_words(class);
        if let Some(i) = self.empties_list.pop() {
            let p = &mut self.pages[i];
            // SAFETY: a página inteira é deste espaço e não tem objeto vivo.
            unsafe { std::ptr::write_bytes(p.base, 0, p.bytes) };
            self.blocks -= p.blocks;
            *p = Page::formatted(p.base, p.bytes, class, words);
            self.blocks += p.blocks;
            return i;
        }
        let base = self.reserve.request_fn();
        self.new_page(Page::formatted(base, PAGINA, class, words))
    }

    /// Uma região grande zerada para um corpo de `words` palavras. Devolve o
    /// índice.
    fn large_page(&mut self, words: usize) -> usize {
        let bytes = region_bytes(words);
        let base = self.large_ones.request_fn(bytes);
        self.new_page(Page::formatted(base, bytes, LARGE, words))
    }

    fn new_page(&mut self, page: Page) -> usize {
        self.blocks += page.blocks;
        self.map.insert_entry(page.base as usize, self.pages.len());
        self.pages.push(page);
        self.pages.len() - 1
    }

    /// Tira a página `i` de `pages` (a última toma o lugar dela) e do mapa de
    /// páginas, sem soltá-la: devolve-a.
    fn remove_page(&mut self, i: usize) -> Page {
        let p = self.pages.swap_remove(i);
        self.map.remove_it(p.base as usize);
        self.blocks -= p.blocks;
        let old_value = self.pages.len();
        if i < old_value {
            // A que estava em `old_value` agora está em `i`.
            self.map.insert_entry(self.pages[i].base as usize, i);
            if let Some(v) = self.empties_list.iter_mut().find(|v| **v == old_value) {
                *v = i;
            }
        }
        p
    }

    /// Devolve a página `p` (sem objeto vivo): à reserva, ou ao cache das
    /// regiões grandes.
    fn release_page(&mut self, p: &Page) {
        if p.class == LARGE {
            self.large_ones.give_back(p.base, p.bytes);
        } else {
            self.reserve.give_back(p.base);
        }
    }

    /// Até `k` blocos zerados e contíguos da classe `class` tirados da região
    /// dela; quando ela acaba, a região passa a ser uma faixa livre da classe ou
    /// uma página zerada. Devolve o início e quantos. O cabeçalho sai zerado.
    ///
    /// As faixas livres guardam o lixo dos mortos (a varredura não os toca): o
    /// trecho entregue é zerado aqui, logo antes de o programa o usar.
    fn take_from_region(&mut self, class: usize, k: usize) -> (*mut u8, usize) {
        let size = bytes_do_bloco(class_words(class));
        let (mut cursor, mut end) = self.region[class];
        if (end as usize).saturating_sub(cursor as usize) < size {
            (cursor, end) = match self.free_list[class].pop() {
                Some(range) => {
                    self.dirty_region[class] = true;
                    range
                }
                None => {
                    let i = self.zeroed_page(class);
                    self.dirty_region[class] = false;
                    let p = &self.pages[i];
                    (p.start(), p.start().wrapping_add(p.blocks * size))
                }
            };
        }
        let j = k.min((end as usize - cursor as usize) / size);
        self.region[class] = (cursor.wrapping_add(j * size), end);
        if self.dirty_region[class] {
            // SAFETY: blocos livres da classe, sem objeto vivo.
            unsafe { std::ptr::write_bytes(cursor, 0, j * size) };
        }
        (cursor, j)
    }

    /// Uma faixa de até `k` blocos livres, zerados e contíguos da classe
    /// `class` para a TLAB: o início e quantos.
    pub(crate) fn take_range(&mut self, class: usize, k: usize) -> (*mut u8, usize) {
        debug_assert!(class != LARGE && class <= LARGEST_CLASS && k > 0);
        let (start, j) = self.take_from_region(class, k);
        self.ranges.push((start, start.wrapping_add(j * bytes_do_bloco(class_words(class))), class));
        self.demand[class] += j;
        self.handed_out += j;
        self.in_use[class] += j;
        (start, j)
    }

    /// Devolve os blocos zerados `[start, end)` da classe (o resto não usado da
    /// última faixa da TLAB): de volta à região, se ela continua dali; senão, às
    /// faixas livres. A faixa entregue encolhe junto.
    pub(crate) fn return_range(&mut self, class: usize, start: *mut u8, end: *mut u8) {
        let k = (end as usize - start as usize) / bytes_do_bloco(class_words(class));
        self.handed_out -= k;
        self.in_use[class] -= k;
        if let Some(f) = self.ranges.iter_mut().rev().find(|f| f.2 == class && f.1 == end) {
            f.1 = start;
        }
        if self.region[class].0 == end {
            self.region[class].0 = start;
        } else {
            self.free_list[class].push((start, end));
        }
    }

    /// Devolve às faixas livres os blocos mortos `[start, end)` da classe, sem
    /// tocá-los — no `--gc-stress` e com a verificação, zerados já (o estado
    /// [`LIVRE`] deixa o handle velho ser apanhado, N4).
    ///
    /// # Safety
    /// `[start, end)` são blocos da classe de uma página deste espaço, sem
    /// objeto vivo.
    unsafe fn release_range(&mut self, class: usize, start: *mut u8, end: *mut u8) {
        // Com o veneno, cada bloco entra na quarentena; volta à lista livre
        // o que passou da capacidade dela.
        if let Some(q) = self.quarantine.as_mut() {
            let size = bytes_do_bloco(class_words(class));
            let mut b = start;
            while b < end {
                // SAFETY: o contrato da função.
                unsafe { q.pin(class, b, size) };
                b = b.wrapping_add(size);
            }
            for (c, b) in q.released_set() {
                let t = bytes_do_bloco(class_words(c));
                self.free_list[c].push((b, b.wrapping_add(t)));
            }
            return;
        }
        if self.clear_dead {
            // SAFETY: o contrato da função.
            unsafe { std::ptr::write_bytes(start, 0, end as usize - start as usize) };
        }
        self.insert_free_range(class, start, end);
    }

    /// Junta vizinhos na ponta da lista antes de publicar a faixa. Cada
    /// faixa retirada entra no intervalo final; não ordena nem percorre os
    /// demais livres. Nunca junta classes ou páginas diferentes.
    fn insert_free_range(&mut self, class: usize, mut start: *mut u8, mut end: *mut u8) {
        let page = start as usize & !(PAGINA - 1);
        let free_list = &mut self.free_list[class];
        while let Some(&(a, b)) = free_list.last() {
            if a as usize & !(PAGINA - 1) != page {
                break;
            }
            if b == start {
                start = a;
            } else if end == a {
                end = b;
            } else {
                break;
            }
            free_list.pop();
        }
        free_list.push((start, end));
    }

    /// Marca o velho `b` como [`REMEMBERED`] (a barreira de escrita).
    #[inline]
    pub(crate) fn remember(&mut self, b: *mut Header) {
        // SAFETY: bloco do espaço.
        unsafe {
            if (*b).estado == VELHO {
                (*b).estado = REMEMBERED;
                self.remembered.push(b);
            }
        }
    }

    /// Solta os corpos de fora e os anexos dos objetos que a coleta não marcou
    /// (o bit no mapa de marcas da página): devolve os bytes soltos.
    fn release_dead_out_of_line(&mut self) -> usize {
        let mut released_set = 0;
        self.with_out_of_line.retain(|&b| {
            // SAFETY: bloco com corpo de fora, de uma página viva.
            unsafe {
                if marked(b) {
                    return true;
                }
                released_set += block_size(usize::from((*body(b)).n));
                Self::release_body(b);
            }
            false
        });
        let mut of_attachments = 0;
        self.attachments.retain(|_, a| {
            // SAFETY: bloco com anexo, de uma página viva.
            if unsafe { marked(a.block) } {
                return true;
            }
            // SAFETY: o contrato de `Heap::attach`: `release(ptr)` é a única
            // liberação de `ptr`, e o dono morreu.
            unsafe { (a.release)(a.ptr) };
            of_attachments += a.bytes;
            false
        });
        self.attachment_bytes -= of_attachments;
        released_set + of_attachments
    }

    /// Bytes dos corpos de fora dos objetos vivos.
    fn out_of_line_bytes(&self) -> usize {
        // SAFETY: blocos com corpo de fora, vivos.
        self.with_out_of_line.iter().map(|&b| unsafe { block_size(usize::from((*body(b)).n)) }).sum()
    }

    /// Anexa `ptr` ao bloco `b` (ver `Heap::attach`).
    pub(crate) fn attach(&mut self, b: *mut Header, release: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize) {
        self.attachments.insert(b as usize, NativeAttachment { block: b, release, ptr, bytes });
        self.attachment_bytes += bytes;
    }

    /// Os bytes do anexo de `b` passam a ser `bytes` (o anexo cresceu ou
    /// encolheu); devolve a diferença.
    pub(crate) fn adjust_attachment(&mut self, b: *mut Header, bytes: usize) -> isize {
        let a = self.attachments.get_mut(&(b as usize)).expect("bug do runtime: ajustar_anexo sem anexo");
        let delta = bytes as isize - a.bytes as isize;
        a.bytes = bytes;
        self.attachment_bytes = (self.attachment_bytes as isize + delta) as usize;
        delta
    }

    /// A varredura da coleta menor, só pelo mapa de marcas (a marcação acendeu o
    /// bit dos jovens alcançados, já velhos): cada trecho de jovens sem bit numa
    /// faixa entregue volta às faixas livres sem ser tocado; a região grande de
    /// um jovem morto volta ao cache delas. Os lembrados voltam a velhos.
    /// Devolve (mortos, bytes soltos).
    /// Os jovens que a marcação menor em curso alcançou (os promovidos): os
    /// entregues um a um e os blocos marcados das faixas das TLABs. Antes de
    /// [`ObjectSpace::sweep_young`] (o ARC os registra,
    /// docs/ARC-IMPLEMENTACAO.md).
    pub(crate) fn young_marked(&self, f: &mut dyn FnMut(*mut Header)) {
        for &(b, _) in &self.young_set {
            // SAFETY: bloco entregue desde a última coleta, numa página viva.
            if unsafe { marked(b) } {
                f(b);
            }
        }
        for &(start, end, class) in &self.ranges {
            let size = bytes_do_bloco(class_words(class));
            let mut p = start;
            while p < end {
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { marked(p.cast()) } {
                    f(p.cast());
                }
                p = p.wrapping_add(size);
            }
        }
    }

    /// Visita cada bloco entregue desde a última coleta e ainda ocupado
    /// (os um a um e os das faixas das TLABs; um bloco da faixa que a TLAB
    /// não chegou a entregar está zerado, [`LIVRE`]).
    pub(crate) fn young_handed_out(&self, f: &mut dyn FnMut(*mut Header)) {
        for &(b, _) in &self.young_set {
            // SAFETY: bloco entregue desde a última coleta, numa página viva.
            if unsafe { (*b).estado } == JOVEM {
                f(b);
            }
        }
        for &(start, end, class) in &self.ranges {
            let size = bytes_do_bloco(class_words(class));
            let mut p = start;
            while p < end {
                let b: *mut Header = p.cast();
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { (*b).estado } == JOVEM {
                    f(b);
                }
                p = p.wrapping_add(size);
            }
        }
    }

    /// Solta o bloco `b`, morto pelo RC (o ARC puro, fora de qualquer
    /// varredura): o corpo de fora, o anexo, o bit de marca e o bloco, que
    /// volta às faixas livres da classe (o objeto grande devolve a página).
    /// Devolve os bytes soltos.
    ///
    /// # Safety
    /// `b` é bloco ocupado de uma página deste espaço, fora da lista dos
    /// jovens, cujo objeto morreu e que ninguém mais lê.
    pub(crate) unsafe fn release_dead(&mut self, b: *mut Header) -> usize {
        let mut released_set = 0;
        // SAFETY: o contrato da função.
        unsafe {
            if (*b).flags & FORA != 0 {
                released_set += block_size(usize::from((*body(b)).n));
                self.with_out_of_line.retain(|&x| x != b);
                Self::release_body(b);
            }
            if let Some(a) = self.attachments.remove(&(b as usize)) {
                (a.release)(a.ptr);
                self.attachment_bytes -= a.bytes;
                released_set += a.bytes;
            }
            unmark_block(b);
        }
        let Some(i) = self.map.get(b as usize & !(PAGINA - 1)) else { return released_set };
        let class = self.pages[i].class;
        self.live_set -= 1;
        if class == LARGE {
            let p = self.remove_page(i);
            released_set += p.bytes;
            self.large_ones.give_back(p.base, p.bytes);
        } else {
            let size = bytes_do_bloco(class_words(class));
            released_set += size;
            self.in_use[class] -= 1;
            let start = b.cast::<u8>();
            // SAFETY: o bloco morto da classe, sem objeto vivo.
            unsafe { self.release_range(class, start, start.wrapping_add(size)) };
        }
        released_set
    }

    /// A geometria da página do bloco do handle `h`: o endereço do primeiro
    /// bloco, os bytes de cada um e quantos cabem (a tabela de metadados do
    /// ARC por página, `arc::Geometry`). `None` fora do espaço.
    pub(crate) fn geometry(&self, h: i64) -> Option<(u64, u64, u32)> {
        let a = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        let i = self.map.get(a & !(PAGINA - 1))?;
        let p = &self.pages[i];
        if p.empty {
            return None;
        }
        Some((p.start() as u64, p.size() as u64, u32::try_from(p.blocks).ok()?))
    }

    pub(crate) fn sweep_young(&mut self) -> (usize, usize) {
        let (mut dead_set, mut released_set) = (0, 0);
        released_set += self.release_dead_out_of_line();
        let young_set = std::mem::take(&mut self.young_set);
        for &(b, class) in &young_set {
            // SAFETY: bloco entregue desde a última coleta, de uma página
            // ainda viva (as páginas só saem na coleta completa).
            unsafe {
                if marked(b) {
                    continue;
                }
                dead_set += 1;
                if class == LARGE {
                    // A região do objeto grande morto volta ao cache delas já
                    // (a varredura dos grandes da Julia, `sweep_big`): até a
                    // coleta completa, ela seria memória que nenhum gatilho
                    // conta (docs/NATIVO-PLANO.md §14.2).
                    if let Some(i) = self.map.get(b as usize & !(PAGINA - 1)) {
                        let p = self.remove_page(i);
                        released_set += p.bytes;
                        self.large_ones.give_back(p.base, p.bytes);
                    }
                } else {
                    let size = bytes_do_bloco(class_words(class));
                    released_set += size;
                    self.in_use[class] -= 1;
                    let start = b.cast::<u8>();
                    self.release_range(class, start, start.wrapping_add(size));
                }
            }
        }
        self.young_set = young_set;
        self.young_set.clear();
        let ranges = std::mem::take(&mut self.ranges);
        // Todos os entregues marcados: nenhum morto nas faixas.
        let all_live = self.marked_set >= self.handed_out;
        for &(start, end, class) in if all_live { &ranges[..0] } else { &ranges[..] } {
            let size = bytes_do_bloco(class_words(class));
            let mut chunk: *mut u8 = std::ptr::null_mut();
            let mut dead_in_range = 0;
            let mut p = start;
            while p < end {
                // SAFETY: bloco da faixa, numa página viva.
                if unsafe { marked(p.cast()) } {
                    if !chunk.is_null() {
                        // SAFETY: blocos mortos da faixa.
                        unsafe { self.release_range(class, chunk, p) };
                        chunk = std::ptr::null_mut();
                    }
                } else {
                    dead_in_range += 1;
                    if chunk.is_null() {
                        chunk = p;
                    }
                }
                p = p.wrapping_add(size);
            }
            if !chunk.is_null() {
                // SAFETY: blocos mortos da faixa.
                unsafe { self.release_range(class, chunk, end) };
            }
            dead_set += dead_in_range;
            released_set += dead_in_range * size;
            self.in_use[class] -= dead_in_range;
        }
        self.ranges = ranges;
        self.ranges.clear();
        for b in std::mem::take(&mut self.remembered) {
            // SAFETY: bloco lembrado, velho.
            unsafe { (*b).estado = VELHO };
        }
        self.live_set -= dead_set;
        self.handed_out = 0;
        self.marked_set = 0;
        for d in self.demand.iter_mut() {
            *d = 0;
        }
        (dead_set, released_set)
    }

    /// Solta o corpo de fora do objeto morto do bloco `b`, se ele tem um.
    ///
    /// # Safety
    /// `b` é bloco deste espaço cujo objeto morreu.
    #[inline]
    unsafe fn release_body(b: *mut Header) {
        // SAFETY: o contrato da função.
        unsafe {
            if (*b).flags & FORA != 0 {
                release_out_of_line_body(body(b));
                (*b).flags &= !FORA;
            }
        }
    }

    /// Um bloco novo de corpo de `words` palavras zeradas, classe `cid`,
    /// `flags` e `n` no cabeçalho (saturado em `u16::MAX`), estado `JOVEM`: o
    /// handle.
    #[inline]
    pub(crate) fn allocate(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> i64 {
        let (b, class) = match size_class(words) {
            Some(c) => {
                self.in_use[c] += 1;
                self.demand[c] += 1;
                (self.take_from_region(c, 1).0.cast::<Header>(), c)
            }
            None => {
                let i = self.large_page(words);
                (self.pages[i].start().cast::<Header>(), LARGE)
            }
        };
        self.young_set.push((b, class));
        self.handed_out += 1;
        // SAFETY: bloco livre tirado agora, zerado.
        unsafe {
            (*b).estado = JOVEM;
            (*b).flags = flags;
            (*b).n = u16::try_from(n).unwrap_or(u16::MAX);
            (*b).class_id = cid;
            // Objeto grande que não é string nem `INSTANCIA` (`map` sem uso,
            // §2.3): o `map` guarda as palavras do corpo, para
            // `body_words` não ir ao mapa de páginas a cada leitura.
            if n > usize::from(u16::MAX) && flags & crate::layout::flags::FORMA != INSTANCIA && !crate::layout::cid::is_string(cid) {
                (*b).map = u32::try_from(words).unwrap_or(0);
            }
        }
        self.live_set += 1;
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    /// O bloco do handle `h`, se `h` é o de um bloco de alguma página deste
    /// espaço (vivo ou não: o estado fica com quem pergunta).
    #[inline]
    pub fn block_of(&self, h: i64) -> Option<*mut Header> {
        if !is_object(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        let base = b & !(PAGINA - 1);
        let p = &self.pages[self.map.get(base)?];
        p.ix(b - base).map(|i| p.block(i))
    }

    /// O handle aponta para um bloco ocupado, mesmo sem zerar cabeçalhos mortos.
    /// Consulta faixas livres e região ainda não entregue antes do cabeçalho.
    pub(crate) fn occupied_block(&self, h: i64) -> bool {
        let Some(b) = self.block_of(h) else { return false; };
        let address = b as usize;
        let page = &self.pages[self.map.get(address & !(PAGINA - 1)).unwrap()];
        if page.class != LARGE {
            let c = page.class;
            if self.free_list[c].iter().any(|&(start, end)|
                (start as usize) <= address && address < end as usize) {
                return false;
            }
            let (start, end) = self.region[c];
            if (start as usize) <= address && address < end as usize {
                return false;
            }
        }
        // SAFETY: bloco localizado pelo índice de páginas deste espaço.
        let estado = unsafe { (*b).estado };
        estado != LIVRE && estado != POISON_STATE
    }

    /// O endereço do handle `h` cai numa página deste espaço (viva ou vazia)?
    /// Distingue o handle de um objeto coletado do escalar usado como handle.
    pub(crate) fn in_page(&self, h: i64) -> bool {
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        self.map.get(b & !(PAGINA - 1)).is_some()
    }

    /// As palavras do corpo do bloco `b` (vivo, desta página): em `INSTANCIA`, as
    /// dos campos e do mapa; nos demais, as do cabeçalho, ou as da região grande
    /// quando o `n` saturou.
    ///
    /// # Safety
    /// `b` é bloco vivo de uma página deste espaço.
    #[inline]
    pub(crate) unsafe fn body_words(&self, b: *const Header) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            let n = usize::from((*b).n);
            if shape(b) == INSTANCIA {
                return crate::layout::palavras_de_instancia(n);
            }
            if n != usize::from(u16::MAX) {
                return n;
            }
            // O objeto grande que não é string guarda as palavras em `map` (`allocate`).
            let m = (*b).map;
            if m != 0 && !crate::layout::cid::is_string((*b).class_id) {
                return m as usize;
            }
        }
        self.region_words(b)
    }

    /// As palavras do bloco da região grande de `b` (pelo mapa de páginas).
    #[cold]
    #[inline(never)]
    fn region_words(&self, b: *const Header) -> usize {
        self.map.get(b as usize & !(PAGINA - 1)).map_or(0, |i| self.pages[i].words)
    }

    /// Empilha as referências do corpo de `b` pelo formato, sem mexer em cartões
    /// (a verificação); devolve as posições percorridas.
    ///
    /// # Safety
    /// `b` é bloco vivo de uma página deste espaço.
    pub(crate) unsafe fn push_body(&self, b: *mut Header, stack: &mut Vec<i64>) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            match shape(b) {
                INSTANCIA => push_references(body(b), stack),
                REFS => {
                    let n = body_refs(b, self.body_words(b));
                    push_refs(b, n, stack)
                }
                _ => 0,
            }
        }
    }

    /// Empilha as referências de um lembrado na coleta menor: `INSTANCIA` pelo
    /// mapa; `REFS` com `CARTOES`, só os elementos dos cartões sujos (e os
    /// zera); `REFS` sem cartões, todas. Devolve as posições percorridas.
    ///
    /// # Safety
    /// `b` é bloco lembrado de uma página deste espaço.
    pub(crate) unsafe fn push_remembered(&self, b: *mut Header, stack: &mut Vec<i64>) -> usize {
        // SAFETY: o contrato da função.
        unsafe {
            if shape(b) == REFS && (*b).flags & CARTOES != 0 {
                let n = body_refs(b, self.body_words(b));
                return push_dirty_cards(b, n, stack);
            }
            self.push_body(b, stack)
        }
    }

    /// Apaga o mapa de marcas de cada página (o começo da coleta completa).
    pub(crate) fn clear_marks(&mut self) {
        for p in self.pages.iter().filter(|p| !p.empty) {
            // SAFETY: o mapa de marcas da página.
            unsafe { std::ptr::write_bytes(p.base, 0, PAGE_HEAD) };
        }
    }

    /// A varredura da coleta completa, só pelo mapa de marcas: o bit aceso é
    /// vivo (a marcação já o deixou velho); cada trecho sem bit volta às faixas
    /// livres, refeitas em ordem de endereço, sem que os blocos sejam lidos. A
    /// página sem vivo fica inteira como vazia enquanto os blocos livres da classe
    /// dela não passam dos vivos, da demanda do ciclo e do pico recente (e de duas
    /// páginas); as demais voltam à reserva. A região grande sem vivo volta ao
    /// cache delas. Devolve (mortos, vivos, bytes vivos).
    pub(crate) fn sweep(&mut self) -> (usize, usize, usize) {
        let (mut live_set, mut live_bytes) = (0, 0);
        for r in self.region.iter_mut() {
            *r = (std::ptr::null_mut(), std::ptr::null_mut());
        }
        self.release_dead_out_of_line();
        // Por página: o trecho `[de, until)` de `free_ranges` com as faixas
        // livres dela e os vivos.
        let mut fragments: Vec<(usize, usize, usize)> = Vec::with_capacity(self.pages.len());
        let mut free_ranges: Vec<(*mut u8, *mut u8)> = Vec::new();
        let mut class_live = vec![0usize; N_CLASSES];
        let mut class_free_lists = vec![0usize; N_CLASSES];
        for p in &self.pages {
            let de = free_ranges.len();
            if p.empty {
                fragments.push((de, de, 0));
                continue;
            }
            let size = p.size();
            let mut live_in_page = 0;
            // Os mortos em quarentena (`DARTFORGE_GC_VENENO`): não são vivos,
            // mas também não voltam à lista livre, e seguram a página.
            let mut pinned_in_page = 0;
            let mut free_ptr: *mut u8 = std::ptr::null_mut();
            let mut q = p.start();
            for _ in 0..p.blocks {
                // SAFETY: bloco da página.
                let alive = unsafe { marked(q.cast()) };
                // SAFETY: bloco sem marca da página.
                let pinned = !alive && p.class != LARGE && self.quarantine.as_mut().is_some_and(|qt| unsafe { qt.retain_ref(p.class, q, size) });
                if alive || pinned {
                    if alive {
                        live_in_page += 1;
                    } else {
                        pinned_in_page += 1;
                    }
                    if !free_ptr.is_null() {
                        free_ranges.push((free_ptr, q));
                        free_ptr = std::ptr::null_mut();
                    }
                } else if free_ptr.is_null() {
                    free_ptr = q;
                }
                q = q.wrapping_add(size);
            }
            if !free_ptr.is_null() {
                free_ranges.push((free_ptr, q));
            }
            if live_in_page + pinned_in_page == 0 || p.class == LARGE {
                free_ranges.truncate(de);
            } else if self.clear_dead {
                for &(start, end) in &free_ranges[de..] {
                    // SAFETY: blocos sem objeto vivo da página.
                    unsafe { std::ptr::write_bytes(start, 0, end as usize - start as usize) };
                }
            }
            live_set += live_in_page;
            if p.class == LARGE {
                live_bytes += live_in_page * p.bytes;
            } else {
                live_bytes += live_in_page * size;
                class_live[p.class] += live_in_page;
                class_free_lists[p.class] += p.blocks - live_in_page - pinned_in_page;
            }
            // O terceiro campo decide se a página fica: os presos a seguram.
            fragments.push((de, free_ranges.len(), live_in_page + pinned_in_page));
        }
        live_bytes += self.out_of_line_bytes() + self.attachment_bytes;
        let dead_set = self.live_set.saturating_sub(live_set);
        for (c, p) in self.peak.iter_mut().enumerate() {
            // Os blocos ocupados (vivos e mortos desta coleta): o tamanho que o
            // heap teve neste ciclo.
            *p = (*p - *p / 8).max(self.in_use[c]);
        }
        self.in_use.copy_from_slice(&class_live);
        // As páginas vazias que passam da folga voltam à reserva (as últimas
        // primeiro: as faixas preferem os endereços baixos); as já vazias de
        // antes contam como folga de todas as classes.
        let mut released_one = vec![false; self.pages.len()];
        let mut stored = 0usize;
        for i in (0..self.pages.len()).rev() {
            let p = &self.pages[i];
            if fragments[i].2 != 0 {
                continue;
            }
            if p.class == LARGE {
                released_one[i] = true;
                continue;
            }
            if p.empty {
                // Vazia desde a coleta anterior e não reusada: sobra.
                if stored >= 2 {
                    released_one[i] = true;
                } else {
                    stored += 1;
                }
                continue;
            }
            // A folga: o que sobreviveu, o que se alocou desde a última coleta
            // (a demanda do próximo ciclo), o pico recente e duas páginas.
            let folga = class_live[p.class].max(self.demand[p.class]).max(self.peak[p.class]).max(2 * p.blocks);
            if class_free_lists[p.class] >= folga + p.blocks {
                class_free_lists[p.class] -= p.blocks;
                released_one[i] = true;
            }
        }
        for l in self.free_list.iter_mut() {
            l.clear();
        }
        self.empties_list.clear();
        let pages = std::mem::take(&mut self.pages);
        for (i, mut p) in pages.into_iter().enumerate() {
            if released_one[i] {
                self.map.remove_it(p.base as usize);
                self.blocks -= p.blocks;
                self.release_page(&p);
                continue;
            }
            let (de, until, live_in_page) = fragments[i];
            if live_in_page == 0 {
                p.empty = true;
                self.empties_list.push(self.pages.len());
            } else if p.class != LARGE {
                self.free_list[p.class].extend_from_slice(&free_ranges[de..until]);
            }
            self.map.insert_entry(p.base as usize, self.pages.len());
            self.pages.push(p);
        }
        // As faixas de endereço baixo saem primeiro (`pop`).
        for l in self.free_list.iter_mut() {
            l.reverse();
        }
        // A quarentena: o que passou da capacidade volta à lista livre.
        if let Some(q) = self.quarantine.as_mut() {
            for (c, b) in q.released_set() {
                let t = bytes_do_bloco(class_words(c));
                self.free_list[c].push((b, b.wrapping_add(t)));
            }
        }
        // As vazias de endereço baixo saem primeiro (`pop`).
        self.empties_list.reverse();
        self.reserve.trim_free();
        self.large_ones.trim_free();
        self.live_set = live_set;
        self.handed_out = 0;
        self.marked_set = 0;
        self.young_set.clear();
        self.ranges.clear();
        self.remembered.clear();
        for d in self.demand.iter_mut() {
            *d = 0;
        }
        (dead_set, live_set, live_bytes)
    }

    /// Zera as faixas livres e o resto sujo das regiões: depois disso, todo bloco
    /// sem objeto tem o estado [`LIVRE`].
    fn clear_free(&mut self) {
        for ranges in &self.free_list {
            for &(start, end) in ranges {
                // SAFETY: blocos livres de uma página do espaço.
                unsafe { std::ptr::write_bytes(start, 0, end as usize - start as usize) };
            }
        }
        for (c, &(cursor, end)) in self.region.iter().enumerate() {
            if self.dirty_region[c] && !cursor.is_null() {
                // SAFETY: blocos livres da região da classe.
                unsafe { std::ptr::write_bytes(cursor, 0, end as usize - cursor as usize) };
            }
        }
    }

    /// Visita o bloco de cada objeto vivo (ou morto desde a última coleta).
    pub(crate) fn for_each_live(&mut self, mut f: impl FnMut(*mut Header)) {
        self.clear_free();
        for p in self.pages.iter().filter(|p| !p.empty) {
            for j in 0..p.blocks {
                let b = p.block(j);
                // SAFETY: bloco da página.
                let estado = unsafe { (*b).estado };
                if estado != LIVRE && estado != POISON_STATE {
                    f(b);
                }
            }
        }
    }

    /// Quantas páginas e regiões (para o rastro da coleta).
    pub(crate) fn n_pages(&self) -> usize {
        self.pages.len()
    }

    /// Bytes das regiões grandes no cache (fora do heap vivo).
    #[allow(dead_code)]
    pub(crate) fn cached_bytes(&self) -> usize {
        self.large_ones.bytes
    }

    /// A quebra da memória do espaço (`DARTFORGE_GC_MEMORIA=1`): por classe de
    /// tamanho, as páginas, os blocos e os ocupados (os vivos da última coleta
    /// completa mais os entregues desde então); as páginas vazias, as da
    /// reserva, as regiões grandes vivas e em cache, os corpos de fora e os
    /// anexos. Uma linha por item, em KiB.
    pub(crate) fn memory_report(&self) -> String {
        use std::fmt::Write;
        let kib = |b: usize| b / 1024;
        let mut class_pages = vec![0usize; N_CLASSES];
        let (mut large_ones, mut large_bytes, mut empties_list) = (0, 0, 0);
        let (mut dead_large, mut dead_large_bytes) = (0, 0);
        for p in &self.pages {
            if p.empty {
                empties_list += 1;
            } else if p.class == LARGE {
                large_ones += 1;
                large_bytes += p.bytes;
                // SAFETY: o bloco da região, legível.
                if unsafe { (*p.block(0)).estado } == LIVRE {
                    dead_large += 1;
                    dead_large_bytes += p.bytes;
                }
            } else {
                class_pages[p.class] += 1;
            }
        }
        let mut s = String::new();
        let (mut total_pages, mut total_occupied) = (0, 0);
        let mut line_list = Vec::new();
        for c in 1..N_CLASSES {
            let k = class_pages[c];
            if k == 0 {
                continue;
            }
            let size = bytes_do_bloco(class_words(c));
            let per_page = (PAGINA - PAGE_HEAD) / size;
            let occupied = self.in_use[c];
            total_pages += k;
            total_occupied += occupied * size;
            line_list.push(format!(
                "  classe {c:>2} ({size:>5} B): {k:>3} pág., {occupied:>6}/{:>6} blocos ({}%)",
                k * per_page,
                occupied * 100 / (k * per_page).max(1)
            ));
        }
        let _ = writeln!(
            s,
            "[memória] páginas de classe: {total_pages} ({} KiB), ocupado {} KiB ({}%), classes com página: {}",
            kib(total_pages * PAGINA),
            kib(total_occupied),
            total_occupied * 100 / (total_pages * PAGINA).max(1),
            line_list.len()
        );
        for l in line_list {
            let _ = writeln!(s, "{l}");
        }
        let _ = writeln!(
            s,
            "[memória] vazias guardadas: {empties_list} ({} KiB); reserva: {} livres, {} sujas ({} KiB sujas)",
            kib(empties_list * PAGINA),
            self.reserve.free_list.len(),
            self.reserve.dirty.len(),
            kib(self.reserve.dirty.len() * PAGINA)
        );
        let _ = writeln!(
            s,
            "[memória] regiões grandes: {large_ones} no espaço ({} KiB; {dead_large} mortas, {} KiB), {} em cache ({} KiB); corpos de fora: {} ({} KiB); anexos: {} ({} KiB)",
            kib(large_bytes),
            kib(dead_large_bytes),
            self.large_ones.released_list.len(),
            kib(self.large_ones.bytes),
            self.with_out_of_line.len(),
            kib(self.out_of_line_bytes()),
            self.attachments.len(),
            kib(self.attachment_bytes)
        );
        let _ = writeln!(
            s,
            "[memória] vetores do espaço: páginas {} KiB, livres {} faixas, jovens {} KiB, faixas {} KiB, lembrados {} KiB",
            kib(self.pages.capacity() * std::mem::size_of::<Page>() + self.map.regions.len() * 4 * (1 << PageMap::BITS)),
            self.free_list.iter().map(Vec::len).sum::<usize>(),
            kib(self.young_set.capacity() * 16),
            kib(self.ranges.capacity() * 24),
            kib(self.remembered.capacity() * 8)
        );
        s
    }
}

/// A memória residente e o pico do processo, e os bytes confirmados do heap do
/// processo (o do alocador do Rust), em bytes (`DARTFORGE_GC_MEMORIA=1`).
pub(crate) fn process_memory() -> (usize, usize, usize) {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct Counters {
            cb: u32,
            misses: u32,
            peak: usize,
            current: usize,
            rest: [usize; 6],
        }
        #[repr(C)]
        #[derive(Default)]
        struct HeapOverview {
            cb: u32,
            allocated: usize,
            confirmed: usize,
            reserved: usize,
            maximum: usize,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessHeap() -> *mut std::ffi::c_void;
            fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Counters, n: u32) -> i32;
            fn HeapSummary(h: *mut std::ffi::c_void, flags: u32, r: *mut HeapOverview) -> i32;
        }
        let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
        let mut r = HeapOverview { cb: std::mem::size_of::<HeapOverview>() as u32, ..Default::default() };
        // SAFETY: estruturas do tamanho informado.
        unsafe {
            K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb);
            HeapSummary(GetProcessHeap(), 0, &mut r);
        }
        (c.current, c.peak, r.confirmed)
    }
    #[cfg(not(windows))]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let field = |name_str: &str| {
            status
                .lines()
                .find(|l| l.starts_with(name_str))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(0, |k| k * 1024)
        };
        (field("VmRSS:"), field("VmHWM:"), 0)
    }
}

impl Drop for ObjectSpace {
    fn drop(&mut self) {
        for &b in &self.with_out_of_line {
            // SAFETY: bloco com corpo de fora (do alocador do sistema).
            unsafe { Self::release_body(b) };
        }
        for (_, a) in std::mem::take(&mut self.attachments) {
            // SAFETY: o contrato de `Heap::attach`; o espaço acabou.
            unsafe { (a.release)(a.ptr) };
        }
        for p in std::mem::take(&mut self.pages) {
            // As de `PAGINA` bytes saem com a reserva (o `Drop` dela); as
            // regiões grandes, direto ao sistema.
            if p.class == LARGE {
                // SAFETY: região de `system::reserve_space`, sem uso.
                unsafe { system::free(p.base, p.bytes) };
            } else {
                self.reserve.free_list.push(p.base);
            }
        }
    }
}

#[cfg(test)]
mod tests_ranges {
    use super::*;

    #[test]
    fn contiguous_dead_refill_a_range_without_touching_live() {
        for order in [[0, 1, 2], [2, 1, 0], [0, 2, 1]] {
            let mut e = ObjectSpace::new(true);
            e.quarantine = None;
            let class = size_class(2).unwrap();
            let size = bytes_do_bloco(class_words(class));
            let blocks: Vec<_> = (0..4).map(|_| {
                let h = e.allocate(129, 2, INSTANCIA, 2);
                e.block_of(h).unwrap()
            }).collect();
            e.young_set.clear();
            for i in order {
                // SAFETY: bloco ocupado entregue acima, sem uso posterior.
                unsafe { e.release_dead(blocks[i]) };
            }
            assert_eq!(e.live_set, 1);
            assert_eq!(e.free_list[class].len(), 1);
            e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
            let (start, n) = e.take_from_region(class, TLAB_BLOCKS);
            assert_eq!(start, blocks[0].cast());
            assert_eq!(n, 3);
            // SAFETY: a faixa devolvida está reservada, e o quarto bloco vive.
            unsafe {
                assert!(std::slice::from_raw_parts(start, n * size).iter().all(|&b| b == 0));
                assert_eq!((*blocks[3]).class_id, 129);
            }
        }
    }

    #[test]
    fn blocks_separated_by_live_do_not_merge() {
        let mut e = ObjectSpace::new(false);
        e.quarantine = None;
        let class = size_class(2).unwrap();
        let blocks: Vec<_> = (0..3).map(|_| {
            let h = e.allocate(129, 2, INSTANCIA, 2);
            e.block_of(h).unwrap()
        }).collect();
        e.young_set.clear();
        // SAFETY: apenas os extremos morreram; o bloco central continua vivo.
        unsafe {
            e.release_dead(blocks[0]);
            e.release_dead(blocks[2]);
        }
        assert_eq!(e.free_list[class].len(), 2);
        e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
        let (_, n) = e.take_from_region(class, TLAB_BLOCKS);
        assert_eq!(n, 1);
        // SAFETY: o objeto central não foi solto nem reutilizado.
        unsafe { assert_eq!((*blocks[1]).class_id, 129) };
    }

    #[test]
    fn quarantine_hands_out_only_blocks_that_left_the_ring() {
        let mut e = ObjectSpace::new(false);
        e.quarantine = Some(Quarantine {
            ring: std::collections::VecDeque::new(),
            pinned_set: crate::hash::HashSet::default(),
            capacidade: 1,
        });
        let class = size_class(2).unwrap();
        let blocks: Vec<_> = (0..3).map(|_| {
            let h = e.allocate(129, 2, INSTANCIA, 2);
            e.block_of(h).unwrap()
        }).collect();
        e.young_set.clear();
        for &b in &blocks {
            // SAFETY: bloco ocupado, sem objeto vivo.
            unsafe { e.release_dead(b) };
        }
        assert_eq!(e.free_list[class].len(), 2);
        assert!(e.quarantine.as_ref().unwrap().pinned_set.contains(&(blocks[2] as usize)));
        e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
        let (first, n) = e.take_from_region(class, TLAB_BLOCKS);
        assert_eq!(n, 1);
        assert_eq!(first, blocks[1].cast());
        e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
        let (second, n) = e.take_from_region(class, TLAB_BLOCKS);
        assert_eq!(n, 1);
        assert_eq!(second, blocks[0].cast());
        // SAFETY: o último bloco permanece envenenado, fora das faixas livres.
        unsafe { assert_eq!((*blocks[2]).estado, POISON_STATE) };
    }
}
