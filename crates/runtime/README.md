# Managed AOT runtime

The heap is a precise, non-generational, non-compacting tracing GC: it collects by iterative marking and sweeping, including unrooted cycles. A value in a `Ref` position is `0` (null), **even** (heap handle: `(index + 1) << 1`) or **odd** (`Smi`: a small `int` `(v << 1) | 1`, which does not allocate, is not a root and is never followed by the collector; outside the range it becomes `Value::BoxedInt`). The free list reuses slots; dead handles must not be used (there is no generation counter). The implementation offers neither a multithreaded ABI nor isolation between runtimes. Strings have UTF-16 code unit semantics (one-byte and two-byte forms, like the VM), not UTF-8.

## What is still missing (2026-09-23)

The current collector is correct for what it covers (explicit roots, precise tracing, cycles, `Smi`, heap ceiling, stress mode in CI), but it is still the minimum. In priority order:

**Correctness — required by Dart semantics**

1. **Weak references**: `WeakReference` and `Expando` (the latter with *ephemerons*: a dead key releases the value). They do not exist in the heap today.
2. **Finalizers**: `Finalizer` runs the callback as an event (the specification allows never running it, and today it never runs); `NativeFinalizer` has execution guarantees that `dart:ffi` depends on.
3. **Per-isolate heap** (decision in `docs/NATIVO-PLANO.md` §7.1, per the specification §6.3): today there is one global heap per process, with state in `thread_local!`, with no isolation and no multiple threads. `package:analyzer`, the builders and the macro executor need isolates.

**Performance**

4. **Young generation** (*scavenger*, like the VM): most Dart objects die young; today every collection walks the whole heap. Requires a write barrier.
5. **Compaction / moving objects**: the heap fragments and never returns memory — the sweep visits the historical maximum of slots and the capacity never shrinks.
6. **Access and allocation**: every field read goes through the handle table (indirection + bounds check) inside a `RefCell`; there is no *bump pointer* allocation. Safe, but slow on the hot path.
7. **Cost of roots** (step 6 of `docs/NATIVO-PLANO.md`): today one `set_root` per defined `Ref` value. Planned: root only what is live at a collection point; a root as a `store` into a frame on the stack (LLVM's `shadow-stack` strategy, not statepoints — see `docs/PESQUISA-LLVM-DART-AOT.md`); the extern effects table (`crates/emit_native/src/llvm/externs.rs`) to skip roots around calls that do not allocate.
8. **Pauses proportional to the heap**: there is no incremental or concurrent marking (relevant for 60 fps UI).

**Measurement**

9. **No comparison with the official VM** (allocation time, pauses, resident memory); the numbers below are internal microbenchmarks. Methodology to follow: `docs/PESQUISA-LLVM-DART-AOT.md` §7.

Proposed order: 1–2 (the SDK compiled from source already exercises them), 3, 9 (decide the rest by numbers), 6–7, then 4, 5 and 8. A future alternative memory management, off this track: `docs/EXPERIMENTO-ARC.md`.

Every emitted function that registers references opens a frame for parameters and SSA results. Functions without references omit the GC push/pop, avoiding that cost in purely scalar recursion. Roots stay until the function returns, so loop temporaries may stay alive for the whole function. This conservative duration retention does not turn scalars into roots: field tracing uses exact tags. Pop does not collect; the caller registers the returned reference immediately, before allocating again. int?/bool? fields use presence/payload scalar slots; references use one slot with is_ref=1.

Automatic collection happens before allocating, after a threshold of 256 allocations (adjusted by the number of live objects) or when the estimated-bytes threshold is exceeded (minimum 1 MiB; after a collection, twice the live bytes). DARTFORGE_GC_STRESS forces a collection before every allocation. The result of a new allocation must be rooted before the next one. dartforge_gc_collect allows an explicit collection.

## ABI

- dartforge_gc_push(frame:*mut RootFrame); dartforge_gc_desempilhar(frame); dartforge_gc_collect(). The frame `{ previous, n, [n x i64] }` lives on the stack of the generated function, which writes each root into its slot with a `store`.
- dartforge_object_new(class_id:i64, field_count:i64) -> i64; dartforge_object_get(handle:i64,index:i64) -> i64; dartforge_object_set(handle:i64,index:i64,bits:i64,is_ref:i8); dartforge_object_class(handle:i64) -> i64.
- dartforge_string_new(ptr:*const u8,len:i64) -> i64; dartforge_string_concat(a:i64,b:i64) -> i64; dartforge_string_equal(a:i64,b:i64) -> i8; dartforge_print_string(handle:i64).
- Strings are UTF-8 and immutable. Concatenation requires non-null operands; equality and printing accept a zero handle. There is no Unicode normalization.
- The only unsafe memory access is reading the foreign UTF-8 constant in string_new. The emitter guarantees a readable region of len bytes; the pointer may be ignored for len=0. The other heap operations are safe Rust and validate indices. Breaking the contract causes an explicit failure, not execution of Dart with defined behavior.

The driver embeds heap.rs and the runtime fragments (the list and the order live only in `FRAGMENTOS`, in build.rs — the single source also for the JIT's `abi` module and the symbol table) into a single standalone file. The workspace tests the same heap.rs module with unsafe forbidden. There is no dependency on the Dart VM, the Dart GC, a grow-only arena or the Swift runtime.

## References consulted

Dart SDK tag 3.6.2, revision b0cc5495e0f5e8ae150825a5352e708cb49e65ff: runtime/vm/heap/heap.h and runtime/vm/heap/marker.h. The latter describes marking of reachable objects and the roots of the mark-sweep collector. The Dart VM has generations/concurrency that this runtime does not implement yet. No code from these references was copied; the Rust implementation uses explicit roots and tags suited to the current LLVM emitter.

## Review and historical microbenchmark of the append ABI

Marking uses a per-slot bitmap and a reused work stack, with no per-object hash. The sweep visits the historical maximum of slots, including free slots; capacity is never reduced automatically. Frames are searched from the top down: the emitter's normal case finds the current frame immediately, but registering in an ancestor costs depth. Duplicate roots are still retained and processed; long functions and loops may consume memory linear in the number of registrations. This description records the previous append ABI; the fixed-slot increment below replaces it in emitted code.

Collecting in stress mode on every allocation with N retained references causes quadratic work; that mode checks correctness and does not represent a production configuration. In normal mode, the adaptive threshold avoids sweeping on every allocation. The HeapStats counters are cumulative: allocations, collections, reclaimed, roots_scanned, slots_scanned, and a live_objects/reserved_slots snapshot. They are not an exact measurement of resident bytes nor of the system allocator.

Reproduction: cargo test -p dartforge-runtime --release gc_microbenchmark -- --ignored --nocapture. Single measurement on Windows x64/rustc1.98.1, 2026-09-20, with no comparison against Dart or Swift:

| Configuration | Allocate+root+automatic GC | Collect live | Collect dead | Collections | Roots visited |
| --- | --- | --- | --- | --- | --- |
| 100,000 objects, normal | 7,814,400 ns | 2,102,900 ns | 6,436,200 ns | 8 | 193,184 |
| 2,000 objects, stress | 7,567,900 ns | 6,900 ns | 38,700 ns | 2,002 | 2,001,000 |

Both end with zero live objects; reserved slots remain reusable. The numbers are a local sample, subject to noise, with no promise of a performance advantage. Additional tests cover a 20,000-object cycle without recursion, object identity, handle transfer on return, Unicode/NUL and nullable comparison.

Swift revision 6e75592c4025239130e370877ab0fab4006b675b, docs/SIL/ARCOptimization.md and stdlib/public/runtime/HeapObject.cpp consulted: retain/release operations and ownership optimization belong to Swift's ARC model. Copying that model does not solve the strong cycles that Dart allows; here reachability tracing collects those cycles without requiring weak/unowned from the program. No Swift code was copied.

Contract correction: string_equal accepts zero in either operand (two zeros are equal); print_string(0) prints null. string_concat still requires non-null strings. When allocating a pre-filled Value::Object directly through the Rust API, its input references must be rooted before the collection that precedes the allocation, as the ABI requires. The emitter uses object_new with zeroed fields followed by object_set.



## Fixed slots and byte accounting

The current ABI receives slot_count when opening the frame, and set_root replaces an existing slot, including with zero. It does not resize the frame during loops; an invalid index is a contract error. The Rust API push_frame()/root() and the legacy gc_root symbol still exist for old regressions, but the emitter uses fixed slots. The runtime does not deduplicate roots from distinct slots: local copies are independent and must stay protected.

HeapStats includes live_roots/peak_roots (non-null slots), root_slots/peak_root_slots (reserved size of active frames), estimated_bytes/peak_estimated_bytes (size_of<Value> per live object plus String.capacity, or the capacity of the field vector multiplied by size_of<(i64,bool)>). It does not include the empty capacity of the handle table, the bitmap, the tracing stack, roots, allocator overhead or RSS. The count is subtracted when the payload is collected. Payload multiplication/addition uses checked arithmetic; thresholds saturate. Live objects may exceed the thresholds and are not discarded artificially.

DARTFORGE_GC_STATS=1 prints exactly one JSON object to stderr after dartforge_entry returns successfully; without the setting it prints no statistics. It does not force a final collection. The dartforge_gc key contains allocations, collections, reclaimed, live_objects, reserved_slots, root_slots, peak_root_slots, live_roots, peak_roots, live_bytes and peak_live_bytes. The two bytes fields expose the estimates above, not resident memory.

Reproduction: cargo test -p dartforge-runtime --release fixed_slot_microbenchmark -- --ignored --nocapture. Single local sample, Windows/rustc1.98.1 on 2026-09-20, 100,000 transient strings and one replaced root:

| Mode | Allocate+replace+automatic GC | Measured final collection | Collections | Reclaimed | Peak root slots | Peak estimated bytes |
| --- | --- | --- | --- | --- | --- | --- |
| Normal | 3,895,100 ns | 1,900 ns | 391 | 99,999 | 1 | 12,336 |
| Stress | 3,582,100 ns | 0 ns | 100,001 | 99,999 | 1 | 96 |

Zero ns is the observed clock resolution for a short interval, not an absence of work. The difference between the modes does not support a performance comparison: a short sample with no statistical repetition. Main evidence: a constant root peak and the collection of 99,999 objects; reserved object slots stay at 257 (normal) or 2 (stress) instead of growing with 100,000 iterations.

The Dart 3.6.2 and Swift references listed above motivate separating roots, identity and ownership; static slots are a protocol of our own between the LLVM emitter and the tracing runtime, not an adoption of Swift's ARC.

## Canonical enum values

`dartforge_enum_get` uses the key (nominal class ID, ordinal). The cache keeps each
singleton reachable until process shutdown and protects the name string while it is
being created. `permanent_roots` counts these roots separately from the frame slots;
`roots_scanned` includes both. Frames may end with zero roots while enums stay alive.
The byte estimate includes the managed payloads, but not the overhead of the
canonicalization HashMap.
