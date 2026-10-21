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

/// Microseconds until the module wants `dartforge_resume` (after a `-1`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_delay_us() -> f64 {
    event_loop_delay_micros()
}

/// Continues a program that returned `-1`: runs the events that are due and
/// returns the exit status, or `-1` again while timers remain.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_resume() -> i32 {
    resume_event_loop();
    if event_loop_suspended() {
        return PENDING;
    }
    finish_program()
}

/// `main`'s arguments in a module: none, as dart2wasm's `main` receives without
/// arguments from the host (a module has no command line).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_main_arguments() -> i64 {
    dart_string_list(&[], true)
}
