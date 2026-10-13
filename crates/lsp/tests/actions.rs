//! `textDocument/codeAction` through JSON-RPC messages: insert `;`, import
//! the library of an undefined name, the fixes for the published semantic
//! codes (immediate and typed ones of the current version), the type
//! annotation assist and the versioned edits.

mod common;

use common::{Project, apply};
use serde_json::{Value, json};

fn actions(p: &mut Project, relative: &str, from: (u32, u32), until: (u32, u32), extra: Value) -> Value {
    let mut params = json!({
        "textDocument": {"uri": p.uri(relative)},
        "range": {"start": {"line": from.0, "character": from.1}, "end": {"line": until.0, "character": until.1}},
        "context": {"diagnostics": []},
    });
    if let Value::Object(more) = extra {
        for (k, v) in more {
            params["context"][k] = v;
        }
    }
    let r = p.request("textDocument/codeAction", params)["result"].clone();
    // The source actions (always offered) and the creations for undefined
    // names have their own tests (`tests/fixes.rs`); they are left out here.
    let outside = |k: &str| {
        k.starts_with("source")
            || k.starts_with("quickfix.ignore")
            || (k.starts_with("refactor.") && !["refactor.inline", "refactor.add.typeAnnotation", "refactor.add.showCombinator", "refactor.convert.forEachToForIndex", "refactor.convert.conditionalToIfElse", "refactor.convert.toSingleQuotedString", "refactor.convert.toDoubleQuotedString", "refactor.convert.isNotEmpty", "refactor.convert.toIntLiteral", "refactor.replace.withVar", "refactor.splitIfConjunction", "refactor.add.returnType", "refactor.convert.toNullAware", "refactor.convert.toMultilineString", "refactor.convert.toSpread", "refactor.convert.toIfElement", "refactor.convert.blockComment", "refactor.convert.lineComment", "refactor.convert.relativeToPackageImport", "refactor.convert.packageToRelativeImport", "refactor.convert.partOfToPartUri", "refactor.convert.toMapLiteral", "refactor.convert.toSetLiteral", "refactor.encapsulateField", "refactor.convert.toConstructorFieldParameter", "refactor.shadowField", "refactor.convert.toGenericFunctionSyntax", "refactor.destructureLocalVariableAssignment", "refactor.convert.ifCaseStatement", "refactor.convert.ifCaseStatementChain", "refactor.convert.switchStatement", "refactor.convert.toForElement", "refactor.convert.toSuperParameters", "refactor.convert.classToEnum", "refactor.convert.switchExpression", "refactor.sort.child.properties.last", "refactor.add.diagnosticPropertyReference"].contains(&k) && !k.starts_with("refactor.surround.") && !k.starts_with("refactor.flutter."))
            || k == "quickfix.change.to"
            || ["method", "function", "class", "mixin", "getter", "field", "localVariable", "parameter"]
                .iter()
                .any(|s| k == format!("quickfix.create.{s}"))
    };
    match r {
        Value::Array(l) => Value::Array(l.into_iter().filter(|a| !outside(a["kind"].as_str().unwrap_or(""))).collect()),
        other => other,
    }
}

fn titles(r: &Value) -> Vec<String> {
    r.as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn announces_capability() {
    let p = Project::new("acoes-cap");
    let r = p.initialization.clone();
    assert_eq!(
        r["result"]["capabilities"]["codeActionProvider"],
        json!(true)
    );
}

#[test]
fn insert_semicolon_on_published_diagnostic() {
    let mut p = Project::with_literals("acoes-pv");
    let text = "void f() {\n  var x = 1\n  print(x);\n}\n";
    let published = p.open("lib/a.dart", text);
    let diagnostic = published["params"]["diagnostics"][0].clone();
    assert_eq!(diagnostic["code"], "expected_token");
    let r = actions(&mut p, "lib/a.dart", (1, 10), (1, 10), json!({}));
    assert_eq!(titles(&r), vec!["Insert ';'"]);
    assert_eq!(r[0]["kind"], "quickfix.insertSemicolon");
    // The action's diagnostic is the same one that was published.
    assert_eq!(r[0]["diagnostics"][0], diagnostic);
    assert_eq!(
        apply(&r[0]["edit"], &p.uri("lib/a.dart"), text),
        "void f() {\n  var x = 1;\n  print(x);\n}\n"
    );
    // Outside the diagnostic, nothing.
    assert_eq!(
        actions(&mut p, "lib/a.dart", (0, 0), (0, 3), json!({})),
        json!([])
    );
    // An `only` that does not include quickfix filters everything.
    assert_eq!(
        actions(
            &mut p,
            "lib/a.dart",
            (1, 10),
            (1, 10),
            json!({"only": ["refactor"]})
        ),
        json!([])
    );
    assert_eq!(
        titles(&actions(
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
fn import_library_from_sdk_and_project() {
    let mut p = Project::with_literals("acoes-importar");
    p.write(
        "lib/util/soma.dart",
        "class Soma {}\nint _privada() => 0;\n",
    );
    p.write("lib/parte.dart", "part of 'outro.dart';\nclass Soma {}\n");
    let text = "import 'dart:core';\n\nvoid f() {\n  var r = Random();\n  Soma? s;\n  print(pi);\n  _privada();\n}\n";
    p.open("lib/a.dart", text);
    let uri = p.uri("lib/a.dart");
    // Call to an undefined name: SDK (without `dart:_` libraries).
    // (The fixes apply to the diagnostics of the line, as in Dart: the unused
    // `r` on the same line also shows up.)
    let r = actions(&mut p, "lib/a.dart", (3, 11), (3, 11), json!({}));
    let import = action(&r, "Import library 'dart:math'");
    assert_eq!(titles(&r).iter().filter(|t| t.starts_with("Import")).count(), 1);
    assert_eq!(import["kind"], "quickfix.import.librarySdk");
    assert_eq!(
        apply(&import["edit"], &uri, text),
        "import 'dart:core';\nimport 'dart:math';\n\nvoid f() {\n  var r = Random();\n  Soma? s;\n  print(pi);\n  _privada();\n}\n"
    );
    // Undefined type: project library (a part does not count), relative.
    // (The unused `s` on the same line also gets its fix.)
    let r = actions(&mut p, "lib/a.dart", (4, 3), (4, 3), json!({}));
    // Both forms, like `dart language-server` 3.6.2.
    assert_eq!(
        titles(&r).into_iter().filter(|t| t.starts_with("Import")).collect::<Vec<_>>(),
        vec!["Import library 'package:projeto/util/soma.dart'", "Import library 'util/soma.dart'"]
    );
    let import = action(&r, "Import library 'util/soma.dart'");
    assert_eq!(import["kind"], "quickfix.import.libraryProject1");
    assert!(
        apply(&import["edit"], &uri, text)
            .starts_with("import 'dart:core';\n\nimport 'util/soma.dart';\n")
    );
    // SDK top-level variable declared in the library, not in a part.
    assert_eq!(
        titles(&actions(&mut p, "lib/a.dart", (5, 9), (5, 9), json!({}))),
        vec!["Import library 'dart:math'"]
    );
    // A private name is never importable. On a defined name on the same line,
    // the fixes are those of the line's diagnostics (the `pi`), as in Dart.
    assert_eq!(
        actions(&mut p, "lib/a.dart", (6, 4), (6, 4), json!({})),
        json!([])
    );
    assert_eq!(
        titles(&actions(&mut p, "lib/a.dart", (5, 4), (5, 4), json!({}))),
        vec!["Import library 'dart:math'"]
    );
}

#[test]
fn import_from_outside_lib_uses_package_and_without_directives_inserts_at_top() {
    let mut p = Project::with_literals("acoes-pacote");
    p.write("lib/soma.dart", "int somar(int a, int b) => a + b;\n");
    let text = "void main() {\n  print(somar(1, 2));\n}\n";
    p.open("bin/main.dart", text);
    let r = actions(&mut p, "bin/main.dart", (1, 9), (1, 9), json!({}));
    assert_eq!(
        titles(&r),
        vec!["Import library 'package:projeto/soma.dart'"]
    );
    assert_eq!(
        apply(&r[0]["edit"], &p.uri("bin/main.dart"), text),
        "import 'package:projeto/soma.dart';\n\nvoid main() {\n  print(somar(1, 2));\n}\n"
    );
    // Once imported, the action disappears.
    let with_import =
        "import 'package:projeto/soma.dart';\n\nvoid main() {\n  print(somar(1, 2));\n}\n";
    p.change("bin/main.dart", 2, with_import);
    assert_eq!(
        actions(&mut p, "bin/main.dart", (3, 9), (3, 9), json!({})),
        json!([])
    );
}

#[test]
fn incomplete_code_still_imports() {
    let mut p = Project::with_literals("acoes-incompleto");
    // The statement with `Random` is well formed; another, broken one does not
    // prevent it.
    let text = "void f() {\n  var r = Random();\n  r.\n}\n";
    p.open("lib/a.dart", text);
    let r = actions(&mut p, "lib/a.dart", (1, 11), (1, 11), json!({}));
    assert_eq!(titles(&r), vec!["Import library 'dart:math'"]);
}

/// UTF-16 line and column of the start of the `n`-th occurrence of `needle`.
fn location(text: &str, needle: &str, n: usize) -> (u32, u32) {
    let i = text.match_indices(needle).nth(n).unwrap_or_else(|| panic!("no {needle:?}")).0;
    let before = &text[..i];
    (
        before.matches('\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

/// Opens, waits for the typed publication and requests the actions at `needle`.
fn actions_at(p: &mut Project, rel: &str, text: &str, needle: &str) -> Value {
    p.open(rel, text);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let pos = location(text, needle, 0);
    actions(p, rel, pos, pos, json!({}))
}

/// The action titled `title`.
fn action<'a>(r: &'a Value, title: &str) -> &'a Value {
    r.as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == title)
        .unwrap_or_else(|| panic!("no {title:?} in {:?}", titles(r)))
}

#[test]
fn removes_unused_local_and_its_assignments() {
    let mut p = Project::with_literals("acoes-local");
    let text = "void f() {\n  var x = 1;\n  var y = 2;\n  x = 3;\n  print(y);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "x = 1");
    let a = action(&r, "Remove unused local variable");
    assert_eq!(a["kind"], "quickfix.remove.unusedLocalVariable");
    assert_eq!(a["diagnostics"][0]["code"], "unused_local_variable");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "void f() {\n  var y = 2;\n  print(y);\n}\n"
    );
    // In a list, only the variable.
    let text = "void f() {\n  var a = 1, b = 2;\n  var c = 3, d = 4;\n  print(a + d);\n}\n";
    let r = actions_at(&mut p, "lib/b.dart", text, "b = 2");
    assert_eq!(
        apply(&action(&r, "Remove unused local variable")["edit"], &p.uri("lib/b.dart"), text),
        "void f() {\n  var a = 1;\n  var c = 3, d = 4;\n  print(a + d);\n}\n"
    );
    let pos = location(text, "c = 3", 0);
    let r = actions(&mut p, "lib/b.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Remove unused local variable")["edit"], &p.uri("lib/b.dart"), text),
        "void f() {\n  var a = 1, b = 2;\n  var d = 4;\n  print(a + d);\n}\n"
    );
    // Used: nothing to remove from it (the unused `b` on the same line gets its
    // fix, as in Dart, which fixes the diagnostics of the line).
    let pos = location(text, "a = 1", 0);
    let r = actions(&mut p, "lib/b.dart", pos, pos, json!({}));
    assert!(r.as_array().unwrap().iter().filter(|a| a["title"] == "Remove unused local variable").all(|a| a["diagnostics"][0]["range"]["start"]["character"] != pos.1));
}

#[test]
fn removes_unused_local_function() {
    let mut p = Project::with_literals("acoes-elemento");
    let text = "void f() {\n  int g() => 1;\n  print(2);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "g()");
    let a = action(&r, "Remove unused element");
    assert_eq!(a["kind"], "quickfix.remove.unusedElement");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), "void f() {\n  print(2);\n}\n");
}

#[test]
fn fixes_for_published_typed_diagnostics() {
    let mut p = Project::with_literals("acoes-tipados");
    let cases: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "lib/cast.dart",
            "void f(int x) {\n  print((x as int).isEven);\n}\n",
            "as int",
            "Remove unnecessary cast",
            "quickfix.remove.unnecessaryCast",
            // Only the ` as int` goes away; the parentheses stay (Dart 3.6.2 server).
            "void f(int x) {\n  print((x).isEven);\n}\n",
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
    for &(rel, text, needle, title, kind, expected) in cases {
        let r = actions_at(&mut p, rel, text, needle);
        let a = action(&r, title);
        assert_eq!(a["kind"], kind, "{rel}");
        assert!(a["diagnostics"][0]["code"].is_string(), "{a}");
        assert_eq!(apply(&a["edit"], &p.uri(rel), text), expected, "{rel}");
    }
}

#[test]
fn stale_version_typed_diagnostic_produces_no_fix() {
    let mut p = Project::with_literals("acoes-versao");
    let text = "void f(int x) {\n  print((x as int).isEven);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "as int");
    assert!(titles(&r).contains(&"Remove unnecessary cast".to_string()));
    // New version, with the typed analysis suspended: the typed publication of
    // version 1 does not apply to version 2, even with the same text in place.
    p.server.pause_typed_analysis(true);
    p.change("lib/a.dart", 2, &format!("{text}// fim\n"));
    let pos = location(text, "as int", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert!(!titles(&r).contains(&"Remove unnecessary cast".to_string()), "{r}");
    // Once resumed, the publication of version 2 applies again.
    p.server.pause_typed_analysis(false);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert!(titles(&r).contains(&"Remove unnecessary cast".to_string()), "{r}");
    // Outside the diagnostic's range, nothing.
    assert!(!titles(&actions(&mut p, "lib/a.dart", (0, 0), (0, 4), json!({}))).contains(&"Remove unnecessary cast".to_string()));
}

#[test]
fn versioned_edit_when_client_accepts() {
    let mut p = Project::with_literals("acoes-versionada");
    p.restart(json!({"capabilities": {
        "workspace": {"workspaceEdit": {"documentChanges": true}},
        "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}
    }}));
    let text = "void f() {\n  var x = 1;\n}\n";
    p.open("lib/a.dart", text);
    p.change("lib/a.dart", 5, text);
    let pos = location(text, "x = 1", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    let a = action(&r, "Remove unused local variable");
    let changes = a["edit"]["documentChanges"].as_array().unwrap_or_else(|| panic!("{a}"));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["textDocument"], json!({"uri": p.uri("lib/a.dart"), "version": 5}));
}

#[test]
fn type_annotation_assist() {
    let mut p = Project::with_literals("acoes-tipo");
    let text = "void f() {\n  var nome = 'x';\n  final n = 1;\n  var d;\n  var lista = <int>[n];\n  print([nome, n, d, lista]);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "nome =");
    let a = action(&r, "Add type annotation");
    assert_eq!(a["kind"], "refactor.add.typeAnnotation");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        text.replace("var nome", "String nome")
    );
    let pos = location(text, "final n", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Add type annotation")["edit"], &p.uri("lib/a.dart"), text), text.replace("final n", "final int n"));
    let pos = location(text, "lista =", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Add type annotation")["edit"], &p.uri("lib/a.dart"), text), text.replace("var lista", "List<int> lista"));
    // `dynamic` is not annotated; nor in the initializer.
    let pos = location(text, "d;", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Add type annotation".to_string()));
    let pos = location(text, "'x'", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Add type annotation".to_string()));
    // `context.only` with `quickfix` excludes the assist; with `refactor`, only it.
    let pos = location(text, "nome =", 0);
    assert!(titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({"only": ["quickfix"]}))).iter().all(|t| t != "Add type annotation"));
    assert_eq!(titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({"only": ["refactor"]}))), vec!["Add type annotation"]);
}

/// L06: the Dart 3.6.2 fixes for more published codes, with the titles and
/// edits of `analysis_server`.
#[test]
fn fixes_for_final_field_abstract_condition_and_element() {
    let mut p = Project::with_literals("acoes-l06");
    let cases: &[(&str, &str, &str, &str, &str, &str)] = &[
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
    for &(rel, text, needle, title, kind, expected) in cases {
        let r = actions_at(&mut p, rel, text, needle);
        let a = action(&r, title);
        assert_eq!(a["kind"], kind, "{rel}");
        assert!(a["diagnostics"][0]["code"].is_string(), "{a}");
        assert_eq!(apply(&a["edit"], &p.uri(rel), text), expected, "{rel}");
    }
    // A `final` field that is written without being a field (local): no field.
    let text = "void f() {\n  final int w = 1;\n  print(w);\n}\n";
    let r = actions_at(&mut p, "lib/local.dart", text, "w = 1");
    assert!(!titles(&r).iter().any(|t| t.starts_with("Make field")), "{r}");
}

/// L06: `Create file` for the relative URI that does not exist, only for a
/// client that accepts the `create` operation; the part is born with the
/// `part of`.
#[test]
fn create_file_for_missing_uri() {
    let capabilities = json!({
        "workspace": {"workspaceEdit": {"documentChanges": true, "resourceOperations": ["create"]}},
        "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}
    });
    let mut p = Project::with_capabilities("acoes-criar", capabilities);
    let text = "import 'novo.dart';\npart 'parte.dart';\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "'novo.dart'");
    let a = action(&r, "Create file 'novo.dart'");
    assert_eq!(a["kind"], "quickfix.create.file");
    let changes = a["edit"]["documentChanges"].as_array().expect("documentChanges");
    // Like `dart language-server` 3.6.2: the `create` (without options) and the
    // content in a separate edit.
    assert_eq!(changes[0], json!({"kind": "create", "uri": p.uri("lib/novo.dart")}));
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert_eq!(changes[1]["edits"][0]["newText"], "// TODO Implement this library.");
    let pos = common_pos(text, "'parte.dart'");
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    let a = action(&r, "Create file 'parte.dart'");
    let changes = a["edit"]["documentChanges"].as_array().expect("documentChanges");
    assert_eq!(changes[0]["uri"], p.uri("lib/parte.dart"));
    assert_eq!(changes[1]["edits"][0]["newText"], "part of 'a.dart';\n\n");

    // Without the `create` operation in the client, the fix is not offered.
    let mut p = Project::with_literals("actions-create-without");
    let r = actions_at(&mut p, "lib/a.dart", text, "'novo.dart'");
    assert!(!titles(&r).iter().any(|t| t.starts_with("Create file")), "{r}");
}

fn common_pos(text: &str, needle: &str) -> (u32, u32) {
    location(text, needle, 0)
}

#[test]
fn show_combinator_assist_on_import() {
    let mut p = Project::with_literals("acoes-show");
    p.write("lib/b.dart", "int um = 1;\nclass B {}\nvoid g() {}\nextension E on int {\n  int get dobro => this * 2;\n}\nvoid naoUsada() {}\n");
    let text = "import 'b.dart';\n\nvoid f() {\n  B();\n  g();\n  print(um + 1.dobro);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "b.dart'");
    let a = action(&r, "Add explicit 'show' combinator");
    assert_eq!(a["kind"], "refactor.add.showCombinator");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("'b.dart';", "'b.dart' show B, E, g, um;"));
}

#[test]
fn for_index_assist() {
    let mut p = Project::with_literals("acoes-for-indice");
    let text = "void f(List<String> nomes) {
  for (final nome in nomes) {
    print(nome);
  }
}
";
    let r = actions_at(&mut p, "lib/a.dart", text, "for (");
    let a = action(&r, "Convert to for-index loop");
    assert_eq!(a["kind"], "refactor.convert.forEachToForIndex");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "void f(List<String> nomes) {
  for (int i = 0; i < nomes.length; i++) {
    final nome = nomes[i];
    print(nome);
  }
}
"
    );
    // Inside the body, outside the header, no.
    let pos = location(text, "print", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Convert to for-index loop".to_string()));
}

#[test]
fn conditional_to_if_else_assist() {
    let mut p = Project::with_literals("acoes-if-else");
    let text = "int f(bool c) {
  var x = c ? 1 : 2;
  x = c ? 3 : 4;
  return c ? x : 5;
}
";
    let r = actions_at(&mut p, "lib/a.dart", text, "var x");
    let a = action(&r, "Replace conditional with 'if-else'");
    assert_eq!(a["kind"], "refactor.convert.conditionalToIfElse");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "int f(bool c) {
  int x;
  if (c) {
    x = 1;
  } else {
    x = 2;
  }
  x = c ? 3 : 4;
  return c ? x : 5;
}
"
    );
    let pos = location(text, "x = c ? 3", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Replace conditional with 'if-else'")["edit"], &p.uri("lib/a.dart"), text),
        "int f(bool c) {
  var x = c ? 1 : 2;
  if (c) {
    x = 3;
  } else {
    x = 4;
  }
  return c ? x : 5;
}
"
    );
    let pos = location(text, "return", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Replace conditional with 'if-else'")["edit"], &p.uri("lib/a.dart"), text),
        "int f(bool c) {
  var x = c ? 1 : 2;
  x = c ? 3 : 4;
  if (c) {
    return x;
  } else {
    return 5;
  }
}
"
    );
}

#[test]
fn quotes_assist() {
    let mut p = Project::with_literals("acoes-aspas");
    let text = "void f(int n) {\n  print('it\\'s \"x\"');\n  print(\"a $n b\");\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "it");
    let a = action(&r, "Convert to double quoted string");
    assert_eq!(a["kind"], "refactor.convert.toDoubleQuotedString");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("'it\\'s \"x\"'", "\"it's \\\"x\\\"\""));
    assert!(!titles(&r).contains(&"Convert to single quoted string".to_string()));
    let pos = location(text, "a $n", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to single quoted string")["edit"], &p.uri("lib/a.dart"), text), text.replace("\"a $n b\"", "'a $n b'"));
}

#[test]
fn is_not_empty_and_int_literal_assists() {
    let mut p = Project::with_literals("acoes-isnotempty");
    let text = "class C {\n  bool get isEmpty => true;\n  bool get isNotEmpty => false;\n}\nclass D {\n  bool get isEmpty => true;\n}\nvoid f(C c, D d) {\n  print(!c.isEmpty);\n  print(!d.isEmpty);\n  print(1_000.0 + 2e3 + 1.5);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "isEmpty);\n  print(!d");
    let a = action(&r, "Convert to 'isNotEmpty'");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("!c.isEmpty", "c.isNotEmpty"));
    let pos = location(text, "isEmpty);\n  print(1", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Convert to 'isNotEmpty'".to_string()));
    let pos = location(text, "1_000.0", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to an int literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("1_000.0", "1_000"));
    let pos = location(text, "2e3", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to an int literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("2e3", "2000"));
    let pos = location(text, "1.5", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Convert to an int literal".to_string()));
}

#[test]
fn replace_with_var_assist() {
    let mut p = Project::with_literals("acoes-var");
    let text = "void f(List<int> l) {\n  List<int> a = [1];\n  final String s = 'x';\n  for (int x in l) {\n    print([a, s, x]);\n  }\n  num n = 1;\n  print(n);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "List<int> a");
    let a = action(&r, "Replace type annotation with 'var'");
    assert_eq!(a["kind"], "refactor.replace.withVar");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("List<int> a = [1]", "var a = <int>[1]"));
    let pos = location(text, "String s", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Replace type annotation with 'var'")["edit"], &p.uri("lib/a.dart"), text), text.replace("final String s", "final s"));
    let pos = location(text, "int x", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Replace type annotation with 'var'")["edit"], &p.uri("lib/a.dart"), text), text.replace("int x in", "var x in"));
    // `num` with an `int` initializer: the types differ.
    let pos = location(text, "num n", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Replace type annotation with 'var'".to_string()));
}

#[test]
fn split_and_condition_assist() {
    let mut p = Project::with_literals("acoes-split-and");
    let text = "void f(bool a, bool b) {\n  if (a && b) {\n    print(1);\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "&&");
    let a = action(&r, "Split && condition");
    assert_eq!(a["kind"], "refactor.splitIfConjunction");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "void f(bool a, bool b) {\n  if (a) {\n    if (b) {\n      print(1);\n    }\n  }\n}\n"
    );
    // Outside the operator, no.
    let pos = location(text, "a &&", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Split && condition".to_string()));
}

#[test]
fn return_type_assist() {
    let mut p = Project::with_literals("acoes-retorno");
    let text = "f() => 1;\nclass C {\n  get g => 'x';\n}\nh(bool b) async {\n  if (b) return 1;\n  return 2;\n}\nk() {}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "f()");
    let a = action(&r, "Add return type");
    assert_eq!(a["kind"], "refactor.add.returnType");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("f() =>", "int f() =>"));
    let pos = location(text, "g =>", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Add return type")["edit"], &p.uri("lib/a.dart"), text), text.replace("get g", "String get g"));
    let pos = location(text, "h(bool", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Add return type")["edit"], &p.uri("lib/a.dart"), text), text.replace("h(bool b)", "Future<int> h(bool b)"));
    let pos = location(text, "k()", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Add return type".to_string()));
}

#[test]
fn null_aware_assist() {
    let mut p = Project::with_literals("acoes-null-aware");
    let text = "class A {\n  int get n => 1;\n  int m() => 2;\n}\nvoid f(A? a) {\n  print(a == null ? null : a.n);\n  print(a != null ? a.m() : null);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "? null");
    let a = action(&r, "Convert to use '?.'");
    assert_eq!(a["kind"], "refactor.convert.toNullAware");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("a == null ? null : a.n", "a?.n"));
    let pos = location(text, "? a.m", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to use '?.'")["edit"], &p.uri("lib/a.dart"), text), text.replace("a != null ? a.m() : null", "a?.m()"));
}

#[test]
fn multiline_and_spread_assists() {
    let mut p = Project::with_literals("acoes-spread");
    let text = "void f(List<int> a, List<int>? b) {\n  print('ab');\n  print([1]..addAll(a));\n  print(<int>[]..addAll(b ?? []));\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "ab'");
    let a = action(&r, "Convert to multiline string");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("'ab'", "'''\nab'''"));
    let pos = location(text, "addAll(a)", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to a spread")["edit"], &p.uri("lib/a.dart"), text), text.replace("[1]..addAll(a)", "[1, ...a]"));
    let pos = location(text, "addAll(b", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to a spread")["edit"], &p.uri("lib/a.dart"), text), text.replace("<int>[]..addAll(b ?? [])", "<int>[...?b]"));
}

#[test]
fn element_and_documentation_assists() {
    let mut p = Project::with_literals("acoes-doc");
    let text = "/// Um.\n/// Dois.\nvoid f(bool c) {\n  print([1]..add(2));\n  print([c ? 1 : 2]);\n}\n\n/** Tres. */\nvoid g() {}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "Um.");
    let a = action(&r, "Convert to block documentation comment");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("/// Um.\n/// Dois.", "/**\n * Um.\n * Dois.\n */"));
    let pos = location(text, "Tres", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to line documentation comment")["edit"], &p.uri("lib/a.dart"), text), text.replace("/** Tres. */", "/// Tres."));
    let pos = location(text, "add(2", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    let a = action(&r, "Inline invocation of 'add'");
    assert_eq!(a["kind"], "refactor.inline");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("[1]..add(2)", "[1, 2]"));
    let pos = location(text, "? 1", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to an 'if' element")["edit"], &p.uri("lib/a.dart"), text), text.replace("[c ? 1 : 2]", "[if (c) 1 else 2]"));
}

#[test]
fn import_uri_assists() {
    let mut p = Project::with_literals("acoes-uris");
    p.write("lib/src/b.dart", "int b = 1;\n");
    p.write("lib/c.dart", "int c = 2;\n");
    let text = "import 'src/b.dart';\nimport 'package:projeto/c.dart';\n\nvoid f() => print([b, c]);\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "src/b");
    let a = action(&r, "Convert to 'package:' import");
    assert_eq!(a["kind"], "refactor.convert.relativeToPackageImport");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("'src/b.dart'", "'package:projeto/src/b.dart'"));
    assert!(!titles(&r).contains(&"Convert to a relative import".to_string()));
    let pos = location(text, "projeto/c", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to a relative import")["edit"], &p.uri("lib/a.dart"), text), text.replace("'package:projeto/c.dart'", "'c.dart'"));
    assert!(!titles(&r).contains(&"Convert to 'package:' import".to_string()));
}

#[test]
fn map_literal_assist() {
    let mut p = Project::with_literals("acoes-mapa");
    let text = "void f() {\n  var a = Map<String, int>();\n  Map<int, int> b = Map();\n  print([a, b]);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "Map<String");
    let a = action(&r, "Convert to map literal");
    assert_eq!(a["kind"], "refactor.convert.toMapLiteral");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("Map<String, int>()", "<String, int>{}"));
    let pos = location(text, "Map();", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to map literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("= Map();", "= {};"));
}

#[test]
fn set_literal_assist() {
    let mut p = Project::with_literals("acoes-conjunto");
    let text = "void f() {\n  var a = Set<int>();\n  var b = Set.of([1, 2]);\n  Set<int> c = Set();\n  var d = Set();\n  print([a, b, c, d]);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "Set<int>()");
    let a = action(&r, "Convert to set literal");
    assert_eq!(a["kind"], "refactor.convert.toSetLiteral");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("Set<int>()", "<int>{}"));
    let pos = location(text, "Set.of", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to set literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("Set.of([1, 2])", "{1, 2}"));
    let pos = location(text, "Set();\n  var d", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to set literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("Set<int> c = Set();", "Set<int> c = {};"));
    let pos = location(text, "Set();\n  print", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert to set literal")["edit"], &p.uri("lib/a.dart"), text), text.replace("var d = Set();", "var d = <dynamic>{};"));
}

#[test]
fn encapsulate_field_assist() {
    let mut p = Project::with_literals("acoes-encapsular");
    let text = "class A {\n  int x = 0;\n  A(this.x);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "x = 0");
    let a = action(&r, "Encapsulate field");
    assert_eq!(a["kind"], "refactor.encapsulateField");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "class A {\n  int _x = 0;\n\n  int get x => _x;\n\n  set x(int value) {\n    _x = value;\n  }\n  A(this._x);\n}\n"
    );
}

#[test]
fn field_parameter_and_shadow_assists() {
    let mut p = Project::with_literals("acoes-campo");
    let text = "class A {\n  int x;\n  num? y;\n  A(int v) : x = v;\n  void f() {\n    if (y is int) {\n      print(y);\n    }\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "v) :");
    let a = action(&r, "Convert to field formal parameter");
    assert_eq!(a["kind"], "refactor.convert.toConstructorFieldParameter");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("A(int v) : x = v;", "A(this.x);"));
    let pos = location(text, "y is", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Create a local variable that shadows the field")["edit"], &p.uri("lib/a.dart"), text),
        text.replace("    if (y is int)", "    var y = this.y;\n    if (y is int)")
    );
}

#[test]
fn function_syntax_assist() {
    let mut p = Project::with_literals("acoes-funcao");
    let text = "typedef int F<T>(T x);\nvoid g(int h(String s), void k(x)) {}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "F<T>");
    let a = action(&r, "Convert into 'Function' syntax");
    assert_eq!(a["kind"], "refactor.convert.toGenericFunctionSyntax");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("typedef int F<T>(T x);", "typedef F<T> = int Function(T x);"));
    let pos = location(text, "h(String", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(apply(&action(&r, "Convert into 'Function' syntax")["edit"], &p.uri("lib/a.dart"), text), text.replace("int h(String s)", "int Function(String s) h"));
    // `k(x)`: the untyped parameter prevents it.
    let pos = location(text, "k(x)", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).contains(&"Convert into 'Function' syntax".to_string()));
}

#[test]
fn surround_with_assist() {
    let mut p = Project::with_literals("acoes-envolver");
    let text = "void f() {\n  print(1);\n  print(2);\n}\n";
    p.open("lib/a.dart", text);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let from = location(text, "print(1)", 0);
    let until = location(text, "\n}", 0);
    let r = actions(&mut p, "lib/a.dart", from, until, json!({}));
    let a = action(&r, "Surround with 'if'");
    assert_eq!(a["kind"], "refactor.surround.if");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "void f() {\n  if (condition) {\n    print(1);\n    print(2);\n  }\n}\n"
    );
    assert_eq!(
        apply(&action(&r, "Surround with 'try-catch'")["edit"], &p.uri("lib/a.dart"), text),
        "void f() {\n  try {\n    print(1);\n    print(2);\n  } on Exception catch (e) {\n    // TODO\n  }\n}\n"
    );
    assert!(!titles(&r).contains(&"Surround with 'setState'".to_string()));
    // Without a selection, nothing.
    let pos = location(text, "print(1)", 0);
    assert!(!titles(&actions(&mut p, "lib/a.dart", pos, pos, json!({}))).iter().any(|t| t.starts_with("Surround with")));
}

#[test]
fn destructure_assist() {
    let mut p = Project::with_literals("acoes-desestruturar");
    let text = "class P {\n  int x = 1;\n  int y = 2;\n}\nvoid f((int, {String n}) r, P q) {\n  var a = r;\n  var b = q;\n  print(b.x + b.y);\n  print(a);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "a = r");
    let a = action(&r, "Destructure variable assignment");
    assert_eq!(a["kind"], "refactor.destructureLocalVariableAssignment");
    assert_eq!(apply(&a["edit"], &p.uri("lib/a.dart"), text), text.replace("var a = r;", "var ($1, :n) = r;"));
    let pos = location(text, "b = q", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Destructure variable assignment")["edit"], &p.uri("lib/a.dart"), text),
        text.replace("var b = q;", "var P(:x, :y) = q;").replace("print(b.x + b.y);", "print(x + y);")
    );
}

#[test]
fn if_case_assist() {
    let mut p = Project::with_literals("acoes-if-case");
    let text = "Object? g() => null;\nvoid f() {\n  final v = g();\n  if (v is int) {\n    print(v);\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "if (v");
    let a = action(&r, "Convert to 'if-case' statement");
    assert_eq!(a["kind"], "refactor.convert.ifCaseStatement");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "Object? g() => null;\nvoid f() {\n  if (g() case final int v) {\n    print(v);\n  }\n}\n"
    );
}

#[test]
fn if_case_chain_assist() {
    let mut p = Project::with_literals("acoes-cadeia");
    let text = "void f(Object o) {\n  switch (o) {\n    case int x when x > 0:\n      print(x);\n    case String _:\n    case double _:\n      print(1);\n      break;\n    default:\n      print(2);\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "switch");
    let a = action(&r, "Convert to 'if-case' statement chain");
    assert_eq!(a["kind"], "refactor.convert.ifCaseStatementChain");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "void f(Object o) {\n  if (o case int x when x > 0) {\n    print(x);\n  } else if (o case String _ || double _) {\n    print(1);\n  } else {\n    print(2);\n  }\n}\n"
    );
}

#[test]
fn switch_statement_assist() {
    let mut p = Project::with_literals("acoes-switch");
    let text = "int f(Object o, int n) {\n  if (o is int) {\n    print(1);\n  } else if (o is String) {\n    print(2);\n  } else {\n    print(3);\n  }\n  return switch (n) {\n    1 => 10,\n    _ => 20,\n  };\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "if (o");
    let a = action(&r, "Convert to switch statement");
    assert_eq!(a["kind"], "refactor.convert.switchStatement");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        text.replace(
            "if (o is int) {\n    print(1);\n  } else if (o is String) {\n    print(2);\n  } else {\n    print(3);\n  }",
            "switch (o) {\n    case int():\n      print(1);\n    case String():\n      print(2);\n    default:\n      print(3);\n  }"
        )
    );
    let pos = location(text, "switch (n)", 0);
    let r = actions(&mut p, "lib/a.dart", pos, pos, json!({}));
    assert_eq!(
        apply(&action(&r, "Convert to switch statement")["edit"], &p.uri("lib/a.dart"), text),
        text.replace("  return switch (n) {\n    1 => 10,\n    _ => 20,\n  };", "  switch (n) {\n    case 1:\n      return 10;\n    default:\n      return 20;\n  }")
    );
}

#[test]
fn for_to_map_assist() {
    let mut p = Project::with_literals("acoes-for-mapa");
    let text = "void f(List<int> l) {\n  print(Map<int, int>.fromIterable(l, key: (k) => k, value: (v) => v + 1));\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "fromIterable");
    let a = action(&r, "Convert to a 'for' element");
    assert_eq!(a["kind"], "refactor.convert.toForElement");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        text.replace("Map<int, int>.fromIterable(l, key: (k) => k, value: (v) => v + 1)", "{ for (var e in l) e : e + 1 }")
    );
}

#[test]
fn super_parameters_assist() {
    let mut p = Project::with_literals("acoes-super");
    let text = "class A {\n  A(int x, {int y = 0});\n}\nclass B extends A {\n  B(int x, {int y = 0}) : super(x, y: y);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "B(int");
    let a = action(&r, "Convert to using super parameters");
    assert_eq!(a["kind"], "refactor.convert.toSuperParameters");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        text.replace("B(int x, {int y = 0}) : super(x, y: y);", "B(super.x, {super.y});")
    );
}

#[test]
fn class_to_enum_assist() {
    let mut p = Project::with_literals("acoes-enum");
    let text = "class _E {\n  static const _E c0 = _E(0, 'a');\n  static const _E c1 = _E(1, 'b');\n\n  final int index;\n\n  final String code;\n\n  const _E(this.index, this.code);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "_E {");
    let a = action(&r, "Convert class to an enum");
    assert_eq!(a["kind"], "refactor.convert.classToEnum");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "enum _E {\n  c0('a'),\n  c1('b');\n\n  final String code;\n\n  const _E(this.code);\n}\n"
    );
    let text = "class _E {\n  static const _E c0 = _E();\n  static const _E c1 = _E();\n}\nvoid f() {}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "_E {");
    assert!(r.as_array().unwrap().iter().all(|a| a["title"] != "Convert class to an enum"));
    let text = "class _E {\n  static const a = _E._('a');\n  static const b = _E._('b');\n  final String s;\n  const _E._(this.s);\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "_E {");
    assert_eq!(
        apply(&action(&r, "Convert class to an enum")["edit"], &p.uri("lib/a.dart"), text),
        "enum _E {\n  a._('a'),\n  b._('b');\n\n  final String s;\n  const _E._(this.s);\n}\n"
    );
}

#[test]
fn switch_expression_assist() {
    let mut p = Project::with_literals("acoes-switch-expr");
    let text = "int f(bool b) {\n  switch (b) {\n    case true:\n      return 1;\n    case false:\n      return 0;\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "switch (b)");
    let a = action(&r, "Convert to switch expression");
    assert_eq!(a["kind"], "refactor.convert.switchExpression");
    assert_eq!(
        apply(&a["edit"], &p.uri("lib/a.dart"), text),
        "int f(bool b) {\n  return switch (b) {\n    true => 1,\n    false => 0\n  };\n}\n"
    );
    let text = "void f(String s) {\n  switch (s) {\n    case 'foo':\n      print('foo');\n    case 'bar':\n      print('bar');\n    default:\n      throw 'unrecognized';\n  }\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "(s)");
    assert_eq!(
        apply(&action(&r, "Convert to switch expression")["edit"], &p.uri("lib/a.dart"), text),
        "void f(String s) {\n  print(switch (s) {\n    'foo' => 'foo',\n    'bar' => 'bar',\n    _ => throw 'unrecognized'\n  });\n}\n"
    );
    let text = "String f(String s) {\n  var name = '';\n  switch (s) {\n    case 'foo':\n      name = 'foo';\n    case 'bar':\n      name = 'bar';\n    default:\n      throw 'unrecognized';\n  }\n  return name;\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "(s)");
    assert_eq!(
        apply(&action(&r, "Convert to switch expression")["edit"], &p.uri("lib/a.dart"), text),
        "String f(String s) {\n  var name = '';\n  name = switch (s) {\n    'foo' => 'foo',\n    'bar' => 'bar',\n    _ => throw 'unrecognized'\n  };\n  return name;\n}\n"
    );
    let text = "int f(String name) {\n  switch (name) {\n    case 'red':\n      return 1;\n  }\n  throw 'Only'\n    ' supports'\n    ' red';\n}\n";
    let r = actions_at(&mut p, "lib/a.dart", text, "switch (name)");
    assert_eq!(
        apply(&action(&r, "Convert to switch expression")["edit"], &p.uri("lib/a.dart"), text),
        "int f(String name) {\n  return switch (name) {\n    'red' => 1,\n    _ => throw 'Only'\n      ' supports'\n      ' red',\n  };\n}\n"
    );
}

#[test]
fn flutter_assists_() {
    let mut p = Project::with_literals("acoes-flutter");
    p.install_flutter();
    let text = "import 'package:flutter/widgets.dart';\n\nclass A extends StatelessWidget {\n  const A({super.key});\n  @override\n  Widget build(BuildContext context) {\n    return Center(child: Text('a'));\n  }\n}\n";
    let uri = p.uri("lib/a.dart");
    // On `Text`: the wrappers.
    let r = actions_at(&mut p, "lib/a.dart", text, "Text(");
    assert_eq!(apply(&action(&r, "Wrap with Center")["edit"], &uri, text), text.replace("child: Text('a')", "child: Center(child: Text('a'))"));
    assert_eq!(
        apply(&action(&r, "Wrap with Padding")["edit"], &uri, text),
        text.replace("child: Text('a')", "child: Padding(\n      padding: const EdgeInsets.all(8.0),\n      child: Text('a'),\n    )")
    );
    assert_eq!(action(&r, "Wrap with widget...")["kind"], "refactor.flutter.wrap.generic");
    action(&r, "Wrap with Column");
    action(&r, "Wrap with Builder");
    // On `Center`: remove (the child stays in place).
    let r = actions_at(&mut p, "lib/a.dart", text, "Center(");
    assert_eq!(apply(&action(&r, "Remove this widget")["edit"], &uri, text), text.replace("Center(child: Text('a'))", "Text('a')"));
    // On the `child` label: `children: [...]`.
    let r = actions_at(&mut p, "lib/a.dart", text, "child:");
    assert_eq!(apply(&action(&r, "Convert to children:")["edit"], &uri, text), text.replace("child: Text('a')", "children: [Text('a')]"));
    // Move within the `children` list.
    let list = "import 'package:flutter/widgets.dart';\n\nWidget f() => Column(children: [Text('a'), Text('b')]);\n";
    let uri2 = p.uri("lib/b.dart");
    let r = actions_at(&mut p, "lib/b.dart", list, "Text('b')");
    assert_eq!(apply(&action(&r, "Move widget up")["edit"], &uri2, list), list.replace("[Text('a'), Text('b')]", "[Text('b'), Text('a')]"));
    assert!(r.as_array().unwrap().iter().all(|a| a["title"] != "Move widget down"));
}

#[test]
fn flutter_to_stateful() {
    let mut p = Project::with_literals("acoes-flutter-stateful");
    p.install_flutter();
    let text = "import 'package:flutter/widgets.dart';\n\nclass A extends StatelessWidget {\n  const A({super.key, required this.titulo});\n\n  final String titulo;\n\n  @override\n  Widget build(BuildContext context) {\n    return Text(titulo);\n  }\n}\n";
    let uri = p.uri("lib/a.dart");
    let r = actions_at(&mut p, "lib/a.dart", text, "A extends");
    let a = action(&r, "Convert to StatefulWidget");
    assert_eq!(a["kind"], "refactor.flutter.convert.toStatefulWidget");
    assert_eq!(
        apply(&a["edit"], &uri, text),
        "import 'package:flutter/widgets.dart';\n\nclass A extends StatefulWidget {\n  const A({super.key, required this.titulo});\n\n  final String titulo;\n\n  @override\n  State<A> createState() => _AState();\n}\n\nclass _AState extends State<A> {\n  @override\n  Widget build(BuildContext context) {\n    return Text(widget.titulo);\n  }\n}\n"
    );
}

/// The cases of 3.6.2's `flutter_convert_to_stateful_widget_test.dart`.
#[test]
fn flutter_to_stateful_like_analysis_server() {
    let mut p = Project::with_literals("acoes-flutter-stateful-2");
    p.install_flutter();
    let cases: &[(&str, &str, &str)] = &[
        ("test_simple", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  final String aaa;
  final String bbb;

  const MyWidget(this.aaa, this.bbb);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(aaa),
        Text(bbb),
        Text('$aaa'),
        Text('${bbb}'),
      ],
    );
  }
}
"#, r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  final String aaa;
  final String bbb;

  const MyWidget(this.aaa, this.bbb);

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.aaa),
        Text(widget.bbb),
        Text('${widget.aaa}'),
        Text('${widget.bbb}'),
      ],
    );
  }
}
"#),
        ("test_fields", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  static String staticField2 = '';
  String instanceField4 = '';
  String instanceField5 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1) : instanceField2 = '' {
    instanceField3 = '';
  }

  @override
  Widget build(BuildContext context) {
    instanceField4 = instanceField1;
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(instanceField3),
        Text(instanceField4),
        Text(instanceField5),
        Text(staticField1),
        Text(staticField2),
        Text(staticField3),
      ],
    );
  }
}
"#, r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  static String staticField2 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1) : instanceField2 = '' {
    instanceField3 = '';
  }

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField4 = '';

  String instanceField5 = '';

  @override
  Widget build(BuildContext context) {
    instanceField4 = widget.instanceField1;
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(widget.instanceField2),
        Text(widget.instanceField3),
        Text(instanceField4),
        Text(instanceField5),
        Text(MyWidget.staticField1),
        Text(MyWidget.staticField2),
        Text(MyWidget.staticField3),
      ],
    );
  }
}
"#),
        ("test_getters", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(staticGetter1),
        Text(staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  static String get staticGetter1 => '';

  String get instanceGetter1 => '';

  static String get staticGetter2 => '';

  String get instanceGetter2 => '';
}
"#, r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();

  static String get staticGetter1 => '';

  static String get staticGetter2 => '';
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(MyWidget.staticGetter1),
        Text(MyWidget.staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  String get instanceGetter1 => '';

  String get instanceGetter2 => '';
}
"#),
        ("test_methods", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField = '';
  final String instanceField1;
  String instanceField2 = '';

  MyWidget(this.instanceField1);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    staticMethod1();
  }

  static void staticMethod1() {
    print('static 1');
  }

  void instanceMethod2() {
    print('instance 2');
  }

  static void staticMethod2() {
    print('static 2');
  }
}
"#, r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField = '';
  final String instanceField1;

  MyWidget(this.instanceField1);

  @override
  State<MyWidget> createState() => _MyWidgetState();

  static void staticMethod1() {
    print('static 1');
  }

  static void staticMethod2() {
    print('static 2');
  }
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField2 = '';

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(instanceField2),
        Text(MyWidget.staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    MyWidget.staticMethod1();
  }

  void instanceMethod2() {
    print('instance 2');
  }
}
"#),
    ];
    for (i, (name, before, after)) in cases.iter().enumerate() {
        let rel = format!("lib/s{i}.dart");
        let uri = p.uri(&rel);
        let r = actions_at(&mut p, &rel, before, "MyWidget extends");
        assert_eq!(apply(&action(&r, "Convert to StatefulWidget")["edit"], &uri, before), *after, "{name}");
    }
}

/// The cases of 3.6.2's `flutter_convert_to_stateless_widget_test.dart`
/// (`None`: `assertNoAssist`).
#[test]
fn flutter_to_stateless_like_analysis_server() {
    let mut p = Project::with_literals("acoes-flutter-stateless");
    p.install_flutter();
    let cases: &[(&str, &str, &str, Option<&str>)] = &[
        ("test_comment", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget();

  @override
  State<MyWidget> createState() => _MyWidgetState();

  /// some m
  void m() {}
}

/// comment for state
class _MyWidgetState extends State<MyWidget> {
  // something for a
  final bool a = false;

  // another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }

  /// some m
  void m() {}
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  const MyWidget();

  // something for a
  final bool a = false;

  // another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }

  /// some m
  void m() {}
}
"#)),
        ("test_default_override", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {

  @override
  void initState() {
    // some comment
    super.initState();
  }

  @override
  void dispose() => super.dispose();

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_empty", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_fields", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  final void Function(bool) onHover;
  static String staticField2 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1, this.onHover) : instanceField2 = '' {
    instanceField3 = '';
  }

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField4 = '';

  String instanceField5 = '';

  @override
  Widget build(BuildContext context) {
    instanceField4 = widget.instanceField1;
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(widget.instanceField2),
        Text(widget.instanceField3),
        Text(instanceField4),
        InkResponse(
          onHover: (_) => widget.onHover(true),
          child: Text(instanceField5),
        ),
        Text(MyWidget.staticField1),
        Text(MyWidget.staticField2),
        Text(MyWidget.staticField3),
      ],
    );
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  final void Function(bool) onHover;
  static String staticField2 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1, this.onHover) : instanceField2 = '' {
    instanceField3 = '';
  }

  String instanceField4 = '';

  String instanceField5 = '';

  @override
  Widget build(BuildContext context) {
    instanceField4 = instanceField1;
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(instanceField3),
        Text(instanceField4),
        InkResponse(
          onHover: (_) => onHover(true),
          child: Text(instanceField5),
        ),
        Text(staticField1),
        Text(staticField2),
        Text(staticField3),
      ],
    );
  }
}
"#)),
        ("test_getters", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();

  static String get staticGetter1 => '';

  static String get staticGetter2 => '';
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(MyWidget.staticGetter1),
        Text(MyWidget.staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  String get instanceGetter1 => '';

  String get instanceGetter2 => '';
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(staticGetter1),
        Text(staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  String get instanceGetter1 => '';

  String get instanceGetter2 => '';

  static String get staticGetter1 => '';

  static String get staticGetter2 => '';
}
"#)),
        ("test_methods", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField = '';
  final String instanceField1;

  MyWidget(this.instanceField1);

  @override
  State<MyWidget> createState() => _MyWidgetState();

  static void staticMethod1() {
    print('static 1');
  }

  static void staticMethod2() {
    print('static 2');
  }
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField2 = '';

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(instanceField2),
        Text(MyWidget.staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    MyWidget.staticMethod1();
  }

  void instanceMethod2() {
    print('instance 2');
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField = '';
  final String instanceField1;

  MyWidget(this.instanceField1);

  String instanceField2 = '';

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    staticMethod1();
  }

  void instanceMethod2() {
    print('instance 2');
  }

  static void staticMethod1() {
    print('static 1');
  }

  static void staticMethod2() {
    print('static 2');
  }
}
"#)),
        ("test_override", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {

  @override
  void initState() {
    print('');
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_public", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => MyWidgetState();
}

class MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_simple", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  final String aaa;
  final String $bbb;

  const MyWidget(this.aaa, this.$bbb);

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.aaa),
        Text(widget.$bbb),
        Text('${widget.aaa}'),
        Text('${widget.$bbb}'),
      ],
    );
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  final String aaa;
  final String $bbb;

  const MyWidget(this.aaa, this.$bbb);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(aaa),
        Text($bbb),
        Text('$aaa'),
        Text('${$bbb}'),
      ],
    );
  }
}
"#)),
        ("test_state_first", r#"
import 'package:flutter/widgets.dart';

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.aaa),
        Text(widget.$bbb),
        Text('${widget.aaa}'),
        Text('${widget.$bbb}'),
      ],
    );
  }
}

class MyWidget extends StatefulWidget {
  final String aaa;
  final String $bbb;

  const MyWidget(this.aaa, this.$bbb);

  @override
  State<MyWidget> createState() => _MyWidgetState();
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  final String aaa;
  final String $bbb;

  const MyWidget(this.aaa, this.$bbb);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(aaa),
        Text($bbb),
        Text('$aaa'),
        Text('${$bbb}'),
      ],
    );
  }
}
"#)),
        ("test_state_used_anotherWidget", r#"
import 'package:flutter/widgets.dart';

class FirstWidget extends StatefulWidget {
  const FirstWidget({super.key});

  @override
  createState() => _MyWidgetState();
}

class SecondWidget extends StatefulWidget {
  const SecondWidget({super.key});

  @override
  createState() => _MyWidgetState();
}

class _MyWidgetState extends State<FirstWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"irstWidget e"#, None),
        ("test_state_used_createState", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();

  State<MyWidget> another() => createState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_state_used_createState_return_stateClass", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  _MyWidgetState createState() => _MyWidgetState();

  _MyWidgetState another() => createState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_state_used_instanceCreationExpression", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();

  State<MyWidget> another() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_static_field", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  static var field = 1;

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, None),
        ("test_static_method", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  const MyWidget({super.key});

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {

  @override
  Widget build(BuildContext context) {
    return Container();
  }

  static void staticMethod1() {
    print('static 1');
  }
}
"#, r#"yWidget exte"#, None),
        ("test_typeParam_bound", r#"
import 'package:flutter/widgets.dart';

class MyWidget<T extends String> extends StatefulWidget {
  @override
  State<MyWidget<T>> createState() => _MyWidgetState<T>();
}

class _MyWidgetState<T extends String> extends State<MyWidget<T>> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget<T ex"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget<T extends String> extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_typeParam_different_bound", r#"
import 'package:flutter/widgets.dart';

class MyWidget<T extends String> extends StatefulWidget {
  @override
  State createState() => _MyWidgetState();
}

class _MyWidgetState<T extends List> extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget<T ex"#, None),
        ("test_typeParam_different_name", r#"
import 'package:flutter/widgets.dart';

class MyWidget<T> extends StatefulWidget {
  @override
  State<MyWidget<T>> createState() => _MyWidgetState<T>();
}

class _MyWidgetState<S> extends State<MyWidget<S>> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget<T> e"#, None),
        ("test_typeParam_empty", r#"
import 'package:flutter/widgets.dart';

class MyWidget<T> extends StatefulWidget {
  @override
  State<MyWidget<T>> createState() => _MyWidgetState<T>();
}

class _MyWidgetState<T> extends State<MyWidget<T>> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget<T> e"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget<T> extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
    ];
    let mut failures = Vec::new();
    for (i, (name, before, needle, after)) in cases.iter().enumerate() {
        let rel = format!("lib/s{i}.dart");
        let uri = p.uri(&rel);
        let r = actions_at(&mut p, &rel, before, needle);
        let found = r.as_array().unwrap().iter().find(|a| a["title"] == "Convert to StatelessWidget").cloned();
        match (after, found) {
            (None, None) => {}
            (Some(d), Some(a)) => {
                let obtained = apply(&a["edit"], &uri, before);
                if obtained != *d {
                    failures.push(format!("{name}: obtido\n{obtained}\nesperado\n{d}"));
                }
            }
            (d, a) => failures.push(format!("{name}: expected {} and got {}", d.is_some(), a.is_some())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n----\n"));
}

/// The cases of 3.6.2's `flutter_convert_to_stateful_widget_test.dart`
/// (`None`: `assertNoAssist`).
#[test]
fn flutter_to_stateful_full() {
    let mut p = Project::with_literals("acoes-flutter-stateful-3");
    p.install_flutter();
    let cases: &[(&str, &str, &str, Option<&str>)] = &[
        ("test_comment", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  // something for a
  final bool a = false;

  const MyWidget();

  // another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {

  const MyWidget();

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  // something for a
  final bool a = false;

  // another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_comment_documentation", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  /// something for a
  final bool a = false;

  const MyWidget();

  /// another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {

  const MyWidget();

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  /// something for a
  final bool a = false;

  /// another for b
  final bool b = true;

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_empty", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_empty_typeParam", r#"
import 'package:flutter/widgets.dart';

class MyWidget<T> extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"yWidget<T> e"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget<T> extends StatefulWidget {
  @override
  State<MyWidget<T>> createState() => _MyWidgetState<T>();
}

class _MyWidgetState<T> extends State<MyWidget<T>> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_fields", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  static String staticField2 = '';
  String instanceField4 = '';
  String instanceField5 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1) : instanceField2 = '' {
    instanceField3 = '';
  }

  @override
  Widget build(BuildContext context) {
    instanceField4 = instanceField1;
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(instanceField3),
        Text(instanceField4),
        Text(instanceField5),
        Text(staticField1),
        Text(staticField2),
        Text(staticField3),
      ],
    );
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField1 = '';
  final String instanceField1;
  final String instanceField2;
  String instanceField3 = '';
  static String staticField2 = '';
  static String staticField3 = '';

  MyWidget(this.instanceField1) : instanceField2 = '' {
    instanceField3 = '';
  }

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField4 = '';

  String instanceField5 = '';

  @override
  Widget build(BuildContext context) {
    instanceField4 = widget.instanceField1;
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(widget.instanceField2),
        Text(widget.instanceField3),
        Text(instanceField4),
        Text(instanceField5),
        Text(MyWidget.staticField1),
        Text(MyWidget.staticField2),
        Text(MyWidget.staticField3),
      ],
    );
  }
}
"#)),
        ("test_getters", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(staticGetter1),
        Text(staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  static String get staticGetter1 => '';

  String get instanceGetter1 => '';

  static String get staticGetter2 => '';

  String get instanceGetter2 => '';
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();

  static String get staticGetter1 => '';

  static String get staticGetter2 => '';
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(MyWidget.staticGetter1),
        Text(MyWidget.staticGetter2),
        Text(instanceGetter1),
        Text(instanceGetter2),
      ],
    );
  }

  String get instanceGetter1 => '';

  String get instanceGetter2 => '';
}
"#)),
        ("test_methods", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  static String staticField = '';
  final String instanceField1;
  String instanceField2 = '';

  MyWidget(this.instanceField1);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(instanceField1),
        Text(instanceField2),
        Text(staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    staticMethod1();
  }

  static void staticMethod1() {
    print('static 1');
  }

  void instanceMethod2() {
    print('instance 2');
  }

  static void staticMethod2() {
    print('static 2');
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  static String staticField = '';
  final String instanceField1;

  MyWidget(this.instanceField1);

  @override
  State<MyWidget> createState() => _MyWidgetState();

  static void staticMethod1() {
    print('static 1');
  }

  static void staticMethod2() {
    print('static 2');
  }
}

class _MyWidgetState extends State<MyWidget> {
  String instanceField2 = '';

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.instanceField1),
        Text(instanceField2),
        Text(MyWidget.staticField),
      ],
    );
  }

  void instanceMethod1() {
    instanceMethod1();
    instanceMethod2();
    MyWidget.staticMethod1();
  }

  void instanceMethod2() {
    print('instance 2');
  }
}
"#)),
        ("test_noExtraUnderscore", r#"
import 'package:flutter/widgets.dart';

class _MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#, r#"MyWidget ext"#, Some(r#"
import 'package:flutter/widgets.dart';

class _MyWidget extends StatefulWidget {
  @override
  State<_MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<_MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#)),
        ("test_simple", r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatelessWidget {
  final String aaa;
  final String bbb;

  const MyWidget(this.aaa, this.bbb);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(aaa),
        Text(bbb),
        Text('$aaa'),
        Text('${bbb}'),
      ],
    );
  }
}
"#, r#"yWidget exte"#, Some(r#"
import 'package:flutter/widgets.dart';

class MyWidget extends StatefulWidget {
  final String aaa;
  final String bbb;

  const MyWidget(this.aaa, this.bbb);

  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(widget.aaa),
        Text(widget.bbb),
        Text('${widget.aaa}'),
        Text('${widget.bbb}'),
      ],
    );
  }
}
"#)),
    ];
    let mut failures = Vec::new();
    for (i, (name, before, needle, after)) in cases.iter().enumerate() {
        let rel = format!("lib/s{i}.dart");
        let uri = p.uri(&rel);
        let r = actions_at(&mut p, &rel, before, needle);
        let found = r.as_array().unwrap().iter().find(|a| a["title"] == "Convert to StatefulWidget").cloned();
        match (after, found) {
            (None, None) => {}
            (Some(d), Some(a)) => {
                let obtained = apply(&a["edit"], &uri, before);
                if obtained != *d {
                    failures.push(format!("{name}: obtido\n{obtained}\nesperado\n{d}"));
                }
            }
            (d, a) => failures.push(format!("{name}: expected {} and got {}", d.is_some(), a.is_some())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n----\n"));
}

/// The cases of 3.6.2's `add_diagnostic_property_reference_test.dart` (the
/// assist and those of the fix, with the cursor on the property name).
#[test]
fn diagnostic_reference_like_analysis_server() {
    let mut p = Project::with_literals("acoes-diagnostico");
    p.install_flutter();
    let cases: &[(&str, &str, &str, Option<&str>)] = &[
        ("test_boolField_debugFillProperties", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class W extends Widget {
  bool property = true;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"property"#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class W extends Widget {
  bool property = true;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('property', property));
  }
}
"#)),
        ("test_notAvailable_mixin", r#"
mixin MyMixin {
  String get foo() {}
}
"#, r#"() {}
}
"#, None),
        ("test_notAvailable_outsideDiagnosticable", r#"
class C {
  String get f => '';
}
"#, r#" => '';
"#, None),
        ("fix_test_boolField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool get absorbing => _absorbing;
  bool _absorbing = false;
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('absorbing', absorbing));
  }
}
"#, r#"ignoringSemantics ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool get absorbing => _absorbing;
  bool _absorbing = false;
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('absorbing', absorbing));
    properties.add(DiagnosticsProperty<bool>('ignoringSemantics', ignoringSemantics));
  }
}
"#)),
        ("fix_test_boolField_empty", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
  }
}
"#, r#"ignoringSemantics ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    properties.add(DiagnosticsProperty<bool>('ignoringSemantics', ignoringSemantics));
  }
}
"#)),
        ("fix_test_boolField_empty_customParamName", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder props) {
  }
}
"#, r#"ignoringSemantics ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool ignoringSemantics = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder props) {
    props.add(DiagnosticsProperty<bool>('ignoringSemantics', ignoringSemantics));
  }
}
"#)),
        ("fix_test_boolGetter", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool get absorbing => _absorbing;
  bool _absorbing = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"absorbing ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  bool get absorbing => _absorbing;
  bool _absorbing = false;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('absorbing', absorbing));
  }
}
"#)),
        ("fix_test_doubleField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  double field = 4.2;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  double field = 4.2;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DoubleProperty('field', field));
  }
}
"#)),
        ("fix_test_dynamicField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  dynamic field;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field;
"#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  dynamic field;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty('field', field));
  }
}
"#)),
        ("fix_test_enumField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Foo field = Foo.bar;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
enum Foo {bar}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Foo field = Foo.bar;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(EnumProperty<Foo>('field', field));
  }
}
enum Foo {bar}
"#)),
        ("fix_test_functionField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ValueChanged<double> onChanged = (d) {};
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
typedef ValueChanged<T> = void Function(T value);
"#, r#"onChanged ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ValueChanged<double> onChanged = (d) {};
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(ObjectFlagProperty<ValueChanged<double>>.has('onChanged', onChanged));
  }
}
typedef ValueChanged<T> = void Function(T value);
"#)),
        ("fix_test_intField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  int field = 0;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  int field = 0;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(IntProperty('field', field));
  }
}
"#)),
        ("fix_test_iterableField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Iterable<String> field = [];
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Iterable<String> field = [];

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(IterableProperty<String>('field', field));
  }
}
"#)),
        ("fix_test_listField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  List<List<String>> field = [];
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  List<List<String>> field = [];
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(IterableProperty<List<String>>('field', field));
  }
}
"#)),
        ("fix_test_objectField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Object field = '';
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  Object field = '';
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<Object>('field', field));
  }
}
"#)),
        ("fix_test_stringField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  String field = '';
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  String field = '';
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(StringProperty('field', field));
  }
}
"#)),
        ("fix_test_stringField_noDebugFillProperties", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  String field = '';
}
"#, r#"field ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  String field = '';

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(StringProperty('field', field));
  }
}
"#)),
        ("fix_test_typeOutOfScopeField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ClassNotInScope<bool> onChanged;

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"onChanged;
"#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ClassNotInScope<bool> onChanged;

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<ClassNotInScope<bool>>('onChanged', onChanged));
  }
}
"#)),
        ("fix_test_typeOutOfScopeGetter", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ClassNotInScope<bool> get onChanged => null;

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"onChanged ="#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  ClassNotInScope<bool> get onChanged => null;

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<ClassNotInScope<bool>>('onChanged', onChanged));
  }
}
"#)),
        ("fix_test_varField", r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  var field;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
  }
}
"#, r#"field;
"#, Some(r#"
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class C extends Widget with Diagnosticable {
  var field;
  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty('field', field));
  }
}
"#)),
    ];
    let mut failures = Vec::new();
    for (i, (name, before, needle, after)) in cases.iter().enumerate() {
        let rel = format!("lib/d{i}.dart");
        let uri = p.uri(&rel);
        let r = actions_at(&mut p, &rel, before, needle);
        let found = r.as_array().unwrap().iter().find(|a| a["title"] == "Add a debug reference to this property").cloned();
        match (after, found) {
            (None, None) => {}
            (Some(d), Some(a)) => {
                let obtained = apply(&a["edit"], &uri, before);
                if obtained != *d {
                    failures.push(format!("{name}: obtido\n{obtained}\nesperado\n{d}"));
                }
            }
            (d, a) => failures.push(format!("{name}: expected {} and got {}", d.is_some(), a.is_some())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n----\n"));
}

/// Client with `experimental.snippetTextEdit` and `documentChanges`: the
/// actions with a selection or linked groups come out as `SnippetTextEdit`
/// (Dart's `createWorkspaceEdit(..., allowSnippets: true)`, §13.8.1-G).
fn with_snippets(name: &str) -> Project {
    Project::with_capabilities(
        name,
        json!({
            "experimental": {"snippetTextEdit": true},
            "workspace": {"workspaceEdit": {"documentChanges": true}},
            "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}},
        }),
    )
}

#[test]
fn snippets_in_assists() {
    let mut p = with_snippets("acoes-snippets");
    let text = "void f() {\n  print(1);\n  print(r'$x');\n}\nint g() => 1;\n";
    p.open("lib/a.dart", text);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let from = location(text, "print(1)", 0);
    let until = location(text, "\n}", 0);
    let r = actions(&mut p, "lib/a.dart", from, until, json!({}));
    // One group (`CONDITION`) and the selection: `${1:condition}` and `$0`; the
    // text's `$` is escaped.
    let e = &action(&r, "Surround with 'if'")["edit"]["documentChanges"][0]["edits"][0];
    assert_eq!(e["insertTextFormat"], 2);
    assert_eq!(e["newText"], "  if (${1:condition}) {\n    print(1);\n    print(r'\\$x');\n  }$0\n");
    // Several groups: numbered in text order.
    let e = &action(&r, "Surround with 'for'")["edit"]["documentChanges"][0]["edits"][0];
    assert!(e["newText"].as_str().unwrap().starts_with("  for (var ${1:v} = ${2:init}; ${3:condition}; ${4:increment}) {\n"), "{e}");
    // `Surround with block` has only text: a plain edit.
    let e = &action(&r, "Surround with block")["edit"]["documentChanges"][0]["edits"][0];
    assert!(e.get("insertTextFormat").is_none(), "{e}");
    // `Convert to block body`: the selection before the line break that closes
    // the block.
    let pos = location(text, "g()", 0);
    let r = p.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": pos.0, "character": pos.1}, "end": {"line": pos.0, "character": pos.1}}, "context": {"diagnostics": []}}),
    )["result"]
        .clone();
    let e = &action(&r, "Convert to block body")["edit"]["documentChanges"][0]["edits"][0];
    assert_eq!(e["insertTextFormat"], 2);
    assert_eq!(e["newText"], " {\n  return 1;$0\n}");
}

#[test]
fn without_the_capability_the_edit_is_plain() {
    let mut p = Project::with_capabilities(
        "actions-without-snippets",
        json!({
            "workspace": {"workspaceEdit": {"documentChanges": true}},
            "textDocument": {"codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}},
        }),
    );
    let text = "void f() {\n  print(1);\n}\n";
    p.open("lib/a.dart", text);
    p.server.await_diagnostics(std::time::Duration::from_secs(120));
    let from = location(text, "print(1)", 0);
    let until = location(text, "\n}", 0);
    let r = actions(&mut p, "lib/a.dart", from, until, json!({}));
    let e = &action(&r, "Surround with 'if'")["edit"]["documentChanges"][0]["edits"][0];
    assert!(e.get("insertTextFormat").is_none(), "{e}");
    assert_eq!(e["newText"], "  if (condition) {\n    print(1);\n  }\n");
}
