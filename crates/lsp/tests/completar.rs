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
    // Locais antes dos membros, membros antes do topo, palavras no fim.
    let pos = |rotulo: &str| r_rotulos.iter().position(|r| r == rotulo).unwrap();
    assert!(pos("local") < pos("proprio"));
    assert!(pos("proprio") < pos("global"));
    assert!(pos("global") < pos("dobro(…)"));
    assert!(pos("dobro(…)") < pos("return"));

    // Com prefixo, só o que começa com ele (sem diferenciar maiúsculas).
    let r = completar(
        &mut p,
        "lib/b.dart",
        &texto.replace("    ▮\n", "    var z = para▮\n"),
    );
    assert_eq!(rotulos(&r), vec!["parametro"]);
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
    assert_eq!(rotulos(&r), vec!["class"]);
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
