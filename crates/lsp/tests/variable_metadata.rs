//! Variable metadata and accessor presentation follow the installed SDK.
mod common;
use common::Project;
use serde_json::{Value, json};

const SOURCE: &str = "import 'dart:core';\nimport 'dart:core' as core;\ntypedef Mark = core.Deprecated;\n@Mark('old') int top = 1, other = 2;\nclass Fields {\n  @Mark('old') int field = 1, sibling = 2;\n  @Mark('old') int get explicit => 1;\n}\nvoid main() {\n  final value = Fields();\n  value.field;\n  value.field = 2;\n  top;\n}\n";

fn hover(project: &mut Project, needle: &str, shift: usize) -> String {
    let prefix = &SOURCE[..SOURCE.find(needle).unwrap() + shift];
    project.in_position(
        "textDocument/hover",
        "lib/main.dart",
        prefix.matches('\n').count() as u32,
        prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        json!({}),
    )["result"]["contents"]["value"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn named<'a>(symbols: &'a Value, name: &str) -> &'a Value {
    symbols
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap()
}

fn exercise(name: &str, version: Option<&str>, modern: bool) {
    let mut project = Project::new(name);
    if let Some(version) = version {
        std::fs::write(project.root.join("sdk/version"), version).unwrap();
    }
    project.restart(json!({"capabilities": {"textDocument": {
        "hover": {"contentFormat": ["markdown"]},
        "documentSymbol": {"hierarchicalDocumentSymbolSupport": true}
    }}}));
    project.open("lib/main.dart", SOURCE);
    for (needle, shift, expected) in [
        ("int top", 5, "(deprecated) int top"),
        ("int field", 5, "(deprecated) int field"),
        (
            "value.field;",
            7,
            if modern {
                "int get field"
            } else {
                "(deprecated) int field"
            },
        ),
        (
            "value.field =",
            7,
            if modern {
                "set field(int value)"
            } else {
                "(deprecated) int field"
            },
        ),
        (
            "  top;",
            3,
            if modern {
                "int get top"
            } else {
                "(deprecated) int top"
            },
        ),
        ("get explicit", 5, "(deprecated) int get explicit"),
    ] {
        let value = hover(&mut project, needle, shift);
        assert!(
            value.starts_with(&format!("```dart\n{expected}\n```")),
            "{needle}: {value}"
        );
    }
    let response = project.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": project.uri("lib/main.dart")}}),
    );
    let symbols = &response["result"];
    for name in ["top", "other"] {
        assert_eq!(named(symbols, name)["deprecated"], !modern, "{response}");
    }
    let members = &named(symbols, "Fields")["children"];
    for name in ["field", "sibling"] {
        assert_eq!(named(members, name)["deprecated"], !modern, "{response}");
    }
    assert_eq!(named(members, "explicit")["deprecated"], true);
}

#[test]
fn floor_sdk_and_reduced_sdk_keep_variable_metadata() {
    exercise("variable-metadata-floor", Some("3.6.2\n"), false);
    exercise("variable-metadata-reduced", None, false);
}

#[test]
fn modern_sdk_uses_accessors_even_in_a_floor_language_package() {
    exercise("variable-metadata-modern", Some("3.13.5\n"), true);
}
