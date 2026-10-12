//! A2 acceptance: the 7-error probe produces **the same JSON, byte for byte**,
//! as the one `dart analyze --format=json` of SDK 3.6.2 recorded
//! (`corpus/diagnosticos/sonda/dart-analyze.json`, with the file path replaced
//! by `<SONDA>`), given each diagnostic's code, arguments and range — that is:
//! the generated table, template rendering, severity, type, documentation
//! URL, UTF-16/line/column conversion and reporting order are right. The
//! analysis (`types`, request T1) decides code, arguments and range; the
//! scoreboard of the `sonda` group measures how much of it already gets there.

use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_parity::json;

fn d(unique: &str, start: usize, end: usize, args: &[&str]) -> Diagnostic {
    let c = Codigo::por_unico(unique).unwrap_or_else(|| panic!("code {unique}"));
    Diagnostic::com_codigo(c, Span { start, end }, args.iter().copied())
}

#[test]
fn seven_error_probe_matches_the_oracle() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnosticos/sonda");
    let source = std::fs::read_to_string(root.join("lib/a.dart")).expect("probe");
    let expected = std::fs::read_to_string(root.join("dart-analyze.json")).expect("recorded oracle");
    let lines = json::Linhas::new(&source);
    let diags = [
        d("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION", 41, 44, &["String", "int", "f"]),
        d("CompileTimeErrorCode.UNDEFINED_FUNCTION", 83, 84, &["h"]),
        d("CompileTimeErrorCode.NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE", 105, 106, &["y"]),
        d("CompileTimeErrorCode.ARGUMENT_TYPE_NOT_ASSIGNABLE", 113, 116, &["String", "int", ""]),
        d("CompileTimeErrorCode.NON_BOOL_CONDITION", 125, 126, &[]),
        d("CompileTimeErrorCode.UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE", 148, 154, &["length"]),
        d("WarningCode.UNUSED_LOCAL_VARIABLE", 162, 167, &["nunca"]),
    ];
    // Deliberately shuffled order: the reporting order comes from `json::ordenar`.
    let mut v: Vec<_> = diags.iter().rev().map(|x| json::para_json("<SONDA>", &lines, x, false)).collect();
    json::ordenar(&mut v);
    assert_eq!(json::escrever(v), expected.trim_end());
}
