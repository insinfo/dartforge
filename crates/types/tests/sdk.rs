//! Integration tests with the real Dart SDK (3.6.2).
//!
//! Marked with `#[ignore]` so as not to block environments without an
//! installed SDK. To run:
//! `cargo test -p dartforge-types --test sdk -- --ignored --nocapture`

use dartforge_elements::load::load_lenient;
use dartforge_elements::model::*;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::subtyping::{is_subtype, SubtypeEnv};
use dartforge_types::table::{CoreTypes, Type, TypeTable};
use dartforge_types::{resolve_outline, OutlineTypes};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn get_sdk() -> Option<(PathBuf, SdkLayout)> {
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

fn load_full_sdk() -> Option<(Program, Interner, TypeTable, CoreTypes, OutlineTypes)> {
    let (_lib_dir, sdk) = get_sdk()?;

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    let mut imports = String::new();
    for (name, lib) in &sdk.libraries {
        if lib.supported && !name.starts_with('_') {
            imports.push_str(&format!("import 'dart:{name}';\n"));
        }
    }
    imports.push_str("void main() {}\n");
    fs::write(&entry, &imports).unwrap();

    let mut interner = Interner::new();
    let (program, _elements_diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);

    let (outline, _types_diags) = resolve_outline(&program, &interner, &mut table, &core);

    Some((program, interner, table, core, outline))
}

#[test]
#[ignore]
fn outline_do_sdk_resolve() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK 3.6.2 not available, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    let mut imports = String::new();
    for (name, lib) in &sdk.libraries {
        if lib.supported && !name.starts_with('_') {
            imports.push_str(&format!("import 'dart:{name}';\n"));
        }
    }
    imports.push_str("void main() {}\n");
    fs::write(&entry, &imports).unwrap();

    let mut interner = Interner::new();
    let (program, _elements_diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);

    let (outline, types_diags) = resolve_outline(&program, &interner, &mut table, &core);

    let mut total_elements = 0;
    total_elements += program.classes.len();
    total_elements += program.functions.len();
    total_elements += program.variables.len();
    total_elements += program.typedefs.len();
    total_elements += program.extensions.len();

    let total_annotations = outline.functions.len() + outline.variables.len() + outline.classes.len();

    println!("============================================================");
    println!(" Dart SDK 3.6.2 Resolution Statistics (dartforge-types)");
    println!("============================================================");
    println!("Loaded libraries:               {}", program.libraries.len());
    println!("Units (.dart files):            {}", program.units.len());
    println!("Total elements in the outline:  {}", total_elements);
    println!("  - Classes / Mixins / Enums:   {}", program.classes.len());
    println!("  - Functions / Methods / Ctors: {}", program.functions.len());
    println!("  - Variables / Fields:          {}", program.variables.len());
    println!("  - Typedefs:                   {}", program.typedefs.len());
    println!("  - Extensions:                 {}", program.extensions.len());
    println!("Resolved annotations:           {}", total_annotations);
    println!("Unique TypeIds (hash-consed):   {}", table.len());
    println!(
        "TypeTable payload bytes:        {} bytes ({:.2} KB / {:.2} MB)",
        table.payload_bytes(),
        table.payload_bytes() as f64 / 1024.0,
        table.payload_bytes() as f64 / 1_048_576.0
    );
    println!("types diagnostics:              {}", types_diags.len());
    println!("============================================================");

    assert!(program.libraries.len() >= 30, "must load the SDK libraries");
    assert!(table.len() > 100, "must intern more than 100 unique types");
}

#[test]
#[ignore]
fn sdk_hierarchy_instantiates() {
    let Some((program, mut interner, mut table, core, outline)) = load_full_sdk() else {
        eprintln!("SDK not available, skipping.");
        return;
    };

    let mut find_class = |lib_uri: &str, name: &str| -> ClassId {
        let sym = interner.intern(name);
        let lib_idx = program
            .libraries
            .iter()
            .position(|l| l.uri == lib_uri)
            .unwrap_or_else(|| panic!("library {lib_uri} must exist"));
        let lib = &program.libraries[lib_idx];
        match lib.exported.get(&sym).and_then(|b| b.getter) {
            Some(Element::Class(cid)) => cid,
            _ => panic!("class {name} not found in {lib_uri}"),
        }
    };

    let list_cls = find_class("dart:core", "List");
    let iter_cls = find_class("dart:core", "Iterable");
    let map_cls = find_class("dart:core", "Map");
    let _int_cls = find_class("dart:core", "int");
    let comp_cls = find_class("dart:core", "Comparable");

    // 1. List<int> has Iterable<int> in its hierarchy (and NOT Iterable<E>)
    let list_int = table.intern(Type::Interface {
        class: list_cls,
        args: Box::new([core.int]),
        nullable: false,
    });

    let super_iter = outline
        .hierarchy
        .supertype_of(list_int, iter_cls, &mut table, &core)
        .expect("List<int> must inherit Iterable");

    match table.get(super_iter) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, iter_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int, "List<int> must have supertype Iterable<int>");
        }
        other => panic!("expected Interface Iterable<int>, got {:?}", other),
    }

    // 2. int has Comparable<num> in the SDK hierarchy
    let super_comp = outline
        .hierarchy
        .supertype_of(core.int, comp_cls, &mut table, &core)
        .expect("int must inherit Comparable");

    match table.get(super_comp) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, comp_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.num, "int must implement Comparable<num>");
        }
        other => panic!("expected Interface Comparable<num>, got {:?}", other),
    }

    // 3. Map<String, int> has Object in its hierarchy
    let map_str_int = table.intern(Type::Interface {
        class: map_cls,
        args: Box::new([core.string, core.int]),
        nullable: false,
    });

    let super_obj = outline
        .hierarchy
        .supertype_of(map_str_int, core.object_class.unwrap(), &mut table, &core)
        .expect("Map<String, int> must inherit Object");
    assert_eq!(super_obj, core.object);
}

#[test]
#[ignore]
fn known_subtyping() {
    let Some((program, mut interner, mut table, core, outline)) = load_full_sdk() else {
        eprintln!("SDK not available, skipping.");
        return;
    };

    let mut find_class = |lib_uri: &str, name: &str| -> ClassId {
        let sym = interner.intern(name);
        let lib_idx = program
            .libraries
            .iter()
            .position(|l| l.uri == lib_uri)
            .unwrap_or_else(|| panic!("library {lib_uri} must exist"));
        let lib = &program.libraries[lib_idx];
        match lib.exported.get(&sym).and_then(|b| b.getter) {
            Some(Element::Class(cid)) => cid,
            _ => panic!("class {name} not found in {lib_uri}"),
        }
    };

    let list_cls = find_class("dart:core", "List");
    let iter_cls = find_class("dart:core", "Iterable");
    let fut_cls = find_class("dart:async", "Future");

    let list_int = table.intern(Type::Interface {
        class: list_cls,
        args: Box::new([core.int]),
        nullable: false,
    });
    let iter_num = table.intern(Type::Interface {
        class: iter_cls,
        args: Box::new([core.num]),
        nullable: false,
    });
    let iter_int = table.intern(Type::Interface {
        class: iter_cls,
        args: Box::new([core.int]),
        nullable: false,
    });

    let fut_int = table.intern(Type::Interface {
        class: fut_cls,
        args: Box::new([core.int]),
        nullable: false,
    });
    let fut_obj = table.intern(Type::Interface {
        class: fut_cls,
        args: Box::new([core.object]),
        nullable: false,
    });

    let fn_num_to_int = table.intern(Type::Function {
        type_params: Box::new([]),
        ret: core.int,
        positional: Box::new([core.num]),
        optional: Box::new([]),
        named: Box::new([]),
        nullable: false,
    });
    let fn_int_to_num = table.intern(Type::Function {
        type_params: Box::new([]),
        ret: core.num,
        positional: Box::new([core.int]),
        optional: Box::new([]),
        named: Box::new([]),
        nullable: false,
    });

    let mut env = SubtypeEnv::new(&mut table, &outline.hierarchy, &core);

    // 1. List<int> <: Iterable<int>
    assert!(
        is_subtype(list_int, iter_int, &mut env),
        "List<int> <: Iterable<int>"
    );

    // 2. List<int> <: Iterable<num> (type argument covariance)
    assert!(
        is_subtype(list_int, iter_num, &mut env),
        "List<int> <: Iterable<num>"
    );

    // 3. Future<int> <: Future<Object>
    assert!(
        is_subtype(fut_int, fut_obj, &mut env),
        "Future<int> <: Future<Object>"
    );

    // 4. int Function(num) <: num Function(int) (parameter contravariance and return covariance)
    assert!(
        is_subtype(fn_num_to_int, fn_int_to_num, &mut env),
        "int Function(num) <: num Function(int)"
    );

    // 5. num Function(int) <: int Function(num) is FALSE
    assert!(
        !is_subtype(fn_int_to_num, fn_num_to_int, &mut env),
        "num Function(int) must NOT be a subtype of int Function(num)"
    );
}
