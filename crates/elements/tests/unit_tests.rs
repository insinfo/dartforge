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

    SdkLayout::load(&lib_dir, "dartdevc").expect("loads a fake sdk layout")
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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");

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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let p_sym = interner.intern("p");
    let b_sym = interner.intern("B");

    // B must not be in the root scope without a prefix
    assert!(!my_lib.scope.contains_key(&b_sym));

    // B must be in prefix p
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

    // Exports with show followed by hide
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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");
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

    let prog = load(&a_dart, &sdk, None, &mut interner).expect("must resolve cyclic reexports");

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

    // Both reexport each other and converge at the fixpoint
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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let foo_sym = interner.intern("Foo");
    let binding = my_lib.scope.get(&foo_sym).expect("Foo must be in scope");

    // A local declaration shadows the import: not ambiguous!
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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");
    let my_lib = prog
        .libraries
        .iter()
        .find(|l| l.uri.contains("main.dart"))
        .unwrap();

    let conflict_sym = interner.intern("Conflict");
    let binding = my_lib
        .scope
        .get(&conflict_sym)
        .expect("Conflict must be in scope");

    // Must be marked as ambiguous
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

    let prog = load(&main_dart, &sdk, None, &mut interner).expect("loading failed");

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
    assert!(res.is_err(), "must fail because of an inheritance cycle");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|e| e.message.contains("inheritance cycle")),
        "expected an inheritance cycle diagnostic, got: {:?}",
        errs
    );
}

/// `build_runner` generated files outside `lib/` (`test/`, `web/`): a
/// relative import of a `.template.dart` that only exists in
/// `.dart_tool/build/generated/<package>/test/` resolves to it, with the URI
/// of the original place, and the generated file's relative imports go back
/// to `test/`. `dart analyze` 3.6.2 gives "No issues found!" on this project
/// (`v01/min/gentest`).
#[test]
fn build_runner_output_outside_lib() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(proj.join("test")).unwrap();
    fs::create_dir_all(proj.join("lib")).unwrap();
    fs::create_dir_all(proj.join(".dart_tool/build/generated/gentest/test")).unwrap();
    fs::write(proj.join("pubspec.yaml"), "name: gentest\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    fs::write(
        proj.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"gentest","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    )
    .unwrap();
    fs::write(proj.join("test/x_test.dart"), "import 'x_test.template.dart' as ng;\nint f() => ng.valor;\nconst base = 1;\n").unwrap();
    fs::write(
        proj.join(".dart_tool/build/generated/gentest/test/x_test.template.dart"),
        "import 'x_test.dart' as orig;\nconst valor = orig.base + 1;\n",
    )
    .unwrap();
    let entrada = proj.join("test/x_test.dart");
    let config = proj.join(".dart_tool/package_config.json");
    let prog = load(&entrada, &sdk, Some(&config), &mut interner).expect("loading failed");
    let gerado = prog.libraries.iter().find(|l| l.uri.ends_with("test/x_test.template.dart")).expect("generated library");
    assert!(gerado.uri.starts_with("file://"), "{}", gerado.uri);
    let unidade = &prog.units[gerado.units[0].0 as usize];
    let caminho = unidade.path.as_ref().unwrap().to_string_lossy().replace('\\', "/");
    assert!(caminho.contains(".dart_tool/build/generated/gentest/test/x_test.template.dart"), "{caminho}");
    let value = interner.intern("valor");
    assert!(gerado.declared.contains_key(&value));
    // The generated file's relative import (`x_test.dart`) is the `test/`
    // library, not a nonexistent file in the generated folder.
    let orig = interner.intern("orig");
    let target = gerado.imports.iter().find(|i| i.prefix == Some(orig)).expect("import from the generated file");
    assert!(prog.library(target.library).uri.ends_with("/proj/test/x_test.dart"), "{}", prog.library(target.library).uri);
}

/// `parts-with-imports`: a part's imports apply in it and in its parts and
/// hide those of the file that included it (bare name and prefix); the
/// library's declarations hide every import. Same result as the 3.13.4 CFE
/// (`corpus/macros/406_partes_imports_313`).
#[test]
fn import_scope_per_unit() {
    let tmp = tempdir().unwrap();
    let mut sdk = empty_sdk(tmp.path());
    sdk.experimentos.push(dartforge_frontend::features::Feature::EnhancedParts);
    let mut interner = Interner::new();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join("a.dart"), "int nome() => 1;\n").unwrap();
    fs::write(proj.join("b.dart"), "int nome() => 2;\nint local() => 2;\n").unwrap();
    fs::write(proj.join("c.dart"), "int nome() => 3;\n").unwrap();
    fs::write(proj.join("main.dart"), "import 'a.dart';\nimport 'a.dart' as p;\npart 'parte.dart';\nint local() => 0;\n").unwrap();
    fs::write(proj.join("parte.dart"), "part of 'main.dart';\nimport 'b.dart';\nimport 'b.dart' as p;\npart 'neta.dart';\npart 'outra.dart';\n").unwrap();
    fs::write(proj.join("neta.dart"), "part of 'parte.dart';\n").unwrap();
    fs::write(proj.join("outra.dart"), "part of 'parte.dart';\nimport 'c.dart';\n").unwrap();
    let prog = load(&proj.join("main.dart"), &sdk, None, &mut interner).expect("loading failed");
    let lib = prog.libraries.iter().position(|l| l.uri.ends_with("/main.dart")).map(|i| LibraryId(i as u32)).unwrap();
    let unidade = |suffix: &str| {
        *prog.library(lib).units.iter().find(|u| prog.unit(**u).uri.ends_with(suffix)).expect(suffix)
    };
    let de = |u: UnitId, nome: &str| -> String {
        let b = prog.lookup_na_unidade(u, interner.lookup(nome).unwrap()).expect(nome);
        assert!(!b.ambiguous, "{nome}");
        match b.getter {
            Some(Element::Function(f)) => prog.library(prog.function(f).library).uri.rsplit('/').next().unwrap().to_string(),
            other => format!("{other:?}"),
        }
    };
    let prefixed = |u: UnitId| -> String {
        let (p, n) = (interner.lookup("p").unwrap(), interner.lookup("nome").unwrap());
        let b = prog.lookup_prefixed_na_unidade(u, p, n).expect("p.nome");
        match b.getter {
            Some(Element::Function(f)) => prog.library(prog.function(f).library).uri.rsplit('/').next().unwrap().to_string(),
            other => format!("{other:?}"),
        }
    };
    let (m, pa, ne, either) = (unidade("/main.dart"), unidade("/parte.dart"), unidade("/neta.dart"), unidade("/outra.dart"));
    assert_eq!((de(m, "nome"), de(pa, "nome"), de(ne, "nome"), de(either, "nome")), ("a.dart".into(), "b.dart".into(), "b.dart".into(), "c.dart".into()));
    assert_eq!((prefixed(m), prefixed(pa), prefixed(ne), prefixed(either)), ("a.dart".into(), "b.dart".into(), "b.dart".into(), "b.dart".into()));
    // The library's declaration wins over the part's import.
    assert_eq!(de(pa, "local"), "main.dart");
    // Without imports in parts, nothing changes: the library scope applies to
    // all of them.
    let proj2 = tmp.path().join("proj2");
    fs::create_dir_all(&proj2).unwrap();
    fs::write(proj2.join("main.dart"), "import 'a.dart';\npart 'parte.dart';\n").unwrap();
    fs::write(proj2.join("a.dart"), "int nome() => 1;\n").unwrap();
    fs::write(proj2.join("parte.dart"), "part of 'main.dart';\n").unwrap();
    let prog2 = load(&proj2.join("main.dart"), &sdk, None, &mut interner).expect("loading failed");
    assert!(prog2.libraries.iter().all(|l| l.escopos_de_unidade.is_empty()));
}

/// A `part` of the file itself (the language's `part/self_test.dart`): the
/// load ends, with the unit only once.
#[test]
fn part_of_itself_does_not_reload() {
    let dir = tempdir().unwrap();
    let sdk = empty_sdk(dir.path());
    let main = dir.path().join("self_test.dart");
    fs::write(&main, "part 'self_test.dart';\nmain() {}\n").unwrap();
    let mut names_ = Interner::new();
    let (p, _) = dartforge_elements::load::load_lenient(&main, &sdk, None, &mut names_);
    let lib = p.library(p.entry.unwrap());
    assert_eq!(lib.units.len(), 1);
}

/// `part 'package:p/x.dart'` (`diff_match_patch`,
/// docs/NATIVO-PROJETOS-REAIS.md C5): the part's absolute URI goes to the
/// `package_config`, like an `import`'s; before, it was joined to the
/// library's directory (`lib/src/package:p/…`).
#[test]
fn part_by_package_uri() {
    let tmp = tempdir().unwrap();
    let sdk = empty_sdk(tmp.path());
    let mut interner = Interner::new();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(proj.join("lib/src/sub")).unwrap();
    fs::create_dir_all(proj.join(".dart_tool")).unwrap();
    fs::create_dir_all(proj.join("bin")).unwrap();
    fs::write(proj.join("pubspec.yaml"), "name: p\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    fs::write(
        proj.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    )
    .unwrap();
    fs::write(proj.join("lib/src/a.dart"), "library a;\npart 'package:p/src/sub/b.dart';\nint f() => g();\n").unwrap();
    fs::write(proj.join("lib/src/sub/b.dart"), "part of a;\nint g() => 1;\n").unwrap();
    fs::write(proj.join("bin/main.dart"), "import 'package:p/src/a.dart';\nvoid main() { f(); }\n").unwrap();
    let config = proj.join(".dart_tool/package_config.json");
    let prog = load(&proj.join("bin/main.dart"), &sdk, Some(&config), &mut interner).expect("loading failed");
    let a = prog.libraries.iter().find(|l| l.uri == "package:p/src/a.dart").expect("library a");
    assert_eq!(a.units.len(), 2, "the part entered the library");
    let g = interner.intern("g");
    assert!(a.declared.contains_key(&g));
}
