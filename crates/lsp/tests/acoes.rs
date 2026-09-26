//! `textDocument/codeAction` pelas mensagens JSON-RPC: inserir `;` e
//! importar a biblioteca de um nome indefinido.

mod comum;

use comum::{Projeto, aplicar};
use serde_json::{Value, json};

fn acoes(p: &mut Projeto, relativo: &str, de: (u32, u32), ate: (u32, u32), extra: Value) -> Value {
    let mut params = json!({
        "textDocument": {"uri": p.uri(relativo)},
        "range": {"start": {"line": de.0, "character": de.1}, "end": {"line": ate.0, "character": ate.1}},
        "context": {"diagnostics": []},
    });
    if let Value::Object(mais) = extra {
        for (k, v) in mais {
            params["context"][k] = v;
        }
    }
    p.requisitar("textDocument/codeAction", params)["result"].clone()
}

fn titulos(r: &Value) -> Vec<String> {
    r.as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn anuncia_a_capacidade() {
    let mut p = Projeto::novo("acoes-cap");
    let r = p.requisitar("initialize", json!({"capabilities": {}}));
    assert_eq!(
        r["result"]["capabilities"]["codeActionProvider"],
        json!({"codeActionKinds": ["quickfix"]})
    );
}

#[test]
fn inserir_ponto_e_virgula_no_diagnostico_publicado() {
    let mut p = Projeto::novo("acoes-pv");
    let texto = "void f() {\n  var x = 1\n  print(x);\n}\n";
    let publicado = p.abrir("lib/a.dart", texto);
    let diagnostico = publicado["params"]["diagnostics"][0].clone();
    assert_eq!(diagnostico["code"], "expected_token");
    let r = acoes(&mut p, "lib/a.dart", (1, 10), (1, 10), json!({}));
    assert_eq!(titulos(&r), vec!["Insert ';'"]);
    assert_eq!(r[0]["kind"], "quickfix.insertSemicolon");
    // O diagnóstico da ação é o mesmo que foi publicado.
    assert_eq!(r[0]["diagnostics"][0], diagnostico);
    assert_eq!(
        aplicar(&r[0]["edit"], &p.uri("lib/a.dart"), texto),
        "void f() {\n  var x = 1;\n  print(x);\n}\n"
    );
    // Fora do diagnóstico, nada.
    assert_eq!(
        acoes(&mut p, "lib/a.dart", (0, 0), (0, 3), json!({})),
        json!([])
    );
    // `only` que não inclui quickfix filtra tudo.
    assert_eq!(
        acoes(
            &mut p,
            "lib/a.dart",
            (1, 10),
            (1, 10),
            json!({"only": ["refactor"]})
        ),
        json!([])
    );
    assert_eq!(
        titulos(&acoes(
            &mut p,
            "lib/a.dart",
            (1, 10),
            (1, 10),
            json!({"only": ["quickfix"]})
        )),
        vec!["Insert ';'"]
    );
}

#[test]
fn importar_biblioteca_do_sdk_e_do_projeto() {
    let mut p = Projeto::novo("acoes-importar");
    p.gravar(
        "lib/util/soma.dart",
        "class Soma {}\nint _privada() => 0;\n",
    );
    p.gravar("lib/parte.dart", "part of 'outro.dart';\nclass Soma {}\n");
    let texto = "import 'dart:core';\n\nvoid f() {\n  var r = Random();\n  Soma? s;\n  print(pi);\n  _privada();\n}\n";
    p.abrir("lib/a.dart", texto);
    let uri = p.uri("lib/a.dart");
    // Chamada a um nome indefinido: SDK (sem bibliotecas `dart:_`).
    let r = acoes(&mut p, "lib/a.dart", (3, 11), (3, 11), json!({}));
    assert_eq!(titulos(&r), vec!["Import library 'dart:math'"]);
    assert_eq!(r[0]["kind"], "quickfix.import.librarySdk");
    assert_eq!(
        aplicar(&r[0]["edit"], &uri, texto),
        "import 'dart:core';\nimport 'dart:math';\n\nvoid f() {\n  var r = Random();\n  Soma? s;\n  print(pi);\n  _privada();\n}\n"
    );
    // Tipo indefinido: biblioteca do projeto (parte não conta), relativa.
    let r = acoes(&mut p, "lib/a.dart", (4, 3), (4, 3), json!({}));
    assert_eq!(titulos(&r), vec!["Import library 'util/soma.dart'"]);
    assert_eq!(r[0]["kind"], "quickfix.import.libraryProject1");
    assert!(
        aplicar(&r[0]["edit"], &uri, texto)
            .starts_with("import 'dart:core';\nimport 'util/soma.dart';\n")
    );
    // Variável de topo do SDK declarada na biblioteca, não numa parte.
    assert_eq!(
        titulos(&acoes(&mut p, "lib/a.dart", (5, 9), (5, 9), json!({}))),
        vec!["Import library 'dart:math'"]
    );
    // Nome privado nunca é importável; nome definido não pede import.
    assert_eq!(
        acoes(&mut p, "lib/a.dart", (6, 4), (6, 4), json!({})),
        json!([])
    );
    assert_eq!(
        acoes(&mut p, "lib/a.dart", (5, 4), (5, 4), json!({})),
        json!([])
    );
}

#[test]
fn importar_de_fora_de_lib_usa_package_e_sem_diretivas_insere_no_topo() {
    let mut p = Projeto::novo("acoes-pacote");
    p.gravar("lib/soma.dart", "int somar(int a, int b) => a + b;\n");
    let texto = "void main() {\n  print(somar(1, 2));\n}\n";
    p.abrir("bin/main.dart", texto);
    let r = acoes(&mut p, "bin/main.dart", (1, 9), (1, 9), json!({}));
    assert_eq!(
        titulos(&r),
        vec!["Import library 'package:projeto/soma.dart'"]
    );
    assert_eq!(
        aplicar(&r[0]["edit"], &p.uri("bin/main.dart"), texto),
        "import 'package:projeto/soma.dart';\n\nvoid main() {\n  print(somar(1, 2));\n}\n"
    );
    // Depois de importada, a ação some.
    let com_import =
        "import 'package:projeto/soma.dart';\n\nvoid main() {\n  print(somar(1, 2));\n}\n";
    p.mudar("bin/main.dart", 2, com_import);
    assert_eq!(
        acoes(&mut p, "bin/main.dart", (3, 9), (3, 9), json!({})),
        json!([])
    );
}

#[test]
fn codigo_incompleto_ainda_importa() {
    let mut p = Projeto::novo("acoes-incompleto");
    // O comando com `Random` está bem formado; outro, quebrado, não o impede.
    let texto = "void f() {\n  var r = Random();\n  r.\n}\n";
    p.abrir("lib/a.dart", texto);
    let r = acoes(&mut p, "lib/a.dart", (1, 11), (1, 11), json!({}));
    assert_eq!(titulos(&r), vec!["Import library 'dart:math'"]);
}
