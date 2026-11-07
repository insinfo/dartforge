//! Core print follows the host CRT mode without changing raw dart:io output.
#![cfg(feature = "native")]
use std::path::PathBuf;
use std::process::{Command, Output};
const CASES: &[&str] = &[
    "none",
    "stdout-get",
    "stdout-terminal",
    "stdout-write-empty",
    "stdout-flush",
    "stdout-write",
    "stderr-get",
    "stderr-write",
    "stdin-get",
    "platform",
    "stdout-line-terminator",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("dartforge-stdio-{}-{unique}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires the host Dart 3.6.2 SDK, LLVM and a preserved native compiler"]
fn print_and_stdio_preserve_exact_host_bytes() {
    let fixture = Fixture::new();
    let source = fixture.0.join("main.dart");
    std::fs::write(&source, include_str!("fixtures/print_stdio_modes.dart")).unwrap();
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("host Dart 3.6.2 SDK"));
    assert_eq!(
        std::fs::read_to_string(sdk.join("version")).unwrap().trim(),
        "3.6.2"
    );
    let dart = sdk
        .join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let official = fixture.0.join("official.exe");
    success(
        &Command::new(&dart)
            .args(["compile", "exe"])
            .arg(&source)
            .arg("-o")
            .arg(&official)
            .output()
            .unwrap(),
    );
    let references: Vec<_> = CASES
        .iter()
        .map(|case| {
            let vm = Command::new(&dart).arg(&source).arg(case).output().unwrap();
            let aot = Command::new(&official).arg(case).output().unwrap();
            success(&vm);
            success(&aot);
            assert_eq!(aot.stdout, vm.stdout, "official stdout: {case}");
            assert_eq!(aot.stderr, vm.stderr, "official stderr: {case}");
            vm
        })
        .collect();
    let compiler = std::env::var_os("DARTFORGE_COMPILER")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge").into());
    for optimize in [false, true] {
        let executable = fixture.0.join(format!("native-{optimize}.exe"));
        let mut build = Command::new(&compiler);
        build
            .arg("aot")
            .arg(&source)
            .arg(&executable)
            .arg("--sdk")
            .arg(sdk.join("lib"));
        if optimize {
            build.arg("--optimize");
        }
        success(&build.output().unwrap());
        for (case, expected) in CASES.iter().zip(&references) {
            let actual = Command::new(&executable).arg(case).output().unwrap();
            success(&actual);
            assert_eq!(
                actual.stdout, expected.stdout,
                "native stdout: optimize={optimize}, {case}"
            );
            assert_eq!(
                actual.stderr, expected.stderr,
                "native stderr: optimize={optimize}, {case}"
            );
        }
    }
    #[cfg(feature = "jit")]
    for (case, expected) in CASES.iter().zip(&references) {
        let actual = Command::new(&compiler)
            .arg("run")
            .arg("--sdk")
            .arg(sdk.join("lib"))
            .arg(&source)
            .args(["--", case])
            .output()
            .unwrap();
        success(&actual);
        assert_eq!(actual.stdout, expected.stdout, "JIT stdout: {case}");
        assert_eq!(actual.stderr, expected.stderr, "JIT stderr: {case}");
    }
}
