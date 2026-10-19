//! The production profile of `aot --optimize` (`DartFlags::PRODUCTION`): no
//! asserts, `dart.vm.product` true, and the platform constants folded and
//! propagated so that the other systems' backends leave the executable.

use dartforge_emit_native::{CompileOptions, DartFlags};
use std::path::{Path, PathBuf};

fn sdk() -> PathBuf {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .map(PathBuf::from)
        .or_else(|_| dartforge_emit_native::dart_sdk())
        .expect("test SDK");
    assert!(sdk.join("libraries.json").is_file(), "test SDK without libraries.json: {}", sdk.display());
    sdk
}

fn compile(entry: &Path, exe: &Path, flags: DartFlags) {
    let (entry, exe, sdk) = (entry.to_path_buf(), exe.to_path_buf(), sdk());
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions { contracts: dartforge_emit_native::contracts::ContractMode::Verify,
            sdk: Some(&sdk), packages: None, timings: false,
            optimize: true, language_version: None, experiments: Vec::new(), debug: false,
            generator: None,
            cpu: None,
        };
        dartforge_emit_native::compile_with_flags(&entry, &exe, &options, flags)
            .unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    }).unwrap().join().unwrap();
}

fn run(exe: &Path) -> String {
    let output = std::process::Command::new(exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

/// `dart compile exe` skips the asserts without evaluating condition or
/// message and defines `dart.vm.product`; `dart run --enable-asserts` (the
/// oracle of the corpus, `DartFlags::DEVELOPMENT`) runs them and leaves
/// `dart.vm.product` false. `hasEnvironment` is true in both.
#[test]
#[ignore = "compiles the source SDK in production (slow when cold); runs in CI"]
fn production_flags_follow_dart_compile_exe() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("profile.dart");
    std::fs::write(&entry, r#"
int evaluated = 0;
bool note(bool value) { evaluated++; return value; }
class Box { final int n; Box(this.n) : assert(note(n > 0), 'positive'); }
void main() {
  try {
    assert(note(false), 'message ${note(true)}');
    print('assert skipped');
  } on AssertionError catch (e) {
    print('AssertionError: ${e.message}');
  }
  print('evaluated: $evaluated');
  try { Box(0); print('box built'); } on AssertionError { print('box refused'); }
  print('product: ${const bool.fromEnvironment("dart.vm.product")}');
  print('has product: ${const bool.hasEnvironment("dart.vm.product")}');
  print('profile: ${const bool.fromEnvironment("dart.vm.profile", defaultValue: true)}');
  print('text: ${const String.fromEnvironment("dart.vm.product", defaultValue: "absent")}');
}
"#).unwrap();
    let production = dir.path().join("production.exe");
    compile(&entry, &production, DartFlags::PRODUCTION);
    assert_eq!(run(&production),
        "assert skipped\nevaluated: 0\nbox built\nproduct: true\nhas product: true\nprofile: false\ntext: true\n");
    let development = dir.path().join("development.exe");
    compile(&entry, &development, DartFlags::DEVELOPMENT);
    assert_eq!(run(&development),
        "AssertionError: message true\nevaluated: 2\nbox refused\nproduct: false\nhas product: true\nprofile: false\ntext: false\n");
}

/// `Platform.operatingSystem` folds to the target and travels into the
/// omitted optional parameter of the resolver (`optimize/constant_args.rs`):
/// the backends of the other systems are not in the IR, nor in the program.
#[test]
#[ignore = "compiles the source SDK in production (slow when cold); runs in CI"]
fn platform_constant_prunes_the_other_backends() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("main.dart");
    std::fs::write(&entry, include_str!("fixtures/constant_args/main.dart")).unwrap();
    let (mine, others, separator) = match std::env::consts::OS {
        "macos" => ("MacBackend", ["WindowsBackend", "LinuxBackend"], "posix separator"),
        "windows" => ("WindowsBackend", ["MacBackend", "LinuxBackend"], "windows separator"),
        _ => ("LinuxBackend", ["WindowsBackend", "MacBackend"], "posix separator"),
    };
    let ir = {
        let (entry, sdk) = (entry.clone(), sdk());
        std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
            let options = CompileOptions { contracts: dartforge_emit_native::contracts::ContractMode::Verify,
                sdk: Some(&sdk), packages: None, timings: false,
                optimize: true, language_version: None, experiments: Vec::new(), debug: false,
                generator: None,
                cpu: None,
            };
            dartforge_emit_native::emit_ir_with_flags(&entry, &options, DartFlags::PRODUCTION)
                .unwrap_or_else(|e| panic!("did not emit:\n{e}")).text
        }).unwrap().join().unwrap()
    };
    // The class-id catalog remains available to tooling even for dead classes.
    // Executable definitions, rather than catalog comments, prove the pruning.
    let definitions: Vec<_> = ir.lines().filter(|line| line.starts_with("define ")).collect();
    assert!(definitions.iter().any(|line| line.contains(mine)), "the target's backend stays");
    for other in others {
        assert!(!definitions.iter().any(|line| line.contains(other)), "{other} bodies should have left the IR");
    }
    let exe = dir.path().join("main.exe");
    compile(&entry, &exe, DartFlags::PRODUCTION);
    let name = match std::env::consts::OS { "macos" => "macos", "windows" => "win32", _ => "linux" };
    assert_eq!(run(&exe), format!("{name} backend\n{separator}\n{}\n", if cfg!(windows) { 2 } else { 1 }));
}

/// Batch map fills and direct list allocation must preserve the implicit
/// downcasts, their evaluation order, and reified generic element types.
#[test]
#[ignore = "compiles the source SDK in production (slow when cold); runs in CI"]
fn literal_implicit_casts_match_dart() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("literal_casts.dart");
    std::fs::write(&entry, include_str!("../../../corpus/native/145_literal_implicit_casts.dart")).unwrap();
    let oracle = std::process::Command::new("dart").arg("run").arg(&entry).output().unwrap();
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let expected = String::from_utf8_lossy(&oracle.stdout).replace("\r\n", "\n");
    let exe = dir.path().join("literal_casts.exe");
    compile(&entry, &exe, DartFlags::PRODUCTION);
    assert_eq!(run(&exe), expected);
    let stressed = std::process::Command::new(&exe).env("DARTFORGE_GC_STRESS", "1").output().unwrap();
    assert!(stressed.status.success(), "{}", String::from_utf8_lossy(&stressed.stderr));
    assert_eq!(String::from_utf8_lossy(&stressed.stdout).replace("\r\n", "\n"), expected);
}
