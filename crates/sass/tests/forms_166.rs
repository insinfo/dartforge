//! The compatibility mode with dart-sass 1.66.0
//! (`DartSassVersion::V1_66_0`) against the real dart-sass 1.66.0, shape by
//! shape and in both styles (`tests/formas_166`, oracle recorded by
//! `scripts/sass-forms-166.sh`); and the same shapes in the default mode
//! against dart-sass 1.102.0 (`esperado_102/`), so that the differences
//! between the versions are checked from both sides.
//!
//! Criterion: each `f*` shape comes out with **exactly** the oracle's
//! bytes, or is refused when the oracle refuses it (`.erro`). The `r*`
//! shapes 1.66 compiles, but the 1.66 mode does not guarantee the output and
//! must refuse them — never generate something else.
use dartforge_sass::{from_path, Options, OutputStyle, DartSassVersion};
use std::path::Path;

#[test]
fn shapes_match_dart_sass_166() {
    check("esperado", DartSassVersion::V1_66_0);
}

/// Shapes where the 1.102 port still comes out like 1.66 (known
/// divergences of the default mode, not of the 1.66 mode): the `&` at the root
/// (dart-sass 1.99), which the port refuses. The test
/// requires them to keep diverging: when one is fixed, it leaves this list.
const PENDING_102: &[&str] = &["f26_pai_na_raiz"];

#[test]
fn shapes_match_dart_sass_102() {
    check("esperado_102", DartSassVersion::V1_102_0);
}

fn check(dir: &str, version: DartSassVersion) {
    let pending: &[&str] = if version == DartSassVersion::V1_102_0 {
        PENDING_102
    } else {
        &[]
    };
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/formas_166");
    let mut shapes: Vec<_> = std::fs::read_dir(base.join("fontes"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    shapes.sort();
    assert!(shapes.len() >= 25, "shapes: {}", shapes.len());
    let mut different = Vec::new();
    let mut equal = 0;
    let mut pending_equal = Vec::new();
    for f in &shapes {
        let shape = f.file_stem().unwrap().to_string_lossy().to_string();
        let is_pending = pending.contains(&shape.as_str());
        let before = different.len();
        let mode_refusal = shape.starts_with('r') && version == DartSassVersion::V1_66_0;
        let official_error = base.join(dir).join(format!("{shape}.erro")).is_file();
        for (style, name) in [
            (OutputStyle::Expanded, "expanded"),
            (OutputStyle::Compressed, "compressed"),
        ] {
            let options = Options::default().style(style).quiet(true).version(version);
            let r = from_path(f, &options);
            let expected =
                std::fs::read_to_string(base.join(dir).join(format!("{shape}.{name}.css")))
                    .ok()
                    .map(|mut css| {
                        // `from_path` ends `expanded` with `\n`.
                        if !css.is_empty() && style == OutputStyle::Expanded {
                            css.push('\n');
                        }
                        css
                    });
            match (r, expected) {
                (Err(_), _) if mode_refusal => equal += 1,
                (Ok(css), _) if mode_refusal => different.push(format!(
                    "{shape} ({name}): the mode should refuse and generated {css:?}"
                )),
                (Err(_), None) if official_error => equal += 1,
                (Ok(css), Some(e)) if css == e => equal += 1,
                (Ok(css), Some(e)) => {
                    different.push(format!("{shape} ({name}): mode {css:?} ≠ oracle {e:?}"))
                }
                (Ok(css), None) => different.push(format!(
                    "{shape} ({name}): the oracle refuses and the mode generated {css:?}"
                )),
                (Err(e), _) => different.push(format!(
                    "{shape} ({name}): the oracle compiles and the mode refused: {e}"
                )),
            }
        }
        if is_pending {
            if different.len() == before {
                pending_equal.push(shape);
            }
            different.truncate(before);
        }
    }
    println!("{equal} equal out of {}", shapes.len() * 2);
    assert!(different.is_empty(), "{}", different.join("\n"));
    assert!(
        pending_equal.is_empty(),
        "pending shapes that already come out equal (remove them from the list): {pending_equal:?}"
    );
}
