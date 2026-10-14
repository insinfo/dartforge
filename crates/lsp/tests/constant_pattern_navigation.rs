//! Bare constant-pattern names use semantic references without capturing bindings.

mod common;

use common::{Project, apply, mark};
use serde_json::json;

fn position(source: &str, needle: &str) -> (u32, u32) {
    let offset = source.find(needle).unwrap();
    (source[..offset].matches('\n').count() as u32, source[..offset].rsplit('\n').next().unwrap().encode_utf16().count() as u32)
}

#[test]
fn bare_constant_definition_and_rename_include_pattern_occurrences() {
    let mut project = Project::new("constant-pattern-top-level");
    let (source, line, character) = mark("const threshold = 1;\nint read(int x) => switch (x) { th▮reshold => 42, _ => 0 };\n");
    project.open("lib/main.dart", &source);
    let definition = project.in_position("textDocument/definition", "lib/main.dart", line, character, json!({}));
    assert_eq!(definition["result"]["range"]["start"], json!({"line":0,"character":6}), "{definition}");
    let from_pattern = project.in_position("textDocument/rename", "lib/main.dart", line, character, json!({"newName":"item"}));
    let (line, character) = position(&source, "threshold =");
    let from_declaration = project.in_position("textDocument/rename", "lib/main.dart", line, character, json!({"newName":"item"}));
    assert_eq!(from_pattern["result"], from_declaration["result"]);
    assert_eq!(apply(&from_pattern["result"], &project.uri("lib/main.dart"), &source), source.replace("threshold", "item"), "{from_pattern}");
}

#[test]
fn imported_constant_identity_does_not_capture_other_library_or_local_binding() {
    let mut project = Project::new("constant-pattern-import-identity");
    let model = "const threshold = 1;\n";
    project.write("lib/a.dart", model);
    project.write("lib/b.dart", "const threshold = 2;\n");
    let (source, line, character) = mark("import 'a.dart' show threshold;\nimport 'a.dart' as a;\nimport 'b.dart' as b;\nint read(int x) => switch (x) { th▮reshold => a.threshold + b.threshold, _ => 0 };\nint binding(int x) { if (x case final threshold) { return threshold; } return 0; }\n");
    project.open("lib/main.dart", &source);
    let definition = project.in_position("textDocument/definition", "lib/main.dart", line, character, json!({}));
    assert_eq!(definition["result"]["uri"], project.uri("lib/a.dart"), "{definition}");
    let rename = project.in_position("textDocument/rename", "lib/main.dart", line, character, json!({"newName":"item"}));
    assert_eq!(apply(&rename["result"], &project.uri("lib/a.dart"), model), model.replace("threshold", "item"));
    assert!(rename["result"]["changes"].get(project.uri("lib/b.dart")).is_none(), "{rename}");
    let expected = source.replace("show threshold", "show item").replace("{ threshold =>", "{ item =>").replace("a.threshold", "a.item");
    assert_eq!(apply(&rename["result"], &project.uri("lib/main.dart"), &source), expected, "{rename}");
    let (line, character) = position(&source, "threshold) {");
    let binding = project.in_position("textDocument/definition", "lib/main.dart", line, character, json!({}));
    assert_eq!(binding["result"]["uri"], project.uri("lib/main.dart"), "{binding}");
}

#[test]
fn lexical_static_constant_and_enum_value_resolve_to_their_members() {
    for (index, marked, declaration) in [
        ("class C { static const threshold = 1; static int read(int x) => switch (x) { th▮reshold => 42, _ => 0 }; }\n", "threshold ="),
        ("enum E { first, second; int read() => switch (this) { fi▮rst => 1, _ => 0 }; }\n", "first,"),
    ].iter().enumerate().map(|(i, (s, d))| (i, *s, *d)) {
        let mut project = Project::new(&format!("constant-pattern-member-{index}"));
        let (source, line, character) = mark(marked);
        project.open("lib/main.dart", &source);
        let definition = project.in_position("textDocument/definition", "lib/main.dart", line, character, json!({}));
        let (dl, dc) = position(&source, declaration);
        assert_eq!(definition["result"]["range"]["start"], json!({"line":dl,"character":dc}), "{definition}");
        let rename = project.in_position("textDocument/rename", "lib/main.dart", line, character, json!({"newName":"item"}));
        let old = declaration.split([' ', ',']).next().unwrap();
        assert_eq!(apply(&rename["result"], &project.uri("lib/main.dart"), &source), source.replace(old, "item"), "{rename}");
    }
}

#[test]
fn unresolved_pattern_name_does_not_acquire_an_unrelated_local_declaration() {
    let mut project = Project::new("constant-pattern-unresolved");
    let (source, line, character) = mark("int read(int x) => switch (x) { mi▮ssing => 1, _ => 0 };\nint other() { const missing = 1; return missing; }\n");
    project.open("lib/main.dart", &source);
    let definition = project.in_position("textDocument/definition", "lib/main.dart", line, character, json!({}));
    assert!(definition["result"].is_null(), "{definition}");
}
