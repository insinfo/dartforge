//! Actual SDK JSON UTF8 byte stores, replacement callbacks and refusal paths.
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn node(js: &str, suffix: &str) -> String {
    let mut process = Command::new("node")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!("globalThis.self=globalThis;globalThis.window=globalThis;\n{js}\n{suffix}")
                .as_bytes(),
        )
        .unwrap();
    let result = process.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
#[ignore = "requires Dart3.6.2 and Node; isolated experimental flags"]
fn json_utf8_store_differential() {
    for enabled in ["0", "1"] {
        for minify in ["0", "1"] {
            let result = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "json_utf8_store_worker",
                    "--nocapture",
                ])
                .env("DARTFORGE_JSON_UTF8_STORE_WORKER", minify)
                .env("DARTFORGE_JSPROD_JSON_UTF8_STORE", enabled)
                .env("DARTFORGE_JSPROD_FRESH_LIST_APPEND", "0")
                .env("DARTFORGE_JSPROD_FRESH_LIST_APPEND_CLOSED", "0")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "enabled={enabled} minify={minify}\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

#[test]
#[ignore = "invoked by isolated JSON UTF8 differential parent"]
fn json_utf8_store_worker() {
    let Ok(minify) = std::env::var("DARTFORGE_JSON_UTF8_STORE_WORKER") else {
        return;
    };
    let minify = minify == "1";
    let enabled = std::env::var("DARTFORGE_JSPROD_JSON_UTF8_STORE").unwrap() == "1";
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/json_utf8_store.dart");
    let oracle = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let expected = String::from_utf8(oracle.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    let compiled = compile(&source, None, minify);
    if !minify {
        assert_eq!(
            compiled.matches("/* certified JSON UTF8 store */").count(),
            usize::from(enabled),
            "actual SDK store must execute the experiment"
        );
    }
    assert_eq!(
        node(&compiled, ""),
        expected,
        "enabled={enabled} minify={minify}"
    );
    if enabled && !minify {
        let foreign = compile(
            &source.with_file_name("json_utf8_store_foreign.dart"),
            None,
            false,
        );
        assert!(!foreign.contains("/* certified JSON UTF8 store */"));
        assert_eq!(node(&foreign, ""), "123,34,120,34,58,49,125\n");
        #[cfg(unix)]
        modified_sdk(&source, &expected);
    }
}

fn compile(source: &Path, sdk: Option<&Path>, minify: bool) -> String {
    dartforge_emit_js_production::compile_own_sdk(
        source,
        sdk,
        None,
        dartforge_emit_js_production::Options {
            minify,
            ..Default::default()
        },
        &Default::default(),
    )
    .expect("JSON UTF8 compilation")
    .js
}

#[cfg(unix)]
fn modified_sdk(source: &Path, expected: &str) {
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_SDK_LIB").unwrap());
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/js-json-utf8-sdk");
    std::fs::create_dir_all(&directory).unwrap();
    for (label, changed) in [
        ("lifecycle", Path::new("convert/json.dart")),
        (
            "native",
            Path::new("_internal/js_dev_runtime/private/native_typed_data.dart"),
        ),
    ] {
        let root = directory.join(format!(
            "{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fn copy(from: &Path, to: &Path, relative: &Path, changed: &Path) {
            if relative == changed {
                let text = std::fs::read_to_string(from).unwrap();
                let changed_text = if relative == Path::new("convert/json.dart") {
                    text.replacen(
                        "buffer[index++] = byte;",
                        "buffer[index++] = byte; // changed lifecycle owner",
                        1,
                    )
                } else {
                    text.replacen("void operator []=(int index, int value) {","void operator []=(int index, int value) {\n    JS('void', r'globalThis.__jsonByteStores = (globalThis.__jsonByteStores || 0) + 1');",1)
                };
                assert_ne!(text, changed_text);
                std::fs::write(to, changed_text).unwrap();
            } else if relative == Path::new("convert/convert.dart") {
                std::fs::copy(from, to).unwrap();
            } else if changed.starts_with(relative)
                || Path::new("convert/convert.dart").starts_with(relative)
            {
                std::fs::create_dir(to).unwrap();
                for entry in std::fs::read_dir(from).unwrap() {
                    let entry = entry.unwrap();
                    copy(
                        &entry.path(),
                        &to.join(entry.file_name()),
                        &relative.join(entry.file_name()),
                        changed,
                    );
                }
            } else {
                std::os::unix::fs::symlink(from, to).unwrap();
            }
        }
        copy(&sdk, &root, Path::new(""), changed);
        let compiled = compile(source, Some(&root), false);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            !compiled.contains("/* certified JSON UTF8 store */"),
            "modified {label} must retain actual checked dispatch"
        );
        let suffix = if label == "native" {
            "if (!(globalThis.__jsonByteStores > 0)) throw Error('modified SDK setter did not execute');"
        } else {
            ""
        };
        assert_eq!(node(&compiled, suffix), expected, "modified {label}");
    }
}

// Diagnostic only: preserve public error evidence independently of the failing
// original VM/runtimeType comparison. The driver compares backend results.
#[test]
#[ignore = "invoked explicitly by the backend error-classification driver"]
fn json_utf8_error_semantics_diagnostic() {
    let Ok(directory) = std::env::var("DARTFORGE_JSON_UTF8_CLASSIFICATION_DIR") else {
        return;
    };
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/json_utf8_error_semantics.dart");
    let compiled = compile(&source, None, false);
    let enabled = std::env::var("DARTFORGE_JSPROD_JSON_UTF8_STORE").is_ok_and(|v| v == "1");
    assert_eq!(
        compiled.matches("/* certified JSON UTF8 store */").count(),
        usize::from(enabled),
        "actual diagnostic store must exercise the experiment"
    );
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(Path::new(&directory).join("program.js"), &compiled).unwrap();
    let output = node(&compiled, "");
    std::fs::write(Path::new(&directory).join("program.stdout"), &output).unwrap();
    print!("{output}");
}
