//! Declaration presence must follow each loaded SDK, including repeated compilation.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let prefix = format!(
            "dartforge-environment-presence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        loop {
            let path = std::env::temp_dir()
                .join(format!("{prefix}-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not reserve {}: {error}", path.display()),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn sdk() -> PathBuf {
    PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"))
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node"]
fn declaration_presence_and_conditional_imports_match_dart2js() {
    let scratch = Scratch::new();
    let sdk = sdk();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/environment_presence.dart");
    let reference = scratch.0.join("reference.js");
    let dart = sdk.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    let output = Command::new(dart)
        .args(["compile", "js", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(&reference)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = successful(Command::new("node").arg(reference).output().unwrap());
    assert!(
        String::from_utf8_lossy(&expected).ends_with("default import\n")
            || String::from_utf8_lossy(&expected).ends_with("default import\r\n")
    );
    let compiled = dartforge_emit_js_production::compile_own_sdk(
        &source,
        Some(&sdk.join("lib")),
        None,
        dartforge_emit_js_production::Options {
            minify: false,
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    let artifact = scratch.0.join("ours.js");
    std::fs::write(&artifact, compiled.js).unwrap();
    assert_eq!(
        successful(Command::new("node").arg(artifact).output().unwrap()),
        expected
    );
}

/// Keep real SDK files at their logical locations while varying one layout flag.
fn absolute_sdk_paths(value: &mut serde_json::Value, sdk_lib: &Path) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if key == "uri" || key == "patches" {
                    match value {
                        serde_json::Value::String(text) => {
                            *text = sdk_lib.join(&*text).to_string_lossy().into_owned()
                        }
                        serde_json::Value::Array(items) => {
                            for item in items {
                                let text = item.as_str().unwrap();
                                *item = serde_json::Value::String(
                                    sdk_lib.join(text).to_string_lossy().into_owned(),
                                );
                            }
                        }
                        _ => panic!("SDK path"),
                    }
                } else {
                    absolute_sdk_paths(value, sdk_lib);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                absolute_sdk_paths(item, sdk_lib);
            }
        }
        _ => {}
    }
}

#[test]
#[ignore = "requires Dart 3.6.2 and the generated DDC runtime plus Node"]
fn successive_compilations_keep_their_own_sdk_declarations() {
    let scratch = Scratch::new();
    let sdk_lib = sdk().join("lib");
    let mut libraries: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sdk_lib.join("libraries.json")).unwrap()).unwrap();
    absolute_sdk_paths(&mut libraries, &sdk_lib);
    let layout = scratch.0.join("sdk-lib");
    std::fs::create_dir(&layout).unwrap();
    let source = scratch.0.join("main.dart");
    std::fs::write(
        &source,
        r#"
void main() {
  print(const bool.hasEnvironment('dart.library.html'));
  print(const String.fromEnvironment('dart.library.html', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.html', defaultValue: true));
  print(const bool.fromEnvironment('dart.library.notreal', defaultValue: true));
}
"#,
    )
    .unwrap();
    for (round, supported) in [false, true, false].into_iter().enumerate() {
        libraries["dartdevc"]["libraries"]["html"]["supported"] =
            serde_json::Value::Bool(supported);
        std::fs::write(
            layout.join("libraries.json"),
            serde_json::to_vec(&libraries).unwrap(),
        )
        .unwrap();
        let emitted = dartforge_emit_js::compile(&source, Some(&layout), None).unwrap();
        let output_dir = scratch.0.join(format!("output-{round}"));
        dartforge_emit_js::write(
            &emitted,
            &output_dir,
            &dartforge_emit_js::default_dart_sdk_js(),
        )
        .unwrap();
        let actual = successful(
            Command::new("node")
                .arg(output_dir.join("main.mjs"))
                .output()
                .unwrap(),
        );
        let text = String::from_utf8(actual).unwrap().replace("\r\n", "\n");
        assert_eq!(
            text,
            if supported {
                "true\ntrue\ntrue\ntrue\n"
            } else {
                "false\nmissing\ntrue\ntrue\n"
            }
        );
    }
}
