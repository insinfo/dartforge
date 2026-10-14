//! PE subsystem options reject invalid values before loading or compiling Dart.
#![cfg(feature = "native")]

use std::process::Command;

fn reject(args: &[&str], expected: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .args(args)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(expected),
        "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn missing_and_invalid_subsystems_are_rejected() {
    for args in [
        vec!["compile-native", "missing.dart", "--subsystem=wrong"],
        vec!["compile-native", "missing.dart", "--subsystem", "wrong"],
        vec!["aot", "missing.dart", "missing.exe", "--subsystem=wrong"],
        vec!["aot", "missing.dart", "missing.exe", "--subsystem", "wrong"],
    ] {
        reject(&args, "--subsystem=wrong: the subsystems are `console` and `gui`");
    }
    for args in [
        vec!["compile-native", "missing.dart", "--subsystem"],
        vec!["aot", "missing.dart", "missing.exe", "--subsystem"],
    ] {
        reject(&args, "--subsystem requires console or gui");
    }
}

#[cfg(not(windows))]
#[test]
fn gui_is_rejected_on_other_hosts_including_ir_only() {
    for args in [
        vec!["compile-native", "missing.dart", "--subsystem=gui"],
        vec!["compile-native", "missing.dart", "--emit-ir", "--subsystem", "gui"],
        vec!["aot", "missing.dart", "missing.exe", "--subsystem=gui"],
    ] {
        reject(&args, "--subsystem=gui is only supported on Windows");
    }
}

#[test]
fn default_ignores_an_inherited_gui_policy() {
    for args in [
        vec!["compile-native", "missing.dart", "--digest"],
        vec!["compile-native", "missing.dart", "--digest", "--subsystem=console"],
        vec!["aot", "missing.dart", "missing.exe"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
            .args(&args).env("DARTFORGE_SUBSYSTEM", "wrong").output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("--subsystem="), "{args:?}: {stderr}");
        assert!(stderr.contains("missing.dart"), "{args:?}: {stderr}");
    }
}
