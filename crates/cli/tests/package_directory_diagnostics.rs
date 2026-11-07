//! Package aliases must preserve diagnostics and UTF-16 locations in the CLI.
#![cfg(any(unix, windows))]

use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and oracle on PATH"]
fn package_directory_aliases_keep_constant_diagnostics() {
    let sdk = std::env::var_os("DARTFORGE_SDK_LIB").expect("DARTFORGE_SDK_LIB");
    let directory = Directory(std::env::temp_dir().join(format!(
        "dartforge package diagnostics {}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    let root = directory.0.join("real");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let root = dartforge_elements::config::canonicalize(&root).unwrap();
    let alias = directory.0.join("alias");
    link_directory(&root, &alias);
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: package_directory_diagnostics\nenvironment:\n  sdk: '>=3.6.2 <4.0.0'\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::write(
        root.join("lib/annotation.dart"),
        "class Marker { const Marker(int value); }\r\n",
    )
    .unwrap();
    let source = "// 🧭 annotation API\r\nimport 'annotation.dart' as annotation;\r\nimport 'value.dart';\r\n@annotation.Marker(runtimeValue)\r\nvoid decorated() {}\r\nvoid main() {}\r\n";
    std::fs::write(root.join("lib/main.dart"), source).unwrap();
    let configurations = [
        url::Url::from_directory_path(&alias).unwrap().to_string(),
        url::Url::from_directory_path(&root).unwrap().to_string(),
        "../".to_string(),
    ];
    for constant in [false, true] {
        std::fs::write(
            root.join("lib/value.dart"),
            if constant {
                "const int runtimeValue = 1;\r\n"
            } else {
                "int runtimeValue = 1;\r\n"
            },
        )
        .unwrap();
        for root_uri in &configurations {
            std::fs::write(
                root.join(".dart_tool/package_config.json"),
                serde_json::to_vec(&json!({
                    "configVersion": 2,
                    "packages": [{
                        "name": "package_directory_diagnostics",
                        "rootUri": root_uri,
                        "packageUri": "lib/",
                        "languageVersion": "3.6"
                    }]
                }))
                .unwrap(),
            )
            .unwrap();
            let oracle = Command::new("dart")
                .args(["analyze", "--format=machine"])
                .arg(root.join("lib/main.dart"))
                .output()
                .unwrap();
            assert_eq!(
                oracle.status.code(),
                Some(if constant { 0 } else { 3 }),
                "{}{}",
                String::from_utf8_lossy(&oracle.stdout),
                String::from_utf8_lossy(&oracle.stderr)
            );
            for target in [root.join("lib/main.dart"), alias.join("lib/main.dart")] {
                for all in [false, true] {
                    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
                    command
                        .args(["analyze", "--format=json", "--sdk-path"])
                        .arg(&sdk);
                    if all {
                        command.arg("--all");
                    }
                    let output = command.arg(&target).output().unwrap();
                    assert!(
                        output.stderr.is_empty(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(
                        output.status.code(),
                        oracle.status.code(),
                        "rootUri={root_uri}, constant={constant}, target={}, all={all}: {report}",
                        target.display()
                    );
                    let diagnostics = report["diagnostics"].as_array().unwrap();
                    if constant {
                        assert!(diagnostics.is_empty(), "{report}");
                    } else {
                        assert_eq!(diagnostics.len(), 1, "{report}");
                        let diagnostic = &diagnostics[0];
                        assert_eq!(diagnostic["code"], "const_with_non_constant_argument");
                        assert_eq!(diagnostic["severity"], "ERROR");
                        let byte = source.rfind("runtimeValue").unwrap();
                        let offset = source[..byte].encode_utf16().count();
                        assert_eq!(diagnostic["location"]["range"]["start"]["offset"], offset);
                        assert_eq!(
                            diagnostic["location"]["range"]["end"]["offset"],
                            offset + "runtimeValue".len()
                        );
                    }
                }
            }
        }
    }
}
