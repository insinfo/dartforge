//! Correções e criações com os títulos do Dart 3.6.2 (`docs/LSP.md`,
//! "Paridade com o servidor do Dart"): sobrescritas ausentes, classe
//! abstrata, `noSuchMethod`, variável final, e as criações para nomes
//! indefinidos; dicas embutidas e tokens semânticos no mesmo projeto.

mod common;

use common::{Project, apply};
use serde_json::{Value, json};

fn location(text: &str, needle: &str, n: usize) -> (u32, u32) {
    let i = text.match_indices(needle).nth(n).unwrap_or_else(|| panic!("sem {needle:?}")).0;
    let before = &text[..i];
    (before.matches('\n').count() as u32, before.rsplit('\n').next().unwrap().encode_utf16().count() as u32)
}

fn actions(p: &mut Project, rel: &str, pos: (u32, u32)) -> Value {
    p.request(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": p.uri(rel)},
            "range": {"start": {"line": pos.0, "character": pos.1}, "end": {"line": pos.0, "character": pos.1}},
            "context": {"diagnostics": []},
        }),
    )["result"]
        .clone()
}

fn titles(r: &Value) -> Vec<String> {
    r.as_array().unwrap().iter().map(|a| a["title"].as_str().unwrap().to_string()).collect()
}

fn action<'a>(r: &'a Value, title: &str) -> &'a Value {
    r.as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == title)
        .unwrap_or_else(|| panic!("sem {title:?} em {:?}", titles(r)))
}

/// Executa o comando de uma ação (as refatorações do Dart vêm como
/// `dart.edit.refactor`) e devolve a edição do `workspace/applyEdit` que o
/// servidor pede.
fn command_edit(p: &mut Project, action: &Value) -> Value {
    let command = &action["command"];
    p.server.receive(json!({"jsonrpc": "2.0", "id": 9000, "method": "workspace/executeCommand", "params": {
        "command": command["command"], "arguments": command["arguments"],
    }}));
    let outputs = p.server.pump();
    outputs
        .into_iter()
        .find(|m| m["method"] == "workspace/applyEdit")
        .unwrap_or_else(|| panic!("sem workspace/applyEdit para {action}"))["params"]["edit"]
        .clone()
}

#[test]
fn missing_overrides_abstract_class_and_no_such_method() {
    let mut p = Project::with_literals("correcoes-sobrescritas");
    let text = "abstract class Forma {\n  double area();\n  double get perimetro;\n}\n\nclass Quadrado extends Forma {\n  final double lado;\n  Quadrado(this.lado);\n}\n";
    p.open("lib/a.dart", text);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let r = actions(&mut p, "lib/a.dart", location(text, "Quadrado extends", 0));
    let a = action(&r, "Create 2 missing overrides");
    assert_eq!(a["kind"], "quickfix.create.missingOverrides");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "abstract class Forma {\n  double area();\n  double get perimetro;\n}\n\nclass Quadrado extends Forma {\n  final double lado;\n  Quadrado(this.lado);\n\n  @override\n  double area() {\n    // TODO: implement area\n    throw UnimplementedError();\n  }\n\n  @override\n  // TODO: implement perimetro\n  double get perimetro => throw UnimplementedError();\n}\n"
    );
    let a = action(&r, "Make class 'Quadrado' abstract");
    assert_eq!(a["kind"], "quickfix.makeClassAbstract");
    assert!(apply(&a["edit"], &p.uri("lib/a.dart"), text).contains("abstract class Quadrado extends Forma"));
    let a = action(&r, "Create 'noSuchMethod' method");
    assert!(apply(&a["edit"], &p.uri("lib/a.dart"), text).contains("  @override\n  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);\n}"));
}

#[test]
fn create_method_function_class_and_getter_for_undefined_names() {
    let mut p = Project::with_literals("correcoes-criar");
    let text = "class Ponto {\n  int x = 0;\n}\n\nvoid usar(Ponto q) {\n  q.escalar(2);\n  print(q.raio);\n  var r = Retangulo(2, 3);\n  q.y;\n}\n";
    p.open("lib/a.dart", text);
    let r = actions(&mut p, "lib/a.dart", location(text, "escalar", 0));
    let a = action(&r, "Create method 'escalar'");
    assert_eq!(a["kind"], "quickfix.create.method");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "class Ponto {\n  int x = 0;\n\n  void escalar(int i) {}\n}\n\nvoid usar(Ponto q) {\n  q.escalar(2);\n  print(q.raio);\n  var r = Retangulo(2, 3);\n  q.y;\n}\n"
    );
    let r = actions(&mut p, "lib/a.dart", location(text, "raio", 0));
    let t = titles(&r);
    assert!(t.contains(&"Create getter 'raio'".to_string()) && t.contains(&"Create field 'raio'".to_string()), "{t:?}");
    let r = actions(&mut p, "lib/a.dart", location(text, "Retangulo", 0));
    let t = titles(&r);
    assert!(t.contains(&"Create class 'Retangulo'".to_string()) && t.contains(&"Create function 'Retangulo'".to_string()), "{t:?}");
    // `q.y` está a um passo de `x`: trocar.
    let (l, c) = location(text, "q.y", 0);
    let r = actions(&mut p, "lib/a.dart", (l, c + 2));
    let a = action(&r, "Change to 'x'");
    assert!(apply(&a["edit"], &p.uri("lib/a.dart"), text).contains("  q.x;\n"));
}

#[test]
fn inlay_type_and_parameter_hints() {
    let mut p = Project::with_literals("correcoes-dicas");
    let text = "int soma(int a, int b) => a + b;\nvoid f() {\n  var t = soma(1, 2);\n  final l = [1, 2];\n  print(t + l.length);\n}\n";
    p.open("lib/a.dart", text);
    let r = p.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 6, "character": 0}}}),
    )["result"]
        .clone();
    let hints: Vec<(u64, u64, String)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["position"]["line"].as_u64().unwrap(), d["position"]["character"].as_u64().unwrap(), d["label"][0]["value"].as_str().unwrap().to_string()))
        .collect();
    assert!(hints.contains(&(2, 6, "int".into())), "{hints:?}");
    assert!(hints.contains(&(2, 15, "a:".into())), "{hints:?}");
    assert!(hints.contains(&(2, 18, "b:".into())), "{hints:?}");
    assert!(hints.contains(&(3, 8, "List<int>".into())), "{hints:?}");
    assert!(hints.contains(&(3, 12, "<int>".into())), "{hints:?}");
}

#[test]
fn annotation_initializer_setter_and_pattern_hints() {
    let mut p = Project::with_literals("correcoes-dicas-2");
    let text = "class A {\n  const A(this.m, [int? n]);\n  final String m;\n  A.b(int x) : this(x.toString());\n  set s(v) {}\n}\nclass B extends A {\n  B(String q) : super(q);\n  factory B.f(q) = B;\n}\n@A('x')\nvoid g() {\n  var c = C<int>();\n  for (var MapEntry(key: k, value: v) in {1: 'a'}.entries) {}\n}\nclass C<T> {}\nclass _U {\n  late final _w = _calc();\n  final _v = 1;\n  List<int> _calc() => [1];\n  late final _x = 1;\n  late final _y = this._calc();\n  late var _z = _calc();\n}\n";
    p.open("lib/a.dart", text);
    let r = p.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 16, "character": 0}}}),
    )["result"]
        .clone();
    let hints: Vec<(u64, u64, String)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["position"]["line"].as_u64().unwrap(), d["position"]["character"].as_u64().unwrap(), d["label"].as_array().unwrap().iter().map(|x| x["value"].as_str().unwrap()).collect::<String>()))
        .collect();
    assert!(hints.contains(&(3, 20, "m:".into())), "{hints:?}");
    assert!(hints.contains(&(4, 6, "void".into())), "{hints:?}");
    assert!(hints.contains(&(7, 22, "m:".into())), "{hints:?}");
    assert!(hints.contains(&(8, 14, "dynamic".into())), "{hints:?}");
    assert!(hints.contains(&(10, 3, "m:".into())), "{hints:?}");
    assert!(!hints.contains(&(12, 11, "<int>".into())), "{hints:?}");
    assert!(hints.contains(&(13, 19, "<int, String>".into())), "{hints:?}");
    assert!(hints.contains(&(17, 13, "List<int>".into())), "{hints:?}");
    assert!(hints.contains(&(18, 8, "int".into())), "{hints:?}");
    // `late` com inicializador que chama método da própria classe: o tipo
    // do método, sem falso ciclo de inferência.
    assert!(hints.contains(&(21, 13, "List<int>".into())), "{hints:?}");
    assert!(hints.contains(&(22, 11, "List<int>".into())), "{hints:?}");
}

#[test]
fn nested_type_and_collection_for_in_hints() {
    let mut p = Project::with_literals("correcoes-dicas-3");
    let text = "class C<T> {}\nvoid f(List<String> l) {\n  var m = Map<String, List<C>>();\n  var n = [for (var s in l) s];\n  print([m, n]);\n}\n";
    p.open("lib/a.dart", text);
    let r = p.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 6, "character": 0}}}),
    )["result"]
        .clone();
    let hints: Vec<(u64, u64, String)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["position"]["line"].as_u64().unwrap(), d["position"]["character"].as_u64().unwrap(), d["label"].as_array().unwrap().iter().map(|x| x["value"].as_str().unwrap()).collect::<String>()))
        .collect();
    // `C` cru dentro de `List<C>`: `<dynamic>` depois dele.
    assert!(hints.contains(&(2, 28, "<dynamic>".into())), "{hints:?}");
    // A variável do `for-in` de coleção.
    assert!(hints.contains(&(3, 20, "String".into())), "{hints:?}");
}

#[test]
fn basic_semantic_tokens() {
    let mut p = Project::with_literals("correcoes-tokens");
    let r = p.initialization.clone();
    let caption = r["result"]["capabilities"]["semanticTokensProvider"]["legend"].clone();
    let types: Vec<String> = caption["tokenTypes"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect();
    let text = "/// Doc.\nclass A {\n  int x = 1;\n  void m(int y) {\n    print(y + x);\n  }\n}\n";
    p.open("lib/a.dart", text);
    let r = p.request("textDocument/semanticTokens/full", json!({"textDocument": {"uri": p.uri("lib/a.dart")}}))["result"].clone();
    let data: Vec<u64> = r["data"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let (mut l, mut c) = (0u64, 0u64);
    let mut tokens = Vec::new();
    for g in data.chunks(5) {
        if g[0] > 0 {
            l += g[0];
            c = g[1];
        } else {
            c += g[1];
        }
        tokens.push((l, c, g[2], types[g[3] as usize].clone()));
    }
    let type_in = |line: u64, column: u64| tokens.iter().find(|t| t.0 == line && t.1 == column).map(|t| t.3.clone());
    assert_eq!(type_in(0, 0).as_deref(), Some("comment"));
    assert_eq!(type_in(1, 0).as_deref(), Some("keyword"));
    assert_eq!(type_in(1, 6).as_deref(), Some("class"));
    assert_eq!(type_in(2, 2).as_deref(), Some("class"));
    assert_eq!(type_in(2, 6).as_deref(), Some("variable"));
    assert_eq!(type_in(2, 10).as_deref(), Some("number"));
    assert_eq!(type_in(3, 7).as_deref(), Some("method"));
    assert_eq!(type_in(3, 13).as_deref(), Some("parameter"));
    assert_eq!(type_in(4, 4).as_deref(), Some("function"));
    assert_eq!(type_in(4, 10).as_deref(), Some("parameter"));
    assert_eq!(type_in(4, 14).as_deref(), Some("property"));
}

#[test]
fn rewrite_assists() {
    // As refatorações (`Inline Local Variable`) exigem `workspace.applyEdit`
    // no cliente, como no `dart language-server` 3.6.2.
    let mut p = Project::with_capabilities(
        "correcoes-assistencias",
        json!({
            "workspace": {"applyEdit": true},
            "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}
        }),
    );
    let text = "int dobro(int v) => v * 2;\nvoid f(int a) {\n  var x = dobro(a);\n  if (x > 1) print(x);\n  dobro(3);\n}\n";
    p.open("lib/a.dart", text);
    let uri = p.uri("lib/a.dart");
    // No nome da função de corpo `=>`: bloco e async.
    let r = actions(&mut p, "lib/a.dart", location(text, "dobro(int", 0));
    let a = action(&r, "Convert to block body");
    assert_eq!(apply(&a["edit"], &uri, text).lines().take(3).collect::<Vec<_>>(), vec!["int dobro(int v) {", "  return v * 2;", "}"]);
    let a = action(&r, "Convert to async function body");
    assert!(apply(&a["edit"], &uri, text).starts_with("Future<int> dobro(int v) async => v * 2;"));
    // No `var x`: dividir; num uso, embutir.
    let r = actions(&mut p, "lib/a.dart", location(text, "x = dobro", 0));
    let a = action(&r, "Split variable declaration");
    assert!(apply(&a["edit"], &uri, text).contains("  int x;\n  x = dobro(a);\n"));
    let (l, c) = location(text, "print(x)", 0);
    let r = actions(&mut p, "lib/a.dart", (l, c + 6));
    let a = action(&r, "Inline Local Variable");
    assert_eq!(a["kind"], "refactor.inline");
    let edit = command_edit(&mut p, a);
    assert!(apply(&edit, &uri, text).contains("  if (dobro(a) > 1) print(dobro(a));\n"));
    // `if` sem chaves.
    let a = action(&r, "Use curly braces");
    assert!(apply(&a["edit"], &uri, text).contains("  if (x > 1) {\n    print(x);\n  }\n"));
    // Comando de expressão com valor: guardar num local.
    let r = actions(&mut p, "lib/a.dart", location(text, "dobro(3)", 0));
    let a = action(&r, "Assign value to new local variable");
    assert!(apply(&a["edit"], &uri, text).contains("  var dobro2 = dobro(3);\n") || apply(&a["edit"], &uri, text).contains("  var dobro = dobro(3);\n"));
}
