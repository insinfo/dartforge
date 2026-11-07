//! Resolved outline metadata, current import overlays and protocol shapes.
mod common;
use common::Project;
use serde_json::{Value, json};

fn symbols(project: &mut Project) -> Value {
    project.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": project.uri("lib/main.dart")}}),
    )["result"]
        .clone()
}

fn named<'a>(values: &'a Value, name: &str) -> &'a Value {
    values
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()
}

#[test]
fn outline_deprecation_uses_resolved_core_identity() {
    let mut project = Project::with_capabilities(
        "symbol-metadata-identity",
        json!({"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}}),
    );
    std::fs::write(
        project.root.join("sdk/lib/core/core.dart"),
        format!(
            "{}\nconst Deprecated deprecated = Deprecated('next');\n",
            common::CORE
        ),
    )
    .unwrap();
    project.open(
        "lib/main.dart",
        include_str!("data/deprecated_metadata.dart"),
    );
    let result = symbols(&mut project);
    for (name, expected) in [
        ("Old", true),
        ("ExplicitNew", true),
        ("ExtendingOnly", false),
        ("InvalidConstructor", false),
        ("CoreConstant", true),
        ("InvalidConstantCall", true),
        ("CustomAnnotation", false),
        ("CustomConstant", false),
        ("ConstantAlias", false),
        ("ConstructorAlias", true),
        ("MissingCall", true),
        ("MissingMessage", true),
        ("AliasNew", true),
        ("AliasMissing", false),
        ("OptionalOnly", false),
    ] {
        assert_eq!(
            named(&result, name)["deprecated"],
            expected,
            "{name}: {result}"
        );
    }
}

#[test]
fn flat_symbols_preserve_own_flags_without_inheriting_owner_metadata() {
    let mut project = Project::new("symbol-metadata-flat");
    project.open("lib/main.dart", "@Deprecated('old') class Old { Old(); void clean() {} @Deprecated('old method') void legacy() {} } class New { @Deprecated('old constructor') New.old(); }");
    let result = symbols(&mut project);
    for (name, container, expected) in [
        ("Old", None, true),
        ("Old", Some("Old"), false),
        ("clean", Some("Old"), false),
        ("legacy", Some("Old"), true),
        ("New", None, false),
        ("New.old", Some("New"), true),
    ] {
        let matches: Vec<_> = result
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["name"] == name && v["containerName"].as_str() == container)
            .collect();
        assert_eq!(matches.len(), 1, "{name}: {result}");
        assert_eq!(matches[0]["deprecated"], expected, "{name}: {result}");
    }
}

#[test]
fn imported_annotation_alias_tracks_unsaved_changes_and_utf16_positions() {
    let mut project = Project::with_capabilities(
        "symbol-metadata-overlays",
        json!({"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}}),
    );
    let original = "import 'dart:core' as core; typedef Legacy = core.Deprecated;";
    let custom = "class Legacy { const Legacy(String message); }";
    project.open("lib/annotations.dart", original);
    let source = "import 'annotations.dart' as marks;\r\n/*🧭*/ @marks.Legacy('old') class Example { @marks.Legacy('old') void legacy() {} void clean() {} }";
    project.open("lib/main.dart", source);
    for (version, annotation_source, expected) in [(2, custom, false), (3, original, true)] {
        project.change("lib/annotations.dart", version, annotation_source);
        let result = symbols(&mut project);
        let class = named(&result, "Example");
        assert_eq!(class["deprecated"], expected, "{result}");
        assert_eq!(named(&class["children"], "legacy")["deprecated"], expected);
        assert_eq!(named(&class["children"], "clean")["deprecated"], false);
        assert_eq!(
            class["selectionRange"]["start"],
            json!({"line": 1, "character": 34})
        );
    }
    assert_eq!(
        std::fs::read_to_string(project.root.join("projeto/lib/annotations.dart")).unwrap(),
        original
    );
}

#[test]
fn plain_outlines_do_not_load_a_semantic_session() {
    let mut project = Project::new("symbol-metadata-plain");
    project.open(
        "lib/main.dart",
        "class Plain { void clean() {} } void main() {}",
    );
    let result = symbols(&mut project);
    assert_eq!(named(&result, "Plain")["deprecated"], false);
    assert_eq!(project.server.analyzer().session_stats().loaded, 0);
}
