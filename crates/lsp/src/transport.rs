//! JSON-RPC transport over stdio: `Content-Length` framing.
//!
//! The design copies rust-analyzer's (`main_loop.rs`): one thread reads
//! `stdin` frame by frame and enqueues JSON values; the main thread
//! dispatches; responses and notifications go out through `stdout` under a
//! `Mutex`, one whole message at a time. **`stdout` is protocol only**: this
//! module never writes anything but frames, and no logging goes through here
//! (logs go to `stderr` in the binary).
//!
//! A frame is an ASCII header (`Content-Length: N`, plus ignored headers
//! such as `Content-Type`), an empty line and exactly N bytes of JSON.
//! Nothing outside this format goes out on `stdout` — the `clean_stdout`
//! test decodes the whole session frame by frame to prove it.
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

/// Reads one frame from `reader`; `None` at the end of the stream (the
/// client went away).
///
/// An I/O error or a malformed header aborts reading with an error; invalid
/// JSON after a valid `Content-Length` is an error too — resynchronizing by
/// guessing boundaries is never attempted, because it corrupts the session.
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
                io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length")
            })?);
        }
    }
    let size = size.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "frame without Content-Length")
    })?;
    let mut body = vec![0u8; size];
    reader.read_exact(&mut body)?;
    let value: Value =
        serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(value))
}

/// The only key of a response's `result` that carries the result already
/// written as JSON text ([`raw_result`]).
pub const RAW_RESULT: &str = "$dartforge/rawResult";

/// A response's `result` given as JSON text: [`write_message`] writes `json`
/// as the result itself. For results of tens of thousands of entries (the
/// references of an SDK type), whose tree of JSON values would take ~50
/// times the text's memory.
pub fn raw_result(json: String) -> Value {
    let mut result = serde_json::Map::new();
    result.insert(RAW_RESULT.to_string(), Value::String(json));
    Value::Object(result)
}

/// Writes `message` as a frame: header, empty line and body.
///
/// The caller writes the whole frame under a `Mutex`, so that two responses
/// never interleave on `stdout`.
pub fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let invalid = |e| io::Error::new(io::ErrorKind::InvalidData, e);
    let raw = message
        .get("result")
        .and_then(Value::as_object)
        .filter(|r| r.len() == 1)
        .and_then(|r| r.get(RAW_RESULT))
        .and_then(Value::as_str);
    let body = match raw {
        // The keys in the order `serde_json` writes a message's (sorted).
        Some(raw) => {
            let mut body = b"{\"id\":".to_vec();
            serde_json::to_writer(&mut body, &message["id"]).map_err(invalid)?;
            body.extend_from_slice(b",\"jsonrpc\":\"2.0\",\"result\":");
            body.extend_from_slice(raw.as_bytes());
            body.push(b'}');
            body
        }
        None => serde_json::to_vec(message).map_err(invalid)?,
    };
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

/// Serializes `message` into an in-memory frame (tests and protocol-level
/// measurements).
pub fn frame(message: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    write_message(&mut output, message).expect("serializing to a Vec does not fail");
    output
}

/// Decodes every frame in `bytes`, rejecting any byte outside the framing.
/// This is what the `clean_stdout` test asserts about the session.
pub fn decode_all(mut bytes: &[u8]) -> io::Result<Vec<Value>> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let mut reader = bytes;
        let message = read_message(&mut reader)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "remaining bytes without a frame",
            )
        })?;
        let consumed = bytes.len() - reader.len();
        messages.push(message);
        bytes = &bytes[consumed..];
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A raw result goes out as the result itself, in the frame `serde_json`
    /// would write for the same value.
    #[test]
    fn raw_result_is_written_as_the_result() {
        let raw = json!({"jsonrpc": "2.0", "id": 7, "result": raw_result("[1,{\"a\":2}]".into())});
        let plain = json!({"jsonrpc": "2.0", "id": 7, "result": [1, {"a": 2}]});
        assert_eq!(frame(&raw), frame(&plain));
        let text = json!({"jsonrpc": "2.0", "id": "x", "result": raw_result("null".into())});
        assert_eq!(
            decode_all(&frame(&text)).unwrap()[0],
            json!({"jsonrpc": "2.0", "id": "x", "result": null})
        );
    }
}
