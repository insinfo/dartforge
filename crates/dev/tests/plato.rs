//! Platô de memória da sessão residente (PLANO.md, item 1 de "O que falta").
//!
//! Vinte edições sucessivas — corpo, assinatura pública e import — e os bytes
//! vivos têm de estabilizar num platô em vez de crescer com N. Um crescimento
//! aqui significa que alguma estrutura viva guarda o modelo da edição
//! anterior: é o defeito que faz o analyzer e o `webdev` do Dart chegarem a
//! gigabytes.
//!
//! **Um único `#[test]` neste binário**: o alocador contador é global e mede o
//! processo inteiro; dois testes em paralelo mediriam um ao outro.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::Sessao;
use std::path::{Path, PathBuf};

/// SDK simulado: `dart:core` com o mínimo para tipar o projeto do teste.
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

/// Projeto: uma biblioteca de apoio e a entrada que a importa.
fn projeto(dir: &Path) -> (PathBuf, PathBuf) {
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let apoio = proj.join("apoio.dart");
    std::fs::write(&apoio, "int dobro(int x) => x + x;\n").unwrap();
    let main = proj.join("main.dart");
    std::fs::write(&main, "import 'apoio.dart';\nvoid main() { print(dobro(2)); }\n").unwrap();
    (main, apoio)
}

/// Três formas de edição, que invalidam coisas diferentes.
fn editar(apoio: &Path, main: &Path, i: usize) {
    match i % 3 {
        // Corpo: só o texto da função muda; a API fica igual.
        0 => std::fs::write(apoio, format!("int dobro(int x) {{ var y = {i:03}; return x + x; }}\n")).unwrap(),
        // Assinatura pública: entra e sai uma função de topo.
        1 => std::fs::write(
            apoio,
            format!("int dobro(int x) => x + x;\nint triplo{i:03}(int x) => x + x + x;\n"),
        )
        .unwrap(),
        // Import: a entrada passa a importar (e deixar de importar) dart:core
        // explicitamente, mudando o grafo.
        _ => std::fs::write(
            main,
            format!("import 'apoio.dart';\nimport 'dart:core';\nvoid main() {{ print(dobro({i:03})); }}\n"),
        )
        .unwrap(),
    }
}

#[test]
fn edicoes_sucessivas_estabilizam_em_plato() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = sdk_simulado(tmp.path());
    let (main, apoio) = projeto(tmp.path());
    let saida = tmp.path().join("js");

    let mut sessao = Sessao::nova(&main, Some(&sdk), None, &saida).expect("sessão");
    sessao.compilar().expect("primeira compilação");

    // Aquecimento: as primeiras edições ainda internam nomes novos e crescem
    // as arenas; o platô é medido depois que a forma do projeto se repete.
    for i in 0..4 {
        editar(&apoio, &main, i);
        sessao.arquivo_mudou(&apoio);
        sessao.arquivo_mudou(&main);
        sessao.compilar().expect("compilação de aquecimento");
    }

    let base = dartforge_instrument::live_bytes();
    let mut maior = base;
    for i in 4..24 {
        editar(&apoio, &main, i);
        sessao.arquivo_mudou(&apoio);
        sessao.arquivo_mudou(&main);
        let r = sessao.compilar().expect("compilação da edição");
        assert!(r.modulos >= 2, "o projeto tem pelo menos dois módulos");
        maior = maior.max(dartforge_instrument::live_bytes());
    }
    let fim = dartforge_instrument::live_bytes();

    // Tolerância: o texto das edições varia de tamanho e o `Interner` guarda
    // os nomes novos de cada revisão (`triplo007`…), que são poucos e curtos.
    let limite = base + 256 * 1024;
    assert!(
        fim <= limite && maior <= limite,
        "memória cresceu com as edições: base {base} B, maior {maior} B, fim {fim} B (limite {limite} B)"
    );
}
