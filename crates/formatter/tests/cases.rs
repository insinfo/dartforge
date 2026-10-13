//! Fixed formatter cases: every `tests/cases/<name>.dart`, once formatted, must
//! equal `<name>.expected` byte for byte, generated with
//! `dart format -o show --summary none` from SDK 3.6.2. To (re)generate an
//! expected file: `dart format -o show --summary none cases/x.dart > cases/x.expected`.

use std::path::Path;

use dartforge_formatter::{Opcoes, formatar};

#[test]
fn cases_match_dart_format() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .expect("tests/cases")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "dart"))
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut failures = Vec::new();
    for p in &names {
        let source = std::fs::read_to_string(p).expect("case");
        let expected = std::fs::read_to_string(p.with_extension("expected")).expect("expected");
        match formatar(&source, &Opcoes::default()) {
            Ok(output) if output == expected => {}
            Ok(output) => {
                let line = output
                    .lines()
                    .zip(expected.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(output.lines().count().min(expected.lines().count()));
                failures.push(format!(
                    "{}: differs at line {}:\n  ours:     {:?}\n  expected: {:?}",
                    p.display(),
                    line + 1,
                    output.lines().nth(line).unwrap_or(""),
                    expected.lines().nth(line).unwrap_or("")
                ));
            }
            Err(e) => failures.push(format!("{}: error {e}", p.display())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn syntax_error_is_rejected() {
    assert!(formatar("void main( {", &Opcoes::default()).is_err());
}

#[test]
fn configurable_line_length() {
    let o = Opcoes {
        largura: 20,
        ..Opcoes::default()
    };
    let output = formatar("void main(){var x=[1,2,3,];print(x);}", &o).unwrap();
    assert_eq!(output, "void main() {\n  var x = [\n    1,\n    2,\n    3,\n  ];\n  print(x);\n}\n");
}
