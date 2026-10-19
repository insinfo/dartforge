//! Destination validation happens before source loading and never changes JIT's host.
#![cfg(feature = "native")]
use std::process::Command;

#[test]
fn target_errors_are_actionable() {
    if cfg!(windows) && !cfg!(target_arch = "x86_64") {
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge"))
            .args(["aot", "missing.dart", "out.exe"]).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("requires x64"));
        return;
    }
    for (args, message) in [
        (
            vec!["aot", "missing.dart", "--target", "windows-arm64"],
            "unsupported native target",
        ),
        (vec!["aot", "missing.dart", "--target"], "requires a value"),
        (
            vec![
                "aot",
                "missing.dart",
                "--target=linux-arm64",
                "--target=linux-x64",
            ],
            "specified twice",
        ),
        (
            vec!["run", "missing.dart", "--target=linux-arm64"],
            "JIT runs on its host",
        ),
        (
            vec!["aot", "missing.dart", "--target-bundle", "missing-bundle"],
            "target bundle",
        ),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
