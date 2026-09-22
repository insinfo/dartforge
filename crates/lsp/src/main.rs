//! Binário `dartforge-lsp`: servidor LSP do DartForge por stdio.
//!
//! Uso: `dartforge-lsp [--stdio]` fala JSON-RPC no `stdin`/`stdout`;
//! `dartforge-lsp --version` imprime a versão. Todo log vai para `stderr`:
//! `stdout` é só protocolo, e um byte fora do enquadramento corrompe a
//! sessão do editor.
//!
//! Fios: uma thread lê `stdin` quadro a quadro ([`transporte::ler_mensagem`])
//! e enfileira valores no [`Servidor`]; a thread principal despacha com
//! [`Servidor::bombear`] e escreve cada mensagem de saída por inteiro sob um
//! `Mutex`. O servidor nunca envia requisições, então não há resposta a
//! tratar nem correlação de ids no caminho de volta.

use dartforge_lsp::Servidor;
use dartforge_lsp::transporte::{escrever_mensagem, ler_mensagem};
use serde_json::Value;
use std::io::{self, BufReader};
use std::sync::{Arc, Mutex, mpsc};

/// Mensagem da thread leitora para o despacho: quadro ou fim do fluxo.
enum Entrada {
    /// Quadro JSON decodificado.
    Mensagem(Value),
    /// `stdin` fechou ou falhou de forma irrecuperável.
    Fim,
}

fn main() {
    let codigo = executar(std::env::args().skip(1).collect());
    std::process::exit(codigo);
}

/// Ponto de entrada testável: devolve o código de saída em vez de sair.
fn executar(argumentos: Vec<String>) -> i32 {
    if argumentos.iter().any(|a| a == "--version" || a == "-V") {
        println!("dartforge-lsp {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    if argumentos
        .iter()
        .any(|a| a == "--help" || a == "-h" || (!a.starts_with('-') && a != "--stdio"))
    {
        println!("Uso: dartforge-lsp [--stdio] [--version]");
        return 0;
    }
    servir_stdio()
}

/// Laço principal: enfileira pela leitora, despacha na principal, sai no `exit`.
fn servir_stdio() -> i32 {
    if std::env::var("RUST_LOG").is_ok() || std::env::var("DARTFORGE_LSP_DEBUG").is_ok() {
        eprintln!("[dartforge-lsp] log em stderr; stdout é só protocolo");
    }
    let (tx, rx) = mpsc::channel::<Entrada>();
    std::thread::spawn(move || ler_tudo(tx));

    let saida = Arc::new(Mutex::new(io::stdout()));
    let mut servidor = Servidor::new();
    let mut fechou = false;
    loop {
        if !servidor.tem_pendente() && !fechou {
            match rx.recv() {
                Ok(Entrada::Mensagem(mensagem)) => {
                    servidor.registrar_recebida(&mensagem);
                    servidor.receber(mensagem);
                }
                Ok(Entrada::Fim) | Err(_) => fechou = true,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(Entrada::Mensagem(mensagem)) => {
                    servidor.registrar_recebida(&mensagem);
                    servidor.receber(mensagem);
                }
                Ok(Entrada::Fim) => {
                    fechou = true;
                    break;
                }
                Err(_) => break,
            }
        }
        for resposta in servidor.bombear() {
            let mut stdout = saida.lock().expect("stdout não envenenado");
            if escrever_mensagem(&mut *stdout, &resposta).is_err() {
                return 1;
            }
        }
        if servidor.deve_encerrar() {
            break;
        }
        if fechou && !servidor.tem_pendente() {
            break;
        }
    }
    servidor.codigo_saida()
}

/// Thread leitora: decodifica quadros até o fim do fluxo e os enfileira.
///
/// JSON inválido após `Content-Length` válido é registrado em `stderr` e a
/// leitura continua no próximo quadro: a fronteira seguinte é conhecida (o
/// corpo tem tamanho exato), então a sessão não corrompe.
fn ler_tudo(tx: mpsc::Sender<Entrada>) {
    let stdin = io::stdin();
    let mut leitor = BufReader::new(stdin.lock());
    loop {
        match ler_mensagem(&mut leitor) {
            Ok(Some(mensagem)) => {
                if tx.send(Entrada::Mensagem(mensagem)).is_err() {
                    return;
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("[dartforge-lsp] quadro ignorado: {e}");
            }
        }
    }
    let _ = tx.send(Entrada::Fim);
}
