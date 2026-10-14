//! Checked scalar arithmetic and the certified SDK modulo-result fast path.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn sdk_modulo_preserves_numbers_errors_user_identity_and_foreign_fallbacks() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/native_number_ranges.dart");
    let oracle = Command::new("dart").arg("run").arg(&source).output().expect("Dart");
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let expected = String::from_utf8(oracle.stdout).unwrap().replace("\r\n", "\n");
    for minify in [false, true] {
        let options = dartforge_emit_js_production::Options { minify, ..Default::default() };
        let production = dartforge_emit_js_production::compile_own_sdk(&source, None, None, options, &Default::default()).expect("production compilation");
        if !minify {
            assert_eq!(production.js.matches("typeof result === 'number' ? result > 0").count(), 1, "the canonical helper must actually be optimized");
            assert!(production.js.contains("if (other[$lessThan](0))"), "retain the unsafe divisor adapter's original dispatch");
        }
        let probes = r#"
const symbols = Object.getOwnPropertySymbols(Number.prototype);
const mod = symbols.find(s => s.description === 'dartx.%');
const lt = symbols.find(s => s.description === 'dartx.<');
if (!mod || !lt) throw Error('live Number helpers missing');
const helper = Number.prototype[mod];
let failed = false;
try { helper.call(7n, 3n); } catch (error) { failed = error instanceof TypeError; }
if (!failed) throw Error('foreign BigInt comparison must retain original failure');
const effects = [];
const other = {valueOf() {effects.push('coerce'); return -3;}, [lt](zero) {effects.push('lt'); if (zero !== 0) throw Error('argument order'); return true;}};
if (helper.call(-7, other) !== 2 || effects.join(',') !== 'coerce,lt,coerce') throw Error('unsafe divisor dispatch/order changed');
"#;
        let mut node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
        node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{}\n{}", production.js, probes).as_bytes()).unwrap();
        let output = node.wait_with_output().unwrap();
        assert!(output.status.success(), "minify={minify}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n"), expected, "minify={minify}");
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires Dart 3.6.2 SDK library path and Node on PATH"]
fn changed_sdk_modulo_body_keeps_original_dispatch() {
    use std::fs;
    use std::path::{PathBuf, Path};
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_SDK_LIB").expect("SDK lib path"));
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let overlay = std::env::temp_dir().join(format!("dartforge-number-sdk-{}-{unique}", std::process::id()));
    let number = Path::new("_internal/js_dev_runtime/private/js_number.dart");
    let library = Path::new("_internal/js_dev_runtime/private/interceptors.dart");
    fn copy_overlay(from: &Path, to: &Path, relative: &Path, number: &Path, library: &Path) {
        if relative == number || relative == library {
            let text = std::fs::read_to_string(from).unwrap();
            let text = if relative == number {
                let patched = text.replacen("if (result > 0) return result;", "if (result > 0) return result + 1;", 1);
                assert_ne!(patched, text, "expected SDK 3.6.2 modulo body");
                patched
            } else { text };
            std::fs::write(to, text).unwrap();
        } else if number.starts_with(relative) || library.starts_with(relative) {
            std::fs::create_dir(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                copy_overlay(&entry.path(), &to.join(entry.file_name()), &relative.join(entry.file_name()), number, library);
            }
        } else {
            std::os::unix::fs::symlink(from, to).unwrap();
        }
    }
    copy_overlay(&sdk, &overlay, Path::new(""), number, library);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/native_number_ranges.dart");
    let options = dartforge_emit_js_production::Options { minify: false, ..Default::default() };
    let compiled = dartforge_emit_js_production::compile_own_sdk(&source, Some(&overlay), None, options, &Default::default());
    let counter_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/positive_counter_modulo.dart");
    let counter = dartforge_emit_js_production::compile_own_sdk(&counter_source, Some(&overlay), None, dartforge_emit_js_production::Options { minify: false, ..Default::default() }, &Default::default());
    fs::remove_dir_all(&overlay).unwrap();
    let production = compiled.expect("modified SDK compilation");
    let counter = counter.expect("modified SDK counter compilation");
    assert!(!counter.js.contains("i % 3"), "altered SDK helper must disable the counter fast path");
    let mut counter_node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
    counter_node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{}", counter.js).as_bytes()).unwrap();
    let counter_output = counter_node.wait_with_output().unwrap();
    assert!(counter_output.status.success(), "{}", String::from_utf8_lossy(&counter_output.stderr));
    assert!(String::from_utf8(counter_output.stdout).unwrap().contains("canonical=[0, 2, 3, 0, 2, 3, 0, 2]"));

    assert!(!production.js.contains("typeof result === 'number' ? result > 0"), "changed body must fail certification closed");
    let mut node = Command::new("node").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Node");
    node.stdin.take().unwrap().write_all(format!("globalThis.self=globalThis;\n{}", production.js).as_bytes()).unwrap();
    let output = node.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8(output.stdout).unwrap().contains("int=100:2:2"), "the modified SDK implementation, not the certified implementation, must execute");
}
