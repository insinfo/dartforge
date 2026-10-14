//! Exact physical typed-array brands preserve mixed-width numeric reads.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn node_output(js: &str) -> String {
    let mut node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
    node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{js}").as_bytes()).unwrap();
    let output = node.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n")
}

#[path = "support/typed_guards.rs"]
mod typed_guards;
use typed_guards::{guard_checks, helper_expression};

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn mixed_width_reads_preserve_precision_signedness_and_checked_fallbacks() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/typed_loop_brands.dart");
    let oracle = Command::new("dart").arg("run").arg(&source).output().expect("Dart");
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let expected = String::from_utf8(oracle.stdout).unwrap().replace("\r\n", "\n");
    let directory = std::env::temp_dir().join(format!("dartforge-typed-brands-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&directory).unwrap();
    let official = directory.join("official-O2.js");
    let compiled = Command::new("dart").args(["compile", "js", "-O2"]).arg(&source).arg("-o").arg(&official).output().expect("dart2js");
    assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    assert_eq!(node_output(&std::fs::read_to_string(&official).unwrap()), expected);
    for minify in [false, true] {
        let options = dartforge_emit_js_production::Options { minify, ..Default::default() };
        let production = dartforge_emit_js_production::compile_own_sdk(&source, None, None, options, &Default::default()).expect("production compilation");
        let mut checks = String::new();
        if !minify {
            assert_eq!(production.js.matches(" <= 4294967296").count(), 7);
            assert_eq!(production.js.matches(" = t$typedReadGuard").count(), 8);
            let helper = helper_expression(&production.js);
            for name in ["mutate", "offset"] {
                let start = production.js.find(&format!("function {name}(")).expect("live refusal");
                let body = &production.js[start..];
                let body = &body[..body.find("\n};").expect("function end")];
                assert!(!body.contains(" = t$typedReadGuard"), "effectful/offset loop versioned: {name}");
            }
            for guard in production.js.lines().filter_map(|line| line.trim().strip_prefix("if (").and_then(|s| s.strip_suffix(") {"))).filter(|s| s.starts_with("(t$") && s.contains(" = t$typedReadGuard")) {
                checks.push_str(&guard_checks(guard, helper));
            }
        }
        assert_eq!(node_output(&format!("{}\n{checks}", production.js)), expected, "minify={minify}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}
