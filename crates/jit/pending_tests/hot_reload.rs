//! Test suite for Hot Reload in `crates/jit`.
//!
//! Hot reload is the ability to update functions and entry points in a
//! running JIT session, without restarting the host process and preserving
//! the state of the managed heap.

use dartforge_jit::JitSession;

/// Emits the LLVM IR of a Dart source, failing with a descriptive diagnostic.
fn compile_dart_to_ir(source: &str) -> String {
    dartforge_compiler::compile_llvm(source).unwrap_or_else(|err| {
        panic!("failed to compile Dart to LLVM IR: {err:?}\nSource:\n{source}")
    })
}

/// Test 1: Updating the body of a function.
///
/// Module v1: `int getValue() => 1; void main() { print(getValue()); }`
/// Runs in the session -> output: "1\n"
/// Hot reload with Module v2: `int getValue() => 42; void main() { print(getValue()); }`
/// Runs in the same session -> output: "42\n"
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn function_body_update_preserves_session() {
    let source_v1 = "\
int getValue() => 1;
void main() {
  print(getValue());
}
";
    let source_v2 = "\
int getValue() => 42;
void main() {
  print(getValue());
}
";

    let ir_v1 = compile_dart_to_ir(source_v1);
    let ir_v2 = compile_dart_to_ir(source_v2);

    let mut session = JitSession::new().expect("failed to initialize JitSession");
    session
        .add_ir_module("app", &ir_v1)
        .expect("failed to add the initial module");

    let (out1, _) = session
        .run_entry_capturing()
        .expect("failed to run entry v1");
    assert_eq!(out1, "1\n");

    // Runs hot reload replacing module 'app' with version v2
    let report = session
        .hot_reload("app", &ir_v2)
        .expect("hot reload to v2 failed");
    assert_eq!(report.ir_bytes, ir_v2.len());

    let (out2, _) = session
        .run_entry_capturing()
        .expect("failed to run entry v2 after reload");
    assert_eq!(out2, "42\n");
}

/// Test 1 (direct IR): Updating a function body using explicit LLVM IR.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn function_body_update_direct_llvm_ir() {
    let ir_v1 = "\
declare void @dartforge_print_i64(i64)
define i64 @getValue() {
  ret i64 1
}
define void @dartforge_entry() {
  %v = call i64 @getValue()
  call void @dartforge_print_i64(i64 %v)
  ret void
}
";

    let ir_v2 = "\
declare void @dartforge_print_i64(i64)
define i64 @getValue() {
  ret i64 42
}
define void @dartforge_entry() {
  %v = call i64 @getValue()
  call void @dartforge_print_i64(i64 %v)
  ret void
}
";

    let mut session = JitSession::new().expect("failed to initialize JitSession");
    session
        .add_ir_module("app", ir_v1)
        .expect("failed to add v1");

    let (out1, _) = session.run_entry_capturing().expect("execution v1");
    assert_eq!(out1, "1\n");

    session.hot_reload("app", ir_v2).expect("hot reload v2");

    let (out2, _) = session.run_entry_capturing().expect("execution v2");
    assert_eq!(out2, "42\n");
}

/// Test 2: Preservation of the managed heap state across the reload.
///
/// Creates class instances in the managed heap, invokes hot reload and checks
/// that the heap's counters and allocations persist across the reload.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn state_preservation_across_hot_reload() {
    let source_v1 = "\
class Contador {
  int valor;
  Contador(this.valor);
  int dobro() { return valor * 2; }
}
void main() {
  Contador c1 = Contador(21);
  Contador c2 = Contador(42);
  print(c1.dobro());
  print(c2.valor);
}
";

    let source_v2 = "\
class Contador {
  int valor;
  Contador(this.valor);
  int triplo() { return valor * 3; }
}
void main() {
  Contador c3 = Contador(10);
  print(c3.triplo());
}
";

    let ir_v1 = compile_dart_to_ir(source_v1);
    let ir_v2 = compile_dart_to_ir(source_v2);

    let mut session = JitSession::new().expect("failed to initialize JitSession");
    session.add_ir_module("app", &ir_v1).expect("adiciona v1");

    let (out1, _) = session.run_entry_capturing().expect("executa v1");
    assert_eq!(out1, "42\n42\n");

    let stats_after_v1 = session.gc_stats();
    // Two allocations of Contador c1 and c2
    assert!(
        stats_after_v1.allocations >= 2,
        "expected at least 2 allocations, got {}",
        stats_after_v1.allocations
    );

    // Invokes hot reload
    session.hot_reload("app", &ir_v2).expect("hot reload v2");

    // Checks that before running v2, the heap preserved exactly the previous state
    let stats_mid = session.gc_stats();
    assert_eq!(
        stats_mid.allocations, stats_after_v1.allocations,
        "heap counters must not be reset by hot reload"
    );

    // Runs the new entry v2
    let (out2, _) = session.run_entry_capturing().expect("executa v2");
    assert_eq!(out2, "30\n");

    // Checks that the heap accumulated new allocations on top of the persisted state
    let stats_after_v2 = session.gc_stats();
    assert!(
        stats_after_v2.allocations > stats_after_v1.allocations,
        "new allocations must accumulate in the persistent heap"
    );
}

/// Test 3: Transactional Safety.
///
/// Tries to reload with invalid IR (syntax error in the LLVM IR).
/// It must return `Err` at the `"parse-ir"` stage and the following call to `run_entry`
/// must keep running the previous working code, without corruption or crash.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn transactional_safety_on_invalid_ir() {
    let source_v1 = "\
int getValue() => 1;
void main() {
  print(getValue());
}
";
    let ir_v1 = compile_dart_to_ir(source_v1);

    let mut session = JitSession::new().expect("failed to initialize JitSession");
    session.add_ir_module("app", &ir_v1).expect("adiciona v1");

    let (out1, _) = session.run_entry_capturing().expect("executa v1");
    assert_eq!(out1, "1\n");

    // Hot reload attempt with syntactically invalid IR
    let invalid_ir = "THIS TEXT IS NOT VALID LLVM IR! @@@ {{{ }}}";
    let err = session
        .hot_reload("app", invalid_ir)
        .expect_err("hot reload with invalid IR should fail");

    assert_eq!(err.stage, "parse-ir");
    assert!(
        err.message.starts_with("invalid IR"),
        "unexpected message: {}",
        err.message
    );

    // The session stays intact: the previous code runs normally
    let (out_fallback, _) = session
        .run_entry_capturing()
        .expect("previous code must keep running without errors");
    assert_eq!(out_fallback, "1\n");

    // After the transactional failure, a reload with valid IR must work perfectly
    let source_v2 = "\
int getValue() => 42;
void main() {
  print(getValue());
}
";
    let ir_v2 = compile_dart_to_ir(source_v2);
    session
        .hot_reload("app", &ir_v2)
        .expect("a subsequent valid reload must work");

    let (out2, _) = session.run_entry_capturing().expect("executa v2");
    assert_eq!(out2, "42\n");
}

/// Test 4: Multiple consecutive reloads without memory leaks.
///
/// Runs 5 reloads in sequence confirming the behaviour update
/// and the stability of the JIT session.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn multiple_consecutive_reloads_without_leaks() {
    let mut session = JitSession::new().expect("failed to initialize JitSession");

    let initial_ir = compile_dart_to_ir("int f() => 0; void main() { print(f()); }");
    session
        .add_ir_module("app", &initial_ir)
        .expect("adds the initial module");

    let (out0, _) = session.run_entry_capturing().expect("runs the initial one");
    assert_eq!(out0, "0\n");

    for i in 1..=5 {
        let code = format!("int f() => {i}; void main() {{ print(f()); }}");
        let ir = compile_dart_to_ir(&code);

        let report = session
            .hot_reload("app", &ir)
            .unwrap_or_else(|e| panic!("falha no reload #{i}: {e}"));

        assert_eq!(report.ir_bytes, ir.len());

        let (out, _) = session
            .run_entry_capturing()
            .unwrap_or_else(|e| panic!("execution #{i} failed: {e}"));

        assert_eq!(out, format!("{i}\n"));

        // Ensures that only 1 module resides in the session's active list
        assert_eq!(session.module_names(), vec!["app"]);
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Behaviour hot reload: stable identity, per-generation implementation.
//
// The tests above go through the **promotion** path: the module is incorporated by
// `add_ir_module` and the first reload installs the trampolines. The ones below use
// `add_reloadable_module`, which is born reloadable and is the recommended path of the
// development loop.
// ───────────────────────────────────────────────────────────────────────────────

/// Basic reload: the same symbol, new body, in the same session.
///
/// `dartforge_entry` calls `teste()` **through the trampoline**, not through the body, so
/// swapping the implementation of `teste` changes what the entry prints without the
/// entry having to be rewritten.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn basic_reload_swaps_body_in_same_session() {
    let ir_v1 = compile_dart_to_ir("int teste() => 1;\nvoid main() { print(teste()); }\n");
    let ir_v2 = compile_dart_to_ir("int teste() => 2;\nvoid main() { print(teste()); }\n");

    let mut session = JitSession::new().expect("session");
    let first_one = session
        .add_reloadable_module("app", &ir_v1)
        .expect("generation 1");
    assert_eq!(first_one.generation, 1);
    assert!(!first_one.promoted);
    assert_eq!(session.generation("app"), Some(1));

    let (out, _) = session.run_entry_capturing().expect("execution of generation 1");
    assert_eq!(out, "1\n");

    let second_one = session.hot_reload("app", &ir_v2).expect("generation 2");
    assert_eq!(second_one.generation, 2);
    assert_eq!(
        second_one.new_entries, 0,
        "no new entry: only the body changed"
    );
    assert!(second_one.entries >= 2, "{second_one:?}");
    assert_eq!(second_one.retained_generations, 2);
    assert!(
        second_one.total >= second_one.parse_ir + second_one.add_module + second_one.link + second_one.publish,
        "the total must cover the stages: {second_one:?}"
    );

    let (out, _) = session.run_entry_capturing().expect("execution of generation 2");
    assert_eq!(out, "2\n");

    // The session identity did not change: one module, two generations.
    assert_eq!(session.module_names(), vec!["app"]);
    assert_eq!(session.retained_generations(), 2);
}

/// Minimal IR of a counter in the managed heap, with a parameterized increment.
///
/// `df_fn_0` creates the counter and returns the handle; `df_fn_1` adds `step` to field
/// 0 and returns the new value. Both signatures are stable between versions —
/// what changes is only the body of `df_fn_1`, which is the accepted case of version 1.
fn counter_ir(step: i64) -> String {
    format!(
        "\
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
define i64 @df_fn_0() {{
  %h = call i64 @dartforge_object_new(i64 7, i64 1)
  ret i64 %h
}}
define i64 @df_fn_1(i64 %h) {{
  %v = call i64 @dartforge_object_get(i64 %h, i64 0)
  %n = add i64 %v, {step}
  call void @dartforge_object_set(i64 %h, i64 0, i64 %n, i8 0)
  ret i64 %n
}}
"
    )
}

/// Preserved state: the counter crosses the reload without being zeroed.
///
/// This is what separates hot reload from restarting the process. The counter lives in the
/// thread's managed heap, which belongs to the session and not to the module: swapping the generation
/// unloads nothing from the heap, and the object stays at the same handle, with the same
/// value, exactly as the Dart VM keeps instances in place and swaps only the
/// code pointers.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn live_counter_survives_reload() {
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("contador", &counter_ir(1))
        .expect("generation 1");

    let create = session.stable_entry("df_fn_0").expect("creation entry");
    let add_up = session
        .stable_entry("df_fn_1")
        .expect("increment entry");
    assert_eq!(create.signature(), "i64 ()");
    assert_eq!(add_up.signature(), "i64 (i64)");

    let counter = create.call(&session).expect("creates the counter");
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 1);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 2);
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 3);

    let before = session.gc_stats();
    // New step: 10 instead of 1. Same signature, different body.
    session
        .hot_reload("contador", &counter_ir(10))
        .expect("generation 2");
    let after = session.gc_stats();
    assert_eq!(
        (before.allocations, before.live_objects),
        (after.allocations, after.live_objects),
        "the reload cannot allocate nor free anything in the managed heap"
    );

    // 3 is the value accumulated by generation 1; if the heap had been zeroed, the
    // result would be 10. The increment of 10 proves the new generation is the one that
    // runs; 13 proves its state is not new.
    assert_eq!(
        add_up.call_with(&session, counter).unwrap(),
        13,
        "the counter was zeroed by the reload"
    );
    assert_eq!(add_up.call_with(&session, counter).unwrap(), 23);
}

/// A callback registered before the edit starts running the new implementation.
///
/// The address is captured before the reload and used afterwards, without being resolved
/// again — it is what an application does when it keeps a function pointer in a
/// callback registry. It stays correct because it points to the trampoline.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn callback_registered_before_edit_runs_new_body() {
    let ir = |value: i64| format!("define i64 @df_fn_0() {{\n  ret i64 {value}\n}}\n");

    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(1))
        .expect("generation 1");

    // The application's «callback registry»: an address kept from before.
    let callback = session.stable_entry("df_fn_0").expect("stable entry");
    let address_before = session.lookup("df_fn_0").expect("entry address");
    assert_eq!(callback.call(&session).unwrap(), 1);

    session.hot_reload("app", &ir(2)).expect("generation 2");
    assert_eq!(
        session.lookup("df_fn_0").expect("entry address"),
        address_before,
        "the stable entry cannot change address between generations"
    );
    assert_eq!(
        callback.call(&session).unwrap(),
        2,
        "the old callback stayed on the old body"
    );

    session.hot_reload("app", &ir(3)).expect("generation 3");
    assert_eq!(callback.call(&session).unwrap(), 3);
    assert_eq!(session.retained_generations(), 3);
}

/// A reload failure does not destroy the working version.
///
/// This is the most important requirement of the work, and these are three different failures:
/// Dart that does not compile (it does not even reach the JIT), invalid IR (stage `parse-ir`) and
/// valid IR the session cannot link (stage `contract`). After the three, the
/// good version keeps running, and a valid reload still works.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn reload_failure_does_not_destroy_good_version() {
    let good_ir = compile_dart_to_ir("int teste() => 7;\nvoid main() { print(teste()); }\n");

    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &good_ir)
        .expect("generation 1");
    let (out, _) = session.run_entry_capturing().expect("good execution");
    assert_eq!(out, "7\n");

    // 1. Dart that does not compile: the failure is the front end's and the JIT is not even called.
    let error = dartforge_compiler::compile_llvm("int teste() => ;\nvoid main() {}\n")
        .expect_err("this Dart should not compile");
    assert!(!format!("{error:?}").is_empty());

    // 2. Invalid IR.
    let error = session
        .hot_reload("app", "THIS IS NOT LLVM IR @@@ {{{")
        .expect_err("invalid IR must fail");
    assert_eq!(error.stage, "parse-ir");

    // 3. Valid IR whose references the session cannot resolve. Refused before
    //    any effect, with the missing name in the message.
    let error = session
        .hot_reload(
            "app",
            "declare i64 @sqlite3_open(i64)\n\
             define i64 @df_fn_0() {\n  %r = call i64 @sqlite3_open(i64 0)\n  ret i64 %r\n}\n\
             define void @dartforge_entry() {\n  ret void\n}\n",
        )
        .expect_err("an unknown external reference must fail");
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("sqlite3_open"), "{error}");

    // The good version stays exactly where it was, in generation 1.
    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(session.retained_generations(), 1);
    let (out, _) = session
        .run_entry_capturing()
        .expect("the good version must keep running");
    assert_eq!(out, "7\n");

    // And the session stays reloadable: nothing in it was left halfway.
    let new_ir = compile_dart_to_ir("int teste() => 8;\nvoid main() { print(teste()); }\n");
    session
        .hot_reload("app", &new_ir)
        .expect("valid reload after the failures");
    let (out, _) = session.run_entry_capturing().expect("execution of generation 2");
    assert_eq!(out, "8\n");
}

/// A signature change is refused with a diagnostic, without corrupting the session.
///
/// The message names the symbol and both signatures, because the reader needs to
/// know **what** became incompatible to decide to restart.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn changed_signature_is_rejected_with_diagnostic() {
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", "define i64 @df_fn_0() {\n  ret i64 1\n}\n")
        .expect("generation 1");
    let input = session.stable_entry("df_fn_0").expect("stable entry");
    assert_eq!(input.call(&session).unwrap(), 1);

    let error = session
        .hot_reload("app", "define i64 @df_fn_0(i64 %a0) {\n  ret i64 %a0\n}\n")
        .expect_err("a changed signature must be refused");
    assert_eq!(error.stage, "contract");
    assert_eq!(
        error.message,
        "the signature of df_fn_0 changed from i64 () to i64 (i64); \
         a calling contract change requires restarting the session"
    );

    // A function that disappears is also refused, and for the same reason: its
    // stable entry would be stuck on the old body.
    let error = session
        .hot_reload("app", "define i64 @df_fn_9() {\n  ret i64 1\n}\n")
        .expect_err("a missing function must be refused");
    assert_eq!(error.stage, "contract");
    assert!(error.message.contains("df_fn_0"), "{error}");

    // Session intact: same generation, same implementation, same result.
    assert_eq!(session.generation("app"), Some(1));
    assert_eq!(input.call(&session).unwrap(), 1);
    session
        .hot_reload("app", "define i64 @df_fn_0() {\n  ret i64 5\n}\n")
        .expect("the compatible body still reloads");
    assert_eq!(input.call(&session).unwrap(), 5);
}

/// A change in the number of fields of a class is refused before corrupting the heap.
///
/// Objects already alive keep the layout they were allocated with; new code reading
/// a field that does not exist would hit a runtime `panic`, or worse, another field.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn changed_class_layout_is_rejected() {
    let ir = |fields: i64| {
        format!(
            "declare i64 @dartforge_object_new(i64, i64)\n\
             define i64 @df_new_0() {{\n  \
             %h = call i64 @dartforge_object_new(i64 0, i64 {fields})\n  ret i64 %h\n}}\n"
        )
    };
    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &ir(2))
        .expect("generation 1");
    let error = session
        .hot_reload("app", &ir(3))
        .expect_err("a changed layout must be refused");
    assert_eq!(error.stage, "contract");
    assert!(
        error.message
            .contains("the class with id 0 had 2 fields and now has 3"),
        "{error}"
    );
    assert_eq!(session.generation("app"), Some(1));
}

/// Exclusive directory for the executable of the AOT side of the differential.
struct TempDirectory(std::path::PathBuf);
impl TempDirectory {
    /// Reserves a unique directory even under simultaneous test execution.
    fn new(label_name: &str) -> Self {
        let mark = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let file_path = std::env::temp_dir().join(format!(
            "dartforge-reload-{label_name}-{}-{mark}",
            std::process::id()
        ));
        std::fs::create_dir(&file_path).unwrap();
        Self(file_path)
    }
}
impl Drop for TempDirectory {
    /// Removes only the directory this test created.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Differential: after the reload, JIT and AOT agree on the same program.
///
/// The contract of both profiles cannot be loosened by hot reload. The reloaded
/// program must produce, in the live session, exactly what the AOT executable
/// of the **same Dart** produces — even having gone through the trampolines, which only
/// exist in the JIT.
#[test]
#[ignore = "requires LLVM-C.dll on PATH and native Clang/rustc (DARTFORGE_CLANG/DARTFORGE_RUSTC); use scripts/env.ps1"]
fn reloaded_jit_and_aot_agree() {
    const V1: &str = "\
class Contador {
  int valor;
  Contador(this.valor);
  int calcular() { return valor * 2; }
}
int extra() => 1;
void main() {
  Contador c = Contador(21);
  print(c.calcular());
  print(extra());
}
";
    const V2: &str = "\
class Contador {
  int valor;
  Contador(this.valor);
  int calcular() { return valor * 3 + 1; }
}
int extra() => 9;
void main() {
  Contador c = Contador(21);
  print(c.calcular());
  print(extra());
}
";
    let ir_v1 = compile_dart_to_ir(V1);
    let ir_v2 = compile_dart_to_ir(V2);

    let mut session = JitSession::new().expect("session");
    session.add_reloadable_module("app", &ir_v1).expect("v1");
    let (output_v1, _) = session.run_entry_capturing().expect("execution v1");
    assert_eq!(output_v1, "42\n1\n");
    session.hot_reload("app", &ir_v2).expect("v2");
    let (jit_output, _) = session.run_entry_capturing().expect("execution v2");

    let dir = TempDirectory::new("diferencial");
    let executable = dir.0.join(if cfg!(windows) {
        "programa.exe"
    } else {
        "programa"
    });
    dartforge_native::build_executable(
        &ir_v2,
        &executable,
        &dartforge_native::NativeOptions::default(),
    )
    .unwrap_or_else(|error| panic!("the AOT driver failed: {error}"));
    let result = std::process::Command::new(&executable).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let aot_output = String::from_utf8(result.stdout)
        .unwrap()
        .replace("\r\n", "\n");

    assert_eq!(jit_output, aot_output, "reloaded JIT diverged from the AOT");
    assert_eq!(jit_output, "64\n9\n");
}

// ───────────────────────────────────────────────────────────────────────────────
// Measurement of the reload cycle.
// ───────────────────────────────────────────────────────────────────────────────

/// Dart program of the measurement corpus, with the body parameterized by the generation.
///
/// It has a class, a method, dispatch, a string and a top-level function, so that analysis
/// has work comparable to a real edit, and not to a `ret i64`.
fn measurement_source(generation: u64) -> String {
    format!(
        "\
class Caixa {{
  int valor;
  Caixa(this.valor);
  int dobro() {{ return valor * 2; }}
}}
String etiqueta(String nome) {{ return 'caixa ' + nome; }}
int base() => {generation};
void main() {{
  Caixa c = Caixa(base());
  int total = 0;
  for (int i = 1; i <= 8; i = i + 1) {{
    total = total + i;
  }}
  print(total + c.dobro());
  print(etiqueta('um'));
}}
"
    )
}

/// Dart analysis and IR generation, timed separately.
///
/// Repeats the pipeline of `dartforge_compiler::compile_llvm_with_options` because the
/// public function returns both phases added together, and `docs/DESEMPENHO.md` forbids
/// obtaining a phase by subtraction. The returned IR is compared with the public
/// function's in the test itself; if the compiler pipeline changes, the test reports it.
fn compile_measuring(source: &str) -> (std::time::Duration, std::time::Duration, String) {
    let start = std::time::Instant::now();
    let tokens = dartforge_lexer::lex(source).expect("lex");
    let mut ast = dartforge_parser::parse(&tokens, source.len()).expect("parse");
    let expansion = dartforge_macros::expand(&mut ast, source.len()).expect("macros");
    dartforge_hir::expand_mixins(&mut ast).expect("mixins");
    let resolution = dartforge_semantic::analyze(&ast).expect("semantics");
    let analysis = start.elapsed();
    drop(expansion);

    let start = std::time::Instant::now();
    let ir = dartforge_llvm::emit(&dartforge_hir::lower_resolved(ast, resolution)).expect("emission");
    let emission = start.elapsed();
    (analysis, emission, ir)
}

/// Median and p95 of a sample, in microseconds, by the project's methodology.
fn median_and_p95(samples: &mut [std::time::Duration]) -> (f64, f64) {
    samples.sort_unstable();
    let micros = |d: std::time::Duration| d.as_secs_f64() * 1e6;
    let median = micros(samples[samples.len() / 2]);
    let ix = ((samples.len() as f64) * 0.95).ceil() as usize - 1;
    (median, micros(samples[ix.min(samples.len() - 1)]))
}

/// Measures the reload cycle per stage and prints the table of `docs/JIT.md`.
///
/// Run with `-- --ignored --nocapture`. The important reading is not the total: it is
/// the proportion. `PLANO.md` insists on that because a backend twice as fast
/// at native generation does not make the reload twice as fast if generation is
/// a small fraction of the cycle — the same arithmetic as increment 26.
#[test]
#[ignore = "measurement; requires LLVM-C.dll reachable by the loader; use scripts/env.ps1 and --nocapture"]
fn reload_cycle_measurement() {
    // The hand-assembled pipeline must produce the same IR as the public API.
    let (_, _, assembled_ir) = compile_measuring(&measurement_source(0));
    assert_eq!(
        assembled_ir,
        compile_dart_to_ir(&measurement_source(0)),
        "the measurement pipeline diverged from dartforge_compiler::compile_llvm"
    );

    const WARMUP: u64 = 3;
    const SAMPLES: u64 = 20;

    let mut session = JitSession::new().expect("session");
    session
        .add_reloadable_module("app", &assembled_ir)
        .expect("generation 1");
    session.run_entry_capturing().expect("initial execution");

    let mut analysis = Vec::new();
    let mut emission = Vec::new();
    let mut parse = Vec::new();
    let mut contrato = Vec::new();
    let mut addition = Vec::new();
    let mut trampolines = Vec::new();
    let mut linkage = Vec::new();
    let mut publication = Vec::new();
    let mut jit_total = Vec::new();
    let mut cycle = Vec::new();
    let mut bytes = 0usize;

    for generation in 1..=(WARMUP + SAMPLES) {
        let source = measurement_source(generation);
        let start = std::time::Instant::now();
        let (analysis_time, emission_time, ir) = compile_measuring(&source);
        let report_data = session
            .hot_reload("app", &ir)
            .unwrap_or_else(|error| panic!("recarga {generation}: {error}"));
        let total_cycle = start.elapsed();
        let (out, _) = session.run_entry_capturing().expect("execution after reload");
        assert_eq!(out, format!("{}\ncaixa um\n", 36 + 2 * generation));
        if generation <= WARMUP {
            continue;
        }
        analysis.push(analysis_time);
        emission.push(emission_time);
        parse.push(report_data.parse_ir);
        contrato.push(report_data.contract);
        addition.push(report_data.add_module);
        trampolines.push(report_data.stubs);
        linkage.push(report_data.link);
        publication.push(report_data.publish);
        jit_total.push(report_data.total);
        cycle.push(total_cycle);
        bytes = report_data.ir_bytes;
    }

    println!("\n# Reload cycle, {SAMPLES} samples after {WARMUP} warm-ups");
    println!(
        "# IR of {bytes} bytes; generations retained at the end: {}",
        session.retained_generations()
    );
    println!("| Stage | Median (µs) | p95 (µs) |");
    println!("| --- | --- | --- |");
    for (label_name, samples) in [
        ("analysis of the new Dart", &mut analysis),
        ("IR generation", &mut emission),
        ("IR parsing (parse-ir)", &mut parse),
        ("contract check", &mut contrato),
        ("adding the module to the JIT", &mut addition),
        ("publishing the trampolines", &mut trampolines),
        ("in-memory linking", &mut linkage),
        ("publishing the pointers", &mut publication),
        ("total do hot_reload", &mut jit_total),
        ("complete cycle", &mut cycle),
    ] {
        let (median, p95) = median_and_p95(samples);
        println!("| {label_name} | {median:.1} | {p95:.1} |");
    }
}
