//! `textDocument/completion` through JSON-RPC messages, with incomplete code.

mod common;

use common::{Project, item, mark, labels};
use serde_json::json;

/// Opens `text` (with the `▮` marker) and completes at the marker.
fn completion(project: &mut Project, relative: &str, text: &str) -> serde_json::Value {
    let (text, line, column) = mark(text);
    project.open(relative, &text);
    project.in_position(
        "textDocument/completion",
        relative,
        line,
        column,
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
fn announces_capability() {
    let mut p = Project::new("completar-cap");
    let r = p.initialization.clone();
    assert_eq!(
        r["result"]["capabilities"]["completionProvider"]["triggerCharacters"],
        json!(["."])
    );
}

#[test]
fn members_by_receiver_type_with_dangling_dot() {
    let mut p = Project::new("completar-membros");
    // `b.` without a name or `;`: the statement does not parse and the parser
    // discards it.
    let r = completion(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(B b) {{\n  b.▮\n}}\n"),
    );
    let r_labels = labels(&r);
    for expected in [
        "marcado",
        "campo",
        "rotulo",
        "met(…)",
        "toString()",
        "hashCode",
    ] {
        assert!(
            r_labels.contains(&expected.to_string()),
            "{expected} in {r_labels:?}"
        );
    }
    // Static members and constructors do not appear on an instance.
    assert!(
        !r_labels
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
    // The replaced range is the (empty) prefix at the cursor position.
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"],
        json!({
            "start": {"line": 12, "character": 4}, "end": {"line": 12, "character": 4}
        })
    );
    // The order and the sortText of `dart language-server` 3.6.2 on the same
    // text (the server only sorts when it truncates).
    let order: Vec<(String, String)> = r["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["label"].as_str().unwrap().to_string(), i["sortText"].as_str().unwrap().to_string()))
        .collect();
    let expected_: Vec<(String, String)> = [
        ("marcado", "9444"),
        ("met(…)", "9499"),
        ("campo", "9447"),
        ("rotulo", "9447"),
        ("toString()", "9499"),
        ("noSuchMethod(…)", "9555"),
        ("hashCode", "9449"),
        ("runtimeType", "9449"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(order, expected_);
}

#[test]
fn filters_by_typed_prefix_without_semicolon() {
    let mut p = Project::new("completar-prefixo");
    // `a.ca` followed by another statement: without the closure, both
    // statements disappear.
    let r = completion(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.ca▮\n  print(1);\n}}\n"),
    );
    assert_eq!(labels(&r), vec!["campo"]);
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"]["start"],
        json!({"line": 12, "character": 4})
    );
    assert_eq!(
        item(&r, "campo")["textEdit"]["range"]["end"],
        json!({"line": 12, "character": 6})
    );
    // Case does not matter in the filter.
    let r = completion(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.TOS▮\n}}\n"),
    );
    assert_eq!(labels(&r), vec!["toString()"]);
}

#[test]
fn generic_types_are_substituted() {
    let mut p = Project::new("completar-generico");
    let r = completion(
        &mut p,
        "lib/a.dart",
        "void f(List<String> xs) {\n  xs.▮\n}\n",
    );
    assert_eq!(item(&r, "first")["detail"], "String");
    assert_eq!(item(&r, "add(…)")["detail"], "(String value) → void");
    // The receiver is the result of another expression, inside arguments.
    let r = completion(
        &mut p,
        "lib/b.dart",
        "void f(List<String> xs) {\n  print(xs.first.▮);\n}\n",
    );
    assert!(labels(&r).contains(&"toUpperCase()".to_string()));
    assert_eq!(
        item(&r, "substring(…)")["detail"],
        "(int start, [int? end]) → String"
    );
}

#[test]
fn scope_locals_parameters_members_top_level_and_keywords() {
    let mut p = Project::new("completar-escopo");
    p.write(
        "lib/util.dart",
        "int dobro(int x) => x * 2;\nint _escondido = 0;\n",
    );
    let text = format!(
        "import 'util.dart';\nimport 'util.dart' as u;\n{CLASSES}int global = 0;\nclass C extends A {{\n  int proprio = 0;\n  void m(int parametro) {{\n    var local = '';\n    {{ var interno = 1; }}\n    ▮\n    var depois = 2;\n  }}\n}}\n"
    );
    let r = completion(&mut p, "lib/a.dart", &text);
    let r_labels = labels(&r);
    for expected in [
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
            r_labels.contains(&expected.to_string()),
            "{expected} in {r_labels:?}"
        );
    }
    // Out of scope, declared later, private to another library.
    for missing in ["interno", "depois", "_escondido"] {
        assert!(
            !r_labels.contains(&missing.to_string()),
            "{missing} in {r_labels:?}"
        );
    }
    assert_eq!(item(&r, "local")["detail"], "String");
    assert_eq!(item(&r, "parametro")["detail"], "int");
    assert_eq!(item(&r, "dobro(…)")["kind"], 3);
    // Dart's relevance at the start of a statement (`Block_statement`): the
    // `sortText` of each item, that of `dart language-server` 3.6.2 on the same
    // text (the server does not sort the list; the client sorts by it).
    if let Some(label) = [
        ("local", "9424"),
        ("parametro", "9444"),
        ("return", "9444"),
        ("proprio", "9455"),
        ("campo", "9455"),
        ("if", "9463"),
        ("A", "9477"),
        ("C", "9477"),
        ("dobro(…)", "9486"),
        ("met(…)", "9487"),
        ("global", "9496"),
        ("this", "9497"),
        ("u", "9499"),
    ]
    .iter()
    .filter(|(label, order)| item(&r, label)["sortText"] != *order)
    .map(|(label, order)| format!("{label}: {} (expected {order})", item(&r, label)["sortText"]))
    .collect::<Vec<_>>()
    .into_iter()
    .reduce(|a, b| format!("{a}; {b}"))
    {
        panic!("diverging sortText: {label}");
    }

    // With a prefix, first what starts with it (case-insensitively); then what
    // only matches fuzzily (`Comparable` contains `to`).
    let r = completion(
        &mut p,
        "lib/b.dart",
        &text.replace("    ▮\n", "    var z = para▮\n"),
    );
    assert_eq!(labels(&r), vec!["parametro", "Comparable"]);
}

#[test]
fn import_prefix_and_statics() {
    let mut p = Project::new("completar-prefixo-import");
    p.write(
        "lib/util.dart",
        "int dobro(int x) => x * 2;\nclass Util {}\n",
    );
    let r = completion(
        &mut p,
        "lib/a.dart",
        "import 'util.dart' as u;\nvoid f() {\n  u.▮\n}\n",
    );
    // The `exportNamespace` order (the source's), with the constructor
    // (`dart language-server` 3.6.2).
    assert_eq!(labels(&r), vec!["dobro(…)", "Util", "Util()"]);
    let r = completion(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f() {{\n  A.▮\n}}\n"),
    );
    // Constructors before statics (`dart language-server` 3.6.2).
    assert_eq!(labels(&r), vec!["vazio()", "contar()"]);
}

#[test]
fn named_arguments_not_yet_passed() {
    let mut p = Project::new("completar-nomeados");
    let r = completion(
        &mut p,
        "lib/a.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.met(1, idade: 2, ▮);\n}}\n"),
    );
    let r_labels = labels(&r);
    assert_eq!(r_labels[0], "nome: ");
    assert!(!r_labels.contains(&"idade: ".to_string()));
    assert_eq!(item(&r, "nome: ")["detail"], "String?");
    // Without closing the call: the analyzed text gets `);`.
    let r = completion(
        &mut p,
        "lib/b.dart",
        &format!("{CLASSES}void f(A a) {{\n  a.met(1, ▮\n}}\n"),
    );
    // Declaration order (`dart language-server` 3.6.2).
    assert_eq!(&labels(&r)[..2], ["nome: ", "idade: "]);
}

#[test]
fn nothing_in_comments_and_strings() {
    let mut p = Project::new("completar-comentario");
    let r = completion(&mut p, "lib/a.dart", "void f(int x) {\n  // x.▮\n}\n");
    assert_eq!(r["result"]["items"], json!([]));
    let r = completion(
        &mut p,
        "lib/b.dart",
        "void f(int x) {\n  print('x.▮');\n}\n",
    );
    assert_eq!(r["result"]["items"], json!([]));
    // Interpolation is code.
    let r = completion(
        &mut p,
        "lib/c.dart",
        "void f(int x) {\n  print('${x.▮}');\n}\n",
    );
    assert!(labels(&r).contains(&"isEven".to_string()));
}

#[test]
fn top_level_offers_declaration_keywords_and_types() {
    let mut p = Project::new("completar-topo");
    let r = completion(&mut p, "lib/a.dart", &format!("{CLASSES}cl▮\n"));
    // The prefix first; `Comparable` matches fuzzily (`c…l`).
    assert_eq!(labels(&r), vec!["class", "Comparable"]);
    let r = completion(&mut p, "lib/b.dart", &format!("{CLASSES}▮\n"));
    let r_labels = labels(&r);
    // After declarations, `dart language-server` 3.6.2 does not offer `import`
    // (only the declaration keywords and the types).
    assert!(r_labels.contains(&"A".to_string()) && r_labels.contains(&"class".to_string()), "{r_labels:?}");
    assert!(!r_labels.contains(&"import".to_string()), "{r_labels:?}");
}

#[test]
fn edited_document_uses_current_text() {
    let mut p = Project::new("completar-vigente");
    p.open("lib/a.dart", "class A { int velho = 0; }\nvoid f(A a) {}\n");
    p.change(
        "lib/a.dart",
        2,
        "class A { int novo = 0; }\nvoid f(A a) {\n  a.\n}\n",
    );
    let r = p.in_position("textDocument/completion", "lib/a.dart", 2, 4, json!({}));
    let r_labels = labels(&r);
    assert!(r_labels.contains(&"novo".to_string()) && !r_labels.contains(&"velho".to_string()));
}

#[test]
fn part_file_uses_owning_library() {
    let mut p = Project::new("completar-parte");
    p.write(
        "lib/a.dart",
        "part 'p.dart';\npart 'q.dart';\nclass A { int campo = 0; }\nint global = 0;\n",
    );
    let r = completion(
        &mut p,
        "lib/p.dart",
        "part of 'a.dart';\nvoid f(A a) {\n  a.▮\n}\n",
    );
    assert!(labels(&r).contains(&"campo".to_string()), "{r}");
    let r = completion(
        &mut p,
        "lib/q.dart",
        "part of 'a.dart';\nvoid g() {\n  glo▮\n}\n",
    );
    assert_eq!(labels(&r), vec!["global"]);
    // A part that the owner does not declare: enters alone, without the
    // owner's names.
    let r = completion(
        &mut p,
        "lib/r.dart",
        "part of 'a.dart';\nvoid h() {\n  glo▮\n}\n",
    );
    assert_eq!(r["result"]["items"], json!([]));
}

/// The item labeled `label` whose `detail` is `detail`.
fn item_with<'a>(r: &'a serde_json::Value, label: &str, detail: &str) -> &'a serde_json::Value {
    r["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == label && i["detail"] == detail)
        .unwrap_or_else(|| panic!("no {label} ({detail}) in {:?}", labels(r)))
}

#[test]
fn auto_import_from_sdk_and_project() {
    let mut p = Project::new("completar-autoimport");
    p.write(
        "lib/util.dart",
        "/// Soma dois.\nint somar(int a, int b) => a + b;\nclass Utilitario {}\n",
    );
    // Without `workspace.applyEdit`, the Dart server does not suggest
    // not-imported items.
    let r = completion(&mut p, "lib/a.dart", "void f() {\n  Rand▮\n}\n");
    assert!(!labels(&r).contains(&"Random".to_string()), "{r}");
    p.restart(json!({"capabilities": {"workspace": {"applyEdit": true}}}));
    // Like `dart language-server` 3.6.2: in the list, the item carries only the
    // `importUris` in `data`; the `detail` and the import edit come in
    // `completionItem/resolve`.
    let r = completion(&mut p, "lib/a.dart", "void f() {\n  Rand▮\n}\n");
    let random = item(&r, "Random").clone();
    assert_eq!(random["kind"], 7);
    assert!(random.get("detail").is_none(), "{random}");
    assert!(random.get("additionalTextEdits").is_none(), "{random}");
    assert_eq!(random["data"]["importUris"], json!(["dart:math"]));
    assert_eq!(random["textEdit"]["newText"], "Random");
    let resolved = p.request("completionItem/resolve", random);
    assert_eq!(resolved["result"]["detail"], "Auto import from 'dart:math'");
    assert_eq!(
        resolved["result"]["additionalTextEdits"],
        json!([{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "import 'dart:math';\n\n"}])
    );
    // The SDK's private library (`dart:_interna`) is not offered.
    assert!(!r.to_string().contains("dart:_interna"));
    // From the project (with `package_config`, the `package:` URI), after the
    // existing imports (a blank line between the groups), like
    // `dart language-server` 3.6.2.
    let r = completion(&mut p, "lib/b.dart", "import 'dart:math';\nvoid f() {\n  som▮\n}\n");
    let absorb = item(&r, "somar(…)").clone();
    assert_eq!(absorb["detail"], "(int a, int b) → int");
    let resolved = p.request("completionItem/resolve", absorb);
    assert_eq!(resolved["result"]["detail"], "Auto import from 'package:projeto/util.dart'\n\n(int a, int b) → int");
    assert_eq!(resolved["result"]["additionalTextEdits"][0]["newText"], "\n\nimport 'package:projeto/util.dart';");
    assert_eq!(resolved["result"]["additionalTextEdits"][0]["range"]["start"], json!({"line": 0, "character": 19}));
    // Already imported: the item is the in-scope one, without an additional edit.
    let r = completion(&mut p, "lib/c.dart", "import 'dart:math';\nvoid f() {\n  Rand▮\n}\n");
    let random = item(&r, "Random");
    assert!(random.get("additionalTextEdits").is_none(), "{random}");
    assert_eq!(labels(&r).iter().filter(|x| *x == "Random").count(), 1);
    // In a part, the import would go to another file: not offered.
    p.write("lib/dona.dart", "part 'parte.dart';\n");
    let r = completion(&mut p, "lib/parte.dart", "part of 'dona.dart';\nvoid g() {\n  Rand▮\n}\n");
    assert!(!labels(&r).contains(&"Random".to_string()), "{r}");
    // With nothing typed, there are no not-imported items.
    let r = completion(&mut p, "lib/d.dart", "void f() {\n  ▮\n}\n");
    assert!(!r.to_string().contains("importUris"));
}

#[test]
fn resolve_brings_documentation() {
    let mut p = Project::new("completar-resolve");
    let r = p.initialization.clone();
    assert_eq!(r["result"]["capabilities"]["completionProvider"]["resolveProvider"], true);
    p.write("lib/util.dart", "/// Soma dois.\nint somar(int a, int b) => a + b;\n");
    let r = completion(
        &mut p,
        "lib/a.dart",
        "class A {\n  /// O campo guardado.\n  int campo = 1;\n}\nvoid f(A a) {\n  a.ca▮\n  som▮\n}\n".replacen("  som▮\n", "", 1).as_str(),
    );
    let field = item(&r, "campo").clone();
    let resolved = p.request("completionItem/resolve", field);
    assert_eq!(resolved["result"]["documentation"], "O campo guardado.");
    assert_eq!(resolved["result"]["label"], "campo");
    // Item of a not-imported library (only with `workspace.applyEdit`): the
    // documentation comes from its file.
    p.restart(json!({"capabilities": {"workspace": {"applyEdit": true}, "textDocument": {"completion": {"completionItem": {"documentationFormat": ["markdown"]}}}}}));
    let r = completion(&mut p, "lib/b.dart", "void f() {\n  som▮\n}\n");
    let absorb = item(&r, "somar(…)").clone();
    let resolved = p.request("completionItem/resolve", absorb);
    assert_eq!(resolved["result"]["documentation"], json!({"kind": "markdown", "value": "Soma dois."}));
    // Without `data`, the item comes back as it came.
    let resolved = p.request("completionItem/resolve", json!({"label": "x"}));
    assert_eq!(resolved["result"], json!({"label": "x"}));
}

#[test]
fn call_snippets_when_client_accepts() {
    let mut p = Project::new("completar-snippet");
    let class = "class A {\n  void met(int x, {String? nome, int? idade}) {}\n  void req({required int n}) {}\n  void nada() {}\n  int campo = 0;\n}\n";
    // Without snippet support: only the name.
    let r = completion(&mut p, "lib/a.dart", &format!("{class}void f(A a) {{\n  a.me▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
    assert!(item(&r, "met(…)").get("insertTextFormat").is_none());
    // Only with `snippetSupport`: `dart.completeFunctionCalls` is off by
    // default, and the item is only the name (`dart language-server` 3.6.2).
    p.restart(json!({"capabilities": {"workspace": {"configuration": true}, "textDocument": {"completion": {"completionItem": {"snippetSupport": true}}}}}));
    let r = completion(&mut p, "lib/b.dart", &format!("{class}void f(A a) {{\n  a.▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
    // With `completeFunctionCalls`: the required parameters as stops; only one
    // is the final one (`$0`), as in the Dart server.
    p.configure(json!({"completeFunctionCalls": true}));
    let r = completion(&mut p, "lib/c.dart", &format!("{class}void f(A a) {{\n  a.▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met(${0:x})");
    assert_eq!(item(&r, "met(…)")["insertTextFormat"], 2);
    assert_eq!(item(&r, "req(…)")["textEdit"]["newText"], "req(n: ${0:n})");
    assert_eq!(item(&r, "nada()")["textEdit"]["newText"], "nada()");
    assert_eq!(item(&r, "nada()")["insertTextFormat"], 2);
    // A field is not a call.
    assert_eq!(item(&r, "campo")["textEdit"]["newText"], "campo");
    assert!(item(&r, "campo").get("insertTextFormat").is_none());
    // Parentheses already written: only the name.
    let r = completion(&mut p, "lib/d.dart", &format!("{class}void f(A a) {{\n  a.me▮(1);\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
    // `completeFunctionCalls: false` turns it off.
    p.configure(json!({"completeFunctionCalls": false}));
    let r = completion(&mut p, "lib/e.dart", &format!("{class}void f(A a) {{\n  a.me▮\n}}\n"));
    assert_eq!(item(&r, "met(…)")["textEdit"]["newText"], "met");
}

#[test]
fn type_position_offers_only_types() {
    let mut p = Project::new("completar-tipos");
    let base = "import 'dart:math' as m;\nclass Caixa<T> {\n  List<▮> itens = [];\n}\nint valor = 0;\nint dobro(int x) => x;\n";
    let r = completion(&mut p, "lib/a.dart", base);
    let r_labels = labels(&r);
    for expected in ["T", "Caixa", "int", "String", "m", "dynamic"] {
        assert!(r_labels.contains(&expected.to_string()), "{expected} in {r_labels:?}");
    }
    for forbidden in ["valor", "dobro(…)", "print(…)", "itens", "true", "null"] {
        assert!(!r_labels.contains(&forbidden.to_string()), "{forbidden} in {r_labels:?}");
    }
    // Function parameter: the class's `T` is not in scope.
    let r = completion(&mut p, "lib/b.dart", "int valor = 0;\nvoid f(▮ x) {}\n");
    let r_labels = labels(&r);
    assert!(r_labels.contains(&"int".to_string()) && !r_labels.contains(&"valor".to_string()) && !r_labels.contains(&"T".to_string()), "{r_labels:?}");
    // `m.` at the start of a top-level declaration: the Dart 3.6.2 server
    // treats it as the start of the member (declaration keywords and types,
    // those of the prefix as `m.Random`), not as a prefixed type name.
    let r = completion(&mut p, "lib/c.dart", "import 'dart:math' as m;\nm.▮ aleatorio;\n");
    let r_labels = labels(&r);
    assert!(r_labels.contains(&"m.Random".to_string()) && r_labels.contains(&"class".to_string()), "{r_labels:?}");
    assert!(!r_labels.contains(&"pi".to_string()) && !r_labels.contains(&"m.pi".to_string()), "{r_labels:?}");
    // Type of a local in the body: types and statement keywords, no locals.
    let r = completion(&mut p, "lib/d.dart", "void g(int local) {\n  Str▮ s = '';\n}\n");
    let r_labels = labels(&r);
    assert!(r_labels.contains(&"String".to_string()) && !r_labels.contains(&"local".to_string()), "{r_labels:?}");
    // Not imported in a type position (with `workspace.applyEdit`): only types
    // (the class, not the function).
    p.restart(json!({"capabilities": {"workspace": {"applyEdit": true}}}));
    p.write("lib/util.dart", "class Utilitario {}\nint utilidade() => 0;\n");
    let r = completion(&mut p, "lib/e.dart", "void f(Uti▮ u) {}\n");
    let r_labels = labels(&r);
    assert!(r_labels.contains(&"Utilitario".to_string()) && !r_labels.iter().any(|x| x.starts_with("utilidade")), "{r_labels:?}");
}

#[test]
fn fuzzy_and_relevance() {
    let mut p = Project::new("completar-relevancia");
    let class = "class X {\n  int vtabela = 0;\n  int valorTotal = 0;\n  int total = 0;\n}\n";
    let r = completion(&mut p, "lib/a.dart", &format!("{class}void f(X x) {{\n  x.vt▮\n}}\n"));
    // Prefix before word initials; `total` does not match.
    assert_eq!(labels(&r), vec!["vtabela", "valorTotal"]);
    let r = completion(&mut p, "lib/b.dart", &format!("{CLASSES}void f(A a) {{\n  a.rtl▮\n}}\n"));
    // The `FuzzyMatcher` (`TEXT` style) does not match `rtl` with `label`
    // (score -1).
    assert!(labels(&r).is_empty(), "{:?}", labels(&r));
    // Own members before those inherited from `Object`.
    let r = completion(&mut p, "lib/c.dart", &format!("{CLASSES}void f(B b) {{\n  b.▮\n}}\n"));
    let r_labels = labels(&r);
    let pos = |n: &str| r_labels.iter().position(|x| x == n).unwrap();
    assert!(pos("marcado") < pos("toString()") && pos("campo") < pos("hashCode"), "{r_labels:?}");
}

#[test]
fn statics_and_constructors_after_class_name_mid_name() {
    // The cursor inside the already written name (`A.v▮azio()`): the class's
    // constructors and statics, not the instance members.
    let mut p = Project::new("completar-estatico-meio");
    let text = format!("{CLASSES}void f() {{\n  var x = A.v▮azio();\n  var y = A.contar();\n}}\n");
    let r = completion(&mut p, "lib/a.dart", &text);
    let names = labels(&r);
    assert!(names.iter().any(|n| n == "vazio()"), "{names:?}");
    assert!(!names.iter().any(|n| n == "toString"), "{names:?}");
    // `A.c▮ontar()`: the static (the receiver resolves to the class even with
    // the name replaced by the sentinel).
    let text = format!("{CLASSES}void f() {{\n  A.c▮ontar();\n}}\n");
    let names = labels(&completion(&mut p, "lib/b.dart", &text));
    assert!(names.iter().any(|n| n == "contar()"), "{names:?}");
}

#[test]
fn class_name_call_targets_like_the_resolved_tree() {
    // `A.▮vazio()` invokes a constructor: an `InstanceCreationExpression` in
    // the analyzer, whose name lists only the constructors; `A.▮contar()` is a
    // static call whose class target has no static type (no `Type` members).
    let mut p = Project::new("completar-alvo-classe");
    let text = format!("{CLASSES}void f() {{\n  var x = A.▮vazio();\n}}\n");
    assert_eq!(labels(&completion(&mut p, "lib/a.dart", &text)), vec!["vazio()"]);
    let text = format!("{CLASSES}void f() {{\n  A.▮contar();\n}}\n");
    assert_eq!(labels(&completion(&mut p, "lib/b.dart", &text)), vec!["vazio()", "contar()"]);
}

#[test]
fn prefixed_constants_ranked_by_their_inferred_types() {
    // In a fresh session the constants of `chars.dart` were not inferred yet;
    // their types come on demand, so the `int` context ranks `plus` first.
    let mut p = Project::new("completar-tipo-sob-demanda");
    p.write("lib/chars.dart", "const letter = 'a';\nconst plus = 0x2b;\n");
    let r = completion(&mut p, "lib/a.dart", "import 'chars.dart' as chars;\nint f() => chars.▮;\n");
    let sort = |label: &str| item(&r, label)["sortText"].as_str().unwrap().to_string();
    assert!(sort("plus") < sort("letter"), "{r}");
    assert_eq!(item(&r, "plus")["detail"], "int");
}

#[test]
fn not_imported_types_in_a_declaration_name() {
    // `int Ran▮ge()`: the name position offers types, the not-imported ones
    // included (the prefix is the matcher's).
    let mut p = Project::new("completar-nome-declaracao");
    p.restart(json!({"capabilities": {"workspace": {"applyEdit": true}}}));
    let r = completion(&mut p, "lib/a.dart", "class C {\n  int Ran▮ge() => 0;\n}\n");
    assert_eq!(item(&r, "Random")["data"]["importUris"], json!(["dart:math"]), "{r}");
}
