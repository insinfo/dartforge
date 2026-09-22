//! Unidades `dart:` não recebem inferência de corpos: só o outline do SDK é
//! necessário para tipar código do usuário (docs/EMISSAO-DDC.md — o `dart:*`
//! executado é o `dart_sdk.js`). O programa do usuário continua tipado, e uma
//! constante do SDK sem tipo escrito ainda ganha tipo pelo inicializador.

use dartforge_elements::load::load;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, Type, TypeTable};
use dartforge_types::{infer_program_bodies, resolve_outline};
use std::fs;
use tempfile::tempdir;

fn sdk_simulado(dir: &std::path::Path) -> SdkLayout {
    let lib_dir = dir.join("lib");
    fs::create_dir_all(lib_dir.join("core")).unwrap();
    fs::create_dir_all(lib_dir.join("convert")).unwrap();
    fs::write(
        lib_dir.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{
            "core":{"uri":"core/core.dart","patches":["core/core_patch.dart"]},
            "convert":{"uri":"convert/convert.dart","patches":[]}}}}"#,
    )
    .unwrap();
    fs::write(
        lib_dir.join("core/core.dart"),
        r#"
        library dart.core;
        part 'int.dart';
        class Object { const Object(); }
        class num extends Object {}
        class double extends num {}
        class String extends Object { int get length => 0; }
        class bool extends Object {}
        class Null extends Object {}
        class Function extends Object {}
        class Record extends Object {}
        class Iterable<E> extends Object {}
        class List<E> extends Object implements Iterable<E> {}
        class Map<K, V> extends Object {}
        class Set<E> extends Object implements Iterable<E> {}
        external void print(Object? o);
        "#,
    )
    .unwrap();
    fs::write(
        lib_dir.join("core/int.dart"),
        r#"
        part of dart.core;
        class int extends num {
            int operator +(int other) { var soma = this; return soma; }
            external int operator -(int other);
        }
        "#,
    )
    .unwrap();
    fs::write(
        lib_dir.join("core/core_patch.dart"),
        r#"
        @patch
        void print(Object? o) { var texto = o; texto = o; }
        "#,
    )
    .unwrap();
    fs::write(
        lib_dir.join("convert/convert.dart"),
        r#"
        library dart.convert;
        import 'dart:core';
        class JsonCodec extends Object {
            const JsonCodec();
            Object? decode(String s) { var x = s.length; return x; }
        }
        const json = JsonCodec();
        "#,
    )
    .unwrap();
    SdkLayout::load(&lib_dir, "dartdevc").unwrap()
}

#[test]
fn sdk_sem_corpos_e_usuario_tipado() {
    let tmp = tempdir().unwrap();
    let sdk = sdk_simulado(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(
        &main,
        r#"
        import 'dart:convert';
        int dobro(int x) { var y = x + x; return y; }
        void main() { var d = json.decode("1"); print(d); var c = JsonCodec(); print(c); }
        "#,
    )
    .unwrap();

    let mut interner = Interner::new();
    let program = load(&main, &sdk, None, &mut interner).expect("programa carrega");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, diags) = infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);

    // Unidades do SDK (biblioteca, parte e patch): tabelas vazias, sem avisos.
    let mut sdk_units = 0;
    for (i, u) in program.units.iter().enumerate() {
        if program.library(u.library).is_sdk {
            sdk_units += 1;
            assert!(u.ast.exprs.len() > 0, "{}: a unidade do SDK tem expressões na AST", u.uri);
            assert!(bodies.units[i].static_types.is_empty(), "{}: SDK não deve ter corpos inferidos", u.uri);
            assert!(bodies.units[i].resolved.is_empty(), "{}: SDK não deve ter resoluções", u.uri);
        }
    }
    assert_eq!(sdk_units, 4, "core, int.dart (parte), core_patch e convert");
    assert!(diags.is_empty(), "sem avisos do usuário nem do SDK: {diags:?}");

    // O usuário continua tipado: `y` em `dobro` é `int`.
    let entry_unit = program.library(program.entry.unwrap()).units[0];
    let unit = program.unit(entry_unit);
    let tabela = &bodies.units[entry_unit.0 as usize];
    assert_eq!(tabela.static_types.len(), unit.ast.exprs.len());
    let int_class = program.classes.iter().position(|c| interner.resolve(c.name) == "int").unwrap();
    let mut y_int = false;
    for (i, e) in unit.ast.exprs.iter().enumerate() {
        if let dartforge_frontend::ast::ExprKind::Identifier(n) = &e.kind {
            if interner.resolve(n.sym) == "y" {
                let ty = table.get(tabela.static_types[i]);
                if let Type::Interface { class, .. } = ty {
                    y_int |= class.0 as usize == int_class;
                }
            }
        }
    }
    assert!(y_int, "`y` deve ter tipo `int`");

    // `const json = JsonCodec()` do SDK, sem tipo escrito, ainda tem o
    // inicializador visitado (`inferred` preenchido) — é o que `json.decode`
    // no usuário precisa. (Hoje `JsonCodec()` sem `new` infere `dynamic` tanto
    // no SDK quanto no usuário: lacuna do resolvedor, não deste corte.)
    let json_var = program.variables.iter().position(|v| interner.resolve(v.name) == "json").unwrap();
    assert!(outline.variables[json_var].inferred.is_some(), "json tem tipo inferido");
}
