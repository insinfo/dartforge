//! Features beyond definition/hover/references, through JSON-RPC over a
//! temporary project with a minimal SDK: signature help, highlights,
//! implementations, type definition, folding, selection ranges and type
//! hierarchy (rules in `docs/LSP.md`, "Paridade com o servidor do Dart").

mod common;

use common::Project;
use serde_json::{Value, json};

/// UTF-16 line and column of the start of the `n`-th occurrence of
/// `needle`, plus `shift` columns.
fn location(text: &str, needle: &str, n: usize, shift: u32) -> (u32, u32) {
    let i = text.match_indices(needle).nth(n).unwrap_or_else(|| panic!("no {needle:?} #{n}")).0;
    let before = &text[..i];
    (before.matches('\n').count() as u32, before.rsplit('\n').next().unwrap().encode_utf16().count() as u32 + shift)
}

const MODEL: &str = "/// Uma caixa de [T].
class Caixa<T> {
  T valor;
  Caixa(this.valor);
  /// Troca o valor.
  void trocar(T novo, {bool avisar = false}) {}
}

abstract class Forma {
  double area();
}

class Quadrado extends Forma {
  @override
  double area() => 1;
}

class Circulo implements Forma {
  @override
  double area() => 3;
}

/// Soma dois números.
int soma(int a, [int b = 0]) => a + b;
";

const USE: &str = "import 'modelo.dart';

void usar(List<int> lista, Forma f) {
  lista.add(1);
  soma(1, 2);
  final c = Caixa<String>('a');
  c.trocar('b', avisar: true);
  var total = soma(3);
  total = total + 1;
  print(total);
  [1].map((x) => soma(x));
  f.area();
}
";

fn project(name: &str, capabilities: Value) -> Project {
    let mut p = Project::with_capabilities(name, capabilities);
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    p
}

fn signature(p: &mut Project, needle: &str, n: usize, shift: u32, context: Value) -> Value {
    let (l, c) = location(USE, needle, n, shift);
    p.in_position("textDocument/signatureHelp", "lib/uso.dart", l, c, json!({"context": context}))["result"].clone()
}

#[test]
fn signature_help_like_dart() {
    let mut p = project("assinatura", json!({"textDocument": {"signatureHelp": {"signatureInformation": {"documentationFormat": ["markdown"]}}}}));
    let invoked = json!({"triggerKind": 1});
    // Member of a generic receiver: the type already substituted.
    let r = signature(&mut p, "add(", 0, 4, invoked.clone());
    assert_eq!(r["signatures"][0]["label"], "add(int value)", "{r}");
    assert_eq!(r["activeParameter"], 0);
    // Top-level function with an optional and a default; the second argument active.
    let r = signature(&mut p, "soma(1, 2)", 0, 8, invoked.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    assert_eq!(r["signatures"][0]["parameters"][1]["label"], "int b = 0");
    assert_eq!(r["activeParameter"], 1);
    assert_eq!(r["signatures"][0]["documentation"]["value"], "Soma dois números.");
    // Constructor with the creation's type arguments.
    let r = signature(&mut p, "Caixa<String>(", 0, 14, invoked.clone());
    assert_eq!(r["signatures"][0]["label"], "Caixa(String valor)", "{r}");
    // Substituted generic method, the named one active by its name.
    let r = signature(&mut p, "avisar: true", 0, 2, invoked.clone());
    assert_eq!(r["signatures"][0]["label"], "trocar(String novo, {bool avisar = false})", "{r}");
    assert_eq!(r["activeParameter"], 1);
    // Automatic trigger only on the `(` that opens the list.
    let auto = json!({"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": false});
    let r = signature(&mut p, "soma(3)", 0, 5, auto.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    let r = signature(&mut p, "soma(1, 2)", 0, 8, auto);
    assert!(r.is_null(), "{r}");
    // Inside a closure passed as an argument: the inner call, never the outer one.
    let r = signature(&mut p, "soma(x)", 0, 5, invoked.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    let r = signature(&mut p, "(x) =>", 0, 6, invoked);
    assert!(r.is_null(), "{r}");
}

#[test]
fn highlights_implementations_and_type() {
    let mut p = project("navegar_mais", json!({}));
    // Highlights: declaration and uses of the local in the file.
    let (l, c) = location(USE, "total", 1, 1);
    let r = p.in_position("textDocument/documentHighlight", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let lines: Vec<u64> = r.as_array().unwrap().iter().map(|h| h["range"]["start"]["line"].as_u64().unwrap()).collect();
    assert_eq!(lines, vec![7, 8, 8, 9], "{r}");
    // Implementations of an abstract class and of one of its methods.
    let (l, c) = location(MODEL, "Forma {", 0, 1);
    let r = p.in_position("textDocument/implementation", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let mut names: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
    names.sort();
    assert_eq!(names, vec![12, 17], "{r}");
    let (l, c) = location(USE, "area()", 0, 1);
    let r = p.in_position("textDocument/implementation", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let mut lines: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
    lines.sort();
    assert_eq!(lines, vec![14, 19], "{r}");
    // Type definition: the class of the static type.
    let (l, c) = location(USE, "c.trocar", 0, 0);
    let r = p.in_position("textDocument/typeDefinition", "lib/uso.dart", l, c, json!({}))["result"].clone();
    assert!(r["uri"].as_str().unwrap().ends_with("modelo.dart"), "{r}");
    assert_eq!(r["range"]["start"], json!({"line": 1, "character": 6}));
}

#[test]
fn type_definition_like_the_dart_handler() {
    let mut p = Project::new("tipo-def");
    let text = "class A {\n  int x = 0;\n  A(int v) : x = v;\n  @override\n  String toString() => '';\n  @Deprecated('m')\n  void m() {\n    x = 1;\n  }\n}\n";
    p.open("lib/td.dart", text);
    let mut td = |needle: &str| {
        let (l, c) = location(text, needle, 0, 0);
        p.in_position("textDocument/typeDefinition", "lib/td.dart", l, c, json!({}))["result"].clone()
    };
    let empty = |r: &Value| r.is_null() || r.as_array().is_some_and(|a| a.is_empty());
    // The `returnType` of the constructor declaration: the class.
    assert_eq!(td("A(int")["range"]["start"], json!({"line": 0, "character": 6}));
    // The initializer's field and `@override`: nothing.
    let r = td("x = v");
    assert!(empty(&r), "{r}");
    let r = td("override");
    assert!(empty(&r), "{r}");
    // `@Deprecated(…)`: the class.
    let r = td("Deprecated");
    assert!(r["uri"].as_str().is_some_and(|u| u.contains("core")), "{r}");
    // The target of a field write: the field's type.
    let r = td("x = 1");
    assert!(r["uri"].as_str().is_some_and(|u| u.contains("core")), "{r}");
}

#[test]
fn class_name_highlights_in_constructor() {
    let mut p = Project::new("destaques-construtor");
    let text = "class A {\n  A();\n  A.n();\n}\nA f() => A.n();\n";
    p.open("lib/d.dart", text);
    let mut lines_and_columns = |needle: &str, n: usize| {
        let (l, c) = location(text, needle, n, 0);
        let r = p.in_position("textDocument/documentHighlight", "lib/d.dart", l, c, json!({}))["result"].clone();
        let mut v: Vec<(u64, u64)> = r.as_array().unwrap().iter().map(|h| (h["range"]["start"]["line"].as_u64().unwrap(), h["range"]["start"]["character"].as_u64().unwrap())).collect();
        v.sort();
        v
    };
    // The `A` of `A();` and of `A.n();` is the class: its group.
    let expected = vec![(0, 6), (1, 2), (2, 2), (4, 0), (4, 9)];
    assert_eq!(lines_and_columns("A();", 0), expected);
    assert_eq!(lines_and_columns("A.n();", 0), expected);
    // The declared `n`: the constructor and the use.
    assert_eq!(lines_and_columns("n();", 0), vec![(2, 4), (4, 11)]);
}

#[test]
fn operator_call_hierarchy() {
    let mut p = Project::new("chamadas-operador");
    let text = "class A {\n  @override\n  bool operator ==(Object o) => true;\n  @override\n  int get hashCode => 0;\n}\nbool f(A a, A b) => a == b;\n";
    p.open("lib/o.dart", text);
    let (l, c) = location(text, "== b", 0, 0);
    let r = p.in_position("textDocument/prepareCallHierarchy", "lib/o.dart", l, c, json!({}))["result"].clone();
    assert_eq!(r[0]["name"], "==", "{r}");
}

#[test]
fn implementation_by_member_kind() {
    // A read getter is not implemented by a homonymous setter; the declared
    // field finds the getter or, if not final, the setter.
    let mut p = Project::new("implementacao-especie");
    let text = "class A {\n  int get v => 0;\n  set v(int x) {}\n  int w = 0;\n}\nclass B extends A {\n  @override\n  set v(int x) {}\n  @override\n  set w(int x) {}\n}\nvoid f(A a) => print(a.v);\n";
    p.open("lib/i.dart", text);
    let mut imp = |needle: &str, d: u32| {
        let (l, c) = location(text, needle, 0, d);
        p.in_position("textDocument/implementation", "lib/i.dart", l, c, json!({}))["result"].clone()
    };
    let r = imp("v);", 0);
    assert!(r.as_array().is_some_and(|a| a.is_empty()), "{r}");
    let r = imp("w = 0", 0);
    let lines: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
    assert_eq!(lines, vec![9], "{r}");
}

#[test]
fn type_hierarchy() {
    let mut p = project("hierarquia", json!({}));
    let (l, c) = location(MODEL, "Quadrado", 0, 1);
    let r = p.in_position("textDocument/prepareTypeHierarchy", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let item = r[0].clone();
    assert_eq!(item["name"], "Quadrado", "{r}");
    let sup = p.request("typeHierarchy/supertypes", json!({"item": item}))["result"].clone();
    let names: Vec<&str> = sup.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Forma"], "{sup}");
    let shape = sup[0].clone();
    let sub = p.request("typeHierarchy/subtypes", json!({"item": shape}))["result"].clone();
    let mut names: Vec<&str> = sub.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, vec!["Circulo", "Quadrado"], "{sub}");
}

#[test]
fn folds_and_selection() {
    let mut p = project("dobras", json!({"textDocument": {"foldingRange": {"lineFoldingOnly": true}}}));
    let r = p.request("textDocument/foldingRange", json!({"textDocument": {"uri": p.uri("lib/modelo.dart")}}))["result"].clone();
    let regions: Vec<(u64, u64)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["startLine"].as_u64().unwrap(), d["endLine"].as_u64().unwrap()))
        .collect();
    // Class Caixa (1–6), Forma (8–10), Quadrado (12–15), Circulo (17–20).
    assert_eq!(regions, vec![(1, 6), (8, 10), (12, 15), (17, 20)], "{r}");
    let (l, c) = location(USE, "soma(1, 2)", 0, 1);
    let r = p.request(
        "textDocument/selectionRange",
        json!({"textDocument": {"uri": p.uri("lib/uso.dart")}, "positions": [{"line": l, "character": c}]}),
    )["result"]
        .clone();
    // The identifier, the call, ... up to the whole function.
    let first = &r[0]["range"];
    assert_eq!(first["start"], json!({"line": l, "character": c - 1}));
    assert_eq!(first["end"], json!({"line": l, "character": c + 3}));
    let mut level = &r[0];
    let mut n = 0;
    while level.get("parent").is_some() {
        level = &level["parent"];
        n += 1;
    }
    assert!(n >= 3, "{r}");
    assert_eq!(level["range"]["start"], json!({"line": 2, "character": 0}));
}

#[test]
fn call_hierarchy() {
    let mut p = project("chamadas", json!({}));
    // `soma` is called three times by `usar` (once inside a closure).
    let (l, c) = location(MODEL, "soma(int", 0, 1);
    let r = p.in_position("textDocument/prepareCallHierarchy", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let item = r[0].clone();
    assert_eq!(item["name"], "soma", "{r}");
    assert_eq!(item["kind"], 12);
    let received = p.request("callHierarchy/incomingCalls", json!({"item": item}))["result"].clone();
    let callers: Vec<(&str, usize)> = received
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["from"]["name"].as_str().unwrap(), c["fromRanges"].as_array().unwrap().len()))
        .collect();
    assert_eq!(callers, vec![("usar", 3)], "{received}");
    // `usar` calls `somar`, `soma`, the constructor of `Caixa`, `trocar`, `print`, `map` and `area`.
    let (l, c) = location(USE, "usar", 0, 1);
    let r = p.in_position("textDocument/prepareCallHierarchy", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let done = p.request("callHierarchy/outgoingCalls", json!({"item": r[0].clone()}))["result"].clone();
    let mut names: Vec<&str> = done.as_array().unwrap().iter().map(|c| c["to"]["name"].as_str().unwrap()).collect();
    names.sort();
    for expected in ["Caixa", "add", "area", "soma", "trocar"] {
        assert!(names.contains(&expected), "{names:?}");
    }
}

#[test]
fn implementation_follows_the_index_relations() {
    // `appendAllSubtypes` walks the index: a mixin written through a `typedef`
    // is recorded on the alias (no subtype), a class alias is a subtype.
    let mut p = Project::new("implementacao-indice");
    let text = "mixin class M {\n  void m() {}\n}\ntypedef MA = M;\nclass ViaAlias with MA {\n  @override\n  void m() {}\n}\nclass Direct with M {\n  @override\n  void m() {}\n}\nclass Named = Object with M;\n";
    p.open("lib/i.dart", text);
    let lines = |r: &Value| {
        let mut v: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
        v.sort();
        v
    };
    let (l, c) = location(text, "M {", 0, 0);
    let r = p.in_position("textDocument/implementation", "lib/i.dart", l, c, json!({}))["result"].clone();
    assert_eq!(lines(&r), vec![8, 12], "Direct and Named, not ViaAlias: {r}");
    let (l, c) = location(text, "m() {}", 0, 0);
    let r = p.in_position("textDocument/implementation", "lib/i.dart", l, c, json!({}))["result"].clone();
    assert_eq!(lines(&r), vec![10], "only Direct's override: {r}");
}
