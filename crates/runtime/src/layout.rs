//! The layout contract of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md §2 and §3.1).
//!
//! Every numeric constant that generated code uses to read or write a heap block
//! comes from here: the runtime uses it as `crate::layout` and the emitter as
//! `dartforge_runtime::layout`. The module depends on no other (it is also compiled
//! inside `RUNTIME_MAIN`, the text that the AOT passes to the standalone `rustc`).
//!
//! A block is a 16-byte header ([`crate::layout::Header`]) followed by the body, in
//! 8-byte words. The handle of a block is `block + 2`
//! ([`crate::layout::HANDLE_OFFSET`]); a `Smi` is odd and null is `0`
//! ([`crate::layout::smi`]). The collector walks the body by the format in `flags`
//! ([`crate::layout::flags::FORMAT`]), without looking at the class.

/// A value in a reference position: `0` (null), a `Smi` (odd) or a block handle
/// (`block + 2`).
pub type Ref = i64;

/// The header of every block of the object space (16 bytes, fixed C layout: a
/// contract with the emitter).
///
/// * `state`: [`state::FREE`], [`state::YOUNG`], [`state::SURVIVOR`] or
///   [`state::SURVIVOR_B`], [`state::OLD`], [`state::REMEMBERED`] or
///   [`state::PERMANENT`];
/// * `flags`: the bits of [`flags`] (out-of-line body, body format, cards,
///   attachment, compact shape, external memory);
/// * `n`: for `INSTANCE`, the number of fields; for `RAW`/`REFS`, the words of
///   the body (saturated at `u16::MAX` for a large object: the region knows the
///   real value);
/// * `class_id`: the cid;
/// * `map`: for `INSTANCE`, the reference bits of the first 32 fields; for
///   strings, the hash (30 bits, 0 = not computed yet); for the others, 0;
/// * `metadata`: the RTI, `id + 1` (0 = none).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub state: u8,
    pub flags: u8,
    pub n: u16,
    pub class_id: i32,
    pub map: u32,
    pub metadata: u32,
}

const _: () = {
    assert!(std::mem::size_of::<Header>() == HEADER_SIZE);
    assert!(std::mem::align_of::<Header>() == 4);
    assert!(std::mem::offset_of!(Header, state) == offsets::STATE);
    assert!(std::mem::offset_of!(Header, flags) == offsets::FLAGS);
    assert!(std::mem::offset_of!(Header, n) == offsets::N);
    assert!(std::mem::offset_of!(Header, class_id) == offsets::CLASS);
    assert!(std::mem::offset_of!(Header, map) == offsets::MAP);
    assert!(std::mem::offset_of!(Header, metadata) == offsets::METADATA);
};

/// Header bytes.
pub const HEADER_SIZE: usize = 16;
/// The handle of a block is `block + HANDLE_OFFSET`.
pub const HANDLE_OFFSET: i64 = 2;
/// Size (and alignment) of a page of the object space.
pub const PAGE: usize = 64 * 1024;
/// The beginning of each page holds the mark bitmap (one bit per word of the page).
pub const PAGE_HEAD: usize = PAGE / 64;
/// Largest exact size class, in body words.
pub const LARGEST_CLASS: usize = 64;
/// The medium size classes, in body words: each one fills the page (`k` blocks of
/// `16 + 8w` bytes in the `PAGE - PAGE_HEAD` usable bytes; see §2.6).
pub const MEDIUM_CLASSES: [usize; 24] = [
    70, 82, 98, 110, 124, 142, 166, 199, 222, 250, 286, 334, 401, 446, 502, 574, 670, 804, 894,
    1006, 1150, 1342, 1610, 2014,
];
/// Largest body (in words) outside a large region.
pub const LARGEST_MEDIUM: usize = 2014;
/// How many size classes: the exact `1..=64` ones and the medium ones; index 0 is not used.
pub const N_CLASSES: usize = 1 + LARGEST_CLASS + MEDIUM_CLASSES.len();
/// Largest body (in words) with inline allocation through the TLAB (`tlab[w]`, `w` in
/// `1..=TLAB_N`).
pub const TLAB_N: usize = 16;
/// Largest field index that the emitter reads inline (above it, the call
/// `dartforge_object_get`/`_set`).
pub const INLINE_FIELDS: usize = 4096;
/// Elements per card of a large list (the VM's `kSlotsPerCardLog2 = 5`).
pub const ELEMENTS_PER_CARD: usize = 32;
/// Elements covered by one word of cards (64 cards).
pub const ELEMENTS_PER_CARD_WORD: usize = 2048;
/// The first cid of the classes that are not in [`cid::SDK_CLASSES`].
pub const FIRST_FREE_CID: i64 = 128;

const _: () = {
    assert!(N_CLASSES == 89);
    assert!(MEDIUM_CLASSES[MEDIUM_CLASSES.len() - 1] == LARGEST_MEDIUM);
    assert!(ELEMENTS_PER_CARD_WORD == 64 * ELEMENTS_PER_CARD);
    assert!(TLAB_N <= LARGEST_CLASS);
};

/// States of a block ([`Header::state`]).
pub mod state {
    /// Free block (on the free list or never used).
    pub const FREE: u8 = 0;
    /// Allocated since the last collection.
    pub const YOUNG: u8 = 1;
    /// Young that survived one minor collection (the aging of the generational
    /// collector): still young for the write barrier, promoted to [`OLD`] by the
    /// next collection that finds it alive. The survivors of consecutive minor
    /// collections alternate between this state and [`SURVIVOR_B`], so that the marking
    /// tells the survivor it promotes from the young object it keeps without a
    /// second pass over the headers.
    pub const SURVIVOR: u8 = 2;
    /// Survived the collections that promote (or a full one).
    pub const OLD: u8 = 3;
    /// An old block that received a reference to a young one after the last
    /// collection (the barrier put it on the remembered list).
    pub const REMEMBERED: u8 = 4;
    /// Static object, outside the heap (image section): never marked, swept,
    /// remembered or written.
    pub const PERMANENT: u8 = 5;
    /// The other survivor state (see [`SURVIVOR`]).
    pub const SURVIVOR_B: u8 = 6;

    /// Is a block in state `s` young for the write barrier ([`YOUNG`],
    /// [`SURVIVOR`] or [`SURVIVOR_B`])? The generated code's `@df.young_child`
    /// compiles the same test.
    #[inline(always)]
    pub const fn is_young(s: u8) -> bool {
        s.wrapping_sub(YOUNG) <= SURVIVOR - YOUNG || s == SURVIVOR_B
    }
}

/// Bits of [`Header::flags`].
pub mod flags {
    /// The fields (`INSTANCE`) live in an out-of-line body.
    pub const OUT_OF_LINE: u8 = 0x01;
    /// The mask of the body format.
    pub const FORMAT: u8 = 0x06;
    /// Fields with the reference map (the collector follows the set bits).
    pub const INSTANCE: u8 = 0x00;
    /// Bytes without references (the collector follows nothing).
    pub const RAW: u8 = 0x02;
    /// Word 0 raw; words 1.. are `Ref`s (the collector follows the non-null even ones).
    pub const REFS: u8 = 0x04;
    /// The large list has cards after the last element.
    pub const CARDS: u8 = 0x08;
    /// The body holds a native pointer (`b+16`) released when the owner dies.
    pub const ATTACHMENT: u8 = 0x10;
    /// The mask of the compact shape of a `RAW` `_List`/`_ImmutableList`.
    pub const ELEMENT: u8 = 0x60;
    /// `i64` elements.
    pub const ELEMENT_INT: u8 = 0x20;
    /// Elements with the bits of an `f64`.
    pub const ELEMENT_DOUBLE: u8 = 0x40;
    /// 0/1 elements.
    pub const ELEMENT_BOOL: u8 = 0x60;
    /// A typed list over outside memory (`asTypedList`).
    pub const EXTERNAL: u8 = 0x80;
}

/// The fixed cids (§2.4): the same in the runtime, the emitter, the JIT and every generation.
pub mod cid {
    pub const NULL: i32 = 1;
    pub const SMI: i32 = 2;
    pub const MINT: i32 = 3;
    pub const DOUBLE: i32 = 4;
    pub const BOOL: i32 = 5;
    pub const ONE_BYTE_STRING: i32 = 6;
    pub const TWO_BYTE_STRING: i32 = 7;
    pub const LIST: i32 = 8;
    pub const IMMUTABLE_LIST: i32 = 9;
    pub const GROWABLE_LIST: i32 = 10;
    pub const CLOSURE: i32 = 11;
    pub const RECORD: i32 = 12;
    pub const CONTEXT: i32 = 13;
    pub const CELL: i32 = 14;
    pub const STRING_ACCUMULATOR: i32 = 15;
    pub const REGEXP_PROGRAM: i32 = 16;
    pub const SEND_PORT: i32 = 17;
    pub const CAPABILITY: i32 = 18;
    pub const FLOAT32X4: i32 = 19;
    pub const INT32X4: i32 = 20;
    pub const FLOAT64X2: i32 = 21;
    /// `_Int8List` + t, for t = `TIPO_*` 0–13 of `typed_data.rs`.
    pub const FIRST_TYPED: i32 = 22;
    /// `_Int8ArrayView` + t.
    pub const FIRST_VIEW: i32 = 36;
    /// `_UnmodifiableInt8ArrayView` + t.
    pub const FIRST_UNMODIFIABLE_VIEW: i32 = 50;
    pub const BYTE_DATA_VIEW: i32 = 64;
    pub const UNMODIFIABLE_BYTE_DATA_VIEW: i32 = 65;
    /// How many typed list element types (`TYPE_INT8` … `TYPE_FLOAT64X2`).
    pub const ELEMENT_TYPES: i32 = 14;

    /// The library of the runtime's internal classes (with no Dart declaration): the
    /// cid stays reserved and the emitter's numbering does not find the class in the SDK.
    pub const INTERNAL: &str = "";

    /// `(cid, library, class)` of every class with a fixed cid, in cid order: the
    /// source of the emitter's numbering (`context.rs`) and of the ABI check
    /// (`dartforge_register_cids`). The library is the URI (`dart:core`), the key of
    /// the stable numbering; the runtime's internal ones have [`INTERNAL`].
    pub const SDK_CLASSES: &[(i32, &str, &str)] = &[
        (NULL, "dart:core", "Null"),
        (SMI, "dart:core", "_Smi"),
        (MINT, "dart:core", "_Mint"),
        (DOUBLE, "dart:core", "_Double"),
        (BOOL, "dart:core", "bool"),
        (ONE_BYTE_STRING, "dart:core", "_OneByteString"),
        (TWO_BYTE_STRING, "dart:core", "_TwoByteString"),
        (LIST, "dart:core", "_List"),
        (IMMUTABLE_LIST, "dart:core", "_ImmutableList"),
        (GROWABLE_LIST, "dart:core", "_GrowableList"),
        (CLOSURE, "dart:core", "_Closure"),
        (RECORD, "dart:core", "_Record"),
        (CONTEXT, INTERNAL, "_Contexto"),
        (CELL, INTERNAL, "_Celula"),
        (STRING_ACCUMULATOR, INTERNAL, "_AcumuladorDeTexto"),
        (REGEXP_PROGRAM, INTERNAL, "_ProgramaDeRegExp"),
        (SEND_PORT, "dart:isolate", "_SendPort"),
        (CAPABILITY, "dart:isolate", "_Capability"),
        (FLOAT32X4, "dart:typed_data", "_Float32x4"),
        (INT32X4, "dart:typed_data", "_Int32x4"),
        (FLOAT64X2, "dart:typed_data", "_Float64x2"),
        (22, "dart:typed_data", "_Int8List"),
        (23, "dart:typed_data", "_Uint8List"),
        (24, "dart:typed_data", "_Uint8ClampedList"),
        (25, "dart:typed_data", "_Int16List"),
        (26, "dart:typed_data", "_Uint16List"),
        (27, "dart:typed_data", "_Int32List"),
        (28, "dart:typed_data", "_Uint32List"),
        (29, "dart:typed_data", "_Int64List"),
        (30, "dart:typed_data", "_Uint64List"),
        (31, "dart:typed_data", "_Float32List"),
        (32, "dart:typed_data", "_Float64List"),
        (33, "dart:typed_data", "_Float32x4List"),
        (34, "dart:typed_data", "_Int32x4List"),
        (35, "dart:typed_data", "_Float64x2List"),
        (36, "dart:typed_data", "_Int8ArrayView"),
        (37, "dart:typed_data", "_Uint8ArrayView"),
        (38, "dart:typed_data", "_Uint8ClampedArrayView"),
        (39, "dart:typed_data", "_Int16ArrayView"),
        (40, "dart:typed_data", "_Uint16ArrayView"),
        (41, "dart:typed_data", "_Int32ArrayView"),
        (42, "dart:typed_data", "_Uint32ArrayView"),
        (43, "dart:typed_data", "_Int64ArrayView"),
        (44, "dart:typed_data", "_Uint64ArrayView"),
        (45, "dart:typed_data", "_Float32ArrayView"),
        (46, "dart:typed_data", "_Float64ArrayView"),
        (47, "dart:typed_data", "_Float32x4ArrayView"),
        (48, "dart:typed_data", "_Int32x4ArrayView"),
        (49, "dart:typed_data", "_Float64x2ArrayView"),
        (50, "dart:typed_data", "_UnmodifiableInt8ArrayView"),
        (51, "dart:typed_data", "_UnmodifiableUint8ArrayView"),
        (52, "dart:typed_data", "_UnmodifiableUint8ClampedArrayView"),
        (53, "dart:typed_data", "_UnmodifiableInt16ArrayView"),
        (54, "dart:typed_data", "_UnmodifiableUint16ArrayView"),
        (55, "dart:typed_data", "_UnmodifiableInt32ArrayView"),
        (56, "dart:typed_data", "_UnmodifiableUint32ArrayView"),
        (57, "dart:typed_data", "_UnmodifiableInt64ArrayView"),
        (58, "dart:typed_data", "_UnmodifiableUint64ArrayView"),
        (59, "dart:typed_data", "_UnmodifiableFloat32ArrayView"),
        (60, "dart:typed_data", "_UnmodifiableFloat64ArrayView"),
        (61, "dart:typed_data", "_UnmodifiableFloat32x4ArrayView"),
        (62, "dart:typed_data", "_UnmodifiableInt32x4ArrayView"),
        (63, "dart:typed_data", "_UnmodifiableFloat64x2ArrayView"),
        (BYTE_DATA_VIEW, "dart:typed_data", "_ByteDataView"),
        (
            UNMODIFIABLE_BYTE_DATA_VIEW,
            "dart:typed_data",
            "_UnmodifiableByteDataView",
        ),
    ];

    /// The cid of the internal typed list of type `ty` (`TIPO_*`).
    pub const fn typed(ty: u8) -> i32 {
        FIRST_TYPED + ty as i32
    }

    /// The cid of the view of type `ty`, modifiable or not.
    pub const fn view(ty: u8, unmodifiable: bool) -> i32 {
        (if unmodifiable {
            FIRST_UNMODIFIABLE_VIEW
        } else {
            FIRST_VIEW
        }) + ty as i32
    }

    /// `_OneByteString` or `_TwoByteString`.
    pub const fn is_string(c: i32) -> bool {
        (c.wrapping_sub(ONE_BYTE_STRING) as u32) < 2
    }

    /// `_List` or `_ImmutableList`.
    pub const fn is_fixed_list(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 2
    }

    /// Core list: `_List`, `_ImmutableList` or `_GrowableList`.
    pub const fn is_list(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 3
    }

    /// Internal typed list (`_Int8List` … `_Float64x2List`).
    pub const fn is_internal_typed(c: i32) -> bool {
        (c.wrapping_sub(FIRST_TYPED) as u32) < ELEMENT_TYPES as u32
    }

    /// Any typed list or view (22..=65).
    pub const fn is_typed(c: i32) -> bool {
        (c.wrapping_sub(FIRST_TYPED) as u32) < 44
    }

    /// Unmodifiable view (`_UnmodifiableXArrayView`, `_UnmodifiableByteDataView`).
    pub const fn is_unmodifiable_view(c: i32) -> bool {
        (c.wrapping_sub(FIRST_UNMODIFIABLE_VIEW) as u32) < ELEMENT_TYPES as u32
            || c == UNMODIFIABLE_BYTE_DATA_VIEW
    }

    /// SIMD value (`_Float32x4`, `_Int32x4`, `_Float64x2`).
    pub const fn is_simd(c: i32) -> bool {
        (c.wrapping_sub(FLOAT32X4) as u32) < 3
    }
}

/// Offsets from the block (the handle is block + 2: the word at `b + k` is at
/// `h + k - 2`).
pub mod offsets {
    pub const STATE: usize = 0;
    pub const FLAGS: usize = 1;
    pub const N: usize = 2;
    pub const CLASS: usize = 4;
    pub const MAP: usize = 8;
    /// The hash of strings lives in the place of the map.
    pub const STRING_HASH: usize = 8;
    pub const METADATA: usize = 12;
    /// The first word of the body.
    pub const BODY: usize = 16;
    /// Strings, `_List`, `_ImmutableList`, `_GrowableList`, typed lists and views.
    pub const LENGTH: usize = 16;
    /// The code units of a string.
    pub const UNITS: usize = 24;
    /// The first element of a `_List`/`_ImmutableList`.
    pub const ELEMENTS: usize = 24;
    /// The value of `_Mint`, `_Double`, `bool`, and the SIMD lanes.
    pub const VALUE: usize = 16;
    /// The address of the bytes of a typed list (internal, external or view).
    pub const DATA: usize = 24;
    /// The bytes of an internal typed list.
    pub const INTERNAL_BYTES: usize = 32;
    pub const VIEW_BASE: usize = 32;
    pub const VIEW_OFFSET: usize = 40;
    /// The storage (`_List`) of a `_GrowableList`.
    pub const GROWABLE_DATA: usize = 24;
    pub const CLOSURE_CODE: usize = 16;
    pub const CLOSURE_CONTEXT: usize = 24;
    pub const CLOSURE_TYPED: usize = 32;
    pub const CLOSURE_ABI: usize = 40;
    pub const RECORD_SHAPE: usize = 16;
    pub const RECORD_FIELDS: usize = 24;
    /// The native pointer of a block with `ATTACHMENT`.
    pub const ATTACHMENT: usize = 16;
}

/// Offsets of the thread's `Context` (§3.4).
pub mod context {
    pub const PENDING: usize = 0;
    pub const TOP: usize = 8;
    pub const AREAS: usize = 16;
    pub const N_AREAS: usize = 24;
    /// The entries `AREAS` always points to: a module id below this indexes it
    /// without the `N_AREAS` bound, and the table never moves while the ids fit
    /// (the production `@df.get_area`, whose loads the optimizer may reuse).
    pub const AREA_CAPACITY: usize = 4096;
    pub const INTERRUPT: usize = 32;
    pub const EMPTIES: usize = 40;
    pub const REGISTERED: usize = 48;
    pub const N_REGISTERED: usize = 56;
    /// The cursor of the TLAB of `w` words at `TLAB + 16·w`, the end at
    /// `TLAB + 16·w + 8`, `w` in `1..=TLAB_N`.
    pub const TLAB: usize = 64;
    pub const SUBTYPES: usize = 336;
    pub const N_SUBTYPES: usize = 344;
    pub const SUBTYPES_WIDTH: usize = 352;
    /// The handles of the static `true` and `false` boxes of the thread's runtime
    /// (generated code reads them from here, not from the data symbols: between the
    /// executable and the SDK DLL only the active runtime counts, §4.10 item 9).
    pub const TRUE: usize = 360;
    pub const FALSE: usize = 368;
    /// The thread's native stack limit (`Context::stack_limit`).
    pub const STACK_LIMIT: usize = 376;
    /// The address of the thread's lookup tables (`Context::tables`,
    /// [`super::thread_tables`]).
    pub const TABLES: usize = 384;
    /// The real ARC's reuse lists (`Context::reuse`): for each size class `c` in
    /// `1..=REUSE_CLASSES`, at `REUSE + 8·c`, the first free cell of that class, its
    /// in-object word holding the next one (`super::arc_word`).
    pub const REUSE: usize = 392;
    /// The size classes with a reuse list: those of the TLAB's bodies plus the
    /// in-object word.
    pub const REUSE_CLASSES: usize = super::TLAB_N + 1;

    /// The bytes the generated code took from the reuse lists since the runtime last
    /// counted them (`Context::reuse_taken`).
    pub const REUSE_TAKEN: usize = REUSE + 8 * (REUSE_CLASSES + 1);

    /// The real ARC's candidate buffer of the generated code (`Context::candidates`):
    /// the handles of the objects its inline release made cycle candidates, how many,
    /// and the room; the runtime moves them into its candidate buffer.
    pub const CANDIDATES: usize = REUSE_TAKEN + 8;
    pub const N_CANDIDATES: usize = CANDIDATES + 8;
    pub const CANDIDATES_CAP: usize = N_CANDIDATES + 8;

    /// The head of the reuse list of class `c`.
    pub const fn reuse(c: usize) -> usize {
        REUSE + 8 * c
    }

    /// The cursor of the TLAB of `w` words.
    pub const fn tlab_cursor(w: usize) -> usize {
        TLAB + 16 * w
    }

    /// The end of the TLAB of `w` words.
    pub const fn tlab_end(w: usize) -> usize {
        TLAB + 16 * w + 8
    }

    const _: () = assert!(tlab_end(super::TLAB_N) + 8 == SUBTYPES);
}

/// Offsets in the thread's lookup tables (`heap::ThreadTables`), which the
/// generated code reaches through the pointer at [`context::TABLES`]. They are a
/// thread-local of their own, all zeros at the start, so that they take no room
/// in the executable's thread-local template (the `Context` has non-zero fields
/// and is copied whole to each new thread).
pub mod thread_tables {
    /// The type-check memo ([`super::rti_memo`]): [`super::rti_memo::SETS`] sets of
    /// [`super::rti_memo::WAYS`] entries of four words.
    pub const RTI_MEMO: usize = 0;
    /// The canonical one-character Latin-1 strings: 256 handles, 0 = not
    /// requested yet.
    pub const ONE_CHAR: usize = RTI_MEMO + super::rti_memo::N * super::rti_memo::ENTRY;
}

/// The thread's type-check memo (`ThreadTables::rti_memo`), shared by every use site
/// of `is T`, of `P<i>` and of the reified type of a new object: two-way sets of
/// entries `[a, b, kind, answer]`, the set a hash of the first three words (a
/// new entry takes way 0 and moves the old one to way 1). The
/// per-site caches of the generated code (two words each) hold one key, and the
/// generic code of `dart:async` sees several instantiations at the same site
/// (`_StreamController.add` of each stream of a request): there a site miss
/// probes this memo inline (`llvm/mod.rs`, `@df.rti_*_probe`) before entering
/// the runtime, which fills both. An all-zero entry never matches: `a` is a
/// value key (never 0) or a class id (positive).
///
/// * `is`: `a` the value key, `b` `T`, kind [`IS`], answer 0 or 1;
/// * `P<i>`: `a` the key of `this`, `b` the model, kind the class (≥ 0), answer
///   the type;
/// * the stored type: `a` the class id, `b` the requested type, kind [`SET`],
///   answer the header metadata.
pub mod rti_memo {
    /// Sets (a power of two).
    pub const SETS: usize = 512;
    /// Entries per set.
    pub const WAYS: usize = 2;
    /// Entries.
    pub const N: usize = SETS * WAYS;
    /// Bytes of an entry.
    pub const ENTRY: usize = 32;
    /// The kind of an `is` entry.
    pub const IS: i64 = -1;
    /// The kind of a stored-type entry.
    pub const SET: i64 = -2;
    pub const K1: u64 = 0x9E37_79B9_7F4A_7C15;
    pub const K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    pub const K3: u64 = 0x1656_67B1_9E37_79F9;
    /// The shift that leaves the set in the top bits of the hash.
    pub const SHIFT: u32 = 64 - SETS.trailing_zeros();

    /// The set of `[a, b, kind]` (its first entry is `set * WAYS`); the
    /// generated code computes the same.
    #[inline(always)]
    pub fn set(a: i64, b: i64, kind: i64) -> usize {
        let h = (a as u64).wrapping_mul(K1)
            ^ (b as u64).wrapping_mul(K2)
            ^ (kind as u64).wrapping_mul(K3);
        (h >> SHIFT) as usize
    }
}

/// A `Ref` with a tagged `Smi` (R10, docs/NATIVO-PLANO.md §6.2 and §7.1).
///
/// A `Ref` in generated code is an `i64` with the tag in the low bit, like the VM's
/// `Smi` (`runtime/vm/object.h`; there `kSmiTag = 0` and a heap pointer has bit 1 —
/// here the bit is inverted so that `0` remains null):
///
/// * `0` — null;
/// * **even** and positive — a heap handle (`block + 2`, [`is_object`]);
/// * **odd** — a small `int`, `(v << 1) | 1`, for `v` in `[-2^62, 2^62)`: the
///   `Smi`. It does not allocate, is not a root, and the collector never follows it.
///
/// An `int` outside that range in a `Ref` position goes to the heap as a `_Mint`.
/// The form is canonical: a value that fits in a `Smi` is never boxed.
pub mod smi {
    /// Smallest and largest `int` that fit in a `Smi` (63 signed bits).
    pub const MIN: i64 = -(1 << 62);
    pub const MAX: i64 = (1 << 62) - 1;

    /// The `Ref` of an `int` that fits in a `Smi`.
    #[inline]
    pub fn of(v: i64) -> Option<i64> {
        (MIN..=MAX).contains(&v).then_some((v << 1) | 1)
    }

    /// Is the `Ref` a `Smi`?
    #[inline]
    pub fn is_smi(r: i64) -> bool {
        r & 1 == 1
    }

    /// The value of a `Smi` (arithmetic shift).
    #[inline]
    pub fn value(r: i64) -> i64 {
        r >> 1
    }

    /// Does the `Ref` point into the heap (it is neither null nor a `Smi`)?
    #[inline]
    pub fn e_handle(r: i64) -> bool {
        r != 0 && r & 1 == 0
    }
}

/// Is the `Ref` the handle of a block (`block + 2`, blocks aligned to 8)? A negative
/// value is never an object.
#[inline]
pub fn is_object(h: Ref) -> bool {
    h & (7 | i64::MIN) == 2
}

/// Extension words of the reference map for `n` fields (the first 32 bits are in the
/// header).
#[inline]
pub const fn map_words(n: usize) -> usize {
    if n > 32 { (n - 32).div_ceil(64) } else { 0 }
}

/// Fields the block keeps: at least one (the place for the free-list link and for
/// the address of the out-of-line body).
#[inline]
pub const fn capacity(n: usize) -> usize {
    if n == 0 { 1 } else { n }
}

/// Body words of an `INSTANCE` with `n` fields.
#[inline]
pub const fn instance_words(n: usize) -> usize {
    capacity(n) + map_words(n)
}

/// Body words of a string of `len` units: the length and the units (1 or 2 bytes
/// each), padded with zeros up to the word.
#[inline]
pub const fn string_words(len: usize, two: bool) -> usize {
    1 + (len * if two { 2 } else { 1 }).div_ceil(8)
}

/// Does a list of `len` elements have cards (is it a large object)?
#[inline]
pub const fn has_cards(len: usize) -> bool {
    1 + len > LARGEST_MEDIUM
}

/// Card words after the last element of a large list.
#[inline]
pub const fn card_words(len: usize) -> usize {
    if has_cards(len) {
        len.div_ceil(ELEMENTS_PER_CARD_WORD)
    } else {
        0
    }
}

/// Body words of a `_List`/`_ImmutableList` of `len` elements: the length, the
/// elements and, if large, the cards.
#[inline]
pub const fn list_words(len: usize) -> usize {
    1 + len + card_words(len)
}

/// Body words of an internal typed list of `len` elements of `element_size`
/// bytes: the length, the data address and the bytes.
#[inline]
pub const fn typed_words(len: usize, element_size: usize) -> usize {
    2 + (len * element_size).div_ceil(8)
}

/// The size class of a body of `words` words: `1..=64` exact (0 words uses class 1),
/// `65..=88` medium; `None` above [`LARGEST_MEDIUM`] (large region).
#[inline]
pub const fn size_class(words: usize) -> Option<usize> {
    if words <= LARGEST_CLASS {
        return Some(if words == 0 { 1 } else { words });
    }
    let mut i = 0;
    while i < MEDIUM_CLASSES.len() {
        if words <= MEDIUM_CLASSES[i] {
            return Some(LARGEST_CLASS + 1 + i);
        }
        i += 1;
    }
    None
}

/// The body words of a block of class `class` (`1..N_CLASSES`).
#[inline]
pub const fn class_words(class: usize) -> usize {
    if class <= LARGEST_CLASS {
        class
    } else {
        MEDIUM_CLASSES[class - LARGEST_CLASS - 1]
    }
}

/// Bytes of a block with a body of `words` words.
#[inline]
pub const fn block_bytes(words: usize) -> usize {
    HEADER_SIZE + 8 * words
}

/// The real ARC's in-object count word (docs/ARC-IMPLEMENTACAO.md §9): under prompt
/// death (`dartforge_memory_arc_prompt_v1`) every block of the space reserves its last
/// word, `block + S - 8` with `S` the block size its page publishes at
/// [`arc_word::HEAD_SIZE`] (an allocation of `w` body words takes the size class of
/// `w + 1`). The count lives in the low [`arc_word::RC`] bits; the flags above bit 47,
/// so that a free cell can keep the address of the next one there (an address is
/// below 2⁴⁸ and reads as unregistered). Tracing and the nursery ARC keep the
/// 16-byte header and the class of `w`.
pub mod arc_word {
    /// The class ids below which a class can have a drop function (the runtime keeps
    /// them in a table indexed by class id); the classes above it, such as the
    /// `async` frames, die through the header's map.
    pub const DROP_CLASSES: usize = 1 << 16;
    /// The strong occurrences (owners plus edges).
    pub const RC: u64 = (1 << 40) - 1;
    /// Registered and alive: counted from now on. A zeroed word is a young object
    /// nobody counted yet.
    pub const LIVE: u64 = 1 << 48;
    /// Logically dead, the cell still out of reuse (a dead cycle candidate waits for
    /// the candidate buffer).
    pub const DEAD: u64 = 1 << 49;
    /// Inside the discard transaction of a set (the cycle round).
    pub const COLLECTING: u64 = 1 << 50;
    /// In the cycle candidate buffer.
    pub const CANDIDATE: u64 = 1 << 51;
    /// No reference position nor side edge: never a cycle candidate.
    pub const CYCLE_FREE: u64 = 1 << 52;
    /// Raw writes pending since the last settle point: counted writes go to the runtime.
    pub const SNAPSHOT: u64 = 1 << 53;
    /// Zero count held by an observational root, on the deferred list.
    pub const DEFERRED: u64 = 1 << 54;
    /// Side edges (a Dart finalizer's action, a counted ephemeron's value).
    pub const SIDE_EDGES: u64 = 1 << 55;
    /// Target or holder of a weak reference.
    pub const WEAK: u64 = 1 << 56;
    /// Value, detach key or owner of a finalizer attachment, or finalizable.
    pub const FINALIZER: u64 = 1 << 57;
    /// Holder or key of an ephemeron.
    pub const EPHEMERON: u64 = 1 << 58;
    /// Named by an identity index (`late` marks, permanents, constants, tear-offs).
    pub const INDEXED: u64 = 1 << 59;
    /// The flags whose death cleans side tables.
    pub const OBSERVED: u64 = SIDE_EDGES | WEAK | FINALIZER | EPHEMERON | INDEXED;
    /// The page (or large region) head word with the block size `S` for the
    /// generated code's inline counting: 0 when it must call the runtime (an
    /// unprepared page, or the audit's inventory).
    pub const HEAD_INLINE: usize = 0;
    /// The page head word with the block size `S`, for the runtime.
    pub const HEAD_SIZE: usize = 8;

    /// Bits 40 to 47 stay free: the count's overflow margin and the candidate
    /// pass's own mark (`ArcHeapState::purge_dead_candidates`).
    const _: () = assert!(RC < LIVE && OBSERVED & (LIVE | DEAD | RC) == 0);
}

/// Word 0 of the header, as the inline allocation writes it:
/// `state | flags << 8 | n << 16 | cid << 32`, with `n` saturated at `u16::MAX`
/// (for `INSTANCE`, `n` is the number of fields; for the others, the body words).
#[inline]
pub const fn header_word(state: u8, flags: u8, n: usize, cid: i32) -> u64 {
    let n = if n > u16::MAX as usize {
        u16::MAX as u64
    } else {
        n as u64
    };
    state as u64 | (flags as u64) << 8 | n << 16 | (cid as u32 as u64) << 32
}

/// The `hashCode` of a string: the VM's `StringHasher` (`runtime/vm/hash.h`,
/// `CombineHashes` per unit and `FinalizeHash` in 30 bits), with 0 replaced by 1. It
/// is what the runtime writes into `map` and what the emitter writes into static
/// literals.
pub fn string_hash(units: impl IntoIterator<Item = u16>) -> u32 {
    let mut h: u32 = 0;
    for u in units {
        h = h.wrapping_add(u32::from(u));
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h = h.wrapping_add(h << 15);
    h &= (1u32 << 30) - 1;
    if h == 0 { 1 } else { h }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn layout_size_class_is_monotonic_and_covers() {
        let mut previous = 0;
        for w in 0..=LARGEST_MEDIUM {
            let c = size_class(w).expect("body class up to the largest medium one");
            assert!(
                (1..N_CLASSES).contains(&c),
                "class {c} of {w} words out of range"
            );
            assert!(c >= previous, "size_class is not monotonic at {w}");
            assert!(class_words(c) >= w, "class {c} does not fit {w} words");
            // The smallest class that fits: the previous one does not fit.
            if c > 1 {
                assert!(
                    class_words(c - 1) < w.max(1),
                    "class {c} is not the smallest for {w}"
                );
            }
            previous = c;
        }
        assert_eq!(size_class(LARGEST_MEDIUM + 1), None);
        assert_eq!(size_class(0), Some(1));
        assert_eq!(size_class(64), Some(64));
        assert_eq!(size_class(65), Some(65));
        assert_eq!(class_words(N_CLASSES - 1), LARGEST_MEDIUM);
    }

    #[test]
    fn layout_medium_classes_fill_the_page() {
        let useful = PAGE - PAGE_HEAD;
        let blocks = [
            112, 96, 80, 72, 64, 56, 48, 40, 36, 32, 28, 24, 20, 18, 16, 14, 12, 10, 9, 8, 7, 6, 5,
            4,
        ];
        for (i, &w) in MEDIUM_CLASSES.iter().enumerate() {
            let k = useful / block_bytes(w);
            assert_eq!(k, blocks[i], "medium class of {w} words");
            // The largest that fits the same number of blocks: one more word loses one.
            assert!(
                useful / block_bytes(w + 1) < k,
                "the class of {w} words does not fill the page"
            );
        }
        // Maximum internal waste of 25% (between 1610 and 2014).
        let mut previous = LARGEST_CLASS;
        for &w in &MEDIUM_CLASSES {
            assert!(
                (w - previous - 1) * 4 <= w,
                "internal waste above 25% before {w}"
            );
            previous = w;
        }
    }

    #[test]
    fn layout_string_hash_equals_vm() {
        for s in [
            "",
            "a",
            "abc",
            "hello world",
            "ç€😀",
            "0123456789abcdef0123456789",
        ] {
            let t = crate::heap::Text::from_rust_str(s);
            let u: Vec<u16> = s.encode_utf16().collect();
            assert_eq!(
                i64::from(string_hash(u.iter().copied())),
                t.hash_vm(),
                "hash of {s:?}"
            );
        }
        // Lone surrogate and a unit above 0xFF.
        let u = vec![0xD800, 0x61, 0x1234, 0xFF];
        let t = crate::heap::Text::from_units(u.clone());
        assert_eq!(i64::from(string_hash(u)), t.hash_vm());
        assert_ne!(string_hash([]), 0);
        assert!(string_hash("xyz".encode_utf16()) < 1 << 30);
    }

    #[test]
    fn layout_header_word_and_handles() {
        let p = header_word(state::YOUNG, flags::RAW, 3, cid::ONE_BYTE_STRING);
        assert_eq!(p, 1 | 0x02 << 8 | 3 << 16 | 6 << 32);
        assert_eq!(
            header_word(state::PERMANENT, 0, 100_000, 200) >> 16 & 0xFFFF,
            0xFFFF
        );
        assert_eq!(header_word(0, 0, 0, -1) >> 32, 0xFFFF_FFFF);
        assert!(is_object(0x1000 + HANDLE_OFFSET));
        assert!(!is_object(0));
        assert!(!is_object(0x1000));
        assert!(!is_object(0x1004));
        assert!(!is_object(0x1006));
        assert!(!is_object(smi::of(7).unwrap()));
        assert!(!is_object(i64::MIN | 2));
        assert_eq!(smi::value(smi::of(-5).unwrap()), -5);
        assert_eq!(smi::of(smi::MAX + 1), None);
    }

    #[test]
    fn layout_body_sizes() {
        assert_eq!(string_words(0, false), 1);
        assert_eq!(string_words(8, false), 2);
        assert_eq!(string_words(9, false), 3);
        assert_eq!(string_words(4, true), 2);
        assert_eq!(string_words(5, true), 3);
        assert_eq!(list_words(0), 1);
        assert_eq!(list_words(LARGEST_MEDIUM - 1), LARGEST_MEDIUM);
        assert!(!has_cards(LARGEST_MEDIUM - 1));
        assert!(has_cards(LARGEST_MEDIUM));
        assert_eq!(list_words(LARGEST_MEDIUM), 1 + LARGEST_MEDIUM + 1);
        assert_eq!(list_words(4096), 1 + 4096 + 2);
        assert_eq!(typed_words(3, 1), 3);
        assert_eq!(typed_words(2, 8), 4);
        assert_eq!(instance_words(0), 1);
        assert_eq!(instance_words(33), 34);
    }

    #[test]
    fn layout_sdk_cids_in_order_and_predicates() {
        for (i, &(c, _, _)) in cid::SDK_CLASSES.iter().enumerate() {
            assert_eq!(c, i as i32 + 1, "DO_SDK out of order at position {i}");
            assert!(i64::from(c) < FIRST_FREE_CID);
        }
        assert_eq!(
            cid::SDK_CLASSES.len(),
            cid::UNMODIFIABLE_BYTE_DATA_VIEW as usize
        );
        assert_eq!(cid::typed(1), 23);
        assert_eq!(
            cid::SDK_CLASSES[cid::typed(13) as usize - 1].2,
            "_Float64x2List"
        );
        assert_eq!(
            cid::SDK_CLASSES[cid::view(1, false) as usize - 1].2,
            "_Uint8ArrayView"
        );
        assert_eq!(
            cid::SDK_CLASSES[cid::view(13, true) as usize - 1].2,
            "_UnmodifiableFloat64x2ArrayView"
        );
        for c in 0..200 {
            assert_eq!(cid::is_string(c), c == 6 || c == 7);
            assert_eq!(cid::is_fixed_list(c), c == 8 || c == 9);
            assert_eq!(cid::is_list(c), (8..=10).contains(&c));
            assert_eq!(cid::is_internal_typed(c), (22..=35).contains(&c));
            assert_eq!(cid::is_typed(c), (22..=65).contains(&c));
            assert_eq!(
                cid::is_unmodifiable_view(c),
                (50..=63).contains(&c) || c == 65
            );
            assert_eq!(cid::is_simd(c), (19..=21).contains(&c));
        }
    }
}
