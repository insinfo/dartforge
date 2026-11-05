//! `definition`, `hover` and `references` through semantic identity
//! (`crates/lsp/src/project.rs`), over JSON-RPC: a matrix by shape (local,
//! parameter, local function, inherited member, generic field, getter,
//! constructors, prefix, enum, extension, type, type parameter, SDK,
//! documentation), with comments, parts and open texts not saved yet.

mod common;

use common::Project;
use serde_json::{Value, json};

/// UTF-16 line and column of the start of the `n`-th occurrence of `needle`.
fn location(text: &str, needle: &str, n: usize) -> (u32, u32) {
    let i = text
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("no {needle:?} #{n}"))
        .0;
    let before = &text[..i];
    (
        before.matches('\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

fn start(l: u32, c: u32) -> Value {
    json!({"line": l, "character": c})
}

fn request(
    p: &mut Project,
    method: &str,
    rel: &str,
    text: &str,
    needle: &str,
    n: usize,
    extra: Value,
) -> Value {
    let (l, c) = location(text, needle, n);
    p.in_position(method, rel, l, c, extra)["result"].clone()
}

/// (uri, start) of the definition.
fn definition(p: &mut Project, rel: &str, text: &str, needle: &str, n: usize) -> (String, Value) {
    let r = request(
        p,
        "textDocument/definition",
        rel,
        text,
        needle,
        n,
        json!({}),
    );
    assert!(r.is_object(), "no definition at {needle:?}: {r}");
    (
        r["uri"].as_str().unwrap().to_string(),
        r["range"]["start"].clone(),
    )
}

fn hover(p: &mut Project, rel: &str, text: &str, needle: &str, n: usize) -> String {
    let r = request(p, "textDocument/hover", rel, text, needle, n, json!({}));
    r["contents"]
        .as_str()
        .unwrap_or_else(|| panic!("no hover at {needle:?}: {r}"))
        .to_string()
}

/// (project-relative uri, line, column) of each reference.
fn references(
    p: &mut Project,
    rel: &str,
    text: &str,
    needle: &str,
    n: usize,
    include: bool,
) -> Vec<(String, u32, u32)> {
    let r = request(
        p,
        "textDocument/references",
        rel,
        text,
        needle,
        n,
        json!({"context": {"includeDeclaration": include}}),
    );
    let base = format!("{}/", p.uri(""));
    r.as_array()
        .unwrap_or_else(|| panic!("no references: {r}"))
        .iter()
        .map(|l| {
            (
                l["uri"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches(&base)
                    .to_string(),
                l["range"]["start"]["line"].as_u64().unwrap() as u32,
                l["range"]["start"]["character"].as_u64().unwrap() as u32,
            )
        })
        .collect()
}

const MODEL: &str = "/// Algo que se compara.
abstract class Comparavel {}

class Base {
  /// Faz algo com [x].
  void met(int x, {int y = 0, required String nome}) {}
}

/// Uma caixa de [T].
class Caixa<T> extends Base implements Comparavel {
  /// O valor guardado.
  T valor;
  Caixa(this.valor);
  Caixa.vazia(T v) : this(v);
  int get dobro => 2;
  @override
  void met(int x, {int y = 0, required String nome}) {}
}

enum Cor { azul, verde }

extension Ext on int {
  int get triplo => this * 3;
}

int contador = 0;
";

const USE: &str = "import 'modelo.dart';
import 'modelo.dart' as m;

void usar(Caixa<int> c, num n) {
  c.met(1, nome: 'a');
  var total = c.valor + c.dobro;
  if (n is int) print(n);
  int dobro(int v) => v * 2;
  print(dobro(total));
  var k = Caixa.vazia(3);
  var k2 = Caixa(4);
  m.contador++;
  print(Cor.azul);
  print(5.triplo);
  print(k.valor + k2.valor);
}
";

#[test]
fn definition_by_shape() {
    let mut p = Project::new("nav-definicao");
    // The model stays only on disk; the use is open.
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    let model = p.uri("lib/modelo.dart");
    let usage = p.uri("lib/uso.dart");
    let at = |t: &str, a: &str, n: usize| {
        let (l, c) = location(t, a, n);
        start(l, c)
    };
    let cases: &[(&str, usize, &str, &str, usize)] = &[
        // (needle at the use, occurrence, expected file, needle at the target, occurrence)
        // Inherited and overridden member: goes to the receiver class's one.
        ("met(1", 0, "modelo", "met(int x", 1),
        // Field of a generic class.
        ("valor + c", 0, "modelo", "valor;", 0),
        // Getter (there is a homonymous local function: no confusion).
        ("dobro;", 0, "modelo", "dobro =>", 0),
        // Local function.
        ("dobro(total", 0, "uso", "dobro(int v", 0),
        // Local.
        ("total));", 0, "uso", "total =", 0),
        // Promoted parameter.
        ("n);", 0, "uso", "n) {", 0),
        // Named constructor and constructor written without a name.
        ("vazia(3", 0, "modelo", "vazia(T", 0),
        ("Caixa(4", 0, "modelo", "Caixa(this", 0),
        // Import prefix and element through the prefix.
        ("m.contador", 0, "uso", "m;", 0),
        ("contador++", 0, "modelo", "contador =", 0),
        // Enum constant, extension getter, annotated type.
        ("azul", 0, "modelo", "azul", 0),
        ("triplo", 0, "modelo", "triplo =>", 0),
        ("Caixa<int>", 0, "modelo", "Caixa<T>", 0),
    ];
    for &(needle, n, file, target, dn) in cases {
        let (uri, pos) = definition(&mut p, "lib/uso.dart", USE, needle, n);
        let (expected_uri, text) = if file == "modelo" {
            (&model, MODEL)
        } else {
            (&usage, USE)
        };
        assert_eq!(&uri, expected_uri, "{needle}");
        assert_eq!(pos, at(text, target, dn), "{needle}");
    }
    // From the model: type parameter and documentation reference.
    p.open("lib/modelo.dart", MODEL);
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "T valor", 0);
    assert_eq!((uri, pos), (model.clone(), at(MODEL, "T> extends", 0)));
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "x].", 0);
    assert_eq!((uri, pos), (model.clone(), at(MODEL, "x, {int y", 0)));
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "T].", 0);
    assert_eq!((uri, pos), (model.clone(), at(MODEL, "T> extends", 0)));
    // SDK element: the SDK file.
    let (uri, _) = definition(&mut p, "lib/uso.dart", USE, "print(n", 0);
    assert!(uri.ends_with("sdk/lib/core/core.dart"), "{uri}");
    // Keyword and whitespace: nothing.
    assert_eq!(
        request(
            &mut p,
            "textDocument/definition",
            "lib/uso.dart",
            USE,
            "var total",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn hover_documentation_like_compute_documentation() {
    let mut p = Project::new("nav-hover-doc");
    let text = "class Base {\n  /// Faz algo com [x].\n  void met(int x) {}\n  final String campo = '';\n}\nclass Filho extends Base {\n  /// O valor.\n  final int valor;\n  Filho(this.valor);\n  @override\n  void met(int x) {}\n  @override\n  String get campo => '';\n}\n";
    p.open("lib/a.dart", text);
    // The parameter shows the documentation of the owning method, inherited.
    assert_eq!(
        hover(&mut p, "lib/a.dart", text, "x) {}", 1),
        "int x\nType: int\n\nFaz algo com [x].\n\nCopied from `Base`."
    );
    // `this.x`: the field's type and documentation.
    assert_eq!(
        hover(&mut p, "lib/a.dart", text, "valor);", 0),
        "int valor\nType: int\n\nO valor."
    );
    // A getter that overrides a field: the synthetic accessor has no
    // documentation.
    assert_eq!(
        hover(&mut p, "lib/a.dart", text, "campo =>", 0),
        "String get campo"
    );
    // The method read through the receiver is the substituted `Member`.
    let usage = "void f(List<String> l) {\n  l.add('a');\n}\n";
    p.open("lib/b.dart", usage);
    assert_eq!(
        hover(&mut p, "lib/b.dart", usage, "add(", 0),
        "void add(String value)\nType: void Function(String)"
    );
    // The class name in `A.n(…)` shows the constructor; the declaration of a
    // type parameter has no hover; the target of `x += …` shows the
    // `writeType`.
    let usage = "class A<T> {\n  A.n(T v);\n}\nvoid g<V>(V v) {\n  var a = A.n(1);\n  num x = 1;\n  x += 2;\n  print([a, x]);\n}\n";
    p.open("lib/c.dart", usage);
    assert_eq!(
        hover(&mut p, "lib/c.dart", usage, "A.n(1", 0),
        "(new) A<int> A.n(int v)"
    );
    assert_eq!(
        request(
            &mut p,
            "textDocument/hover",
            "lib/c.dart",
            usage,
            "V>(V",
            0,
            json!({})
        ),
        Value::Null
    );
    assert_eq!(
        hover(&mut p, "lib/c.dart", usage, "x += 2", 0),
        "num x\nType: num"
    );
}

#[test]
fn hover_by_shape_with_documentation() {
    let mut p = Project::new("nav-hover");
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    let cases: &[(&str, usize, &str)] = &[
        // An override without documentation inherits the overridden member's.
        (
            "met(1",
            0,
            // Three parameters: one per line, like the analyzer's `multiline`; at a
            // call, the invocation's type.
            "void met(\n  int x, {\n  int y = 0,\n  required String nome,\n})\nType: void Function(int, {required String nome, int y})\n\nFaz algo com [x].\n\nCopied from `Base`.",
        ),
        ("valor + c", 0, "T valor\nType: int\n\nO valor guardado."),
        ("dobro;", 0, "int get dobro\nType: int"),
        (
            "dobro(total",
            0,
            "int dobro(int v)\nType: int Function(int)",
        ),
        ("total));", 0, "int total\nType: int"),
        // The displayed type is the promoted one; the declaration, the written one.
        ("n);", 0, "num n\nType: int"),
        ("azul", 0, "Cor azul\nType: Cor"),
        ("contador++", 0, "int contador\nType: int"),
        (
            "Caixa<int>",
            0,
            "class Caixa<T> extends Base implements Comparavel\n\nUma caixa de [T].",
        ),
        (
            "print(n",
            0,
            "void print(Object? object)\nType: void Function(Object?)",
        ),
    ];
    for &(needle, n, expected) in cases {
        assert_eq!(
            hover(&mut p, "lib/uso.dart", USE, needle, n),
            expected,
            "{needle}"
        );
    }
    let constructor = hover(&mut p, "lib/uso.dart", USE, "vazia(3", 0);
    // The `ConstructorMember`: the arguments of the created type.
    assert_eq!(constructor, "(new) Caixa<int> Caixa.vazia(int v)");
    // Markdown: code block, type and documentation after `---`.
    p.restart(
        json!({"capabilities": {"textDocument": {"hover": {"contentFormat": ["markdown"]}}}}),
    );
    let r = request(
        &mut p,
        "textDocument/hover",
        "lib/uso.dart",
        USE,
        "valor + c",
        0,
        json!({}),
    );
    assert_eq!(
        r["contents"]["value"],
        "```dart\nT valor\n```\nType: `int`\n\n*package:projeto/modelo.dart*\n\n---\nO valor guardado."
    );
    // The range is that of the name under the cursor.
    let (l, c) = location(USE, "valor + c", 0);
    assert_eq!(r["range"]["start"], start(l, c));
    assert_eq!(r["range"]["end"], start(l, c + 5));
    // Prefix and keyword have no hover.
    assert_eq!(
        request(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USE,
            "m.contador",
            0,
            json!({})
        ),
        Value::Null
    );
    assert_eq!(
        request(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USE,
            "var k",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn open_texts_and_parts() {
    let mut p = Project::new("nav-abertos");
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    // The model open and changed without saving: definition and hover follow
    // the buffer, not the disk.
    let changed = MODEL.replace("int contador = 0;", "\n/// Quantos.\ndouble contador = 0;");
    p.open("lib/modelo.dart", &changed);
    p.write("lib/modelo.dart", MODEL);
    let (_, pos) = definition(&mut p, "lib/uso.dart", USE, "contador++", 0);
    let (l, c) = location(&changed, "contador =", 0);
    assert_eq!(pos, start(l, c));
    assert_eq!(
        hover(&mut p, "lib/uso.dart", USE, "contador++", 0),
        "double contador\nType: double\n\nQuantos."
    );
    // Part and owning library.
    let owning = "library partes;\npart 'parte.dart';\n/// Base comum.\nint base = 1;\nint dobro() => metade() * 4;\n";
    let part = "part of 'partes.dart';\nint metade() => base ~/ 2;\n";
    p.write("lib/partes.dart", owning);
    p.open("lib/parte.dart", part);
    let (uri, pos) = definition(&mut p, "lib/parte.dart", part, "base ~/", 0);
    assert_eq!(uri, p.uri("lib/partes.dart"));
    let (l, c) = location(owning, "base =", 0);
    assert_eq!(pos, start(l, c));
    assert_eq!(
        hover(&mut p, "lib/parte.dart", part, "base ~/", 0),
        "int base\nType: int\n\nBase comum."
    );
    p.open("lib/partes.dart", owning);
    let (uri, pos) = definition(&mut p, "lib/partes.dart", owning, "metade()", 0);
    assert_eq!(uri, p.uri("lib/parte.dart"));
    let (l, c) = location(part, "metade", 0);
    assert_eq!(pos, start(l, c));
}

#[test]
fn references_without_false_positives_from_shadow_homonym_and_library() {
    let mut p = Project::new("nav-referencias");
    let a = "int f() => 1;\nvoid g() { f(); f(); }\n/// Veja [f].\nvoid h() {}\n";
    let b = "import 'a.dart';\nvoid x() => print(f());\n";
    // Homonym in another library, which does not import `a.dart`.
    let c = "int f() => 2;\nvoid y() => print(f());\n";
    // Prefix, `show` and a homonymous local that shadows.
    let d = "import 'a.dart' as p;\nimport 'a.dart' show f;\nvoid z() { p.f(); int f = 0; print(f); }\n";
    p.write("lib/b.dart", b);
    p.write("lib/c.dart", c);
    p.write("lib/d.dart", d);
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "f()", 0, true);
    // The Dart server's order: the other files by path, the declaring one and
    // the declaration at the end.
    let expected: Vec<(String, u32, u32)> = vec![
        ("lib/b.dart".into(), 1, 18),
        ("lib/d.dart".into(), 1, 21),
        ("lib/d.dart".into(), 2, 13),
        ("lib/a.dart".into(), 1, 11),
        ("lib/a.dart".into(), 1, 16),
        ("lib/a.dart".into(), 2, 10),
        ("lib/a.dart".into(), 0, 4),
    ];
    assert_eq!(refs, expected);
    // Without the declaration.
    let without = references(&mut p, "lib/a.dart", a, "f()", 0, false);
    assert_eq!(without, expected[..expected.len() - 1].to_vec());
    // Starting from a use in a closed file gives the same set.
    p.open("lib/b.dart", b);
    assert_eq!(
        references(&mut p, "lib/b.dart", b, "f()", 0, true),
        expected
    );
    // The homonym of `c.dart` only sees itself.
    p.open("lib/c.dart", c);
    assert_eq!(
        references(&mut p, "lib/c.dart", c, "f()", 1, true),
        vec![
            ("lib/c.dart".to_string(), 1, 18),
            ("lib/c.dart".to_string(), 0, 4)
        ]
    );
    // The shadowing local only sees itself.
    p.open("lib/d.dart", d);
    assert_eq!(
        references(&mut p, "lib/d.dart", d, "f = 0", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 35),
            ("lib/d.dart".to_string(), 2, 22)
        ]
    );
    // The prefix: the use and the declaration, which the Dart 3.6.2 server
    // gives at the start of the file (0:0).
    assert_eq!(
        references(&mut p, "lib/d.dart", d, "p.f", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 11),
            ("lib/d.dart".to_string(), 0, 0)
        ]
    );
}

#[test]
fn documentation_references_like_the_index() {
    // `[A.n]` resolves to the constructor: the analyzer's index does not cite
    // its `A`; `[A.+]` (operator) and `[A]` do.
    let mut p = Project::new("nav-referencias-doc");
    let a = "class A {\n  A.n();\n  A operator +(A o) => this;\n}\n/// [A.n], [A.+] e [A].\nvoid h() {}\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "A {", 0, false);
    assert!(
        refs.contains(&("lib/a.dart".to_string(), 4, 12)),
        "{refs:?}"
    );
    assert!(
        refs.contains(&("lib/a.dart".to_string(), 4, 20)),
        "{refs:?}"
    );
    assert!(
        !refs.contains(&("lib/a.dart".to_string(), 4, 5)),
        "{refs:?}"
    );
    // The constructor: from the end of the prefix to the end.
    let refs = references(&mut p, "lib/a.dart", a, "n();", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 6)), "{refs:?}");
}

#[test]
fn class_references_in_new_and_const_creations() {
    let mut p = Project::new("nav-referencias-criacao");
    let a = "class A {\n  const A.n();\n}\nvoid f() {\n  new A.n();\n  const A.n();\n  A.n();\n}\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "A {", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 6)), "{refs:?}");
    assert!(refs.contains(&("lib/a.dart".to_string(), 5, 8)), "{refs:?}");
    assert!(refs.contains(&("lib/a.dart".to_string(), 6, 2)), "{refs:?}");
}

#[test]
fn member_references_through_family_and_sdk() {
    let mut p = Project::new("nav-ref-membro");
    let a = "class A { void m() {} }\nclass B extends A { void m() {} }\nclass D { void m() {} }\nvoid t(A a, B b, D d) { a.m(); b.m(); d.m(); print(1); }\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "m()", 0, true);
    assert_eq!(
        refs,
        vec![
            ("lib/a.dart".to_string(), 3, 26),
            ("lib/a.dart".to_string(), 3, 33),
            ("lib/a.dart".to_string(), 0, 15),
        ]
    );
    // SDK element: the declaration in the SDK and the uses only in the project.
    let refs = references(&mut p, "lib/a.dart", a, "print", 0, true);
    assert_eq!(refs.len(), 2, "{refs:?}");
    assert!(refs[1].0.ends_with("sdk/lib/core/core.dart"), "{refs:?}");
    assert_eq!(refs[0], ("lib/a.dart".to_string(), 3, 45));
}

#[test]
fn workspace_symbols_include_closed_project_files() {
    let mut p = Project::new("nav-workspace");
    p.write(
        "lib/fechado.dart",
        "class CarregadorRemoto {}\nvoid carregarTudo() {}\n",
    );
    p.write("lib/outro.dart", "int nada = 0;\n");
    p.open("lib/aberto.dart", "void carregarDados() {}\n");
    // Only those of the project (the SDK and the packages also come in, after).
    let in_project = |r: &serde_json::Value| -> Vec<String> {
        r["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| {
                s["location"]["uri"]
                    .as_str()
                    .unwrap()
                    .contains("nav-workspace")
            })
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect()
    };
    let r = p.request("workspace/symbol", json!({"query": "carregar"}));
    // Inter-file order follows directory discovery, which is platform
    // dependent. Check inclusion here; the parity oracle compares full order.
    let found = in_project(&r);
    let mut names = found.clone();
    names.sort();
    assert_eq!(
        names,
        vec!["CarregadorRemoto", "carregarDados()", "carregarTudo()"]
    );
    // Within the closed file, classes precede functions.
    assert!(
        found.iter().position(|n| n == "CarregadorRemoto")
            < found.iter().position(|n| n == "carregarTudo()")
    );
    // Fuzzy: word initials.
    let r = p.request("workspace/symbol", json!({"query": "cR"}));
    assert_eq!(in_project(&r), vec!["CarregadorRemoto"]);
}

#[test]
fn workspace_symbols_resolve_relative_package_roots_as_uris() {
    let mut p = Project::new("nav-symbol-package-uri");
    p.write(
        "../shared package/pubspec.yaml",
        "name: shared_dependency\n",
    );
    p.write(
        "../shared package/lib/api.dart",
        "class PackageUriNeedle {}\n",
    );
    p.write(
        ".dart_tool/package_config.json",
        &json!({"configVersion": 2, "packages": [
            {"name": "projeto", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6"},
            {"name": "shared_dependency", "rootUri": "../../shared%20package/", "packageUri": "lib/../lib/", "languageVersion": "3.6"}
        ]}).to_string(),
    );
    p.open(
        "lib/anchor.dart",
        "import 'package:shared_dependency/api.dart'; void anchor() {}\n",
    );
    let response = p.request("workspace/symbol", json!({"query": "PackageUriNeedle"}));
    let items = response["result"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{response}");
    assert_eq!(
        items[0]["location"]["uri"],
        url::Url::from_file_path(p.root.join("shared package/lib/api.dart"))
            .unwrap()
            .to_string()
    );
}

#[test]
fn workspace_symbols_include_nested_package_files() {
    let mut p = Project::new("nav-symbol-nested-package");
    p.write("benchmarks/pubspec.yaml", "name: benchmark_app\n");
    p.write(
        "benchmarks/lib/app.dart",
        "class SessionNestedBenchmark {}\n",
    );
    p.write("example/pubspec.yaml", "name: example_app\n");
    p.write(
        "example/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[]}"#,
    );
    p.write("example/lib/app.dart", "class SessionNestedExample {}\n");
    p.write("lib/parent.dart", "class SessionNestedParent {}\n");
    p.open("lib/anchor.dart", "void anchor() {}\n");
    let result = p.request("workspace/symbol", json!({"query": "SessionNested"}));
    let items = result["result"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    // A pubspec alone stays in the containing context. A child package
    // configuration creates a later context, regardless of directory order.
    assert_eq!(items[2]["name"], "SessionNestedExample");
    let benchmark = items
        .iter()
        .find(|v| v["name"] == "SessionNestedBenchmark")
        .unwrap();
    assert_eq!(
        benchmark["location"]["uri"],
        p.uri("benchmarks/lib/app.dart")
    );
}

#[test]
fn workspace_symbols_keep_dependency_resolution_in_its_referring_context() {
    let mut p = Project::with_capabilities(
        "nav-symbol-catalog-contexts",
        json!({"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}}),
    );
    for (name, text) in [
        ("version_one", "class CatalogNeedleVersionOne {}"),
        ("version_two", "class CatalogNeedleVersionTwo {}"),
        ("version_three", "class CatalogNeedleVersionThree {}"),
        ("child_dependency", "class CatalogNeedleChildDependency {}"),
        (
            "bridge",
            "import 'package:versioned/api.dart'; class CatalogNeedleBridge {}",
        ),
    ] {
        p.write(&format!("../{name}/lib/api.dart"), text);
    }
    p.write("../version_one/lib/old.dart", "class CatalogNeedleOld {}");
    let package = |name: &str, root: &str| {
        json!({
            "name": name, "rootUri": root, "packageUri": "lib/", "languageVersion": "3.6",
        })
    };
    p.write(
        ".dart_tool/package_config.json",
        &json!({"configVersion": 2, "packages": [
            package("projeto", "../"), package("versioned", "../../version_one/"),
            package("bridge", "../../bridge/"),
        ]})
        .to_string(),
    );
    p.write("example/pubspec.yaml", "name: catalog_child\n");
    let child_config = |version: &str| {
        json!({"configVersion": 2, "packages": [
            package("catalog_child", "../"), package("projeto", "../../"),
            package("versioned", &format!("../../../{version}/")),
            package("bridge", "../../../bridge/"),
            package("child_dependency", "../../../child_dependency/"),
        ]})
        .to_string()
    };
    p.write(
        "example/.dart_tool/package_config.json",
        &child_config("version_two"),
    );
    p.write(
        "example/lib/anchor.dart",
        "import 'package:bridge/api.dart'; import 'package:bridge/adapter.dart'; import 'package:catalog_child/generated.dart';",
    );
    p.write(
        "example/.dart_tool/build/generated/catalog_child/lib/generated.dart",
        "class CatalogNeedleGenerated {}",
    );
    p.write(
        "example/.dart_tool/build/generated/bridge/lib/adapter.dart",
        "import 'api.dart'; class CatalogNeedleGeneratedBridge {}",
    );
    p.write("example/test/runner.dart", "import 'runner.template.dart';");
    p.write(
        "example/.dart_tool/build/generated/catalog_child/test/runner.template.dart",
        "class CatalogNeedleGeneratedTest {}",
    );
    p.open(
        "lib/anchor.dart",
        "import 'package:bridge/api.dart'; void anchor() {}",
    );
    let symbols = |p: &mut Project, query: &str| {
        p.request("workspace/symbol", json!({"query": query}))["result"].clone()
    };
    let versions = symbols(&mut p, "CatalogNeedleVersion");
    assert_eq!(versions.as_array().unwrap().len(), 1, "{versions}");
    assert_eq!(versions[0]["name"], "CatalogNeedleVersionTwo");
    assert_eq!(
        versions[0]["location"]["uri"],
        url::Url::from_file_path(p.root.join("version_two/lib/api.dart"))
            .unwrap()
            .to_string()
    );
    assert_eq!(
        symbols(&mut p, "CatalogNeedleChildDependency")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let bridge = symbols(&mut p, "CatalogNeedleBridge");
    assert_eq!(
        bridge
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["CatalogNeedleBridge", "CatalogNeedleGeneratedBridge"]
    );
    assert_eq!(
        symbols(&mut p, "CatalogNeedleGenerated")
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // A URI that exists only in the parent's package version remains visible.
    assert_eq!(
        symbols(&mut p, "CatalogNeedleOld")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(symbols(&mut p, "CatalogNeedleVersion"), versions);

    p.write(
        "example/.dart_tool/package_config.json",
        &child_config("version_three"),
    );
    p.server.receive(json!({"jsonrpc": "2.0", "method": "workspace/didChangeWatchedFiles",
        "params": {"changes": [{"uri": p.uri("example/.dart_tool/package_config.json"), "type": 2}]}}));
    let versions = symbols(&mut p, "CatalogNeedleVersion");
    assert_eq!(versions.as_array().unwrap().len(), 1, "{versions}");
    assert_eq!(versions[0]["name"], "CatalogNeedleVersionThree");
}

#[test]
fn opening_an_external_project_appends_its_added_symbols() {
    let mut p = Project::new("nav-symbol-external-root");
    p.write("lib/entry.dart", "class LiveRootProbeMain {}\n");
    let initial = p.request("workspace/symbol", json!({"query": "LiveRootProbe"}))["result"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(initial.len(), 1);
    let external = p.root.join("a");
    std::fs::create_dir_all(external.join("lib")).unwrap();
    std::fs::write(external.join("pubspec.yaml"), "name: external\n").unwrap();
    let file = external.join("lib/entry.dart");
    let source = "class LiveRootProbeExternal {}\n";
    std::fs::write(&file, source).unwrap();
    let uri = url::Url::from_file_path(file).unwrap().to_string();
    p.server.receive(
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": source}
        }}),
    );
    p.server.pump();
    let opened = p.request("workspace/symbol", json!({"query": "LiveRootProbe"}))["result"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(opened.len(), 2);
    assert_eq!(opened[0], initial[0]);
    assert_eq!(opened[1]["name"], "LiveRootProbeExternal");
    p.server.receive(
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {
            "textDocument": {"uri": uri}
        }}),
    );
    p.server.pump();
    assert_eq!(
        p.request("workspace/symbol", json!({"query": "LiveRootProbe"}))["result"],
        json!(initial)
    );
}

#[test]
fn workspace_symbols_preserve_discovery_order_through_file_events() {
    fn symbols(p: &mut Project, query: &str) -> Vec<Value> {
        p.request("workspace/symbol", json!({"query": query}))["result"]
            .as_array()
            .unwrap()
            .clone()
    }
    fn watched(p: &mut Project, relative: &str, kind: u32) {
        p.server.receive(
            json!({"jsonrpc": "2.0", "method": "workspace/didChangeWatchedFiles",
            "params": {"changes": [{"uri": p.uri(relative), "type": kind}]}}),
        );
        p.server.pump();
    }

    let mut p = Project::with_capabilities(
        "nav-symbol-events",
        json!({"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}}),
    );
    let declarations = |prefix: &str| {
        (0..300)
            .map(|i| format!("class SessionMember{prefix}{i:03} {{}}\n"))
            .collect::<String>()
    };
    let a_text = declarations("A");
    let b_text = declarations("B");
    p.write("lib/a.dart", &a_text);
    p.write("lib/b.dart", &b_text);
    let before = symbols(&mut p, "SessionMember");
    assert_eq!(before.len(), 500);
    let a = symbols(&mut p, "SessionMemberA");
    let b = symbols(&mut p, "SessionMemberB");
    assert_eq!((a.len(), b.len()), (300, 300));

    p.write("lib/created.dart", "class SessionMemberCreated {}\n");
    watched(&mut p, "lib/created.dart", 1);
    assert_eq!(symbols(&mut p, "SessionMember"), before);
    let created = symbols(&mut p, "SessionMemberCreated");
    assert_eq!(created.len(), 1);

    let (first_path, first_text, first, other) =
        if before[0]["location"]["uri"] == p.uri("lib/a.dart") {
            ("lib/a.dart", &a_text, a, b)
        } else {
            ("lib/b.dart", &b_text, b, a)
        };
    std::fs::remove_file(p.path(first_path)).unwrap();
    watched(&mut p, first_path, 3);
    p.write(first_path, first_text);
    watched(&mut p, first_path, 1);
    // No query occurred between removal and recreation: the old file has
    // nonetheless left its original position and follows the new file.
    let mut expected = other;
    expected.extend(created);
    let remaining = before.len() - expected.len();
    expected.extend(first.into_iter().take(remaining));
    assert_eq!(symbols(&mut p, "SessionMember"), expected);
}

#[test]
fn workspace_symbols_keep_events_before_the_first_search() {
    let capabilities =
        json!({"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}});
    let mut p = Project::with_capabilities("nav-symbol-before-first", capabilities.clone());
    for prefix in ["A", "B"] {
        let text = (0..300)
            .map(|i| format!("class SessionBefore{prefix}{i:03} {{}}\n"))
            .collect::<String>();
        p.write(&format!("lib/{prefix}.dart"), &text);
    }
    p.open("lib/anchor.dart", "void anchor() {}\n");
    let initial =
        p.request("workspace/symbol", json!({"query": "SessionBefore"}))["result"].clone();
    assert_eq!(initial.as_array().unwrap().len(), 500);

    // A fresh server sees the same initial context, but its first search
    // occurs after a watcher event, without a cached result to preserve.
    p.restart(json!({"capabilities": capabilities}));
    p.write("lib/created.dart", "class SessionBeforeCreated {}\n");
    p.server.receive(
        json!({"jsonrpc": "2.0", "method": "workspace/didChangeWatchedFiles",
        "params": {"changes": [{"uri": p.uri("lib/created.dart"), "type": 1}]}}),
    );
    p.server.pump();
    assert_eq!(
        p.request("workspace/symbol", json!({"query": "SessionBefore"}))["result"],
        initial
    );
    let created = p.request("workspace/symbol", json!({"query": "SessionBeforeCreated"}));
    assert_eq!(created["result"].as_array().unwrap().len(), 1);
    assert_eq!(created["result"][0]["name"], "SessionBeforeCreated");
}

#[test]
fn workspace_symbols_only_analyze_projects_with_open_files_when_requested() {
    let mut p = Project::new("nav-symbol-open-projects");
    p.write("lib/closed.dart", "class SessionOnlyOpenClosed {}\n");
    assert!(
        !p.request("workspace/symbol", json!({"query": "SessionOnlyOpen"}))["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    p.restart(json!({"initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": true}}));
    assert_eq!(
        p.request("workspace/symbol", json!({"query": "SessionOnlyOpen"}))["result"],
        json!([])
    );
    // Folder notifications retain configuration scopes, without pinning an
    // analysis context when only open documents should create contexts.
    p.server.receive(
        json!({"jsonrpc": "2.0", "method": "workspace/didChangeWorkspaceFolders",
        "params": {"event": {"added": [{"uri": p.uri(""), "name": "project"}], "removed": []}}}),
    );
    p.server.pump();
    assert_eq!(
        p.request("workspace/symbol", json!({"query": "String"}))["result"],
        json!([])
    );
    p.open("lib/open.dart", "class SessionOnlyOpenEditor {}\n");
    let active =
        p.request("workspace/symbol", json!({"query": "SessionOnlyOpen"}))["result"].clone();
    assert_eq!(active.as_array().unwrap().len(), 2);
    p.server
        .receive(json!({"jsonrpc": "2.0", "method": "textDocument/didClose",
        "params": {"textDocument": {"uri": p.uri("lib/open.dart")}}}));
    p.server.pump();
    for query in ["SessionOnlyOpen", "String"] {
        assert_eq!(
            p.request("workspace/symbol", json!({"query": query}))["result"],
            json!([])
        );
    }
    p.open("lib/open.dart", "class SessionOnlyOpenEditor {}\n");
    assert_eq!(
        p.request("workspace/symbol", json!({"query": "SessionOnlyOpen"}))["result"],
        active
    );
}

#[test]
fn color_presentations_keep_the_open_projects_analysis_context() {
    let mut p = Project::new("nav-open-project-colors");
    let sdk_lib = p.root.join("sdk/lib");
    let catalog = sdk_lib.join("libraries.json");
    let mut libraries: Value =
        serde_json::from_str(&std::fs::read_to_string(&catalog).unwrap()).unwrap();
    libraries["dartdevc"]["libraries"]["ui"] = json!({"uri": "ui/ui.dart", "patches": []});
    std::fs::write(catalog, serde_json::to_vec(&libraries).unwrap()).unwrap();
    std::fs::create_dir_all(sdk_lib.join("ui")).unwrap();
    std::fs::write(
        sdk_lib.join("ui/ui.dart"),
        "library dart.ui;\nclass Color { const Color(int value); }\n",
    )
    .unwrap();
    p.restart(json!({}));
    let text = "import 'dart:ui';\nconst color = Color(0xff336699);\n";
    p.open("lib/color.dart", text);
    let (line, character) = location(text, "Color(", 0);
    let params = json!({"textDocument": {"uri": p.uri("lib/color.dart")},
            "color": {"alpha": 1.0, "red": 0.2, "green": 0.4, "blue": 0.6},
            "range": {"start": start(line, character), "end": start(line, character + 17)}});
    let before = p.request("textDocument/colorPresentation", params.clone())["result"].clone();
    assert_eq!(before.as_array().unwrap().len(), 4, "{before}");
    assert_eq!(before[0]["label"], "Color.fromARGB(255, 51, 102, 153)");
    p.restart(json!({"initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": true}}));
    assert_eq!(
        p.request("textDocument/colorPresentation", params)["result"],
        before
    );
}

/// On a case-insensitive file system (macOS, Windows), the editor may send
/// the URI with a different case than the disk's: the document's unit is
/// still found (the canonical path in every load). Where case matters, the
/// test does not apply.
#[test]
fn uri_with_different_case_than_disk() {
    let mut p = Project::new("nav-caixa");
    let text = "void f() {\n  var s = 'a';\n  var n = s.length;\n}\n";
    p.write("lib/a.dart", text);
    let real = p.path("lib/a.dart");
    let other =
        std::path::PathBuf::from(real.to_string_lossy().replace("/lib/a.dart", "/LIB/a.dart"));
    if !other.exists() {
        return;
    }
    let uri = url::Url::from_file_path(&other).unwrap().to_string();
    p.server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
        }}),
    );
    p.server.pump();
    // The member by receiver type only comes out of the semantic analysis.
    let (l, c) = location(text, "length", 0);
    let r = p.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": {"line": l, "character": c + 1}}),
    );
    assert!(
        r["result"]["contents"]
            .to_string()
            .contains("int get length"),
        "{r}"
    );
}

#[test]
fn references_follow_the_searched_elements() {
    let mut p = Project::new("nav-ref-elementos");
    // `d.toString()` on `dynamic` resolves to `Object.toString`
    // (`_resolveReceiverDynamicBounded`), so it is a reference of the family.
    let a = "class S {\n  @override\n  String toString() => 's';\n}\nvoid f(dynamic d) {\n  d.toString();\n  d.toString(1);\n  print(d.hashCode);\n}\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "toString() =>", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 5, 4)), "{refs:?}");
    // Not with arguments that do not match `Object.toString()`.
    assert!(
        !refs.contains(&("lib/a.dart".to_string(), 6, 4)),
        "{refs:?}"
    );
    // The `[x]` of a constructor with `this.x` cites the parameter, and the
    // `super.x` that points to it is a reference of the parameter: both count
    // for the field itself, not for an override in a subclass.
    let b = "class P {\n  final int x;\n  /// Takes [x].\n  P(this.x);\n}\nclass R extends P {\n  R(super.x);\n  @override\n  int get x => 1;\n}\n";
    p.open("lib/b.dart", b);
    let doc = ("lib/b.dart".to_string(), 2, 13);
    let sup = ("lib/b.dart".to_string(), 6, 10);
    let refs = references(&mut p, "lib/b.dart", b, "x;", 0, false);
    assert!(refs.contains(&doc) && refs.contains(&sup), "{refs:?}");
    let refs = references(&mut p, "lib/b.dart", b, "x => 1", 0, false);
    assert!(!refs.contains(&doc) && !refs.contains(&sup), "{refs:?}");
    assert!(
        refs.contains(&("lib/b.dart".to_string(), 3, 9)),
        "the `this.x` writes the family's field: {refs:?}"
    );
}

#[test]
fn definition_like_the_navigation_computer() {
    let mut p = Project::new("nav-definicao-regioes");
    let a = "class A {\n  final int x;\n  /// Docs.\n  A(this.x);\n  A.named(this.x);\n  const A.c(this.x);\n}\nclass B extends A {\n  B(super.x) : super();\n  int twice() => x * 2;\n}\nclass D {\n  const D();\n}\n@D()\nvoid f(List<int> xs) {\n  for (final e in xs) {\n    print(e);\n  }\n}\n";
    p.open("lib/a.dart", a);
    let class_a = start(0, 6);
    // The class name of a constructor declaration is the class (after a doc
    // comment or a keyword the identifier is the node visited); an unnamed
    // constructor that starts at its name keeps the constructor.
    assert_eq!(
        definition(&mut p, "lib/a.dart", a, "A(this.x)", 0).1,
        class_a
    );
    assert_eq!(definition(&mut p, "lib/a.dart", a, "A.named", 0).1, class_a);
    assert_eq!(
        definition(&mut p, "lib/a.dart", a, "D();", 0).1,
        start(11, 6)
    );
    assert_eq!(
        definition(&mut p, "lib/a.dart", a, "B(super", 0).1,
        start(8, 2)
    );
    // The name of an annotation is the class.
    assert_eq!(
        definition(&mut p, "lib/a.dart", a, "D()\nvoid", 0).1,
        start(11, 6)
    );
    // `super.x` goes to the super constructor's parameter.
    assert_eq!(
        definition(&mut p, "lib/a.dart", a, "x) : super", 0).1,
        start(3, 9)
    );
    // The variable of a `for-in` has no definition.
    let r = request(
        &mut p,
        "textDocument/definition",
        "lib/a.dart",
        a,
        "e in",
        0,
        json!({}),
    );
    assert!(
        r.is_null() || r.as_array().is_some_and(|l| l.is_empty()),
        "{r}"
    );
}

#[test]
fn hover_of_writes_labels_and_unresolved_names() {
    let mut p = Project::new("nav-hover-escritas");
    p.write(
        "lib/m.dart",
        "class M {\n  /// Makes one.\n  M({int Function(int)? size});\n}\n",
    );
    let a = "import 'm.dart';\nclass S {\n  int get v => 0;\n  set v(int x) {}\n}\nvoid f(S s) {\n  s.v = 1;\n  M(size: null);\n  print(undefinedThing(1));\n}\n";
    p.open("lib/a.dart", a);
    // A setter written to shows the `writeType`.
    let h = hover(&mut p, "lib/a.dart", a, "v = 1", 0);
    assert!(h.contains("set v(int x)") && h.contains("Type: int"), "{h}");
    // And its type definition is the written type's class.
    let r = request(
        &mut p,
        "textDocument/typeDefinition",
        "lib/a.dart",
        a,
        "v = 1",
        0,
        json!({}),
    );
    let r = if r.is_array() { r[0].clone() } else { r };
    assert!(
        r["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("core/core.dart")),
        "{r}"
    );
    // The label of a named argument of a library whose bodies were not
    // inferred: the parameter, with the outline's type.
    let h = hover(&mut p, "lib/a.dart", a, "size:", 0);
    assert!(
        h.contains("{int Function(int)? size}") && h.contains("Type: int Function(int)?"),
        "{h}"
    );
    // An undefined name: only the `InvalidType`.
    assert_eq!(
        hover(&mut p, "lib/a.dart", a, "undefinedThing", 0),
        "Type: InvalidType"
    );
}
