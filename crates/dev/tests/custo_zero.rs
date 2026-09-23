//! Portão estrutural da regra de custo zero (PLANO.md, "geração de código e
//! macros: rápidas, e custo zero para quem não usa"): num projeto sem
//! builders, a sessão não constrói motor, o relatório não tem motor, e uma
//! edição aloca **exatamente** o mesmo que numa sessão construída sem etapas
//! (o motor desativado por construção).
//!
//! **Um único `#[test]` neste binário**: o alocador contador é global.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::Sessao;
use std::path::{Path, PathBuf};

fn sdk_simulado(dir: &Path) -> PathBuf {
    let lib = dir.join("sdk/lib");
    std::fs::create_dir_all(lib.join("core")).unwrap();
    std::fs::write(
        lib.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("core/core.dart"),
        r#"
        library dart.core;
        class Object { const Object(); String toString() => ""; }
        class num extends Object {}
        class int extends num { int operator +(int o) => this; }
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
        class Type extends Object {}
        class Symbol extends Object {}
        class Enum extends Object {}
        class StackTrace extends Object {}
        class Invocation extends Object {}
        class Comparable<T> extends Object {}
        void print(Object? o) {}
        "#,
    )
    .unwrap();
    lib
}

/// Um pacote sem `build_runner`: `pubspec.yaml`, `package_config.json` com
/// só a raiz, e a entrada importando uma biblioteca de apoio.
fn projeto(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let proj = dir.join("proj");
    std::fs::create_dir_all(proj.join("lib")).unwrap();
    std::fs::create_dir_all(proj.join(".dart_tool")).unwrap();
    std::fs::write(proj.join("pubspec.yaml"), "name: proj\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    let cfg = proj.join(".dart_tool/package_config.json");
    std::fs::write(
        &cfg,
        r#"{"configVersion":2,"packages":[{"name":"proj","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    )
    .unwrap();
    let apoio = proj.join("lib/apoio.dart");
    std::fs::write(&apoio, "int dobro(int x) => x + x;\n").unwrap();
    let main = proj.join("main.dart");
    std::fs::write(&main, "import 'package:proj/apoio.dart';\nvoid main() { print(dobro(2)); }\n").unwrap();
    (main, apoio, cfg)
}

#[test]
fn projeto_sem_builders_custa_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = sdk_simulado(tmp.path());
    let (main, apoio, cfg) = projeto(tmp.path());
    // Mesmo comprimento de caminho de saída nas duas sessões.
    let (s1, s2) = (tmp.path().join("saida1"), tmp.path().join("saida2"));
    let mut com_deteccao = Sessao::nova(&main, Some(&sdk), Some(&cfg), &s1).expect("sessão com detecção");
    let mut sem_etapas = Sessao::com_etapas(&main, Some(&sdk), Some(&cfg), &s2, Vec::new()).expect("sessão sem etapas");
    assert!(!com_deteccao.tem_etapas(), "projeto sem build_runner ganhou etapa de geração");
    let r1 = com_deteccao.compilar().expect("primeira compilação");
    let r2 = sem_etapas.compilar().expect("primeira compilação");
    assert!(r1.motor.is_none() && r2.motor.is_none(), "relatório com motor num projeto sem builders");
    assert_eq!(r1.modulos, r2.modulos);
    for i in 0..4 {
        std::fs::write(&apoio, format!("int dobro(int x) {{ var y = {i:03}; return x + x; }}\n")).unwrap();
        let mut contagens = Vec::new();
        for s in [&mut com_deteccao, &mut sem_etapas] {
            let antes = dartforge_instrument::allocation_count();
            for p in s.mudancas() {
                s.arquivo_mudou(&p);
            }
            s.arquivo_mudou(&apoio);
            let r = s.compilar().expect("edição");
            contagens.push(dartforge_instrument::allocation_count() - antes);
            assert!(r.motor.is_none());
            assert_eq!(r.unidades_reanalisadas, 1, "edição de corpo reanalisa só a unidade editada");
        }
        assert_eq!(
            contagens[0], contagens[1],
            "a edição alocou diferente com a detecção do motor ({}) e sem etapas ({})",
            contagens[0], contagens[1]
        );
    }
    assert_eq!(dartforge_build::instancias(), 0, "um motor de build foi construído");
}
