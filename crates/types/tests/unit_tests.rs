//! Unit tests without the SDK for `dartforge-types`.
//!
//! Covers the fast unit test requirements (<100ms):
//! 1. Generic typedef with substitution (`typedef F<T> = T Function(T);`) expanded to `int Function(int)`
//! 2. Override inference: getter, setter and method inherit types from the generic superclass (`Base<int>`)
//! 3. Inheritance with a mixin application: `class C extends B with M` gets the union of the instantiated interfaces
//! 4. Extension type: `extension type E(int rep)` erases to `int` and does not subtype `int` unless it uses `implements int`
//! 5. Accumulated diagnostics: a nonexistent type error is recorded and resolved as `Type::Dynamic` without panicking

use dartforge_elements::load::load;
use dartforge_elements::model::*;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::ops::{erase_extension_type, substitute};
use dartforge_types::subtyping::is_subtype;
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_types::resolve_outline;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
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
        class Object {}
        class int extends Object {}
        class num extends Object {}
        class double extends num {}
        class String extends Object {}
        class bool extends Object {}
        class Null extends Object {}
        class Function extends Object {}
        class Record extends Object {}
        class Iterable<E> extends Object {}
        class List<E> extends Object implements Iterable<E> {}
        class Map<K, V> extends Object {}
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

#[test]
fn test_generic_typedef_expansion() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_typedef;
        import 'dart:core';

        typedef F<T> = T Function(T);

        class Target {
            F<int>? cb;
        }
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the program with a typedef");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "there must be no resolution errors: {:?}", diags);

    // Finds the field Target.cb
    let cb_sym = interner.intern("cb");
    let (cb_idx, _) = prog
        .variables
        .iter()
        .enumerate()
        .find(|(_, v)| v.name == cb_sym)
        .expect("finds variable cb");

    let cb_ty = outline.variables[cb_idx]
        .declared_type
        .expect("the declared type of cb must be present");

    // Checks that it is Type::Function with return int and positional int, marked nullable
    match table.get(cb_ty) {
        Type::Function {
            ret,
            positional,
            nullable,
            ..
        } => {
            assert!(*nullable, "cb must be nullable (F<int>?)");
            assert_eq!(*ret, core.int, "the expanded return of the typedef must be int");
            assert_eq!(positional.len(), 1, "must have 1 positional parameter");
            assert_eq!(positional[0], core.int, "the parameter must be int");
        }
        other => panic!("expected Type::Function for F<int>?, got {:?}", other),
    }

    // Also tests direct expansion through the typedef table
    let f_sym = interner.intern("F");
    let (typedef_idx, _) = prog
        .typedefs
        .iter()
        .enumerate()
        .find(|(_, t)| t.name == f_sym)
        .expect("encontra typedef F");

    let typedef_data = &outline.typedefs[typedef_idx];
    assert_eq!(typedef_data.type_params.len(), 1);
    let mut subst_map = HashMap::new();
    subst_map.insert(typedef_data.type_params[0], core.int);
    let substituted = substitute(typedef_data.target_type, &subst_map, &mut table);

    match table.get(substituted) {
        Type::Function { ret, positional, .. } => {
            assert_eq!(*ret, core.int);
            assert_eq!(positional.len(), 1);
            assert_eq!(positional[0], core.int);
        }
        other => panic!("expected an expanded function, got {:?}", other),
    }
}

#[test]
fn test_override_inference() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_override;
        import 'dart:core';

        class Base<T> {
            T get val => throw 0;
            void setVal(T x) {}
            T compute(T input) => input;
        }

        class Child extends Base<int> {
            get val => 42;
            void setVal(x) {}
            compute(input) => input;
        }
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the program with an override");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "there must be no resolution errors: {:?}", diags);

    let child_sym = interner.intern("Child");
    let (child_cid, _) = prog
        .classes
        .iter()
        .enumerate()
        .find(|(_, c)| c.name == child_sym)
        .expect("encontra classe Child");

    let child_elem = &prog.classes[child_cid];

    // 1. Getter 'val' without an annotation in Child inherits return 'int' from Base<int>
    let val_sym = interner.intern("val");
    let val_fn_id = child_elem
        .instance_members
        .get(&val_sym)
        .expect("finds member val in Child");
    let val_fn_data = &outline.functions[val_fn_id.0 as usize];
    assert_eq!(
        val_fn_data.return_type, core.int,
        "the return of 'get val' in Child must be inferred as int"
    );

    // 2. Method 'setVal(x)' with an omitted parameter inherits type 'int' from Base<int>
    let set_val_sym = interner.intern("setVal");
    let set_val_fn_id = child_elem
        .instance_members
        .get(&set_val_sym)
        .expect("finds member setVal in Child");
    let set_val_fn_data = &outline.functions[set_val_fn_id.0 as usize];
    assert_eq!(set_val_fn_data.parameters.len(), 1);
    assert_eq!(
        set_val_fn_data.parameters[0].ty, core.int,
        "parameter 'x' in setVal(x) must be inferred as int"
    );

    // 3. Method 'compute(input)' without return or parameter type inherits both as 'int'
    let compute_sym = interner.intern("compute");
    let compute_fn_id = child_elem
        .instance_members
        .get(&compute_sym)
        .expect("finds member compute in Child");
    let compute_fn_data = &outline.functions[compute_fn_id.0 as usize];
    assert_eq!(
        compute_fn_data.return_type, core.int,
        "the return of 'compute' in Child must be inferred as int"
    );
    assert_eq!(compute_fn_data.parameters.len(), 1);
    assert_eq!(
        compute_fn_data.parameters[0].ty, core.int,
        "parameter 'input' in compute must be inferred as int"
    );
}

#[test]
fn test_mixin_application_instantiated_supertypes() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_mixin;
        import 'dart:core';

        abstract class I1<T> {}
        abstract class I2<U> {}

        class B<T> implements I1<T> {}
        mixin M<U> implements I2<U> {}

        class C extends B<int> with M<String> {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the program with a mixin");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "there must be no resolution errors: {:?}", diags);

    let mut find_cls = |name: &str| -> ClassId {
        let sym = interner.intern(name);
        ClassId(
            prog.classes
                .iter()
                .position(|c| c.name == sym)
                .unwrap_or_else(|| panic!("class {name} must exist")) as u32,
        )
    };

    let c_cls = find_cls("C");
    let b_cls = find_cls("B");
    let m_cls = find_cls("M");
    let i1_cls = find_cls("I1");
    let i2_cls = find_cls("I2");

    let c_ty = table.intern(Type::Interface {
        class: c_cls,
        args: Box::new([]),
        nullable: false,
    });

    // C tem B<int>
    let b_super = outline
        .hierarchy
        .supertype_of(c_ty, b_cls, &mut table, &core)
        .expect("C must inherit B");
    match table.get(b_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, b_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int);
        }
        other => panic!("expected Interface B<int>, got {:?}", other),
    }

    // C tem M<String>
    let m_super = outline
        .hierarchy
        .supertype_of(c_ty, m_cls, &mut table, &core)
        .expect("C must include mixin M");
    match table.get(m_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, m_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.string);
        }
        other => panic!("expected Interface M<String>, got {:?}", other),
    }

    // C transitively inherits I1<int> through B<int>
    let i1_super = outline
        .hierarchy
        .supertype_of(c_ty, i1_cls, &mut table, &core)
        .expect("C must transitively inherit I1");
    match table.get(i1_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, i1_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int);
        }
        other => panic!("expected Interface I1<int>, got {:?}", other),
    }

    // C transitively inherits I2<String> through M<String>
    let i2_super = outline
        .hierarchy
        .supertype_of(c_ty, i2_cls, &mut table, &core)
        .expect("C must transitively inherit I2");
    match table.get(i2_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, i2_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.string);
        }
        other => panic!("expected Interface I2<String>, got {:?}", other),
    }
}

#[test]
fn test_extension_type_erasure_and_subtyping() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_ext;
        import 'dart:core';

        extension type Id(int rep) {}
        extension type SubId(int rep) implements int {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the extension type");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "there must be no resolution errors: {:?}", diags);

    let mut find_cls = |name: &str| -> ClassId {
        let sym = interner.intern(name);
        ClassId(
            prog.classes
                .iter()
                .position(|c| c.name == sym)
                .unwrap_or_else(|| panic!("class {name} must exist")) as u32,
        )
    };

    let id_cls = find_cls("Id");
    let sub_id_cls = find_cls("SubId");

    let id_ty = table.intern(Type::ExtensionType {
        decl: id_cls,
        args: Box::new([]),
        nullable: false,
    });
    let sub_id_ty = table.intern(Type::ExtensionType {
        decl: sub_id_cls,
        args: Box::new([]),
        nullable: false,
    });

    // 1. Extension type erasure function through the representation
    let rep_fn = |cls: ClassId, args: &[TypeId], tbl: &mut TypeTable| -> Option<TypeId> {
        let class_elem = &prog.classes[cls.0 as usize];
        if class_elem.kind == ClassKind::ExtensionType {
            let rep_var = class_elem.representation?;
            let uninst_rep = outline.variables[rep_var.0 as usize].declared_type?;
            let formals = &outline.classes[cls.0 as usize].type_params;
            let mut subst = HashMap::new();
            for (&f, &a) in formals.iter().zip(args.iter()) {
                subst.insert(f, a);
            }
            Some(substitute(uninst_rep, &subst, tbl))
        } else {
            None
        }
    };

    let erased_id = erase_extension_type(id_ty, &mut table, &rep_fn);
    assert_eq!(erased_id, core.int, "erase_extension_type(Id) must return int");

    let erased_sub = erase_extension_type(sub_id_ty, &mut table, &rep_fn);
    assert_eq!(erased_sub, core.int, "erase_extension_type(SubId) must return int");

    // 2. Subtipagem
    let mut env = dartforge_types::subtyping::SubtypeEnv::new(
        &mut table,
        &outline.hierarchy,
        &core,
    );

    // Id does not implement int: Id <: int is FALSE
    assert!(
        !is_subtype(id_ty, core.int, &mut env),
        "Id without implements int must not be a subtype of int"
    );
    // int <: Id is FALSE
    assert!(
        !is_subtype(core.int, id_ty, &mut env),
        "int must not be a subtype of Id"
    );

    // SubId implements int: SubId <: int is TRUE
    assert!(
        is_subtype(sub_id_ty, core.int, &mut env),
        "SubId with implements int must be a subtype of int"
    );
    // int <: SubId continua FALSO
    assert!(
        !is_subtype(core.int, sub_id_ty, &mut env),
        "int must not be a subtype of SubId"
    );

    // SubId <: Object is TRUE (via int <: Object)
    assert!(
        is_subtype(sub_id_ty, core.object, &mut env),
        "SubId must be a subtype of Object"
    );
}

#[test]
fn test_accumulated_diagnostics_unknown_type() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let the_proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&the_proj_dir).unwrap();
    let main_dart = the_proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        library test_err;
        import 'dart:core';

        class GoodAndBad {
            TipoInexistente badField;
            int goodField = 10;
        }
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loads the program with a type error");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    // Runs resolution without panicking, capturing accumulated diagnostics
    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);

    assert!(
        !diags.is_empty(),
        "must accumulate a diagnostic for the nonexistent type"
    );
    assert!(
        diags.iter().any(|d| d.code.is_some_and(|c| c.info().name == "undefined_class")
            && d.message.starts_with("Undefined class '")),
        "the error must be the analyzer's undefined_class: {:?}",
        diags
    );

    // The field with an error becomes dynamic
    let bad_sym = interner.intern("badField");
    let good_sym = interner.intern("goodField");

    let mut found_bad = false;
    let mut found_good = false;

    for (idx, v) in prog.variables.iter().enumerate() {
        if v.name == bad_sym {
            assert_eq!(
                outline.variables[idx].declared_type,
                Some(core.dynamic_),
                "a field with an invalid type must fall back to Type::Dynamic"
            );
            found_bad = true;
        } else if v.name == good_sym {
            assert_eq!(
                outline.variables[idx].declared_type,
                Some(core.int),
                "a field with a valid type must be resolved normally as int"
            );
            found_good = true;
        }
    }

    assert!(found_bad && found_good, "both fields must have been inspected");
}

/// `class C extends C` (`recursive_interface_inheritance`): the outline
/// finishes (the superclass chain with a cycle is walked once).
#[test]
fn superclass_of_itself_terminates() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    fs::write(&main_dart, "class C extends C {\n  var foo = 0;\n  bar();\n}\n").unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let _ = resolve_outline(&prog, &interner, &mut table, &core);
}

/// `invalid_override` (`types::sobrescritas`), in the forms of the analyzer's
/// corpus (3.6.2 oracle): return, parameter, setter, `covariant` and the check
/// of covariant parameters against all supertypes.
#[test]
fn corpus_invalid_overrides() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "class A {\n  int g() => 0;\n  void m(int a) {}\n  set s(num v) {}\n  void c(num a) {}\n}\nclass B extends A {\n  String g() => '';\n  void m(String a) {}\n  set s(int v) {}\n  void c(dynamic a) {}\n}\nclass C extends B {\n  void c(covariant String a) {}\n}\nclass D extends A {\n  int g() => 1;\n  void m(covariant int a) {}\n}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let classes: Vec<ClassId> =
        (0..prog.classes.len()).map(|i| ClassId(i as u32)).filter(|c| prog.class(*c).library == lib).collect();
    let mut v: Vec<(String, String)> =
        dartforge_types::sobrescritas::sobrescritas_invalidas(&prog, &interner, &mut table, &core, &outline, &classes)
            .into_iter()
            .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
            .collect();
    v.sort();
    assert_eq!(
        v,
        vec![
            ("c".to_string(), "'C.c' ('void Function(String)') isn't a valid override of 'A.c' ('void Function(num)').".to_string()),
            ("g".to_string(), "'B.g' ('String Function()') isn't a valid override of 'A.g' ('int Function()').".to_string()),
            ("m".to_string(), "'B.m' ('void Function(String)') isn't a valid override of 'A.m' ('void Function(int)').".to_string()),
            ("s".to_string(), "The setter 'B.s' ('void Function(int)') isn't a valid override of 'A.s' ('void Function(num)').".to_string()),
        ]
    );
}

/// `types::variancia`: variance positions and supertypes (checked against
/// `dart analyze` 3.6.2, which keeps the written variance even without the
/// experiment).
#[test]
fn type_parameter_variance() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "class A<in T> {\n  T get g => throw 0;\n  void m(T x) {}\n  void c(covariant T x) {}\n}\nclass B<out T> {\n  final T f;\n  B(this.f);\n}\nclass K<X> {}\nclass C<X> extends K<void Function(X)> {}\nclass D<out X> extends K<void Function(X)> {}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let v: Vec<(String, String)> = dartforge_types::variancia::variancia(&prog, &interner, &table, &outline, lib)
        .into_iter()
        .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
        .collect();
    assert_eq!(
        v,
        vec![
            ("T".to_string(), "The 'in' type parameter 'T' can't be used in an 'out' position.".to_string()),
            ("X".to_string(), "'X' can't be used contravariantly or invariantly in 'K<void Function(X)>'.".to_string()),
            (
                "X".to_string(),
                "'X' is an 'out' type parameter and can't be used in an 'in' position in 'K<void Function(X)>'.".to_string()
            ),
        ]
    );
}

/// `getter_not_subtype_setter_types` (`types::sobrescritas::getters_e_setters`):
/// top-level, static and the interface (the inherited getter carries the class
/// name).
#[test]
fn getter_and_setter_with_incompatible_types() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "int get a => 0;\nset a(String v) {}\nget b => 0;\nset b(int v) {}\nclass A {\n  static num get s => 0;\n  static set s(int v) {}\n  int get foo => 0;\n}\nclass B extends A {\n  set foo(String v) {}\n}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let classes: Vec<ClassId> =
        (0..prog.classes.len()).map(|i| ClassId(i as u32)).filter(|c| prog.class(*c).library == lib).collect();
    let mut v: Vec<(String, String)> =
        dartforge_types::sobrescritas::getters_e_setters(&prog, &interner, &mut table, &core, &outline, lib, &classes)
            .into_iter()
            .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
            .collect();
    v.sort();
    assert_eq!(
        v,
        vec![
            ("a".to_string(), "The return type of getter 'a' is 'int' which isn't a subtype of the type 'String' of its setter 'a'.".to_string()),
            ("b".to_string(), "The return type of getter 'b' is 'dynamic' which isn't a subtype of the type 'int' of its setter 'b'.".to_string()),
            ("foo".to_string(), "The return type of getter 'A.foo' is 'int' which isn't a subtype of the type 'String' of its setter 'foo'.".to_string()),
            ("s".to_string(), "The return type of getter 's' is 'num' which isn't a subtype of the type 'int' of its setter 's'.".to_string()),
        ]
    );
}

/// `types::sobrescritas::membros_abstratos`: an abstract member in a concrete
/// class, inherited ones without an implementation (sorted, with `getter`),
/// the concrete superclass that already leaves them out, and `noSuchMethod`.
#[test]
fn abstract_members_in_concrete_class() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "abstract class I {\n  int get g;\n  void m();\n}\nclass A {\n  void f();\n}\nclass B implements I {}\nclass C extends B {}\nclass D implements I {\n  noSuchMethod(i) => null;\n}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let classes: Vec<ClassId> =
        (0..prog.classes.len()).map(|i| ClassId(i as u32)).filter(|c| prog.class(*c).library == lib).collect();
    let mut v: Vec<(String, String)> =
        dartforge_types::sobrescritas::membros_abstratos(&prog, &interner, &mut table, &core, &outline, &classes)
            .into_iter()
            .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
            .collect();
    v.sort();
    assert_eq!(
        v,
        vec![
            ("B".to_string(), "Missing concrete implementations of 'I.m' and 'getter I.g'.".to_string()),
            ("void f();".to_string(), "'f' must have a method body because 'A' isn't abstract.".to_string()),
        ]
    );
}

/// `membros_em_conflito` and `default_values` of `types::sobrescritas`
/// (analyzer corpus, 3.6.2 oracle).
#[test]
fn conflicting_members_and_default_values() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "class A {\n  int foo = 0;\n  void bar() {}\n}\nclass B extends A {\n  void foo() {}\n  int get bar => 0;\n}\nvoid f([int a]) {}\nvoid g({String? b, int c = 0}) {}\nclass C {\n  C({this.x});\n  int x;\n}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let mut v: Vec<(String, String)> = dartforge_types::sobrescritas::membros_em_conflito(&prog, &interner, &mut table, &core, &outline, lib)
        .into_iter()
        .chain(dartforge_types::sobrescritas::default_values(&prog, &interner, &mut table, &outline, lib))
        .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
        .collect();
    v.sort();
    assert_eq!(
        v,
        vec![
            ("a".to_string(), "The parameter 'a' can't have a value of 'null' because of its type, but the implicit default value is 'null'.".to_string()),
            ("bar".to_string(), "Class 'B' can't define field 'bar' and have method 'A.bar' with the same name.".to_string()),
            ("foo".to_string(), "Class 'B' can't define method 'foo' and have field 'A.foo' with the same name.".to_string()),
            ("x".to_string(), "The parameter 'x' can't have a value of 'null' because of its type, but the implicit default value is 'null'.".to_string()),
        ]
    );
}

/// `membros_de_enum`: concrete `index`/`hashCode` and `values` in a class
/// that implements `Enum` (`illegal_*` corpus, 3.6.2 oracle).
#[test]
fn members_forbidden_in_enum_implementers() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    fs::write(
        tmp.path().join("lib/core/core.dart"),
        "library dart.core;\nclass Object { int get hashCode => 0; }\nclass int {}\nclass num {}\nclass double {}\nclass String {}\nclass bool {}\nclass Null {}\nclass Function {}\nclass Record {}\nclass Iterable<E> {}\nclass List<E> implements Iterable<E> {}\nclass Map<K, V> {}\nabstract class Enum { int get index; }\n",
    )
    .unwrap();
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let source = "abstract class A implements Enum {\n  int get index => 0;\n  int values = 0;\n}\n";
    fs::write(&main_dart, source).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let classes: Vec<ClassId> =
        (0..prog.classes.len()).map(|i| ClassId(i as u32)).filter(|c| prog.class(*c).library == lib).collect();
    let mut v: Vec<(String, String)> =
        dartforge_types::sobrescritas::membros_de_enum(&prog, &interner, &mut table, &core, &outline, &classes)
            .into_iter()
            .map(|(_, d)| (source[d.span.start..d.span.end].to_string(), d.message))
            .collect();
    v.sort();
    assert_eq!(
        v,
        vec![
            ("index".to_string(), "A concrete instance member named 'index' can't be declared in a class that implements 'Enum'.".to_string()),
            ("values".to_string(), "An instance member named 'values' can't be declared in a class that implements 'Enum'.".to_string()),
        ]
    );
}
