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
//! JSON. Nada fora desse formato sai em `stdout` — o teste `stdout_limpo`
//! decodifica a sessão inteira quadro a quadro para provar.
//!
//! ```
//! use serde_json::json;
//! let quadro = dartforge_lsp::transporte::enquadrar(&json!({"jsonrpc": "2.0", "id": 1, "method": "shutdown"}));
//! let mut leitor = std::io::BufReader::new(&quadro[..]);
//! let lida = dartforge_lsp::transporte::ler_mensagem(&mut leitor).unwrap().unwrap();
//! assert_eq!(lida["method"], "shutdown");
//! ```

use serde_json::Value;
use std::io::{self, BufRead, Write};

/// Lê um quadro de `leitor`; `None` no fim do fluxo (cliente foi embora).
///
/// Erro de E/S ou cabeçalho malformado aborta a leitura com erro; JSON
/// inválido após um `Content-Length` válido também é erro — nunca se tenta
/// ressincronizar adivinhando fronteiras, porque isso corrompe a sessão.
pub fn ler_mensagem(leitor: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut tamanho: Option<usize> = None;
    loop {
        let mut linha = String::new();
        let lidos = leitor.read_line(&mut linha)?;
        if lidos == 0 {
            return Ok(None);
        }
        let linha = linha.trim();
        if linha.is_empty() {
            break;
        }
        if let Some(valor) = linha.strip_prefix("Content-Length:") {
            tamanho = Some(valor.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Content-Length inválido")
            })?);
        }
    }
    let tamanho = tamanho
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quadro sem Content-Length"))?;
    let mut corpo = vec![0u8; tamanho];
    leitor.read_exact(&mut corpo)?;
    let valor: Value = serde_json::from_slice(&corpo)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(valor))
}

/// Escreve `mensagem` como um quadro: cabeçalho, linha vazia e corpo.
///
/// O chamador escreve o quadro inteiro sob um `Mutex`, para que duas
/// respostas nunca se intercalem no `stdout`.
pub fn escrever_mensagem(escritor: &mut impl Write, mensagem: &Value) -> io::Result<()> {
    let corpo =
        serde_json::to_vec(mensagem).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write!(escritor, "Content-Length: {}\r\n\r\n", corpo.len())?;
    escritor.write_all(&corpo)?;
    escritor.flush()?;
    Ok(())
}

/// Serializa `mensagem` num quadro em memória (testes e medição pelo protocolo).
pub fn enquadrar(mensagem: &Value) -> Vec<u8> {
    let mut saida = Vec::new();
    escrever_mensagem(&mut saida, mensagem).expect("serializar para Vec não falha");
    saida
}

/// Decodifica todos os quadros de `bytes`, recusando qualquer byte fora do
/// enquadramento. É o que o teste `stdout_limpo` afirma sobre a sessão.
pub fn decodificar_tudo(mut bytes: &[u8]) -> io::Result<Vec<Value>> {
    let mut mensagens = Vec::new();
    while !bytes.is_empty() {
        let mut leitor = bytes;
        let mensagem = ler_mensagem(&mut leitor)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "bytes restantes sem quadro")
        })?;
        let consumidos = bytes.len() - leitor.len();
        mensagens.push(mensagem);
        bytes = &bytes[consumidos..];
    }
    Ok(mensagens)
}
