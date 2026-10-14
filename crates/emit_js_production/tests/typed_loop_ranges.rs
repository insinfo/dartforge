//! Checked typed-loop fast paths and refused effectful/mismatched cases.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
#[path = "support/typed_guards.rs"]
mod typed_guards;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn typed_loops_preserve_ranges_precision_order_and_mutations() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/typed_loop_ranges.dart");
    let oracle = Command::new("dart").arg("run").arg(&source).output().expect("Dart");
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let expected = String::from_utf8(oracle.stdout).unwrap().replace("\r\n", "\n");
    for minify in [false, true] {
        let options = dartforge_emit_js_production::Options { minify, ..Default::default() };
        let production = dartforge_emit_js_production::compile_own_sdk(&source, None, None, options, &Default::default()).expect("production compilation");
        if !minify {
            // Only precision/product/shadowBuiltins/bytes qualify. Offset indexes, receiver or
            // index mutation, callback calls and custom List.length must stay
            // checked. Five F64 receivers and one byte receiver are guarded.
            assert_eq!(production.js.matches(" <= 4294967296").count(), 4);
            assert_eq!(production.js.matches(" = t$typedReadGuard").count(), 6);
        }
        let mut guard_checks = String::new();
        if !minify {
            let helper = typed_guards::helper_expression(&production.js);
            for guard in production.js.lines().filter_map(|line| line.trim().strip_prefix("if (").and_then(|s| s.strip_suffix(") {"))).filter(|s| s.starts_with("(t$") && s.contains(" = t$typedReadGuard")) {
                guard_checks.push_str(&typed_guards::guard_checks(guard, helper));
            }
        }
        let mut node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
        node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{}\n{}", production.js, guard_checks).as_bytes()).unwrap();
        let output = node.wait_with_output().unwrap();
        assert!(output.status.success(), "minify={minify}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n"), expected, "minify={minify}");
    }
}
