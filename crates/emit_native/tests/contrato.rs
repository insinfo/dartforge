//! Os três defeitos que o diferencial JIT × AOT (`crates/jit/tests/execucao.rs`)
//! achou no emissor nativo, cada um preso pelo IR que o contrato de
//! representação produz (docs/NATIVO-PLANO.md §6). Sem Clang: só a emissão.

use dartforge_emit_native::{CompileOptions, emitir_ir};
use std::path::Path;

const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

/// O IR de `dart_main` do programa, ou `None` sem o SDK na máquina.
fn main_de(fonte: &str) -> Option<String> {
    if !Path::new(SDK).join("libraries.json").is_file() {
        eprintln!("SDK ausente em {SDK}; teste pulado");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("main.dart");
    std::fs::write(&entrada, fonte).unwrap();
    let ir = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let options = CompileOptions {
                sdk: Some(Path::new(SDK)),
                packages: None,
                timings: false,
                optimize: false,
            };
            emitir_ir(&entrada, &options).expect("emitir IR").texto
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(
        !ir.contains("dartforge_erro_de_compilacao"),
        "não compilou:\n{ir}"
    );
    let ini = ir.find("define void @dart_main(").expect("dart_main");
    let fim = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    Some(ir[ini..fim].to_string())
}

/// (1) `for` cuja variável é reatribuída no corpo: a variável mora num
/// `alloca` (R6) e a condição relê o valor dela a cada volta — antes ela era
/// um valor SSA no mapa por nome, e a condição via para sempre o inicial.
#[test]
fn for_com_variavel_reatribuida_le_o_local() {
    let Some(main) = main_de(
        "void main() {\n  for (var i = 0; i < 10; i++) {\n    if (i == 3) i = 7;\n    print(i);\n  }\n}\n",
    ) else {
        return;
    };
    assert!(main.contains("alloca i64"), "{main}");
    assert!(
        main.matches("load i64, ptr").count() >= 3,
        "a condição, o `if` e o `print` leem o local:\n{main}"
    );
    assert!(
        main.matches("store i64").count() >= 3,
        "inicial, `i = 7` e `i++` gravam:\n{main}"
    );
}

/// (2) `c.dobro()`: chamada do método pelo elemento resolvido (N1) — antes
/// caía no `Const(Int(0))` de "chamada não reconhecida" e virava `print(0)`.
#[test]
fn chamada_de_metodo_com_retorno() {
    let Some(main) = main_de(
        "class C {\n  int v;\n  C(this.v);\n  int dobro() => v * 2;\n}\nvoid main() {\n  final c = C(21);\n  print(c.dobro());\n}\n",
    ) else {
        return;
    };
    assert!(main.contains("_dobro(i64"), "{main}");
    assert!(!main.contains("@dartforge_print_i64(i64 0)"), "{main}");
}

/// (3) `print(soma(40, 2))` com retorno `int`: o resultado é `I64` (R1) e sai
/// por `print_i64` — antes ia a `print_handle` como se fosse handle.
#[test]
fn funcao_de_topo_com_retorno_int_imprime_o_escalar() {
    let Some(main) =
        main_de("int soma(int a, int b) => a + b;\nvoid main() {\n  print(soma(40, 2));\n}\n")
    else {
        return;
    };
    assert!(main.contains("= call i64 @df_fn_"), "{main}");
    assert!(main.contains("@dartforge_print_i64(i64 %v"), "{main}");
    assert!(!main.contains("@dartforge_print_handle"), "{main}");
}
