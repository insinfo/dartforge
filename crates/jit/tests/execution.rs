//! Execution through the JIT and agreement with the AOT executable.
//!
//! The central test is the differential: the **same** LLVM IR, produced only
//! once by the native emitter, run by both profiles — ORCv2 (through the isolated
//! executor `dartforge-run-ir`) and AOT (Clang + linking with the runtime) —
//! must produce the same stdout and the same exit code. Everything else
//! protects the boundary with the LLVM C API and the pre-check of externals.
//!
//! Running through a subprocess is not a detail: the runtime ends the process
//! (`process::exit`) in the same cases in which the AOT executable ends, so a
//! program with an uncaught exception would bring down the test process.
use dartforge_jit::{JitSession, RUNTIME_SYMBOLS};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Exclusive directory; the test does not remove externally provided paths.
struct OutputDir(PathBuf);
impl OutputDir {
    /// Reserves a unique directory even in simultaneous executions.
    fn new(label: &str) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-jit-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDir {
    /// Cleans only the directory created by the test itself.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Observable result of an execution: what both profiles must match.
#[derive(Debug, PartialEq, Eq)]
struct Execution {
    stdout: String,
    code: Option<i32>,
    stderr: String,
}

/// Time limit of each process run by the tests.
///
/// A program that does not end cannot hang the suite. In CI this happened
/// and the job only stopped at the 60 min limit, without saying where it was.
const PROCESS_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Runs a process with a time limit and normalizes line breaks.
///
/// stdout and stderr are read on their own threads, so that a full pipe does not
/// block the child. When the limit is exceeded, the process is killed and the test fails
/// saying which command it was.
fn execute_fn(command: &mut Command) -> Execution {
    use std::io::Read;
    use std::process::Stdio;
    let description = format!("{command:?}");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("process did not start");
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let err_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err.read_to_end(&mut b);
        b
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > PROCESS_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{description} exceeded {PROCESS_LIMIT:?} and was killed");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = out_reader.join().unwrap();
    let stderr = err_reader.join().unwrap();
    Execution {
        stdout: String::from_utf8_lossy(&stdout).replace("\r\n", "\n"),
        code: status.code(),
        stderr: String::from_utf8_lossy(&stderr).replace("\r\n", "\n"),
    }
}

/// Writes to stderr the differential stage and the time since the start.
///
/// Without it, a slow or hung test in CI does not say which stage it is in. The
/// write goes directly to the process's `stderr`, and not through `eprintln!`, which the
/// test harness captures and only shows if the test fails. This way the line
/// shows up in the log while the test runs.
fn stage(start: std::time::Instant, name: &str) {
    use std::io::Write;
    let line = format!("[differential {:>7.1?}] {name}\n", start.elapsed());
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// Runs an IR through the JIT's isolated executor.
fn run_through_jit(dir: &Path, ir: &str) -> Execution {
    let file = dir.join("programa.ll");
    std::fs::write(&file, ir).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge-run-ir"));
    command.arg(&file);
    // The IR with the source SDK (the default) imports the runtime and the SDK from the
    // shared library, which the executor receives through the environment, as
    // `dartforge run` finds it in the cache.
    if dartforge_jit::ir_uses_source_sdk(ir) && std::env::var_os("DARTFORGE_SDK_DLL").is_none() {
        let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source SDK library");
        command.env("DARTFORGE_SDK_DLL", dll);
    }
    execute_fn(&mut command)
}

// ─── Symbol table × emitter, without LLVM ─────────────────────────────────

/// Every symbol the emitter declares exists in the table published by the JIT.
///
/// The emitter's list of externs is `dartforge_emit_native::llvm::externs::EXTERNS`
/// (the effects table, single source on the emitter side). The JIT table is
/// generated from the runtime source. When the two diverge, the AOT fails at linking
/// and the JIT at the pre-check; this test reports it first, without LLVM or Clang.
#[test]
fn every_emitter_extern_exists_in_runtime() {
    let declared_set: Vec<&str> = dartforge_emit_native::llvm::externs::EXTERNS.iter().map(|e| e.name()).collect();
    assert!(declared_set.len() > 100, "unexpected externs table: {declared_set:?}");
    let missing_list: Vec<&&str> = declared_set.iter().filter(|name| !RUNTIME_SYMBOLS.contains(name)).collect();
    assert!(missing_list.is_empty(), "the emitter declares externs the runtime does not define: {missing_list:?}");
}

// ─── In-process session, hand-written IR ──────────────────────────────────

/// Invalid IR becomes a `Result` with the stage and the LLVM diagnostic, without `panic`.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn invalid_ir_becomes_result_with_message() {
    let error = dartforge_jit::run_ir("this is definitely not LLVM IR").unwrap_err();
    assert_eq!(error.stage, "parse-ir");
    assert!(error.message.starts_with("invalid IR"), "{error}");

    // A syntactically valid IR but without the entry fails at lookup, not before.
    let mut session = JitSession::new().unwrap();
    session
        .add_ir_module("sem_entrada", "define i64 @sem_entrada() {\n  ret i64 0\n}\n")
        .unwrap();
    let error = session.run_entry().unwrap_err();
    assert_eq!(error.stage, "lookup");
    assert!(error.message.contains("dartforge_entry"), "{error}");
}

/// IR the parser accepts but the verifier refuses (a value that does not dominate its
/// use) is refused by the JIT, as Clang refuses it in the AOT.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn ir_rejected_by_verifier_does_not_run() {
    let ir = "define void @dartforge_entry() {\nentrada:\n  br i1 true, label %a, label %b\n\
              a:\n  %x = add i64 1, 2\n  br label %b\nb:\n  %y = add i64 %x, 1\n  ret void\n}\n";
    let error = dartforge_jit::run_ir(ir).unwrap_err();
    assert_eq!(error.stage, "parse-ir");
    assert!(error.message.contains("verifier"), "{error}");
}

/// An external that neither the runtime nor the listed CRT define is refused with its name.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn unknown_external_fails_loudly() {
    let mut session = JitSession::new().unwrap();
    let error = session
        .add_ir_module(
            "unknown",
            "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  \
             call void @sqlite3_open()\n  ret void\n}\n",
        )
        .unwrap_err();
    assert_eq!(error.stage, "symbols");
    assert!(error.message.contains("sqlite3_open"), "{error}");
    assert!(session.module_names().is_empty());
}

/// A divergent target is refused showing both strings; the host's passes.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn divergent_target_fails_loudly() {
    let mut session = JitSession::new().unwrap();
    // A triple that is never the host's.
    let other_one = "riscv64-unknown-linux-gnu";
    let error = session
        .add_ir_module("other_target", &format!("target triple = \"{other_one}\"\ndefine void @f() {{\n  ret void\n}}\n"))
        .unwrap_err();
    assert_eq!(error.stage, "layout");
    assert!(error.message.contains(other_one), "{error}");

    // The header the native emitter writes (`target::ir_header`) must
    // be compatible with this process's LLJIT: same architecture,
    // system and ABI, even if the installed LLVM spells the triple another
    // way (`x86_64-pc-linux-gnu`, `arm64-apple-macosx15.7.3`).
    let header = dartforge_emit_native::target::ir_header();
    session
        .add_ir_module("alvo_do_emissor", &format!("{header}define void @g() {{\n  ret void\n}}\n"))
        .unwrap();
}

/// Two modules coexist in the session and one resolves symbols of the other.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn two_modules_in_one_session() {
    let library = "define i64 @jit_dobro(i64 %n) {\n  %r = mul i64 %n, 2\n  ret i64 %r\n}\n";
    let program = "declare i64 @jit_dobro(i64)\ndefine void @dartforge_entry() {\n  \
                    %v = call i64 @jit_dobro(i64 21)\n  ret void\n}\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("biblioteca", library).unwrap();
    session.add_ir_module("programa", program).unwrap();
    assert_eq!(session.module_names(), vec!["biblioteca", "programa"]);
    assert_ne!(session.lookup("jit_dobro").unwrap(), 0);
    let report_data = session.run_entry().unwrap();
    assert_eq!(report_data.exit_code, 0);
    assert!(report_data.total >= report_data.lookup);
}

/// Two complete programs do not fit in the same session: both define the entry.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn duplicate_entry_becomes_result() {
    let min = "define void @dartforge_entry() {\n  ret void\n}\n";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("primeiro", min).unwrap();
    let error = session.add_ir_module("segundo", min).unwrap_err();
    assert_eq!(error.stage, "add-module");
    assert!(error.message.contains("dartforge_entry"), "{error}");
    assert_eq!(session.module_names(), vec!["primeiro"]);
}

/// Removing the module unloads its code and the entry is no longer resolvable.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn removing_module_unloads_entry() {
    let mut session = JitSession::new().unwrap();
    session
        .add_ir_module("programa", "define void @dartforge_entry() {\n  ret void\n}\n")
        .unwrap();
    assert_eq!(session.run_entry().unwrap().exit_code, 0);
    // A second execution starts from a clean runtime and also ends fine.
    assert_eq!(session.run_entry().unwrap().exit_code, 0);
    session.remove_module(0).unwrap();
    assert!(session.module_names().is_empty());
    assert_eq!(session.run_entry().unwrap_err().stage, "lookup");
    assert_eq!(session.remove_module(0).unwrap_err().stage, "resource");
    assert_eq!(session.remove_module(7).unwrap_err().stage, "resource");
}

// ─── Isolated executor, hand-written IR ───────────────────────────────────

/// The executor prints through the runtime and returns the program's code.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn runner_prints_through_runtime() {
    let dir = OutputDir::new("executor");
    let ir = "\
@.s = private unnamed_addr constant [3 x i8] c\"ola\"
declare void @dartforge_print_i64(i64)
declare void @dartforge_print_bool(i8)
declare i64 @dartforge_string_new(ptr, i64)
declare void @dartforge_print_string(i64)
define void @dartforge_entry() {
  call void @dartforge_print_i64(i64 -42)
  call void @dartforge_print_bool(i8 1)
  %s = call i64 @dartforge_string_new(ptr @.s, i64 3)
  call void @dartforge_print_string(i64 %s)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "-42\ntrue\nola\n", "{execution:?}");
    assert_eq!(execution.code, Some(0), "{execution:?}");
}

/// An uncaught exception ends as in the AOT and in the VM: `Unhandled exception:`
/// and the message on stderr, code 255.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn runner_uncaught_exception_exits_with_255() {
    let dir = OutputDir::new("excecao");
    let ir = "\
declare void @dartforge_print_i64(i64)
declare void @dartforge_exception_throw(i64, i8)
define void @dartforge_entry() {
  call void @dartforge_print_i64(i64 1)
  call void @dartforge_exception_throw(i64 7, i8 1)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "1\n", "{execution:?}");
    assert_eq!(execution.code, Some(255), "{execution:?}");
    assert!(execution.stderr.starts_with("Unhandled exception:\n7\n"), "{execution:?}");
}

/// `double` and a frame larger than 4 KiB: the COFF path the old JIT never
/// exercised (`_fltused`, `__chkstk`, floating point constants).
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn runner_double_and_large_frame() {
    let dir = OutputDir::new("coff");
    let ir = "\
declare void @dartforge_print_f64(double)
define double @soma(double %a, double %b) {
  %buf = alloca [16384 x i8]
  store volatile i8 1, ptr %buf
  %r = fadd double %a, %b
  ret double %r
}
define void @dartforge_entry() {
  %v = call double @soma(double 1.25, double 1.25)
  call void @dartforge_print_f64(double %v)
  call void @dartforge_print_f64(double 0.1)
  ret void
}
";
    let execution = run_through_jit(&dir.0, ir);
    assert_eq!(execution.stdout, "2.5\n0.1\n", "{execution:?}");
    assert_eq!(execution.code, Some(0), "{execution:?}");
}

/// A failure of the JIT itself exits with 70 and the stage on stderr, without stdout.
#[test]
#[ignore = "requires LLVM-C.dll reachable by the loader; use scripts/env.ps1"]
fn runner_jit_failure_exits_with_70() {
    let dir = OutputDir::new("falha");
    let execution = run_through_jit(&dir.0, "declare void @nada()\ndefine void @dartforge_entry() {\n  call void @nada()\n  ret void\n}\n");
    assert_eq!(execution.code, Some(70), "{execution:?}");
    assert!(execution.stderr.contains("symbols"), "{execution:?}");
    assert!(execution.stdout.is_empty());
}

// ─── JIT × AOT differential over the native emitter's IR ──────────────────

/// Programs of the new track with the output the VM gives, each one going through
/// both profiles from the same IR.
///
/// The first three are the forms the emitter got wrong on 2026-09-23 (found
/// by this differential): `for` with the variable reassigned, instance
/// method and `print` of an `int` returned by a function. They were fixed with the
/// native representation contract (`crates/emit_native/tests/contract.rs`)
/// and stay here as a regression of both profiles.
const PROGRAMS: &[(&str, &str, &str)] = &[
    (
        "for",
        "\
void main() {
  int total = 0;
  for (int i = 1; i <= 4; i = i + 1) {
    total = total + i;
  }
  print(total);
  print(total > 5);
}
",
        "10\ntrue\n",
    ),
    (
        "metodo",
        "\
class Contador {
  int valor;
  Contador(this.valor);
  int dobro() { return valor * 2; }
}
void main() {
  Contador c = Contador(21);
  print(c.dobro());
}
",
        "42\n",
    ),
    (
        "retorno_int",
        "\
int soma(int a, int b) { return a + b; }
int maior(int a, int b) {
  if (a > b) {
    return a;
  } else {
    return b;
  }
}
void main() {
  print(soma(40, 2));
  print(soma(1, 2) > 2);
  print(maior(3, 9));
}
",
        "42\ntrue\n9\n",
    ),
    (
        "strings",
        "\
String saudacao(String nome) { return 'ola ' + nome; }
void main() {
  print('inicio');
  print(saudacao('mundo'));
}
",
        "inicio\nola mundo\n",
    ),
];

/// Emits the LLVM IR of a Dart file as the JIT receives it
/// (`emit_reloadable_ir`: without static objects, which the JIT does not have,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.11); the AOT driver links that same text.
fn emit_ir(input: &Path) -> String {
    let options = dartforge_emit_native::CompileOptions { contracts: dartforge_emit_native::contracts::ContractMode::Verify, sdk: None, packages: None, timings: false, optimize: false, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
    dartforge_emit_native::emit_reloadable_ir(input, &options, None)
        .unwrap_or_else(|error| panic!("the test program did not emit IR: {error}"))
        .text
}

/// Emits the IR of `source` once and runs that same text through both profiles.
fn run_in_both_profiles(label_name: &str, source: &str) -> (Execution, Execution) {
    let start = std::time::Instant::now();
    let dir = OutputDir::new(label_name);
    let file = dir.0.join("programa.dart");
    std::fs::write(&file, source).unwrap();
    stage(start, "emitir_ir");
    // Large stack like the CLI: lowering is recursive over the AST.
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || emit_ir(&file))
        .unwrap()
        .join()
        .unwrap();
    stage(start, &format!("IR with {} bytes; run through the JIT", ir.len()));

    let jit = run_through_jit(&dir.0, &ir);
    stage(start, &format!("JIT finished ({:?}); compile and link through the AOT", jit.code));

    let executable = dir.0.join("programa.exe");
    dartforge_emit_native::driver::compile_and_link(
        &ir,
        &executable,
        &dartforge_emit_native::driver::NativeDriverOptions::default(),
    )
    .unwrap_or_else(|error| panic!("the AOT driver failed: {error}"));
    stage(start, "executar o AOT");
    let aot = execute_fn(&mut Command::new(&executable));
    stage(start, &format!("AOT terminou ({:?})", aot.code));
    (jit, aot)
}

/// The same IR produces the same output and the same code in both profiles, and the
/// output is the VM's.
///
/// Central contract: development and production may diverge in compile
/// time, never in observable result. Divergence between the profiles
/// in a failing program is covered at scale by the CI `jit` job
/// (`dartforge-differential --jit-aot`, whole corpus).
#[test]
#[ignore = "requires LLVM-C.dll on PATH, Clang (DARTFORGE_CLANG), rustc and the Dart 3.6.2 SDK"]
fn jit_and_aot_agree_on_same_ir() {
    for (label_name, source, expected) in PROGRAMS {
        let (jit, aot) = run_in_both_profiles(label_name, source);
        assert_eq!(jit.stdout, aot.stdout, "{label_name}\nJIT {jit:?}\nAOT {aot:?}");
        assert_eq!(jit.code, aot.code, "{label_name}\nJIT {jit:?}\nAOT {aot:?}");
        assert_eq!(&jit.stdout, expected, "{label_name}: both profiles agree, but not with the VM: {jit:?}");
        assert_eq!(jit.code, Some(0), "{label_name}: {jit:?}");
    }
}

/// Consecutive cases that label the same body must deliver the bindings
/// of the winning pattern, including when the body captures the variable.
#[test]
#[ignore = "requires LLVM-C.dll on PATH, Clang (DARTFORGE_CLANG), rustc and the Dart 3.6.2 SDK"]
fn shared_cases_deliver_variables_to_body() {
    let source = r#"
sealed class Expr {}
class A extends Expr { final int x; A(this.x); }
class B extends Expr { final int x; B(this.x); }
int Function() ler(Expr e) {
  switch (e) {
    case A(:final x):
    case B(:final x):
      return () => x;
  }
}
void main() {
  print(ler(A(3))());
  print(ler(B(7))());
}
"#;
    let (jit, aot) = run_in_both_profiles("cases-compartilhados", source);
    assert_eq!(jit.stdout, "3\n7\n", "{jit:?}");
    assert_eq!(jit.stdout, aot.stdout, "JIT {jit:?}\nAOT {aot:?}");
    assert_eq!(jit.code, Some(0), "{jit:?}");
    assert_eq!(jit.code, aot.code, "JIT {jit:?}\nAOT {aot:?}");
}

/// Shadow-stack sessions need bounded COFF code/data too, not only maps mode.
/// Materialize several objects, keep them alive together, and check the actual
/// addresses after relocation rather than inspecting allocator configuration.
#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn windows_shadow_stack_code_and_data_have_bounded_rvas() {
    let mut session = JitSession::new().unwrap();
    for index in 0..16 {
        let ir = format!("@payload{index} = constant [4096 x i8] zeroinitializer, align 16\n\
            define i64 @reader{index}() uwtable {{\n\
              %p = ptrtoint ptr @payload{index} to i64\n\
              ret i64 %p\n}}\n");
        session.add_ir_module(&format!("bounded{index}"), &ir).unwrap();
        let code = session.lookup(&format!("reader{index}")).unwrap();
        let data = session.lookup(&format!("payload{index}")).unwrap();
        assert!(data > code, "COFF data must follow code: code={code:x} data={data:x}");
        assert!(data - code < (256 << 20), "COFF RVAs must fit in the bounded region");
    }
}

/// Backend-generated COFF floating-point pool symbols need the same ownership
/// policy as LLJIT's default layer, including when a later object repeats them.
#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn windows_constant_pool_symbols_materialize_across_objects() {
    let mut session = JitSession::new().unwrap();
    for index in 0..4 {
        let ir = format!("define double @scaled{index}(double %x) {{\n\
            %result = fmul double %x, 1.234567890000000e+02\n\
            ret double %result\n}}\n");
        session.add_ir_module(&format!("constant_pool{index}"), &ir).unwrap();
        assert_ne!(session.lookup(&format!("scaled{index}")).unwrap(), 0);
    }
}
