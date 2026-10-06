//! Correções e criações com os títulos do Dart 3.6.2 (`docs/LSP.md`,
//! "Paridade com o servidor do Dart"): sobrescritas ausentes, classe
//! abstrata, `noSuchMethod`, variável final, e as criações para nomes
//! indefinidos; dicas embutidas e tokens semânticos no mesmo projeto.

mod comum;

use comum::{Projeto, aplicar};
use serde_json::{Value, json};

fn onde(texto: &str, agulha: &str, n: usize) -> (u32, u32) {
    let i = texto.match_indices(agulha).nth(n).unwrap_or_else(|| panic!("sem {agulha:?}")).0;
    let antes = &texto[..i];
    (antes.matches('\n').count() as u32, antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32)
}

fn acoes(p: &mut Projeto, rel: &str, pos: (u32, u32)) -> Value {
    p.requisitar(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": p.uri(rel)},
            "range": {"start": {"line": pos.0, "character": pos.1}, "end": {"line": pos.0, "character": pos.1}},
            "context": {"diagnostics": []},
        }),
    )["result"]
        .clone()
}

fn titulos(r: &Value) -> Vec<String> {
    r.as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap().to_string()).collect()
}

fn acao<'a>(r: &'a Value, titulo: &str) -> &'a Value {
    r.as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == titulo)
        .unwrap_or_else(|| panic!("sem {titulo:?} em {:?}", titulos(r)))
}

/// Executa o comando de uma ação (as refatorações do Dart vêm como
/// `dart.edit.refactor`) e devolve a edição do `workspace/applyEdit` que o
/// servidor pede.
fn edicao_do_comando(p: &mut Projeto, acao: &Value) -> Value {
    let comando = &acao["command"];
    p.servidor.receber(json!({"jsonrpc": "2.0", "id": 9000, "method": "workspace/executeCommand", "params": {
        "command": comando["command"], "arguments": comando["arguments"],
    }}));
    let saidas = p.servidor.bombear();
    saidas
        .into_iter()
        .find(|m| m["method"] == "workspace/applyEdit")
        .unwrap_or_else(|| panic!("sem workspace/applyEdit para {acao}"))["params"]["edit"]
        .clone()
}

#[test]
fn sobrescritas_ausentes_classe_abstrata_e_no_such_method() {
    let mut p = Projeto::com_literais("correcoes-sobrescritas");
    let texto = "abstract class Forma {\n  double area();\n  double get perimetro;\n}\n\nclass Quadrado extends Forma {\n  final double lado;\n  Quadrado(this.lado);\n}\n";
    p.abrir("lib/a.dart", texto);
    p.servidor.aguardar_diagnosticos(std::time::Duration::from_secs(120));
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "Quadrado extends", 0));
    let a = acao(&r, "Create 2 missing overrides");
    assert_eq!(a["kind"], "quickfix.create.missingOverrides");
    assert_eq!(
        aplicar(&a["edit"], &p.uri("lib/a.dart"), texto),
        "abstract class Forma {\n  double area();\n  double get perimetro;\n}\n\nclass Quadrado extends Forma {\n  final double lado;\n  Quadrado(this.lado);\n\n  @override\n  double area() {\n    // TODO: implement area\n    throw UnimplementedError();\n  }\n\n  @override\n  // TODO: implement perimetro\n  double get perimetro => throw UnimplementedError();\n}\n"
    );
    let a = acao(&r, "Make class 'Quadrado' abstract");
    assert_eq!(a["kind"], "quickfix.makeClassAbstract");
    assert!(aplicar(&a["edit"], &p.uri("lib/a.dart"), texto).contains("abstract class Quadrado extends Forma"));
    let a = acao(&r, "Create 'noSuchMethod' method");
    assert!(aplicar(&a["edit"], &p.uri("lib/a.dart"), texto).contains("  @override\n  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);\n}"));
}

#[test]
fn criar_metodo_funcao_classe_e_getter_para_nomes_indefinidos() {
    let mut p = Projeto::com_literais("correcoes-criar");
    let texto = "class Ponto {\n  int x = 0;\n}\n\nvoid usar(Ponto q) {\n  q.escalar(2);\n  print(q.raio);\n  var r = Retangulo(2, 3);\n  q.y;\n}\n";
    p.abrir("lib/a.dart", texto);
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "escalar", 0));
    let a = acao(&r, "Create method 'escalar'");
    assert_eq!(a["kind"], "quickfix.create.method");
    assert_eq!(
        aplicar(&a["edit"], &p.uri("lib/a.dart"), texto),
        "class Ponto {\n  int x = 0;\n\n  void escalar(int i) {}\n}\n\nvoid usar(Ponto q) {\n  q.escalar(2);\n  print(q.raio);\n  var r = Retangulo(2, 3);\n  q.y;\n}\n"
    );
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "raio", 0));
    let t = titulos(&r);
    assert!(t.contains(&"Create getter 'raio'".to_string()) && t.contains(&"Create field 'raio'".to_string()), "{t:?}");
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "Retangulo", 0));
    let t = titulos(&r);
    assert!(t.contains(&"Create class 'Retangulo'".to_string()) && t.contains(&"Create function 'Retangulo'".to_string()), "{t:?}");
    // `q.y` está a um passo de `x`: trocar.
    let (l, c) = onde(texto, "q.y", 0);
    let r = acoes(&mut p, "lib/a.dart", (l, c + 2));
    let a = acao(&r, "Change to 'x'");
    assert!(aplicar(&a["edit"], &p.uri("lib/a.dart"), texto).contains("  q.x;\n"));
}

#[test]
fn dicas_embutidas_de_tipo_e_parametro() {
    let mut p = Projeto::com_literais("correcoes-dicas");
    let texto = "int soma(int a, int b) => a + b;\nvoid f() {\n  var t = soma(1, 2);\n  final l = [1, 2];\n  print(t + l.length);\n}\n";
    p.abrir("lib/a.dart", texto);
    let r = p.requisitar(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 6, "character": 0}}}),
    )["result"]
        .clone();
    let dicas: Vec<(u64, u64, String)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["position"]["line"].as_u64().unwrap(), d["position"]["character"].as_u64().unwrap(), d["label"][0]["value"].as_str().unwrap().to_string()))
        .collect();
    assert!(dicas.contains(&(2, 6, "int".into())), "{dicas:?}");
    assert!(dicas.contains(&(2, 15, "a:".into())), "{dicas:?}");
    assert!(dicas.contains(&(2, 18, "b:".into())), "{dicas:?}");
    assert!(dicas.contains(&(3, 8, "List<int>".into())), "{dicas:?}");
    assert!(dicas.contains(&(3, 12, "<int>".into())), "{dicas:?}");
}

#[test]
fn dicas_de_anotacao_inicializador_setter_e_padrao() {
    let mut p = Projeto::com_literais("correcoes-dicas-2");
    let texto = "class A {\n  const A(this.m, [int? n]);\n  final String m;\n  A.b(int x) : this(x.toString());\n  set s(v) {}\n}\nclass B extends A {\n  B(String q) : super(q);\n  factory B.f(q) = B;\n}\n@A('x')\nvoid g() {\n  var c = C<int>();\n  for (var MapEntry(key: k, value: v) in {1: 'a'}.entries) {}\n}\nclass C<T> {}\nclass _U {\n  late final _w = _calc();\n  final _v = 1;\n  List<int> _calc() => [1];\n  late final _x = 1;\n  late final _y = this._calc();\n  late var _z = _calc();\n}\n";
    p.abrir("lib/a.dart", texto);
    let r = p.requisitar(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 16, "character": 0}}}),
    )["result"]
        .clone();
    let dicas: Vec<(u64, u64, String)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["position"]["line"].as_u64().unwrap(), d["position"]["character"].as_u64().unwrap(), d["label"].as_array().unwrap().iter().map(|x| x["value"].as_str().unwrap()).collect::<String>()))
        .collect();
    assert!(dicas.contains(&(3, 20, "m:".into())), "{dicas:?}");
    assert!(dicas.contains(&(4, 6, "void".into())), "{dicas:?}");
    assert!(dicas.contains(&(7, 22, "m:".into())), "{dicas:?}");
    assert!(dicas.contains(&(8, 14, "dynamic".into())), "{dicas:?}");
    assert!(dicas.contains(&(10, 3, "m:".into())), "{dicas:?}");
    assert!(!dicas.contains(&(12, 11, "<int>".into())), "{dicas:?}");
    assert!(dicas.contains(&(13, 19, "<int, String>".into())), "{dicas:?}");
    assert!(dicas.contains(&(17, 13, "List<int>".into())), "{dicas:?}");
    assert!(dicas.contains(&(18, 8, "int".into())), "{dicas:?}");
    // `late` com inicializador que chama método da própria classe: o tipo
    // do método, sem falso ciclo de inferência.
    assert!(dicas.contains(&(21, 13, "List<int>".into())), "{dicas:?}");
    assert!(dicas.contains(&(22, 11, "List<int>".into())), "{dicas:?}");
}

#[test]
fn tokens_semanticos_basicos() {
    let mut p = Projeto::com_literais("correcoes-tokens");
    let r = p.inicializacao.clone();
    let legenda = r["result"]["capabilities"]["semanticTokensProvider"]["legend"].clone();
    let tipos: Vec<String> = legenda["tokenTypes"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect();
    let texto = "/// Doc.\nclass A {\n  int x = 1;\n  void m(int y) {\n    print(y + x);\n  }\n}\n";
    p.abrir("lib/a.dart", texto);
    let r = p.requisitar("textDocument/semanticTokens/full", json!({"textDocument": {"uri": p.uri("lib/a.dart")}}))["result"].clone();
    let dados: Vec<u64> = r["data"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let (mut l, mut c) = (0u64, 0u64);
    let mut tokens = Vec::new();
    for g in dados.chunks(5) {
        if g[0] > 0 {
            l += g[0];
            c = g[1];
        } else {
            c += g[1];
        }
        tokens.push((l, c, g[2], tipos[g[3] as usize].clone()));
    }
    let tipo_em = |linha: u64, coluna: u64| tokens.iter().find(|t| t.0 == linha && t.1 == coluna).map(|t| t.3.clone());
    assert_eq!(tipo_em(0, 0).as_deref(), Some("comment"));
    assert_eq!(tipo_em(1, 0).as_deref(), Some("keyword"));
    assert_eq!(tipo_em(1, 6).as_deref(), Some("class"));
    assert_eq!(tipo_em(2, 2).as_deref(), Some("class"));
    assert_eq!(tipo_em(2, 6).as_deref(), Some("variable"));
    assert_eq!(tipo_em(2, 10).as_deref(), Some("number"));
    assert_eq!(tipo_em(3, 7).as_deref(), Some("method"));
    assert_eq!(tipo_em(3, 13).as_deref(), Some("parameter"));
    assert_eq!(tipo_em(4, 4).as_deref(), Some("function"));
    assert_eq!(tipo_em(4, 10).as_deref(), Some("parameter"));
    assert_eq!(tipo_em(4, 14).as_deref(), Some("property"));
}

#[test]
fn assistencias_de_reescrita() {
    // As refatorações (`Inline Local Variable`) exigem `workspace.applyEdit`
    // no cliente, como no `dart language-server` 3.6.2.
    let mut p = Projeto::com_capacidades(
        "correcoes-assistencias",
        json!({
            "workspace": {"applyEdit": true},
            "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}
        }),
    );
    let texto = "int dobro(int v) => v * 2;\nvoid f(int a) {\n  var x = dobro(a);\n  if (x > 1) print(x);\n  dobro(3);\n}\n";
    p.abrir("lib/a.dart", texto);
    let uri = p.uri("lib/a.dart");
    // No nome da função de corpo `=>`: bloco e async.
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "dobro(int", 0));
    let a = acao(&r, "Convert to block body");
    assert_eq!(aplicar(&a["edit"], &uri, texto).lines().take(3).collect::<Vec<_>>(), vec!["int dobro(int v) {", "  return v * 2;", "}"]);
    let a = acao(&r, "Convert to async function body");
    assert!(aplicar(&a["edit"], &uri, texto).starts_with("Future<int> dobro(int v) async => v * 2;"));
    // No `var x`: dividir; num uso, embutir.
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "x = dobro", 0));
    let a = acao(&r, "Split variable declaration");
    assert!(aplicar(&a["edit"], &uri, texto).contains("  int x;\n  x = dobro(a);\n"));
    let (l, c) = onde(texto, "print(x)", 0);
    let r = acoes(&mut p, "lib/a.dart", (l, c + 6));
    let a = acao(&r, "Inline Local Variable");
    assert_eq!(a["kind"], "refactor.inline");
    let edicao = edicao_do_comando(&mut p, a);
    assert!(aplicar(&edicao, &uri, texto).contains("  if (dobro(a) > 1) print(dobro(a));\n"));
    // `if` sem chaves.
    let a = acao(&r, "Use curly braces");
    assert!(aplicar(&a["edit"], &uri, texto).contains("  if (x > 1) {\n    print(x);\n  }\n"));
    // Comando de expressão com valor: guardar num local.
    let r = acoes(&mut p, "lib/a.dart", onde(texto, "dobro(3)", 0));
    let a = acao(&r, "Assign value to new local variable");
    assert!(aplicar(&a["edit"], &uri, texto).contains("  var dobro2 = dobro(3);\n") || aplicar(&a["edit"], &uri, texto).contains("  var dobro = dobro(3);\n"));
}
