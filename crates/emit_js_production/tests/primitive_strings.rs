//! `print`, interpolations and `toString()` of non-nullable strings, numbers
//! and booleans are converted in place, `i % k` with a positive literal `k`
//! is written inline, a list literal iterated by `for-in` is a bare array,
//! and the rti bootstrap is left out when no type is ever
//! evaluated. Each switch, on and off, gives the VM's output; and a program
//! that only prints a string carries no rti.

use std::path::{Path, PathBuf};
use std::process::Command;

fn directory(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dartforge-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vm(source: &Path) -> String {
    let vm = Command::new("dart")
        .arg("run")
        .arg(source)
        .output()
        .unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    String::from_utf8(vm.stdout).unwrap().replace("\r\n", "\n")
}

/// Compiles `source` with `env` and runs it on Node: (stdout, file size).
fn production(source: &Path, artifact: &Path, env: &[(&str, &str)]) -> (String, usize) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"));
    c.arg(source).arg("-o").arg(artifact);
    for (k, v) in env {
        c.env(k, v);
    }
    let result = c.output().unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let run = Command::new("node")
        .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
        .arg(artifact)
        .output()
        .unwrap();
    assert!(
        run.status.success() && run.stderr.is_empty(),
        "{env:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    (
        String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
        std::fs::metadata(artifact).unwrap().len() as usize,
    )
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn every_switch_matches_the_vm() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/primitive_strings.dart");
    let expected = vm(&source);
    assert!(expected.contains("-7 % 3 = 2"));
    let dir = directory("primitive-strings");
    let switches = [
        None,
        Some("DARTFORGE_JSPROD_RTI_GATE"),
        Some("DARTFORGE_JSPROD_OBJECT_GUARD"),
        Some("DARTFORGE_JSPROD_PRINT_LINE"),
        Some("DARTFORGE_JSPROD_STR_PRIMITIVES"),
        Some("DARTFORGE_JSPROD_LITERAL_MODULO"),
        Some("DARTFORGE_JSPROD_BARE_LOOP_LISTS"),
    ];
    for (i, switch) in switches.iter().enumerate() {
        let env: Vec<(&str, &str)> = switch.iter().map(|s| (*s, "0")).collect();
        let (out, _) = production(&source, &dir.join(format!("out{i}.js")), &env);
        assert_eq!(out, expected, "{switch:?}=0");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn printing_a_string_carries_no_rti() {
    let dir = directory("print-hi");
    let source = dir.join("hi.dart");
    std::fs::write(&source, "void main() { print('hi'); print(1 + 2); }\n").unwrap();
    let expected = vm(&source);
    let (out, size) = production(&source, &dir.join("hi.js"), &[]);
    assert_eq!(out, expected);
    // Measured at 2.5 KB; with the rti it was 30 KB.
    assert!(size < 8000, "print('hi') takes {size} bytes");
    let (out, size) = production(
        &source,
        &dir.join("hi0.js"),
        &[("DARTFORGE_JSPROD_RTI_GATE", "0")],
    );
    assert_eq!(out, expected);
    assert!(size > 8000, "the switch keeps the rti ({size} bytes)");
    std::fs::remove_dir_all(dir).unwrap();
}
