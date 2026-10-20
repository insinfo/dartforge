//! Nonnegative counter modulo keeps checks and refuses escaping/mutable bindings.

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

fn function<'a>(js: &'a str, name: &str) -> &'a str {
    let start = js
        .find(&format!("function {name}("))
        .expect("live function");
    let tail = &js[start..];
    &tail[..tail.find("\n};").expect("function end")]
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn canonical_counters_and_refusals_match_checked_vm_and_o2() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/positive_counter_modulo.dart");
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
    let directory = std::env::temp_dir().join(format!(
        "dartforge-counter-{}-{}",
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
        expected
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
        .expect("production compilation");
        if !minify {
            for (name, divisor) in [
                ("canonical", 3),
                ("prefix", 7),
                ("getterBound", 5),
                ("callBound", 5),
                ("userNamedIntrinsic", 11),
                ("maximumDivisor", 1000000000),
            ] {
                assert!(
                    function(&production.js, name).contains(&format!("i % {divisor}")),
                    "actual fast path: {name}"
                );
            }
            for name in [
                "negative",
                "mutated",
                "shadowed",
                "captured",
                "callback",
                "nested",
                "nonunit",
                "negativeDivisor",
                "oversizedDivisor",
                "dynamicDivisor",
                "conditionWrite",
                "conditionClosure",
            ] {
                assert!(
                    function(&production.js, name).contains("[$modulo]("),
                    "checked refusal: {name}"
                );
            }
        }
        assert_eq!(node_output(&production.js), expected, "minify={minify}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn zero_divisor_keeps_sdk_dispatch_and_existing_divergence_visible() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/positive_counter_modulo_zero.dart");
    let oracle = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .expect("Dart");
    assert!(oracle.status.success());
    assert_eq!(
        String::from_utf8(oracle.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "zeroError=true\n"
    );
    let options = dartforge_emit_js_production::Options {
        minify: false,
        ..Default::default()
    };
    let production = dartforge_emit_js_production::compile_own_sdk(
        &source,
        None,
        None,
        options,
        &Default::default(),
    )
    .expect("production compilation");
    assert!(function(&production.js, "zeroDivisor").contains("i[$modulo](0)"));
    assert_eq!(
        node_output(&production.js),
        "NaN\n",
        "existing SDK Number/int-zero divergence must stay explicit, not counted as VM parity"
    );
}

#[test]
#[ignore = "requires SDK source and Node on PATH; private JS intrinsic unavailable on the VM"]
fn resolved_foreign_intrinsic_refuses_hidden_counter_mutation() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/positive_counter_modulo_foreign.dart");
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
        .expect("SDK intrinsic compilation");
        if !minify {
            assert!(function(&production.js, "rawMutation").contains("i[$modulo](3)"));
        }
        assert_eq!(
            node_output(&production.js),
            "[2, 1]\n",
            "the raw mutation reaches a negative counter and must keep Euclidean modulo"
        );
    }
}
