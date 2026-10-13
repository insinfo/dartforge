//! The object space (docs/NATIVO-ESPACO-UNIFICADO.md §2.6–§2.8 and §3.2).
//!
//! Every heap block lives here: a 16-byte [`crate::layout::Header`] followed by the body,
//! in 8-byte words. Pages of [`crate::layout::PAGINA`] bytes hold blocks of a
//! single **size class** (`layout::size_class`: the exact ones from 1 to 64
//! words and 24 medium ones up to 2014 words, each filling the page); a larger
//! body has its own **large region**, rounded to 4 KiB and aligned to
//! [`crate::layout::PAGINA`] (`VirtualAlloc`/`mmap`), freed already by the minor collection that finds it
//! dead, into a per-size cache with a ceiling. Pages come from chunks
//! reserved from the system and only take memory where written; freed ones go back
//! to the system except for a small hot cache (docs/NATIVO-PLANO.md §14).
//!
//! The start of each page (and of each region) is the mark bitmap: one bit per
//! word. Marking sets the bit of the reached object; sweeps read only the
//! bits, without touching the blocks, live or dead (the VM also marks outside the object
//! in the *old space*, `marking.cc`; here the bitmap sits alongside, per page, like
//! Immix's). The collector does not move: an object's address is fixed while it lives.
//!
//! The collector walks the body by the **format** in `flags` (§2.3), without looking at the
//! class: `INSTANCIA` by the reference map, `BRUTO` nothing, `REFS` the words
//! `1..=word 0`. Large `REFS` have **cards** (1 bit per 32 elements,
//! after the last element): the minor collection walks only the elements of the dirty
//! cards of a remembered object. A block with an **attachment** (`ATTACHMENT`) keeps at `b+16`
//! a native pointer, freed when the owner dies.
//!
//! It is the non-moving counterpart of the VM's *new space*
//! (`runtime/vm/heap/scavenger.cc` and the inline allocation of
//! `stub_code_compiler.cc`): allocating is taking the next block from the class region —
//! from generated code, from the TLAB (`Context::tlab`) — without `malloc`.
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

/// The class of the large regions in the per-class vectors (index 0, which no
/// size class uses).
pub(crate) const LARGE: usize = 0;

/// Granularity of the large regions (the system page).
const REGION_GRANULARITY: usize = 4096;

/// Up to how many blocks each TLAB refill hands out (a contiguous range of the
/// free list: fewer, if contiguity ends earlier).
pub(crate) const TLAB_BLOCKS: usize = 256;

// ─── Poison and quarantine (`DARTFORGE_GC_VENENO`) ─────────────────────────

/// The state of a poisoned block: the first byte of the poison pattern.
/// No valid state uses it; handle validation rejects it.
pub(crate) const POISON_STATE: u8 = 0xDE;
/// The word the dead block is filled with: even, and with the first byte
/// (the state) in [`POISON_STATE`].
const POISON_WORD: u64 = 0xDFDF_DFDF_DFDF_DFDE;

/// The quarantine of dead blocks (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §7.5): with `DARTFORGE_GC_VENENO=1` (or `=<n>` blocks), each block that
/// dies is filled with the poison pattern and only goes back to the free list after
/// `n` others (1 000 by default) have died after it. A use after
/// free then reads the poison and fails handle validation, instead of reading a
/// new object placed at the same spot. The collector does not move objects: this is the
/// equivalent of a copying collector's *from-space* quarantine.
struct Quarantine {
    /// (class, block), in the order they entered.
    ring: std::collections::VecDeque<(usize, *mut u8)>,
    /// The ring's blocks, so the full sweep does not return them.
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

    /// Poisons the block `b` of `size` bytes and puts it in the ring.
    ///
    /// # Safety
    /// `b` is a block without a live object of a page of the space.
    unsafe fn pin(&mut self, class: usize, b: *mut u8, size: usize) {
        let words = b.cast::<u64>();
        for i in 0..size / 8 {
            // SAFETY: the function contract; the block is 8-aligned.
            unsafe { words.add(i).write(POISON_WORD) };
        }
        self.ring.push_back((class, b));
        self.pinned_set.insert(b as usize);
    }

    /// In the full sweep: does the unmarked block `b` stay in (or enter) the
    /// quarantine? Already in the ring, yes; with the state of an object that just
    /// died, it enters; free (zeroed or already poisoned and released), no.
    ///
    /// # Safety
    /// `b` is an unmarked block of a page of the space.
    unsafe fn retain_ref(&mut self, class: usize, b: *mut u8, size: usize) -> bool {
        if self.pinned_set.contains(&(b as usize)) {
            return true;
        }
        // SAFETY: the function contract.
        let estado = unsafe { *b };
        if estado == LIVRE || estado == POISON_STATE {
            return false;
        }
        // SAFETY: the function contract.
        unsafe { self.pin(class, b, size) };
        true
    }

    /// The blocks beyond capacity, oldest first: they leave
    /// the ring (they stay poisoned until handed out, which zeroes them).
    fn released_set(&mut self) -> Vec<(usize, *mut u8)> {
        let mut v = Vec::new();
        while self.ring.len() > self.capacidade {
            let (class, b) = self.ring.pop_front().expect("ring beyond capacity");
            self.pinned_set.remove(&(b as usize));
            v.push((class, b));
        }
        v
    }
}

// ─── Mark bitmap ───────────────────────────────────────────────────────────

/// The word and the bit of the mark bitmap of block `b`.
#[inline]
pub(crate) fn mark_bit(b: *const Header) -> (*mut u64, u64) {
    let a = b as usize;
    let base = a & !(PAGINA - 1);
    let desl = a - base;
    ((base + (desl >> 9) * 8) as *mut u64, 1u64 << ((desl >> 3) & 63))
}

/// Was block `b` marked (by the full collection in progress, or by the minor one that
/// promoted it)?
///
/// # Safety
/// `b` is a block of a page of the space (never a static: its bitmap would fall
/// outside a page).
#[inline]
pub(crate) unsafe fn marked(b: *const Header) -> bool {
    let (w, m) = mark_bit(b);
    // SAFETY: the function contract; the bitmap is at the start of the page.
    unsafe { *w & m != 0 }
}

/// Sets the mark bit of block `b`.
///
/// # Safety
/// `b` is a block of a page of the space.
#[inline]
pub(crate) unsafe fn mark_block(b: *const Header) {
    let (w, m) = mark_bit(b);
    // SAFETY: the function contract.
    unsafe { *w |= m };
}

/// Clears the mark bit of block `b` (the block pure ARC frees: when
/// handed out again as young, the old mark would make it look alive).
///
/// # Safety
/// `b` is a block of a page of the space.
#[inline]
pub(crate) unsafe fn unmark_block(b: *const Header) {
    let (w, m) = mark_bit(b);
    // SAFETY: the function contract.
    unsafe { *w &= !m };
}

// ─── The body of a block ───────────────────────────────────────────────────

/// Bytes of an `INSTANCIA` block (or out-of-line body) of `n` fields.
#[inline]
pub const fn block_size(n: usize) -> usize {
    bytes_do_bloco(capacidade(n) + palavras_do_mapa(n))
}

/// The body format of block `b` (`flags & FORMA`).
///
/// # Safety
/// `b` points to a readable header.
#[inline]
pub(crate) unsafe fn shape(b: *const Header) -> u8 {
    // SAFETY: the function contract.
    unsafe { (*b).flags & FORMA }
}

/// The body of an `INSTANCIA` object (the block itself, or the out-of-line body).
///
/// # Safety
/// `b` is the header of a live block.
#[inline]
pub(crate) unsafe fn body(b: *mut Header) -> *mut Header {
    // SAFETY: the function contract; with FORA, the first field is the address.
    unsafe {
        if (*b).flags & FORA != 0 { *fields_of(b).cast::<*mut Header>() } else { b }
    }
}

/// The words after a header (block or body).
#[inline]
pub(crate) fn fields_of(c: *mut Header) -> *mut i64 {
    c.wrapping_add(1).cast()
}

/// Is field `i` of an `INSTANCIA` body a reference?
///
/// # Safety
/// `c` is a valid body and `i < n`.
#[inline]
pub(crate) unsafe fn is_reference(c: *const Header, i: usize) -> bool {
    // SAFETY: the function contract.
    unsafe {
        if i < 32 {
            (*c).map >> i & 1 != 0
        } else {
            let ext = fields_of(c.cast_mut()).add(capacidade(usize::from((*c).n))).cast::<u64>();
            *ext.add((i - 32) / 64) >> ((i - 32) % 64) & 1 != 0
        }
    }
}

/// Sets or clears the reference bit of field `i` of an `INSTANCIA` body.
///
/// # Safety
/// `c` is a valid body and `i < n`.
#[inline]
pub(crate) unsafe fn mark_reference(c: *mut Header, i: usize, is_ref_kind: bool) {
    // SAFETY: the function contract.
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

/// Pushes the reference fields of an `INSTANCIA` body; returns how many fields
/// it has. `map` is only a reference map in `INSTANCIA` (§2.3).
///
/// # Safety
/// `c` is a valid `INSTANCIA` body.
#[inline]
pub(crate) unsafe fn push_references(c: *const Header, stack: &mut Vec<i64>) -> usize {
    // SAFETY: the function contract.
    unsafe {
        debug_assert!(shape(c) == INSTANCIA, "map read as a reference map in a body that is not INSTANCIA");
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

/// How many `Ref`s a `REFS` body of `words` words holds: word 0 (the
/// length of the `_List`, the number of fields of the `_Record`), capped at the body.
///
/// # Safety
/// `b` is a valid `REFS` block with `words` body words.
#[inline]
pub(crate) unsafe fn body_refs(b: *const Header, words: usize) -> usize {
    // SAFETY: the function contract; word 0 exists (`words ≥ 1`).
    let n = unsafe { *fields_of(b.cast_mut()) };
    usize::try_from(n).unwrap_or(0).min(words.saturating_sub(1))
}

/// The card words of a `REFS` body with `CARTOES` of `len` elements
/// (after the last element: `b + 24 + 8·len`).
#[inline]
pub(crate) fn cards_of(b: *mut Header, len: usize) -> *mut u64 {
    fields_of(b).wrapping_add(1 + len).cast()
}

/// Dirties the card of element `i` (element index, from 0) of a list
/// with `CARTOES` of `len` elements.
///
/// # Safety
/// `b` is a `REFS` block with `CARTOES` and `i < len`.
#[inline]
pub(crate) unsafe fn dirty_card(b: *mut Header, len: usize, i: usize) {
    let p = cards_of(b, len).wrapping_add(i / ELEMENTOS_POR_PALAVRA_DE_CARTAO);
    // SAFETY: the function contract.
    unsafe { *p |= 1u64 << ((i / ELEMENTOS_POR_CARTAO) & 63) };
}

/// Dirties all cards (the explicit barrier of whoever wrote from outside).
///
/// # Safety
/// `b` is a `REFS` block with `CARTOES` of `len` elements.
pub(crate) unsafe fn dirty_all_cards(b: *mut Header, len: usize) {
    let c = cards_of(b, len);
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: the function contract.
        unsafe { *c.add(w) = u64::MAX };
    }
}

/// Pushes the `Ref`s of words `1..=n` of a `REFS` body, skipping null and `Smi`
/// (the caller filters the rest); returns `n`.
///
/// # Safety
/// `b` is a valid `REFS` block with at least `n + 1` body words.
#[inline]
pub(crate) unsafe fn push_refs(b: *const Header, n: usize, stack: &mut Vec<i64>) -> usize {
    let p = fields_of(b.cast_mut());
    for i in 1..=n {
        // SAFETY: the function contract.
        let r = unsafe { *p.add(i) };
        if r != 0 && r & 1 == 0 {
            stack.push(r);
        }
    }
    n
}

/// Pushes the `Ref`s of the elements of the dirty cards of a `REFS` list with
/// `CARTOES` of `len` elements and clears the cards; returns how many elements
/// it walked.
///
/// # Safety
/// `b` is a `REFS` block with `CARTOES` of `len` elements.
pub(crate) unsafe fn push_dirty_cards(b: *mut Header, len: usize, stack: &mut Vec<i64>) -> usize {
    let c = cards_of(b, len);
    let elements = fields_of(b).wrapping_add(1);
    let mut work = 0;
    for w in 0..len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) {
        // SAFETY: the function contract.
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

/// Clears the cards of a `REFS` list with `CARTOES` of `len` elements.
///
/// # Safety
/// `b` is a `REFS` block with `CARTOES` of `len` elements.
pub(crate) unsafe fn clear_cards(b: *mut Header, len: usize) {
    // SAFETY: the function contract.
    unsafe { std::ptr::write_bytes(cards_of(b, len), 0, len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO)) };
}

/// An out-of-line body of `n` zeroed fields (the system allocator).
pub(crate) fn new_out_of_line_body(n: usize) -> *mut Header {
    let layout = std::alloc::Layout::from_size_align(block_size(n), 8).expect("layout do corpo");
    // SAFETY: layout of non-zero size.
    let p = unsafe { std::alloc::alloc_zeroed(layout) }.cast::<Header>();
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    // SAFETY: new memory, zeroed, of the body's size.
    unsafe {
        (*p).n = u16::try_from(n).expect("object with too many fields");
    }
    p
}

/// Frees an out-of-line body.
///
/// # Safety
/// `c` came from [`new_out_of_line_body`] and nobody else uses it.
pub(crate) unsafe fn release_out_of_line_body(c: *mut Header) {
    // SAFETY: the function contract.
    unsafe {
        let n = usize::from((*c).n);
        std::alloc::dealloc(c.cast(), std::alloc::Layout::from_size_align(block_size(n), 8).expect("layout do corpo"));
    }
}

/// Bytes of a large region for a body of `words` words: the mark bitmap
/// and the block, rounded to [`REGION_GRANULARITY`].
#[inline]
pub(crate) const fn region_bytes(words: usize) -> usize {
    (PAGE_HEAD + bytes_do_bloco(words)).next_multiple_of(REGION_GRANULARITY)
}

/// Bytes that an allocation of a body of `words` words takes: the class
/// block, or the large region.
#[inline]
pub(crate) const fn allocation_bytes(words: usize) -> usize {
    match size_class(words) {
        Some(c) => bytes_do_bloco(class_words(c)),
        None => region_bytes(words),
    }
}

// ─── Pages ─────────────────────────────────────────────────────────────────

/// A page of the object space (blocks of a single size class) or a
/// large region (a single block).
#[derive(Debug)]
pub(crate) struct Page {
    base: *mut u8,
    /// Allocated bytes (`PAGINA`, or those of the large region).
    bytes: usize,
    /// The size class, or [`LARGE`].
    class: usize,
    /// Body words of each block (the class's; in the large region, the actual one).
    words: usize,
    blocks: usize,
    /// The inverse of `block_bytes(words) / 8` (odd, or a power of 2
    /// times odd: see `ix`) modulo 2³²: the index of a block comes from a
    /// multiplication, and an offset that is not a multiple of the size gives an
    /// index beyond `blocks` (the exact divisibility test, *Hacker's
    /// Delight* 10-17).
    inverse: u32,
    /// `block_bytes(words) / 8` = `odd << shift`.
    offset: u32,
    /// Page without live objects kept for reuse (in
    /// [`ObjectSpace::empties_list`]): its content is garbage from the dead until it is
    /// zeroed and formatted again, for any class.
    empty: bool,
}

impl Page {
    /// The page of blocks of class `class` (body of `words` words) at
    /// `base` (`bytes` bytes).
    fn formatted(base: *mut u8, bytes: usize, class: usize, words: usize) -> Self {
        let size = bytes_do_bloco(words);
        let blocks = if class == LARGE { 1 } else { (PAGINA - PAGE_HEAD) / size };
        let in_words = u32::try_from(size / 8).unwrap_or(1);
        let offset = in_words.trailing_zeros();
        let inverse = odd_inverse(in_words >> offset);
        Page { base, bytes, class, words, blocks, inverse, offset, empty: false }
    }
    /// The first block (after the mark bitmap).
    fn start(&self) -> *mut u8 {
        self.base.wrapping_add(PAGE_HEAD)
    }
    fn size(&self) -> usize {
        bytes_do_bloco(self.words)
    }
    fn block(&self, i: usize) -> *mut Header {
        self.start().wrapping_add(i * self.size()).cast()
    }
    /// The index of the block that starts `desl` bytes after the base, if one
    /// starts there.
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

/// The inverse of `a` (odd) modulo 2³² (Newton: each step doubles the correct bits).
const fn odd_inverse(a: u32) -> u32 {
    let mut x = a;
    let mut i = 0;
    while i < 5 {
        x = x.wrapping_mul(2u32.wrapping_sub(a.wrapping_mul(x)));
        i += 1;
    }
    x
}

/// Page base (`address >> 16`) → index + 1 in `pages`: tables of 2¹⁶
/// positions per 4 GiB region (almost always just one), like an
/// allocator's *pagemap* — handle validation without hashing.
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
        self.define(base, u32::try_from(ix + 1).expect("too many pages"));
    }
    fn remove_it(&mut self, base: usize) {
        self.define(base, 0);
    }
}

// ─── System memory ─────────────────────────────────────────────────────────

/// The reservation of the object space pages of [`PAGINA`] bytes
/// (docs/NATIVO-PLANO.md §14): chunks of [`PAGES_PER_CHUNK`] pages
/// reserved from the system at once, on all platforms, and the pages
/// freed by collection. A new page comes zeroed from the system and only enters
/// resident memory where the program writes (before, outside Linux, each
/// page was an `alloc_zeroed` aligned to 64 KiB, which on Windows `HeapAlloc`
/// serves with twice the bytes, all zeroed: 128 KiB resident per page).
/// A freed page stays dirty in a small hot cache and the rest goes back to the system
/// ([`system::discard`]) without undoing the reservation — Julia's `gc_free_pages`.
#[derive(Default)]
struct PageReserve {
    /// The reserved chunks `(start, bytes)`.
    chunks: Vec<(*mut u8, usize)>,
    /// Zeroed pages yet to be used (never used, or returned to the system): on
    /// Windows, committed only when they leave ([`system::confirm`]).
    free_list: Vec<*mut u8>,
    /// Pages freed by collection, still with the garbage of the dead (and in resident
    /// memory): the next page comes from them, zeroed by hand. The full collection
    /// keeps at most [`HOT_PAGES`] (the most recently freed) and returns
    /// to the system the others and those left unused since the previous full one
    /// ([`PageReserve::trim_free`]).
    dirty: Vec<*mut u8>,
    /// How many of the `dirty` ones (those at the bottom) were left unused since the last
    /// [`PageReserve::trim_free`].
    stopped: usize,
}

/// Pages of each chunk the [`PageReserve`] reserves (2 MiB).
const PAGES_PER_CHUNK: usize = 32;

/// How many dirty pages the reserve keeps from one full collection to the next: those
/// that fit in the young semispace (`YOUNG_LIMIT`, 2 MiB), what the next
/// minor collection may ask for. Julia keeps the collection interval
/// (`gc_free_pages`, `default_collect_interval`).
const HOT_PAGES: usize = 32;

/// Ceiling of the bytes of the cache of freed large regions ([`LargeRegions`]).
const LARGE_CACHE_CEILING: usize = 4 * 1024 * 1024;

#[cfg(unix)]
mod mapping {
    unsafe extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
        fn munmap(addr: *mut u8, len: usize) -> i32;
        #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
        fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    }
    /// `MAP_ANONYMOUS`: 0x20 on Linux, 0x1000 on macOS and the BSDs.
    #[cfg(target_os = "linux")]
    const MAP_ANON: i32 = 0x20;
    #[cfg(not(target_os = "linux"))]
    const MAP_ANON: i32 = 0x1000;
    const PROT_RW: i32 = 1 | 2;
    const MAP_PRIVATE: i32 = 2;
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    const MAP_FIXED: i32 = 0x10;
    /// New `bytes`, zeroed, aligned to `alignment` (power of 2).
    pub fn map_pages(bytes: usize, alignment: usize) -> *mut u8 {
        let total = bytes + alignment;
        // SAFETY: new anonymous mapping.
        let p = unsafe { mmap(std::ptr::null_mut(), total, PROT_RW, MAP_PRIVATE | MAP_ANON, -1, 0) };
        if p as isize == -1 || p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, alignment).expect("layout"));
        }
        let start = (p as usize).next_multiple_of(alignment);
        let before = start - p as usize;
        let after = total - before - bytes;
        // SAFETY: the leftovers of the mapping itself, outside `[start, start + bytes)`.
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
    /// Unmaps `[p, p + bytes)`.
    ///
    /// # Safety
    /// `[p, p + bytes)` came from [`map_pages`] and nobody else uses it.
    pub unsafe fn unmap(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { munmap(p, bytes) };
    }
    /// Returns the memory of `[p, p + bytes)` to the system, which comes back zeroed: on
    /// Linux, `madvise(MADV_DONTNEED)`; on the other Unixes (where
    /// `MADV_DONTNEED` does not guarantee zero), a new anonymous mapping on top.
    ///
    /// # Safety
    /// `[p, p + bytes)` is a private anonymous mapping with nothing alive.
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        #[cfg(target_os = "linux")]
        {
            const MADV_DONTNEED: i32 = 4;
            // SAFETY: the function contract.
            if unsafe { madvise(p, bytes, MADV_DONTNEED) } == 0 {
                return;
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // SAFETY: the function contract; the new mapping replaces the
            // range at the same address.
            let q = unsafe { mmap(p, bytes, PROT_RW, MAP_PRIVATE | MAP_ANON | MAP_FIXED, -1, 0) };
            if q == p {
                return;
            }
        }
        // Without the discard the memory does not come back zeroed: zero it by hand.
        // SAFETY: the function contract.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

/// System memory: the large regions (reserved and committed at
/// once, zeroed, aligned to [`PAGINA`], with the system's page
/// granularity — a 20 KiB object takes 24 KiB; the rest of the aligned address range
/// is not committed) and the page chunks of the [`PageReserve`]
/// (reserved, committed page by page on Windows).
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

    /// `VirtualAlloc` of `bytes` with `tipo`; aborts when out of memory.
    #[cfg(windows)]
    fn virtual_alloc(address: *mut u8, bytes: usize, tipo: u32) -> *mut u8 {
        // SAFETY: new reservation (null `address`) or commit inside a
        // reservation of this module.
        let p = unsafe { VirtualAlloc(address, bytes, tipo, PAGE_READWRITE) };
        if p.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(bytes, PAGINA).expect("layout"));
        }
        p
    }

    /// Zeroed `bytes` aligned to [`PAGINA`], committed.
    #[cfg(windows)]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        // The Windows allocation granularity (64 KiB) gives the alignment
        // of a page of the space.
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE | MEM_COMMIT);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc without the 64 KiB alignment");
        p
    }
    /// Returns `[p, p + bytes)` to the system.
    ///
    /// # Safety
    /// Came from [`reserve_space`] or from [`reserve_chunk`] and nobody else uses it.
    #[cfg(windows)]
    pub unsafe fn free(p: *mut u8, _bytes: usize) {
        // SAFETY: the function contract.
        unsafe { VirtualFree(p, 0, MEM_RELEASE) };
    }
    /// A chunk of `bytes` aligned to [`PAGINA`] for pages: on Windows only
    /// reserved ([`confirm`] each page before use).
    #[cfg(windows)]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        let p = virtual_alloc(std::ptr::null_mut(), bytes, MEM_RESERVE);
        assert!(p as usize % PAGINA == 0, "VirtualAlloc without the 64 KiB alignment");
        p
    }
    /// Commits `[p, p + bytes)` of a chunk (zeroed if it was not
    /// committed; committing what is already committed changes nothing).
    #[cfg(windows)]
    pub fn confirm(p: *mut u8, bytes: usize) {
        virtual_alloc(p, bytes, MEM_COMMIT);
    }
    /// Returns the memory of `[p, p + bytes)` of a chunk to the system (it leaves
    /// resident and committed memory; it comes back zeroed on the next [`confirm`]).
    ///
    /// # Safety
    /// `[p, p + bytes)` belongs to a chunk, with nothing alive.
    #[cfg(windows)]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { VirtualFree(p, bytes, MEM_DECOMMIT) };
    }

    /// Zeroed `bytes` aligned to [`PAGINA`].
    #[cfg(unix)]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        super::mapping::map_pages(bytes, PAGINA)
    }
    /// Returns `[p, p + bytes)` to the system.
    ///
    /// # Safety
    /// Came from [`reserve_space`] or from [`reserve_chunk`] and nobody else uses it.
    #[cfg(unix)]
    pub unsafe fn free(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { super::mapping::unmap(p, bytes) };
    }
    /// A chunk of `bytes` aligned to [`PAGINA`] for pages (`mmap` only
    /// takes memory where written).
    #[cfg(unix)]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        super::mapping::map_pages(bytes, PAGINA)
    }
    /// Nothing to commit on Unix.
    #[cfg(unix)]
    pub fn confirm(_p: *mut u8, _bytes: usize) {}
    /// Returns the memory of `[p, p + bytes)` of a chunk to the system (it comes back
    /// zeroed).
    ///
    /// # Safety
    /// `[p, p + bytes)` belongs to a chunk, with nothing alive.
    #[cfg(unix)]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { super::mapping::discard(p, bytes) };
    }

    /// Zeroed `bytes` aligned to [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reserve_space(bytes: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(bytes, PAGINA).expect("region layout");
        // SAFETY: layout of non-zero size.
        let p = unsafe { std::alloc::alloc_zeroed(layout) };
        if p.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        p
    }
    /// Returns `[p, p + bytes)` to the system.
    ///
    /// # Safety
    /// Came from [`reserve_space`] or from [`reserve_chunk`] and nobody else uses it.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn free(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { std::alloc::dealloc(p, std::alloc::Layout::from_size_align(bytes, PAGINA).expect("region layout")) };
    }
    /// A chunk of zeroed `bytes` aligned to [`PAGINA`].
    #[cfg(not(any(windows, unix)))]
    pub fn reserve_chunk(bytes: usize) -> *mut u8 {
        reserve_space(bytes)
    }
    /// Nothing to commit.
    #[cfg(not(any(windows, unix)))]
    pub fn confirm(_p: *mut u8, _bytes: usize) {}
    /// Zeroes `[p, p + bytes)` (no way to return it to the system).
    ///
    /// # Safety
    /// `[p, p + bytes)` belongs to a chunk, with nothing alive.
    #[cfg(not(any(windows, unix)))]
    pub unsafe fn discard(p: *mut u8, bytes: usize) {
        // SAFETY: the function contract.
        unsafe { std::ptr::write_bytes(p, 0, bytes) };
    }
}

impl PageReserve {
    /// A zeroed page of [`PAGINA`] bytes, aligned to [`PAGINA`].
    fn request_fn(&mut self) -> *mut u8 {
        if let Some(p) = self.dirty.pop() {
            self.stopped = self.stopped.min(self.dirty.len());
            // SAFETY: page of the reserve, unused.
            unsafe { std::ptr::write_bytes(p, 0, PAGINA) };
            return p;
        }
        if self.free_list.is_empty() {
            let bytes = PAGINA * PAGES_PER_CHUNK;
            let start = system::reserve_chunk(bytes);
            self.chunks.push((start, bytes));
            // Those with low addresses go first (`pop`).
            for i in (0..PAGES_PER_CHUNK).rev() {
                self.free_list.push(start.wrapping_add(i * PAGINA));
            }
        }
        let p = self.free_list.pop().expect("new chunk without pages");
        system::confirm(p, PAGINA);
        p
    }
    /// Returns page `p` (without live objects) to the reserve, dirty.
    fn give_back(&mut self, p: *mut u8) {
        self.dirty.push(p);
    }

    /// On each full collection: the dirty pages idle since the previous call, and those
    /// beyond [`HOT_PAGES`] (the ones freed longest ago), go back to the
    /// system ([`system::discard`]: they leave resident memory and come back
    /// zeroed).
    fn trim_free(&mut self) {
        let k = self.stopped.max(self.dirty.len().saturating_sub(HOT_PAGES)).min(self.dirty.len());
        for p in self.dirty.drain(..k) {
            // SAFETY: page of a chunk, unused.
            unsafe { system::discard(p, PAGINA) };
            self.free_list.push(p);
        }
        self.stopped = self.dirty.len();
    }
}

impl Drop for PageReserve {
    fn drop(&mut self) {
        for &(p, bytes) in &self.chunks {
            // SAFETY: the chunk came from `reserve_chunk`; the space that used it
            // is gone.
            unsafe { system::free(p, bytes) };
        }
    }
}

/// The cache of freed large regions, by size: a program that rebuilds a
/// large object every round (the buffer, the list) reuses the region instead of
/// requesting it and returning it to the system every time. It has a ceiling
/// ([`LARGE_CACHE_CEILING`]; the oldest go first), and those that
/// stay unused from one full collection to the next go back to the system: the VM
/// keeps no large page at all (`CanUseCache`, `page.cc`), and a region that dies
/// in the minor collection comes here right away (docs/NATIVO-PLANO.md §14).
#[derive(Default)]
struct LargeRegions {
    /// `(bytes, start)`, the oldest first.
    released_list: Vec<(usize, *mut u8)>,
    /// How many of the `released_list` (those at the start) were left unused since the last
    /// [`LargeRegions::trim_free`].
    stopped: usize,
    /// Bytes of the `released_list`.
    bytes: usize,
}

impl LargeRegions {
    /// A zeroed region of `bytes` bytes, aligned to [`PAGINA`].
    fn request_fn(&mut self, bytes: usize) -> *mut u8 {
        if let Some(i) = self.released_list.iter().rposition(|&(b, _)| b == bytes) {
            let (_, p) = self.released_list.remove(i);
            if i < self.stopped {
                self.stopped -= 1;
            }
            self.bytes -= bytes;
            // SAFETY: cache region, unused.
            unsafe { std::ptr::write_bytes(p, 0, bytes) };
            return p;
        }
        system::reserve_space(bytes)
    }
    /// Keeps the region `p` of `bytes` bytes (without live objects); above the ceiling,
    /// the oldest go back to the system (one that alone exceeds it, directly).
    fn give_back(&mut self, p: *mut u8, bytes: usize) {
        if bytes > LARGE_CACHE_CEILING {
            // SAFETY: region without live objects.
            unsafe { system::free(p, bytes) };
            return;
        }
        self.released_list.push((bytes, p));
        self.bytes += bytes;
        let mut k = 0;
        while self.bytes > LARGE_CACHE_CEILING {
            let (b, q) = self.released_list[k];
            self.bytes -= b;
            // SAFETY: cache region, unused.
            unsafe { system::free(q, b) };
            k += 1;
        }
        if k > 0 {
            self.released_list.drain(..k);
            self.stopped = self.stopped.saturating_sub(k);
        }
    }
    /// The freed regions unused since the previous call go back to the system.
    fn trim_free(&mut self) {
        let k = self.stopped.min(self.released_list.len());
        for (bytes, p) in self.released_list.drain(..k) {
            self.bytes -= bytes;
            // SAFETY: cache region, unused.
            unsafe { system::free(p, bytes) };
        }
        self.stopped = self.released_list.len();
    }
}

impl Drop for LargeRegions {
    fn drop(&mut self) {
        for &(bytes, p) in &self.released_list {
            // SAFETY: cache region, unused; the space is gone.
            unsafe { system::free(p, bytes) };
        }
    }
}

/// A native attachment (`ATTACHMENT`, §2.8): the owner, the function that frees the pointer, the
/// pointer and the bytes it takes outside the heap.
#[derive(Clone, Copy)]
pub(crate) struct NativeAttachment {
    pub block: *mut Header,
    pub release: unsafe fn(*mut u8),
    pub ptr: *mut u8,
    pub bytes: usize,
}

// ─── The space ─────────────────────────────────────────────────────────────

/// The object space: pages of blocks per size class, large regions
/// and the free ranges (contiguous runs of zeroed blocks) per class, rebuilt
/// on each full collection (in address order).
pub struct ObjectSpace {
    pub(crate) pages: Vec<Page>,
    /// The page of each base (handle validation).
    map: PageMap,
    /// The ranges `[start, end)` of free blocks of each class, zeroed
    /// (header included) or with the garbage of the dead (zeroed on the way out).
    free_list: Vec<Vec<(*mut u8, *mut u8)>>,
    /// Live blocks (handed out and not returned).
    pub live_set: usize,
    /// Blocks in all pages.
    pub blocks: usize,
    /// The blocks handed out one by one by the runtime since the last collection, with the
    /// class ([`LARGE`] in the large regions): with
    /// [`ObjectSpace::ranges`], what the minor collection sweeps.
    young_set: Vec<(*mut Header, usize)>,
    /// The ranges `(start, end, class)` handed to the TLABs since the last
    /// collection.
    ranges: Vec<(*mut u8, *mut u8, usize)>,
    /// The not yet used region `[cursor, end)` of the page in use of each class
    /// (the *bump pointer*).
    region: Vec<(*mut u8, *mut u8)>,
    /// The pages without live objects kept by the full collection (indices in
    /// `pages`), to be zeroed.
    empties_list: Vec<usize>,
    /// The old objects the write barrier marked ([`REMEMBERED`]).
    pub(crate) remembered: Vec<*mut Header>,
    /// Blocks handed out since the last collection, per class.
    demand: Vec<usize>,
    /// The recent peak of occupied blocks of each class, which decays on each full
    /// collection.
    peak: Vec<usize>,
    /// The objects with an out-of-line body ([`FORA`]).
    pub(crate) with_out_of_line: Vec<*mut Header>,
    /// Whether each class's region came from a free range (garbage of the dead) or from
    /// a zeroed page.
    dirty_region: Vec<bool>,
    /// Blocks handed out and not freed of each class.
    in_use: Vec<usize>,
    /// Zero the dead already during the sweep (`--gc-stress` and verification).
    pub(crate) clear_dead: bool,
    /// Blocks handed out since the last collection (minus those returned).
    handed_out: usize,
    /// Objects of the space marked by the collection in progress.
    pub(crate) marked_set: usize,
    /// The quarantine of the dead (`DARTFORGE_GC_VENENO`); `None` without it.
    quarantine: Option<Quarantine>,
    /// Where the pages of [`PAGINA`] bytes come from.
    reserve: PageReserve,
    /// The cache of freed large regions.
    large_ones: LargeRegions,
    /// The native attachments of live blocks (or those dead since the last collection),
    /// by block address.
    attachments: crate::hash::HashMap<usize, NativeAttachment>,
    /// Bytes of the attachments.
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

    /// A zeroed page for blocks of class `class` (of one page): an
    /// empty one reused (zeroed and formatted now) or a new one. Returns the index.
    fn zeroed_page(&mut self, class: usize) -> usize {
        debug_assert!(class != LARGE);
        let words = class_words(class);
        if let Some(i) = self.empties_list.pop() {
            let p = &mut self.pages[i];
            // SAFETY: the whole page belongs to this space and has no live object.
            unsafe { std::ptr::write_bytes(p.base, 0, p.bytes) };
            self.blocks -= p.blocks;
            *p = Page::formatted(p.base, p.bytes, class, words);
            self.blocks += p.blocks;
            return i;
        }
        let base = self.reserve.request_fn();
        self.new_page(Page::formatted(base, PAGINA, class, words))
    }

    /// A zeroed large region for a body of `words` words. Returns the
    /// index.
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

    /// Removes page `i` from `pages` (the last one takes its place) and from the page
    /// map, without freeing it: returns it.
    fn remove_page(&mut self, i: usize) -> Page {
        let p = self.pages.swap_remove(i);
        self.map.remove_it(p.base as usize);
        self.blocks -= p.blocks;
        let old_value = self.pages.len();
        if i < old_value {
            // The one that was at `old_value` is now at `i`.
            self.map.insert_entry(self.pages[i].base as usize, i);
            if let Some(v) = self.empties_list.iter_mut().find(|v| **v == old_value) {
                *v = i;
            }
        }
        p
    }

    /// Returns page `p` (without live objects): to the reserve, or to the cache of
    /// large regions.
    fn release_page(&mut self, p: &Page) {
        if p.class == LARGE {
            self.large_ones.give_back(p.base, p.bytes);
        } else {
            self.reserve.give_back(p.base);
        }
    }

    /// Up to `k` zeroed and contiguous blocks of class `class` taken from its
    /// region; when it runs out, the region becomes a free range of the class or
    /// a zeroed page. Returns the start and how many. The header comes out zeroed.
    ///
    /// Free ranges keep the garbage of the dead (the sweep does not touch them): the
    /// handed-out range is zeroed here, right before the program uses it.
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
            // SAFETY: free blocks of the class, without live objects.
            unsafe { std::ptr::write_bytes(cursor, 0, j * size) };
        }
        (cursor, j)
    }

    /// A range of up to `k` free, zeroed and contiguous blocks of class
    /// `class` for the TLAB: the start and how many.
    pub(crate) fn take_range(&mut self, class: usize, k: usize) -> (*mut u8, usize) {
        debug_assert!(class != LARGE && class <= LARGEST_CLASS && k > 0);
        let (start, j) = self.take_from_region(class, k);
        self.ranges.push((start, start.wrapping_add(j * bytes_do_bloco(class_words(class))), class));
        self.demand[class] += j;
        self.handed_out += j;
        self.in_use[class] += j;
        (start, j)
    }

    /// Returns the zeroed blocks `[start, end)` of the class (the unused rest of the
    /// last TLAB range): back to the region, if it continues from there; otherwise, to the
    /// free ranges. The handed-out range shrinks accordingly.
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

    /// Returns the dead blocks `[start, end)` of the class to the free ranges, without
    /// touching them — under `--gc-stress` and with verification, zeroed right away (the
    /// [`LIVRE`] state lets the stale handle be caught, N4).
    ///
    /// # Safety
    /// `[start, end)` are blocks of the class of a page of this space, without
    /// live objects.
    unsafe fn release_range(&mut self, class: usize, start: *mut u8, end: *mut u8) {
        // With poison, each block enters the quarantine; what exceeded its capacity
        // goes back to the free list.
        if let Some(q) = self.quarantine.as_mut() {
            let size = bytes_do_bloco(class_words(class));
            let mut b = start;
            while b < end {
                // SAFETY: the function contract.
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
            // SAFETY: the function contract.
            unsafe { std::ptr::write_bytes(start, 0, end as usize - start as usize) };
        }
        self.insert_free_range(class, start, end);
    }

    /// Merges neighbours at the tip of the list before publishing the range. Each
    /// removed range enters the final interval; it neither sorts nor walks the
    /// other free ones. Never merges different classes or pages.
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

    /// Marks the old block `b` as [`REMEMBERED`] (the write barrier).
    #[inline]
    pub(crate) fn remember(&mut self, b: *mut Header) {
        // SAFETY: block of the space.
        unsafe {
            if (*b).estado == VELHO {
                (*b).estado = REMEMBERED;
                self.remembered.push(b);
            }
        }
    }

    /// Frees the out-of-line bodies and the attachments of the objects the collection did not mark
    /// (the bit in the page's mark bitmap): returns the freed bytes.
    fn release_dead_out_of_line(&mut self) -> usize {
        let mut released_set = 0;
        self.with_out_of_line.retain(|&b| {
            // SAFETY: block with an out-of-line body, of a live page.
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
            // SAFETY: block with an attachment, of a live page.
            if unsafe { marked(a.block) } {
                return true;
            }
            // SAFETY: the contract of `Heap::attach`: `release(ptr)` is the only
            // release of `ptr`, and the owner died.
            unsafe { (a.release)(a.ptr) };
            of_attachments += a.bytes;
            false
        });
        self.attachment_bytes -= of_attachments;
        released_set + of_attachments
    }

    /// Bytes of the out-of-line bodies of live objects.
    fn out_of_line_bytes(&self) -> usize {
        // SAFETY: blocks with an out-of-line body, live.
        self.with_out_of_line.iter().map(|&b| unsafe { block_size(usize::from((*body(b)).n)) }).sum()
    }

    /// Attaches `ptr` to block `b` (see `Heap::attach`).
    pub(crate) fn attach(&mut self, b: *mut Header, release: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize) {
        self.attachments.insert(b as usize, NativeAttachment { block: b, release, ptr, bytes });
        self.attachment_bytes += bytes;
    }

    /// The bytes of the attachment of `b` become `bytes` (the attachment grew or
    /// shrank); returns the difference.
    pub(crate) fn adjust_attachment(&mut self, b: *mut Header, bytes: usize) -> isize {
        let a = self.attachments.get_mut(&(b as usize)).expect("runtime bug: adjust_attachment without attachment");
        let delta = bytes as isize - a.bytes as isize;
        a.bytes = bytes;
        self.attachment_bytes = (self.attachment_bytes as isize + delta) as usize;
        delta
    }

    /// The minor collection sweep, only through the mark bitmap (marking set the
    /// bit of the reached young objects, now old): each run of unmarked young objects in a
    /// handed-out range goes back to the free ranges without being touched; the large region of
    /// a dead young object goes back to their cache. The remembered ones become old again.
    /// Returns (dead, freed bytes).
    /// The young objects the minor marking in progress reached (the promoted ones): those
    /// handed out one by one and the marked blocks of the TLAB ranges. Before
    /// [`ObjectSpace::sweep_young`] (ARC registers them,
    /// docs/ARC-IMPLEMENTACAO.md).
    pub(crate) fn young_marked(&self, f: &mut dyn FnMut(*mut Header)) {
        for &(b, _) in &self.young_set {
            // SAFETY: block handed out since the last collection, in a live page.
            if unsafe { marked(b) } {
                f(b);
            }
        }
        for &(start, end, class) in &self.ranges {
            let size = bytes_do_bloco(class_words(class));
            let mut p = start;
            while p < end {
                // SAFETY: block of the range, in a live page.
                if unsafe { marked(p.cast()) } {
                    f(p.cast());
                }
                p = p.wrapping_add(size);
            }
        }
    }

    /// Visits each block handed out since the last collection and still occupied
    /// (those one by one and those of the TLAB ranges; a block of the range the TLAB
    /// did not hand out is zeroed, [`LIVRE`]).
    pub(crate) fn young_handed_out(&self, f: &mut dyn FnMut(*mut Header)) {
        for &(b, _) in &self.young_set {
            // SAFETY: block handed out since the last collection, in a live page.
            if unsafe { (*b).estado } == JOVEM {
                f(b);
            }
        }
        for &(start, end, class) in &self.ranges {
            let size = bytes_do_bloco(class_words(class));
            let mut p = start;
            while p < end {
                let b: *mut Header = p.cast();
                // SAFETY: block of the range, in a live page.
                if unsafe { (*b).estado } == JOVEM {
                    f(b);
                }
                p = p.wrapping_add(size);
            }
        }
    }

    /// Frees block `b`, killed by RC (pure ARC, outside any
    /// sweep): the out-of-line body, the attachment, the mark bit and the block, which
    /// goes back to the class's free ranges (a large object returns the page).
    /// Returns the freed bytes.
    ///
    /// # Safety
    /// `b` is an occupied block of a page of this space, outside the young
    /// list, whose object died and that nobody reads anymore.
    pub(crate) unsafe fn release_dead(&mut self, b: *mut Header) -> usize {
        let mut released_set = 0;
        // SAFETY: the function contract.
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
            // SAFETY: the dead block of the class, without live objects.
            unsafe { self.release_range(class, start, start.wrapping_add(size)) };
        }
        released_set
    }

    /// The geometry of the page of the block of handle `h`: the address of the first
    /// block, the bytes of each and how many fit (the per-page ARC
    /// metadata table, `arc::Geometry`). `None` outside the space.
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
            // SAFETY: block handed out since the last collection, of a page
            // still alive (pages only leave in the full collection).
            unsafe {
                if marked(b) {
                    continue;
                }
                dead_set += 1;
                if class == LARGE {
                    // The region of a dead large object goes back to their cache right away
                    // (Julia's sweep of large objects, `sweep_big`): until the
                    // full collection, it would be memory that no trigger
                    // counts (docs/NATIVO-PLANO.md §14.2).
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
        // All handed-out blocks marked: no dead in the ranges.
        let all_live = self.marked_set >= self.handed_out;
        for &(start, end, class) in if all_live { &ranges[..0] } else { &ranges[..] } {
            let size = bytes_do_bloco(class_words(class));
            let mut chunk: *mut u8 = std::ptr::null_mut();
            let mut dead_in_range = 0;
            let mut p = start;
            while p < end {
                // SAFETY: block of the range, in a live page.
                if unsafe { marked(p.cast()) } {
                    if !chunk.is_null() {
                        // SAFETY: dead blocks of the range.
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
                // SAFETY: dead blocks of the range.
                unsafe { self.release_range(class, chunk, end) };
            }
            dead_set += dead_in_range;
            released_set += dead_in_range * size;
            self.in_use[class] -= dead_in_range;
        }
        self.ranges = ranges;
        self.ranges.clear();
        for b in std::mem::take(&mut self.remembered) {
            // SAFETY: remembered block, old.
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

    /// Frees the out-of-line body of the dead object of block `b`, if it has one.
    ///
    /// # Safety
    /// `b` is a block of this space whose object died.
    #[inline]
    unsafe fn release_body(b: *mut Header) {
        // SAFETY: the function contract.
        unsafe {
            if (*b).flags & FORA != 0 {
                release_out_of_line_body(body(b));
                (*b).flags &= !FORA;
            }
        }
    }

    /// A new block with a body of `words` zeroed words, class `cid`,
    /// `flags` and `n` in the header (saturated at `u16::MAX`), state `JOVEM`: the
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
        // SAFETY: free block just taken, zeroed.
        unsafe {
            (*b).estado = JOVEM;
            (*b).flags = flags;
            (*b).n = u16::try_from(n).unwrap_or(u16::MAX);
            (*b).class_id = cid;
            // Large object that is neither a string nor `INSTANCIA` (`map` unused,
            // §2.3): `map` keeps the body words, so that
            // `body_words` does not go to the page map on every read.
            if n > usize::from(u16::MAX) && flags & crate::layout::flags::FORMA != INSTANCIA && !crate::layout::cid::is_string(cid) {
                (*b).map = u32::try_from(words).unwrap_or(0);
            }
        }
        self.live_set += 1;
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    /// The block of handle `h`, if `h` is that of a block of some page of this
    /// space (live or not: the state is up to the caller).
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

    /// The handle points to an occupied block, even without zeroing dead headers.
    /// Consults free ranges and the not yet handed-out region before the header.
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
        // SAFETY: block located by this space's page index.
        let estado = unsafe { (*b).estado };
        estado != LIVRE && estado != POISON_STATE
    }

    /// Does the address of handle `h` fall in a page of this space (live or empty)?
    /// Distinguishes the handle of a collected object from a scalar used as a handle.
    pub(crate) fn in_page(&self, h: i64) -> bool {
        let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
        self.map.get(b & !(PAGINA - 1)).is_some()
    }

    /// The body words of block `b` (live, of this page): in `INSTANCIA`, those
    /// of the fields and the map; in the others, those of the header, or those of the large region
    /// when `n` saturated.
    ///
    /// # Safety
    /// `b` is a live block of a page of this space.
    #[inline]
    pub(crate) unsafe fn body_words(&self, b: *const Header) -> usize {
        // SAFETY: the function contract.
        unsafe {
            let n = usize::from((*b).n);
            if shape(b) == INSTANCIA {
                return crate::layout::palavras_de_instancia(n);
            }
            if n != usize::from(u16::MAX) {
                return n;
            }
            // A large object that is not a string keeps the words in `map` (`allocate`).
            let m = (*b).map;
            if m != 0 && !crate::layout::cid::is_string((*b).class_id) {
                return m as usize;
            }
        }
        self.region_words(b)
    }

    /// The words of the block of the large region of `b` (through the page map).
    #[cold]
    #[inline(never)]
    fn region_words(&self, b: *const Header) -> usize {
        self.map.get(b as usize & !(PAGINA - 1)).map_or(0, |i| self.pages[i].words)
    }

    /// Pushes the references of the body of `b` by format, without touching cards
    /// (verification); returns the positions walked.
    ///
    /// # Safety
    /// `b` is a live block of a page of this space.
    pub(crate) unsafe fn push_body(&self, b: *mut Header, stack: &mut Vec<i64>) -> usize {
        // SAFETY: the function contract.
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

    /// Pushes the references of a remembered object in the minor collection: `INSTANCIA` by the
    /// map; `REFS` with `CARTOES`, only the elements of the dirty cards (and
    /// clears them); `REFS` without cards, all. Returns the positions walked.
    ///
    /// # Safety
    /// `b` is a remembered block of a page of this space.
    pub(crate) unsafe fn push_remembered(&self, b: *mut Header, stack: &mut Vec<i64>) -> usize {
        // SAFETY: the function contract.
        unsafe {
            if shape(b) == REFS && (*b).flags & CARTOES != 0 {
                let n = body_refs(b, self.body_words(b));
                return push_dirty_cards(b, n, stack);
            }
            self.push_body(b, stack)
        }
    }

    /// Clears the mark bitmap of each page (the start of the full collection).
    pub(crate) fn clear_marks(&mut self) {
        for p in self.pages.iter().filter(|p| !p.empty) {
            // SAFETY: the page's mark bitmap.
            unsafe { std::ptr::write_bytes(p.base, 0, PAGE_HEAD) };
        }
    }

    /// The full collection sweep, only through the mark bitmap: a set bit is
    /// live (marking already made it old); each unmarked run goes back to the free
    /// ranges, rebuilt in address order, without the blocks being read. A
    /// page without live objects stays whole as empty while the free blocks of its class
    /// do not exceed the live ones, the cycle's demand and the recent peak (and two
    /// pages); the others go back to the reserve. A large region without live objects goes back to
    /// their cache. Returns (dead, live, live bytes).
    pub(crate) fn sweep(&mut self) -> (usize, usize, usize) {
        let (mut live_set, mut live_bytes) = (0, 0);
        for r in self.region.iter_mut() {
            *r = (std::ptr::null_mut(), std::ptr::null_mut());
        }
        self.release_dead_out_of_line();
        // Per page: the run `[de, until)` of `free_ranges` with its free
        // ranges and the live ones.
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
            // The dead in quarantine (`DARTFORGE_GC_VENENO`): they are not live,
            // but they do not go back to the free list either, and they hold the page.
            let mut pinned_in_page = 0;
            let mut free_ptr: *mut u8 = std::ptr::null_mut();
            let mut q = p.start();
            for _ in 0..p.blocks {
                // SAFETY: block of the page.
                let alive = unsafe { marked(q.cast()) };
                // SAFETY: unmarked block of the page.
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
                    // SAFETY: blocks without live objects of the page.
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
            // The third field decides whether the page stays: the held ones keep it.
            fragments.push((de, free_ranges.len(), live_in_page + pinned_in_page));
        }
        live_bytes += self.out_of_line_bytes() + self.attachment_bytes;
        let dead_set = self.live_set.saturating_sub(live_set);
        for (c, p) in self.peak.iter_mut().enumerate() {
            // The occupied blocks (live and dead of this collection): the size the
            // heap had in this cycle.
            *p = (*p - *p / 8).max(self.in_use[c]);
        }
        self.in_use.copy_from_slice(&class_live);
        // The empty pages beyond the slack go back to the reserve (the last ones
        // first: the ranges prefer low addresses); those already empty from
        // before count as slack for all classes.
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
                // Empty since the previous collection and not reused: surplus.
                if stored >= 2 {
                    released_one[i] = true;
                } else {
                    stored += 1;
                }
                continue;
            }
            // The slack: what survived, what was allocated since the last collection
            // (the demand of the next cycle), the recent peak and two pages.
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
        // The low-address ranges go first (`pop`).
        for l in self.free_list.iter_mut() {
            l.reverse();
        }
        // The quarantine: what exceeded the capacity goes back to the free list.
        if let Some(q) = self.quarantine.as_mut() {
            for (c, b) in q.released_set() {
                let t = bytes_do_bloco(class_words(c));
                self.free_list[c].push((b, b.wrapping_add(t)));
            }
        }
        // The low-address empty pages go first (`pop`).
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

    /// Zeroes the free ranges and the dirty rest of the regions: after that, every block
    /// without an object has the [`LIVRE`] state.
    fn clear_free(&mut self) {
        for ranges in &self.free_list {
            for &(start, end) in ranges {
                // SAFETY: free blocks of a page of the space.
                unsafe { std::ptr::write_bytes(start, 0, end as usize - start as usize) };
            }
        }
        for (c, &(cursor, end)) in self.region.iter().enumerate() {
            if self.dirty_region[c] && !cursor.is_null() {
                // SAFETY: free blocks of the class region.
                unsafe { std::ptr::write_bytes(cursor, 0, end as usize - cursor as usize) };
            }
        }
    }

    /// Visits the block of each live object (or dead since the last collection).
    pub(crate) fn for_each_live(&mut self, mut f: impl FnMut(*mut Header)) {
        self.clear_free();
        for p in self.pages.iter().filter(|p| !p.empty) {
            for j in 0..p.blocks {
                let b = p.block(j);
                // SAFETY: block of the page.
                let estado = unsafe { (*b).estado };
                if estado != LIVRE && estado != POISON_STATE {
                    f(b);
                }
            }
        }
    }

    /// How many pages and regions (for the collection trace).
    pub(crate) fn n_pages(&self) -> usize {
        self.pages.len()
    }

    /// Bytes of the large regions in the cache (outside the live heap).
    #[allow(dead_code)]
    pub(crate) fn cached_bytes(&self) -> usize {
        self.large_ones.bytes
    }

    /// The memory breakdown of the space (`DARTFORGE_GC_MEMORIA=1`): per size
    /// class, the pages, the blocks and the occupied ones (the live ones of the last full
    /// collection plus those handed out since then); the empty pages, those of the
    /// reserve, the live and cached large regions, the out-of-line bodies and the
    /// attachments. One line per item, in KiB.
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
                // SAFETY: the region's block, readable.
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
                "  class {c:>2} ({size:>5} B): {k:>3} pages, {occupied:>6}/{:>6} blocks ({}%)",
                k * per_page,
                occupied * 100 / (k * per_page).max(1)
            ));
        }
        let _ = writeln!(
            s,
            "[memory] class pages: {total_pages} ({} KiB), occupied {} KiB ({}%), classes with a page: {}",
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
            "[memory] kept empty pages: {empties_list} ({} KiB); reserve: {} free, {} dirty ({} KiB dirty)",
            kib(empties_list * PAGINA),
            self.reserve.free_list.len(),
            self.reserve.dirty.len(),
            kib(self.reserve.dirty.len() * PAGINA)
        );
        let _ = writeln!(
            s,
            "[memory] large regions: {large_ones} in the space ({} KiB; {dead_large} dead, {} KiB), {} in cache ({} KiB); out-of-line bodies: {} ({} KiB); attachments: {} ({} KiB)",
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
            "[memory] space vectors: pages {} KiB, free {} ranges, young {} KiB, ranges {} KiB, remembered {} KiB",
            kib(self.pages.capacity() * std::mem::size_of::<Page>() + self.map.regions.len() * 4 * (1 << PageMap::BITS)),
            self.free_list.iter().map(Vec::len).sum::<usize>(),
            kib(self.young_set.capacity() * 16),
            kib(self.ranges.capacity() * 24),
            kib(self.remembered.capacity() * 8)
        );
        s
    }
}

/// The resident memory and the peak of the process, and the committed bytes of the
/// process heap (the Rust allocator's), in bytes (`DARTFORGE_GC_MEMORIA=1`).
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
        // SAFETY: structures of the given size.
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
            // SAFETY: block with an out-of-line body (from the system allocator).
            unsafe { Self::release_body(b) };
        }
        for (_, a) in std::mem::take(&mut self.attachments) {
            // SAFETY: the contract of `Heap::attach`; the space is gone.
            unsafe { (a.release)(a.ptr) };
        }
        for p in std::mem::take(&mut self.pages) {
            // Those of `PAGINA` bytes leave with the reserve (its `Drop`); the
            // large regions, directly to the system.
            if p.class == LARGE {
                // SAFETY: region from `system::reserve_space`, unused.
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
                // SAFETY: occupied block handed out above, not used afterwards.
                unsafe { e.release_dead(blocks[i]) };
            }
            assert_eq!(e.live_set, 1);
            assert_eq!(e.free_list[class].len(), 1);
            e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
            let (start, n) = e.take_from_region(class, TLAB_BLOCKS);
            assert_eq!(start, blocks[0].cast());
            assert_eq!(n, 3);
            // SAFETY: the returned range is reserved, and the fourth block is alive.
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
        // SAFETY: only the ends died; the central block is still alive.
        unsafe {
            e.release_dead(blocks[0]);
            e.release_dead(blocks[2]);
        }
        assert_eq!(e.free_list[class].len(), 2);
        e.region[class] = (std::ptr::null_mut(), std::ptr::null_mut());
        let (_, n) = e.take_from_region(class, TLAB_BLOCKS);
        assert_eq!(n, 1);
        // SAFETY: the central object was neither freed nor reused.
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
            // SAFETY: occupied block, without live objects.
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
        // SAFETY: the last block stays poisoned, outside the free ranges.
        unsafe { assert_eq!((*blocks[2]).estado, POISON_STATE) };
    }
}
