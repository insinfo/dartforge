//! Contract policy errors are diagnosed before loading source or SDK files.
#![cfg(feature = "native")]

use std::process::Command;

fn arguments(command: &str, flag: &str) -> Vec<String> {
    let output = if command == "aot" { vec!["out"] } else { vec!["-o", "out"] };
    [vec![command, "missing.dart"], output, vec![flag]]
        .concat().into_iter().map(str::to_string).collect()
}

#[test]
fn invalid_contract_modes_are_rejected_by_both_native_commands() {
    for command in ["aot", "compile-native"] {
        for flag in ["--dartforge-contracts=trust", "--dartforge-contracts="] {
            let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
                .args(arguments(command, flag)).output().unwrap();
            assert!(!output.status.success());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(diagnostic.contains("expected verify or ignore"), "{diagnostic}");
            assert!(!diagnostic.contains("error loading"), "{diagnostic}");
        }
    }
}

#[test]
fn contract_mode_requires_a_value() {
    for command in ["aot", "compile-native"] {
        let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
            .args(arguments(command, "--dartforge-contracts"))
            .output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("requires verify or ignore"));
    }
}
