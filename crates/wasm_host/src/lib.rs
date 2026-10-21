//! The independent executor of DartForge `wasm32-linear` modules: Wasmtime
//! (Cranelift), configured with exactly the profile's features, implementing the
//! host ABI v1 (`dartforge_host_v1`, docs/WASM-HOST-ABI-V1.md). The browser
//! adapter (`runtime/wasm/dartforge_host_v1.mjs`) implements the same contract,
//! so one `.wasm` file, unchanged, runs in both.
//!
//! The module is first checked by the strict gate (`dartforge-wasm-gate`):
//! the executor never runs what the profile does not admit.

use dartforge_wasm_gate::{HOST_MODULE, Policy};
use wasmtime::{Caller, Config, Engine, Linker, Memory, Module, Store};

/// The ABI version this host implements (`dartforge_abi_version`).
pub const ABI_VERSION: i32 = 1;

/// Status codes of `write`.
pub mod write_status {
    pub const OK: i32 = 0;
    /// The `(offset, length)` range is outside the linear memory.
    pub const OUT_OF_RANGE: i32 = -1;
    /// Neither 1 (stdout) nor 2 (stderr).
    pub const BAD_CHANNEL: i32 = -2;
}

/// How a run ended.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The SHA-256 of the module that ran.
    pub sha256: String,
    /// `dartforge_run`'s status, when it returned.
    pub status: Option<i32>,
    /// The engine trap, when it did not return (never a Dart exception).
    pub trap: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Why a module did not run at all.
#[derive(Debug)]
pub enum RunError {
    /// The profile gate refused it.
    Gate(Vec<String>),
    /// The engine refused it (parse, link or instantiation).
    Engine(String),
    /// The module's ABI version is not this host's.
    Abi(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Gate(e) => write!(f, "rejected by the wasm32-linear gate: {}", e.join("; ")),
            RunError::Engine(e) => write!(f, "engine: {e}"),
            RunError::Abi(e) => write!(f, "host ABI: {e}"),
        }
    }
}

struct HostState {
    memory: Option<Memory>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Wasmtime with exactly the `wasm32-linear` features: the engine itself would
/// reject what the gate let through by mistake.
fn profile_engine() -> Result<Engine, RunError> {
    let mut config = Config::new();
    config
        .wasm_multi_value(true)
        .wasm_bulk_memory(true)
        .wasm_reference_types(true)
        .wasm_simd(true)
        .wasm_tail_call(true)
        .wasm_extended_const(true)
        .wasm_exceptions(true)
        .wasm_relaxed_simd(false)
        .wasm_gc(false)
        .wasm_function_references(false)
        .wasm_multi_memory(false)
        .wasm_memory64(false)
        .wasm_custom_page_sizes(false)
        .wasm_wide_arithmetic(false)
        .wasm_stack_switching(false);
    Engine::new(&config).map_err(|e| RunError::Engine(e.to_string()))
}

/// `write(channel, offset, length)`: copies the bytes out of the linear memory
/// (they need not outlive the call), with the range checked without overflow.
fn write(mut caller: Caller<'_, HostState>, channel: i32, offset: i32, length: i32) -> i32 {
    let Some(memory) = caller.data().memory else {
        return write_status::OUT_OF_RANGE;
    };
    let (Ok(start), Ok(len)) = (u32::try_from(offset), u32::try_from(length)) else {
        return write_status::OUT_OF_RANGE;
    };
    let Some(end) = start.checked_add(len) else {
        return write_status::OUT_OF_RANGE;
    };
    let data = memory.data(&caller);
    let Some(bytes) = data.get(start as usize..end as usize) else {
        return write_status::OUT_OF_RANGE;
    };
    let bytes = bytes.to_vec();
    let state = caller.data_mut();
    match channel {
        1 => state.stdout.extend_from_slice(&bytes),
        2 => state.stderr.extend_from_slice(&bytes),
        _ => return write_status::BAD_CHANNEL,
    }
    write_status::OK
}

/// Gates, instantiates and runs `bytes`: checks `dartforge_abi_version`, then
/// calls `dartforge_run` once.
///
/// # Errors
/// When the module does not run at all ([`RunError`]); a trap during the run is
/// an [`Outcome`] with `trap`.
pub fn run(bytes: &[u8]) -> Result<Outcome, RunError> {
    let policy = Policy {
        required_exports: vec![
            "memory".to_string(),
            "dartforge_abi_version".to_string(),
            "dartforge_run".to_string(),
        ],
        ..Policy::default()
    };
    let report = dartforge_wasm_gate::validate(bytes, &policy).map_err(RunError::Gate)?;
    let engine = profile_engine()?;
    let module = Module::new(&engine, bytes).map_err(|e| RunError::Engine(e.to_string()))?;
    let mut store = Store::new(
        &engine,
        HostState {
            memory: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
    );
    let mut linker = Linker::<HostState>::new(&engine);
    linker
        .func_wrap(HOST_MODULE, "write", write)
        .map_err(|e| RunError::Engine(e.to_string()))?;
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| RunError::Engine(e.to_string()))?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| RunError::Abi("no exported memory".to_string()))?;
    store.data_mut().memory = Some(memory);
    let version = instance
        .get_typed_func::<(), i32>(&mut store, "dartforge_abi_version")
        .map_err(|e| RunError::Abi(format!("dartforge_abi_version: {e}")))?
        .call(&mut store, ())
        .map_err(|e| RunError::Abi(format!("dartforge_abi_version trapped: {e}")))?;
    if version != ABI_VERSION {
        return Err(RunError::Abi(format!(
            "module ABI {version}, host ABI {ABI_VERSION}"
        )));
    }
    let entry = instance
        .get_typed_func::<(), i32>(&mut store, "dartforge_run")
        .map_err(|e| RunError::Abi(format!("dartforge_run: {e}")))?;
    let mut outcome = Outcome {
        sha256: report.sha256,
        ..Outcome::default()
    };
    match entry.call(&mut store, ()) {
        Ok(status) => outcome.status = Some(status),
        Err(e) => outcome.trap = Some(format!("{e:#}")),
    }
    let state = store.into_data();
    outcome.stdout = state.stdout;
    outcome.stderr = state.stderr;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(text: &str) -> Vec<u8> {
        wat::parse_str(text).expect("valid text fixture")
    }

    const HELLO: &str = r#"(module
        (import "dartforge_host_v1" "write" (func $write (param i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (data (i32.const 16) "hello\n")
        (func (export "dartforge_abi_version") (result i32) (i32.const 1))
        (func (export "dartforge_run") (result i32)
          (drop (call $write (i32.const 1) (i32.const 16) (i32.const 6)))
          (call $write (i32.const 1) (i32.const 65530) (i32.const 100))))"#;

    #[test]
    fn writes_and_reports_out_of_range() {
        let o = run(&module(HELLO)).expect("runs");
        assert_eq!(o.stdout, b"hello\n");
        // The second write crosses the end of the memory: an error code, not a trap.
        assert_eq!(o.status, Some(write_status::OUT_OF_RANGE));
        assert!(o.trap.is_none());
    }

    #[test]
    fn trap_is_not_a_status() {
        let o = run(&module(
            r#"(module (memory (export "memory") 1)
               (func (export "dartforge_abi_version") (result i32) (i32.const 1))
               (func (export "dartforge_run") (result i32) unreachable))"#,
        ))
        .expect("runs");
        assert_eq!(o.status, None);
        assert!(o.trap.is_some());
    }

    #[test]
    fn profile_features_run() {
        // Sign extension, SIMD128, a tail call and a caught Wasm exception:
        // -128 + 4 + 7 = -117.
        let o = run(&module(
            r#"(module (memory (export "memory") 1)
               (func (export "dartforge_abi_version") (result i32) (i32.const 1))
               (func $tail (param i32) (result i32)
                 (return_call $add (local.get 0)))
               (tag $e (param i32))
               (func $throws (result i32) (throw $e (i32.const 7)))
               (func $seven (result i32)
                 (block $h (result i32)
                   (try_table (result i32) (catch $e $h) (call $throws))))
               (func $add (param i32) (result i32) (i32.add (local.get 0) (call $seven)))
               (func (export "dartforge_run") (result i32)
                 (call $tail (i32.add (i32.extend8_s (i32.const 128))
                   (i32x4.extract_lane 1 (i32x4.add (v128.const i32x4 0 1 0 0)
                                                    (v128.const i32x4 0 3 0 0)))))))"#,
        ))
        .expect("runs");
        assert_eq!((o.status, o.trap), (Some(-117), None));
    }

    #[test]
    fn wrong_abi_and_outside_profile_are_refused() {
        let wrong = module(
            r#"(module (memory (export "memory") 1)
               (func (export "dartforge_abi_version") (result i32) (i32.const 2))
               (func (export "dartforge_run") (result i32) (i32.const 0)))"#,
        );
        assert!(matches!(run(&wrong), Err(RunError::Abi(_))));
        let wasm_gc = module(
            r#"(module (memory (export "memory") 1)
               (func (export "dartforge_abi_version") (result i32) (i32.const 1))
               (type $p (struct (field i32)))
               (func (export "dartforge_run") (result i32)
                 (struct.get $p 0 (struct.new $p (i32.const 0)))))"#,
        );
        assert!(matches!(run(&wasm_gc), Err(RunError::Gate(_))));
        let unknown = module(
            r#"(module (import "env" "abort" (func))
               (memory (export "memory") 1)
               (func (export "dartforge_abi_version") (result i32) (i32.const 1))
               (func (export "dartforge_run") (result i32) (i32.const 0)))"#,
        );
        assert!(matches!(run(&unknown), Err(RunError::Gate(_))));
    }
}
