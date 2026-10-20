//! "Incremental = from scratch" (docs/LSP-ESPECIFICACAO.md §16.8), in short
//! mode: a sequence of edits applied to a server with the incremental path
//! (the unit swap in `didChange` and speculative completion, §16.10 I1 to
//! I4) and to another without it; after each step, the responses at the
//! sampled positions are equal. Body edits do not drop the session;
//! signature and directive edits do.

mod common;

use common::Project;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};

/// A project whose server does not use the incremental path.
fn without_incremental(name: &str) -> Project {
    let mut p = Project::new(name);
    let sdk = SdkLayout::load(&p.root.join("sdk/lib"), "dartdevc").unwrap();
    p.server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)).without_incremental());
    let root = p.uri("");
    p.request("initialize", json!({"rootUri": root, "capabilities": {}}));
    p
}

/// (line, column) of the `n`-th occurrence (0: the first) of `needle` in
/// `text`, plus `delta` columns (ASCII).
fn position(text: &str, needle: &str, n: usize, delta: u32) -> (u32, u32) {
    let i = text
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("no {needle}"))
        .0;
    let before = &text[..i];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().unwrap().len() as u32;
    (line, column + delta)
}

/// The responses compared at one position, with the project root (which
/// changes from one temporary project to the other, with the SDK beside it)
/// replaced by `RAIZ/`.
fn responses(p: &mut Project, l: u32, c: u32) -> Vec<Value> {
    let project = p.uri("");
    let root = project.trim_end_matches('/').trim_end_matches("projeto");
    let mut v = Vec::new();
    for method in [
        "textDocument/hover",
        "textDocument/definition",
        "textDocument/documentHighlight",
        "textDocument/completion",
    ] {
        let r = p.in_position(method, "lib/a.dart", l, c, json!({}))["result"].clone();
        v.push(serde_json::from_str(&r.to_string().replace(root, "RAIZ/")).unwrap());
    }
    v
}

const V0: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  for (final x in xs) {
    total += x;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor);
}
";

/// Body of `contar` with new types before `B` (shifts the `TypeId`s of the
/// clauses of `B`).
const V1: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  var lista = <String>[];
  Map<int, String> m = {};
  for (final x in xs) {
    total += x + lista.length + m.length;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor);
}
";

/// Body of `main`.
const V2: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs) {
  var total = 0;
  var lista = <String>[];
  Map<int, String> m = {};
  for (final x in xs) {
    total += x + lista.length + m.length;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor + b.dobro(contar([1, 2])));
  b.
}
";

/// Signature of `contar`.
const V3: &str = "class A<T> {
  T valor;
  A(this.valor);
  int dobro(int x) => x * 2;
}

int contar(List<int> xs, [int inicio = 0]) {
  var total = inicio;
  for (final x in xs) {
    total += x;
  }
  return total;
}

class B extends A<int> {
  B() : super(0);
  String nome = '';
}

void main() {
  var b = B();
  print(b.valor + b.dobro(contar([1, 2])));
  b.
}
";

/// The sampled positions of a text.
fn samples(text: &str) -> Vec<(u32, u32)> {
    let mut v = vec![
        position(text, "b.valor", 0, 2),
        position(text, "B()", 1, 0),
        position(text, "total", 1, 0),
        position(text, "xs", 1, 1),
    ];
    if text.contains("  b.\n") {
        v.push(position(text, "  b.\n", 0, 4));
    }
    if text.contains("lista.length") {
        v.push(position(text, "lista.length", 0, 6));
    }
    v
}

#[test]
fn body_and_signature_edits_match_from_scratch() {
    let mut hot = Project::new("incremental-quente");
    let mut cold = without_incremental("incremental-frio");
    hot.open("lib/a.dart", V0);
    cold.open("lib/a.dart", V0);
    for (l, c) in samples(V0) {
        assert_eq!(
            responses(&mut hot, l, c),
            responses(&mut cold, l, c),
            "V0 at {l}:{c}"
        );
    }
    let mut version = 1;
    let mut swaps = 0;
    for (text, body) in [
        (V1, true),
        (V2, true),
        (V3, false),
        (V2, false),
        (V1, true),
        (V0, true),
    ] {
        version += 1;
        let before = hot.server.analyzer().session_stats();
        hot.change("lib/a.dart", version, text);
        cold.change("lib/a.dart", version, text);
        let after = hot.server.analyzer().session_stats();
        if body {
            swaps += 1;
            assert_eq!(
                (after.body_swaps, after.invalidated),
                (swaps, before.invalidated),
                "{after:?}"
            );
        } else {
            assert_eq!(after.body_swaps, swaps, "{after:?}");
        }
        for (l, c) in samples(text) {
            let r = responses(&mut hot, l, c);
            assert_eq!(
                r,
                responses(&mut cold, l, c),
                "version {version} at {l}:{c}"
            );
            // Speculative completion undoes the swap: the retained state answers the
            // same after it.
            assert_eq!(
                responses(&mut hot, l, c),
                r,
                "repeated, version {version} at {l}:{c}"
            );
        }
    }
    // Body edits without reloading: the warm session loaded less.
    let q = hot.server.analyzer().session_stats();
    let f = cold.server.analyzer().session_stats();
    assert!(q.loaded < f.loaded, "{q:?} {f:?}");
}

#[test]
fn directives_and_shape_drop_session() {
    let mut p = Project::new("incremental-diretivas");
    p.open("lib/a.dart", V0);
    responses(&mut p, 1, 4);
    let e = p.server.analyzer().session_stats();
    // A new directive.
    p.change("lib/a.dart", 2, &format!("import 'dart:math';\n{V0}"));
    let d = p.server.analyzer().session_stats();
    assert_eq!(
        (d.invalidated, d.body_swaps),
        (e.invalidated + 1, 0),
        "{d:?}"
    );
    responses(&mut p, 2, 4);
    // A new declaration (shape).
    p.change(
        "lib/a.dart",
        3,
        &format!("import 'dart:math';\n{V0}int z() => 0;\n"),
    );
    let f = p.server.analyzer().session_stats();
    assert_eq!(
        (f.invalidated, f.body_swaps),
        (d.invalidated + 1, 0),
        "{f:?}"
    );
    // A body that leaks (open brace): refuses or swaps, never a response
    // different from the one from scratch.
    responses(&mut p, 2, 4);
    let leaks = format!(
        "import 'dart:math';\n{}int z() => 0;\n",
        V0.replace("return total;", "return total; {")
    );
    p.change("lib/a.dart", 4, &leaks);
    let mut cold = without_incremental("incremental-vaza");
    cold.open("lib/a.dart", &leaks);
    for (l, c) in [(2, 4), (21, 9)] {
        assert_eq!(
            responses(&mut p, l, c),
            responses(&mut cold, l, c),
            "{l}:{c}"
        );
    }
}
