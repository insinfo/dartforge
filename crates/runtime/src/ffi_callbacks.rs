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
//   inside the callback itself is allowed. Published contexts and trampolines
//   remain stable until process exit; closing releases the Dart closure root,
//   while late calls use only the retained native tombstone.

/// Modes of a callback (the `mode` of `DartForge_ffi_callback_new`).
const MODE_PERSISTENT: i64 = 0;
const MODE_LOCAL: i64 = 1;
const MODE_LISTENER: i64 = 2;
const MODE_SYNC: i64 = 3;

thread_local! {
    // Only live local entries on their native owner thread modify this depth.
    // Foreign listeners/synchronous requests and late calls never consult it.
    static OWNER_FFI_CALLBACK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn in_owner_ffi_callback() -> bool {
    OWNER_FFI_CALLBACK_DEPTH.with(|depth| depth.get() != 0)
}

/// The state of a callback; its address is the trampoline's `nest` value.
pub struct CallbackContext {
    mode: i64,
    /// The owning isolate (queue id).
    isolate: u64,
    owner_thread: std::thread::ThreadId,
    sync_timeout: std::time::Duration,
    compound_sizes: Vec<usize>,
    /// The closure (local and persistent modes), a root of the owner's heap.
    closure: std::sync::atomic::AtomicI64,
    /// The port of the `NativeCallable.listener`.
    port: i64,
    /// The native signature (RTI), for the arguments' `Pointer<X>`s.
    fn_signature: i64,
    /// The signature key (`i_pp`): letters of the return and of the parameters.
    key: String,
    /// The exceptional return value, in the bits of the return's C type.
    exceptional: i64,
    /// Calls in progress (the closure may close the callback from inside).
    active_list: std::sync::atomic::AtomicU32,
    closed_flag: std::sync::atomic::AtomicBool,
    late_calls: std::sync::atomic::AtomicU64,
    /// The C function pointer (the trampoline).
    trampoline: usize,
}

/// Process-wide diagnostic count of retired callback invocations/dropped posts.
static LATE_CALLBACK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn record_late_callback(c: &CallbackContext) {
    c.late_calls
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let _ = LATE_CALLBACK_CALLS.fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |count| Some(count.saturating_add(1)),
    );
}

/// Returns the process-wide count of late callback invocations.
/// The diagnostic counter saturates at i64::MAX and does not access Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_late_calls() -> i64 {
    LATE_CALLBACK_CALLS
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(i64::MAX as u64) as i64
}

/// Function pointer → context, of the live callbacks.
fn live_callbacks() -> &'static std::sync::Mutex<crate::hash::HashMap<usize, usize>> {
    static T: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<usize, usize>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Recognizes synchronous trampolines that require the owning isolate's thread.
/// Listeners only post messages and do not enter Dart on this thread.
fn callback_requires_isolate_thread(address: usize) -> bool {
    let live_set = live_callbacks().lock().unwrap_or_else(|e| e.into_inner());
    let Some(&ctx) = live_set.get(&address) else {
        return false;
    };
    // SAFETY: published contexts are retained for the process lifetime, and mode
    // is immutable. The registry includes only callbacks that are still live.
    let mode = unsafe { (*(ctx as *const CallbackContext)).mode };
    matches!(mode, MODE_PERSISTENT | MODE_LOCAL | MODE_SYNC)
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
    callback_entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(key, input, 0);
}

/// The executable memory of the trampolines: regions mapped once and never
/// returned, divided into slices of [`SLICE_SIZE`] bytes. Published slots are
/// never recycled: native callers may retain their addresses indefinitely.
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

    /// WebAssembly code cannot write executable memory: a module never makes a
    /// trampoline (`create` refuses native callbacks there first).
    #[cfg(target_arch = "wasm32")]
    fn map_pages() -> Option<usize> {
        None
    }

    #[cfg(target_arch = "wasm32")]
    unsafe fn write_code(_p: *mut u8, _input: usize, _ctx: usize) {
        unreachable!("WebAssembly has no executable memory for trampolines")
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
        let p = unsafe {
            mmap(
                std::ptr::null_mut(),
                REGION_SIZE,
                PROT_RWX,
                MAP_PRIVATE | MAP_ANON | MAP_JIT,
                -1,
                0,
            )
        };
        if p.is_null() || p as isize == -1 {
            None
        } else {
            Some(p as usize)
        }
    }

    #[cfg(windows)]
    fn map_pages() -> Option<usize> {
        const MEM_COMMIT_RESERVE: u32 = 0x1000 | 0x2000;
        const PAGE_EXECUTE_READWRITE: u32 = 0x40;
        // SAFETY: reserves and commits new pages.
        let p = unsafe {
            VirtualAlloc(
                std::ptr::null_mut(),
                REGION_SIZE,
                MEM_COMMIT_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        if p.is_null() { None } else { Some(p as usize) }
    }

    // Only slots whose address was never returned or stored may be recycled.
    pub(super) fn release_unpublished(memory: usize) {
        FREE_SLOTS.lock().unwrap_or_else(|e| e.into_inner()).push(memory);
    }

    pub fn allocate() -> Option<usize> {
        let mut free_list = FREE_SLOTS.lock().unwrap_or_else(|e| e.into_inner());
        if free_list.is_empty() {
            let base = map_pages()?;
            free_list.extend(
                (0..REGION_SIZE / SLICE_SIZE)
                    .rev()
                    .map(|i| base + i * SLICE_SIZE),
            );
        }
        free_list.pop()
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
            ('f' | 'd', None, _) => integer
                .map(|i| (i as f64).to_bits() as i64)
                .ok_or_else(|| "exceptionalReturn must be a double".to_string()),
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
    // dart2wasm has no native callbacks either: a module cannot write code, and
    // its function table is fixed at link time.
    if cfg!(target_arch = "wasm32") {
        return Err("native callbacks (NativeCallable, Pointer.fromFunction) are not available in WebAssembly".to_string());
    }
    let Some(memory) = executable_memory::allocate() else {
        return Err("could not allocate executable memory for a native callback".to_string());
    };
    // The owner lease is acquired only after executable allocation succeeds.
    // Retirement and selection share a lock; after this point the chosen code
    // is permanently resident even if the callback is immediately closed.
    let input = callback_entries().lock().unwrap_or_else(|e| e.into_inner()).publish(&ctx.key);
    let Some(input) = input else {
        executable_memory::release_unpublished(memory);
        let text = RTI.with(|u| u.borrow().text(ctx.fn_signature));
        return Err(format!(
            "the native signature `{text}` was not compiled into this program (it must be written as a constant type argument)"
        ));
    };
    let ctx = Box::into_raw(ctx);
    let f = executable_memory::write_trampoline(memory, input, ctx as usize);
    // SAFETY: the context has just been created and only this thread knows it.
    unsafe { (*ctx).trampoline = memory };
    live_callbacks()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(f, ctx as usize);
    Ok(f)
}

/// `DartForge_ffi_callback_new(signature, function, exceptional, mode, port)`: the
/// C function pointer of a new callback (`port` is the listener's, or the site of a
/// `fromFunction`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_new(
    fn_signature: i64,
    function: i64,
    exceptional: i64,
    mode: i64,
    port: i64,
) -> i64 {
    create_callback(
        fn_signature,
        function,
        exceptional,
        mode,
        port,
        std::time::Duration::ZERO,
    )
}

/// Creates a synchronous callback with a finite pre-claim queue timeout.
/// Handle parameters/returns and compound returns are unsupported.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_sync_new(
    fn_signature: i64,
    function: i64,
    exceptional: i64,
    port: i64,
    timeout_micros: i64,
) -> i64 {
    if timeout_micros <= 0 || port <= 0 {
        throw_argument_error(
            "Synchronous callback requires a positive queue timeout and owner port",
        );
        return 0;
    }
    let timeout = std::time::Duration::from_micros(timeout_micros as u64);
    if crate::clock::Instant::now().checked_add(timeout).is_none() {
        throw_argument_error("Synchronous callback queue timeout exceeds the native clock range");
        return 0;
    }
    create_callback(
        fn_signature,
        function,
        exceptional,
        MODE_SYNC,
        port,
        timeout,
    )
}

fn create_callback(
    fn_signature: i64,
    function: i64,
    exceptional: i64,
    mode: i64,
    port: i64,
    sync_timeout: std::time::Duration,
) -> i64 {
    if !matches!(
        mode,
        MODE_PERSISTENT | MODE_LOCAL | MODE_LISTENER | MODE_SYNC
    ) {
        throw_argument_error("Invalid native callback mode");
        return 0;
    }
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
    if mode == MODE_SYNC {
        if port <= 0 || sync_timeout.is_zero() {
            throw_argument_error(
                "Synchronous callback requires an owner port and positive queue timeout",
            );
            return 0;
        }
        if ret == KeyPart::Letter('H') || params.contains(&KeyPart::Letter('H')) {
            throw_unsupported("Synchronous callbacks cannot take or return Handle values");
            return 0;
        }
        if matches!(ret, KeyPart::Compound(_)) {
            throw_unsupported("Synchronous callbacks cannot return structs or unions");
            return 0;
        }
    }
    // Native threads cannot consult the owning isolate's RTI or heap.
    let compound_sizes = params
        .iter()
        .map(|part| match part {
            KeyPart::Compound(rti) => compound_from_rti(*rti).map(|layout| layout.size as usize),
            _ => Some(0),
        })
        .collect::<Option<Vec<_>>>();
    let Some(compound_sizes) = compound_sizes else {
        throw_unsupported("Native callback compound argument has no known layout");
        return 0;
    };
    let exceptional = match ret {
        // A returned struct: no exceptional return (the VM rejects it); with an
        // exception, the struct comes back zeroed.
        KeyPart::Compound(_) if exceptional != 0 => {
            throw_argument_error(
                "exceptionalReturn must not be given for a struct or union return",
            );
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
    let persistent =
        (mode == MODE_PERSISTENT).then(|| (key.clone(), port, closure_code(function), exceptional));
    if let Some(k) = &persistent
        && let Some(f) = PERSISTENT.with(|p| p.borrow().get(k).copied())
    {
        return f as i64;
    }
    let ctx = Box::new(CallbackContext {
        mode,
        isolate: isolate_id(),
        owner_thread: std::thread::current().id(),
        sync_timeout,
        compound_sizes,
        closure: std::sync::atomic::AtomicI64::new(if mode == MODE_LISTENER {
            0
        } else {
            function
        }),
        port: if matches!(mode, MODE_LISTENER | MODE_SYNC) {
            port
        } else {
            0
        },
        fn_signature: ns,
        key,
        exceptional,
        active_list: std::sync::atomic::AtomicU32::new(0),
        closed_flag: std::sync::atomic::AtomicBool::new(false),
        late_calls: std::sync::atomic::AtomicU64::new(0),
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

/// Retires a published callback without invalidating its native address.
/// Must run on the owning isolate, after removal from the live registry.
fn retire_callback(ctx: *const CallbackContext) {
    // SAFETY: published contexts are never freed; only the owner touches its heap.
    let c = unsafe { &*ctx };
    c.closed_flag
        .store(true, std::sync::atomic::Ordering::Release);
    cancel_sync_requests(ctx as usize, false);
    c.closure.store(0, std::sync::atomic::Ordering::Release);
    HEAP.with(|h| h.borrow_mut().release_global_root(ctx as i64));
}

/// Retires every callback before the owning isolate's heap is destroyed.
/// Native pointers remain callable, including persistent `fromFunction` entries.
pub(crate) fn retire_isolate_callbacks() {
    let mine = isolate_id();
    let contexts = {
        let mut live = live_callbacks().lock().unwrap_or_else(|e| e.into_inner());
        let mut contexts = Vec::new();
        live.retain(|_, ctx| {
            // SAFETY: published contexts are retained; isolate is immutable.
            let belongs = unsafe { (*(*ctx as *const CallbackContext)).isolate == mine };
            if belongs {
                // Publish retirement before removing thread-affinity metadata.
                unsafe {
                    (*(*ctx as *const CallbackContext))
                        .closed_flag
                        .store(true, std::sync::atomic::Ordering::Release)
                };
                contexts.push(*ctx);
            }
            !belongs
        });
        contexts
    };
    for ctx in contexts {
        retire_callback(ctx as *const CallbackContext);
        cancel_sync_requests(ctx, true);
    }
    // Explicit shutdown normally precedes TLS destruction. A repeated cleanup
    // must not fail merely because the cache has already been destroyed.
    let _ = PERSISTENT.try_with(|p| p.borrow_mut().clear());
}

/// `DartForge_ffi_callback_delete(address)`: the `close()` of a `NativeCallable`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_delete(address: i64) {
    let mine = std::thread::current().id();
    let ctx = {
        let mut live = live_callbacks().lock().unwrap_or_else(|e| e.into_inner());
        let Some(&ctx) = live.get(&(address as usize)) else {
            return;
        };
        // SAFETY: published contexts are retained; isolate is immutable.
        if unsafe { (*(ctx as *const CallbackContext)).owner_thread } != mine {
            eprintln!("Cannot close native callback outside its owning isolate.");
            std::process::abort();
        }
        // Finalizer classification must never observe an unregistered LIVE
        // synchronous callback. Publish its tombstone while holding the registry.
        unsafe {
            (*(ctx as *const CallbackContext))
                .closed_flag
                .store(true, std::sync::atomic::Ordering::Release)
        };
        live.remove(&(address as usize));
        ctx
    };
    retire_callback(ctx as *const CallbackContext);
}

/// `DartForge_ffi_callback_keep(delta)`: the counter of local `NativeCallable`s
/// that keep the isolate alive (`keepIsolateAlive`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_keep(delta: i64) {
    adjust_callbacks_keeping_alive(delta);
}

/// The C entry started: 0 = call locally; 1 = post listener; 2 = late call.
/// A LIVE local callback outside the owning isolate is still fatal. A closed
/// callback does not enter an isolate, touch a Dart heap or inspect exceptions.
///
/// # Safety
/// `ctx` is a published callback context, retained for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_enter(ctx: *mut CallbackContext) -> i64 {
    // SAFETY: guaranteed by the caller; shared references avoid foreign-thread
    // aliasing during close. Mutable state is atomic, metadata is immutable.
    let c = unsafe { &*ctx };
    if c.closed_flag.load(std::sync::atomic::Ordering::Acquire) {
        record_late_callback(c);
        return 2;
    }
    if c.mode == MODE_LISTENER {
        return 1;
    }
    if std::thread::current().id() != c.owner_thread {
        if c.mode == MODE_SYNC {
            return 3;
        }
        eprintln!("Cannot invoke native callback outside an isolate.");
        std::process::abort();
    }
    c.active_list
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    OWNER_FFI_CALLBACK_DEPTH.with(|depth| depth.set(depth.get().checked_add(1).expect("Native callback depth overflow")));
    0
}

/// Returns the retained exceptional C bits of a late invocation.
/// No Dart heap, pending exception or owner-thread access is needed.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_late_return(ctx: *const i64) -> i64 {
    // SAFETY: exceptional is immutable and the context is retained.
    unsafe { (*(ctx.cast::<CallbackContext>())).exceptional }
}

/// The callback's closure (the entry calls through it).
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_closure(ctx: i64) -> i64 {
    // SAFETY: guaranteed by the caller.
    unsafe {
        (*(ctx as *const CallbackContext))
            .closure
            .load(std::sync::atomic::Ordering::Acquire)
    }
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
/// The Dart helper `_dartforgePointer` runs inside a native scope: the token of
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
pub unsafe extern "C" fn dartforge_arc_ffi_callback_pointer_owned_v1(
    ctx: i64,
    i: i64,
    address: i64,
) -> i64 {
    // SAFETY: the contract above.
    owner_in_native_scope(|| unsafe { dartforge_ffi_callback_pointer(ctx, i, address) })
}

/// The C entry finished the call: returns 1 (and the exceptional return in `out`)
/// if the closure threw — the exception is discarded, as in the VM. A callback
/// closed during the call remains a native tombstone after this frame leaves.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`]; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_exit(
    ctx: *mut CallbackContext,
    out: *mut i64,
) -> u8 {
    // SAFETY: guaranteed by the caller.
    let c = unsafe { &*ctx };
    let active = c
        .active_list
        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    assert!(active > 0, "Callback exit without an active local entry");
    OWNER_FFI_CALLBACK_DEPTH.with(|depth| depth.set(depth.get().checked_sub(1).expect("Native callback exit without an owner entry")));
    let threw = dartforge_exception_pending() != 0;
    if threw {
        dartforge_exception_clear();
        // SAFETY: guaranteed by the caller.
        unsafe { *out = c.exceptional };
    }
    u8::from(threw)
}

/// Native request state: deadlines only cancel unclaimed work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncRequestState {
    Pending,
    Running,
    Completed(i64),
    Cancelled,
}

struct SyncCallbackRequest {
    context: usize,
    owner_thread: std::thread::ThreadId,
    deadline: crate::clock::Instant,
    exceptional: i64,
    state: std::sync::Mutex<SyncRequestState>,
    ready: std::sync::Condvar,
}

fn sync_requests()
-> &'static std::sync::Mutex<crate::hash::HashMap<i64, std::sync::Arc<SyncCallbackRequest>>> {
    static REQUESTS: std::sync::OnceLock<
        std::sync::Mutex<crate::hash::HashMap<i64, std::sync::Arc<SyncCallbackRequest>>>,
    > = std::sync::OnceLock::new();
    REQUESTS.get_or_init(Default::default)
}

fn sync_request(id: i64) -> Option<std::sync::Arc<SyncCallbackRequest>> {
    sync_requests()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&id)
        .cloned()
}

fn sync_wait_edges()
-> &'static std::sync::Mutex<crate::hash::HashMap<std::thread::ThreadId, std::thread::ThreadId>> {
    static EDGES: std::sync::OnceLock<
        std::sync::Mutex<crate::hash::HashMap<std::thread::ThreadId, std::thread::ThreadId>>,
    > = std::sync::OnceLock::new();
    EDGES.get_or_init(Default::default)
}

fn wait_edge_would_cycle(
    edges: &crate::hash::HashMap<std::thread::ThreadId, std::thread::ThreadId>,
    caller: std::thread::ThreadId,
    owner: std::thread::ThreadId,
) -> bool {
    if caller == owner || edges.contains_key(&caller) {
        return true;
    }
    let mut current = owner;
    for _ in 0..=edges.len() {
        if current == caller {
            return true;
        }
        let Some(&next) = edges.get(&current) else {
            return false;
        };
        current = next;
    }
    true
}

struct SyncWaitEdge(std::thread::ThreadId);
impl SyncWaitEdge {
    fn register(owner: std::thread::ThreadId) -> Option<Self> {
        let caller = std::thread::current().id();
        let mut edges = sync_wait_edges().lock().unwrap_or_else(|e| e.into_inner());
        if wait_edge_would_cycle(&edges, caller, owner) {
            return None;
        }
        edges.insert(caller, owner);
        Some(Self(caller))
    }
}
impl Drop for SyncWaitEdge {
    fn drop(&mut self) {
        sync_wait_edges()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

fn cancel_sync_requests(context: usize, owner_stopped: bool) {
    let mut requests = sync_requests().lock().unwrap_or_else(|e| e.into_inner());
    requests.retain(|_, request| {
        if request.context != context {
            return true;
        }
        let mut state = request.state.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*state, SyncRequestState::Pending)
            || (owner_stopped && matches!(*state, SyncRequestState::Running))
        {
            *state = SyncRequestState::Cancelled;
            request.ready.notify_all();
            return false;
        }
        true
    });
}

fn wait_sync_request(request: &SyncCallbackRequest) -> i64 {
    let mut state = request.state.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        match *state {
            SyncRequestState::Completed(bits) => return bits,
            SyncRequestState::Cancelled => return request.exceptional,
            SyncRequestState::Running => {
                // Claimed handlers may borrow caller-owned native buffers. A
                // deadline must not return those buffers while Dart still runs.
                state = request.ready.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            SyncRequestState::Pending => {
                let now = crate::clock::Instant::now();
                if now >= request.deadline {
                    *state = SyncRequestState::Cancelled;
                    request.ready.notify_all();
                    return request.exceptional;
                }
                let remaining = request.deadline.saturating_duration_since(now);
                (state, _) = request
                    .ready
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|e| e.into_inner());
            }
        }
    }
}

/// Copies native argument words using metadata computed on the owner thread.
/// # Safety
/// Compound words point to readable arguments of the precomputed byte size.
unsafe fn callback_portable_arguments(c: &CallbackContext, bits: &[i64]) -> Vec<Portable> {
    let (_, params) = key_parts(&c.key);
    bits.iter()
        .zip(params)
        .enumerate()
        .map(|(i, (bits, part))| match part {
            KeyPart::Letter('f' | 'd') => Portable::Double(f64::from_bits(*bits as u64)),
            KeyPart::Letter('b') => Portable::Bool(*bits != 0),
            KeyPart::Compound(_) => {
                let size = c.compound_sizes[i];
                let bytes = if size == 0 {
                    Vec::new()
                } else {
                    // SAFETY: the generated entry owns these bytes throughout the call.
                    unsafe { std::slice::from_raw_parts(*bits as usize as *const u8, size) }
                        .to_vec()
                };
                Portable::Bytes(bytes)
            }
            KeyPart::Letter(_) => Portable::Int(*bits),
        })
        .collect()
}

/// Delivers a foreign-thread synchronous request without touching Dart TLS.
/// Returns C bits; pre-claim timeout/close/cycle rejection uses exceptional bits.
/// # Safety
/// `ctx` is a retained MODE_SYNC context; args has n native signature words,
/// including readable compound arguments. Caller buffers remain valid until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_sync(
    ctx: *const i64,
    args: *const i64,
    n: i64,
) -> i64 {
    // SAFETY: the generated entry provides a retained native context.
    let c = unsafe { &*ctx.cast::<CallbackContext>() };
    if c.closed_flag.load(std::sync::atomic::Ordering::Acquire) {
        record_late_callback(c);
        return c.exceptional;
    }
    let (_, params) = key_parts(&c.key);
    if c.mode != MODE_SYNC || n < 0 || n as usize != params.len() || (n > 0 && args.is_null()) {
        return c.exceptional;
    }
    let Some(_edge) = SyncWaitEdge::register(c.owner_thread) else {
        return c.exceptional;
    };
    let Some(deadline) = crate::clock::Instant::now().checked_add(c.sync_timeout) else {
        return c.exceptional;
    };
    static NEXT_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let Ok(id) = NEXT_ID.fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |id| id.checked_add(1),
    ) else {
        return c.exceptional;
    };
    let request = std::sync::Arc::new(SyncCallbackRequest {
        context: ctx as usize,
        owner_thread: c.owner_thread,
        deadline,
        exceptional: c.exceptional,
        state: std::sync::Mutex::new(SyncRequestState::Pending),
        ready: std::sync::Condvar::new(),
    });
    {
        let mut requests = sync_requests().lock().unwrap_or_else(|e| e.into_inner());
        // Admission and retirement share this mutex: close cannot miss a request
        // admitted before its closed flag, and no request is admitted afterwards.
        if c.closed_flag.load(std::sync::atomic::Ordering::Acquire) {
            return c.exceptional;
        }
        requests.insert(id, request.clone());
    }
    let bits = if n == 0 {
        &[]
    } else {
        // SAFETY: the generated entry supplies n readable words.
        unsafe { std::slice::from_raw_parts(args, n as usize) }
    };
    let mut items = vec![Portable::Int(ctx as i64), Portable::Int(id)];
    // SAFETY: the native entry and foreign caller retain compound buffers.
    items.extend(unsafe { callback_portable_arguments(c, bits) });
    // No request/table/heap lock is held across posting or waiting.
    post(c.port, Portable::List(items).to_graph());
    let result = wait_sync_request(&request);
    sync_requests()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    result
}

fn request_from_message(message: i64) -> Option<(i64, std::sync::Arc<SyncCallbackRequest>)> {
    let (context, id) = HEAP.with(|h| {
        let h = h.borrow();
        if !h.is_list(message) || h.list_len(message) < 2 {
            return None;
        }
        let number = |index| match h.list_get(message, index) {
            crate::heap::Value::Int(value) => Some(value),
            crate::heap::Value::Ref(value) => h.int_of(value),
            _ => None,
        };
        Some((number(0)?, number(1)?))
    })?;
    let request = sync_request(id)?;
    if request.context != context as usize || request.owner_thread != std::thread::current().id() {
        return None;
    }
    Some((id, request))
}

fn claim_sync_request(request: &SyncCallbackRequest, now: crate::clock::Instant) -> bool {
    let mut state = request.state.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(*state, SyncRequestState::Pending) {
        return false;
    }
    // SAFETY: requests only carry retained callback contexts.
    let closed = unsafe {
        (*(request.context as *const CallbackContext))
            .closed_flag
            .load(std::sync::atomic::Ordering::Acquire)
    };
    if closed || now >= request.deadline {
        *state = SyncRequestState::Cancelled;
        request.ready.notify_all();
        return false;
    }
    *state = SyncRequestState::Running;
    true
}

/// Claims a queued request on its owner before allocating/boxing Dart arguments.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_sync_claim(message: i64) -> u8 {
    let Some((id, request)) = request_from_message(message) else {
        return 0;
    };
    let claimed = claim_sync_request(&request, crate::clock::Instant::now());
    let cancelled = matches!(
        *request.state.lock().unwrap_or_else(|e| e.into_inner()),
        SyncRequestState::Cancelled
    );
    if !claimed && cancelled {
        sync_requests()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }
    u8::from(claimed)
}

fn sync_result_bits(c: &CallbackContext, result: i64) -> Option<i64> {
    let (ret, _) = key_parts(&c.key);
    let KeyPart::Letter(ret) = ret else {
        return None;
    };
    match ret {
        'v' => Some(0),
        'p' => address_of(result),
        'b' => HEAP.with(|h| h.borrow().bool_of(result)).map(i64::from),
        'f' | 'd' => HEAP
            .with(|h| h.borrow().double_of(result))
            .map(|value| value.to_bits() as i64),
        'H' => None,
        _ => HEAP.with(|h| h.borrow().int_of(result)),
    }
}

fn complete_sync_request(request: &SyncCallbackRequest, bits: i64) {
    let mut state = request.state.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(*state, SyncRequestState::Running) {
        *state = SyncRequestState::Completed(bits);
        request.ready.notify_all();
    }
}

/// Completes one claimed request even if its callback closed inside the handler.
/// Invalid scalar results and caught callback failures use exceptional C bits.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_callback_sync_complete(
    message: i64,
    result: i64,
    failed: u8,
) {
    let Some((id, request)) = request_from_message(message) else {
        return;
    };
    // SAFETY: requests carry retained contexts and this is the owner's thread.
    let c = unsafe { &*(request.context as *const CallbackContext) };
    let pending = dartforge_exception_pending() != 0;
    let bits = if failed != 0 || pending {
        c.exceptional
    } else {
        sync_result_bits(c, result).unwrap_or(c.exceptional)
    };
    if pending {
        dartforge_exception_clear();
    }
    complete_sync_request(&request, bits);
    sync_requests()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
}

/// The C entry of a `listener`: the `n` arguments (bits of each C type) become the
/// message `[context, a0, a1, …]` to the `NativeCallable`'s port.
///
/// # Safety
/// As [`dartforge_ffi_callback_enter`]; `args` has `n` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_post(
    ctx: *const CallbackContext,
    args: *const i64,
    n: i64,
) {
    // SAFETY: guaranteed by the caller.
    let c = unsafe { &*ctx };
    if c.closed_flag.load(std::sync::atomic::Ordering::Acquire) {
        // Close may win between listener enter and post. The retained metadata
        // remains valid; discard before decoding native argument addresses.
        record_late_callback(c);
        return;
    }
    // SAFETY: the entry supplies exactly the signature's argument words.
    let bits = if n <= 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args, n as usize) }
    };
    let mut items = vec![Portable::Int(ctx as i64)];
    // SAFETY: compound addresses remain valid throughout this C invocation.
    items.extend(unsafe { callback_portable_arguments(c, bits) });
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
        (0..h.list_len(message))
            .map(|i| h.list_get(message, i))
            .collect()
    });
    let integer = |v: &crate::heap::Value| argument_int(*v).unwrap_or(0);
    let Some(ctx) = items.first().map(integer) else {
        return message;
    };
    let sync_request = items
        .get(1)
        .and_then(|value| argument_int(*value))
        .and_then(|id| sync_request(id));
    let claimed_sync = sync_request.as_ref().is_some_and(|request| {
        request.context == ctx as usize
            && request.owner_thread == std::thread::current().id()
            && matches!(
                *request.state.lock().unwrap_or_else(|e| e.into_inner()),
                SyncRequestState::Running
            )
    });
    let alive = live_callbacks()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .any(|&c| c as i64 == ctx);
    if !alive && !claimed_sync {
        return dart_fixed_list(&[]);
    }
    // SAFETY: the live registry or claimed request proves this retained context.
    let (key, ns, skip) = unsafe {
        let c = &*(ctx as *const CallbackContext);
        if c.mode == MODE_SYNC && !claimed_sync {
            return dart_fixed_list(&[]);
        }
        (
            c.key.clone(),
            c.fn_signature,
            if c.mode == MODE_SYNC { 2 } else { 1 },
        )
    };
    let types: Vec<i64> = RTI.with(|u| match u.borrow().ty(ns) {
        Type::Function { pos, .. } => pos.to_vec(),
        _ => Vec::new(),
    });
    // Each argument becomes a `Ref` rooted in the frame (an integer's box, the
    // `Pointer`, the copied struct), and the list is built at the end.
    let n = items.len().saturating_sub(skip);
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(n.max(1)));
    let (_, params) = key_parts(&key);
    let mut out = Vec::with_capacity(n);
    for (i, (v, p)) in items.iter().skip(skip).zip(params).enumerate() {
        let x = match p {
            KeyPart::Letter('p') => new_pointer(integer(v), types.get(i).copied()),
            // The struct's bytes become a struct over Dart memory.
            KeyPart::Compound(rti) => {
                let bytes = match v {
                    crate::heap::Value::Ref(r) => HEAP
                        .with(|h| h.borrow().typed_bytes(*r).map(<[u8]>::to_vec))
                        .unwrap_or_default(),
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
            mode: MODE_LOCAL,
            isolate: 0,
            owner_thread: std::thread::current().id(),
            sync_timeout: std::time::Duration::from_secs(1),
            compound_sizes: Vec::new(),
            closure: std::sync::atomic::AtomicI64::new(closure),
            port: 0,
            fn_signature: 0,
            key: String::new(),
            exceptional: 0,
            active_list: std::sync::atomic::AtomicU32::new(0),
            closed_flag: std::sync::atomic::AtomicBool::new(false),
            late_calls: std::sync::atomic::AtomicU64::new(0),
            trampoline: 0,
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
                mode,
                isolate: 0,
                owner_thread: std::thread::current().id(),
                sync_timeout: std::time::Duration::from_secs(1),
                compound_sizes: Vec::new(),
                closure: std::sync::atomic::AtomicI64::new(0),
                port: 0,
                fn_signature: 0,
                key: String::new(),
                exceptional: 0,
                active_list: std::sync::atomic::AtomicU32::new(0),
                closed_flag: std::sync::atomic::AtomicBool::new(false),
                late_calls: std::sync::atomic::AtomicU64::new(0),
                trampoline: 0,
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

#[cfg(test)]
mod late_callback_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};

    fn context(mode: i64, closure: i64, exceptional: i64) -> &'static CallbackContext {
        let ctx = Box::leak(Box::new(CallbackContext {
            mode,
            isolate: isolate_id(),
            owner_thread: std::thread::current().id(),
            sync_timeout: std::time::Duration::from_secs(1),
            compound_sizes: Vec::new(),
            closure: AtomicI64::new(closure),
            port: 0,
            fn_signature: 0,
            key: "v_".to_string(),
            exceptional,
            active_list: AtomicU32::new(0),
            closed_flag: AtomicBool::new(false),
            late_calls: AtomicU64::new(0),
            trampoline: 0,
        }));
        let address = ctx as *const CallbackContext as usize;
        live_callbacks().lock().unwrap().insert(address, address);
        if closure != 0 {
            HEAP.with(|h| h.borrow_mut().set_global_root(address as i64, closure));
        }
        ctx
    }

    extern "C" fn never_dispatch(_: i64) -> i64 {
        panic!("event work ran inside a native callback");
    }

    #[test]
    fn owner_callback_depth_refuses_turns_and_balances_exceptional_and_normal_exit() {
        for mode in [MODE_PERSISTENT, MODE_LOCAL, MODE_SYNC] {
            let ctx = context(mode, 0, 91);
            let address = ctx as *const CallbackContext as usize;
            assert!(!in_owner_ffi_callback());
            // SAFETY: the retained context belongs to this native thread.
            unsafe {
                assert_eq!(dartforge_ffi_callback_enter(address as *mut CallbackContext), 0);
                assert_eq!(dartforge_ffi_callback_enter(address as *mut CallbackContext), 0);
            }
            assert_eq!(OWNER_FFI_CALLBACK_DEPTH.with(|depth| depth.get()), 2);
            dartforge_event_loop(never_dispatch);
            assert_eq!(dartforge_exception_pending(), 1);
            assert!(!EVENT_LOOP_ACTIVE.with(|active| active.get()));
            let mut result = 0;
            // The ordinary exceptional-return protocol still catches and clears
            // the StateError, while the outer native frame remains active.
            assert_eq!(unsafe { dartforge_ffi_callback_exit(address as *mut CallbackContext, &mut result) }, 1);
            assert_eq!(result, 91);
            assert_eq!(dartforge_exception_pending(), 0);
            assert!(in_owner_ffi_callback());
            assert!(EventLoopGuard::enter().is_none());
            result = 17;
            assert_eq!(unsafe { dartforge_ffi_callback_exit(address as *mut CallbackContext, &mut result) }, 0);
            assert_eq!(result, 17);
            assert!(!in_owner_ffi_callback());
            assert!(EventLoopGuard::enter().is_some());
            dartforge_native_DartForge_ffi_callback_delete(address as i64);
            assert_eq!(unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) }, 2);
            assert!(!in_owner_ffi_callback());
        }
        let listener = context(MODE_LISTENER, 0, 0);
        let address = listener as *const CallbackContext as usize;
        assert_eq!(unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) }, 1);
        assert!(!in_owner_ffi_callback());
        dartforge_native_DartForge_ffi_callback_delete(address as i64);
    }

    #[test]
    fn closed_local_returns_retained_bits_on_foreign_threads() {
        let exceptional = (-17.25f64).to_bits() as i64;
        let ctx = context(MODE_LOCAL, 0, exceptional);
        let address = ctx as *const CallbackContext as usize;
        dartforge_native_DartForge_ffi_callback_delete(address as i64);
        dartforge_native_DartForge_ffi_callback_delete(address as i64);
        std::thread::spawn(move || {
            for _ in 0..1000 {
                // SAFETY: the published context remains a stable tombstone.
                unsafe {
                    assert_eq!(
                        dartforge_ffi_callback_enter(address as *mut CallbackContext),
                        2
                    );
                    assert_eq!(
                        dartforge_ffi_callback_late_return(address as *const i64),
                        exceptional
                    );
                }
            }
        })
        .join()
        .unwrap();
        assert_eq!(ctx.late_calls.load(Ordering::Relaxed), 1000);
        assert_eq!(ctx.active_list.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn close_inside_active_callback_releases_root_without_freeing_context() {
        let child = dartforge_arc_object_owned_v1(177, 0);
        let environment = dartforge_arc_object_owned_v1(i64::from(crate::layout::cid::CONTEXT), 1);
        dartforge_arc_write_field_ref_v1(environment, 0, child);
        let closure = dartforge_arc_closure_owned_v1(0, environment, 0, 0);
        dartforge_arc_release(child);
        dartforge_arc_release(environment);
        let ctx = context(MODE_LOCAL, closure, 91);
        let address = ctx as *const CallbackContext as usize;
        // The generated Dart call frame owns its closure while the callback runs.
        let owner = unsafe { dartforge_arc_ffi_callback_closure_owned_v1(address as i64) };
        dartforge_arc_release(closure);
        // SAFETY: the context is published on this owning isolate.
        assert_eq!(
            unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
            0
        );
        dartforge_native_DartForge_ffi_callback_delete(address as i64);
        assert!(ctx.closed_flag.load(Ordering::Acquire));
        assert_eq!(ctx.closure.load(Ordering::Acquire), 0);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 3);
        let mut out = 0;
        // SAFETY: the active frame is still valid despite retirement.
        assert_eq!(
            unsafe { dartforge_ffi_callback_exit(address as *mut CallbackContext, &mut out) },
            0
        );
        dartforge_arc_release(owner);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 0);
        assert_eq!(dartforge_arc_observe_heap_v1(environment), 0);
        assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
        assert_eq!(ctx.active_list.load(Ordering::Relaxed), 0);
        assert_eq!(
            unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
            2
        );
    }

    #[test]
    fn listener_close_between_enter_and_post_drops_without_decoding_arguments() {
        let ctx = context(MODE_LISTENER, 0, 0);
        let address = ctx as *const CallbackContext as usize;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let worker_barrier = barrier.clone();
        let worker = std::thread::spawn(move || {
            // SAFETY: a published listener can be entered from any thread.
            assert_eq!(
                unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
                1
            );
            worker_barrier.wait();
            worker_barrier.wait();
            // Deliberately invalid args would fault if a retired post decoded them.
            // SAFETY: closed posts return before inspecting args.
            unsafe {
                dartforge_ffi_callback_post(
                    address as *const CallbackContext,
                    std::ptr::null(),
                    100,
                )
            };
        });
        barrier.wait();
        dartforge_native_DartForge_ffi_callback_delete(address as i64);
        barrier.wait();
        worker.join().unwrap();
        assert_eq!(ctx.late_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn isolate_retirement_tombstones_persistent_callbacks_and_releases_roots() {
        let closure = dartforge_arc_object_owned_v1(178, 1);
        let ctx = context(MODE_PERSISTENT, closure, 33);
        let address = ctx as *const CallbackContext as usize;
        PERSISTENT.with(|p| p.borrow_mut().insert(("v_".into(), 1, 2, 33), address));
        dartforge_arc_release(closure);
        retire_isolate_callbacks();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(closure), 0);
        assert!(PERSISTENT.with(|p| p.borrow().is_empty()));
        assert!(!live_callbacks().lock().unwrap().contains_key(&address));
        assert_eq!(
            unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
            2
        );
        assert_eq!(
            unsafe {
                dartforge_ffi_callback_late_return((ctx as *const CallbackContext).cast::<i64>())
            },
            33
        );
    }
}

#[cfg(test)]
mod sync_callback_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    fn context(key: &str, sizes: Vec<usize>) -> &'static CallbackContext {
        Box::leak(Box::new(CallbackContext {
            mode: MODE_SYNC,
            isolate: isolate_id(),
            owner_thread: std::thread::current().id(),
            sync_timeout: Duration::from_secs(1),
            compound_sizes: sizes,
            closure: AtomicI64::new(0),
            port: 0,
            fn_signature: 0,
            key: key.into(),
            exceptional: 77,
            active_list: AtomicU32::new(0),
            closed_flag: AtomicBool::new(false),
            late_calls: AtomicU64::new(0),
            trampoline: 0,
        }))
    }

    fn request(c: &'static CallbackContext, deadline: Instant) -> Arc<SyncCallbackRequest> {
        Arc::new(SyncCallbackRequest {
            context: c as *const CallbackContext as usize,
            owner_thread: c.owner_thread,
            deadline,
            exceptional: c.exceptional,
            state: Mutex::new(SyncRequestState::Pending),
            ready: Condvar::new(),
        })
    }

    #[test]
    fn owner_native_identity_enters_directly_foreign_identity_marshals_without_heap() {
        let c = context("i_", vec![]);
        let address = c as *const CallbackContext as usize;
        assert_eq!(
            unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
            0
        );
        let mut out = 0;
        assert_eq!(
            unsafe { dartforge_ffi_callback_exit(address as *mut CallbackContext, &mut out) },
            0
        );
        std::thread::spawn(move || {
            HEAP.with(|h| {
                let _heap_borrow = h.borrow_mut();
                RTI.with(|r| {
                    let _rti_borrow = r.borrow_mut();
                    assert_eq!(
                        unsafe { dartforge_ffi_callback_enter(address as *mut CallbackContext) },
                        3
                    );
                    assert!(!in_owner_ffi_callback());
                });
            });
        })
        .join()
        .unwrap();
        assert_eq!(c.active_list.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn foreign_compound_encoding_uses_owner_metadata_not_foreign_rti_or_heap() {
        let c = context("i_S900001.", vec![8]);
        let address = c as *const CallbackContext as usize;
        std::thread::spawn(move || {
            let data = [0x04030201u32, 0x08070605u32];
            HEAP.with(|h| {
                let _heap_borrow = h.borrow_mut();
                RTI.with(|r| {
                    let _rti_borrow = r.borrow_mut();
                    let c = unsafe { &*(address as *const CallbackContext) };
                    let arguments =
                        unsafe { callback_portable_arguments(c, &[data.as_ptr() as i64]) };
                    let Portable::Bytes(bytes) = &arguments[0] else {
                        panic!("compound bytes expected")
                    };
                    assert_eq!(bytes, &[1, 2, 3, 4, 5, 6, 7, 8]);
                });
            });
        })
        .join()
        .unwrap();
    }

    #[test]
    fn logical_deadline_cancels_only_pending_and_prevents_late_claim() {
        let c = context("i_", vec![]);
        let deadline = Instant::now();
        let expired = request(c, deadline);
        assert!(!claim_sync_request(&expired, deadline));
        assert_eq!(wait_sync_request(&expired), 77);
        assert_eq!(*expired.state.lock().unwrap(), SyncRequestState::Cancelled);
        assert!(!claim_sync_request(
            &expired,
            deadline - Duration::from_secs(1)
        ));
        complete_sync_request(&expired, 88);
        assert_eq!(wait_sync_request(&expired), 77);
        let pending = request(c, deadline - Duration::from_secs(1));
        assert_eq!(wait_sync_request(&pending), 77);
        assert!(!claim_sync_request(&pending, deadline));
    }

    #[test]
    fn claimed_work_survives_close_and_expired_deadline_until_completion() {
        let c = context("i_", vec![]);
        let deadline = Instant::now();
        let running = request(c, deadline);
        assert!(claim_sync_request(
            &running,
            deadline - Duration::from_secs(1)
        ));
        // Install a unique registry identity solely for this test.
        let id = -(c as *const CallbackContext as i64);
        sync_requests().lock().unwrap().insert(id, running.clone());
        c.closed_flag.store(true, Ordering::Release);
        cancel_sync_requests(c as *const CallbackContext as usize, false);
        assert_eq!(*running.state.lock().unwrap(), SyncRequestState::Running);
        assert!(sync_request(id).is_some());
        complete_sync_request(&running, 88);
        assert_eq!(wait_sync_request(&running), 88);
        sync_requests().lock().unwrap().remove(&id);
    }

    #[test]
    fn close_cancels_pending_and_owner_shutdown_also_cancels_running() {
        let c = context("i_", vec![]);
        let id = -(c as *const CallbackContext as i64);
        let pending = request(c, Instant::now() + Duration::from_secs(10));
        sync_requests().lock().unwrap().insert(id, pending.clone());
        cancel_sync_requests(c as *const CallbackContext as usize, false);
        assert_eq!(wait_sync_request(&pending), 77);
        assert!(sync_request(id).is_none());
        let running = request(c, Instant::now() + Duration::from_secs(10));
        assert!(claim_sync_request(&running, Instant::now()));
        sync_requests().lock().unwrap().insert(id, running.clone());
        cancel_sync_requests(c as *const CallbackContext as usize, true);
        assert_eq!(wait_sync_request(&running), 77);
        assert!(sync_request(id).is_none());
    }

    #[test]
    fn completion_wakes_foreign_waiter_and_only_first_result_wins() {
        let c = context("i_", vec![]);
        let running = request(c, Instant::now());
        assert!(claim_sync_request(
            &running,
            running.deadline - Duration::from_secs(1)
        ));
        let other = running.clone();
        let worker = std::thread::spawn(move || wait_sync_request(&other));
        complete_sync_request(&running, 123);
        complete_sync_request(&running, 456);
        assert_eq!(worker.join().unwrap(), 123);
    }

    #[test]
    fn protocol_wait_edges_reject_cycles_and_drop_cleans_registration() {
        let a = std::thread::current().id();
        let b = std::thread::spawn(|| std::thread::current().id())
            .join()
            .unwrap();
        let c = std::thread::spawn(|| std::thread::current().id())
            .join()
            .unwrap();
        let mut edges = crate::hash::HashMap::default();
        edges.insert(a, b);
        edges.insert(b, c);
        assert!(wait_edge_would_cycle(&edges, c, a));
        assert!(wait_edge_would_cycle(&edges, a, c));
        assert!(!wait_edge_would_cycle(
            &edges,
            c,
            std::thread::spawn(|| std::thread::current().id())
                .join()
                .unwrap()
        ));
        {
            let _edge = SyncWaitEdge::register(b).unwrap();
            assert!(SyncWaitEdge::register(c).is_none());
        }
        assert!(!sync_wait_edges().lock().unwrap().contains_key(&a));
    }

    #[test]
    fn scalar_reply_conversions_require_actual_types_and_preserve_float_bits() {
        let double = context("d_", vec![]);
        let value = dartforge_arc_box_double_owned_v1(-17.25);
        assert_eq!(
            sync_result_bits(double, value),
            Some((-17.25f64).to_bits() as i64)
        );
        let integer = context("i_", vec![]);
        assert_eq!(sync_result_bits(integer, value), None);
        assert_eq!(sync_result_bits(integer, 0), None);
        let object = dartforge_arc_object_owned_v1(199, 0);
        assert_eq!(sync_result_bits(integer, object), None);
        let boolean = context("b_", vec![]);
        assert_eq!(sync_result_bits(boolean, object), None);
        dartforge_arc_release(value);
        dartforge_arc_release(object);
    }
}
