//! "Incremental = do zero" (docs/LSP-ESPECIFICACAO.md §16.8), no modo curto:
//! uma sequência de edições aplicada a um servidor com o caminho incremental
//! (a troca de unidade no `didChange` e o completar especulativo, §16.10 I1
//! a I4) e a outro sem ele; depois de cada passo, as respostas nas posições
//! amostradas são iguais. As edições de corpo não derrubam a sessão; as de
//! assinatura e de diretivas derrubam.

mod common;

use common::Project;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};

/// Um projeto cujo servidor não usa o caminho incremental.
fn without_incremental(name: &str) -> Project {
    let mut p = Project::new(name);
    let sdk = SdkLayout::load(&p.root.join("sdk/lib"), "dartdevc").unwrap();
    p.server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)).without_incremental());
    let root = p.uri("");
    p.request("initialize", json!({"rootUri": root, "capabilities": {}}));
    p
}

/// (linha, coluna) da `n`-ésima ocorrência (0: a primeira) de `needle` em
/// `text`, mais `delta` colunas (ASCII).
fn position(text: &str, needle: &str, n: usize, delta: u32) -> (u32, u32) {
    let i = text.match_indices(needle).nth(n).unwrap_or_else(|| panic!("sem {needle}")).0;
    let before = &text[..i];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().unwrap().len() as u32;
    (line, column + delta)
}

/// As respostas comparadas numa posição, com a raiz do projeto (que
/// muda de um projeto temporário para o outro, com o SDK ao lado) trocada
/// por `RAIZ/`.
fn responses(p: &mut Project, l: u32, c: u32) -> Vec<Value> {
    let project = p.uri("");
    let root = project.trim_end_matches('/').trim_end_matches("projeto");
    let mut v = Vec::new();
    for method in ["textDocument/hover", "textDocument/definition", "textDocument/documentHighlight", "textDocument/completion"] {
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

/// Corpo de `contar` com tipos novos antes de `B` (desloca os `TypeId`
/// das cláusulas de `B`).
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

/// Corpo de `main`.
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

/// Assinatura de `contar`.
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

/// As posições amostradas de um texto.
fn samples(text: &str) -> Vec<(u32, u32)> {
    let mut v = vec![position(text, "b.valor", 0, 2), position(text, "B()", 1, 0), position(text, "total", 1, 0), position(text, "xs", 1, 1)];
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
        assert_eq!(responses(&mut hot, l, c), responses(&mut cold, l, c), "V0 em {l}:{c}");
    }
    let mut version = 1;
    let mut swaps = 0;
    for (text, body) in [(V1, true), (V2, true), (V3, false), (V2, false), (V1, true), (V0, true)] {
        version += 1;
        let before = hot.server.analyzer().session_stats();
        hot.change("lib/a.dart", version, text);
        cold.change("lib/a.dart", version, text);
        let after = hot.server.analyzer().session_stats();
        if body {
            swaps += 1;
            assert_eq!((after.body_swaps, after.invalidated), (swaps, before.invalidated), "{after:?}");
        } else {
            assert_eq!(after.body_swaps, swaps, "{after:?}");
        }
        for (l, c) in samples(text) {
            let r = responses(&mut hot, l, c);
            assert_eq!(r, responses(&mut cold, l, c), "versão {version} em {l}:{c}");
            // O completar especulativo desfaz a troca: o estado retido
            // responde igual depois dele.
            assert_eq!(responses(&mut hot, l, c), r, "repetido, versão {version} em {l}:{c}");
        }
    }
    // Edições de corpo sem recarga: a sessão quente carregou menos.
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
    // Uma diretiva nova.
    p.change("lib/a.dart", 2, &format!("import 'dart:math';\n{V0}"));
    let d = p.server.analyzer().session_stats();
    assert_eq!((d.invalidated, d.body_swaps), (e.invalidated + 1, 0), "{d:?}");
    responses(&mut p, 2, 4);
    // Uma declaração nova (forma).
    p.change("lib/a.dart", 3, &format!("import 'dart:math';\n{V0}int z() => 0;\n"));
    let f = p.server.analyzer().session_stats();
    assert_eq!((f.invalidated, f.body_swaps), (d.invalidated + 1, 0), "{f:?}");
    // Um corpo que vaza (chave aberta): recusa ou troca, nunca resposta
    // diferente da do zero.
    responses(&mut p, 2, 4);
    let leaks = format!("import 'dart:math';\n{}int z() => 0;\n", V0.replace("return total;", "return total; {"));
    p.change("lib/a.dart", 4, &leaks);
    let mut cold = without_incremental("incremental-vaza");
    cold.open("lib/a.dart", &leaks);
    for (l, c) in [(2, 4), (21, 9)] {
        assert_eq!(responses(&mut p, l, c), responses(&mut cold, l, c), "{l}:{c}");
    }
}
