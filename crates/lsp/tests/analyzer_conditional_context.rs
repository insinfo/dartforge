//! Analyzer conditional imports keep absent variables distinct from false.
mod common;

use common::Project;
use serde_json::{Value, json};
use std::time::Duration;

fn wait(project: &mut Project) -> Vec<Value> {
    let mut output = project.server.pump();
    output.extend(project.server.await_diagnostics(Duration::from_secs(120)));
    output
}
fn publication<'a>(output: &'a [Value], uri: &str) -> &'a Value {
    output.iter().rev().find(|value| value["method"] == "textDocument/publishDiagnostics" && value["params"]["uri"] == uri).unwrap_or_else(|| panic!("no publication for {uri}: {output:?}"))
}

#[test]
fn missing_false_conditional_import_clears_verified_false_positive_in_editor() {
    let mut project = Project::new("analyzer-conditional-context");
    project.write("lib/primary.dart", "class PublicApi { _Private get value => _Private(); } class _Private { bool get flag => true; }\n");
    project.write("lib/alternative.dart", "class PublicApi { _Private get value => _Private(); } class _Private { int get flag => 1; }\n");
    let uri = project.uri("lib/consumer.dart");
    project.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"dart","version":1,"text":"void consume() { if (1) {} }\n"}}}));
    let initial = wait(&mut project);
    let diagnostics = publication(&initial, &uri)["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "non_bool_condition");
    let corrected = "import 'primary.dart' if (dart.library.core == 'false') 'alternative.dart';\nvoid consume() { if (PublicApi().value.flag) {} }\n";
    project.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":corrected}]}}));
    let cleared = wait(&mut project);
    let publication = publication(&cleared, &uri);
    assert_eq!(publication["params"]["version"], 2);
    assert!(publication["params"]["diagnostics"].as_array().unwrap().is_empty(), "{publication}");
}
