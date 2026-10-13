//! Persistent session: one process, one initialized LLVM, modules compiled
//! once and many executions with clean Dart state between them.
//!
//! It is the shape of the macro and builder executor (a rule of `PLANO.md`): the
//! macro code is compiled once and stays cached, and each execution pays
//! only for the execution. The tests pin both guarantees (clean state and a cache that
//! goes into several sessions); the measurement, with `--ignored --nocapture`, gives the cost
//! of each piece.
use dartforge_jit::{JitSession, compile_module};
use std::time::{Duration, Instant};

/// A program with a static: throws if it finds it already initialized, and
/// initializes it. On the first execution it ends fine; on a second one that inherited
/// the first one's state, it would end with an uncaught exception (255).
const WITH_STATIC: &str = "\
@dfg_0 = internal global i64 0
@dfg_0_ok = internal global i8 0
declare void @dartforge_exception_throw(i64, i8)
define void @dartforge_entry() {
entrada:
  %ok = load i8, ptr @dfg_0_ok
  %ja = icmp ne i8 %ok, 0
  br i1 %ja, label %sujo, label %limpo
sujo:
  call void @dartforge_exception_throw(i64 5, i8 1)
  ret void
limpo:
  store i64 42, ptr @dfg_0
  store i8 1, ptr @dfg_0_ok
  ret void
}
";

/// The shortest possible execution: the entry returns.
const TRIVIAL: &str = "define void @dartforge_entry() {\n  ret void\n}\n";

/// Several executions in the same session start with zeroed statics.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn sequential_runs_start_clean() {
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("estatico", WITH_STATIC).unwrap();
    for turn in 1..=3 {
        let report_data = session.run_entry().unwrap();
        assert_eq!(report_data.exit_code, 0, "execution {turn} inherited the previous one's state");
    }
}

/// A module compiled once goes into several sessions and runs clean in all of them.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn compiled_module_enters_several_sessions() {
    let compiled = compile_module("estatico", WITH_STATIC).unwrap();
    assert!(!compiled.object().is_empty());
    for _ in 0..2 {
        let mut session = JitSession::new().unwrap();
        session.add_compiled_module(&compiled).unwrap();
        assert_eq!(session.run_entry().unwrap().exit_code, 0);
        assert_eq!(session.run_entry().unwrap().exit_code, 0);
    }
}

/// The object goes through the same checks as the IR: an unknown external fails.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn compiled_module_passes_preverification() {
    let ir = "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  call void @sqlite3_open()\n  ret void\n}\n";
    let compiled = compile_module("externo", ir).unwrap();
    let mut session = JitSession::new().unwrap();
    let error = session.add_compiled_module(&compiled).unwrap_err();
    assert_eq!(error.stage, "symbols");
    assert!(error.message.contains("sqlite3_open"), "{error}");
}

/// The `main` path also resets the statics of the program's JIT module.
/// This session does not load the SDK DLL and does not test its internal statics.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn main_resets_jit_module_globals_between_runs() {
    let ir = "\
@dfg_0_ok = internal global i8 0
define i32 @main() {
  %anterior = load i8, ptr @dfg_0_ok
  store i8 1, ptr @dfg_0_ok
  %codigo = zext i8 %anterior to i32
  ret i32 %codigo
}
";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("programa", ir).unwrap();
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);
}

/// Median and p95 of `n` samples of `f`, after 3 warm-ups.
fn measure(n: usize, mut f: impl FnMut() -> Duration) -> (Duration, Duration) {
    for _ in 0..3 {
        f();
    }
    let mut v: Vec<Duration> = (0..n).map(|_| f()).collect();
    v.sort();
    (v[n / 2], v[(n * 95 / 100).min(n - 1)])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Cost of each piece of the persistent session, for `docs/JIT.md`.
///
/// Run with:
/// `cargo test -p dartforge-jit --release --test persistent_session persistent_session_measurement -- --ignored --nocapture`
///
/// (i) creating the session; (ii) loading a module and generating code until the entry
/// is resolved, from the IR and from the cached object; (iii) a trivial execution
/// in an already warmed-up session. The «emitter IR» module is that of a small Dart
/// program, emitted by `dartforge_emit_native::emitir_ir`.
#[test]
#[ignore = "measurement: requires LLVM-C.dll and the Dart 3.6.2 SDK; run with --nocapture"]
fn persistent_session_measurement() {
    let n = 30;
    let (p50, p95) = measure(n, || {
        let t = Instant::now();
        let session = JitSession::new().unwrap();
        let d = t.elapsed();
        drop(session);
        d
    });
    println!("(i)   create the session (LLJIT + runtime symbols): median {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));

    /// Removes the directory on `Drop`, also when emission panics.
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Directory(std::env::temp_dir().join(format!("dartforge-sessao-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let source = dir.0.join("macro.dart");
    std::fs::write(&source, "int dobro(int x) { return x * 2; }\nvoid main() {\n  int y = dobro(21);\n}\n").unwrap();
    let emitted = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions { sdk: None, packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
            // With the source SDK (the emitter's only mode since the unified
            // space): the program imports the runtime and the SDK from the shared
            // library, which the session loads (`JitSession::new_for_ir_com`).
            dartforge_emit_native::emitir_ir_recarregavel(&source, &options, None).unwrap().texto
        })
        .unwrap()
        .join()
        .unwrap();
    drop(dir);
    let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("source SDK library");
    // The IR session: the embedded runtime for the trivial one, the SDK library
    // for the emitted one; and execution through each profile's entry.
    let new_session = |ir: &str| JitSession::new_for_ir_com(ir, Some(&dll)).unwrap();
    let execute_fn = |s: &JitSession| if s.usa_sdk_da_fonte() { s.run_main().unwrap().total } else { s.run_entry().unwrap().total };

    for (label_name, ir) in [("trivial", TRIVIAL), ("IR do emissor", emitted.as_str())] {
        let (p50, p95) = measure(n, || {
            let mut session = new_session(ir);
            let t = Instant::now();
            session.add_ir_module("m", ir).unwrap();
            session.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  load from IR, {label_name} ({} bytes): parse + verification + generation, median {:.3} ms, p95 {:.3} ms",
            ir.len(),
            ms(p50),
            ms(p95)
        );
        let (c50, c95) = measure(n, || {
            let t = Instant::now();
            compile_module("m", ir).unwrap();
            t.elapsed()
        });
        println!("      compile into the cache, {label_name}: median {:.3} ms, p95 {:.3} ms (once per module)", ms(c50), ms(c95));
        let compiled = compile_module("m", ir).unwrap();
        let (p50, p95) = measure(n, || {
            let mut session = new_session(ir);
            let t = Instant::now();
            session.add_compiled_module(&compiled).unwrap();
            session.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  load from the cache, {label_name} ({} bytes of object): median {:.3} ms, p95 {:.3} ms",
            compiled.object().len(),
            ms(p50),
            ms(p95)
        );
        let mut session = new_session(ir);
        session.add_compiled_module(&compiled).unwrap();
        let (p50, p95) = measure(100, || execute_fn(&session));
        println!("(iii) one execution, {label_name}, warmed-up session: median {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));
    }
}
