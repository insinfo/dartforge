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

fn is_parser_diagnostic(msg: &str) -> bool {
    msg.contains("esperava")
        || msg.contains("não suportad")
        || msg.contains("token inesperado")
        || msg.contains("caractere inválido")
}

#[test]
fn test_sdk_core_load_and_patch() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK não disponível no ambiente, pulando teste.");
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
        if is_parser_diagnostic(&d.message) {
            parser_diags.push(d);
        } else {
            elements_diags.push(d);
        }
    }

    println!(
        "[test_sdk_core_load_and_patch] Recusados pelo parser: {}, Recusados pelo elements: {}",
        parser_diags.len(),
        elements_diags.len()
    );

    for ed in &elements_diags {
        eprintln!("Erro elements: {}", ed);
    }
    assert!(
        elements_diags.is_empty(),
        "O elements produziu diagnósticos indevidos: {:?}",
        elements_diags
    );

    // 1. dart:core carregou
    let core_id = program.core.expect("dart:core deve estar registrado");
    let core_lib = &program.libraries[core_id.0 as usize];
    assert_eq!(core_lib.uri, "dart:core");

    // 2. int existe em dart:core
    let int_sym = interner.intern("int");
    let int_binding = core_lib
        .exported
        .get(&int_sym)
        .expect("int deve estar exportado por dart:core");
    let int_cid = match int_binding.getter {
        Some(Element::Class(cid)) => cid,
        other => panic!("int deve ser uma classe, obteve {:?}", other),
    };

    // 3. int.parse vindo do patch e não external
    let int_class = &program.classes[int_cid.0 as usize];
    let parse_sym = interner.intern("parse");
    let parse_fn_id = int_class
        .static_members
        .get(&parse_sym)
        .expect("int.parse deve existir nos static_members de int");

    let parse_fn = &program.functions[parse_fn_id.0 as usize];
    assert!(
        !parse_fn.external,
        "int.parse vindo do patch deve ter external == false"
    );
    println!("int.parse verificado com sucesso (patched_by={:?}, external={})", parse_fn.patched_by, parse_fn.external);
}

#[test]
fn todo_dart_x_carrega() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK não disponível no ambiente, pulando teste.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");

    // Importa todas as bibliotecas suportadas do SDK declaradas em libraries.json
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
        if is_parser_diagnostic(&d.message) {
            parser_diags.push(d);
        } else {
            elements_diags.push(d);
        }
    }

    println!(
        "[todo_dart_x_carrega] Bibliotecas carregadas: {}, Unidades: {}, Classes: {}, Funções: {}",
        program.libraries.len(),
        program.units.len(),
        program.classes.len(),
        program.functions.len()
    );
    println!(
        "[todo_dart_x_carrega] Recusados pelo parser: {}, Recusados pelo elements: {}",
        parser_diags.len(),
        elements_diags.len()
    );

    for ed in &elements_diags {
        eprintln!("Erro elements: {}", ed);
    }
    assert!(
        elements_diags.is_empty(),
        "O elements produziu diagnósticos indevidos: {:?}",
        elements_diags
    );

    // Valida que int.parse de dart:core foi aplicado
    let core_id = program.core.expect("dart:core registrado");
    let core_lib = &program.libraries[core_id.0 as usize];
    let int_sym = interner.intern("int");
    let int_cid = match core_lib.exported.get(&int_sym).and_then(|b| b.getter) {
        Some(Element::Class(cid)) => cid,
        _ => panic!("int não encontrado em dart:core"),
    };
    let parse_sym = interner.intern("parse");
    let parse_fn_id = program.classes[int_cid.0 as usize]
        .static_members
        .get(&parse_sym)
        .expect("int.parse presente");
    assert!(!program.functions[parse_fn_id.0 as usize].external);
}

#[test]
fn supertipos_do_sdk_resolvem() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK não disponível no ambiente, pulando teste.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(&entry, "import 'dart:core'; import 'dart:async'; import 'dart:collection'; void main() {}\n").unwrap();

    let mut interner = Interner::new();
    let (program, diags) = load_lenient(&entry, &sdk, None, &mut interner);

    let elements_diags: Vec<_> = diags.into_iter().filter(|d| !is_parser_diagnostic(&d.message)).collect();
    assert!(elements_diags.is_empty(), "Diagnósticos do elements: {:?}", elements_diags);

    // Assegura que todas as classes que declararam extends têm supertype_class resolvido (exceto Object)
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
        "[supertipos_do_sdk_resolvem] Supertipos com extends: {}, Resolvidos para ClassId: {}",
        total_with_super, resolved_count
    );
    assert!(total_with_super > 0, "Deve haver classes com supertipo no SDK");
    assert_eq!(total_with_super, resolved_count, "Todos os supertipos declarados devem resolver para ClassId");
}

#[test]
fn namespaces_do_sdk_sem_ambiguidade_em_uso() {
    let Some((_lib_dir, sdk)) = get_sdk() else {
        eprintln!("SDK não disponível no ambiente, pulando teste.");
        return;
    };

    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(&entry, "import 'dart:core'; void main() {}\n").unwrap();

    let mut interner = Interner::new();
    let (program, _) = load_lenient(&entry, &sdk, None, &mut interner);

    let core_id = program.core.expect("dart:core registrado");
    let core_lib = &program.libraries[core_id.0 as usize];

    // Nenhum export de dart:core pode ser ambíguo
    for (sym, binding) in &core_lib.exported {
        assert!(
            !binding.ambiguous,
            "Símbolo '{}' exportado por dart:core está ambíguo",
            interner.resolve(*sym)
        );
    }
}

