//! Testes integrados com o Dart SDK real (3.6.2).
//!
//! Marcados com `#[ignore]` para não travar ambientes sem SDK instalado.
//! Para rodar:
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

fn carregar_sdk_completo() -> Option<(Program, Interner, TypeTable, CoreTypes, OutlineTypes)> {
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
        eprintln!("SDK 3.6.2 não disponível, pulando teste.");
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

    let mut total_elementos = 0;
    total_elementos += program.classes.len();
    total_elementos += program.functions.len();
    total_elementos += program.variables.len();
    total_elementos += program.typedefs.len();
    total_elementos += program.extensions.len();

    let total_anotacoes = outline.functions.len() + outline.variables.len() + outline.classes.len();

    println!("============================================================");
    println!(" Estatísticas de Resolução do SDK Dart 3.6.2 (dartforge-types)");
    println!("============================================================");
    println!("Bibliotecas carregadas:         {}", program.libraries.len());
    println!("Unidades (arquivos .dart):      {}", program.units.len());
    println!("Elementos totais no outline:    {}", total_elementos);
    println!("  - Classes / Mixins / Enums:   {}", program.classes.len());
    println!("  - Funções / Métodos / Ctors:  {}", program.functions.len());
    println!("  - Variáveis / Campos:         {}", program.variables.len());
    println!("  - Typedefs:                   {}", program.typedefs.len());
    println!("  - Extensions:                 {}", program.extensions.len());
    println!("Anotações resolvidas:           {}", total_anotacoes);
    println!("TypeIds únicos (hash-consed):   {}", table.len());
    println!(
        "Payload bytes da TypeTable:     {} bytes ({:.2} KB / {:.2} MB)",
        table.payload_bytes(),
        table.payload_bytes() as f64 / 1024.0,
        table.payload_bytes() as f64 / 1_048_576.0
    );
    println!("Diagnósticos de types:          {}", types_diags.len());
    println!("============================================================");

    assert!(program.libraries.len() >= 30, "deve carregar as bibliotecas do SDK");
    assert!(table.len() > 100, "deve internar mais de 100 tipos únicos");
}

#[test]
#[ignore]
fn hierarquia_do_sdk_instancia() {
    let Some((program, mut interner, mut table, core, outline)) = carregar_sdk_completo() else {
        eprintln!("SDK não disponível, pulando.");
        return;
    };

    let mut find_class = |lib_uri: &str, name: &str| -> ClassId {
        let sym = interner.intern(name);
        let lib_idx = program
            .libraries
            .iter()
            .position(|l| l.uri == lib_uri)
            .unwrap_or_else(|| panic!("biblioteca {lib_uri} deve existir"));
        let lib = &program.libraries[lib_idx];
        match lib.exported.get(&sym).and_then(|b| b.getter) {
            Some(Element::Class(cid)) => cid,
            _ => panic!("classe {name} não encontrada em {lib_uri}"),
        }
    };

    let list_cls = find_class("dart:core", "List");
    let iter_cls = find_class("dart:core", "Iterable");
    let map_cls = find_class("dart:core", "Map");
    let _int_cls = find_class("dart:core", "int");
    let comp_cls = find_class("dart:core", "Comparable");

    // 1. List<int> tem Iterable<int> na hierarquia (e NÃO Iterable<E>)
    let list_int = table.intern(Type::Interface {
        class: list_cls,
        args: Box::new([core.int]),
        nullable: false,
    });

    let super_iter = outline
        .hierarchy
        .supertype_of(list_int, iter_cls, &mut table, &core)
        .expect("List<int> deve herdar Iterable");

    match table.get(super_iter) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, iter_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.int, "List<int> deve ter supertipo Iterable<int>");
        }
        other => panic!("esperava Interface Iterable<int>, obteve {:?}", other),
    }

    // 2. int tem Comparable<num> na hierarquia do SDK
    let super_comp = outline
        .hierarchy
        .supertype_of(core.int, comp_cls, &mut table, &core)
        .expect("int deve herdar Comparable");

    match table.get(super_comp) {
        Type::Interface { class, args, .. } => {
            assert_eq!(*class, comp_cls);
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], core.num, "int deve implementar Comparable<num>");
        }
        other => panic!("esperava Interface Comparable<num>, obteve {:?}", other),
    }

    // 3. Map<String, int> tem Object na hierarquia
    let map_str_int = table.intern(Type::Interface {
        class: map_cls,
        args: Box::new([core.string, core.int]),
        nullable: false,
    });

    let super_obj = outline
        .hierarchy
        .supertype_of(map_str_int, core.object_class.unwrap(), &mut table, &core)
        .expect("Map<String, int> deve herdar Object");
    assert_eq!(super_obj, core.object);
}

#[test]
#[ignore]
fn subtipagem_conhecida() {
    let Some((program, mut interner, mut table, core, outline)) = carregar_sdk_completo() else {
        eprintln!("SDK não disponível, pulando.");
        return;
    };

    let mut find_class = |lib_uri: &str, name: &str| -> ClassId {
        let sym = interner.intern(name);
        let lib_idx = program
            .libraries
            .iter()
            .position(|l| l.uri == lib_uri)
            .unwrap_or_else(|| panic!("biblioteca {lib_uri} deve existir"));
        let lib = &program.libraries[lib_idx];
        match lib.exported.get(&sym).and_then(|b| b.getter) {
            Some(Element::Class(cid)) => cid,
            _ => panic!("classe {name} não encontrada em {lib_uri}"),
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

    // 2. List<int> <: Iterable<num> (covariância de tipo de argumento)
    assert!(
        is_subtype(list_int, iter_num, &mut env),
        "List<int> <: Iterable<num>"
    );

    // 3. Future<int> <: Future<Object>
    assert!(
        is_subtype(fut_int, fut_obj, &mut env),
        "Future<int> <: Future<Object>"
    );

    // 4. int Function(num) <: num Function(int) (contravariância de parâmetro e covariância de retorno)
    assert!(
        is_subtype(fn_num_to_int, fn_int_to_num, &mut env),
        "int Function(num) <: num Function(int)"
    );

    // 5. num Function(int) <: int Function(num) é FALSO
    assert!(
        !is_subtype(fn_int_to_num, fn_num_to_int, &mut env),
        "num Function(int) NÃO deve ser subtipo de int Function(num)"
    );
}
