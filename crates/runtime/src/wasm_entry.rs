// The runtime side of a WebAssembly module's entry (host ABI v1,
// docs/WASM-HOST-ABI-V1.md), compiled only into the wasm32 runtime (build.rs,
// `WASM_ONLY`). The program defines `dartforge_run`, which hands its entry to
// `dartforge_init` (`core`), as the C `main` of a native executable does.

/// The version of the host ABI this runtime speaks (docs/WASM-HOST-ABI-V1.md):
/// the host refuses a module of another version before running it.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_abi_version() -> i32 {
    1
}

/// What `dartforge_run` and `dartforge_resume` return while the program waits
/// for a timer: the host calls `dartforge_resume` after `dartforge_delay_us`.
const PENDING: i32 = -1;

/// What they return when the program has no event of its own left but
/// JavaScript still holds a Dart callback or object (`js_interop.rs`): the
/// program goes on when the host calls back, as in dart2wasm, and ends with the
/// host.
const WAITING: i32 = -2;

/// The status of a program whose loop returned: pending, waiting for the host,
/// or ended (its exit status).
fn loop_status() -> i32 {
    if event_loop_suspended() {
        return PENDING;
    }
    if js_host_waiting() {
        return WAITING;
    }
    finish_program()
}

/// Microseconds until the module wants `dartforge_resume` (after a `-1`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_delay_us() -> f64 {
    event_loop_delay_micros()
}

/// Continues a program that returned `-1`: runs the events that are due and
/// returns the exit status, `-1` again while timers remain, or `-2` while only
/// the host can call back.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_resume() -> i32 {
    resume_event_loop();
    loop_status()
}

/// `main`'s arguments in a module: none, as dart2wasm's `main` receives without
/// arguments from the host (a module has no command line).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_main_arguments() -> i64 {
    dart_string_list(&[], true)
}

/// The engine stack V8 gives a thread (Node and Chrome, `--stack-size` 984
/// KiB): the budget of a host that does not call [`dartforge_engine_stack`].
const V8_STACK: i32 = 984 * 1024;

/// What the budget keeps back for the frames it does not count: the runtime's,
/// the host's (a JavaScript callback between Dart frames), the functions that
/// do not call, and building the `StackOverflowError` itself.
const ENGINE_RESERVE: i32 = 160 * 1024;

/// WebAssembly: the bytes of engine stack the module's frames may still take
/// (`emit_native/src/wasm_frames.rs`). The prologue of each function that
/// calls subtracts its frame and throws `StackOverflowError` below zero; its
/// returns add it back.
#[unsafe(export_name = "dartforge_engine_budget")]
pub static ENGINE_BUDGET: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(V8_STACK - ENGINE_RESERVE);

/// WebAssembly host ABI v1 (docs/WASM-HOST-ABI-V1.md): the bytes of engine
/// stack the host gives the module, before `dartforge_init`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_engine_stack(bytes: i32) {
    ENGINE_BUDGET.store(
        bytes.saturating_sub(ENGINE_RESERVE).max(0),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// `DartForge_uri_base()` (`native_sdk/wasm/uri_patch.dart`): the `Uri.base`
/// the host gives on this call (the runtime option `DARTFORGE_URI_BASE`; a
/// page's `location.href`), or `null` when it has none.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_uri_base() -> i64 {
    crate::host::option("DARTFORGE_URI_BASE").map_or(0, |base| allocate_str(&base))
}
