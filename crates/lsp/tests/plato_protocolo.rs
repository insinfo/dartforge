//! Platô de memória do servidor inteiro, através de mensagens JSON de verdade.
//!
//! É o `plato_documentos.rs` elevado ao protocolo: abre K documentos reais,
//! aplica K edições incrementais em cada um e fecha tudo, com cada mensagem
//! serializada em quadro `Content-Length`, decodificada e despachada pelo
//! [`dartforge_lsp::Servidor`] — nunca por chamada direta de API. O que esta
//! medição prova é o que o PLANO.md exige: N edições sucessivas estabilizam
//! `live_bytes` num platô em vez de crescer com N (o modo de falha do LSP do
//! Dart, que só sobe até ser preciso matar o processo).
//!
//! Binário separado de propósito: o alocador contador é global ao processo e
//! o harness roda os `#[test]` de um binário em threads paralelas, liberando
//! estruturas de testes terminados em momento não determinístico. Qualquer
//! outro teste neste binário mediria junto — por isso há um só `#[test]`
//! aqui (a razão documentada em `plato_documentos.rs`).
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::Servidor;
use dartforge_lsp::transporte::{enquadrar, ler_mensagem};
use serde_json::{Value, json};
use std::io::BufReader;

/// Documentos por rodada, como no teste do dono dos documentos.
const K: usize = 24;

/// Corpus padrão: o projeto de referência do proprietário.
const CORPUS_PADRAO: &str = "C:/MyDartProjects/new_sali";

/// Envia `mensagem` pelo transporte de verdade: serializa em quadro, lê o
/// quadro de volta e despacha; as saídas são descartadas como transitórias.
fn trocar(servidor: &mut Servidor, mensagem: &Value) {
    let quadro = enquadrar(mensagem);
    let mut leitor = BufReader::new(&quadro[..]);
    let lida = ler_mensagem(&mut leitor)
        .expect("quadro gerado aqui mesmo decodifica")
        .expect("quadro não vazio");
    servidor.receber(lida);
    let saidas = servidor.bombear();
    // Toca as saídas para que nada seja otimizado para fora da medição.
    let mut bytes = 0usize;
    for saida in &saidas {
        bytes += enquadrar(saida).len();
    }
    std::hint::black_box(bytes);
}

/// Coleta até K arquivos `.dart` do corpus, ordenados para repetibilidade.
fn coletar_corpus() -> Vec<(String, String)> {
    let raiz = std::env::var("DARTFORGE_CORPUS").unwrap_or_else(|_| CORPUS_PADRAO.to_string());
    let mut arquivos = Vec::new();
    coletar_dir(std::path::Path::new(&raiz), &mut arquivos);
    arquivos.sort();
    arquivos.truncate(K);
    let mut docs = Vec::new();
    for caminho in arquivos {
        if let Ok(texto) = std::fs::read_to_string(&caminho) {
            let uri = format!("file:///plato/{}", docs.len());
            docs.push((uri, texto));
        }
    }
    docs
}

/// Varre o diretório ignorando saídas de build, como o exemplo de medição.
fn coletar_dir(dir: &std::path::Path, saida: &mut Vec<std::path::PathBuf>) {
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
            coletar_dir(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

/// Reserva quando o corpus não está ao alcance (CI sem o projeto de
/// referência): textos sintéticos de tamanho fixo por documento e revisão.
fn sinteticos() -> Vec<(String, String)> {
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

/// Abre os documentos pelo protocolo e aplica K rodadas de edição incremental
/// em cada um, devolvendo os bytes vivos após cada rodada.
fn editar(servidor: &mut Servidor, docs: &[(String, String)]) -> Vec<usize> {
    trocar(
        servidor,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    trocar(
        servidor,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    for (uri, texto) in docs {
        trocar(
            servidor,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": texto}},
            }),
        );
    }
    assert_eq!(servidor.documentos_abertos(), docs.len());
    let mut vivos = Vec::with_capacity(K);
    for rodada in 1..=K {
        for (uri, _) in docs {
            // Alterna inserir e remover um espaço no início da linha 0: o
            // texto retido tem tamanho constante e a edição é incremental
            // (intervalo UTF-16), como a tecla de um editor.
            let mudanca = if rodada % 2 == 1 {
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
            trocar(
                servidor,
                &json!({
                    "jsonrpc": "2.0", "method": "textDocument/didChange",
                    "params": {"textDocument": {"uri": uri, "version": rodada as i32 + 1}, "contentChanges": mudanca},
                }),
            );
        }
        vivos.push(dartforge_instrument::live_bytes());
    }
    vivos
}

/// Fecha todos os documentos pelo protocolo.
fn fechar(servidor: &mut Servidor, docs: &[(String, String)]) {
    for (uri, _) in docs {
        trocar(
            servidor,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": uri}},
            }),
        );
    }
    assert_eq!(servidor.documentos_abertos(), 0);
}

/// `live_bytes` estabiliza ao longo das rodadas, e fechar tudo devolve ao
/// nível inicial: nada das edições, versões ou diagnósticos sobrevive.
///
/// Num só `#[test]`: ver o comentário no topo do arquivo.
#[test]
fn plato_pelo_protocolo() {
    let docs = coletar_corpus();
    let docs_reais = docs.len() == K;
    let docs = if docs_reais { docs } else { sinteticos() };
    let mut servidor = Servidor::new();
    let antes = dartforge_instrument::live_bytes();

    let vivos = editar(&mut servidor, &docs);
    assert_eq!(vivos.len(), K);
    let referencia = vivos[4..8].iter().max().copied().unwrap();
    let depois = vivos[8..].iter().max().copied().unwrap();
    assert!(
        depois <= referencia.saturating_add(8 * 1024),
        "memória cresceu com as edições: platô {referencia} bytes, cauda {depois} bytes"
    );

    fechar(&mut servidor, &docs);
    let fechado = dartforge_instrument::live_bytes();
    assert!(
        fechado <= antes.saturating_add(K * 192),
        "fechar não liberou os documentos: antes {antes}, depois {fechado}"
    );
    // Os textos do harness também retêm memória: solta-os junto com o
    // servidor antes de afirmar o retorno ao nível inicial.
    drop(docs);
    drop(servidor);
    assert!(dartforge_instrument::live_bytes() <= antes.saturating_add(64));

    if !docs_reais {
        eprintln!("aviso: corpus indisponível, platô medido com documentos sintéticos");
    }
}
