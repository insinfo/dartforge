//! Binário `dartforge-lsp`: servidor LSP do DartForge por stdio.
//!
//! Uso: `dartforge-lsp [--stdio]` fala JSON-RPC no `stdin`/`stdout`;
//! `dartforge-lsp --version` imprime a versão. Todo log vai para `stderr`:
//! `stdout` é só protocolo, e um byte fora do enquadramento corrompe a
//! sessão do editor.
//!
//! Fios: uma thread lê `stdin` quadro a quadro ([`transport::read_message`])
//! e enfileira valores no [`Server`]; a thread principal despacha com
//! [`Server::pump`] e escreve cada mensagem de saída por inteiro sob um
//! `Mutex`. O servidor nunca envia requisições, então não há resposta a
//! tratar nem correlação de ids no caminho de volta. A análise tipada
//! (`docs/LSP.md`, "Diagnósticos tipados") roda numa terceira thread e
//! acorda o despacho com [`Entry::Wake`] quando termina.

use dartforge_lsp::{SemanticAnalyzer, Server};
use dartforge_lsp::transport::{write_message, read_message};
use serde_json::Value;
use std::io::{self, BufReader};
use std::sync::{Arc, Mutex, mpsc};

/// Mensagem da thread leitora para o despacho: quadro ou fim do fluxo.
enum Entry {
    /// Quadro JSON decodificado.
    Message(Value),
    /// `stdin` fechou ou falhou de forma irrecuperável.
    End,
    /// A análise tipada terminou: há diagnósticos a publicar.
    Wake,
}

fn main() {
    let code = run(std::env::args().skip(1).collect());
    std::process::exit(code);
}

/// Ponto de entrada testável: devolve o código de saída em vez de sair.
fn run(arguments: Vec<String>) -> i32 {
    if arguments.iter().any(|a| a == "--version" || a == "-V") {
        println!("dartforge-lsp {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    if arguments
        .iter()
        .any(|a| a == "--help" || a == "-h" || (!a.starts_with('-') && a != "--stdio"))
    {
        println!("Uso: dartforge-lsp [--stdio] [--version]");
        return 0;
    }
    serve_stdio()
}

/// Laço principal: enfileira pela leitora, despacha na principal, sai no `exit`.
fn serve_stdio() -> i32 {
    if std::env::var("RUST_LOG").is_ok() || std::env::var("DARTFORGE_LSP_DEBUG").is_ok() {
        eprintln!("[dartforge-lsp] log em stderr; stdout é só protocolo");
    }
    let (tx, rx) = mpsc::channel::<Entry>();
    let wake = tx.clone();
    std::thread::spawn(move || read_all(tx));

    let output = Arc::new(Mutex::new(io::stdout()));
    let mut server = Server::with_analyzer(SemanticAnalyzer::discover());
    // A análise tipada roda noutra thread e acorda o laço ao terminar.
    server.on_diagnostics(move || {
        let _ = wake.send(Entry::Wake);
    });
    let mut closed = false;
    loop {
        if !server.has_pending() && !closed {
            match rx.recv() {
                Ok(Entry::Message(message)) => {
                    server.register_received(&message);
                    server.receive(message);
                }
                Ok(Entry::Wake) => {}
                Ok(Entry::End) | Err(_) => closed = true,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(Entry::Message(message)) => {
                    server.register_received(&message);
                    server.receive(message);
                }
                Ok(Entry::End) => {
                    closed = true;
                    break;
                }
                Ok(Entry::Wake) => {}
                Err(_) => break,
            }
        }
        for response in server.pump() {
            let mut stdout = output.lock().expect("stdout não envenenado");
            if write_message(&mut *stdout, &response).is_err() {
                return 1;
            }
        }
        if server.must_shutdown() {
            break;
        }
        if closed && !server.has_pending() {
            break;
        }
    }
    server.exit_code()
}

/// Thread leitora: decodifica quadros até o fim do fluxo e os enfileira.
///
/// JSON inválido após `Content-Length` válido é registrado em `stderr` e a
/// leitura continua no próximo quadro: a fronteira seguinte é conhecida (o
/// corpo tem tamanho exato), então a sessão não corrompe.
fn read_all(tx: mpsc::Sender<Entry>) {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => {
                if tx.send(Entry::Message(message)).is_err() {
                    return;
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("[dartforge-lsp] quadro ignorado: {e}");
            }
        }
    }
    let _ = tx.send(Entry::End);
}
