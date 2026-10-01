//! Testes unitários sem SDK para `dartforge-types`.
//!
//! Cobre os requisitos de testes unitários rápidos (<100ms):
//! 1. Typedef genérico com substituição (`typedef F<T> = T Function(T);`) expandido para `int Function(int)`
//! 2. Inferência de override: getter, setter e método herdam tipos da superclasse genérica (`Base<int>`)
//! 3. Herança com mixin application: `class C extends B with M` recebe a união das interfaces instanciadas
//! 4. Extension type: `extension type E(int rep)` apaga para `int` e não subtipa `int` a menos que use `implements int`
//! 5. Diagnósticos acumulados: erro de tipo inexistente é gravado e resolvido como `Type::Dynamic` sem pânico

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

    SdkLayout::load(&lib_dir, "dartdevc").expect("carrega layout sdk simulado")
}

#[test]
fn test_generic_typedef_expansion() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega programa com typedef");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "não deve haver erros de resolução: {:?}", diags);

    // Encontra o campo Target.cb
    let cb_sym = interner.intern("cb");
    let (cb_idx, _) = prog
        .variables
        .iter()
        .enumerate()
        .find(|(_, v)| v.name == cb_sym)
        .expect("encontra variável cb");

    let cb_ty = outline.variables[cb_idx]
        .declared_type
        .expect("tipo declarado de cb deve estar presente");

    // Verifica que é Type::Function com retorno int e posicional int, marcado como nullable
    match table.get(cb_ty) {
        Type::Function {
            ret,
            positional,
            nullable,
            ..
        } => {
            assert!(*nullable, "cb deve ser nullable (F<int>?)");
            assert_eq!(*ret, core.int, "retorno expandido do typedef deve ser int");
            assert_eq!(positional.len(), 1, "deve ter 1 parâmetro posicional");
            assert_eq!(positional[0], core.int, "parâmetro deve ser int");
        }
        other => panic!("esperava Type::Function para F<int>?, obteve {:?}", other),
    }

    // Testa também expansão direta via tabela de typedefs
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
        other => panic!("esperava função expandida, obteve {:?}", other),
    }
}

#[test]
fn test_override_inference() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega programa com override");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "não deve haver erros de resolução: {:?}", diags);

    let child_sym = interner.intern("Child");
    let (child_cid, _) = prog
        .classes
        .iter()
        .enumerate()
        .find(|(_, c)| c.name == child_sym)
        .expect("encontra classe Child");

    let child_elem = &prog.classes[child_cid];

    // 1. Getter 'val' sem anotação em Child herda retorno 'int' de Base<int>
    let val_sym = interner.intern("val");
    let val_fn_id = child_elem
        .instance_members
        .get(&val_sym)
        .expect("encontra membro val em Child");
    let val_fn_data = &outline.functions[val_fn_id.0 as usize];
    assert_eq!(
        val_fn_data.return_type, core.int,
        "retorno de 'get val' em Child deve ser inferido como int"
    );

    // 2. Método 'setVal(x)' com parâmetro omitido herda tipo 'int' de Base<int>
    let set_val_sym = interner.intern("setVal");
    let set_val_fn_id = child_elem
        .instance_members
        .get(&set_val_sym)
        .expect("encontra membro setVal em Child");
    let set_val_fn_data = &outline.functions[set_val_fn_id.0 as usize];
    assert_eq!(set_val_fn_data.parameters.len(), 1);
    assert_eq!(
        set_val_fn_data.parameters[0].ty, core.int,
        "parâmetro 'x' em setVal(x) deve ser inferido como int"
    );

    // 3. Método 'compute(input)' sem retorno nem tipo de parâmetro herda ambos como 'int'
    let compute_sym = interner.intern("compute");
    let compute_fn_id = child_elem
        .instance_members
        .get(&compute_sym)
        .expect("encontra membro compute em Child");
    let compute_fn_data = &outline.functions[compute_fn_id.0 as usize];
    assert_eq!(
        compute_fn_data.return_type, core.int,
        "retorno de 'compute' em Child deve ser inferido como int"
    );
    assert_eq!(compute_fn_data.parameters.len(), 1);
    assert_eq!(
        compute_fn_data.parameters[0].ty, core.int,
        "parâmetro 'input' em compute deve ser inferido como int"
    );
}

#[test]
fn test_mixin_application_instantiated_supertypes() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega programa com mixin");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "não deve haver erros de resolução: {:?}", diags);

    let mut find_cls = |name: &str| -> ClassId {
        let sym = interner.intern(name);
        ClassId(
            prog.classes
                .iter()
                .position(|c| c.name == sym)
                .unwrap_or_else(|| panic!("classe {name} deve existir")) as u32,
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
        .expect("C deve herdar B");
    match table.get(b_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, b_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int);
        }
        other => panic!("esperava Interface B<int>, obteve {:?}", other),
    }

    // C tem M<String>
    let m_super = outline
        .hierarchy
        .supertype_of(c_ty, m_cls, &mut table, &core)
        .expect("C deve incluir mixin M");
    match table.get(m_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, m_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.string);
        }
        other => panic!("esperava Interface M<String>, obteve {:?}", other),
    }

    // C herda transitivamente I1<int> através de B<int>
    let i1_super = outline
        .hierarchy
        .supertype_of(c_ty, i1_cls, &mut table, &core)
        .expect("C deve herdar transitivamente I1");
    match table.get(i1_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, i1_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int);
        }
        other => panic!("esperava Interface I1<int>, obteve {:?}", other),
    }

    // C herda transitivamente I2<String> através de M<String>
    let i2_super = outline
        .hierarchy
        .supertype_of(c_ty, i2_cls, &mut table, &core)
        .expect("C deve herdar transitivamente I2");
    match table.get(i2_super) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, i2_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.string);
        }
        other => panic!("esperava Interface I2<String>, obteve {:?}", other),
    }
}

#[test]
fn test_extension_type_erasure_and_subtyping() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega extension type");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);
    assert!(diags.is_empty(), "não deve haver erros de resolução: {:?}", diags);

    let mut find_cls = |name: &str| -> ClassId {
        let sym = interner.intern(name);
        ClassId(
            prog.classes
                .iter()
                .position(|c| c.name == sym)
                .unwrap_or_else(|| panic!("classe {name} deve existir")) as u32,
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

    // 1. Função de apagamento de tipo de extensão via representação
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
    assert_eq!(erased_id, core.int, "erase_extension_type(Id) deve retornar int");

    let erased_sub = erase_extension_type(sub_id_ty, &mut table, &rep_fn);
    assert_eq!(erased_sub, core.int, "erase_extension_type(SubId) deve retornar int");

    // 2. Subtipagem
    let mut env = dartforge_types::subtyping::SubtypeEnv::new(
        &mut table,
        &outline.hierarchy,
        &core,
    );

    // Id não implementa int: Id <: int é FALSO
    assert!(
        !is_subtype(id_ty, core.int, &mut env),
        "Id sem implements int não deve ser subtipo de int"
    );
    // int <: Id é FALSO
    assert!(
        !is_subtype(core.int, id_ty, &mut env),
        "int não deve ser subtipo de Id"
    );

    // SubId implementa int: SubId <: int é VERDADEIRO
    assert!(
        is_subtype(sub_id_ty, core.int, &mut env),
        "SubId com implements int deve ser subtipo de int"
    );
    // int <: SubId continua FALSO
    assert!(
        !is_subtype(core.int, sub_id_ty, &mut env),
        "int não deve ser subtipo de SubId"
    );

    // SubId <: Object é VERDADEIRO (via int <: Object)
    assert!(
        is_subtype(sub_id_ty, core.object, &mut env),
        "SubId deve ser subtipo de Object"
    );
}

#[test]
fn test_accumulated_diagnostics_unknown_type() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();
    let main_dart = proj_dir.join("main.dart");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carrega programa com erro de tipo");
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);

    // Executa resolução sem pânico, capturando diagnósticos acumulados
    let (outline, diags) = resolve_outline(&prog, &interner, &mut table, &core);

    assert!(
        !diags.is_empty(),
        "deve acumular diagnóstico para o tipo inexistente"
    );
    assert!(
        diags.iter().any(|d| d.code.is_some_and(|c| c.info().nome == "undefined_class")
            && d.message.starts_with("Undefined class '")),
        "o erro deve ser o undefined_class do analyzer: {:?}",
        diags
    );

    // O campo com erro vira dynamic
    let bad_sym = interner.intern("badField");
    let good_sym = interner.intern("goodField");

    let mut found_bad = false;
    let mut found_good = false;

    for (idx, v) in prog.variables.iter().enumerate() {
        if v.name == bad_sym {
            assert_eq!(
                outline.variables[idx].declared_type,
                Some(core.dynamic_),
                "campo com tipo inválido deve ter fallback para Type::Dynamic"
            );
            found_bad = true;
        } else if v.name == good_sym {
            assert_eq!(
                outline.variables[idx].declared_type,
                Some(core.int),
                "campo com tipo válido deve ser resolvido normalmente como int"
            );
            found_good = true;
        }
    }

    assert!(found_bad && found_good, "ambos os campos devem ter sido inspecionados");
}

/// `class C extends C` (`recursive_interface_inheritance`): o outline termina
/// (a cadeia de superclasses com ciclo é percorrida uma vez).
#[test]
fn superclasse_de_si_mesma_termina() {
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

/// `invalid_override` (`types::sobrescritas`), nas formas do corpus do
/// analyzer (oráculo 3.6.2): retorno, parâmetro, setter, `covariant` e a
/// verificação dos parâmetros covariantes contra todos os supertipos.
#[test]
fn sobrescritas_invalidas_do_corpus() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "class A {\n  int g() => 0;\n  void m(int a) {}\n  set s(num v) {}\n  void c(num a) {}\n}\nclass B extends A {\n  String g() => '';\n  void m(String a) {}\n  set s(int v) {}\n  void c(dynamic a) {}\n}\nclass C extends B {\n  void c(covariant String a) {}\n}\nclass D extends A {\n  int g() => 1;\n  void m(covariant int a) {}\n}\n";
    fs::write(&main_dart, fonte).unwrap();
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
            .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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

/// `types::variancia`: posições de variância e supertipos (conferidos com o
/// `dart analyze` 3.6.2, que guarda a variância escrita mesmo sem o
/// experimento).
#[test]
fn variancia_dos_parametros_de_tipo() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "class A<in T> {\n  T get g => throw 0;\n  void m(T x) {}\n  void c(covariant T x) {}\n}\nclass B<out T> {\n  final T f;\n  B(this.f);\n}\nclass K<X> {}\nclass C<X> extends K<void Function(X)> {}\nclass D<out X> extends K<void Function(X)> {}\n";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let v: Vec<(String, String)> = dartforge_types::variancia::variancia(&prog, &interner, &table, &outline, lib)
        .into_iter()
        .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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
/// topo, estático e a interface (o getter herdado leva o nome da classe).
#[test]
fn getter_e_setter_de_tipos_incompativeis() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "int get a => 0;\nset a(String v) {}\nget b => 0;\nset b(int v) {}\nclass A {\n  static num get s => 0;\n  static set s(int v) {}\n  int get foo => 0;\n}\nclass B extends A {\n  set foo(String v) {}\n}\n";
    fs::write(&main_dart, fonte).unwrap();
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
            .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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

/// `types::sobrescritas::membros_abstratos`: membro abstrato em classe
/// concreta, herdados sem implementação (ordenados, com `getter`), a
/// superclasse concreta que já os deixa de fora, e o `noSuchMethod`.
#[test]
fn membros_abstratos_em_classe_concreta() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "abstract class I {\n  int get g;\n  void m();\n}\nclass A {\n  void f();\n}\nclass B implements I {}\nclass C extends B {}\nclass D implements I {\n  noSuchMethod(i) => null;\n}\n";
    fs::write(&main_dart, fonte).unwrap();
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
            .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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

/// `membros_em_conflito` e `valores_padrao` de `types::sobrescritas`
/// (corpus do analyzer, oráculo 3.6.2).
#[test]
fn membros_em_conflito_e_valores_padrao() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "class A {\n  int foo = 0;\n  void bar() {}\n}\nclass B extends A {\n  void foo() {}\n  int get bar => 0;\n}\nvoid f([int a]) {}\nvoid g({String? b, int c = 0}) {}\nclass C {\n  C({this.x});\n  int x;\n}\n";
    fs::write(&main_dart, fonte).unwrap();
    let (prog, _) = dartforge_elements::load::load_lenient(&main_dart, &sdk, None, &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let lib = prog.entry.unwrap();
    let mut v: Vec<(String, String)> = dartforge_types::sobrescritas::membros_em_conflito(&prog, &interner, &mut table, &core, &outline, lib)
        .into_iter()
        .chain(dartforge_types::sobrescritas::valores_padrao(&prog, &interner, &mut table, &outline, lib))
        .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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

/// `membros_de_enum`: `index`/`hashCode` concretos e `values` numa classe
/// que implementa `Enum` (corpus `illegal_*`, oráculo 3.6.2).
#[test]
fn membros_proibidos_em_quem_implementa_enum() {
    let tmp = tempdir().unwrap();
    let sdk = mock_sdk(tmp.path());
    fs::write(
        tmp.path().join("lib/core/core.dart"),
        "library dart.core;\nclass Object { int get hashCode => 0; }\nclass int {}\nclass num {}\nclass double {}\nclass String {}\nclass bool {}\nclass Null {}\nclass Function {}\nclass Record {}\nclass Iterable<E> {}\nclass List<E> implements Iterable<E> {}\nclass Map<K, V> {}\nabstract class Enum { int get index; }\n",
    )
    .unwrap();
    let mut interner = Interner::new();
    let main_dart = tmp.path().join("main.dart");
    let fonte = "abstract class A implements Enum {\n  int get index => 0;\n  int values = 0;\n}\n";
    fs::write(&main_dart, fonte).unwrap();
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
            .map(|(_, d)| (fonte[d.span.start..d.span.end].to_string(), d.message))
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
