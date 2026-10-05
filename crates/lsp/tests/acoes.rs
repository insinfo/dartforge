//! `textDocument/codeAction` pelas mensagens JSON-RPC: inserir `;`,
//! importar a biblioteca de um nome indefinido, as correções dos códigos
//! semânticos publicados (imediatos e tipados da versão vigente), a
//! assistência de anotação de tipo e as edições versionadas.

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
    let r = p.requisitar("textDocument/codeAction", params)["result"].clone();
    // As ações de fonte (sempre oferecidas) e as criações para nomes
    // indefinidos têm testes próprios (`tests/correcoes.rs`); aqui ficam fora.
    let fora = |k: &str| {
        k.starts_with("source")
            || k.starts_with("quickfix.ignore")
            || (k.starts_with("refactor.") && k != "refactor.add.typeAnnotation")
            || k == "quickfix.change.to"
            || ["method", "function", "class", "mixin", "getter", "field", "localVariable"]
                .iter()
                .any(|s| k == format!("quickfix.create.{s}"))
    };
    match r {
        Value::Array(l) => Value::Array(l.into_iter().filter(|a| !fora(a["kind"].as_str().unwrap_or(""))).collect()),
        outro => outro,
    }
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
        json!(true)
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
    // (As correções valem para os diagnósticos da linha, como no Dart: o
    // `r` não usado da mesma linha também aparece.)
    let r = acoes(&mut p, "lib/a.dart", (3, 11), (3, 11), json!({}));
    let importar = acao(&r, "Import library 'dart:math'");
    assert_eq!(titulos(&r).iter().filter(|t| t.starts_with("Import")).count(), 1);
    assert_eq!(importar["kind"], "quickfix.import.librarySdk");
    assert_eq!(
        aplicar(&importar["edit"], &uri, texto),
        "import 'dart:core';\nimport 'dart:math';\n\nvoid f() {\n  var r = Random();\n  Soma? s;\n  print(pi);\n  _privada();\n}\n"
    );
    // Tipo indefinido: biblioteca do projeto (parte não conta), relativa.
    // (O `s` não usado da mesma linha também tem a sua correção.)
    let r = acoes(&mut p, "lib/a.dart", (4, 3), (4, 3), json!({}));
    assert_eq!(titulos(&r).into_iter().filter(|t| t.starts_with("Import")).collect::<Vec<_>>(), vec!["Import library 'util/soma.dart'"]);
    let importar = acao(&r, "Import library 'util/soma.dart'");
    assert_eq!(importar["kind"], "quickfix.import.libraryProject1");
    assert!(
        aplicar(&importar["edit"], &uri, texto)
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

/// Linha e coluna UTF-16 do início da `n`-ésima ocorrência de `agulha`.
fn onde(texto: &str, agulha: &str, n: usize) -> (u32, u32) {
    let i = texto.match_indices(agulha).nth(n).unwrap_or_else(|| panic!("sem {agulha:?}")).0;
    let antes = &texto[..i];
    (
        antes.matches('\n').count() as u32,
        antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

/// Abre, espera a publicação tipada e pede as ações em `agulha`.
fn acoes_em(p: &mut Projeto, rel: &str, texto: &str, agulha: &str) -> Value {
    p.abrir(rel, texto);
    p.servidor.aguardar_diagnosticos(std::time::Duration::from_secs(120));
    let pos = onde(texto, agulha, 0);
    acoes(p, rel, pos, pos, json!({}))
}

/// A ação de título `titulo`.
fn acao<'a>(r: &'a Value, titulo: &str) -> &'a Value {
    r.as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == titulo)
        .unwrap_or_else(|| panic!("sem {titulo:?} em {:?}", titulos(r)))
}

#[test]
fn remove_local_nao_usado_e_as_atribuicoes() {
    let mut p = Projeto::novo("acoes-local");
    let texto = "void f() {\n  var x = 1;\n  var y = 2;\n  x = 3;\n  print(y);\n}\n";
    let r = acoes_em(&mut p, "lib/a.dart", texto, "x = 1");
    let a = acao(&r, "Remove unused local variable");
    assert_eq!(a["kind"], "quickfix.remove.unusedLocalVariable");
    assert_eq!(a["diagnostics"][0]["code"], "unused_local_variable");
    assert_eq!(
        aplicar(&a["edit"], &p.uri("lib/a.dart"), texto),
        "void f() {\n  var y = 2;\n  print(y);\n}\n"
    );
    // Numa lista, só a variável.
    let texto = "void f() {\n  var a = 1, b = 2;\n  var c = 3, d = 4;\n  print(a + d);\n}\n";
    let r = acoes_em(&mut p, "lib/b.dart", texto, "b = 2");
    assert_eq!(
        aplicar(&acao(&r, "Remove unused local variable")["edit"], &p.uri("lib/b.dart"), texto),
        "void f() {\n  var a = 1;\n  var c = 3, d = 4;\n  print(a + d);\n}\n"
    );
    let pos = onde(texto, "c = 3", 0);
    let r = acoes(&mut p, "lib/b.dart", pos, pos, json!({}));
    assert_eq!(
        aplicar(&acao(&r, "Remove unused local variable")["edit"], &p.uri("lib/b.dart"), texto),
        "void f() {\n  var a = 1, b = 2;\n  var d = 4;\n  print(a + d);\n}\n"
    );
    // Usado: nada a remover dele (o `b` da mesma linha, não usado, tem a
    // sua correção, como no Dart, que corrige os diagnósticos da linha).
    let pos = onde(texto, "a = 1", 0);
    let r = acoes(&mut p, "lib/b.dart", pos, pos, json!({}));
    assert!(r.as_array().unwrap().iter().filter(|a| a["title"] == "Remove unused local variable").all(|a| a["diagnostics"][0]["range"]["start"]["character"] != pos.1));
}

#[test]
fn remove_funcao_local_nao_usada() {
    let mut p = Projeto::novo("acoes-elemento");
    let texto = "void f() {\n  int g() => 1;\n  print(2);\n}\n";
    let r = acoes_em(&mut p, "lib/a.dart", texto, "g()");
    let a = acao(&r, "Remove unused element");
    assert_eq!(a["kind"], "quickfix.remove.unusedElement");
    assert_eq!(aplicar(&a["edit"], &p.uri("lib/a.dart"), texto), "void f() {\n  print(2);\n}\n");
}

#[test]
fn correcoes_dos_diagnosticos_tipados_publicados() {
    let mut p = Projeto::novo("acoes-tipados");
    let casos: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "lib/cast.dart",
            "void f(int x) {\n  print((x as int).isEven);\n}\n",
            "as int",
            "Remove unnecessary cast",
            "quickfix.remove.unnecessaryCast",
            "void f(int x) {\n  print(x.isEven);\n}\n",
        ),
        (
            "lib/excl.dart",
            "void f(int y) {\n  print(y!);\n}\n",
            "!",
            "Remove the '!'",
            "quickfix.remove.nonNullAssertion",
            "void f(int y) {\n  print(y);\n}\n",
        ),
        (
            "lib/nulo.dart",
            "void f(int x) {\n  print(x?.isEven);\n}\n",
            "?.",
            "Replace with '.'",
            "quickfix.replace.withNotNullAware",
            "void f(int x) {\n  print(x.isEven);\n}\n",
        ),
        (
            "lib/estatico.dart",
            "class C {\n  static int s = 0;\n}\nvoid f(C c) {\n  print(c.s);\n}\n",
            "s);",
            "Change access to static using 'C'",
            "quickfix.change.toStaticAccess",
            "class C {\n  static int s = 0;\n}\nvoid f(C c) {\n  print(C.s);\n}\n",
        ),
        (
            "lib/registro.dart",
            "(int,) f() => (1);\n",
            "(1)",
            "Add trailing comma",
            "quickfix.add.trailingComma",
            "(int,) f() => (1,);\n",
        ),
    ];
    for &(rel, texto, agulha, titulo, especie, esperado) in casos {
        let r = acoes_em(&mut p, rel, texto, agulha);
        let a = acao(&r, titulo);
        assert_eq!(a["kind"], especie, "{rel}");
        assert!(a["diagnostics"][0]["code"].is_string(), "{a}");
        assert_eq!(aplicar(&a["edit"], &p.uri(rel), texto), esperado, "{rel}");
    }
}

#[test]
fn diagnostico_tipado_de_versao_velha_nao_gera_correcao() {
    let mut p = Projeto::novo("acoes-versao");
    let texto = "void f(int x) {\n  print((x as int).isEven);\n}\n";
    let r = acoes_em(&mut p, "lib/a.dart", texto, "as int");
    assert!(titulos(&r).contains(&"Remove unnecessary cast".to_string()));
    // Nova versão, com a análise tipada suspensa: a publicação tipada da
    // versão 1 não vale para a 2, mesmo com o mesmo texto no lugar.
    p.servidor.pausar_analise_tipada(true);
    p.mudar("lib/a.dart", 2, &format!("{texto}// fim\n"));
    let pos = onde(texto, "as int", 0);
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert!(!titulos(&r).contains(&"Remove unnecessary cast".to_string()), "{r}");
    // Retomada, a publicação da versão 2 volta a valer.
    p.servidor.pausar_analise_tipada(false);
    p.servidor.aguardar_diagnosticos(std::time::Duration::from_secs(120));
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert!(titulos(&r).contains(&"Remove unnecessary cast".to_string()), "{r}");
    // Fora do intervalo do diagnóstico, nada.
    assert!(!titulos(&acoes(&mut p, "lib/a.dart", (0, 0), (0, 4), json!({}))).contains(&"Remove unnecessary cast".to_string()));
}

#[test]
fn edicao_versionada_quando_o_cliente_aceita() {
    let mut p = Projeto::novo("acoes-versionada");
    p.requisitar(
        "initialize",
        json!({"capabilities": {"workspace": {"workspaceEdit": {"documentChanges": true}}}}),
    );
    let texto = "void f() {\n  var x = 1;\n}\n";
    p.abrir("lib/a.dart", texto);
    p.mudar("lib/a.dart", 5, texto);
    let pos = onde(texto, "x = 1", 0);
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    let a = acao(&r, "Remove unused local variable");
    let mudancas = a["edit"]["documentChanges"].as_array().unwrap_or_else(|| panic!("{a}"));
    assert_eq!(mudancas.len(), 1);
    assert_eq!(mudancas[0]["textDocument"], json!({"uri": p.uri("lib/a.dart"), "version": 5}));
}

#[test]
fn assistencia_de_anotacao_de_tipo() {
    let mut p = Projeto::novo("acoes-tipo");
    let texto = "void f() {\n  var nome = 'x';\n  final n = 1;\n  var d;\n  var lista = <int>[n];\n  print([nome, n, d, lista]);\n}\n";
    let r = acoes_em(&mut p, "lib/a.dart", texto, "nome =");
    let a = acao(&r, "Add type annotation");
    assert_eq!(a["kind"], "refactor.add.typeAnnotation");
    assert_eq!(
        aplicar(&a["edit"], &p.uri("lib/a.dart"), texto),
        texto.replace("var nome", "String nome")
    );
    let pos = onde(texto, "final n", 0);
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(aplicar(&acao(&r, "Add type annotation")["edit"], &p.uri("lib/a.dart"), texto), texto.replace("final n", "final int n"));
    let pos = onde(texto, "lista =", 0);
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(aplicar(&acao(&r, "Add type annotation")["edit"], &p.uri("lib/a.dart"), texto), texto.replace("var lista", "List<int> lista"));
    // `dynamic` não se anota; no inicializador também não.
    let pos = onde(texto, "d;", 0);
    assert!(!titulos(&acoes(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Add type annotation".to_string()));
    let pos = onde(texto, "'x'", 0);
    assert!(!titulos(&acoes(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Add type annotation".to_string()));
    // `context.only` com `quickfix` exclui a assistência; com `refactor`, só ela.
    let pos = onde(texto, "nome =", 0);
    assert!(titulos(&acoes(&mut p, "lib/a.dart", pos, pos, json!({"only": ["quickfix"]}))).iter().all(|t| t != "Add type annotation"));
    assert_eq!(titulos(&acoes(&mut p, "lib/a.dart", pos, pos, json!({"only": ["refactor"]}))), vec!["Add type annotation"]);
}

/// L06: as correções do Dart 3.6.2 para mais códigos publicados, com os
/// títulos e as edições do `analysis_server`.
#[test]
fn correcoes_de_campo_final_abstrato_condicao_e_elemento() {
    let mut p = Projeto::novo("acoes-l06");
    let casos: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "lib/final_tipado.dart",
            "class C {\n  final int y = 2;\n  void m() {\n    y = 4;\n  }\n}\n",
            "y = 4",
            "Make field 'y' not final",
            "quickfix.makeFieldNotFinal",
            "class C {\n  int y = 2;\n  void m() {\n    y = 4;\n  }\n}\n",
        ),
        (
            "lib/final_sem_tipo.dart",
            "class C {\n  final z = 3;\n  void m() {\n    this.z = 5;\n  }\n}\n",
            "z = 5",
            "Make field 'z' not final",
            "quickfix.makeFieldNotFinal",
            "class C {\n  var z = 3;\n  void m() {\n    this.z = 5;\n  }\n}\n",
        ),
        (
            "lib/abstrato1.dart",
            "abstract class A {\n  abstract int x = 1;\n}\n",
            "x = 1",
            "Remove initializer",
            "quickfix.remove.initializer",
            "abstract class A {\n  abstract int x;\n}\n",
        ),
        (
            "lib/abstrato2.dart",
            "abstract class A {\n  abstract int x = 1;\n}\n",
            "x = 1",
            "Remove the 'abstract' keyword",
            "quickfix.remove.abstract",
            "abstract class A {\n  int x = 1;\n}\n",
        ),
        (
            "lib/condicao.dart",
            "void f(int? n) {\n  if (n) {}\n}\n",
            "n) {}",
            "Add != null",
            "quickfix.add.neNull",
            "void f(int? n) {\n  if (n != null) {}\n}\n",
        ),
        (
            "lib/topo.dart",
            "int _g() => 1;\nvoid main() {}\n",
            "_g",
            "Remove unused element",
            "quickfix.remove.unusedElement",
            "void main() {}\n",
        ),
        (
            "lib/membro.dart",
            "class B {\n  /// Doc.\n  void _p() {}\n  void q() {}\n}\n",
            "_p",
            "Remove unused element",
            "quickfix.remove.unusedElement",
            "class B {\n  void q() {}\n}\n",
        ),
    ];
    for &(rel, texto, agulha, titulo, especie, esperado) in casos {
        let r = acoes_em(&mut p, rel, texto, agulha);
        let a = acao(&r, titulo);
        assert_eq!(a["kind"], especie, "{rel}");
        assert!(a["diagnostics"][0]["code"].is_string(), "{a}");
        assert_eq!(aplicar(&a["edit"], &p.uri(rel), texto), esperado, "{rel}");
    }
    // Campo `final` que é escrito sem ser campo (local): nada de campo.
    let texto = "void f() {\n  final int w = 1;\n  print(w);\n}\n";
    let r = acoes_em(&mut p, "lib/local.dart", texto, "w = 1");
    assert!(!titulos(&r).iter().any(|t| t.starts_with("Make field")), "{r}");
}

/// L06: `Create file` para a URI relativa que não existe, só ao cliente que
/// aceita a operação `create`; a parte nasce com o `part of`.
#[test]
fn criar_arquivo_da_uri_ausente() {
    let capacidades = json!({"workspace": {"workspaceEdit": {"documentChanges": true, "resourceOperations": ["create"]}}});
    let mut p = Projeto::com_capacidades("acoes-criar", capacidades);
    let texto = "import 'novo.dart';\npart 'parte.dart';\n";
    let r = acoes_em(&mut p, "lib/a.dart", texto, "'novo.dart'");
    let a = acao(&r, "Create file 'novo.dart'");
    assert_eq!(a["kind"], "quickfix.create.file");
    let mudancas = a["edit"]["documentChanges"].as_array().expect("documentChanges");
    assert_eq!(mudancas[0]["kind"], "create");
    assert_eq!(mudancas[0]["uri"], p.uri("lib/novo.dart"));
    assert_eq!(mudancas.len(), 1, "{mudancas:?}");
    let pos = common_pos(texto, "'parte.dart'");
    let r = acoes(&mut p, "lib/a.dart", pos, pos, json!({}));
    let a = acao(&r, "Create file 'parte.dart'");
    let mudancas = a["edit"]["documentChanges"].as_array().expect("documentChanges");
    assert_eq!(mudancas[0]["uri"], p.uri("lib/parte.dart"));
    assert_eq!(mudancas[1]["edits"][0]["newText"], "part of 'a.dart';\n");

    // Sem a operação `create` no cliente, a correção não é oferecida.
    let mut p = Projeto::novo("acoes-criar-sem");
    let r = acoes_em(&mut p, "lib/a.dart", texto, "'novo.dart'");
    assert!(!titulos(&r).iter().any(|t| t.starts_with("Create file")), "{r}");
}

fn common_pos(texto: &str, agulha: &str) -> (u32, u32) {
    onde(texto, agulha, 0)
}
