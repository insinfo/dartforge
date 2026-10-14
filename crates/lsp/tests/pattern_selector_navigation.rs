//! Object-pattern selector navigation follows resolved member identity.

mod common;

use common::{Project, apply, mark};
use serde_json::json;

#[test]
fn explicit_selector_resolves_cross_file_getter_and_preserves_binding_navigation() {
    let mut project = Project::new("pattern-selector-getter");
    let model = "class C { int get value => 1; }\nclass Other { int get value => 2; }\n";
    project.write("lib/model.dart", model);
    let (source, line, character) = mark("import 'model.dart';\nint read(C c) { if (c case C(va▮lue:final captured)) { return captured; } return 0; }\n");
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert_eq!(reply["result"]["uri"], project.uri("lib/model.dart"), "{reply}");
    assert_eq!(reply["result"]["range"]["start"], json!({"line":0,"character":18}), "{reply}");
    let usage = source.find("return captured").unwrap() + "return ".len();
    let character = source[..usage].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    let binding = project.in_position("textDocument/definition", "lib/consumer.dart", 1, character, json!({}));
    assert_eq!(binding["result"]["uri"], project.uri("lib/consumer.dart"), "{binding}");
    assert_eq!(binding["result"]["range"]["start"]["character"], source.lines().nth(1).unwrap().find("captured)").unwrap() as u32);
}

#[test]
fn nested_prefixed_pattern_resolves_inherited_field_not_same_named_getter() {
    let mut project = Project::new("pattern-selector-inherited");
    let model = "class Base { int value = 1; }\nclass C extends Base {}\nclass Other { int get value => 2; }\nclass Outer { C get inner => C(); }\n";
    project.write("lib/model.dart", model);
    let (source, line, character) = mark("import 'model.dart' as m;\nint read(m.Outer o) { if (o case m.Outer(inner:m.C(va▮lue /* colon: keep */ :final captured))) { return captured; } return 0; }\n");
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert_eq!(reply["result"]["uri"], project.uri("lib/model.dart"), "{reply}");
    assert_eq!(reply["result"]["range"]["start"], json!({"line":0,"character":17}), "{reply}");
}

#[test]
fn extension_selector_resolves_actual_extension_getter() {
    let mut project = Project::new("pattern-selector-extension");
    let model = "class C {}\nextension E on C { int get value => 1; }\n";
    project.write("lib/model.dart", model);
    let (source, line, character) = mark("import 'model.dart';\nint read(C c) { if (c case C(va▮lue:final captured)) { return captured; } return 0; }\n");
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert_eq!(reply["result"]["uri"], project.uri("lib/model.dart"), "{reply}");
    assert_eq!(reply["result"]["range"]["start"], json!({"line":1,"character":27}), "{reply}");
}

#[test]
fn shorthand_binding_and_record_label_do_not_become_getter_references() {
    let mut project = Project::new("pattern-selector-refusals");
    let (source, line, character) = mark("class C { int get value => 1; }\nint read(C c) { if (c case C(:final va▮lue)) { return value; } return 0; }\n");
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert_eq!(reply["result"]["range"]["start"], json!({"line":1,"character":character-2}), "{reply}");
    let (record, line, character) = mark("int read(({int value}) r) { final (va▮lue:final captured) = r; return captured; }\n");
    project.open("lib/consumer.dart", &record);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert!(reply["result"].is_null(), "record labels must not resolve as class members: {reply}");
    let (unknown, line, character) = mark("class C {}\nint value = 7;\nint read(C c) { if (c case C(va▮lue:final captured)) { return captured; } return 0; }\n");
    project.open("lib/consumer.dart", &unknown);
    let reply = project.in_position("textDocument/definition", "lib/consumer.dart", line, character, json!({}));
    assert!(reply["result"].is_null(), "unresolved selector must not fall back to a same-named top-level variable: {reply}");
}

#[test]
fn explicit_selector_rename_changes_only_the_resolved_member_family() {
    let mut project = Project::new("pattern-selector-rename");
    let model = "class C { int get value => 1; }\nclass Other { int get value => 2; }\n";
    project.write("lib/model.dart", model);
    let (source, line, character) = mark("import 'model.dart';\nint read(C c, Other o) { if (c case C(va▮lue:final value)) { return value + c.value + o.value; } return 0; }\n");
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position("textDocument/rename", "lib/consumer.dart", line, character, json!({"newName":"item"}));
    assert_eq!(apply(&reply["result"], &project.uri("lib/model.dart"), model), model.replacen("value", "item", 1), "{reply}");
    assert_eq!(apply(&reply["result"], &project.uri("lib/consumer.dart"), &source), source.replace("C(value:", "C(item:").replace("c.value", "c.item"), "{reply}");
}
