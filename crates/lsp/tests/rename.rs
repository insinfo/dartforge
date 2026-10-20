//! `textDocument/prepareRename` and `textDocument/rename` through JSON-RPC
//! messages: locals, parameters, members with overrides, top level across
//! files, refusals and incomplete code.

mod common;

use common::{Project, apply, mark};
use serde_json::{Value, json};

/// Opens `text` (with the `▮` marker) and returns (text without the marker,
/// line, column).
fn open(p: &mut Project, relative: &str, text: &str) -> (String, u32, u32) {
    let (text, line, column) = mark(text);
    p.open(relative, &text);
    (text, line, column)
}

fn rename(p: &mut Project, relative: &str, line: u32, column: u32, new: &str) -> Value {
    p.in_position(
        "textDocument/rename",
        relative,
        line,
        column,
        json!({"newName": new}),
    )
}

fn error(response: &Value) -> String {
    assert_eq!(response["error"]["code"], -32010, "{response}");
    response["error"]["message"].as_str().unwrap().to_string()
}

#[test]
fn announces_prepare_when_client_accepts() {
    let mut p = Project::new("renomear-cap");
    let r =
        p.restart(json!({"capabilities": {"textDocument": {"rename": {"prepareSupport": true}}}}));
    assert_eq!(
        r["result"]["capabilities"]["renameProvider"],
        json!({"prepareProvider": true})
    );
    let r = p.restart(json!({"capabilities": {}}));
    assert_eq!(r["result"]["capabilities"]["renameProvider"], json!(true));
}

#[test]
fn local_and_parameter() {
    let mut p = Project::new("renomear-local");
    let source = "int f(int par) {\n  var lo▮cal = par + 1;\n  local += 2;\n  print('$local ${local}');\n  return local;\n}\nint g() { var local = 0; return local; }\n";
    let (text, l, c) = open(&mut p, "lib/a.dart", source);
    let uri = p.uri("lib/a.dart");
    let prep = p.in_position("textDocument/prepareRename", "lib/a.dart", l, c, json!({}));
    assert_eq!(prep["result"]["placeholder"], "local");
    assert_eq!(
        prep["result"]["range"],
        json!({"start": {"line": 1, "character": 6}, "end": {"line": 1, "character": 11}})
    );
    let r = rename(&mut p, "lib/a.dart", l, c, "total");
    assert_eq!(
        apply(&r["result"], &uri, &text),
        "int f(int par) {\n  var total = par + 1;\n  total += 2;\n  print('$total ${total}');\n  return total;\n}\nint g() { var local = 0; return local; }\n"
    );
    // Parameter, starting from a use.
    let r = rename(&mut p, "lib/a.dart", 1, 17, "base");
    assert!(
        apply(&r["result"], &uri, &text).starts_with("int f(int base) {\n  var local = base + 1;")
    );
    // Whitespace: nothing to rename.
    let prep = p.in_position("textDocument/prepareRename", "lib/a.dart", 1, 0, json!({}));
    assert_eq!(prep["result"], Value::Null);
}

#[test]
fn named_parameter_renames_labels() {
    let mut p = Project::new("renomear-nomeado");
    let source = "void f({int? li▮mite}) { print(limite); }\nvoid g() { f(limite: 1); f(); }\n";
    let (text, l, c) = open(&mut p, "lib/a.dart", source);
    let uri = p.uri("lib/a.dart");
    let r = rename(&mut p, "lib/a.dart", l, c, "teto");
    assert_eq!(
        apply(&r["result"], &uri, &text),
        "void f({int? teto}) { print(teto); }\nvoid g() { f(teto: 1); f(); }\n"
    );
}

#[test]
fn member_with_overrides_and_instance_uses_in_other_file() {
    let mut p = Project::new("renomear-membro");
    let base = "class A {\n  void met() {}\n  void usa() { met(); this.met(); }\n}\nclass B extends A {\n  @override\n  void met() { super.met(); }\n}\nclass C implements A {\n  void met() {}\n  void usa() {}\n}\nclass D { void met() {} }\n";
    // The file that uses it stays only on disk (closed).
    let uses =
        "import 'a.dart';\nvoid f(A a, B b, D d) { a.met(); b.met(); d.met(); a..met()..usa(); }\n";
    p.write("lib/usos.dart", uses);
    let (text, _, _) = open(
        &mut p,
        "lib/a.dart",
        &base.replacen("void met() { super", "void m▮et() { super", 1),
    );
    // Renames starting from the override in B.
    let r = rename(&mut p, "lib/a.dart", 6, 8, "executar");
    let new = apply(&r["result"], &p.uri("lib/a.dart"), &text);
    assert_eq!(
        new,
        "class A {\n  void executar() {}\n  void usa() { executar(); this.executar(); }\n}\nclass B extends A {\n  @override\n  void executar() { super.executar(); }\n}\nclass C implements A {\n  void executar() {}\n  void usa() {}\n}\nclass D { void met() {} }\n"
    );
    assert_eq!(
        apply(&r["result"], &p.uri("lib/usos.dart"), uses),
        "import 'a.dart';\nvoid f(A a, B b, D d) { a.executar(); b.executar(); d.met(); a..executar()..usa(); }\n"
    );
    // Conflict with an existing member in the family.
    let r = rename(&mut p, "lib/a.dart", 6, 8, "usa");
    assert_eq!(
        error(&r),
        "Renamed method will shadow method 'A.usa'.",
        "{r}"
    );
}

#[test]
fn field_with_getter_setter_this_and_initializer() {
    let mut p = Project::new("renomear-campo");
    let source = "class P {\n  int va▮lor;\n  final int outro;\n  P(this.valor) : outro = 0;\n  P.nomeado({required this.valor}) : outro = 1;\n  P.init(int v) : valor = v, outro = 2;\n}\nvoid f(P p) { p.valor = p.valor + 1; P.nomeado(valor: 3); }\n";
    let (text, l, c) = open(&mut p, "lib/a.dart", source);
    let r = rename(&mut p, "lib/a.dart", l, c, "quantia");
    assert_eq!(
        apply(&r["result"], &p.uri("lib/a.dart"), &text),
        "class P {\n  int quantia;\n  final int outro;\n  P(this.quantia) : outro = 0;\n  P.nomeado({required this.quantia}) : outro = 1;\n  P.init(int v) : quantia = v, outro = 2;\n}\nvoid f(P p) { p.quantia = p.quantia + 1; P.nomeado(quantia: 3); }\n"
    );
    // From the `this.value` parameter the target is the same field.
    let r2 = rename(&mut p, "lib/a.dart", 3, 10, "quantia");
    assert_eq!(r2["result"], r["result"]);
}

#[test]
fn top_level_across_files_with_types_constructors_and_show() {
    let mut p = Project::new("renomear-topo");
    let a = "class Caixa {\n  Caixa();\n  Caixa.vazia();\n  static Caixa criar() => Caixa();\n}\nint contador = 0;\n";
    let b = "import 'a.dart' show Caixa, contador;\nimport 'a.dart' as pa;\nCaixa f(List<Caixa> xs) {\n  Caixa c = new Caixa();\n  pa.Caixa d = pa.Caixa.vazia();\n  contador++;\n  return xs.first is Caixa ? c : d;\n}\n";
    p.write("lib/a.dart", a);
    p.open("lib/b.dart", b);
    // From a use in a file that imports; the declaration is closed.
    let r = rename(&mut p, "lib/b.dart", 2, 1, "Pacote");
    assert_eq!(
        apply(&r["result"], &p.uri("lib/a.dart"), a),
        "class Pacote {\n  Pacote();\n  Pacote.vazia();\n  static Pacote criar() => Pacote();\n}\nint contador = 0;\n"
    );
    assert_eq!(
        apply(&r["result"], &p.uri("lib/b.dart"), b),
        "import 'a.dart' show Pacote, contador;\nimport 'a.dart' as pa;\nPacote f(List<Pacote> xs) {\n  Pacote c = new Pacote();\n  pa.Pacote d = pa.Pacote.vazia();\n  contador++;\n  return xs.first is Pacote ? c : d;\n}\n"
    );
    // Top-level variable: read, write and `show`.
    let r = rename(&mut p, "lib/b.dart", 5, 3, "total");
    assert!(apply(&r["result"], &p.uri("lib/b.dart"), b).contains("show Caixa, total;"));
    assert!(apply(&r["result"], &p.uri("lib/b.dart"), b).contains("  total++;"));
    assert_eq!(
        apply(&r["result"], &p.uri("lib/a.dart"), a).lines().last(),
        Some("int total = 0;")
    );
    // A class name cannot be a keyword (Dart 3.6.2 message).
    assert_eq!(
        error(&rename(&mut p, "lib/b.dart", 2, 1, "dynamic")),
        "Class name must not be a keyword."
    );
}

#[test]
fn refusals() {
    let mut p = Project::new("renomear-recusas");
    p.write(
        "lib/outra.dart",
        "import 'a.dart';\nvoid g() => publica();\n",
    );
    let source = "class A {\n  @override\n  String toString() => '';\n}\nvoid publica() {}\nvoid f(int x) {\n  var y = 1;\n  print(x + y);\n}\n";
    open(&mut p, "lib/a.dart", &format!("{source}▮"));
    // The messages of `dart language-server` 3.6.2 for the same requests.
    // SDK element.
    let prep = p.in_position("textDocument/prepareRename", "lib/a.dart", 7, 3, json!({}));
    assert_eq!(
        prep["error"]["message"],
        "The function 'print' is defined in the SDK, so cannot be renamed.",
        "{prep}"
    );
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 7, 3, "imprimir")),
        "The function 'print' is defined in the SDK, so cannot be renamed."
    );
    // The override of an SDK member renames only itself.
    let r = rename(&mut p, "lib/a.dart", 2, 10, "texto");
    let edits = r["result"]["changes"]
        .as_object()
        .expect("changes")
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(
        edits,
        json!([{"newText": "texto", "range": {"start": {"line": 2, "character": 9}, "end": {"line": 2, "character": 17}}}]),
        "{r}"
    );
    // Invalid names.
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 6, 7, "class")),
        "Variable name must not be a keyword."
    );
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 6, 7, "1y")),
        "Variable name must begin with a lowercase letter or underscore."
    );
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 6, 7, "a-b")),
        "Variable name must not contain '-'."
    );
    // Conflict with a parameter in the same body.
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 6, 7, "x")),
        "Usage of parameter \"x\" declared in \"a.dart\" will be shadowed by renamed local variable."
    );
    // A public name that would become private with a use in another library.
    assert_eq!(
        error(&rename(&mut p, "lib/a.dart", 4, 6, "_privada")),
        format!(
            "Renamed function will be invisible in '{}'.",
            std::path::Path::new("lib").join("outra.dart").display()
        )
    );
    // Keyword under the cursor: nothing to rename.
    let prep = p.in_position("textDocument/prepareRename", "lib/a.dart", 5, 1, json!({}));
    assert_eq!(prep["result"], Value::Null);
}

#[test]
fn incomplete_code_in_file() {
    let mut p = Project::new("renomear-incompleto");
    // A broken statement elsewhere in the body does not prevent renaming.
    let source = "void f(int x) {\n  var so▮ma = x;\n  x.\n  print(soma);\n}\n";
    let (text, l, c) = open(&mut p, "lib/a.dart", source);
    let r = rename(&mut p, "lib/a.dart", l, c, "total");
    let new = apply(&r["result"], &p.uri("lib/a.dart"), &text);
    // `x.` followed by `print(sum);` parses as `x.print(sum);`: the argument is
    // still a use of the local.
    assert_eq!(
        new,
        "void f(int x) {\n  var total = x;\n  x.\n  print(total);\n}\n"
    );
    // The missing operand becomes a synthetic identifier (fasta's
    // `ensureIdentifier`): the statement stays in the tree and the use inside
    // it is renamed too.
    let source = "void f(int x) {\n  var so▮ma = x;\n  var q = soma + ;\n  print(soma);\n}\n";
    let (text, l, c) = open(&mut p, "lib/b.dart", source);
    let r = rename(&mut p, "lib/b.dart", l, c, "total");
    let new = apply(&r["result"], &p.uri("lib/b.dart"), &text);
    assert_eq!(
        new,
        "void f(int x) {\n  var total = x;\n  var q = total + ;\n  print(total);\n}\n"
    );
}

#[test]
fn package_import_from_bin() {
    let mut p = Project::new("renomear-pacote");
    let a = "String saudar(String nome) => nome;\n";
    let main = "import 'package:projeto/a.dart';\nvoid main() { print(saudar('x')); }\n";
    p.write("lib/a.dart", a);
    p.open("bin/main.dart", main);
    let r = rename(&mut p, "bin/main.dart", 1, 22, "cumprimentar");
    assert_eq!(
        apply(&r["result"], &p.uri("lib/a.dart"), a),
        "String cumprimentar(String nome) => nome;\n"
    );
    assert_eq!(
        apply(&r["result"], &p.uri("bin/main.dart"), main),
        "import 'package:projeto/a.dart';\nvoid main() { print(cumprimentar('x')); }\n"
    );
}

#[test]
fn enum_extension_getter_and_setter() {
    let mut p = Project::new("renomear-diversos");
    let source = "enum Cor { azul, verde }\nextension Ext on int {\n  int get dobro => this * 2;\n  int triplo() => this * 3;\n}\nclass K {\n  int _v = 0;\n  int get v => _v;\n  set v(int n) { _v = n; }\n}\nint get topo => 1;\nset topo(int x) {}\nvoid f(K k) {\n  var c = Cor.azul;\n  print(c == Cor.azul);\n  print(2.dobro + 3.triplo());\n  k.v = k.v + 1;\n  topo = topo;\n}\n";
    p.open("lib/a.dart", source);
    let uri = p.uri("lib/a.dart");
    // Enum constant, starting from a use.
    let r = rename(&mut p, "lib/a.dart", 13, 14, "celeste");
    let new = apply(&r["result"], &uri, source);
    assert!(new.starts_with("enum Cor { celeste, verde }"), "{r}");
    assert!(
        new.contains("var c = Cor.celeste;") && new.contains("c == Cor.celeste"),
        "{new}"
    );
    // Extension getter and extension method.
    let r = rename(&mut p, "lib/a.dart", 15, 10, "duplo");
    let new = apply(&r["result"], &uri, source);
    assert!(
        new.contains("int get duplo") && new.contains("2.duplo + 3.triplo()"),
        "{r}"
    );
    let r = rename(&mut p, "lib/a.dart", 3, 7, "vezesTres");
    assert!(
        apply(&r["result"], &uri, source).contains("3.vezesTres()"),
        "{r}"
    );
    // Instance getter and setter together, starting from the write.
    let r = rename(&mut p, "lib/a.dart", 16, 4, "valor");
    let new = apply(&r["result"], &uri, source);
    assert!(
        new.contains("int get valor => _v;") && new.contains("set valor(int n)"),
        "{r}"
    );
    assert!(new.contains("k.valor = k.valor + 1;"), "{new}");
    // Top-level getter and setter together.
    let r = rename(&mut p, "lib/a.dart", 10, 9, "cima");
    let new = apply(&r["result"], &uri, source);
    assert!(
        new.contains("int get cima => 1;\nset cima(int x) {}") && new.contains("cima = cima;"),
        "{r}"
    );
}

/// UTF-16 line and column of the start of the `n`-th occurrence of `needle`.
fn location(text: &str, needle: &str, n: usize) -> (u32, u32) {
    let i = text
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("no {needle:?}"))
        .0;
    let before = &text[..i];
    (
        before.matches('\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

fn rename_in(p: &mut Project, rel: &str, text: &str, needle: &str, n: usize, new: &str) -> Value {
    let (l, c) = location(text, needle, n);
    rename(p, rel, l, c, new)
}

#[test]
fn family_through_implicit_extends_object() {
    // `getHierarchyMembers` goes up to `Object` and down through the subtypes
    // the index finds: a class without `extends` in a file that cites `Object`
    // is included; one in a file that does not cite it is not.
    let mut p = Project::new("renomear-object");
    p.write(
        "lib/c.dart",
        "class C {\n  @override\n  String toString() => 'c';\n}\n",
    );
    let a = "import 'c.dart';\nclass A {\n  @override\n  String toString() => 'a';\n}\nclass B {\n  @override\n  String toString() => 'b';\n}\nvoid f(A a, B b, C c, Object o) {\n  a.toString();\n  b.toString();\n  c.toString();\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = rename_in(&mut p, "lib/a.dart", a, "toString() => 'a'", 0, "texto");
    let new = apply(&r["result"], &uri, a);
    assert!(
        new.contains("String texto() => 'a'") && new.contains("String texto() => 'b'"),
        "{new}"
    );
    assert!(
        new.contains("a.texto();\n  b.texto();\n  c.toString();"),
        "{new}"
    );
}

#[test]
fn factory_redirection_outside_candidates() {
    // A file that only cites the class in the `ConstructorName` of a factory
    // redirection is not a search candidate (Dart leaves the reference as is).
    let mut p = Project::new("renomear-redirecionamento");
    p.write(
        "lib/x.dart",
        "import 'b.dart';\nabstract class X {\n  factory X() = B;\n}\n",
    );
    let b = "import 'x.dart';\nclass B implements X {\n  B();\n}\n";
    p.open("lib/b.dart", b);
    let r = rename_in(&mut p, "lib/b.dart", b, "B implements", 0, "C");
    assert!(
        apply(&r["result"], &p.uri("lib/b.dart"), b).contains("class C implements X"),
        "{r}"
    );
    assert!(!r["result"].to_string().contains("x.dart"), "{r}");
}

#[test]
fn field_with_positional_this_argument_and_docs_by_declaration() {
    // The positional argument of a `this.x` is a zero-length reference in the
    // index: renaming the field inserts the new name before it (like Dart).
    let mut p = Project::new("renomear-campo-posicional");
    let a = "class A {\n  final int? x;\n  A([this.x]);\n}\nvoid f(A a) {\n  A(a.x);\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = rename_in(&mut p, "lib/a.dart", a, "x;", 0, "y");
    assert_eq!(
        apply(&r["result"], &uri, a),
        "class A {\n  final int? y;\n  A([this.y]);\n}\nvoid f(A a) {\n  A(ya.y);\n}\n",
        "{r}"
    );
    // The `[c]` of the documentation only changes in the comment of the
    // declaration that declares the renamed parameter.
    let b = "class B {\n  /// Usa [c].\n  B.um(int c);\n  /// Usa [c].\n  B.dois(int c);\n}\n";
    p.open("lib/b.dart", b);
    let uri_b = p.uri("lib/b.dart");
    let r = rename_in(&mut p, "lib/b.dart", b, "c);", 1, "d");
    assert_eq!(
        apply(&r["result"], &uri_b, b),
        "class B {\n  /// Usa [c].\n  B.um(int c);\n  /// Usa [d].\n  B.dois(int d);\n}\n",
        "{r}"
    );
}

#[test]
fn named_parameter_through_hierarchy() {
    // `getHierarchyNamedParameters`: the named homonym of each method of the
    // hierarchy, with the declaration and the references.
    let mut p = Project::new("renomear-nomeado-hierarquia");
    p.write("lib/p.dart", "class P {\n  void add(String n, {Iterable<String>? allowed}) {\n    print(allowed);\n  }\n}\n");
    let q = "import 'p.dart';\nclass Q implements P {\n  @override\n  void add(String n, {Iterable<String>? allowed}) {}\n}\nvoid f(P p) => p.add('a', allowed: []);\n";
    p.open("lib/q.dart", q);
    let r = rename_in(&mut p, "lib/q.dart", q, "allowed}", 0, "permitidos");
    let text = r["result"].to_string();
    assert!(text.contains("p.dart"), "{r}");
    assert_eq!(
        apply(&r["result"], &p.uri("lib/q.dart"), q),
        q.replace("allowed", "permitidos"),
        "{r}"
    );
}

#[test]
fn field_through_super_formal_and_unnamed_constructor_doc() {
    // The `super.x` points to the `this.x` of the superclass constructor:
    // renaming the field includes it. `[A.new]` cites the constructor, not the
    // class.
    let mut p = Project::new("renomear-super-formal");
    let a = "/// Veja [A.new].\nclass A {\n  final int x;\n  A(this.x);\n}\nclass B extends A {\n  B(super.x);\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = rename_in(&mut p, "lib/a.dart", a, "x;", 0, "y");
    assert_eq!(
        apply(&r["result"], &uri, a),
        a.replace("this.x", "this.y")
            .replace("int x", "int y")
            .replace("super.x", "super.y"),
        "{r}"
    );
    let r = rename_in(&mut p, "lib/a.dart", a, "A {", 0, "C");
    let new = apply(&r["result"], &uri, a);
    assert!(new.starts_with("/// Veja [A.new].\nclass C {"), "{new}");
}

#[test]
fn named_constructor() {
    let mut p = Project::new("renomear-construtor");
    let a = "/// Crie com [Caixa.vazia].\nclass Caixa {\n  final int v;\n  Caixa(this.v);\n  Caixa.vazia() : this(0);\n  Caixa.outra() : this.vazia();\n  factory Caixa.fab() = Caixa.vazia;\n}\nclass Sub extends Caixa {\n  Sub() : super.vazia();\n}\nvoid f() {\n  var x = Caixa.vazia();\n  var y = new Caixa.vazia();\n  print([x, y, Caixa(1)]);\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let prep = {
        let (l, c) = location(a, "vazia() :", 0);
        p.in_position("textDocument/prepareRename", "lib/a.dart", l, c, json!({}))
    };
    assert_eq!(prep["result"]["placeholder"], "vazia", "{prep}");
    let r = rename_in(&mut p, "lib/a.dart", a, "vazia() :", 0, "nova");
    assert_eq!(
        apply(&r["result"], &uri, a),
        a.replace("vazia", "nova"),
        "{r}"
    );
    // Starting from a use, the same result.
    let r2 = rename_in(&mut p, "lib/a.dart", a, "vazia();\n  var y", 0, "nova");
    assert_eq!(r2["result"], r["result"]);
    // Conflict with another constructor (the Dart 3.6.2 server's message).
    assert_eq!(
        error(&rename_in(&mut p, "lib/a.dart", a, "vazia() :", 0, "outra")),
        "Class 'Caixa' already declares constructor with name 'outra'."
    );
}

#[test]
fn import_prefix() {
    let mut p = Project::new("renomear-prefixo");
    p.write(
        "lib/a.dart",
        "const anot = 0;\nclass Caixa {}\nCaixa criar() => Caixa();\n",
    );
    let b =
        "import 'a.dart' as pa;\n@pa.anot\npa.Caixa f(pa.Caixa c) => pa.criar();\nint pb = 0;\n";
    p.open("lib/b.dart", b);
    let uri = p.uri("lib/b.dart");
    let r = rename_in(&mut p, "lib/b.dart", b, "pa.Caixa f", 0, "q");
    assert_eq!(
        apply(&r["result"], &uri, b),
        b.replace("pa", "q").replace("qb", "pb"),
        "{r}"
    );
    // From the directive: Dart's `RenameImportRefactoring` replaces the whole
    // `pa.` of each use (the same final text).
    let r2 = rename_in(&mut p, "lib/b.dart", b, "pa;", 0, "q");
    assert_eq!(apply(&r2["result"], &uri, b), apply(&r["result"], &uri, b));
    let changes = r2["result"]["changes"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(
        changes[1],
        json!({"newText": "q.", "range": {"start": {"line": 1, "character": 1}, "end": {"line": 1, "character": 4}}})
    );
    // Through the use, the prefix is a member of the library: the name of one
    // of its top-level declarations is a conflict; through the directive it is
    // not (only the prefix name validation).
    assert_eq!(
        error(&rename_in(&mut p, "lib/b.dart", b, "pa.Caixa f", 0, "pb")),
        "Library already declares getter with name 'pb'."
    );
    assert!(rename_in(&mut p, "lib/b.dart", b, "pa;", 0, "pb")["result"]["changes"].is_object());
    assert_eq!(
        error(&rename_in(&mut p, "lib/b.dart", b, "pa;", 0, "dynamic")),
        "Import prefix name must not be a keyword."
    );
    // Through a use in an expression (`pa.create()`), `getElementOfNode` gives
    // the import: the `pa.` of each use becomes `q.`, and so does the
    // documentation's `[pa.create]`.
    let c = "import 'a.dart' as pa;\n/// Veja [pa.criar].\nvoid g() => pa.criar();\n";
    p.open("lib/c.dart", c);
    let uri_c = p.uri("lib/c.dart");
    let r3 = rename_in(&mut p, "lib/c.dart", c, "pa.criar()", 0, "q");
    assert_eq!(
        apply(&r3["result"], &uri_c, c),
        c.replace("pa", "q"),
        "{r3}"
    );
    let changes = r3["result"]["changes"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(
        changes
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["newText"] == "q" || e["newText"] == "q."),
        "{r3}"
    );
    assert!(
        changes
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["newText"] == "q."
                && e["range"]["start"] == json!({"line": 1, "character": 10})),
        "{r3}"
    );
}

#[test]
fn type_parameter_respects_shadowing() {
    let mut p = Project::new("renomear-tipo");
    let a = "class Caixa<T> {\n  T v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<T> lista() => [v];\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = rename_in(&mut p, "lib/a.dart", a, "T v", 0, "E");
    assert_eq!(
        apply(&r["result"], &uri, a),
        "class Caixa<E> {\n  E v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<E> lista() => [v];\n}\n",
        "{r}"
    );
    // The method's `T` is another parameter.
    let r = rename_in(&mut p, "lib/a.dart", a, "T x", 0, "U");
    assert_eq!(
        apply(&r["result"], &uri, a),
        "class Caixa<T> {\n  T v;\n  Caixa(this.v);\n  S pegar<S, U>(U x, S s) => s;\n  List<T> lista() => [v];\n}\n"
    );
    // A sibling in the same list is a conflict; the name of a type used in the
    // scope is not (the Dart 3.6.2 server renames `T` to `List`).
    assert_eq!(
        error(&rename_in(&mut p, "lib/a.dart", a, "T x", 0, "S")),
        "Duplicate type parameter 'S'."
    );
    let r = rename_in(&mut p, "lib/a.dart", a, "T v", 0, "List");
    assert_eq!(
        apply(&r["result"], &uri, a),
        "class Caixa<List> {\n  List v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<List> lista() => [v];\n}\n",
        "{r}"
    );
}

#[test]
fn named_in_overrides_and_documentation() {
    let mut p = Project::new("renomear-sobrescrita");
    let a = "class A {\n  /// Usa [n] e [valor].\n  void f({int? n}) {}\n  int valor = 0;\n}\nclass B extends A {\n  @override\n  void f({int? n}) { print(n); }\n}\nvoid g(A a, B b) { a.f(n: 1); b.f(n: 2); print(a.valor); }\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = rename_in(&mut p, "lib/a.dart", a, "n})", 0, "m");
    assert_eq!(
        apply(&r["result"], &uri, a),
        "class A {\n  /// Usa [m] e [valor].\n  void f({int? m}) {}\n  int valor = 0;\n}\nclass B extends A {\n  @override\n  void f({int? m}) { print(m); }\n}\nvoid g(A a, B b) { a.f(m: 1); b.f(m: 2); print(a.valor); }\n",
        "{r}"
    );
    // Field cited in the documentation.
    let r = rename_in(&mut p, "lib/a.dart", a, "valor = 0", 0, "total");
    let new = apply(&r["result"], &uri, a);
    assert!(
        new.contains("/// Usa [n] e [total].") && new.contains("print(a.total)"),
        "{new}"
    );
}

#[test]
fn class_file_with_versioned_document_changes() {
    let mut p = Project::new("rename-file");
    let class = "class MinhaClasse {}\n";
    let uses = "import 'minha_classe.dart';\nMinhaClasse? x;\n";
    let main = "import 'package:projeto/minha_classe.dart';\nvoid main() { MinhaClasse(); }\n";
    p.write("lib/minha_classe.dart", class);
    p.write("bin/main.dart", main);
    p.open("lib/usa.dart", uses);
    // `dart.renameFilesWithClasses` belongs to the client configuration
    // (`LspClientConfiguration`), requested through `workspace/configuration`.
    let capabilities = json!({"workspace": {"configuration": true, "workspaceEdit": {"documentChanges": true, "resourceOperations": ["create", "rename"]}}});
    p.restart(json!({"capabilities": capabilities}));
    p.configure(json!({"renameFilesWithClasses": "always"}));
    let r = rename_in(
        &mut p,
        "lib/usa.dart",
        uses,
        "MinhaClasse?",
        0,
        "OutraClasse",
    );
    let changes = r["result"]["documentChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("{r}"))
        .clone();
    // The file operation comes after the text edits.
    let last = changes.last().unwrap();
    assert_eq!(last["kind"], "rename", "{r}");
    assert_eq!(last["oldUri"], p.uri("lib/minha_classe.dart"));
    assert_eq!(last["newUri"], p.uri("lib/outra_classe.dart"));
    let edits = |uri: &str| -> (Value, Value) {
        let d = changes
            .iter()
            .find(|d| d["textDocument"]["uri"] == uri)
            .unwrap_or_else(|| panic!("no {uri}"));
        (d["textDocument"]["version"].clone(), d["edits"].clone())
    };
    // An open file carries the current version; a closed one, `null`.
    let uri_uses = p.uri("lib/usa.dart");
    let (version, e) = edits(&uri_uses);
    assert_eq!(version, json!(1));
    let mut map = serde_json::Map::new();
    map.insert(uri_uses.clone(), e);
    // The Dart 3.6.2 server's `rename` does not rewrite the `import`s (the
    // client asks for them in `workspace/willRenameFiles`).
    assert_eq!(
        apply(&json!({"changes": map}), &uri_uses, uses),
        "import 'minha_classe.dart';\nOutraClasse? x;\n"
    );
    let uri_main = p.uri("bin/main.dart");
    let (version, e) = edits(&uri_main);
    assert_eq!(version, Value::Null);
    let mut map = serde_json::Map::new();
    map.insert(uri_main.clone(), e);
    assert_eq!(
        apply(&json!({"changes": map}), &uri_main, main),
        "import 'package:projeto/minha_classe.dart';\nvoid main() { OutraClasse(); }\n"
    );
    // The `SourceChange` order: the declaration's file and the others by path;
    // the file operation at the end.
    let order: Vec<String> = changes
        .iter()
        .filter_map(|d| d["textDocument"]["uri"].as_str().map(str::to_string))
        .collect();
    assert_eq!(
        order,
        vec![
            p.uri("lib/minha_classe.dart"),
            uri_main.clone(),
            uri_uses.clone()
        ]
    );
    // Without the option, the class changes and the file stays.
    p.restart(json!({"capabilities": capabilities}));
    let r = rename_in(
        &mut p,
        "lib/usa.dart",
        uses,
        "MinhaClasse?",
        0,
        "OutraClasse",
    );
    let changes = r["result"]["documentChanges"].as_array().unwrap();
    assert!(changes.iter().all(|m| m.get("kind").is_none()), "{r}");
    let text_uses = changes
        .iter()
        .find(|d| d["textDocument"]["uri"] == uri_uses)
        .unwrap();
    assert_eq!(text_uses["edits"].as_array().unwrap().len(), 1);
}

#[test]
fn conflicts_by_scope() {
    let mut p = Project::new("renomear-escopo");
    let a = "int topo = 0;\nint valor = 0;\nvoid f() {\n  { var a = 1; print(a); }\n  { var b = 2; print(b); }\n}\nvoid g() { var x = 1; print(topo + x); }\nvoid h() { var c = 1; { var d = 2; print(c + d); } }\nclass K {\n  int campo = 1;\n  int soma() => campo + valor;\n}\n";
    p.open("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    // Disjoint blocks: allowed.
    let r = rename_in(&mut p, "lib/a.dart", a, "b = 2", 0, "a");
    assert!(
        apply(&r["result"], &uri, a).contains("{ var a = 2; print(a); }"),
        "{r}"
    );
    // The responses of `dart language-server` 3.6.2 for the same requests.
    // A local that would shadow the use of the top-level name.
    assert_eq!(
        error(&rename_in(&mut p, "lib/a.dart", a, "x = 1", 0, "topo")),
        "Usage of top level variable \"topo\" declared in \"a.dart\" will be shadowed by renamed local variable."
    );
    // A top-level name renamed to the name of a local: Dart renames (the
    // declaration and the use).
    let r = rename_in(&mut p, "lib/a.dart", a, "topo = 0", 0, "x");
    assert_eq!(apply(&r["result"], &uri, a), a.replace("topo", "x"), "{r}");
    // A local that already exists in a nested block of the scope.
    assert_eq!(
        error(&rename_in(&mut p, "lib/a.dart", a, "c = 1", 0, "d")),
        "Duplicate local variable 'd'."
    );
    // A field with the name of a top-level declaration used in the class: renames.
    let r = rename_in(&mut p, "lib/a.dart", a, "campo = 1", 0, "valor");
    assert_eq!(
        apply(&r["result"], &uri, a),
        a.replace("int campo = 1", "int valor = 1")
            .replace("=> campo +", "=> valor +"),
        "{r}"
    );
    // A top-level name that would become the member inside the class.
    assert_eq!(
        error(&rename_in(&mut p, "lib/a.dart", a, "valor = 0", 0, "campo")),
        "Reference to renamed top level variable will be shadowed by getter 'campo'."
    );
}

#[test]
fn homonym_in_other_library_stays_intact() {
    let mut p = Project::new("renomear-homonimo");
    let a = "int f() => 1;\n";
    let b = "import 'a.dart';\nint g() => f();\n";
    let c = "int f() => 2;\nint h() => f();\n";
    p.write("lib/b.dart", b);
    p.write("lib/c.dart", c);
    p.open("lib/a.dart", a);
    let r = rename(&mut p, "lib/a.dart", 0, 4, "um");
    assert_eq!(
        apply(&r["result"], &p.uri("lib/a.dart"), a),
        "int um() => 1;\n"
    );
    assert_eq!(
        apply(&r["result"], &p.uri("lib/b.dart"), b),
        "import 'a.dart';\nint g() => um();\n"
    );
    assert!(
        r["result"]["changes"].get(p.uri("lib/c.dart")).is_none(),
        "{r}"
    );
}
