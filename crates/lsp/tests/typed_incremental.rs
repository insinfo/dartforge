//! Incremental typed analysis (`docs/LSP.md`, "Edição incremental"): after a
//! sequence of edits — inside bodies, to a public API that other files use,
//! inside a part, closing a document — what the editor shows for every file of
//! the workspace package is what a server that starts from scratch with the
//! same texts shows; and an edit that keeps the API reanalyzes only its
//! library.
mod common;

use common::Project;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);

/// The package on disk: a model used by a service, a page and (through a
/// barrel) a user, and a library with a part that calls a private helper.
const FILES: &[(&str, &str)] = &[
    (
        "lib/model.dart",
        "class Model {\n  int value = 0;\n  int twice() => value * 2;\n}\n\nint total(Model m) {\n  return m.twice();\n}\n",
    ),
    (
        "lib/service.dart",
        "import 'model.dart';\n\nclass Service {\n  Model make() => Model();\n  int run(Model m) {\n    return m.twice() + total(m);\n  }\n}\n",
    ),
    (
        "lib/page.dart",
        "import 'service.dart';\nimport 'model.dart';\n\nvoid page() {\n  final s = Service();\n  final int x = s.run(s.make());\n  print(x);\n}\n",
    ),
    (
        "lib/barrel.dart",
        "export 'model.dart';\nexport 'service.dart';\n",
    ),
    (
        "lib/user.dart",
        "import 'package:projeto/barrel.dart';\n\nint use(Model m) {\n  if (1) {}\n  return m.twice();\n}\n",
    ),
    (
        "lib/lib.dart",
        "library lib;\n\npart 'lib_part.dart';\n\nint _helper() => 1;\n\nvoid useIt() {}\n",
    ),
    (
        "lib/lib_part.dart",
        "part of 'lib.dart';\n\nvoid partFn() {\n  _helper();\n}\n",
    ),
    ("lib/alone.dart", "void alone() {\n  print(1);\n}\n"),
];

/// What the editor shows: the last publication of each file, by relative path
/// (a file never published has no diagnostics), as (code, range, message).
type Shown = BTreeMap<String, Vec<String>>;

fn record(p: &Project, outputs: &[Value], shown: &mut Shown) {
    let root = url::Url::from_file_path(p.path("")).unwrap().to_string();
    for m in outputs {
        if m["method"] != "textDocument/publishDiagnostics" {
            continue;
        }
        let uri = m["params"]["uri"].as_str().unwrap();
        let Some(relative) = uri.strip_prefix(&root) else {
            continue;
        };
        let list = m["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| format!("{} {} {}", d["code"], d["range"], d["message"]))
            .collect();
        shown.insert(relative.trim_start_matches('/').to_string(), list);
    }
    shown.retain(|_, v| !v.is_empty());
}

/// A server on a copy of the package with `open` (relative path, text) open,
/// from scratch: what it shows once its analysis settles.
fn from_scratch(name: &str, open: &BTreeMap<&str, String>) -> Shown {
    let mut p = Project::new(name);
    for (relative, text) in FILES {
        p.write(relative, text);
    }
    let mut shown = Shown::new();
    p.server
        .receive(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    let mut outputs = p.server.pump();
    for (relative, text) in open {
        p.server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":p.uri(relative),"languageId":"dart","version":1,"text":text}
            }}),
        );
        outputs.extend(p.server.pump());
    }
    outputs.extend(p.server.await_diagnostics(LIMIT));
    record(&p, &outputs, &mut shown);
    shown
}

#[test]
fn incremental_edits_publish_what_a_fresh_analysis_publishes() {
    let mut p = Project::new("typed-incremental");
    for (relative, text) in FILES {
        p.write(relative, text);
    }
    let mut shown = Shown::new();
    p.server
        .receive(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    let mut outputs = p.server.pump();
    outputs.extend(p.server.await_diagnostics(LIMIT));
    record(&p, &outputs, &mut shown);
    assert!(
        shown.contains_key("lib/user.dart"),
        "the workspace analysis did not publish user.dart: {shown:?}"
    );
    let mut open: BTreeMap<&str, String> = BTreeMap::new();
    let mut versions: BTreeMap<&str, i64> = BTreeMap::new();
    let text_of = |relative: &str| {
        FILES
            .iter()
            .find(|(r, _)| *r == relative)
            .map(|(_, t)| t.to_string())
            .unwrap()
    };
    // (step, file, edit): each edit replaces the first occurrence of a text.
    let steps: &[(&str, &str, &str, &str)] = &[
        (
            "body edit in the model",
            "lib/model.dart",
            "value * 2",
            "value * 3",
        ),
        (
            "API edit in the model",
            "lib/model.dart",
            "int twice()",
            "int triple()",
        ),
        (
            "body edit in a part",
            "lib/lib_part.dart",
            "  _helper();\n",
            "",
        ),
        (
            "body error in the service",
            "lib/service.dart",
            "    return",
            "    if (2) {}\n    return",
        ),
        (
            "API restored in the model",
            "lib/model.dart",
            "int triple()",
            "int twice()",
        ),
        (
            "type error in a page body",
            "lib/page.dart",
            "final int x",
            "final String x",
        ),
        (
            "new private unused in the model",
            "lib/model.dart",
            "int total",
            "int _spare() => 0;\n\nint total",
        ),
        (
            "body edit in the owner",
            "lib/lib.dart",
            "void useIt() {}",
            "void useIt() {\n  _helper();\n}",
        ),
        (
            "body edit in the part again",
            "lib/lib_part.dart",
            "void partFn() {\n",
            "void partFn() {\n  _helper();\n",
        ),
    ];
    for (step, (label, relative, from, to)) in steps.iter().enumerate() {
        let before = p.server.typed_analyzed_files();
        if !open.contains_key(relative) {
            let text = text_of(relative);
            p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":p.uri(relative),"languageId":"dart","version":1,"text":text}
            }}));
            let mut outputs = p.server.pump();
            outputs.extend(p.server.await_diagnostics(LIMIT));
            record(&p, &outputs, &mut shown);
            open.insert(relative, text);
            versions.insert(relative, 1);
        }
        let opened = p.server.typed_analyzed_files();
        let text = open[relative].replacen(from, to, 1);
        assert_ne!(text, open[relative], "step {label}: the edit applies");
        let version = versions[relative] + 1;
        versions.insert(relative, version);
        // The immediate (syntactic) publication goes out with the change.
        p.server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":p.uri(relative),"version":version},
                "contentChanges":[{"text":text}]
            }}),
        );
        let mut outputs = p.server.pump();
        open.insert(relative, text);
        outputs.extend(p.server.await_diagnostics(LIMIT));
        record(&p, &outputs, &mut shown);
        let analyzed = p.server.typed_analyzed_files() - opened;
        let expected = from_scratch(&format!("typed-incremental-fresh-{step}"), &open);
        assert_eq!(
            shown, expected,
            "step {label}: the incremental publications differ from a fresh analysis"
        );
        eprintln!(
            "{label}: {analyzed} files analyzed for the edit ({} with the opening)",
            p.server.typed_analyzed_files() - before
        );
        // A body edit reanalyzes only its library: one file, or the owner with
        // its part.
        if label.starts_with("body") {
            let library = if relative.starts_with("lib/lib") {
                2
            } else {
                1
            };
            assert!(
                analyzed <= library,
                "step {label}: {analyzed} files analyzed for a body edit"
            );
        }
    }
    // Closing a document: its disk text counts again.
    for relative in ["lib/model.dart", "lib/lib_part.dart"] {
        p.server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
                "textDocument":{"uri":p.uri(relative)}
            }}),
        );
        let mut outputs = p.server.pump();
        outputs.extend(p.server.await_diagnostics(LIMIT));
        record(&p, &outputs, &mut shown);
        open.remove(relative);
        let expected = from_scratch(
            &format!("typed-incremental-fresh-close-{}", open.len()),
            &open,
        );
        assert_eq!(
            shown, expected,
            "closing {relative}: the incremental publications differ from a fresh analysis"
        );
    }
}
