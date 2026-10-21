// Native runtime, core: the `main` entry, per-thread state (heap, class names and
// subtypes), objects, equality, boxes and records. The fragments
// `crates/runtime/src/*.rs` listed in `FRAGMENTS` (`crates/runtime/build.rs`) are
// concatenated into a single Rust program; this is the first one.

// Standalone harness: managed handles and a C ABI with explicit roots.

// SAFETY: the emitter defines this entry with the C signature void(void).
//
// Only in the AOT executable. When this file is compiled as a module of the
// `dartforge-runtime` crate (cfg `dartforge_runtime_embedded`, set by build.rs),
// the JIT is what calls the entry, and a C `main` would clash with the Rust binary's.
#[cfg(not(any(dartforge_runtime_embedded, dartforge_runtime_dll)))]
unsafe extern "C" {
    fn dartforge_entry();
}

/// Invokes once the program linked against the Rust runtime.
#[cfg(not(any(dartforge_runtime_embedded, dartforge_runtime_dll, target_arch = "wasm32")))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    // The context of the main thread before any generated prologue (see
    // `dartforge_init`).
    dartforge_context();
    // SAFETY: the object was emitted for this ABI and linked by the same native driver.
    unsafe { dartforge_entry() };
    let code = finalize_program();
    if code != 0 {
        std::process::exit(code);
    }
    0
}

/// The program entry with the SDK from source (P5c): the runtime and the SDK live
/// in a DLL (cfg `dartforge_runtime_dll`, without the C `main`), and the
/// executable's `main` — which the emitter writes — calls this function with its
/// `dartforge_entry`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_init(input: extern "C" fn(), to_string_fn: extern "C" fn(i64) -> i64) -> i32 {
    // The context of the main thread, before any generated prologue: the
    // production prologue on macOS arm64 reads the `pthread` slot of the
    // context without a null check (`sdk_module::context_in_tsd`), so the key
    // is created here — or the process aborts — and this thread's slot is
    // filled (`gc_roots::publish_in_tsd`). Every other thread that runs Dart
    // code does the same at its start (`isolates::run_isolate`).
    // A WebAssembly module's runtime panic is not a Dart exception: its message
    // goes to the host's standard error before the trap (`panic = "abort"`).
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(|info| {
        eprintln!("dartforge runtime panic: {info}");
    }));
    dartforge_context();
    TO_STRING.with(|p| p.set(Some(to_string_fn)));
    // The main isolate uses the allocator's free lists (`allocator.rs`).
    enable_allocation_cache();
    if debug() {
        // Debugging: a runtime panic shows the stack of Dart functions
        // (`DARTFORGE_CALL_TRACE=1` at compile time).
        let pattern = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |i| {
            show_trace();
            pattern(i);
        }));
    }
    // The main isolate accepts requests at the safepoint (the JIT reload) while
    // the entry runs — `main` and the event loop.
    mark_main_isolate();
    input();
    // A WebAssembly module whose loop waits for a timer returns to the host
    // ("pending"); `dartforge_resume` finishes it (`wasm_entry.rs`).
    #[cfg(target_arch = "wasm32")]
    if event_loop_suspended() {
        return PENDING;
    }
    finish_program()
}

/// The end of the program after its last event: the exit code, or the end of
/// the process with it.
fn finish_program() -> i32 {
    unmark_main_isolate();
    let code = finalize_program();
    if code != 0 {
        crate::console::exit(code);
    }
    0
}

/// What happens after `dartforge_entry` returns, in BOTH profiles.
///
/// A pending exception: writes `Unhandled exception:`, the message and the stack
/// trace to stderr and returns 255, like the VM. Otherwise, with
/// `DARTFORGE_GC_STATS=1`, writes the collector statistics, and returns the global
/// exit code (`dart:io`'s `exitCode`; 0 without it). The caller ends the process
/// with the code (the `main` above, in AOT; the runner, in the JIT). It does not
/// call `exit` here: it is the only stretch of the runtime that both profiles
/// drive, and it is written only once.
pub fn finalize_program() -> i32 {
    // Retire on every return path, after finalizers and exception formatting.
    // The standalone profile does not run unmark_main_isolate().
    struct RetireCallbacks;
    impl Drop for RetireCallbacks {
        fn drop(&mut self) { retire_isolate_callbacks(); }
    }
    let _retire_callbacks = RetireCallbacks;
    // The main isolate has ended: the attached `NativeFinalizer`s run.
    shutdown_isolate_finalizers();
    // `Isolate.exit` on the main isolate: it ended, without an error.
    let pending = EXCEPTION.with(|slot| slot.borrow().is_some()) && !unwinding_now();
    if pending {
        // The stack trace before the error's `toString()`, which runs Dart code (a
        // `throw` handled in there would replace the stored stack trace).
        let trace = exception_trace_text();
        let (bits, tag) = EXCEPTION.with(|slot| {
            let value = slot.borrow().expect("exception checked above");
            (exception_bits(value), exception_tag(value))
        });
        HEAP.with(|heap| {
            let heap = heap.borrow();
            let detail = match tag {
                1 => format!("{bits}"),
                2 => format!("{}", bits != 0),
                _ => match TO_STRING.with(|p| p.get()) {
                    // SDK from source: the Dart `toString()` of the thrown object.
                    Some(f) => {
                        drop(heap);
                        take_exception();
                        let t = with_roots(&[bits], || dart_r1(f as usize, bits));
                        let s = HEAP.with(|h| h.borrow().text(t).map(|text| text.to_dart_string()));
                        // `toString()` also failed: the runtime's description (the
                        // object's class).
                        s.unwrap_or_else(|| {
                            take_exception();
                            HEAP.with(|h| describe_handle(&h.borrow(), bits))
                        })
                    }
                    None => describe_handle(&heap, bits),
                },
            };
            // The VM's format and code (`Unhandled exception:`, the message, the
            // stack trace; 255). The stack trace is what the runtime has of the exception.
            use std::io::Write;
            let mut err = crate::console::stderr();
            let _ = writeln!(err, "Unhandled exception:\n{detail}");
            if !trace.is_empty() {
                let _ = write!(err, "{trace}");
                if !trace.ends_with('\n') {
                    let _ = writeln!(err);
                }
            }
        });
        return 255;
    }
    // `DARTFORGE_GC_STATS=1` writes the statistics; `=require` too, and returns 70
    // if the run did not exercise the collector: no collection, or an image with
    // roots by maps and no root read from a map
    // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5, the proof that it collected: a
    // test that never collected at the dangerous point must not pass).
    let statistics = crate::console::env_var("DARTFORGE_GC_STATS").unwrap_or_default();
    let mut nothing_exercised = false;
    if statistics == "1" || statistics == "require" {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            let (frames, map_roots, checked) = crate::heap::walk_counts();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"permanent_roots\":{},\"smi_caixas_evitadas\":{},\"map_frames\":{},\"map_roots\":{},\"checked_roots\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes, s.permanent_roots, s.boxes_avoided,
                frames, map_roots, checked);
            nothing_exercised = s.collections == 0 || (crate::heap::has_stack_maps() && map_roots == 0);
        });
    }
    if statistics == "require" && nothing_exercised {
        eprintln!("dartforge: DARTFORGE_GC_STATS=require and the run did not exercise the collector (no collection, or no root read from a map)");
        return 70;
    }
    if crate::console::env_var("DARTFORGE_GC_MEMORY").is_ok_and(|v| !v.is_empty() && v != "0") {
        HEAP.with(|heap| eprint!("[memory] end of program\n{}", heap.borrow().memory_report()));
    }
    global_exit_code()
}

/// The global exit code (`Process::GlobalExitCode`), which the normal end of the
/// program returns: `dart:io`'s `exitCode` sets it (fragment `io_platform`); a
/// target without `dart:io` keeps 0.
static EXIT_CODE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The code the process ends with when the program finishes without an error.
fn global_exit_code() -> i32 {
    EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed) as i32
}

use crate::heap::{CONTEXT, Heap, Text, StringMut};
use std::cell::RefCell;
use crate::hash::{HashMap, HashSet};

thread_local! {
    /// SDK from source: the Dart `toString()` of a value (the program's
    /// `dartforge_dispatch_toString`), for the uncaught exception.
    static TO_STRING: std::cell::Cell<Option<extern "C" fn(i64) -> i64>> = const { std::cell::Cell::new(None) };
    static HEAP: RefCell<Heap> = RefCell::new(Heap::for_isolate(crate::console::env_var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::default());
    static SUBCLASSES: RefCell<HashMap<i64, Vec<i64>>> = RefCell::new(HashMap::default());
    /// The already computed answers of [`is_subclass`]: every type check goes
    /// through here (the covariance of each `Map.[]=`, for example), and the graph
    /// search allocated a set per query. Cleared on each subclass registration (the
    /// load and each reload).
    static SUBTYPE_COMPUTED: RefCell<HashMap<(i64, i64), u8>> = RefCell::new(HashMap::default());
    /// In front of [`SUBTYPE_COMPUTED`], a direct-access table (class, target,
    /// answer): the `v is String`/`is num`/`is Map` of
    /// `_JsonStringifier.writeJsonValue` and of the JSON bench's `somaValor` paid the
    /// tuple's hash and the map's `borrow` on every test. Cleared together with the
    /// map.
    static SUBTYPE_FAST: std::cell::Cell<[(i64, i64, u8); FAST_SUBTYPE_SLOTS]> =
        const { std::cell::Cell::new([(-1, -1, 0); FAST_SUBTYPE_SLOTS]) };
    /// The subtype bitmaps that generated code reads inline (`df.subclass`,
    /// `llvm/mod.rs`; [`crate::heap::Context::subtypes`]).
    static SUBTYPE_MAPS: RefCell<SubtypeMaps> = RefCell::new(SubtypeMaps::default());
}

/// For each target class `C` already queried, the bitmap of the classes `cid <: C`
/// (the VM's type testing stub, which answers `is C` without going to the runtime):
/// the `is C` of generated code reads one bit, without a call. The map of `C` is
/// built on the first query that goes to the runtime ([`dartforge_is_subclass`]),
/// through the inverse graph (the subclasses of `C`), and all are discarded on each
/// new subclass relation (the load of a module, a reload): no map answers for an
/// old graph.
#[derive(Default)]
struct SubtypeMaps {
    /// The maps, by target; `pointers` is what the [`crate::heap::Context`]
    /// publishes (null = not built).
    maps: Vec<Option<Box<[u64]>>>,
    pointers: Vec<*const u64>,
    /// The width of all maps in bits (a multiple of 64): above the largest id with a
    /// registered relation. 0 = to be computed.
    width: usize,
    /// The inverse graph (direct supertype → direct subtypes), of the current
    /// [`SUBCLASSES`]; empty = to be computed.
    inverse: HashMap<i64, Vec<i64>>,
}

/// The largest target with a bitmap (the class ids are dense and small).
const LARGEST_MAPPED_TARGET: i64 = 1 << 16;

impl SubtypeMaps {
    /// Discards all maps (the graph changed) and removes the pointers from the
    /// context.
    fn discard(&mut self) {
        crate::heap::CONTEXT.with(|c| {
            c.n_subtypes.set(0);
            c.subtype_width.set(0);
            c.subtypes.set(std::ptr::null());
        });
        self.maps.clear();
        self.pointers.clear();
        self.width = 0;
        self.inverse.clear();
    }
    /// Builds the map of target `c` (0 ≤ c < [`LARGEST_MAPPED_TARGET`]) and
    /// publishes it.
    ///
    /// Nothing that generated code may have read is reallocated or released here:
    /// the pointer table is born with a place for each class of the width (a target
    /// above it has no registered subtype, and stays in the runtime), and only
    /// [`SubtypeMaps::discard`] releases it — starting from
    /// `dartforge_register_subclass`, a call that LLVM treats as any write.
    /// `dartforge_is_subclass` is declared `memory(read)` (`llvm/externs.rs`): a
    /// loop may keep the table read before it.
    fn build(&mut self, c: i64) {
        let target = c as usize;
        if self.maps.get(target).is_some_and(Option::is_some) {
            return;
        }
        if self.width == 0 {
            SUBCLASSES.with(|m| {
                let m = m.borrow();
                // Only the dense ids count for the width: there are ids outside the
                // range (the runtime's `_Type` class, `0x3FFF_FF01`), which stay in
                // the runtime.
                let mut larger = 0i64;
                let dense = |id: i64| if (0..LARGEST_MAPPED_TARGET).contains(&id) { id } else { 0 };
                for (&sub, supers) in m.iter() {
                    larger = larger.max(dense(sub));
                    for &s in supers {
                        larger = larger.max(dense(s));
                        self.inverse.entry(s).or_default().push(sub);
                    }
                }
                self.width = (usize::try_from(larger).unwrap_or(0) + 1).div_ceil(64) * 64;
            });
            self.maps = (0..self.width).map(|_| None).collect();
            self.pointers = vec![std::ptr::null(); self.width];
            let (p, n, l) = (self.pointers.as_ptr(), self.pointers.len(), self.width);
            crate::heap::CONTEXT.with(|ctx| {
                if !crate::heap::inline_class() {
                    return;
                }
                ctx.subtypes.set(p);
                ctx.n_subtypes.set(n);
                ctx.subtype_width.set(l);
            });
        }
        if target >= self.pointers.len() {
            return;
        }
        let words = self.width / 64;
        let mut map = vec![0u64; words].into_boxed_slice();
        let mut enable = |cid: i64| {
            if let Ok(i) = usize::try_from(cid)
                && i < words * 64
            {
                map[i / 64] |= 1 << (i % 64);
            }
        };
        if c == 0 {
            // `Object` is a supertype of every class.
            map.fill(u64::MAX);
        } else {
            let mut seen_set = HashSet::default();
            let mut queue = vec![c];
            seen_set.insert(c);
            while let Some(x) = queue.pop() {
                enable(x);
                if let Some(subs) = self.inverse.get(&x) {
                    for &s in subs {
                        if seen_set.insert(s) {
                            queue.push(s);
                        }
                    }
                }
            }
        }
        self.pointers[target] = map.as_ptr();
        self.maps[target] = Some(map);
    }
}

/// Slots of [`SUBTYPE_FAST`] (a power of 2).
const FAST_SUBTYPE_SLOTS: usize = 512;

/// The slot of (class, target) in [`SUBTYPE_FAST`].
fn subtype_slot(class_id: i64, target_class: i64) -> usize {
    (class_id.wrapping_mul(0x9E37_79B9).wrapping_add(target_class) as usize) & (FAST_SUBTYPE_SLOTS - 1)
}

/// The heap for the fast-path functions that the emitter declares with restricted
/// effects (`memory(none)`, `memory(inaccessiblemem: read)`, `llvm/externs.rs`):
/// without `RefCell::borrow`, which would write the borrow counter — a write those
/// declarations do not allow, even if undone at the end. The users only read the
/// heap (the `&mut` serves to take from a vector the mutable pointer of the
/// elements, without writing anything).
fn heap_without_borrow<R>(f: impl FnOnce(&mut Heap) -> R) -> R {
    HEAP.with(|h| {
        // SAFETY: only generated code calls these functions, and it never runs
        // inside a heap borrow (the runtime does not call Dart code while holding
        // one: that code's `borrow` would panic). The heap was already initialized
        // by the program's first access.
        f(unsafe { &mut *h.as_ptr() })
    })
}

/// Registers a class name by id for display in toString/print.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_class_name(class_id: i64, ptr: *const u8, len: i64) {
    let len = usize::try_from(len).expect("invalid length");
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).expect("UTF-8").to_string();
    CLASS_NAMES.with(|map| map.borrow_mut().insert(class_id, name));
}

/// Registers a direct subtyping relation: sub_id <: super_id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_subclass(sub_id: i64, super_id: i64) {
    SUBCLASSES.with(|map| {
        // Without duplicating: the publication of a reload redoes the registrations.
        let mut supers = map.borrow_mut();
        let list = supers.entry(sub_id).or_default();
        if !list.contains(&super_id) {
            list.push(super_id);
            SUBTYPE_COMPUTED.with(|c| c.borrow_mut().clear());
            SUBTYPE_FAST.with(|c| c.set([(-1, -1, 0); FAST_SUBTYPE_SLOTS]));
            SUBTYPE_MAPS.with(|m| m.borrow_mut().discard());
        }
    });
}

/// Queries nominal subtype membership at run time.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_is_subclass(class_id: i64, target_class: i64) -> u8 {
    let r = is_subclass(class_id, target_class);
    // The next `is` of this target comes from the bitmap, inline.
    if (0..LARGEST_MAPPED_TARGET).contains(&target_class) {
        SUBTYPE_MAPS.with(|m| {
            if let Ok(mut m) = m.try_borrow_mut() {
                m.build(target_class);
            }
        });
    }
    if debug() {
        eprintln!("[depurar] is_subclass({class_id}, {target_class}) = {r}");
    }
    r
}

fn is_subclass(class_id: i64, target_class: i64) -> u8 {
    if class_id == target_class {
        return 1;
    }
    if target_class == 0 {
        // Object is a supertype of every nominal class
        return 1;
    }
    let slot = subtype_slot(class_id, target_class);
    // SAFETY: the table is thread-local and no reference to it survives this read
    // (nor the write below) — the `Cell` only does not give access to one element
    // without copying the whole array.
    let (c, t, r) = SUBTYPE_FAST.with(|x| unsafe { (*x.as_ptr())[slot] });
    if c == class_id && t == target_class {
        return r;
    }
    let r = match SUBTYPE_COMPUTED.with(|c| c.borrow().get(&(class_id, target_class)).copied()) {
        Some(r) => r,
        None => {
            let r = subclass_by_graph(class_id, target_class);
            SUBTYPE_COMPUTED.with(|c| c.borrow_mut().insert((class_id, target_class), r));
            r
        }
    };
    // SAFETY: as in the read above.
    SUBTYPE_FAST.with(|x| unsafe { (*x.as_ptr())[slot] = (class_id, target_class, r) });
    r
}

/// The search of [`is_subclass`] in the graph of direct supertypes.
fn subclass_by_graph(class_id: i64, target_class: i64) -> u8 {
    SUBCLASSES.with(|map| {
        let map = map.borrow();
        let mut visited = crate::hash::HashSet::default();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(class_id);
        visited.insert(class_id);
        while let Some(curr) = queue.pop_front() {
            if curr == target_class {
                return 1;
            }
            if let Some(supers) = map.get(&curr) {
                for &s in supers {
                    if visited.insert(s) {
                        queue.push_back(s);
                    }
                }
            }
        }
        0
    })
}



/// Allocates an initially zeroed object, with fields still without references.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new(class_id: i64, field_count: i64) -> i64 {
    let n = usize::try_from(field_count).expect("invalid fields");
    let cid = i32::try_from(class_id).expect("invalid class");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate_instance(cid, n);
        // The next allocation of this size comes inline, from the TLAB (without
        // collecting here: `h` has no root yet).
        heap.refill_tlab(crate::layout::instance_words(n));
        h
    })
}

/// [`dartforge_object_new`] with the reference map of the first 32 fields
/// already written: bit `i` of `map` set when field `i` holds a reference — the
/// declared representation of the class, the same word the inline allocation
/// writes on its fast path (`llvm/mod.rs`, `emit_inline_allocation`; this is its
/// slow path, and the one of `AllocObject`). The stores into the fresh object
/// then skip the read-modify-write of a bit they agree with; a store that
/// disagrees still updates it. A set bit over a field still null (0) is what the
/// collector already sees in the objects the runtime creates (`Heap::new_object`
/// with null `Ref` fields): null is not a handle, and the marker skips it.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn dartforge_object_new_m(class_id: i64, field_count: i64, map: i64) -> i64 {
    let n = usize::try_from(field_count).expect("invalid fields");
    let cid = i32::try_from(class_id).expect("invalid class");
    let map = u32::try_from(map).expect("reference map beyond 32 bits");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate_instance(cid, n);
        // SAFETY: `h` is the handle (`block + HANDLE_OFFSET`) of the block this
        // thread has just allocated and nobody else references yet; its header
        // (`layout::Header`, `HEADER_SIZE` bytes, the `u32` map at
        // `offsets::MAP`, 4-byte aligned in a 16-byte aligned block) is inside
        // the block, and the heap borrow keeps the collector away.
        unsafe {
            let header = (h - crate::layout::HANDLE_OFFSET) as *mut u8;
            header.add(crate::layout::offsets::MAP).cast::<u32>().write(map);
        }
        heap.refill_tlab(crate::layout::instance_words(n));
        h
    })
}
// The fields of a user object are read and written inline by generated code
// (`llvm/mod.rs`, `GetField`/`SetField`): 8-byte words after the block's header
// (`heap::Header`), with the reference map in the header.

use crate::heap::{INLINE_FIELDS, EMPTY_OBJECT};

/// The address of the fields (8-byte words) of object `h`; for something that is
/// not an object, the zeroed fields of [`EMPTY_OBJECT`]. Generated code no longer
/// uses it (it reads the block inline); it remains for the ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_fields(h: i64) -> i64 {
    heap_without_borrow(|heap| heap.object_fields(h).map_or(EMPTY_OBJECT.fields.as_ptr() as i64, |p| p as i64))
}

/// Generated code's write barrier (`llvm/mod.rs`): the old object `h` received a
/// `Ref` in a field written inline, and the next minor collection must walk it
/// (`Heap::remember_object`). Does not allocate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_remember(h: i64) {
    heap_without_borrow(|heap| heap.remember_object(h));
}

/// Enables the ARC (`--memory=arc`, docs/ARC-IMPLEMENTACAO.md): the entry of a
/// program compiled that way calls this function before any Dart code. The suffix
/// is the version of the memory ABI (docs/ARC-CICLOS-ESPECIFICACAO.md §24): a
/// runtime without it does not link with the program.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_memory_arc_v1() {
    HEAP.with(|heap| heap.borrow_mut().enable_arc());
    ARC_MEMORY.with(|m| m.set(true));
}

thread_local! {
    /// The ARC is enabled on this isolate: the cheap question of the natives that
    /// write references without the `Heap` (`natives_hash.rs`, `natives_lists.rs`).
    static ARC_MEMORY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The ARC is enabled on this isolate.
#[inline(always)]
fn arc_enabled() -> bool {
    ARC_MEMORY.with(std::cell::Cell::get)
}

/// A native's raw write (without the `Heap`) replaced `old` with `new` in a strong
/// position of `h`: the ARC counts the swap (`Heap::arc_raw_write`). Only with
/// [`arc_enabled`]. Does not allocate.
#[cold]
fn arc_raw_write(h: i64, old: i64, new: i64) {
    heap_without_borrow(|heap| heap.arc_raw_write(h, old, new));
}

/// Under ARC, the write of `Ref` `v` into word `word ≥ 1` of the `REFS` body of `h`
/// (the list element and the `_GrowableList` storage that the emitter writes inline
/// under tracing): counts the swap and applies the barrier (`Heap::write_ref`).
/// Does not allocate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_write_ref(h: i64, word: i64, v: i64) {
    let word = usize::try_from(word).expect("negative word");
    HEAP.with(|heap| heap.borrow_mut().write_ref(h, word, v));
}

/// Under pure ARC, the references that generated code wrote inline into the
/// freshly allocated object `h` (`AllocObject`, the context, the cell and the
/// inline closure) start counting (`Heap::arc_count_initial`). Does not allocate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_initial(h: i64) {
    HEAP.with(|heap| heap.borrow_mut().arc_count_initial(h));
}

/// Gets the bits of a field by the stable index chosen by the emitter.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(o) = heap.object(handle) else {
            return 0;
        };
        let idx = usize::try_from(index).unwrap_or(usize::MAX);
        o.get(idx).map_or(0, |(bits, _)| bits)
    })
}
/// Writes a field and states explicitly whether its bits are a managed reference.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    HEAP.with(|heap| heap.borrow_mut().set(handle, index, bits, is_ref != 0));
}

/// The state of a `late` field without an initializer. The bit is independent of
/// the field's bits: `0`, `false` and `null` may be values already assigned.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initialized(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().late_initialized((handle, index))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_mark_initialized(handle: i64, index: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut().mark_late((handle, index));
    });
}

/// The index `-(index+2)` of the same side table represents an evaluation in
/// progress. `-1` is reserved for the state of locals captured in a `Cell`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initializing(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().late_initialized((handle, -index - 2))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_set_initializing(handle: i64, index: i64, active: u8) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let key = (handle, -index - 2);
        if active != 0 {
            heap.mark_late(key);
        } else {
            heap.unmark_late(key);
        }
    });
}

/// Compares equality (Dart's ==) between two reference handles.
///
/// Numbers compare by value, with `num`'s rule: `1 == 1.0` (R9); a `Smi` and a
/// `_Mint` never have the same value (the form is canonical, R10), but the
/// comparison is by value anyway. Strings, by their units.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_equal(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        use crate::heap::Value::{Bool, Double, Int, Ref};
        u8::from(match (heap.value(a), heap.value(b)) {
            (Int(x), Int(y)) => x == y,
            (Bool(x), Bool(y)) => x == y,
            (Double(x), Double(y)) => x == y,
            (Int(x), Double(y)) => (x as f64) == y,
            (Double(x), Int(y)) => x == (y as f64),
            (Ref(x), Ref(y)) => heap.is_string(x) && heap.is_string(y) && heap.strings_equal(x, y),
            _ => false,
        })
    })
}

/// `identical(a, b)` on references, with the VM's semantics
/// (`Instance::IsIdenticalTo`): the same handle, or two `_Mint`s with the same
/// value, or two bitwise-equal `_Double`s (R9; §2.10). It is the slow path of
/// `@df.identical_one`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_identical(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    HEAP.with(|heap| u8::from(heap.borrow().identical_one(a, b)))
}

/// `Box` (R3/R10): an `int` in a `Ref` position — the `Smi` when it fits in 63
/// bits (no allocation), otherwise the `_Mint` (the slow path of `@df.box_int`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().box_int(v))
}

/// Throws the `TypeError` of a failed implicit coercion (`null` or another type
/// where the scalar was expected).
fn throw_type_error() {
    let err = dartforge_type_error_new();
    dartforge_exception_throw(err, 3);
}

/// `Unbox` (R3/R10): the `int` of a reference (`Smi` or `_Mint`); null or another
/// type throws TypeError. The slow path of `@df.unbox_int`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_int(h: i64) -> i64 {
    if crate::heap::smi::is_smi(h) {
        return crate::heap::smi::value(h);
    }
    let v = HEAP.with(|heap| heap.borrow().int_of(h));
    v.unwrap_or_else(|| {
        throw_type_error();
        0
    })
}

/// `Unbox`: the `double` of a reference (a boxed `int` is not a `double`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_double(h: i64) -> f64 {
    let v = HEAP.with(|heap| heap.borrow().double_of(h));
    v.unwrap_or_else(|| {
        throw_type_error();
        0.0
    })
}

/// `Unbox`: the `bool` of a reference.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_bool(h: i64) -> u8 {
    let v = HEAP.with(|heap| heap.borrow().bool_of(h));
    v.map_or_else(
        || {
            throw_type_error();
            0
        },
        u8::from,
    )
}


/// The class of a value: the cid from an object's header, 1 (`Null`) for null and
/// 2 (`_Smi`) for a `Smi` (§2.4). Generated code reads it inline (`@df.class`);
/// this is the runtime's and that of `ClassID.getID`. Only reads the heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_value_class(handle: i64) -> i64 {
    heap_without_borrow(|heap| i64::from(heap.class(handle)))
}

/// A value of the `(bits, tag)` pair ABI (tags 1 int, 2 bool, 3 reference,
/// 4 double) as a [`crate::heap::Value`].
fn pair_value(bits: i64, tag: i64) -> crate::heap::Value {
    use crate::heap::Value;
    match tag {
        1 => Value::Int(bits),
        2 => Value::Bool(bits != 0),
        3 => Value::Ref(bits),
        4 => Value::Double(f64::from_bits(bits as u64)),
        _ => panic!("invalid value tag"),
    }
}

/// Creates a positional record from a flat array of (bits, tag) pairs: the slow
/// path of `AllocRecord` with an unboxed element, which boxes here with the right
/// roots.
///
/// # Safety
/// `pairs` points to `2 * len` readable words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("invalid length");
    let value_list: Vec<crate::heap::Value> = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: the function's contract.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2).map(|pair| pair_value(pair[0], pair[1])).collect()
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // Owning copies of the arguments and of the boxes last until the record
        // holds the edges; no collection happens after closing.
        let frame = heap.push_owning_frame(0);
        for v in &value_list {
            if let crate::heap::Value::Ref(r) = v {
                heap.root(frame, *r);
            }
        }
        let mut refs = Vec::with_capacity(len);
        for v in value_list {
            let r = heap.to_ref(v);
            heap.root(frame, r);
            refs.push(r);
        }
        let h = heap.new_record(&refs);
        heap.pop_frame(frame);
        h
    })
}

// ─── Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §3.6): P0a skeleton ───────

#[cfg(test)]
mod tests_metadata_registration {
    use super::*;

    #[test]
    fn names_preserve_utf8_and_do_not_allocate_dart_objects() {
        let cid = 1_900_003;
        let class_name = "Naïve;@\\\"";
        let argument = "df_test_argument_é;@\\\"";
        let before = HEAP.with(|heap| heap.borrow().stats().allocations);
        let previous_class = CLASS_NAMES.with(|names| names.borrow_mut().remove(&cid));
        let hash = name_hash(argument);
        let previous_argument = argument_names().write().unwrap().remove(&hash);
        // SAFETY: both UTF-8 slices outlive the calls and have exact byte lengths.
        unsafe {
            dartforge_register_class_name(cid, class_name.as_ptr(), class_name.len() as i64);
            dartforge_register_argument_name(argument.as_ptr(), argument.len() as i64);
        }
        assert_eq!(
            CLASS_NAMES
                .with(|names| names.borrow().get(&cid).cloned())
                .as_deref(),
            Some(class_name)
        );
        assert_eq!(
            argument_names()
                .read()
                .unwrap()
                .get(&hash)
                .map(String::as_str),
            Some(argument)
        );
        assert_eq!(HEAP.with(|heap| heap.borrow().stats().allocations), before);
        CLASS_NAMES.with(|names| {
            let mut names = names.borrow_mut();
            names.remove(&cid);
            if let Some(previous) = previous_class {
                names.insert(cid, previous);
            }
        });
        let mut names = argument_names().write().unwrap();
        names.remove(&hash);
        if let Some(previous) = previous_argument {
            names.insert(hash, previous);
        }
    }
}

#[cfg(test)]
mod tests_owners_record_and_concat {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn record_getter_borrows_field_without_creating_independent_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let text = HEAP.with(|h| h.borrow_mut().allocate_str("borrowed field"));
            dartforge_arc_retain(text);
            let pair_list = [text, 3];
            // SAFETY: a valid bits/tag pair; its Ref has an independent token.
            let record = unsafe { dartforge_record_new(pair_list.as_ptr(), 1) };
            dartforge_arc_retain(record);
            dartforge_arc_release(text);
            assert_eq!(dartforge_native_DartForge_record_numFields(record), 1);
            assert_eq!(dartforge_native_DartForge_record_shape(record), 1);
            assert_eq!(dartforge_native_DartForge_record_numFields(0), 0);
            assert_eq!(dartforge_native_DartForge_record_shape(0), 0);
            let field = dartforge_native_DartForge_record_fieldAt(record, 0);
            assert_eq!(field, text);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(field)));
            dartforge_arc_release(record);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(field)));
            assert!(!HEAP.with(|h| h.borrow().is_live_object(record)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    fn with_arc_heap(f: impl FnOnce()) {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        f();
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    #[allow(unsafe_code)]
    fn abi_record_preserves_aliases_and_boxes_until_publish() {
        with_arc_heap(|| {
            let (frame, text) = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let frame = h.push_owning_frame(1);
                let text = h.allocate_str("record");
                h.set_root(frame, 0, text);
                (frame, text)
            });
            let pair_list = [text, 3, i64::MAX, 1, text, 3, 2.5f64.to_bits() as i64, 4];
            // SAFETY: four valid bits/tag pairs, alive during the call.
            let record = unsafe { dartforge_record_new(pair_list.as_ptr(), 4) };
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.set_root(frame, 0, record);
                h.collect();
                let fields = h.record(record).unwrap().to_vec();
                assert_eq!(fields[0], text);
                assert_eq!(fields[2], text);
                assert!(matches!(h.value(fields[1]), crate::heap::Value::Int(i64::MAX)));
                assert!(matches!(h.value(fields[3]), crate::heap::Value::Double(x) if x == 2.5));
                h.pop_frame(frame);
                h.collect();
                assert!(!h.is_live_object(record));
                assert!(fields.iter().all(|&r| !h.is_live_object(r)));
            });
        });
    }

    #[test]
    fn concatenating_distinct_forms_preserves_boxes_and_releases_inputs() {
        with_arc_heap(|| {
            let (frame, a, b) = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let frame = h.push_owning_frame(2);
                let a = h.new_list(crate::layout::cid::LIST, 1, crate::lists::Element::Int);
                h.set_root(frame, 0, a);
                h.list_set(a, 0, crate::heap::Value::Int(i64::MAX));
                let b = h.new_list(crate::layout::cid::LIST, 1, crate::lists::Element::Double);
                h.set_root(frame, 1, b);
                h.list_set(b, 0, crate::heap::Value::Double(2.5));
                (frame, a, b)
            });
            let out = concat_lists(a, b).unwrap();
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.set_root(frame, 0, out);
                h.set_root(frame, 1, 0);
                h.collect();
                assert!(!h.is_live_object(a) && !h.is_live_object(b));
                let data = h.list_data(out);
                let boxes = h.words(data)[1..3].to_vec();
                assert!(matches!(h.value(boxes[0]), crate::heap::Value::Int(i64::MAX)));
                assert!(matches!(h.value(boxes[1]), crate::heap::Value::Double(x) if x == 2.5));
                h.pop_frame(frame);
                h.collect();
                assert!(!h.is_live_object(out) && !h.is_live_object(data));
                assert!(boxes.iter().all(|&r| !h.is_live_object(r)));
            });
        });
    }
}

/// A positional record (`_Record`, `REFS`) with the `n` `Ref`s in `refs`: the path
/// of an `AllocRecord` too large for the TLAB. Does not throw.
///
/// # Safety
/// `refs` points to `n` readable words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_from_refs(refs: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("invalid length");
    let fields = if n == 0 {
        &[][..]
    } else {
        // SAFETY: the function's contract.
        unsafe { std::slice::from_raw_parts(refs, n) }
    };
    // `new_record` roots the fields during the allocation.
    HEAP.with(|heap| heap.borrow_mut().new_record(fields))
}

/// [`dartforge_record_from_refs`] under ARC, for every record literal: the borrowed
/// `Ref`s gain the record's edges and the record a code owner before another
/// collection. Neither throws nor runs Dart.
///
/// # Safety
/// `refs` points to `n` readable words with live references (or null).
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_box_int_owned_v1(i64::MAX);
/// // SAFETY: one field alive during creation.
/// let r = unsafe { dartforge_arc_record_from_refs_owned_v1([a].as_ptr(), 1) };
/// dartforge_arc_release(a);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(a), 1);
/// dartforge_arc_release(r);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(r), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_record_from_refs_owned_v1(refs: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("invalid length");
    let fields = if n == 0 {
        &[][..]
    } else {
        // SAFETY: the function's contract.
        unsafe { std::slice::from_raw_parts(refs, n) }
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let r = heap.new_counted_record(fields);
        heap.retain_code_owner(r);
        r
    })
}

// ─── Identity, closure and record natives (moved from `natives_lists.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9) ────────────────────────────────────────

/// `ClassID.getID(o)`: the value's cid (`ClassID_getID`; the emitter does it inline
/// through `@df.class`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ClassID_getID(o: i64) -> i64 {
    dartforge_value_class(o)
}

/// `identical(a, b)` (`Identical_comparison`; lowering does it inline through
/// `@df.identical_one`, `lower/boxes.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Identical_comparison(a: i64, b: i64) -> u8 {
    dartforge_identical(a, b)
}

/// The receiver of a method tear-off: the only `Ref` field of a one-field
/// `_Contexto` (`AllocEnv [receiver_obj]`, `lower/closures.rs`). A closure with a
/// direct environment or with another context has no receiver (it is only equal to
/// itself).
fn tearoff_receiver(heap: &Heap, c: &crate::boxes::ClosureRef) -> Option<i64> {
    let (ctx, is_ref_kind) = c.context;
    if !is_ref_kind || !crate::heap::is_object(ctx) || heap.class(ctx) != crate::layout::cid::CONTEXT {
        return None;
    }
    let o = heap.object(ctx)?;
    match (o.len(), o.get(0)) {
        (1, Some((r, true))) => Some(r),
        _ => None,
    }
}

/// `_Closure.==` (`Closure_equals`): the same function and the same receiver (the
/// tear-off of a method on the same object); ordinary closures are only equal to
/// themselves.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Closure_equals(this: i64, other_one: i64) -> u8 {
    if this == other_one {
        return 1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (Some(a), Some(b)) = (heap.closure(this), heap.closure(other_one)) else { return 0 };
        if a.code != b.code {
            return 0;
        }
        match (tearoff_receiver(&heap, &a), tearoff_receiver(&heap, &b)) {
            (Some(x), Some(y)) => u8::from(x == y),
            _ => 0,
        }
    })
}

/// `_Closure._computeHash` (`Closure_computeHash`): consistent with
/// `Closure_equals` — the function and, for a method tear-off, the receiver.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Closure_computeHash(this: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(c) = heap.closure(this) else { return 0 };
        let mut h = (c.code as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(r) = tearoff_receiver(&heap, &c) {
            h ^= (r as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        }
        // A positive 30-bit `Smi`, like the VM's hash.
        ((h >> 34) & 0x3FFF_FFFF) as i64
    })
}

/// `_Record._numFields`: how many fields (the runtime's positional record).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_record_numFields(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().record(this).map_or(0, |c| c.len() as i64))
}

/// `_Record._shape`: the shape — for a positional record, the number of fields.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_record_shape(this: i64) -> i64 {
    dartforge_native_DartForge_record_numFields(this)
}

/// `_Record._fieldNames`: the names (none in a positional record): an empty
/// `_ImmutableList`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_record_fieldNames(_this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::IMMUTABLE_LIST, 0, crate::lists::Element::General))
}

/// `_Record._fieldAt(i)`: the field (already in a `Ref` position), or null outside
/// the shape.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_record_fieldAt(this: i64, i: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let fields = heap.record(this);
        usize::try_from(i).ok().and_then(|i| fields.and_then(|c| c.get(i).copied())).unwrap_or(0)
    })
}

/// `Object._getHash`/`identityHashCode` (`Object_getHash`, §2.10): an `int` is its
/// own value; `true`/`false`, the VM's constants (1231/1237); a `String`, the hash of
/// its content (the VM's `_identityHashCode` is `String_getHashCode`); the other
/// objects, the header's identity hash (`Heap::identity_hash`, stable because the
/// collector does not move).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Object_getHash(o: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.value(o) {
            crate::heap::Value::Int(i) => return i,
            crate::heap::Value::Bool(b) => return if b { 1231 } else { 1237 },
            _ => {}
        }
        if o == 0 {
            return 2011;
        }
        if heap.is_string(o) {
            return heap.string_hash(o).map_or(0, i64::from);
        }
        heap.identity_hash(o).unwrap_or((o >> 3) & 0x3fff_ffff)
    })
}
