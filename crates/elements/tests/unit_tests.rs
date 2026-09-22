use dartforge_elements::load::load;
use dartforge_elements::model::*;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn empty_sdk(dir: &Path) -> SdkLayout {
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
        "library dart.core; class Object {} class int extends Object {} class String extends Object {} class bool extends Object {}",
    )
    .unwrap();

    SdkLayout::load(&lib_dir, "dartdevc").expect("carrega layout sdk falso")
}

#[test]
fn test_part_and_part_of() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");
    let part_dart = proj_dir.join("foo.dart");

    fs::write(
        &main_dart,
        r#"
        library my_lib;
        part 'foo.dart';
        class MainClass {}
        "#,
    )
    .unwrap();

    fs::write(
        &part_dart,
        r#"
        part of my_lib;
        class PartClass {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");

    assert!(prog.libraries.len() >= 2); // dart:core e my_lib
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .expect("encontra my_lib");

    assert_eq!(my_lib.units.len(), 2);
    let u0 = &prog.units[my_lib.units[0].0 as usize];
    let u1 = &prog.units[my_lib.units[1].0 as usize];

    assert_eq!(u0.role, UnitRole::Library);
    assert_eq!(u1.role, UnitRole::Part);

    let main_sym = interner.intern("MainClass");
    let part_sym = interner.intern("PartClass");

    assert!(my_lib.declared.contains_key(&main_sym));
    assert!(my_lib.declared.contains_key(&part_sym));
}

#[test]
fn test_prefixes() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");
    let lib_b_dart = proj_dir.join("lib_b.dart");

    fs::write(
        &main_dart,
        r#"
        import 'lib_b.dart' as p;
        class A {}
        "#,
    )
    .unwrap();

    fs::write(
        &lib_b_dart,
        r#"
        class B {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let p_sym = interner.intern("p");
    let b_sym = interner.intern("B");

    // B não deve estar no escopo raiz sem prefixo
    assert!(!my_lib.scope.contains_key(&b_sym));

    // B deve estar no prefixo p
    assert!(my_lib.prefixes.contains_key(&p_sym));
    let pref_ns = &my_lib.prefixes[&p_sym];
    assert!(pref_ns.contains_key(&b_sym));
}

#[test]
fn test_show_hide_sequence() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");
    let lib_b_dart = proj_dir.join("lib_b.dart");

    // Exporta com show seguido de hide
    fs::write(
        &main_dart,
        r#"
        export 'lib_b.dart' show a, b hide b;
        "#,
    )
    .unwrap();

    fs::write(
        &lib_b_dart,
        r#"
        class a {}
        class b {}
        class c {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let a_sym = interner.intern("a");
    let b_sym = interner.intern("b");
    let c_sym = interner.intern("c");

    assert!(my_lib.exported.contains_key(&a_sym));
    assert!(!my_lib.exported.contains_key(&b_sym));
    assert!(!my_lib.exported.contains_key(&c_sym));
}

#[test]
fn test_cyclic_reexports() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let a_dart = proj_dir.join("a.dart");
    let b_dart = proj_dir.join("b.dart");

    fs::write(
        &a_dart,
        r#"
        export 'b.dart';
        class FromA {}
        "#,
    )
    .unwrap();

    fs::write(
        &b_dart,
        r#"
        export 'a.dart';
        class FromB {}
        "#,
    )
    .unwrap();

    let prog = load(&a_dart, &sdk, None, &mut interner).expect("deve resolver reexports cíclicos");

    let lib_a = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("a.dart"))
        .unwrap();
    let lib_b = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("b.dart"))
        .unwrap();

    let sym_a = interner.intern("FromA");
    let sym_b = interner.intern("FromB");

    // Ambos reexportam mutuamente e convergem no ponto fixo
    assert!(lib_a.exported.contains_key(&sym_a));
    assert!(lib_a.exported.contains_key(&sym_b));

    assert!(lib_b.exported.contains_key(&sym_a));
    assert!(lib_b.exported.contains_key(&sym_b));
}

#[test]
fn test_local_shadowing() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");
    let helper_dart = proj_dir.join("helper.dart");

    fs::write(
        &main_dart,
        r#"
        import 'helper.dart';
        class Foo {}
        "#,
    )
    .unwrap();

    fs::write(
        &helper_dart,
        r#"
        class Foo {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let foo_sym = interner.intern("Foo");
    let binding = my_lib.scope.get(&foo_sym).expect("Foo deve estar no escopo");

    // Declaração local sombreia o import: não é ambíguo!
    assert!(!binding.ambiguous);
    let local_foo = my_lib.declared[&foo_sym].getter;
    assert_eq!(binding.getter, local_foo);
}

#[test]
fn test_ambiguous_imports() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");
    let lib_1 = proj_dir.join("lib1.dart");
    let lib_2 = proj_dir.join("lib2.dart");

    fs::write(
        &main_dart,
        r#"
        import 'lib1.dart';
        import 'lib2.dart';
        "#,
    )
    .unwrap();

    fs::write(
        &lib_1,
        r#"
        class Conflict {}
        "#,
    )
    .unwrap();

    fs::write(
        &lib_2,
        r#"
        class Conflict {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let conflict_sym = interner.intern("Conflict");
    let binding = my_lib
        .scope
        .get(&conflict_sym)
        .expect("Conflict deve estar no escopo");

    // Deve estar marcado como ambíguo
    assert!(binding.ambiguous);
}

#[test]
fn test_supertype_resolution() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        class Base {}
        mixin Mix {}
        class Iface {}
        class Sub extends Base with Mix implements Iface {}
        "#,
    )
    .unwrap();

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("carregamento falhou");

    let sub_sym = interner.intern("Sub");
    let base_sym = interner.intern("Base");
    let mix_sym = interner.intern("Mix");
    let iface_sym = interner.intern("Iface");

    let sub_class = prog.classes.iter().find(|c| c.name == sub_sym).unwrap();

    let base_id = ClassId(prog.classes.iter().position(|c| c.name == base_sym).unwrap() as u32);
    let mix_id = ClassId(prog.classes.iter().position(|c| c.name == mix_sym).unwrap() as u32);
    let iface_id = ClassId(prog.classes.iter().position(|c| c.name == iface_sym).unwrap() as u32);

    assert_eq!(sub_class.supertype_class, Some(base_id));
    assert_eq!(sub_class.mixin_classes, vec![mix_id]);
    assert_eq!(sub_class.interface_classes, vec![iface_id]);
}

#[test]
fn test_inheritance_cycle_detection() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();

    let proj_dir = tmp.path().join("proj");
    fs::create_dir_all(&proj_dir).unwrap();

    let main_dart = proj_dir.join("main.dart");

    fs::write(
        &main_dart,
        r#"
        class A extends B {}
        class B extends A {}
        "#,
    )
    .unwrap();

    let res = load(&main_dart, &sdk, None, &mut interner);
    assert!(res.is_err(), "deve falhar por ciclo de herança");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|e| e.message.contains("ciclo de herança")),
        "esperava diagnóstico de ciclo de herança, obteve: {:?}",
        errs
    );
}
