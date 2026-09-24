//! Suíte de testes normativos de corpos de funções, inferência de tipos, análise de fluxo e diagnósticos.
//!
//! Cobre:
//! 1. `promocao_e_inferencia`: testes unitários rápidos (<100ms) sem SDK.
//! 2. `negativos_do_analyzer`: >= 40 casos negativos do analyzer oficial.
//! 3. `corpos_do_sdk_tipam`: todas as 36 bibliotecas do SDK tipam com estatísticas.
//! 4. `corpus_pub_tipa`: pacotes de `references/pub` tipam.
//! 5. `new_sali_tipa`: frontend e backend do `new_sali` tipam.

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

    SdkLayout::load(&lib_dir, "dartdevc").expect("carrega layout sdk simulado")
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
// 1. Promoção de Fluxo e Inferência (sem SDK)
// ===========================================================================

#[test]
fn promocao_e_inferencia() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega programa de teste");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, outline_diags) = resolve_outline(&prog, &interner, &mut table, &core);

    assert!(outline_diags.is_empty(), "Outline não deve ter diagnósticos: {outline_diags:?}");

    let (body_types, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    // A unidade 0 é `dart:core` (sem inferência de corpos); a do usuário é a da entrada.
    let entry_unit = prog.library(prog.entry.unwrap()).units[0];
    let unit = &body_types.units[entry_unit.0 as usize];
    assert!(unit.static_types.len() > 10, "Expressões devem estar tipadas");

    // Verifica que não houve diagnósticos espúrios
    assert!(diags.is_empty(), "Corpos válidos não devem emitir diagnósticos: {diags:?}");
}

// ===========================================================================
// 2. Casos Negativos do Analyzer (>= 40 testes traduzidos da suite oficial)
// ===========================================================================

fn verificar_diagnostico(codigo_dart: &str, diagnostic_esperado: DiagnosticCode) {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

    fs::write(&main_dart, format!("library test_neg;\nimport 'dart:core';\n{codigo_dart}")).unwrap();

    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let encontrou = diags.iter().any(|d| d.message.contains(diagnostic_esperado.template));
    assert!(
        encontrou,
        "Esperava diagnóstico '{}', mas obteve:\n{:?}",
        diagnostic_esperado.template,
        diags
    );
}

#[test]
fn getter_de_classe_sem_setter_em_atribuicao_simples() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; class A { int get x => 0; } class B { int get x => 0; set x(int v) {} } void f(A a, B b) { a.x = 0; a.x += 0; ++a.x; a.x++; a.y = 0; b.x = 0; b.x += 0; ++b.x; b.x++; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)).collect();
    assert_eq!(readonly.len(), 4, "{diags:?}");
    for (d, alvo) in readonly.iter().zip(["a.x = 0", "a.x += 0", "++a.x", "a.x++"]) {
        let x = fonte.find(alvo).unwrap() + alvo.find("x").unwrap();
        assert_eq!(d.span.start as usize, x, "{diags:?}");
        assert_eq!(d.span.end as usize, x + 1, "{diags:?}");
        assert!(d.message.contains("'x' na classe 'A'"), "{diags:?}");
    }
    let missing: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_SETTER.template)).collect();
    assert_eq!(missing.len(), 1, "{diags:?}");
    assert_eq!(missing[0].span.start as usize, fonte.find("a.y = 0").unwrap() + 2);
}

#[test]
fn getter_lexico_sem_setter_em_atribuicoes_e_incrementos() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; class A { int get x => 0; static int get s => 0; void f() { x = 0; x += 0; ++x; x++; s = 0; s += 0; ++s; s++; } } class B extends A { void g() { x = 0; } }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)).collect();
    assert_eq!(readonly.len(), 9, "{diags:?}");
    for (d, alvo) in readonly.iter().zip(["x = 0", "x += 0", "++x", "x++", "s = 0", "s += 0", "++s", "s++"]) {
        let indice = fonte.find(alvo).unwrap() + alvo.find(|c| c == 'x' || c == 's').unwrap();
        assert_eq!(d.span.start as usize, indice, "{diags:?}");
        assert_eq!(d.span.end as usize, indice + 1, "{diags:?}");
        assert!(d.message.contains("na classe 'A'"), "{diags:?}");
    }
    let herdado = fonte.rfind("x = 0").unwrap();
    assert_eq!(readonly[8].span.start as usize, herdado, "{diags:?}");
    assert!(readonly[8].message.contains("na classe 'A'"), "{diags:?}");
}

#[test]
fn getter_de_extensao_sem_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on int { int get x => 0; } void f() { 0.x = 0; 0.x += 0; ++0.x; 0.x++; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let readonly: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)).collect();
    assert_eq!(readonly.len(), 4, "{diags:?}");
    for (d, alvo) in readonly.iter().zip(["0.x = 0", "0.x += 0", "++0.x", "0.x++"]) {
        let indice = fonte.find(alvo).unwrap() + alvo.find('x').unwrap();
        assert_eq!(d.span.start as usize, indice, "{diags:?}");
        assert_eq!(d.span.end as usize, indice + 1, "{diags:?}");
        assert!(d.message.contains("na classe 'E'"), "{diags:?}");
    }
}

#[test]
fn sobreposicao_explicita_de_extensao_sem_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on int { int get foo => 0; } extension F on int { set bar(int v) {} } void f() { E(0).foo = 1; E(0).foo += 1; F(0).bar = 1; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_SETTER.template)).collect();
    assert_eq!(ausentes.len(), 2, "{diags:?}");
    for d in ausentes {
        assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "foo", "{diags:?}");
        assert!(d.message.contains("'foo' em 'E'"), "{diags:?}");
    }
    assert!(!diags.iter().any(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)), "{diags:?}");
    assert!(!diags.iter().any(|d| d.message.contains("'bar'")), "{diags:?}");
}

#[test]
fn sobreposicao_explicita_sem_getter_usa_codigo_de_extensao() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on int { set foo(int v) {} } void f() { E(0).foo; E(0).foo += 1; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_GETTER.template)).collect();
    assert_eq!(ausentes.len(), 2, "{diags:?}");
    for d in ausentes {
        assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "foo");
        assert!(d.message.contains("'foo' em 'E'"));
    }
    assert!(!diags.iter().any(|d| d.message.starts_with(UNDEFINED_GETTER.template)), "{diags:?}");
}

#[test]
fn getter_estatico_ausente_em_extensao_do_oraculo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (fonte, offset) in [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_getter/UndefinedExtensionGetter__static_withou_8f374a5b.dart")), 40),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_getter/UndefinedExtensionGetter__static_withInference.dart")), 35),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, fonte).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_GETTER.template)).collect();
        assert_eq!(ausentes.len(), 1, "{diags:?}");
        assert_eq!((ausentes[0].span.start, ausentes[0].span.end), (offset, offset + 1));
        assert!(ausentes[0].message.contains("'v' em 'E'"));
    }
}

#[test]
fn sobreposicao_explicita_sem_metodo_usa_codigo_de_extensao() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on String {} void f() { E('a').m(); }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_METHOD.template)).collect();
    assert_eq!(ausentes.len(), 1, "{diags:?}");
    assert_eq!(&fonte[ausentes[0].span.start as usize..ausentes[0].span.end as usize], "m");
    assert!(ausentes[0].message.contains("'m' em 'E'"));
}

#[test]
fn metodo_estatico_ausente_em_extensao_do_oraculo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (fonte, offset) in [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_method/UndefinedExtensionMethod__static_withInference.dart")), 35),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_method/UndefinedExtensionMethod__static_withou_709c4429.dart")), 40),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, fonte).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_METHOD.template)).collect();
        assert_eq!(ausentes.len(), 1, "{diags:?}");
        assert_eq!((ausentes[0].span.start, ausentes[0].span.end), (offset, offset + 1));
        assert!(ausentes[0].message.contains("'m' em 'E'"));
    }
}

#[test]
fn sobreposicao_explicita_de_extensao_rejeita_membros_estaticos() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on String { static String get empty => ''; static void set empty(String s) {} } void f() { E('a').empty; E('a').empty = 'b'; E('a').empty += 'b'; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let estaticos: Vec<_> = diags.iter().filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template).collect();
    assert_eq!(estaticos.len(), 3, "{diags:?}");
    for d in estaticos {
        assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "empty", "{diags:?}");
    }
    assert!(!diags.iter().any(|d| d.message.starts_with(UNDEFINED_EXTENSION_SETTER.template)), "{diags:?}");
}

#[test]
fn metodo_estatico_em_sobreposicao_de_extensao() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on String { static String empty() => ''; String instance() => ''; } void f() { E('a').empty(); E('a').instance(); }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let estaticos: Vec<_> = diags.iter().filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template).collect();
    assert_eq!(estaticos.len(), 1, "{diags:?}");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(&fonte[estaticos[0].span.start as usize..estaticos[0].span.end as usize], "empty");
    assert!(!diags.iter().any(|d| d.message.contains("instance")), "{diags:?}");
}

#[test]
fn call_estatico_em_sobreposicao_de_extensao() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; extension E on int { static void call() {} } void f() { E(0)(); }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let estaticos: Vec<_> = diags.iter().filter(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template).collect();
    assert_eq!(estaticos.len(), 1, "{diags:?}");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(&fonte[estaticos[0].span.start as usize..estaticos[0].span.end as usize], "()");

    let fonte_instancia = "library test; import 'dart:core'; extension F on int { void call() {} } void g() { F(0)(); }";
    fs::write(&main_dart, fonte_instancia).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    assert!(!diags.iter().any(|d| d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template), "{diags:?}");
}

#[test]
fn override_sem_call_do_oraculo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/invocation_of_extension_without_call/InvocationOfExtensionWithoutCall__insta_ae18bfa4.dart"));
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let ausentes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(INVOCATION_OF_EXTENSION_WITHOUT_CALL.template)).collect();
    assert_eq!(ausentes.len(), 1, "{diags:?}");
    assert_eq!(&fonte[ausentes[0].span.start as usize..ausentes[0].span.end as usize], "E(0)");
}

#[test]
fn operador_unario_em_override_do_oraculo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (fonte, ausente) in [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_operator/UndefinedExtensionOperator__prefix_minu_99f91916.dart")), true),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_operator/UndefinedExtensionOperator__prefix_minus_defined.dart")), false),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, fonte).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let operadores: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template)).collect();
        assert_eq!(operadores.len(), usize::from(ausente), "{diags:?}");
        if ausente {
            assert_eq!((operadores[0].span.start, operadores[0].span.end), (33, 34));
            assert!(operadores[0].message.contains("'unary-' em 'E'"));
        }
    }
}

#[test]
fn operador_binario_em_override_do_oraculo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let main_dart = tmp.path().join("main.dart");
    for (fonte, ausente) in [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_operator/UndefinedExtensionOperator__binary_undefined.dart")), true),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/undefined_extension_operator/UndefinedExtensionOperator__binary_defined.dart")), false),
    ] {
        let mut interner = Interner::new();
        fs::write(&main_dart, fonte).unwrap();
        let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
        let operadores: Vec<_> = diags.iter().filter(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template)).collect();
        assert_eq!(operadores.len(), usize::from(ausente), "{diags:?}");
        if ausente {
            assert_eq!((operadores[0].span.start, operadores[0].span.end), (40, 41));
            assert!(operadores[0].message.contains("'+' em 'E'"));
        }
    }

    let fonte = "extension E on String {} void f() { E('a') /* + */ + 1; }";
    let mut interner = Interner::new();
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);
    let operador = diags.iter().find(|d| d.message.starts_with(UNDEFINED_EXTENSION_OPERATOR.template)).expect("operador ausente");
    assert_eq!(operador.span.start, fonte.rfind('+').unwrap());
}

#[test]
fn campo_final_sem_setter_e_late_final_atribuivel() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; class A { final int x = 0; late final int y; late final int z = 0; static final int s = 0; static const int c = 0; void f() { x = 0; x += 0; ++x; x++; s = 0; z = 1; c = 1; y = 1; } } void g(A a) { a.x = 0; a.x += 0; ++a.x; a.x++; a.z = 1; a.y = 1; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finais: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template)).collect();
    assert_eq!(finais.len(), 11, "{diags:?}");
    for d in finais {
        let trecho = &fonte[d.span.start as usize..d.span.end as usize];
        assert!(trecho == "x" || trecho == "s" || trecho == "z", "{diags:?}");
        assert!(d.message.contains(&format!("'{trecho}'")), "{diags:?}");
    }
    let constantes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_CONST.template)).collect();
    assert_eq!(constantes.len(), 1, "{diags:?}");
    assert_eq!(&fonte[constantes[0].span.start as usize..constantes[0].span.end as usize], "c");
    assert!(!diags.iter().any(|d| d.message.contains("'y'")), "{diags:?}");
}

#[test]
fn membros_estaticos_somente_leitura_em_atribuicoes() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; class A { static final int x = 0; static int get g => 0; static late final int l; static const int c = 0; } void f() { A.x = 0; A.x += 0; ++A.x; A.x++; A.g = 0; A.g += 0; ++A.g; A.g++; A.c = 1; A.l = 1; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finais: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template)).collect();
    let getters: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL_NO_SETTER.template)).collect();
    let constantes: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_CONST.template)).collect();
    assert_eq!(finais.len(), 4, "{diags:?}");
    assert_eq!(getters.len(), 4, "{diags:?}");
    assert_eq!(constantes.len(), 1, "{diags:?}");
    for d in finais {
        assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "x");
    }
    for d in getters {
        assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "g");
        assert!(d.message.contains("na classe 'A'"));
    }
    assert_eq!(&fonte[constantes[0].span.start as usize..constantes[0].span.end as usize], "c");
    assert!(!diags.iter().any(|d| d.message.contains("'l'")), "{diags:?}");
}

#[test]
fn getter_de_topo_sem_setter_e_par_com_setter() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library test; import 'dart:core'; int get x => 0; int get y => 0; set y(int v) {} void f() { x = 0; x += 0; ++x; x++; y = 0; y += 0; ++y; y++; }";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let finais: Vec<_> = diags.iter().filter(|d| d.message.starts_with(ASSIGNMENT_TO_FINAL.template)).collect();
    assert_eq!(finais.len(), 4, "{diags:?}");
    for (d, alvo) in finais.iter().zip(["x = 0", "x += 0", "++x", "x++"]) {
        let indice = fonte.find(alvo).unwrap() + alvo.find('x').unwrap();
        assert_eq!(d.span.start as usize, indice, "{diags:?}");
        assert_eq!(d.span.end as usize, indice + 1, "{diags:?}");
        assert!(d.message.contains("'x'"), "{diags:?}");
    }
    assert!(!diags.iter().any(|d| d.message.contains("'y'")), "{diags:?}");
}

#[test]
fn atribuicao_a_final_local_marca_somente_o_identificador() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "library teste; import 'dart:core'; void f() { final int x = 0; x = 1; x += 1; ++x; }";
    fs::write(&main_dart, fonte).unwrap();

    let (prog, _) = load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let (_, diags) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

    let achados: Vec<_> = diags.iter().filter(|d| d.message.contains(ASSIGNMENT_TO_FINAL_LOCAL.template)).collect();
    assert_eq!(achados.len(), 3, "{diags:?}");
    for d in achados {
        assert_eq!(&fonte[d.span.start..d.span.end], "x", "{d:?}");
    }
}

#[test]
fn const_local_e_final_de_topo_nao_sao_final_local() {
    verificar_diagnostico("void f() { const x = 1; x = 2; }", ASSIGNMENT_TO_CONST);
    verificar_diagnostico("final int x = 1; void f() { x = 2; }", ASSIGNMENT_TO_FINAL);
}

#[test]
fn negativos_do_analyzer_40_casos() {
    // 1. ARGUMENT_TYPE_NOT_ASSIGNABLE: passa String para int
    verificar_diagnostico(
        "void f(int x) {} void main() { f('str'); }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 2. ARGUMENT_TYPE_NOT_ASSIGNABLE: passa Object para String
    verificar_diagnostico(
        "void f(String s) {} void main(Object o) { f(o); }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 3. ARGUMENT_TYPE_NOT_ASSIGNABLE: operador + de int recebe String
    verificar_diagnostico(
        "void main() { int a = 1; var b = a + 'x'; }",
        ARGUMENT_TYPE_NOT_ASSIGNABLE,
    );

    // 4. RETURN_OF_INVALID_TYPE: retorna String de função int
    verificar_diagnostico(
        "int f() { return 'not an int'; }",
        RETURN_OF_INVALID_TYPE,
    );

    // 5. RETURN_OF_INVALID_TYPE: retorna int de função String
    verificar_diagnostico(
        "String f() { return 42; }",
        RETURN_OF_INVALID_TYPE,
    );

    // 6. ASSIGNMENT_TO_FINAL_LOCAL: reatribuição de variável final
    verificar_diagnostico(
        "void main() { final int x = 1; x = 2; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 7. ASSIGNMENT_TO_FINAL_LOCAL: reatribuição de final inferida
    verificar_diagnostico(
        "void main() { final x = 'a'; x = 'b'; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 8. ASSIGNMENT_TO_FINAL_LOCAL: reatribuição de parâmetro final
    verificar_diagnostico(
        "void main(final int x) { x = 3; }",
        ASSIGNMENT_TO_FINAL_LOCAL,
    );

    // 9. UNDEFINED_GETTER: acesso a getter inexistente em int
    verificar_diagnostico(
        "void main() { int x = 1; var y = x.propriedadeInexistente; }",
        UNDEFINED_GETTER,
    );

    // 10. UNDEFINED_GETTER: acesso a getter inexistente em String
    verificar_diagnostico(
        "void main() { String s = 'a'; var y = s.foo; }",
        UNDEFINED_GETTER,
    );

    // 11. UNDEFINED_METHOD: chamada a método inexistente em int
    verificar_diagnostico(
        "void main() { int x = 1; x.metodoInexistente(); }",
        UNDEFINED_METHOD,
    );

    // 12. UNDEFINED_METHOD: chamada a método inexistente em String
    verificar_diagnostico(
        "void main() { String s = 'hello'; s.bar(); }",
        UNDEFINED_METHOD,
    );

    // 13. UNDEFINED_SETTER: atribuição a setter inexistente em int
    verificar_diagnostico(
        "void main() { int x = 1; x.setterInexistente = 2; }",
        UNDEFINED_SETTER,
    );

    // 14. UNDEFINED_IDENTIFIER: variável não declarada
    verificar_diagnostico(
        "void main() { var y = variavelTotalmenteInexistente; }",
        UNDEFINED_IDENTIFIER,
    );

    // 15. NOT_ENOUGH_POSITIONAL_ARGUMENTS: função esperando 2 argumentos recebe 1
    verificar_diagnostico(
        "void f(int a, int b) {} void main() { f(1); }",
        NOT_ENOUGH_POSITIONAL_ARGUMENTS,
    );

    // 16. NOT_ENOUGH_POSITIONAL_ARGUMENTS: função esperando 1 argumento recebe 0
    verificar_diagnostico(
        "void f(int a) {} void main() { f(); }",
        NOT_ENOUGH_POSITIONAL_ARGUMENTS,
    );

    // 17. EXTRA_POSITIONAL_ARGUMENTS: função sem argumentos recebe 1
    verificar_diagnostico(
        "void f() {} void main() { f(1); }",
        EXTRA_POSITIONAL_ARGUMENTS,
    );

    // 18. EXTRA_POSITIONAL_ARGUMENTS: função de 1 argumento recebe 2
    verificar_diagnostico(
        "void f(int a) {} void main() { f(1, 2); }",
        EXTRA_POSITIONAL_ARGUMENTS,
    );

    // 19. UNDEFINED_NAMED_PARAMETER: parâmetro nomeado inexistente passado
    verificar_diagnostico(
        "void f({int? x}) {} void main() { f(nomeInexistente: 1); }",
        UNDEFINED_NAMED_PARAMETER,
    );

    // 20. UNDEFINED_NAMED_PARAMETER: função sem nomeados recebe nomeado
    verificar_diagnostico(
        "void f(int a) {} void main() { f(1, foo: 2); }",
        UNDEFINED_NAMED_PARAMETER,
    );

    // 21. MISSING_REQUIRED_ARGUMENT: parâmetro nomeado required omitido
    verificar_diagnostico(
        "void f({required int x}) {} void main() { f(); }",
        MISSING_REQUIRED_ARGUMENT,
    );

    // 22. DEFINITELY_UNASSIGNED: uso de local não inicializada
    verificar_diagnostico(
        "void main() { int x; var y = x + 1; }",
        DEFINITELY_UNASSIGNED_VARIABLE,
    );

    // 23. DEFINITELY_UNASSIGNED: uso em if onde só um ramo atribui
    verificar_diagnostico(
        "void main(bool c) { int x; if (c) { x = 1; } var y = x + 1; }",
        DEFINITELY_UNASSIGNED_VARIABLE,
    );

    // 24. REFERENCED_BEFORE_DECLARATION: variável local referenciada antes da linha
    verificar_diagnostico(
        "void main() { print(x); int x = 10; } void print(Object o) {}",
        REFERENCED_BEFORE_DECLARATION,
    );

    // 25. NON_BOOL_CONDITION: condição de if é int
    verificar_diagnostico(
        "void main() { if (42) {} }",
        NON_BOOL_CONDITION,
    );

    // 26. NON_BOOL_CONDITION: condição de while é String
    verificar_diagnostico(
        "void main() { while ('loop') {} }",
        NON_BOOL_CONDITION,
    );

    // 27. NON_BOOL_CONDITION: condição de do-while é int
    verificar_diagnostico(
        "void main() { do {} while (0); }",
        NON_BOOL_CONDITION,
    );

    // 28. NON_BOOL_CONDITION: operador ternário com condição int
    verificar_diagnostico(
        "void main() { var x = 1 ? 'a' : 'b'; }",
        NON_BOOL_CONDITION,
    );

    // 29. INVALID_ASSIGNMENT: atribuição de String para variável int
    verificar_diagnostico(
        "void main() { int x = 1; x = 'nao int'; }",
        INVALID_ASSIGNMENT,
    );

    // 30. INVALID_ASSIGNMENT: atribuição de Object para variável int
    verificar_diagnostico(
        "void main(Object o) { int x = 1; x = o; }",
        INVALID_ASSIGNMENT,
    );

    // 31. EQUAL_ELEMENTS_IN_CONST_SET: conjunto const com chaves duplicadas
    verificar_diagnostico(
        "void main() { const s = {1, 1}; }",
        EQUAL_ELEMENTS_IN_CONST_SET,
    );

    // 32. EQUAL_KEYS_IN_CONST_MAP: mapa const com chaves repetidas
    verificar_diagnostico(
        "void main() { const m = {'a': 1, 'a': 2}; }",
        EQUAL_KEYS_IN_CONST_MAP,
    );

    // 33. CONST_EVAL_THROWS_EXCEPTION: divisão por zero em const
    verificar_diagnostico(
        "void main() { const x = 1 ~/ 0; }",
        CONST_EVAL_THROWS_EXCEPTION,
    );

    // 34. CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE: const com variável não-const
    verificar_diagnostico(
        "void main() { var a = 1; const b = a; }",
        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE,
    );

    // 35. INVALID_NULL_AWARE_OPERATOR: bang (!) em tipo garantidamente não-nulo
    verificar_diagnostico(
        "void main() { int x = 1; var y = x!; }",
        INVALID_NULL_AWARE_OPERATOR,
    );

    // 36. INVALID_NULL_AWARE_OPERATOR: ?. em int não-nulo
    verificar_diagnostico(
        "void main() { int x = 1; var y = x?.toString(); }",
        INVALID_NULL_AWARE_OPERATOR,
    );

    // 37. UNNECESSARY_CAST: cast de int para int
    verificar_diagnostico(
        "void main() { int x = 1; var y = x as int; }",
        UNNECESSARY_CAST,
    );

    // 38. UNNECESSARY_TYPE_CHECK_TRUE: teste is Object em int
    verificar_diagnostico(
        "void main() { int x = 1; if (x is Object) {} }",
        UNNECESSARY_TYPE_CHECK_TRUE,
    );

    // 39. DEAD_CODE: código após return incondicional
    verificar_diagnostico(
        "void main() { return; int x = 1; }",
        DEAD_CODE,
    );

    // 40. DEAD_CODE: código após throw incondicional
    verificar_diagnostico(
        "void main() { throw 'erro'; int x = 1; }",
        DEAD_CODE,
    );
}

// ===========================================================================
// 3. Corpos do SDK Real Tipam (36 bibliotecas)
// ===========================================================================

#[test]
#[ignore]
fn corpos_do_sdk_tipam() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 não disponível, pulando teste.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    let mut imports = String::new();
    let mut bib_cont = 0;
    for (name, lib) in &sdk.libraries {
        if lib.supported && !name.starts_with('_') {
            imports.push_str(&format!("import 'dart:{name}';\n"));
            bib_cont += 1;
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
    let (body_types, body_diags) = infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    let tempo = t0.elapsed();

    let mut total_exprs = 0;
    let mut resolved_exprs = 0;
    for u in &body_types.units {
        total_exprs += u.static_types.len();
        resolved_exprs += u.resolved.iter().filter(|r| r.is_some()).count();
    }

    println!("============================================================");
    println!(" Tipagem de Corpos do SDK Dart 3.6.2 ({bib_cont} bibliotecas)");
    println!("============================================================");
    println!("Unidades (.dart):               {}", program.units.len());
    println!("Expressões tipadas:             {}", total_exprs);
    println!("Expressões resolvidas:          {} ({:.1}%)",
        resolved_exprs,
        (resolved_exprs as f64 / total_exprs as f64) * 100.0
    );
    println!("Tipos únicos em TypeTable:      {}", table.len());
    println!("Tempo de inferência:            {:.2?}", tempo);
    println!("Diagnósticos de Parser:         {}", elements_diags.len());
    println!("Diagnósticos de Outline:        {}", outline_diags.len());
    println!("Diagnósticos de Corpos:         {}", body_diags.len());
    println!("============================================================");

    // Desde a emissão DDC os corpos do SDK não são inferidos (só o outline):
    // as tabelas das unidades do SDK ficam vazias e nenhum aviso vem delas.
    assert_eq!(total_exprs, 0, "unidades do SDK não recebem inferência de corpos");
    assert!(body_diags.is_empty(), "nenhum aviso de corpos deve vir do SDK: {}", body_diags.len());
}

// ===========================================================================
// 4. Corpus Pub Tipa
// ===========================================================================

#[test]
#[ignore]
fn corpus_pub_tipa() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 não disponível, pulando teste.");
        return;
    };

    let pub_dir = PathBuf::from("references/pub");
    if !pub_dir.exists() {
        eprintln!("references/pub não existe, pulando teste.");
        return;
    }

    let Ok(entries) = fs::read_dir(&pub_dir) else { return; };
    for entry in entries.flatten() {
        let pkg_path = entry.path();
        if !pkg_path.is_dir() { continue; }

        let lib_path = pkg_path.join("lib");
        if !lib_path.exists() { continue; }

        let Ok(files) = fs::read_dir(&lib_path) else { continue; };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "dart") {
                let mut interner = Interner::new();
                let (prog, _) = load_lenient(&p, &sdk, None, &mut interner);
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &prog, &interner);
                let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
                let (body_types, _) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

                let mut total_exprs = 0;
                for u in &body_types.units {
                    total_exprs += u.static_types.len();
                }
                println!("Pacote {}: {} expressões tipadas", pkg_path.file_name().unwrap().to_string_lossy(), total_exprs);
                break;
            }
        }
    }
}

// ===========================================================================
// 5. new_sali Tipa (Frontend e Backend)
// ===========================================================================

#[test]
#[ignore]
fn new_sali_tipa() {
    let Some((_lib_dir, sdk)) = get_real_sdk() else {
        eprintln!("SDK 3.6.2 não disponível, pulando teste.");
        return;
    };

    let frontend_entry = PathBuf::from("C:/MyDartProjects/new_sali/frontend/web/main.dart");
    let backend_entry = PathBuf::from("C:/MyDartProjects/new_sali/backend/bin/server.dart");

    for (label, entry) in [("Frontend", frontend_entry), ("Backend", backend_entry)] {
        if !entry.exists() {
            eprintln!("{entry:?} não encontrado, pulando {label}.");
            continue;
        }

        let pkg_cfg = entry.parent().and_then(|p| p.parent()).map(|r| r.join(".dart_tool/package_config.json"));

        let mut interner = Interner::new();
        let (prog, _) = load_lenient(&entry, &sdk, pkg_cfg.as_deref(), &mut interner);
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &prog, &interner);
        let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
        let (body_types, _) = infer_program_bodies(&prog, &interner, &mut table, &core, &mut outline);

        let mut total_exprs = 0;
        let mut resolved_exprs = 0;
        for u in &body_types.units {
            total_exprs += u.static_types.len();
            resolved_exprs += u.resolved.iter().filter(|r| r.is_some()).count();
        }

        println!("new_sali {label}: {} unidades, {} expressões tipadas, {} resolvidas",
            prog.units.len(),
            total_exprs,
            resolved_exprs
        );
        assert!(total_exprs > 0, "new_sali {label} deve conter expressões em corpos");
    }
}
