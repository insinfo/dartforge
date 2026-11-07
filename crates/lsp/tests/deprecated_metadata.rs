//! Annotation metadata follows resolved core identity in hover and completion.
mod common;
use common::{Project, item};
use serde_json::json;

const SOURCE: &str = include_str!("data/deprecated_metadata.dart");
// Expected properties were recorded from both official SDK 3.6.2 and 3.13.5
// servers for this exact source, including invalid-code recovery cases.
const CASES: &[(&str, bool)] = &[
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
];

fn project(name: &str) -> Project {
    let project = Project::with_capabilities(
        name,
        json!({"textDocument": {
            "completion": {"completionItem": {"deprecatedSupport": true,
                "tagSupport": {"valueSet": [1]}}},
            "hover": {"contentFormat": ["markdown"]}
        }}),
    );
    // Keep the common minimal SDK while supplying the core constant used by
    // the official fixture. The user's same-named class/constant stay distinct.
    std::fs::write(
        project.root.join("sdk/lib/core/core.dart"),
        format!(
            "{}\nconst Deprecated deprecated = Deprecated('next release');\n",
            common::CORE
        ),
    )
    .unwrap();
    project
}

fn position(offset: usize) -> (u32, u32) {
    let prefix = &SOURCE[..offset];
    (
        prefix.matches('\n').count().try_into().unwrap(),
        prefix
            .rsplit('\n')
            .next()
            .unwrap()
            .encode_utf16()
            .count()
            .try_into()
            .unwrap(),
    )
}

#[test]
fn completion_deprecation_uses_core_identity_and_constructor_aliases() {
    let mut project = project("metadata-completion-identity");
    project.open("lib/main.dart", SOURCE);
    let (line, column) = position(SOURCE.find("core.List< >").unwrap() + "core.List<".len());
    let response = project.in_position(
        "textDocument/completion",
        "lib/main.dart",
        line,
        column,
        json!({}),
    );
    for &(name, expected) in CASES {
        let candidate = item(&response, name);
        assert_eq!(
            candidate
                .get("deprecated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            expected,
            "{name}: {candidate}"
        );
        assert_eq!(
            candidate.get("tags").is_some_and(|tags| tags
                .as_array()
                .unwrap()
                .iter()
                .any(|tag| tag == 1)),
            expected,
            "{name}: {candidate}"
        );
    }
}

#[test]
fn hover_deprecation_uses_core_identity_and_constructor_aliases() {
    let mut project = project("metadata-hover-identity");
    project.open("lib/main.dart", SOURCE);
    for &(name, expected) in CASES {
        let (line, column) =
            position(SOURCE.find(&format!("class {name} ")).unwrap() + "class ".len() + 1);
        let response = project.in_position(
            "textDocument/hover",
            "lib/main.dart",
            line,
            column,
            json!({}),
        );
        let contents = response["result"]["contents"]["value"].as_str().unwrap();
        assert_eq!(
            contents.starts_with(&format!("```dart\n(deprecated) class {name}\n")),
            expected,
            "{name}: {response}"
        );
    }
}
