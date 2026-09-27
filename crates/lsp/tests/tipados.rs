//! Diagnósticos tipados no fluxo contínuo (auditoria L01): a análise do
//! `dartforge analyze` roda em segundo plano sobre os documentos abertos e
//! publica, pela versão vigente, os códigos de `verificados.txt`.
mod comum;

use comum::Projeto;
use serde_json::{Value, json};
use std::time::Duration;

const LIMITE: Duration = Duration::from_secs(120);

/// Códigos de uma publicação.
fn codigos(publicacao: &Value) -> Vec<String> {
    publicacao["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("sem diagnósticos: {publicacao}"))
        .iter()
        .map(|d| d["code"].as_str().unwrap_or("").to_string())
        .collect()
}

/// A última publicação para `uri` em `saidas`.
fn ultima<'a>(saidas: &'a [Value], uri: &str) -> Option<&'a Value> {
    saidas
        .iter()
        .rev()
        .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
}

/// Abre (gravando no disco) e devolve tudo o que saiu até a análise tipada
/// ficar ociosa: a publicação imediata e as tipadas, em ordem.
fn abrir_e_esperar(p: &mut Projeto, relativo: &str, texto: &str) -> Vec<Value> {
    p.gravar(relativo, texto);
    p.servidor.receber(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":p.uri(relativo),"languageId":"dart","version":1,"text":texto}
        }}),
    );
    let mut saidas = p.servidor.bombear();
    saidas.extend(p.servidor.aguardar_diagnosticos(LIMITE));
    saidas
}

/// Troca o texto (versão nova, sem salvar) e devolve tudo o que saiu até a
/// análise tipada ficar ociosa.
fn mudar_e_esperar(p: &mut Projeto, relativo: &str, versao: i64, texto: &str) -> Vec<Value> {
    p.servidor.receber(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":p.uri(relativo),"version":versao},
            "contentChanges":[{"text":texto}]
        }}),
    );
    let mut saidas = p.servidor.bombear();
    saidas.extend(p.servidor.aguardar_diagnosticos(LIMITE));
    saidas
}

#[test]
fn codigo_tipado_publicado_chega_ao_editor() {
    let mut p = Projeto::novo("tipado-basico");
    let texto = "void f() {\n  if (1) {}\n}\n";
    // Pausada, a análise tipada não corre com o fluxo imediato.
    p.servidor.pausar_analise_tipada(true);
    let imediata = p.abrir("lib/a.dart", texto);
    // O fluxo imediato é sintático: a condição não-bool depende de tipos.
    assert!(codigos(&imediata).is_empty(), "{imediata}");
    p.servidor.pausar_analise_tipada(false);
    let saidas = p.servidor.aguardar_diagnosticos(LIMITE);
    let uri = p.uri("lib/a.dart");
    let tipada =
        ultima(&saidas, &uri).unwrap_or_else(|| panic!("sem publicação tipada: {saidas:?}"));
    assert_eq!(tipada["params"]["version"], 1);
    assert_eq!(codigos(tipada), vec!["non_bool_condition"], "{tipada}");
    let d = &tipada["params"]["diagnostics"][0];
    assert_eq!(d["range"]["start"], json!({"line": 1, "character": 6}));
    assert_eq!(d["range"]["end"], json!({"line": 1, "character": 7}));
    assert_eq!(d["severity"], 1);
    assert_eq!(d["source"], "dartforge");
}

#[test]
fn so_codigos_publicados_e_ignore_respeitado() {
    let mut p = Projeto::novo("tipado-publicados");
    // `undefined_identifier` não está em `verificados.txt`: não aparece.
    // O `// ignore:` vale como no `dartforge analyze` (para avisos; erro
    // não é ignorável), nos dois fluxos.
    let texto = "void f() {\n  nada;\n  // ignore: unused_local_variable\n  var y = 1;\n  var z = 2;\n  if (1) {}\n}\n";
    p.servidor.pausar_analise_tipada(true);
    let imediata = p.abrir("lib/a.dart", texto);
    assert_eq!(
        codigos(&imediata),
        vec!["unused_local_variable"],
        "{imediata}"
    );
    p.servidor.pausar_analise_tipada(false);
    let saidas = p.servidor.aguardar_diagnosticos(LIMITE);
    let tipada = ultima(&saidas, &p.uri("lib/a.dart"))
        .expect("publicação tipada")
        .clone();
    assert_eq!(
        codigos(&tipada),
        vec!["unused_local_variable", "non_bool_condition"],
        "{tipada}"
    );
    assert_eq!(
        tipada["params"]["diagnostics"][0]["range"]["start"]["line"],
        4
    );
    assert_eq!(
        tipada["params"]["diagnostics"][1]["range"]["start"]["line"],
        5
    );
    let publicados = dartforge_analise::publicacao::verificados();
    for c in codigos(&tipada) {
        assert!(publicados.contains(&c.as_str()), "{c} não é publicado");
    }
}

#[test]
fn resultado_de_versao_velha_e_descartado() {
    let mut p = Projeto::novo("tipado-versao");
    let uri = p.uri("lib/a.dart");
    // Pausada durante o `didOpen`, a análise não termina dentro do mesmo
    // `bombear` (que a publicaria).
    p.servidor.pausar_analise_tipada(true);
    p.abrir("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.servidor.pausar_analise_tipada(false);
    // A análise da versão 1 termina, mas o resultado ainda não foi publicado.
    assert!(p.servidor.esperar_analise_ociosa(LIMITE));
    // A versão 2 corrige o erro antes de o servidor publicar o da 1.
    p.servidor.receber(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":2},
            "contentChanges":[{"text":"void f() {\n  if (true) {}\n}\n"}]
        }}),
    );
    let saidas = p.servidor.bombear();
    assert!(
        saidas.iter().all(|m| m["params"]["version"] == 2),
        "publicou versão velha: {saidas:?}"
    );
    assert_eq!(p.servidor.tipados_descartados(), 1);
    let mut depois = saidas;
    depois.extend(p.servidor.aguardar_diagnosticos(LIMITE));
    let tipada = ultima(&depois, &uri).expect("publicação da versão 2");
    assert_eq!(tipada["params"]["version"], 2);
    assert!(codigos(tipada).is_empty(), "{tipada}");
}

#[test]
fn edicoes_rapidas_viram_uma_analise() {
    let mut p = Projeto::novo("tipado-rajada");
    let uri = p.uri("lib/a.dart");
    p.servidor.pausar_analise_tipada(true);
    p.abrir("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    for v in 2..=10 {
        let cond = if v % 2 == 0 { "true" } else { "1" };
        p.mudar(
            "lib/a.dart",
            v,
            &format!("void f() {{\n  if ({cond}) {{}}\n}}\n"),
        );
    }
    p.servidor.pausar_analise_tipada(false);
    let saidas = p.servidor.aguardar_diagnosticos(LIMITE);
    let tipadas: Vec<&Value> = saidas
        .iter()
        .filter(|m| m["params"]["uri"] == uri)
        .collect();
    // Uma análise para a rajada inteira, da última versão.
    assert_eq!(tipadas.len(), 1, "{saidas:?}");
    assert_eq!(tipadas[0]["params"]["version"], 10);
    assert!(codigos(tipadas[0]).is_empty());
}

#[test]
fn editar_importado_atualiza_quem_importa() {
    let mut p = Projeto::novo("tipado-dependente");
    let uri_a = p.uri("lib/a.dart");
    let uri_b = p.uri("lib/b.dart");
    abrir_e_esperar(&mut p, "lib/b.dart", "int x = 0;\n");
    let saidas = abrir_e_esperar(
        &mut p,
        "lib/a.dart",
        "import 'b.dart';\nvoid f() {\n  if (x) {}\n}\n",
    );
    let a = ultima(&saidas, &uri_a).expect("a.dart analisado");
    assert_eq!(codigos(a), vec!["non_bool_condition"], "{a}");
    // Só `b.dart` muda (sem salvar): `a.dart` é republicado sem o erro.
    let saidas = mudar_e_esperar(&mut p, "lib/b.dart", 2, "bool x = true;\n");
    let a =
        ultima(&saidas, &uri_a).unwrap_or_else(|| panic!("a.dart não foi republicado: {saidas:?}"));
    assert_eq!(a["params"]["version"], 1);
    assert!(codigos(a).is_empty(), "{a}");
    assert!(ultima(&saidas, &uri_b).is_some());
    // E volta o erro quando `b.dart` volta a `int`.
    let saidas = mudar_e_esperar(&mut p, "lib/b.dart", 3, "int x = 0;\n");
    let a = ultima(&saidas, &uri_a).expect("a.dart republicado");
    assert_eq!(codigos(a), vec!["non_bool_condition"], "{a}");
}

#[test]
fn fechar_descarta_resultado_pendente() {
    let mut p = Projeto::novo("tipado-fechar");
    let uri = p.uri("lib/a.dart");
    p.servidor.pausar_analise_tipada(true);
    p.abrir("lib/a.dart", "void f() {\n  if (1) {}\n}\n");
    p.servidor.pausar_analise_tipada(false);
    assert!(p.servidor.esperar_analise_ociosa(LIMITE));
    p.servidor.receber(
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
            "textDocument":{"uri":uri}
        }}),
    );
    let saidas = p.servidor.bombear();
    assert_eq!(saidas.len(), 1, "{saidas:?}");
    assert!(codigos(&saidas[0]).is_empty());
    assert_eq!(p.servidor.tipados_descartados(), 1);
}
