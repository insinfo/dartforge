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
const ALLOWED: &[(&str, &str, &str)] = &[
    ("arc_native_ports.rs", "let test_body: extern \"C\" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(target as usize) };", "somente teste cfg(test): alvo conferido igual ao corpo Rust estático extern C, que apenas retém a referência e não lança nem chama Dart"),
    ("ffi_api_nativa.rs", "let f: HandleFinalizer = unsafe { std::mem::transmute(f) };", "o `Dart_HandleFinalizer` em C do chamador da API nativa"),
    ("ffi_api_nativa.rs", "let handler: NativePortHandler = unsafe { std::mem::transmute(handler) };", "o `Dart_NativeMessageHandler` em C"),
    ("heap.rs", "let f: extern \"C\" fn(usize) = unsafe { std::mem::transmute(f) };", "a `NativeFinalizerFunction` em C do `dart:ffi`"),
    ("io_observador.rs", "std::mem::transmute::<*mut c_void, _>(s(c\"FSEvent", "funções do CoreServices achadas em tempo de execução"),
    ("io_windows_eventos.rs", "std::mem::transmute::<usize, Fn", "as extensões do Winsock (`AcceptEx`, `ConnectEx`…) achadas pelo `WSAIoctl`"),
    ("isolates.rs", "let call: extern \"C\" fn(i64) -> i64 = unsafe { std::mem::transmute(call) };", "o endereço segue para o laço de eventos, que chama pela porta (`dart_r1`)"),
    ("ports.rs", "std::mem::transmute::<usize, extern \"C\" fn()>(p)", "os trampolins da geração publicada pelo JIT, que não tem as exceções por tabelas"),
];

#[test]
fn every_fn_transmute_outside_ports_is_listed() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut new_ones = Vec::new();
    for input in std::fs::read_dir(&source).expect("ler crates/runtime/src") {
        let file_path = input.expect("entrada").path();
        let Some(name) = file_path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.ends_with(".rs") || name == "exceptions_tables.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&file_path).expect("ler fonte");
        for (n, line) in text.lines().enumerate() {
            if !line.contains("transmute") || line.trim_start().starts_with("//") {
                continue;
            }
            let allowed = ALLOWED.iter().any(|(file, chunk, _)| *file == name && line.contains(chunk));
            if !allowed {
                new_ones.push(format!("{name}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        new_ones.is_empty(),
        "`transmute` fora das portas e fora da lista (chame o código Dart por `dart_r<n>`/`dart_v<n>`, ou acrescente o caso com o motivo em PERMITIDOS):\n{}",
        new_ones.join("\n")
    );
}

/// A lista não guarda entrada morta: cada uma ainda casa com alguma linha.
#[test]
fn the_list_has_no_dead_entry() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (file, chunk, reason) in ALLOWED {
        let text = std::fs::read_to_string(source.join(file)).unwrap_or_default();
        assert!(text.contains(chunk), "{file}: a entrada ({reason}) não casa mais com nada: tire-a da lista");
    }
}
