//! Minimal WebSocket for the development server (RFC 6455).
//!
//! `webdev` reloads the browser over a bidirectional channel — `dwds` is
//! transport-agnostic and accepts SSE **or** WebSocket
//! (`dwds/lib/src/handlers/socket_connections.dart`). Here it is WebSocket.
//!
//! Only what is needed to send text from the server to the browser: the
//! handshake (§4.2.2) and unmasked text frames (§5.2, the server does not
//! mask). SHA-1 is required by the protocol — there is no choice of
//! algorithm — and it is checked against the RFC's own vector (§1.3).
use std::io::Write;
use std::net::TcpStream;

/// The protocol's fixed GUID (RFC 6455 §1.3).
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Value of the `Sec-WebSocket-Accept` header for the client's key.
pub fn accept_key(key: &str) -> String {
    let mut input = String::with_capacity(key.len() + GUID.len());
    input.push_str(key);
    input.push_str(GUID);
    base64(&sha1(input.as_bytes()))
}

/// Answers the handshake; the connection then speaks frames.
///
/// # Errors
///
/// I/O errors of the connection.
pub fn handshake(stream: &mut TcpStream, key: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key(key)
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

/// Writes a text frame (opcode 1, FIN, unmasked).
///
/// # Errors
///
/// I/O errors of the connection.
pub fn text_frame(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
    let data = text.as_bytes();
    let mut frame = Vec::with_capacity(data.len() + 10);
    frame.push(0x81);
    match data.len() {
        n if n < 126 => frame.push(n as u8),
        n if n <= u16::MAX as usize => {
            frame.push(126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            frame.push(127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(data);
    stream.write_all(&frame)?;
    stream.flush()
}

/// SHA-1 (FIPS 180-4). It exists here because the WebSocket handshake
/// requires it; it is not used for anything that depends on collision
/// resistance.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bits = (data.len() as u64) * 8;
    let mut m = data.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for block in m.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, p) in block.as_chunks::<4>().0.iter().enumerate() {
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
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for block in data.chunks(3) {
        let b = [
            block[0],
            *block.get(1).unwrap_or(&0),
            *block.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if block.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if block.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RFC 6455 §1.3 vector: the key `dGhlIHNhbXBsZSBub25jZQ==` produces
    /// `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`. If this fails, no browser completes
    /// the handshake.
    #[test]
    fn rfc_accept_key() {
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    /// Classic FIPS 180-4 vectors.
    #[test]
    fn sha1_known() {
        let hex = |d: [u8; 20]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn base64_known() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// Exercise the actual handshake and frame writer over a connected socket.
    #[test]
    fn handshake_and_frames_preserve_wire_bytes_and_utf8() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        server
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let frames = [
            (String::new(), vec![0x81, 0]),
            ("a".repeat(125), vec![0x81, 125]),
            ("b".repeat(126), vec![0x81, 126, 0, 126]),
            ("é".repeat(64), vec![0x81, 126, 0, 128]),
            ("c".repeat(65_535), vec![0x81, 126, 255, 255]),
            ("🦀".repeat(16_384), vec![0x81, 127, 0, 0, 0, 0, 0, 1, 0, 0]),
        ];
        std::thread::scope(|scope| {
            let sent = &frames;
            let writer = scope.spawn(move || {
                handshake(&mut server, "dGhlIHNhbXBsZSBub25jZQ==").unwrap();
                for (text, _) in sent {
                    text_frame(&mut server, text).unwrap();
                }
            });
            let expected = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
            let mut response = vec![0; expected.len()];
            client.read_exact(&mut response).unwrap();
            assert_eq!(response, expected);
            for (text, header) in &frames {
                let mut received_header = vec![0; header.len()];
                client.read_exact(&mut received_header).unwrap();
                assert_eq!(&received_header, header);
                let mut payload = vec![0; text.len()];
                client.read_exact(&mut payload).unwrap();
                assert_eq!(payload, text.as_bytes());
            }
            assert_eq!(client.read(&mut [0]).unwrap(), 0);
            writer.join().unwrap();
        });
    }
}
