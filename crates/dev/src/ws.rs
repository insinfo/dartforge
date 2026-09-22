//! WebSocket mínimo do servidor de desenvolvimento (RFC 6455).
//!
//! O `webdev` recarrega o navegador por um canal bidirecional — o `dwds` é
//! agnóstico de transporte e aceita SSE **ou** WebSocket
//! (`dwds/lib/src/handlers/socket_connections.dart`). Aqui é WebSocket.
//!
//! Só o necessário para mandar texto do servidor ao navegador: o aperto de
//! mão (§4.2.2) e quadros de texto sem máscara (§5.2, o servidor não
//! mascara). O SHA-1 é exigido pelo protocolo — não há escolha de algoritmo —
//! e está verificado contra o vetor do próprio RFC (§1.3).
use std::io::Write;
use std::net::TcpStream;

/// GUID fixo do protocolo (RFC 6455 §1.3).
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Valor do cabeçalho `Sec-WebSocket-Accept` para a chave do cliente.
pub fn aceite(chave: &str) -> String {
    let mut entrada = String::with_capacity(chave.len() + GUID.len());
    entrada.push_str(chave);
    entrada.push_str(GUID);
    base64(&sha1(entrada.as_bytes()))
}

/// Responde o aperto de mão; a conexão passa a falar quadros.
pub fn apertar_mao(fluxo: &mut TcpStream, chave: &str) -> std::io::Result<()> {
    let resposta = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        aceite(chave)
    );
    fluxo.write_all(resposta.as_bytes())?;
    fluxo.flush()
}

/// Escreve um quadro de texto (opcode 1, FIN, sem máscara).
pub fn quadro_texto(fluxo: &mut TcpStream, texto: &str) -> std::io::Result<()> {
    let dados = texto.as_bytes();
    let mut quadro = Vec::with_capacity(dados.len() + 10);
    quadro.push(0x81);
    match dados.len() {
        n if n < 126 => quadro.push(n as u8),
        n if n <= u16::MAX as usize => {
            quadro.push(126);
            quadro.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            quadro.push(127);
            quadro.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    quadro.extend_from_slice(dados);
    fluxo.write_all(&quadro)?;
    fluxo.flush()
}

/// SHA-1 (FIPS 180-4). Existe aqui porque o aperto de mão do WebSocket o
/// exige; não é usado para nada que dependa de resistência a colisão.
fn sha1(dados: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bits = (dados.len() as u64) * 8;
    let mut m = dados.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for bloco in m.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, p) in bloco.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut saida = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        saida[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    saida
}

fn base64(dados: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(dados.len().div_ceil(3) * 4);
    for bloco in dados.chunks(3) {
        let b = [bloco[0], *bloco.get(1).unwrap_or(&0), *bloco.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if bloco.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if bloco.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Vetor do RFC 6455 §1.3: a chave `dGhlIHNhbXBsZSBub25jZQ==` produz
    /// `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`. Se isto falhar, nenhum navegador
    /// completa o aperto de mão.
    #[test]
    fn aceite_do_rfc() {
        assert_eq!(aceite("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    /// Vetores clássicos do FIPS 180-4.
    #[test]
    fn sha1_conhecidos() {
        let hex = |d: [u8; 20]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn base64_conhecidos() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// Cabeçalho do quadro: FIN+texto, tamanho curto e tamanho de 16 bits.
    #[test]
    fn cabecalho_do_quadro() {
        // O quadro é montado pela mesma lógica de `quadro_texto`; aqui se
        // verifica o cabeçalho sem precisar de socket.
        let monta = |n: usize| {
            let mut q = vec![0x81u8];
            match n {
                n if n < 126 => q.push(n as u8),
                n if n <= u16::MAX as usize => {
                    q.push(126);
                    q.extend_from_slice(&(n as u16).to_be_bytes());
                }
                n => {
                    q.push(127);
                    q.extend_from_slice(&(n as u64).to_be_bytes());
                }
            }
            q
        };
        assert_eq!(monta(5), vec![0x81, 5]);
        assert_eq!(monta(200), vec![0x81, 126, 0, 200]);
        assert_eq!(monta(70000)[0..2], [0x81, 127]);
    }
}
