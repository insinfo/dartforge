//! Mede memória e tempo do servidor LSP sobre um projeto inteiro, pelo protocolo.
//!
//! `cargo run -q --release -p dartforge-lsp --example memoria -- C:/MyDartProjects/new_sali`
//!
//! Abre todos os `.dart` do projeto com quadros `didOpen` de verdade,
//! aplica uma edição incremental em cada um e fecha tudo, imprimindo bytes
//! de fonte, bytes vivos com tudo retido, pico, alocações e tempo. É o par
//! da medição do front-end (`dartforge-frontend --example memoria`), com a
//! diferença de que aqui o custo inclui transporte, documentos e
//! diagnósticos publicados — o que um editor faz com o projeto aberto.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::Servidor;
use dartforge_lsp::transporte::{enquadrar, ler_mensagem};
use serde_json::{Value, json};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            if caminho
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

/// Uma ida pelo transporte: serializa, decodifica e despacha.
fn trocar(servidor: &mut Servidor, mensagem: &Value) -> usize {
    let quadro = enquadrar(mensagem);
    let mut leitor = BufReader::new(&quadro[..]);
    let lida: Value = ler_mensagem(&mut leitor)
        .expect("quadro válido")
        .expect("quadro não vazio");
    servidor.receber(lida);
    servidor
        .bombear()
        .iter()
        .map(enquadrar)
        .map(|q| q.len())
        .sum()
}

fn main() {
    let mut arquivos = Vec::new();
    for raiz in std::env::args().skip(1) {
        coletar(Path::new(&raiz), &mut arquivos);
    }
    arquivos.sort();
    let inicio = Instant::now();
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let alocacoes_base = dartforge_instrument::allocation_count();
    let mut servidor = Servidor::new();
    trocar(
        &mut servidor,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    let mut bytes_fonte = 0usize;
    let mut bytes_saida = 0usize;
    let mut uris = Vec::with_capacity(arquivos.len());
    for (i, arquivo) in arquivos.iter().enumerate() {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        bytes_fonte += fonte.len();
        let uri = format!("file:///memoria/{i}.dart");
        bytes_saida += trocar(
            &mut servidor,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": fonte}},
            }),
        );
        uris.push(uri);
    }
    // Uma edição incremental por documento: insere um espaço na linha 0.
    for uri in &uris {
        bytes_saida += trocar(
            &mut servidor,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri, "version": 2},
                    "contentChanges": [{
                        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                        "text": " ",
                    }],
                },
            }),
        );
    }
    let tempo = inicio.elapsed();
    let vivos = dartforge_instrument::live_bytes() - base;
    let pico = dartforge_instrument::peak_bytes() - base;
    let alocacoes = dartforge_instrument::allocation_count() - alocacoes_base;
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    println!("arquivos abertos: {}", uris.len());
    println!("fonte: {:.2} MiB", mib(bytes_fonte));
    println!(
        "vivo com tudo retido: {:.2} MiB ({:.2}x a fonte)",
        mib(vivos),
        vivos as f64 / bytes_fonte.max(1) as f64
    );
    println!("pico: {:.2} MiB", mib(pico));
    println!("bytes publicados em diagnósticos: {bytes_saida}");
    println!(
        "alocações: {alocacoes}; tempo: {:.0} ms ({:.1} MiB/s)",
        tempo.as_secs_f64() * 1e3,
        mib(bytes_fonte) / tempo.as_secs_f64().max(1e-9)
    );
    for uri in &uris {
        trocar(
            &mut servidor,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": uri}},
            }),
        );
    }
    drop(uris);
    drop(servidor);
    println!(
        "após fechar tudo: {} bytes acima da base",
        dartforge_instrument::live_bytes().saturating_sub(base)
    );
}
