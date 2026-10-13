//! The policy is selected by the option, even with an inherited ARC environment.
#![cfg(feature = "native")]

use std::{
    path::Path,
    process::{Command, Output},
};

fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    cmd.env("DARTFORGE_MEMORY", "arc");
    cmd
}

fn check(output: Output, expected: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        expected,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn invalid_value_is_rejected_before_loading_the_source() {
    let mut cases = vec![
        vec!["compile-native", "--memory=wrong", "missing.dart"],
        vec!["aot", "missing.dart", "missing.exe", "--memory=wrong"],
    ];
    if cfg!(feature = "jit") {
        cases.extend([
            vec!["run", "--memory=wrong", "missing.dart"],
            vec!["reload", "missing.dart", "--memory=wrong"],
        ]);
    }
    for args in cases {
        let output = cli().args(&args).output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--memory=wrong: the memory policies"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "native proof with the source-built SDK and LLVM; run in CI"]
fn aot_only_enables_arc_with_an_explicit_option() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("tests/fixtures/memory_cli.dart");
    let dir = root.join(format!("../../target/memory-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for arc in [false, true] {
        let exe = dir.join(format!(
            "{}.{}",
            if arc { "arc" } else { "default" },
            if cfg!(windows) { "exe" } else { "bin" }
        ));
        let mut cmd = cli();
        cmd.arg("compile-native")
            .arg(&source)
            .arg("-o")
            .arg(&exe)
            .env("TEMP", &dir)
            .env("TMP", &dir);
        if arc {
            cmd.arg("--memory=arc");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        check(
            Command::new(exe)
                .env("DARTFORGE_MEMORY", "arc")
                .output()
                .unwrap(),
            if arc { "1\n\n" } else { "0\n\n" },
        );
    }
}

#[cfg(feature = "jit")]
#[test]
#[ignore = "JIT and reload proof with the source-built SDK and LLVM; run in CI"]
fn jit_and_reload_select_the_policy_and_keep_the_program_arguments() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/memory_cli.dart");
    for arc in [false, true] {
        let mut run = cli();
        run.arg("run");
        if arc {
            run.arg("--memory=arc");
        }
        run.arg(&source).arg("--memory=arc");
        check(
            run.output().unwrap(),
            if arc {
                "1\n--memory=arc\n"
            } else {
                "0\n--memory=arc\n"
            },
        );
        for restart in [false, true] {
            let mut reload = cli();
            reload.arg("reload").arg(&source).arg("--once");
            if arc {
                reload.arg("--memory=arc");
            }
            if restart {
                reload.arg("--restart");
            }
            check(
                reload.output().unwrap(),
                if arc { "1\n\n" } else { "0\n\n" },
            );
        }
    }
}
