//! Normative test suite for function bodies, type inference, flow analysis and diagnostics.
//!
//! Covers:
//! 1. `promotion_and_inference`: fast unit tests (<100ms) without the SDK.
//! 2. `negativos_do_analyzer`: >= 40 negative cases from the official analyzer.
//! 3. `sdk_bodies_type_check`: all 36 SDK libraries type-check, with statistics.
//! 4. `pub_corpus_type_checks`: packages from `references/pub` type-check.
//! 5. `new_sali_type_checks`: the `new_sali` frontend and backend type-check.

use dartforge_elements::load::{load, load_lenient};
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::codes::*;
use dartforge_types::table::{CoreTypes, TypeTable};
use dartforge_types::{infer_program_bodies, resolve_outline};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn mock_sdk(dir: &Path) -> SdkLayout {
    let lib_dir = dir.join("lib");
    fs::create_dir_all(&lib_dir).unwrap();
    let libraries_json = lib_dir.join("libraries.json");
    fs::write(
        &libraries_json,
        r#"{
            "dartdevc": {
                "libraries": {
                    "core": {
                        "uri": "core/core.dart",
                        "patches": []
                    },
                    "async": {
                        "uri": "async/async.dart",
                        "patches": []
                    }
                }
            }
        }"#,
    )
    .unwrap();

    let core_dir = lib_dir.join("core");
    fs::create_dir_all(&core_dir).unwrap();
    fs::write(
        core_dir.join("core.dart"),
        r#"
        library dart.core;
        class Object {
            const Object();
            bool operator ==(Object other) => true;
            String toString() => "";
        }
        abstract interface class Enum {
            int get index;
        }
        class int extends num {
            int operator +(int other) => this;
            int operator -(int other) => this;
            int operator *(int other) => this;
            bool operator <(num other) => true;
            bool operator >(num other) => true;
            bool operator <=(num other) => true;
            bool operator >=(num other) => true;
        }
        class num extends Object {}
        class double extends num {}
        class String extends Object {
            int get length => 0;
            String operator +(String other) => this;
        }
        class bool extends Object {}
        class Null extends Object {}
        class Function extends Object {}
        class Record extends Object {}
        class Iterable<E> extends Object {
            int get length => 0;
        }
        class List<E> extends Object implements Iterable<E> {
            int get length => 0;
            E operator [](int index);
            void operator []=(int index, E value);
            void add(E value);
        }
        class Map<K, V> extends Object {
            V? operator [](Object? key);
            void operator []=(K key, V value);
        }
        class Set<E> extends Object implements Iterable<E> {}
        "#,
    )
    .unwrap();

    let async_dir = lib_dir.join("async");
    fs::create_dir_all(&async_dir).unwrap();
    fs::write(
        async_dir.join("async.dart"),
        r#"
        library dart.async;
        import 'dart:core';
        class Future<T> extends Object {}
        "#,
    )
    .unwrap();

    SdkLayout::load(&lib_dir, "dartdevc").expect("loads the simulated sdk layout")
}

fn get_real_sdk() -> Option<(PathBuf, SdkLayout)> {
    let lib_dir = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        if p.join("libraries.json").exists() {
            Some(p)
        } else {
            None
        }
    })?;
    let layout = SdkLayout::load(&lib_dir, "dartdevc").ok()?;
    Some((lib_dir, layout))
}

// ===========================================================================
// 1. Flow Promotion and Inference (without the SDK)
// ===========================================================================

#[test]
fn promotion_and_inference() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_flow;
        import 'dart:core';

        void test_is_promotion(Object x) {
            if (x is int) {
                var y = x + 1;
            }
        }

        void test_null_promotion(int? x) {
            if (x != null) {
                var y = x + 1;
            }
        }

        void test_and_promotion(Object x) {
            if (x is String && x.length > 0) {
                var s = x + "!";
            }
        }

        int test_join_lub(bool cond) {
            int a;
            if (cond) {
                a = 1;
            } else {
                a = 2;
            }
            return a;
        }

        void test_collection_inference() {
            var lista = [1, 2, 3];
            var mapa = {"a": 1, "b": 2};
        }

        void test_closure_inference() {
            var dobro = (int x) => x + x;
            var r = dobro(10);
        }
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the test program");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, outline_diags) = resolve_outline(&prog, &interner, &mut table, &core);

    assert!(
        outline_diags.is_empty(),
        "The outline must not have diagnostics: {outline_diags:?}"
    );

    let (body_types, diags) =
        infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    // Unit 0 is `dart:core` (without body inference); the user's is the entry one.
    let entry_unit = prog.library(prog.entry.unwrap()).units[0];
    let unit = &body_types.units[entry_unit.0 as usize];
    assert!(unit.expression_count() > 10, "Expressions must be typed");

    // Checks that there were no spurious diagnostics
    assert!(
        diags.is_empty(),
        "Valid bodies must not emit diagnostics: {diags:?}"
    );
}

// ===========================================================================
// 2. Analyzer Negative Cases (>= 40 tests translated from the official suite)
// ===========================================================================

fn verify_diagnostic(dart_code: &str, expected_diagnostic: DiagnosticCode) {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        format!("library test_neg;\nimport 'dart:core';\n{dart_code}"),
    )
    .unwrap();

    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    // The diagnostic may come out with the `types::codes` template or already
    // with the analyzer's code (the official English message).
    let found = diags.iter().any(|d| {
        d.message.contains(expected_diagnostic.template)
            || d.code
                .is_some_and(|c| c.info().name == expected_diagnostic.name)
    });
    assert!(
        found,
        "Expected diagnostic '{}', but got:\n{:?}",
        expected_diagnostic.template, diags
    );
}

#[test]
fn class_getter_without_setter_in_simple_assignment() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class A { int get x => 0; } class B { int get x => 0; set x(int v) {} } void f(A a, B b) { a.x = 0; a.x += 0; ++a.x; a.x++; a.y = 0; b.x = 0; b.x += 0; ++b.x; b.x++; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message
                .starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)
        })
        .collect();
    assert_eq!(readonly.len(), 4, "{diags:?}");
    for (d, target) in readonly
        .iter()
        .zip(["a.x = 0", "a.x += 0", "++a.x", "a.x++"])
    {
        let x = source.find(target).unwrap() + target.find("x").unwrap();
        assert_eq!(d.span.start, x, "{diags:?}");
        assert_eq!(d.span.end, x + 1, "{diags:?}");
        assert!(d.message.contains("'x' in class 'A'"), "{diags:?}");
    }
    let missing: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(UNDEFINED_SETTER.template))
        .collect();
    assert_eq!(missing.len(), 1, "{diags:?}");
    assert_eq!(missing[0].span.start, source.find("a.y = 0").unwrap() + 2);
}

#[test]
fn assign_to_types_and_top_level_function() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class C<T> { void m() { T = null; } } enum E { e } typedef F = C<int>; void f() {} void g() { C = null; E = null; F = null; dynamic = 1; f = null; } void h() { var C = 0; C = 1; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    assert_eq!(diags.len(), 6, "{diags:?}");
    let types: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_TYPE.template))
        .collect();
    assert_eq!(types.len(), 5, "{diags:?}");
    for (d, snippet) in types.iter().zip([
        "T = null",
        "C = null",
        "E = null",
        "F = null",
        "dynamic = 1",
    ]) {
        assert_eq!(d.span.start, source.find(snippet).unwrap(), "{diags:?}");
        assert_eq!(
            d.span.end,
            source.find(snippet).unwrap() + snippet.split(' ').next().unwrap().len(),
            "{diags:?}"
        );
    }
    let functions: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_FUNCTION.template))
        .collect();
    assert_eq!(functions.len(), 1, "{diags:?}");
    assert_eq!(
        functions[0].span.start,
        source.find("f = null").unwrap(),
        "{diags:?}"
    );
}

#[test]
fn local_function_is_not_assignable_final_variable() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; void f() { g(int x) {} g = 0; void Function() h = () {}; h = () {}; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(
        diags[0]
            .message
            .starts_with(ASSIGNMENT_TO_FUNCTION.template),
        "{diags:?}"
    );
    assert_eq!(
        diags[0].span.start,
        source.find("g = 0").unwrap(),
        "{diags:?}"
    );
    assert_eq!(
        diags[0].span.end,
        source.find("g = 0").unwrap() + 1,
        "{diags:?}"
    );
}

#[test]
fn lexical_getter_without_setter_in_assignments_and_increments() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class A { int get x => 0; static int get s => 0; void f() { x = 0; x += 0; ++x; x++; s = 0; s += 0; ++s; s++; } } class B extends A { void g() { x = 0; } }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message
                .starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)
        })
        .collect();
    assert_eq!(readonly.len(), 9, "{diags:?}");
    for (d, target) in readonly.iter().zip([
        "x = 0", "x += 0", "++x", "x++", "s = 0", "s += 0", "++s", "s++",
    ]) {
        let index = source.find(target).unwrap() + target.find(['x', 's']).unwrap();
        assert_eq!(d.span.start, index, "{diags:?}");
        assert_eq!(d.span.end, index + 1, "{diags:?}");
        assert!(d.message.contains("in class 'A'"), "{diags:?}");
    }
    let inherited = source.rfind("x = 0").unwrap();
    assert_eq!(readonly[8].span.start, inherited, "{diags:?}");
    assert!(readonly[8].message.contains("in class 'A'"), "{diags:?}");
}

#[test]
fn extension_getter_without_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on int { int get x => 0; } void f() { 0.x = 0; 0.x += 0; ++0.x; 0.x++; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message
                .starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)
        })
        .collect();
    assert_eq!(readonly.len(), 4, "{diags:?}");
    for (d, target) in readonly
        .iter()
        .zip(["0.x = 0", "0.x += 0", "++0.x", "0.x++"])
    {
        let index = source.find(target).unwrap() + target.find('x').unwrap();
        assert_eq!(d.span.start, index, "{diags:?}");
        assert_eq!(d.span.end, index + 1, "{diags:?}");
        assert!(d.message.contains("in class 'E'"), "{diags:?}");
    }
}

#[test]
fn explicit_extension_override_without_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on int { int get foo => 0; } extension F on int { set bar(int v) {} } void f() { E(0).foo = 1; E(0).foo += 1; F(0).bar = 1; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let the_missing: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_SETTER.template))
        .collect();
    assert_eq!(the_missing.len(), 2, "{diags:?}");
    for d in the_missing {
        assert_eq!(&source[d.span.start..d.span.end], "foo", "{diags:?}");
        assert!(d.message.contains("'foo' in 'E'"), "{diags:?}");
    }
    assert!(
        !diags.iter().any(|d| d
            .message
            .starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)),
        "{diags:?}"
    );
    assert!(
        !diags.iter().any(|d| d.message.contains("'bar'")),
        "{diags:?}"
    );
}

#[test]
fn explicit_override_without_getter_uses_extension_code() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on int { set foo(int v) {} } void f() { E(0).foo; E(0).foo += 1; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let the_missing: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_GETTER.template))
        .collect();
    assert_eq!(the_missing.len(), 2, "{diags:?}");
    for d in the_missing {
        assert_eq!(&source[d.span.start..d.span.end], "foo");
        assert!(d.message.contains("'foo' in 'E'"));
    }
    assert!(
        !diags
            .iter()
            .any(|d| d.message.starts_with(UNDEFINED_GETTER.template)),
        "{diags:?}"
    );
}

#[test]
fn missing_static_getter_in_extension_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, offset) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_getter/UndefinedExtensionGetter__static_withou_8f374a5b.dart"
            )),
            40,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_getter/UndefinedExtensionGetter__static_withInference.dart"
            )),
            35,
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let the_missing: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_GETTER.template))
            .collect();
        assert_eq!(the_missing.len(), 1, "{diags:?}");
        assert_eq!(
            (the_missing[0].span.start, the_missing[0].span.end),
            (offset, offset + 1)
        );
        assert!(the_missing[0].message.contains("'v' in 'E'"));
    }
}

#[test]
fn missing_static_setter_in_extension_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/diagnostics/analyzer/undefined_extension_setter/UndefinedExtensionSetter__static_undefined.dart"
    ));
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let the_found: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_SETTER.template))
        .collect();
    assert_eq!(the_found.len(), 1, "{diags:?}");
    let d = the_found[0];
    assert_eq!(&source[d.span.start..d.span.end], "foo");
    assert_eq!(d.span.start, source.find("E.foo").unwrap() + 2);
    assert!(d.message.contains("'foo' in 'E'"));
}

#[test]
fn static_access_to_extension_instance_members_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, the_name) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__extension_getter.dart"
            )),
            "g",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__extension_method.dart"
            )),
            "m",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__extension_setter.dart"
            )),
            "s",
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let the_found: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.message
                    .starts_with(STATIC_ACCESS_TO_INSTANCE_MEMBER.template)
            })
            .collect();
        assert_eq!(the_found.len(), 1, "{the_name}: {diags:?}");
        let d = the_found[0];
        assert_eq!(&source[d.span.start..d.span.end], the_name);
        assert_eq!(
            d.span.start,
            source.find(&format!("E.{the_name}")).unwrap() + 2
        );
        assert!(
            !diags.iter().any(
                |d| d.message.starts_with(UNDEFINED_EXTENSION_GETTER.template)
                    || d.message.starts_with(UNDEFINED_EXTENSION_METHOD.template)
            ),
            "{diags:?}"
        );
    }
}

#[test]
fn static_access_to_class_instance_members_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    let positives = [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__method_invocation.dart"
            )),
            "m",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__method_reference.dart"
            )),
            "m",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__propertyA_5eff7611.dart"
            )),
            "t",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__propertyA_618b8c7e.dart"
            )),
            "f",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__propertyA_baf43dcf.dart"
            )),
            "f",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__propertyA_dfc9b0ea.dart"
            )),
            "f",
        ),
    ];
    for (source, the_name) in positives {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let the_found: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.message
                    .starts_with(STATIC_ACCESS_TO_INSTANCE_MEMBER.template)
            })
            .collect();
        assert_eq!(the_found.len(), 1, "{the_name}: {diags:?}");
        let d = the_found[0];
        assert_eq!(&source[d.span.start..d.span.end], the_name);
        assert_eq!(
            d.span.start,
            source.rfind(&format!(".{the_name}")).unwrap() + 1
        );
        // The parity bridge translates this internal template into the oracle's
        // English message; the types test checks the code and the argument.
        assert_eq!(
            d.message,
            format!(
                "{}: '{the_name}'",
                STATIC_ACCESS_TO_INSTANCE_MEMBER.template
            )
        );
        assert!(
            !diags.iter().any(|other| other.span == d.span
                && other.message.starts_with(UNDEFINED_GETTER.template)),
            "{diags:?}"
        );
    }
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__static_method.dart"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__static_pr_24220736.dart"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/static_access_to_instance_member/StaticAccessToInstanceMember__static_pr_d7df5494.dart"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__alias.dart"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__instanceMember.dart"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__instanceSetter.dart"
        )),
        // Dart 3.13.4 oracle: a named constructor and a same-named instance
        // method coexist; `A.named` is the constructor's tear-off.
        "class A { A.named(); void named() {} } var tearoff = A.named;",
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        assert!(
            !diags.iter().any(|d| d
                .message
                .starts_with(STATIC_ACCESS_TO_INSTANCE_MEMBER.template)),
            "{diags:?}"
        );
    }
    // A private name from another library is not an accessible class member.
    fs::write(tmp.path().join("lib.dart"), "class A { void _m() {} }").unwrap();
    fs::write(&main_dart, "import 'lib.dart'; void f() { A._m(); }").unwrap();
    let mut interner = Interner::new();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    assert!(
        !diags.iter().any(|d| d
            .message
            .starts_with(STATIC_ACCESS_TO_INSTANCE_MEMBER.template)),
        "{diags:?}"
    );
}

#[test]
fn write_to_instance_method_via_type_and_undefined_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/diagnostics/linguagem/static/field3_test.dart"
    ));
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let offset = source.find("Foo.m = 1").unwrap() + "Foo.".len();
    let at_target: Vec<_> = diags.iter().filter(|d| d.span.start == offset).collect();
    assert_eq!(at_target.len(), 1, "{diags:?}");
    assert!(
        at_target[0].message.starts_with(UNDEFINED_SETTER.template),
        "{diags:?}"
    );
    assert!(at_target[0].message.contains("'m'"), "{diags:?}");
    assert_eq!(at_target[0].span.end, offset + 1);
}

#[test]
fn missing_new_after_instantiated_type_keeps_constructor_diagnostic() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/diagnostics/linguagem/constructor/unnamed_new_error_test.dart"
    ));
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    for (expr, code) in [
        ("NoUnnamed.new();", NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT),
        ("NoUnnamed<int>.new();", NEW_WITH_UNDEFINED_CONSTRUCTOR),
    ] {
        let offset = source.find(expr).unwrap() + expr.find(".new").unwrap() + 1;
        let at_target: Vec<_> = diags.iter().filter(|d| d.span.start == offset).collect();
        assert_eq!(at_target.len(), 1, "{expr}: {diags:?}");
        // By the analyzer's code (the report carries its message, not the legacy
        // text of `codes.rs`); or, without a code, by the legacy text.
        let by_code = at_target[0]
            .code
            .is_some_and(|c| c.info().name == code.name);
        assert!(
            by_code || at_target[0].message.starts_with(code.template),
            "{expr}: {diags:?}"
        );
        assert_eq!(at_target[0].span.end, offset + 3);
    }
}

#[test]
fn enum_generative_constructor_reference_is_not_missing_constructor() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, expr, suffix) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor/InvalidReferenceToGenerativeEnumConstru_f77bae06.dart"
            )),
            "E.new",
            ";",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor/InvalidReferenceToGenerativeEnumConstru_24430155.dart"
            )),
            "E.named",
            ";",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor/InvalidReferenceToGenerativeEnumConstru_3e9c8dca.dart"
            )),
            "E.named",
            "();",
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let offset = source.find(&format!("  {expr}{suffix}")).unwrap() + 2;
        let target: Vec<_> = diags.iter().filter(|d| d.span.start == offset).collect();
        assert_eq!(target.len(), 1, "{expr}: {diags:?}");
        assert!(
            target[0]
                .message
                .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template),
            "{expr}: {diags:?}"
        );
        assert_eq!(target[0].span.end, offset + expr.len());
    }
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor/InvalidReferenceToGenerativeEnumConstru_4790a320.dart"
    ));
    let mut interner = Interner::new();
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    assert!(
        !diags.iter().any(|d| d
            .message
            .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template)),
        "{diags:?}"
    );
}

#[test]
fn enum_generative_constructor_invocation_with_new_const_and_unnamed() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    // (corpus file, snippet with the starting point, offset of the
    // constructor name in the snippet, length of the oracle's range)
    for (file, snippet, shift, comp) in [
        (
            "InvalidReferenceToGenerativeEnumConstru_f38943a5.dart",
            "  E();",
            2,
            1,
        ),
        (
            "InvalidReferenceToGenerativeEnumConstru_b2744770.dart",
            "  new E();",
            6,
            1,
        ),
        (
            "InvalidReferenceToGenerativeEnumConstru_3b68350d.dart",
            "  const E();",
            8,
            1,
        ),
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor").join(file);
        let source = fs::read_to_string(&path).unwrap();
        let mut interner = Interner::new();
        fs::write(&main_dart, &source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let offset = source.find(snippet).unwrap() + shift;
        let target: Vec<_> = diags.iter().filter(|d| d.span.start == offset).collect();
        assert_eq!(target.len(), 1, "{file}: {diags:?}");
        assert!(
            target[0]
                .message
                .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template),
            "{file}: {diags:?}"
        );
        assert_eq!(target[0].span.end, offset + comp, "{file}: {diags:?}");
    }
    // Negatives: a factory with the same name and redirections to the
    // generative one in an initializer (`: this(...)`) still do not report;
    // an explicit name without a target (`const E.foo()`) is another code.
    for file in [
        "InvalidReferenceToGenerativeEnumConstru_7e4106b1.dart",
        "InvalidReferenceToGenerativeEnumConstru_4fcf1842.dart",
        "InvalidReferenceToGenerativeEnumConstru_bd7327c1.dart",
        "../const_with_undefined_constructor/ConstWithUndefinedConstructor__enum_not_6ecbcaf1.dart",
        "../const_with_undefined_constructor/ConstWithUndefinedConstructor__enum_not_a082a644.dart",
        "../const_with_undefined_constructor/ConstWithUndefinedConstructor__enum_unresolved.dart",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor").join(file);
        let source = fs::read_to_string(&path).unwrap();
        let mut interner = Interner::new();
        fs::write(&main_dart, &source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        assert!(
            !diags.iter().any(|d| d
                .message
                .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template)),
            "{file}: {diags:?}"
        );
    }
}

#[test]
fn redirecting_factory_to_enum_generative_reports_at_target() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    // (corpus file, snippet with the starting point, offset of the target
    // in the snippet, length of the oracle's range)
    for (file, snippet, shift, comp) in [
        (
            "InvalidReferenceToGenerativeEnumConstru_31f7f2b6.dart",
            " = E.named;",
            3,
            7,
        ),
        (
            "InvalidReferenceToGenerativeEnumConstru_f6f82211.dart",
            " = E;",
            3,
            1,
        ),
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/analyzer/invalid_reference_to_generative_enum_constructor").join(file);
        let source = fs::read_to_string(&path).unwrap();
        let mut interner = Interner::new();
        fs::write(&main_dart, &source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let offset = source.find(snippet).unwrap() + shift;
        let target: Vec<_> = diags.iter().filter(|d| d.span.start == offset).collect();
        assert_eq!(target.len(), 1, "{file}: {diags:?}");
        assert!(
            target[0]
                .message
                .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template),
            "{file}: {diags:?}"
        );
        assert_eq!(target[0].span.end, offset + comp, "{file}: {diags:?}");
    }
    // Negatives: factory to factory, a non-enum class and an undefined name
    // still do not report this code.
    for (the_name, source) in [
        (
            "fabrica_para_fabrica",
            "enum E { v(); factory E.named() => v; const factory E.other() = E.named; const E(); }",
        ),
        (
            "classe_comum",
            "class C { const C(); const factory C.a() = C; }",
        ),
        (
            "nome_indefinido",
            "enum E { v; const factory E.a() = E.inexistente; const E(); }",
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        assert!(
            !diags.iter().any(|d| d
                .message
                .starts_with(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template)),
            "{the_name}: {diags:?}"
        );
    }
}

#[test]
fn instance_member_in_explicit_instantiation_has_own_code_and_span() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, access) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__alias.dart"
            )),
            "TA<int>.i",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__instanceMember.dart"
            )),
            "A<int>.i",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__instanceSetter.dart"
            )),
            "A<int>.i",
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let the_found: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.message
                    .starts_with(CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template)
            })
            .collect();
        assert_eq!(the_found.len(), 1, "{access}: {diags:?}");
        let d = the_found[0];
        assert_eq!(
            d.message,
            format!(
                "{}: 'i'",
                CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template
            )
        );
        assert_eq!(&source[d.span.start..d.span.end], access);
    }
}

#[test]
fn static_or_unknown_member_in_explicit_instantiation() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, code, argument) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__staticMember.dart"
            )),
            CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER,
            "'i'",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__staticSetter.dart"
            )),
            CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER,
            "'i'",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/class_instantiation_access_to_member/ClassInstantiationAccessToMember__exten_0fa28792.dart"
            )),
            CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER,
            "'A', 'i'",
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let the_found: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(code.template))
            .collect();
        assert_eq!(the_found.len(), 1, "{diags:?}");
        let d = the_found[0];
        assert_eq!(d.message, format!("{}: {argument}", code.template));
        assert_eq!(&source[d.span.start..d.span.end], "A<int>.i");
    }
}

#[test]
fn explicit_override_without_method_uses_extension_code() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source =
        "library test; import 'dart:core'; extension E on String {} void f() { E('a').m(); }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let the_missing: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_METHOD.template))
        .collect();
    assert_eq!(the_missing.len(), 1, "{diags:?}");
    assert_eq!(
        &source[the_missing[0].span.start..the_missing[0].span.end],
        "m"
    );
    assert!(the_missing[0].message.contains("'m' in 'E'"));
}

#[test]
fn missing_static_method_in_extension_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, offset) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_method/UndefinedExtensionMethod__static_withInference.dart"
            )),
            35,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_method/UndefinedExtensionMethod__static_withou_709c4429.dart"
            )),
            40,
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let the_missing: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_METHOD.template))
            .collect();
        assert_eq!(the_missing.len(), 1, "{diags:?}");
        assert_eq!(
            (the_missing[0].span.start, the_missing[0].span.end),
            (offset, offset + 1)
        );
        assert!(the_missing[0].message.contains("'m' in 'E'"));
    }
}

#[test]
fn explicit_extension_override_rejects_static_members() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on String { static String get empty => ''; static void set empty(String s) {} } void f() { E('a').empty; E('a').empty = 'b'; E('a').empty += 'b'; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let statics: Vec<_> = diags
        .iter()
        .filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template)
        .collect();
    assert_eq!(statics.len(), 3, "{diags:?}");
    for d in statics {
        assert_eq!(&source[d.span.start..d.span.end], "empty", "{diags:?}");
    }
    assert!(
        !diags
            .iter()
            .any(|d| d.message.starts_with(UNDEFINED_EXTENSION_SETTER.template)),
        "{diags:?}"
    );
}

#[test]
fn static_method_in_extension_override() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on String { static String empty() => ''; String instance() => ''; } void f() { E('a').empty(); E('a').instance(); }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let statics: Vec<_> = diags
        .iter()
        .filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template)
        .collect();
    assert_eq!(statics.len(), 1, "{diags:?}");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(&source[statics[0].span.start..statics[0].span.end], "empty");
    assert!(
        !diags.iter().any(|d| d.message.contains("instance")),
        "{diags:?}"
    );
}

#[test]
fn static_call_in_extension_override() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; extension E on int { static void call() {} } void f() { E(0)(); }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let statics: Vec<_> = diags
        .iter()
        .filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template)
        .collect();
    assert_eq!(statics.len(), 1, "{diags:?}");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(&source[statics[0].span.start..statics[0].span.end], "()");

    let instance_source = "library test; import 'dart:core'; extension F on int { void call() {} } void g() { F(0)(); }";
    fs::write(&main_dart, instance_source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    assert!(
        !diags
            .iter()
            .any(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template),
        "{diags:?}"
    );
}

#[test]
fn override_without_call_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/diagnostics/analyzer/invocation_of_extension_without_call/InvocationOfExtensionWithoutCall__insta_ae18bfa4.dart"
    ));
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let the_missing: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message
                .starts_with(INVOCATION_OF_EXTENSION_WITHOUT_CALL.template)
        })
        .collect();
    assert_eq!(the_missing.len(), 1, "{diags:?}");
    assert_eq!(
        &source[the_missing[0].span.start..the_missing[0].span.end],
        "E(0)"
    );
}

#[test]
fn unary_operator_in_override_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, missing_) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__prefix_minu_99f91916.dart"
            )),
            true,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__prefix_minus_defined.dart"
            )),
            false,
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let operators: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template))
            .collect();
        assert_eq!(operators.len(), usize::from(missing_), "{diags:?}");
        if missing_ {
            assert_eq!((operators[0].span.start, operators[0].span.end), (33, 34));
            assert!(operators[0].message.contains("'unary-' in 'E'"));
        }
    }
}

#[test]
fn binary_operator_in_override_per_oracle() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, missing_) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__binary_undefined.dart"
            )),
            true,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__binary_defined.dart"
            )),
            false,
        ),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let operators: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template))
            .collect();
        assert_eq!(operators.len(), usize::from(missing_), "{diags:?}");
        if missing_ {
            assert_eq!((operators[0].span.start, operators[0].span.end), (40, 41));
            assert!(operators[0].message.contains("'+' in 'E'"));
        }
    }

    let source = "extension E on String {} void f() { E('a') /* + */ + 1; }";
    let mut interner = Interner::new();
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let operator = diags
        .iter()
        .find(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template))
        .expect("missing operator");
    assert_eq!(operator.span.start, source.rfind('+').unwrap());
}

#[test]
fn index_operators_in_override_per_oracle() {
    let cases: &[(&str, &[&str])] = &[
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_get_hasGetter.dart"
            )),
            &[],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_get_hasNone.dart"
            )),
            &["[]"],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_get_hasSetter.dart"
            )),
            &["[]"],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_getSe_517d7597.dart"
            )),
            &["[]"],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_getSe_eb3ada23.dart"
            )),
            &["[]="],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_getSet_hasBoth.dart"
            )),
            &[],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_getSet_hasNone.dart"
            )),
            &["[]", "[]="],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_set_hasGetter.dart"
            )),
            &["[]="],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_set_hasNone.dart"
            )),
            &["[]="],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../corpus/diagnostics/analyzer/undefined_extension_operator/UndefinedExtensionOperator__index_set_hasSetter.dart"
            )),
            &[],
        ),
    ];
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (source, the_expected) in cases {
        let mut interner = Interner::new();
        fs::write(&main_dart, source).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let mut the_found: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template))
            .map(|d| {
                let operator = if d.message.contains("'[]='") {
                    "[]="
                } else {
                    "[]"
                };
                (operator, d.span.start, d.span.end)
            })
            .collect();
        the_found.sort();
        let start = source.find("[0]").unwrap();
        let mut expected: Vec<_> = the_expected
            .iter()
            .map(|the_name| (*the_name, start, start + 3))
            .collect();
        expected.sort();
        assert_eq!(the_found, expected, "{diags:?}");
    }
    let source = "class A {} extension E on A {} void f(A a) { E(a) /* [ */ [0]; }";
    let mut interner = Interner::new();
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let operator = diags
        .iter()
        .find(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template))
        .expect("missing operator");
    assert_eq!(operator.span.start, source.rfind("[0]").unwrap());
}

#[test]
fn final_field_without_setter_and_assignable_late_final() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class A { final int x = 0; late final int y; late final int z = 0; static final int s = 0; static const int c = 0; void f() { x = 0; x += 0; ++x; x++; s = 0; z = 1; c = 1; y = 1; } } void g(A a) { a.x = 0; a.x += 0; ++a.x; a.x++; a.z = 1; a.y = 1; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finals: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template))
        .collect();
    assert_eq!(finals.len(), 11, "{diags:?}");
    for d in finals {
        let snippet = &source[d.span.start..d.span.end];
        assert!(
            snippet == "x" || snippet == "s" || snippet == "z",
            "{diags:?}"
        );
        assert!(d.message.contains(&format!("'{snippet}'")), "{diags:?}");
    }
    let constants: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_CONST.template))
        .collect();
    assert_eq!(constants.len(), 1, "{diags:?}");
    assert_eq!(&source[constants[0].span.start..constants[0].span.end], "c");
    assert!(
        !diags.iter().any(|d| d.message.contains("'y'")),
        "{diags:?}"
    );
}

#[test]
fn read_only_static_members_in_assignments() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class A { static final int x = 0; static int get g => 0; static late final int l; static const int c = 0; } void f() { A.x = 0; A.x += 0; ++A.x; A.x++; A.g = 0; A.g += 0; ++A.g; A.g++; A.c = 1; A.l = 1; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finals: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template))
        .collect();
    let getters: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message
                .starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)
        })
        .collect();
    let constants: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_CONST.template))
        .collect();
    assert_eq!(finals.len(), 4, "{diags:?}");
    assert_eq!(getters.len(), 4, "{diags:?}");
    assert_eq!(constants.len(), 1, "{diags:?}");
    for d in finals {
        assert_eq!(&source[d.span.start..d.span.end], "x");
    }
    for d in getters {
        assert_eq!(&source[d.span.start..d.span.end], "g");
        assert!(d.message.contains("in class 'A'"));
    }
    assert_eq!(&source[constants[0].span.start..constants[0].span.end], "c");
    assert!(
        !diags.iter().any(|d| d.message.contains("'l'")),
        "{diags:?}"
    );
}

#[test]
fn top_level_getter_without_setter_and_pair_with_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; int get x => 0; int get y => 0; set y(int v) {} void f() { x = 0; x += 0; ++x; x++; y = 0; y += 0; ++y; y++; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finals: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template))
        .collect();
    assert_eq!(finals.len(), 4, "{diags:?}");
    for (d, target) in finals.iter().zip(["x = 0", "x += 0", "++x", "x++"]) {
        let index = source.find(target).unwrap() + target.find('x').unwrap();
        assert_eq!(d.span.start, index, "{diags:?}");
        assert_eq!(d.span.end, index + 1, "{diags:?}");
        assert!(d.message.contains("'x'"), "{diags:?}");
    }
    assert!(
        !diags.iter().any(|d| d.message.contains("'y'")),
        "{diags:?}"
    );
}

#[test]
fn class_method_is_not_setter_even_with_extension() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; class A { void foo() {} int bar = 0; void g() { foo = 0; } } extension E on A { set foo(int v) {} } void f(A a) { a.foo = 0; a.foo += 1; a.foo++; ++a.foo; a.bar = 0; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let methods: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_METHOD.template))
        .collect();
    assert_eq!(methods.len(), 5, "{diags:?}");
    for d in methods {
        assert_eq!(&source[d.span.start..d.span.end], "foo", "{diags:?}");
    }
    assert!(
        !diags.iter().any(|d| d.message.contains("'bar'")),
        "{diags:?}"
    );
}

#[test]
fn top_level_late_final_without_initializer_accepts_write() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core'; late final int x; late final int y = 0; void f() { x = 0; x += 0; ++x; x++; y = 0; y += 0; ++y; y++; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finals: Vec<_> = diags
        .iter()
        .filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template))
        .collect();
    assert_eq!(finals.len(), 4, "{diags:?}");
    for d in finals {
        assert_eq!(&source[d.span.start..d.span.end], "y", "{diags:?}");
    }
    assert!(
        !diags.iter().any(|d| d.message.contains("'x'")),
        "{diags:?}"
    );
}

#[test]
fn assignment_to_final_local_marks_only_identifier() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source =
        "library teste; import 'dart:core'; void f() { final int x = 0; x = 1; x += 1; ++x; }";
    fs::write(&main_dart, source).unwrap();

    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let findings: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains(ASSIGNMENT_TO_FINAL_LOCAL.template))
        .collect();
    assert_eq!(findings.len(), 3, "{diags:?}");
    for d in findings {
        assert_eq!(&source[d.span.start..d.span.end], "x", "{d:?}");
    }
}

#[test]
fn const_local_and_top_level_final_are_not_final_local() {
    verify_diagnostic("void f() { const x = 1; x = 2; }", ASSIGNMENT_TO_CONST);
    verify_diagnostic("final int x = 1; void f() { x = 2; }", ASSIGNMENT_TO_FINAL);
}

#[test]
fn pattern_assignment_checks_local_write_flow() {
    let cases = [
        (
            "int f(){final a=1;(a,)=(2,);return a;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(){final int a;(a,)=(1,);(a,)=(2,);return a;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(bool b){final int a;if(b)a=1;(a,)=(2,);return a;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(){late final int a;a=1;(a,)=(2,);return a;}",
            "late_final_local_already_assigned",
            1,
        ),
        (
            "int f(){late final int a=1;(a,)=(2,);return a;}",
            "late_final_local_already_assigned",
            1,
        ),
        (
            "int f(){final a=1;var b=0;(b,(a,))=(2,(3,));return a+b;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(){final a=1;[a]=[2];return a;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(){final int a;(a,a)=(1,2);return a;}",
            "assignment_to_final_local",
            1,
        ),
        (
            "int f(){final int a;(a,)=(a=1,);return a;}",
            "assignment_to_final_local",
            1,
        ),
        ("int f(){final int a;(a,)=(2,);return a;}", "", 0),
        ("int f(){late final int a;(a,)=(2,);return a;}", "", 0),
        ("int f(){var a=1;(a,)=(2,);return a;}", "", 0),
        // SDK 3.6.2's analyzer omits this error, but its compiler crashes on the
        // same invalid program. Keep the ordinary const-write restriction.
        (
            "int f(){const a=1;(a,)=(2,);return a;}",
            "assignment_to_const",
            1,
        ),
        (
            "int f(){int a()=>1;(a,)=(()=>2,);return a();}",
            "pattern_assignment_not_local_variable",
            1,
        ),
    ];
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    for ((source, expected, count), decoration) in cases
        .into_iter()
        .flat_map(|case| [false, true].map(|decoration| (case, decoration)))
    {
        let mut interner = Interner::new();
        let path = tmp.path().join("main.dart");
        fs::write(&path, source).unwrap();
        let (prog, _) = load_lenient(&path, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        table.preserve_decoration = decoration;
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let writes: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.code.is_some_and(|c| {
                    matches!(
                        c.info().name,
                        "assignment_to_final_local"
                            | "late_final_local_already_assigned"
                            | "assignment_to_const"
                            | "pattern_assignment_not_local_variable"
                    )
                }) || d.message.starts_with(ASSIGNMENT_TO_FINAL_LOCAL.template)
                    || d.message.starts_with(ASSIGNMENT_TO_CONST.template)
            })
            .collect();
        assert_eq!(writes.len(), count, "{source}: {diags:?}");
        for d in writes {
            assert!(
                d.code.is_some_and(|c| c.info().name == expected)
                    || (expected == "assignment_to_final_local"
                        && d.message.starts_with(ASSIGNMENT_TO_FINAL_LOCAL.template))
                    || (expected == "assignment_to_const"
                        && d.message.starts_with(ASSIGNMENT_TO_CONST.template)),
                "{source}: {d:?}"
            );
            assert_eq!(&source[d.span.start..d.span.end], "a", "{source}: {d:?}");
            let expected_offset = if let Some(start) = source.find("(a,a)") {
                start + 3
            } else if let Some(start) = source.rfind("(a,") {
                start + 1
            } else {
                source.find("[a]").unwrap() + 1
            };
            assert_eq!(d.span.start, expected_offset, "{source}: {d:?}");
        }
    }
}

#[test]
fn analyzer_negatives_40_cases() {
    // 1. ARGUMENT_TYPE_NOT_ASSIGNABLE: passes String to int
    verify_diagnostic(
        "void f(int x) {} void main() { f('str'); }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 2. ARGUMENT_TYPE_NOT_ASSIGNABLE: passes Object to String
    verify_diagnostic(
        "void f(String s) {} void main(Object o) { f(o); }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 3. ARGUMENT_TYPE_NOT_ASSIGNABLE: int's + operator receives a String
    verify_diagnostic(
        "void main() { int a = 1; var b = a + 'x'; }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 4. RETURN_OF_INVALID_TYPE: returns String from an int function
    verify_diagnostic("int f() { return 'not an int'; }", RETURN_OF_INVALID_TYPE);

    // 5. RETURN_OF_INVALID_TYPE: returns int from a String function
    verify_diagnostic("String f() { return 42; }", RETURN_OF_INVALID_TYPE);

    // 6. ASSIGNMENT_TO_FINAL_LOCAL: reassignment of a final variable
    verify_diagnostic(
        "void main() { final int x = 1; x = 2; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 7. ASSIGNMENT_TO_FINAL_LOCAL: reassignment of an inferred final
    verify_diagnostic(
        "void main() { final x = 'a'; x = 'b'; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 8. ASSIGNMENT_TO_FINAL_LOCAL: reassignment of a final parameter
    verify_diagnostic(
        "void main(final int x) { x = 3; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 9. UNDEFINED_GETTER: access to a nonexistent getter on int
    verify_diagnostic(
        "void main() { int x = 1; var y = x.propriedadeInexistente; }",
        UNDEFINED_GETTER,
    );

    // 10. UNDEFINED_GETTER: access to a nonexistent getter on String
    verify_diagnostic(
        "void main() { String s = 'a'; var y = s.foo; }",
        UNDEFINED_GETTER,
    );

    // 11. UNDEFINED_METHOD: call to a nonexistent method on int
    verify_diagnostic(
        "void main() { int x = 1; x.metodoInexistente(); }",
        UNDEFINED_METHOD,
    );

    // 12. UNDEFINED_METHOD: call to a nonexistent method on String
    verify_diagnostic(
        "void main() { String s = 'hello'; s.bar(); }",
        UNDEFINED_METHOD,
    );

    // 13. UNDEFINED_SETTER: assignment to a nonexistent setter on int
    verify_diagnostic(
        "void main() { int x = 1; x.setterInexistente = 2; }",
        UNDEFINED_SETTER,
    );

    // 14. UNDEFINED_IDENTIFIER: undeclared variable
    verify_diagnostic(
        "void main() { var y = variavelTotalmenteInexistente; }",
        UNDEFINED_IDENTIFIER,
    );

    // 15. NOT_ENOUGH_POSITIONAL_ARGUMENTS: a function expecting 2 arguments gets 1
    verify_diagnostic(
        "void f(int a, int b) {} void main() { f(1); }",
        NOT_ENOUGH_POSITIONAL_ARGUMENTS,
    );

    // 16. NOT_ENOUGH_POSITIONAL_ARGUMENTS: a function expecting 1 argument gets 0
    verify_diagnostic(
        "void f(int a) {} void main() { f(); }",
        NOT_ENOUGH_POSITIONAL_ARGUMENTS,
    );

    // 17. EXTRA_POSITIONAL_ARGUMENTS: a function without arguments gets 1
    verify_diagnostic(
        "void f() {} void main() { f(1); }",
        EXTRA_POSITIONAL_ARGUMENTS,
    );

    // 18. EXTRA_POSITIONAL_ARGUMENTS: a 1-argument function gets 2
    verify_diagnostic(
        "void f(int a) {} void main() { f(1, 2); }",
        EXTRA_POSITIONAL_ARGUMENTS,
    );

    // 19. UNDEFINED_NAMED_PARAMETER: a nonexistent named parameter is passed
    verify_diagnostic(
        "void f({int? x}) {} void main() { f(nomeInexistente: 1); }",
        UNDEFINED_NAMED_PARAMETER,
    );

    // 20. UNDEFINED_NAMED_PARAMETER: a function without named ones gets a named one
    verify_diagnostic(
        "void f(int a) {} void main() { f(1, foo: 2); }",
        UNDEFINED_NAMED_PARAMETER,
    );

    // 21. MISSING_REQUIRED_ARGUMENT: a required named parameter is omitted
    verify_diagnostic(
        "void f({required int x}) {} void main() { f(); }",
        MISSING_REQUIRED_ARGUMENT,
    );

    // 22. DEFINITELY_UNASSIGNED: use of an uninitialized local
    verify_diagnostic(
        "void main() { int x; var y = x + 1; }",
        DEFINITELY_UNASSIGNED_VARIABLE,
    );

    // 23. DEFINITELY_UNASSIGNED: use in an if where only one branch assigns
    verify_diagnostic(
        "void main(bool c) { int x; if (c) { x = 1; } var y = x + 1; }",
        DEFINITELY_UNASSIGNED_VARIABLE,
    );

    // 24. REFERENCED_BEFORE_DECLARATION: local variable referenced before its line
    verify_diagnostic(
        "void main() { print(x); int x = 10; } void print(Object o) {}",
        REFERENCED_BEFORE_DECLARATION,
    );

    // 25. NON_BOOL_CONDITION: an if condition is int
    verify_diagnostic("void main() { if (42) {} }", NON_BOOL_CONDITION);

    // 26. NON_BOOL_CONDITION: a while condition is String
    verify_diagnostic("void main() { while ('loop') {} }", NON_BOOL_CONDITION);

    // 27. NON_BOOL_CONDITION: a do-while condition is int
    verify_diagnostic("void main() { do {} while (0); }", NON_BOOL_CONDITION);

    // 28. NON_BOOL_CONDITION: ternary operator with an int condition
    verify_diagnostic("void main() { var x = 1 ? 'a' : 'b'; }", NON_BOOL_CONDITION);

    // 29. INVALID_ASSIGNMENT: assignment of String to an int variable
    verify_diagnostic(
        "void main() { int x = 1; x = 'nao int'; }",
        INVALID_ASSIGNMENT,
    );

    // 30. INVALID_ASSIGNMENT: assignment of Object to an int variable
    verify_diagnostic(
        "void main(Object o) { int x = 1; x = o; }",
        INVALID_ASSIGNMENT,
    );

    // 31. EQUAL_ELEMENTS_IN_CONST_SET: const set with duplicate keys
    verify_diagnostic(
        "void main() { const s = {1, 1}; }",
        EQUAL_ELEMENTS_IN_CONST_SET,
    );

    // 32. EQUAL_KEYS_IN_CONST_MAP: const map with repeated keys
    verify_diagnostic(
        "void main() { const m = {'a': 1, 'a': 2}; }",
        EQUAL_KEYS_IN_CONST_MAP,
    );

    // 33. CONST_EVAL_THROWS_EXCEPTION: division by zero in a const
    verify_diagnostic(
        "void main() { const x = 1 ~/ 0; }",
        CONST_EVAL_THROWS_EXCEPTION,
    );

    // 34. CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE: const with a non-const variable
    verify_diagnostic(
        "void main() { var a = 1; const b = a; }",
        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE,
    );

    // 35. UNNECESSARY_NON_NULL_ASSERTION: bang (!) on a type guaranteed non-null
    // (the analyzer does not use `invalid_null_aware_operator` for the `!`).
    verify_diagnostic(
        "void main() { int x = 1; var y = x!; }",
        UNNECESSARY_NON_NULL_ASSERTION,
    );

    // 36. INVALID_NULL_AWARE_OPERATOR: ?. on a non-null int
    verify_diagnostic(
        "void main() { int x = 1; var y = x?.toString(); }",
        INVALID_NULL_AWARE_OPERATOR,
    );

    // 37. UNNECESSARY_CAST: cast from int to int
    verify_diagnostic(
        "void main() { int x = 1; var y = x as int; }",
        UNNECESSARY_CAST,
    );

    // 38. UNNECESSARY_TYPE_CHECK_TRUE: an is Object test on int
    verify_diagnostic(
        "void main() { int x = 1; if (x is Object) {} }",
        UNNECESSARY_TYPE_CHECK_TRUE,
    );

    // 39. DEAD_CODE: code after an unconditional return
    verify_diagnostic("void main() { return; int x = 1; }", DEAD_CODE);

    // 40. DEAD_CODE: code after an unconditional throw
    verify_diagnostic("void main() { throw 'erro'; int x = 1; }", DEAD_CODE);
}

// ===========================================================================
// 3. Real SDK Bodies Type-Check (36 libraries)
// ===========================================================================

#[test]
#[ignore]
fn sdk_bodies_type_check() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 not available, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    let mut imports = String::new();
    let mut lib_count = 0;
    for (name, lib) in &sdk.libraries {
        if lib.supported && !name.starts_with('_') {
            imports.push_str(&format!("import 'dart:{name}';\n"));
            lib_count += 1;
        }
    }
    imports.push_str("void main() {}\n");
    fs::write(&entry, &imports).unwrap();

    let mut interner = Interner::new();
    let (program, elements_diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = resolve_outline(&program, &interner, &mut table, &core);

    let t0 = std::time::Instant::now();
    let (body_types, body_diags) =
        infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    let time = t0.elapsed();

    let mut total_exprs = 0;
    let mut resolved_exprs = 0;
    for u in &body_types.units {
        total_exprs += u.expression_count();
        resolved_exprs += u.resolved_targets().count();
    }

    println!("============================================================");
    println!(" Dart SDK 3.6.2 Body Typing ({lib_count} libraries)");
    println!("============================================================");
    println!("Units (.dart):                  {}", program.units.len());
    println!("Typed expressions:              {}", total_exprs);
    println!(
        "Resolved expressions:           {} ({:.1}%)",
        resolved_exprs,
        (resolved_exprs as f64 / total_exprs as f64) * 100.0
    );
    println!("Unique types in TypeTable:      {}", table.len());
    println!("Inference time:                 {:.2?}", time);
    println!("Parser diagnostics:             {}", elements_diags.len());
    println!("Outline diagnostics:            {}", outline_diags.len());
    println!("Body diagnostics:               {}", body_diags.len());
    println!("============================================================");

    // Since the DDC emission, SDK bodies are not inferred (only the outline):
    // the tables of the SDK units stay empty and no warning comes from them.
    assert_eq!(total_exprs, 0, "SDK units do not get body inference");
    assert!(
        body_diags.is_empty(),
        "no body warning may come from the SDK: {}",
        body_diags.len()
    );
}

// ===========================================================================
// 4. Corpus Pub Tipa
// ===========================================================================

#[test]
#[ignore]
fn pub_corpus_type_checks() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 not available, skipping test.");
        return;
    };

    let pub_dir = PathBuf::from("references/pub");
    if !pub_dir.exists() {
        eprintln!("references/pub does not exist, skipping test.");
        return;
    }

    let Ok(entries) = fs::read_dir(&pub_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let pkg_path = entry.path();
        if !pkg_path.is_dir() {
            continue;
        }

        let lib_path = pkg_path.join("lib");
        if !lib_path.exists() {
            continue;
        }

        let Ok(files) = fs::read_dir(&lib_path) else {
            continue;
        };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "dart") {
                let mut interner = Interner::new();
                let (prog, _) = load_lenient(&p, &sdk, None, &mut interner);
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &prog, &interner);
                let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
                let (body_types, _) =
                    infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

                let mut total_exprs = 0;
                for u in &body_types.units {
                    total_exprs += u.expression_count();
                }
                println!(
                    "Package {}: {} typed expressions",
                    pkg_path.file_name().unwrap().to_string_lossy(),
                    total_exprs
                );
                break;
            }
        }
    }
}

// ===========================================================================
// 5. new_sali Type-Checks (Frontend and Backend)
// ===========================================================================

#[test]
#[ignore]
fn new_sali_type_checks() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 not available, skipping test.");
        return;
    };

    let frontend_entry = PathBuf::from("C:/MyDartProjects/new_sali/frontend/web/main.dart");
    let backend_entry = PathBuf::from("C:/MyDartProjects/new_sali/backend/bin/server.dart");

    for (label, entry) in [("Frontend", frontend_entry), ("Backend", backend_entry)] {
        if !entry.exists() {
            eprintln!("{entry:?} not found, skipping {label}.");
            continue;
        }

        let pkg_cfg = entry
            .parent()
            .and_then(|p| p.parent())
            .map(|r| r.join(".dart_tool/package_config.json"));

        let mut interner = Interner::new();
        let (prog, _) = load_lenient(&entry, &sdk, pkg_cfg.as_deref(), &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (body_types, _) =
            infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let mut total_exprs = 0;
        let mut resolved_exprs = 0;
        for u in &body_types.units {
            total_exprs += u.expression_count();
            resolved_exprs += u.resolved_targets().count();
        }

        println!(
            "new_sali {label}: {} units, {} typed expressions, {} resolved",
            prog.units.len(),
            total_exprs,
            resolved_exprs
        );
        assert!(
            total_exprs > 0,
            "new_sali {label} must contain expressions in bodies"
        );
    }
}

/// Body diagnostics of a source over the simulated SDK.
fn diagnostics_of(source: &str) -> Vec<dartforge_diagnostics::Diagnostic> {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline).1
}

#[test]
fn inherited_member_uses_most_specific_declaration() {
    // `CompilationUnitElement implements UriReferencedElement implements
    // _ExistingElement implements Element`: the non-nullable `library` of
    // `_ExistingElement` overrides `Element`'s nullable one, declared earlier.
    let source = "library test; import 'dart:core';
        class Lib { int get n => 0; }
        abstract class Elem { Lib? get library; }
        abstract class Existe implements Elem { Lib get library; }
        abstract class Uri implements Existe {}
        abstract class Unit implements Uri, Elem {}
        int f(Unit u) => u.library.n;";
    let diags = diagnostics_of(source);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn creation_of_non_class_name() {
    let source = "library test; import 'dart:core'; class C<T> { m() => new T(); } void f() { new A(); const B.x(); }";
    let diags = diagnostics_of(source);
    let creations: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.code
                .is_some_and(|c| c.info().name == "creation_with_non_type")
        })
        .map(|d| (&source[d.span.start..d.span.end], d.message.as_str()))
        .collect();
    assert_eq!(
        creations,
        vec![
            ("T", "The name 'T' isn't a class."),
            ("A", "The name 'A' isn't a class."),
            ("B.x", "The name 'x' isn't a class."),
        ],
        "{diags:?}"
    );
}

#[test]
fn imported_same_name_extensions_remain_applicable() {
    // `package:collection` and the analyzer each declare an
    // `IterableExtension`: the name conflicts, the members stay accessible.
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    fs::write(
        tmp.path().join("a.dart"),
        "library a; extension E on String { int get primeiro => 0; }",
    )
    .unwrap();
    fs::write(
        tmp.path().join("b.dart"),
        "library b; extension E on String { int get segundo => 0; }",
    )
    .unwrap();
    let main_dart = tmp.path().join("main.dart");
    fs::write(
        &main_dart,
        "library test; import 'dart:core'; import 'a.dart'; import 'b.dart'; int f(String s) => s.primeiro + s.segundo;",
    )
    .unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn cases_sharing_body_and_irrefutable_case_assign() {
    // Join variable of `case`s that share the body (dart_style), and a
    // `case _:` that makes the no-match exit unreachable.
    let source = "library test; import 'dart:core';
        class A { int? i; int? j; }
        int f(Object o) {
          switch (o) {
            case A a when a.i != null:
            case A a when a.j != null:
              return a.i ?? 0;
          }
          return 0;
        }
        int g(Object o) {
          int x;
          switch (o) {
            case int _:
              x = 1;
            case _:
              x = 0;
          }
          return x;
        }";
    let diags = diagnostics_of(source);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn static_member_through_instance() {
    let source = "library test; import 'dart:core'; class C { static void a() {} static int get g => 0; } void f(C c) { c.a(); c.g; }";
    let diags = diagnostics_of(source);
    let v: Vec<_> = diags
        .iter()
        .map(|d| (&source[d.span.start..d.span.end], d.message.as_str()))
        .collect();
    assert_eq!(
        v,
        vec![
            (
                "a",
                "The static method 'a' can't be accessed through an instance."
            ),
            (
                "g",
                "The static getter 'g' can't be accessed through an instance."
            ),
        ],
        "{diags:?}"
    );
}

#[test]
fn creation_via_alias_infers_nested_argument_from_context() {
    // The context constrains the alias's parameter inside List<X>, both in
    // the implicit creation and in new and the named constructor.
    let source = "library test; import 'dart:core';
        class C<X> { C(); C.nome(); }
        typedef D<X> = C<List<X>>;
        C<List<int>> implicita() => D();
        C<List<int>> explicita() => new D();
        C<List<int>> nomeada() => D.nome();
        C<List<int>> nomeadaExplicita() => new D.nome();
        C<List<String>> outroContexto() => D();";
    let diags = diagnostics_of(source);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn creation_via_alias_with_written_argument_keeps_incompatible_type() {
    // Written arguments prevail over the context; inferring dynamic would
    // hide the incompatibility that this case needs to keep.
    let source = "library test; import 'dart:core';
        class C<X> { C(); C.nome(); }
        typedef D<X> = C<List<X>>;
        C<List<String>> f() => D<int>();
        C<List<String>> g() => D<int>.nome();";
    let diags = diagnostics_of(source);
    assert_eq!(diags.len(), 2, "{diags:?}");
    for d in &diags {
        assert!(
            d.message.contains("C<List<int>>") && d.message.contains("C<List<String>>"),
            "{d:?}"
        );
    }
}

#[test]
fn constant_in_case_and_promotion_on_rhs_of_if_null() {
    // `case _k:` reads the constant (it does not declare `_k` without a
    // value); `o ??= a!` does not promote `a` after the statement (the right
    // side may not run).
    let source = "library test; import 'dart:core';
        const _k = 1;
        int g(int t) {
          switch (t) {
            case _k:
              return _k;
          }
          return t;
        }
        int? h(int? a) {
          int? o;
          o ??= a!;
          return a!;
        }";
    let diags = diagnostics_of(source);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn type_arguments_out_of_bounds() {
    // `C<Object>` and the alias `F<Object>` violate `X extends num`;
    // `C<dynamic>` is super-bounded (the inverted `C<Never>` fits); `C<int>`
    // fits; `extends` does not accept super-bounded. The bound of the
    // typedef's parameter must be resolved in the outline.
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "library test; import 'dart:core';
        class C<X extends num> {}
        typedef F<X extends num> = X Function();
        class D extends C<dynamic> {}
        void f() { C<Object> a; F<Object> b; C<int> c; C<dynamic> d; }";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let unit = prog.library(prog.entry.unwrap()).units[0];
    let diags = dartforge_types::bound_checks::arguments_out_of_bounds(
        &prog, &interner, &mut table, &core, &outline, None, unit,
    );
    let v: Vec<(&str, &str)> = diags
        .iter()
        .map(|d| (&source[d.span.start..d.span.end], d.message.as_str()))
        .collect();
    assert_eq!(
        v,
        vec![
            (
                "dynamic",
                "'dynamic' doesn't conform to the bound 'num' of the type parameter 'X'."
            ),
            (
                "Object",
                "'Object' doesn't conform to the bound 'num' of the type parameter 'X'."
            ),
            (
                "Object",
                "'Object' doesn't conform to the bound 'num' of the type parameter 'X'."
            ),
        ],
        "{diags:?}"
    );
}

#[test]
fn void_result_use() {
    // Argument, initializer and condition with a `void` value: the analyzer
    // reports `use_of_void_result` (at the name, in a function invocation),
    // not `argument_type_not_assignable`/`invalid_assignment`.
    // `void v = f();` and `f();` as a statement are allowed.
    let source = "library test; import 'dart:core';
        void f() {}
        void g(Object? o) {}
        void h(void x) {
          g(f());
          Object? y = f();
          void v = f();
          f();
          if (x) {}
        }";
    let diags = diagnostics_of(source);
    let v: Vec<(&str, &str)> = diags
        .iter()
        .map(|d| {
            (
                &source[d.span.start..d.span.end],
                d.code.map_or("", |c| c.info().name),
            )
        })
        .collect();
    assert_eq!(
        v,
        vec![
            ("f", "use_of_void_result"),
            ("f", "use_of_void_result"),
            ("x", "use_of_void_result")
        ],
        "{diags:?}"
    );
}

// ===========================================================================
// Inference rules measured against the oracle (analyzer 3.6.2), with the real
// SDK: the displayed type of an expression, as in the corpus dump.
// ===========================================================================

/// `(snippet, ty)` of each expression of `source`, in code order (with the
/// real SDK; `None` without it). The types come out as in the corpus dump.
fn types_with_sdk(source: &str) -> Option<Vec<(String, String)>> {
    let (_, sdk) = get_real_sdk()?;
    let tmp = tempdir().unwrap();
    let main = tmp.path().join("main.dart");
    fs::write(&main, source).unwrap();
    let lines = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let d = dartforge_types::dump::dump(&main, &sdk, None);
            d.units.into_iter().next().expect("dumped unit").lines
        })
        .unwrap()
        .join()
        .unwrap();
    // ASCII source: UTF-16 offsets = byte offsets.
    Some(
        lines
            .into_iter()
            .map(|l| (source[l.offset..l.offset + l.length].to_string(), l.ty))
            .collect(),
    )
}

/// Type of the `n`-th (from 0) expression whose text is `snippet`.
fn type_of<'a>(types: &'a [(String, String)], snippet: &str, n: usize) -> &'a str {
    types
        .iter()
        .filter(|(t, _)| t == snippet)
        .nth(n)
        .map(|(_, ty)| ty.as_str())
        .unwrap_or_else(|| panic!("expression `{snippet}` #{n} not found in {types:?}"))
}

#[test]
fn override_inherits_inferred_type_of_untyped_field() {
    // Override inference over a field without a written type: the field's
    // type only exists after the initializer's inference, and the
    // overriding getter and setter inherit it (also as the body's context).
    let Some(t) = types_with_sdk(
        "class A { var lista = [1.5]; var n = 1; }
class B extends A {
  @override get lista => [];
  @override set n(v) { print(v); }
}
void f(B b) { print(b.lista); }",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "[]", 0), "List<double>");
    assert_eq!(type_of(&t, "v", 0), "int");
    assert_eq!(type_of(&t, "b.lista", 0), "List<double>");
}

#[test]
fn instantiation_to_bounds_follows_dependencies_and_written_bound() {
    // A raw `A<T extends U, U extends num>` is `A<num, num>`; a written
    // `Object?` is a bound (not the implicit one, which becomes `dynamic`)
    // and appears in the display.
    let Some(t) = types_with_sdk(
        "class A<T extends U, U extends num> {}
class O<T extends Object?> {}
class P<T> {}
void g<T, U extends Object?>(T t, U u) {}
void f() { A a = A(); var o = O(); var p = P(); print([a, o, p, g]); }",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "A()", 0), "A<num, num>");
    assert_eq!(type_of(&t, "O()", 0), "O<Object?>");
    assert_eq!(type_of(&t, "P()", 0), "P<dynamic>");
    assert_eq!(
        type_of(&t, "g", 0),
        "void Function<T, U extends Object?>(T, U)"
    );
}

#[test]
fn horizontal_inference_follows_dependencies_between_closures() {
    // `c` fixes T; `b` depends on T and fixes U; `a` depends on U: the order
    // is c, b, a, not the text order.
    let Some(t) = types_with_sdk(
        "V h<T, U, V>(V Function(U) a, U Function(T) b, T c) => a(b(c));
void f() { var r = h((u) => u.isEven, (t) => t * 2, 1); print(r); }",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "(u) => u.isEven", 0), "bool Function(int)");
    assert_eq!(type_of(&t, "(t) => t * 2", 0), "int Function(int)");
    assert_eq!(type_of(&t, "r", 0), "bool");
}

#[test]
fn generator_without_yield_and_empty_return_like_analyzer_3_6_2() {
    // A generator without `yield` or `return`: element `dynamic`; `return;`
    // in a generator contributes `Null` (the 3.6.2 rule, before the 2024-12
    // change).
    let Some(t) = types_with_sdk(
        "void f() {
  var c = () sync* {};
  var d = () sync* { yield 1; return; };
  var e = () async* { yield 1; };
  print([c, d, e]);
}",
    ) else {
        return;
    };
    assert_eq!(
        type_of(&t, "() sync* {}", 0),
        "Iterable<dynamic> Function()"
    );
    assert_eq!(
        type_of(&t, "() sync* { yield 1; return; }", 0),
        "Iterable<int?> Function()"
    );
    assert_eq!(
        type_of(&t, "() async* { yield 1; }", 0),
        "Stream<int> Function()"
    );
}

#[test]
fn if_null_with_non_completing_rhs_promotes_lhs() {
    // `y ?? (throw 0)`: on the branch where `y` is not null it is promoted;
    // the other does not complete, so `y` stays promoted afterwards. With a
    // right side that completes, nothing is promoted.
    let Some(t) = types_with_sdk(
        "void f(int? y, int? w) {
  var z = y ?? (throw 0);
  print([z, y]);
  var k = w ?? 0;
  print([k, w]);
}",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "y", 1), "int");
    assert_eq!(type_of(&t, "[z, y]", 0), "List<int>");
    assert_eq!(type_of(&t, "w", 1), "int?");
}

#[test]
fn completing_finally_carries_promotions_past_try() {
    // `attachFinally`: the `finally`'s promotions hold after the `try`, and
    // what the `finally` writes keeps its state.
    let Some(t) = types_with_sdk(
        "void f(Object o, int? x) {
  try { print(0); } finally { o as int; }
  print(o);
  try { print(1); } finally { x = 1; }
  print(x);
}",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "o", 1), "int");
    assert_eq!(type_of(&t, "x", 1), "int");
}

#[test]
fn member_from_different_interfaces_uses_most_specific_signature() {
    // Combined member signature: between `num get v` (I1) and `int get v`
    // (I2), the one whose type is a subtype of all the others wins, not the
    // first in the lookup.
    let Some(t) = types_with_sdk(
        "abstract class I1 { num get v; }
abstract class I2 { int get v; }
abstract class J implements I1, I2 {}
abstract class K implements I2, I1 {}
void f(J j, K k) { print([j.v, k.v]); }",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "j.v", 0), "int");
    assert_eq!(type_of(&t, "k.v", 0), "int");
}

#[test]
fn property_access_on_never_receiver() {
    // analyzer 3.6.2: `id.x` (PrefixedIdentifier) with `id: Never` is
    // `Never`; `failure().x` (PropertyAccess) only looks in `Object`: an
    // `Object` member has its type, another name gets the recovery type
    // `InvalidType`.
    let Some(t) = types_with_sdk(
        "Never falha() => throw 0;
void f(bool b, Never n) {
  if (b) print(falha().x);
  if (b) print(falha().hashCode);
  if (b) print(n.x);
}",
    ) else {
        return;
    };
    assert_eq!(type_of(&t, "falha().x", 0), "InvalidType");
    assert_eq!(type_of(&t, "falha().hashCode", 0), "int");
    assert_eq!(type_of(&t, "n.x", 0), "Never");
}

/// The `dead_code` fragments as the analyzer's `NullSafetyDeadCodeVerifier`
/// reports them: from the first unreachable node to the end of the basic
/// block (the last statement; in a constructor, its end), the whole
/// unreachable branch (`if (false) { … }`, the body of `while (false)`), and
/// the updates of a `for` whose body is unreachable. Checked against
/// `dart analyze` 3.6.2.
#[test]
fn dead_code_snippets_like_analyzer() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");
    let code = "void h() { for (int i = 0; false; i++) { print(i); } print(2); }\n\
        void k() { return; print(1); if (true) { print(2); } }\n\
        class A { A() { return; print(3); } }\n\
        void m(bool b) { if (false) { print(4); } else { print(5); } while (false) {} }\n\
        void print(Object? o) {}\n";
    let source = format!("library test_neg;\nimport 'dart:core';\n{code}");
    fs::write(&main_dart, &source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let mut snippets: Vec<&str> = diags
        .iter()
        .filter(|d| {
            d.message.contains(DEAD_CODE.template)
                || d.code.is_some_and(|c| c.info().name == "dead_code")
        })
        .map(|d| &source[d.span.start..d.span.end])
        .collect();
    snippets.sort();
    let mut expected = vec![
        "i++",
        "{ print(i); }",
        "print(1); if (true) { print(2); }",
        "print(3); }",
        "{ print(4); }",
        "{}",
    ];
    expected.sort();
    assert_eq!(snippets, expected, "{diags:?}");
}

/// A signature omitted in an augmentation: the types come from the augmented
/// declaration. Analyzer 3.13.4 (`augmentations`, `enhanced-parts`) reports
/// exactly these four errors; without the inheritance, `f`, `g` and `topo`
/// would be `dynamic` and nothing would be reported. (The 3.13.4 CFE does not
/// inherit: it compiles and fails at runtime, a divergence recorded in
/// docs/AUGMENTATIONS.md.)
#[test]
fn augmentation_inherits_omitted_types() {
    let tmp = tempdir().unwrap();
    let mut sdk = mock_sdk(tmp.path());
    sdk.experiments
        .push(dartforge_frontend::features::Feature::Augmentations);
    sdk.experiments
        .push(dartforge_frontend::features::Feature::EnhancedParts);
    let mut interner = Interner::new();
    let the_proj = tmp.path().join("proj");
    fs::create_dir_all(&the_proj).unwrap();
    let principal = "part 'p.dart';\n\
        class C {\n  String f(int x);\n  int get g;\n}\n\
        List<int> topo(int n);\n\
        void main() {\n  int a = C().f(3);\n  C().f('x');\n  String b = C().g;\n  String c = topo(1);\n}\n";
    fs::write(the_proj.join("main.dart"), principal).unwrap();
    fs::write(
        the_proj.join("p.dart"),
        "part of 'main.dart';\naugment class C {\n  augment f(x) => 'v';\n  augment get g => 41;\n}\naugment topo(n) => <int>[];\n",
    )
    .unwrap();
    let (prog, load_diags) = load_lenient(&the_proj.join("main.dart"), &sdk, None, &mut interner);
    assert!(
        load_diags.iter().all(|d| !d.message.contains("augment")),
        "{load_diags:?}"
    );
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let snippet = |d: &dartforge_diagnostics::Diagnostic| {
        principal
            .get(d.span.start..d.span.end)
            .unwrap_or("")
            .to_string()
    };
    // The analyzer's code, or the `types::codes` template (the parity bridge
    // converts one into the other).
    let code = |d: &dartforge_diagnostics::Diagnostic| match d.code.map(|c| c.info().name) {
        Some(n) => n,
        None if d.message.starts_with(INVALID_ASSIGNMENT.template) => "invalid_assignment",
        None if d.message.starts_with(ARGUMENT_TYPE_NOT_ASSIGNABLE.template) => {
            "argument_type_not_assignable"
        }
        None => "",
    };
    let mut findings: Vec<(String, String)> = diags
        .iter()
        .filter(|d| {
            matches!(
                code(d),
                "invalid_assignment" | "argument_type_not_assignable"
            )
        })
        .map(|d| (code(d).to_string(), snippet(d)))
        .collect();
    findings.sort();
    assert_eq!(
        findings,
        vec![
            (
                "argument_type_not_assignable".to_string(),
                "'x'".to_string()
            ),
            ("invalid_assignment".to_string(), "C().f(3)".to_string()),
            ("invalid_assignment".to_string(), "C().g".to_string()),
            ("invalid_assignment".to_string(), "topo(1)".to_string()),
        ],
        "{diags:?}"
    );
}

/// Enum augmentation: new values (after those of the declaration), instance
/// and static members. `dart analyze` 3.13.4 (`augmentations`,
/// `enhanced-parts`) reports nothing on this program; here neither, and the
/// types come out as its own (`int n = Cor.values.length` without an error;
/// `String s = Cor.azul.index` is `invalid_assignment`, as there). The 3.13.4
/// CFE rejects enum augmentation ("already declared"): no runtime oracle.
#[test]
fn enum_augmentation_adds_values_and_members() {
    let tmp = tempdir().unwrap();
    let mut sdk = mock_sdk(tmp.path());
    sdk.experiments
        .push(dartforge_frontend::features::Feature::Augmentations);
    sdk.experiments
        .push(dartforge_frontend::features::Feature::EnhancedParts);
    let mut interner = Interner::new();
    let the_proj = tmp.path().join("proj");
    fs::create_dir_all(&the_proj).unwrap();
    let principal = "part 'p.dart';\n\
        enum Cor {\n  vermelho('v'),\n  verde('g');\n  const Cor(this.sigla);\n  final String sigla;\n}\n\
        void main() {\n  int n = Cor.values.length;\n  String m = Cor.azul.maiuscula;\n  Cor c = Cor.porSigla('g');\n  String s = Cor.azul.index;\n}\n";
    fs::write(the_proj.join("main.dart"), principal).unwrap();
    fs::write(
        the_proj.join("p.dart"),
        "part of 'main.dart';\naugment enum Cor {\n  azul('b');\n  String get maiuscula => sigla;\n  static Cor porSigla(String s) => Cor.verde;\n}\n",
    )
    .unwrap();
    let (prog, load_diags) = load_lenient(&the_proj.join("main.dart"), &sdk, None, &mut interner);
    assert!(
        load_diags.iter().all(|d| !d.message.contains("augment")),
        "{load_diags:?}"
    );
    let color_class = prog
        .classes
        .iter()
        .find(|c| interner.resolve(c.name) == "Cor")
        .expect("Cor");
    let values: Vec<&str> = color_class
        .enum_constants
        .iter()
        .map(|v| interner.resolve(prog.variable(*v).name))
        .collect();
    assert_eq!(values, vec!["vermelho", "verde", "azul"]);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let errors: Vec<String> = diags
        .iter()
        .filter(|d| d.severity == dartforge_diagnostics::Severity::Error)
        .map(|d| {
            format!(
                "{} @ {}",
                d.message,
                principal.get(d.span.start..d.span.end).unwrap_or("?")
            )
        })
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    // `invalid_assignment` comes out with the analyzer's code and text.
    assert!(
        errors[0]
            .starts_with("A value of type 'int' can't be assigned to a variable of type 'String'.")
            && errors[0].ends_with("@ Cor.azul.index"),
        "{errors:?}"
    );
}

#[test]
fn switch_on_nullable_sealed_is_exhaustive() {
    // `isAlwaysExhaustive` ignores the `?`: `case var y?` and `case null`
    // cover the `S?` of a `sealed` class, and `r` is assigned after the
    // `switch` (the `fieldType` of analyzer 7.7.1's `ast_builder.dart`).
    let source = "library test; import 'dart:core';
        sealed class S {}
        class A extends S {}
        String f(S? x) {
          String r;
          switch (x) {
            case var y?: r = 'a';
            case null: r = 'n';
          }
          return r;
        }";
    let diags = diagnostics_of(source);
    assert!(
        !diags
            .iter()
            .any(|d| d.message.contains("is read before it is assigned")),
        "{diags:?}"
    );
}

#[test]
fn wildcard_of_representation_type_covers_extension_type() {
    // `promoteForPattern` compares extension type erasures: `int _` matches
    // every `E`, so `j` is assigned after the `switch`
    // (tests/language/extension_type/fully_covers_test.dart).
    let source = "library test; import 'dart:core';
        extension type E(int i) {}
        void use(Object? o) {}
        void f(E e) {
          int j;
          switch (e) {
            case int _: j = 0;
          }
          use(j);
        }";
    let diags = diagnostics_of(source);
    assert!(
        !diags
            .iter()
            .any(|d| d.message.contains("is read before it is assigned")),
        "{diags:?}"
    );
}

/// Body and constant (`const_eval::verify`) diagnostics of a source over the
/// SDK 3.6.2, or `None` (the test skips) when it is not available.
fn constant_diagnostics_of(source: &str) -> Option<Vec<dartforge_diagnostics::Diagnostic>> {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 not available, skipping test.");
        return None;
    };
    let tmp = tempdir().unwrap();
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let entry = prog.entry.expect("the entry library");
    // As the native compiler: the entry library's bodies, with its locals.
    let (bodies, mut diags) = dartforge_types::infer_library_bodies_with_locals(
        &prog,
        &interner,
        &mut table,
        &core,
        &mut outline,
        &[entry],
    );
    let constants = dartforge_types::const_eval::verify(
        &prog,
        &interner,
        &mut table,
        &core,
        &outline,
        &bodies,
        &[entry].into_iter().collect(),
        &[entry],
    );
    diags.extend(constants.into_iter().map(|(_, d)| d));
    Some(diags)
}

/// The names of the diagnostic codes in `diags`.
fn code_names(diags: &[dartforge_diagnostics::Diagnostic]) -> Vec<&'static str> {
    diags
        .iter()
        .filter_map(|d| d.code.map(|c| c.info().name))
        .collect()
}

#[test]
fn constant_is_test_selects_the_conditional_branch() {
    // `l is String ? l.length : d` with a list: the `is` is false and the
    // `length` read is not evaluated (tests/language/constants_2018).
    let source = "class T {
          final Object value;
          const T.length(dynamic l, int d) : value = l is String ? l.length : d;
          const T.length2(dynamic l, int d) : value = l is! String ? d : l.length;
        }
        void main() {
          const l = [1];
          print(const T.length(l, 42));
          print(const T.length2(l, 42));
          print(const T.length('s', 42));
        }";
    let Some(diags) = constant_diagnostics_of(source) else {
        return;
    };
    assert!(diags.is_empty(), "{diags:?}");
    // The taken branch is still evaluated: `length` on a list is an error.
    let source = "class T {
          final Object value;
          const T.length(dynamic l, int d) : value = l is! String ? l.length : d;
        }
        void main() {
          const l = [1];
          print(const T.length(l, 42));
        }";
    let diags = constant_diagnostics_of(source).unwrap();
    assert!(
        code_names(&diags).contains(&"const_eval_property_access"),
        "{diags:?}"
    );
}

#[test]
fn constant_is_test_uses_the_constructor_type_arguments() {
    // `x is T ? x : null` in `C<num>`: `T` is `num`, and the field `T? t`
    // receives the `int` (tests/language/nnbd/const/potentially_constant_types).
    let source = "class C<T> {
          final T? t;
          final bool isListT;
          const C(dynamic x) : t = (x is T) ? x : null, isListT = x is List<T>;
        }
        void main() {
          print(const C<num>(0));
          print(const C<num>('hello'));
          print(const C<int>(<num>[0]));
        }";
    let Some(diags) = constant_diagnostics_of(source) else {
        return;
    };
    assert!(diags.is_empty(), "{diags:?}");
}
