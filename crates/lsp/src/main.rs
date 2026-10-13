//! The `dartforge-lsp` binary: DartForge's LSP server over stdio.
//!
//! Usage: `dartforge-lsp [--stdio]` speaks JSON-RPC on `stdin`/`stdout`;
//! `dartforge-lsp --version` prints the version. All logging goes to
//! `stderr`: `stdout` is protocol only, and one byte outside the framing
//! corrupts the editor session.
//!
//! Threads: one thread reads `stdin` frame by frame
//! ([`transport::read_message`]) and enqueues values in the [`Server`]; the
//! main thread dispatches with [`Server::pump`] and writes each outgoing
//! message whole under a `Mutex`. The server never sends requests, so there
//! is no response to handle nor id correlation on the way back. The typed
//! analysis (`docs/LSP.md`, "Diagnósticos tipados") runs in a third thread
//! and wakes the dispatcher with [`Entry::Wake`] when it finishes.

use dartforge_lsp::{SemanticAnalyzer, Server};
use dartforge_lsp::transport::{write_message, read_message};
use serde_json::Value;
use std::io::{self, BufReader};
use std::sync::{Arc, Mutex, mpsc};

/// Message from the reader thread to the dispatcher: a frame or end of stream.
enum Entry {
    /// Decoded JSON frame.
    Message(Value),
    /// `stdin` closed or failed unrecoverably.
    End,
    /// The typed analysis finished: there are diagnostics to publish.
    Wake,
}

fn main() {
    let code = run(std::env::args().skip(1).collect());
    std::process::exit(code);
}

/// Testable entry point: returns the exit code instead of exiting.
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

/// Main loop: enqueues from the reader, dispatches on the main thread, exits on `exit`.
fn serve_stdio() -> i32 {
    if std::env::var("RUST_LOG").is_ok() || std::env::var("DARTFORGE_LSP_DEBUG").is_ok() {
        eprintln!("[dartforge-lsp] log em stderr; stdout é só protocolo");
    }
    let (tx, rx) = mpsc::channel::<Entry>();
    let wake = tx.clone();
    std::thread::spawn(move || read_all(tx));

    let output = Arc::new(Mutex::new(io::stdout()));
    let mut server = Server::with_analyzer(SemanticAnalyzer::discover());
    // The typed analysis runs in another thread and wakes the loop when it finishes.
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

/// Reader thread: decodes frames until the end of the stream and enqueues them.
///
/// Invalid JSON after a valid `Content-Length` is logged to `stderr` and
/// reading continues with the next frame: the next boundary is known (the
/// body has an exact size), so the session is not corrupted.
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
