//! Production keeps only caches used by live constant-record literals.

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
fn live_records_keep_canonical_identity_without_unused_library_caches() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/const_record_cache.dart");
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
        "dartforge-const-records-{}-{}",
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
    for minify in [false, false, true] {
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
            assert_eq!(
                production.js.matches(".$constCache = new Map();").count(),
                3
            );
            for library in [
                "L$const_record_cache",
                "L$const_record_cache__records",
                "L$const_record_cache__cycle_a",
            ] {
                assert!(
                    production
                        .js
                        .contains(&format!("{library}.$constCache = new Map();")),
                    "used cache omitted: {library}"
                );
            }
            for library in [
                "L$const_record_cache__plain",
                "L$const_record_cache__cycle_b",
            ] {
                assert!(
                    !production.js.contains(&format!("{library}.$constCache")),
                    "unused cache emitted: {library}"
                );
            }
        }
        assert_eq!(node_output(&production.js), expected, "minify={minify}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires SDK source on PATH"]
fn development_warm_fragments_keep_the_existing_cache_prelude() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/const_record_cache.dart");
    dartforge_emit_js::compile_with_options(
        &source,
        None,
        None,
        &Default::default(),
        None,
        false,
        |analysis| {
            let cache =
                std::cell::RefCell::new(dartforge_emit_js::module::FragmentCache::default());
            let emit = || {
                dartforge_emit_js::emit_program_with_cache(
                    analysis.program,
                    analysis.interner,
                    analysis.table,
                    analysis.core,
                    analysis.outline,
                    analysis.bodies,
                    Some(&cache),
                )
                .map_err(|d| format!("{d:?}"))
            };
            let cold = emit()?;
            assert!(!cache.borrow().is_empty());
            let warm = emit()?;
            assert_eq!(cold.modules, warm.modules);
            assert_eq!(cold.entry, warm.entry);
            let text = cold
                .modules
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                text.matches(".$constCache = new Map();").count(),
                5,
                "development preserves caches even in record-free libraries"
            );
            Ok(())
        },
    )
    .expect("development cold/warm compilation");
}
