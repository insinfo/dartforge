//! Prefixed annotations must still reject non-constant constructor arguments.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires the Dart 3.6.2 SDK and oracle on PATH"]
fn prefixed_annotations_keep_constant_argument_validation() {
    let sdk = std::env::var_os("DARTFORGE_SDK_LIB").expect("DARTFORGE_SDK_LIB");
    let root = std::env::temp_dir().join(format!(
        "dartforge-annotation-constant-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    std::fs::write(
        fixture.0.join("pubspec.yaml"),
        "name: annotation_constant_fixture\nenvironment:\n  sdk: '>=3.6.2 <4.0.0'\n",
    )
    .unwrap();
    std::fs::write(
        fixture.0.join("annotation.dart"),
        "class Marker { const Marker(int value); }\n",
    )
    .unwrap();
    for (prefixed, constant) in [(true, false), (false, false), (true, true)] {
        let import = if prefixed {
            "import 'annotation.dart' as annotation;"
        } else {
            "import 'annotation.dart';"
        };
        let value = if constant {
            "const int runtimeValue = 1;"
        } else {
            "int runtimeValue = 1;"
        };
        let annotation = if prefixed {
            "@annotation.Marker(runtimeValue)"
        } else {
            "@Marker(runtimeValue)"
        };
        let source =
            format!("{import}\n{value}\n{annotation}\nvoid decorated() {{}}\nvoid main() {{}}\n");
        let file = fixture.0.join("main.dart");
        std::fs::write(&file, &source).unwrap();
        let oracle = Command::new("dart")
            .args(["analyze", "--format=machine"])
            .arg(&file)
            .output()
            .unwrap();
        assert_eq!(
            oracle.status.code(),
            Some(if constant { 0 } else { 3 }),
            "{}",
            String::from_utf8_lossy(&oracle.stdout)
        );
        if !constant {
            assert!(
                String::from_utf8_lossy(&oracle.stdout)
                    .contains("|CONST_WITH_NON_CONSTANT_ARGUMENT|")
            );
        }
        for all in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
            command
                .args(["analyze", "--format=json", "--sdk-path"])
                .arg(&sdk);
            if all {
                command.arg("--all");
            }
            let output = command.arg(&file).output().unwrap();
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                output.status.code(),
                oracle.status.code(),
                "prefixed={prefixed}, constant={constant}, all={all}: {report}"
            );
            let diagnostics = report["diagnostics"].as_array().unwrap();
            if constant {
                assert!(diagnostics.is_empty(), "{report}");
            } else {
                assert_eq!(diagnostics.len(), 1, "{report}");
                let diagnostic = &diagnostics[0];
                assert_eq!(diagnostic["code"], "const_with_non_constant_argument");
                assert_eq!(diagnostic["severity"], "ERROR");
                let start = source.rfind("runtimeValue").unwrap();
                assert_eq!(diagnostic["location"]["range"]["start"]["offset"], start);
                assert_eq!(
                    diagnostic["location"]["range"]["end"]["offset"],
                    start + "runtimeValue".len()
                );
            }
        }
    }
}
