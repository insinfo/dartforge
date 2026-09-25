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

/// `Iterable.generate<E>` usa `id is E Function(int)` para decidir se o
/// gerador implícito de inteiros pode atender a `E`.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn iterable_generate_testa_assinatura_generica_no_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("iterable_generate_function_rti.dart");
    let exe = dir.path().join("iterable_generate_function_rti.exe");
    std::fs::write(&entrada, include_str!("fixtures/iterable_generate_function_rti.dart")).unwrap();
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
               "[0, 1, 2]\ntrue\n[v0, v1]\n");
}

/// `name`/`index` implícitos (`$name`, `index`, `this.name`) no corpo de um
/// membro de enum do programa (especificação §13 "Enums"): o elemento
/// resolvido é o getter implícito, sem corpo — o valor mora nos dois campos
/// implícitos do objeto (posições 1 e 0), os mesmos que o acesso explícito
/// (`E.a.name`, `membro_de_enum`) lê. Sem Clang: só a emissão.
#[test]
fn enum_name_index_implicito_le_os_campos_do_valor() {
    let fonte = "enum E { a, b; String d() => 'v=$name'; int i() => index + 1; }\n\
        void main() { print(E.a.d()); print(E.b.i()); }\n";
    let Some(ir) = ir_de_fonte(fonte) else { return };
    assert!(
        ir.contains("@dartforge_object_get(i64 %") && ir.contains(", i64 1)"),
        "o $name implícito lê o campo 1:\n{ir}"
    );
    assert!(ir.contains(", i64 0)"), "o index implícito lê o campo 0:\n{ir}");
}

/// O programa acima executado (AOT com SDK da fonte): implícito e explícito
/// dão o mesmo texto da VM.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn enum_name_index_implicito_no_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("enum_name_index_implicito.dart");
    let exe = dir.path().join("enum_name_index_implicito.exe");
    std::fs::write(&entrada, include_str!("fixtures/enum_name_index_implicito.dart")).unwrap();
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
               "terra:1\nmercurio:0\nterra\n0\nterra\n0\n");
}

/// `values` implícito (sem o prefixo `E.`) no corpo de um membro de enum:
/// a lista dos valores, como o caminho explícito. O getter implícito não
/// tem corpo — antes saía uma chamada a um símbolo nunca definido, e a
/// ligação falhava. Sem Clang: a emissão não deixa chamada a `.values()`.
#[test]
fn enum_values_implicito_vira_a_lista_dos_valores() {
    let fonte = "enum E { a, b; E get proximo => values[(index + 1) % 2]; static E primeiro() => values.first; }\n\
        void main() { print(E.a.proximo); print(E.primeiro()); }\n";
    let Some(ir) = ir_de_fonte(fonte) else { return };
    let chamadas = ir
        .lines()
        .filter(|l| l.contains("call ") && l.contains(".values("))
        .collect::<Vec<_>>();
    assert!(chamadas.is_empty(), "chamada a values sem define:\n{chamadas:?}\n{ir}");
}

/// O programa acima executado (AOT com SDK da fonte).
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn enum_values_implicito_no_sdk() {    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("enum_values_implicito.dart");
    let exe = dir.path().join("enum_values_implicito.exe");
    std::fs::write(&entrada, include_str!("fixtures/enum_values_implicito.dart")).unwrap();
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
               "E.b\nE.a\nE.a\n2\n");
}

/// `toString()` padrão de instância de classe genérica inclui os argumentos
/// (`Instance of 'Caixa<int>'`, como a VM); sem argumentos, o nome
/// registrado, como antes. Só execução prova (o runtime monta o texto).
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn classe_generica_tostring_padrao_no_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("classe_generica_tostring_padrao.dart");
    let exe = dir.path().join("classe_generica_tostring_padrao.exe");
    std::fs::write(&entrada, include_str!("fixtures/classe_generica_tostring_padrao.dart")).unwrap();
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
               "true\nInstance of 'Caixa<String>'\nInstance of 'Caixa<Caixa<int>>'\ntrue\n");
}

/// Encaminhador `noSuchMethod` estático de método só com posicionais
/// (casos 57/216/223, primeira fatia): `p.saudacao('mundo')` com
/// `Servico.saudacao` abstrato e `Proxy.noSuchMethod` monta
/// `Invocation.method(#saudacao, ['mundo'])` e chama o nsm — antes era
/// "chamada de membro sem implementação compilada". Sem Clang: só a emissão.
#[test]
fn nsm_encaminhador_metodo_posicional_emite_nsm() {
    let fonte = "abstract class Servico { String saudacao(String quem); }\n\
        class Proxy implements Servico {\n\
        \x20 dynamic noSuchMethod(Invocation i) => 'nsm(${i.positionalArguments.join(',')})';\n\
        }\n\
        void main() { final p = Proxy(); print(p.saudacao('mundo')); }\n";
    let Some(ir) = ir_de_fonte(fonte) else { return };
    assert!(ir.contains("noSuchMethod"), "sem chamada ao nsm:\n{ir}");
}

/// O programa acima executado (AOT com SDK da fonte): a saída bate com a VM 3.6.2.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn nsm_encaminhador_metodo_no_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("nsm_encaminhador_metodo.dart");
    let exe = dir.path().join("nsm_encaminhador_metodo.exe");
    std::fs::write(&entrada, include_str!("fixtures/nsm_encaminhador_metodo.dart")).unwrap();
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
               "nsm(mundo)\nSymbol(\"saudacao\")\n");
}

/// Encaminhador `noSuchMethod` estático de getter (caso 216, `p.versao`):
/// o getter abstrato sem implementação concreta monta
/// `Invocation.getter(#versao)` e chama o nsm — antes era "chamada de
/// membro sem implementação compilada". Sem Clang: só a emissão.
#[test]
fn nsm_encaminhador_getter_emite_nsm() {
    let fonte = "abstract class Servico { int get versao; }\n\
        class Proxy implements Servico {\n\
        \x20 dynamic noSuchMethod(Invocation i) => 42;\n\
        }\n\
        void main() { final p = Proxy(); print(p.versao); }\n";
    let Some(ir) = ir_de_fonte(fonte) else { return };
    assert!(ir.contains("noSuchMethod"), "sem chamada ao nsm:\n{ir}");
}

/// O programa acima executado (AOT com SDK da fonte): a saída bate com a VM 3.6.2.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn nsm_encaminhador_getter_no_sdk() {
    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("nsm_encaminhador_getter.dart");
    let exe = dir.path().join("nsm_encaminhador_getter.exe");
    std::fs::write(&entrada, include_str!("fixtures/nsm_encaminhador_getter.dart")).unwrap();
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
               "42\n");
}

/// Enum do programa é subtipo do `Enum` do SDK (especificação §13): a
/// aresta vai no registro do módulo do programa, e `is`/`as` a enxergam.
#[test]
#[ignore = "fixture AOT com SDK da fonte e LLVM; rodada no Pesado"]
fn enum_e_subtipo_de_enum_no_sdk() {    let sdk = std::env::var("DARTFORGE_TEST_SDK_LIB")
        .or_else(|_| std::env::var("DARTFORGE_SDK_LIB"))
        .expect("SDK de teste");
    let dir = tempfile::tempdir().unwrap();
    let entrada = dir.path().join("enum_e_subtipo_de_enum.dart");
    let exe = dir.path().join("enum_e_subtipo_de_enum.exe");
    std::fs::write(&entrada, include_str!("fixtures/enum_e_subtipo_de_enum.dart")).unwrap();
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
               "true\nfalse\ntrue\nfalse\n");
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
