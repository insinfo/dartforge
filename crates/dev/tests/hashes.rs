//! Os dois hashes por biblioteca separam corpo de API pública, e a sessão
//! reaproveita o que não mudou.

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

#[test]
fn corpo_privado_nao_muda_a_api_e_assinatura_muda() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = sdk_simulado(tmp.path());
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let apoio = proj.join("apoio.dart");
    let main = proj.join("main.dart");
    std::fs::write(&apoio, "int dobro(int x) { var s = 1; return x + s; }\n").unwrap();
    std::fs::write(&main, "import 'apoio.dart';\nvoid main() { print(dobro(2)); }\n").unwrap();
    let saida = tmp.path().join("js");

    let mut sessao = Sessao::nova(&main, Some(&sdk), None, &saida).expect("sessão");
    let r = sessao.compilar().expect("primeira");
    assert!(r.primeira);
    assert!(r.modulos >= 2, "uma biblioteca por arquivo: {}", r.modulos);
    let unidades = sessao.unidades_em_cache();
    assert!(unidades >= 3, "entrada, apoio e o core do SDK: {unidades}");

    // 1. Edição de corpo: o valor muda, a assinatura não.
    std::fs::write(&apoio, "int dobro(int x) { var s = 2; return x + s; }\n").unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("edição de corpo");
    assert_eq!(r.corpo_alterado.len(), 1, "só `apoio.dart` mudou: {:?}", r.corpo_alterado);
    assert!(r.api_alterada.is_empty(), "corpo não é API: {:?}", r.api_alterada);
    assert_eq!(r.dependentes_invalidados, 0);
    assert_eq!(r.unidades_reanalisadas, 1, "só o arquivo editado é reanalisado");
    assert_eq!(r.modulos_escritos, 1, "só o módulo de `apoio` muda de texto");

    // 2. Edição de API: uma função pública nova.
    std::fs::write(&apoio, "int dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\n").unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("edição de API");
    assert_eq!(r.api_alterada.len(), 1, "a API de `apoio` mudou: {:?}", r.api_alterada);
    assert_eq!(r.dependentes_invalidados, 1, "`main.dart` importa `apoio.dart`");

    // 3. Função privada nova: muda o corpo da biblioteca, não a API.
    std::fs::write(
        &apoio,
        "int dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("edição privada");
    assert_eq!(r.corpo_alterado.len(), 1);
    assert!(r.api_alterada.is_empty(), "declaração privada não é API: {:?}", r.api_alterada);

    // 3b. Statement novo dentro de um método de classe: é a edição mais comum
    // (uma tecla num corpo) e não pode mexer na API — senão cada tecla
    // invalidaria todos os dependentes.
    std::fs::write(
        &apoio,
        "class Conta {\n  int saldo = 0;\n  int somar(int x) { saldo = saldo + x; return saldo; }\n}\nint dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    sessao.compilar().expect("classe nova");
    std::fs::write(
        &apoio,
        "class Conta {\n  int saldo = 0;\n  int somar(int x) { print(x); saldo = saldo + x; return saldo; }\n}\nint dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("statement novo no método");
    assert_eq!(r.corpo_alterado.len(), 1, "o corpo mudou: {:?}", r.corpo_alterado);
    assert!(
        r.api_alterada.is_empty(),
        "inserir um statement num método não é mudança de API: {:?}",
        r.api_alterada
    );
    assert_eq!(r.dependentes_invalidados, 0);

    // 3c. Parâmetro novo num método público: aí sim a API mudou, e o
    // dependente entra na conta.
    std::fs::write(
        &apoio,
        "class Conta {\n  int saldo = 0;\n  int somar(int x, int y) { print(x); saldo = saldo + x; return saldo; }\n}\nint dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("parâmetro novo");
    assert_eq!(r.api_alterada.len(), 1, "parâmetro novo é API: {:?}", r.api_alterada);
    assert_eq!(r.dependentes_invalidados, 1, "`main.dart` depende de `apoio.dart`");

    // 3d. Renomear um método público muda a API.
    std::fs::write(
        &apoio,
        "class Conta {\n  int saldo = 0;\n  int adicionar(int x, int y) { print(x); saldo = saldo + x; return saldo; }\n}\nint dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("renomear método");
    assert_eq!(r.api_alterada.len(), 1, "renomear é API: {:?}", r.api_alterada);

    // 3e. Comentário e espaço em branco não são API nem corpo emitido.
    std::fs::write(
        &apoio,
        "// um comentário novo\nclass Conta {\n  int saldo = 0;\n\n  int adicionar(int x, int y) { print(x); saldo = saldo + x; return saldo; }\n}\nint dobro(int x) { var s = 2; return x + s; }\nint triplo(int x) => x;\nint _oculto(int x) => x;\n",
    )
    .unwrap();
    sessao.arquivo_mudou(&apoio);
    let r = sessao.compilar().expect("comentário");
    assert!(r.api_alterada.is_empty(), "comentário não é API: {:?}", r.api_alterada);
    assert_eq!(r.modulos_escritos, 0, "o JS não mudou, nada a reescrever");

    // 4. Nada muda: nenhuma biblioteca acusa alteração e nada é reescrito.
    let r = sessao.compilar().expect("sem edição");
    assert!(r.corpo_alterado.is_empty(), "{:?}", r.corpo_alterado);
    assert!(r.api_alterada.is_empty());
    assert_eq!(r.unidades_reanalisadas, 0, "nada para reanalisar");
    assert_eq!(r.modulos_escritos, 0, "nenhum módulo mudou de texto");
}
