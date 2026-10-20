//! Object-pattern getter renames preserve independent local bindings.

mod common;

use common::{Project, apply, mark};
use serde_json::json;

#[test]
fn field_rename_updates_resolved_selectors_without_renaming_bindings() {
    for (index, pattern) in [
        ":var value",
        ":final value",
        ":final int value",
        ":int value",
        ": /* colon: keep */ final value",
        "value:final value",
    ]
    .iter()
    .enumerate()
    {
        let mut project = Project::new(&format!("rename-pattern-field-{index}"));
        let (model, line, character) = mark(
            "class C { final int va▮lue; C(this.value); }\nclass Other { int get value => 2; }\n",
        );
        project.open("lib/model.dart", &model);
        let consumer = format!(
            "import 'model.dart';\nint read(C c, Other o) {{ if (c case C({pattern})) {{ if (o case Other(:final value)) {{ return value; }} return value; }} return 0; }}\n"
        );
        project.open("lib/consumer.dart", &consumer);
        let reply = project.in_position(
            "textDocument/rename",
            "lib/model.dart",
            line,
            character,
            json!({"newName":"item"}),
        );
        let edit = &reply["result"];
        assert_eq!(
            apply(edit, &project.uri("lib/model.dart"), &model),
            model.replacen("value", "item", 2),
            "{reply}"
        );
        let replacement = if pattern.starts_with(':') {
            format!("item{pattern}")
        } else {
            "item:final value".into()
        };
        assert_eq!(
            apply(edit, &project.uri("lib/consumer.dart"), &consumer),
            consumer.replacen(pattern, &replacement, 1),
            "{reply}"
        );
    }
}

#[test]
fn inherited_getter_and_nested_pattern_use_resolved_member_identity() {
    let mut project = Project::new("rename-pattern-inherited-getter");
    let (model, line, character) = mark(
        "class Base { int get va▮lue => 1; }\nclass C extends Base {}\nclass Outer { C get inner => C(); }\n",
    );
    project.open("lib/model.dart", &model);
    let consumer = "import 'model.dart';\nint read(Outer o) { if (o case Outer(inner:C(:final value))) { return value; } return 0; }\n";
    project.open("lib/consumer.dart", consumer);
    let reply = project.in_position(
        "textDocument/rename",
        "lib/model.dart",
        line,
        character,
        json!({"newName":"item"}),
    );
    assert_eq!(
        apply(&reply["result"], &project.uri("lib/model.dart"), &model),
        model.replace("value", "item"),
        "{reply}"
    );
    assert_eq!(
        apply(
            &reply["result"],
            &project.uri("lib/consumer.dart"),
            consumer
        ),
        consumer.replace("C(:final value)", "C(item:final value)"),
        "{reply}"
    );
}

#[test]
fn private_getter_rename_keeps_shorthand_selector_insertion() {
    let mut project = Project::new("rename-pattern-private-getter");
    let (source, line, character) = mark(
        "class C { int get va▮lue => 1; }\nint read(C c) { if (c case C(:final value)) { return value; } return 0; }\n",
    );
    project.open("lib/consumer.dart", &source);
    let reply = project.in_position(
        "textDocument/rename",
        "lib/consumer.dart",
        line,
        character,
        json!({"newName":"_item"}),
    );
    let expected = source
        .replacen("get value", "get _item", 1)
        .replace("C(:final value)", "C(_item:final value)");
    assert_eq!(
        apply(&reply["result"], &project.uri("lib/consumer.dart"), &source),
        expected,
        "{reply}"
    );
}
