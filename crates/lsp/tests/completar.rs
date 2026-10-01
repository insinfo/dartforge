//! `textDocument/completion` pelas mensagens JSON-RPC, com código incompleto.

mod comum;

use comum::{Projeto, item, marcar, rotulos};
use serde_json::json;

/// Abre `texto` (com o marcador `▮`) e completa no marcador.
fn completar(projeto: &mut Projeto, relativo: &str, texto: &str) -> serde_json::Value {
    let (texto, linha, coluna) = marcar(texto);
    projeto.abrir(relativo, &texto);
    projeto.na_posicao(
        "textDocument/completion",
        relativo,
        linha,
        coluna,
        json!({}),
    )
}

const CLASSES: &str = "class A {
  int campo = 1;
  String get rotulo => '';
  void met(int x, {String? nome, int? idade}) {}
  static int contar() => 0;
  A();
  A.vazio();
}
class B extends A {
  bool marcado = false;
}
";

#[test]
fn anuncia_a_capacidade() {
    let mut p = Projeto::novo("completar-cap");
    let r = p.requisitar("initialize", json!({"capabilities": {}}));
    assert_eq!(
        r["result"]["capabilities"]["completionProvider"]["triggerCharacters"],
        json!(["."])
    );
}

#[test]
fn membros_pelo_tipo_do_receptor_com_ponto_solto() {
    let mut p = Projeto::novo("completar-membros");
    // `b.` sem nome nem `;`: o comando não analisa e o parser o descarta.
    let r = completar(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(B b) {{\n  b.▮\n}}\n"),
    );
    let r_rotulos = rotulos(&r);
    for esperado in [
        "marcado",
        "campo",
        "rotulo",
        "met(…)",
        "toString()",
        "hashCode",
    ] {
        assert!(
            r_rotulos.contains(&esperado.to_string()),
            "{esperado} em {r_rotulos:?}"
        );
    }
    // Membros estáticos e construtores não aparecem numa instância.
    assert!(
        !r_rotulos
            .iter()
            .any(|r| r.starts_with("contar") || r.starts_with("vazio"))
    );
    assert_eq!(
        item(&r, "met(…)")["detail"],
        "(int x, {String? nome, int? idade}) → void"
    );
    assert_eq!(item(&r, "met(…)")["kind"], 2);
    assert_eq!(item(&r, "campo")["detail"], "int");
    assert_eq!(item(&r, "campo")["kind"], 5);
    assert_eq!(item(&r, "rotulo")["kind"], 10);
    // O intervalo substituído é o prefixo (vazio) na posição do cursor.
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"],
        json!({
            "start": {"line": 12, "character": 4}, "end": {"line": 12, "character": 4}
        })
    );
    // A ordem é estável: sortText crescente segue a ordem da lista.
    let ordem: Vec<String> = r["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["sortText"].as_str().unwrap().to_string())
        .collect();
    let mut ordenada = ordem.clone();
    ordenada.sort();
    assert_eq!(ordem, ordenada);
}

#[test]
fn filtra_pelo_prefixo_digitado_sem_ponto_e_virgula() {
    let mut p = Projeto::novo("completar-prefixo");
    // `a.ca` seguido de outro comando: sem o fecho, os dois comandos somem.
    let r = completar(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.ca▮\n  print(1);\n}}\n"),
    );
    assert_eq!(rotulos(&r), vec!["campo"]);
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"]["start"],
        json!({"line": 12, "character": 4})
    );
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"]["end"],
        json!({"line": 12, "character": 6})
    );
    // Maiúsculas não importam no filtro.
    let r = completar(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.TOS▮\n}}\n"),
    );
    assert_eq!(rotulos(&r), vec!["toString()"]);
}

#[test]
fn tipos_genericos_sao_substituidos() {
    let mut p = Projeto::novo("completar-generico");
    let r = completar(
        &mut p,
        "lib/a.dart",
        "void f(List<String> xs) {\n  xs.▮\n}\n",
    );
    assert_eq!(item(&r, "first")["detail"], "String");
    assert_eq!(item(&r, "add(…)")["detail"], "(String value) → void");
    // O receptor é o resultado de outra expressão, dentro de argumentos.
    let r = completar(
        &mut p,
        "lib/b.dart",
        "void f(List<String> xs) {\n  print(xs.first.▮);\n}\n",
    );
    assert!(rotulos(&r).contains(&"toUpperCase()".to_string()));
    assert_eq!(
        item(&r, "substring(…)")["detail"],
        "(int start, [int? end]) → String"
    );
}

#[test]
fn escopo_locais_parametros_membros_topo_e_palavras() {
    let mut p = Projeto::novo("completar-escopo");
    p.gravar(
        "lib/util.dart",
        "int dobro(int x) => x * 2;\nint _escondido = 0;\n",
    );
    let texto = format!(
        "import 'util.dart';\nimport 'util.dart' as u;\n{CLASSES}int global = 0;\nclass C extends A {{\n  int proprio = 0;\n  void m(int parametro) {{\n    var local = '';\n    {{ var interno = 1; }}\n    ▮\n    var depois = 2;\n  }}\n}}\n"
    );
    let r = completar(&mut p, "lib/a.dart", &texto);
    let r_rotulos = rotulos(&r);
    for esperado in [
        "local",
        "parametro",
        "proprio",
        "campo",
        "met(…)",
        "global",
        "dobro(…)",
        "u",
        "C",
        "A",
        "return",
        "if",
        "this",
    ] {
        assert!(
            r_rotulos.contains(&esperado.to_string()),
            "{esperado} em {r_rotulos:?}"
        );
    }
    // Fora de escopo, declarado adiante, privado de outra biblioteca.
    for ausente in ["interno", "depois", "_escondido"] {
        assert!(
            !r_rotulos.contains(&ausente.to_string()),
            "{ausente} em {r_rotulos:?}"
        );
    }
    assert_eq!(item(&r, "local")["detail"], "String");
    assert_eq!(item(&r, "parametro")["detail"], "int");
    assert_eq!(item(&r, "dobro(…)")["kind"], 3);
    // A relevância do Dart no começo de um comando (`Block_statement`):
    // o local mais próximo, `return`, o campo, a função de topo e, por
    // último, a variável de topo.
    let pos = |rotulo: &str| r_rotulos.iter().position(|r| r == rotulo).unwrap();
    assert!(pos("local") < pos("return"));
    assert!(pos("return") < pos("proprio"));
    assert!(pos("proprio") < pos("dobro(…)"));
    assert!(pos("dobro(…)") < pos("global"));

    // Com prefixo, primeiro o que começa com ele (sem diferenciar
    // maiúsculas); depois o que só casa por aproximação (`Comparable`
    // contém `para`).
    let r = completar(
        &mut p,
        "lib/b.dart",
        &texto.replace("    ▮\n", "    var z = para▮\n"),
    );
    assert_eq!(rotulos(&r), vec!["parametro", "Comparable"]);
}

#[test]
fn prefixo_de_import_e_estaticos() {
    let mut p = Projeto::novo("completar-prefixo-import");
    p.gravar(
        "lib/util.dart",
        "int dobro(int x) => x * 2;\nclass Util {}\n",
    );
    let r = completar(
        &mut p,
        "lib/a.dart",
        "import 'util.dart' as u;\nvoid f() {\n  u.▮\n}\n",
    );
    assert_eq!(rotulos(&r), vec!["dobro(…)", "Util"]);
    let r = completar(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f() {{\n  A.▮\n}}\n"),
    );
    assert_eq!(rotulos(&r), vec!["contar()", "vazio()"]);
}

#[test]
fn argumentos_nomeados_ainda_nao_passados() {
    let mut p = Projeto::novo("completar-nomeados");
    let r = completar(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.met(1, idade: 2, ▮);\n}}\n"),
    );
    let r_rotulos = rotulos(&r);
    assert_eq!(r_rotulos[0], "nome: ");
    assert!(!r_rotulos.contains(&"idade: ".to_string()));
    assert_eq!(item(&r, "nome: ")["detail"], "String?");
    // Sem o fecho da chamada: o texto analisado recebe `);`.
    let r = completar(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.met(1, ▮\n}}\n"),
    );
    assert_eq!(&rotulos(&r)[..2], ["idade: ", "nome: "]);
}

#[test]
fn nada_em_comentarios_e_strings() {
    let mut p = Projeto::novo("completar-comentario");
    let r = completar(&mut p, "lib/a.dart", "void f(int x) {\n  // x.▮\n}\n");
    assert_eq!(r["result"]["items"], json!([]));
    let r = completar(
        &mut p,
        "lib/b.dart",
        "void f(int x) {\n  print('x.▮');\n}\n",
    );
    assert_eq!(r["result"]["items"], json!([]));
    // Interpolação é código.
    let r = completar(
        &mut p,
        "lib/c.dart",
        "void f(int x) {\n  print('${x.▮}');\n}\n",
    );
    assert!(rotulos(&r).contains(&"isEven".to_string()));
}

#[test]
fn topo_oferece_palavras_de_declaracao_e_tipos() {
    let mut p = Projeto::novo("completar-topo");
    let r = completar(&mut p, "lib/a.dart", &format!("{CLASSES}cl▮\n"));
    // O prefixo primeiro; `Comparable` casa por aproximação (`c…l`).
    assert_eq!(rotulos(&r), vec!["class", "Comparable"]);
    let r = completar(&mut p, "lib/b.dart", &format!("{CLASSES}▮\n"));
    let r_rotulos = rotulos(&r);
    assert!(r_rotulos.contains(&"A".to_string()) && r_rotulos.contains(&"import".to_string()));
}

#[test]
fn documento_editado_usa_o_texto_vigente() {
    let mut p = Projeto::novo("completar-vigente");
    p.abrir("lib/a.dart", "class A { int velho = 0; }\nvoid f(A a) {}\n");
    p.mudar(
        "lib/a.dart",
        2,
        "class A { int novo = 0; }\nvoid f(A a) {\n  a.\n}\n",
    );
    let r = p.na_posicao("textDocument/completion", "lib/a.dart", 2, 4, json!({}));
    let r_rotulos = rotulos(&r);
    assert!(r_rotulos.contains(&"novo".to_string()) && !r_rotulos.contains(&"velho".to_string()));
}

#[test]
fn arquivo_parte_usa_a_biblioteca_dona() {
    let mut p = Projeto::novo("completar-parte");
    p.gravar(
        "lib/a.dart",
        "part 'p.dart';\npart 'q.dart';\nclass A { int campo = 0; }\nint global = 0;\n",
    );
    let r = completar(
        &mut p,
        "lib/p.dart",
        "part of 'a.dart';\nvoid f(A a) {\n  a.▮\n}\n",
    );
    assert!(rotulos(&r).contains(&"campo".to_string()), "{r}");
    let r = completar(
        &mut p,
        "lib/q.dart",
        "part of 'a.dart';\nvoid g() {\n  glo▮\n}\n",
    );
    assert_eq!(rotulos(&r), vec!["global"]);
    // Parte que a dona não declara: entra sozinha, sem os nomes da dona.
    let r = completar(
        &mut p,
        "lib/r.dart",
        "part of 'a.dart';\nvoid h() {\n  glo▮\n}\n",
    );
    assert_eq!(r["result"]["items"], json!([]));
}

/// O item de rótulo `rotulo` cujo `detail` é `detalhe`.
fn item_com<'a>(r: &'a serde_json::Value, rotulo: &str, detalhe: &str) -> &'a serde_json::Value {
    r["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == rotulo && i["detail"] == detalhe)
        .unwrap_or_else(|| panic!("sem {rotulo} ({detalhe}) em {:?}", rotulos(r)))
}

#[test]
fn importacao_automatica_do_sdk_e_do_projeto() {
    let mut p = Projeto::novo("completar-autoimport");
    p.gravar(
        "lib/util.dart",
        "/// Soma dois.\nint somar(int a, int b) => a + b;\nclass Utilitario {}\n",
    );
    let r = completar(&mut p, "lib/a.dart", "void f() {\n  Rand▮\n}\n");
    let random = item_com(&r, "Random", "Auto import from 'dart:math'");
    assert_eq!(random["kind"], 7);
    assert_eq!(
        random["additionalTextEdits"],
        json!([{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "import 'dart:math';\n\n"}])
    );
    assert_eq!(random["textEdit"]["newText"], "Random");
    // A biblioteca privada do SDK (`dart:_interna`) não é oferecida.
    assert!(!r.to_string().contains("dart:_interna"));
    // Do projeto, com import relativo e depois dos imports existentes.
    let r = completar(&mut p, "lib/b.dart", "import 'dart:math';\nvoid f() {\n  som▮\n}\n");
    let somar = item_com(&r, "somar(…)", "Auto import from 'util.dart'");
    assert_eq!(
        somar["additionalTextEdits"][0]["newText"],
        "\nimport 'util.dart';"
    );
    assert_eq!(somar["additionalTextEdits"][0]["range"]["start"], json!({"line": 0, "character": 19}));
    // Já importado: o item é o do escopo, sem edição adicional.
    let r = completar(&mut p, "lib/c.dart", "import 'dart:math';\nvoid f() {\n  Rand▮\n}\n");
    let random = item(&r, "Random");
    assert!(random.get("additionalTextEdits").is_none(), "{random}");
    assert_eq!(rotulos(&r).iter().filter(|x| *x == "Random").count(), 1);
    // Numa parte, o import iria para outro arquivo: não oferece.
    p.gravar("lib/dona.dart", "part 'parte.dart';\n");
    let r = completar(&mut p, "lib/parte.dart", "part of 'dona.dart';\nvoid g() {\n  Rand▮\n}\n");
    assert!(!rotulos(&r).contains(&"Random".to_string()), "{r}");
    // Sem nada digitado, não há não importados.
    let r = completar(&mut p, "lib/d.dart", "void f() {\n  ▮\n}\n");
    assert!(!r.to_string().contains("Auto import"));
}

#[test]
fn resolve_traz_a_documentacao() {
    let mut p = Projeto::novo("completar-resolve");
    let r = p.requisitar("initialize", json!({"capabilities": {}}));
    assert_eq!(r["result"]["capabilities"]["completionProvider"]["resolveProvider"], true);
    p.gravar("lib/util.dart", "/// Soma dois.\nint somar(int a, int b) => a + b;\n");
    let r = completar(
        &mut p,
        "lib/a.dart",
        "class A {\n  /// O campo guardado.\n  int campo = 1;\n}\nvoid f(A a) {\n  a.ca▮\n  som▮\n}\n".replacen("  som▮\n", "", 1).as_str(),
    );
    let campo = item(&r, "campo").clone();
    let resolvido = p.requisitar("completionItem/resolve", campo);
    assert_eq!(resolvido["result"]["documentation"], "O campo guardado.");
    assert_eq!(resolvido["result"]["label"], "campo");
    // Item de biblioteca não importada: a documentação vem do arquivo dele.
    let r = completar(&mut p, "lib/b.dart", "void f() {\n  som▮\n}\n");
    let somar = item(&r, "somar(…)").clone();
    p.requisitar(
        "initialize",
        json!({"capabilities": {"textDocument": {"completion": {"completionItem": {"documentationFormat": ["markdown"]}}}}}),
    );
    let resolvido = p.requisitar("completionItem/resolve", somar);
    assert_eq!(resolvido["result"]["documentation"], json!({"kind": "markdown", "value": "Soma dois."}));
    // Sem `data`, o item volta como veio.
    let resolvido = p.requisitar("completionItem/resolve", json!({"label": "x"}));
    assert_eq!(resolvido["result"], json!({"label": "x"}));
}

#[test]
fn snippets_de_chamada_quando_o_cliente_aceita() {
    let mut p = Projeto::novo("completar-snippet");
    let classe = "class A {\n  void met(int x, {String? nome, int? idade}) {}\n  void req({required int n}) {}\n  void nada() {}\n  int campo = 0;\n}\n";
    // Sem suporte a snippets: só o nome.
    let r = completar(&mut p, "lib/a.dart", &format!("{classe}void f(A a) {{\n  a.me▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
    assert!(item(&r, "met(…)").get("insertTextFormat").is_none());
    p.requisitar(
        "initialize",
        json!({"capabilities": {"textDocument": {"completion": {"completionItem": {"snippetSupport": true}}}}}),
    );
    let r = completar(&mut p, "lib/b.dart", &format!("{classe}void f(A a) {{\n  a.▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met(${1:x})$0");
    assert_eq!(item(&r, "met(…)")["insertTextFormat"], 2);
    assert_eq!(item(&r, "req(…)")["textEdit"]["newText"], "req(n: ${1:n})$0");
    assert_eq!(item(&r, "nada()")["textEdit"]["newText"], "nada()$0");
    // Campo não é chamada.
    assert_eq!(item(&r, "campo")["textEdit"]["newText"], "campo");
    assert!(item(&r, "campo").get("insertTextFormat").is_none());
    // Parênteses já escritos: só o nome.
    let r = completar(&mut p, "lib/c.dart", &format!("{classe}void f(A a) {{\n  a.me▮(1);\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
    // `completeFunctionCalls: false` desliga.
    p.requisitar(
        "initialize",
        json!({"capabilities": {"textDocument": {"completion": {"completionItem": {"snippetSupport": true}}}},
               "initializationOptions": {"completeFunctionCalls": false}}),
    );
    let r = completar(&mut p, "lib/d.dart", &format!("{classe}void f(A a) {{\n  a.me▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
}

#[test]
fn posicao_de_tipo_so_oferece_tipos() {
    let mut p = Projeto::novo("completar-tipos");
    let base = "import 'dart:math' as m;\nclass Caixa<T> {\n  List<▮> itens = [];\n}\nint valor = 0;\nint dobro(int x) => x;\n";
    let r = completar(&mut p, "lib/a.dart", base);
    let r_rotulos = rotulos(&r);
    for esperado in ["T", "Caixa", "int", "String", "m", "dynamic"] {
        assert!(r_rotulos.contains(&esperado.to_string()), "{esperado} em {r_rotulos:?}");
    }
    for proibido in ["valor", "dobro(…)", "print(…)", "itens", "true", "null"] {
        assert!(!r_rotulos.contains(&proibido.to_string()), "{proibido} em {r_rotulos:?}");
    }
    // Parâmetro de função: o `T` da classe não está em escopo.
    let r = completar(&mut p, "lib/b.dart", "int valor = 0;\nvoid f(▮ x) {}\n");
    let r_rotulos = rotulos(&r);
    assert!(r_rotulos.contains(&"int".to_string()) && !r_rotulos.contains(&"valor".to_string()) && !r_rotulos.contains(&"T".to_string()), "{r_rotulos:?}");
    // Pelo prefixo: só os tipos do espaço dele (`pi` é valor).
    let r = completar(&mut p, "lib/c.dart", "import 'dart:math' as m;\nm.▮ aleatorio;\n");
    assert_eq!(rotulos(&r), vec!["Random"]);
    // Tipo de local no corpo: tipos e palavras de comando, sem locais.
    let r = completar(&mut p, "lib/d.dart", "void g(int local) {\n  Str▮ s = '';\n}\n");
    let r_rotulos = rotulos(&r);
    assert!(r_rotulos.contains(&"String".to_string()) && !r_rotulos.contains(&"local".to_string()), "{r_rotulos:?}");
    // Não importado em posição de tipo: só tipos (a classe, não a função).
    p.gravar("lib/util.dart", "class Utilitario {}\nint utilidade() => 0;\n");
    let r = completar(&mut p, "lib/e.dart", "void f(Uti▮ u) {}\n");
    let r_rotulos = rotulos(&r);
    assert!(r_rotulos.contains(&"Utilitario".to_string()) && !r_rotulos.iter().any(|x| x.starts_with("utilidade")), "{r_rotulos:?}");
}

#[test]
fn aproximado_e_relevancia() {
    let mut p = Projeto::novo("completar-relevancia");
    let classe = "class X {\n  int vtabela = 0;\n  int valorTotal = 0;\n  int total = 0;\n}\n";
    let r = completar(&mut p, "lib/a.dart", &format!("{classe}void f(X x) {{\n  x.vt▮\n}}\n"));
    // Prefixo antes de iniciais de palavras; `total` não casa.
    assert_eq!(rotulos(&r), vec!["vtabela", "valorTotal"]);
    let r = completar(&mut p, "lib/b.dart", &format!("{CLASSES}void f(A a) {{\n  a.rtl▮\n}}\n"));
    assert_eq!(rotulos(&r), vec!["rotulo"]);
    // Membros próprios antes dos herdados de `Object`.
    let r = completar(&mut p, "lib/c.dart", &format!("{CLASSES}void f(B b) {{\n  b.▮\n}}\n"));
    let r_rotulos = rotulos(&r);
    let pos = |n: &str| r_rotulos.iter().position(|x| x == n).unwrap();
    assert!(pos("marcado") < pos("toString()") && pos("campo") < pos("hashCode"), "{r_rotulos:?}");
}
