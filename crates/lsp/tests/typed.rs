//! Typed diagnostics in the continuous flow (audit L01): the `dartforge
//! analyze` analysis runs in the background over the open documents and
//! publishes, for the current version, the codes in `verificados.txt`.
mod common;

use common::Project;
use serde_json::{Value, json};
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);

/// Codes of a publication.
fn codes(publication: &Value) -> Vec<String> {
    publication["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("no diagnostics: {publication}"))
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect()
}

/// The last publication for `uri` in `outputs`.
fn last<'a>(outputs: &'a [Value], uri: &str) -> Option<&'a Value> {
    outputs
        .iter()
        .rev()
        .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
}

/// Opens (writing to disk) and returns everything that went out until the
/// typed analysis goes idle: the immediate publication and the typed ones,
/// in order.
fn open_and_wait(p: &mut Project, relative: &str, text: &str) -> Vec<Value> {
    p.write(relative, text);
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":p.uri(relative),"languageId":"dart","version":1,"text":text}
        }}),
    );
    let mut outputs = p.server.pump();
    outputs.extend(p.server.await_diagnostics(LIMIT));
    outputs
}

/// Replaces the text (new version, without saving) and returns everything
/// that went out until the typed analysis goes idle.
fn change_and_wait(p: &mut Project, relative: &str, version: i64, text: &str) -> Vec<Value> {
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":p.uri(relative),"version":version},
            "contentChanges":[{"text":text}]
        }}),
    );
    let mut outputs = p.server.pump();
    outputs.extend(p.server.await_diagnostics(LIMIT));
    outputs
}

#[test]
fn published_typed_code_reaches_editor() {
    let mut p = Project::new("tipado-basico");
    let text = "void f() {\n  if (1) {}\n}\n";
    // While paused, the typed analysis does not run alongside the immediate flow.
    p.server.pause_typed_analysis(true);
    let immediate = p.open("lib/a.dart", text);
    // The immediate flow is syntactic: the non-bool condition depends on types.
    assert!(codes(&immediate).is_empty(), "{immediate}");
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    let uri = p.uri("lib/a.dart");
    let typed =
        last(&outputs, &uri).unwrap_or_else(|| panic!("no typed publication: {outputs:?}"));
    assert_eq!(typed["params"]["version"], 1);
    assert_eq!(codes(typed), vec!["non_bool_condition"], "{typed}");
    let d = &typed["params"]["diagnostics"][0];
    assert_eq!(d["range"]["start"], json!({"line": 1, "character": 6}));
    assert_eq!(d["range"]["end"], json!({"line": 1, "character": 7}));
    assert_eq!(d["severity"], 1);
    assert_eq!(d["source"], "dartforge");
}

#[test]
fn only_published_codes_and_ignore_respected() {
    let mut p = Project::new("tipado-publicados");
    // `unchecked_use_of_nullable_value` is not in `verificados.txt`: it does not
    // appear. `// ignore:` works as in `dartforge analyze` (for warnings; an
    // error cannot be ignored), in both flows.
    let text = "void f() {\n  <int?>[].first.isEven;\n  // ignore: unused_local_variable\n  var y = 1;\n  var z = 2;\n  if (1) {}\n}\n";
    p.server.pause_typed_analysis(true);
    let immediate = p.open("lib/a.dart", text);
    assert_eq!(
        codes(&immediate),
        vec!["unused_local_variable"],
        "{immediate}"
    );
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    let typed = last(&outputs, &p.uri("lib/a.dart"))
        .expect("typed publication")
        .clone();
    // The order of `dart language-server` 3.6.2: errors before warnings.
    assert_eq!(
        codes(&typed),
        vec!["non_bool_condition", "unused_local_variable"],
        "{typed}"
    );
    assert_eq!(
        typed["params"]["diagnostics"][0]["range"]["start"]["line"],
        5
    );
    assert_eq!(
        typed["params"]["diagnostics"][1]["range"]["start"]["line"],
        4
    );
    let published = dartforge_analysis::publication::verified();
    for c in codes(&typed) {
        assert!(published.contains(&c.as_str()), "{c} is not published");
    }
}

#[test]
fn stale_version_result_is_discarded() {
    let mut p = Project::new("tipado-versao");
    let uri = p.uri("lib/a.dart");
    // Paused during `didOpen`, the analysis does not finish within the same
    // `pump` (which would publish it).
    p.server.pause_typed_analysis(true);
    p.open("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.server.pause_typed_analysis(false);
    // The analysis of version 1 finishes, but the result has not been published yet.
    assert!(p.server.wait_idle_analysis(LIMIT));
    // Version 2 fixes the error before the server publishes the one from version 1.
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":2},
            "contentChanges":[{"text":"void f() {\n  if (true) {}\n}\n"}]
        }}),
    );
    let outputs = p.server.pump();
    assert!(
        outputs.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").all(|m| m["params"]["version"] == 2),
        "published a stale version: {outputs:?}"
    );
    assert_eq!(p.server.typed_discarded(), 1);
    // Version 2 has no errors, and those of version 1 never reached the client:
    // nothing to publish (the Dart server does not publish an empty list in
    // this case).
    let mut after = outputs;
    after.extend(p.server.await_diagnostics(LIMIT));
    assert!(last(&after, &uri).is_none(), "{after:?}");
}

#[test]
fn quick_edits_become_one_analysis() {
    let mut p = Project::new("tipado-rajada");
    let uri = p.uri("lib/a.dart");
    p.server.pause_typed_analysis(true);
    p.open("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    // The last version has the error: an empty list without previously
    // published errors would not be published (as in the Dart server).
    for v in 2..=10 {
        let cond = if v % 2 == 0 { "1" } else { "true" };
        p.change(
            "lib/a.dart",
            v,
            &format!("void f() {{\n  if ({cond}) {{}}\n}}\n"),
        );
    }
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    let typed: Vec<&Value> = outputs
        .iter()
        .filter(|m| m["params"]["uri"] == uri)
        .collect();
    // One analysis for the whole burst, of the last version.
    assert_eq!(typed.len(), 1, "{outputs:?}");
    assert_eq!(typed[0]["params"]["version"], 10);
    assert_eq!(codes(typed[0]), vec!["non_bool_condition"]);
}

#[test]
fn editing_imported_updates_importer() {
    let mut p = Project::new("tipado-dependente");
    let uri_a = p.uri("lib/a.dart");
    let uri_b = p.uri("lib/b.dart");
    open_and_wait(&mut p, "lib/b.dart", "int x = 0;\n");
    let outputs = open_and_wait(
        &mut p,
        "lib/a.dart",
        "import 'b.dart';\nvoid f() {\n  if (x) {}\n}\n",
    );
    let a = last(&outputs, &uri_a).expect("a.dart analyzed");
    assert_eq!(codes(a), vec!["non_bool_condition"], "{a}");
    // Only `b.dart` changes (without saving): `a.dart` is republished without the error.
    let outputs = change_and_wait(&mut p, "lib/b.dart", 2, "bool x = true;\n");
    let a =
        last(&outputs, &uri_a).unwrap_or_else(|| panic!("a.dart was not republished: {outputs:?}"));
    assert_eq!(a["params"]["version"], 1);
    assert!(codes(a).is_empty(), "{a}");
    // `b.dart` stays without errors: nothing to publish for it.
    assert!(last(&outputs, &uri_b).is_none());
    // And the error comes back when `b.dart` goes back to `int`.
    let outputs = change_and_wait(&mut p, "lib/b.dart", 3, "int x = 0;\n");
    let a = last(&outputs, &uri_a).expect("a.dart republished");
    assert_eq!(codes(a), vec!["non_bool_condition"], "{a}");
}

#[test]
fn close_discards_pending_result() {
    let mut p = Project::new("typed-close");
    let uri = p.uri("lib/a.dart");
    p.server.pause_typed_analysis(true);
    p.open("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.server.pause_typed_analysis(false);
    assert!(p.server.wait_idle_analysis(LIMIT));
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
            "textDocument":{"uri":uri}
        }}),
    );
    // The result with the error is discarded, and nothing had been published
    // before: closing does not publish.
    let outputs = p.server.pump();
    assert!(last(&outputs, &uri).is_none(), "{outputs:?}");
    assert_eq!(p.server.typed_discarded(), 1);
}

/// Codes of a `textDocument/diagnostic` report.
fn pulled_codes(report: &Value) -> Vec<String> {
    report["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items: {report}"))
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect()
}

/// L08: a client that announces `textDocument.diagnostic` gets the
/// `diagnosticProvider` and pulls; the server does not push
/// `publishDiagnostics`.
#[test]
fn pulled_diagnostics_instead_of_pushed() {
    // The capability is only announced to whoever asks for it.
    for (capabilities, announces) in [(json!({}), false), (json!({"textDocument": {"diagnostic": {}}}), true)] {
        let mut s = dartforge_lsp::Server::new();
        s.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":capabilities}}));
        let r = s.pump();
        let provider = &r[0]["result"]["capabilities"]["diagnosticProvider"];
        assert_eq!(provider.is_object(), announces, "{provider}");
        if announces {
            assert_eq!(provider["interFileDependencies"], true);
            assert_eq!(provider["workspaceDiagnostics"], false);
        }
    }

    let mut p = Project::with_capabilities(
        "tipado-puxado",
        json!({"textDocument": {"diagnostic": {}}, "workspace": {"diagnostics": {"refreshSupport": true}}}),
    );
    let uri = p.uri("lib/a.dart");
    let document_text = json!({"uri": uri});
    p.server.pause_typed_analysis(true);
    // Opening pushes nothing.
    p.write("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":"void f() {\n  if (1) {}\n}\n"}
    }}));
    assert!(last(&p.server.pump(), &uri).is_none());
    // The immediate (syntactic) flow on request; the same `resultId` comes back
    // `unchanged`.
    let immediate = p.request("textDocument/diagnostic", json!({"textDocument": document_text}));
    assert_eq!(immediate["result"]["kind"], "full", "{immediate}");
    assert_eq!(immediate["result"]["resultId"], "1");
    assert!(pulled_codes(&immediate).is_empty(), "{immediate}");
    let equal = p.request("textDocument/diagnostic", json!({"textDocument": document_text, "previousResultId": "1"}));
    assert_eq!(equal["result"], json!({"kind": "unchanged", "resultId": "1"}));
    // The typed result asks for a `refresh` instead of publishing.
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    assert!(outputs.iter().all(|m| m["method"] != "textDocument/publishDiagnostics"), "{outputs:?}");
    let refresh = outputs
        .iter()
        .find(|m| m["method"] == "workspace/diagnostic/refresh")
        .unwrap_or_else(|| panic!("no refresh: {outputs:?}"));
    assert!(refresh["id"].is_string(), "{refresh}");
    // The client's response to the `refresh` is accepted silently.
    p.server.receive(json!({"jsonrpc":"2.0","id":refresh["id"].clone(),"result":null}));
    assert!(p.server.pump().is_empty());
    // Pulled again, the typed result of the same version comes.
    let typed = p.request("textDocument/diagnostic", json!({"textDocument": document_text, "previousResultId": "1"}));
    assert_eq!(typed["result"]["kind"], "full", "{typed}");
    assert_eq!(typed["result"]["resultId"], "1.t");
    assert_eq!(pulled_codes(&typed), vec!["non_bool_condition"], "{typed}");
    assert_eq!(typed["result"]["items"][0]["range"]["start"], json!({"line": 1, "character": 6}));
    // Closing does not push either; once closed, the document has no diagnostics.
    p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument": document_text}}));
    assert!(p.server.pump().is_empty());
    let closed = p.request("textDocument/diagnostic", json!({"textDocument": document_text}));
    assert_eq!(closed["result"], json!({"kind": "full", "items": []}));
}
