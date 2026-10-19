//! Development server: serves the output, reloads the browser.
//!
//! It is what `webdev` does, without `build_runner` and without
//! `build_web_compilers` (the compilation is ours). Written on top of
//! `std::net`, with no new dependency: HTTP/1.1 with one thread per
//! connection and the reload over **WebSocket**, like `webdev`'s auto-refresh
//! (`dwds` is transport-agnostic and accepts WebSocket or SSE —
//! `dwds/lib/src/handlers/socket_connections.dart`); the protocol is in
//! [`crate::ws`].
//!
//! The serving rules are those that `scripts/flow.mjs` already validated
//! against the real application: fallback to `index.html` **only on
//! navigation** (the browser sends `Accept: text/html`), so that an API
//! `fetch` keeps receiving 404, which is what the application would see in
//! production.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Version of the development protocol; the client checks it on connect.
pub const PROTOCOL_VERSION: u32 = 1;

/// A message from the server to the browser.
///
/// The protocol is **separate from the transport**: today the messages go
/// out as WebSocket text frames, and switching to SSE (or adding it) would
/// touch neither the compiler nor the session. `hot-update` and `debugger`
/// come in here when they exist — that is why the channel is bidirectional
/// from the start, as in `webdev`/`dwds`, Vite and webpack-dev-server.
#[derive(Debug, Clone)]
pub enum Message {
    /// Handshake: version and current generation of the output.
    Hello { generation: u64 },
    /// The compilation finished and changed modules: reload.
    Reload { generation: u64 },
    /// The compilation failed; the message goes to the browser console.
    CompileError { message: String },
}

impl Message {
    /// Serializes to JSON by hand (the crate has no `serde`, and the
    /// protocol has three messages). The keys and values are the protocol's
    /// and are read by the client script.
    pub fn json(&self) -> String {
        match self {
            Message::Hello { generation } => {
                format!("{{\"type\":\"hello\",\"protocol\":{PROTOCOL_VERSION},\"generation\":{generation}}}")
            }
            Message::Reload { generation } => {
                format!("{{\"type\":\"reload\",\"generation\":{generation}}}")
            }
            Message::CompileError { message } => {
                format!("{{\"type\":\"error\",\"message\":{}}}", json_string(message))
            }
        }
    }
}

/// Escapes a string as a JSON literal.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Open browsers: each tab has a channel, as in Vite's model.
#[derive(Default)]
pub struct Reload {
    subscribers: Mutex<Vec<Sender<Message>>>,
    generation: Mutex<u64>,
}

impl Reload {
    /// Notifies every tab; the ones that closed leave the list.
    pub fn send(&self, m: Message) {
        let mut v = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        v.retain(|s| s.send(m.clone()).is_ok());
    }
    /// A compilation changed modules: new generation and reload.
    pub fn trigger(&self) {
        let g = {
            let mut g = self.generation.lock().unwrap_or_else(|e| e.into_inner());
            *g += 1;
            *g
        };
        self.send(Message::Reload { generation: g });
    }
    /// The compilation failed: the browser shows it in the console and stays
    /// connected.
    pub fn error(&self, message: &str) {
        self.send(Message::CompileError { message: message.to_string() });
    }
    fn current_generation(&self) -> u64 {
        *self.generation.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn subscribe(&self) -> Receiver<Message> {
        let (tx, rx) = channel();
        self.subscribers.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }
    /// Tabs open at the moment.
    pub fn open_tabs(&self) -> usize {
        self.subscribers.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Starts the server on a thread and returns the actual port.
///
/// `output` is the directory of the emitted modules; `web` is the project
/// directory with `index.html` and `assets/` (optional). `port` 0 picks a
/// free one.
///
/// # Errors
///
/// When the port cannot be bound or the thread cannot be spawned.
pub fn serve(
    output: &Path,
    web: Option<&Path>,
    port: u16,
    reload: Arc<Reload>,
) -> Result<u16, String> {
    serve_with_generated(output, web, port, reload, None)
}

/// Like [`serve`], looking first at the build engine's outputs: a `.css`
/// from `web/` requested by the browser comes from the current generation
/// (and is the demand that materializes the `sass_builder` action).
///
/// # Errors
///
/// When the port cannot be bound or the thread cannot be spawned.
pub fn serve_with_generated(
    output: &Path,
    web: Option<&Path>,
    port: u16,
    reload: Arc<Reload>,
    generated: Option<crate::generation::Provider>,
) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let output = output.to_path_buf();
    let web = web.map(|p| p.to_path_buf());
    std::thread::Builder::new()
        .name("dartforge-serve".into())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(stream) = connection else { continue };
                let (output, web, reload, generated) = (output.clone(), web.clone(), reload.clone(), generated.clone());
                // One thread per connection: the WebSocket stays open while the tab lives.
                let _ = std::thread::Builder::new()
                    .stack_size(256 * 1024)
                    .spawn(move || handle(stream, &output, web.as_deref(), &reload, generated.as_ref()));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(port)
}

fn handle(
    mut stream: TcpStream,
    output: &Path,
    web: Option<&Path>,
    reload: &Reload,
    generated: Option<&crate::generation::Provider>,
) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    let mut accepts_html = false;
    let mut ws_key: Option<String> = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if line.trim().is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("accept:") {
                    accepts_html = v.contains("text/html");
                }
                if lower.starts_with("sec-websocket-key:") {
                    ws_key = line.split(':').nth(1).map(|v| v.trim().to_string());
                }
            }
            Err(_) => return,
        }
    }
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        let _ = respond(&mut stream, 405, "text/plain; charset=utf-8", b"method not supported");
        return;
    }
    let path = target.split(['?', '#']).next().unwrap_or("/");

    // Reload channel: a WebSocket open while the tab lives, one text frame
    // per compilation (`webdev`'s auto-refresh uses the same design).
    if path == RELOAD_ROUTE {
        let Some(key) = ws_key else {
            let _ = respond(&mut stream, 400, "text/plain; charset=utf-8", b"expected a WebSocket");
            return;
        };
        if crate::ws::handshake(&mut stream, &key).is_err() {
            return;
        }
        let rx = reload.subscribe();
        let hello = Message::Hello { generation: reload.current_generation() };
        if crate::ws::text_frame(&mut stream, &hello.json()).is_err() {
            return;
        }
        while let Ok(m) = rx.recv() {
            if crate::ws::text_frame(&mut stream, &m.json()).is_err() {
                return;
            }
        }
        return;
    }

    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    // `..` in the path does not leave the served directory.
    if rel.split('/').any(|p| p == "..") {
        let _ = respond(&mut stream, 400, "text/plain; charset=utf-8", b"invalid path");
        return;
    }

    if rel == "index.html" {
        match index_html(output, web) {
            Some(html) => {
                let _ = respond(&mut stream, 200, "text/html; charset=utf-8", html.as_bytes());
            }
            None => {
                let _ = respond(&mut stream, 404, "text/plain; charset=utf-8", b"no index.html");
            }
        }
        return;
    }

    // Output generated by the build engine (natural path under `web/`).
    if let (Some(pv), Some(w)) = (generated, web) {
        let natural = w.join(rel);
        if let Some(data) = pv(&natural) {
            let _ = respond(&mut stream, 200, mime_type(&natural), &data);
            return;
        }
    }
    // The file comes from the output; if it is not there, from the project's
    // `web` directory.
    let mut file = output.join(rel);
    if !file.is_file()
        && let Some(w) = web {
            let alternative = w.join(rel);
            if alternative.is_file() {
                file = alternative;
            }
        }
    if !file.is_file() {
        // SPA fallback only for navigation: an API `fetch` keeps getting 404.
        let no_extension = !rel.rsplit('/').next().unwrap_or("").contains('.');
        if accepts_html && no_extension
            && let Some(html) = index_html(output, web) {
                let _ = respond(&mut stream, 200, "text/html; charset=utf-8", html.as_bytes());
                return;
            }
        let _ = respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found");
        return;
    }
    let mut data = Vec::new();
    if std::fs::File::open(&file).and_then(|mut f| f.read_to_end(&mut data)).is_err() {
        let _ = respond(&mut stream, 500, "text/plain; charset=utf-8", b"read error");
        return;
    }
    let _ = respond(&mut stream, 200, mime_type(&file), &data);
}

/// Route of the reload channel (outside the application's namespace). The
/// value is part of the protocol with the client script.
const RELOAD_ROUTE: &str = "/__dartforge_reload";

/// Script injected into `index.html`: reloads the page on every
/// compilation. Its text is served as is to the browser.
const RELOAD_SCRIPT: &str = "<script>
(function () {
  var generation = null;
  function connect() {
    var p = location.protocol === 'https:' ? 'wss://' : 'ws://';
    var s = new WebSocket(p + location.host + '/__dartforge_reload');
    s.onmessage = function (ev) {
      var m; try { m = JSON.parse(ev.data); } catch (e) { return; }
      if (m.type === 'hello') { if (generation !== null && m.generation !== generation) location.reload(); generation = m.generation; }
      else if (m.type === 'reload') location.reload();
      else if (m.type === 'error') console.error('[dartforge] ' + m.message);
    };
    s.onclose = function () { setTimeout(connect, 500); };
  }
  connect();
})();
</script>
";

/// The served `index.html`: the project's, with `main.dart.js` replaced by
/// our module and the reload script; or, if the project has none, a minimal
/// one.
fn index_html(output: &Path, web: Option<&Path>) -> Option<String> {
    let own = output.join("index.html");
    let source = web
        .map(|w| w.join("index.html"))
        .filter(|p| p.is_file())
        .or(Some(own).filter(|p| p.is_file()))
        .and_then(|p| std::fs::read_to_string(p).ok());
    let Some(source) = source else {
        if !output.join("main.mjs").is_file() {
            return None;
        }
        return Some(format!(
            "<!DOCTYPE html>\n<html lang=\"pt-br\">\n<head>\n<meta charset=\"utf-8\">\n<title>DartForge</title>\n{RELOAD_SCRIPT}<script type=\"module\" src=\"main.mjs\"></script>\n</head>\n<body></body>\n</html>\n"
        ));
    };
    // The official bootstrap goes out; our module comes in.
    let mut html = String::with_capacity(source.len() + 256);
    let mut rest = source.as_str();
    while let Some(i) = rest.find("<script") {
        let end = match rest[i..].find("</script>") {
            Some(f) => i + f + "</script>".len(),
            None => break,
        };
        let tag = &rest[i..end];
        if tag.contains("main.dart.js") {
            html.push_str(&rest[..i]);
            rest = &rest[end..];
        } else {
            html.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    html.push_str(rest);
    let entry = format!("{RELOAD_SCRIPT}<script type=\"module\" src=\"main.mjs\"></script>\n");
    match html.find("</head>") {
        Some(i) => html.insert_str(i, &entry),
        None => html.push_str(&entry),
    }
    Some(html)
}

fn respond(stream: &mut TcpStream, code: u16, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn mime_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The project's `index.html` loses the official bootstrap and gains our
    /// module plus the reload script.
    #[test]
    fn index_replaces_bootstrap_and_injects_reload() {
        // `tempfile` deletes the directory on `Drop`, even when an assertion fails.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(
            dir.join("index.html"),
            "<html><head><title>x</title><script defer src=\"main.dart.js\"></script></head><body></body></html>",
        )
        .unwrap();
        let html = index_html(dir, Some(dir)).expect("index");
        assert!(!html.contains("main.dart.js"), "{html}");
        assert!(html.contains("src=\"main.mjs\""), "{html}");
        assert!(html.contains(RELOAD_ROUTE), "{html}");
        assert!(html.contains("<title>x</title>"));
    }

    /// Without `index.html` and without `main.mjs` there is nothing to serve.
    #[test]
    fn no_output_no_index() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(index_html(tmp.path(), None).is_none());
    }
}
