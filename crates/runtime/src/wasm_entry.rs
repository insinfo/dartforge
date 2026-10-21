// The entry of a WebAssembly module (host ABI v1, docs/WASM-HOST-ABI-V1.md): the
// counterpart of the C `main` of `core`, compiled only into the wasm32 runtime
// (build.rs, `WASM_ONLY`).

/// The version of the host ABI this runtime speaks (docs/WASM-HOST-ABI-V1.md):
/// the host refuses a module of another version before running it.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_abi_version() -> i32 {
    1
}

/// The entry of a WebAssembly module (host ABI v1): runs the program once and
/// returns its exit status, as the C `main` of a native executable does. A host
/// `exit` ends the run from inside instead.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_run() -> i32 {
    // A runtime panic is not a Dart exception: its message goes to the host's
    // standard error before the trap (`panic = "abort"`).
    std::panic::set_hook(Box::new(|info| {
        eprintln!("dartforge runtime panic: {info}");
    }));
    dartforge_context();
    // SAFETY: the object was emitted for this ABI and linked by the same driver.
    unsafe { dartforge_entry() };
    finalize_program()
}

