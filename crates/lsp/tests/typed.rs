//! Diagnósticos tipados no fluxo contínuo (auditoria L01): a análise do
//! `dartforge analyze` roda em segundo plano sobre os documentos abertos e
//! publica, pela versão vigente, os códigos de `verificados.txt`.
mod common;

use common::Project;
use serde_json::{Value, json};
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);

/// Códigos de uma publicação.
fn codes(publication: &Value) -> Vec<String> {
    publication["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("sem diagnósticos: {publication}"))
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect()
}

/// A última publicação para `uri` em `outputs`.
fn last<'a>(outputs: &'a [Value], uri: &str) -> Option<&'a Value> {
    outputs
        .iter()
        .rev()
        .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
}

/// Abre (gravando no disco) e devolve tudo o que saiu até a análise tipada
/// ficar ociosa: a publicação imediata e as tipadas, em ordem.
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

/// Troca o texto (versão nova, sem salvar) e devolve tudo o que saiu até a
/// análise tipada ficar ociosa.
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
    // Pausada, a análise tipada não corre com o fluxo imediato.
    p.server.pause_typed_analysis(true);
    let immediate = p.open("lib/a.dart", text);
    // O fluxo imediato é sintático: a condição não-bool depende de tipos.
    assert!(codes(&immediate).is_empty(), "{immediate}");
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    let uri = p.uri("lib/a.dart");
    let typed =
        last(&outputs, &uri).unwrap_or_else(|| panic!("sem publicação tipada: {outputs:?}"));
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
    // `unchecked_use_of_nullable_value` não está em `verificados.txt`: não
    // aparece. O `// ignore:` vale como no `dartforge analyze` (para avisos;
    // erro não é ignorável), nos dois fluxos.
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
        .expect("publicação tipada")
        .clone();
    // A ordem do `dart language-server` 3.6.2: os erros antes dos avisos.
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
    let published = dartforge_analysis::publicacao::verificados();
    for c in codes(&typed) {
        assert!(published.contains(&c.as_str()), "{c} não é publicado");
    }
}

#[test]
fn stale_version_result_is_discarded() {
    let mut p = Project::new("tipado-versao");
    let uri = p.uri("lib/a.dart");
    // Pausada durante o `didOpen`, a análise não termina dentro do mesmo
    // `pump` (que a publicaria).
    p.server.pause_typed_analysis(true);
    p.open("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.server.pause_typed_analysis(false);
    // A análise da versão 1 termina, mas o resultado ainda não foi publicado.
    assert!(p.server.wait_idle_analysis(LIMIT));
    // A versão 2 corrige o erro antes de o servidor publicar o da 1.
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":2},
            "contentChanges":[{"text":"void f() {\n  if (true) {}\n}\n"}]
        }}),
    );
    let outputs = p.server.pump();
    assert!(
        outputs.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").all(|m| m["params"]["version"] == 2),
        "publicou versão velha: {outputs:?}"
    );
    assert_eq!(p.server.typed_discarded(), 1);
    // A versão 2 não tem erros, e os da 1 nunca chegaram ao cliente: nada
    // a publicar (o servidor do Dart não publica lista vazia nesse caso).
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
    // A última versão tem o erro: uma lista vazia sem erros publicados
    // antes não seria publicada (como no servidor do Dart).
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
    // Uma análise para a rajada inteira, da última versão.
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
    let a = last(&outputs, &uri_a).expect("a.dart analisado");
    assert_eq!(codes(a), vec!["non_bool_condition"], "{a}");
    // Só `b.dart` muda (sem salvar): `a.dart` é republicado sem o erro.
    let outputs = change_and_wait(&mut p, "lib/b.dart", 2, "bool x = true;\n");
    let a =
        last(&outputs, &uri_a).unwrap_or_else(|| panic!("a.dart não foi republicado: {outputs:?}"));
    assert_eq!(a["params"]["version"], 1);
    assert!(codes(a).is_empty(), "{a}");
    // `b.dart` segue sem erros: nada a publicar para ele.
    assert!(last(&outputs, &uri_b).is_none());
    // E volta o erro quando `b.dart` volta a `int`.
    let outputs = change_and_wait(&mut p, "lib/b.dart", 3, "int x = 0;\n");
    let a = last(&outputs, &uri_a).expect("a.dart republicado");
    assert_eq!(codes(a), vec!["non_bool_condition"], "{a}");
}

#[test]
fn close_discards_pending_result() {
    let mut p = Project::new("tipado-fechar");
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
    // O resultado com o erro é descartado, e nada fora publicado antes:
    // o fechamento não publica.
    let outputs = p.server.pump();
    assert!(last(&outputs, &uri).is_none(), "{outputs:?}");
    assert_eq!(p.server.typed_discarded(), 1);
}

/// Códigos de um relatório de `textDocument/diagnostic`.
fn pulled_codes(report: &Value) -> Vec<String> {
    report["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("sem itens: {report}"))
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect()
}

/// L08: o cliente que anuncia `textDocument.diagnostic` recebe o
/// `diagnosticProvider` e puxa; o servidor não empurra `publishDiagnostics`.
#[test]
fn pulled_diagnostics_instead_of_pushed() {
    // A capacidade só é anunciada a quem a pede.
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
    // Abrir não empurra nada.
    p.write("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":"void f() {\n  if (1) {}\n}\n"}
    }}));
    assert!(last(&p.server.pump(), &uri).is_none());
    // O fluxo imediato (sintático) por pedido; o mesmo `resultId` volta
    // `unchanged`.
    let immediate = p.request("textDocument/diagnostic", json!({"textDocument": document_text}));
    assert_eq!(immediate["result"]["kind"], "full", "{immediate}");
    assert_eq!(immediate["result"]["resultId"], "1");
    assert!(pulled_codes(&immediate).is_empty(), "{immediate}");
    let equal = p.request("textDocument/diagnostic", json!({"textDocument": document_text, "previousResultId": "1"}));
    assert_eq!(equal["result"], json!({"kind": "unchanged", "resultId": "1"}));
    // O resultado tipado pede `refresh` em vez de publicar.
    p.server.pause_typed_analysis(false);
    let outputs = p.server.await_diagnostics(LIMIT);
    assert!(outputs.iter().all(|m| m["method"] != "textDocument/publishDiagnostics"), "{outputs:?}");
    let refresh = outputs
        .iter()
        .find(|m| m["method"] == "workspace/diagnostic/refresh")
        .unwrap_or_else(|| panic!("sem refresh: {outputs:?}"));
    assert!(refresh["id"].is_string(), "{refresh}");
    // A resposta do cliente ao `refresh` é aceita em silêncio.
    p.server.receive(json!({"jsonrpc":"2.0","id":refresh["id"].clone(),"result":null}));
    assert!(p.server.pump().is_empty());
    // Puxado de novo, vem o tipado da mesma versão.
    let typed = p.request("textDocument/diagnostic", json!({"textDocument": document_text, "previousResultId": "1"}));
    assert_eq!(typed["result"]["kind"], "full", "{typed}");
    assert_eq!(typed["result"]["resultId"], "1.t");
    assert_eq!(pulled_codes(&typed), vec!["non_bool_condition"], "{typed}");
    assert_eq!(typed["result"]["items"][0]["range"]["start"], json!({"line": 1, "character": 6}));
    // Fechar também não empurra; fechado, o documento não tem diagnóstico.
    p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument": document_text}}));
    assert!(p.server.pump().is_empty());
    let closed = p.request("textDocument/diagnostic", json!({"textDocument": document_text}));
    assert_eq!(closed["result"], json!({"kind": "full", "items": []}));
}
