//! Transporte JSON-RPC por stdio: enquadramento `Content-Length`.
//!
//! O desenho copia o do rust-analyzer (`main_loop.rs`): uma thread lê
//! `stdin` quadro a quadro e enfileira valores JSON; a thread principal
//! despacha; respostas e notificações saem por `stdout` sob um `Mutex`, uma
//! mensagem inteira por vez. **`stdout` é só protocolo**: este módulo nunca
//! escreve nada além de quadros, e nenhum log passa por aqui (logs vão para
//! `stderr` no binário).
//!
//! Um quadro é um cabeçalho ASCII (`Content-Length: N`, mais cabeçalhos
//! ignorados como `Content-Type`), uma linha vazia e exatamente N bytes de
//! JSON. Nada fora desse formato sai em `stdout` — o teste `clean_stdout`
//! decodifica a sessão inteira quadro a quadro para provar.
//!
//! ```
//! use serde_json::json;
//! let frame = dartforge_lsp::transport::frame(&json!({"jsonrpc": "2.0", "id": 1, "method": "shutdown"}));
//! let mut reader = std::io::BufReader::new(&frame[..]);
//! let read = dartforge_lsp::transport::read_message(&mut reader).unwrap().unwrap();
//! assert_eq!(read["method"], "shutdown");
//! ```

use serde_json::Value;
use std::io::{self, BufRead, Write};

/// Lê um quadro de `reader`; `None` no fim do fluxo (cliente foi embora).
///
/// Erro de E/S ou cabeçalho malformado aborta a leitura com erro; JSON
/// inválido após um `Content-Length` válido também é erro — nunca se tenta
/// ressincronizar adivinhando fronteiras, porque isso corrompe a sessão.
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut size: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            size = Some(value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Content-Length inválido")
            })?);
        }
    }
    let size = size
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quadro sem Content-Length"))?;
    let mut body = vec![0u8; size];
    reader.read_exact(&mut body)?;
    let value: Value = serde_json::from_slice(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(value))
}

/// Escreve `message` como um quadro: cabeçalho, linha vazia e corpo.
///
/// O chamador escreve o quadro inteiro sob um `Mutex`, para que duas
/// respostas nunca se intercalem no `stdout`.
pub fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body =
        serde_json::to_vec(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

/// Serializa `message` num quadro em memória (testes e medição pelo protocolo).
pub fn frame(message: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    write_message(&mut output, message).expect("serializar para Vec não falha");
    output
}

/// Decodifica todos os quadros de `bytes`, recusando qualquer byte fora do
/// enquadramento. É o que o teste `clean_stdout` afirma sobre a sessão.
pub fn decode_all(mut bytes: &[u8]) -> io::Result<Vec<Value>> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let mut reader = bytes;
        let message = read_message(&mut reader)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "bytes restantes sem quadro")
        })?;
        let consumed = bytes.len() - reader.len();
        messages.push(message);
        bytes = &bytes[consumed..];
    }
    Ok(messages)
}
