//! Directory aliases must preserve typed diagnostics across unsaved dependency edits.
#![cfg(any(unix, windows))]
mod common;
use common::Project;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);
fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
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
fn assert_main_diagnostics(outputs: &[Value], uri: &str, constant: bool) {
    let publication = outputs
        .iter()
        .rev()
        .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri);
    if constant {
        let publication = publication.expect("the previous error must be cleared");
        assert_eq!(
            publication["params"]["diagnostics"],
            json!([]),
            "{publication}"
        );
    } else {
        let publication = publication.unwrap_or_else(|| panic!("no main diagnostics: {outputs:?}"));
        let diagnostics = publication["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1, "{publication}");
        assert_eq!(diagnostics[0]["code"], "const_with_non_constant_argument");
        assert_eq!(diagnostics[0]["severity"], 1);
        assert_eq!(
            diagnostics[0]["range"],
            json!({"start":{"line":3,"character":19},"end":{"line":3,"character":31}})
        );
        assert_eq!(publication["params"]["version"], 1);
    }
}
#[test]
fn package_directory_aliases_preserve_unsaved_constant_edits() {
    let mut project = Project::new("package-directory-diagnostics");
    let root = dartforge_elements::config::without_verbatim(project.path(""));
    let alias = dartforge_elements::config::without_verbatim(project.root.join("linked-project"));
    link_directory(&root, &alias);
    project.write(".dart_tool/package_config.json", &serde_json::to_string(&json!({
        "configVersion":2,
        "packages":[{"name":"projeto","rootUri":url::Url::from_directory_path(&alias).unwrap().to_string(),"packageUri":"lib/","languageVersion":"3.6"}]
    })).unwrap());
    let value = "int runtimeValue = 1;\r\n";
    let main = "// 🧭 annotation API\r\nimport 'annotation.dart' as annotation;\r\nimport 'value.dart';\r\n@annotation.Marker(runtimeValue)\r\nvoid decorated() {}\r\nvoid main() {}\r\n";
    let mut outputs = vec![project.open(
        "lib/annotation.dart",
        "class Marker { const Marker(int value); }\r\n",
    )];
    outputs.push(project.open("lib/value.dart", value));
    outputs.push(project.open("lib/main.dart", main));
    outputs.extend(project.server.await_diagnostics(LIMIT));
    let uri = project.uri("lib/main.dart");
    assert_main_diagnostics(&outputs, &uri, false);
    for (version, constant) in [(2, true), (3, false)] {
        let text = if constant {
            "const int runtimeValue = 1;\r\n"
        } else {
            value
        };
        project.server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":project.uri("lib/value.dart"),"version":version},
                "contentChanges":[{"text":text}]
            }}),
        );
        let mut outputs = project.server.pump();
        outputs.extend(project.server.await_diagnostics(LIMIT));
        assert_main_diagnostics(&outputs, &uri, constant);
        assert_eq!(
            std::fs::read_to_string(project.path("lib/value.dart")).unwrap(),
            value
        );
    }
}
