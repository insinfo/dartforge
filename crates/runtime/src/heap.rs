//! Precise heap with iterative tracing, explicit roots and observable counters.
//! Cells, environments, closures and lists are infrastructure: they do not yet imply
//! Dart lowering to LLVM. The detailed contract is in CONTRACT.md in this crate.

/// Cumulative work counters; they do not represent physical allocator bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeapStats {
    pub allocations: u64,
    pub collections: u64,
    pub reclaimed: u64,
    pub roots_scanned: u64,
    pub slots_scanned: u64,
    pub live_objects: usize,
    pub reserved_slots: usize,
    /// Canonical enum values kept alive until the runtime shuts down.
    pub permanent_roots: usize,
    pub live_roots: usize,
    pub peak_roots: usize,
    pub root_slots: usize,
    pub peak_root_slots: usize,
    /// Live headers and payload capacities, without auxiliary metadata/RSS.
    pub estimated_bytes: usize,
    pub peak_estimated_bytes: usize,
    /// `int`s in a `Ref` position that became a `Smi` (R10) instead of a box: without the
    /// `Smi`, each would be an allocation. `allocations + boxes_avoided` is the
    /// count from before R10, exactly.
    pub boxes_avoided: u64,
}

/// A bounded policy for automatic collection, independent of RC/destruction.
#[derive(Debug)]
struct AutomaticGcPolicy {
    origin: crate::clock::Instant,
    window: Option<(crate::clock::Instant, usize)>,
    debt: bool,
    idle_requested: bool,
}
impl Default for AutomaticGcPolicy {
    fn default() -> Self {
        Self {
            origin: crate::clock::Instant::now(),
            window: None,
            debt: false,
            idle_requested: false,
        }
    }
}
impl AutomaticGcPolicy {
    fn refresh(&mut self, now: crate::clock::Instant) {
        if self.window.is_some_and(|(deadline, _)| now >= deadline) {
            self.window = None;
        }
    }
    fn arm(
        &mut self,
        now: crate::clock::Instant,
        duration: std::time::Duration,
        budget: usize,
    ) -> i64 {
        if duration.is_zero() || budget == 0 {
            return -1;
        }
        let Some(deadline) = now.checked_add(duration) else {
            return -1;
        };
        self.refresh(now);
        if self.debt {
            return 0;
        }
        self.window = Some(match self.window {
            Some((old_deadline, remaining)) => (old_deadline.min(deadline), remaining.min(budget)),
            None => (deadline, budget),
        });
        1
    }
    fn charge(&mut self, bytes: usize) {
        if let Some((deadline, remaining)) = self.window {
            if bytes >= remaining {
                self.window = None;
                self.debt = true;
            } else {
                self.window = Some((deadline, remaining - bytes));
            }
        }
    }
    fn collect_before(
        &mut self,
        now: crate::clock::Instant,
        bytes: usize,
        ordinary: bool,
        forced: bool,
    ) -> bool {
        self.refresh(now);
        if forced {
            return true;
        }
        if self.debt && self.window.is_none() {
            return true;
        }
        if let Some((_, remaining)) = self.window {
            if bytes >= remaining {
                self.window = None;
                self.debt = true;
                return true;
            }
            if ordinary {
                self.debt = true;
            }
            return false;
        }
        ordinary
    }
    fn cancel(&mut self) {
        self.window = None;
    }
    fn serviced(&mut self) {
        self.window = None;
        self.debt = false;
        self.idle_requested = false;
    }
    fn is_active(&self) -> bool {
        self.window.is_some() || self.debt
    }
    fn blocks_tlabs(&self) -> bool {
        self.debt
    }
}

// `Text`, `StringMut`, `UnitsIter` and `push_point` live in `text.rs` (P1,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.3).
pub use crate::text::{StringMut, Text, UnitsIter, push_point};

/// An object field: the bits and whether they are a reference (what the collector follows).
pub type Field = (i64, bool);

// The layout contract (docs/NATIVO-ESPACO-UNIFICADO.md §3.1) lives in
// `crate::layout`; `heap` re-exports it for those who already used it from here.
pub use crate::layout::{
    HANDLE_OFFSET, Header, INLINE_FIELDS, LARGEST_CLASS, PAGE, PAGE_HEAD, Ref, TLAB_N, capacity,
    is_object, map_words, smi,
};

// The header ([`Header`], `crate::layout`) of a user object in the
// object space ([`ObjectSpace`]) is followed by the fields (8 bytes each,
// the bits) and, with more than 32 fields, by the words that extend the reference
// map. Fixed C layout: a contract with the emitter (`llvm/mod.rs`).
//
// The object handle is the header address with bit 1 set (`block +
// 2`, [`DESLOCAMENTO_DO_HANDLE`]); blocks are 8-aligned and the `Smi` is
// odd (R10), so `h & 7 == 2` ([`is_object`]) identifies the object without a
// lookup.
//
// As in the VM, the field does not carry the reference mark: the **map** (`map`,
// one bit per field, and the extension words) says which fields the collector
// follows — the VM takes that from the class (the *unboxed fields bitmap*); here each
// write sets or clears the field's bit, so a field that the representation
// sometimes makes `Ref` and sometimes scalar stays precise.
//
// An object whose number of fields changed after creation (the error that gains
// the stack trace, the JIT reload that changes the class layout, J03) cannot change
// address: the fields go to an **out-of-line body** (bit [`OUT_OF_LINE`] in
// `flags`), with the same layout (header + fields + extension), whose address
// sits in place of the first field. Generated code reads the out-of-line body with a
// branchless select (`llvm/mod.rs`).

/// States of a block ([`Header::state`]; `crate::layout::state`). The
/// collector is generational without moving objects ("sticky" marks): an object is born
/// [`YOUNG`] (inline allocation writes 1); one that survives a minor collection becomes a
/// survivor (still young: [`SURVIVOR`] or [`SURVIVOR_B`], alternating per minor
/// collection), and one that survives the next one (or a full one) becomes
/// [`OLD`] — the aging, so that a structure under construction at a minor collection does
/// not die old. The minor collection only marks and sweeps the young; old ones count as
/// live, and old-to-young references are found through the [`REMEMBERED`] ones
/// — the write barrier (`Heap::get_mut` and the runtime's field writes,
/// and the generated code's one after writing a `Ref` into a field,
/// `llvm/mod.rs`) marks the old object that receives a reference, like the VM's *store
/// buffer*. [`PERMANENT`] is the static object, outside the heap.
pub const FREE: u8 = crate::layout::state::FREE;
pub const YOUNG: u8 = crate::layout::state::YOUNG;
pub const SURVIVOR: u8 = crate::layout::state::SURVIVOR;
pub const SURVIVOR_B: u8 = crate::layout::state::SURVIVOR_B;
pub const OLD: u8 = crate::layout::state::OLD;
pub const REMEMBERED: u8 = crate::layout::state::REMEMBERED;
pub const PERMANENT: u8 = crate::layout::state::PERMANENT;
/// Bit of [`Header::flags`]: the fields live in an out-of-line body.
pub const OUT_OF_LINE: u8 = crate::layout::flags::OUT_OF_LINE;

// [`PAGE`] is the page size of the object space (and its alignment:
// the page of a handle is `h & !(PAGE - 1)`). [`PAGE_HEAD`] are the
// bytes at the start of each page with the mark bitmap: one bit per 8-byte word
// of the page (a block's bit is that of the word where it starts). Marking
// sets the bit of the reached object; sweeps read only the bits, without touching the
// blocks, live or dead (the VM also marks outside the object in the *old space*,
// `marking.cc`, through the header bit; here the bitmap sits alongside, per page,
// like Immix's).

// The object space (pages, size classes, large regions, mark
// bitmap, cards, attachments) lives in `crate::space`. [`LARGEST_CLASS`] and
// [`TLAB_N`] count body words (docs/NATIVO-ESPACO-UNIFICADO.md §2.6): the
// TLAB of `w` words is `Context::tlab[w]`, `w` in `1..=TLAB_N`.
use crate::layout::{block_bytes, flags as block_flags};
pub use crate::space::{ObjectSpace, block_size};
use crate::space::{
    TLAB_BLOCKS, allocation_bytes, body, body_refs, fields_of, is_reference, mark_block,
    mark_reference, marked, new_out_of_line_body, release_out_of_line_body, shape,
};

/// The minor collection comes every `YOUNG_LIMIT` allocated bytes (the size of the
/// *new space*; the VM's semispace starts smaller and grows up to 8 MiB on 64
/// bits — fixed here, between the cost per collection and the memory peak). 4 MiB
/// since 2026-10-27 (macOS arm64, cycles against 2 MiB): binary trees −4%,
/// `objects_in_collections` −4%, `fresh_list_append` −14%, JSON and strings −1 to −2%,
/// the keep-alive HTTP server −3% instructions per request, for about 2 MiB more of
/// resident memory; `temporary_objects` +1%. In 2026-09 the same test had chosen 2 MiB
/// because small JSON decoding lost, which no longer holds…
const YOUNG_LIMIT: usize = 4 * 1024 * 1024;
/// How far the young limit grows when the minor collections keep finding many
/// survivors ([`Heap::adapt_young_limit`]), like the VM's semispace, which doubles
/// while a scavenge sees less than 90% garbage (`Scavenger::NewSizeInWords`).
/// 8 MiB: binary trees −5% time per round (16 MiB gained nothing over it).
const YOUNG_MAX: usize = 8 * 1024 * 1024;
/// The survival above which a minor collection doubles the young limit: more than
/// one block in `YOUNG_GROWTH_SURVIVAL` reached (10%, the VM's
/// `new_gen_garbage_threshold` of 90).
const YOUNG_GROWTH_SURVIVAL: usize = 10;
/// How much the heap grows until the next full collection, in % of what
/// survived the last one: 150 (double, 200, cost ~15 MB more peak in
/// `objetos_escapam` with no time gain, since the sweep only reads the mark
/// bitmap).
const GROWTH: usize = 150;
/// … or every `YOUNG_COUNT` allocations (small runtime values).
const YOUNG_COUNT: usize = 256 * 1024;
/// Under `--gc-stress`, one full collection every so many (the others minor:
/// that is where a missing barrier would show up) …
const MINORS_PER_FULL_UNDER_STRESS: u64 = 8;
/// … or, with a lot of live data, every `work / TRABALHO_POR_MENOR_NO_ESTRESSE`
/// (the marking work of the last full one: objects and reference words
/// visited): a full one costs the whole live heap, and one every 8
/// allocations made stress quadratic (program 99, ~1 s without stress,
/// went over 6 min; a live list of 40 thousand elements cost 100 µs per
/// allocation). This way the cost of full collections per allocation stays at ~64 words
/// visited; the minor one still comes before every allocation.
const WORK_PER_MINOR_UNDER_STRESS: usize = 64;
/// How many emptied frame slot vectors [`Heap::pop_frame`] keeps for reuse.
const FRAME_POOL: usize = 32;
/// The longest pause of aging (in minor collections) when the survivors keep
/// living on ([`Heap::run_collection`]).
const AGING_MAX_PAUSE: u32 = 16;
/// Below this many survivors, their survival rate does not change the aging.
const AGING_MIN_SAMPLE: usize = 1024;

/// Nobody's object: zeroed header and zeroed fields. The inline read
/// of a field of something that is not an object of the space reads from here (what
/// `dartforge_object_get` returned); it is never written — it is in read-only
/// memory, and writing to it is a compiler bug that ends the process.
#[repr(C)]
pub struct EmptyObject {
    pub header: Header,
    pub fields: [i64; INLINE_FIELDS],
}
pub static EMPTY_OBJECT: EmptyObject = EmptyObject {
    header: Header {
        state: FREE,
        flags: 0,
        n: 0,
        class_id: 0,
        map: 0,
        metadata: 0,
    },
    fields: [0; INLINE_FIELDS],
};

/// The runtime's view of an object of the space ([`Heap::object`]): the
/// class and the fields, `(bits, is reference)`.
#[derive(Clone, Copy)]
pub struct Obj<'a> {
    body: *const Header,
    /// The object's class.
    pub class_id: i64,
    _heap: std::marker::PhantomData<&'a Heap>,
}

#[allow(unsafe_code)]
impl Obj<'_> {
    /// How many fields.
    pub fn len(&self) -> usize {
        // SAFETY: body alive while the heap borrow lasts.
        usize::from(unsafe { (*self.body).n })
    }
    /// No fields?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Field `i`: the bits and whether it is a reference.
    ///
    /// # Panics
    /// With `i` outside the object.
    pub fn field(&self, i: usize) -> Field {
        assert!(
            i < self.len(),
            "field {i} outside the object of {} fields",
            self.len()
        );
        // SAFETY: `i < n`; live body.
        unsafe {
            (
                *fields_of(self.body.cast_mut()).add(i),
                is_reference(self.body, i),
            )
        }
    }
    /// Field `i`, if it exists.
    pub fn get(&self, i: usize) -> Option<Field> {
        (i < self.len()).then(|| self.field(i))
    }
    /// The first field, if it exists.
    pub fn first(&self) -> Option<Field> {
        self.get(0)
    }
    /// The fields, in order.
    pub fn iter(&self) -> impl Iterator<Item = Field> + '_ {
        (0..self.len()).map(|i| self.field(i))
    }
    /// The copied fields.
    pub fn to_vec(&self) -> Vec<Field> {
        self.iter().collect()
    }
}

impl std::fmt::Debug for Obj<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Obj")
            .field("class_id", &self.class_id)
            .field("campos", &self.to_vec())
            .finish()
    }
}

/// The survivor state that alternates with `s` ([`SURVIVOR`] ↔ [`SURVIVOR_B`]).
#[inline(always)]
fn other_survivor_state(s: u8) -> u8 {
    if s == SURVIVOR { SURVIVOR_B } else { SURVIVOR }
}

/// Was handle `h` marked by the collection in progress (live)? Null, `Smi` and
/// any scalar: no.
///
/// In the minor collection, an old object counts as live, as does a young one in the
/// survivor state `kept` that the marking gave it (with aging; otherwise `OLD`).
fn marked_in_collection(objects: &ObjectSpace, minor: bool, kept: u8, h: i64) -> bool {
    if is_object(h) {
        // SAFETY: block of a page of the space.
        #[allow(unsafe_code)]
        return match objects.block_of(h) {
            Some(b) => unsafe {
                if minor {
                    let e = (*b).state;
                    e == OLD || e == REMEMBERED || e == kept
                } else {
                    marked(b)
                }
            },
            // A static (image section) never dies.
            None => is_static(h),
        };
    }
    false
}

// `is_object` and the `smi` module (the `Ref` tag, R10) live in
// `crate::layout` and are re-exported at the start of this module.

/// A native finalizer and its data (`Dart_NewFinalizableHandle`).
pub type Finalizer = (fn(usize), usize);

/// The root frame of a generated function (G1), on its stack: the
/// previous one, the number of slots and the slots, which the generated code writes with a
/// plain `store` (LLVM's shadow stack, `ShadowStackGC`). The runtime only
/// links and unlinks the frame ([`stack_frame`]) and walks it during
/// collection; the runtime's own frames remain in `Heap::frames`.
#[repr(C)]
pub struct RootFrame {
    previous: *const RootFrame,
    n: i64,
    slots: [i64; 0],
}

/// macOS arm64: the byte offset, from the base of the thread's TSD
/// (`TPIDRRO_EL0`), of the `pthread` key slot that holds the address of the
/// thread's [`Context`] — `key * 8`, what `pthread_getspecific` reads; 0 until
/// the first `dartforge_context` creates the key. The production prologue reads
/// the slot directly (`emit_native::sdk_module::with_context_in_tls`), like Go
/// reads its `g`, instead of a `thread_local` variable, whose every access
/// goes through `tlv_get_addr`; an empty slot, or 0 here, goes to
/// `dartforge_context`, which fills the slot of the thread (`gc_roots.rs`).
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(unsafe_code, non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dartforge_context_slot: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// The heap's ARC state (`--memory=arc`). A WebAssembly module never has it
/// (the option is native only): there every query answers "off" as a constant,
/// so the optimizer drops the ARC paths from the module.
#[derive(Debug)]
pub(crate) struct ArcSlot(Option<Box<HeapArc>>);

impl ArcSlot {
    pub(crate) const NONE: ArcSlot = ArcSlot(None);

    #[inline(always)]
    fn get(&self) -> Option<&HeapArc> {
        if cfg!(target_arch = "wasm32") {
            None
        } else {
            self.0.as_deref()
        }
    }

    #[inline(always)]
    pub(crate) fn is_some(&self) -> bool {
        self.get().is_some()
    }

    #[inline(always)]
    pub(crate) fn as_ref(&self) -> Option<&HeapArc> {
        self.get()
    }

    #[inline(always)]
    pub(crate) fn as_mut(&mut self) -> Option<&mut Box<HeapArc>> {
        if cfg!(target_arch = "wasm32") {
            None
        } else {
            self.0.as_mut()
        }
    }

    #[inline(always)]
    pub(crate) fn as_deref_mut(&mut self) -> Option<&mut HeapArc> {
        self.as_mut().map(|a| &mut **a)
    }

    #[inline(always)]
    pub(crate) fn take(&mut self) -> Option<Box<HeapArc>> {
        if cfg!(target_arch = "wasm32") {
            None
        } else {
            self.0.take()
        }
    }

    pub(crate) fn set(&mut self, arc: Box<HeapArc>) {
        self.0 = Some(arc);
    }
}

/// A [`Context`] field the generated code reads at a fixed offset: an 8-byte cell
/// on every target. On wasm32 the 4-byte pointer or `usize` sits in the low half
/// (little-endian) and the high half stays zero, so both the `load ptr` and the
/// `load i64` of the generated code read it, and the offsets and the IR are the
/// same on every target (docs/PLANO-WASM-MVP.md §3.2).
#[repr(C, align(8))]
pub struct Slot<T> {
    value: T,
    #[cfg(target_pointer_width = "32")]
    high: u32,
}

impl<T> Slot<T> {
    pub const fn new(value: T) -> Self {
        Slot {
            value,
            #[cfg(target_pointer_width = "32")]
            high: 0,
        }
    }
}

impl<T> std::ops::Deref for Slot<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

const _: () = assert!(std::mem::size_of::<Slot<*const u8>>() == 8);

/// The thread context that the generated code reads and writes directly, without
/// a call (`llvm/mod.rs`): the pending exception (mirror of
/// `exceptions::EXCEPTION`, checked after each call) and the top of
/// this thread's shadow stack (of its isolate). The address comes from
/// `dartforge_context`, once per activation; it does not change while the
/// thread lives. The offsets are a contract with the emitter.
#[repr(C)]
pub struct Context {
    /// 1 with a pending exception (offset 0).
    pub pending: std::cell::Cell<u8>,
    /// The top frame of the shadow stack (offset 8).
    pub top: Slot<std::cell::Cell<*const RootFrame>>,
    /// The globals area of each module in this isolate, by module id
    /// (`@df.area_id`; 0 = no id): the fast path of the function
    /// prologue (`@df.get_area`). Address of the elements (offset 16)
    /// and length (24); a null entry goes to the runtime.
    pub areas: Slot<std::cell::Cell<*const *mut i64>>,
    pub n_areas: Slot<std::cell::Cell<usize>>,
    /// The isolate's interrupt request (offset 32): the control port
    /// sets it, from another thread, when a control message arrives
    /// (`ports.rs`). The generated code reads it, atomically, at the start of each loop
    /// iteration (the J01 safepoint).
    pub interrupt: std::sync::atomic::AtomicU8,
    /// The header of [`EMPTY_OBJECT`] (offset 40): the inline read
    /// of a field of something that is not an object of the space uses this object
    /// instead of the header at `h - 2` (`llvm/mod.rs`), and reads zeros, as
    /// `dartforge_object_get` did.
    pub empties: Slot<*const Header>,
    /// The classes with a registered method table (offset 48; one
    /// 0/1 byte per id, `selectors.rs`) and how many (56): the inline allocation
    /// of a class that registers the table on the first allocation
    /// (`dartforge_object_new_t`) only skips the runtime after registration.
    pub registered: Slot<std::cell::Cell<*const u8>>,
    pub n_registered: Slot<std::cell::Cell<usize>>,
    /// The isolate's TLAB (offset 64): for each number of fields
    /// `n ≤ TLAB_N`, a range `[cursor, end)` of contiguous free blocks of the
    /// object space already counted as allocation
    /// (`Heap::refill_tlab`): the cursor at `64 + 16n`, the end at
    /// `72 + 16n`. The generated code allocates by advancing the cursor one block (the
    /// VM's *bump pointer*, `llvm/mod.rs`); an exhausted range goes to the runtime,
    /// which refills it.
    pub tlab: [[Slot<std::cell::Cell<*mut u8>>; 2]; TLAB_N + 1],
    /// The subtype bitmaps (`selectors.rs`, fragment `core.rs`): for
    /// each target class `C` (index; offset 336), the address of the bitmap
    /// in which bit `cid` says whether `cid <: C` (null = not computed yet), and
    /// how many targets (344); the width of all bitmaps in bits (352): a
    /// `cid` outside it goes to the runtime (`dartforge_is_subclass`, which also
    /// builds the bitmap on the first lookup). A new subclass registration
    /// (loading, a reload) clears the bitmaps.
    pub subtypes: Slot<std::cell::Cell<*const *const u64>>,
    pub n_subtypes: Slot<std::cell::Cell<usize>>,
    pub subtype_width: Slot<std::cell::Cell<usize>>,
    /// The handles (address + 2) of the static `true` (offset
    /// 360) and `false` (368) boxes of this runtime: `@df.box_bool` and
    /// `@df.unbox_bool` read them from here, without referencing the
    /// `dartforge_true`/`_falso` data of another image (the executable and the SDK
    /// DLL each have their own copy; only the active runtime's is canonical).
    pub true_value: Slot<*const u8>,
    pub false_value: Slot<*const u8>,
    /// The thread's native stack limit (offset 376): the prologue of
    /// each function that calls another compares the address of its frame with it
    /// and, below it, throws `StackOverflowError` instead of overflowing the system
    /// stack (the VM's `stack_overflow_check`). 0 = not computed yet
    /// (fragment `gc_roots.rs`, `dartforge_context`); 1 = no known limit.
    pub stack_limit: Slot<std::cell::Cell<usize>>,
    /// The thread's [`ThreadTables`] (offset 384, `layout::context::TABLES`),
    /// set by the first `dartforge_context` of the thread, before any generated
    /// code reads it.
    pub tables: Slot<std::cell::Cell<*const ThreadTables>>,
}

/// The lookup tables of a thread that the generated code reads through
/// [`Context::tables`] (`layout::thread_tables`): a thread-local apart from the
/// [`Context`], all zeros at the start, so that it stays out of the executable's
/// thread-local template.
#[repr(C)]
pub struct ThreadTables {
    /// The type-check memo (`layout::rti_memo`): written by the runtime's cached
    /// `is`, `P<i>` and stored-type paths (and by the generated `P<i>` with a
    /// tuple), read by the generated code when its per-site cache misses.
    /// Zeroed with the runtime's memos.
    pub rti_memo: [[std::cell::Cell<i64>; 4]; crate::layout::rti_memo::N],
    /// The one-character Latin-1 strings already returned by `String_charAt` on
    /// this thread: the canonical literals (`Heap::string_literal`, permanent),
    /// 0 = not requested yet. The generated `String.[]` reads them inline
    /// (`@df.string_one_char`) and only calls the native for an empty entry, a
    /// non-Latin-1 unit or an index out of range.
    pub one_char: [std::cell::Cell<i64>; 256],
}

const _: () = {
    use crate::layout::thread_tables as t;
    assert!(std::mem::offset_of!(ThreadTables, rti_memo) == t::RTI_MEMO);
    assert!(std::mem::offset_of!(ThreadTables, one_char) == t::ONE_CHAR);
    assert!(
        std::mem::size_of::<[[std::cell::Cell<i64>; 4]; crate::layout::rti_memo::N]>()
            == crate::layout::rti_memo::N * crate::layout::rti_memo::ENTRY
    );
};

thread_local! {
    /// This thread's [`ThreadTables`].
    pub static TABLES: ThreadTables = const {
        ThreadTables {
            rti_memo: [const { [const { std::cell::Cell::new(0) }; 4] }; crate::layout::rti_memo::N],
            one_char: [const { std::cell::Cell::new(0) }; 256],
        }
    };
}

impl ThreadTables {
    /// Stores `[a, b, kind] -> answer` in [`ThreadTables::rti_memo`].
    #[inline]
    pub(crate) fn rti_memo_store(&self, a: i64, b: i64, kind: i64, answer: i64) {
        use crate::layout::rti_memo::{WAYS, set};
        const _: () = assert!(WAYS == 2);
        let first = set(a, b, kind) * WAYS;
        let (way0, way1) = (&self.rti_memo[first], &self.rti_memo[first + 1]);
        for (old, new) in way1.iter().zip(way0) {
            old.set(new.get());
        }
        for (w, v) in way0.iter().zip([a, b, kind, answer]) {
            w.set(v);
        }
    }

    /// Empties [`ThreadTables::rti_memo`].
    pub(crate) fn rti_memo_clear(&self) {
        for e in &self.rti_memo {
            for w in e {
                w.set(0);
            }
        }
    }
}

/// The `is C` read inline by the generated code ([`Context::subtypes`]). To
/// measure, `DARTFORGE_NO_INLINE_CLASS=1` does not publish the bitmaps: the generated
/// code always goes to the runtime (the answer is the same).
pub fn inline_class() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| crate::console::env_var_os("DARTFORGE_NO_INLINE_CLASS").is_none())
}

const _: () = {
    use crate::layout::context as c;
    assert!(std::mem::offset_of!(Context, pending) == c::PENDING);
    assert!(std::mem::offset_of!(Context, top) == c::TOP);
    assert!(std::mem::offset_of!(Context, areas) == c::AREAS);
    assert!(std::mem::offset_of!(Context, n_areas) == c::N_AREAS);
    assert!(std::mem::offset_of!(Context, interrupt) == c::INTERRUPT);
    assert!(std::mem::offset_of!(Context, empties) == c::EMPTIES);
    assert!(std::mem::offset_of!(Context, registered) == c::REGISTERED);
    assert!(std::mem::offset_of!(Context, n_registered) == c::N_REGISTERED);
    assert!(std::mem::offset_of!(Context, tlab) == c::TLAB);
    assert!(std::mem::offset_of!(Context, subtypes) == c::SUBTYPES);
    assert!(std::mem::offset_of!(Context, n_subtypes) == c::N_SUBTYPES);
    assert!(std::mem::offset_of!(Context, subtype_width) == c::SUBTYPES_WIDTH);
    assert!(std::mem::offset_of!(Context, true_value) == c::TRUE);
    assert!(std::mem::offset_of!(Context, false_value) == c::FALSE);
    assert!(std::mem::offset_of!(Context, stack_limit) == c::STACK_LIMIT);
    assert!(std::mem::offset_of!(Context, tables) == c::TABLES);
};

thread_local! {
    pub static CONTEXT: Context = const {
        Context {
            pending: std::cell::Cell::new(0),
            top: Slot::new(std::cell::Cell::new(std::ptr::null())),
            areas: Slot::new(std::cell::Cell::new(std::ptr::null())),
            n_areas: Slot::new(std::cell::Cell::new(0)),
            interrupt: std::sync::atomic::AtomicU8::new(0),
            empties: Slot::new(&raw const EMPTY_OBJECT.header),
            registered: Slot::new(std::cell::Cell::new(std::ptr::null())),
            n_registered: Slot::new(std::cell::Cell::new(0)),
            tlab: [const { [Slot::new(std::cell::Cell::new(std::ptr::null_mut())), Slot::new(std::cell::Cell::new(std::ptr::null_mut()))] }; TLAB_N + 1],
            subtypes: Slot::new(std::cell::Cell::new(std::ptr::null())),
            n_subtypes: Slot::new(std::cell::Cell::new(0)),
            subtype_width: Slot::new(std::cell::Cell::new(0)),
            true_value: Slot::new((&raw const dartforge_true).cast::<u8>().wrapping_add(HANDLE_OFFSET as usize)),
            false_value: Slot::new((&raw const dartforge_false).cast::<u8>().wrapping_add(HANDLE_OFFSET as usize)),
            stack_limit: Slot::new(std::cell::Cell::new(0)),
            tables: Slot::new(std::cell::Cell::new(std::ptr::null())),
        }
    };
}

/// Links `q` at the top of the shadow stack. `q.n` and the (zeroed) slots were
/// already written by the generated code.
///
/// # Safety
/// `q` points to a valid frame that lives until the matching
/// [`unstack_frame`].
#[allow(unsafe_code)]
pub unsafe fn stack_frame(q: *mut RootFrame) {
    CONTEXT.with(|c| {
        // SAFETY: `q` is the frame on the stack of the calling function, alive until the
        // `unstack_frame` before each of its returns.
        unsafe { (*q).previous = c.top.get() };
        c.top.set(q);
    });
}

/// Unlinks `q`, which must be the top (returns close the frames
/// in order, G3).
///
/// # Safety
/// `q` is the valid frame passed to the last [`stack_frame`].
#[allow(unsafe_code)]
pub unsafe fn unstack_frame(q: *const RootFrame) {
    CONTEXT.with(|c| {
        assert!(
            std::ptr::eq(c.top.get(), q),
            "compiler bug: root frame closed out of order"
        );
        // SAFETY: `q` is the top, still on the caller's stack.
        c.top.set(unsafe { (*q).previous });
    });
}

/// The `n` field of a root frame: the number of slots in the low 40
/// bits; in the high ones, how many of the last are check slots (the check
/// build of the map-based walk, `llvm/mod.rs`).
const FRAME_SLOTS: u64 = (1 << 40) - 1;

/// Bit 62 of the `n` field: the frame is in its own native frame (the
/// unoptimized program, without inlining; the emitter's `com_quadros_ordenados`).
/// Only between two such frames is the address order guaranteed.
const FRAME_ORDERED: u64 = 1 << 62;

/// Visits the roots of all frames of this thread's shadow stack.
///
/// The frames are `alloca`s of the generated code on the thread's native stack, which
/// grows downwards: each previous frame is at a higher address. A
/// previous frame below the current one is a dead frame, of a function that already returned —
/// the top an unwind failed to restore (the landing pad of
/// `--exceptions=tables` without the top `store`, the sabotage `pad_without_top`
/// of §14.7) and that a new frame linked. Reading its slots would be reading
/// garbage; the process exits with code 3. The check only holds between two
/// frames with [`FRAME_ORDERED`]: with inlining (optimized code, the SDK),
/// the frame of the inlined function and that of its caller are `alloca`s of the same
/// native frame, in arbitrary address order.
#[allow(unsafe_code)]
fn visit_frames(mut f: impl FnMut(i64)) {
    let mut q = CONTEXT.with(|c| c.top.get());
    while !q.is_null() {
        // SAFETY: each linked frame is on the stack of a still active function
        // of this thread, with `n` slots after the header (the address order,
        // checked below before following, guarantees it).
        unsafe {
            let n = ((*q).n as u64 & FRAME_SLOTS) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in 0..n {
                f(*slots.add(i));
            }
            let previous = (*q).previous;
            if !previous.is_null()
                && (previous as usize) <= (q as usize)
                && (*q).n as u64 & FRAME_ORDERED != 0
                && (*previous).n as u64 & FRAME_ORDERED != 0
            {
                eprintln!("dartforge: dead root frame on the shadow stack");
                crate::console::exit(3);
            }
            q = previous;
        }
    }
}

// ---------------------------------------------------------------------------
// Roots through stack maps (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8).
//
// With `--roots=maps` the values live across a call do not go to the
// shadow stack: LLVM (`rewrite-statepoints-for-gc`) leaves them in a callee-saved register
// or in a slot of the native frame and describes, for each call, where
// they are — the object's `.llvm_stackmaps` section (`.llvm_st` in the image). During
// collection the runtime walks the thread's native stack and, for each frame whose
// return address has a record, reads the slots and the registers (the value
// the unwinder restored for that frame). Frames without a record
// (the runtime, C, the system, functions that only have a shadow stack) are
// skipped; the shadow stack is still visited as always.

/// The DWARF register of the target's SP and FP, as `.llvm_stackmaps`
/// writes them (x86-64: RSP 7, RBP 6; aarch64: SP 31, x29 29).
#[cfg(target_arch = "aarch64")]
const DWARF_SP: u16 = 31;
#[cfg(target_arch = "aarch64")]
const DWARF_FP: u16 = 29;
#[cfg(not(target_arch = "aarch64"))]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const DWARF_SP: u16 = 7;
#[cfg(not(target_arch = "aarch64"))]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const DWARF_FP: u16 = 6;

/// The DWARF registers preserved across a call, by the target's
/// convention (a map `Register` outside them is a defect: the value
/// would not survive the call). Windows x64: rbx, rsi, rdi, rbp, r12–r15;
/// SysV x86-64: rbx, rbp, r12–r15; aarch64: x19–x29.
#[cfg(target_arch = "aarch64")]
const PRESERVED: u32 = ((1 << 11) - 1) << 19;
#[cfg(all(not(target_arch = "aarch64"), windows))]
const PRESERVED: u32 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (0xF << 12);
#[cfg(all(not(target_arch = "aarch64"), not(windows)))]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const PRESERVED: u32 = (1 << 3) | (1 << 6) | (0xF << 12);

/// The kind of a map root: slot relative to SP, slot relative to FP, or
/// register (the value is its DWARF number).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const ROOT_AT_SP: u8 = 0;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const ROOT_AT_FP: u8 = 1;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
const ROOT_IN_REGISTER: u8 = 2;

/// A function with a stack map in an image: the address of its start and
/// where the description is, which is only decoded on the first lookup
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.5 and §14.5, E3.2).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
#[derive(Clone, Copy)]
struct FunctionWithMap {
    start: u64,
    /// DFGM: the start of the function's stream; `.llvm_stackmaps`: its first
    /// record.
    data: usize,
    /// `.llvm_stackmaps`: how many records and the blob's constants vector
    /// (the `ConstIndex`es); DFGM: `u32::MAX` and the blob version (1: slots only;
    /// 2: with the register mask).
    n_llvm: u32,
    constants: usize,
}

/// The records of a function, decoded: (offset of the return address
/// relative to the function start, start in `slots`, count), in
/// offset order; and the roots, (kind, slot offset in bytes
/// or DWARF register number): [`ROOT_AT_SP`], [`ROOT_AT_FP`],
/// [`ROOT_IN_REGISTER`].
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
#[derive(Default)]
struct FunctionRecords {
    records: Vec<(u32, u32, u32)>,
    slots: Vec<(u8, i32)>,
}

/// The maps of an image (the executable, the SDK DLL).
struct StackMap {
    /// What identifies the image in the registry: the base on Windows, the start
    /// of the section on ELF and Mach-O.
    key: usize,
    /// Sorted by start.
    functions: Vec<FunctionWithMap>,
    decoded: Vec<std::sync::OnceLock<FunctionRecords>>,
}

// SAFETY: the stored addresses are of read-only sections of the image,
// mapped while the process lives; nothing is written through them.
#[allow(unsafe_code)]
unsafe impl Send for StackMap {}
#[allow(unsafe_code)]
unsafe impl Sync for StackMap {}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
impl StackMap {
    fn new(key: usize) -> Self {
        StackMap {
            key,
            functions: Vec::new(),
            decoded: Vec::new(),
        }
    }

    /// Sorts the index and prepares the lazy decodings. Two
    /// entries with the same start (the linker's ICF merged two identical
    /// functions, with identical maps) become one.
    fn do_close(&mut self) {
        self.functions.sort_by_key(|f| f.start);
        self.functions.dedup_by_key(|f| f.start);
        self.decoded = (0..self.functions.len())
            .map(|_| std::sync::OnceLock::new())
            .collect();
    }

    /// The function with the largest start that does not exceed `ret`.
    fn function_of(&self, ret: u64) -> Option<usize> {
        self.functions
            .partition_point(|f| f.start <= ret)
            .checked_sub(1)
    }

    #[allow(unsafe_code)]
    fn records(&self, i: usize) -> &FunctionRecords {
        // SAFETY: `functions[i]` points to the description of a function in a
        // mapped section (checked when registering the image).
        self.decoded[i].get_or_init(|| unsafe { decode_function(self.functions[i]) })
    }

    /// The slots of the call of function `i` that returns at `ret`.
    ///
    /// On Windows x64, for some of the calls, the code generator puts a
    /// `nop` after the `call` (for the system unwinder) and the record
    /// points after it (§14.5): the following offset also holds, only
    /// when the byte at the return address is that `nop`.
    #[allow(unsafe_code)]
    fn slots_of(&self, i: usize, ret: u64) -> Option<&[(u8, i32)]> {
        let r = self.records(i);
        let offset = u32::try_from(ret.checked_sub(self.functions[i].start)?).ok()?;
        let find_one = |d: u32| {
            r.records.binary_search_by_key(&d, |g| g.0).ok().map(|k| {
                let (_, begin, n) = r.records[k];
                &r.slots[begin as usize..(begin + n) as usize]
            })
        };
        find_one(offset).or_else(|| {
            // SAFETY: `ret` is a return address of this thread's
            // stack, a byte of mapped code.
            let nop = cfg!(all(windows, target_arch = "x86_64"))
                && !sabotage("no_nop")
                && unsafe { *(ret as *const u8) } == 0x90;
            if nop { find_one(offset + 1) } else { None }
        })
    }
}

static STACK_MAPS: std::sync::RwLock<Vec<StackMap>> = std::sync::RwLock::new(Vec::new());
/// Some image registered maps: the collection walks the native stack.
static HAS_STACK_MAPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The numbers of the map-based walk, for the whole process (§7.5, the proof
/// that it collected): native frames walked, roots read from maps and
/// roots checked against the shadow stack (`DARTFORGE_GC_WALK`).
static FRAMES_WALKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static MAP_ROOTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ROOTS_CHECKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// (frames walked, map roots visited, roots checked).
pub fn walk_counts() -> (u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        FRAMES_WALKED.load(Relaxed),
        MAP_ROOTS.load(Relaxed),
        ROOTS_CHECKED.load(Relaxed),
    )
}

/// Some image of the process registered stack maps.
pub fn has_stack_maps() -> bool {
    HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire)
}

/// A test sabotage enabled (`DARTFORGE_SABOTAGE=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 and §14.7): each targeted
/// test must fail with its own. The runtime's: `no_nop` (the reader
/// without the `nop` rule), `slack` (D8: the 4 KiB stack slack) and
/// `no_ports` (D9: the Rust → Dart ports are not registered) and
/// `unrooted_type` (D12: the canonical `Type` object stays only in the cache).
pub fn sabotage(name: &str) -> bool {
    static ENABLED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ENABLED
        .get_or_init(|| {
            crate::console::env_var("DARTFORGE_SABOTAGE")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .iter()
        .any(|s| s == name)
}

/// `DARTFORGE_GC_WALK=check` (§3.6, E2.5): each collection checks that
/// the map-based walk visited the roots that the check build
/// (`DARTFORGE_ROOTS_CHECK=1` at compile time) wrote on the shadow stack, and
/// an anomaly of the walk aborts instead of just ending it.
fn checked_walk() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| crate::console::env_var("DARTFORGE_GC_WALK").is_ok_and(|v| v == "check"))
}

/// Ends the process: a map the runtime cannot read is a lost
/// root, and continuing would free a live object.
fn map_rejected(reason: &str) -> ! {
    eprintln!("dartforge: stack map rejected: {reason}");
    std::process::abort()
}

/// Registers the stack maps of the PE image loaded at `base` (once per
/// image). The module that has functions with roots in the map calls this at
/// startup, with the `__ImageBase` of the image it was linked into.
#[allow(unsafe_code)]
pub fn register_stack_map(base: usize) {
    if base == 0 {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == base) {
        return;
    }
    // SAFETY: `base` is the start of a PE image loaded in this process.
    let map = unsafe { read_image_map(base) };
    if map.functions.is_empty() {
        map_rejected(
            "the image was compiled with map-based roots and has neither the `.dfgcm` section nor `.llvm_st` (the linker renames the section when there is debug information)",
        );
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Registers the compact map of an ELF or Mach-O image: the `dfgcm` section
/// (`__DATA_CONST,__dfgcm` on Mach-O) between `start` and `end`, which the
/// linker delimits (`__start_dfgcm`/`__stop_dfgcm`; `section$start$…`).
#[allow(unsafe_code)]
pub fn register_map_section(start: usize, end: usize) {
    if start == 0 || end <= start {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == start) {
        return;
    }
    let mut map = StackMap::new(start);
    // SAFETY: `[start, end)` is the map section, mapped and readable.
    unsafe { read_dfgm(start as *const u8, end - start, 0, &mut map) };
    map.do_close();
    if map.functions.is_empty() {
        map_rejected(
            "the compact stack map section is empty in an image compiled with map-based roots",
        );
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Registers the stack map in LLVM format (`.llvm_stackmaps` version 3)
/// of a Mach-O image: the `__LLVM_STACKMAPS,__llvm_stackmaps` section between
/// `start` and `end` (`section$start$…`/`section$end$…`), with the function
/// addresses already adjusted by the loader. An empty section is not an error here: the
/// registering module may have no function with a map, and the `.no_dead_strip`
/// that keeps the section goes into every module that has one (`llvm/mod.rs`).
#[allow(unsafe_code)]
pub fn register_llvm_section(start: usize, end: usize) {
    if start == 0 || end <= start {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == start) {
        return;
    }
    let mut map = StackMap::new(start);
    // SAFETY: `[start, end)` is the map section, mapped and readable.
    unsafe { read_stackmaps(start as *const u8, end - start, &mut map) };
    map.do_close();
    if map.functions.is_empty() {
        return;
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Removes from the index the map registered with key `key` (the section start):
/// the JIT calls it when it frees the memory of an object (a retired generation
/// of the reload, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.8). The
/// publication of a generation happens without a Dart frame on the stack, so no
/// frame in progress uses the map being removed.
pub fn unregister_map(key: usize) {
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    maps.retain(|m| m.key != key);
}

/// Reads the stack maps of the PE image at `base`: the `.dfgcm` section (the
/// compact map, `emit_native/src/gcmap.rs`) and `.llvm_st` (the `.llvm_stackmaps`
/// of objects that did not go through the converter — the LTO ones —, with the name
/// truncated to 8 bytes).
///
/// # Safety
/// `base` is the start of a mapped PE image.
#[allow(unsafe_code)]
unsafe fn read_image_map(base: usize) -> StackMap {
    let mut map = StackMap::new(base);
    // SAFETY: the headers of a mapped PE image (DOS, NT, sections).
    unsafe {
        let b = base as *const u8;
        let nt = b.add(b.add(0x3c).cast::<u32>().read_unaligned() as usize);
        let n_sections = nt.add(6).cast::<u16>().read_unaligned() as usize;
        let optional = nt.add(20).cast::<u16>().read_unaligned() as usize;
        let sections = nt.add(24 + optional);
        for i in 0..n_sections {
            let s = sections.add(40 * i);
            let name = std::slice::from_raw_parts(s, 8);
            let size = s.add(8).cast::<u32>().read_unaligned() as usize;
            let rva = s.add(12).cast::<u32>().read_unaligned() as usize;
            if name == b".llvm_st" {
                read_stackmaps(b.add(rva), size, &mut map);
            } else if name == b".dfgcm\0\0" {
                read_dfgm(b.add(rva), size, base, &mut map);
            }
        }
    }
    map.do_close();
    map
}

/// The size of a `.llvm_stackmaps` v3 record starting at `r`
/// (header, locations, padding, *live-outs*, padding).
///
/// # Safety
/// `r` is the start of a record inside a mapped section.
#[allow(unsafe_code)]
unsafe fn llvm_record_end(r: *const u8) -> *const u8 {
    let align_up = |q: *const u8| ((q as usize + 7) & !7) as *const u8;
    // SAFETY: the record format (§14.3).
    unsafe {
        let n_locals = r.add(14).cast::<u16>().read_unaligned() as usize;
        let q = align_up(r.add(16 + 12 * n_locals));
        let n_exits = q.add(2).cast::<u16>().read_unaligned() as usize;
        align_up(q.add(4 + 4 * n_exits))
    }
}

/// Indexes the `.llvm_stackmaps` version 3 blobs of `[p, p + size)` (one
/// per linked object, with padding zeros between them): per function, the
/// start and the first record. Records are only read on lookup.
///
/// # Safety
/// `[p, p + size)` is the section, mapped and readable.
#[allow(unsafe_code)]
unsafe fn read_stackmaps(mut p: *const u8, size: usize, map: &mut StackMap) {
    // SAFETY: all reads stay inside the section, per the format.
    unsafe {
        let end = p.add(size);
        while (p as usize) + 16 <= end as usize {
            if *p == 0 {
                p = p.add(1);
                continue;
            }
            if *p != 3 {
                map_rejected("`.llvm_stackmaps` version other than 3");
            }
            let n_functions = p.add(4).cast::<u32>().read_unaligned() as usize;
            let n_constants = p.add(8).cast::<u32>().read_unaligned() as usize;
            let functions = p.add(16);
            let constants = functions.add(24 * n_functions);
            let mut r = constants.add(8 * n_constants);
            for i in 0..n_functions {
                let address = functions.add(24 * i).cast::<u64>().read_unaligned();
                let n_records = functions.add(24 * i + 16).cast::<u64>().read_unaligned();
                if n_records > u64::from(u32::MAX - 1) {
                    map_rejected("`.llvm_stackmaps` function with too many records");
                }
                map.functions.push(FunctionWithMap {
                    start: address,
                    data: r as usize,
                    n_llvm: n_records as u32,
                    constants: constants as usize,
                });
                for _ in 0..n_records {
                    r = llvm_record_end(r);
                }
                if r as usize > end as usize {
                    map_rejected("truncated `.llvm_stackmaps`");
                }
            }
            p = r;
        }
    }
}

/// Indexes the DFGM v1 and v2 blobs of `[p, p + size)` (the format is in
/// `emit_native/src/gcmap.rs`; between the objects' contributions there are padding
/// zeros, in multiples of 4). The address of each function comes from the
/// index according to the blob flags: 0, offset relative to `base` (COFF's
/// `ADDR32NB`); 1, 32-bit offset relative to the field itself
/// (ELF); 2, 64-bit absolute address (Mach-O).
///
/// # Safety
/// `[p, p + size)` is the section, mapped and readable.
#[allow(unsafe_code)]
unsafe fn read_dfgm(mut p: *const u8, size: usize, base: usize, map: &mut StackMap) {
    // SAFETY: all reads stay inside the section, per the format.
    unsafe {
        let end = p.add(size);
        while (p as usize) + 16 <= end as usize {
            if std::slice::from_raw_parts(p, 4) != b"DFGM" {
                p = p.add(4);
                continue;
            }
            let version = *p.add(4);
            if version != 1 && version != 2 {
                map_rejected("compact map version other than 1 and 2");
            }
            let flag_bits = p.add(6).cast::<u16>().read_unaligned();
            let total = p.add(8).cast::<u32>().read_unaligned() as usize;
            let n_functions = p.add(12).cast::<u32>().read_unaligned() as usize;
            let input = if flag_bits & 2 != 0 { 16 } else { 8 };
            if total < 16 + input * n_functions
                || (p as usize) + total > end as usize
                || (flag_bits & 3 == 0 && base == 0)
            {
                map_rejected("compact map with the wrong header");
            }
            let ix = p.add(16);
            let stream = ix.add(input * n_functions);
            for i in 0..n_functions {
                let e = ix.add(input * i);
                let (function_start, offset_in_stream) = if flag_bits & 2 != 0 {
                    (
                        e.cast::<u64>().read_unaligned(),
                        e.add(8).cast::<u32>().read_unaligned(),
                    )
                } else if flag_bits & 1 != 0 {
                    let rel = i64::from(e.cast::<i32>().read_unaligned());
                    (
                        (e as i64).wrapping_add(rel) as u64,
                        e.add(4).cast::<u32>().read_unaligned(),
                    )
                } else {
                    (
                        base as u64 + u64::from(e.cast::<u32>().read_unaligned()),
                        e.add(4).cast::<u32>().read_unaligned(),
                    )
                };
                map.functions.push(FunctionWithMap {
                    start: function_start,
                    data: stream.add(offset_in_stream as usize) as usize,
                    n_llvm: u32::MAX,
                    constants: usize::from(version),
                });
            }
            p = p.add(total);
        }
    }
}

/// Decodes the records of a function (the first lookup of it).
///
/// # Safety
/// `f` comes from [`read_stackmaps`] or [`read_dfgm`] over a mapped section.
#[allow(unsafe_code)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
unsafe fn decode_function(f: FunctionWithMap) -> FunctionRecords {
    let mut d = FunctionRecords::default();
    // SAFETY: the reads follow the format starting at `f.data`.
    unsafe {
        if f.n_llvm == u32::MAX {
            let mut q = f.data as *const u8;
            let varint = |q: &mut *const u8| -> u64 {
                let mut r = 0u64;
                let mut s = 0u32;
                loop {
                    let b = **q;
                    *q = q.add(1);
                    if s < 64 {
                        r |= u64::from(b & 0x7f) << s;
                    }
                    s += 7;
                    if b & 0x80 == 0 {
                        return r;
                    }
                }
            };
            let n_records = varint(&mut q);
            let _frame = varint(&mut q);
            let mut ret = 0u64;
            // (start in `slots`, count) of the previous record's set.
            let mut previous = (0u32, 0u32);
            for _ in 0..n_records {
                ret += varint(&mut q);
                let c = varint(&mut q);
                if c & 1 == 0 {
                    let first = d.slots.len() as u32;
                    let mut slot = 0i64;
                    for _ in 0..(c >> 1) {
                        let s = varint(&mut q);
                        let u = s >> 1;
                        slot += ((u >> 1) as i64) ^ -((u & 1) as i64);
                        d.slots.push((
                            if s & 1 == 1 { ROOT_AT_FP } else { ROOT_AT_SP },
                            (slot * 8) as i32,
                        ));
                    }
                    // v2: the mask of registers with a root.
                    if f.constants >= 2 {
                        let mask = varint(&mut q);
                        if mask & !u64::from(PRESERVED) != 0 {
                            map_rejected("non-preserved register with a root in the compact map");
                        }
                        for r in 0..32 {
                            if mask & (1 << r) != 0 {
                                d.slots.push((ROOT_IN_REGISTER, r));
                            }
                        }
                    }
                    previous = (first, d.slots.len() as u32 - first);
                }
                if ret > u64::from(u32::MAX) {
                    map_rejected("return offset beyond 32 bits in the compact map");
                }
                d.records.push((ret as u32, previous.0, previous.1));
            }
        } else {
            let mut r = f.data as *const u8;
            for _ in 0..f.n_llvm {
                let offset = r.add(8).cast::<u32>().read_unaligned();
                let n_locals = r.add(14).cast::<u16>().read_unaligned() as usize;
                let locals = r.add(16);
                if n_locals < 3 {
                    map_rejected("statepoint record without the three initial locations");
                }
                let n_deopt = locals.add(2 * 12 + 8).cast::<i32>().read_unaligned().max(0) as usize;
                let start = d.slots.len();
                if 3 + n_deopt > n_locals {
                    map_rejected("statepoint record with more `deopt` locations than locations");
                }
                // The roots: each `"deopt"` location (the §14.12 form) and the
                // base of each (base, derived) pair of `"gc-live"`.
                for j in
                    (3..3 + n_deopt).chain((3 + n_deopt..n_locals.saturating_sub(1)).step_by(2))
                {
                    let l = locals.add(12 * j);
                    let ty = *l;
                    let width = l.add(2).cast::<u16>().read_unaligned();
                    let register = l.add(4).cast::<u16>().read_unaligned();
                    let value = l.add(8).cast::<i32>().read_unaligned();
                    // Constant: null is not a root, nor the `undef` the
                    // optimizer left in an operand (0xFEFEFEFE); any other
                    // non-null even constant would be a raw value treated
                    // as a reference (§3.5).
                    if ty == 4 || ty == 5 {
                        let c = if ty == 4 {
                            i64::from(value)
                        } else {
                            (f.constants as *const u8)
                                .add(8 * value.max(0) as usize)
                                .cast::<i64>()
                                .read_unaligned()
                        };
                        if c != 0 && c & 1 == 0 && c != 0xFEFE_FEFE {
                            map_rejected("non-null even constant as a root in the stack map");
                        }
                        continue;
                    }
                    let slot = if ty == 1
                        && width == 8
                        && register < 32
                        && PRESERVED & (1 << register) != 0
                    {
                        (ROOT_IN_REGISTER, i32::from(register))
                    } else if ty == 3
                        && width == 8
                        && (register == DWARF_SP || register == DWARF_FP)
                    {
                        (
                            if register == DWARF_FP {
                                ROOT_AT_FP
                            } else {
                                ROOT_AT_SP
                            },
                            value,
                        )
                    } else {
                        map_rejected(
                            "location that is neither `Indirect [SP|FP + d]` nor a preserved 8-byte `Register`",
                        )
                    };
                    if !d.slots[start..].contains(&slot) {
                        d.slots.push(slot);
                    }
                }
                d.records
                    .push((offset, start as u32, (d.slots.len() - start) as u32));
                r = llvm_record_end(r);
            }
            d.records.sort_unstable_by_key(|g| g.0);
        }
    }
    if d.records.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        map_rejected("two stack map records at the same return address");
    }
    d
}

/// Visits the roots of the native frame stopped at return address
/// `ret`, with the frame's SP at the call point in `sp` and the FP in `fp`;
/// `register` gives the value of a preserved register (by DWARF
/// number) in that frame, as the unwinder restored it.
/// `exact_start` is the function start as the unwinder knows it: if
/// it has a map and the address has no record, it is a defect of the emitter or of the
/// converter, and the process aborts. Returns whether the frame had a map.
#[allow(unsafe_code)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
fn visit_frame(
    maps: &[StackMap],
    exact_start: Option<u64>,
    ret: u64,
    sp: u64,
    fp: u64,
    register: &dyn Fn(u16) -> u64,
    f: &mut dyn FnMut(i64),
) -> bool {
    for m in maps {
        let Some(i) = m.function_of(ret) else {
            continue;
        };
        match m.slots_of(i, ret) {
            Some(slots) => {
                for &(ty, d) in slots {
                    if ty == ROOT_IN_REGISTER {
                        f(register(d as u16) as i64);
                        continue;
                    }
                    let b = if ty == ROOT_AT_FP { fp } else { sp };
                    // SAFETY: the slot belongs to the frame of a still active function,
                    // stopped at the call the record describes.
                    f(unsafe { *((b as i64).wrapping_add(i64::from(d)) as *const i64) });
                }
                MAP_ROOTS.fetch_add(slots.len() as u64, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            None => {
                if exact_start == Some(m.functions[i].start) {
                    map_rejected(
                        "function with a map without an exact record for the return address",
                    );
                }
            }
        }
    }
    false
}

/// An anomaly of the walk (SP that does not grow, outside the stack): ends the
/// walk; with `DARTFORGE_GC_WALK=check`, aborts.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // stack maps are native only
fn walk_anomaly(reason: &str) {
    if checked_walk() {
        map_rejected(&format!("stack walk anomaly: {reason}"));
    }
}

/// Visits the roots of this thread's native frames that have a stack map:
/// unwinds the stack from here (`RtlVirtualUnwind`, without calling
/// handlers) and, in each frame, reads the slots of the return address's
/// record. Without a registered map, does nothing.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
fn visit_frames_by_maps(mut f: impl FnMut(i64)) {
    if !HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    /// The x86-64 `CONTEXT`: 1232 bytes, 16-aligned. `Rip`
    /// (0xF8) and the integers from `Rax` (0x78) to `R15` (0xF0) are read.
    #[repr(C, align(16))]
    struct CpuContext([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(context: *mut CpuContext);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, history: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            ty: u32,
            base: u64,
            pc: u64,
            function: *const u8,
            context: *mut CpuContext,
            data: *mut *mut u8,
            frame: *mut u64,
            pointers: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let maps = STACK_MAPS.read().unwrap_or_else(|e| e.into_inner());
    let mut context = CpuContext([0; 1232]);
    let field = |c: &CpuContext, d: usize| {
        u64::from_le_bytes(c.0[d..d + 8].try_into().expect("eight bytes"))
    };
    /// The offset in `CONTEXT` of each DWARF register 0–15 (rax,
    /// rdx, rcx, rbx, rsi, rdi, rbp, rsp, r8–r15).
    const IN_CONTEXT: [usize; 16] = [
        0x78, 0x88, 0x80, 0x90, 0xA8, 0xB0, 0xA0, 0x98, 0xB8, 0xC0, 0xC8, 0xD0, 0xD8, 0xE0, 0xE8,
        0xF0,
    ];
    let mut frames = 0u64;
    // SAFETY: the context is this thread's, captured here; each step
    // unwinds a frame with its function's unwind table, and
    // stops when the SP stops growing or leaves the thread's stack. The slots
    // read belong to the frame of a still active function, stopped at the call the
    // record describes.
    unsafe {
        RtlCaptureContext(&mut context);
        let (mut low, mut high) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut low, &mut high);
        let mut pc = field(&context, 0xF8);
        let mut base = 0u64;
        let mut function = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !function.is_null() {
            let sp_before = field(&context, 0x98);
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frame = 0u64;
            RtlVirtualUnwind(
                0,
                base,
                pc,
                function,
                &mut context,
                &mut data,
                &mut frame,
                std::ptr::null_mut(),
            );
            // Now the context is the caller's: `Rip` is the return address
            // in it and `Rsp`, its SP at the call point.
            let (ret, sp, fp) = (
                field(&context, 0xF8),
                field(&context, 0x98),
                field(&context, 0xA0),
            );
            if ret == 0 {
                break;
            }
            if sp <= sp_before || (sp as usize) < low || (sp as usize) >= high {
                walk_anomaly("the SP does not grow or left the thread's stack");
                break;
            }
            frames += 1;
            // The caller's function: the exact start (the `BeginAddress` of the
            // `.pdata` entry) and the entry, for the next step.
            let mut next_base = 0u64;
            let next_one = RtlLookupFunctionEntry(ret, &mut next_base, std::ptr::null_mut());
            let start = (!next_one.is_null())
                .then(|| next_base + u64::from(next_one.cast::<u32>().read_unaligned()));
            // The preserved registers in the context are the caller's, at the call
            // point: unwinding the inner frames restored them.
            let register = |r: u16| {
                IN_CONTEXT
                    .get(usize::from(r))
                    .map_or(0, |&d| field(&context, d))
            };
            visit_frame(&maps, start, ret, sp, fp, &register, &mut f);
            pc = ret;
            base = next_base;
            function = next_one;
        }
    }
    FRAMES_WALKED.fetch_add(frames, std::sync::atomic::Ordering::Relaxed);
}

/// The stack pointer of a frame at the point of the call it made, in the
/// `_Unwind_Backtrace` step: `cfa` is the frame's own `_Unwind_GetCFA`
/// and `callee_cfa`, that of the previous frame of the walk (the one it called).
/// libgcc (Linux) returns in `_Unwind_GetCFA` the frame's CFA — the caller's
/// SP at the call point —, so this frame's SP is the callee's
/// CFA. LLVM's libunwind (macOS) returns the frame's own SP, already
/// restored: that is it. Using the callee's on macOS read the `[SP + d]` slots of the
/// map in the inner frame (map-based roots in production, where the LTO of
/// `ld64.lld` leaves roots in slots) and misaligned the symbolic stack trace.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
fn frame_top(cfa: usize, callee_cfa: usize) -> usize {
    if cfg!(target_vendor = "apple") {
        cfa
    } else {
        callee_cfa
    }
}

/// The same on the Itanium unwinder targets (Linux and macOS, x86-64 and
/// aarch64; §3.6): `_Unwind_Backtrace` hands out the frames from inside
/// out; a frame's SP at the call point is the CFA of the frame it
/// called (the previous one in the walk), the FP is the register restored in it, and
/// the function start is that of the FDE region. The runtime and the generated code must
/// have unwind tables (`uwtable`; `-C
/// force-unwind-tables=yes` in the runtime), otherwise the walk sees zero frames.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
#[allow(unsafe_code)]
#[inline(never)]
fn visit_frames_by_maps(mut f: impl FnMut(i64)) {
    if !HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    unsafe extern "C" {
        fn _Unwind_Backtrace(
            step: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
            argument: *mut u8,
        ) -> i32;
        fn _Unwind_GetIP(context: *mut u8) -> usize;
        fn _Unwind_GetCFA(context: *mut u8) -> usize;
        fn _Unwind_GetGR(context: *mut u8, register: i32) -> usize;
        fn _Unwind_GetRegionStart(context: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continue) and `_URC_NORMAL_STOP` (stop).
    const CONTINUE: i32 = 0;
    const TO: i32 = 4;
    struct State<'a> {
        maps: &'a [StackMap],
        f: &'a mut dyn FnMut(i64),
        /// The CFA of the previous frame of the walk (the callee).
        previous_cfa: Option<usize>,
        frames: u64,
    }
    unsafe extern "C" fn step(context: *mut u8, argument: *mut u8) -> i32 {
        // SAFETY: `argument` is the `State` below, alive during the walk;
        // `context` is the unwinder's, valid during the call.
        unsafe {
            let e = &mut *argument.cast::<State<'_>>();
            let ip = _Unwind_GetIP(context) as u64;
            if ip == 0 {
                return TO;
            }
            let cfa = _Unwind_GetCFA(context);
            if let Some(previous) = e.previous_cfa {
                if cfa < previous {
                    walk_anomaly("the CFA does not grow");
                    return TO;
                }
                let sp = frame_top(cfa, previous);
                e.frames += 1;
                let start = _Unwind_GetRegionStart(context) as u64;
                let fp = _Unwind_GetGR(context, i32::from(DWARF_FP)) as u64;
                // The closure inherits the `unsafe` block above: the unwinder's
                // context is valid during the step.
                let register = |r: u16| _Unwind_GetGR(context, i32::from(r)) as u64;
                visit_frame(
                    e.maps,
                    (start != 0).then_some(start),
                    ip,
                    sp as u64,
                    fp,
                    &register,
                    e.f,
                );
            }
            e.previous_cfa = Some(cfa);
            CONTINUE
        }
    }
    let maps = STACK_MAPS.read().unwrap_or_else(|e| e.into_inner());
    let mut state = State {
        maps: &maps,
        f: &mut f,
        previous_cfa: None,
        frames: 0,
    };
    // SAFETY: the unwinder walks this thread's stack; `step` only reads.
    unsafe {
        _Unwind_Backtrace(step, std::ptr::addr_of_mut!(state).cast::<u8>());
    }
    FRAMES_WALKED.fetch_add(state.frames, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(unix, any(target_arch = "x86_64", target_arch = "aarch64"))
)))]
fn visit_frames_by_maps(_f: impl FnMut(i64)) {}

/// The return addresses of this thread's stack, from inside out, with the
/// function start of each one (0 when the unwinder does not provide it) and the stack
/// top of the frame the address is in (its stack pointer at the
/// call; 0 when unknown), up to `maximum` frames: the symbolic stack trace
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14) keeps them at `throw` and
/// only symbolizes them when the text is requested; the top matches the frame with the
/// trace stack (the chain of awaiters).
/// The same walk as `visit_frames_by_maps`, without reading roots: it does not
/// depend on the maps, and holds in both root modes.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn return_addresses(maximum: usize) -> Vec<(u64, u64, u64)> {
    /// The x86-64 `CONTEXT` (see `visit_frames_by_maps`).
    #[repr(C, align(16))]
    struct CpuContext([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(context: *mut CpuContext);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, history: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            ty: u32,
            base: u64,
            pc: u64,
            function: *const u8,
            context: *mut CpuContext,
            data: *mut *mut u8,
            frame: *mut u64,
            pointers: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let mut out = Vec::new();
    let mut context = CpuContext([0; 1232]);
    let field = |c: &CpuContext, d: usize| {
        u64::from_le_bytes(c.0[d..d + 8].try_into().expect("eight bytes"))
    };
    // SAFETY: as in `visit_frames_by_maps`: the context is this
    // thread's, each step unwinds a frame with its function's table, and the
    // walk stops when the SP stops growing or leaves the stack.
    unsafe {
        RtlCaptureContext(&mut context);
        let (mut low, mut high) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut low, &mut high);
        let mut pc = field(&context, 0xF8);
        let mut base = 0u64;
        let mut function = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !function.is_null() && out.len() < maximum {
            let sp_before = field(&context, 0x98);
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frame = 0u64;
            RtlVirtualUnwind(
                0,
                base,
                pc,
                function,
                &mut context,
                &mut data,
                &mut frame,
                std::ptr::null_mut(),
            );
            let (ret, sp) = (field(&context, 0xF8), field(&context, 0x98));
            if ret == 0 || sp <= sp_before || (sp as usize) < low || (sp as usize) >= high {
                break;
            }
            let mut next_base = 0u64;
            let next_one = RtlLookupFunctionEntry(ret, &mut next_base, std::ptr::null_mut());
            let start = if next_one.is_null() {
                0
            } else {
                next_base + u64::from(next_one.cast::<u32>().read_unaligned())
            };
            out.push((ret, start, sp));
            pc = ret;
            base = next_base;
            function = next_one;
        }
    }
    out
}

/// The same on the Itanium unwinder targets, through `_Unwind_Backtrace`: the
/// IP of each frame after the first is the return address, and the function
/// start is that of the FDE region.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn return_addresses(maximum: usize) -> Vec<(u64, u64, u64)> {
    unsafe extern "C" {
        fn _Unwind_Backtrace(
            step: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
            argument: *mut u8,
        ) -> i32;
        fn _Unwind_GetIP(context: *mut u8) -> usize;
        fn _Unwind_GetRegionStart(context: *mut u8) -> usize;
        fn _Unwind_GetCFA(context: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continue) and `_URC_NORMAL_STOP` (stop).
    const CONTINUE: i32 = 0;
    const TO: i32 = 4;
    struct State {
        out: Vec<(u64, u64, u64)>,
        maximum: usize,
        /// The CFA of the previous (deeper) frame: the stack top of this one.
        previous_cfa: u64,
    }
    unsafe extern "C" fn step(context: *mut u8, argument: *mut u8) -> i32 {
        // SAFETY: `argument` is the `State` below, alive during the walk;
        // `context` is the unwinder's, valid during the call.
        unsafe {
            let e = &mut *argument.cast::<State>();
            let ip = _Unwind_GetIP(context) as u64;
            if ip == 0 || e.out.len() >= e.maximum {
                return TO;
            }
            let cfa = _Unwind_GetCFA(context) as u64;
            let top = if e.out.is_empty() {
                e.previous_cfa
            } else {
                frame_top(cfa as usize, e.previous_cfa as usize) as u64
            };
            e.out
                .push((ip, _Unwind_GetRegionStart(context) as u64, top));
            e.previous_cfa = cfa;
            CONTINUE
        }
    }
    let mut state = State {
        out: Vec::new(),
        maximum,
        previous_cfa: 0,
    };
    // SAFETY: the unwinder walks this thread's stack; `step` only reads.
    unsafe {
        _Unwind_Backtrace(step, std::ptr::addr_of_mut!(state).cast::<u8>());
    }
    state.out
}

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(unix, any(target_arch = "x86_64", target_arch = "aarch64"))
)))]
pub fn return_addresses(_max: usize) -> Vec<(u64, u64, u64)> {
    Vec::new()
}

/// The check build (§15.2, E2.5): the values of the check slots of
/// the frames of this thread's shadow stack (the last `k`
/// slots of a frame whose `n` field carries `k` in bits 40 and up).
#[allow(unsafe_code)]
fn visit_check_slots(mut f: impl FnMut(i64)) {
    let mut q = CONTEXT.with(|c| c.top.get());
    while !q.is_null() {
        // SAFETY: each linked frame is on the stack of a still active
        // function of this thread (as in `visit_frames`).
        unsafe {
            let n = (*q).n as u64;
            let total = (n & FRAME_SLOTS) as usize;
            let k = ((n & !FRAME_ORDERED) >> 40) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in total.saturating_sub(k)..total {
                f(*slots.add(i));
            }
            q = (*q).previous;
        }
    }
}

/// `DARTFORGE_GC_WALK=check`: every root the check build
/// wrote on the shadow stack must be among those the map-based walk
/// visited (`from_maps`). A missing one is a root the map lost.
#[allow(unsafe_code)]
fn check_walk(from_maps: &[i64]) {
    let seen_set: crate::hash::HashSet<i64> = from_maps.iter().copied().collect();
    let mut checked = 0u64;
    visit_check_slots(|h| {
        // Only heap objects: a `Smi` (or null) in the slot is not a root, and
        // the image's static object (`PERMANENT`) is never collected — the
        // compiler removes it from `"deopt"` (`remove_statics_from_deopt`, §14.12).
        if !crate::layout::is_object(h) {
            return;
        }
        // SAFETY: `h` is the handle of an object the generated code wrote into the
        // check slot, alive at this collection point.
        if unsafe { (*((h - HANDLE_OFFSET) as *const Header)).state } == PERMANENT {
            return;
        }
        checked += 1;
        if !seen_set.contains(&h) {
            map_rejected(&format!(
                "walk check: root {h:#x} is alive on the check shadow stack and the map-based walk did not visit it"
            ));
        }
    });
    ROOTS_CHECKED.fetch_add(checked, std::sync::atomic::Ordering::Relaxed);
}

/// The heap of an isolate: the object space (every runtime value is a block
/// of it, docs/NATIVO-ESPACO-UNIFICADO.md), the roots, the side tables per
/// handle and the collection.
/// The seeded scheduled collection (`DARTFORGE_GC_SCHEDULE=<seed>,<rate>`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5): one collection in one out of every
/// `rate` allocations, drawn by a fixed generator from the seed. The
/// same seed and the same program collect at the same points: it finds the defect
/// that full `--gc-stress` hides by changing timing, and reproduces it. As
/// under stress, without TLAB (every allocation goes through the runtime).
#[derive(Debug)]
struct Schedule {
    state: std::cell::Cell<u64>,
    rate: u64,
    /// The draw already came out and the collection has not come yet: the next query
    /// answers the same (the slow path queries again).
    pending: std::cell::Cell<bool>,
}

impl Schedule {
    fn from_env() -> Option<Schedule> {
        let v = crate::console::env_var("DARTFORGE_GC_SCHEDULE").ok()?;
        let (seed, rate) = v.split_once(',')?;
        let seed: u64 = seed.trim().parse().ok()?;
        let rate: u64 = rate.trim().parse().ok().filter(|&t| t > 0)?;
        let state = seed ^ 0x9E37_79B9_7F4A_7C15;
        Some(Schedule {
            state: std::cell::Cell::new(if state == 0 { 1 } else { state }),
            rate,
            pending: std::cell::Cell::new(false),
        })
    }

    /// Does this allocation collect? (xorshift64*.)
    fn draws(&self) -> bool {
        if self.pending.get() {
            return true;
        }
        let mut x = self.state.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        let yes = x
            .wrapping_mul(0x2545_F491_4F6C_DD1D)
            .is_multiple_of(self.rate);
        self.pending.set(yes);
        yes
    }
}

#[derive(Debug)]
pub struct Heap {
    /// Identifier, slots and category: `true` counts each occurrence in ARC.
    frames: Vec<(i64, Vec<i64>, bool)>,
    /// Emptied slot vectors of closed frames, for the next ones ([`FRAME_POOL`]).
    frame_pool: Vec<Vec<i64>>,
    next_frame: i64,
    allocations: usize,
    stress: bool,
    /// `DARTFORGE_GC_SCHEDULE`; `None` without it.
    schedule: Option<Schedule>,
    stats: HeapStats,
    gc_policy: AutomaticGcPolicy,
    /// Positions walked by the last marking.
    marking_work: usize,
    pending: Vec<i64>,
    byte_threshold: usize,
    /// HARD ceiling of the heap, in bytes.
    ///
    /// Not to be confused with `byte_threshold`, which is the COLLECTION trigger. This is the
    /// process limit: a program in an infinite loop that allocates strings eats
    /// the whole machine's memory before any harness timeout
    /// fires (that is what froze the machine running the native corpus in
    /// parallel). When exceeded, the process exits with a readable message and code
    /// 255, which is what the VM uses for an uncaught exception — it becomes a failure
    /// on the scoreboard instead of a hang.
    ///
    /// Default 256 MiB; `DARTFORGE_HEAP_MAX_MB` adjusts it, and 0 disables the ceiling.
    byte_limit: usize,
    enum_values: crate::hash::HashMap<(i64, i64), i64>,
    /// Canonical tear-offs of top-level functions, by code ID.
    ///
    /// The Dart 3.6.2 oracle requires `identical(f, f)` to be true for two
    /// tear-offs of the same top-level function; each `code_id` has a single handle,
    /// kept alive as a permanent root, like enum singletons.
    tearoffs: crate::hash::HashMap<i64, i64>,
    /// Compiler literals, canonical by UTF-16 units. They stay
    /// rooted by the isolate; strings created at run time do not go here.
    literals: crate::hash::HashMap<Vec<u16>, i64>,
    /// Permanent and immutable objects that a message between ports of the
    /// same isolate passes by identity, without copying (the VM shares the
    /// deeply immutable ones): canonical constants, enum values and
    /// `const` globals (the generated code marks them, `dartforge_mark_permanent`),
    /// top-level tear-offs and literals.
    permanents: crate::hash::HashSet<i64>,
    /// Canonical constant → the generated getter that produces it (the same address
    /// in all isolates): a message to another isolate carries the getter,
    /// and the destination receives its own canonical instance (`identical` across
    /// isolates, like the VM group's shared objects).
    constants: crate::hash::HashMap<i64, usize>,
    /// The canonical instances, lists and records of `const` creations, by
    /// class, type arguments and contents (`canonical_instance`). Roots, like
    /// the literals.
    canonical_instances: crate::hash::HashMap<Vec<i64>, i64>,
    /// Canonical tear-off → its code (the inverse of `tearoffs`).
    tearoff_code: crate::hash::HashMap<i64, i64>,
    /// `DARTFORGE_GC_OFF=1`: never collects. Diagnostic instrument
    /// (docs/NATIVO-PLANO.md §6): a program that dies with "handle already
    /// collected" and passes with collection disabled is missing a root; one that
    /// dies the same way in both modes has a scalar used as a handle.
    gc_disabled: bool,
    /// Nesting depth of the regions where no allocation may collect
    /// ([`Heap::defer_collection`]). Allocations still count; the collection
    /// they would have triggered waits for the next allocation outside.
    collection_deferred: u32,
    /// Current value of each `Ref` global of the program (top-level variable or
    /// static field), by id: permanent roots (N6/G6 of the contract).
    globals: crate::hash::HashMap<i64, i64>,
    /// The `Ref` slots of the lean globals areas of this isolate (production):
    /// the start of each area and the indices of its `Ref` slots. The
    /// collector reads them as roots ([`Heap::register_area_roots`]).
    area_roots: Vec<(*const i64, Box<[u32]>)>,
    /// Roots that live in the runtime and not in a frame (G6): the pending exception
    /// and the current stack trace. 0 = none.
    runtime_roots: [i64; 2],
    /// Short-lived roots of a runtime fast path (`Heap::list_push`'s growth):
    /// pushed before an allocation and truncated right after, cheaper than a
    /// root frame. Read by [`Heap::root_list`].
    pub(crate) scratch_roots: Vec<i64>,
    /// The shared empty storages (capacity 0) of `_GrowableList`s by shape
    /// (`lists::Element::code`), under tracing memory: an empty literal and its
    /// shape change share them instead of allocating one each (the VM shares
    /// `_emptyList`). Capacity 0 is never written. Roots ([`Heap::root_list`]).
    pub(crate) empty_storages: [i64; 4],
    /// Shared occurrences held by messages in this isolate.
    message_owners: crate::hash::HashMap<u64, Ref>,
    next_message_owner: u64,
    /// SSA tokens of the ARC ABI, with multiplicity separate from slots and maps.
    /// The inventory sustains reachability and auditing; maps do not count again.
    /// With [`Heap::virtual_code_tokens`], only the tokens the runtime itself holds
    /// (the returns a native scope keeps) are here, as roots without a count.
    code_owners: crate::hash::HashMap<Ref, usize>,
    /// The tokens of the generated code are virtual (`dartforge_arc_virtual_tokens_v1`,
    /// the prepared code compiled with `DARTFORGE_ARC_TOKENS=virtual`, the fast mode
    /// without prompt death): the preparation proves them statically and the code
    /// emits no retain/release.
    /// Under the deferred roots a token adds nothing a root does not already give
    /// (a zero RC never kills an object that a root, the shadow stack included,
    /// still sees), so a token neither counts nor enters the inventory; the moves of
    /// a token into a counted slot retain instead of transferring.
    virtual_code_tokens: bool,
    /// The real ARC (`dartforge_memory_arc_prompt_v1`, the prepared code with exact
    /// tokens over the pure ARC): every reference counts from its first write, the
    /// code's tokens are counted owners, and a count that reaches zero kills the
    /// object at the next settle point ([`Heap::arc_settle`]) instead of at the
    /// next collection: the code's release settles at once, the other runtime entries
    /// that release or observe (field and list writes, native scopes, weak
    /// references) settle before returning. Only the runtime's observational roots
    /// still protect a zero; the shadow stack does not (the tokens own what it holds).
    prompt_death: bool,
    /// The objects that died at a settle point, waiting for the physical reclaimer of
    /// the next drain (the young ones go with the sweep of the young).
    prompt_dead: Vec<Ref>,
    /// `late` fields already written, by handle and physical index. The mark stays
    /// outside the value: zero and null are valid program assignments.
    late_fields_initialized: crate::hash::HashSet<(i64, i64)>,
    /// The `late_fields_initialized` marks written since the last
    /// collection: only they can belong to a young object, and the minor collection checks only
    /// these (old objects do not die in it).
    new_lates: Vec<(i64, i64)>,
    /// The layout epoch (`LAYOUT_EPOCH`) of this heap's objects: the one from
    /// when it was born, and that of each migration applied afterwards (J03).
    pub layout_epoch: u64,
    /// Where every runtime value lives ([`ObjectSpace`]).
    objects: ObjectSpace,
    /// The heap of the thread's isolate (not a test one): refills the TLAB of its
    /// [`Context`].
    published: bool,
    /// The [`Context`] of the thread whose isolate this heap serves (null in a
    /// test heap): the TLAB ranges live there, and the runtime's own allocations
    /// bump them ([`Heap::allocate_from_tlab`]) without a thread-local access per
    /// allocation. Set by [`Heap::for_isolate`], on the thread that uses the heap.
    context: *const Context,
    /// The runtime's own allocations bump the TLAB ([`Heap::allocate_from_tlab`]);
    /// `DARTFORGE_NO_RUNTIME_TLAB=1` sends them all through the general path
    /// (the ablation switch for measurements: the result is the same).
    runtime_tlab: bool,
    /// Up to how many blocks each TLAB refill hands out ([`TLAB_BLOCKS`];
    /// `DARTFORGE_TLAB_BLOCKS=<n>` to measure other sizes).
    tlab_blocks: usize,
    /// Bytes allocated since the last collection (the minor collection trigger).
    young_bytes: usize,
    /// The collection in progress is minor (young only).
    minor_collection: bool,
    /// Aging (`DARTFORGE_GC_AGING=0` turns it off): a young object is promoted
    /// by the second minor collection it survives, not the first. Never under
    /// ARC (which registers the promoted ones of each minor collection).
    aging: bool,
    /// The minor collection in progress ages (`aging` and no ARC).
    aging_now: bool,
    /// The state of the survivors of the last minor collection ([`SURVIVOR`] or
    /// [`SURVIVOR_B`]): the next one promotes the ones it reaches and gives the
    /// other state to the young ones it keeps.
    survivor_state: u8,
    /// Minor collections that still promote on the first survival, and the length
    /// of the last such pause: when most survivors live on (a structure under
    /// construction that stays, the linked list), aging only marks them twice, and
    /// it backs off for 1, 2, 4… up to [`AGING_MAX_PAUSE`] collections.
    aging_pause: (u32, u32),
    /// `--gc-stress`: collections since the last full one and its marking
    /// work (the spacing of full ones, [`Heap::collect_automatic`]).
    minors_since_full: u64,
    work_in_full: usize,
    /// Bytes outside the heap that count toward the triggers
    /// ([`Heap::count_external`]); the attachments' bytes the space counts separately.
    external: u128,
    /// Internal sum before the saturated projection into HeapStats; preserves discounts after overflow.
    exact_estimated_bytes: u128,
    /// `DARTFORGE_GC_VERIFY=1`: every minor collection checks, through a
    /// full traversal, that no reachable young object was left unmarked (a
    /// missing write barrier).
    /// How much the heap may grow until the next full collection, in % of what
    /// survived the last one ([`GROWTH`]; to measure,
    /// `DARTFORGE_GC_GROWTH`).
    growth: usize,
    /// The bytes between minor collections ([`YOUNG_LIMIT`]; to measure,
    /// `DARTFORGE_GC_YOUNG_KB`).
    young_limit: usize,
    /// The ceiling of [`Heap::adapt_young_limit`] ([`YOUNG_MAX`]; the young limit
    /// itself when `DARTFORGE_GC_YOUNG_KB` fixes it).
    young_max: usize,
    verify: bool,
    /// Checks every handle against the page map before reading the block (the
    /// N4 invalid handle messages): under `--gc-stress`, with verification,
    /// in tests and in the debug runtime, or with
    /// `DARTFORGE_VALIDATE_HANDLES=1`. Otherwise, an object handle is the
    /// block (the generated code's invariant): only the `FREE` state (object already
    /// collected) is checked, in the header read that happens anyway.
    pub(crate) validate_handles: bool,
    /// `DARTFORGE_GC_TRACE=1`: one line per collection on stderr (the kind, the
    /// marked ones, the estimate and the trigger).
    trace: bool,
    /// `DARTFORGE_GC_MEMORY=1`: after each full collection (and at the end of the
    /// program), the memory breakdown of the space per size class, of the
    /// runtime tables and of the process ([`Heap::memory_report`]).
    /// With `2`, after every collection.
    memory: u8,
    /// Native finalizable pairs (the VM's `Dart_NewFinalizableHandle`):
    /// object → (finalizer, pair). When the object dies, the collection removes the
    /// entry and calls `finalizer(pair)` — which only frees system
    /// resources (closes a file, drops a reference count) and never
    /// touches the heap.
    pub finalizable: crate::hash::HashMap<i64, Finalizer>,
    /// Weak references (`WeakReference`, the VM's `WeakReference_*`):
    /// holder object → target. The target is NOT followed by marking; if it does not
    /// survive through another path, the collection replaces it with 0 (null).
    pub weak_refs: crate::hash::HashMap<i64, i64>,
    /// Ephemerons (`_WeakProperty`, the base of `Expando`): holder →
    /// (key, value). The value is only reached if the key is (fixed point in
    /// marking); a dead key clears both.
    pub ephemerons: crate::hash::HashMap<i64, (i64, i64)>,
    /// The `Finalizer`/`NativeFinalizer` attachments (the VM's `FinalizerEntry`):
    /// value, owner and `detach` key are weak; the Dart action is a
    /// strong edge of the owner. Dead value: the action of a `Finalizer` goes to
    /// [`Heap::ready_finalizations`] (the event loop calls it); that of a
    /// `NativeFinalizer` runs right after the collection.
    attachments: Vec<FinalizerAttachment>,
    /// Positions of the Dart attachments per owner; rebuilt after removals from the vector.
    attachments_by_owner: crate::hash::HashMap<Ref, Vec<usize>>,
    /// The `Finalizer` actions whose value died, waiting for the event
    /// loop (roots until then).
    pub ready_finalizations: std::collections::VecDeque<i64>,
    /// ARC (`--memory=arc`, docs/ARC-IMPLEMENTACAO.md): enabled by the
    /// program at startup (`dartforge_memory_arc_v1`); `None` is the usual
    /// tracing collector.
    pub(crate) arc: ArcSlot,
}

/// A finalizer attachment (see `Heap::attachments`).
#[derive(Debug, Clone, Copy)]
pub struct FinalizerAttachment {
    /// The `Finalizer`/`NativeFinalizer` (identity for `detach`); 0 for a native
    /// action without one (a `JSValue`'s handle).
    pub owner: i64,
    pub value: i64,
    /// The `detach` key (0 = none).
    pub detach: i64,
    pub action: FinalizerAction,
}

/// What a finalizer does when the value dies.
#[derive(Debug, Clone, Copy)]
pub enum FinalizerAction {
    /// An argument-less Dart closure (`callback(token)` already applied).
    Dart(i64),
    /// `function(token)`, a C function, and external bytes accounted until execution/detach.
    Native(usize, usize, usize),
}

#[allow(unsafe_code)]
impl Heap {
    /// Runs the native actions of all still live attachments (the isolate
    /// ended: the VM guarantees `NativeFinalizer`s at shutdown) and
    /// discards the `Finalizer` ones.
    pub fn shutdown_finalizers(&mut self) {
        let attachments = std::mem::take(&mut self.attachments);
        self.attachments_by_owner.clear();
        while self.finish_ready_finalization().is_some() {}
        for a in attachments {
            if let FinalizerAction::Dart(action) = a.action {
                self.arc_raw_write(a.owner, action, 0);
            }
            if let FinalizerAction::Native(f, token, bytes) = a.action {
                self.count_external(-(bytes as isize));
                // SAFETY: `f` is the `NativeFinalizerFunction` the program
                // registered, `void f(void* token)`.
                let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
                f(token);
            }
        }
    }
}
impl Drop for Heap {
    /// A published heap owns the TLAB ranges of its thread [`Context`]: emptied
    /// before its object space goes, so that a heap installed later on this
    /// thread never hands out blocks of it (the tests replace the thread heap).
    fn drop(&mut self) {
        if !self.context.is_null() {
            // SAFETY: `context` is the `Context` of this thread (`for_isolate`), a
            // thread-local without a destructor: it outlives every heap of the thread.
            #[allow(unsafe_code)]
            Self::clear_tlab(unsafe { &*self.context });
        }
    }
}
impl Heap {
    /// Initializes the heap; stress forces a collection before each allocation.
    pub fn new(stress: bool) -> Self {
        Self {
            frames: Vec::new(),
            frame_pool: Vec::new(),
            next_frame: 1,
            allocations: 0,
            stress,
            schedule: Schedule::from_env(),
            stats: HeapStats::default(),
            gc_policy: AutomaticGcPolicy::default(),
            marking_work: 0,
            pending: Vec::new(),
            byte_threshold: 2 * YOUNG_LIMIT,
            growth: crate::console::env_var("DARTFORGE_GC_GROWTH")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(GROWTH, |c| c.max(110)),
            young_limit: Self::young_from_env().unwrap_or(YOUNG_LIMIT),
            young_max: Self::young_from_env().unwrap_or(YOUNG_MAX),
            byte_limit: Self::limit_from_env(),
            enum_values: crate::hash::HashMap::default(),
            tearoffs: crate::hash::HashMap::default(),
            literals: crate::hash::HashMap::default(),
            permanents: crate::hash::HashSet::default(),
            constants: crate::hash::HashMap::default(),
            canonical_instances: crate::hash::HashMap::default(),
            tearoff_code: crate::hash::HashMap::default(),
            gc_disabled: crate::console::env_var("DARTFORGE_GC_OFF").as_deref() == Ok("1"),
            collection_deferred: 0,
            globals: crate::hash::HashMap::default(),
            area_roots: Vec::new(),
            runtime_roots: [0, 0],
            scratch_roots: Vec::new(),
            empty_storages: [0; 4],
            message_owners: crate::hash::HashMap::default(),
            next_message_owner: 0,
            code_owners: crate::hash::HashMap::default(),
            virtual_code_tokens: false,
            prompt_death: false,
            prompt_dead: Vec::new(),
            late_fields_initialized: crate::hash::HashSet::default(),
            new_lates: Vec::new(),
            layout_epoch: LAYOUT_EPOCH.load(std::sync::atomic::Ordering::Acquire),
            // The runtime tests check that the handle of a dead object is
            // rejected: there, as under `--gc-stress`, the dead are zeroed right away.
            external: 0,
            exact_estimated_bytes: 0,
            objects: ObjectSpace::new(
                stress
                    || cfg!(test)
                    || crate::console::env_var("DARTFORGE_GC_VERIFY").as_deref() == Ok("1"),
            ),
            published: false,
            context: std::ptr::null(),
            runtime_tlab: crate::console::env_var_os("DARTFORGE_NO_RUNTIME_TLAB")
                .is_none_or(|v| v != "1"),
            tlab_blocks: crate::console::env_var("DARTFORGE_TLAB_BLOCKS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(TLAB_BLOCKS, |n| n.clamp(1, 1 << 16)),
            young_bytes: 0,
            arc: ArcSlot::NONE,
            minor_collection: false,
            aging: crate::console::env_var("DARTFORGE_GC_AGING").as_deref() != Ok("0"),
            aging_now: false,
            survivor_state: SURVIVOR,
            aging_pause: (0, 0),
            minors_since_full: 0,
            work_in_full: 0,
            verify: crate::console::env_var("DARTFORGE_GC_VERIFY").as_deref() == Ok("1"),
            validate_handles: stress
                || cfg!(any(test, debug_assertions))
                // Poison only catches use-after-free in validation.
                || crate::console::env_var("DARTFORGE_GC_POISON").is_ok_and(|v| !v.is_empty() && v != "0")
                || crate::console::env_var("DARTFORGE_GC_VERIFY").as_deref() == Ok("1")
                || crate::console::env_var("DARTFORGE_VALIDATE_HANDLES").as_deref() == Ok("1"),
            trace: crate::console::env_var("DARTFORGE_GC_TRACE").as_deref() == Ok("1"),
            memory: crate::console::env_var("DARTFORGE_GC_MEMORY")
                .ok()
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(0),
            finalizable: crate::hash::HashMap::default(),
            weak_refs: crate::hash::HashMap::default(),
            ephemerons: crate::hash::HashMap::default(),
            attachments: Vec::new(),
            attachments_by_owner: crate::hash::HashMap::default(),
            ready_finalizations: std::collections::VecDeque::new(),
        }
    }
    /// Owning slot of the runtime: 0 = pending exception, 1 = current stack trace.
    ///
    /// # Panics
    /// `which` is neither 0 nor 1, or a dead handle is detected by validation.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.set_runtime_root(0, 0);
    /// ```
    pub fn set_runtime_root(&mut self, which: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.check_live(handle);
        }
        let old = self.runtime_roots[which];
        if handle != old {
            self.arc_swap_owning_root(handle, 0);
        }
        self.runtime_roots[which] = handle;
        if handle != old {
            self.arc_swap_owning_root(0, old);
        }
    }
    /// The interned string with the `units` (the `literals` table, a root), if it already
    /// exists: the JIT's and the runtime's literal (`text.rs`, P1).
    pub fn literal(&self, units: &[u16]) -> Option<Ref> {
        self.literals.get(units).copied()
    }
    /// Interns `h` (a string with the `units`) as a canonical literal: root
    /// (`literals`) and permanent (identity across ports of the same isolate).
    /// The entry counts one owner; repeating the same registration does not retain again.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let text = heap.allocate_str("A");
    /// heap.store_literal(vec![65], text);
    /// assert_eq!(heap.literal(&[65]), Some(text));
    /// ```
    pub fn store_literal(&mut self, units: Vec<u16>, h: Ref) {
        if smi::e_handle(h) {
            self.check_live(h);
        }
        let old = self.literals.get(&units).copied().unwrap_or(0);
        self.arc_swap_owning_root(h, old);
        self.literals.insert(units, h);
        self.permanents.insert(h);
    }
    /// Copies the value to the global owner; null or `Smi` releases the previous owner.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.set_global_root(10, 0);
    /// ```
    pub fn set_global_root(&mut self, id: i64, handle: i64) {
        if smi::e_handle(handle) {
            self.check_live(handle);
        }
        let old = self.globals.get(&id).copied().unwrap_or(0);
        if handle != old {
            self.arc_swap_owning_root(handle, 0);
        }
        if !smi::e_handle(handle) {
            // null or `Smi`: nothing in the heap to keep alive.
            self.globals.remove(&id);
        } else {
            self.globals.insert(id, handle);
        }
        if handle != old {
            self.arc_swap_owning_root(0, old);
        }
    }

    /// Transfers an SSA token to the owning registry of a global slot.
    /// The caller publishes the bits in the actual storage before this call,
    /// with no safepoint in between. The registry does not keep null/Smi as values.
    pub(crate) fn move_code_to_global(&mut self, id: i64, value: Ref) {
        let old = self.globals.get(&id).copied().unwrap_or(0);
        if self.virtual_code_tokens {
            // A virtual token has no count to hand over: the slot retains.
            self.set_global_root(id, value);
            return;
        }
        if smi::e_handle(value) {
            self.check_live(value);
            // Without the inventory (the real ARC) the token's count simply moves.
            if self.code_inventory() {
                let n = *self
                    .code_owners
                    .get(&value)
                    .expect("ARC global move without a code token");
                if n == 1 {
                    self.code_owners.remove(&value);
                } else {
                    self.code_owners.insert(value, n - 1);
                }
            }
            self.globals.insert(id, value);
        } else {
            self.globals.remove(&id);
        }
        self.arc_swap_owning_root(0, old);
    }

    /// Moves the root of a global from one slot address to another (the globals
    /// area of a reloaded module, `gc_roots.rs`).
    /// Releases the destination's previous owner. A missing source or a move to
    /// the same address does not change the owners.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.move_global_root(10, 20);
    /// ```
    pub fn move_global_root(&mut self, of: i64, to: i64) {
        if of == to {
            return;
        }
        if let Some(h) = self.globals.remove(&of) {
            let old = self.globals.get(&to).copied().unwrap_or(0);
            self.globals.insert(to, h);
            self.arc_swap_owning_root(0, old);
        }
    }

    /// Releases the root of a global that ceased to exist.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.release_global_root(10);
    /// ```
    pub fn release_global_root(&mut self, id: i64) {
        if let Some(h) = self.globals.remove(&id) {
            self.arc_swap_owning_root(0, h);
        }
    }

    /// Marks `handle` as permanent (see `permanents`).
    pub fn mark_permanent(&mut self, handle: i64) {
        if smi::e_handle(handle) {
            self.permanents.insert(handle);
        }
    }

    /// Marks `handle` as the canonical constant that `getter` produces.
    pub fn mark_constant(&mut self, handle: i64, getter: usize) {
        if smi::e_handle(handle) {
            self.permanents.insert(handle);
            if getter != 0 {
                self.constants.insert(handle, getter);
            }
        }
    }

    /// The canonical form of the instance, list or record `h` that a `const`
    /// creation built: the first one with the same class, type arguments and
    /// contents, as the VM canonicalizes constants (`const C(1)` and
    /// `const C.named(1, 2)` that store the same field values are identical,
    /// and so are two `const` lists with the same elements). The string fields
    /// of an instance become the canonical literals first. Anything else comes
    /// back as it is.
    pub fn canonical_instance(&mut self, h: i64) -> i64 {
        use crate::layout::cid;
        if !is_object(h) {
            return h;
        }
        let class = self.class(h);
        let mut key = vec![i64::from(class), self.metadata(h)];
        if class == cid::IMMUTABLE_LIST {
            let n = self.list_len(h);
            key.push(n as i64);
            for i in 0..n {
                let v = self.list_get(h, i);
                self.push_value_key(&mut key, v);
            }
        } else if class == cid::RECORD {
            let words = self.words(h).to_vec();
            let n = usize::try_from(words[0]).expect("record shape");
            key.push(words[0]);
            for &w in &words[1..=n] {
                self.push_value_key(&mut key, Value::Ref(w));
            }
            key.extend_from_slice(&words[n + 1..]);
        } else {
            let Some(o) = self.object(h) else {
                return h;
            };
            if matches!(class, cid::MINT | cid::DOUBLE | cid::BOOL) {
                return h;
            }
            let fields = o.to_vec();
            key.push(fields.len() as i64);
            for (i, (bits, is_ref)) in fields.into_iter().enumerate() {
                if !is_ref {
                    key.extend([0, bits]);
                    continue;
                }
                let mut bits = bits;
                if self.is_string(bits) && !is_static(bits) {
                    let t = self.text(bits).expect("checked").to_string_fn();
                    bits = self.string_literal(t.view());
                    self.set_field(h, i, bits, true);
                }
                self.push_value_key(&mut key, Value::Ref(bits));
            }
        }
        if let Some(&canonical) = self.canonical_instances.get(&key) {
            return canonical;
        }
        // The table is an owner (`owning_roots`): its new entry counts at once. The
        // nursery's promotion recounted it; the pure ARC never did.
        self.arc_swap_owning_root(h, 0);
        self.canonical_instances.insert(key, h);
        h
    }

    /// Appends to a canonical key the identity of the value `v`: numbers and
    /// booleans by value (boxed or not), strings by their canonical literal,
    /// any other reference by its handle (the elements of a constant are
    /// constants already).
    fn push_value_key(&mut self, key: &mut Vec<i64>, v: Value) {
        use crate::layout::cid;
        let r = match v {
            Value::Int(i) => return key.extend([3, i]),
            Value::Double(d) => return key.extend([4, d.to_bits() as i64]),
            Value::Bool(b) => return key.extend([5, i64::from(b)]),
            Value::Ref(r) => r,
        };
        if smi::is_smi(r) {
            return key.extend([3, smi::value(r)]);
        }
        if !is_object(r) {
            return key.extend([1, r]);
        }
        match self.class(r) {
            cid::MINT => key.extend([3, self.words(r)[0]]),
            cid::DOUBLE => key.extend([4, self.words(r)[0]]),
            cid::BOOL => key.extend([5, i64::from(self.bool_of(r) == Some(true))]),
            _ if self.is_string(r) && !is_static(r) => {
                let t = self.text(r).expect("checked").to_string_fn();
                let literal = self.string_literal(t.view());
                key.extend([1, literal]);
            }
            _ => key.extend([1, r]),
        }
    }

    /// The getter of the canonical constant `handle`, if it is one.
    pub fn constant_getter(&self, handle: i64) -> Option<usize> {
        self.constants.get(&handle).copied()
    }

    /// The code of the canonical tear-off `handle`, if it is one.
    pub fn tearoff_code(&self, handle: i64) -> Option<i64> {
        self.tearoff_code.get(&handle).copied()
    }

    /// Whether `handle` is permanent and immutable (see `permanents`): a
    /// message in the same isolate passes it without copying.
    pub fn is_permanent(&self, handle: i64) -> bool {
        self.permanents.contains(&handle) || is_static(handle)
    }
    /// Reads the heap ceiling from the environment once, at creation.
    fn limit_from_env() -> usize {
        const DEFAULT: usize = 256 * 1024 * 1024;
        match crate::console::env_var("DARTFORGE_HEAP_MAX_MB") {
            Ok(v) => match v.trim().parse::<usize>() {
                Ok(0) => usize::MAX,
                Ok(mb) => mb.saturating_mul(1024 * 1024),
                Err(_) => DEFAULT,
            },
            Err(_) => DEFAULT,
        }
    }

    /// Bytes the heap takes: the blocks in use (with the large regions and the
    /// outside bytes, attachments and external ones), plus `extra`.
    fn total_bytes(&self, extra: usize) -> usize {
        self.stats.estimated_bytes.saturating_add(extra)
    }

    /// Stops the process when the heap exceeds the ceiling.
    fn verify_ceiling(&self, extra: usize) {
        if self.byte_limit == usize::MAX {
            return;
        }
        let total = self.total_bytes(extra);
        if total > self.byte_limit {
            Self::abort_out_of_memory(total, self.byte_limit);
        }
    }

    #[cold]
    #[inline(never)]
    fn abort_out_of_memory(total: usize, limit: usize) -> ! {
        // No `panic!`: the runtime is called through `extern "C"` from the generated
        // code, and a panic crossing that boundary aborts with an unreadable stack
        // dump. One line and code 255 (the same as the VM's for an
        // uncaught exception) is what the harness needs.
        let mib = 1024 * 1024;
        eprintln!(
            "Out of memory: the DartForge heap reached {} MiB, above the ceiling of {} MiB (adjust with DARTFORGE_HEAP_MAX_MB, 0 disables it).",
            total / mib,
            limit / mib
        );
        crate::console::exit(255);
    }

    /// Gets the singleton of an enum value, protecting the internal allocations.
    /// The table keeps a persistent owner for each created singleton.
    ///
    /// # Panics
    /// The class identity or the index is negative.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let value = heap.enum_value(900, 0, "zero");
    /// assert_eq!(heap.enum_value(900, 0, "zero"), value);
    /// ```
    pub fn enum_value(&mut self, class_id: i64, index: i64, name: &str) -> i64 {
        assert!(class_id >= 0 && index >= 0, "invalid enum identity");
        if let Some(&handle) = self.enum_values.get(&(class_id, index)) {
            return handle;
        }
        let frame = self.push_owning_frame(1);
        let text = self.allocate_str(name);
        self.set_root(frame, 0, text);
        let object = self.new_object(class_id, &[(index, false), (text, true)]);
        self.arc_swap_owning_root(object, 0);
        self.enum_values.insert((class_id, index), object);
        self.permanents.insert(object);
        self.pop_frame(frame);
        object
    }
    /// The already registered canonical tear-off of the top-level function `code` (the
    /// `tearoffs` table, a root), if it exists (`boxes.rs`, P2).
    pub fn registered_tearoff(&self, code: i64) -> Option<Ref> {
        self.tearoffs.get(&code).copied()
    }
    /// Registers `h` as the canonical tear-off of `code`: root (`tearoffs`), the
    /// inverse (`tearoff_code`) and permanent.
    /// The table counts one owner, including replacement without duplicating aliases.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let closure = heap.new_closure(123, (0, false), 0, 0);
    /// heap.register_tearoff(123, closure);
    /// assert_eq!(heap.registered_tearoff(123), Some(closure));
    /// ```
    pub fn register_tearoff(&mut self, code: i64, h: Ref) {
        if smi::e_handle(h) {
            self.check_live(h);
        }
        let old = self.tearoffs.get(&code).copied().unwrap_or(0);
        self.arc_swap_owning_root(h, old);
        self.tearoffs.insert(code, h);
        self.tearoff_code.insert(h, code);
        self.permanents.insert(h);
    }
    /// Opens a root frame with a monotonic identifier.
    pub fn push_frame(&mut self) -> i64 {
        self.push_frame_with_slots(0)
    }
    /// Reserves fixed slots, initially null, reused by all iterations.
    pub fn push_frame_with_slots(&mut self, slots: usize) -> i64 {
        let id = self.next_frame;
        self.next_frame = id.checked_add(1).expect("frames exhausted");
        self.stats.root_slots = self
            .stats
            .root_slots
            .checked_add(slots)
            .expect("slots exceed usize");
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
        // The slot vectors of closed frames are reused: a native that roots
        // its arguments around an allocation opened a frame per call, and
        // the `vec!` cost a `malloc` and a `free` each time.
        let mut roots = self.frame_pool.pop().unwrap_or_default();
        roots.resize(slots, 0);
        self.frames.push((id, roots, false));
        id
    }
    /// Opens owning slots: copying retains and closing releases under ARC.
    /// Observational frames must keep using `push_frame_with_slots`.
    ///
    /// # Panics
    /// The identifier or the sum of slots ran out.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let frame = heap.push_owning_frame(1);
    /// heap.set_root(frame, 0, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn push_owning_frame(&mut self, slots: usize) -> i64 {
        let id = self.push_frame_with_slots(slots);
        self.frames.last_mut().unwrap().2 = true;
        id
    }

    /// Requires an owner before an ABI operation; an observational frame cannot
    /// accept a copy the compiler believes is counted.
    pub(crate) fn check_owning_frame(&self, frame: i64) {
        let (_, _, owning) = self
            .frames
            .iter()
            .rfind(|(id, _, _)| *id == frame)
            .expect("nonexistent ARC frame");
        assert!(*owning, "ARC frame must be owning");
    }

    fn owning_roots(&self) -> Vec<Ref> {
        self.frames
            .iter()
            .filter(|(_, _, owner)| *owner)
            .flat_map(|(_, slots, _)| slots.iter().copied())
            .chain(self.globals.values().copied())
            .chain(self.literals.values().copied())
            .chain(self.canonical_instances.values().copied())
            .chain(self.tearoffs.values().copied())
            .chain(self.enum_values.values().copied())
            .chain(self.message_owners.values().copied())
            .chain(
                self.code_owners
                    .iter()
                    .filter(|_| !self.virtual_code_tokens)
                    .flat_map(|(&h, &n)| std::iter::repeat_n(h, n)),
            )
            .chain(self.ready_finalizations.iter().copied())
            .chain(self.runtime_roots.iter().copied())
            .collect()
    }

    /// Publishes an owning occurrence in the queue, without collection between retention and write.
    #[cfg(test)]
    pub(crate) fn add_ready_finalization(&mut self, action: Ref) {
        self.check_live(action);
        self.arc_swap_owning_root(action, 0);
        self.ready_finalizations.push_back(action);
    }

    /// Consumes the owner of the first finalization; an empty queue is a no-op.
    pub(crate) fn finish_ready_finalization(&mut self) -> Option<Ref> {
        let action = self.ready_finalizations.pop_front()?;
        self.arc_swap_owning_root(0, action);
        Some(action)
    }

    /// Transfers the queue occurrence to the active callback, without additional retention.
    pub(crate) fn move_finalization_to_slot(&mut self, frame: i64, slot: usize) -> Ref {
        let ix = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        assert!(self.frames[ix].2, "move requires an owning frame");
        let old = *self.frames[ix].1.get(slot).expect("invalid root slot");
        let value = *self
            .ready_finalizations
            .front()
            .expect("empty finalization queue");
        self.arc_swap_owning_root(0, old);
        self.ready_finalizations.pop_front();
        self.frames[ix].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        value
    }

    /// Retains a message occurrence in this heap's domain.
    pub(crate) fn retain_message_owner(&mut self, value: Ref) -> u64 {
        self.check_live(value);
        let id = self
            .next_message_owner
            .checked_add(1)
            .expect("message owners exhausted");
        self.arc_swap_owning_root(value, 0);
        self.message_owners.insert(id, value);
        self.next_message_owner = id;
        id
    }

    /// Consumes a message occurrence in this heap, without collection.
    pub(crate) fn release_message_owner(&mut self, id: u64) {
        let value = self
            .message_owners
            .remove(&id)
            .expect("nonexistent message owner");
        self.arc_swap_owning_root(0, value);
    }

    /// Diagnostic snapshot: bit 0 for a live heap block, bit 1 for a code owner.
    /// Consults the page index before reading the header; does not dereference
    /// addresses outside the heap nor include static objects, null or Smi.
    pub(crate) fn observe_code_owner(&self, address: i64) -> u8 {
        let alive = self.objects.occupied_block(address);
        u8::from(alive) | (u8::from(self.code_owners.contains_key(&address)) << 1)
    }

    /// Does `address` have a code token? Bit 1 of [`Self::observe_code_owner`],
    /// without searching for the block in the free lists (the check of each
    /// Owned return of the ports).
    pub(crate) fn has_code_owner(&self, address: i64) -> bool {
        self.virtual_code_tokens
            || !self.code_inventory()
            || self.code_owners.contains_key(&address)
    }

    /// Makes the tokens of the generated code virtual ([`Heap::virtual_code_tokens`]).
    /// Only before any token exists: the program's entry calls it right after
    /// turning on the ARC.
    ///
    /// # Panics
    /// Tokens already in the inventory.
    pub(crate) fn set_virtual_code_tokens(&mut self) {
        assert!(
            self.code_owners.is_empty(),
            "virtual ARC tokens must be chosen before the first token"
        );
        self.virtual_code_tokens = true;
    }

    /// Keeps a token the runtime received from Dart (the Owned return of a port
    /// that a native scope holds until it closes). Counted tokens already are in the
    /// inventory; a virtual one becomes a root without a count, so that the value
    /// survives the collections until [`Heap::drop_held_code_token`].
    pub(crate) fn hold_code_token(&mut self, value: Ref) {
        if self.virtual_code_tokens && smi::e_handle(value) {
            *self.code_owners.entry(value).or_insert(0) += 1;
        }
    }

    /// Ends the hold of [`Heap::hold_code_token`]: a counted token is released, a
    /// virtual one only leaves the inventory.
    pub(crate) fn drop_held_code_token(&mut self, value: Ref) {
        if !self.virtual_code_tokens {
            self.release_code_owner(value);
            return;
        }
        if !smi::e_handle(value) {
            return;
        }
        match self.code_owners.entry(value) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if *e.get() == 1 {
                    e.remove();
                } else {
                    *e.get_mut() -= 1;
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                panic!("ARC bug: a native scope released a token it does not hold")
            }
        }
    }

    /// Creates an occurrence for the generated code, without collection or Dart execution.
    /// The published observational location does not create another owner.
    /// A virtual token does nothing ([`Heap::virtual_code_tokens`]).
    pub(crate) fn retain_code_owner(&mut self, value: Ref) {
        if !smi::e_handle(value) || self.virtual_code_tokens {
            return;
        }
        if !self.code_inventory() {
            // The real ARC: the token is a counted owner, nothing more.
            self.arc_swap_owning_root(value, 0);
            return;
        }
        self.check_live(value);
        self.arc_swap_owning_root(value, 0);
        // A lookup only in the inventory (the path of each code retain).
        let n = self.code_owners.entry(value).or_insert(0);
        *n = n.checked_add(1).expect("code ARC tokens exhausted");
    }

    /// Consumes exactly one ABI occurrence, never a slot owner.
    /// A virtual token does nothing ([`Heap::virtual_code_tokens`]).
    pub(crate) fn release_code_owner(&mut self, value: Ref) {
        if !smi::e_handle(value) || self.virtual_code_tokens {
            return;
        }
        if !self.code_inventory() {
            self.arc_swap_owning_root(0, value);
            return;
        }
        match self.code_owners.entry(value) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if *e.get() == 1 {
                    e.remove();
                } else {
                    *e.get_mut() -= 1;
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                panic!("ARC release without a code token")
            }
        }
        self.arc_swap_owning_root(0, value);
    }

    /// Copies an owning slot into an independent code token.
    pub(crate) fn copy_root_to_code(&mut self, frame: i64, slot: usize) -> Ref {
        self.check_owning_frame(frame);
        let i = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .unwrap();
        let value = *self.frames[i].1.get(slot).expect("invalid root slot");
        self.retain_code_owner(value);
        value
    }

    /// Transfers a code token into a slot, without retaining the source.
    /// Validates everything before consuming/publishing; releases the old one after publishing.
    pub(crate) fn move_code_to_root(&mut self, frame: i64, slot: usize, value: Ref) {
        self.check_owning_frame(frame);
        let i = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .unwrap();
        let old = *self.frames[i].1.get(slot).expect("invalid root slot");
        if self.virtual_code_tokens {
            // A virtual token has no count to hand over: the owning slot retains.
            self.set_root(frame, slot, value);
            return;
        }
        // Without the inventory (the real ARC) the token's count simply moves.
        if smi::e_handle(value) && self.code_inventory() {
            self.check_live(value);
            let n = *self
                .code_owners
                .get(&value)
                .expect("ARC move without a code token");
            if n == 1 {
                self.code_owners.remove(&value);
            } else {
                self.code_owners.insert(value, n - 1);
            }
        }
        self.frames[i].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        // The new occurrence was already counted in the code inventory.
        self.arc_swap_owning_root(0, old);
    }

    /// Retains before releasing, with no collection nor Dart call between the operations.
    ///
    /// In nursery mode a young target is not registered: a young object has no
    /// count until promotion, which counts the owning occurrences that still hold it
    /// ([`Heap::sync_arc`]). Registering it here would give it a count that misses
    /// the occurrences of young owners, and its death by RC before promotion would
    /// release children it never counted.
    fn arc_swap_owning_root(&mut self, new: Ref, old: Ref) {
        if new == old {
            return;
        }
        let Some(arc) = self.arc.as_ref() else {
            return;
        };
        if arc.pure {
            // Disjoint fields: the space answers the registration, without moving the
            // ARC state out of the heap and back (the path of every code token).
            let validate = self.validate_handles;
            let objects = &self.objects;
            let arc = self.arc.as_deref_mut().expect("ARC on");
            retain_registering_in(objects, validate, arc, new);
            arc.state.release(old).unwrap_or_else(|e| arc_failure(e));
            return;
        }
        // The nursery: a young object (the usual target of the prepared code's
        // tokens) has no metadata, and its header says so without a table lookup.
        let (new, old) = (self.arc_counted(new), self.arc_counted(old));
        let arc = self.arc.as_deref_mut().expect("ARC on");
        if let Some(new) = new {
            arc.state.retain_ref(new).unwrap_or_else(|e| arc_failure(e));
        }
        if let Some(old) = old {
            arc.state.release(old).unwrap_or_else(|e| arc_failure(e));
        }
    }

    /// Under the nursery, `h` if its count may exist (a handle that is not a young
    /// object of the space); `None` for a scalar, null or young object, whose
    /// retain and release do nothing.
    #[inline]
    fn arc_counted(&self, h: Ref) -> Option<Ref> {
        (smi::e_handle(h) && !self.young_child(h)).then_some(h)
    }

    /// Copies the root into the slot; zero removes the previous reference.
    /// In an owning frame, retains the new occurrence before releasing the old one.
    /// A `Smi` occupies the slot like any `Ref`, but the collector does not follow it.
    pub fn set_root(&mut self, frame: i64, slot: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.check_live(handle);
        }
        let ix = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        let old = *self.frames[ix].1.get(slot).expect("invalid root slot");
        let owning = self.frames[ix].2;
        if owning && handle != old {
            self.arc_swap_owning_root(handle, 0);
        }
        let previous = &mut self.frames[ix].1[slot];
        self.stats.live_roots -= usize::from(*previous != 0);
        self.stats.live_roots += usize::from(handle != 0);
        *previous = handle;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        if owning && handle != old {
            self.arc_swap_owning_root(0, old);
        }
    }
    /// Transfers an occurrence between owning slots, zeroing the source.
    /// Releases the previous content of the destination without retaining the transferred value.
    /// Moving a slot onto itself preserves its content and its count.
    /// There is no collection nor Dart call during the transfer.
    ///
    /// # Panics
    /// Some frame does not exist, is not owning, or the slot is invalid.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let caller = heap.push_owning_frame(1);
    /// let callee = heap.push_owning_frame(1);
    /// heap.move_root(callee, 0, caller, 0);
    /// heap.pop_frame(callee);
    /// heap.pop_frame(caller);
    /// ```
    pub fn move_root(&mut self, origin: i64, source_slot: usize, dest: i64, dest_slot: usize) {
        let of = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == origin)
            .expect("nonexistent source frame");
        let to = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == dest)
            .expect("nonexistent destination frame");
        assert!(
            self.frames[of].2 && self.frames[to].2,
            "move requires owning frames"
        );
        let value = *self.frames[of]
            .1
            .get(source_slot)
            .expect("invalid source slot");
        let old = *self.frames[to]
            .1
            .get(dest_slot)
            .expect("invalid destination slot");
        if of == to && source_slot == dest_slot {
            return;
        }
        self.frames[of].1[source_slot] = 0;
        self.frames[to].1[dest_slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.arc_swap_owning_root(0, old);
    }

    /// Transfers a global owner into an owning slot, consuming the global.
    /// Releases the previous content of the slot without retaining the transferred value.
    ///
    /// # Panics
    /// The global does not exist, the frame is not owning, or the slot is invalid.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let text = heap.allocate_str("event");
    /// heap.set_global_root(-1, text);
    /// let frame = heap.push_owning_frame(1);
    /// heap.move_global_to_slot(-1, frame, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn move_global_to_slot(&mut self, global: i64, frame: i64, slot: usize) {
        let ix = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        assert!(self.frames[ix].2, "move requires an owning frame");
        let old = *self.frames[ix].1.get(slot).expect("invalid root slot");
        let value = *self.globals.get(&global).expect("owner global inexistente");
        self.globals.remove(&global);
        self.frames[ix].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.arc_swap_owning_root(0, old);
    }

    /// Protects a handle until the function returns; null does not occupy a root.
    pub fn root(&mut self, frame: i64, handle: i64) {
        if !smi::e_handle(handle) {
            return;
        }
        self.check_live(handle);
        let ix = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        if self.frames[ix].2 {
            self.arc_swap_owning_root(handle, 0);
        }
        self.frames[ix].1.push(handle);
        self.stats.live_roots += 1;
        self.stats.root_slots += 1;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
    }
    /// Closes exactly the top frame, without collecting between the return and the caller's root.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _, _)| *id), Some(frame));
        let (_, mut roots, owning) = self.frames.pop().unwrap();
        if owning {
            for &h in &roots {
                self.arc_swap_owning_root(0, h);
            }
        }
        self.stats.root_slots -= roots.len();
        self.stats.live_roots -= roots.iter().filter(|handle| **handle != 0).count();
        if self.frame_pool.len() < FRAME_POOL {
            roots.clear();
            self.frame_pool.push(roots);
        }
    }
    /// Collects if some trigger asks for it, checks the ceiling and counts the allocation of
    /// `bytes`.
    fn before_allocate(&mut self, bytes: usize) {
        // Two different limits (G6):
        // * the TRIGGER (`threshold`, `byte_threshold`): how much is allocated since
        //   the last collection, recomputed at the end of each one
        //   (`recompute_triggers`) from the survivors and the slack up to
        //   the ceiling;
        // * the CEILING (`byte_limit`): only the allocation that would exceed it forces
        //   one last collection; if it still exceeds it, memory is out.
        // The old gate (`!self.frames.is_empty()`) existed because the generated
        // code did not register roots; with each function's frame (G1),
        // collecting without an open frame is just collecting with the permanent roots.
        if !self.gc_disabled && self.collection_deferred == 0 && self.needs_collection(bytes) {
            self.collect_automatic(bytes);
        }
        // After the collection: if it still exceeds the ceiling, there is nothing to reclaim.
        self.verify_ceiling(bytes);
        self.count_allocation(1, bytes);
    }
    /// A new user object with the fields `fields` (those the runtime
    /// builds by hand: errors, enum values, the `Type` class…). Collects first
    /// like [`Heap::allocate`]: the references in `fields` need a root
    /// from the caller.
    pub fn new_object(&mut self, class_id: i64, fields: &[Field]) -> i64 {
        let h = self.allocate_object(class_id, fields.len());
        for (i, &(bits, is_ref_kind)) in fields.iter().enumerate() {
            self.set_field(h, i, bits, is_ref_kind);
        }
        h
    }
    /// Counts `k` allocations of `bytes` in total for the triggers and the
    /// counters.
    #[inline]
    fn count_allocation(&mut self, k: usize, bytes: usize) {
        self.add_estimated_bytes(bytes);
        self.allocations += k;
        self.stats.allocations += k as u64;
        self.young_bytes = self.young_bytes.saturating_add(bytes);
        self.gc_policy_after_growth(false);
    }
    /// Invalidates cached reservations without collecting: callers may still
    /// hold an unrooted newly allocated header or attachment.
    fn gc_policy_after_growth(&mut self, external_growth: bool) {
        // External growth can invalidate prepaid quotas even after cancellation
        // or expiry. Never let an old cached range conceal that pressure.
        if !external_growth && !self.gc_policy.is_active() {
            return;
        }
        // The clock only expires a window: without one, `refresh` does nothing
        // (and external growth comes with every socket read and buffer write).
        if self.gc_policy.window.is_some() {
            self.gc_policy.refresh(crate::clock::Instant::now());
        }
        let pressure = self.allocations >= YOUNG_COUNT
            || self.young_bytes >= self.young_limit
            || self.stats.estimated_bytes >= self.byte_threshold
            || (self.byte_limit != usize::MAX && self.total_bytes(0) >= self.byte_limit);
        if self.gc_policy.window.is_some() && pressure {
            self.gc_policy.debt = true;
        }
        if self.gc_policy.debt || (external_growth && pressure) {
            self.return_tlabs();
        }
    }
    /// Arms automatic-GC deferral after returning unspent inline reservations.
    pub(crate) fn gc_policy_defer(&mut self, duration: std::time::Duration, budget: usize) -> i64 {
        let status = self
            .gc_policy
            .arm(crate::clock::Instant::now(), duration, budget);
        if status == 1 {
            self.return_tlabs();
        }
        status
    }
    pub(crate) fn gc_policy_elapsed_micros(&self) -> i64 {
        self.gc_policy
            .origin
            .elapsed()
            .as_micros()
            .min(i64::MAX as u128) as i64
    }
    pub(crate) fn gc_policy_cancel(&mut self) {
        self.gc_policy.cancel();
    }
    pub(crate) fn gc_policy_request_idle(&mut self) {
        self.gc_policy.idle_requested = true;
    }
    pub(crate) fn gc_policy_idle_deadline(&self) -> Option<crate::clock::Instant> {
        if self.gc_policy.idle_requested {
            self.gc_policy.window.map(|(deadline, _)| deadline)
        } else {
            None
        }
    }
    /// Called only after the owner scheduler establishes genuine idleness.
    pub(crate) fn gc_policy_collect_if_idle(&mut self) -> bool {
        self.gc_policy.refresh(crate::clock::Instant::now());
        if !self.gc_policy.idle_requested || self.gc_policy.window.is_some() {
            return false;
        }
        self.collect();
        true
    }
    /// Was the `late` field `key` (handle, index) already written?
    #[inline]
    pub fn late_initialized(&self, key: (i64, i64)) -> bool {
        !self.late_fields_initialized.is_empty() && self.late_fields_initialized.contains(&key)
    }
    /// Marks the `late` field `key` as written.
    pub fn mark_late(&mut self, key: (i64, i64)) {
        if self.late_fields_initialized.insert(key) {
            self.new_lates.push(key);
        }
    }
    /// Undoes the mark of the `late` field `key`.
    pub fn unmark_late(&mut self, key: (i64, i64)) {
        self.late_fields_initialized.remove(&key);
    }
    /// The heap of the thread's isolate: refills the TLAB of the [`Context`] (the
    /// generated code's inline allocation).
    pub fn for_isolate(stress: bool) -> Self {
        let mut h = Self::new(stress);
        h.published = true;
        h.context = CONTEXT.with(|c| {
            // Whatever a previous heap of this thread left in the TLAB (the tests
            // swap the thread heap) belongs to another object space: this heap
            // starts with empty ranges.
            Self::clear_tlab(c);
            c as *const Context
        });
        h
    }
    /// Empties the TLAB ranges of `c` without returning them to any heap: the
    /// blocks stay counted as handed out in the heap that took them (its next
    /// collection sweeps the unused ones, zeroed and `FREE`, as dead).
    fn clear_tlab(c: &Context) {
        for n in 0..=TLAB_N {
            c.tlab[n][0].set(std::ptr::null_mut());
            c.tlab[n][1].set(std::ptr::null_mut());
        }
    }
    /// The [`Context`] whose TLAB this heap uses, if it is the heap of the
    /// thread's isolate and the TLAB is in use (never under `--gc-stress` nor
    /// `DARTFORGE_GC_SCHEDULE`: there the ranges stay empty and every allocation
    /// goes through the triggers).
    #[inline(always)]
    fn tlab_context(&self) -> Option<*const Context> {
        if self.context.is_null() || self.stress || self.schedule.is_some() {
            return None;
        }
        Some(self.context)
    }
    /// Allocates a user object of `n` zeroed fields (the path of
    /// `dartforge_object_new`), in the object space.
    ///
    /// The fast path checks the triggers of [`Heap::allocate`] with one
    /// comparison each; if any fires (collection, ceiling, `--gc-stress`), it goes
    /// to the general path.
    #[inline]
    pub fn allocate_object(&mut self, class_id: i64, n: usize) -> i64 {
        let cid = i32::try_from(class_id).expect("class id beyond 32 bits");
        self.allocate_block(
            cid,
            crate::layout::instance_words(n),
            block_flags::INSTANCE,
            n,
        )
    }
    /// A block with a body of `words` zeroed words, class `cid`, `flags` and
    /// `n` in the header (in `INSTANCE`, the number of fields; in the others, the
    /// words): the path of [`Heap::allocate`], [`Heap::allocate_object`] and of
    /// `dartforge_allocate`.
    #[inline]
    pub(crate) fn allocate_block(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> Ref {
        if let Some(h) = self.allocate_from_tlab(cid, words, flags, n) {
            return h;
        }
        self.allocate_block_outside_tlab(cid, words, flags, n)
    }
    /// The runtime's bump allocation from the TLAB of the thread [`Context`]: the
    /// same ranges, and the same header write, as the generated code's inline
    /// allocation (`@df.allocate`, `llvm/mod.rs`). A block of `words ≤ TLAB_N`
    /// body words when the range of that size has room; `None` otherwise (the
    /// general path, [`Heap::allocate_block_outside_tlab`], which also refills
    /// the range). Only through [`Heap::tlab_context`]: the heap of the thread's
    /// isolate, never under `--gc-stress` or `DARTFORGE_GC_SCHEDULE`.
    ///
    /// The block comes zeroed and already counted (bytes, allocations, `live_set`,
    /// `handed_out`) by [`Heap::refill_tlab`], which only hands out what fits
    /// before the next trigger: nothing is counted or checked here. The object
    /// lives in a range of `ObjectSpace::ranges`, which the minor sweep,
    /// `young_marked` and `young_handed_out` walk: a young object like any other.
    /// Does not collect: a caller that holds unrooted references may use it
    /// before falling back to a rooted general allocation.
    #[inline(always)]
    pub(crate) fn allocate_from_tlab(
        &mut self,
        cid: i32,
        words: usize,
        flags: u8,
        n: usize,
    ) -> Option<Ref> {
        if words.wrapping_sub(1) >= TLAB_N || !self.runtime_tlab {
            return None;
        }
        // SAFETY: `context` is the `Context` of this thread (`for_isolate`), a
        // thread-local without a destructor: it outlives every heap of the thread.
        #[allow(unsafe_code)]
        let c = unsafe { &*self.tlab_context()? };
        let cursor = c.tlab[words][0].get();
        let next = cursor.wrapping_add(block_bytes(words));
        if next > c.tlab[words][1].get() {
            return None;
        }
        c.tlab[words][0].set(next);
        // SAFETY: a zeroed free block of a range that `refill_tlab` took from this
        // heap's space for the Context (word 0 is the header).
        #[allow(unsafe_code)]
        unsafe {
            cursor
                .cast::<u64>()
                .write(crate::layout::header_word(YOUNG, flags, n, cid));
        }
        Some(cursor as i64 + HANDLE_OFFSET)
    }
    /// [`Heap::allocate_block`] outside the TLAB: the triggers, the counting and a
    /// block from the space; then the TLAB of `words` is refilled, so that the
    /// next allocations of that size (by the runtime or the generated code) come
    /// from it.
    #[inline(never)]
    fn allocate_block_outside_tlab(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> Ref {
        let bytes = allocation_bytes(words);
        let h = if self.needs_collection(bytes) {
            self.allocate_slow(cid, words, flags, n)
        } else {
            self.count_allocation(1, bytes);
            self.objects.allocate(cid, words, flags, n)
        };
        self.refill_tlab(words);
        h
    }
    /// Would an allocation of `bytes` now exceed some collection trigger?
    /// The minor one comes every [`YOUNG_LIMIT`] bytes (or [`YOUNG_COUNT`]
    /// allocations) since the last collection; the full one, when the total exceeds the
    /// trigger the last full one computed (`byte_threshold`) or the ceiling.
    #[inline]
    fn needs_collection(&mut self, bytes: usize) -> bool {
        let forced = self.stress
            || self.schedule.as_ref().is_some_and(Schedule::draws)
            || (self.byte_limit != usize::MAX && self.total_bytes(bytes) > self.byte_limit);
        let ordinary = self.allocations >= YOUNG_COUNT
            || self.young_bytes.saturating_add(bytes) > self.young_limit
            || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold;
        if !self.gc_policy.is_active() {
            return forced || ordinary;
        }
        let collect =
            self.gc_policy
                .collect_before(crate::clock::Instant::now(), bytes, ordinary, forced);
        if self.gc_policy.debt {
            self.return_tlabs();
        }
        collect
    }
    /// The collection the triggers ask for before allocating `bytes`: the full one if
    /// the total exceeded its trigger (or the ceiling, or every
    /// [`MINORS_PER_FULL_UNDER_STRESS`] under `--gc-stress`); otherwise the minor one.
    fn collect_automatic(&mut self, bytes: usize) {
        let full = self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.byte_limit != usize::MAX && self.total_bytes(bytes) > self.byte_limit)
            || (self.stress
                && self.minors_since_full + 1
                    >= MINORS_PER_FULL_UNDER_STRESS
                        .max((self.work_in_full / WORK_PER_MINOR_UNDER_STRESS) as u64));
        self.run_collection(!full);
    }
    #[cold]
    #[inline(never)]
    fn allocate_slow(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> Ref {
        self.before_allocate(allocation_bytes(words));
        self.objects.allocate(cid, words, flags, n)
    }
    /// Refills the TLAB for `n` fields of the [`Context`] (if exhausted) with
    /// a contiguous range of up to `TLAB_BLOCKS` free blocks, counted
    /// as allocated now — without collecting: only what fits before the next
    /// trigger (what exceeds it is left to the runtime, which collects). No TLAB
    /// under `--gc-stress` (every allocation goes through the runtime, which collects first)
    /// nor in test heaps.
    pub fn refill_tlab(&mut self, words: usize) {
        let n = words;
        if n == 0 || n > TLAB_N {
            return;
        }
        let Some(c) = self.tlab_context() else { return };
        // SAFETY: `context` is the `Context` of this thread (`for_isolate`), a
        // thread-local without a destructor: it outlives every heap of the thread.
        #[allow(unsafe_code)]
        let c = unsafe { &*c };
        let size = block_bytes(n);
        if (c.tlab[n][1].get() as usize).saturating_sub(c.tlab[n][0].get() as usize) >= size {
            return;
        }
        if self.gc_policy.window.is_some() {
            self.gc_policy.refresh(crate::clock::Instant::now());
        }
        if self.gc_policy.blocks_tlabs() {
            return;
        }
        let mut k = self
            .tlab_blocks
            .min(YOUNG_COUNT.saturating_sub(self.allocations))
            .min(self.young_limit.saturating_sub(self.young_bytes) / size)
            .min(
                self.byte_threshold
                    .saturating_sub(self.stats.estimated_bytes)
                    / size,
            );
        if self.byte_limit != usize::MAX {
            k = k.min(self.byte_limit.saturating_sub(self.total_bytes(0)) / size);
        }
        if let Some((_, remaining)) = self.gc_policy.window {
            // Reservations count as gross growth now. Leave one byte so their
            // accounting cannot create debt while the range is unpublished.
            k = k.min(remaining.saturating_sub(1) / size);
        }
        if k == 0 {
            return;
        }
        let (start, k) = self.objects.take_range(n, k);
        self.objects.live_set += k;
        // This transaction has already checked all quotas and the policy
        // allowance. Do not run the growth checkpoint until the range exists
        // in Context: returning TLABs here would miss this reservation.
        let bytes = k * size;
        self.add_estimated_bytes(bytes);
        self.allocations += k;
        self.stats.allocations += k as u64;
        self.young_bytes = self.young_bytes.saturating_add(bytes);
        c.tlab[n][0].set(start);
        c.tlab[n][1].set(start.wrapping_add(k * size));
    }
    /// Returns the TLABs of the [`Context`] (what the generated code did not use) before
    /// a collection: the rest of each range goes back to the front of the free list
    /// and leaves the allocation count.
    fn return_tlabs(&mut self) {
        if self.context.is_null() {
            return;
        }
        // SAFETY: `context` is the `Context` of this thread (`for_isolate`), a
        // thread-local without a destructor: it outlives every heap of the thread.
        #[allow(unsafe_code)]
        let c = unsafe { &*self.context };
        for n in 1..=TLAB_N {
            let (cursor, end) = (
                c.tlab[n][0].replace(std::ptr::null_mut()),
                c.tlab[n][1].replace(std::ptr::null_mut()),
            );
            let size = block_bytes(n);
            let k = (end as usize).saturating_sub(cursor as usize) / size;
            if k > 0 {
                self.objects.return_range(n, cursor, end);
                let bytes = k * size;
                self.objects.live_set -= k;
                self.allocations = self.allocations.saturating_sub(k);
                self.stats.allocations = self.stats.allocations.saturating_sub(k as u64);
                self.subtract_estimated_bytes(bytes);
                self.young_bytes = self.young_bytes.saturating_sub(bytes);
            }
        }
    }
    /// The live block of object `h` (`h & 3 == 2`), or an N4 `panic` for an invalid
    /// handle or an already collected object.
    ///
    /// A static object (image section, `PERMANENT`) is also returned:
    /// it reads like any block; writers check
    /// ([`Heap::check_writable`]).
    ///
    /// The failures have different causes and the message is the harness's grouping
    /// key (docs/NATIVO-PLANO.md §6.4, N4): fixed text on the first line and
    /// the detail on the second — dereferenced null, `Smi` or negative scalar
    /// used as a handle, handle outside the space (positive scalar) and an already
    /// collected object (missing root).
    #[inline(always)]
    fn live_block(&self, handle: i64) -> *mut Header {
        if !self.validate_handles && is_object(handle) {
            let b = (handle - HANDLE_OFFSET) as *mut Header;
            // SAFETY: without validation, an object handle is a block of the space
            // or a static (`validate_handles`).
            #[allow(unsafe_code)]
            if unsafe { (*b).state } != FREE {
                return b;
            }
        }
        self.live_block_checked(handle)
    }
    /// [`Heap::live_block`] through the page map, with the N4 messages (the
    /// path of `validate_handles` and that of the failures).
    #[cold]
    #[inline(never)]
    fn live_block_checked(&self, handle: i64) -> *mut Header {
        if !is_object(handle) {
            Self::invalid_handle(handle);
        }
        match self.objects.block_of(handle) {
            // SAFETY: block of a page of the space.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).state } == crate::space::POISON_STATE => {
                panic!(
                    "compiler bug: handle already collected (missing root; poisoned block, DARTFORGE_GC_POISON)\nhandle {handle}"
                )
            }
            // SAFETY: block of a page of the space.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).state } != 0 => b,
            Some(_) => {
                panic!("compiler bug: handle already collected (missing root)\nhandle {handle}")
            }
            None if is_static(handle) => (handle - HANDLE_OFFSET) as *mut Header,
            None if self.objects.in_page(handle) => {
                panic!("compiler bug: handle already collected (missing root)\nhandle {handle}")
            }
            None => panic!(
                "compiler bug: handle beyond the table (scalar used as a handle)\nhandle {handle}"
            ),
        }
    }
    /// The block of `h` if it is an object of the space (live or `FREE`; never a
    /// static): the page map with `validate_handles`, otherwise the address
    /// itself.
    #[inline]
    fn space_block(&self, h: i64) -> Option<*mut Header> {
        if self.validate_handles {
            return self.objects.block_of(h);
        }
        if !is_object(h) {
            return None;
        }
        let b = (h - HANDLE_OFFSET) as *mut Header;
        // SAFETY: `validate_handles` (an object handle is a block or a static).
        #[allow(unsafe_code)]
        (unsafe { (*b).state } != PERMANENT).then_some(b)
    }
    /// Does the child `v` written into an old object require the barrier (§2.7)? Only a young object:
    /// null, `Smi`, old and static do not.
    #[inline]
    fn young_child(&self, v: Ref) -> bool {
        if !smi::e_handle(v) || v < 0 {
            return false;
        }
        // SAFETY: block of the space.
        #[allow(unsafe_code)]
        self.space_block(v)
            .is_some_and(|b| crate::layout::state::is_young(unsafe { (*b).state }))
    }
    /// The N4 panic of a handle that is not an object's.
    #[cold]
    #[inline(never)]
    fn invalid_handle(handle: i64) -> ! {
        if handle == 0 {
            panic!("compiler bug: null handle (0) dereferenced");
        }
        if smi::is_smi(handle) {
            panic!(
                "compiler bug: Smi used as a handle (int in a Ref position read as an object)\nSmi {}",
                smi::value(handle)
            );
        }
        if handle < 0 {
            panic!("compiler bug: negative handle (scalar used as a handle)\nhandle {handle}");
        }
        panic!("compiler bug: handle beyond the table (scalar used as a handle)\nhandle {handle}");
    }
    /// Panics if `b` is a static object (writing to it would bring down the process:
    /// the image section is read-only).
    #[inline]
    #[allow(unsafe_code)]
    fn check_writable(b: *const Header) {
        // SAFETY: readable block (from `live_block`).
        assert!(
            unsafe { (*b).state } != PERMANENT,
            "compiler bug: write to a static object"
        );
    }
    /// The identity `hashCode` of object `h` of the space: taken from the
    /// address, which does not change while the object lives (the collector does not move) and
    /// is unique among the live — 30 scattered bits, never 0 (the VM draws one and
    /// keeps it in the header; here the header has no room and does not need it).
    /// `None` if `h` is not a live object of the space.
    pub fn identity_hash(&self, h: i64) -> Option<i64> {
        let b = match self.space_block(h) {
            Some(b) => b,
            None if self.is_static(h) => (h - HANDLE_OFFSET) as *mut Header,
            None => return None,
        };
        // SAFETY: block of a page of the space.
        #[allow(unsafe_code)]
        if unsafe { (*b).state } == FREE {
            return None;
        }
        let x = ((b as u64 >> 3).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 34) as i64;
        Some(x.max(1))
    }
    /// The view of object `h` of the space (class and fields), or `None` if `h`
    /// is not a live object of the space (null, `Smi`, runtime value, scalar).
    pub fn object(&self, h: i64) -> Option<Obj<'_>> {
        let b = self.space_block(h)?;
        // SAFETY: block of a page of the space, checked alive.
        #[allow(unsafe_code)]
        unsafe {
            // The map is only a reference map in `INSTANCE` (§2.3).
            if (*b).state == FREE || shape(b) != block_flags::INSTANCE {
                return None;
            }
            Some(Obj {
                body: body(b),
                class_id: i64::from((*b).class_id),
                _heap: std::marker::PhantomData,
            })
        }
    }
    /// The read view of object `h`: of the space or static (the
    /// image constants, never written). The audited ARC reads
    /// read fields of both; writing uses [`Heap::object`].
    pub fn readable_object(&self, h: i64) -> Option<Obj<'_>> {
        if let Some(o) = self.object(h) {
            return Some(o);
        }
        if !self.is_static(h) {
            return None;
        }
        let b = (h - HANDLE_OFFSET) as *mut Header;
        // SAFETY: static block of the image, alive for the whole process.
        #[allow(unsafe_code)]
        unsafe {
            if shape(b) != block_flags::INSTANCE {
                return None;
            }
            Some(Obj {
                body: body(b),
                class_id: i64::from((*b).class_id),
                _heap: std::marker::PhantomData,
            })
        }
    }
    /// The class of object `h` of the space (`None` if it is not a live object).
    #[inline]
    pub fn object_class(&self, h: i64) -> Option<i64> {
        self.object(h).map(|o| o.class_id)
    }
    /// Writes field `i` of object `h` (with the reference mark and the
    /// write barrier).
    ///
    /// # Panics
    /// If `h` is not a live object or `i` is outside it.
    pub fn set_field(&mut self, h: i64, i: usize, bits: i64, is_ref_kind: bool) {
        if is_ref_kind && smi::e_handle(bits) {
            self.check_live(bits);
        }
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: live block.
        #[allow(unsafe_code)]
        let instance = unsafe { shape(b) } == block_flags::INSTANCE;
        assert!(
            instance,
            "runtime bug: set_field on an object that is not INSTANCIA"
        );
        self.objects.remember(b);
        // SAFETY: live block; `i` checked against the number of fields.
        #[allow(unsafe_code)]
        let old = unsafe {
            let c = body(b);
            assert!(
                i < usize::from((*c).n),
                "field {i} outside the object of {} fields",
                (*c).n
            );
            let old = if is_reference(c, i) {
                *fields_of(c).add(i)
            } else {
                0
            };
            *fields_of(c).add(i) = bits;
            mark_reference(c, i, is_ref_kind);
            old
        };
        if self.arc.is_some() {
            self.arc_swapped(b, h, old, if is_ref_kind { bits } else { 0 });
        }
    }
    /// Panics (N4) if `h` is not a live handle.
    fn check_live(&self, h: i64) {
        self.live_block(h);
    }
    /// The generational barrier of a runtime fast path that wrote a `Ref` into
    /// the space block `b` directly (`set_field` does the same).
    pub(crate) fn objects_remember(&mut self, b: *mut Header) {
        self.objects.remember(b);
    }
    /// The generated code's write barrier (`dartforge_remember`): the old
    /// object `h` received a `Ref` in a field.
    pub fn remember_object(&mut self, h: i64) {
        if let Some(b) = self.space_block(h) {
            self.objects.remember(b);
        }
    }
    /// The metadata of a live handle (0 = none).
    pub fn metadata(&self, handle: i64) -> i64 {
        // SAFETY: live block (or static).
        #[allow(unsafe_code)]
        i64::from(unsafe { (*self.live_block(handle)).metadata })
    }
    /// Writes the metadata of a live handle.
    pub fn set_metadata(&mut self, handle: i64, value: i64) {
        let b = self.live_block(handle);
        Self::check_writable(b);
        // SAFETY: live block.
        #[allow(unsafe_code)]
        unsafe {
            (*b).metadata = u32::try_from(value).expect("metadata beyond 32 bits")
        };
    }
    /// Migrates the objects of the classes in `plan` to the new layout (J03):
    /// each new position receives the value of the indicated old one (`-1`: zero, which
    /// is `null` for the nullable field and "uninitialized" for `late`), and
    /// the initialized-`late` marks follow the field.
    pub fn migrate_instances(&mut self, plan: &crate::hash::HashMap<i64, Vec<i64>>) {
        let mut blocks = Vec::new();
        self.objects.for_each_live(|b| blocks.push(b));
        for b in blocks {
            // SAFETY: live block of the space.
            #[allow(unsafe_code)]
            let class_id = i64::from(unsafe { (*b).class_id });
            let Some(origin) = plan.get(&class_id) else {
                continue;
            };
            let h = b as i64 + HANDLE_OFFSET;
            let olds = self.object(h).expect("live object").to_vec();
            let value_list: Vec<Field> = origin
                .iter()
                .map(|&o| {
                    usize::try_from(o)
                        .ok()
                        .and_then(|o| olds.get(o).copied())
                        .unwrap_or((0, false))
                })
                .collect();
            self.swap_fields(b, &value_list);
        }
        let marks: Vec<(i64, i64)> = self
            .late_fields_initialized
            .iter()
            .copied()
            .filter(|&(_, i)| i >= 0)
            .collect();
        for (h, i) in marks {
            let Some(class_id) = self.object_class(h) else {
                continue;
            };
            let Some(origin) = plan.get(&class_id) else {
                continue;
            };
            let new_area = origin.iter().position(|&o| o == i);
            self.late_fields_initialized.remove(&(h, i));
            if let Some(j) = new_area {
                self.mark_late((h, j as i64));
            }
        }
    }

    /// Replaces the fields of the object of block `b` with `value_list`: in the block itself
    /// if the number of fields is its own, otherwise in an out-of-line body (the object does not
    /// change address; the generated code follows the body, [`OUT_OF_LINE`]).
    fn swap_fields(&mut self, b: *mut Header, value_list: &[Field]) {
        self.objects.remember(b);
        // Under ARC, the migration (the JIT reload) and the added field are
        // a raw write: the snapshot counts the fields from before and those after.
        if self.arc.is_some() {
            self.arc_snapshot(b, b as i64 + HANDLE_OFFSET);
        }
        // SAFETY: live block of the space; the out-of-line body is new and of the size
        // of `value_list`.
        #[allow(unsafe_code)]
        unsafe {
            let current = body(b);
            let c = if usize::from((*current).n) == value_list.len() {
                current
            } else {
                let c = new_out_of_line_body(value_list.len());
                (*c).class_id = (*b).class_id;
                if (*b).flags & OUT_OF_LINE != 0 {
                    release_out_of_line_body(current);
                } else {
                    self.objects.with_out_of_line.push(b);
                }
                *fields_of(b).cast::<*mut Header>() = c;
                (*b).flags |= OUT_OF_LINE;
                c
            };
            for (i, &(bits, is_ref_kind)) in value_list.iter().enumerate() {
                *fields_of(c).add(i) = bits;
                mark_reference(c, i, is_ref_kind);
            }
        }
    }

    /// Guarantees object `handle` at least `n` fields (the new ones,
    /// zeroed): in an out-of-line body (`Heap::swap_fields`).
    pub fn ensure_fields(&mut self, handle: i64, n: usize) {
        let Some(o) = self.object(handle) else { return };
        if o.len() >= n {
            return;
        }
        let mut new_ones = o.to_vec();
        new_ones.resize(n, (0, false));
        let b = self.live_block(handle);
        self.swap_fields(b, &new_ones);
    }
    /// The address of the fields of object `handle` (8-byte words; the
    /// path of `dartforge_object_fields`); `None` if it is not a live object.
    /// Whoever asks for the pointer may write through it: it counts as a write
    /// barrier, but the reference map is up to the writer.
    #[inline]
    pub fn object_fields(&mut self, handle: i64) -> Option<*mut i64> {
        let b = self.space_block(handle)?;
        // SAFETY: block of the space.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).state == FREE {
                return None;
            }
            self.objects.remember(b);
            if self.arc.is_some() {
                self.arc_snapshot(b, handle);
            }
            Some(fields_of(body(b)))
        }
    }
    /// Updates a field with an explicit tag; scalar values are never roots.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        self.set_field(
            handle,
            usize::try_from(index).expect("invalid index"),
            bits,
            is_ref,
        );
    }
    /// Marks roots and typed edges iteratively and frees even unreachable cycles.
    /// Marks everything reachable from `pending`; returns how many
    /// objects it marked.
    fn mark_pending(&mut self) -> usize {
        // One copy of the marking for each kind of collection: the test of the state
        // of each object (young in the minor one, unmarked in the full one) runs without
        // branching on the kind.
        match (self.minor_collection, self.aging_now) {
            (true, true) => self.mark::<true, true>(),
            (true, false) => self.mark::<true, false>(),
            _ => self.mark::<false, false>(),
        }
    }

    /// Marks from `pending`. The minor one walks the [`YOUNG`] objects and the
    /// survivors of the previous minor collection (state `survivor_state`). With
    /// `AGING` (minor only), a reached young object takes the other survivor state
    /// (it stays young) and a reached survivor is promoted; a promoted object with a
    /// child that stays young is remembered, its cards all dirty. Without it,
    /// every reached object becomes old.
    fn mark<const MINOR: bool, const AGING: bool>(&mut self) -> usize {
        let minor = MINOR;
        let mut live = 0_usize;
        // The stack and the counters in local variables: writing to `self` for
        // each object (the `Vec` and the work) kept the compiler from
        // keeping them in registers (the writes to objects may, as far as
        // it knows, point to `self`).
        let mut stack = std::mem::take(&mut self.pending);
        let mut work = 0usize;
        let mut marked_objects = 0usize;
        // The promoted objects that point to a young one (aging).
        let mut remembered: Vec<*mut Header> = Vec::new();
        // The edges come from roots and from `is_ref` fields: the full
        // validation of the handle (the page and the block start) is left to
        // `--gc-stress`, the mode that hunts missing roots (N4).
        let validate = self.stress;
        // Marking adds no attachment: without any, the lookup per marked object
        // (a call through `dyn FnMut`, ~10% of a linked list's marking) is skipped.
        let attachments = !self.attachments_by_owner.is_empty();
        let (s_old, s_new) = (
            self.survivor_state,
            other_survivor_state(self.survivor_state),
        );
        // Does this collection walk the object in state `e`? In the minor one, only the young
        // and the survivors of the previous one (an old one counts as live, and its
        // references to young objects are among the remembered); in the full one,
        // what has not been marked yet.
        // In the full one, marked means the bit in the page's mark bitmap (the
        // sweep reads only the bits).
        // SAFETY (of the uses): `b` is a block of a page of the space.
        #[allow(unsafe_code)]
        // The static (`PERMANENT`) is checked before the mark bitmap: its
        // bitmap would fall outside a heap page (§2.8).
        let walks = |b: *const Header| unsafe {
            if minor {
                let e = (*b).state;
                e == YOUNG || e == s_old
            } else {
                (*b).state != PERMANENT && !marked(b)
            }
        };
        let objects = &self.objects;
        // With aging: does the child `bits` stay young after this collection?
        #[allow(unsafe_code)]
        let stays_young = |bits: i64| {
            let b = if validate {
                match objects.block_of(bits) {
                    Some(b) => b,
                    None => return false,
                }
            } else {
                (bits - HANDLE_OFFSET) as *mut Header
            };
            // SAFETY: block of the space.
            let e = unsafe { (*b).state };
            e == YOUNG || e == s_new
        };
        // An edge: the first object child still to be marked goes to
        // `next_offset` (followed without the stack); the others, to the stack.
        // With aging, `kid` records a child that stays young.
        #[allow(unsafe_code)]
        let visit_ref = |bits: i64, next_offset: &mut i64, stack: &mut Vec<i64>, kid: &mut bool| {
            if !smi::e_handle(bits) {
                return;
            }
            if is_object(bits) {
                if AGING && stays_young(bits) {
                    *kid = true;
                }
                if !validate && !walks((bits - HANDLE_OFFSET) as *const Header) {
                    return;
                }
                if *next_offset == 0 {
                    *next_offset = bits;
                    return;
                }
            }
            stack.push(bits);
        };
        while let Some(handle) = stack.pop() {
            // null and `Smi` (R10) are not edges: the collector never follows a
            // `Smi`, which does not point to the heap.
            if !smi::e_handle(handle) {
                continue;
            }
            if is_object(handle) {
                let mut current = handle;
                // Follows directly the first child still to be marked (the linked
                // list, the branch of a tree) without going through the stack.
                while current != 0 {
                    if validate && self.objects.block_of(current).is_none() && is_static(current) {
                        break;
                    }
                    let b = if validate {
                        self.live_block(current)
                    } else {
                        debug_assert!(
                            self.objects.block_of(current).is_some(),
                            "invalid edge {current}"
                        );
                        (current - HANDLE_OFFSET) as *mut Header
                    };
                    let mut next_offset = 0;
                    // A child that stays young (aging only).
                    let mut kid = false;
                    // SAFETY: live block of the space.
                    #[allow(unsafe_code)]
                    unsafe {
                        if !walks(b) {
                            break;
                        }
                        // Both promote already during marking (the reached young object
                        // becomes old, the remembered one becomes old again in the full one:
                        // the sweep reads only the bits) and set the bit. With aging,
                        // only the survivor is promoted; the young one stays young.
                        let was = (*b).state;
                        let promoted = !AGING || was == s_old;
                        let now = if promoted { OLD } else { s_new };
                        if was != now {
                            (*b).state = now;
                        }
                        mark_block(b);
                        live += 1;
                        marked_objects += 1;
                        if attachments {
                            self.visit_attachment_edges(current, &mut |action| {
                                visit_ref(action, &mut next_offset, &mut stack, &mut kid)
                            });
                        }
                        // The body by format (§2.8): `RAW` has no
                        // references; `REFS`, the words `1..=word 0`;
                        // `INSTANCE`, the fields the map says are references. What
                        // this collection does not walk (old in the minor one, already
                        // marked) does not even enter the stack; the first one still to
                        // mark is followed directly.
                        let ty = shape(b);
                        if ty != block_flags::INSTANCE {
                            if ty == block_flags::REFS {
                                let w = self.objects.body_words(b);
                                let n = body_refs(b, w);
                                work += n;
                                let p = fields_of(b);
                                for i in 1..=n {
                                    visit_ref(*p.add(i), &mut next_offset, &mut stack, &mut kid);
                                }
                                if !minor && (*b).flags & block_flags::CARDS != 0 {
                                    crate::space::clear_cards(b, n);
                                }
                                if AGING && promoted && kid {
                                    if (*b).flags & block_flags::CARDS != 0 {
                                        crate::space::dirty_all_cards(b, n);
                                    }
                                    (*b).state = REMEMBERED;
                                    remembered.push(b);
                                }
                            } else if AGING && promoted && kid {
                                (*b).state = REMEMBERED;
                                remembered.push(b);
                            }
                            current = next_offset;
                            continue;
                        }
                        let c = body(b);
                        let n = usize::from((*c).n);
                        work += n;
                        let fields = fields_of(c);
                        let visit = visit_ref;
                        let mut m = (*c).map;
                        while m != 0 {
                            let i = m.trailing_zeros() as usize;
                            visit(*fields.add(i), &mut next_offset, &mut stack, &mut kid);
                            m &= m - 1;
                        }
                        if n > 32 {
                            let ext = fields.add(capacity(n)).cast::<u64>();
                            for w in 0..map_words(n) {
                                let mut m = *ext.add(w);
                                while m != 0 {
                                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                                    visit(*fields.add(i), &mut next_offset, &mut stack, &mut kid);
                                    m &= m - 1;
                                }
                            }
                        }
                        if AGING && promoted && kid {
                            (*b).state = REMEMBERED;
                            remembered.push(b);
                        }
                    }
                    current = next_offset;
                }
                continue;
            }
            // A root or edge that is not an object handle: the N4 messages.
            Self::invalid_handle(handle);
        }
        self.pending = stack;
        self.marking_work += work;
        self.objects.marked_set += marked_objects;
        self.objects.remembered.append(&mut remembered);
        live
    }

    /// The strong reference positions of object `h`, by body shape, the
    /// same ones marking walks (§2.8): `REFS`, the words
    /// `1..=word 0`; `INSTANCE`, the fields of the map (and of the extended map);
    /// `RAW`, none. A static or a free block has no edges here (the
    /// static is immortal under ARC, docs/ARC-CICLOS-ESPECIFICACAO.md §19.1).
    fn ref_positions(&self, h: i64, f: &mut dyn FnMut(*mut i64)) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: block of the space; only the live ones are walked.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).state == FREE {
                return;
            }
            let ty = shape(b);
            if ty == block_flags::REFS {
                let w = self.objects.body_words(b);
                let n = body_refs(b, w);
                let p = fields_of(b);
                for i in 1..=n {
                    f(p.add(i));
                }
                return;
            }
            if ty != block_flags::INSTANCE {
                return;
            }
            let c = body(b);
            let n = usize::from((*c).n);
            let fields = fields_of(c);
            let mut m = (*c).map;
            while m != 0 {
                f(fields.add(m.trailing_zeros() as usize));
                m &= m - 1;
            }
            if n > 32 {
                let ext = fields.add(capacity(n)).cast::<u64>();
                for w in 0..map_words(n) {
                    let mut m = *ext.add(w);
                    while m != 0 {
                        f(fields.add(32 + w * 64 + m.trailing_zeros() as usize));
                        m &= m - 1;
                    }
                }
            }
        }
    }

    /// Enumerates strong occurrences of the body and of the attachments belonging to the owner.
    #[allow(unsafe_code)]
    fn visit_strong_references(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: the positions belong to the live body, without mutation during the visit.
        self.ref_positions(h, &mut |p| f(unsafe { *p }));
        self.visit_attachment_edges(h, f);
    }

    /// Visits only the owner's occurrences, preserving aliases of distinct attachments.
    fn visit_attachment_edges(&self, owner: Ref, f: &mut dyn FnMut(Ref)) {
        if self.attachments_by_owner.is_empty() {
            return;
        }
        if let Some(indices) = self.attachments_by_owner.get(&owner) {
            for &ix in indices {
                if let FinalizerAction::Dart(action) = self.attachments[ix].action {
                    f(action);
                }
            }
        }
    }

    /// Removals shift positions; rebuilds the index before the next strong walk.
    fn reindex_attachments(&mut self) {
        Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
    }

    fn reindex_attachment_list(
        attachments: &[FinalizerAttachment],
        index_by_owner: &mut crate::hash::HashMap<Ref, Vec<usize>>,
    ) {
        index_by_owner.clear();
        for (ix, a) in attachments.iter().enumerate() {
            if matches!(a.action, FinalizerAction::Dart(_)) {
                index_by_owner.entry(a.owner).or_default().push(ix);
            }
        }
    }

    /// Registers the side edge without turning the owner into a global root.
    pub(crate) fn add_attachment(&mut self, attachment: FinalizerAttachment) {
        // A native action may have no owner (0): the JavaScript handle of a
        // `JSValue` (`js_interop.rs`) is released by the value's death alone.
        if attachment.owner != 0 || matches!(attachment.action, FinalizerAction::Dart(_)) {
            self.check_live(attachment.owner);
        }
        if let FinalizerAction::Dart(action) = attachment.action {
            self.check_live(action);
            if let Some(b) = self.space_block(attachment.owner) {
                self.arc_swapped(b, attachment.owner, 0, action);
            }
            self.remember_object(attachment.owner);
            if let Some(arc) = self.arc.as_mut() {
                arc.state.mark_side_edges(attachment.owner);
            }
            self.attachments_by_owner
                .entry(attachment.owner)
                .or_default()
                .push(self.attachments.len());
        }
        if let FinalizerAction::Native(_, _, bytes) = attachment.action {
            let delta = isize::try_from(bytes).expect("externalSize exceeds the address size");
            self.count_external(delta);
        }
        self.attachments.push(attachment);
    }

    /// Cancels the still registered attachments of the key and releases their Dart edges.
    pub(crate) fn detach_finalizer(&mut self, owner: Ref, detach: Ref) {
        if owner == 0 {
            return;
        }
        let mut actions = Vec::new();
        let mut external = Vec::new();
        self.attachments.retain(|a| {
            if a.owner != owner || a.detach != detach {
                return true;
            }
            if let FinalizerAction::Dart(action) = a.action {
                actions.push(action);
            }
            if let FinalizerAction::Native(_, _, bytes) = a.action {
                external.push(bytes);
            }
            false
        });
        self.reindex_attachments();
        for action in actions {
            self.arc_raw_write(owner, action, 0);
        }
        for bytes in external {
            self.count_external(-(bytes as isize));
        }
    }

    /// Registers the `Ref` slots `slots` of the globals area that starts at
    /// `area` as roots: each collection reads the current value of every one
    /// (null and `Smi` are skipped). The generated code then stores into those
    /// slots without telling the runtime (the lean area of production).
    ///
    /// # Safety
    /// `area` has every index of `slots` and stays allocated, at the same
    /// address, until [`Heap::forget_area_roots`] for it.
    #[allow(unsafe_code)]
    pub unsafe fn register_area_roots(&mut self, area: *const i64, slots: &[u32]) {
        if slots.is_empty() || self.area_roots.iter().any(|(a, _)| *a == area) {
            return;
        }
        self.area_roots.push((area, slots.into()));
    }

    /// Drops the registration of the area that starts at `area`.
    pub fn forget_area_roots(&mut self, area: *const i64) {
        self.area_roots.retain(|(a, _)| *a != area);
    }

    /// Enters (`true`) or leaves (`false`) a region where allocations never
    /// collect. The stack overflow handler builds its error inside one
    /// (`gc_roots.rs`, `dartforge_stack_overflow`): the overflow happens in the
    /// prologue of a callee, and the emitter treats calls to functions without
    /// any collection point as non-collecting (`llvm/mod.rs`, `non_collecting`),
    /// so the caller's live references are not in its roots frame there.
    pub fn defer_collection(&mut self, enter: bool) {
        if enter {
            self.collection_deferred += 1;
        } else {
            self.collection_deferred = self.collection_deferred.saturating_sub(1);
        }
    }
    /// The full collection (`dartforge_gc_collect` and the tests): marks
    /// from the roots and sweeps everything; what survives becomes old.
    pub fn collect(&mut self) {
        self.run_collection(false);
    }

    /// Pushes the roots into `dest`: the runtime's permanent ones, the
    /// globals, the ready finalizations and the frames (the runtime's and the
    /// generated code's shadow stack).
    #[allow(unsafe_code)]
    fn root_list(&self, dest: &mut Vec<i64>) {
        for (area, slots) in &self.area_roots {
            for &slot in slots.iter() {
                // SAFETY: `register_area_roots`: the area outlives its
                // registration and has the slot.
                let value = unsafe { area.add(slot as usize).read() };
                if smi::e_handle(value) {
                    dest.push(value);
                }
            }
        }
        dest.extend(self.message_owners.values().copied());
        dest.extend(self.code_owners.keys().copied());
        dest.extend(self.enum_values.values().copied());
        dest.extend(self.tearoffs.values().copied());
        dest.extend(self.literals.values().copied());
        dest.extend(self.canonical_instances.values().copied());
        dest.extend(self.globals.values().copied());
        dest.extend(self.runtime_roots.iter().copied().filter(|&h| h != 0));
        dest.extend(self.scratch_roots.iter().copied().filter(|&h| h != 0));
        dest.extend(self.empty_storages.iter().copied().filter(|&h| h != 0));
        dest.extend(self.ready_finalizations.iter().copied());
        dest.extend(
            self.frames
                .iter()
                .flat_map(|(_, roots, _)| roots.iter().copied()),
        );
        // The real ARC: the code's tokens own what its frames hold, and a slot may
        // still keep a handle that died at its last release (the slot is not cleared
        // before a later collection point). The shadow stack is no root there.
        if self.prompt_death {
            return;
        }
        visit_frames(|h| dest.push(h));
        let from_maps = dest.len();
        visit_frames_by_maps(|h| dest.push(h));
        if checked_walk() {
            check_walk(&dest[from_maps..]);
        }
    }

    /// `DARTFORGE_GC_VERIFY=1`, after the marking of a minor collection:
    /// a full traversal from the roots, without generations, must not
    /// find an unmarked young object — it would be an old-to-young reference that
    /// the write barrier did not remember.
    ///
    /// # Panics
    /// With the first such reference.
    fn verify_minor_collection(&self) {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let mut seen = crate::hash::HashSet::default();
        let mut origin: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
        while let Some(h) = stack.pop() {
            if !smi::e_handle(h) || h < 0 || !seen.insert(h) {
                continue;
            }
            let before = stack.len();
            if is_object(h) {
                if self.objects.block_of(h).is_none() && is_static(h) {
                    continue;
                }
                let b = self.live_block(h);
                // SAFETY: live block of the space.
                #[allow(unsafe_code)]
                let state = unsafe { (*b).state };
                // Young and not walked by this marking: born after the last
                // collection, or a survivor of it (the kept ones have the other state).
                if state == YOUNG || state == self.survivor_state {
                    // The parent: cid, state, flags and the child's position in the body.
                    let parent_node = origin.get(&h).copied();
                    let desc = |x: i64| {
                        let c = self.header(x);
                        let pos = self.words(x).iter().position(|&w| w == h);
                        format!(
                            "cid {} state {} flags {:#x} n {} position {pos:?}",
                            c.class_id, c.state, c.flags, c.n
                        )
                    };
                    panic!(
                        "collector bug: reachable young object without a mark in the minor collection (missing write barrier)\nobject {h} (cid {}), reached from {parent_node:?} ({})",
                        self.header(h).class_id,
                        parent_node.map(desc).unwrap_or_default()
                    );
                }
                self.visit_strong_references(h, &mut |child| stack.push(child));
            } else {
                Self::invalid_handle(h);
            }
            for &f in &stack[before..] {
                origin.entry(f).or_insert(h);
            }
        }
    }

    /// A collection: `minor` marks and sweeps only the young (old ones count as
    /// live, and those remembered by the write barrier enter as roots);
    /// the full one, everything. In both, what survives becomes old.
    fn run_collection(&mut self, minor: bool) {
        // Pure ARC does not trace: draining decides lifetime by RC and by the
        // direct roots (docs/ARC-IMPLEMENTACAO.md §6).
        if self.arc.as_ref().is_some_and(|a| a.pure) {
            self.drain_arc(!minor);
            self.gc_policy.serviced();
            return;
        }
        check_collection_allowed();
        // Under ARC, marking is always the minor one (the young); the lifetime of old objects is
        // RC, and the requested full collection is the cycle round and the reclamation
        // (`sync_arc`).
        let full_arc = self.arc.is_some() && !minor;
        let minor = minor || self.arc.is_some();
        if let Some(a) = &self.schedule {
            a.pending.set(false);
        }
        // What the TLAB did not use becomes free again.
        self.return_tlabs();
        self.minor_collection = minor;
        // The ARC nursery ages too (a young survivor stays uncounted), except in
        // the full synchronization, whose reclamation keeps only counted objects.
        let mut aging = minor && self.aging && !full_arc;
        if aging && self.aging_pause.0 > 0 {
            self.aging_pause.0 -= 1;
            aging = false;
        }
        self.aging_now = aging;
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self.literals.len() as u64;
        self.stats.roots_scanned += self.message_owners.len() as u64;
        self.stats.roots_scanned += self.code_owners.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots, _)| roots.len() as u64)
            .sum::<u64>();
        if !minor {
            self.objects.clear_marks();
        }
        self.marking_work = 0;
        let mut pending_list = std::mem::take(&mut self.pending);
        pending_list.clear();
        self.root_list(&mut pending_list);
        if aging {
            // The remembered old objects: their references may be young. One
            // that points to a young object (which stays young after this collection)
            // stays remembered; the others become old again.
            let previous = std::mem::take(&mut self.objects.remembered);
            let mut kept = Vec::new();
            // The ARC nursery's old-to-young references, as in the minor collection
            // without aging below; promotion counts those whose target it promotes.
            let mut old_to_young = self
                .arc
                .as_mut()
                .map(|a| std::mem::take(&mut a.old_to_young));
            for &b in &previous {
                let start = pending_list.len();
                // SAFETY: remembered block, live.
                #[allow(unsafe_code)]
                let (work, mut keep) =
                    unsafe { self.objects.scan_remembered(b, &mut pending_list) };
                self.marking_work += work;
                let owner = b as i64 + HANDLE_OFFSET;
                self.visit_attachment_edges(owner, &mut |action| {
                    keep |= self.young_child(action);
                    pending_list.push(action);
                });
                if let Some(edges) = old_to_young.as_mut() {
                    for &v in &pending_list[start..] {
                        if self.young_child(v) {
                            edges.push((owner, v));
                        }
                    }
                }
                if keep {
                    kept.push(b);
                } else {
                    // SAFETY: remembered block, live.
                    #[allow(unsafe_code)]
                    unsafe {
                        (*b).state = OLD;
                    }
                }
            }
            self.objects.remembered = kept;
            if let (Some(arc), Some(edges)) = (self.arc.as_mut(), old_to_young) {
                arc.old_to_young = edges;
            }
        } else if minor {
            // The remembered old objects: their references may be young. Under the
            // ARC nursery, each strong reference from one of them to a young object
            // is an occurrence that promotion counts (`sync_arc`): it is recorded
            // here, where the card scan already reads only the dirty cards, instead
            // of walking every remembered object again (a large list per collection).
            let mut old_to_young = self
                .arc
                .as_mut()
                .map(|a| std::mem::take(&mut a.old_to_young));
            for &b in &self.objects.remembered {
                let start = pending_list.len();
                // SAFETY: remembered block, live.
                #[allow(unsafe_code)]
                let work = unsafe { self.objects.push_remembered(b, &mut pending_list) };
                self.marking_work += work;
                let owner = b as i64 + HANDLE_OFFSET;
                self.visit_attachment_edges(owner, &mut |action| pending_list.push(action));
                if let Some(edges) = old_to_young.as_mut() {
                    for &v in &pending_list[start..] {
                        if self.young_child(v) {
                            edges.push((owner, v));
                        }
                    }
                }
            }
            if let (Some(arc), Some(edges)) = (self.arc.as_mut(), old_to_young) {
                arc.old_to_young = edges;
            }
        }
        self.pending = pending_list;
        // The survivor state the marking gives the young objects it keeps (with
        // aging; without it nothing stays young, and `OLD` repeats the old test).
        let kept = if aging {
            other_survivor_state(self.survivor_state)
        } else {
            OLD
        };
        let mut live = self.mark_pending();
        // Ephemerons: the value of a live holder is reached when the
        // key is — which may make other keys live, up to the fixed point.
        if !self.ephemerons.is_empty() {
            loop {
                let objects = &self.objects;
                let marked = |h: i64| marked_in_collection(objects, minor, kept, h);
                let before = self.pending.len();
                for (&holder, &(key, value)) in &self.ephemerons {
                    if marked(holder)
                        && (marked(key) || !smi::e_handle(key))
                        && smi::e_handle(value)
                        && !marked(value)
                    {
                        self.pending.push(value);
                    }
                }
                if self.pending.len() == before {
                    break;
                }
                live += self.mark_pending();
            }
        }
        // Side tables: only the handles that survived stay (G6).
        // Verification walks the whole live heap: under `--gc-stress` (one
        // minor collection per allocation), spaced like the full ones, so the cost per
        // allocation does not grow with the heap.
        if minor
            && self.verify
            && (!self.stress
                || self.minors_since_full.is_multiple_of((self.work_in_full / WORK_PER_MINOR_UNDER_STRESS).max(1) as u64))
        {
            self.verify_minor_collection();
        }
        let promotion = if self.arc.is_some() {
            Some(self.promotion_data())
        } else {
            None
        };
        let objects = &self.objects;
        let alive = |h: &i64| marked_in_collection(objects, minor, kept, *h);
        self.weak_refs.retain(|holder, target| {
            if smi::e_handle(*target) && !alive(target) {
                *target = 0;
            }
            alive(holder)
        });
        self.ephemerons.retain(|holder, (key, value)| {
            if smi::e_handle(*key) && !alive(key) {
                *key = 0;
                *value = 0;
            }
            alive(holder)
        });
        // In the minor one, only the `late` marks written since the last collection can
        // belong to a young object (old ones do not die in it): check only those, and
        // not the whole table (one per live object with a `late` field).
        if minor {
            // With aging, the mark of an object that stays young is checked again
            // by the next minor collection.
            let lates = std::mem::take(&mut self.new_lates);
            for &key in &lates {
                if !alive(&key.0) {
                    self.late_fields_initialized.remove(&key);
                } else if aging
                    && objects.block_of(key.0).is_some_and(|b| {
                        // SAFETY: block of the space.
                        #[allow(unsafe_code)]
                        unsafe {
                            (*b).state == kept
                        }
                    })
                {
                    self.new_lates.push(key);
                }
            }
        } else {
            self.new_lates.clear();
            self.late_fields_initialized.retain(|(h, _)| alive(h));
        }
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        self.attachments.retain_mut(|a| {
            if matches!(a.action, FinalizerAction::Dart(_)) && !alive(&a.owner) {
                return false;
            }
            if matches!(a.action, FinalizerAction::Native(_, _, _))
                && smi::e_handle(a.owner)
                && !alive(&a.owner)
            {
                a.owner = 0;
            }
            if smi::e_handle(a.detach) && !alive(&a.detach) {
                a.detach = 0;
            }
            if alive(&a.value) {
                return true;
            }
            match a.action {
                FinalizerAction::Dart(action) => {
                    ready.push(action);
                    attachment_edges.push((a.owner, action));
                }
                FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
            }
            false
        });
        if let Some(arc) = self.arc.as_mut() {
            for &action in &ready {
                arc.state
                    .retain_ref(action)
                    .unwrap_or_else(|e| arc_failure(e));
            }
        }
        self.ready_finalizations.extend(ready);
        // The identity tables across ports: purged by the mark bit
        // like the others (whoever needs the constant alive keeps it in a global,
        // the getter already writes there; §2.8).
        self.permanents.retain(|h| alive(h));
        self.constants.retain(|h, _| alive(h));
        self.tearoff_code.retain(|h, _| alive(h));
        let mut finalize = Vec::new();
        self.finalizable.retain(|h, &mut pair| {
            let stays = alive(h);
            if !stays {
                finalize.push(pair);
            }
            stays
        });
        self.reindex_attachments();
        for (owner, action) in attachment_edges {
            if !promotion
                .as_ref()
                .is_some_and(|p| p.promoted.contains(&owner))
            {
                self.arc_raw_write(owner, action, 0);
            }
        }
        // The estimate: the minor one subtracts what it freed (blocks, large regions,
        // out-of-line bodies and attachments of the dead); the full one rebuilds it from the live ones, with
        // the external bytes.
        if minor {
            self.adapt_young_limit(live, self.objects.handed_out);
            let (dead_set, released_bytes) = self.objects.sweep_young(aging);
            self.stats.reclaimed += dead_set as u64;
            // Most survivors promoted: aging pauses (doubling the pause while
            // that repeats); most dead, it goes on.
            let (seen, promoted) = self.objects.survivors_seen;
            if seen >= AGING_MIN_SAMPLE && promoted * 4 >= seen * 3 {
                let pause = (self.aging_pause.1 * 2).clamp(1, AGING_MAX_PAUSE);
                self.aging_pause = (pause, pause);
            } else if seen >= AGING_MIN_SAMPLE {
                self.aging_pause.1 = 0;
            }
            self.subtract_estimated_bytes(released_bytes);
        } else {
            let (dead_set, _, object_bytes) = self.objects.sweep();
            self.stats.reclaimed += dead_set as u64;
            self.set_estimated_bytes((object_bytes as u128).saturating_add(self.external));
        }
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            #[allow(unsafe_code)]
            // SAFETY: the attachment's `NativeFinalizerFunction`, `void f(void*)`;
            // as in the VM, it runs during collection and cannot touch the heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
        self.allocations = 0;
        self.young_bytes = 0;
        self.minor_collection = false;
        self.aging_now = false;
        if aging {
            self.survivor_state = kept;
        }
        if let Some(promotion) = promotion {
            self.sync_arc(full_arc, promotion);
        } else if minor {
            self.minors_since_full += 1;
        } else {
            self.minors_since_full = 0;
            self.work_in_full = self.marking_work + live;
            self.recompute_triggers(live);
        }
        if self.memory > 1 || (self.memory == 1 && !minor) {
            eprintln!(
                "[memory] after the {} collection",
                if minor { "minor" } else { "full" }
            );
            eprint!("{}", self.memory_report());
        }
        if self.trace {
            eprintln!(
                "[gc] {} marked={live} work={} estimated={} trigger={} pages={} live={} survivor_runs={} remembered={}",
                if minor { "minor" } else { "full" },
                self.marking_work,
                self.stats.estimated_bytes,
                self.byte_threshold,
                self.objects.n_pages(),
                self.objects.live_set,
                self.objects.survivors.len(),
                self.objects.remembered.len()
            );
        }
        // Clear debt only after the collector has actually completed.
        self.gc_policy.serviced();
    }

    /// `DARTFORGE_GC_YOUNG_KB`: a fixed young limit, in KiB (at least 64).
    fn young_from_env() -> Option<usize> {
        crate::console::env_var("DARTFORGE_GC_YOUNG_KB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|kb| (kb * 1024).max(64 * 1024))
    }

    /// After a traced minor collection that reached `reached` blocks with
    /// `handed_out` allocated since the previous one: when more than one in
    /// [`YOUNG_GROWTH_SURVIVAL`] survived, the young limit doubles (up to
    /// `young_max`), so that more of what is under construction dies before the
    /// next one and fewer collections mark the same structures again. A program
    /// whose young objects die young (a server answering requests, ~0.5%
    /// survival) never grows and keeps the resident memory of the base limit. The
    /// limit does not shrink back, as in the VM.
    fn adapt_young_limit(&mut self, reached: usize, handed_out: usize) {
        if self.arc.as_ref().is_none()
            && !self.stress
            && self.young_limit < self.young_max
            && handed_out >= AGING_MIN_SAMPLE
            && reached.saturating_mul(YOUNG_GROWTH_SURVIVAL) > handed_out
        {
            self.young_limit = (self.young_limit * 2).min(self.young_max);
        }
    }

    /// The memory breakdown (`DARTFORGE_GC_MEMORY=1`): the object space, the
    /// runtime's side tables (entries and capacity bytes) and the
    /// process (resident, peak, committed system heap).
    pub fn memory_report(&self) -> String {
        use std::fmt::Write;
        fn table<K, V>(m: &crate::hash::HashMap<K, V>) -> (usize, usize) {
            (
                m.len(),
                m.capacity() * (std::mem::size_of::<(K, V)>() + 1) / 1024,
            )
        }
        fn item_set<K>(m: &crate::hash::HashSet<K>) -> (usize, usize) {
            (
                m.len(),
                m.capacity() * (std::mem::size_of::<K>() + 1) / 1024,
            )
        }
        let mut s = self.objects.memory_report();
        let literals: usize = self
            .literals
            .keys()
            .map(|k| k.capacity() * 2)
            .sum::<usize>()
            / 1024;
        let _ = writeln!(
            s,
            "[memory] tables (entries, KiB): globals {:?}, literals {:?} (+{literals} KiB of keys), constants {:?}, permanent {:?}, enum {:?}, tearoffs {:?}, tearoff code {:?}, late {:?}, weak {:?}, ephemerons {:?}, finalizable {:?}; estimated {} KiB, trigger {} KiB",
            table(&self.globals),
            table(&self.literals),
            table(&self.constants),
            item_set(&self.permanents),
            table(&self.enum_values),
            table(&self.tearoffs),
            table(&self.tearoff_code),
            item_set(&self.late_fields_initialized),
            table(&self.weak_refs),
            table(&self.ephemerons),
            table(&self.finalizable),
            self.stats.estimated_bytes / 1024,
            self.byte_threshold / 1024
        );
        let (rss, peak, system_heap) = crate::space::process_memory();
        let _ = writeln!(
            s,
            "[memory] process: resident {} KiB, peak {} KiB, committed system heap {} KiB",
            rss / 1024,
            peak / 1024,
            system_heap / 1024
        );
        s
    }

    /// The triggers of the next collection, from what survived this one.
    ///
    /// Without a ceiling, the heap grows [`GROWTH`]% (geometric). With a ceiling, the
    /// next collection comes when half of the remaining slack is spent — the
    /// hysteresis: a collection that found almost everything alive does not repeat on the
    /// next allocation (before, going past half the ceiling collected on EVERY
    /// allocation, and a program with a lot of live data stopped making progress). The minimum
    /// step guarantees progress even with the slack at its end; the ceiling itself is
    /// checked separately (`allocate`).
    fn recompute_triggers(&mut self, live_set: usize) {
        const MIN_STEP: usize = 256 * 1024;
        // [`GROWTH`]% of what survived, and no less than three quarters of the previous
        // trigger (the hysteresis: a large structure that dies and comes back — the
        // list rebuilt every round — does not make the trigger plummet and climb back
        // with a full collection at each doubling).
        // And at least two young semispaces above what survived: with
        // little live data, its double came before the minor collection's
        // trigger, and every collection was a full one (the long tree of
        // `objetos_escapam/arvores` re-marked 250 times).
        let growth = (self.stats.estimated_bytes.saturating_mul(self.growth)) / 100;
        let growth = growth
            .max(
                self.stats
                    .estimated_bytes
                    .saturating_add(2 * self.young_limit),
            )
            .max(self.byte_threshold - self.byte_threshold / 4);
        self.byte_threshold = if self.byte_limit == usize::MAX {
            growth
        } else {
            let slack = self.byte_limit.saturating_sub(self.total_bytes(0));
            growth.min(
                self.stats
                    .estimated_bytes
                    .saturating_add((slack / 2).max(MIN_STEP)),
            )
        };
        // There is no longer a count trigger for the full collection: it
        // existed because every collection swept the whole slot table, and with
        // few live objects it collected every 256 allocations (a `sort` of 100 thousand
        // `int`s with a few `_Mint`s walked the whole list every time). The
        // minor collection sweeps only the young and comes by bytes or by
        // `YOUNG_COUNT` allocations; the full one, by bytes.
        let _ = live_set;
    }

    /// Gets the counters without walking the objects or their roots.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.objects.live_set,
            reserved_slots: 0,
            permanent_roots: self.enum_values.len() + self.tearoffs.len() + self.literals.len(),
            ..self.stats
        }
    }
}

// ---------------------------------------------------------------------------
// The unified space API (docs/NATIVO-ESPACO-UNIFICADO.md §3.2): every runtime
// value is a block of the object space; the per-type views are in
// `text.rs`, `boxes.rs`, `lists.rs` and `typed_lists.rs`.
// ---------------------------------------------------------------------------

/// A value without a decided heap representation (what `TaggedValue` was) in the
/// runtime APIs (the box, if needed, is up to whoever writes into a `Ref` position).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Ref(Ref),
    Int(i64),
    Double(f64),
    Bool(bool),
}

/// The static boxes of `false` and `true` (cid 5, `RAW`, `PERMANENT`): the
/// header and the value, in the form of a block; the handle is the address + 2. The
/// generated code references them as `@dartforge_false`/`@dartforge_true`
/// (`llvm/externs.rs`). They are runtime statics (§2.11): never marked,
/// swept nor written; [`is_static`] recognizes them by address.
#[allow(unsafe_code, non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dartforge_false: [u64; 3] = [
    crate::layout::header_word(
        PERMANENT,
        crate::layout::flags::RAW,
        1,
        crate::layout::cid::BOOL,
    ),
    0,
    0,
];
#[allow(unsafe_code, non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dartforge_true: [u64; 3] = [
    crate::layout::header_word(
        PERMANENT,
        crate::layout::flags::RAW,
        1,
        crate::layout::cid::BOOL,
    ),
    0,
    1,
];

/// The ranges `[start, end)` of the static objects section (`.dfimg`) of each
/// loaded image (the executable, the SDK DLL): registered by
/// `dartforge_register_image`, which the `@df.prepare_isolate` of each image
/// calls. An object handle outside the heap pages is only accepted as a
/// static inside one of them (the validation of `--gc-stress` and of verification,
/// and the runtime's reads); statics belong to the process, shared by
/// all isolates.
static IMAGES: std::sync::RwLock<Vec<(usize, usize)>> = std::sync::RwLock::new(Vec::new());

/// Registers the statics section `[start, end)` of an image (idempotent).
pub fn register_image(start: usize, end: usize) {
    if start >= end {
        return;
    }
    let mut v = IMAGES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !v.contains(&(start, end)) {
        v.push((start, end));
    }
}

/// The ranges of the registered images, in registration order (the index of the
/// static literals, `text.rs`).
pub fn images() -> Vec<(usize, usize)> {
    IMAGES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Is `h` a static object (`PERMANENT`): one of the runtime's `bool` boxes
/// or a block of the statics section of a registered image? Never reads memory
/// outside those ranges.
#[allow(unsafe_code)]
pub fn is_static(h: Ref) -> bool {
    if !is_object(h) {
        return false;
    }
    let b = (h - HANDLE_OFFSET) as usize;
    let from_runtime = b == std::ptr::addr_of!(dartforge_false) as usize
        || b == std::ptr::addr_of!(dartforge_true) as usize;
    let in_image = from_runtime || {
        let v = IMAGES
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        v.iter()
            .any(|&(i, f)| i <= b && b + std::mem::size_of::<Header>() <= f)
    };
    // SAFETY: `b` is in a registered range (readable memory of the image) or is
    // one of the runtime's boxes.
    in_image && unsafe { (*(b as *const Header)).state } == PERMANENT
}

impl Heap {
    /// A zeroed block of `words` body words, class `cid` and `flags`
    /// (format, compact shape, cards…), state `YOUNG`; collects first if a
    /// trigger asks for it (the references the caller holds need a root). The header's `n`
    /// is `words` (saturated): for `INSTANCE`, use
    /// [`Heap::allocate_instance`]. Whoever allocates a large general `_List` sets
    /// `CARDS` (`layout::has_cards`) and reserves their words
    /// (`layout::list_words`).
    pub fn allocate(&mut self, cid: i32, words: usize, flags: u8) -> Ref {
        debug_assert!(
            flags & block_flags::FORMAT != block_flags::INSTANCE || words <= 32,
            "INSTANCIA with an extended map through `allocate`: use `allocate_instance`"
        );
        self.allocate_block(cid, words, flags, words)
    }
    /// An `INSTANCE` object of `fields` zeroed fields (today's `allocate_object`).
    pub fn allocate_instance(&mut self, cid: i32, fields: usize) -> Ref {
        self.allocate_object(i64::from(cid), fields)
    }
    /// The header of block `h` (also of a static). N4 panic for null,
    /// `Smi`, dead or invalid.
    #[inline]
    #[allow(unsafe_code)]
    pub fn header(&self, h: Ref) -> &Header {
        let b = self.live_block(h);
        // SAFETY: live block of a page of the space (or static); the
        // heap borrow keeps it (no collection with `&self`).
        unsafe { &*b }
    }
    /// The class of `h`: 1 for null, 2 for `Smi`, the header cid for an
    /// object. N4 panic for dead or invalid.
    pub fn class(&self, h: Ref) -> i32 {
        if h == 0 {
            crate::layout::cid::NULL
        } else if smi::is_smi(h) {
            crate::layout::cid::SMI
        } else if is_object(h) {
            self.header(h).class_id
        } else {
            Self::invalid_handle(h)
        }
    }
    /// Is `h` a live object of the space, or a static (without panicking)?
    #[allow(unsafe_code)]
    pub fn is_live_object(&self, h: Ref) -> bool {
        if !self.validate_handles {
            // SAFETY: `validate_handles` (an object handle is a block or a static).
            return is_object(h)
                && unsafe { (*((h - HANDLE_OFFSET) as *const Header)).state } != FREE;
        }
        match self.objects.block_of(h) {
            // SAFETY: block of a page of the space.
            Some(b) => (unsafe { (*b).state }) != FREE,
            None => is_static(h),
        }
    }
    /// Is `h` a static object (image section, `PERMANENT`)?
    #[allow(unsafe_code)]
    pub fn is_static(&self, h: Ref) -> bool {
        if !self.validate_handles {
            // SAFETY: `validate_handles` (an object handle is a block or a static).
            return is_object(h)
                && unsafe { (*((h - HANDLE_OFFSET) as *const Header)).state } == PERMANENT;
        }
        self.objects.block_of(h).is_none() && is_static(h)
    }
    /// The body words of block `b` (readable): the `INSTANCE` body (the
    /// out-of-line one, if any) and its number of words.
    #[inline]
    #[allow(unsafe_code)]
    fn body_and_words(&self, b: *mut Header) -> (*mut Header, usize) {
        // SAFETY: readable block (from `live_block`).
        unsafe {
            if shape(b) == block_flags::INSTANCE {
                let c = body(b);
                return (c, crate::layout::instance_words(usize::from((*c).n)));
            }
            if (*b).state == PERMANENT {
                let n = usize::from((*b).n);
                if n != usize::from(u16::MAX) {
                    return (b, n);
                }
                // A static literal with more than 65 534 words: the length
                // tells (strings are the only large statics).
                let c = (*b).class_id;
                assert!(
                    crate::layout::cid::is_string(c),
                    "compiler bug: large static that is not a string"
                );
                let len = usize::try_from(*fields_of(b)).unwrap_or(0);
                return (
                    b,
                    crate::layout::string_words(len, c == crate::layout::cid::TWO_BYTE_STRING),
                );
            }
            (b, self.objects.body_words(b))
        }
    }
    /// The whole body of `h`, in words (word 0 is `b+16`).
    #[inline]
    #[allow(unsafe_code)]
    pub fn words(&self, h: Ref) -> &[i64] {
        let (c, w) = self.body_and_words(self.live_block(h));
        // SAFETY: the body of the live block has `w` words; the heap borrow
        // keeps it.
        unsafe { std::slice::from_raw_parts(fields_of(c), w) }
    }
    /// The body (word 0, `b + 16`) of a block of this heap that nobody has
    /// published yet — one just allocated, or one a native is still filling —,
    /// for the caller to write in the same heap borrow, before any other
    /// allocation: the rule of [`Heap::words_mut`] without the revalidation of
    /// the handle (the object is a `RAW`/`REFS` block or an `INSTANCE` whose body
    /// is still inline). Under ARC, takes the snapshot `words_mut` would take.
    #[inline]
    pub(crate) fn fresh_body(&mut self, h: Ref) -> *mut i64 {
        debug_assert!(
            is_object(h),
            "runtime bug: fresh_body of a value that is not an object"
        );
        let b = (h - HANDLE_OFFSET) as *mut Header;
        if self.arc.is_some() {
            self.arc_snapshot(b, h);
        }
        fields_of(b)
    }
    /// The whole body of `h`, writable **without barrier**: only `RAW`, or before
    /// publishing the object (whoever writes a `Ref` into a published object calls
    /// [`Heap::remember`]/[`Heap::remember_element`]).
    #[inline]
    #[allow(unsafe_code)]
    pub fn words_mut(&mut self, h: Ref) -> &mut [i64] {
        let b = self.live_block(h);
        Self::check_writable(b);
        if self.arc.is_some() {
            self.arc_snapshot(b, h);
        }
        let (c, w) = self.body_and_words(b);
        // SAFETY: the body of the live block has `w` words; the mutable heap
        // borrow is exclusive.
        unsafe { std::slice::from_raw_parts_mut(fields_of(c), w) }
    }
    /// The whole body of `h`, in bytes.
    #[allow(unsafe_code)]
    pub fn bytes(&self, h: Ref) -> &[u8] {
        let p = self.words(h);
        // SAFETY: the same words, seen as bytes.
        unsafe { std::slice::from_raw_parts(p.as_ptr().cast(), p.len() * 8) }
    }
    /// The whole body of `h`, in writable bytes (the rules of [`Heap::words_mut`]).
    #[allow(unsafe_code)]
    pub fn bytes_mut(&mut self, h: Ref) -> &mut [u8] {
        let p = self.words_mut(h);
        // SAFETY: the same words, seen as bytes.
        unsafe { std::slice::from_raw_parts_mut(p.as_mut_ptr().cast(), p.len() * 8) }
    }
    /// Replaces the `mask` bits of `flags` with `value`: only `FORMAT`/`ELEMENT` of a
    /// `_List`/`_ImmutableList` (§2.16).
    #[allow(unsafe_code)]
    pub fn set_flags(&mut self, h: Ref, mask: u8, value: u8) {
        assert!(
            mask & !(block_flags::FORMAT | block_flags::ELEMENT) == 0,
            "runtime bug: set_flags only swaps FORMAT and ELEMENT"
        );
        let b = self.live_block(h);
        Self::check_writable(b);
        // The shape decides which words are references: ARC snapshots the
        // counted ones before the swap.
        if self.arc.is_some() {
            self.arc_snapshot(b, h);
        }
        // SAFETY: live and writable block.
        unsafe {
            assert!(
                crate::layout::cid::is_fixed_list((*b).class_id),
                "runtime bug: set_flags outside _List/_ImmutableList"
            );
            (*b).flags = ((*b).flags & !mask) | (value & mask);
        }
    }
    /// The element barrier of §2.7 after writing `v` into element `i` (index
    /// from 0, word `i + 1`) of the `REFS` block `b`.
    #[allow(unsafe_code)]
    fn element_barrier(&mut self, b: *mut Header, i: usize, v: Ref) {
        if !self.young_child(v) {
            return;
        }
        // SAFETY: live block of the space.
        unsafe {
            let e = (*b).state;
            if e != OLD && e != REMEMBERED {
                return;
            }
            if (*b).flags & block_flags::CARDS != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                if i < len {
                    crate::space::dirty_card(b, len, i);
                }
            }
        }
        self.objects.remember(b);
    }
    /// The writable `REFS` block of `h`.
    #[allow(unsafe_code)]
    fn refs_block(&self, h: Ref) -> *mut Header {
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: live block.
        assert!(
            unsafe { shape(b) } == block_flags::REFS,
            "runtime bug: write_ref on an object that is not REFS"
        );
        b
    }
    /// Writes the `Ref` `v` into word `word ≥ 1` of the `REFS` body of `h`, with
    /// barrier (and card).
    #[allow(unsafe_code)]
    pub fn write_ref(&mut self, h: Ref, word: usize, v: Ref) {
        if smi::e_handle(v) && self.stress {
            self.check_live(v);
        }
        let b = self.refs_block(h);
        let (c, w) = self.body_and_words(b);
        assert!(
            word >= 1 && word < w,
            "word {word} outside the REFS body of {w} words"
        );
        // SAFETY: `word < w`.
        let old = unsafe { std::mem::replace(&mut *fields_of(c).add(word), v) };
        if self.arc.is_some() {
            self.arc_swapped(b, h, old, v);
        }
        self.element_barrier(b, word - 1, v);
    }
    /// Copies `v` into the words `word..` of the `REFS` body of `h`, with a single
    /// barrier: in an old object with cards, dirties the cards of the whole range (the
    /// in-place decompaction and the filling in `lists.rs` rely on this)
    /// and remembers it if some value is young.
    #[allow(unsafe_code)]
    pub fn write_refs(&mut self, h: Ref, word: usize, v: &[Ref]) {
        let b = self.refs_block(h);
        let (c, w) = self.body_and_words(b);
        assert!(
            word >= 1 && word + v.len() <= w,
            "words {word}..+{} outside the REFS body of {w} words",
            v.len()
        );
        // ARC counts each swap: the words from before, one by one.
        let olds: Vec<Ref> = if self.arc.is_some() {
            // SAFETY: the range is inside the body.
            unsafe { std::slice::from_raw_parts(fields_of(c).add(word), v.len()).to_vec() }
        } else {
            Vec::new()
        };
        // SAFETY: the range is inside the body; `v` does not point into the
        // mutable heap (it is a slice of the caller).
        unsafe { std::ptr::copy(v.as_ptr(), fields_of(c).add(word), v.len()) };
        if v.is_empty() {
            return;
        }
        for (&old, &new) in olds.iter().zip(v) {
            self.arc_swapped(b, h, old, new);
        }
        // SAFETY: live block of the space.
        unsafe {
            let e = (*b).state;
            if e != OLD && e != REMEMBERED {
                return;
            }
            if (*b).flags & block_flags::CARDS != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                let (of, until) = ((word - 1).min(len), (word - 1 + v.len()).min(len));
                let mut i = of;
                while i < until {
                    crate::space::dirty_card(b, len, i);
                    i = (i / crate::layout::ELEMENTS_PER_CARD + 1)
                        * crate::layout::ELEMENTS_PER_CARD;
                }
            }
        }
        if v.iter().any(|&x| self.young_child(x)) {
            self.objects.remember(b);
        }
    }
    /// The explicit barrier (whoever wrote through [`Heap::words_mut`] into a published
    /// object): remembers the old object `h`; in a list with cards, dirties all of them.
    #[allow(unsafe_code)]
    pub fn remember(&mut self, h: Ref) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: block of the space.
        unsafe {
            let e = (*b).state;
            if (e == OLD || e == REMEMBERED)
                && shape(b) == block_flags::REFS
                && (*b).flags & block_flags::CARDS != 0
            {
                let len = body_refs(b, self.objects.body_words(b));
                crate::space::dirty_all_cards(b, len);
            }
        }
        self.objects.remember(b);
    }
    /// The explicit element barrier: remembers the old object `h` and dirties the card of
    /// element `i` (index from 0).
    #[allow(unsafe_code)]
    pub fn remember_element(&mut self, h: Ref, i: usize) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: block of the space.
        unsafe {
            let e = (*b).state;
            if e != OLD && e != REMEMBERED {
                return;
            }
            if shape(b) == block_flags::REFS && (*b).flags & block_flags::CARDS != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                if i < len {
                    crate::space::dirty_card(b, len, i);
                }
            }
        }
        self.objects.remember(b);
    }
    /// Attaches to block `h` (`ATTACHMENT`) the native pointer `ptr`, which takes `bytes`
    /// bytes outside the heap: written at `b+16`, `release(ptr)` when the owner dies
    /// (or when the heap ends). The bytes count toward the triggers.
    ///
    /// # Safety
    /// `release(ptr)` is the only release of `ptr`, and is valid; the body of `h` has
    /// at least one word and is `RAW`.
    #[allow(unsafe_code)]
    pub unsafe fn attach(
        &mut self,
        h: Ref,
        release: unsafe fn(*mut u8),
        ptr: *mut u8,
        bytes: usize,
    ) {
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: the function contract; live block with word 0.
        unsafe {
            // The block may have been born with `ATTACHMENT` (`allocate(.., RAW | ATTACHMENT)`, the
            // form of §2.4): a duplicate is a pointer already written.
            assert!(
                (*b).flags & block_flags::ATTACHMENT == 0 || *fields_of(b) == 0,
                "runtime bug: duplicate attachment"
            );
            (*b).flags |= block_flags::ATTACHMENT;
            *fields_of(b) = ptr as i64;
        }
        self.objects.attach(b, release, ptr, bytes);
        self.add_estimated_bytes(bytes);
        self.young_bytes = self.young_bytes.saturating_add(bytes);
        if bytes > 0 {
            self.gc_policy_after_growth(true);
        }
    }
    /// The bytes of the attachment of `h` become `bytes` (the `StringBuffer`
    /// accumulator grew): the difference enters the collection triggers, like
    /// [`Heap::count_external`], and leaves when the owner dies.
    pub fn adjust_attachment(&mut self, h: Ref, bytes: usize) {
        let b = self.live_block(h);
        let delta = self.objects.adjust_attachment(b, bytes);
        if delta >= 0 {
            self.add_estimated_bytes(delta.unsigned_abs());
            self.young_bytes = self.young_bytes.saturating_add(delta.unsigned_abs());
            if delta > 0 {
                self.gc_policy_after_growth(true);
            }
        } else {
            self.subtract_estimated_bytes(delta.unsigned_abs());
        }
    }
    /// The native pointer attached to `h` (null without an attachment).
    #[allow(unsafe_code)]
    pub fn attachment(&self, h: Ref) -> *mut u8 {
        let b = self.live_block(h);
        // SAFETY: live block.
        unsafe {
            if (*b).flags & block_flags::ATTACHMENT != 0 {
                *fields_of(b) as *mut u8
            } else {
                std::ptr::null_mut()
            }
        }
    }
    /// Projects the estimate without losing bytes that will be subtracted later.
    fn set_estimated_bytes(&mut self, bytes: u128) {
        self.exact_estimated_bytes = bytes;
        self.stats.estimated_bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
    }

    fn add_estimated_bytes(&mut self, bytes: usize) {
        // Gross growth, including TLAB reservations and external attachments.
        // Discounts and collector estimate recomputation do not restore budget.
        self.gc_policy.charge(bytes);
        self.set_estimated_bytes(self.exact_estimated_bytes.saturating_add(bytes as u128));
    }

    fn subtract_estimated_bytes(&mut self, bytes: usize) {
        self.set_estimated_bytes(self.exact_estimated_bytes.saturating_sub(bytes as u128));
    }

    /// Adds `delta` to the bytes outside the heap (they count toward the triggers until whoever
    /// counted them subtracts them).
    pub fn count_external(&mut self, delta: isize) {
        if delta >= 0 {
            let d = delta.unsigned_abs();
            self.external = self.external.saturating_add(d as u128);
            self.add_estimated_bytes(d);
            self.young_bytes = self.young_bytes.saturating_add(d);
            if d > 0 {
                self.gc_policy_after_growth(true);
            }
        } else {
            let d = delta.unsigned_abs();
            self.external = self.external.saturating_sub(d as u128);
            self.subtract_estimated_bytes(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Enums preserve identity and names without external frames, even under stress.
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
        let fields = heap.object(first).expect("enum must be an object");
        assert_eq!(fields.class_id, 1);
        assert_eq!(fields.field(0), (0, false));
        assert!(
            heap.text(fields.field(1).0)
                .is_some_and(|name| name == "red")
        );
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// A single root object transitively keeps cycles and strings of its fields.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.new_object(1, &[(0, true)]);
        heap.root(outer, parent);
        let inner = heap.push_frame();
        let text = heap.allocate_str("ação 🦀");
        heap.root(inner, text);
        heap.set(parent, 0, text, true);
        heap.pop_frame(inner);
        heap.collect();
        assert!(heap.text(text).is_some_and(|s| s == "ação 🦀"));
        heap.set(parent, 0, 0, true);
        heap.collect();
        assert!(!heap.is_live_object(text));
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }
    /// Cycles survive through a root, then are collected and their slots reused.
    #[test]
    fn cycles_and_slot_reuse() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.new_object(1, &[(0, true)]);
        heap.root(frame, a);
        let b = heap.new_object(2, &[(a, true)]);
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // The blocks of the dead go back to the free list of their size.
        let c = heap.new_object(3, &[(0, false)]);
        assert!(c == a || c == b);
        assert_eq!(heap.stats().live_objects, 1);
    }
    /// Scalar bits that coincide with handles do not retain objects.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.allocate_str("disposable");
        let object = heap.new_object(1, &[(text, false)]);
        heap.root(frame, object);
        heap.collect();
        assert!(!heap.is_live_object(text));
        assert!(heap.is_live_object(object));
    }
    /// Nested frames preserve arguments; stress collects during thousands of allocations.
    #[test]
    fn nested_frames_stress() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let permanent = heap.allocate_str("alive");
        heap.root(outer, permanent);
        for _ in 0..1000 {
            let inner = heap.push_frame();
            let temporary = heap.allocate_str("temporary");
            heap.root(inner, temporary);
            heap.collect();
            assert!(heap.text(permanent).is_some_and(|s| s == "alive"));
            heap.pop_frame(inner);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 1);
    }
}

#[cfg(test)]
mod object_space {
    //! The object space: blocks by number of fields, handles with bit
    //! 1, free lists rebuilt during the sweep and freed pages.
    use super::*;

    #[test]
    fn object_handles_and_scalars_are_distinct() {
        let mut heap = Heap::new(false);
        let o = heap.allocate_object(7, 3);
        let s = heap.allocate_str("x");
        assert!(is_object(o) && is_object(s));
        assert!(matches!(heap.object(o), Some(f) if f.class_id == 7 && f.len() == 3));
        // The string is not `INSTANCE`: it has no field view.
        assert!(heap.object(s).is_none());
        // Arbitrary scalars are not live objects (nor do they crash the lookup).
        for x in [2, 6, 18, o + 4, o - 4, o + 8, -2, i64::MAX - 1] {
            assert!(!heap.is_live_object(x) || x == o || x == s, "{x}");
        }
        heap.set(o, 1, s, true);
        assert_eq!(heap.object(o).expect("object").field(1), (s, true));
    }

    #[test]
    fn collection_returns_blocks_and_keeps_reachable() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        // A reachable linked list of 10 thousand nodes and 10 thousand dead ones.
        let mut head = 0;
        for i in 0..10_000 {
            let garbage = heap.allocate_object(1, 2);
            heap.set(garbage, 0, i, false);
            let no = heap.allocate_object(2, 2);
            heap.set(no, 0, i, false);
            heap.set(no, 1, head, true);
            head = no;
            heap.set_root(frame, 0, head);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 10_000);
        let mut total_sum = 0;
        let mut no = head;
        while no != 0 {
            let fields = heap.object(no).expect("object");
            total_sum += fields.field(0).0;
            no = fields.field(1).0;
        }
        assert_eq!(total_sum, (0..10_000).sum::<i64>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // The pages stay while the recent peak of live objects asks for them (the
        // structure may come back); the peak decays on each full collection, and the
        // empty ones beyond the slack go back to the system.
        let retained_list = heap.objects.pages.len();
        assert!(retained_list > 2);
        for _ in 0..20 {
            heap.collect();
        }
        assert!(
            heap.objects.pages.len() <= 2,
            "{} pages for {retained_list}",
            heap.objects.pages.len()
        );
    }

    #[test]
    fn growing_object_followed_through_field_pointer() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let o = heap.allocate_object(3, 1);
        heap.root(frame, o);
        heap.set(o, 0, 41, false);
        heap.ensure_fields(o, 5);
        heap.set(o, 4, 42, false);
        heap.collect();
        let fields = heap.object(o).expect("object");
        assert_eq!(
            (fields.len(), fields.field(0).0, fields.field(4).0),
            (5, 41, 42)
        );
        heap.pop_frame(frame);
        heap.collect();
        // The block went back to the template: a new 1-field object reuses it zeroed.
        let new = heap.allocate_object(4, 1);
        assert_eq!(new, o);
        assert!(
            matches!(heap.object(new), Some(f) if f.class_id == 4 && f.len() == 1 && f.field(0) == (0, false))
        );
    }

    #[test]
    fn identity_hash_is_stable_and_distinct() {
        let mut heap = Heap::new(false);
        let a = heap.allocate_object(1, 0);
        let b = heap.allocate_object(1, 0);
        let ha = heap.identity_hash(a).expect("object");
        assert_eq!(heap.identity_hash(a), Some(ha));
        assert_ne!(heap.identity_hash(b), Some(ha));
        assert!(ha > 0 && ha <= 0x3fff_ffff);
    }

    /// The generated code's inline allocation: advances the cursor of the TLAB
    /// range and writes the header (`llvm/mod.rs`).
    #[allow(unsafe_code)]
    pub(super) fn allocate_like_generated_code(heap: &mut Heap, class_id: i64, n: usize) -> i64 {
        // The TLAB is by body words (`n ≤ TLAB_N` fields: `max(n, 1)`).
        let w = crate::layout::instance_words(n);
        let size = block_size(n);
        let b = CONTEXT.with(|c| {
            let (cursor, end) = (c.tlab[w][0].get(), c.tlab[w][1].get());
            if (end as usize).saturating_sub(cursor as usize) < size {
                return None;
            }
            c.tlab[w][0].set(cursor.wrapping_add(size));
            Some(cursor)
        });
        let Some(b) = b else {
            let h = heap.allocate_object(class_id, n);
            heap.refill_tlab(w);
            return h;
        };
        // SAFETY: free block of the range, zeroed by the handout (the generated
        // code writes only the header word).
        unsafe {
            *b.cast::<u64>() = 1 | (n as u64) << 16 | (class_id as u64) << 32;
        }
        b as i64 + HANDLE_OFFSET
    }

    /// Does the TLAB range of `words` body words have blocks left?
    fn tlab_has_room(words: usize) -> bool {
        CONTEXT.with(|c| c.tlab[words][0].get() != c.tlab[words][1].get())
    }

    /// The runtime's allocations (`Heap::allocate`, `new_string`,
    /// `allocate_object`) come from the same TLAB ranges as the generated code's:
    /// the first allocation of a size fills the range, the following ones bump
    /// it, and the objects are young like any other (kept by a root, swept with
    /// the range).
    #[test]
    fn runtime_allocations_bump_the_tlab_ranges() {
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            let frame = heap.push_frame_with_slots(1);
            let w = crate::layout::string_words(6, false);
            assert!(!tlab_has_room(w));
            let first = heap.allocate_str("first!");
            // The general path filled the range: the next block of the size is the
            // one at the cursor.
            assert!(tlab_has_room(w));
            let cursor = CONTEXT.with(|c| c.tlab[w][0].get());
            let second = heap.allocate_str("second");
            assert_eq!(second - HANDLE_OFFSET, cursor as i64);
            assert_eq!(
                CONTEXT.with(|c| c.tlab[w][0].get()),
                cursor.wrapping_add(block_bytes(w))
            );
            assert_eq!(heap.text(second).unwrap(), "second");
            let hd = *heap.header(second);
            assert_eq!(
                (
                    hd.state,
                    hd.flags,
                    usize::from(hd.n),
                    hd.class_id,
                    hd.map,
                    hd.metadata
                ),
                (
                    YOUNG,
                    crate::layout::flags::RAW,
                    w,
                    crate::layout::cid::ONE_BYTE_STRING,
                    0,
                    0
                )
            );
            // An object of two fields comes from the same range (the size class
            // is by body words), with the field count in the header.
            let o = heap.allocate_object(5, 2);
            assert_eq!(
                o - HANDLE_OFFSET,
                cursor.wrapping_add(block_bytes(w)) as i64
            );
            let hd = *heap.header(o);
            assert_eq!(
                (hd.state, hd.flags, usize::from(hd.n), hd.class_id),
                (YOUNG, crate::layout::flags::INSTANCE, 2, 5)
            );
            heap.set(o, 1, second, true);
            heap.set_root(frame, 0, o);
            let before = heap.stats().collections;
            heap.run_collection(true);
            assert_eq!(heap.stats().collections, before + 1);
            assert!(heap.is_live_object(o) && heap.is_live_object(second));
            assert!(
                !heap.is_live_object(first),
                "unrooted string of the range survived"
            );
            assert_eq!(heap.text(second).unwrap(), "second");
            assert_eq!(
                heap.object(o).expect("live object").field(1),
                (second, true)
            );
            heap.set_root(frame, 0, 0);
            heap.run_collection(false);
            assert_eq!(heap.stats().live_objects, 0);
        })
        .join()
        .unwrap();
    }

    /// A heap published later on the same thread (the tests swap the thread
    /// heap) starts with empty ranges and, when dropped, empties the ones it
    /// filled; a test heap never touches them: no heap hands out blocks of
    /// another object space.
    #[test]
    fn a_published_heap_never_hands_out_another_heap_ranges() {
        std::thread::spawn(|| {
            let mut first = Heap::for_isolate(false);
            first.allocate_object(5, 2);
            assert!(tlab_has_room(2));
            let mut second = Heap::for_isolate(false);
            assert!(!tlab_has_room(2));
            let h = second.allocate_object(5, 2);
            assert!(tlab_has_room(2));
            assert!(second.objects.block_of(h).is_some() && first.objects.block_of(h).is_none());
            let h2 = second.allocate_object(5, 2);
            assert!(second.objects.block_of(h2).is_some() && first.objects.block_of(h2).is_none());
            drop(second);
            assert!(!tlab_has_room(2));
            let h3 = first.allocate_object(5, 2);
            assert!(first.objects.block_of(h3).is_some());
            // The range `first` lost is swept with the others.
            first.run_collection(true);
            assert_eq!(first.stats().live_objects, 0);
            first.allocate_object(5, 2);
            assert!(tlab_has_room(2));
            let cursor = CONTEXT.with(|c| c.tlab[2][0].get());
            let mut test_heap = Heap::new(false);
            let t = test_heap.allocate_object(5, 2);
            assert_eq!(CONTEXT.with(|c| c.tlab[2][0].get()), cursor);
            assert!(test_heap.objects.block_of(t).is_some() && first.objects.block_of(t).is_none());
        })
        .join()
        .unwrap();
    }

    /// The strings the runtime builds through the TLAB keep their text and
    /// canonical form: one byte when every unit fits, two bytes otherwise; pieces
    /// of every length (the small copies by word moves, the large ones by
    /// `memcpy`).
    #[test]
    fn strings_built_through_the_tlab_keep_text_and_form() {
        use crate::layout::cid::{ONE_BYTE_STRING, TWO_BYTE_STRING};
        use crate::text::TextRef;
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            let frame = heap.push_frame_with_slots(4);
            let ascii: String = (0..100u8).map(|i| char::from(b'a' + i % 26)).collect();
            for n in 0..=100 {
                let s = heap.allocate_str(&ascii[..n]);
                assert_eq!(heap.text(s).unwrap(), &ascii[..n]);
                assert_eq!(heap.header(s).class_id, ONE_BYTE_STRING);
                heap.set_root(frame, 0, s);
                let (a, b) = (n / 3, n - n / 4);
                let t = heap.string_slice(s, a, b);
                assert_eq!(heap.text(t).unwrap(), &ascii[a..b]);
                heap.set_root(frame, 1, t);
                let j = heap.join_strings(&[s, t, s]);
                assert_eq!(
                    heap.text(j).unwrap().to_dart_string(),
                    format!("{}{}{}", &ascii[..n], &ascii[a..b], &ascii[..n])
                );
                assert_eq!(heap.header(j).class_id, ONE_BYTE_STRING);
            }
            // Two-byte text: the form follows the units, and a piece that fits
            // goes back to one byte.
            let units: Vec<u16> = "héllo wörld — ✓ ok".encode_utf16().collect();
            let s = heap.allocate_string(TextRef::TwoByte(&units));
            assert_eq!(heap.header(s).class_id, TWO_BYTE_STRING);
            assert_eq!(heap.text(s).unwrap(), TextRef::TwoByte(&units));
            heap.set_root(frame, 2, s);
            let latin = heap.string_slice(s, 0, 5);
            assert_eq!(heap.header(latin).class_id, ONE_BYTE_STRING);
            assert_eq!(heap.text(latin).unwrap().to_dart_string(), "héllo");
            heap.set_root(frame, 3, latin);
            let mixed = heap.join_strings(&[latin, s, latin]);
            assert_eq!(heap.header(mixed).class_id, TWO_BYTE_STRING);
            assert_eq!(
                heap.text(mixed).unwrap().to_dart_string(),
                "héllohéllo wörld — ✓ okhéllo"
            );
            let tail = heap.string_slice(mixed, 5, 5 + units.len());
            assert_eq!(heap.header(tail).class_id, TWO_BYTE_STRING);
            assert_eq!(heap.text(tail).unwrap(), TextRef::TwoByte(&units));
            heap.pop_frame(frame);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn tlab_ranges_survive_and_return_to_list() {
        // On a new thread: its `Context` starts without a TLAB.
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            let frame = heap.push_frame_with_slots(1);
            let mut head = 0;
            let mut garbage = Vec::new();
            for i in 0..10_000 {
                let link = allocate_like_generated_code(&mut heap, 5, 2);
                heap.set(link, 0, smi::of(i).unwrap(), false);
                heap.set(link, 1, head, true);
                if i % 3 == 0 {
                    head = link;
                    heap.set_root(frame, 0, head);
                } else {
                    garbage.push(link);
                }
                if i % 2_500 == 1_249 {
                    heap.run_collection(i % 5_000 != 1_249);
                }
            }
            heap.run_collection(true);
            let mut n = 0;
            let mut h = head;
            while h != 0 {
                let o = heap.object(h).expect("live link");
                assert_eq!(o.class_id, 5);
                n += 1;
                h = o.field(1).0;
            }
            assert_eq!(n, 3_334);
            assert_eq!(heap.stats().live_objects, 3_334);
            // The dead went back to the list: allocating again does not grow the heap.
            let pages = heap.objects.pages.len();
            for _ in 0..2_000 {
                allocate_like_generated_code(&mut heap, 6, 2);
            }
            assert_eq!(heap.objects.pages.len(), pages);
            heap.set_root(frame, 0, 0);
            heap.run_collection(false);
            assert_eq!(heap.stats().live_objects, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn sweep_without_touching_dead_hands_out_zeroed_blocks() {
        // The path outside tests: sweeps only read the mark
        // bitmap, and handing out zeroes the dirty range.
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            heap.objects.clear_dead = false;
            let frame = heap.push_frame_with_slots(1);
            let mut head = 0;
            for round in 0..6 {
                for i in 0..20_000 {
                    let link = allocate_like_generated_code(&mut heap, 5, 3);
                    // The generated code writes only the header word: every field
                    // (the first one included) and the reference map must come
                    // zeroed from the handout.
                    let o = heap.object(link).expect("freshly allocated");
                    assert_eq!(
                        (o.field(0), o.field(1), o.field(2)),
                        ((0, false), (0, false), (0, false)),
                        "dirty block handed out"
                    );
                    heap.set(link, 0, smi::of(i).unwrap(), false);
                    heap.set(link, 1, head, true);
                    heap.set(link, 2, 77, false);
                    if i % 4 == 0 {
                        head = link;
                        heap.set_root(frame, 0, head);
                    }
                    if i % 3_000 == 2_999 {
                        heap.run_collection(i % 9_000 != 8_999);
                    }
                }
                // The whole list survives; on each round, the previous one dies.
                let mut n = 0;
                let mut h = head;
                while h != 0 {
                    let o = heap.object(h).expect("live link");
                    assert_eq!((o.class_id, o.field(2).0), (5, 77));
                    n += 1;
                    h = o.field(1).0;
                }
                assert_eq!(n, 5_000 * (round % 2 + 1));
                if round % 2 == 1 {
                    head = 0;
                    heap.set_root(frame, 0, 0);
                    heap.run_collection(false);
                    assert_eq!(heap.stats().live_objects, 0);
                }
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn minor_collection_follows_remembered_and_releases_dead_young() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, old_gen);
        heap.run_collection(false);
        // Young object only reachable through the old one (the `set` barrier remembers it).
        let young = heap.allocate_object(2, 1);
        heap.set(young, 0, 7, false);
        heap.set(old_gen, 0, young, true);
        let garbage = heap.allocate_object(3, 1);
        heap.run_collection(true);
        assert!(matches!(heap.object(young), Some(f) if f.class_id == 2 && f.field(0).0 == 7));
        assert!(!heap.is_live_object(garbage));
        assert_eq!(heap.stats().live_objects, 2);
        // After it, the young object became old: without a new barrier, another minor collection
        // does not walk it, and it stays alive.
        heap.run_collection(true);
        assert!(heap.is_live_object(young));
        // The full collection frees what only dead old objects reached.
        heap.set_root(frame, 0, 0);
        heap.run_collection(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    #[should_panic(expected = "missing write barrier")]
    fn verification_finds_missing_barrier() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, old_gen);
        heap.run_collection(false);
        let young = heap.allocate_object(2, 0);
        // Writes without barrier (as generated code without it would).
        let b = heap.objects.block_of(old_gen).expect("object");
        #[allow(unsafe_code)]
        // SAFETY: live 1-field block.
        unsafe {
            *fields_of(b) = young;
            mark_reference(b, 0, true);
        }
        heap.run_collection(true);
    }

    #[test]
    #[should_panic(expected = "missing write barrier")]
    fn verification_finds_missing_barrier_in_side_attachment() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let owner = heap.allocate_object(1, 0);
        heap.set_root(frame, 0, owner);
        heap.run_collection(false);
        let action = heap.allocate_object(2, 0);
        // Sabotages the publication: does not call the barrier of add_attachment.
        heap.attachments.push(FinalizerAttachment {
            owner,
            value: owner,
            detach: 0,
            action: FinalizerAction::Dart(action),
        });
        heap.reindex_attachments();
        heap.run_collection(true);
    }

    #[test]
    fn verification_accepts_side_attachment_barrier() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let owner = heap.allocate_object(1, 0);
        heap.set_root(frame, 0, owner);
        heap.run_collection(false);
        let action = heap.allocate_object(2, 0);
        heap.add_attachment(FinalizerAttachment {
            owner,
            value: owner,
            detach: 0,
            action: FinalizerAction::Dart(action),
        });
        heap.run_collection(true);
        assert!(heap.is_live_object(action));
        heap.pop_frame(frame);
        heap.collect();
        assert!(!heap.is_live_object(action));
    }

    /// The workload of `bench/performance/escaping_objects.dart` (`lista_ligada`)
    /// directly on the heap, to measure the collector without compiling Dart:
    /// `cargo test --release -p dartforge-runtime linked_list_on_heap --
    /// --ignored --nocapture` (with `DARTFORGE_GC_TRACE=1`, the collections).
    #[test]
    #[ignore = "measurement"]
    fn linked_list_in_heap() {
        let mut heap = Heap::new(false);
        heap.trace = crate::console::env_var("DARTFORGE_GC_TRACE").as_deref() == Ok("1");
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..6 {
            let t = crate::clock::Instant::now();
            let mut cab = 0;
            for i in 0..1_000_000 {
                let no = heap.allocate_object(1, 2);
                heap.set(no, 0, i, false);
                heap.set(no, 1, cab, true);
                cab = no;
                heap.set_root(frame, 0, cab);
            }
            eprintln!(
                "round: {:?} collections={}",
                t.elapsed(),
                heap.stats().collections
            );
        }
    }

    #[test]
    fn reference_map_beyond_32_fields() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_object(1, 40);
        heap.set_root(frame, 0, o);
        let mut children = Vec::new();
        for i in [0usize, 31, 32, 39] {
            let f = heap.allocate_object(2, 0);
            heap.set(o, i as i64, f, true);
            children.push(f);
        }
        // A scalar that looks like a handle holds nothing.
        let garbage = heap.allocate_object(3, 0);
        heap.set(o, 33, garbage, false);
        heap.run_collection(false);
        for f in &children {
            assert!(heap.object(*f).is_some());
        }
        assert!(heap.object(garbage).is_none());
        let obj = heap.object(o).expect("object");
        assert_eq!(obj.field(32), (children[2], true));
        assert!(!obj.field(33).1);
        // Replacing the reference with a scalar clears the bit.
        heap.set(o, 39, 5, false);
        heap.run_collection(false);
        assert!(heap.object(children[3]).is_none());
    }

    #[test]
    fn out_of_line_body_followed_by_collection_and_barrier() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, o);
        heap.run_collection(false);
        // The old object grows (out-of-line body) and receives a young one in the new
        // field: the minor collection finds it through the remembered set.
        heap.ensure_fields(o, 3);
        let young = heap.allocate_object(2, 0);
        heap.set(o, 2, young, true);
        heap.run_collection(true);
        assert!(heap.object(young).is_some());
        assert_eq!(heap.object(o).expect("object").len(), 3);
        heap.pop_frame(frame);
        heap.run_collection(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn large_object_has_own_page() {
        // Above the largest medium class (`layout::LARGEST_MEDIUM` words): a
        // large region of its own, returned in the full collection.
        let mut heap = Heap::new(false);
        let n = crate::layout::LARGEST_MEDIUM + 100;
        let o = heap.allocate_object(9, n);
        heap.set(o, (n - 1) as i64, 5, false);
        assert!(matches!(heap.object(o), Some(f) if f.field(n - 1).0 == 5));
        heap.collect();
        assert!(!heap.is_live_object(o));
        assert_eq!(heap.objects.n_pages(), 0);
    }
}

#[cfg(test)]
mod handle_failures {
    //! N4: each broken contract has its own message.
    use super::*;

    #[test]
    #[should_panic(expected = "null handle (0) dereferenced")]
    fn null_dereferenced() {
        Heap::new(false).header(0);
    }

    #[test]
    #[should_panic(expected = "negative handle")]
    fn negative_handle() {
        Heap::new(false).header(-2);
    }

    #[test]
    #[should_panic(expected = "handle beyond the table")]
    fn handle_beyond_table() {
        let mut heap = Heap::new(false);
        heap.allocate_str("x");
        heap.header(42);
    }

    #[test]
    #[should_panic(expected = "handle already collected")]
    fn collected_handle() {
        let mut heap = Heap::new(false);
        let h = heap.allocate_str("x");
        heap.collect();
        heap.header(h);
    }
}

#[cfg(test)]
mod runtime_roots {
    //! G6: roots that do not live in a frame, and the side tables.
    use super::*;

    #[test]
    fn pending_exception_survives_collection_without_frame() {
        let mut heap = Heap::new(true);
        let error = heap.allocate_str("failed");
        heap.set_runtime_root(0, error);
        heap.allocate_str("another");
        heap.collect();
        assert!(heap.text(error).is_some_and(|s| s == "failed"));
        heap.set_runtime_root(0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn collection_without_open_frame() {
        // The old gate only collected with a frame; now the threshold suffices (the
        // minor collection, by allocation count).
        let mut heap = Heap::new(false);
        let n = YOUNG_COUNT + 1000;
        for _ in 0..n {
            heap.allocate_str("lixo");
        }
        assert!(heap.stats().collections > 0);
        assert!(heap.stats().live_objects < n);
    }

    #[test]
    fn side_table_purged_when_block_is_reused() {
        let mut heap = Heap::new(false);
        let o = heap.allocate_object(200, 1);
        heap.mark_late((o, 0));
        heap.mark_late((o, -1));
        heap.mark_late((o, -2));
        heap.collect();
        assert!(heap.late_fields_initialized.is_empty());
        let new_area = heap.allocate_object(200, 1);
        assert!(!heap.late_initialized((new_area, 0)));
        assert!(!heap.late_initialized((new_area, -1)));
        assert!(!heap.late_initialized((new_area, -2)));
    }

    #[test]
    fn minor_collection_purges_only_dead_young_marks() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(200, 0);
        heap.set_root(frame, 0, old_gen);
        heap.mark_late((old_gen, 0));
        heap.run_collection(true);
        assert!(
            heap.late_initialized((old_gen, 0)),
            "the live old object stays marked"
        );
        let young = heap.allocate_object(200, 0);
        heap.mark_late((young, 3));
        heap.run_collection(true);
        assert!(
            !heap.late_initialized((young, 3)),
            "the dead young object loses the mark"
        );
        assert!(heap.late_initialized((old_gen, 0)));
        heap.pop_frame(frame);
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    /// Equality preserves Unicode, embedded NUL and nullability without implicit normalization.
    #[test]
    fn unicode_nul_nullable_and_concat() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate_str("á\0🦀");
        heap.root(frame, a);
        let b = heap.allocate_str("á\0🦀");
        heap.root(frame, b);
        assert_ne!(a, b);
        assert!(heap.strings_equal(a, b));
        let c = heap.allocate_str("a\u{301}\0🦀");
        heap.root(frame, c);
        assert!(!heap.strings_equal(a, c));
    }
    /// A return transfers the root without collection between pop and registration in the caller.
    #[test]
    fn returned_handles_and_identity() {
        let mut heap = Heap::new(true);
        let caller = heap.push_frame();
        let callee = heap.push_frame();
        let a = heap.new_object(7, &[]);
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.new_object(7, &[]);
        heap.root(caller, b);
        assert_ne!(a, b);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }
    /// Tracing and destroying a large cycle do not depend on the call stack.
    #[test]
    fn large_cycle_is_iterative() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let first = heap.new_object(1, &[(0, true)]);
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.new_object(1, &[(first, true)]);
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
    /// Reproducible microbenchmark: times include roots and declared automatic collections.
    #[test]
    #[ignore = "microbenchmark; run in release with --ignored --nocapture"]
    fn gc_microbenchmark() {
        for (count, stress) in [(100_000, false), (2_000, true)] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame();
            let started = crate::clock::Instant::now();
            for index in 0..count {
                let handle = heap.new_object(1, &[(index, false)]);
                heap.root(frame, handle);
                std::hint::black_box(handle);
            }
            let allocation_ns = started.elapsed().as_nanos();
            let started = crate::clock::Instant::now();
            heap.collect();
            let collect_live_ns = started.elapsed().as_nanos();
            heap.pop_frame(frame);
            let started = crate::clock::Instant::now();
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
    /// Replaced slots do not grow with iterations; local copies have independent roots.
    #[test]
    fn fixed_slots_preserve_copies_and_nested_returns() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let kept = heap.allocate_str("keep");
        heap.set_root(frame, 0, kept);
        heap.set_root(frame, 1, kept);
        for _ in 0..1000 {
            let inner = heap.push_frame_with_slots(1);
            let temporary = heap.allocate_str("next");
            heap.set_root(inner, 0, temporary);
            heap.pop_frame(inner);
            heap.set_root(frame, 0, temporary);
            heap.collect();
            assert!(heap.text(kept).is_some_and(|s| s == "keep"));
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
    /// A large payload triggers a collection before the object count threshold.
    #[test]
    fn byte_trigger_collects_large_payloads() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..8 {
            let handle = heap.allocate(
                crate::layout::cid::typed(1),
                2 * 1024 * 1024 / 8,
                crate::layout::flags::RAW,
            );
            heap.set_root(frame, 0, handle);
        }
        let stats = heap.stats();
        assert!(stats.collections >= 3);
        assert!(stats.reclaimed >= 5);
        // The peak: the young data between minor collections and the old garbage (each
        // rooted payload becomes old and dies on the next round) until the full one,
        // which comes two young semispaces above what survived.
        assert!(
            stats.peak_estimated_bytes < 2 * YOUNG_LIMIT + 3 * 2 * 1024 * 1024,
            "{}",
            stats.peak_estimated_bytes
        );
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
    /// Measures transient allocations with a single overwritten root, without claiming superiority.
    #[test]
    #[ignore = "fixed slots microbenchmark; run release --ignored --nocapture"]
    fn fixed_slot_microbenchmark() {
        for stress in [false, true] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame_with_slots(1);
            let start = crate::clock::Instant::now();
            for _ in 0..100_000 {
                let handle = heap.allocate_str("temporary string");
                heap.set_root(frame, 0, handle);
            }
            let allocation_ns = start.elapsed().as_nanos();
            let start = crate::clock::Instant::now();
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
/// The process's object layout epoch (J03): grows with each migration
/// that a JIT reload defines (`dartforge_set_migration`).
pub static LAYOUT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// `DARTFORGE_EFFECTS=check` (`effects_check.rs`): the externs running
    /// on this thread that the effects table marks `collects = 0`. With
    /// any of them, a collection is the wrong mark — the defect of a wrong roots
    /// map, only on our side — and ends the process saying which.
    static COLLECTION_FORBIDDEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Forbids collection until the matching [`allow_collection`]: the extern `name` is
/// marked `collects = 0`.
pub fn forbid_collection(name: String) {
    COLLECTION_FORBIDDEN.with(|p| p.borrow_mut().push(name));
}

/// Undoes the last [`forbid_collection`].
pub fn allow_collection() {
    COLLECTION_FORBIDDEN.with(|p| {
        p.borrow_mut().pop();
    });
}

/// Ends the process if an extern marked `collects = 0` is running.
fn check_collection_allowed() {
    let culprit = COLLECTION_FORBIDDEN.with(|p| p.borrow().last().cloned());
    if let Some(name) = culprit {
        eprintln!(
            "dartforge: the extern {name} is marked collects = 0 in effects.tsv and collected"
        );
        std::process::abort();
    }
}

#[cfg(test)]
mod unified_space {
    //! The unified space collector (docs/NATIVO-ESPACO-UNIFICADO.md §2.3–§2.8,
    //! §4.2 step 9): one test per format, cards, attachments, large regions,
    //! statics, the purge of the identity tables and external bytes. All
    //! with the minor collection verification enabled.
    use super::*;
    use crate::layout::{self, cid, flags};

    /// A test heap with the minor collection verification (the full traversal
    /// that reports a missing barrier).
    fn heap() -> Heap {
        let mut h = Heap::new(false);
        h.verify = true;
        h
    }

    /// A registered `Ref` slot of a globals area keeps its object alive and
    /// follows a later store into the slot; null and `Smi` are skipped; a
    /// forgotten area roots nothing.
    #[test]
    #[allow(unsafe_code)]
    fn area_root_slots_are_read_at_each_collection() {
        let mut heap = heap();
        let first = list(&mut heap, 1);
        let second = list(&mut heap, 2);
        let mut area: Vec<i64> = vec![first, 7 << 1 | 1, 0];
        // SAFETY: `area` lives until the end of the test and has the slots.
        unsafe { heap.register_area_roots(area.as_ptr(), &[0, 1, 2]) };
        heap.collect();
        assert!(heap.is_live_object(first));
        assert!(!heap.is_live_object(second));
        let third = list(&mut heap, 3);
        area[0] = third;
        heap.collect();
        assert!(heap.is_live_object(third));
        assert!(!heap.is_live_object(first));
        heap.forget_area_roots(area.as_ptr());
        heap.collect();
        assert!(!heap.is_live_object(third));
    }

    /// A general `_List` (`REFS`) of `len` null elements, with cards if large.
    fn list(heap: &mut Heap, len: usize) -> Ref {
        let mut f = flags::REFS;
        if layout::has_cards(len) {
            f |= flags::CARDS;
        }
        let h = heap.allocate(cid::LIST, layout::list_words(len), f);
        heap.words_mut(h)[0] = len as i64;
        h
    }

    #[test]
    fn raw_keeps_bits_and_follows_nothing() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let target = heap.allocate_object(200, 1);
        let m = heap.allocate(cid::MINT, 2, flags::RAW);
        heap.set_root(frame, 0, m);
        // Bits that look like a handle, in a BRUTO body: the collector does not follow them.
        heap.words_mut(m)[0] = i64::MAX;
        heap.words_mut(m)[1] = target;
        heap.collect();
        assert!(
            !heap.is_live_object(target),
            "BRUTO followed as a reference"
        );
        assert_eq!(heap.words(m)[0], i64::MAX);
        assert_eq!(heap.class(m), cid::MINT);
        assert_eq!(heap.header(m).n, 2);
        heap.pop_frame(frame);
    }

    #[test]
    fn refs_follows_elements_and_barrier_remembers() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let l = list(&mut heap, 3);
        heap.set_root(frame, 0, l);
        let a = heap.allocate_object(200, 0);
        heap.write_ref(l, 1, a);
        heap.write_ref(l, 3, smi::of(7).unwrap());
        heap.collect();
        assert!(heap.is_live_object(a), "REFS element not followed");
        // The list is old: a young object written into it survives the minor collection
        // through the barrier (without it, verification would report).
        let b = heap.allocate_object(201, 0);
        heap.write_ref(l, 2, b);
        assert_eq!(heap.header(l).state, REMEMBERED);
        heap.run_collection(true);
        assert!(
            heap.is_live_object(b),
            "young object of an old list collected"
        );
        assert_eq!(heap.words(l)[1..], [a, b, smi::of(7).unwrap()]);
        // Without a reference, it goes away.
        heap.write_refs(l, 1, &[0, 0]);
        heap.collect();
        assert!(!heap.is_live_object(a) && !heap.is_live_object(b));
        heap.pop_frame(frame);
    }

    #[test]
    fn instance_follows_only_reference_fields() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_instance(200, 40);
        heap.set_root(frame, 0, o);
        let child = heap.allocate_object(201, 0);
        let released = heap.allocate_object(202, 0);
        heap.set_field(o, 35, child, true);
        heap.set_field(o, 36, released, false);
        heap.collect();
        assert!(heap.is_live_object(child));
        assert!(!heap.is_live_object(released), "scalar field followed");
        assert_eq!(heap.words(o).len(), layout::instance_words(40));
        heap.pop_frame(frame);
    }

    #[test]
    fn cards_keep_young_written_into_large_old_list() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let len = 100_000;
        let l = list(&mut heap, len);
        heap.set_root(frame, 0, l);
        assert!(heap.header(l).flags & flags::CARDS != 0);
        heap.collect();
        // Old: scattered old → young writes, each dirtying the card.
        let mut young_set = Vec::new();
        for i in [5, 2048, 40_000, len - 1] {
            let j = heap.allocate_object(200, 0);
            heap.write_ref(l, 1 + i, j);
            young_set.push((i, j));
        }
        heap.run_collection(true);
        for &(i, j) in &young_set {
            assert!(
                heap.is_live_object(j),
                "young object in element {i} collected (card not walked)"
            );
            assert_eq!(heap.words(l)[1 + i], j);
        }
        // With aging, the young elements stay young (survivors): only their
        // cards stay dirty, and the list stays remembered.
        let dirty: u32 = heap.words(l)[1 + len..]
            .iter()
            .map(|&c| c.count_ones())
            .sum();
        assert_eq!(dirty, 4, "only the cards of the young elements stay dirty");
        assert_eq!(heap.header(l).state, REMEMBERED);
        // The next minor collection promotes them and clears the cards.
        heap.run_collection(true);
        for &(i, j) in &young_set {
            assert!(heap.is_live_object(j), "survivor in element {i} collected");
            assert_eq!(heap.header(j).state, OLD);
        }
        let cards = &heap.words(l)[1 + len..];
        assert!(cards.iter().all(|&c| c == 0), "cards not cleared");
        assert_eq!(heap.header(l).state, OLD);
        // The explicit barrier (write from outside) dirties all of them.
        let k = heap.allocate_object(201, 0);
        heap.words_mut(l)[1 + 77_777] = k;
        heap.remember(l);
        heap.run_collection(true);
        assert!(heap.is_live_object(k));
        heap.pop_frame(frame);
    }

    #[test]
    fn aging_promotes_on_the_second_minor_collection() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let a = heap.allocate_object(200, 1);
        heap.set_root(frame, 0, a);
        let garbage = heap.allocate_object(201, 0);
        heap.run_collection(true);
        assert!(layout::state::is_young(heap.header(a).state) && heap.header(a).state != YOUNG);
        assert!(!heap.is_live_object(garbage));
        // A survivor that dies is freed by the next minor collection.
        let b = heap.allocate_object(202, 0);
        heap.set_field(a, 0, b, true);
        heap.run_collection(true);
        assert_eq!(
            heap.header(a).state,
            REMEMBERED,
            "the survivor is promoted, remembered for its young child"
        );
        assert!(layout::state::is_young(heap.header(b).state) && heap.header(b).state != YOUNG);
        heap.set_field(a, 0, 0, false);
        heap.run_collection(true);
        assert!(
            !heap.is_live_object(b),
            "a dead survivor is freed by the minor collection"
        );
        assert_eq!(heap.stats().live_objects, 1);
        heap.pop_frame(frame);
    }

    #[test]
    fn promoted_object_with_young_child_stays_remembered() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let a = heap.allocate_object(200, 1);
        heap.set_root(frame, 0, a);
        heap.run_collection(true);
        // `a` is a survivor (young): generated code writes into it without a barrier.
        let b = heap.allocate_object(201, 1);
        let c = heap.allocate_object(202, 0);
        // SAFETY: test; field 0 of live instances, marked as references.
        #[allow(unsafe_code)]
        unsafe {
            *fields_of(body(heap.live_block(a))) = b;
            mark_reference(body(heap.live_block(a)), 0, true);
            *fields_of(body(heap.live_block(b))) = c;
            mark_reference(body(heap.live_block(b)), 0, true);
        }
        heap.run_collection(true);
        // `a` was promoted pointing to `b`, which stays young: remembered.
        assert_eq!(heap.header(a).state, REMEMBERED);
        assert!(layout::state::is_young(heap.header(b).state) && heap.header(b).state != YOUNG);
        heap.run_collection(true);
        // `b` promoted through the remembered `a`; `b` itself points to the young `c`.
        assert_eq!(heap.header(b).state, OLD);
        assert_eq!(heap.header(a).state, OLD, "nothing young left under `a`");
        assert!(heap.is_live_object(c));
        heap.run_collection(true);
        assert!(heap.is_live_object(c));
        assert_eq!(heap.header(c).state, OLD);
        heap.run_collection(false);
        assert_eq!(heap.stats().live_objects, 3);
        heap.pop_frame(frame);
    }

    #[test]
    fn old_object_pointing_to_survivor_stays_remembered() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(200, 1);
        heap.set_root(frame, 0, old_gen);
        heap.run_collection(false);
        let young = heap.allocate_object(201, 0);
        heap.set_field(old_gen, 0, young, true);
        heap.mark_late((young, 0));
        heap.run_collection(true);
        assert!(
            layout::state::is_young(heap.header(young).state) && heap.header(young).state != YOUNG
        );
        assert_eq!(
            heap.header(old_gen).state,
            REMEMBERED,
            "it still points to a young object"
        );
        heap.run_collection(true);
        assert_eq!(heap.header(young).state, OLD);
        assert_eq!(heap.header(old_gen).state, OLD);
        assert!(heap.late_initialized((young, 0)));
        // The late mark of a survivor that dies goes away.
        let other = heap.allocate_object(202, 0);
        heap.mark_late((other, 1));
        heap.set_field(old_gen, 0, other, true);
        heap.run_collection(true);
        heap.set_field(old_gen, 0, 0, false);
        heap.run_collection(true);
        assert!(
            !heap.late_initialized((other, 1)),
            "the dead survivor loses the mark"
        );
        heap.pop_frame(frame);
    }

    #[test]
    fn aging_off_promotes_on_the_first_minor_collection() {
        let mut heap = heap();
        heap.aging = false;
        let frame = heap.push_frame_with_slots(1);
        let a = heap.allocate_object(200, 0);
        heap.set_root(frame, 0, a);
        heap.run_collection(true);
        assert_eq!(heap.header(a).state, OLD);
        heap.pop_frame(frame);
    }

    static RELEASED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    #[allow(unsafe_code)]
    unsafe fn release_box(p: *mut u8) {
        // SAFETY: `p` came from `Box::into_raw` in the test.
        drop(unsafe { Box::from_raw(p.cast::<[u8; 1000]>()) });
        RELEASED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    #[allow(unsafe_code)]
    fn attachment_released_when_owner_dies() {
        let mut heap = heap();
        let before = RELEASED.load(std::sync::atomic::Ordering::SeqCst);
        let frame = heap.push_frame_with_slots(1);
        let a = heap.allocate(cid::STRING_ACCUMULATOR, 1, flags::RAW);
        let p = Box::into_raw(Box::new([0u8; 1000])).cast::<u8>();
        let bytes_before = heap.stats().estimated_bytes;
        // SAFETY: `release_box` is the only release of `p`.
        unsafe { heap.attach(a, release_box, p, 1000) };
        assert_eq!(heap.attachment(a), p);
        assert_eq!(heap.stats().estimated_bytes, bytes_before + 1000);
        // The attachment grows (the `StringBuffer` accumulator): it counts toward the triggers.
        heap.adjust_attachment(a, 3000);
        assert_eq!(heap.stats().estimated_bytes, bytes_before + 3000);
        assert_eq!(heap.objects.attachment_bytes, 3000);
        heap.set_root(frame, 0, a);
        heap.run_collection(true);
        heap.collect();
        assert_eq!(
            RELEASED.load(std::sync::atomic::Ordering::SeqCst),
            before,
            "attachment released with the owner alive"
        );
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(
            RELEASED.load(std::sync::atomic::Ordering::SeqCst),
            before + 1,
            "attachment of the dead not released"
        );
        assert_eq!(heap.objects.attachment_bytes, 0);
    }

    #[test]
    fn large_region_returned() {
        let mut heap = heap();
        let pages = heap.objects.n_pages();
        let t = heap.allocate(cid::typed(1), 5000, flags::RAW);
        assert!(heap.objects.block_of(t).is_some());
        assert_eq!(
            heap.words(t).len(),
            5000,
            "the saturated n read from the region"
        );
        assert_eq!(heap.objects.n_pages(), pages + 1);
        heap.collect();
        assert_eq!(
            heap.objects.n_pages(),
            pages,
            "dead large region did not leave the space"
        );
        assert!(
            heap.objects.cached_bytes() > 0,
            "freed region without cache"
        );
        // Reused from the cache by an allocation of the same size.
        let u = heap.allocate(cid::typed(1), 5000, flags::RAW);
        assert_eq!(heap.objects.cached_bytes(), 0);
        assert!(heap.words(u).iter().all(|&w| w == 0), "dirty reused region");
        heap.collect();
        heap.collect();
        heap.collect();
        assert_eq!(
            heap.objects.cached_bytes(),
            0,
            "idle region did not go back to the system"
        );
        // A 16 KiB medium class is not a large region.
        let pages = heap.objects.n_pages();
        let m = heap.allocate(cid::typed(1), layout::LARGEST_MEDIUM, flags::RAW);
        assert_eq!(heap.words(m).len(), layout::LARGEST_MEDIUM);
        assert!(heap.objects.n_pages() <= pages + 1);
    }

    /// A static in the form of §2.11 (in the test's read-only memory).
    static STATIC_STRING: [u64; 3] = [
        layout::header_word(
            layout::state::PERMANENT,
            flags::RAW,
            2,
            cid::ONE_BYTE_STRING,
        ),
        0,
        3,
    ];

    #[test]
    fn static_never_marked_nor_swept() {
        let start = std::ptr::addr_of!(STATIC_STRING) as usize;
        register_image(start, start + std::mem::size_of_val(&STATIC_STRING));
        let s = start as i64 + layout::HANDLE_OFFSET;
        let v = Heap::box_bool(true);
        let mut heap = heap();
        heap.stress = true;
        assert!(heap.is_static(s) && heap.is_static(v) && heap.is_live_object(s));
        assert_eq!(heap.class(s), cid::ONE_BYTE_STRING);
        let frame = heap.push_frame_with_slots(2);
        let l = list(&mut heap, 2);
        heap.set_root(frame, 0, l);
        heap.set_root(frame, 1, s);
        heap.write_ref(l, 1, s);
        heap.write_ref(l, 2, v);
        let o = heap.allocate_object(200, 1);
        heap.set_field(o, 0, s, true);
        heap.set_global_root(1, o);
        heap.weak_refs.insert(o, s);
        for minor in [true, false, true, false] {
            heap.run_collection(minor);
        }
        assert_eq!(heap.header(s).state, layout::state::PERMANENT);
        assert_eq!(heap.words(s)[0], 3);
        assert_eq!(
            heap.weak_refs.get(&o),
            Some(&s),
            "weak reference to a static zeroed"
        );
        assert!(heap.is_permanent(s));
        assert_eq!(heap.identity_hash(s), heap.identity_hash(s));
        heap.pop_frame(frame);
    }

    #[test]
    #[should_panic(expected = "write to a static object")]
    fn static_is_not_written() {
        let mut heap = heap();
        heap.words_mut(Heap::box_bool(false));
    }

    #[test]
    fn permanents_and_constants_purged() {
        let mut heap = heap();
        let o = heap.allocate_object(200, 0);
        heap.mark_permanent(o);
        heap.mark_constant(o, 0x1234);
        assert!(heap.is_permanent(o));
        heap.collect();
        assert!(
            heap.permanents.is_empty() && heap.constants.is_empty(),
            "dead handle in the identity tables"
        );
        // Alive through a global, it stays.
        let g = heap.allocate_object(200, 0);
        heap.mark_constant(g, 0x5678);
        heap.set_global_root(9, g);
        heap.collect();
        assert_eq!(heap.constant_getter(g), Some(0x5678));
    }

    #[test]
    fn discount_external_after_saturation_preserves_other_bytes() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = heap();
            if let Some(pure) = mode {
                heap.enable_arc();
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(1);
            let object = list(&mut heap, 0);
            heap.set_root(frame, 0, object);
            let bytes_of_object = heap.stats.estimated_bytes;
            heap.count_external(17);
            for _ in 0..3 {
                heap.count_external(isize::MAX);
            }
            assert_eq!(heap.stats.estimated_bytes, usize::MAX);
            heap.collect();
            assert_eq!(heap.stats.estimated_bytes, usize::MAX);
            for _ in 0..3 {
                heap.count_external(-isize::MAX);
            }
            assert_eq!(heap.external, 17);
            assert_eq!(heap.stats.estimated_bytes, bytes_of_object + 17);
            heap.count_external(-17);
            assert_eq!(heap.stats.estimated_bytes, bytes_of_object);
            heap.pop_frame(frame);
            heap.collect();
            assert_eq!(heap.stats.estimated_bytes, 0);
        }
    }

    #[test]
    fn extreme_external_byte_debt_keeps_the_trigger() {
        let mut heap = heap();
        heap.schedule = None;
        heap.byte_threshold = usize::MAX;
        heap.byte_limit = usize::MAX;
        heap.count_external(isize::MAX);
        heap.count_external(isize::MAX);
        heap.count_external(1);
        assert!(
            heap.needs_collection(16),
            "the debt cannot go back to zero through overflow"
        );
        heap.count_external(1);
        assert_eq!(heap.young_bytes, usize::MAX);
        assert!(heap.needs_collection(16));
    }

    #[test]
    fn external_bytes_fire_the_trigger() {
        let mut heap = heap();
        assert!(!heap.needs_collection(16));
        let too_many = heap.byte_threshold + 1;
        heap.count_external(too_many as isize);
        assert!(
            heap.needs_collection(16),
            "external bytes outside the trigger"
        );
        heap.collect();
        assert!(
            heap.stats().estimated_bytes >= too_many,
            "the full collection lost the external bytes"
        );
        heap.count_external(-(too_many as isize));
        assert!(heap.stats().estimated_bytes < too_many);
    }

    #[test]
    fn set_flags_changes_only_the_shape() {
        let mut heap = heap();
        let l = heap.allocate(cid::LIST, 3, flags::RAW | flags::ELEMENT_INT);
        heap.set_flags(l, flags::FORMAT | flags::ELEMENT, flags::REFS);
        assert_eq!(heap.header(l).flags, flags::REFS);
    }

    #[test]
    fn medium_and_exact_classes_coexist() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let l = list(&mut heap, 200);
        heap.set_root(frame, 0, l);
        let mut hs = Vec::new();
        for w in [1, 7, 64, 65, 70, 71, 300, 1000, 2014] {
            let h = heap.allocate(cid::typed(1), w, flags::RAW);
            heap.words_mut(h)[w - 1] = w as i64;
            hs.push((w, h));
        }
        for (i, &(_, h)) in hs.iter().enumerate() {
            heap.write_ref(l, 1 + i, h);
        }
        heap.run_collection(true);
        heap.collect();
        for &(w, h) in &hs {
            assert_eq!(heap.words(h).len(), w);
            assert_eq!(heap.words(h)[w - 1], w as i64, "body of {w} words lost");
        }
        heap.pop_frame(frame);
    }
}

/// What the ARC synchronization takes from the minor marking ([`Heap::promotion_data`]).
struct PromotionData {
    /// The young objects the marking promoted.
    promoted: Vec<Ref>,
    /// The remembered old objects (the trace).
    remembered: Vec<Ref>,
    /// The older objects the young survivors of the aging reference.
    referenced_by_young: Vec<Ref>,
}

/// The ARC state in the heap (docs/ARC-IMPLEMENTACAO.md, on top of
/// docs/ARC-CICLOS-ESPECIFICACAO.md §19 and §22): the RC of the strong occurrences
/// between old objects and in the owning slots. Observational roots are
/// seen during synchronization, without counting RC; young objects use the minor collection until promotion.
pub struct HeapArc {
    pub state: crate::arc::ArcHeapState,
    /// The old objects handed out raw in this epoch ([`Heap::words_mut`],
    /// [`Heap::object_fields`], [`Heap::set_flags`]): the counted occurrences
    /// they had before. Synchronization counts the current ones and releases
    /// these; until then, the writes counted on the spot skip the object.
    snapshots: crate::hash::HashMap<Ref, Vec<Ref>>,
    ephemerons: ArcEphemerons,
    /// `DARTFORGE_ARC_CHECK=1`: after each synchronization, no
    /// reachable object dead and the RC of each live one equal to the recounted occurrences.
    check: bool,
    /// `DARTFORGE_ARC_CYCLES=always`: the cycle round (*trial
    /// deletion*) on every synchronization; `never`: neither it nor the full collection's
    /// fixed point (the diagnostic); the default: the fixed point in the full collection.
    cycles: Option<bool>,
    /// Pure ARC (the prepared program's; docs/ARC-IMPLEMENTACAO.md §6): every
    /// reference write counts on the spot, also in a young object, and draining
    /// ([`Heap::drain_arc`]) decides lifetime without walking the heap. Otherwise
    /// the nursery (the unprepared program's, [`Heap::enable_arc_mode`]): young
    /// objects are uncounted, the traced minor collection frees the dead ones in
    /// bulk, and the survivors' edges are counted at promotion.
    pure: bool,
    /// Nursery mode: the strong references from remembered old objects to young
    /// ones that the minor collection in progress found (owner, young target),
    /// counted by promotion ([`Heap::sync_arc`]).
    old_to_young: Vec<(Ref, Ref)>,
}

impl std::fmt::Debug for HeapArc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcDoHeap")
            .field("registrados", &self.state.registered_set())
            .field("estatisticas", &self.state.statistics)
            .field("fotos", &self.snapshots.len())
            .field("efemeros", &self.ephemerons.counted.len())
            .finish()
    }
}

/// The counted ephemeron entries: each one is an occurrence of the value,
/// attributed to the key in the graph (the value lives while the key and the holder
/// live; the holder's death releases the entry during synchronization).
#[derive(Default)]
struct ArcEphemerons {
    counted: crate::hash::HashMap<Ref, (Ref, Ref)>,
    by_key: crate::hash::HashMap<Ref, Vec<Ref>>,
}

impl ArcEphemerons {
    fn insert_entry(&mut self, holder: Ref, key: Ref, value: Ref) {
        self.counted.insert(holder, (key, value));
        self.by_key.entry(key).or_default().push(holder);
    }
    fn remove_it(&mut self, holder: Ref) -> Option<(Ref, Ref)> {
        let (key, value) = self.counted.remove(&holder)?;
        if let Some(l) = self.by_key.get_mut(&key) {
            l.retain(|&p| p != holder);
            if l.is_empty() {
                self.by_key.remove(&key);
            }
        }
        Some((key, value))
    }
}

/// The ARC graph over the heap: the positions marking walks and the
/// counted ephemeron entries (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2).
struct HeapGraph<'a> {
    heap: &'a mut Heap,
    ephemerons: &'a mut ArcEphemerons,
}

impl crate::arc::ArcGraph for HeapGraph<'_> {
    fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: the positions are body words of a live block.
        #[allow(unsafe_code)]
        self.heap.visit_strong_references(h, f);
        // Without a counted ephemeron (the common case), no per-object hashing.
        if self.ephemerons.by_key.is_empty() {
            return;
        }
        if let Some(ps) = self.ephemerons.by_key.get(&h) {
            for p in ps {
                if let Some(&(k, v)) = self.ephemerons.counted.get(p)
                    && k == h
                {
                    f(v);
                }
            }
        }
    }
    fn take_edges(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        if !self.heap.attachments_by_owner.is_empty()
            && self.heap.attachments_by_owner.contains_key(&h)
        {
            self.heap.attachments.retain(|a| {
                if a.owner == h
                    && let FinalizerAction::Dart(action) = a.action
                {
                    f(action);
                    return false;
                }
                true
            });
            self.heap.reindex_attachments();
        }
        // SAFETY: the positions are body words of a live block.
        #[allow(unsafe_code)]
        self.heap.ref_positions(h, &mut |p| unsafe {
            f(*p);
            *p = 0;
        });
        if !self.ephemerons.by_key.is_empty() {
            // The conditional edges (the key holds the value) and
            // breaking them, as in `edges` and `break_off`.
            let mut value_list = Vec::new();
            if let Some(ps) = self.ephemerons.by_key.get(&h) {
                for p in ps {
                    if let Some(&(k, v)) = self.ephemerons.counted.get(p)
                        && k == h
                    {
                        value_list.push(v);
                    }
                }
            }
            for v in value_list {
                f(v);
            }
            if let Some(ps) = self.ephemerons.by_key.remove(&h) {
                for p in ps {
                    self.ephemerons.counted.remove(&p);
                    if let Some(e) = self.heap.ephemerons.get_mut(&p)
                        && e.0 == h
                    {
                        *e = (0, 0);
                    }
                }
            }
        }
    }
    fn break_off(&mut self, h: Ref) {
        if !self.heap.attachments_by_owner.is_empty()
            && self.heap.attachments_by_owner.contains_key(&h)
        {
            self.heap
                .attachments
                .retain(|a| a.owner != h || !matches!(a.action, FinalizerAction::Dart(_)));
            self.heap.reindex_attachments();
        }
        // SAFETY: the positions are body words of a live block.
        #[allow(unsafe_code)]
        self.heap.ref_positions(h, &mut |p| unsafe { *p = 0 });
        if self.ephemerons.by_key.is_empty() {
            return;
        }
        if let Some(ps) = self.ephemerons.by_key.remove(&h) {
            for p in ps {
                self.ephemerons.counted.remove(&p);
                if let Some(e) = self.heap.ephemerons.get_mut(&p)
                    && e.0 == h
                {
                    *e = (0, 0);
                }
            }
        }
    }
}

/// Does object `h` have a `RAW` body (no reference positions: string,
/// numeric box, typed data)? ARC's cycle candidate filter
/// (`ArcHeapState::without_edges`).
fn block_without_refs(h: Ref) -> bool {
    // SAFETY: ARC only calls with registered handles, live blocks of the space
    // (the image has no metadata).
    #[allow(unsafe_code)]
    unsafe {
        (*((h - HANDLE_OFFSET) as *const Header)).flags & block_flags::FORMAT == block_flags::RAW
    }
}

/// A counter violation is a compiler or runtime bug, never a Dart
/// error (docs/ARC-CICLOS-ESPECIFICACAO.md §19.3).
#[cold]
fn arc_failure(e: crate::arc::ArcError) -> ! {
    panic!("ARC bug: {e:?}")
}

/// The geometry of the page of block `h` for the ARC metadata table
/// (`arc::Geometry`); `None` outside the space (the metadata goes to the reserve).
/// [`Heap::arc_retain_registering`] over the heap's parts: retains `v`, registering
/// the object of the space without metadata first. `validate` is
/// [`Heap::validate_handles`] (see [`Heap::space_block`]).
#[allow(unsafe_code)]
fn retain_registering_in(space: &ObjectSpace, validate: bool, arc: &mut HeapArc, v: Ref) {
    if arc
        .state
        .retain_if_registered(v)
        .unwrap_or_else(|e| arc_failure(e))
    {
        return;
    }
    let in_space = if validate {
        space.block_of(v).is_some()
    } else {
        // SAFETY: an object handle is a block or a static (`Heap::space_block`).
        is_object(v) && unsafe { (*((v - HANDLE_OFFSET) as *const Header)).state } != PERMANENT
    };
    if in_space {
        arc.state.register_live_with(v, arc_geometry(space, v), 1);
    }
}

fn arc_geometry(space: &ObjectSpace, h: Ref) -> Option<crate::arc::Geometry> {
    space
        .geometry(h)
        .map(|(first, size, blocks)| crate::arc::Geometry {
            first,
            size,
            blocks,
        })
}

/// Pure ARC, in the draining loop (after step 4, in which every young object that
/// survives is registered): the object of the space without metadata died in this
/// draining — the young object nobody counted, or one killed by RC, whose
/// metadata `take_dead` already removed. The static (`PERMANENT`) is not in the
/// space and never dies.
#[allow(unsafe_code)]
fn died_in_drain(
    space: &ObjectSpace,
    validate: bool,
    state: &crate::arc::ArcHeapState,
    h: Ref,
) -> bool {
    if !is_object(h) {
        return false;
    }
    let in_space = if validate {
        space.block_of(h).is_some()
    } else {
        // SAFETY: an object handle is a block or a static (the contract
        // of `space_block` without validation).
        unsafe { (*((h - HANDLE_OFFSET) as *const Header)).state != PERMANENT }
    };
    in_space && state.meta(h).is_none()
}

#[allow(unsafe_code)]
impl Heap {
    /// ARC is enabled.
    pub fn arc_active(&self) -> bool {
        self.arc.is_some()
    }

    /// The ARC counters, if enabled.
    pub fn arc_statistics(&self) -> Option<crate::arc::ArcStats> {
        self.arc.as_ref().map(|a| a.state.statistics)
    }

    /// The swap of `old` for `new` in a strong position of block `b` (of
    /// `h`), counted on the spot if the owner was not snapshotted. Under pure ARC
    /// it always counts, and a destination still without metadata is registered (the young object
    /// that gains its first occurrence). In nursery mode it only counts with an
    /// old owner (the young object's edges enter at promotion).
    fn arc_swapped(&mut self, b: *mut Header, h: Ref, old: Ref, new: Ref) {
        let Some(arc) = self.arc.as_ref() else {
            return;
        };
        // SAFETY: live block of the space.
        let e = unsafe { (*b).state };
        // The nursery's young owner (the common case) first, without lookups.
        if old == new || (!arc.pure && e != OLD && e != REMEMBERED) {
            return;
        }
        let pure = arc.pure;
        let new_in_space = pure && self.space_block(new).is_some();
        let (counted_new, counted_old) = if pure {
            (Some(new), Some(old))
        } else {
            (self.arc_counted(new), self.arc_counted(old))
        };
        let arc = self.arc.as_deref_mut().expect("ARC on");
        if (counted_new.is_none() && counted_old.is_none()) || arc.snapshots.contains_key(&h) {
            return;
        }
        if !pure {
            if let Some(new) = counted_new {
                arc.state.retain_ref(new).unwrap_or_else(|e| arc_failure(e));
            }
            if let Some(old) = counted_old {
                arc.state.release(old).unwrap_or_else(|e| arc_failure(e));
            }
            return;
        } else if !arc
            .state
            .retain_if_registered(new)
            .unwrap_or_else(|e| arc_failure(e))
            && new_in_space
        {
            arc.state
                .register_live_at(new, arc_geometry(&self.objects, new));
            arc.state.retain_ref(new).unwrap_or_else(|e| arc_failure(e));
        }
        arc.state.release(old).unwrap_or_else(|e| arc_failure(e));
    }

    /// The object of the space `h` still without metadata gets registered (RC zero,
    /// live) before its first counted occurrence.
    fn arc_register_if_needed(space: &ObjectSpace, arc: &mut HeapArc, h: Ref) {
        if smi::e_handle(h) && arc.state.meta(h).is_none() {
            arc.state.register_live_at(h, arc_geometry(space, h));
        }
    }

    /// Retains `v` (registering the object of the space without metadata): a
    /// new occurrence, under pure ARC.
    fn arc_retain_registering(&self, arc: &mut HeapArc, v: Ref) {
        retain_registering_in(&self.objects, self.validate_handles, arc, v);
    }

    /// The references the generated code wrote inline into a freshly
    /// allocated object (`AllocObject`, inline context, cell and closure,
    /// `dartforge_arc_initial`): under pure ARC, each one counts. In
    /// nursery mode it does nothing (they count at promotion).
    pub fn arc_count_initial(&mut self, h: Ref) {
        if !smi::e_handle(h) || self.space_block(h).is_none() {
            return;
        }
        let Some(mut arc) = self.arc.take() else {
            return;
        };
        if arc.pure {
            // Retention only changes ARC metadata: it neither collects nor changes the
            // visited body. Counting directly avoids a vector per object
            // and keeps each occurrence, including repeated references.
            self.visit_strong_references(h, &mut |v| self.arc_retain_registering(&mut arc, v));
        }
        self.arc.set(arc);
    }

    /// The raw write of a native that does not go through `Heap` (the `_data` of
    /// `Map` in `natives_hash.rs`, the JSON stack): the swap of `old` for
    /// `new` in a strong position of `h`.
    pub fn arc_raw_write(&mut self, h: Ref, old: Ref, new: Ref) {
        if let Some(b) = self.space_block(h) {
            self.arc_swapped(b, h, old, new);
        }
    }

    /// The snapshot of the counted occurrences of `h` before a raw write
    /// (one per epoch): of an old object in nursery mode, of every object under pure
    /// ARC (the young object also counts writes).
    fn arc_snapshot(&mut self, b: *mut Header, h: Ref) {
        // SAFETY: live block of the space.
        let e = unsafe { (*b).state };
        if e != OLD && e != REMEMBERED && !self.arc.as_ref().is_some_and(|a| a.pure) {
            return;
        }
        if self
            .arc
            .as_ref()
            .is_some_and(|a| a.snapshots.contains_key(&h))
        {
            return;
        }
        let mut value_list = Vec::new();
        // SAFETY: the positions are body words of a live block.
        self.visit_strong_references(h, &mut |v| value_list.push(v));
        let arc = self.arc.as_deref_mut().expect("ARC ligado");
        value_list.retain(|&v| arc.state.meta(v).is_some());
        arc.snapshots.insert(h, value_list);
    }

    /// The young objects promoted by the minor marking in progress and the remembered
    /// old ones, as handles (before the sweep of the young), and the older objects
    /// the young survivors of the aging reference: those references are not
    /// counted until the survivor's promotion, so they protect their targets from
    /// dying by RC meanwhile, like roots.
    fn promotion_data(&self) -> PromotionData {
        let mut promoted = Vec::new();
        let mut kept = Vec::new();
        self.objects.young_marked(&mut |b, promotes| {
            if promotes {
                promoted.push(b as i64 + HANDLE_OFFSET);
            } else {
                kept.push(b as i64 + HANDLE_OFFSET);
            }
        });
        let mut referenced_by_young = Vec::new();
        for h in kept {
            self.visit_strong_references(h, &mut |v| {
                if smi::e_handle(v) && !self.young_child(v) && self.space_block(v).is_some() {
                    referenced_by_young.push(v);
                }
            });
        }
        let remembered = self
            .objects
            .remembered
            .iter()
            .map(|&b| b as i64 + HANDLE_OFFSET)
            .collect();
        PromotionData {
            promoted,
            remembered,
            referenced_by_young,
        }
    }

    /// Enables ARC (`dartforge_memory_arc_v1`, at program startup): a
    /// full tracing collection and the registration of every live object with the RC
    /// of the occurrences that reach it, including owning slots.
    /// Observational roots only protect the objects they see.
    pub fn enable_arc(&mut self) {
        self.enable_arc_mode(false);
    }

    /// [`Self::enable_arc`] choosing the young objects' treatment: `nursery`
    /// leaves them uncounted until promotion; otherwise every write counts on the
    /// spot (the pure ARC). `DARTFORGE_ARC_NURSERY=1` forces the nursery on a
    /// program compiled for the pure ARC (measurement); the reverse is a
    /// compile-time choice, since nursery code writes young objects inline,
    /// without counting. A heap whose ARC is already on keeps its mode: switching
    /// after objects were counted would break the counts.
    pub fn enable_arc_mode(&mut self, nursery: bool) {
        if self.arc.is_some() {
            return;
        }
        let nursery =
            nursery || crate::console::env_var("DARTFORGE_ARC_NURSERY").is_ok_and(|v| v == "1");
        self.run_collection(false);
        let mut arc = Box::new(HeapArc {
            state: crate::arc::ArcHeapState::new(),
            snapshots: crate::hash::HashMap::default(),
            ephemerons: ArcEphemerons::default(),
            check: crate::console::env_var("DARTFORGE_ARC_CHECK").is_ok_and(|v| v == "1"),
            cycles: match crate::console::env_var("DARTFORGE_ARC_CYCLES").as_deref() {
                Ok("always") => Some(true),
                Ok("never") => Some(false),
                _ => None,
            },
            pure: !nursery,
            old_to_young: Vec::new(),
        });
        arc.state.trace_log =
            crate::console::env_var("DARTFORGE_ARC_TRACE").is_ok_and(|v| v == "1");
        arc.state.retain_dead_candidates =
            crate::console::env_var("DARTFORGE_ARC_RETAIN_DEAD_CANDIDATES").is_ok_and(|v| v == "1");
        arc.state.without_edges = Some(block_without_refs);
        let reached_set = self.reachable_by_tracing();
        for &x in &reached_set {
            arc.state
                .register_live_at(x, arc_geometry(&self.objects, x));
        }
        let mut value_list = Vec::new();
        for &x in &reached_set {
            // SAFETY: the positions are body words of a live block.
            self.visit_strong_references(x, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        for (&p, &(k, v)) in &self.ephemerons {
            if arc.state.alive(p) && smi::e_handle(k) && smi::e_handle(v) {
                arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                arc.ephemerons.insert_entry(p, k, v);
            }
        }
        for h in self.owning_roots() {
            self.arc_retain_registering(&mut arc, h);
        }
        self.mark_side_edges(&mut arc);
        for &x in &reached_set {
            arc.state.review(x);
        }
        self.arc.set(arc);
    }

    /// Restores side indications after activation or metadata rebuilding.
    fn mark_side_edges(&self, arc: &mut HeapArc) {
        for &owner in self.attachments_by_owner.keys() {
            arc.state.mark_side_edges(owner);
        }
        for &(key, _) in arc.ephemerons.counted.values() {
            arc.state.mark_side_edges(key);
        }
    }

    /// The objects of the space reachable from the roots, with the ephemeron rule
    /// (the value when the holder and the key are reached), without marks.
    fn reachable_by_tracing(&self) -> Vec<Ref> {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let mut seen: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let mut out = Vec::new();
        loop {
            while let Some(h) = stack.pop() {
                if !smi::e_handle(h)
                    || !is_object(h)
                    || self.space_block(h).is_none()
                    || !seen.insert(h)
                {
                    continue;
                }
                out.push(h);
                // SAFETY: the positions are body words of a live block.
                self.visit_strong_references(h, &mut |v| stack.push(v));
            }
            let reached = |h: &Ref| {
                !smi::e_handle(*h)
                    || seen.contains(h)
                    || (is_object(*h) && self.space_block(*h).is_none())
            };
            for (p, (k, v)) in &self.ephemerons {
                if reached(p) && reached(k) && smi::e_handle(*v) && !seen.contains(v) {
                    stack.push(*v);
                }
            }
            if stack.is_empty() {
                break;
            }
        }
        out
    }

    /// [`Self::reachable_by_tracing`] through the mark bitmap instead of a hash set:
    /// clears every mark and leaves marked exactly the space objects reachable from
    /// the roots (with the ephemeron rule). Only for the nursery's full
    /// synchronization, whose reclamation ([`Self::reclaim_arc`]) rebuilds the marks
    /// right after; the pure ARC reads the marks of its young objects. The edges it
    /// walked count as marking work (the spacing of full collections under
    /// `--gc-stress`, as in the tracing collector).
    fn mark_reachable(&mut self) -> usize {
        self.objects.clear_marks();
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let mut reached = 0;
        let mut work = 0;
        loop {
            while let Some(h) = stack.pop() {
                work += 1;
                if !smi::e_handle(h) {
                    continue;
                }
                let Some(b) = self.space_block(h) else {
                    continue;
                };
                // SAFETY: block of the space.
                if unsafe { marked(b) } {
                    continue;
                }
                // SAFETY: block of the space.
                unsafe { mark_block(b) };
                reached += 1;
                // SAFETY: the positions are body words of a live block.
                self.visit_strong_references(h, &mut |v| stack.push(v));
            }
            let reached_by = |h: &Ref| {
                !smi::e_handle(*h)
                    || self
                        .space_block(*h)
                        // SAFETY: block of the space.
                        .is_none_or(|b| unsafe { marked(b) })
            };
            for (p, (k, v)) in &self.ephemerons {
                if reached_by(p) && reached_by(k) && !reached_by(v) {
                    stack.push(*v);
                }
            }
            if stack.is_empty() {
                break;
            }
        }
        self.marking_work += work;
        reached
    }

    /// The fixed point and the global discard transaction of the full collection (§22.3):
    /// `L` is the reachability from the actual roots through the strong edges, with the
    /// conditional ephemeron rule (the value enters when the holder and the key
    /// are in `L`); every registered live object outside `L` dies in a single batch,
    /// including one with positive RC only through cycles.
    ///
    /// Between full collections an ephemeron entry is an occurrence of the value
    /// attributed to the key: it holds the value while the key lives, even with the
    /// holder already unreachable (retention, never premature death). Here the
    /// holder counts: an entry whose holder is outside `L` leaves the count
    /// before the batch, and the value that survives loses the occurrence. One whose key is
    /// outside `L` leaves through the discard itself (breaking the key zeroes the
    /// entry and releases the value).
    ///
    /// Under the nursery, `L` is the mark bitmap ([`Self::mark_reachable`]); under pure
    /// ARC, whose drain reads the marks of the young objects, a hash set.
    fn arc_fixpoint(&mut self, arc: &mut HeapArc) {
        let (l, reached) = if arc.pure {
            let l: crate::hash::HashSet<Ref> = self.reachable_by_tracing().into_iter().collect();
            let n = l.len();
            (Some(l), n)
        } else {
            (None, self.mark_reachable())
        };
        let in_l = |h: Ref| match &l {
            Some(l) => l.contains(&h),
            // SAFETY: block of the space.
            None => self.space_block(h).is_some_and(|b| unsafe { marked(b) }),
        };
        let d: Vec<Ref> = arc
            .state
            .live_set()
            .filter(|h| {
                !in_l(*h)
                    && arc
                        .state
                        .meta(*h)
                        .is_some_and(|m| m.state == crate::arc::ArcState::Live && !m.immortal)
            })
            .collect();
        if arc.state.trace_log {
            eprintln!(
                "[arc-trace] fixed point: {reached} reached, {} dead",
                d.len()
            );
        }
        if d.is_empty() {
            return;
        }
        let in_d: crate::hash::HashSet<Ref> = d.iter().copied().collect();
        let leaving: Vec<Ref> = arc
            .ephemerons
            .counted
            .iter()
            .filter(|&(p, &(k, _))| in_d.contains(p) && !in_d.contains(&k))
            .map(|(&p, _)| p)
            .collect();
        for p in leaving {
            if let Some((_, v)) = arc.ephemerons.remove_it(p)
                && !in_d.contains(&v)
            {
                arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        let n = d.len();
        let HeapArc {
            state, ephemerons, ..
        } = arc;
        let mut graph = HeapGraph {
            heap: self,
            ephemerons,
        };
        state
            .discard(&mut graph, &d)
            .unwrap_or_else(|e| arc_failure(e));
        state.statistics.dead_by_cycle += n as u64;
    }

    /// ARC synchronization, at the end of the minor collection (§19.3, §19.4, §22):
    ///
    /// 1. the promoted ones get metadata (`rc=0`);
    /// 2. the increments: the occurrences leaving the promoted ones, those the
    ///    remembered old objects have to promoted ones, the current ones of the snapshotted and
    ///    the new ephemeron entries;
    /// 3. the decrements: the snapshots and the replaced ephemeron entries;
    /// 4. the zeros die in cascade, except those seen by the roots (which
    ///    stay in the queue); in the full collection, the global fixed point
    ///    ([`Heap::arc_fixpoint`]); with `DARTFORGE_ARC_CYCLES=always`, the
    ///    cycle round on every synchronization;
    /// 5. the side tables forget the dead (weak, ephemerons,
    ///    finalizers) until there are no new deaths;
    /// 6. in the full collection, the reclamation: the marks are the RC live ones and the
    ///    full sweep returns the blocks of the dead.
    fn sync_arc(&mut self, full: bool, promotion: PromotionData) {
        let PromotionData {
            promoted,
            remembered,
            referenced_by_young,
        } = promotion;
        let mut arc = self.arc.take().expect("ARC ligado");
        if arc.state.trace_log {
            eprintln!(
                "[arc-trace] synchronize full={full} promoted={promoted:?} remembered={remembered:?} snapshots={:?}",
                arc.snapshots.keys().collect::<Vec<_>>()
            );
        }
        // The promoted ones are those registered from here on (a generation at
        // least `first`), without a set of their own.
        let first = arc.state.next_generation();
        for &h in &promoted {
            arc.state
                .register_live_at(h, arc_geometry(&self.objects, h));
        }
        let promoted_now = |state: &crate::arc::ArcHeapState, h: Ref| {
            state.meta(h).is_some_and(|m| m.generation >= first)
        };
        // Promotion rebuilt RC; restores each owning occurrence,
        // without retaining again the owners of objects that were already old.
        for h in self.owning_roots() {
            if smi::e_handle(h) && promoted_now(&arc.state, h) {
                arc.state.retain_ref(h).unwrap_or_else(|e| arc_failure(e));
            }
        }
        let mut value_list = Vec::new();
        let mut release: Vec<Ref> = Vec::new();
        for &h in &promoted {
            // SAFETY: the positions are body words of a live block.
            self.visit_strong_references(h, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        // The references the remembered old objects have to promoted ones, as the
        // minor collection's card scan found them (a snapshotted owner counts all
        // its current references below). A target that stays young (aging) keeps
        // its owner remembered, and a later promotion counts it.
        for (x, v) in std::mem::take(&mut arc.old_to_young) {
            if promoted_now(&arc.state, v) && !arc.snapshots.contains_key(&x) && arc.state.alive(x)
            {
                arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        for (x, snapshot) in std::mem::take(&mut arc.snapshots) {
            if arc.state.alive(x) {
                // SAFETY: the positions are body words of a live block.
                self.visit_strong_references(x, &mut |v| value_list.push(v));
                for v in value_list.drain(..) {
                    arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
            release.extend(snapshot);
        }
        // Ephemerons: the current live entries against the counted ones.
        let mut currents: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        for (&p, &(k, v)) in &self.ephemerons {
            if !(arc.state.alive(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            currents.insert(p);
            match arc.ephemerons.counted.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                before => {
                    if before.is_some()
                        && let Some((_, v0)) = arc.ephemerons.remove_it(p)
                    {
                        release.push(v0);
                    }
                    arc.state.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                    arc.ephemerons.insert_entry(p, k, v);
                }
            }
        }
        let exited: Vec<Ref> = arc
            .ephemerons
            .counted
            .keys()
            .copied()
            .filter(|p| !currents.contains(p))
            .collect();
        for p in exited {
            if let Some((_, v0)) = arc.ephemerons.remove_it(p) {
                release.push(v0);
            }
        }
        for v in release {
            arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
        }
        self.mark_side_edges(&mut arc);
        for &h in &promoted {
            arc.state.review(h);
        }
        // All roots protect the zeros they see; the owning slots
        // already had their occurrences included in RC.
        // A sorted vector: built on every minor collection, consulted only for the
        // zeros (cheaper than hashing every root).
        let mut root_list = Vec::new();
        self.root_list(&mut root_list);
        root_list.retain(|&h| smi::e_handle(h));
        for &h in &root_list {
            arc.state.add_candidate(h);
        }
        // The targets of the young survivors' uncounted references (aging) are
        // protected like the roots' until the survivor is promoted or dies.
        root_list.extend(referenced_by_young);
        root_list.sort_unstable();
        root_list.dedup();
        let protected = |h: Ref| root_list.binary_search(&h).is_ok();
        // With `DARTFORGE_ARC_CYCLES=always`, the cycle round repeats on
        // each pass with new deaths: the discard (and the ephemeron that leaves with the
        // holder) releases survivors that may have been left in a cycle without a
        // root. In the full collection, the fixed point decides everyone's lifetime (§22.3).
        let cycles = arc.cycles == Some(true);
        let mut fixpoint = full && arc.cycles != Some(false);
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        let mut finalize = Vec::new();
        arc.state.resume_deferred();
        loop {
            {
                let HeapArc {
                    state, ephemerons, ..
                } = &mut *arc;
                let mut graph = HeapGraph {
                    heap: self,
                    ephemerons,
                };
                state
                    .drain_zeros(&mut graph, usize::MAX, &protected)
                    .unwrap_or_else(|e| arc_failure(e));
                if cycles {
                    state
                        .collect_cycles(&mut graph, &protected)
                        .unwrap_or_else(|e| arc_failure(e));
                    state
                        .drain_zeros(&mut graph, usize::MAX, &protected)
                        .unwrap_or_else(|e| arc_failure(e));
                }
            }
            if fixpoint {
                fixpoint = false;
                self.arc_fixpoint(&mut arc);
                let HeapArc {
                    state, ephemerons, ..
                } = &mut *arc;
                let mut graph = HeapGraph {
                    heap: self,
                    ephemerons,
                };
                state
                    .drain_zeros(&mut graph, usize::MAX, &protected)
                    .unwrap_or_else(|e| arc_failure(e));
            }
            let dead_set = arc.state.take_dead();
            if dead_set.is_empty() {
                break;
            }
            // Dead: an object of the space, not young, without metadata (what
            // `take_dead` returned now or before; the dead ones' blocks wait for the
            // reclamation). Asking the table costs less than hashing every dead
            // object when the side tables are small.
            let validate = self.validate_handles;
            let objects = &self.objects;
            let state = &arc.state;
            let dead = |h: &Ref| {
                smi::e_handle(*h)
                    && died_in_drain(objects, validate, state, *h)
                    && !crate::layout::state::is_young(
                        // SAFETY: an object of the space (`died_in_drain`).
                        unsafe { (*((*h - HANDLE_OFFSET) as *const Header)).state },
                    )
            };
            drop(dead_set);
            self.weak_refs.retain(|holder, target| {
                if dead(target) {
                    *target = 0;
                }
                !dead(holder)
            });
            let holders: Vec<Ref> = self.ephemerons.keys().copied().filter(dead).collect();
            self.attachments.retain_mut(|a| {
                if matches!(a.action, FinalizerAction::Native(_, _, _)) && dead(&a.owner) {
                    a.owner = 0;
                }
                if dead(&a.detach) {
                    a.detach = 0;
                }
                if !dead(&a.value) {
                    return true;
                }
                match a.action {
                    FinalizerAction::Dart(action) => {
                        ready.push(action);
                        attachment_edges.push((a.owner, action));
                    }
                    FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
                }
                false
            });
            Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
            self.late_fields_initialized.retain(|(h, _)| !dead(h));
            self.new_lates.retain(|(h, _)| !dead(h));
            self.permanents.retain(|h| !dead(h));
            self.constants.retain(|h, _| !dead(h));
            self.tearoff_code.retain(|h, _| !dead(h));
            self.finalizable.retain(|h, &mut pair| {
                let stays = !dead(h);
                if !stays {
                    finalize.push(pair);
                }
                stays
            });
            for p in holders {
                self.ephemerons.remove(&p);
                if let Some((_, v)) = arc.ephemerons.remove_it(p) {
                    arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
        }
        self.reindex_attachments();
        // A young survivor (aging) is not registered: its promotion counts the
        // queue's occurrence.
        for &action in &ready {
            arc.state
                .retain_ref(action)
                .unwrap_or_else(|e| arc_failure(e));
        }
        for (_, action) in attachment_edges {
            arc.state.release(action).unwrap_or_else(|e| arc_failure(e));
        }
        self.ready_finalizations.extend(ready);
        if full {
            self.reclaim_arc(&arc);
        } else {
            self.minors_since_full += 1;
        }
        if arc.check {
            self.check_arc(&mut arc);
        }
        if self.trace {
            let e = arc.state.statistics;
            eprintln!(
                "[arc] {} promoted={} counted={} retains={} releases={} rc_dead={} cycle_dead={} rounds={} examined={} candidates={}",
                if full { "full" } else { "minor" },
                promoted.len(),
                arc.state.registered_set(),
                e.retains,
                e.releases,
                e.dead_by_rc,
                e.dead_by_cycle,
                e.cycle_rounds,
                e.examined,
                arc.state.candidates()
            );
        }
        self.arc.set(arc);
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            // SAFETY: the attachment's `NativeFinalizerFunction`, `void f(void*)`;
            // as in the VM, it runs during collection and cannot touch the heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// Pure ARC draining (docs/ARC-IMPLEMENTACAO.md §6), in place of the
    /// minor collection, without walking the heap:
    ///
    /// 1. the snapshots (raw writes since the last draining) count the
    ///    current occurrences and release the snapshot's (the snapshotted object's lifetime
    ///    follows RC and the roots, like any object's);
    /// 2. the young object (handed out since the last draining) that only a direct root
    ///    sees gets registered (RC zero, protected);
    /// 3. the live ephemeron entries against the counted ones (the value is an
    ///    occurrence attributed to the key);
    /// 4. the registered young object (through a counted occurrence or step 2)
    ///    stays (marked, old, cycle candidate); the rest died without
    ///    anyone seeing it: releases the occurrences it counted;
    /// 5. the zeros die in cascade, except those the roots see; in the
    ///    full collection, the fixed point (with ephemerons) or the cycle round until there are
    ///    no new deaths; the side tables forget the dead;
    /// 6. memory: the young dead through the sweep of the young (without a mark),
    ///    the RC dead one by one.
    ///
    /// The shadow stack (or the maps), observational frames and tables not yet
    /// migrated protect what they see directly, without counting RC.
    /// Owning slots, globals, exception/stack trace and strong edges of objects
    /// count occurrences.
    /// Turns on the real ARC's prompt death ([`Heap::prompt_death`]) over the pure
    /// ARC. Under the nursery (forced by `DARTFORGE_ARC_NURSERY=1`, a measurement)
    /// it stays off: young objects are not counted there.
    pub(crate) fn enable_prompt_death(&mut self) {
        self.prompt_death = self.arc.as_ref().is_some_and(|a| a.pure);
    }

    /// Is the real ARC's prompt death on?
    pub fn prompt_death(&self) -> bool {
        self.prompt_death
    }

    /// Keeps the inventory of code tokens: always, except in the real ARC, whose
    /// tokens are counted owners like any other and need no inventory (the audit,
    /// `DARTFORGE_ARC_CHECK=1`, keeps it to recount them).
    fn code_inventory(&self) -> bool {
        !self.prompt_death || self.arc.as_ref().is_some_and(|a| a.check)
    }

    /// The settle point of the real ARC: each object whose count reached zero dies
    /// now — its weak references read null, its finalizer is queued, the targets of
    /// its edges lose an occurrence and die in turn (an iterative queue). Only the
    /// runtime's observational roots (the frames of the natives, the scratch roots,
    /// the shared empty storages) still protect a zero; it waits for the next drain.
    /// The memory goes back at the next drain. Without prompt death, or with no zero,
    /// it does nothing.
    ///
    /// Called where no native holds an uncounted reference: the code's release, and
    /// the end of the runtime entries that release or observe the dead.
    pub(crate) fn arc_settle(&mut self) {
        if !self.prompt_death || self.collection_deferred > 0 {
            return;
        }
        let Some(arc) = self.arc.as_ref() else {
            return;
        };
        // A zero an observational root held goes back to the queue once no native
        // frame nor scratch root is left (the usual state while Dart code runs).
        let observers = self.scratch_roots.iter().any(|&h| h != 0)
            || self.frames.iter().any(|(_, _, owner)| !*owner);
        let resume = arc.state.has_deferred() && !observers;
        if !arc.state.ha_zeros() && arc.snapshots.is_empty() && !resume {
            return;
        }
        let mut arc = self.arc.take().expect("ARC on");
        // The raw writes and the ephemerons count their current occurrences first:
        // an object they hold must not die by a count that misses them.
        if !arc.snapshots.is_empty() {
            let mut value_list = Vec::new();
            for (x, snapshot) in std::mem::take(&mut arc.snapshots) {
                if self.space_block(x).is_some() && !arc.state.dead(x) {
                    // SAFETY: the positions are body words of a live block.
                    self.visit_strong_references(x, &mut |v| value_list.push(v));
                    for v in value_list.drain(..) {
                        self.arc_retain_registering(&mut arc, v);
                    }
                    Self::arc_register_if_needed(&self.objects, &mut arc, x);
                }
                for v in snapshot {
                    arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
        }
        if !self.ephemerons.is_empty() {
            self.sync_arc_ephemerons(&mut arc);
        }
        // The observational roots of the runtime, few and short-lived (none, while
        // Dart code runs outside a native's callback).
        let mut observed: Vec<Ref> = Vec::new();
        if observers || self.empty_storages.iter().any(|&h| h != 0) {
            observed.extend(
                self.frames
                    .iter()
                    .filter(|(_, _, owner)| !*owner)
                    .flat_map(|(_, roots, _)| roots.iter().copied())
                    .chain(self.scratch_roots.iter().copied())
                    .chain(self.empty_storages.iter().copied())
                    .filter(|&h| smi::e_handle(h)),
            );
        }
        observed.sort_unstable();
        observed.dedup();
        let protected = |h: Ref| observed.binary_search(&h).is_ok();
        if resume {
            arc.state.resume_deferred();
        }
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        let mut finalize = Vec::new();
        loop {
            {
                let HeapArc {
                    state, ephemerons, ..
                } = &mut *arc;
                let mut graph = HeapGraph {
                    heap: self,
                    ephemerons,
                };
                state
                    .drain_zeros(&mut graph, usize::MAX, &protected)
                    .unwrap_or_else(|e| arc_failure(e));
            }
            let first = self.prompt_dead.len();
            if arc.state.transfer_dead(&mut self.prompt_dead) == 0 {
                break;
            }
            // The side tables a death is observable through; usually all empty.
            if self.weak_refs.is_empty()
                && self.ephemerons.is_empty()
                && self.attachments.is_empty()
                && self.finalizable.is_empty()
            {
                continue;
            }
            let mut dead_now: Vec<Ref> = self.prompt_dead[first..].to_vec();
            dead_now.sort_unstable();
            let dead = |h: &Ref| dead_now.binary_search(h).is_ok();
            if !self.weak_refs.is_empty() {
                self.weak_refs.retain(|holder, target| {
                    if dead(target) {
                        *target = 0;
                    }
                    !dead(holder)
                });
            }
            if !self.ephemerons.is_empty() {
                let holders: Vec<Ref> = self
                    .ephemerons
                    .keys()
                    .copied()
                    .filter(|p| dead(p))
                    .collect();
                for e in self.ephemerons.values_mut() {
                    if dead(&e.0) {
                        *e = (0, 0);
                    }
                }
                for p in holders {
                    self.ephemerons.remove(&p);
                    if let Some((_, v)) = arc.ephemerons.remove_it(p) {
                        arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
                    }
                }
            }
            if !self.attachments.is_empty() {
                self.attachments.retain_mut(|a| {
                    if matches!(a.action, FinalizerAction::Native(_, _, _)) && dead(&a.owner) {
                        a.owner = 0;
                    }
                    if dead(&a.detach) {
                        a.detach = 0;
                    }
                    if !dead(&a.value) {
                        return true;
                    }
                    match a.action {
                        FinalizerAction::Dart(action) => {
                            ready.push(action);
                            attachment_edges.push((a.owner, action));
                        }
                        FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
                    }
                    false
                });
                Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
            }
            if !self.finalizable.is_empty() {
                for h in &dead_now {
                    if let Some(pair) = self.finalizable.remove(h) {
                        finalize.push(pair);
                    }
                }
            }
            // The identity indexes (`permanents`, `constants`, `tearoff_code`) and the
            // `late` marks lose the dead at the next drain, before their blocks go back.
        }
        if !ready.is_empty() || !attachment_edges.is_empty() {
            self.reindex_attachments();
            for &action in &ready {
                self.arc_retain_registering(&mut arc, action);
            }
            for (_, action) in attachment_edges {
                arc.state.release(action).unwrap_or_else(|e| arc_failure(e));
            }
            self.ready_finalizations.extend(ready);
        }
        self.arc.set(arc);
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            // SAFETY: the attachment's `NativeFinalizerFunction`, `void f(void*)`;
            // as in the VM, it runs during collection and cannot touch the heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// The pure ARC's ephemerons: counts the current live entries against the
    /// counted ones (the value of a new or changed entry is retained, the one of a
    /// changed or gone entry released).
    fn sync_arc_ephemerons(&mut self, arc: &mut HeapArc) {
        let mut release: Vec<Ref> = Vec::new();
        let mut currents: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let entries: Vec<(Ref, Ref, Ref)> = self
            .ephemerons
            .iter()
            .map(|(&p, &(k, v))| (p, k, v))
            .collect();
        for (p, k, v) in entries {
            if !(arc.state.alive(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            currents.insert(p);
            match arc.ephemerons.counted.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                before => {
                    if before.is_some()
                        && let Some((_, v0)) = arc.ephemerons.remove_it(p)
                    {
                        release.push(v0);
                    }
                    self.arc_retain_registering(arc, v);
                    arc.ephemerons.insert_entry(p, k, v);
                }
            }
        }
        let exited: Vec<Ref> = arc
            .ephemerons
            .counted
            .keys()
            .copied()
            .filter(|p| !currents.contains(p))
            .collect();
        for p in exited {
            if let Some((_, v0)) = arc.ephemerons.remove_it(p) {
                release.push(v0);
            }
        }
        for v in release {
            arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
        }
    }

    fn drain_arc(&mut self, full: bool) {
        check_collection_allowed();
        let trace = self.trace;
        let start = trace.then(crate::clock::Instant::now);
        // The time of each stage, for the trace (roots, snapshots, young objects of the
        // roots, ephemerons, decision on the young, release of the dead young,
        // candidate roots, zeros/cycles, memory).
        // Without a trace, do not reserve metrics nor query the clock.
        let mut phases: Vec<u128> = if trace {
            Vec::with_capacity(10)
        } else {
            Vec::new()
        };
        if let Some(a) = &self.schedule {
            a.pending.set(false);
        }
        self.return_tlabs();
        self.stats.collections += 1;
        let mut arc = self.arc.take().expect("ARC ligado");
        let mut root_list = Vec::new();
        self.root_list(&mut root_list);
        let protected_set: crate::hash::HashSet<Ref> = root_list
            .into_iter()
            .filter(|&h| smi::e_handle(h))
            .collect();
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 1. The snapshots.
        let mut value_list = Vec::new();
        let n_snapshots = arc.snapshots.len();
        for (x, snapshot) in std::mem::take(&mut arc.snapshots) {
            if self.space_block(x).is_some() && !arc.state.dead(x) {
                // SAFETY: the positions are body words of a live block.
                self.visit_strong_references(x, &mut |v| value_list.push(v));
                for v in value_list.drain(..) {
                    self.arc_retain_registering(&mut arc, v);
                }
                // Once registered, the snapshotted object follows RC: without an occurrence or
                // root, it dies later and releases the ones it just counted.
                Self::arc_register_if_needed(&self.objects, &mut arc, x);
            }
            for v in snapshot {
                arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 2. The young objects only the roots see.
        let mut young_set = Vec::new();
        self.objects.young_handed_out(&mut |b| young_set.push(b));
        // Only the roots can add registrations at this stage. Checking
        // all young objects here cost a metadata lookup per allocation,
        // although the decision on each young object already happens in step 4.
        for &h in &protected_set {
            if self.young_child(h) && arc.state.meta(h).is_none() {
                arc.state
                    .register_live_at(h, arc_geometry(&self.objects, h));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 3. Ephemerons: the current live entries against the counted ones.
        self.sync_arc_ephemerons(&mut arc);
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 4. The young: the registered stays, the rest died.
        let mut dead_young: Vec<Ref> = Vec::new();
        let n_young = young_set.len();
        for &b in &young_set {
            let h = b as i64 + HANDLE_OFFSET;
            if arc.state.decide_young(h) {
                // SAFETY: block handed out since the last draining, occupied.
                unsafe {
                    (*b).state = OLD;
                    mark_block(b);
                }
            } else {
                dead_young.push(h);
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        for &h in &dead_young {
            // SAFETY: the positions are body words of a block not yet released.
            self.visit_strong_references(h, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        self.mark_side_edges(&mut arc);
        // Roots generate no decrement when they disappear: what they hold becomes
        // a cycle candidate again.
        for &h in &protected_set {
            arc.state.add_candidate(h);
        }
        let protected = |h: Ref| protected_set.contains(&h);
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // The settle points' dead lose their `late` marks and their entries in the
        // identity indexes before their blocks go back (a reused block must not read a
        // field as initialized): the settle point left them to this drain, whose loop
        // below may find no new death to clean.
        if !self.prompt_dead.is_empty() {
            let mut gone = self.prompt_dead.clone();
            gone.sort_unstable();
            let dead = |h: &Ref| gone.binary_search(h).is_ok();
            self.late_fields_initialized.retain(|(h, _)| !dead(h));
            self.new_lates.retain(|(h, _)| !dead(h));
            self.permanents.retain(|h| !dead(h));
            self.constants.retain(|h, _| !dead(h));
            self.tearoff_code.retain(|h, _| !dead(h));
        }
        // 5. Zeros, cycles and the full collection's fixed point.
        let round = full && arc.cycles != Some(false);
        let mut fixpoint = round && !self.ephemerons.is_empty();
        let cycles = arc.cycles == Some(true) || (round && !fixpoint);
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        let mut finalize = Vec::new();
        let mut dead_rc: Vec<Ref> = Vec::new();
        let mut first_one = true;
        arc.state.resume_deferred();
        // The loop in parts, for the trace: zeros, cycles, ephemeron fixed
        // point, side tables (µs).
        let mut run_loop = [0u128; 4];
        loop {
            let t0 = trace.then(crate::clock::Instant::now);
            {
                let HeapArc {
                    state, ephemerons, ..
                } = &mut *arc;
                let mut graph = HeapGraph {
                    heap: self,
                    ephemerons,
                };
                state
                    .drain_zeros(&mut graph, usize::MAX, &protected)
                    .unwrap_or_else(|e| arc_failure(e));
                if let Some(t0) = t0 {
                    run_loop[0] += t0.elapsed().as_micros();
                }
                if cycles {
                    let t1 = trace.then(crate::clock::Instant::now);
                    state
                        .collect_cycles(&mut graph, &protected)
                        .unwrap_or_else(|e| arc_failure(e));
                    state
                        .drain_zeros(&mut graph, usize::MAX, &protected)
                        .unwrap_or_else(|e| arc_failure(e));
                    if let Some(t1) = t1 {
                        run_loop[1] += t1.elapsed().as_micros();
                    }
                }
            }
            if fixpoint {
                let t2 = trace.then(crate::clock::Instant::now);
                fixpoint = false;
                self.arc_fixpoint(&mut arc);
                let HeapArc {
                    state, ephemerons, ..
                } = &mut *arc;
                let mut graph = HeapGraph {
                    heap: self,
                    ephemerons,
                };
                state
                    .drain_zeros(&mut graph, usize::MAX, &protected)
                    .unwrap_or_else(|e| arc_failure(e));
                if let Some(t2) = t2 {
                    run_loop[2] += t2.elapsed().as_micros();
                }
            }
            let t3 = trace.then(crate::clock::Instant::now);
            let amount = arc.state.transfer_dead(&mut dead_rc);
            // Young objects also require side cleanup in the first round, but
            // are reclaimed by the sweep: they do not enter the RC dead.
            let has_dead = amount != 0 || (first_one && !dead_young.is_empty());
            first_one = false;
            if !has_dead {
                break;
            }
            // The (small) side tables lose the dead: the object of the
            // space without metadata died in this draining — after step 4,
            // every live object of the space is registered. A set with the round's
            // dead (millions, in trees that fall at once) cost more
            // than the cascade.
            let validate = self.validate_handles;
            let space = &self.objects;
            let state = &arc.state;
            let dead = |h: &Ref| died_in_drain(space, validate, state, *h);
            self.weak_refs.retain(|holder, target| {
                if dead(target) {
                    *target = 0;
                }
                !dead(holder)
            });
            let holders: Vec<Ref> = self
                .ephemerons
                .keys()
                .copied()
                .filter(|p| dead(p))
                .collect();
            // A dead key zeroes the entry (breaking the registered key
            // already zeroed it; that of the dead young object, here).
            for e in self.ephemerons.values_mut() {
                if dead(&e.0) {
                    *e = (0, 0);
                }
            }
            self.attachments.retain_mut(|a| {
                if matches!(a.action, FinalizerAction::Native(_, _, _)) && dead(&a.owner) {
                    a.owner = 0;
                }
                if dead(&a.detach) {
                    a.detach = 0;
                }
                if !dead(&a.value) {
                    return true;
                }
                match a.action {
                    FinalizerAction::Dart(action) => {
                        ready.push(action);
                        attachment_edges.push((a.owner, action));
                    }
                    FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
                }
                false
            });
            Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
            self.late_fields_initialized.retain(|(h, _)| !dead(h));
            self.new_lates.retain(|(h, _)| !dead(h));
            self.permanents.retain(|h| !dead(h));
            self.constants.retain(|h, _| !dead(h));
            self.tearoff_code.retain(|h, _| !dead(h));
            self.finalizable.retain(|h, &mut pair| {
                let stays = !dead(h);
                if !stays {
                    finalize.push(pair);
                }
                stays
            });
            for p in holders {
                self.ephemerons.remove(&p);
                if let Some((_, v)) = arc.ephemerons.remove_it(p) {
                    arc.state.release(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
            if let Some(t3) = t3 {
                run_loop[3] += t3.elapsed().as_micros();
            }
        }
        self.reindex_attachments();
        for &action in &ready {
            self.arc_retain_registering(&mut arc, action);
        }
        for (_, action) in attachment_edges {
            arc.state.release(action).unwrap_or_else(|e| arc_failure(e));
        }
        self.ready_finalizations.extend(ready);
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 6. Memory.
        // The settle points' dead: the young ones go with the sweep (their block was
        // handed out since the last drain and has no mark), the old ones one by one.
        for h in std::mem::take(&mut self.prompt_dead) {
            // SAFETY: a block of the space that died logically and is not reused before
            // this reclamation.
            let young = crate::layout::state::is_young(unsafe {
                (*((h - HANDLE_OFFSET) as *const Header)).state
            });
            if !young {
                dead_rc.push(h);
            }
        }
        let (dead_swept, mut released_set) = self.objects.sweep_young(false);
        for &h in &dead_rc {
            if let Some(b) = self.space_block(h) {
                // SAFETY: block of the RC dead, outside the young list
                // (the sweep above emptied it), that nobody reads anymore.
                released_set += unsafe { self.objects.release_dead(b) };
            }
        }
        self.stats.reclaimed += (dead_swept + dead_rc.len()) as u64;
        self.subtract_estimated_bytes(released_set);
        self.allocations = 0;
        self.young_bytes = 0;
        if full {
            let live_set = self.objects.live_set;
            self.minors_since_full = 0;
            self.work_in_full = live_set;
            self.recompute_triggers(live_set);
        } else {
            self.minors_since_full += 1;
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        if arc.check {
            self.check_arc(&mut arc);
        }
        if let Some(start) = start {
            let mut previous = 0;
            let stages: Vec<String> = phases
                .iter()
                .map(|&t| {
                    let d = t - previous;
                    previous = t;
                    d.to_string()
                })
                .collect();
            let e = arc.state.statistics;
            eprintln!(
                "[arc] {} us={} phases={} loop={}/{}/{}/{} young={n_young} young_dead={} snapshots={n_snapshots} counted={} retains={} releases={} rc_dead={} cycle_dead={} rounds={} examined={} candidates={} zeros_seen={} deferred={}",
                if full { "full" } else { "drain" },
                start.elapsed().as_micros(),
                stages.join("/"),
                run_loop[0],
                run_loop[1],
                run_loop[2],
                run_loop[3],
                dead_young.len(),
                arc.state.registered_set(),
                e.retains,
                e.releases,
                e.dead_by_rc,
                e.dead_by_cycle,
                e.cycle_rounds,
                e.examined,
                arc.state.candidates(),
                e.seen_zeros,
                e.deferred_zeros
            );
        }
        self.arc.set(arc);
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            // SAFETY: the attachment's `NativeFinalizerFunction`, `void f(void*)`;
            // as in the VM, it runs during collection and cannot touch the heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// The physical reclamation (§19.4): the marks become the RC live ones and
    /// the full sweep returns the blocks of the dead (and releases their out-of-line
    /// bodies and attachments).
    fn reclaim_arc(&mut self, arc: &HeapArc) {
        self.objects.clear_marks();
        let mut live_set = 0usize;
        for h in arc.state.live_set() {
            if let Some(b) = self.space_block(h) {
                // SAFETY: live block of the space.
                unsafe { mark_block(b) };
                live_set += 1;
            }
        }
        self.objects.marked_set += live_set;
        let (dead_set, _, object_bytes) = self.objects.sweep();
        self.stats.reclaimed += dead_set as u64;
        self.set_estimated_bytes((object_bytes as u128).saturating_add(self.external));
        self.minors_since_full = 0;
        self.work_in_full = live_set + self.marking_work;
        self.recompute_triggers(live_set);
    }

    /// `DARTFORGE_ARC_CHECK=1` (§20.3, the auditor mode): no object
    /// reachable from the roots died, and the RC of each live one is the sum of the
    /// recounted occurrences.
    ///
    /// # Panics
    /// With the first divergence.
    fn check_arc(&mut self, arc: &mut HeapArc) {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let root_list: crate::hash::HashSet<Ref> = stack.iter().copied().collect();
        let mut origin: crate::hash::HashMap<Ref, Ref> = crate::hash::HashMap::default();
        let mut seen: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        while let Some(h) = stack.pop() {
            if !smi::e_handle(h)
                || !is_object(h)
                || self.space_block(h).is_none()
                || !seen.insert(h)
            {
                continue;
            }
            // A young survivor (the nursery's aging) has no count yet.
            if !arc.state.alive(h)
                && !(!arc.pure && crate::layout::state::is_young(self.header(h).state))
            {
                let parent_node = origin.get(&h).copied();
                let desc = |x: Ref| {
                    let c = self.header(x);
                    let pos = self.words(x).iter().position(|&w| w == h);
                    format!(
                        "cid {} state {} flags {:#x} n {} position {pos:?} metadata {:?} snapshot {}",
                        c.class_id,
                        c.state,
                        c.flags,
                        c.n,
                        arc.state.meta(x),
                        arc.snapshots.contains_key(&x)
                    )
                };
                panic!(
                    "ARC bug: reachable object without live RC: handle {h} (cid {}, state {}), metadata {:?}, root {}; reached from {parent_node:?} ({})",
                    self.header(h).class_id,
                    self.header(h).state,
                    arc.state.meta(h),
                    root_list.contains(&h),
                    parent_node.map(desc).unwrap_or_default()
                );
            }
            // SAFETY: the positions are body words of a live block.
            self.visit_strong_references(h, &mut |v| {
                if smi::e_handle(v) && !seen.contains(&v) {
                    origin.entry(v).or_insert(h);
                    stack.push(v);
                }
            });
        }
        let HeapArc {
            state, ephemerons, ..
        } = arc;
        let owning_set = self.owning_roots();
        let graph = HeapGraph {
            heap: self,
            ephemerons,
        };
        if let Err(e) = state.audit(&graph, &owning_set) {
            if let crate::arc::ArcError::Audit { handle, .. } = e {
                // The holders the recount saw: the registered objects with an edge to the
                // handle and the owning tables that hold it.
                let mut parents = Vec::new();
                for p in state.registered_handles() {
                    let mut n = 0;
                    crate::arc::ArcGraph::edges(&graph, p, &mut |v| n += usize::from(v == handle));
                    if n > 0 {
                        parents.push(format!(
                            "{p} (cid {}, state {}) x{n}",
                            graph.heap.header(p).class_id,
                            graph.heap.header(p).state
                        ));
                    }
                }
                let heap = graph.heap;
                let tables = [
                    (
                        "globals",
                        heap.globals.values().filter(|&&h| h == handle).count(),
                    ),
                    (
                        "literals",
                        heap.literals.values().filter(|&&h| h == handle).count(),
                    ),
                    (
                        "canonical",
                        heap.canonical_instances
                            .values()
                            .filter(|&&h| h == handle)
                            .count(),
                    ),
                    (
                        "tearoffs",
                        heap.tearoffs.values().filter(|&&h| h == handle).count(),
                    ),
                    (
                        "enums",
                        heap.enum_values.values().filter(|&&h| h == handle).count(),
                    ),
                    ("code", heap.code_owners.get(&handle).copied().unwrap_or(0)),
                    (
                        "frames",
                        heap.frames
                            .iter()
                            .filter(|(_, _, owner)| *owner)
                            .flat_map(|(_, r, _)| r.iter())
                            .filter(|&&h| h == handle)
                            .count(),
                    ),
                ];
                panic!(
                    "ARC bug: {e:?} (cid {}); parents {parents:?}; owning tables {tables:?}",
                    heap.header(handle).class_id
                );
            }
            arc_failure(e);
        }
    }
}

#[cfg(test)]
mod arc_no_heap {
    //! ARC over the real heap (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2, §22.1).
    use super::*;
    use crate::layout::{self, cid, flags};

    fn list(heap: &mut Heap, len: usize) -> Ref {
        let h = heap.allocate(cid::LIST, layout::list_words(len), flags::REFS);
        heap.words_mut(h)[0] = len as i64;
        h
    }

    /// A heap under ARC whose minor collection promotes every survivor (without
    /// the aging, which [`nursery_aging_protects_old_targets_of_survivors`] covers).
    fn heap_arc() -> Heap {
        let mut heap = Heap::new(false);
        heap.verify = true;
        heap.aging = false;
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        heap
    }

    /// The nursery with aging: a young survivor's references are not counted
    /// until its promotion. The old object it references, whose counted
    /// occurrences are gone, is protected meanwhile (it would die by RC while
    /// reachable); the remembered old owner's reference to a survivor counts when
    /// the survivor is promoted, not before.
    #[test]
    fn nursery_aging_protects_old_targets_of_survivors() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        heap.arc.as_mut().unwrap().pure = false;
        assert!(heap.aging);
        let owning = heap.push_owning_frame(2);
        let old = heap.allocate_str("old target");
        heap.set_root(owning, 0, old);
        let holder = list(&mut heap, 1);
        heap.set_root(owning, 1, holder);
        heap.run_collection(true);
        heap.run_collection(true);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(old).unwrap().rc, 1);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(holder).unwrap().rc, 1);
        // A young survivor holds the old target; an old holder holds the survivor.
        let observer = heap.push_frame_with_slots(1);
        let young = list(&mut heap, 1);
        heap.set_root(observer, 0, young);
        heap.write_refs(young, 1, &[old]);
        heap.write_refs(holder, 1, &[young]);
        heap.set_root(observer, 0, 0);
        heap.set_root(owning, 0, 0);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(old).unwrap().rc, 0);
        heap.run_collection(true);
        assert!(crate::layout::state::is_young(heap.header(young).state));
        assert!(heap.arc.as_ref().unwrap().state.meta(young).is_none());
        assert!(alive(&heap, old), "the survivor protects its old target");
        heap.run_collection(true);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(young).unwrap().rc, 1);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(old).unwrap().rc, 1);
        heap.write_refs(holder, 1, &[0]);
        heap.run_collection(true);
        assert!(!alive(&heap, young) && !alive(&heap, old));
        heap.pop_frame(observer);
        heap.pop_frame(owning);
    }

    /// Nursery mode: a young object held for a while by an owning slot has no
    /// count. Before, the slot registered it, its edge to an old child (written
    /// while the owner was young) was never counted, and its death by RC before
    /// promotion released the child once more than retained (`RcZero`).
    #[test]
    fn nursery_owning_slot_leaves_young_object_uncounted() {
        for promote in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = false;
            let observer = heap.push_frame_with_slots(1);
            let child = heap.allocate_str("old child, seen only by an observer");
            heap.set_root(observer, 0, child);
            heap.run_collection(true);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(child).unwrap().rc, 0);
            let owning = heap.push_owning_frame(1);
            let young = list(&mut heap, 1);
            heap.set_root(owning, 0, young);
            assert!(
                heap.arc.as_ref().unwrap().state.meta(young).is_none(),
                "the owning slot does not register a young object"
            );
            heap.write_refs(young, 1, &[child]);
            if !promote {
                heap.set_root(owning, 0, 0);
            }
            heap.run_collection(true);
            assert_eq!(heap.is_live_object(young), promote);
            let rc = heap.arc.as_ref().unwrap().state.meta(child).unwrap().rc;
            assert_eq!(rc, u64::from(promote));
            if promote {
                assert_eq!(heap.arc.as_ref().unwrap().state.meta(young).unwrap().rc, 1);
            }
            heap.pop_frame(owning);
            heap.collect();
            assert!(!heap.is_live_object(young));
            assert!(heap.is_live_object(child));
            heap.pop_frame(observer);
        }
    }

    #[test]
    fn global_receives_token_without_retaining_and_keeps_owner_apart_from_scalar() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(true)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let value = heap.allocate_str("global owned");
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, value);
            assert!(heap.code_owners.is_empty());
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(value).unwrap().rc, 1);
            }
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, value);
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(value).unwrap().rc, 1);
            }
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    heap.move_code_to_global(10, value);
                }))
                .is_err()
            );
            assert_eq!(heap.globals[&10], value);
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, smi::of(42).unwrap());
            assert!(
                !heap.globals.contains_key(&10),
                "an owner is not scalar storage"
            );
            heap.collect();
            assert!(
                heap.is_live_object(value),
                "token SSA independente protege carga"
            );
            heap.release_code_owner(value);
            heap.collect();
            assert!(!heap.is_live_object(value));
            heap.move_code_to_global(10, 0);
        }
    }

    #[test]
    fn slots_and_tokens_transfer_without_retain_on_move_and_with_alias_at_destination() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(true)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(2);
            let a = heap.allocate_str("carga owned");
            heap.set_root(frame, 0, a);
            let old = heap.allocate_str("replaced destination");
            heap.set_root(frame, 1, old);
            assert_eq!(heap.copy_root_to_code(frame, 0), a);
            assert_eq!(heap.copy_root_to_code(frame, 0), a);
            heap.move_code_to_root(frame, 1, a);
            heap.collect();
            assert!(!heap.is_live_object(old));
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(a).unwrap().rc, 3);
            }
            // The destination already contains a: consumes the token and the old occurrence.
            heap.move_code_to_root(frame, 1, a);
            assert!(heap.code_owners.is_empty());
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(a).unwrap().rc, 2);
            }
            assert_eq!(heap.copy_root_to_code(frame, 1), a);
            heap.pop_frame(frame);
            heap.collect();
            assert!(heap.is_live_object(a));
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(a).unwrap().rc, 1);
            }
            heap.release_code_owner(a);
            heap.collect();
            assert!(!heap.is_live_object(a));
        }
    }

    #[test]
    fn transfer_rejects_invalid_destination_or_token_before_mutating() {
        let mut heap = heap_arc();
        let frame = heap.push_owning_frame(1);
        let a = heap.allocate_str("slot owner is not a code token");
        heap.set_root(frame, 0, a);
        let failure = |heap: &mut Heap, q, s, value| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    heap.move_code_to_root(q, s, value);
                }))
                .is_err()
            );
        };
        failure(&mut heap, frame, 0, a);
        assert!(heap.code_owners.is_empty());
        heap.retain_code_owner(a);
        failure(&mut heap, frame, 1, a);
        failure(&mut heap, -1, 0, a);
        let observational = heap.push_frame_with_slots(1);
        failure(&mut heap, observational, 0, a);
        assert_eq!(heap.code_owners[&a], 1);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
        heap.pop_frame(observational);
        heap.collect();
        assert!(heap.is_live_object(a));
        heap.release_code_owner(a);
        heap.pop_frame(frame);
        heap.collect();
        assert!(!heap.is_live_object(a));
    }

    #[test]
    fn code_tokens_count_aliases_without_duplicating_observational_roots() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(true)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_frame_with_slots(1);
            let value = heap.allocate_str("code tokens");
            heap.set_root(frame, 0, value);
            heap.retain_code_owner(value);
            heap.retain_code_owner(value);
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(value).unwrap().rc, 2);
            }
            heap.set_root(frame, 0, 0);
            heap.release_code_owner(value);
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = heap.arc.as_ref() {
                assert_eq!(arc.state.meta(value).unwrap().rc, 1);
            }
            heap.release_code_owner(value);
            assert!(heap.is_live_object(value), "release does not collect");
            heap.collect();
            assert!(!heap.is_live_object(value));
            assert!(heap.code_owners.is_empty());
            heap.pop_frame(frame);
            for scalar in [0, smi::of(42).unwrap()] {
                heap.retain_code_owner(scalar);
                heap.release_code_owner(scalar);
            }
            assert!(heap.code_owners.is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "ARC release without a code token")]
    fn code_release_does_not_consume_frame_owner() {
        let mut heap = heap_arc();
        let frame = heap.push_owning_frame(1);
        let value = heap.allocate_str("somente slot");
        heap.set_root(frame, 0, value);
        heap.release_code_owner(value);
    }

    #[test]
    fn arc_activation_recounts_tokens_and_last_release_allows_collecting_cycle() {
        let mut heap = Heap::new(true);
        let a = list(&mut heap, 1);
        heap.retain_code_owner(a);
        heap.retain_code_owner(a);
        let b = list(&mut heap, 1);
        heap.write_refs(a, 1, &[b]);
        heap.write_refs(b, 1, &[a]);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        heap.collect();
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 3);
        heap.release_code_owner(a);
        heap.collect();
        assert!(heap.is_live_object(a) && heap.is_live_object(b));
        heap.release_code_owner(a);
        assert!(heap.is_live_object(a) && heap.is_live_object(b));
        heap.collect();
        assert!(!heap.is_live_object(a) && !heap.is_live_object(b));
    }

    #[test]
    fn finalization_queue_counts_aliases_and_consumption() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let action = heap.allocate_str("two finalizations");
            heap.add_ready_finalization(action);
            heap.add_ready_finalization(action);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 2);
            assert_eq!(heap.finish_ready_finalization(), Some(action));
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 1);
            heap.shutdown_finalizers();
            assert_eq!(heap.finish_ready_finalization(), None);
            heap.collect();
            assert!(!heap.is_live_object(action));
        }
    }

    #[test]
    fn dead_attachment_publishes_action_owner_in_queue() {
        for (pure, minor) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(3);
            let owner = heap.allocate_str("finalizador");
            heap.set_root(frame, 0, owner);
            let action = heap.allocate_str("ready action");
            heap.set_root(frame, 1, action);
            let value = heap.allocate_str("weak target");
            heap.set_root(frame, 2, value);
            heap.add_attachment(FinalizerAttachment {
                owner,
                value,
                detach: 0,
                action: FinalizerAction::Dart(action),
            });
            heap.set_root(frame, 2, 0);
            heap.run_collection(minor);
            assert!(heap.attachments.is_empty());
            assert_eq!(heap.ready_finalizations.front(), Some(&action));
            assert!(!heap.is_live_object(value));
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 2);
            heap.pop_frame(frame);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 1);
            assert_eq!(heap.finish_ready_finalization(), Some(action));
            heap.collect();
            assert!(!heap.is_live_object(action));
        }
    }

    #[test]
    fn dart_attachment_does_not_root_cycle_between_owner_and_action() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(true)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(3);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let action = list(&mut heap, 1);
            heap.set_root(frame, 1, action);
            heap.write_refs(action, 1, &[owner]);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 2, target);
            heap.add_attachment(FinalizerAttachment {
                owner,
                value: target,
                detach: 0,
                action: FinalizerAction::Dart(action),
            });
            heap.pop_frame(frame);
            heap.collect();
            assert!(
                !heap.is_live_object(owner),
                "the attachment is not a root of the finalizer"
            );
            assert!(!heap.is_live_object(action));
            assert!(!heap.is_live_object(target));
            assert!(heap.attachments.is_empty());
            assert!(
                heap.ready_finalizations.is_empty(),
                "dead owner and target cancel the Dart action"
            );
        }
    }

    #[test]
    fn attachment_index_preserves_aliases_after_removing_middle_position() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(3);
            let a = list(&mut heap, 0);
            heap.set_root(frame, 0, a);
            let b = list(&mut heap, 0);
            heap.set_root(frame, 1, b);
            let action = list(&mut heap, 0);
            heap.set_root(frame, 2, action);
            for (owner, key) in [(a, 3), (b, 7), (a, 5)] {
                heap.add_attachment(FinalizerAttachment {
                    owner,
                    value: owner,
                    detach: key,
                    action: FinalizerAction::Dart(action),
                });
            }
            heap.set_root(frame, 2, 0);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 3);
            heap.detach_finalizer(b, 7);
            let mut edges = Vec::new();
            heap.visit_attachment_edges(a, &mut |v| edges.push(v));
            assert_eq!(edges, [action, action]);
            heap.visit_attachment_edges(b, &mut |_| panic!("attachment removed from the index"));
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 2);
            heap.detach_finalizer(a, 3);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 1);
            heap.detach_finalizer(a, 5);
            heap.collect();
            assert!(!heap.is_live_object(action));
            assert!(heap.attachments_by_owner.is_empty());
            heap.pop_frame(frame);
        }
    }

    #[test]
    fn finalizer_external_size_leaves_once_per_transition() {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn finalize(_: usize) {
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(false)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            heap.count_external(17);
            let frame = heap.push_owning_frame(2);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            let size = heap.byte_threshold + 1;
            let before = RUNS.load(std::sync::atomic::Ordering::Relaxed);
            let attachment = FinalizerAttachment {
                owner,
                value: target,
                detach: target,
                action: FinalizerAction::Native(finalize as *const () as usize, 0, size),
            };
            heap.add_attachment(attachment);
            assert!(
                heap.needs_collection(16),
                "externalSize must cause memory pressure"
            );
            heap.collect();
            assert_eq!(heap.external, size as u128 + 17);
            heap.detach_finalizer(owner, target);
            heap.detach_finalizer(owner, target);
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before);

            heap.add_attachment(attachment);
            heap.set_root(frame, 1, 0);
            heap.collect();
            heap.collect();
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 1);

            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            heap.add_attachment(FinalizerAttachment {
                value: target,
                ..attachment
            });
            heap.shutdown_finalizers();
            heap.shutdown_finalizers();
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 2);
            heap.pop_frame(frame);
        }
    }

    #[test]
    fn native_attachment_rejects_owner_reused_after_wrapper_death() {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn finalize(token: usize) {
            assert_eq!(token, 17);
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(false)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(2);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            heap.add_attachment(FinalizerAttachment {
                owner,
                value: target,
                detach: target,
                action: FinalizerAction::Native(finalize as *const () as usize, 17, 0),
            });
            let before = RUNS.load(std::sync::atomic::Ordering::Relaxed);
            heap.set_root(frame, 0, 0);
            heap.collect();
            assert_eq!(heap.attachments.len(), 1);
            assert_eq!(heap.attachments[0].owner, 0);
            let retained = heap.push_owning_frame(4096);
            let new = (0..4096)
                .find_map(|slot| {
                    let new = list(&mut heap, 0);
                    heap.set_root(retained, slot, new);
                    (new == owner).then_some(new)
                })
                .expect("reuse the block after consuming the page's free range");
            heap.set_root(frame, 0, new);
            heap.detach_finalizer(new, target);
            assert_eq!(heap.attachments.len(), 1);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before);
            heap.pop_frame(retained);
            heap.pop_frame(frame);
            heap.collect();
            assert!(heap.attachments.is_empty());
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 1);
        }
    }

    #[test]
    fn old_owner_attachment_keeps_young_action_and_detach_releases_chain() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(1);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            heap.collect();
            let temp_value = heap.push_owning_frame(2);
            let target = list(&mut heap, 0);
            heap.set_root(temp_value, 0, target);
            let action = list(&mut heap, 1);
            heap.set_root(temp_value, 1, action);
            heap.write_refs(action, 1, &[target]);
            heap.add_attachment(FinalizerAttachment {
                owner,
                value: target,
                detach: 3,
                action: FinalizerAction::Dart(action),
            });
            heap.pop_frame(temp_value);
            heap.run_collection(true);
            assert!(heap.is_live_object(target));
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(action).unwrap().rc, 1);
            assert!(heap.ready_finalizations.is_empty());
            heap.detach_finalizer(owner, 3);
            heap.collect();
            assert!(!heap.is_live_object(action));
            assert!(!heap.is_live_object(target));
            assert!(heap.is_live_object(owner));
            heap.pop_frame(frame);
        }
    }

    fn alive(heap: &Heap, h: Ref) -> bool {
        heap.arc.as_ref().unwrap().state.alive(h)
    }

    /// The count of `h`, if registered: under pure ARC a young object counts each
    /// owning occurrence at once; in nursery mode it has no count until promotion.
    fn rc(heap: &Heap, h: Ref) -> Option<u64> {
        heap.arc.as_ref().unwrap().state.meta(h).map(|m| m.rc)
    }

    #[test]
    fn owning_frames_count_copies_swaps_and_promotion() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let owner = heap.push_owning_frame(2);
            let observer = heap.push_frame_with_slots(1);
            let a = list(&mut heap, 0);
            heap.set_root(owner, 0, a);
            heap.set_root(owner, 1, a);
            heap.set_root(observer, 0, a);
            assert_eq!(rc(&heap, a), pure.then_some(2));
            heap.set_root(owner, 0, a);
            for _ in 0..2 {
                heap.run_collection(true);
                assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
            }
            let b = list(&mut heap, 0);
            heap.set_root(owner, 0, b);
            assert_eq!(rc(&heap, a), Some(1));
            assert_eq!(rc(&heap, b), pure.then_some(1));
            heap.set_root(owner, 1, 0);
            heap.run_collection(true);
            assert!(
                alive(&heap, a),
                "the observational root still protects the zero"
            );
            heap.pop_frame(observer);
            heap.pop_frame(owner);
            heap.collect();
            assert!(!alive(&heap, a) && !alive(&heap, b));
        }
    }

    #[test]
    fn owners_and_strong_edges_count_distinct_occurrences() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(1);
            let a = list(&mut heap, 1);
            heap.set_root(frame, 0, a);
            let b = list(&mut heap, 1);
            heap.root(frame, b);
            heap.write_ref(a, 1, b);
            heap.write_ref(b, 1, a);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(b).unwrap().rc, 2);
            heap.set_root(frame, 0, smi::of(7).unwrap());
            heap.collect();
            assert!(alive(&heap, a) && alive(&heap, b));
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 1);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, a) && !alive(&heap, b));
        }
    }

    #[test]
    fn owning_move_consuming_alias_and_return_preserves_rc() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let caller = heap.push_owning_frame(2);
            let callee = heap.push_owning_frame(1);
            let a = list(&mut heap, 0);
            heap.set_root(callee, 0, a);
            heap.set_root(caller, 0, a);
            heap.move_root(callee, 0, caller, 0);
            assert_eq!(heap.frames.last().unwrap().1[0], 0);
            assert_eq!(rc(&heap, a), pure.then_some(1));
            heap.pop_frame(callee);
            heap.collect();
            assert!(alive(&heap, a));
            heap.move_root(caller, 0, caller, 0);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 1);
            let b = list(&mut heap, 0);
            heap.set_root(caller, 1, b);
            heap.move_root(caller, 0, caller, 1);
            heap.collect();
            assert!(alive(&heap, a) && !alive(&heap, b));
            heap.pop_frame(caller);
            heap.collect();
            assert!(!alive(&heap, a));
        }
    }

    #[test]
    fn move_rejects_observational_root_before_consuming_owner() {
        let mut heap = heap_arc();
        let owner = heap.push_owning_frame(1);
        let observer = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 0);
        heap.set_root(owner, 0, a);
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heap.move_root(owner, 0, observer, 0);
        }));
        assert!(failure.is_err());
        assert_eq!(heap.frames[0].1[0], a);
        assert_eq!(heap.frames[1].1[0], 0);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 1);
        heap.collect();
        heap.pop_frame(observer);
        heap.pop_frame(owner);
        heap.collect();
        assert!(!alive(&heap, a));
    }

    #[test]
    fn closure_construction_transfers_protection_to_strong_edge() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let env = heap.new_context(&[(42, false)]);
            heap.set_root(frame, 0, env);
            let closure = heap.new_closure(123, (env, true), 0, 0);
            heap.set_root(frame, 0, closure);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(env).unwrap().rc, 1);
            assert_eq!(
                heap.arc.as_ref().unwrap().state.meta(closure).unwrap().rc,
                1
            );
            assert_eq!(heap.closure(closure).unwrap().context.0, env);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, env) && !alive(&heap, closure));
        }
    }

    #[test]
    fn temporary_list_owners_survive_growth_and_decompaction() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let l = heap.new_growable(1, 1, crate::lists::Element::Int);
            heap.set_root(frame, 0, l);
            heap.list_set(l, 0, Value::Int(i64::MAX));
            heap.list_push(l, Value::Double(2.5));
            heap.collect();
            assert_eq!(heap.list_shape(l), crate::lists::Element::General);
            assert!(
                matches!(heap.list_get(l, 0), Value::Ref(r) if matches!(heap.value(r), Value::Int(i64::MAX)))
            );
            assert!(
                matches!(heap.list_get(l, 1), Value::Ref(r) if matches!(heap.value(r), Value::Double(x) if x == 2.5))
            );
            assert_eq!(heap.frames.len(), 1, "no temporary frame may remain open");
            let data = heap.list_data(l);
            let boxes = heap.words(data)[1..3].to_vec();
            for &r in &boxes {
                assert_eq!(heap.arc.as_ref().unwrap().state.meta(r).unwrap().rc, 1);
            }
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, l) && !alive(&heap, data));
            assert!(boxes.iter().all(|&r| !alive(&heap, r)));
        }
    }

    #[test]
    fn typed_view_keeps_base_after_consuming_temporary_owners() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let base = heap.new_typed(crate::typed_lists::ty::UINT8, 8);
            heap.set_root(frame, 0, base);
            let c = crate::typed_lists::view_cid(crate::typed_lists::ty::UINT8, false);
            let first_one = heap.new_view(c, base, 2, 6);
            heap.set_root(frame, 0, first_one);
            let second_one = heap.new_view(c, first_one, 1, 3);
            heap.set_root(frame, 0, second_one);
            heap.collect();
            let view = heap.typed(second_one).unwrap();
            assert_eq!(view.base, Some(base));
            assert_eq!(view.offset, 3);
            assert_eq!(view.len, 3);
            assert!(!alive(&heap, first_one));
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(base).unwrap().rc, 1);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, base) && !alive(&heap, second_one));
        }
    }

    #[test]
    fn persistent_owners_count_aliases_move_and_promotion() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let a = list(&mut heap, 0);
            heap.set_global_root(10, a);
            heap.set_global_root(20, a);
            heap.set_runtime_root(0, a);
            for _ in 0..2 {
                heap.run_collection(true);
                assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 3);
            }
            heap.move_global_root(10, 10);
            heap.set_global_root(20, a);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 3);
            heap.move_global_root(10, 20);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
            heap.release_global_root(20);
            heap.release_global_root(20);
            assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 1);
            heap.collect();
            assert!(alive(&heap, a));
            heap.set_runtime_root(0, 0);
            heap.collect();
            assert!(!alive(&heap, a));
        }
    }

    #[test]
    fn arc_activation_counts_globals_and_runtime_slots_open_under_tracing() {
        let mut heap = Heap::new(false);
        let a = list(&mut heap, 0);
        heap.set_global_root(10, a);
        heap.set_runtime_root(1, a);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
        let b = list(&mut heap, 0);
        heap.set_global_root(10, b);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 1);
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(b).unwrap().rc, 1);
        heap.set_runtime_root(1, 0);
        heap.collect();
        assert!(!alive(&heap, a) && alive(&heap, b));
        heap.set_global_root(10, smi::of(7).unwrap());
        heap.collect();
        assert!(!alive(&heap, b));
    }

    #[test]
    fn canonical_tables_count_slots_without_duplicate_registration() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let units = vec![65u16];
            let text = heap.allocate_str("A");
            heap.store_literal(units.clone(), text);
            heap.store_literal(units.clone(), text);
            let tearoff = heap.new_closure(123, (0, false), 0, 0);
            heap.register_tearoff(123, tearoff);
            heap.register_tearoff(123, tearoff);
            let enum_value_of = heap.enum_value(900, 0, "zero");
            assert_eq!(heap.enum_value(900, 0, "zero"), enum_value_of);
            for _ in 0..2 {
                heap.run_collection(true);
                for h in [text, tearoff, enum_value_of] {
                    assert_eq!(heap.arc.as_ref().unwrap().state.meta(h).unwrap().rc, 1);
                }
            }
            assert_eq!(heap.literal(&units), Some(text));
            assert_eq!(heap.registered_tearoff(123), Some(tearoff));
            let replacement = heap.new_closure(123, (0, false), 0, 0);
            heap.register_tearoff(123, replacement);
            assert_eq!(
                heap.arc.as_ref().unwrap().state.meta(tearoff).unwrap().rc,
                0
            );
            assert_eq!(rc(&heap, replacement), pure.then_some(1));
            heap.collect();
            assert_eq!(heap.registered_tearoff(123), Some(replacement));
            assert!(!alive(&heap, tearoff), "the identity index is not an owner");
            assert!(
                [text, replacement, enum_value_of]
                    .iter()
                    .all(|&h| alive(&heap, h))
            );
        }
    }

    #[test]
    fn message_owners_count_in_tracing_walk() {
        let mut heap = Heap::new(false);
        let text = heap.allocate_str("fila");
        let a = heap.retain_message_owner(text);
        let b = heap.retain_message_owner(text);
        let before = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - before, 2);
        assert!(heap.is_live_object(text));
        heap.release_message_owner(a);
        let before = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - before, 1);
        heap.release_message_owner(b);
        heap.collect();
        assert!(!heap.is_live_object(text));
    }

    #[test]
    fn arc_activation_counts_already_open_owning_frames() {
        let mut heap = Heap::new(false);
        let frame = heap.push_owning_frame(0);
        let a = list(&mut heap, 0);
        heap.root(frame, a);
        heap.root(frame, a);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(a).unwrap().rc, 2);
        heap.collect();
        heap.pop_frame(frame);
        heap.collect();
        assert!(!alive(&heap, a));
    }

    #[test]
    fn arc_promotes_counts_and_releases_by_rc() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 2);
        let b = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.set_root(frame, 0, a);
        heap.run_collection(true);
        assert!(alive(&heap, a) && alive(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(b).unwrap().rc, 1);
        // a[0] = null in an old object: b loses its only occurrence and dies.
        heap.write_ref(a, 1, 0);
        heap.run_collection(true);
        assert!(!alive(&heap, b));
        heap.pop_frame(frame);
        heap.run_collection(true);
        assert!(!alive(&heap, a));
        heap.collect();
    }

    #[test]
    fn arc_roots_register_young_without_retaining_garbage_or_recounting_old() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(2);
        let old_gen = list(&mut heap, 0);
        heap.set_root(frame, 0, old_gen);
        heap.run_collection(true);
        let generation = heap
            .arc
            .as_ref()
            .unwrap()
            .state
            .meta(old_gen)
            .unwrap()
            .generation;
        let young = list(&mut heap, 0);
        let garbage = list(&mut heap, 0);
        heap.set_root(frame, 1, young);
        heap.run_collection(true);
        assert!(alive(&heap, old_gen) && alive(&heap, young));
        assert!(!alive(&heap, garbage));
        assert_eq!(
            heap.arc
                .as_ref()
                .unwrap()
                .state
                .meta(old_gen)
                .unwrap()
                .generation,
            generation
        );
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(young).unwrap().rc, 0);
        heap.pop_frame(frame);
        heap.run_collection(true);
        assert!(!alive(&heap, old_gen) && !alive(&heap, young));
    }

    #[test]
    fn arc_cycle_dies_in_full_and_root_holds_it() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 1);
        let b = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.write_ref(b, 1, a);
        heap.set_root(frame, 0, a);
        heap.collect();
        assert!(
            alive(&heap, a) && alive(&heap, b),
            "the root holds the cycle"
        );
        heap.pop_frame(frame);
        heap.run_collection(true);
        assert!(
            alive(&heap, a),
            "the minor collection does not run the cycle round"
        );
        heap.collect();
        assert!(!alive(&heap, a) && !alive(&heap, b));
    }

    #[test]
    fn arc_raw_write_into_old_reconciles_by_snapshot() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 2);
        let b = list(&mut heap, 1);
        let c = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.write_ref(a, 2, c);
        heap.set_root(frame, 0, a);
        heap.run_collection(true);
        // Raw swap: a = [c, c]; b loses, c gains.
        let p = heap.words_mut(a);
        p[1] = c;
        heap.remember(a);
        heap.run_collection(true);
        assert!(!alive(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().state.meta(c).unwrap().rc, 2);
        heap.pop_frame(frame);
        heap.collect();
    }

    // -- Random graphs with an independent oracle (§26.2) ---------------------

    /// xorshift64*: each seed's sequence is reproducible.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Rng {
            Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }
        fn next_ix(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn until(&mut self, n: usize) -> usize {
            (self.next_ix() % n as u64) as usize
        }
    }

    /// The target of a weak reference or the entry of an ephemeron in the model. A target (or
    /// key) that became unreachable turns into `Pending`: the collection may
    /// zero it at any time, and the full one must zero it.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Weak<T> {
        Holds(T),
        Pending,
    }

    /// The test-side graph: lists with the elements, the frame slots,
    /// the weak references and the ephemerons. Reachability is computed only from here, without looking at the
    /// heap: the strong one through the roots and the conditional ephemeron fixed point
    /// (the value of a reached holder is reached when the key is).
    #[derive(Default)]
    struct Model {
        fields: crate::hash::HashMap<Ref, Vec<Ref>>,
        root_list: Vec<Ref>,
        weak_refs: crate::hash::HashMap<Ref, Weak<Ref>>,
        ephemerons: crate::hash::HashMap<Ref, Weak<(Ref, Ref)>>,
        /// Those registered in ARC that became unreachable: the full collection must
        /// kill them.
        garbage: Vec<Ref>,
    }

    impl Model {
        fn reachable(&self) -> crate::hash::HashSet<Ref> {
            let mut live_set = crate::hash::HashSet::default();
            let mut stack: Vec<Ref> = self.root_list.iter().copied().filter(|&h| h != 0).collect();
            loop {
                while let Some(h) = stack.pop() {
                    if live_set.insert(h) {
                        stack.extend(self.fields[&h].iter().copied().filter(|&f| f != 0));
                    }
                }
                for (p, e) in &self.ephemerons {
                    if let Weak::Holds((k, v)) = *e
                        && live_set.contains(p)
                        && live_set.contains(&k)
                        && !live_set.contains(&v)
                    {
                        stack.push(v);
                    }
                }
                if stack.is_empty() {
                    return live_set;
                }
            }
        }

        fn usable(&self) -> Vec<Ref> {
            let mut v: Vec<Ref> = self.reachable().into_iter().collect();
            v.sort_unstable();
            v
        }

        fn draw(&self, r: &mut Rng) -> Option<Ref> {
            let v = self.usable();
            if v.is_empty() {
                None
            } else {
                Some(v[r.until(v.len())])
            }
        }
    }

    /// After a collection: what is reachable keeps the model's elements
    /// (and, under ARC, is registered alive); the weak reference and the ephemeron with a reachable
    /// target stay, and those with an unreachable target become `Pending` (the
    /// full collection requires them zeroed). The unreachable ones leave the model.
    fn check_model(heap: &Heap, m: &mut Model, full: bool, arc: bool, context: &str) {
        let live_set = m.reachable();
        for &h in &live_set {
            if arc {
                assert!(
                    alive(heap, h),
                    "{context}: reachable {h:#x} without live RC"
                );
            }
            let p = heap.words(h);
            for (i, &f) in m.fields[&h].iter().enumerate() {
                assert_eq!(p[1 + i], f, "{context}: element {i} of {h:#x}");
            }
        }
        if full && arc {
            let current_time: Vec<Ref> = m
                .fields
                .keys()
                .copied()
                .filter(|h| !live_set.contains(h))
                .collect();
            m.garbage.extend(current_time);
            for &h in &m.garbage {
                assert!(
                    !alive(heap, h),
                    "{context}: unreachable {h:#x} survived the full collection ({:?}; ephemerons {:?}; weak {:?})",
                    heap.arc.as_ref().unwrap().state.meta(h),
                    heap.ephemerons
                        .iter()
                        .filter(|(p, (k, v))| **p == h || *k == h || *v == h)
                        .collect::<Vec<_>>(),
                    heap.weak_refs
                        .iter()
                        .filter(|(p, a)| **p == h || **a == h)
                        .collect::<Vec<_>>(),
                );
            }
            m.garbage.clear();
        }
        let dead_set: Vec<Ref> = m
            .fields
            .keys()
            .copied()
            .filter(|h| !live_set.contains(h))
            .collect();
        for h in dead_set {
            m.fields.remove(&h);
            if arc && alive(heap, h) {
                m.garbage.push(h);
            }
        }
        let model_fields = &m.fields;
        m.weak_refs.retain(|p, target| {
            if !live_set.contains(p) {
                return false;
            }
            let current = heap.weak_refs.get(p).copied().unwrap_or(0);
            match *target {
                Weak::Holds(a) if live_set.contains(&a) => assert_eq!(current, a, "{context}: weak {p:#x} with live target"),
                Weak::Holds(a) => {
                    assert!(current == a || current == 0, "{context}: weak {p:#x}");
                    *target = Weak::Pending;
                }
                Weak::Pending => {}
            }
            if full && *target == Weak::Pending {
                assert_eq!(
                    current,
                    0,
                    "{context}: weak {p:#x} not zeroed in the full collection (target {:?}; ephemerons {:?}; pointed to by {:?})",
                    heap.arc.as_ref().map(|a| a.state.meta(current)),
                    heap.ephemerons
                        .iter()
                        .filter(|(q, (k, v))| **q == current || *k == current || *v == current)
                        .map(|(q, kv)| (*q, *kv, live_set.contains(q), live_set.contains(&kv.0), heap.arc.as_ref().map(|a| a.state.alive(*q))))
                        .collect::<Vec<_>>(),
                    model_fields.iter().filter(|(_, c)| c.contains(&current)).map(|(h, _)| (*h, live_set.contains(h))).collect::<Vec<_>>(),
                );
            }
            true
        });
        m.ephemerons.retain(|p, e| {
            if !live_set.contains(p) {
                return false;
            }
            let current = heap.ephemerons.get(p).copied().unwrap_or((0, 0));
            match *e {
                Weak::Holds((k, v)) if live_set.contains(&k) => {
                    assert_eq!(current, (k, v), "{context}: ephemeron {p:#x} with live key")
                }
                Weak::Holds((k, v)) => {
                    assert!(
                        current == (k, v) || current == (0, 0),
                        "{context}: ephemeron {p:#x}"
                    );
                    *e = Weak::Pending;
                }
                Weak::Pending => {}
            }
            if full && *e == Weak::Pending {
                assert_eq!(
                    current,
                    (0, 0),
                    "{context}: ephemeron {p:#x} not zeroed in the full collection"
                );
            }
            true
        });
    }

    /// One seed: allocations, counted and raw writes, roots that come in
    /// and go out, weak references and ephemerons, minor and full collections, with the ARC
    /// audit (`check`) on every synchronization and the model oracle after
    /// each collection.
    fn random_graph(seed: u64, arc: bool, cycles_always: bool, steps: usize) {
        const SLOTS: usize = 6;
        let mut r = Rng::new(seed);
        let mut heap = if arc { heap_arc() } else { Heap::new(false) };
        if cycles_always {
            heap.arc.as_mut().unwrap().cycles = Some(true);
        }
        heap.verify = true;
        let frame = heap.push_frame_with_slots(SLOTS);
        let mut m = Model {
            root_list: vec![0; SLOTS],
            ..Model::default()
        };
        for step in 0..steps {
            let context = format!("semente {seed}, passo {step}, arc {arc}");
            match r.until(20) {
                0..=4 => {
                    let len = 1 + r.until(3);
                    let h = list(&mut heap, len);
                    let s = r.until(SLOTS);
                    heap.set_root(frame, s, h);
                    m.root_list[s] = h;
                    m.fields.insert(h, vec![0; len]);
                }
                5..=8 => {
                    if let Some(a) = m.draw(&mut r) {
                        let i = r.until(m.fields[&a].len());
                        let b = if r.until(5) == 0 {
                            0
                        } else {
                            m.draw(&mut r).unwrap_or(0)
                        };
                        if r.until(3) == 0 {
                            heap.words_mut(a)[1 + i] = b;
                            heap.remember(a);
                        } else {
                            heap.write_ref(a, 1 + i, b);
                        }
                        m.fields.get_mut(&a).unwrap()[i] = b;
                    }
                }
                9..=10 => {
                    let s = r.until(SLOTS);
                    heap.set_root(frame, s, 0);
                    m.root_list[s] = 0;
                }
                11 => {
                    if let Some(h) = m.draw(&mut r) {
                        let s = r.until(SLOTS);
                        heap.set_root(frame, s, h);
                        m.root_list[s] = h;
                    }
                }
                12 => {
                    if let (Some(p), Some(a)) = (m.draw(&mut r), m.draw(&mut r))
                        && !m.ephemerons.contains_key(&p)
                    {
                        heap.weak_refs.insert(p, a);
                        m.weak_refs.insert(p, Weak::Holds(a));
                    }
                }
                13..=14 => {
                    if let (Some(p), Some(k), Some(v)) =
                        (m.draw(&mut r), m.draw(&mut r), m.draw(&mut r))
                        && !m.weak_refs.contains_key(&p)
                    {
                        heap.ephemerons.insert(p, (k, v));
                        m.ephemerons.insert(p, Weak::Holds((k, v)));
                    }
                }
                15..=17 => {
                    heap.run_collection(true);
                    check_model(&heap, &mut m, false, arc, &context);
                }
                _ => {
                    heap.collect();
                    check_model(&heap, &mut m, true, arc, &context);
                }
            }
        }
        for s in 0..SLOTS {
            heap.set_root(frame, s, 0);
            m.root_list[s] = 0;
        }
        heap.collect();
        check_model(
            &heap,
            &mut m,
            true,
            arc,
            &format!("semente {seed}, fim, arc {arc}"),
        );
        if arc {
            assert_eq!(
                heap.arc.as_ref().unwrap().state.live_set().count(),
                0,
                "seed {seed}: a live object without a root was left"
            );
        }
        heap.pop_frame(frame);
    }

    /// How many seeds: `DF_ARC_SEEDS` widens the search outside CI.
    fn seeds(pattern: u64) -> u64 {
        crate::console::env_var("DF_ARC_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(pattern)
    }

    #[test]
    fn arc_random_graphs_against_model() {
        for seed in 0..seeds(150) {
            random_graph(seed, true, false, 400);
        }
    }

    /// With the cycle round on every synchronization (the *trial deletion*
    /// also in the minor ones, besides the full collection's fixed point).
    #[test]
    fn arc_random_graphs_with_cycles_always() {
        for seed in 0..seeds(150) {
            random_graph(seed.wrapping_add(1 << 32), true, true, 400);
        }
    }

    /// The same oracle against tracing: validates the model.
    #[test]
    fn arc_random_graphs_model_holds_under_tracing() {
        for seed in 0..seeds(60) {
            random_graph(seed, false, false, 400);
        }
    }
}

#[cfg(test)]
mod tests_observation_without_reset {
    use super::*;
    #[test]
    fn diagnostic_tells_free_block_with_stale_header() {
        for arc in [false, true] {
            let mut heap = Heap::new(false);
            heap.objects.clear_dead = false;
            if arc {
                heap.enable_arc();
            }
            let first = heap.box_int(i64::MAX);
            heap.retain_code_owner(first);
            let neighbor = heap.box_int(i64::MAX - 1);
            heap.retain_code_owner(neighbor);
            heap.release_code_owner(first);
            heap.collect();
            assert_eq!(heap.observe_code_owner(first), 0, "ARC={arc}");
            assert_eq!(heap.observe_code_owner(neighbor), 3);
            heap.release_code_owner(neighbor);
            heap.collect();
        }
    }
}

#[cfg(test)]
mod gc_policy_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn logical_deadline_and_repeated_requests_cannot_extend_or_restart_allowance() {
        let now = Instant::now();
        let mut policy = AutomaticGcPolicy::default();
        assert_eq!(policy.arm(now, Duration::from_secs(2), 100), 1);
        policy.charge(30);
        assert_eq!(
            policy.arm(now + Duration::from_secs(1), Duration::from_secs(9), 1000),
            1
        );
        assert_eq!(policy.window, Some((now + Duration::from_secs(2), 70)));
        assert!(!policy.collect_before(now + Duration::from_secs(1), 10, true, false));
        assert_eq!(policy.arm(now, Duration::from_secs(1), 100), 0);
        assert!(policy.collect_before(now + Duration::from_secs(2), 10, false, false));
        assert_eq!(
            policy.arm(now + Duration::from_secs(2), Duration::from_secs(1), 100),
            0
        );
        policy.serviced();
        assert_eq!(policy.arm(now, Duration::from_secs(1), 100), 1);
    }

    #[test]
    fn unused_cancel_or_expiry_does_not_manufacture_collection_debt() {
        let mut heap = Heap::new(false);
        heap.schedule = None;
        let before = heap.stats().collections;
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 1);
        heap.allocate_object(308, 1);
        heap.gc_policy_cancel();
        assert!(!heap.gc_policy.debt);
        heap.allocate_object(308, 1);
        assert_eq!(heap.stats().collections, before);
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 1);
        heap.gc_policy.window = Some((Instant::now() - Duration::from_secs(1), 1024));
        heap.allocate_object(308, 1);
        assert!(!heap.gc_policy.debt);
        assert_eq!(heap.stats().collections, before);
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 1);
        heap.young_bytes = heap.young_limit;
        assert!(!heap.needs_collection(16));
        assert!(heap.gc_policy.debt);
        heap.gc_policy_cancel();
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 0);
        heap.allocate_object(308, 1);
        assert_eq!(heap.stats().collections, before + 1);
    }

    #[test]
    fn budget_precheck_and_gross_growth_force_debt_without_refunds() {
        let now = Instant::now();
        let mut policy = AutomaticGcPolicy::default();
        assert_eq!(policy.arm(now, Duration::from_secs(9), 100), 1);
        policy.charge(40);
        assert!(!policy.collect_before(now, 59, true, false));
        assert!(policy.collect_before(now, 60, false, false));
        assert_eq!(policy.arm(now, Duration::from_secs(9), 100), 0);
        policy.serviced();
        assert_eq!(policy.arm(now, Duration::from_secs(9), 100), 1);
        policy.charge(1000);
        assert!(policy.collect_before(now, 1, false, false));
    }

    #[test]
    fn runtime_allocation_external_growth_and_explicit_collection_preserve_roots() {
        let mut heap = Heap::new(false);
        heap.schedule = None;
        let frame = heap.push_frame_with_slots(1);
        let root = heap.new_object(301, &[(77, false)]);
        heap.set_root(frame, 0, root);
        let before = heap.stats().collections;
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 256), 1);
        heap.count_external(300);
        heap.count_external(-300);
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 256), 0);
        heap.allocate_object(302, 1);
        assert_eq!(heap.stats().collections, before + 1);
        assert_eq!(heap.object(root).unwrap().field(0), (77, false));
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 256), 1);
        heap.collect();
        assert!(!heap.gc_policy.blocks_tlabs());
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 256), 1);
    }

    #[test]
    fn stress_schedule_and_hard_ceiling_bypass_the_deferral_gate() {
        for forced in 0..3 {
            let mut heap = Heap::new(forced == 0);
            heap.schedule = if forced == 1 {
                Some(Schedule {
                    state: std::cell::Cell::new(1),
                    rate: 1,
                    pending: std::cell::Cell::new(false),
                })
            } else {
                None
            };
            assert_eq!(
                heap.gc_policy_defer(Duration::from_secs(10), 1024 * 1024),
                1
            );
            if forced == 2 {
                heap.byte_limit = 1;
            }
            assert!(heap.needs_collection(16));
            if forced < 2 {
                let before = heap.stats().collections;
                heap.allocate_object(303, 1);
                assert_eq!(heap.stats().collections, before + 1);
                assert!(!heap.gc_policy.blocks_tlabs());
            }
        }
    }

    #[test]
    fn tlab_reservations_are_drained_and_disabled_until_debt_is_serviced() {
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            heap.schedule = None;
            let frame = heap.push_frame_with_slots(1);
            let root = heap.allocate_object(304, 2);
            heap.set_root(frame, 0, root);
            heap.refill_tlab(2);
            assert!(CONTEXT.with(|c| c.tlab[2][0].get() != c.tlab[2][1].get()));
            assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 1);
            assert!(CONTEXT.with(|c| c.tlab[2][0].get().is_null() && c.tlab[2][1].get().is_null()));
            heap.refill_tlab(2);
            assert!(CONTEXT.with(|c| c.tlab[2][0].get() != c.tlab[2][1].get()));
            // A suppressed ordinary trigger creates real debt before cancel.
            heap.young_bytes = heap.young_limit;
            assert!(!heap.needs_collection(16));
            heap.gc_policy_cancel();
            heap.refill_tlab(2);
            assert!(CONTEXT.with(|c| c.tlab[2][0].get().is_null()));
            assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 0);
            heap.collect();
            heap.refill_tlab(2);
            assert!(CONTEXT.with(|c| c.tlab[2][0].get() != c.tlab[2][1].get()));
            heap.return_tlabs();
        })
        .join()
        .unwrap();
    }

    fn has_reservation(words: usize) -> bool {
        CONTEXT.with(|c| c.tlab[words][0].get() != c.tlab[words][1].get())
    }

    #[test]
    fn bounded_pre_debt_reservation_is_prepaid_and_cannot_reenter_accounting() {
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            heap.schedule = None;
            let size = block_bytes(2);
            let before = heap.stats.estimated_bytes;
            assert_eq!(
                heap.gc_policy_defer(Duration::from_secs(10), 3 * size + 1),
                1
            );
            heap.refill_tlab(2);
            assert!(has_reservation(2));
            assert_eq!(heap.stats.estimated_bytes, before + 3 * size);
            assert_eq!(heap.gc_policy.window.unwrap().1, 1);
            assert!(!heap.gc_policy.debt);
            // A second refill sees the published range, not a new charge.
            heap.refill_tlab(2);
            assert_eq!(heap.gc_policy.window.unwrap().1, 1);
            let h = object_space::allocate_like_generated_code(&mut heap, 309, 2);
            assert_eq!(heap.stats.estimated_bytes, before + 3 * size);
            assert!(heap.object(h).is_some());
            heap.count_external(1);
            assert!(heap.gc_policy.debt);
            assert!(!has_reservation(2));
            assert_eq!(heap.stats.estimated_bytes, before + size + 1);
            assert!(heap.object(h).is_some());
            heap.refill_tlab(2);
            assert!(!has_reservation(2));
            assert_eq!(heap.stats.collections, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn suppressed_ordinary_debt_drains_every_size_class() {
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            heap.schedule = None;
            assert_eq!(
                heap.gc_policy_defer(Duration::from_secs(10), 1024 * 1024),
                1
            );
            heap.refill_tlab(2);
            heap.refill_tlab(3);
            assert!(has_reservation(2) && has_reservation(3));
            heap.young_limit = heap.young_bytes;
            assert!(!heap.needs_collection(16));
            assert!(heap.gc_policy.debt);
            assert!(!has_reservation(2) && !has_reservation(3));
            assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 0);
            heap.refill_tlab(2);
            assert!(!has_reservation(2));
            heap.gc_policy.window = Some((Instant::now() - Duration::from_secs(1), 1024));
            assert!(heap.needs_collection(16));
            assert_eq!(heap.stats.collections, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn unused_expiry_or_cancel_preserves_prepaid_blocks_but_external_pressure_drains_them() {
        for expired in [false, true] {
            std::thread::spawn(move || {
                let mut heap = Heap::for_isolate(false);
                heap.schedule = None;
                assert_eq!(
                    heap.gc_policy_defer(Duration::from_secs(10), 1024 * 1024),
                    1
                );
                heap.refill_tlab(2);
                if expired {
                    heap.gc_policy.window = Some((Instant::now() - Duration::from_secs(1), 1024));
                    heap.refill_tlab(3);
                } else {
                    heap.gc_policy_cancel();
                }
                assert!(!heap.gc_policy.is_active());
                assert!(has_reservation(2));
                let before = heap.stats.estimated_bytes;
                let h = object_space::allocate_like_generated_code(&mut heap, 310, 2);
                assert!(heap.object(h).is_some());
                assert_eq!(heap.stats.estimated_bytes, before);
                heap.byte_limit = before + 1;
                heap.count_external(1);
                assert!(!has_reservation(2) && !has_reservation(3));
                assert!(!heap.gc_policy.debt);
                assert_eq!(heap.stats.collections, 0);
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn external_attachment_growth_invalidates_budget_and_quotas_without_collecting_unrooted_headers()
     {
        unsafe fn release_nothing(_: *mut u8) {}
        for endpoint in 0..3 {
            for limit in 0..3 {
                std::thread::spawn(move || {
                    let mut heap = Heap::for_isolate(false);
                    heap.schedule = None;
                    let h = heap.allocate(
                        crate::layout::cid::STRING_ACCUMULATOR,
                        1,
                        crate::layout::flags::RAW,
                    );
                    if endpoint == 1 {
                        // SAFETY: null is never dereferenced and release is a no-op.
                        unsafe { heap.attach(h, release_nothing, std::ptr::null_mut(), 0) };
                    }
                    let before_collections = heap.stats.collections;
                    assert_eq!(
                        heap.gc_policy_defer(Duration::from_secs(10), 1024 * 1024),
                        1
                    );
                    heap.refill_tlab(2);
                    heap.refill_tlab(3);
                    assert!(has_reservation(2) && has_reservation(3));
                    match limit {
                        0 => heap.gc_policy.window.as_mut().unwrap().1 = 100,
                        1 => heap.young_limit = heap.young_bytes + 100,
                        _ => heap.byte_limit = heap.stats.estimated_bytes + 100,
                    }
                    match endpoint {
                        0 => {
                            // SAFETY: null is never dereferenced and release is a no-op.
                            unsafe { heap.attach(h, release_nothing, std::ptr::null_mut(), 100) };
                        }
                        1 => heap.adjust_attachment(h, 100),
                        _ => heap.count_external(100),
                    }
                    assert!(heap.gc_policy.debt);
                    assert!(!has_reservation(2) && !has_reservation(3));
                    assert_eq!(heap.stats.collections, before_collections);
                    // Raw attachment headers are not instances, so object()
                    // is inapplicable; the precise live-block check must pass.
                    let _ = heap.live_block(h);
                    assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1024), 0);
                    heap.refill_tlab(2);
                    assert!(!has_reservation(2));
                    // A negative counter adjustment cannot forgive recorded debt.
                    if endpoint == 2 {
                        heap.count_external(-100);
                    }
                    assert!(heap.gc_policy.debt);
                })
                .join()
                .unwrap();
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn zero_external_growth_at_quota_equality_preserves_reservations_without_debt() {
        unsafe fn release_nothing(_: *mut u8) {}
        for endpoint in 0..3 {
            std::thread::spawn(move || {
                let mut heap = Heap::for_isolate(false);
                heap.schedule = None;
                let h = heap.allocate(
                    crate::layout::cid::STRING_ACCUMULATOR,
                    1,
                    crate::layout::flags::RAW,
                );
                if endpoint == 1 {
                    // SAFETY: null is never dereferenced and release is a no-op.
                    unsafe { heap.attach(h, release_nothing, std::ptr::null_mut(), 7) };
                }
                assert_eq!(
                    heap.gc_policy_defer(Duration::from_secs(10), 1024 * 1024),
                    1
                );
                heap.refill_tlab(2);
                heap.refill_tlab(3);
                assert!(has_reservation(2) && has_reservation(3));
                heap.young_limit = heap.young_bytes;
                heap.byte_threshold = heap.stats.estimated_bytes;
                heap.byte_limit = heap.stats.estimated_bytes;
                let before = (
                    heap.stats.estimated_bytes,
                    heap.allocations,
                    heap.stats.collections,
                    heap.gc_policy.window,
                );
                match endpoint {
                    0 => {
                        // SAFETY: null is never dereferenced and release is a no-op.
                        unsafe { heap.attach(h, release_nothing, std::ptr::null_mut(), 0) };
                    }
                    1 => heap.adjust_attachment(h, 7),
                    _ => heap.count_external(0),
                }
                assert_eq!(
                    (
                        heap.stats.estimated_bytes,
                        heap.allocations,
                        heap.stats.collections,
                        heap.gc_policy.window
                    ),
                    before
                );
                assert!(!heap.gc_policy.debt);
                assert!(has_reservation(2) && has_reservation(3));
                let _ = heap.live_block(h);
                heap.return_tlabs();
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn idle_collection_requires_request_and_waits_for_cancel_or_expiry() {
        let mut heap = Heap::new(false);
        heap.gc_disabled = true;
        assert!(!heap.gc_policy_collect_if_idle());
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1000), 1);
        heap.gc_policy_request_idle();
        assert!(heap.gc_policy_idle_deadline().is_some());
        assert!(!heap.gc_policy_collect_if_idle());
        heap.gc_policy_cancel();
        assert!(heap.gc_policy_collect_if_idle());
        assert!(!heap.gc_policy_collect_if_idle());
        assert_eq!(heap.gc_policy_defer(Duration::from_secs(10), 1000), 1);
        heap.gc_policy_request_idle();
        heap.gc_policy.window = Some((Instant::now() - Duration::from_secs(1), 1000));
        assert!(heap.gc_policy_collect_if_idle());
    }

    #[test]
    fn idle_collection_keeps_ready_finalizers_and_arc_roots_alive() {
        for arc in [false, true] {
            let mut heap = Heap::new(false);
            if arc {
                heap.enable_arc();
            }
            let frame = heap.push_frame_with_slots(1);
            let owner = heap.allocate_object(305, 0);
            heap.set_root(frame, 0, owner);
            let value = heap.allocate_object(306, 0);
            let action = heap.allocate_object(307, 0);
            heap.add_attachment(FinalizerAttachment {
                owner,
                value,
                detach: 0,
                action: FinalizerAction::Dart(action),
            });
            heap.gc_policy_request_idle();
            assert!(heap.gc_policy_collect_if_idle());
            assert!(heap.object(owner).is_some());
            assert_eq!(heap.ready_finalizations.front().copied(), Some(action));
            assert!(heap.object(action).is_some());
        }
    }
}

/// The real ARC's prompt death: an object dies at the release of its last
/// counted owner — a code token, a field, a list slot — with no collection in
/// between; its weak references read null and its finalizer runs at once.
#[cfg(test)]
mod prompt_death_tests {
    use super::*;
    use crate::layout::{self, cid, flags};

    fn heap_prompt() -> Heap {
        let mut heap = Heap::new(false);
        heap.verify = true;
        heap.enable_arc();
        heap.enable_prompt_death();
        assert!(heap.prompt_death());
        heap
    }

    /// An instance of `fields` fields with one code token, as the prepared factory
    /// returns it.
    fn owned(heap: &mut Heap, fields: usize) -> Ref {
        let h = heap.allocate_instance(1000, fields);
        heap.retain_code_owner(h);
        h
    }

    fn dead(heap: &Heap, h: Ref) -> bool {
        heap.prompt_dead.contains(&h) && heap.arc.as_ref().unwrap().state.meta(h).is_none()
    }

    /// The code's release: what the extern `dartforge_arc_release` does.
    fn release(heap: &mut Heap, h: Ref) {
        heap.release_code_owner(h);
        heap.arc_settle();
    }

    #[test]
    fn last_token_release_kills_at_once_and_clears_the_weak_reference() {
        let mut heap = heap_prompt();
        let collections = heap.stats.collections;
        let leaf = owned(&mut heap, 1);
        let weak = owned(&mut heap, 1);
        heap.weak_refs.insert(weak, leaf);
        heap.retain_code_owner(leaf);
        release(&mut heap, leaf);
        assert!(!dead(&heap, leaf), "a second token still owns it");
        assert_eq!(heap.weak_refs[&weak], leaf);
        release(&mut heap, leaf);
        assert!(dead(&heap, leaf));
        assert_eq!(heap.weak_refs[&weak], 0);
        assert_eq!(heap.stats.collections, collections, "no collection ran");
        release(&mut heap, weak);
        assert!(dead(&heap, weak) && heap.weak_refs.is_empty());
    }

    #[test]
    fn field_and_list_slot_release_their_target_at_once_with_the_cascade() {
        let mut heap = heap_prompt();
        let collections = heap.stats.collections;
        let parent = owned(&mut heap, 1);
        let child = owned(&mut heap, 1);
        let leaf = owned(&mut heap, 1);
        heap.set_field(child, 0, leaf, true);
        heap.set_field(parent, 0, child, true);
        release(&mut heap, leaf);
        release(&mut heap, child);
        assert!(!dead(&heap, child) && !dead(&heap, leaf));
        // The field: overwriting it kills the child and, through it, the leaf.
        heap.set_field(parent, 0, 0, true);
        heap.arc_settle();
        assert!(dead(&heap, child) && dead(&heap, leaf));
        // The list slot.
        let list = heap.allocate(cid::LIST, layout::list_words(2), flags::REFS);
        heap.words_mut(list)[0] = 2;
        heap.arc_settle();
        heap.retain_code_owner(list);
        let element = owned(&mut heap, 0);
        heap.write_refs(list, 1, &[element]);
        release(&mut heap, element);
        assert!(!dead(&heap, element));
        heap.write_refs(list, 1, &[0]);
        heap.arc_settle();
        assert!(dead(&heap, element));
        // The list itself, with an element still in it.
        let other = owned(&mut heap, 0);
        heap.write_refs(list, 2, &[other]);
        release(&mut heap, other);
        release(&mut heap, list);
        assert!(dead(&heap, list) && dead(&heap, other));
        release(&mut heap, parent);
        assert!(dead(&heap, parent));
        assert_eq!(heap.stats.collections, collections, "no collection ran");
        // The next drain gives the memory back and keeps the counts consistent.
        heap.collect();
        assert!(heap.prompt_dead.is_empty());
    }

    static FINALIZED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn finalize(token: usize) {
        FINALIZED.store(token, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn finalizer_runs_at_the_last_release() {
        let mut heap = heap_prompt();
        let object = owned(&mut heap, 1);
        heap.finalizable.insert(object, (finalize, 77));
        heap.retain_code_owner(object);
        release(&mut heap, object);
        assert_eq!(FINALIZED.load(std::sync::atomic::Ordering::SeqCst), 0);
        release(&mut heap, object);
        assert_eq!(FINALIZED.load(std::sync::atomic::Ordering::SeqCst), 77);
        assert!(heap.finalizable.is_empty());
    }

    #[test]
    fn observational_frame_of_a_native_defers_the_death_until_it_closes() {
        let mut heap = heap_prompt();
        let object = owned(&mut heap, 1);
        let frame = heap.push_frame_with_slots(1);
        heap.set_root(frame, 0, object);
        release(&mut heap, object);
        assert!(!dead(&heap, object), "a native's frame still sees it");
        heap.pop_frame(frame);
        heap.arc_settle();
        assert!(dead(&heap, object));
    }

    #[test]
    fn cycle_waits_for_the_collector() {
        let mut heap = heap_prompt();
        let a = owned(&mut heap, 1);
        let b = owned(&mut heap, 1);
        heap.set_field(a, 0, b, true);
        heap.set_field(b, 0, a, true);
        release(&mut heap, a);
        release(&mut heap, b);
        assert!(
            !dead(&heap, a) && !dead(&heap, b),
            "a cycle keeps a positive count"
        );
        heap.collect();
        assert!(!heap.is_live_object(a) && !heap.is_live_object(b));
    }
}
