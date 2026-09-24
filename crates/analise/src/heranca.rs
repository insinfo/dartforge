//! Conflito entre membro estático declarado e membro de instância herdado.
//! Parte delimitada de `ErrorVerifier._checkForConflictingClassMembers`:
//! cadeia linear de superclasses e uma interface direta, sem mixins.

use dartforge_diagnostics::{Diagnostic, codigos::compile_time_error as c};
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{MemberKind, Name};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// Diagnósticos por unidade da biblioteca `lib`. Membros locais conflitantes
/// já são emitidos por `duplicatas`; esta função considera só ancestrais.
pub fn estatico_contra_super(programa: &Program, lib: LibraryId, nomes: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    for (i, classe) in programa.classes.iter().enumerate() {
        if classe.library != lib || classe.decl.is_none()
            || !matches!(classe.kind, ClassKind::Class | ClassKind::Mixin)
            || !classe.mixins.is_empty() || classe.interface_classes.len() > 1
        { continue; }
        let id = ClassId(i as u32);
        let nome_classe = nomes.resolve(classe.name);
        for (unidade, membro) in programa.membros_da_classe(id) {
            let ast = &programa.unit(unidade).ast;
            let declarados: Vec<Name> = match &ast.member(membro).kind {
                MemberKind::Field(lista) if lista.static_ => lista.variables.iter().map(|v| v.name).collect(),
                MemberKind::Method(f) if ast.function(*f).static_ => ast.function(*f).name.into_iter().collect(),
                _ => continue,
            };
            for declarado in declarados {
                let nome = nomes.resolve(declarado.sym);
                // O outline indexa setters pela chave interna `nome_=`.
                let setter = nomes.lookup(&format!("{nome}_="));
                if classe.instance_members.contains_key(&declarado.sym)
                    || setter.is_some_and(|s| classe.instance_members.contains_key(&s))
                { continue; }
                let mut ancestral = classe.supertype_class;
                let mut vistos = HashSet::new();
                let mut dono = None;
                while let Some(base) = ancestral {
                    if !vistos.insert(base) { break; }
                    let herdada = programa.class(base);
                    if herdada.kind == ClassKind::MixinApplication { break; }
                    let visivel = !nome.starts_with('_') || herdada.library == lib;
                    if visivel && (herdada.instance_members.contains_key(&declarado.sym)
                        || setter.is_some_and(|s| herdada.instance_members.contains_key(&s)))
                    {
                        dono = Some(nomes.resolve(herdada.name));
                        break;
                    }
                    ancestral = herdada.supertype_class;
                }
                // `implements` também compõe a interface herdada. Começamos
                // pelo caso de uma interface direta, sem ambiguidade de dono.
                if dono.is_none() {
                    if let Some(&interface) = classe.interface_classes.first() {
                        let herdada = programa.class(interface);
                        let visivel = !nome.starts_with('_') || herdada.library == lib;
                        if visivel && (herdada.instance_members.contains_key(&declarado.sym)
                            || setter.is_some_and(|s| herdada.instance_members.contains_key(&s)))
                        { dono = Some(nomes.resolve(herdada.name)); }
                    }
                }
                if let Some(dono) = dono {
                    saida.push((unidade, Diagnostic::com_codigo(
                        c::CONFLICTING_STATIC_AND_INSTANCE,
                        declarado.span,
                        [nome_classe, nome, dono],
                    )));
                }
            }
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;

    #[test]
    fn membro_estatico_contra_getter_do_super_e_object() {
        // analyzer/test/src/diagnostics/conflicting_static_and_instance_test.dart:
        // test_inSuper_instanceGetter_staticGetter e
        // test_inSuper_implicitObject_staticMethod.
        let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/heranca-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object { String toString() => ''; } class String extends Object {} class int extends Object {}").unwrap();
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        let fonte = "class A { int get foo => 0; }\nclass B extends A { static int get foo => 0; }\nclass C { static String toString() => ''; }\n";
        let entrada = raiz.join("main.dart");
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let lib = programa.entry.unwrap();
        let diags = estatico_contra_super(&programa, lib, &nomes);
        assert_eq!(diags.len(), 2, "{diags:?}");
        assert_eq!(diags[0].1.code, Some(c::CONFLICTING_STATIC_AND_INSTANCE));
        assert_eq!(&fonte[diags[0].1.span.start as usize..diags[0].1.span.end as usize], "foo");
        assert_eq!(diags[0].1.message, "Class 'B' can't define static member 'foo' and have instance member 'A.foo' with the same name.");
        assert_eq!(&fonte[diags[1].1.span.start as usize..diags[1].1.span.end as usize], "toString");
        assert!(diags[1].1.message.contains("Object.toString"));
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn campo_estatico_ve_metodo_transitivo_mas_nao_duplica_conflito_local() {
        let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/heranca-transitiva-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object {} class int extends Object {}").unwrap();
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        let fonte = "class A { void foo() {} }\nclass B extends A { static int foo = 0; }\nclass C extends B { static void foo() {} }\nclass D extends A { void foo() {} static void foo() {} }\n";
        let entrada = raiz.join("main.dart");
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let diags = estatico_contra_super(&programa, programa.entry.unwrap(), &nomes);
        assert_eq!(diags.len(), 2, "{diags:?}");
        assert!(diags.iter().all(|(_, d)| d.message.contains("A.foo")), "{diags:?}");
        assert!(diags.iter().all(|(_, d)| &fonte[d.span.start as usize..d.span.end as usize] == "foo"));
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn dez_casos_de_superclasse_do_corpus_oficial() {
        let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/heranca-corpus-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object { String toString() => ''; int get runtimeType => 0; } class String extends Object {} class int extends Object {}").unwrap();
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        let corpus = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/diagnosticos/analyzer/conflicting_static_and_instance");
        let mut fontes: Vec<_> = fs::read_dir(corpus).unwrap().flatten()
            .map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains("__inSu_"))).collect();
        fontes.sort();
        assert_eq!(fontes.len(), 10);
        let entrada = raiz.join("main.dart");
        for arquivo in fontes {
            let fonte = fs::read_to_string(&arquivo).unwrap();
            fs::write(&entrada, &fonte).unwrap();
            let mut nomes = Interner::new();
            let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
            let diags = estatico_contra_super(&programa, programa.entry.unwrap(), &nomes);
            assert_eq!(diags.len(), 1, "{}: {diags:?}", arquivo.display());
            let d = &diags[0].1;
            let esperado = fonte.lines().find_map(|l| l.split_once("[diag.conflictingStaticAndInstance] ").map(|(_, m)| m)).unwrap();
            assert_eq!(d.code, Some(c::CONFLICTING_STATIC_AND_INSTANCE));
            assert_eq!(d.message, esperado, "{}", arquivo.display());
            assert!(matches!(&fonte[d.span.start as usize..d.span.end as usize], "foo" | "toString" | "runtimeType"));
        }
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn quinze_casos_de_interface_direta_do_corpus_oficial() {
        // analyzer/test/src/diagnostics/conflicting_static_and_instance_test.dart:
        // `test_inInterface_*`, extraídos em corpus/diagnosticos/analyzer.
        let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/heranca-interface-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object {} class int extends Object {}").unwrap();
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        let corpus = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/diagnosticos/analyzer/conflicting_static_and_instance");
        let mut fontes: Vec<_> = fs::read_dir(corpus).unwrap().flatten()
            .map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains("__inIn_"))).collect();
        fontes.sort();
        assert_eq!(fontes.len(), 15);
        let entrada = raiz.join("main.dart");
        for arquivo in fontes {
            let fonte = fs::read_to_string(&arquivo).unwrap();
            fs::write(&entrada, &fonte).unwrap();
            let mut nomes = Interner::new();
            let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
            let diags = estatico_contra_super(&programa, programa.entry.unwrap(), &nomes);
            assert_eq!(diags.len(), 1, "{}: {diags:?}", arquivo.display());
            let d = &diags[0].1;
            let esperado = fonte.lines().find_map(|l| l.split_once("[diag.conflictingStaticAndInstance] ").map(|(_, m)| m)).unwrap();
            assert_eq!(d.code, Some(c::CONFLICTING_STATIC_AND_INSTANCE));
            assert_eq!(d.message, esperado, "{}", arquivo.display());
            assert_eq!(&fonte[d.span.start as usize..d.span.end as usize], "foo");
        }
        fs::remove_dir_all(&raiz).unwrap();
    }
}
