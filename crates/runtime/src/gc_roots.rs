// Native runtime: the collector's root protocol (frames, globals, collection).

/// Runs `f` with `handles` rooted in a temporary frame (G6).
///
/// For an extern that allocates more than once: the first object is only
/// referenced by a Rust variable while the second one is allocated, and that
/// allocation may collect. The frame is the same protocol as generated code's.
fn with_roots<R>(handles: &[i64], f: impl FnOnce() -> R) -> R {
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_frame_with_slots(handles.len());
        for (i, &x) in handles.iter().enumerate() {
            h.set_root(frame, i, x);
        }
        frame
    });
    let r = f();
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

/// The address of the current thread's context (`heap::Context`): the pending
/// exception and the top of the shadow stack, which generated code reads and writes
/// without a call.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_context() -> *const crate::heap::Context {
    crate::heap::CONTEXT.with(|c| {
        if c.stack_limit.get() == 0 {
            c.stack_limit.set(thread_stack_limit());
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            publish_in_tsd(c);
        }
        c as *const crate::heap::Context
    })
}

/// The first `dartforge_context` of each thread also puts the address of the
/// context in the `pthread` key slot that the production prologue reads directly
/// (`heap::dartforge_context_slot`). The key is created once per process; the slot
/// offset is published only after the key exists.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(unsafe_code)]
fn publish_in_tsd(c: &crate::heap::Context) {
    use std::sync::atomic::Ordering;
    unsafe extern "C" {
        fn pthread_key_create(key: *mut usize, destructor: Option<extern "C" fn(*mut u8)>) -> i32;
        fn pthread_setspecific(key: usize, value: *const crate::heap::Context) -> i32;
    }
    static KEY: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    let key = *KEY.get_or_init(|| {
        let mut key = 0usize;
        // SAFETY: a local `pthread_key_t` (`unsigned long` on Darwin), no destructor.
        (unsafe { pthread_key_create(&mut key, None) } == 0).then_some(key)
    });
    // Without a key (the process ran out of them), the slot offset stays 0 and the
    // prologue always calls `dartforge_context`.
    let Some(key) = key else { return };
    // SAFETY: the key exists; the value is the address of this thread's context,
    // which does not change while the thread lives.
    if unsafe { pthread_setspecific(key, c) } == 0 {
        crate::heap::dartforge_context_slot.store(key * 8, Ordering::Release);
    }
}

/// Slack between the limit that generated code checks and the end of the thread's
/// stack: what the runtime still uses to build and throw the `StackOverflowError`
/// (the stack trace, the allocation) and for the native calls of a function that
/// got past its prologue.
const STACK_SLACK: usize = 256 * 1024;

/// The lowest address the current thread's stack may use, plus the
/// [`STACK_SLACK`]; 1 when the system does not tell (no check).
#[allow(unsafe_code)]
fn thread_stack_limit() -> usize {
    let base = stack_base();
    // The `slack` sabotage (D8, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3):
    // 4 KiB are not enough to build and throw the `StackOverflowError`.
    let slack = if crate::heap::sabotage("slack") { 4 * 1024 } else { STACK_SLACK };
    if base == 0 { 1 } else { base + slack }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let (mut low, mut high) = (0usize, 0usize);
    // SAFETY: two pointers to local `usize`s.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    low
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    // `pthread_attr_t` has 56 bytes on glibc x86-64 and 64 on aarch64.
    #[repr(C, align(8))]
    struct PthreadAttr([u8; 64]);
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_getattr_np(t: usize, a: *mut PthreadAttr) -> i32;
        fn pthread_attr_getstack(a: *const PthreadAttr, address: *mut usize, size: *mut usize) -> i32;
        fn pthread_attr_destroy(a: *mut PthreadAttr) -> i32;
    }
    let mut a = PthreadAttr([0; 64]);
    let (mut address, mut size) = (0usize, 0usize);
    // SAFETY: local attributes, initialized by `pthread_getattr_np` and destroyed
    // at the end.
    unsafe {
        if pthread_getattr_np(pthread_self(), &mut a) != 0 {
            return 0;
        }
        let ok = pthread_attr_getstack(&a, &mut address, &mut size) == 0;
        pthread_attr_destroy(&mut a);
        if ok { address } else { 0 }
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_get_stackaddr_np(t: usize) -> usize;
        fn pthread_get_stacksize_np(t: usize) -> usize;
    }
    // SAFETY: queries about the current thread; the address is the top of the stack.
    unsafe {
        let t = pthread_self();
        pthread_get_stackaddr_np(t).saturating_sub(pthread_get_stacksize_np(t))
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn stack_base() -> usize {
    0
}

/// A function's prologue found the frame below the stack limit
/// (`Context::stack_limit`): throws the SDK's `StackOverflowError`, and the
/// function returns with the pending exception, as after a call that threw. The
/// limit is disabled while the error is built (the stack trace and the allocation
/// use the slack) and enabled again afterwards.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_overflow() {
    let limit = crate::heap::CONTEXT.with(|c| c.stack_limit.replace(1));
    let error = dartforge_stack_overflow_error_new();
    if dartforge_exception_pending() == 0 {
        with_roots(&[error], || dartforge_exception_throw(error, 3));
    }
    crate::heap::CONTEXT.with(|c| c.stack_limit.set(limit));
}

/// Current value of a program `Ref` global, kept as a permanent root.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_global_root(id: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().set_global_root(id, handle));
}
/// Marks a canonical value (constant, enum value, `const` global) as permanent and
/// immutable: a message within the same isolate passes it by identity
/// (`ports.rs`). The value is already a global root of the caller.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_mark_permanent(handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().mark_permanent(handle));
}
/// Marks the canonical constant `handle`, produced by the generated getter at
/// address `getter` (see `Heap::mark_constant`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_mark_constant(handle: i64, getter: i64) {
    HEAP.with(|heap| heap.borrow_mut().mark_constant(handle, getter as usize));
}
/// The canonical form of a constant's value, before the getter stores it: the
/// built string (`const s = 'a${'b'}'`) becomes the canonical literal with the same
/// content, like the VM's constant canonicalization.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_canonical_constant(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().canonical_constant(handle))
}
/// Allows an explicit collection in tests and future maintenance routines.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_collect() {
    HEAP.with(|heap| heap.borrow_mut().collect());
}

// ---------------------------------------------------------------------------
// The isolate's globals area.
//
// Dart statics (and the caches of call sites by selector) are per isolate, like
// the VM's *field table*: each compiled module describes its slots (`@df.area`:
// `[key, n, name_0…name_{n-1}]`, with hashes; in the production executable, which
// does not reload, only `[key, -n]`) and each isolate — a thread — has one zeroed
// area per module, created on first access. The `Ref` globals are roots through
// the slot's address (`dartforge_gc_global_root`).
//
// On a reload (JIT), the new module has another descriptor with the same key. The
// new generation is emitted with the live one's layout (`emit_reloadable_ir`):
// the slots that remain keep their index and the new ones come after, so both
// generations use the SAME area — which grows in place (the capacity is reserved
// with slack) — and old code that still runs (a suspended `async` frame, a closure
// created earlier) sees the same statics as the new code. A descriptor that does
// not extend the layout (a module emitted without the previous layout) gets a new
// area, with the values of the remaining slots, by name; the selector caches (a
// name with `CACHE_BIT`, one pair per call site, named after the function and
// the position) restart zeroed, and every publication zeroes them
// (`empty_area_caches`).
//
// An area's memory is never freed while the isolate lives: a frame may keep its
// address (the `%area` at the start of the function). Growing beyond the capacity
// copies into another allocation and retires the old one.

/// A module's area in this isolate.
struct GlobalsArea {
    /// The descriptors (the module's generations) that use this area.
    descriptors: Vec<usize>,
    key: i64,
    /// The names of the longest layout seen (that of the newest generation).
    names: Vec<i64>,
    slots: Vec<i64>,
}

thread_local! {
    static AREAS: RefCell<Vec<GlobalsArea>> = const { RefCell::new(Vec::new()) };
    /// The allocations that stopped being an area's (see above).
    static RETIRED_AREAS: RefCell<Vec<Vec<i64>>> = const { RefCell::new(Vec::new()) };
    /// The last requested descriptor and its area (the fast path).
    static LAST_AREA: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    /// The table by module id that the `Context` points to (`areas`, `n_areas`).
    static AREA_TABLE: RefCell<Vec<*mut i64>> = const { RefCell::new(Vec::new()) };
}

/// The next module id (`@df.area_id`), the same on every thread.
static NEXT_AREA_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// An area changed address (reload): this thread's table is emptied and each
/// module goes through the runtime once again.
fn forget_area_table() {
    AREA_TABLE.with(|t| t.borrow_mut().iter_mut().for_each(|p| *p = std::ptr::null_mut()));
    LAST_AREA.with(|u| u.set((0, 0)));
}

/// After the publication of a new generation in this isolate (with no Dart frame on
/// the stack, J02): each area keeps only the descriptor of the newest generation
/// (the last one to arrive) — that of a retired generation is an address in its
/// memory, which the JIT may free and reuse — and the retired allocations, which
/// only a `%area` of an old frame would still use, are released.
pub fn forget_previous_area_generations() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            if let Some(&new) = a.descriptors.last() {
                a.descriptors.clear();
                a.descriptors.push(new);
            }
        }
    });
    RETIRED_AREAS.with(|x| x.borrow_mut().clear());
    forget_area_table();
}

/// The slow path of `@df.get_area`: the area (`dartforge_globals_area`) and,
/// in this thread's table, the entry for the module id (assigned now, if it does
/// not have one yet).
///
/// # Safety
/// `descriptor` as in [`dartforge_globals_area`]; `id` points to the
/// `@df.area_id` of the same module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_globals_area_id(descriptor: *const i64, id: *const std::sync::atomic::AtomicI64) -> *mut i64 {
    use std::sync::atomic::Ordering;
    // SAFETY: guaranteed by the caller.
    let p = unsafe { dartforge_globals_area(descriptor) };
    // SAFETY: the module's global, alive for the whole process.
    let id = unsafe { &*id };
    let mut k = id.load(Ordering::Relaxed);
    if k == 0 {
        let new = NEXT_AREA_ID.fetch_add(1, Ordering::Relaxed);
        k = match id.compare_exchange(0, new, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => new,
            Err(current) => current,
        };
    }
    let k = k as usize;
    AREA_TABLE.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() <= k {
            t.resize(k + 16, std::ptr::null_mut());
        }
        t[k] = p;
        CONTEXT.with(|c| {
            c.areas.set(t.as_ptr());
            c.n_areas.set(t.len());
        });
    });
    p
}

/// Zeroed slots for `n` names, with slack for the following generations to grow in
/// place.
fn new_slots(n: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(n + n / 2 + 256);
    v.resize(n, 0);
    v
}

/// The globals area of `descriptor`'s module in this isolate.
///
/// # Safety
/// `descriptor` points to a module's `@df.area`: `n + 2` words (`[key, n, names…]`),
/// or 2 (`[key, -n]`, the lean production one), constant for the whole process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_globals_area(descriptor: *const i64) -> *mut i64 {
    let d = descriptor as usize;
    let (last_one, area) = LAST_AREA.with(|u| u.get());
    if last_one == d {
        return area as *mut i64;
    }
    // SAFETY: guaranteed by the caller. The lean descriptor of the production
    // executable (`[key, -n]`, without the names) does not reload: n zeroed slots,
    // without names and without slack.
    let (key, names, lean) = unsafe {
        let raw = *descriptor.add(1);
        if raw < 0 {
            (*descriptor, &[][..], Some(raw.unsigned_abs() as usize))
        } else {
            (*descriptor, std::slice::from_raw_parts(descriptor.add(2), raw as usize), None)
        }
    };
    let p = AREAS.with(|areas| {
        let mut areas = areas.borrow_mut();
        if let Some(a) = areas.iter_mut().find(|a| a.descriptors.contains(&d)) {
            return a.slots.as_mut_ptr();
        }
        if let Some(n) = lean {
            areas.push(GlobalsArea { descriptors: vec![d], key, names: Vec::new(), slots: vec![0; n] });
            return areas.last_mut().expect("just pushed").slots.as_mut_ptr();
        }
        if let Some(i) = areas.iter().position(|a| a.key == key) {
            let a = &mut areas[i];
            if names.len() >= a.names.len() && names[..a.names.len()] == a.names[..] {
                extend_area(a, names);
                a.descriptors.push(d);
                return a.slots.as_mut_ptr();
            }
            // The descriptor of a previous generation (the layout only grows): its
            // code that is still in use — a function that the reload kept (J04) —
            // only knows slots that exist with the same name at the same index.
            if names.len() < a.names.len() && a.names[..names.len()] == *names {
                a.descriptors.push(d);
                return a.slots.as_mut_ptr();
            }
            let old_value = areas.remove(i);
            let mut new_area = GlobalsArea { descriptors: vec![d], key, names: names.to_vec(), slots: new_slots(names.len()) };
            migrate_area(&old_value, &mut new_area);
            RETIRED_AREAS.with(|x| x.borrow_mut().push(old_value.slots));
            areas.push(new_area);
            forget_area_table();
        } else {
            areas.push(GlobalsArea { descriptors: vec![d], key, names: names.to_vec(), slots: new_slots(names.len()) });
        }
        areas.last_mut().expect("just pushed").slots.as_mut_ptr()
    });
    LAST_AREA.with(|u| u.set((d, p as usize)));
    p
}

/// Extends `a` to the layout `names` (which starts with its own): in place, if it
/// fits; otherwise into a larger allocation, with the roots moved and the old one
/// retired.
fn extend_area(a: &mut GlobalsArea, names: &[i64]) {
    if names.len() > a.slots.capacity() {
        let mut larger = new_slots(names.len());
        larger[..a.slots.len()].copy_from_slice(&a.slots);
        let address = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            for i in 0..a.slots.len() {
                heap.move_global_root(address(&a.slots, i), address(&larger, i));
            }
        });
        let old_value = std::mem::replace(&mut a.slots, larger);
        RETIRED_AREAS.with(|x| x.borrow_mut().push(old_value));
        forget_area_table();
    } else {
        a.slots.resize(names.len(), 0);
    }
    a.names = names.to_vec();
}

/// The bit that marks, in the descriptor, the name of a selector cache slot (the
/// emitter's `CACHE_BIT`, `llvm/mod.rs`); 0 is the cache name of a descriptor
/// older than this mark.
const CACHE_BIT: i64 = 1 << 62;

fn is_cache(name: i64) -> bool {
    name == 0 || name & CACHE_BIT != 0
}

/// Zeroes the selector caches of every area of this isolate. The publication of a
/// generation does it with no Dart frame on the stack, after redoing the method
/// tables: the next use of each cache looks the entry up in the new table (the
/// visibility rule: new calls, also from the code of a previous generation, reach
/// the new implementation). A cache holds a class id and a code address, never a
/// heap reference.
pub fn empty_area_caches() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            for (i, &name) in a.names.iter().enumerate() {
                if is_cache(name) && i < a.slots.len() {
                    a.slots[i] = 0;
                }
            }
        }
    });
}

/// Copies into `new_area` the slots of `old_value` with the same name and moves
/// their roots; those of the slots that disappeared are released. The caches do not
/// migrate.
fn migrate_area(old_value: &GlobalsArea, new_area: &mut GlobalsArea) {
    let address = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, &name) in old_value.names.iter().enumerate() {
            let dest = if is_cache(name) { None } else { new_area.names.iter().position(|&n| n == name) };
            match dest {
                Some(j) => {
                    new_area.slots[j] = old_value.slots[i];
                    heap.move_global_root(address(&old_value.slots, i), address(&new_area.slots, j));
                }
                None => heap.release_global_root(address(&old_value.slots, i)),
            }
        }
    });
}

/// Roots by stack maps (`--roots=maps`): a module with functions described by a
/// map registers, at startup, the image it was linked into
/// (`crate::heap::register_stack_map`; idempotent per image). Neither allocates on
/// the collector's heap nor throws.
///
/// # Safety
/// `base` is the `__ImageBase` of a loaded PE image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_map(base: *const u8) {
    crate::heap::register_stack_map(base as usize);
}

/// The registration of the compact stack map of an ELF or Mach-O image
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Stage 4): a module with functions
/// whose roots are in the map calls this at startup with the bounds of the `dfgcm`
/// section that the linker defined. Idempotent per image.
///
/// # Safety
/// `[start, end)` is the map section of a loaded image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_map_section(start: *const u8, end: *const u8) {
    crate::heap::register_map_section(start as usize, end as usize);
}

/// The registration of an LLVM-format stack map of a Mach-O image (Stage 4): the
/// bounds of the `__LLVM_STACKMAPS,__llvm_stackmaps` section.
///
/// # Safety
/// `[start, end)` is the map section of a loaded image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_map_llvm(start: *const u8, end: *const u8) {
    crate::heap::register_llvm_section(start as usize, end as usize);
}

/// Removes the map registered by [`dartforge_register_map_llvm`] with the same
/// `start` (the JIT, when releasing an object's memory).
///
/// # Safety
/// None: it only compares the address.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_unregister_map(start: *const u8) {
    crate::heap::unregister_map(start as usize);
}

/// The statistics that a targeted test reads at the end (§7.5, the proof that it
/// collected): writes into `out[0..4]` the current isolate's collections, the
/// native frames walked, the roots read from maps and the roots checked against the
/// shadow stack (the last three, process-wide).
///
/// # Safety
/// `out` points to four writable `u64`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_gc_stats(out: *mut u64) {
    if out.is_null() {
        return;
    }
    let gc_count = HEAP.with(|h| h.borrow().stats().collections);
    let (frames, roots, checked) = crate::heap::walk_counts();
    // SAFETY: the caller guarantees the four `u64`s.
    unsafe {
        *out = gc_count;
        *out.add(1) = frames;
        *out.add(2) = roots;
        *out.add(3) = checked;
    }
}

#[cfg(test)]
mod tests_gc_roots {
    use super::*;

    #[test]
    fn lean_descriptor_gives_n_zeroed_slots_and_same_area() {
        std::thread::spawn(|| {
            // `[key, -n]` (production) and `[key, n, names…]` (reload), from
            // different modules, in the same isolate.
            static LEAN: [i64; 2] = [0x1234, -3];
            static COMPLETE: [i64; 4] = [0x5678, 2, 11, 12];
            // SAFETY: constant descriptors for the whole process.
            let (a, b, c) = unsafe {
                (
                    dartforge_globals_area(LEAN.as_ptr()),
                    dartforge_globals_area(COMPLETE.as_ptr()),
                    dartforge_globals_area(LEAN.as_ptr()),
                )
            };
            assert_eq!(a, c);
            assert_ne!(a, b);
            // SAFETY: the area has the descriptor's 3 slots.
            let slots = unsafe { std::slice::from_raw_parts_mut(a, 3) };
            assert_eq!(slots, [0, 0, 0]);
            slots[2] = 7;
            AREAS.with(|areas| {
                let areas = areas.borrow();
                let e = areas.iter().find(|x| x.key == 0x1234).expect("lean area");
                assert!(e.names.is_empty());
                assert_eq!(e.slots, [0, 0, 7]);
                let c = areas.iter().find(|x| x.key == 0x5678).expect("full area");
                assert_eq!(c.names, [11, 12]);
            });
        })
        .join()
        .unwrap();
    }
}

// ─── Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ───────────────────

/// Generated code's slow allocation (`@df.allocate`): a zeroed block of `words`
/// body words, class `cid`, state `YOUNG`; collects if needed and refills the TLAB
/// of `words` (up to `layout::TLAB_N`). Does not throw.
///
/// `flags` are bits 8–31 of word 0 of the header that generated code built
/// (`header >> 8`): the block's `flags` in the low 8 and, for `INSTANCE`, the
/// number of fields in the next 16 (for `RAW`/`REFS`, `n` is `words`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_allocate(cid: i64, words: i64, flags: i64) -> i64 {
    let w = usize::try_from(words).expect("compiler bug: negative words in dartforge_allocate");
    let f = (flags & 0xFF) as u8;
    let n = if f & crate::layout::flags::FORMAT == crate::layout::flags::INSTANCE {
        ((flags >> 8) & 0xFFFF) as usize
    } else {
        w
    };
    let cid = i32::try_from(cid).expect("compiler bug: cid beyond 32 bits");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let r = h.allocate_block(cid, w, f, n);
        h.refill_tlab(w);
        r
    })
}

/// Registers the static object section `[start, end)` of an image (the executable,
/// the SDK DLL): called by each one's `@df.prepare_isolate`
/// (docs/NATIVO-ESPACO-UNIFICADO.md §2.11). Neither allocates nor throws.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_image(start: *const u8, end: *const u8) {
    crate::heap::register_image(start as usize, end as usize);
}
