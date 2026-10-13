//! Platô de memória do servidor inteiro, através de mensagens JSON de verdade.
//!
//! É o `plateau_documents.rs` elevado ao protocolo: abre K documentos reais,
//! aplica K edições incrementais em cada um e fecha tudo, com cada mensagem
//! serializada em quadro `Content-Length`, decodificada e despachada pelo
//! [`dartforge_lsp::Server`] — nunca por chamada direta de API. O que esta
//! medição prova é o que o PLANO.md exige: N edições sucessivas estabilizam
//! `live_bytes` num platô em vez de crescer com N (o modo de falha do LSP do
//! Dart, que só sobe até ser preciso matar o processo).
//!
//! Binário separado de propósito: o alocador contador é global ao processo e
//! o harness roda os `#[test]` de um binário em threads paralelas, liberando
//! estruturas de testes terminados em momento não determinístico. Qualquer
//! outro teste neste binário mediria junto — por isso há um só `#[test]`
//! aqui (a razão documentada em `plateau_documents.rs`).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::Server;
use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::BufReader;

/// Documentos por rodada, como no teste do dono dos documentos.
const K: usize = 24;

/// Corpus padrão: o projeto de referência do proprietário.
const DEFAULT_CORPUS: &str = "C:/MyDartProjects/new_sali";

/// Envia `message` pelo transporte de verdade: serializa em quadro, lê o
/// quadro de volta e despacha; as saídas são descartadas como transitórias.
fn swap(server: &mut Server, message: &Value) {
    let framed = frame(message);
    let mut reader = BufReader::new(&framed[..]);
    let read = read_message(&mut reader)
        .expect("quadro gerado aqui mesmo decodifica")
        .expect("quadro não vazio");
    server.receive(read);
    let outputs = server.pump();
    // Toca as saídas para que nada seja otimizado para fora da medição.
    let mut bytes = 0usize;
    for output in &outputs {
        bytes += frame(output).len();
    }
    std::hint::black_box(bytes);
}

/// Coleta até K arquivos `.dart` do corpus, ordenados para repetibilidade.
fn collect_corpus() -> Vec<(String, String)> {
    let root = std::env::var("DARTFORGE_CORPUS").unwrap_or_else(|_| DEFAULT_CORPUS.to_string());
    let mut files = Vec::new();
    collect_dir(std::path::Path::new(&root), &mut files);
    files.sort();
    files.truncate(K);
    let mut docs = Vec::new();
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let uri = format!("file:///plato/{}", docs.len());
            docs.push((uri, text));
        }
    }
    docs
}

/// Varre o diretório ignorando saídas de build, como o exemplo de medição.
fn collect_dir(dir: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            collect_dir(&path, output);
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
}

/// Reserva quando o corpus não está ao alcance (CI sem o projeto de
/// referência): textos sintéticos de tamanho fixo por documento e revisão.
fn synthetic() -> Vec<(String, String)> {
    (0..K)
        .map(|d| {
            (
                format!("file:///plato/doc{d}.dart"),
                format!(
                    "int f{d}(int n) {{ var soma = 0; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\nvoid main() {{ print(f{d}(3)); }}\n",
                ),
            )
        })
        .collect()
}

/// O `initialize`/`initialized`: o estado da sessão (capacidades,
/// configuração) é do servidor, não dos documentos, e fica na base.
fn initialize(server: &mut Server) {
    swap(
        server,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    swap(
        server,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
}

/// Abre os documentos pelo protocolo e aplica K rodadas de edição incremental
/// em cada um, devolvendo os bytes vivos após cada rodada.
fn edit(server: &mut Server, docs: &[(String, String)]) -> Vec<usize> {
    for (uri, text) in docs {
        swap(
            server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}},
            }),
        );
    }
    assert_eq!(server.open_documents(), docs.len());
    let mut live = Vec::with_capacity(K);
    for round in 1..=K {
        for (uri, _) in docs {
            // Alterna inserir e remover um espaço no início da linha 0: o
            // texto retido tem tamanho constante e a edição é incremental
            // (intervalo UTF-16), como a tecla de um editor.
            let change = if round % 2 == 1 {
                json!([{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                    "text": " ",
                }])
            } else {
                json!([{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
                    "text": "",
                }])
            };
            swap(
                server,
                &json!({
                    "jsonrpc": "2.0", "method": "textDocument/didChange",
                    "params": {"textDocument": {"uri": uri, "version": round as i32 + 1}, "contentChanges": change},
                }),
            );
        }
        live.push(dartforge_instrument::live_bytes());
    }
    live
}

/// Fecha todos os documentos pelo protocolo.
fn close(server: &mut Server, docs: &[(String, String)]) {
    for (uri, _) in docs {
        swap(
            server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": uri}},
            }),
        );
    }
    assert_eq!(server.open_documents(), 0);
}

/// `live_bytes` estabiliza ao longo das rodadas, e fechar tudo devolve ao
/// nível de logo depois do `initialize`: nada das edições, versões ou
/// diagnósticos sobrevive.
///
/// Num só `#[test]`: ver o comentário no topo do arquivo.
#[test]
fn plateau_through_protocol() {
    let docs = collect_corpus();
    let real_docs = docs.len() == K;
    let docs = if real_docs { docs } else { synthetic() };
    // Aquecimento: um servidor descartável faz o ciclo inteiro uma vez, para
    // que alocações únicas do processo (estado preguiçoso da biblioteca
    // padrão e do harness, como o buffer de captura da saída do teste)
    // fiquem fora da base. O que se mede é retenção por documento e por
    // edição, não inicialização do processo.
    {
        let mut warmup = Server::new();
        initialize(&mut warmup);
        edit(&mut warmup, &docs[..1]);
        close(&mut warmup, &docs[..1]);
    }
    let mut server = Server::new();
    let empty = dartforge_instrument::live_bytes();
    initialize(&mut server);
    let before = dartforge_instrument::live_bytes();

    let live = edit(&mut server, &docs);
    assert_eq!(live.len(), K);
    let reference = live[4..8].iter().max().copied().unwrap();
    let after = live[8..].iter().max().copied().unwrap();
    assert!(
        after <= reference.saturating_add(8 * 1024),
        "memória cresceu com as edições: platô {reference} bytes, cauda {after} bytes"
    );

    close(&mut server, &docs);
    let closed = dartforge_instrument::live_bytes();
    assert!(
        closed <= before.saturating_add(K * 192),
        "fechar não liberou os documentos: antes {before}, depois {closed}"
    );
    // Os textos do harness também retêm memória: solta-os junto com o
    // servidor antes de afirmar o retorno ao nível inicial.
    drop(docs);
    drop(server);
    assert!(dartforge_instrument::live_bytes() <= empty.saturating_add(64));

    if !real_docs {
        eprintln!("aviso: corpus indisponível, platô medido com documentos sintéticos");
    }
}
