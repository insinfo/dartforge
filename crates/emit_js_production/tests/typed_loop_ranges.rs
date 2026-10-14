//! Checked typed-loop fast paths and refused effectful/mismatched cases.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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
            assert_eq!(production.js.matches(".length <= 4294967296").count(), 4);
            assert_eq!(production.js.matches(".prototype && !Object.prototype.hasOwnProperty.call(").count(), 6);
        }
        let mut guard_checks = String::new();
        if !minify {
            for guard in production.js.lines().filter_map(|line| line.trim().strip_prefix("if (").and_then(|s| s.strip_suffix(") {"))).filter(|s| s.starts_with("globalThis.ArrayBuffer.isView(")) {
                let brand = if guard.contains("Float64Array") { "Float64Array" } else { "Uint8Array" };
                guard_checks.push_str(&format!(r#"
{{
const check = new Function('a', 'b', {});
const Brand = globalThis.{brand};
if (!check(new Brand(2), new Brand(2))) throw Error('fixed fast path refused');
if (check(new DataView(new ArrayBuffer(8)), new Brand(2))) throw Error('DataView accepted');
if (check(new (class extends Brand {{}})(2), new Brand(2))) throw Error('subclass accepted');
const own = new Brand(2);
Object.defineProperty(own, 'length', {{get() {{throw Error('own length must not be read');}}}});
if (check(own, new Brand(2))) throw Error('own length accepted');
if (typeof SharedArrayBuffer !== 'undefined' && check(new Brand(new SharedArrayBuffer(16)), new Brand(2))) throw Error('shared accepted');
if (Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, 'resizable') && check(new Brand(new ArrayBuffer(16, {{maxByteLength: 32}})), new Brand(2))) throw Error('resizable accepted');
}}
"#, dartforge_emit_js::js::string_literal(&format!("return ({guard});"))));
            }
        }
        let mut node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
        node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{}\n{}", production.js, guard_checks).as_bytes()).unwrap();
        let output = node.wait_with_output().unwrap();
        assert!(output.status.success(), "minify={minify}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n"), expected, "minify={minify}");
    }
}
