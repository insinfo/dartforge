//! Explicit analyzer SDK selection overrides discovery without changing publication.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/analyze-sdk-path-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&root).unwrap();
        for (name, member) in [
            ("selected", "int get selectedMember => 1;"),
            ("environment", ""),
        ] {
            let lib = root.join(name).join("lib");
            std::fs::create_dir_all(lib.join("core")).unwrap();
            std::fs::write(
                lib.join("libraries.json"),
                r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
            )
            .unwrap();
            std::fs::write(lib.join("core/core.dart"), format!(
                "class Object {{}} class bool {{}} class num {{}} class int extends num {{}} class Null {{}} class String {{{member}}}" )).unwrap();
        }
        std::fs::write(
            root.join("main.dart"),
            "int readSelectedSdk(String value) => value.selectedMember;\n",
        )
        .unwrap();
        Self { root }
    }

    fn analyze(&self, options: &[String]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dartforge"))
            .args(["analyze", "--format=json", "--all"])
            .args(options)
            .arg(self.root.join("main.dart"))
            .env("DARTFORGE_SDK_LIB", self.root.join("environment/lib"))
            .env("TMPDIR", &self.root)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn diagnostics(output: &Output) -> serde_json::Value {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["diagnostics"].clone()
}

#[test]
fn explicit_root_and_lib_override_environment() {
    let fixture = Fixture::new();
    let discovered = fixture.analyze(&[]);
    assert_eq!(discovered.status.code(), Some(3));
    assert_eq!(diagnostics(&discovered)[0]["code"], "undefined_getter");
    for path in [
        fixture.root.join("selected"),
        fixture.root.join("selected/lib"),
    ] {
        for options in [
            vec![format!("--sdk-path={}", path.display())],
            vec!["--sdk-path".into(), path.to_string_lossy().into_owned()],
        ] {
            let selected = fixture.analyze(&options);
            assert!(
                selected.status.success(),
                "{:?}: {}",
                options,
                String::from_utf8_lossy(&selected.stderr)
            );
            assert_eq!(diagnostics(&selected), serde_json::json!([]));
        }
    }
}

#[test]
fn invalid_explicit_sdk_does_not_fall_back() {
    let fixture = Fixture::new();
    let absent = fixture.root.join("missing");
    let result = fixture.analyze(&[format!("--sdk-path={}", absent.display())]);
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("--sdk-path") && stderr.contains("libraries.json"),
        "{stderr}"
    );
    assert!(result.stdout.is_empty());
    for options in [
        vec!["--sdk-path=".into()],
        vec!["--sdk-path".into(), "--all".into()],
    ] {
        let result = fixture.analyze(&options);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("--sdk-path requires an SDK directory")
        );
    }
    assert!(!Path::new(&absent).exists());
}
