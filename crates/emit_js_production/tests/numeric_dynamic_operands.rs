//! Statically numeric receivers must check dynamic arguments before JS coercion.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn node_output(js: &str) -> String {
    let mut node = Command::new("node")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node");
    node.stdin
        .take()
        .unwrap()
        .write_all(format!("globalThis.self=globalThis;\n{js}").as_bytes())
        .unwrap();
    let output = node.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn checked_numeric_dynamic_arguments_match_vm_and_checked_o2() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/numeric_dynamic_operands.dart");
    let oracle = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .expect("Dart");
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let expected = String::from_utf8(oracle.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    assert!(expected.contains("+:custom=error:true:[left, right]"));
    assert!(expected.contains("compound=error:true:6.00:[get, right]"));
    assert!(expected.contains("left-throw=true:[left-throw]"));
    assert!(expected.contains("right-throw=true:[left, right-throw]"));
    let directory = std::env::temp_dir().join(format!(
        "dartforge-numeric-dynamic-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let official = directory.join("official-O2.js");
    let compiled = Command::new("dart")
        .args(["compile", "js", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(&official)
        .output()
        .expect("dart2js");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert_eq!(
        node_output(&std::fs::read_to_string(&official).unwrap()),
        expected,
        "the official backend's error classes are compared, not normalized away"
    );
    for minify in [false, true] {
        let options = dartforge_emit_js_production::Options {
            minify,
            ..Default::default()
        };
        let production = dartforge_emit_js_production::compile_own_sdk(
            &source,
            None,
            None,
            options,
            &Default::default(),
        )
        .expect("checked production compilation");
        assert_eq!(node_output(&production.js), expected, "minify={minify}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}
