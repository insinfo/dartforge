// Table-based exceptions (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): the
// Rust → Dart ports and the unwinding personality.
//
// In `--excecoes=tabelas` mode a Dart function that throws without a local handler
// does not return with the pending exception: it unwinds the stack (`@df.throw`,
// generated in IR) up to the `landingpad` of whoever catches it. Two rules keep
// the runtime standing:
//
// * **no Rust frame stays in the path of an unwind**: every call from the runtime
//   to Dart code goes through a generated *port* (`@df.port.<r|v><n>`), which
//   catches any unwind, restores the top of the shadow stack and returns with the
//   exception **pending** — the protocol the runtime already expects from a Dart
//   function that threw;
// * **the exception itself stays where it always was**: the runtime's pending
//   exception (`dartforge_exception_throw`). Unwinding only transfers control.
//
// In the default mode (`checagem`) no port is registered and the calls go directly,
// as before; nobody references the personality.

/// The ports registered by the program module (`@df.ports`), by the index
/// `2 × arity + (1 if it returns a value)`. Process-wide: they are code addresses,
/// the same in every isolate. 0 = no port (`checagem` mode).
static DART_PORTS: [std::sync::atomic::AtomicUsize; 16] = [
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
];

/// `dartforge_register_ports(table, n)`: the entry of a program compiled with
/// `--excecoes=tabelas` hands over the ports before any Dart code runs.
///
/// # Safety
/// `table` points to `n` function addresses (a module constant).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_ports(table: *const usize, n: i64) {
    TABLES_MODE.store(true, std::sync::atomic::Ordering::Release);
    // The `sem_porta` sabotage (D9, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §7.3): the runtime calls Dart directly, and a Dart exception crosses the Rust
    // frames.
    if crate::heap::sabotage("sem_porta") {
        return;
    }
    let n = usize::try_from(n).unwrap_or(0).min(DART_PORTS.len());
    for (i, port) in DART_PORTS.iter().enumerate().take(n) {
        // SAFETY: guaranteed by the caller (a constant vector with `n` addresses).
        let address = unsafe { *table.add(i) };
        port.store(address, std::sync::atomic::Ordering::Release);
    }
}

/// The program was compiled with `--excecoes=tabelas` (the entry registered the
/// ports, or tried to: the `sem_porta` sabotage returns before registering).
static TABLES_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// The direct Rust → Dart calls in progress on this thread in `tables` mode
    /// (only the `sem_porta` sabotage produces them): the address of a local of the
    /// Rust frame of each one.
    static DIRECT_CALLS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A direct call to Dart code, without a port. In `tables` mode it stays
/// registered while it runs: an unwind landing above it would have crossed the
/// Rust frame (D9), and the personality ends the process instead of letting it go
/// on with the runtime state half done.
#[inline(never)]
fn direct<T>(call: impl FnOnce() -> T) -> T {
    if !TABLES_MODE.load(std::sync::atomic::Ordering::Acquire) {
        return call();
    }
    let mark = 0u8;
    let address = std::ptr::addr_of!(mark) as usize;
    DIRECT_CALLS.with(|c| c.borrow_mut().push(address));
    let r = call();
    DIRECT_CALLS.with(|c| c.borrow_mut().pop());
    std::hint::black_box(&mark);
    r
}

/// The landing in frame `frame` (the `EstablisherFrame`) goes over a direct
/// Rust → Dart call in progress: its Rust frame lies between the throw (deeper)
/// and the landing.
fn landing_crosses_rust(frame: usize) -> bool {
    DIRECT_CALLS.with(|c| c.borrow().iter().any(|&m| m < frame))
}

/// The port with index `i`, or 0 in `checagem` mode.
#[inline]
fn dart_port(i: usize) -> usize {
    DART_PORTS[i].load(std::sync::atomic::Ordering::Acquire)
}

// The runtime's calls to Dart code, by signature: `dart_r<n>` returns the value
// (`i64`), `dart_v<n>` returns nothing. `f` is the address of the Dart function,
// with `n` one-word parameters (integers, handles and pointers are passed the
// same way). After the call, the caller checks `dartforge_exception_pending`, as
// always.

fn dart_r0(f: usize) -> i64 {
    let arc_guard = arc_prepare_port(f, 0).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r0_invoke(f))
}

fn dart_r0_invoke(f: usize) -> i64 {
    let p = dart_port(1);
    if p != 0 {
        // SAFETY: port r0 receives the target and 0 words, returning one word.
        let g: extern "C" fn(usize) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: the target's declared ABI has 0 word parameters and an i64 return.
    let g: extern "C" fn() -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g())
}

fn dart_r1(f: usize, a: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 1).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r1_invoke(f, a))
}

fn dart_r1_invoke(f: usize, a: i64) -> i64 {
    let p = dart_port(3);
    if p != 0 {
        // SAFETY: port r1 receives the target and 1 words, returning one word.
        let g: extern "C" fn(usize, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: the target's declared ABI has 1 word parameters and an i64 return.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a))
}

fn dart_r2(f: usize, a: i64, b: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 2).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r2_invoke(f, a, b))
}

fn dart_r2_invoke(f: usize, a: i64, b: i64) -> i64 {
    let p = dart_port(5);
    if p != 0 {
        // SAFETY: port r2 receives the target and 2 words, returning one word.
        let g: extern "C" fn(usize, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: the target's declared ABI has 2 word parameters and an i64 return.
    let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b))
}

fn dart_r3(f: usize, a: i64, b: i64, c: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 3).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r3_invoke(f, a, b, c))
}

fn dart_r3_invoke(f: usize, a: i64, b: i64, c: i64) -> i64 {
    let p = dart_port(7);
    if p != 0 {
        // SAFETY: port r3 receives the target and 3 words, returning one word.
        let g: extern "C" fn(usize, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: the target's declared ABI has 3 word parameters and an i64 return.
    let g: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c))
}

fn dart_r4(f: usize, a: i64, b: i64, c: i64, d: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 4).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r4_invoke(f, a, b, c, d))
}

fn dart_r4_invoke(f: usize, a: i64, b: i64, c: i64, d: i64) -> i64 {
    let p = dart_port(9);
    if p != 0 {
        // SAFETY: port r4 receives the target and 4 words, returning one word.
        let g: extern "C" fn(usize, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d);
    }
    // SAFETY: the target's declared ABI has 4 word parameters and an i64 return.
    let g: extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d))
}

fn dart_r5(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 5).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r5_invoke(f, a, b, c, d, e))
}

fn dart_r5_invoke(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    let p = dart_port(11);
    if p != 0 {
        // SAFETY: port r5 receives the target and 5 words, returning one word.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e);
    }
    // SAFETY: the target's declared ABI has 5 word parameters and an i64 return.
    let g: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d, e))
}

#[allow(clippy::too_many_arguments)]

fn dart_r7(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64, g6: i64, h: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 7).expect("invalid ARC callback boundary");
    arc_finish_port(arc_guard, dart_r7_invoke(f, a, b, c, d, e, g6, h))
}

#[allow(clippy::too_many_arguments)]
fn dart_r7_invoke(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64, g6: i64, h: i64) -> i64 {
    let p = dart_port(15);
    if p != 0 {
        // SAFETY: port r7 receives the target and 7 words, returning one word.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e, g6, h);
    }
    // SAFETY: the target's declared ABI has 7 word parameters and an i64 return.
    let g: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d, e, g6, h))
}

fn dart_v0(f: usize) {
    if let Some(guard) = arc_prepare_port(f, 0).expect("invalid ARC callback boundary") {
        // The consumer ignores the value, but the body's ABI is still Ref Owned.
        // It uses the returning port and consumes that token before going on.
        arc_discard_port(guard, dart_r0_invoke(f));
        return;
    }
    let p = dart_port(0);
    if p != 0 {
        // SAFETY: `@df.port.v0` has the signature (ptr) -> void.
        let g: extern "C" fn(usize) = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: `f` is a generated `void` function without parameters.
    let g: extern "C" fn() = unsafe { std::mem::transmute(f) };
    direct(|| g())
}

fn dart_v1(f: usize, a: i64) {
    if let Some(guard) = arc_prepare_port(f, 1).expect("invalid ARC callback boundary") {
        // The consumer ignores the value, but the body's ABI is still Ref Owned.
        // It uses the returning port and consumes that token before going on.
        arc_discard_port(guard, dart_r1_invoke(f, a));
        return;
    }
    let p = dart_port(2);
    if p != 0 {
        // SAFETY: `@df.port.v1` has the signature (ptr, i64) -> void.
        let g: extern "C" fn(usize, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: `f` is a generated `void` function with one one-word parameter.
    let g: extern "C" fn(i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a))
}

fn dart_v2(f: usize, a: i64, b: i64) {
    if let Some(guard) = arc_prepare_port(f, 2).expect("invalid ARC callback boundary") {
        // The consumer ignores the value, but the body's ABI is still Ref Owned.
        // It uses the returning port and consumes that token before going on.
        arc_discard_port(guard, dart_r2_invoke(f, a, b));
        return;
    }
    let p = dart_port(4);
    if p != 0 {
        // SAFETY: `@df.port.v2` has the signature (ptr, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: `f` is a generated `void` function with two one-word parameters.
    let g: extern "C" fn(i64, i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b))
}

fn dart_v3(f: usize, a: i64, b: i64, c: i64) {
    if let Some(guard) = arc_prepare_port(f, 3).expect("invalid ARC callback boundary") {
        // The consumer ignores the value, but the body's ABI is still Ref Owned.
        // It uses the returning port and consumes that token before going on.
        arc_discard_port(guard, dart_r3_invoke(f, a, b, c));
        return;
    }
    let p = dart_port(6);
    if p != 0 {
        // SAFETY: `@df.port.v3` has the signature (ptr, i64, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: `f` is a generated `void` function with three one-word parameters.
    let g: extern "C" fn(i64, i64, i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c))
}

// ---------------------------------------------------------------------------
// The personality.

/// The code of the structured exception with which generated code unwinds
/// (`RaiseException` in `@df.throw`): a customer code of our own.
#[cfg(all(windows, target_arch = "x86_64"))]
const DART_UNWIND_CODE: u32 = 0xE044_4652;

/// Ends the process with a message: a state that only a compiler or runtime defect
/// produces, at a point from which nothing can be thrown.
fn abort_unwinding(message: &str) -> ! {
    eprintln!("dartforge: defect in exception unwinding: {message}");
    std::process::abort()
}

/// Reads a `uleb128` integer and advances the cursor.
///
/// # Safety
/// `*p` points to a well-formed `uleb128` of the LSDA.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn read_uleb128(p: &mut *const u8) -> u64 {
    let mut result: u64 = 0;
    let mut offset: u32 = 0;
    loop {
        // SAFETY: guaranteed by the caller.
        let byte = unsafe { **p };
        // SAFETY: the next byte still belongs to the table (the `uleb128` ends in a
        // byte without the high bit).
        *p = unsafe { p.add(1) };
        if offset < 64 {
            result |= u64::from(byte & 0x7f) << offset;
        }
        offset += 7;
        if byte & 0x80 == 0 {
            return result;
        }
    }
}

/// The landing-pad offset and the action index that cover `relative_ip`. The
/// addresses are relative to the function; (0, 0) indicates a missing range.
/// Preserves the action: zero identifies a cleanup without a typed action; positive
/// values are indices into the action table, not type selectors.
///
/// The LSDA is the one LLVM emits for a personality it does not know (Itanium
/// format, `GCC_except_table`): `LPStart` omitted, the encoding of the type table
/// (with the offset to its end, when present), the `uleb128` encoding of the *call
/// sites* and their table — start, length, landing pad and action of each range,
/// in order. Actions and types are not interpreted here. The current protocol still
/// treats landing pads as handlers; distinguishing the phases will be needed for
/// native resume.
///
/// # Safety
/// `lsda` points to the LSDA of a generated function.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn site_and_action_table(lsda: *const u8, relative_ip: u64) -> (u64, u64, *const u8) {
    if lsda.is_null() {
        return (0, 0, std::ptr::null());
    }
    let mut p = lsda;
    // SAFETY: the LSDA has at least the three-byte header.
    unsafe {
        if *p != 0xff {
            abort_unwinding("LSDA with an explicit LPStart");
        }
        p = p.add(1);
        let type_encoding = *p;
        p = p.add(1);
        if type_encoding != 0xff {
            let _ = read_uleb128(&mut p);
        }
        // The call-site encoding: `uleb128` (0x01, COFF and ELF) or `udata4`
        // (0x03, which LLVM may use on Mach-O; not checked).
        let encoding = *p;
        if encoding != 0x01 && encoding != 0x03 {
            abort_unwinding("LSDA with call sites in an unexpected encoding");
        }
        p = p.add(1);
        let read_field = |p: &mut *const u8| -> u64 {
            if encoding == 0x01 {
                read_uleb128(p)
            } else {
                let value = u32::from_le_bytes([**p, *p.add(1), *p.add(2), *p.add(3)]);
                *p = p.add(4);
                u64::from(value)
            }
        };
        let size = read_uleb128(&mut p);
        let end = p.add(size as usize);
        while p < end {
            let start = read_field(&mut p);
            let length = read_field(&mut p);
            let landing = read_field(&mut p);
            let action = read_uleb128(&mut p);
            if relative_ip < start {
                break;
            }
            if relative_ip - start < length {
                return (landing, action, end);
            }
        }
    }
    (0, 0, std::ptr::null())
}

/// Legacy query: preserves the index without interpreting the chain.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn lsda_site(lsda: *const u8, relative_ip: u64) -> (u64, u64) {
    // SAFETY: the same well-formed LSDA as the caller's.
    let (landing, action, _) = unsafe { site_and_action_table(lsda, relative_ip) };
    (landing, action)
}

/// Reads SLEB128, including negative offsets between actions.
#[cfg(any(unix, test))]
unsafe fn read_sleb128(p: &mut *const u8) -> i64 {
    let mut bits = 0_u64;
    let mut offset = 0_u32;
    loop {
        // SAFETY: the caller provides a valid and complete encoding.
        let byte = unsafe { **p };
        *p = unsafe { p.add(1) };
        if offset < 64 { bits |= u64::from(byte & 0x7f) << offset; }
        offset += 7;
        if byte & 0x80 == 0 {
            if offset < 64 && byte & 0x40 != 0 { bits |= u64::MAX << offset; }
            return bits as i64;
        }
    }
}

/// Finds the cleanup and the first catch-all selector in a valid chain. The site
/// index is a byte offset, not the catch's selector.
#[cfg(any(unix, test))]
unsafe fn itanium_actions(table: *const u8, ix: u64) -> (bool, i32) {
    if ix == 0 { return (true, 0); }
    let mut cleanup_pad = false;
    let mut selector = 0;
    // SAFETY: the index and the links belong to the valid LSDA provided by the system.
    unsafe {
        let mut p = table.add((ix - 1) as usize);
        loop {
            let filter_obj = read_sleb128(&mut p);
            if filter_obj == 0 { cleanup_pad = true; }
            else if filter_obj > 0 && selector == 0 {
                selector = i32::try_from(filter_obj).unwrap_or_else(|_| abort_unwinding("Itanium selector exceeds i32"));
            } else if filter_obj < 0 {
                abort_unwinding("negative filters not supported in the cleanup protocol");
            }
            // The link is relative to the start of the offset field, and may point
            // backwards; it is not relative to the end of the SLEB128.
            let origin = p;
            let next_offset = read_sleb128(&mut p);
            if next_offset == 0 { return (cleanup_pad, selector); }
            p = origin.offset(next_offset as isize);
        }
    }
}

/// `EXCEPTION_RECORD` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct SehExceptionRecord {
    code: u32,
    flag_bits: u32,
    rec: *mut SehExceptionRecord,
    address: *mut u8,
    n_parameters: u32,
    parameters: [usize; 15],
}

/// `RUNTIME_FUNCTION` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct SehRuntimeFunction {
    start: u32,
    end: u32,
    unwinding: u32,
}

/// `DISPATCHER_CONTEXT` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct SehDispatchContext {
    control_pc: u64,
    image_base: u64,
    function_entry: *const SehRuntimeFunction,
    establisher_frame: u64,
    target_ip: u64,
    context_record: *mut u8,
    language_handler: *mut u8,
    handler_data: *const u8,
    history_table: *mut u8,
    scope_index: u32,
    fill0: u32,
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlUnwindEx(
        target_frame: *mut u8,
        target_ip: *mut u8,
        exception_record: *mut SehExceptionRecord,
        return_value: *mut u8,
        context_record: *mut u8,
        history_table: *mut u8,
    );
}

/// The personality on Windows x86-64: the *language handler* that the `.xdata` of
/// each generated function with a `landingpad` names.
///
/// * an exception that is not our unwind (an access violation, C++ from a native
///   library): not Dart, the search continues;
/// * second pass (`EXCEPTION_UNWINDING`/`EXCEPTION_EXIT_UNWIND`): there is no
///   cleanup to do in the frames in between (the skipped root frames are popped at
///   once by the landing pad, which restores the top);
/// * search: if the call site has a landing pad in the LSDA, `RtlUnwindEx` unwinds
///   up to this frame and continues at the landing pad. It does not return.
///
/// # Safety
/// Called by the system exception dispatcher with the four arguments of an
/// `EXCEPTION_ROUTINE`.
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn seh_personality(rec: *mut u8, frame: *mut u8, dispatch: *mut u8) -> i32 {
    /// `ExceptionContinueSearch`.
    const CONTINUE_SEARCH: i32 = 1;
    let rec = rec.cast::<SehExceptionRecord>();
    let dispatch = dispatch.cast::<SehDispatchContext>();
    // SAFETY: the pointers are the dispatcher's; the LSDA is that of the function
    // whose frame is being examined.
    unsafe {
        if (*rec).code != DART_UNWIND_CODE {
            return CONTINUE_SEARCH;
        }
        if (*rec).flag_bits & 0x6 != 0 {
            return CONTINUE_SEARCH;
        }
        if (*dispatch).function_entry.is_null() {
            return CONTINUE_SEARCH;
        }
        let start = (*dispatch).image_base + u64::from((*(*dispatch).function_entry).start);
        // The return address is that of the instruction after the call, which may
        // already belong to another range: the call site is one byte before.
        let relative_ip = (*dispatch).control_pc.wrapping_sub(1).wrapping_sub(start);
        let (landing, _action) = lsda_site((*dispatch).handler_data, relative_ip);
        if landing == 0 {
            return CONTINUE_SEARCH;
        }
        if landing_crosses_rust(frame as usize) {
            abort_unwinding("a Dart exception crossed Rust frames (a runtime call to Dart code without a port)");
        }
        RtlUnwindEx(
            frame,
            (start + landing) as *mut u8,
            rec,
            std::ptr::null_mut(),
            (*dispatch).context_record,
            (*dispatch).history_table,
        );
    }
    abort_unwinding("RtlUnwindEx returned")
}

/// The personality of the functions generated with `--excecoes=tabelas`
/// (`personality ptr @dartforge_personality` in the IR). Only Windows x86-64 has
/// the mode; on the other targets the emitter rejects it and nobody references this
/// symbol.
///
/// # Safety
/// Only the system exception dispatcher calls it.
#[cfg(not(unix))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personality(rec: *mut u8, frame: *mut u8, context: *mut u8, dispatch: *mut u8) -> i32 {
    let _ = context;
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        // SAFETY: the arguments are those of the system dispatcher.
        unsafe { seh_personality(rec, frame, dispatch) }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (rec, frame, dispatch);
        abort_unwinding("table-based exceptions do not exist on this target")
    }
}

// ---------------------------------------------------------------------------
// The Itanium targets (Linux, macOS; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.11 and Stage 4). Written without compiling or running (2026-10-05).

/// The system unwinder's functions (`libgcc_s` or `libunwind`, which Rust's
/// standard library already links).
#[cfg(unix)]
unsafe extern "C" {
    fn _Unwind_RaiseException(object: *mut UnwindObject) -> i32;
    fn _Unwind_GetIPInfo(context: *mut u8, before_instruction: *mut i32) -> usize;
    fn _Unwind_GetRegionStart(context: *mut u8) -> usize;
    fn _Unwind_GetLanguageSpecificData(context: *mut u8) -> *const u8;
    fn _Unwind_SetGR(context: *mut u8, register: i32, value: usize);
    fn _Unwind_SetIP(context: *mut u8, value: usize);
}

/// The class of this runtime's exceptions (`exception_class`): the eight characters
/// `DARTFRGE`.
#[cfg(unix)]
const DART_UNWIND_CLASS: u64 = u64::from_be_bytes(*b"DARTFRGE");

/// `_Unwind_Exception`: the class, the cleanup and the unwinder's private area (two
/// words on x86-64 and aarch64; space is left over on purpose). The alignment is the
/// target's maximum, as in the C header.
#[cfg(unix)]
#[repr(C, align(16))]
struct UnwindObject {
    class: u64,
    cleanup_pad: Option<unsafe extern "C" fn(i32, *mut UnwindObject)>,
    private: [u64; 6],
}

#[cfg(unix)]
thread_local! {
    /// The thread's unwind object. One is enough: the Dart exception is the
    /// runtime's pending exception, and every landing pad of generated code catches
    /// everything, so there are never two unwinds in progress on the same thread.
    static UNWIND_OBJECT: std::cell::UnsafeCell<UnwindObject> =
        const { std::cell::UnsafeCell::new(UnwindObject { class: DART_UNWIND_CLASS, cleanup_pad: None, private: [0; 6] }) };
}

/// The thread's exception object, ready for the unwinding of table-based exceptions
/// on the Itanium targets: `@df.throw` (generated in IR) hands it itself to
/// `_Unwind_RaiseException`, with no Rust frame on the way to the landing pad (the
/// abort guard of an `extern "C"` function in the runtime's `panic=unwind` variant
/// would stop the search). Neither allocates nor throws.
///
/// # Safety
/// Only generated code calls it, with the Dart exception already pending.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_unwind_object() -> *mut u8 {
    let object = UNWIND_OBJECT.with(|o| o.get());
    // SAFETY: the object belongs to the thread and there is no other unwind in
    // progress; the unwinder only uses its private area.
    unsafe {
        (*object).class = DART_UNWIND_CLASS;
        (*object).cleanup_pad = None;
        (*object).private = [0; 6];
    }
    object.cast()
}

/// `@df.throw`'s `_Unwind_RaiseException` returned: no landing pad caught it (the
/// program entry and the ports have landing pads, so it is a defect). Ends.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unwind_failed(reason: i32) -> ! {
    abort_unwinding(&format!("_Unwind_RaiseException returned {reason} (no frame caught it)"))
}

/// The Itanium personality of the functions with landing pads: the same LSDA and
/// the same rule as SEH (every landing pad catches everything), in both phases of
/// the unwinder.
///
/// # Safety
/// Only the system unwinder calls it.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personality(version: i32, actions: i32, class: u64, _object: *mut u8, context: *mut u8) -> i32 {
    // SAFETY: the dispatcher's contract, preserving the legacy protocol.
    unsafe { itanium_personality(version, actions, class, _object, context, false) }
}

/// Itanium personality for cleanup and catch-all, including mixed landing pads.
///
/// Cleanup does not end the search; it runs in the unwind phase, also for a
/// foreign/forced exception. Hands the landing pad the original object and selector
/// zero, allowing resume without reinitializing its private area. A Dart catch-all
/// uses the action's selector and only handles in the chosen frame. Mixed landing
/// pads run cleanup for a foreign/forced exception with selector zero. Does not
/// interpret typed filters: the positive actions are catch-all. The legacy emitter
/// keeps using `dartforge_personality`.
///
/// # Errors
/// Returns the Itanium protocol codes; a version other than 1 returns a fatal error
/// in the search phase. A missing site allows the search to continue.
///
/// # Safety
/// Only the system unwinder calls it, with a valid context/object and LSDA. The LSDA
/// contains only cleanup and catch-all (null type); actions and links are valid.
/// The landing pad must distinguish the selectors and preserve the received pair.
///
/// ```
/// # #[cfg(unix)] {
/// let _: unsafe extern "C" fn(i32, i32, u64, *mut u8, *mut u8) -> i32 =
///     dartforge_runtime::abi::dartforge_personality_cleanup_itanium;
/// # }
/// ```
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personality_cleanup_itanium(
    version: i32, actions: i32, class: u64, object: *mut u8, context: *mut u8,
) -> i32 {
    // SAFETY: the dispatcher's contract for sites of the cleanup protocol.
    unsafe { itanium_personality(version, actions, class, object, context, true) }
}

/// Decision of the phases, independent of the target's APIs and registers.
#[cfg(any(unix, test))]
fn itanium_decision(version: i32, actions: i32, ours: bool, landing: u64, cleanup_pad: bool, selector: i32, cleanup: bool) -> i32 {
    const FATAL_IN_PHASE_1: i32 = 3;
    const HANDLER_FOUND: i32 = 6;
    const INSTALL_CONTEXT: i32 = 7;
    const CONTINUE_UNWIND: i32 = 8;
    if version != 1 { return FATAL_IN_PHASE_1; }
    if landing == 0 { return CONTINUE_UNWIND; }
    let search = actions & 1 != 0;
    let unwind = actions & 2 != 0;
    let handler_frame = actions & 4 != 0;
    let forced = actions & 8 != 0;
    if !cleanup {
        if !ours { return CONTINUE_UNWIND; }
        if search { return HANDLER_FOUND; }
        return if forced { CONTINUE_UNWIND } else { INSTALL_CONTEXT };
    }
    if search {
        return if ours && !forced && selector > 0 { HANDLER_FOUND } else { CONTINUE_UNWIND };
    }
    if unwind && (cleanup_pad || (ours && !forced && handler_frame && selector > 0)) {
        INSTALL_CONTEXT
    } else {
        CONTINUE_UNWIND
    }
}

#[cfg(unix)]
unsafe fn itanium_personality(
    version: i32, actions: i32, class: u64, object: *mut u8, context: *mut u8, cleanup: bool,
) -> i32 {
    // Validates the version/class before accessing the context, as in the legacy
    // protocol. Cleanup also needs to track foreign exceptions.
    if version != 1 { return 3; }
    let ours = class == DART_UNWIND_CLASS;
    if !cleanup && !ours { return 8; }
    // SAFETY: context and LSDA handed over by the system unwinder.
    unsafe {
        let mut before: i32 = 0;
        let ip = _Unwind_GetIPInfo(context, &mut before) as u64;
        let ip = if before == 0 { ip.wrapping_sub(1) } else { ip };
        let start = _Unwind_GetRegionStart(context) as u64;
        let (landing, action, table) = site_and_action_table(_Unwind_GetLanguageSpecificData(context), ip.wrapping_sub(start));
        let (cleanup_pad, selector) = if cleanup && landing != 0 {
            itanium_actions(table, action)
        } else { (action == 0, if action != 0 { 1 } else { 0 }) };
        let decision = itanium_decision(version, actions, ours, landing, cleanup_pad, selector, cleanup);
        if decision != 7 { return decision; }
        // An intermediate or foreign cleanup uses selector zero. A mixed landing pad
        // chosen as the handler also runs cleanup, with the catch's selector, before
        // branching to the handler or resuming the exception.
        let handles = ours && actions & 4 != 0 && actions & 8 == 0 && selector > 0;
        _Unwind_SetGR(context, 0, if cleanup { object as usize } else { 0 });
        _Unwind_SetGR(context, 1, if cleanup && handles { selector as usize } else { 0 });
        _Unwind_SetIP(context, (start + landing) as usize);
        decision
    }
}

#[cfg(all(test, any(all(windows, target_arch = "x86_64"), unix)))]
mod tests_exceptions_tables {
    use super::*;

    #[test]
    fn action_chain_separates_index_selector_and_cleanup() {
        // SAFETY: complete chains in local arrays, with valid links.
        unsafe {
            assert_eq!(itanium_actions(std::ptr::null(), 0), (true, 0));
            assert_eq!(itanium_actions([0, 0].as_ptr(), 1), (true, 0));
            assert_eq!(itanium_actions([2, 0].as_ptr(), 1), (false, 2));
            assert_eq!(itanium_actions([0, 1, 1, 0].as_ptr(), 1), (true, 1));
            // The record at index 3 goes back three bytes from its link field.
            assert_eq!(itanium_actions([0, 0, 1, 0x7d].as_ptr(), 3), (true, 1));
        }
    }

    #[test]
    fn sleb128_preserves_sign_and_extremes() {
        for (bytes, expected) in [
            (vec![0x7f], -1), (vec![0x7d], -3), (vec![0x80, 0x01], 128),
            (vec![0x80, 0x7f], -128),
            (vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00], i64::MAX),
            (vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f], i64::MIN),
        ] {
            let mut p = bytes.as_ptr();
            // SAFETY: a complete SLEB128 in the vector.
            assert_eq!(unsafe { read_sleb128(&mut p) }, expected);
            assert_eq!(p, unsafe { bytes.as_ptr().add(bytes.len()) });
        }
    }

    #[test]
    fn mixed_landing_cleans_foreign_without_catching_and_selects_dart_catch() {
        assert_eq!(itanium_decision(1, 1, false, 7, true, 1, true), 8);
        assert_eq!(itanium_decision(1, 2, false, 7, true, 1, true), 7);
        assert_eq!(itanium_decision(1, 2 | 8, false, 7, true, 1, true), 7);
        assert_eq!(itanium_decision(1, 1, true, 7, true, 1, true), 6);
        assert_eq!(itanium_decision(1, 2 | 4, true, 7, true, 1, true), 7);
    }

    #[test]
    fn cleanup_does_not_stop_search_and_installs_in_unwind_phase() {
        for ours in [false, true] {
            for forced in [0, 8] {
                assert_eq!(itanium_decision(1, 1 | forced, ours, 7, true, 0, true), 8);
                assert_eq!(itanium_decision(1, 2 | forced, ours, 7, true, 0, true), 7);
                assert_eq!(itanium_decision(1, 0 | forced, ours, 7, true, 0, true), 8);
            }
        }
        assert_eq!(itanium_decision(1, 2, true, 0, true, 0, true), 8);
        assert_eq!(itanium_decision(0, 1, true, 7, true, 0, true), 3);
    }

    #[test]
    fn catch_all_installs_only_in_frame_chosen_for_dart_exception() {
        assert_eq!(itanium_decision(1, 1, true, 7, false, 1, true), 6);
        assert_eq!(itanium_decision(1, 2, true, 7, false, 1, true), 8);
        assert_eq!(itanium_decision(1, 2 | 4, true, 7, false, 1, true), 7);
        for actions in [1, 2, 2 | 4, 2 | 8, 2 | 4 | 8] {
            assert_eq!(itanium_decision(1, actions, false, 7, false, 1, true), 8);
        }
        assert_eq!(itanium_decision(1, 2 | 4 | 8, true, 7, false, 1, true), 8);
    }

    #[test]
    fn legacy_protocol_preserves_statepoint_landings() {
        for action in [0, 1] {
            assert_eq!(itanium_decision(1, 1, true, 7, action == 0, action as i32, false), 6);
            assert_eq!(itanium_decision(1, 2, true, 7, action == 0, action as i32, false), 7);
            assert_eq!(itanium_decision(1, 2 | 8, true, 7, action == 0, action as i32, false), 8);
            assert_eq!(itanium_decision(1, 2, false, 7, action == 0, action as i32, false), 8);
        }
    }

    /// The LSDA of §13.10 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md): a header with a
    /// type table (`catch ptr null`), three ranges — without a landing pad, with a
    /// landing pad at 0x45, without a landing pad —, the action table and the type table.
    const WITH_TYPES: [u8; 28] = [
        0xff, // LPStart omitted
        0x00, // types: absptr
        0x15, // distance to the end of the type table
        0x01, // call sites in uleb128
        12,   // bytes of the call-site table
        0x00, 0x10, 0x00, 0x00, // [0x00, 0x10): no landing pad
        0x10, 0x20, 0x45, 0x01, // [0x10, 0x30): landing pad 0x45
        0x30, 0x50, 0x00, 0x00, // [0x30, 0x80): no landing pad
        0x01, 0x00, // actions
        0x00, // alignment
        0, 0, 0, 0, 0, 0, 0, 0, // type 1 = null
    ];

    #[test]
    fn landing_by_call_site_range() {
        let p = WITH_TYPES.as_ptr();
        // SAFETY: a well-formed LSDA, entirely in the vector.
        unsafe {
            assert_eq!(lsda_site(p, 0x00), (0, 0));
            assert_eq!(lsda_site(p, 0x0f), (0, 0));
            assert_eq!(lsda_site(p, 0x10), (0x45, 1));
            assert_eq!(lsda_site(p, 0x2f), (0x45, 1));
            assert_eq!(lsda_site(p, 0x30), (0, 0));
            assert_eq!(lsda_site(p, 0x7f), (0, 0));
            // After the last range: no landing pad.
            assert_eq!(lsda_site(p, 0x80), (0, 0));
            assert_eq!(lsda_site(p, 0x1000), (0, 0));
        }
    }

    #[test]
    fn without_type_table_and_with_two_byte_uleb128() {
        // A pure `cleanup`: the second byte is 0xff and there is no distance. A
        // range with a hole before it ([0x80, 0x180), landing pad 0x200), in
        // two-byte uleb128.
        let lsda: [u8; 10] = [0xff, 0xff, 0x01, 6, 0x80, 0x01, 0x80, 0x02, 0x80, 0x04];
        // The range's action comes after; the vector ends with it.
        let mut full = lsda.to_vec();
        full.push(0x00);
        full[3] = 7;
        let p = full.as_ptr();
        // SAFETY: a well-formed LSDA, entirely in the vector.
        unsafe {
            assert_eq!(lsda_site(p, 0x00), (0, 0));
            assert_eq!(lsda_site(p, 0x7f), (0, 0));
            assert_eq!(lsda_site(p, 0x80), (0x200, 0));
            assert_eq!(lsda_site(p, 0x17f), (0x200, 0));
            assert_eq!(lsda_site(p, 0x180), (0, 0));
        }
    }

    #[test]
    fn uleb128_action_is_independent_of_address_encoding() {
        // udata4 addresses (Mach-O): the action stays uleb128 and may have more
        // than one byte. It is a raw index, not a catch selector.
        let mut lsda = vec![0xff, 0xff, 0x03, 14];
        for field in [0x10_u32, 0x20, 0x200] {
            lsda.extend(field.to_le_bytes());
        }
        lsda.extend([0x81, 0x01]);
        // SAFETY: header and range complete, entirely in the vector.
        unsafe {
            assert_eq!(lsda_site(lsda.as_ptr(), 0x0f), (0, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x10), (0x200, 129));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x2f), (0x200, 129));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x30), (0, 0));
        }
    }

    #[test]
    fn range_at_address_limit_does_not_add_length() {
        fn uleb(mut value: u64, bytes: &mut Vec<u8>) {
            loop {
                let byte = (value & 0x7f) as u8;
                value >>= 7;
                bytes.push(byte | if value == 0 { 0 } else { 0x80 });
                if value == 0 { break; }
            }
        }
        let mut range = Vec::new();
        for field in [u64::MAX - 1, 2, 7, 0] {
            uleb(field, &mut range);
        }
        let mut lsda = vec![0xff, 0xff, 0x01, range.len() as u8];
        lsda.extend(range);
        // SAFETY: a complete range in uleb128; the conceptual length ends after the
        // largest address without requiring a u64 sum.
        unsafe {
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX - 2), (0, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX - 1), (7, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX), (7, 0));
        }
    }

    #[test]
    fn missing_lsda_has_no_landing_pad() {
        // SAFETY: the null pointer is handled before any read.
        assert_eq!(unsafe { lsda_site(std::ptr::null(), 0x10) }, (0, 0));
    }

    #[test]
    fn without_registered_port_call_is_direct() {
        extern "C" fn total_sum(a: i64, b: i64) -> i64 {
            a + b
        }
        extern "C" fn seven() -> i64 {
            7
        }
        // No test registers ports: the usual mode.
        assert_eq!(dart_r2(total_sum as usize, 40, 2), 42);
        assert_eq!(dart_r0(seven as usize), 7);
    }
}
