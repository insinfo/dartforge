use dartforge_elements::load::load_lenient;
use dartforge_elements::model::*;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn get_sdk() -> Option<(PathBuf, SdkLayout)> {
    let lib_dir = SdkLayout::discover()?;
    let layout = SdkLayout::load(&lib_dir, "dartdevc").ok()?;
    Some((lib_dir, layout))
}

fn is_parser_diagnostic(d: &dartforge_diagnostics::Diagnostic) -> bool {
    d.code.is_some_and(|c| c.info().error_type == dartforge_diagnostics::ErrorType::SyntacticError)
}

#[test]
fn test_sdk_core_load_and_patch() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK not available in the environment, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(
        &entry,
        r#"
        import 'dart:core';
        void main() {}
        "#,
    )
    .unwrap();

    let mut interner = Interner::new();
    let (program, diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let mut parser_diags = Vec::new();
    let mut elements_diags = Vec::new();

    for d in diags {
        if is_parser_diagnostic(&d) {
            parser_diags.push(d);
        } else {
            elements_diags.push(d);
        }
    }

    println!(
        "[test_sdk_core_load_and_patch] Rejected by the parser: {}, rejected by elements: {}",
        parser_diags.len(),
        elements_diags.len()
    );

    for ed in &elements_diags {
        eprintln!("elements error: {}", ed);
    }
    assert!(
        elements_diags.is_empty(),
        "elements produced unexpected diagnostics: {:?}",
        elements_diags
    );

    // 1. dart:core loaded
    let core_id = program.core.expect("dart:core must be registered");
    let core_lib = &program.libraries[core_id.0 as usize];
    assert_eq!(core_lib.uri, "dart:core");

    // 2. int exists in dart:core
    let int_sym = interner.intern("int");
    let int_binding = core_lib
        .exported
        .get(&int_sym)
        .expect("int must be exported by dart:core");
    let int_cid = match int_binding.getter {
        Some(Element::Class(cid)) => cid,
        other => panic!("int must be a class, got {:?}", other),
    };

    // 3. int.parse coming from the patch and not external
    let int_class = &program.classes[int_cid.0 as usize];
    let parse_sym = interner.intern("parse");
    let parse_fn_id = int_class
        .static_members
        .get(&parse_sym)
        .expect("int.parse must exist in int's static_members");

    let parse_fn = &program.functions[parse_fn_id.0 as usize];
    assert!(
        !parse_fn.external,
        "int.parse coming from the patch must have external == false"
    );
    println!("int.parse checked successfully (patched_by={:?}, external={})", parse_fn.patched_by, parse_fn.external);
}

#[test]
fn every_dart_x_loads() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK not available in the environment, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    // Imports every supported SDK library declared in libraries.json
    let mut imports = String::new();
    for (name, lib) in &sdk.libraries {
        if lib.supported && !name.starts_with('_') {
            imports.push_str(&format!("import 'dart:{name}';\n"));
        }
    }
    imports.push_str("void main() {}\n");
    fs::write(&entry, &imports).unwrap();

    let mut interner = Interner::new();
    let (program, diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let mut parser_diags = Vec::new();
    let mut elements_diags = Vec::new();

    for d in diags {
        if is_parser_diagnostic(&d) {
            parser_diags.push(d);
        } else {
            elements_diags.push(d);
        }
    }

    println!(
        "[every_dart_x_loads] Libraries loaded: {}, units: {}, classes: {}, functions: {}",
        program.libraries.len(),
        program.units.len(),
        program.classes.len(),
        program.functions.len()
    );
    println!(
        "[every_dart_x_loads] Rejected by the parser: {}, rejected by elements: {}",
        parser_diags.len(),
        elements_diags.len()
    );

    for ed in &elements_diags {
        eprintln!("elements error: {}", ed);
    }
    assert!(
        elements_diags.is_empty(),
        "elements produced unexpected diagnostics: {:?}",
        elements_diags
    );

    // Checks that dart:core's int.parse was applied
    let core_id = program.core.expect("dart:core registered");
    let core_lib = &program.libraries[core_id.0 as usize];
    let int_sym = interner.intern("int");
    let int_cid = match core_lib.exported.get(&int_sym).and_then(|b| b.getter) {
        Some(Element::Class(cid)) => cid,
        _ => panic!("int not found in dart:core"),
    };
    let parse_sym = interner.intern("parse");
    let parse_fn_id = program.classes[int_cid.0 as usize]
        .static_members
        .get(&parse_sym)
        .expect("int.parse presente");
    assert!(!program.functions[parse_fn_id.0 as usize].external);
}

#[test]
fn sdk_supertypes_resolve() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK not available in the environment, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(&entry, "import 'dart:core'; import 'dart:async'; import 'dart:collection'; void main() {}\n").unwrap();

    let mut interner = Interner::new();
    let (program, diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let elements_diags: Vec<_> = diags.into_iter().filter(|d| !is_parser_diagnostic(d)).collect();
    assert!(elements_diags.is_empty(), "elements diagnostics: {:?}", elements_diags);

    // Ensures every class that declared extends has a resolved supertype_class
    // (except Object)
    let mut resolved_count = 0;
    let mut total_with_super = 0;
    let object_sym = interner.intern("Object");

    for class in &program.classes {
        if class.name == object_sym {
            continue;
        }
        if class.supertype.is_some() {
            total_with_super += 1;
            if class.supertype_class.is_some() {
                resolved_count += 1;
            }
        }
    }

    println!(
        "[sdk_supertypes_resolve] Supertypes with extends: {}, resolved to ClassId: {}",
        total_with_super, resolved_count
    );
    assert!(total_with_super > 0, "There must be classes with a supertype in the SDK");
    assert_eq!(total_with_super, resolved_count, "Every declared supertype must resolve to a ClassId");
}

#[test]
fn sdk_namespaces_without_ambiguity_in_use() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK not available in the environment, skipping test.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(&entry, "import 'dart:core'; void main() {}\n").unwrap();

    let mut interner = Interner::new();
    let (program, _) = load_lenient(&entry, &sdk, None, &mut interner);

    let core_id = program.core.expect("dart:core registered");
    let core_lib = &program.libraries[core_id.0 as usize];

    // No export of dart:core may be ambiguous
    for (sym, binding) in &core_lib.exported {
        assert!(
            !binding.ambiguous,
            "Symbol '{}' exported by dart:core is ambiguous",
            interner.resolve(*sym)
        );
    }
}

