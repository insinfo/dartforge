//! Constructor hover uses the creation's instantiated type and complete name.
mod common;
use common::Project;
use serde_json::json;

const SOURCE: &str = "class Box<T> { const Box(); const Box.named(); }\r\ntypedef Alias<T> = Box<T>;\r\nvoid main() {\r\n  // 😀 keep UTF-16 positions independent of UTF-8 spans\r\n  Box<int>();\r\n  Box<int>.named();\r\n  new Box<int>();\r\n  const Box<int>.named();\r\n  Alias<int>.named();\r\n  final constructor = Box<int>.named;\r\n}\r\n";

fn exercise(modern: bool) {
    let mut project = Project::new(if modern {
        "hover-constructors-modern"
    } else {
        "hover-constructors-floor"
    });
    std::fs::write(
        project.root.join("sdk/version"),
        if modern { "3.13.5\n" } else { "3.6.2\n" },
    )
    .unwrap();
    project
        .restart(json!({"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}));
    project.open("lib/main.dart", SOURCE);
    for (needle, cursor, name, implicit, range_text) in [
        ("  Box<int>();", "Box", "", true, "Box<int>"),
        (
            "  Box<int>.named();",
            "Box",
            "named",
            true,
            "Box<int>.named",
        ),
        (
            "  Box<int>.named();",
            "named",
            "named",
            true,
            "Box<int>.named",
        ),
        ("  new Box<int>();", "new", "", false, "Box<int>"),
        ("  new Box<int>();", "Box", "", false, "Box<int>"),
        (
            "  const Box<int>.named();",
            "Box",
            "named",
            false,
            "Box<int>.named",
        ),
        (
            "  const Box<int>.named();",
            "named",
            "named",
            false,
            "Box<int>.named",
        ),
        (
            "  Alias<int>.named();",
            "Alias",
            "named",
            true,
            "Alias<int>.named",
        ),
        (
            "  Alias<int>.named();",
            "named",
            "named",
            true,
            "Alias<int>.named",
        ),
        (
            "  final constructor = Box<int>.named;",
            "named",
            "named",
            false,
            "named",
        ),
    ] {
        let start = SOURCE.find(needle).unwrap();
        let at = start + needle.find(cursor).unwrap() + 1;
        let prefix = &SOURCE[..at];
        let line = prefix.matches('\n').count();
        let character = prefix.rsplit('\n').next().unwrap().encode_utf16().count();
        let result = project.in_position(
            "textDocument/hover",
            "lib/main.dart",
            line as u32,
            character as u32,
            json!({}),
        );
        let suffix = if name.is_empty() {
            String::new()
        } else {
            format!(".{name}")
        };
        let signature = if modern {
            format!("Box<int>{suffix}()")
        } else {
            format!("Box<int> Box{suffix}()")
        };
        let marker = if implicit { "(new) " } else { "" };
        let footer = if modern {
            "Declared in `Box` in *package:projeto/main.dart*."
        } else {
            "*package:projeto/main.dart*"
        };
        let range_start = needle.find(range_text).unwrap();
        assert_eq!(
            result["result"],
            json!({
                "contents":{"kind":"markdown","value":format!("```dart\n{marker}{signature}\n```\n{footer}")},
                "range":{"start":{"line":line,"character":range_start},"end":{"line":line,"character":range_start+range_text.len()}}
            }),
            "{needle}: {cursor}"
        );
    }
}

#[test]
fn floor_sdk_constructor_names_ranges_and_substitutions() {
    exercise(false);
}

#[test]
fn modern_sdk_constructor_names_ranges_and_substitutions() {
    exercise(true);
}
