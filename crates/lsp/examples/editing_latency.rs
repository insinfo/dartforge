//! Measures the language server on a real project under incremental editing,
//! over the wire, the way an editor drives it.
//!
//! `cargo run --release -p dartforge-lsp --example editing_latency -- \
//!   --server target/release/dartforge-lsp --root <package> --file <lib/a.dart> \
//!   [--api <lib/b.dart>] [--edits 35] [--burst 40] [--interval-ms 60]`
//!
//! The server is the real binary (`--stdio`), so the typed analysis runs in its
//! child processes exactly as in the editor. The session:
//!
//! 1. `initialize` with the package as the workspace folder, `initialized`,
//!    `didOpen` of `--file`: the time to the first typed result of the file and
//!    to the end of the first workspace analysis (every file published);
//! 2. `--edits` single-character insertions inside a method body of `--file`
//!    (a statement typed character by character), waiting for the typed
//!    diagnostics of each version: the latency of each `didChange`;
//! 3. a burst of `--burst` single-character edits sent every `--interval-ms`
//!    without waiting, then the time from the last keystroke to the settled
//!    diagnostics, and how many typed analyses ran meanwhile;
//! 4. `hover` and `completion` right after the edits;
//! 5. with `--api`: a top-level declaration added to (then removed from) that
//!    file, which changes its public API, and a body-only edit in it, each
//!    waited until the analysis settles, with the number of other files
//!    republished;
//! 6. `shutdown` and `exit`.
//!
//! Memory: the physical footprint (`proc_pid_rusage`, what Activity Monitor
//! and jetsam use; the RSS is not a reliable measure on macOS, `docs/LSP.md`
//! "Memória") of the server and of its child processes, sampled every 25 ms;
//! the server's lifetime peak comes from the kernel.
//!
//! Nothing is written to the project: every edit lives in the editor's
//! buffers only.

use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(900);

struct Options {
    server: PathBuf,
    root: PathBuf,
    file: PathBuf,
    api: Option<PathBuf>,
    edits: usize,
    burst: usize,
    interval: Duration,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut server = None;
    let mut root = None;
    let mut file = None;
    let mut api = None;
    let mut edits = 35;
    let mut burst = 40;
    let mut interval = 60;
    while let Some(a) = args.next() {
        let mut value = || args.next().expect("option value");
        match a.as_str() {
            "--server" => server = Some(PathBuf::from(value())),
            "--root" => root = Some(PathBuf::from(value())),
            "--file" => file = Some(PathBuf::from(value())),
            "--api" => api = Some(PathBuf::from(value())),
            "--edits" => edits = value().parse().expect("--edits"),
            "--burst" => burst = value().parse().expect("--burst"),
            "--interval-ms" => interval = value().parse().expect("--interval-ms"),
            other => panic!("unknown option {other}"),
        }
    }
    let root = std::fs::canonicalize(root.expect("--root")).expect("root exists");
    let file = root.join(file.expect("--file"));
    Options {
        server: server.unwrap_or_else(|| PathBuf::from("target/release/dartforge-lsp")),
        file,
        api: api.map(|a| root.join(a)),
        root,
        edits,
        burst,
        interval: Duration::from_millis(interval),
    }
}

/// Footprint measures of the macOS kernel.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod footprint {
    /// `rusage_info_v4` of `<sys/resource.h>`: the UUID, then 35 counters.
    #[repr(C)]
    struct RusageInfoV4 {
        uuid: [u8; 16],
        counters: [u64; 35],
    }
    /// Index of `ri_phys_footprint` and `ri_lifetime_max_phys_footprint`.
    const PHYS_FOOTPRINT: usize = 7;
    const LIFETIME_MAX: usize = 28;

    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RusageInfoV4) -> i32;
    }

    /// (current, lifetime maximum) footprint of `pid`, in bytes.
    pub fn of(pid: u32) -> Option<(u64, u64)> {
        let mut info = RusageInfoV4 {
            uuid: [0; 16],
            counters: [0; 35],
        };
        // SAFETY: `info` is a valid, exclusively borrowed `rusage_info_v4`
        // (flavor 4) that the call only writes.
        let r = unsafe { proc_pid_rusage(pid as i32, 4, &mut info) };
        (r == 0).then(|| (info.counters[PHYS_FOOTPRINT], info.counters[LIFETIME_MAX]))
    }
}

#[cfg(not(target_os = "macos"))]
mod footprint {
    pub fn of(_pid: u32) -> Option<(u64, u64)> {
        None
    }
}

/// The child processes of `pid` (the typed analyses).
fn children(pid: u32) -> Vec<u32> {
    let Ok(out) = Command::new("pgrep")
        .arg("-P")
        .arg(pid.to_string())
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

#[derive(Default)]
struct Memory {
    server_now: AtomicU64,
    child_peak: AtomicU64,
    total_peak: AtomicU64,
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

enum Frame {
    Message(Value),
    End,
}

struct Client {
    input: std::process::ChildStdin,
    reads: Receiver<Frame>,
    next_id: i64,
    /// Publications of typed results seen since the last reset, by URI.
    published: Vec<String>,
    analyzing: bool,
}

impl Client {
    fn send(&mut self, message: &Value) {
        self.input.write_all(&frame(message)).expect("write");
        self.input.flush().expect("flush");
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Takes one frame (answering the server's requests); `None` on timeout.
    fn take(&mut self, deadline: Instant) -> Option<Value> {
        let left = deadline.saturating_duration_since(Instant::now());
        match self.reads.recv_timeout(left) {
            Ok(Frame::Message(f)) => {
                if f.get("method").is_some() && f.get("id").is_some() {
                    let result = if f["method"] == "workspace/configuration" {
                        let n = f["params"]["items"].as_array().map_or(1, Vec::len);
                        json!(vec![json!({}); n])
                    } else {
                        Value::Null
                    };
                    let id = f["id"].clone();
                    self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
                }
                if f["method"] == "$/analyzerStatus" {
                    self.analyzing = f["params"]["isAnalyzing"].as_bool().unwrap_or(false);
                }
                if f["method"] == "textDocument/publishDiagnostics"
                    && let Some(u) = f["params"]["uri"].as_str()
                {
                    self.published.push(u.to_string());
                }
                Some(f)
            }
            Ok(Frame::End) => panic!("the server closed its output"),
            Err(_) => None,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> (Value, Duration) {
        self.next_id += 1;
        let id = self.next_id;
        let started = Instant::now();
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = started + LIMIT;
        loop {
            let f = self.take(deadline).expect("response in time");
            if f["id"] == json!(id) && f.get("method").is_none() {
                return (f, started.elapsed());
            }
        }
    }

    /// Waits until the analysis that the last change started settles
    /// (`$/analyzerStatus` with `isAnalyzing: false` after `true`).
    fn settle(&mut self, deadline: Instant) {
        let mut started = self.analyzing;
        loop {
            let Some(f) = self.take(deadline) else {
                panic!("the analysis did not settle in time");
            };
            if f["method"] == "$/analyzerStatus" {
                if self.analyzing {
                    started = true;
                } else if started {
                    return;
                }
            }
        }
    }

    /// Waits for `$/analyzerStatus` to say the analysis started (it may already
    /// be running), or a short while.
    fn wait_start(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while !self.analyzing {
            if self.take(deadline).is_none() {
                return;
            }
        }
    }

    /// Discards what is queued, without waiting.
    fn drain(&mut self) {
        while self.take(Instant::now()).is_some() {}
    }
}

fn uri_of(path: &Path) -> String {
    url::Url::from_file_path(path).expect("uri").to_string()
}

/// (line, UTF-16 column) of byte `offset` (ASCII edits only are made).
fn position(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[start..].encode_utf16().count() as u32)
}

/// The offset just after the first line that opens a method or function body
/// (`) {`, `) async {`), indented: where a statement can be typed.
fn body_insertion(text: &str) -> usize {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        let indented = line.starts_with("  ");
        let opens = t.ends_with(") {") || t.ends_with(") async {");
        let control = t.starts_with('}')
            || [
                "if ", "if(", "for ", "for(", "while", "switch", "try", "catch",
            ]
            .iter()
            .any(|k| t.starts_with(k));
        if indented && opens && !control && !t.starts_with("class ") {
            return offset + line.len();
        }
        offset += line.len();
    }
    panic!("no method body to type in");
}

fn insert(
    client: &mut Client,
    uri: &str,
    text: &mut String,
    at: usize,
    s: &str,
    version: &mut i32,
) {
    let (line, character) = position(text, at);
    text.insert_str(at, s);
    *version += 1;
    client.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": *version},
               "contentChanges": [{"range": {"start": {"line": line, "character": character},
                                             "end": {"line": line, "character": character}},
                                   "text": s}]}),
    );
}

fn remove(
    client: &mut Client,
    uri: &str,
    text: &mut String,
    start: usize,
    end: usize,
    version: &mut i32,
) {
    let (l0, c0) = position(text, start);
    let (l1, c1) = position(text, end);
    text.replace_range(start..end, "");
    *version += 1;
    client.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": *version},
               "contentChanges": [{"range": {"start": {"line": l0, "character": c0},
                                             "end": {"line": l1, "character": c1}},
                                   "text": ""}]}),
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn summary(label: &str, mut v: Vec<Duration>) {
    if v.is_empty() {
        return;
    }
    v.sort();
    let median = v[v.len() / 2];
    let p90 = v[(v.len() * 9 / 10).min(v.len() - 1)];
    println!(
        "{label}: n={} min {:.0} ms, median {:.0} ms, p90 {:.0} ms, max {:.0} ms",
        v.len(),
        ms(v[0]),
        ms(median),
        ms(p90),
        ms(*v.last().expect("non-empty"))
    );
}

fn main() {
    let o = options();
    let mut child = Command::new(&o.server)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn the server");
    let pid = child.id();
    let input = child.stdin.take().expect("stdin");
    let output = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        while let Ok(Some(f)) = read_message(&mut reader) {
            if tx.send(Frame::Message(f)).is_err() {
                return;
            }
        }
        let _ = tx.send(Frame::End);
    });
    let memory = Arc::new(Memory::default());
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = {
        let (memory, stop) = (Arc::clone(&memory), Arc::clone(&stop));
        std::thread::spawn(move || {
            let peaks: Mutex<std::collections::HashMap<u32, u64>> = Mutex::default();
            while !stop.load(Ordering::Relaxed) {
                let server = footprint::of(pid).map_or(0, |f| f.0);
                memory.server_now.store(server, Ordering::Relaxed);
                let mut kids = 0;
                for c in children(pid) {
                    if let Some((now, max)) = footprint::of(c) {
                        kids += now;
                        let mut p = peaks.lock().expect("peaks");
                        let e = p.entry(c).or_default();
                        *e = (*e).max(max);
                        memory.child_peak.fetch_max(*e, Ordering::Relaxed);
                    }
                }
                memory
                    .total_peak
                    .fetch_max(server + kids, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(25));
            }
        })
    };
    let mut c = Client {
        input,
        reads: rx,
        next_id: 0,
        published: Vec::new(),
        analyzing: false,
    };
    let root_uri = uri_of(&o.root);
    let started = Instant::now();
    c.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri,
               "workspaceFolders": [{"uri": root_uri, "name": "root"}],
               "capabilities": {"textDocument": {"hover": {"contentFormat": ["markdown"]}}}}),
    );
    c.notify("initialized", json!({}));
    let uri = uri_of(&o.file);
    let mut text = std::fs::read_to_string(&o.file).expect("read --file");
    let mut version = 1;
    c.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "dart", "version": version, "text": text}}),
    );
    // 1. The first workspace analysis.
    let deadline = Instant::now() + LIMIT;
    let mut first_typed = None;
    let mut seen_own = 0;
    loop {
        let Some(f) = c.take(deadline) else {
            panic!("no first analysis in time")
        };
        if f["method"] == "textDocument/publishDiagnostics" && f["params"]["uri"] == uri.as_str() {
            seen_own += 1;
            // The first publication is the immediate (syntactic) one.
            if seen_own == 2 && first_typed.is_none() {
                first_typed = Some(started.elapsed());
            }
        }
        if f["method"] == "$/analyzerStatus" && f["params"]["isAnalyzing"] == false {
            break;
        }
    }
    let workspace = started.elapsed();
    let published = c.published.len();
    println!(
        "first analysis: typed result of the file at {:.0} ms (or none if empty), workspace settled at {:.0} ms, {} publications; server footprint {:.1} MiB",
        first_typed.map_or(f64::NAN, ms),
        ms(workspace),
        published,
        mib(memory.server_now.load(Ordering::Relaxed))
    );
    // 2. Typing a statement, waiting for each version's diagnostics.
    let at = body_insertion(&text);
    let statement = "    final dfProbe = [1, 2].length;\n";
    let mut latencies = Vec::new();
    let mut typed_at = at;
    for ch in statement.chars().take(o.edits) {
        insert(
            &mut c,
            &uri,
            &mut text,
            typed_at,
            &ch.to_string(),
            &mut version,
        );
        typed_at += ch.len_utf8();
        let t = Instant::now();
        c.wait_start();
        c.settle(t + LIMIT);
        latencies.push(t.elapsed());
    }
    summary("didChange → typed diagnostics (waiting each)", latencies);
    // 3. A burst without waiting.
    c.published.clear();
    let burst_at = typed_at;
    let mut last = Instant::now();
    for i in 0..o.burst {
        let s = if i % 2 == 0 { "x" } else { "y" };
        insert(&mut c, &uri, &mut text, burst_at + i, s, &mut version);
        last = Instant::now();
        std::thread::sleep(o.interval);
        c.drain();
    }
    c.wait_start();
    c.settle(last + LIMIT);
    let own = c.published.iter().filter(|u| **u == uri).count();
    println!(
        "burst of {} edits every {} ms: settled {:.0} ms after the last keystroke ({} publications of the file)",
        o.burst,
        o.interval.as_millis(),
        ms(last.elapsed()),
        own
    );
    // Undo the burst (one edit) so the statement is complete again.
    remove(
        &mut c,
        &uri,
        &mut text,
        burst_at,
        burst_at + o.burst,
        &mut version,
    );
    let t = Instant::now();
    c.wait_start();
    c.settle(t + LIMIT);
    println!("undo of the burst (one edit): {:.0} ms", ms(t.elapsed()));
    // 4. Queries after the edits.
    let probe = text[at..].find("dfProbe").map(|i| at + i);
    let length = text[at..].find("length").map(|i| at + i);
    let mut hovers = Vec::new();
    let mut completions = Vec::new();
    for _ in 0..5 {
        if let Some(p) = probe {
            let (line, character) = position(&text, p + 2);
            let (_, d) = c.request(
                "textDocument/hover",
                json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
            );
            hovers.push(d);
        }
        if let Some(p) = length {
            let (line, character) = position(&text, p);
            let (r, d) = c.request(
                "textDocument/completion",
                json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
            );
            let _ = r;
            completions.push(d);
        }
    }
    summary("hover after the edits", hovers);
    summary("completion after the edits", completions);
    // 5. API edits in another file.
    if let Some(api) = &o.api {
        let api_uri = uri_of(api);
        let mut api_text = std::fs::read_to_string(api).expect("read --api");
        let mut api_version = 1;
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": api_uri, "languageId": "dart", "version": api_version, "text": api_text}}),
        );
        c.wait_start();
        c.settle(Instant::now() + LIMIT);
        c.drain();
        let end = api_text.len();
        let declaration = "\nint dfApiProbe() => 0;\n";
        for (label, add) in [("add a top-level function", true), ("remove it", false)] {
            c.published.clear();
            if add {
                insert(
                    &mut c,
                    &api_uri,
                    &mut api_text,
                    end,
                    declaration,
                    &mut api_version,
                );
            } else {
                remove(
                    &mut c,
                    &api_uri,
                    &mut api_text,
                    end,
                    end + declaration.len(),
                    &mut api_version,
                );
            }
            let t = Instant::now();
            c.wait_start();
            c.settle(t + LIMIT);
            let others = c.published.iter().filter(|u| **u != api_uri).count();
            println!(
                "API edit ({label}): {:.0} ms, {} other files republished",
                ms(t.elapsed()),
                others
            );
        }
        // A body-only edit in the API file.
        let at = body_insertion(&api_text);
        c.published.clear();
        insert(
            &mut c,
            &api_uri,
            &mut api_text,
            at,
            "    0;\n",
            &mut api_version,
        );
        let t = Instant::now();
        c.wait_start();
        c.settle(t + LIMIT);
        let others = c.published.iter().filter(|u| **u != api_uri).count();
        println!(
            "body edit in the API file: {:.0} ms, {} other files republished",
            ms(t.elapsed()),
            others
        );
    }
    // At rest: the analyses are over and their children gone.
    std::thread::sleep(Duration::from_secs(2));
    let rest = footprint::of(pid).map_or(0, |f| f.0);
    // The children still alive at rest (the resident analysis process).
    let resident: Vec<(u64, u64)> = children(pid)
        .into_iter()
        .filter_map(footprint::of)
        .collect();
    c.request("shutdown", Value::Null);
    let server_peak = footprint::of(pid).map_or(0, |f| f.1);
    c.notify("exit", Value::Null);
    let status = child.wait().expect("server exit");
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    println!(
        "memory: server footprint peak {:.1} MiB, at rest {:.1} MiB, analysis child peak {:.1} MiB, server + children peak (sampled) {:.1} MiB; exit {status}",
        mib(server_peak),
        mib(rest),
        mib(memory.child_peak.load(Ordering::Relaxed)),
        mib(memory.total_peak.load(Ordering::Relaxed)),
    );
    for (now, max) in resident {
        println!(
            "resident analysis process: footprint at rest {:.1} MiB, peak {:.1} MiB",
            mib(now),
            mib(max)
        );
    }
    println!("total {:.1} s", started.elapsed().as_secs_f64());
}
