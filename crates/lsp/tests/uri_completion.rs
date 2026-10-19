//! File URI suggestions preserve editor text while decoding filesystem lookup.
mod common;

use common::{Project, item, labels, mark};
use serde_json::{Value, json};

fn suggest(project: &mut Project, partial: &str) -> Value {
    let (text, line, character) = mark(&format!("import '{partial}▮';\nvoid main() {{}}\n"));
    project.open("lib/main.dart", &text);
    project.in_position(
        "textDocument/completion",
        "lib/main.dart",
        line,
        character,
        json!({}),
    )
}

#[test]
fn encoded_relative_directories_match_dart_server_suggestions() {
    let mut project = Project::new("uri-encoded-relative");
    for (folder, file) in [
        ("folder space", "hello.dart"),
        ("folder space", "fancy é name.dart"),
        ("percent%literal", "p.dart"),
        ("hash#tag", "h.dart"),
    ] {
        project.write(&format!("lib/{folder}/{file}"), "class Example {}\n");
    }
    // Dart 3.6.2 decodes only the lookup path. The typed URI prefix stays as
    // written, and newly suggested filenames keep spaces and Unicode verbatim.
    for (partial, files) in [
        ("folder%20space/", vec!["fancy é name.dart", "hello.dart"]),
        ("percent%25literal/", vec!["p.dart"]),
        ("hash%23tag/", vec!["h.dart"]),
    ] {
        let result = suggest(&mut project, partial);
        let mut actual: Vec<_> = labels(&result)
            .into_iter()
            .filter(|label| label.starts_with(partial))
            .collect();
        actual.sort();
        let expected: Vec<_> = files
            .iter()
            .map(|file| format!("{partial}{file}"))
            .collect();
        assert_eq!(actual, expected, "{partial}: {result}");
        for label in expected {
            assert_eq!(item(&result, &label)["textEdit"]["newText"], label);
        }
    }
}

#[test]
fn encoded_absolute_file_uri_is_resolved_as_a_uri() {
    let mut project = Project::new("uri absolute with spaces");
    project.write("lib/folder space/hello.dart", "class Example {}\n");
    let partial = url::Url::from_directory_path(project.path("lib/folder space"))
        .unwrap()
        .to_string();
    assert!(partial.contains("%20"));
    let result = suggest(&mut project, &partial);
    let label = format!("{partial}hello.dart");
    assert!(labels(&result).contains(&label), "{result}");
    assert_eq!(item(&result, &label)["textEdit"]["newText"], label);
}

#[test]
fn unescaped_directory_and_package_prefix_behavior_are_preserved() {
    let mut project = Project::new("uri-existing-controls");
    project.write("lib/folder space/hello.dart", "class Example {}\n");
    let result = suggest(&mut project, "folder space/");
    assert!(
        labels(&result).contains(&"folder space/hello.dart".into()),
        "{result}"
    );
    // The official server does not complete this encoded package directory.
    let result = suggest(&mut project, "package:projeto/folder%20space/");
    assert!(
        !labels(&result)
            .iter()
            .any(|label| label.ends_with("hello.dart")),
        "{result}"
    );
}
