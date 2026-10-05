//! O `grep` do CI do contrato C9 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
//! §4.6 e §13.15): com as exceções por tabelas, uma exceção Dart que
//! atravessasse um quadro Rust seria comportamento indefinido, e todo ponto
//! em que o runtime chama código Dart passa pelas portas
//! (`excecoes_tabelas.rs`, `dart_r<n>`/`dart_v<n>`). O sinal de um ponto
//! novo que chama direto é um `transmute` para ponteiro de função fora
//! desse arquivo.
//!
//! Os que restam chamam código C (finalizadores nativos e tratadores da API
//! de portas do `dart:ffi`, funções do sistema achadas em tempo de
//! execução) ou só convertem o endereço que segue pela porta; cada um está
//! na lista abaixo com o motivo. Um `transmute` novo fora dela derruba o
//! teste: ou ele passa pela porta, ou entra na lista com o motivo.
//! Escrito sem compilar nem executar (2026-10-05).

use std::path::Path;

/// (arquivo, trecho da linha, motivo).
const PERMITIDOS: &[(&str, &str, &str)] = &[
    ("ffi_api_nativa.rs", "let f: FinalizadorDeHandle = unsafe { std::mem::transmute(f) };", "o `Dart_HandleFinalizer` em C do chamador da API nativa"),
    ("ffi_api_nativa.rs", "let handler: HandlerDePortaNativa = unsafe { std::mem::transmute(handler) };", "o `Dart_NativeMessageHandler` em C"),
    ("heap.rs", "let f: extern \"C\" fn(usize) = unsafe { std::mem::transmute(f) };", "a `NativeFinalizerFunction` em C do `dart:ffi`"),
    ("io_observador.rs", "std::mem::transmute::<*mut c_void, _>(s(c\"FSEvent", "funções do CoreServices achadas em tempo de execução"),
    ("io_windows_eventos.rs", "std::mem::transmute::<usize, Fn", "as extensões do Winsock (`AcceptEx`, `ConnectEx`…) achadas pelo `WSAIoctl`"),
    ("isolados.rs", "let chamar: extern \"C\" fn(i64) -> i64 = unsafe { std::mem::transmute(chamar) };", "o endereço segue para o laço de eventos, que chama pela porta (`dart_r1`)"),
    ("portas.rs", "std::mem::transmute::<usize, extern \"C\" fn()>(p)", "os trampolins da geração publicada pelo JIT, que não tem as exceções por tabelas"),
];

#[test]
fn todo_transmute_para_funcao_fora_das_portas_esta_na_lista() {
    let fonte = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut novos = Vec::new();
    for entrada in std::fs::read_dir(&fonte).expect("ler crates/runtime/src") {
        let caminho = entrada.expect("entrada").path();
        let Some(nome) = caminho.file_name().and_then(|n| n.to_str()) else { continue };
        if !nome.ends_with(".rs") || nome == "excecoes_tabelas.rs" {
            continue;
        }
        let texto = std::fs::read_to_string(&caminho).expect("ler fonte");
        for (n, linha) in texto.lines().enumerate() {
            if !linha.contains("transmute") || linha.trim_start().starts_with("//") {
                continue;
            }
            let permitido = PERMITIDOS.iter().any(|(arquivo, trecho, _)| *arquivo == nome && linha.contains(trecho));
            if !permitido {
                novos.push(format!("{nome}:{}: {}", n + 1, linha.trim()));
            }
        }
    }
    assert!(
        novos.is_empty(),
        "`transmute` fora das portas e fora da lista (chame o código Dart por `dart_r<n>`/`dart_v<n>`, ou acrescente o caso com o motivo em PERMITIDOS):\n{}",
        novos.join("\n")
    );
}

/// A lista não guarda entrada morta: cada uma ainda casa com alguma linha.
#[test]
fn a_lista_nao_tem_entrada_morta() {
    let fonte = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (arquivo, trecho, motivo) in PERMITIDOS {
        let texto = std::fs::read_to_string(fonte.join(arquivo)).unwrap_or_default();
        assert!(texto.contains(trecho), "{arquivo}: a entrada ({motivo}) não casa mais com nada: tire-a da lista");
    }
}
