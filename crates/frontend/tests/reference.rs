//! The reference of a unit (docs/ANALYZER-ESPECIFICACAO.md, T2): which
//! analyzer gives the name and text of the file's diagnostics. The cases are
//! those of `corpus/specification/t2/v`, with the outputs of both live
//! oracles. Written without compiling or running (2026-10-05).

use dartforge_frontend::features::{LibraryFeatures, AnalyzerReference};
use dartforge_frontend::parser::parse_with;
use dartforge_intern::Interner;

/// The unit's reference in a 3.6 library and, already in its form, each
/// diagnostic: code, excerpt, message and fix.
fn read(source: &str) -> (AnalyzerReference, Vec<(&'static str, String, String, String)>) {
    let mut names = Interner::new();
    let read_ = parse_with(source, &mut names, LibraryFeatures::floor());
    let reference = read_.reference;
    let mut v: Vec<_> = read_
        .diagnostics
        .into_iter()
        .filter_map(|d| d.for_reference(reference))
        .map(|d| {
            let correction = d.correction().unwrap_or_default();
            (d.code.map_or("", |c| c.info().name), source[d.span.start..d.span.end].to_string(), d.message, correction)
        })
        .collect();
    v.sort();
    (reference, v)
}

/// c01: 3.6.2 knows `null-aware-elements` (without a release version), and
/// writes `3.6.0` in the fix; the file stays with it.
#[test]
fn null_element_stays_with_3_6() {
    let (reference, v) = read("void f(int? a) {\n  var x = [?a];\n  print(x);\n}\n");
    assert_eq!(reference, AnalyzerReference::V3_6);
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!(v[0].0, "experiment_not_enabled");
    assert_eq!(v[0].2, "This requires the 'null-aware-elements' language feature to be enabled.");
    assert!(v[0].3.contains("to 3.6.0 or higher"), "{}", v[0].3);
}

/// c08 and c18: syntax 3.6.2 does not know moves the file to 3.13.4.
#[test]
fn unknown_syntax_falls_to_3_13() {
    let (reference, v) = read("enum E { a, b }\n\nvoid f() {\n  E e = .a;\n  print(e);\n}\n");
    assert_eq!(reference, AnalyzerReference::V3_13);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].3.contains("to 3.10.0 or higher"), "{}", v[0].3);

    let (reference, v) = read("class A {\n  int _x;\n  A({required this._x});\n}\n");
    assert_eq!(reference, AnalyzerReference::V3_13);
    assert!(v.iter().any(|d| d.0 == "experiment_not_enabled" && d.1.ends_with("_x") && d.3.contains("3.12.0")), "{v:?}");
}

/// c02: the `final` of the representation field, in 3.6.2's text.
#[test]
fn representation_modifier_in_3_6() {
    let (reference, v) = read("extension type E(final int i) {}\n");
    assert_eq!(reference, AnalyzerReference::V3_6);
    assert_eq!(v, [("representation_field_modifier", "final".to_string(), "Representation fields can't have modifiers.".to_string(), "Try removing the modifier.".to_string())]);

    // Without a type, 3.6.2 reports the missing type at the `final` itself
    // (c23, F).
    let (reference, v) = read("extension type F(final i) {}\n");
    assert_eq!(reference, AnalyzerReference::V3_6);
    let v: Vec<(&str, &str)> = v.iter().map(|d| (d.0, d.1.as_str())).collect();
    assert_eq!(v, [("expected_representation_type", "final"), ("representation_field_modifier", "final")]);

    // With two fields, 3.6.2 reports the modifier and the comma (c23, G).
    let (_, v) = read("extension type G(final int i, final int j) {}\n");
    let v: Vec<(&str, &str)> = v.iter().map(|d| (d.0, d.1.as_str())).collect();
    assert_eq!(v, [("multiple_representation_fields", ","), ("representation_field_modifier", "final")]);
}

/// All of c23: the declaring `var` makes 3.13.4 the reference, and then the
/// modifier comes out with the new text, the missing type moves to the name
/// and the modifier of `G` is not reported.
#[test]
fn representation_with_var_in_3_13() {
    let source = "extension type E(var int i) {}\n\nextension type F(final i) {}\n\nextension type G(final int i, final int j) {}\n";
    let (reference, v) = read(source);
    assert_eq!(reference, AnalyzerReference::V3_13);
    let new_ = "Representation fields can't have the modifier 'var'.";
    let summary: Vec<(&str, &str, &str)> = v.iter().map(|d| (d.0, d.1.as_str(), d.2.as_str())).collect();
    assert_eq!(
        summary,
        [
            ("expected_representation_type", "i", "Expected a representation type."),
            ("experiment_not_enabled", "var", "This requires the 'primary-constructors' language feature to be enabled."),
            ("multiple_representation_fields", ",", "Each extension type should have exactly one representation field."),
            ("representation_field_modifier", "final", new_),
            ("representation_field_modifier", "var", new_),
        ]
    );
    let feature = v.iter().find(|d| d.0 == "experiment_not_enabled").unwrap();
    assert!(feature.3.contains("to 3.13 or higher"), "{}", feature.3);
}

/// Above 3.6 the reference is always 3.13.4.
#[test]
fn new_library_is_from_3_13() {
    let mut names = Interner::new();
    let read_ = parse_with("void main() {}\n", &mut names, LibraryFeatures::current());
    assert_eq!(read_.reference, AnalyzerReference::V3_13);
    let read_ = parse_with("void main() {}\n", &mut names, LibraryFeatures::floor());
    assert_eq!(read_.reference, AnalyzerReference::V3_6);
}

/// `named_function_expression` (docs/ANALYZER-ESPECIFICACAO.md, family E):
/// the examples from the live 3.6.2 oracle, with the position of the name.
#[test]
fn function_literal_with_name() {
    let named_ones = |source: &str| -> Vec<(usize, usize)> {
        let mut names = Interner::new();
        parse_with(source, &mut names, LibraryFeatures::floor())
            .diagnostics
            .iter()
            .filter(|d| d.code.is_some_and(|c| c.info().name == "named_function_expression"))
            .map(|d| (d.span.start, d.span.end - d.span.start))
            .collect()
    };
    assert_eq!(named_ones("f() { var f = void g() {}; }\n"), [(19, 1)]);
    assert_eq!(named_ones("f() { var f = g() {}; }\n"), [(14, 1)]);
    assert_eq!(named_ones("f() { var f = int g<T>() => 1; }\n"), [(18, 1)]);
    // Plain call, local function and constructor initializer: nothing.
    assert!(named_ones("f() { g(); h() {} var x = g(); }\n").is_empty());
    assert!(named_ones("class A {\n  int x;\n  A() : x = g() {}\n}\nint g() => 0;\n").is_empty());
    // The guard of a `switch` expression case.
    assert!(named_ones("int f(int o) => switch (o) { _ when g(o) => 1, _ => 0 };\nbool g(int o) => true;\n").is_empty());
}
