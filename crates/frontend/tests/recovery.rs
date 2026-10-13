//! Parser error recovery as in the fasta of `dart analyze` 3.6.2
//! (`_fe_analyzer_shared` 76.0.0 and the analyzer 6.11.0 `AstBuilder`): each
//! case is an oracle sample (`corpus/diags`), with the exact code and
//! excerpt the analyzer reports.
use dartforge_frontend::parser::{Parsed, parse_com};
use dartforge_frontend::{LanguageVersion, LibraryFeatures};
use dartforge_intern::Interner;

fn parse_in(versao: (u16, u16), fonte: &str) -> Parsed {
    let mut names = Interner::new();
    let f = LibraryFeatures::new(LanguageVersion::new(versao.0, versao.1), &[]);
    parse_com(fonte, &mut names, f)
}

/// `(code, excerpt text)` of each diagnostic, in position order (and code
/// order, for the same excerpt).
fn errors_in(versao: (u16, u16), fonte: &str) -> Vec<(String, String)> {
    let p = parse_in(versao, fonte);
    let mut ds: Vec<_> = p
        .diagnostics
        .iter()
        .map(|d| {
            let code = d.code.map(|c| c.info().name.to_string()).unwrap_or_default();
            (d.span.start, d.span.end, code, fonte[d.span.start..d.span.end].to_string())
        })
        .collect();
    ds.sort();
    ds.into_iter().map(|(_, _, c, t)| (c, t)).collect()
}

fn errors(fonte: &str) -> Vec<(String, String)> {
    errors_in((3, 6), fonte)
}

fn pair(c: &str, t: &str) -> (String, String) {
    (c.to_string(), t.to_string())
}

#[test]
fn out_of_order_and_stray_member_modifiers() {
    assert_eq!(
        errors("class C { final late int x = 0; }"),
        vec![pair("modifier_out_of_order", "late")]
    );
    assert_eq!(
        errors("class C { late covariant var x; }"),
        vec![pair("modifier_out_of_order", "covariant")]
    );
    assert_eq!(
        errors("class C { static external void f(); }"),
        vec![pair("modifier_out_of_order", "external")]
    );
    assert_eq!(errors("class C { required int x = 0; }"), vec![pair("extraneous_modifier", "required")]);
    assert_eq!(errors("class C { final final int x = 0; }"), vec![pair("duplicated_modifier", "final")]);
}

#[test]
fn stray_top_level_modifiers() {
    assert_eq!(errors("static int f() => 1;"), vec![pair("extraneous_modifier", "static")]);
    assert_eq!(errors("abstract int x = 0;"), vec![pair("extraneous_modifier", "abstract")]);
    assert_eq!(errors("const t() => null;"), vec![pair("extraneous_modifier", "const")]);
    assert_eq!(errors("var f() => null;"), vec![pair("var_return_type", "var")]);
    assert_eq!(errors("abstract enum E { a }"), vec![pair("extraneous_modifier", "abstract")]);
    assert_eq!(errors("const class C {}"), vec![pair("const_class", "const")]);
}

#[test]
fn parameter_modifiers() {
    assert_eq!(errors("f(x, static int y) {}"), vec![pair("extraneous_modifier", "static")]);
    assert_eq!(errors("f(x, {static y}) {}"), vec![pair("extraneous_modifier", "static")]);
    assert_eq!(errors("f6(const p1) {}"), vec![pair("extraneous_modifier", "const")]);
    assert_eq!(errors("void f(covariant int x) {}"), vec![pair("extraneous_modifier", "covariant")]);
    assert_eq!(
        errors("class C { void m({covariant required int? i}) {} }"),
        vec![pair("modifier_out_of_order", "required")]
    );
    // Valid: no diagnostics.
    assert!(errors("class C { C({required this.x, covariant int y = 0}); int x; }").is_empty());
    assert!(errors("void f({required}) {} void g(int required) {}").is_empty());
}

#[test]
fn wrong_class_modifier_order_becomes_member() {
    // `final abstract class` is an unnamed `final` field followed by the class.
    assert_eq!(
        errors("final abstract class A {}"),
        vec![
            pair("expected_token", "abstract"),
            pair("extraneous_modifier", "abstract"),
            pair("modifier_out_of_order", "abstract"),
            pair("missing_identifier", "class"),
        ]
    );
    assert_eq!(
        errors("final final class A {}"),
        vec![pair("expected_token", "final"), pair("missing_identifier", "final")]
    );
    assert_eq!(errors("sealed mixin M {}"), vec![pair("sealed_mixin", "sealed")]);
    assert_eq!(errors("final mixin class M {}"), vec![pair("final_mixin_class", "final")]);
    assert_eq!(errors("base enum E { a }"), vec![pair("base_enum", "base")]);
    assert_eq!(errors("abstract sealed class S {}"), vec![pair("abstract_sealed_class", "sealed")]);
    // Valid.
    for v in [
        "abstract class A {}",
        "abstract base class A {}",
        "abstract final class A {}",
        "abstract interface class A {}",
        "abstract mixin class A {}",
        "abstract base mixin class A {}",
        "base mixin M {}",
        "sealed class S {}",
        "final class F {}",
        "interface class I {}",
        "mixin class M {}",
        "base mixin class M {}",
    ] {
        assert!(errors(v).is_empty(), "{v}: {:?}", errors(v));
    }
}

#[test]
fn extension_and_mixin_members() {
    assert_eq!(
        errors("extension E on int { late final int v; }"),
        vec![pair("extension_declares_instance_field", "v")]
    );
    assert_eq!(
        errors("extension E on int { int get foo; }"),
        vec![pair("extension_declares_abstract_member", "foo")]
    );
    assert_eq!(
        errors("extension E on int { int operator +(int x); }"),
        vec![pair("extension_declares_abstract_member", "+")]
    );
    assert_eq!(errors("extension E on int { E(); }"), vec![pair("extension_declares_constructor", "E")]);
    assert_eq!(errors("mixin M { M(); }"), vec![pair("mixin_declares_constructor", "M")]);
    assert!(errors("extension E on int { static int x = 0; external int y; int get z => 0; }").is_empty());
}

#[test]
fn extension_without_on() {
    assert_eq!(
        errors("extension E {}"),
        vec![pair("expected_token", "E"), pair("expected_type_name", "{")]
    );
}

#[test]
fn assignment_to_non_assignable_target() {
    assert_eq!(
        errors_in((2, 19), "f(v) { (v) = 0; }"),
        vec![pair("illegal_assignment_to_non_assignable", "(v)"), pair("missing_assignable_selector", "(v)")]
    );
    assert_eq!(errors("f(v) { (v)++; }"), vec![pair("illegal_assignment_to_non_assignable", "++")]);
    assert_eq!(errors("f(v) { ++(v); }"), vec![pair("missing_assignable_selector", ")")]);
    assert_eq!(errors_in((2, 19), "f(x) => ((x)) = 1;"), vec![pair("missing_assignable_selector", "((x))")]);
    assert_eq!(
        errors("class C { m() { super?.m(); } }"),
        vec![pair("invalid_operator_questionmark_period_for_super", "?.")]
    );
    assert!(errors("f(a, b) { a = 1; a.b = 2; a[0] = 3; a?.b = 4; a++; ++a[0]; }").is_empty());
}

#[test]
fn chained_comparison() {
    assert_eq!(errors("f(a, b, c) => a == b == c;"), vec![pair("equality_cannot_be_equality_operand", "==")]);
    assert_eq!(errors("f(a, b, c) => a < b < c;"), vec![pair("equality_cannot_be_equality_operand", "<")]);
    assert!(errors("f(a, b, c) => a < b == c;").is_empty());
}

#[test]
fn member_names_and_forms() {
    assert_eq!(errors("class A { int z = 0, A = 0; }"), vec![pair("member_with_class_name", "A")]);
    assert_eq!(errors("class A { int get A => 0; }"), vec![pair("member_with_class_name", "A")]);
    assert_eq!(errors("get f2() => null;"), vec![pair("getter_with_parameters", "(")]);
    assert_eq!(errors("class C { B.x(); }"), vec![pair("invalid_constructor_name", "B")]);
    assert_eq!(errors("class C { static C() {} }"), vec![pair("static_constructor", "static")]);
}

#[test]
fn patterns_by_context() {
    assert_eq!(
        errors("f() { var x; [x, final y] = [0, 1]; }"),
        vec![pair("pattern_assignment_declares_variable", "y")]
    );
    assert_eq!(
        errors("f() { var (var a, b) = (1, 2); }"),
        vec![pair("variable_pattern_keyword_in_declaration_context", "var")]
    );
    assert_eq!(
        errors("f(x) { switch (x) { case var when: } }"),
        vec![pair("illegal_pattern_variable_name", "when")]
    );
}

#[test]
fn interpolation_without_name_does_not_stop() {
    assert_eq!(errors("f() { var x = \"$\"; }"), vec![pair("missing_identifier", "\"")]);
    assert_eq!(errors("f(x) { var s = \"a $$x b\"; }"), vec![pair("missing_identifier", "$")]);
    // The rest of the unit keeps being parsed.
    assert_eq!(
        errors("f() { var x = '$ '; }\nstatic g() {}"),
        vec![pair("missing_identifier", " "), pair("extraneous_modifier", "static")]
    );
}

#[test]
fn property_with_reserved_word() {
    assert_eq!(errors("class C { var x; m() { x.this; } }"), vec![pair("missing_identifier", "this")]);
}

#[test]
fn constructor_with_type_arguments_after_name() {
    assert_eq!(
        errors("f() => new C<int>.named<int>();"),
        vec![pair("constructor_with_type_arguments", "named")]
    );
}

#[test]
fn parameter_pattern_with_default() {
    assert_eq!(
        errors("typedef F = void Function({int x = 1});"),
        vec![pair("default_value_in_function_type", "=")]
    );
    assert_eq!(errors("void f(int x = 1) {}"), vec![pair("named_parameter_outside_group", "=")]);
}

#[test]
fn await_and_yield_as_name_in_async_body() {
    assert_eq!(errors("f() async { var await = 1; }"), vec![pair("async_keyword_used_as_identifier", "await")]);
    assert!(errors("f() { var await = 1; }").is_empty());
}
