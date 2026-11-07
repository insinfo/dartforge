//! Full prefix hover payloads recorded from actual SDK 3.6.2 and 3.13.5.
mod common;
use common::Project;
use serde_json::{Value, json};

fn exercise(modern: bool) {
    let mut project = Project::new(if modern {
        "hover-prefix-modern"
    } else {
        "hover-prefix-floor"
    });
    std::fs::write(
        project.root.join("sdk/version"),
        if modern { "3.13.5\n" } else { "3.6.2\n" },
    )
    .unwrap();
    let files: Value = serde_json::from_str(r###"{"main.dart": "import 'one.dart' as basic;\nimport \"one.dart\" as quoted show Box;\nimport 'one.dart' deferred as lazyAlias hide one;\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\nimport 'one.dart' if (dart.library.io) 'two.dart' as conditional;\nimport 'missing.dart' as absent;\npart 'piece.dart';\nvoid main() {\n  final constructor = basic.Box<int>.named;\n  /* 🧭 */ basic.one;\n  quoted.Box<int>();\n  lazyAlias.loadLibrary;\n  shared.one;\n  shared.two;\n  conditional.two;\n  absent.value;\n}\n", "one.dart": "library first;\nclass Box<T> { Box.named(); }\nint one = 1;\n", "two.dart": "library second;\nint two = 2;\n", "piece.dart": "part of 'main.dart';\nvoid inPart() { basic.one; shared.two; }\n"}"###).unwrap();
    for (name, text) in files.as_object().unwrap() {
        project.write(
            &format!("lib/{name}"),
            &text.as_str().unwrap().replace('\n', "\r\n"),
        );
    }
    project
        .restart(json!({"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}));
    for file in ["main.dart", "piece.dart"] {
        project.open(
            &format!("lib/{file}"),
            &files[file].as_str().unwrap().replace('\n', "\r\n"),
        );
    }
    let cases: Value = serde_json::from_str(r###"[{"file": "main.dart", "position": {"line": 0, "character": 21}}, {"file": "main.dart", "position": {"line": 1, "character": 21}}, {"file": "main.dart", "position": {"line": 2, "character": 30}}, {"file": "main.dart", "position": {"line": 3, "character": 21}}, {"file": "main.dart", "position": {"line": 4, "character": 21}}, {"file": "main.dart", "position": {"line": 5, "character": 53}}, {"file": "main.dart", "position": {"line": 6, "character": 25}}, {"file": "main.dart", "position": {"line": 9, "character": 22}}, {"file": "main.dart", "position": {"line": 10, "character": 11}}, {"file": "main.dart", "position": {"line": 11, "character": 2}}, {"file": "main.dart", "position": {"line": 12, "character": 2}}, {"file": "main.dart", "position": {"line": 13, "character": 2}}, {"file": "main.dart", "position": {"line": 14, "character": 2}}, {"file": "main.dart", "position": {"line": 15, "character": 2}}, {"file": "main.dart", "position": {"line": 16, "character": 2}}, {"file": "piece.dart", "position": {"line": 1, "character": 16}}, {"file": "piece.dart", "position": {"line": 1, "character": 27}}]"###).unwrap();
    let expected: Value = serde_json::from_str(r###"[{"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as basic;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 26, "line": 0}, "start": {"character": 21, "line": 0}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as quoted;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 27, "line": 1}, "start": {"character": 21, "line": 1}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as lazyAlias;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 39, "line": 2}, "start": {"character": 30, "line": 2}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 27, "line": 3}, "start": {"character": 21, "line": 3}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 27, "line": 4}, "start": {"character": 21, "line": 4}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as conditional;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 64, "line": 5}, "start": {"character": 53, "line": 5}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'missing.dart' as absent;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 31, "line": 6}, "start": {"character": 25, "line": 6}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as basic;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 27, "line": 9}, "start": {"character": 22, "line": 9}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as basic;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 16, "line": 10}, "start": {"character": 11, "line": 10}}}, {"contents": {"kind": "markdown", "value": "Type: `Box<int>`"}, "range": {"end": {"character": 17, "line": 11}, "start": {"character": 2, "line": 11}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as lazyAlias;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 11, "line": 12}, "start": {"character": 2, "line": 12}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 8, "line": 13}, "start": {"character": 2, "line": 13}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 8, "line": 14}, "start": {"character": 2, "line": 14}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as conditional;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 13, "line": 15}, "start": {"character": 2, "line": 15}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'missing.dart' as absent;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 8, "line": 16}, "start": {"character": 2, "line": 16}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as basic;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 21, "line": 1}, "start": {"character": 16, "line": 1}}}, {"contents": {"kind": "markdown", "value": "```dart\nimport 'one.dart' as shared;\nimport 'two.dart' as shared;\n```\nDeclared in *package:projeto/main.dart*."}, "range": {"end": {"character": 33, "line": 1}, "start": {"character": 27, "line": 1}}}]"###).unwrap();
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let actual = project.in_position(
            "textDocument/hover",
            &format!("lib/{}", case["file"].as_str().unwrap()),
            case["position"]["line"].as_u64().unwrap() as u32,
            case["position"]["character"].as_u64().unwrap() as u32,
            json!({}),
        );
        assert_eq!(
            actual["result"],
            if modern {
                expected[index].clone()
            } else {
                Value::Null
            },
            "{case}"
        );
    }
}
#[test]
fn legacy_sdk_keeps_prefix_hover_empty() {
    exercise(false);
}
#[test]
fn modern_sdk_prefix_hover_includes_all_imports_and_owning_library() {
    exercise(true);
}
