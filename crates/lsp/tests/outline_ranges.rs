//! Complete outline payloads recorded from the official SDK 3.6.2 server.
mod common;
use common::Project;
use serde_json::{Value, json};

const SOURCE: &str = include_str!("data/outline_ranges.dart");

fn assert_outline(name: &str, source: &str, expected: &str) {
    let mut project = Project::with_capabilities(
        name,
        json!({"textDocument": {
            "documentSymbol": {"hierarchicalDocumentSymbolSupport": true}
        }}),
    );
    project.open("lib/main.dart", source);
    let value = project.request(
        "textDocument/documentSymbol",
        json!({
            "textDocument": {"uri": project.uri("lib/main.dart")}
        }),
    );
    let expected: Value = serde_json::from_str(expected).unwrap();
    assert_eq!(value["result"], expected);
}

#[test]
fn complete_annotated_outline_matches_official_ranges_and_details() {
    assert_outline(
        "outline-ranges-lf",
        SOURCE,
        include_str!("data/outline_ranges_lf.json"),
    );
}

#[test]
fn complete_outline_preserves_utf16_and_crlf_coordinates() {
    let source = format!("/*🧭 UTF16 prefix*/\r\n{}", SOURCE.replace('\n', "\r\n"));
    assert_outline(
        "outline-ranges-crlf",
        &source,
        include_str!("data/outline_ranges_utf16-crlf.json"),
    );
}
