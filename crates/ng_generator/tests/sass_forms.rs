//! Native Sass (`sass::compile_with`) against the real `sass_builder` 2.2.1,
//! shape by shape and in both styles (`tests/sass_formas`, oracle recorded by
//! `scripts/sass-forms.sh` with the dart-sass 1.102.0 of the lock).
//!
//! Criterion: for each shape and style, the native compiler returns
//! **exactly** the bytes the builder wrote. A shape that the official one
//! rejects (`.erro`) must be refused. No refusal where the official one
//! compiles, and no approximate output.
use dartforge_ng_generator::sass::{Style, compile_with};
use std::path::Path;

#[test]
fn sass_shapes_equal_to_sass_builder() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sass_formas");
    let sources = base.join("fontes");
    let mut shapes: Vec<_> = std::fs::read_dir(&sources)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('f'))
        })
        .collect();
    shapes.sort();
    assert!(shapes.len() >= 40, "shapes: {}", shapes.len());
    let mut lines = Vec::new();
    let mut different = Vec::new();
    for (style, name) in [
        (Style::Compressed, "compressed"),
        (Style::Expanded, "expanded"),
    ] {
        let (mut equal, mut refused) = (0, Vec::new());
        for f in &shapes {
            let shape = f.file_stem().unwrap().to_string_lossy().to_string();
            let text = std::fs::read_to_string(f).unwrap();
            let expected =
                std::fs::read(base.join("esperado").join(format!("{shape}.{name}.css"))).ok();
            match (compile_with(&text, Some(&sources), style), expected) {
                (Ok((css, _)), Some(e)) if css.as_bytes() == e.as_slice() => equal += 1,
                (Ok((css, _)), Some(e)) => different.push(format!(
                    "{shape} ({name}): native {css:?} ≠ official {:?}",
                    String::from_utf8_lossy(&e)
                )),
                (Ok(_), None) => different.push(format!(
                    "{shape} ({name}): the official one rejects and the native one generated"
                )),
                (Err(_), None) => refused.push(shape),
                (Err(_), Some(_)) => different.push(format!(
                    "{shape} ({name}): the official one compiles and the native one refused"
                )),
            }
        }
        lines.push(format!(
            "{name}: {equal} equal, {} refused as in the official one, of {} — refused: {}",
            refused.len(),
            shapes.len(),
            refused.join(" ")
        ));
    }
    println!("{}", lines.join("\n"));
    assert!(different.is_empty(), "{}", different.join("\n"));
}
