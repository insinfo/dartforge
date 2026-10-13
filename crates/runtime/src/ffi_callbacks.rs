// Native runtime: `dart:ffi` callbacks — `Pointer.fromFunction`,
// `NativeCallable.isolateLocal` and `NativeCallable.listener` (the VM's
// `runtime/vm/ffi_callback_metadata.cc` and `ffi_callback_trampolines`).
//
// * The compiler generates, per native signature of the program (the same key as
//   the call trampolines, `lower/ffi.rs`), a C ENTRY: a function with the
//   signature's C ABI and a `nest` parameter — the callback's context. The entry
//   asks the runtime for the mode (`dartforge_ffi_callback_enter`), converts the C
//   arguments to Dart, calls the closure through the uniform convention (through a
//   common HIR body), converts the return value and, if the closure threw, returns
//   the exceptional return value (`dartforge_ffi_callback_exit`).
// * Each callback is a distinct C function pointer: a trampoline of a few
//   instructions that loads the context into the entry's `nest` register (r10 on
//   x86-64, x15 on AArch64 — the same code `llvm.init.trampoline` would write) and
//   jumps to it, written by this file into a slice of executable memory. Writing it
//   here, and not through the intrinsic in the generated module, keeps the stack
//   non-executable in every object (with the intrinsic LLVM omits the
//   `.note.GNU-stack` note, and the LTO partitions do not recover it).
// * Modes: `isolateLocal` and `fromFunction` call the closure right away, and only
//   on the owning isolate's thread (another thread is a fatal error, as in the VM);
//   `fromFunction` is persistent and unique per (function, signature, exceptional
//   return). `listener` may be called from any thread: the entry copies the
//   arguments into a message to the `NativeCallable`'s port, and the owning
//   isolate calls the closure in the event loop.
// * The closure of a local callback is a collector root (a root global of the
//   isolate's heap, through the context's address) until `close()`. Closing from
//   inside the callback itself is allowed: the context is only released when the
//   last call in progress leaves.

/// Modes of a callback (the `mode` of `DartForge_ffi_callback_new`).
const MODE_PERSISTENT: i64 = 0;
const MODE_LOCAL: i64 = 1;
const MODE_LISTENER: i64 = 2;

/// The state of a callback; its address is the trampoline's `nest` value.
pub struct CallbackContext {
    mode: i64,
    /// The owning isolate (queue id).
    isolate: u64,
    /// The closure (local and persistent modes), a root of the owner's heap.
    closure: i64,
    /// The port of the `NativeCallable.listener`.
    port: i64,
    /// The native signature (RTI), for the arguments' `Pointer<X>`s.
    fn_signature: i64,
    /// The signature key (`i_pp`): letters of the return and of the parameters.
    key: String,
    /// The exceptional return value, in the bits of the return's C type.
    exceptional: i64,
    /// Calls in progress (the closure may close the callback from inside).
    active_list: u32,
    closed_flag: bool,
    /// The C function pointer (the trampoline).
    trampoline: usize,
}

/// Signature key → the callback's C entry.
fn callback_entries() -> &'static std::sync::RwLock<crate::hash::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Function pointer → context, of the live callbacks.
fn live_callbacks() -> &'static std::sync::Mutex<crate::hash::HashMap<usize, usize>> {
    static T: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<usize, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Recognizes synchronous trampolines that require the owning isolate's thread.
/// Listeners only post messages and do not enter Dart on this thread.
fn callback_requires_isolate_thread(address: usize) -> bool {
    let live_set = live_callbacks().lock().unwrap_or_else(|e| e.into_inner());
    let Some(&ctx) = live_set.get(&address) else { return false };
    // SAFETY: the registry keeps the context alive; close needs this mutex to
    // remove it before freeing. The mode is immutable after installation.
    let mode = unsafe { (*(ctx as *const CallbackContext)).mode };
    matches!(mode, MODE_PERSISTENT | MODE_LOCAL)
}

thread_local! {
    /// This isolate's `fromFunction`s: (key, site, function code, exceptional) →
    /// function pointer.
    static PERSISTENT: RefCell<crate::hash::HashMap<(String, i64, i64, i64), usize>> = RefCell::new(crate::hash::HashMap::default());
}

/// # Safety
/// `key` points to `n` ASCII bytes; `input` is the C entry generated for the
/// signature (the context in the `nest` parameter).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_register_callback(key: *const u8, n: i64, input: usize) {
    // SAFETY: guaranteed by the caller (a module constant).
    let b = unsafe { std::slice::from_raw_parts(key, n as usize) };
    let key = String::from_utf8_lossy(b).into_owned();
    callback_entries().write().unwrap_or_else(|e| e.into_inner()).insert(key, input);
}

/// The executable memory of the trampolines: regions mapped once and never
/// returned, divided into slices of [`SLICE_SIZE`] bytes with a free list.
mod executable_memory {
    /// The largest LLVM trampoline fits (x86-64: 23 bytes; AArch64: 32).
    pub const SLICE_SIZE: usize = 64;
    const REGION_SIZE: usize = 64 * 1024;

    static FREE_SLOTS: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

    #[cfg(unix)]
    unsafe extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe extern "C" {
        fn pthread_jit_write_protect_np(enabled: i32);
        fn sys_icache_invalidate(start: *mut u8, len: usize);
    }
    #[cfg(windows)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualAlloc(addr: *mut u8, len: usize, ty: u32, protection: u32) -> *mut u8;
        fn FlushInstructionCache(process: usize, addr: *const u8, len: usize) -> i32;
        fn GetCurrentProcess() -> usize;
    }

    #[cfg(unix)]
    fn map_pages() -> Option<usize> {
        const PROT_RWX: i32 = 1 | 2 | 4;
        const MAP_PRIVATE: i32 = 2;
        #[cfg(target_os = "linux")]
        const MAP_ANON: i32 = 0x20;
        #[cfg(not(target_os = "linux"))]
        const MAP_ANON: i32 = 0x1000;
        // On macOS memory that becomes code needs `MAP_JIT` (and writing to it, on
        // Apple Silicon, `pthread_jit_write_protect_np`).
        #[cfg(target_os = "macos")]
        const MAP_JIT: i32 = 0x800;
        #[cfg(not(target_os = "macos"))]
        const MAP_JIT: i32 = 0;
        // SAFETY: a new anonymous mapping, without a file.
        let p = unsafe { mmap(std::ptr::null_mut(), REGION_SIZE, PROT_RWX, MAP_PRIVATE | MAP_ANON | MAP_JIT, -1, 0) };
        if p.is_null() || p as isize == -1 { None } else { Some(p as usize) }
    }

    #[cfg(windows)]
    fn map_pages() -> Option<usize> {
        const MEM_COMMIT_RESERVE: u32 = 0x1000 | 0x2000;
        const PAGE_EXECUTE_READWRITE: u32 = 0x40;
        // SAFETY: reserves and commits new pages.
        let p = unsafe { VirtualAlloc(std::ptr::null_mut(), REGION_SIZE, MEM_COMMIT_RESERVE, PAGE_EXECUTE_READWRITE) };
        if p.is_null() { None } else { Some(p as usize) }
    }

    pub fn allocate() -> Option<usize> {
        let mut free_list = FREE_SLOTS.lock().unwrap_or_else(|e| e.into_inner());
        if free_list.is_empty() {
            let base = map_pages()?;
            free_list.extend((0..REGION_SIZE / SLICE_SIZE).rev().map(|i| base + i * SLICE_SIZE));
        }
        free_list.pop()
    }

    pub fn free(p: usize) {
        FREE_SLOTS.lock().unwrap_or_else(|e| e.into_inner()).push(p);
    }

    /// Writes at `p` the trampoline that loads `ctx` into the `nest` register and
    /// jumps to `input`; returns the function pointer (`p` itself).
    pub fn write_trampoline(p: usize, input: usize, ctx: usize) -> usize {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: toggles the MAP_JIT protection only on this thread.
        unsafe {
            pthread_jit_write_protect_np(0)
        };
        // SAFETY: `p` is a slice of `SLICE_SIZE` writable bytes of this region,
        // used exclusively by this callback.
        unsafe { write_code(p as *mut u8, input, ctx) };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: likewise.
        unsafe {
            pthread_jit_write_protect_np(1)
        };
        sync_instructions(p);
        p
    }

    /// x86-64: `movabs r11, input; movabs r10, ctx; jmp r11`.
    #[cfg(target_arch = "x86_64")]
    unsafe fn write_code(p: *mut u8, input: usize, ctx: usize) {
        let mut c = Vec::with_capacity(23);
        c.extend_from_slice(&[0x49, 0xBB]);
        c.extend_from_slice(&(input as u64).to_le_bytes());
        c.extend_from_slice(&[0x49, 0xBA]);
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&[0x41, 0xFF, 0xE3]);
        // SAFETY: guaranteed by the caller (23 bytes out of 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// AArch64: `ldr x15, [pc+16]; ldr x17, [pc+20]; br x17; nop`, with the context
    /// at +16 and the entry at +24.
    #[cfg(target_arch = "aarch64")]
    unsafe fn write_code(p: *mut u8, input: usize, ctx: usize) {
        let instructions: [u32; 4] = [0x5800_008F, 0x5800_00B1, 0xD61F_0220, 0xD503_201F];
        let mut c = Vec::with_capacity(32);
        for i in instructions {
            c.extend_from_slice(&i.to_le_bytes());
        }
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&(input as u64).to_le_bytes());
        // SAFETY: guaranteed by the caller (32 bytes out of 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// The instruction cache sees the freshly written code.
    fn sync_instructions(p: usize) {
        #[cfg(windows)]
        // SAFETY: the slice has just been written in this process.
        unsafe {
            FlushInstructionCache(GetCurrentProcess(), p as *const u8, SLICE_SIZE)
        };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: invalidates the instruction cache of the written slice.
        unsafe {
            sys_icache_invalidate(p as *mut u8, SLICE_SIZE)
        };
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        // SAFETY: EL0 cache maintenance (`dc cvau`/`ic ivau`), like compiler-rt's
        // `__clear_cache`, line by line over the slice.
        unsafe {
            const LINE: usize = 64;
            let begin = p & !(LINE - 1);
            let mut a = begin;
            while a < p + SLICE_SIZE {
                std::arch::asm!("dc cvau, {0}", in(reg) a);
                a += LINE;
            }
            std::arch::asm!("dsb ish");
            a = begin;
            while a < p + SLICE_SIZE {
                std::arch::asm!("ic ivau, {0}", in(reg) a);
                a += LINE;
            }
            std::arch::asm!("dsb ish", "isb");
        };
        let _ = p;
    }
}

/// One position of a signature key (`lower/ffi.rs`): the letter of a C type or
/// `S<rti>.`, a struct/union by value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum KeyPart {
    Letter(char),
    Compound(i64),
}

/// The parts of the key: the return and the parameters.
fn key_parts(key: &str) -> (KeyPart, Vec<KeyPart>) {
    let mut part_list = Vec::new();
    let mut rest = key;
    let mut ret = None;
    while let Some(c) = rest.chars().next() {
        if c == '_' && ret.is_none() {
            ret = part_list.pop();
            rest = &rest[1..];
            continue;
        }
        if c == 'S' {
            let end = rest.find('.').unwrap_or(rest.len());
            part_list.push(KeyPart::Compound(rest[1..end].parse().unwrap_or(0)));
            rest = &rest[(end + 1).min(rest.len())..];
        } else {
            part_list.push(KeyPart::Letter(c));
            rest = &rest[c.len_utf8()..];
        }
    }
    (ret.unwrap_or(KeyPart::Letter('v')), part_list)
}

/// The C value of `v` (a Dart object) in the type of letter `l`, for the
/// exceptional return.
fn exceptional_bits(v: i64, l: char) -> Result<i64, String> {
    if v == 0 {
        return Ok(0);
    }
    if l == 'p' {
        return address_of(v).ok_or_else(|| "exceptionalReturn must be a Pointer".to_string());
    }
    if l == 'H' {
        return Err("exceptionalReturn must not be given for a Handle return".to_string());
    }
    HEAP.with(|h| {
        let h = h.borrow();
        let integer = h.int_of(v);
        match (l, h.double_of(v), h.bool_of(v)) {
            ('f' | 'd', Some(d), _) => Ok(d.to_bits() as i64),
            ('f' | 'd', None, _) => integer.map(|i| (i as f64).to_bits() as i64).ok_or_else(|| "exceptionalReturn must be a double".to_string()),
            ('b', _, Some(b)) => Ok(i64::from(b)),
            _ => integer.ok_or_else(|| "exceptionalReturn must be an int".to_string()),
        }
    })
}

/// The code of a closure (the identity of a top-level or static function, for the
/// `fromFunction` cache).
fn closure_code(clo: i64) -> i64 {
    HEAP.with(|h| h.borrow().closure(clo).map_or(0, |c| c.code))
}

/// Creates the trampoline of `ctx` and registers it.
fn install_callback(ctx: Box<CallbackContext>) -> Result<usize, String> {
    let rec = callback_entries().read().unwrap_or_else(|e| e.into_inner()).get(&ctx.key).copied();
    let Some(input) = rec else {
        let text = RTI.with(|u| u.borrow().text(ctx.fn_signature));
        return Err(format!("the native signature `{text}` was not compiled into this program (it must be written as a constant type argument)"));
    };
    let Some(memory) = executable_memory::allocate() else {
        return Err("could not allocate executable memory for a native callback".to_string());
    };
    let ctx = Box::into_raw(ctx);
    let f = executable_memory::write_trampoline(memory, input, ctx as usize);
    // SAFETY: the context has just been created and only this thread knows it.
    unsafe { (*ctx).trampoline = memory };
    live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).insert(f, ctx as usize);
    Ok(f)
}

/// `DartForge_ffi_callback_new(signature, function, exceptional, mode, port)`: the
/// C function pointer of a new callback (`port` is the listener's, or the site of a
/// `fromFunction`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_new(fn_signature: i64, function: i64, exceptional: i64, mode: i64, port: i64) -> i64 {
    let Some(ns) = object_type_type(fn_signature) else {
        throw_argument_error("native callback: signature type argument expected");
        return 0;
    };
    let key = match signature_key(ns) {
        Ok(c) => c,
        Err(m) => {
            throw_unsupported(&m);
            return 0;
        }
    };
    let (ret, params) = key_parts(&key);
    if mode == MODE_LISTENER && ret != KeyPart::Letter('v') {
        throw_argument_error("NativeCallable.listener callbacks must return void");
        return 0;
    }
    if mode == MODE_LISTENER && params.contains(&KeyPart::Letter('H')) {
        throw_argument_error("NativeCallable.listener callbacks cannot take a Handle");
        return 0;
    }
    let exceptional = match ret {
        // A returned struct: no exceptional return (the VM rejects it); with an
        // exception, the struct comes back zeroed.
        KeyPart::Compound(_) if exceptional != 0 => {
            throw_argument_error("exceptionalReturn must not be given for a struct or union return");
            return 0;
        }
        KeyPart::Compound(_) => 0,
        KeyPart::Letter(l) => match exceptional_bits(exceptional, l) {
            Ok(b) => b,
            Err(m) => {
                throw_argument_error(&m);
                return 0;
            }
        },
    };
    // `fromFunction`: `port` is the call site (one trampoline per site).
    let persistent = (mode == MODE_PERSISTENT).then(|| (key.clone(), port, closure_code(function), exceptional));
    if let Some(k) = &persistent
        && let Some(f) = PERSISTENT.with(|p| p.borrow().get(k).copied())
    {
        return f as i64;
    }
    let ctx = Box::new(CallbackContext {
        mode,
        isolate: isolate_id(),
        closure: if mode == MODE_LISTENER { 0 } else { function },
        port: if mode == MODE_LISTENER { port } else { 0 },
        fn_signature: ns,
        key,
        exceptional,
        active_list: 0,
        closed_flag: false,
        trampoline: 0,
    });
    let root = &*ctx as *const CallbackContext as i64;
    match install_callback(ctx) {
        Ok(f) => {
            if mode != MODE_LISTENER {
                HEAP.with(|h| h.borrow_mut().set_global_root(root, function));
            }
            if let Some(k) = persistent {
                PERSISTENT.with(|p| p.borrow_mut().insert(k, f));
            }
            f as i64
        }
        Err(m) => {
            throw_unsupported(&m);
            0
        }
    }
}

/// Releases the context and the trampoline (with no call in progress).
fn release_callback(ctx: *mut CallbackContext) {
    // SAFETY: the context left the registry; nobody else reaches it.
    let ctx = unsafe { Box::from_raw(ctx) };
    executable_memory::free(ctx.trampoline);
}

/// `DartForge_ffi_callback_delete(address)`: the `close()` of a `NativeCallable`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_delete(address: i64) {
    let Some(ctx) = live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).remove(&(address as usize)) else {
        return;
    };
    let ctx = ctx as *mut CallbackContext;
    HEAP.with(|h| h.borrow_mut().release_global_root(ctx as i64));
    // SAFETY: the context belongs to this isolate (`close()` runs on it).
    let in_use_flag = unsafe {
        (*ctx).closed_flag = true;
        (*ctx).closure = 0;
        (*ctx).active_list > 0
    };
    if !in_use_flag {
        release_callback(ctx);
    }
}

/// `DartForge_ffi_callback_keep(delta)`: the counter of local `NativeCallable`s
/// that keep the isolate alive (`keepIsolateAlive`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_keep(delta: i64) {
    adjust_callbacks_keeping_alive(delta);
}

/// The C entry started: 0 = call the closure here; 1 = post to the listener. A
/// local callback called outside the owning isolate's thread is fatal.
///
/// # Safety
/// `ctx` is the trampoline's context (alive while the callback is not closed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_enter(ctx: *mut CallbackContext) -> i64 {
    // SAFETY: guaranteed by the caller.
    let c = unsafe { &mut *ctx };
    if c.mode == MODE_LISTENER {
        return 1;
    }
    if c.closed_flag || isolate_id() != c.isolate {
        eprintln!("Cannot invoke native callback outside an isolate.");
        std::process::abort();
    }
    c.active_list += 1;
    0
}

/// The callback's closure (the entry calls through it).
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_closure(ctx: i64) -> i64 {
    // SAFETY: guaranteed by the caller.
    unsafe { (*(ctx as *const CallbackContext)).closure }
}

/// The Dart `Pointer<X>` of argument `i` (address `address`).
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_pointer(ctx: i64, i: i64, address: i64) -> i64 {
    // SAFETY: guaranteed by the caller.
    let ns = unsafe { (*(ctx as *const CallbackContext)).fn_signature };
    let ty = RTI.with(|u| match u.borrow().ty(ns) {
        Type::Function { pos, .. } => pos.get(i as usize).copied(),
        _ => None,
    });
    new_pointer(address, ty)
}

/// The callback's closure under ARC, with a code owner.
///
/// The closure stays a root of the context until `close()`; the owner belongs to
/// generated code, which transfers it or calls release exactly once. Neither
/// allocates on the heap, runs Dart nor leaves a pending exception.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_ffi_callback_closure_owned_v1(ctx: i64) -> i64 {
    // SAFETY: the contract above.
    let closure = unsafe { dartforge_ffi_callback_closure(ctx) };
    dartforge_arc_retain(closure);
    closure
}

/// The `Pointer<X>` of argument `i` under ARC, with a code owner.
///
/// The Dart helper `_dartforgePonteiro` runs inside a native scope: the token of
/// the Owned port's return stays in it and is copied into the caller's owner
/// before closing. An exception from the helper stays pending and the result is
/// null, without an owner. May allocate, collect and run Dart.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
///
/// # Panics
/// Missing helper, an incompatible Owned registration or an unclosed reentrant
/// scope; aborts at the C boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_ffi_callback_pointer_owned_v1(ctx: i64, i: i64, address: i64) -> i64 {
    // SAFETY: the contract above.
    owner_in_native_scope(|| unsafe { dartforge_ffi_callback_pointer(ctx, i, address) })
}

/// The C entry finished the call: returns 1 (and the exceptional return in `out`)
/// if the closure threw — the exception is discarded, as in the VM. Releases the
/// context if it was closed during the call.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`]; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_exit(ctx: *mut CallbackContext, out: *mut i64) -> u8 {
    // SAFETY: guaranteed by the caller.
    let c = unsafe { &mut *ctx };
    c.active_list -= 1;
    let threw = dartforge_exception_pending() != 0;
    if threw {
        dartforge_exception_clear();
        // SAFETY: guaranteed by the caller.
        unsafe { *out = c.exceptional };
    }
    if c.closed_flag && c.active_list == 0 {
        release_callback(ctx);
    }
    u8::from(threw)
}

/// The C entry of a `listener`: the `n` arguments (bits of each C type) become the
/// message `[context, a0, a1, …]` to the `NativeCallable`'s port.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`]; `args` has `n` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_post(ctx: *const CallbackContext, args: *const i64, n: i64) {
    // SAFETY: guaranteed by the caller.
    let c = unsafe { &*ctx };
    // SAFETY: likewise.
    let bits = unsafe { std::slice::from_raw_parts(args, n.max(0) as usize) };
    let mut items = Vec::with_capacity(bits.len() + 1);
    items.push(Portable::Int(ctx as i64));
    let (_, params) = key_parts(&c.key);
    for (b, p) in bits.iter().zip(params) {
        items.push(match p {
            KeyPart::Letter('f' | 'd') => Portable::Double(f64::from_bits(*b as u64)),
            KeyPart::Letter('b') => Portable::Bool(*b != 0),
            // A struct: the bytes (the address is only valid during the call).
            KeyPart::Compound(rti) => {
                let n = compound_from_rti(rti).map_or(0, |c| c.size as usize);
                // SAFETY: the C entry passes the address of the argument's `n`
                // bytes, alive during the call.
                Portable::Bytes(unsafe { std::slice::from_raw_parts(*b as usize as *const u8, n) }.to_vec())
            }
            KeyPart::Letter(_) => Portable::Int(*b),
        });
    }
    post(c.port, Portable::List(items).to_graph());
}

/// `DartForge_ffi_callback_args(message)`: the Dart arguments of a `listener`
/// message (the pointers become `Pointer<X>`s of the signature).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_args(message: i64) -> i64 {
    // The elements of the message (a core list), in the `Ref` position.
    let items: Vec<crate::heap::Value> = HEAP.with(|h| {
        let h = h.borrow();
        if !h.is_list(message) {
            return Vec::new();
        }
        (0..h.list_len(message)).map(|i| h.list_get(message, i)).collect()
    });
    let integer = |v: &crate::heap::Value| argument_int(*v).unwrap_or(0);
    let Some(ctx) = items.first().map(integer) else { return message };
    let alive = live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).values().any(|&c| c as i64 == ctx);
    if !alive {
        return dart_fixed_list(&[]);
    }
    // SAFETY: the context is registered (alive).
    let (key, ns) = unsafe {
        let c = &*(ctx as *const CallbackContext);
        (c.key.clone(), c.fn_signature)
    };
    let types: Vec<i64> = RTI.with(|u| match u.borrow().ty(ns) {
        Type::Function { pos, .. } => pos.to_vec(),
        _ => Vec::new(),
    });
    // Each argument becomes a `Ref` rooted in the frame (an integer's box, the
    // `Pointer`, the copied struct), and the list is built at the end.
    let n = items.len().saturating_sub(1);
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(n.max(1)));
    let (_, params) = key_parts(&key);
    let mut out = Vec::with_capacity(n);
    for (i, (v, p)) in items.iter().skip(1).zip(params).enumerate() {
        let x = match p {
            KeyPart::Letter('p') => new_pointer(integer(v), types.get(i).copied()),
            // The struct's bytes become a struct over Dart memory.
            KeyPart::Compound(rti) => {
                let bytes = match v {
                    crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().typed_bytes(*r).map(<[u8]>::to_vec)).unwrap_or_default(),
                    _ => Vec::new(),
                };
                dartforge_ffi_compound_copy(rti, bytes.as_ptr() as i64)
            }
            KeyPart::Letter(_) => HEAP.with(|h| h.borrow_mut().to_ref(*v)),
        };
        HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
        out.push(x);
    }
    let list = dart_fixed_list(&out);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    list
}

/// The `int` of a list element (a scalar of the compact form, `Smi` or `_Mint`).
fn argument_int(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(i) => Some(i),
        crate::heap::Value::Bool(b) => Some(i64::from(b)),
        crate::heap::Value::Double(_) => None,
        crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().int_of(r)),
    }
}

#[cfg(test)]
mod tests_owned_callback {
    use super::*;

    #[test]
    fn callback_closure_gains_an_owner_and_releases_on_release() {
        let closure = dartforge_arc_object_owned_v1(123, 1);
        let ctx = Box::new(CallbackContext {
            mode: MODE_LOCAL, isolate: 0, closure, port: 0, fn_signature: 0,
            key: String::new(), exceptional: 0, active_list: 0,
            closed_flag: false, trampoline: 0,
        });
        let address = (&*ctx as *const CallbackContext) as i64;
        // SAFETY: the context above lives until the end of the test.
        let owner = unsafe { dartforge_arc_ffi_callback_closure_owned_v1(address) };
        assert_eq!(owner, closure);
        assert_eq!(dartforge_exception_pending(), 0);
        dartforge_arc_release(closure);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 3);
        dartforge_arc_release(owner);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 0);
    }
}

#[cfg(test)]
mod finalizer_callback_tests {
    use super::*;

    #[test]
    fn finalizer_rejects_sync_trampolines_before_creating_attachment() {
        for mode in [MODE_PERSISTENT, MODE_LOCAL, MODE_LISTENER] {
            let ctx = Box::new(CallbackContext {
                mode, isolate: 0, closure: 0, port: 0, fn_signature: 0,
                key: String::new(), exceptional: 0, active_list: 0,
                closed_flag: false, trampoline: 0,
            });
            // An exclusive identity for this test registration; it is never called.
            let address = (&*ctx as *const CallbackContext) as usize;
            live_callbacks().lock().unwrap().insert(address, address);
            let synchronous = mode != MODE_LISTENER;
            assert_eq!(callback_requires_isolate_thread(address), synchronous);
            if synchronous {
                dartforge_exception_clear();
                // Null handles would reveal an improper creation of an attachment.
                dartforge_native_DartForge_finalizer_attach_native(0, 0, address as i64, 0, 0, 17);
                assert_eq!(dartforge_exception_pending(), 1);
                dartforge_exception_clear();
            }
            live_callbacks().lock().unwrap().remove(&address);
            assert!(!callback_requires_isolate_thread(address));
        }
    }
}
