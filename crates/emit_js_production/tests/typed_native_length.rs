//! Checked production semantics for typed views and inherited native-interface aliases.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn inherited_mixin_aliases_preserve_views_order_and_generic_tearoffs() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/typed_native_length.dart");
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
        let mut node = Command::new("node")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node");
        let script = format!("globalThis.self=globalThis;\n{}", production.js);
        node.stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let output = node.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "minify={minify}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            expected,
            "minify={minify}"
        );
    }
}
