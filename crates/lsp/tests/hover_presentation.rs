//! Hover library text and synthetic accessors follow the configured host SDK.
mod common;
use common::Project;
use serde_json::json;
use std::path::Path;

const SOURCE: &str = "class Box { @Deprecated('old') int field = 1; }\nclass Child extends Box {}\nvoid main() { final value = Child(); value.field; }\n";

fn copy_sdk(from: &Path, to: &Path, version: &str) {
    for file in [
        "lib/libraries.json",
        "lib/core/core.dart",
        "lib/math/math.dart",
        "lib/math/aleatorio.dart",
        "lib/interna/interna.dart",
    ] {
        let destination = to.join(file);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(from.join(file), destination).unwrap();
    }
    std::fs::write(to.join("version"), version).unwrap();
}

fn exercise(name: &str, initial: Option<&str>, configured: Option<&str>, modern: bool) {
    let mut project = Project::new(name);
    if let Some(version) = initial {
        std::fs::write(project.root.join("sdk/version"), version).unwrap();
    }
    let mut options =
        json!({"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}});
    if let Some(version) = configured {
        let sdk = project.root.join("client-sdk");
        copy_sdk(&project.root.join("sdk"), &sdk, version);
        options["initializationOptions"] = json!({"sdkPath":sdk});
    }
    project.restart(options);
    project.open("lib/main.dart", SOURCE);
    let offset = SOURCE.find("value.field").unwrap() + "value.".len() + 1;
    let prefix = &SOURCE[..offset];
    let response = project.in_position(
        "textDocument/hover",
        "lib/main.dart",
        prefix.matches('\n').count() as u32,
        prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        json!({}),
    );
    let text = response["result"]["contents"]["value"].as_str().unwrap();
    let description = if modern {
        "int get field"
    } else {
        "(deprecated) int field"
    };
    assert!(
        text.starts_with(&format!("```dart\n{description}\n```")),
        "{response}"
    );
    let footer = if modern {
        "Declared in `Box` in *package:projeto/main.dart*."
    } else {
        "*package:projeto/main.dart*"
    };
    assert!(text.ends_with(footer), "{response}");
}

#[test]
fn installed_sdk_controls_markdown_library_text() {
    exercise("hover-library-floor", Some("3.6.2\n"), None, false);
    exercise("hover-library-modern", Some("3.13.5\n"), None, true);
    exercise("hover-library-reduced", None, None, false);
}

#[test]
fn sdk_path_option_replaces_floor_presentation_with_modern() {
    exercise(
        "hover-library-select-modern",
        Some("3.6.2\n"),
        Some("3.13.5\n"),
        true,
    );
}

#[test]
fn sdk_path_option_replaces_modern_presentation_with_floor() {
    exercise(
        "hover-library-select-floor",
        Some("3.13.5\n"),
        Some("3.6.2\n"),
        false,
    );
}

#[test]
fn hover_without_declaring_type_keeps_the_legacy_transport_shape() {
    let original = json!({
        "range": {"start": 0, "end": 5},
        "description": "int field",
        "ty": "int",
        "documentation": null,
        "library": "package:example/model.dart"
    });
    let hover: dartforge_lsp::Hover = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(hover.declaring_type, None);
    assert_eq!(serde_json::to_value(hover).unwrap(), original);
}
