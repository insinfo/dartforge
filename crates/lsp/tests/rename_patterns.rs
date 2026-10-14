//! Local pattern renames preserve the object getter or named record field.

mod common;

use common::{Project, apply, mark};
use serde_json::json;

fn rename_at(project: &mut Project, source: &str, needle: &str) -> serde_json::Value {
    let offset = source.find(needle).unwrap();
    let line = source[..offset].bytes().filter(|b| *b == b'\n').count() as u32;
    let character = source[..offset].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    project.in_position("textDocument/rename", "lib/consumer.dart", line, character, json!({"newName": "item"}))
}

#[test]
fn shorthand_object_binding_or_use_preserves_foreign_getter() {
    for (index, pattern) in [":var value", ":final value", ":final int value", ":int value", ": /* colon: keep */ final value"].iter().enumerate() {
        let mut project = Project::new(&format!("rename-shorthand-{index}"));
        let model = "class C { final int value; C(this.value); }\n";
        project.write("lib/model.dart", model);
        let source = format!("import 'model.dart';\nint read(C c) {{ if (c case C({pattern})) {{ return value; }} return 0; }}\n");
        project.open("lib/consumer.dart", &source);
        let binding = rename_at(&mut project, &source, "value))");
        let usage = rename_at(&mut project, &source, "value;");
        assert_eq!(binding["result"], usage["result"], "{binding}\n{usage}");
        let expected = source.replace(pattern, &format!("value{}", pattern.replace("value", "item")))
            .replace("return value;", "return item;");
        assert_eq!(apply(&usage["result"], &project.uri("lib/consumer.dart"), &source), expected, "{usage}");
        assert!(usage["result"]["changes"].get(project.uri("lib/model.dart")).is_none(), "getter declaration must not be renamed: {usage}");
    }
}

#[test]
fn explicit_object_label_and_positional_record_need_no_insertions() {
    for (index, source) in [
        "class C { int get value => 1; }\nint read(C c) { if (c case C(value:final value)) { return value; } return 0; }\n",
        "int read((int,) record) { final (value,) = record; return value; }\n",
    ].iter().enumerate() {
        let mut project = Project::new(&format!("rename-explicit-pattern-{index}"));
        project.open("lib/consumer.dart", source);
        let result = rename_at(&mut project, source, "value;");
        let expected = source.replace("final value", "final item").replace("(value,)", "(item,)")
            .replace("return value;", "return item;");
        assert_eq!(apply(&result["result"], &project.uri("lib/consumer.dart"), source), expected, "{result}");
        assert_eq!(result["result"]["changes"][project.uri("lib/consumer.dart")].as_array().unwrap().len(), 2);
    }
}

#[test]
fn nested_object_and_named_record_shorthands_preserve_labels() {
    for (index, source) in [
        "class C { int get value => 1; }\nclass Outer { C get inner => C(); }\nint read(Outer o) { if (o case Outer(inner:C(:final value))) { return value; } return 0; }\n",
        "int read(({int value}) record) { final (:value) = record; return value; }\n",
        "class C { int? get value => 1; }\nint read(C c) { if (c case C(:final value!)) { return value; } return 0; }\n",
    ].iter().enumerate() {
        let mut project = Project::new(&format!("rename-nested-pattern-{index}"));
        project.open("lib/consumer.dart", source);
        let result = rename_at(&mut project, source, "value;");
        let expected = source.replace(":final value", "value:final item").replace(":value)", "value:item)")
            .replace("return value;", "return item;");
        assert_eq!(apply(&result["result"], &project.uri("lib/consumer.dart"), source), expected, "{result}");
    }
}

#[test]
fn unrelated_local_does_not_rewrite_pattern_labels_or_comments() {
    let mut project = Project::new("rename-pattern-unrelated");
    let (source, line, character) = mark("class C { int get value => 1; int get other => 2; }\nint read(C c) { var va▮lue = 0; if (c case C(:final other)) { value += other; } return value; } // :value\n");
    project.open("lib/consumer.dart", &source);
    let result = project.in_position("textDocument/rename", "lib/consumer.dart", line, character, json!({"newName": "item"}));
    let expected = source.replace("var value", "var item").replace("value +=", "item +=").replace("return value;", "return item;");
    assert_eq!(apply(&result["result"], &project.uri("lib/consumer.dart"), &source), expected, "{result}");
}
