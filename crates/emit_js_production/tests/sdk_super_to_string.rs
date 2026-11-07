//! SDK patch bodies also need lexical Object.toString dispatch.
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node"]
fn js_object_fallback_matches_official_production_js() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/sdk_super_to_string.dart");
    let directory = std::env::temp_dir().join(format!(
        "dartforge-sdk-super-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let compiler = std::env::var_os("DARTFORGE_JSPROD")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge-jsprod").into());
    for level in ["-O1", "-O2"] {
        let reference = directory.join("official.js");
        let result = Command::new("dart")
            .args(["compile", "js", level])
            .arg(&source)
            .arg("-o")
            .arg(&reference)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{level}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let artifact = directory.join("ours.js");
        let mut command = Command::new(&compiler);
        command.arg(&source).arg("-o").arg(&artifact).arg(level);
        if let Some(sdk) = std::env::var_os("DARTFORGE_SDK_LIB") {
            command.arg("--sdk").arg(sdk);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{level}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        for path in [&reference, &artifact] {
            let result = Command::new("node")
                .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
                .arg(path)
                .output()
                .unwrap();
            assert!(
                result.status.success() && result.stderr.is_empty(),
                "{level} {}: {}",
                path.display(),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&result.stdout).replace("\r\n", "\n"),
                "JS Object fallback ok\n"
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK"]
fn unreachable_super_selections_keep_dynamic_object_forwarding() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-dead-super-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("main.dart");
    std::fs::write(
        &source,
        r#"
class Dead { String toString() => super.toString(); }
class Alive { String toString() => 'alive'; }
void main() { print(Alive()); }
"#,
    )
    .unwrap();
    let compiled = dartforge_emit_js_production::compile_own_sdk(
        &source,
        None,
        None,
        dartforge_emit_js_production::Options {
            minify: false,
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    assert!(
        compiled
            .js
            .contains("prototype[dartx.toString] = function() { return this.toString(); }")
    );
    std::fs::remove_dir_all(directory).unwrap();
}
