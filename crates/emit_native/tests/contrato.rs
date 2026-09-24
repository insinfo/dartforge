//! Os três defeitos que o diferencial JIT × AOT (`crates/jit/tests/execucao.rs`)
//! achou no emissor nativo, cada um preso pelo IR que o contrato de
//! representação produz (docs/NATIVO-PLANO.md §6). Sem Clang: só a emissão.

use dartforge_emit_native::{CompileOptions, emitir_ir, emitir_ir_com};
use std::path::Path;

const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

/// O IR completo do programa, ou `None` sem o SDK na máquina.
fn ir_de(fonte: &str) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() {
        eprintln!("SDK ausente em {sdk}; teste pulado");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("main.dart");
    std::fs::write(&entrada, fonte).unwrap();
    let ir = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let options = CompileOptions {
                sdk: Some(Path::new(&sdk)),
                packages: None,
                timings: false,
                optimize: false,
                versao_linguagem: None,
                experimentos: Vec::new(),
            };
            emitir_ir(&entrada, &options)
                .unwrap_or_else(|e| panic!("não compilou:\n{e}"))
                .texto
        })
        .unwrap()
        .join()
        .unwrap();
    Some(ir)
}

/// O IR de `dart_main` do programa, ou `None` sem o SDK na máquina.
fn main_de(fonte: &str) -> Option<String> {
    let ir = ir_de(fonte)?;
    let ini = ir.find("define void @dart_main(").expect("dart_main");
    let fim = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    Some(ir[ini..fim].to_string())
}

fn ir_de_fonte(fonte: &str) -> Option<String> {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB").unwrap_or_else(|_| SDK.to_string());
    if !Path::new(&sdk).join("libraries.json").is_file() { return None; }
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("main.dart");
    std::fs::write(&entrada, fonte).unwrap();
    Some(std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        let options = CompileOptions { sdk: Some(Path::new(&sdk)), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new() };
        emitir_ir_com(&entrada, &options, true).unwrap_or_else(|e| panic!("não compilou:\n{e}")).texto
    }).unwrap().join().unwrap())
}

#[test]
fn literal_symbol_usa_classe_do_sdk_e_constante_canonica() {
    let Some(ir) = ir_de_fonte("void main() { final a = #foo; final b = #foo; print(identical(a, b)); print(a); print(const Symbol('foo') == a); print(#_privado); }\n") else { return };
    let main = ir.split("define void @dart_main(").nth(1).expect("main");
    let main = main.split("\n}\n").next().expect("fim de main");
    assert!(main.matches(".get()").count() >= 2, "literal não canonizado:\n{main}");
    assert!(ir.contains("@dartforge_object_new"), "Symbol não alocado como objeto do SDK");
}

/// O erro de aridade de uma closure deve ser a classe real do SDK e ter um
/// `toString` utilizável pelo programa; o texto detalhado da VM ainda varia
/// conforme o nome e a assinatura da função.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn aridade_closure_lanca_no_such_method_error_da_fonte() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("arity_nsm.dart");
    let exe = dir.path().join("arity_nsm.exe");
    std::fs::write(&entrada, include_str!("fixtures/arity_nsm.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), "true\ntrue\n");
}

/// `C.getter(args)` avalia o getter antes dos argumentos, depois chama a
/// closure devolvida. Métodos estáticos comuns continuam com chamada direta.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn getter_estatico_que_retorna_funcao_e_chamado_como_valor() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("static_getter_function.dart");
    let exe = dir.path().join("static_getter_function.exe");
    std::fs::write(&entrada, include_str!("fixtures/static_getter_function.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "getter\narg\n42\n6\ngetter\n8\n");
}

/// A factory redirecionadora passa os tipos da anotação do destino, inclusive
/// quando muda de classe e permuta os parâmetros de tipo.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn factory_redirecionadora_preserva_rti_do_destino() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("redirect_factory_rti.dart");
    let exe = dir.path().join("redirect_factory_rti.exe");
    std::fs::write(&entrada, include_str!("fixtures/redirect_factory_rti.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\nMemory<String>\ntrue\nPairImpl<String, int>\n");
}

/// O getter abstrato de uma interface pode ser implementado por um campo ou
/// por um getter explícito; cada objeto escolhe sua própria implementação.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn getter_de_interface_despacha_campo_e_getter() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("interface_field_getter.dart");
    let exe = dir.path().join("interface_field_getter.exe");
    std::fs::write(&entrada, include_str!("fixtures/interface_field_getter.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "r2: bip r2\nloro: currupaco\n");
}

/// O argumento reificado da coleção, e não o tipo da referência covariante,
/// protege a mutação antes de alterar List, Map ou Set.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn escrita_covariante_em_colecoes_lanca_type_error_sem_mutar() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("covariant_collection_write.dart");
    let exe = dir.path().join("covariant_collection_write.exe");
    std::fs::write(&entrada, include_str!("fixtures/covariant_collection_write.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
               "true\n[1]\ntrue\n{a: 1}\ntrue\n{1}\n");
}

/// A chamada dinâmica deve conferir o tipo nominal do parâmetro do SDK antes
/// de passar um `int` como handle ao native `String_concat`.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn argumento_dinamico_errado_do_sdk_lanca_type_error() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("dynamic_sdk_string_argument.dart");
    let exe = dir.path().join("dynamic_sdk_string_argument.exe");
    std::fs::write(&entrada, include_str!("fixtures/dynamic_sdk_string_argument.dart")).unwrap();
    let exe_para_thread = exe.clone();
    std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let options = CompileOptions {
            sdk: Some(Path::new(&sdk)), packages: None, timings: false,
            optimize: false, versao_linguagem: None, experimentos: Vec::new(),
        };
        dartforge_emit_native::compilar_com(&entrada, &exe_para_thread, &options, true)
            .unwrap_or_else(|e| panic!("não compilou:\n{e}"));
    }).unwrap().join().unwrap();
    let output = std::process::Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), "foobar\ntrue\nfoo\n");
}

/// O getter separado impede que `late String x = x` expanda a própria AST
/// indefinidamente; o teste também fixa a checagem de reentrância por objeto.
#[test]
fn campo_late_auto_referente_emite_getter_com_guarda() {
    let Some(ir) = ir_de("class C { late String x = x; }\nvoid main() { print(C().x); }\n") else { return };
    assert!(ir.contains("@dartforge_late_field_initializing"), "{ir}");
    assert!(ir.contains("@dartforge_stack_overflow_error_new"), "{ir}");
    assert!(ir.contains("@dartforge_late_field_mark_initialized"), "{ir}");
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
    assert!(main.contains(".dobro(i64"), "{main}");
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
    assert!(main.contains("= call i64 @df."), "{main}");
    assert!(main.contains("@dartforge_print_i64(i64 %v"), "{main}");
    assert!(!main.contains("@dartforge_print_handle"), "{main}");
}
