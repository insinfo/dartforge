//! Measures a language server on a real project under incremental editing,
//! over the wire, the way an editor drives it: DartForge's server or the
//! official one (`dart language-server`), with the same session.
//!
//! `cargo run --release -p dartforge-lsp --example editing_latency -- \
//!   (--server target/release/dartforge-lsp | --dart <sdk>/bin/dart) \
//!   --root <package> --file <lib/a.dart> [--private <lib/b.dart>] \
//!   [--api <lib/c.dart>] [--edits 20] [--burst 40] [--interval-ms 60] \
//!   [--reps 5] [--positions 8] [--json <out.json>]`
//!
//! The server is the real binary over stdio, so DartForge's typed analysis runs
//! in its child processes exactly as in the editor. Neither server is told the
//! client supports `workDoneProgress`, so both report their state through
//! `$/analyzerStatus`, which is what "settled" means below. The session:
//!
//! 1. `initialize` with the package as the workspace folder, `initialized`,
//!    `didOpen` of `--file`: the time until the first workspace analysis
//!    settles (the first `isAnalyzing: false` after a `true` that no `true`
//!    follows for 10 s);
//! 2. `--edits` single-character insertions inside a method body of `--file`
//!    (a statement typed character by character), waiting for each version to
//!    settle: the latency of each `didChange`;
//! 3. a burst of `--burst` single-character edits sent every `--interval-ms`
//!    without waiting, then the time from the last keystroke to settled;
//! 4. `--reps` times each, edits whose diagnostic must be published: a type
//!    error in a method body of `--file` (body edit), an unused private
//!    declaration added to `--private` (private API edit) and a public
//!    declaration with a type error added to `--api` (exported API edit, the
//!    file a barrel exports): the time until the publication of that file
//!    carries a diagnostic on the new line, and the time until settled;
//! 5. queries at `--positions` identifiers of `--file` (hover, completion,
//!    definition, references, prepareRename, codeAction): each once right after
//!    a settled body edit (`after edit`) and at every position with no edit in
//!    between (`warm`);
//! 6. every edit undone, settled, and the last publication of every file kept
//!    (`diagnostics` in the JSON, to compare the two servers), then `shutdown`
//!    and `exit`.
//!
//! CPU time (`proc_pid_rusage`): for each edit, from the change to settled,
//! that of the server, of its ended children and of its live descendants; for
//! each request, that of the server process and of the children it waited for
//! (where both servers answer them, or a DartForge reference search child).
//! On a loaded machine it is the steadier measure: the wall time also counts
//! the waits for a core.
//!
//! Memory: the physical footprint (`proc_pid_rusage`, what Activity Monitor
//! and jetsam use; the RSS is not a reliable measure on macOS, `docs/LSP.md`
//! "Memória") and the RSS of the server and of all its descendant processes,
//! sampled every 25 ms.
//!
//! Nothing is written to the project: every edit lives in the editor's
//! buffers only.

use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(900);
/// How long the first analysis must stay idle to count as settled (the official
/// server reports a short busy spell for the opened file, then builds its
/// contexts for seconds without reporting anything, then analyzes them).
const QUIET: Duration = Duration::from_secs(10);

struct Options {
    /// The command line of the server.
    command: Vec<String>,
    label: String,
    root: PathBuf,
    file: PathBuf,
    private: Option<PathBuf>,
    api: Option<PathBuf>,
    edits: usize,
    burst: usize,
    interval: Duration,
    reps: usize,
    positions: usize,
    json: Option<PathBuf>,
    /// `--long N`: the long session instead of the measurements (N rounds;
    /// `0` only starts, rests and reports: the freshly started server).
    long: Option<usize>,
    /// `--generated N`: the files of each wave of the long session.
    generated: usize,
    /// `--csv`: where the long session's samples go.
    csv: Option<PathBuf>,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut command = None;
    let mut label = None;
    let mut root = None;
    let mut file = None;
    let mut private = None;
    let mut api = None;
    let mut edits = 20;
    let mut burst = 40;
    let mut interval = 60;
    let mut reps = 5;
    let mut positions = 8;
    let mut json = None;
    let mut long = None;
    let mut generated = 40;
    let mut csv = None;
    while let Some(a) = args.next() {
        let mut value = || args.next().expect("option value");
        match a.as_str() {
            "--server" => {
                command = Some(vec![value(), "--stdio".to_string()]);
                label.get_or_insert_with(|| "dartforge".to_string());
            }
            "--dart" => {
                command = Some(vec![
                    value(),
                    "language-server".to_string(),
                    "--protocol=lsp".to_string(),
                    "--client-id=dartforge.editing_latency".to_string(),
                ]);
                label.get_or_insert_with(|| "dart".to_string());
            }
            "--label" => label = Some(value()),
            "--root" => root = Some(PathBuf::from(value())),
            "--file" => file = Some(PathBuf::from(value())),
            "--private" => private = Some(PathBuf::from(value())),
            "--api" => api = Some(PathBuf::from(value())),
            "--edits" => edits = value().parse().expect("--edits"),
            "--burst" => burst = value().parse().expect("--burst"),
            "--interval-ms" => interval = value().parse().expect("--interval-ms"),
            "--reps" => reps = value().parse().expect("--reps"),
            "--positions" => positions = value().parse().expect("--positions"),
            "--json" => json = Some(PathBuf::from(value())),
            "--long" => long = Some(value().parse().expect("--long")),
            "--generated" => generated = value().parse().expect("--generated"),
            "--csv" => csv = Some(PathBuf::from(value())),
            other => panic!("unknown option {other}"),
        }
    }
    let root = std::fs::canonicalize(root.expect("--root")).expect("root exists");
    let file = root.join(file.expect("--file"));
    Options {
        command: command.unwrap_or_else(|| {
            vec![
                "target/release/dartforge-lsp".to_string(),
                "--stdio".to_string(),
            ]
        }),
        label: label.unwrap_or_else(|| "dartforge".to_string()),
        file,
        private: private.map(|a| root.join(a)),
        api: api.map(|a| root.join(a)),
        root,
        edits,
        burst,
        interval: Duration::from_millis(interval),
        reps,
        positions,
        long,
        generated,
        csv,
        json,
    }
}

/// Memory measures of the macOS kernel.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod footprint {
    /// `rusage_info_v4` of `<sys/resource.h>`: the UUID, then 35 counters.
    #[repr(C)]
    struct RusageInfoV4 {
        uuid: [u8; 16],
        counters: [u64; 35],
    }
    /// Index of `ri_user_time`, `ri_system_time`, `ri_resident_size`,
    /// `ri_phys_footprint`, `ri_child_user_time`, `ri_child_system_time` and
    /// `ri_lifetime_max_phys_footprint`.
    const USER: usize = 0;
    const SYSTEM: usize = 1;
    const RESIDENT: usize = 6;
    const PHYS_FOOTPRINT: usize = 7;
    const CHILD_USER: usize = 10;
    const CHILD_SYSTEM: usize = 11;
    const LIFETIME_MAX: usize = 28;

    /// The memory (bytes) and CPU time (ns) of one process.
    pub struct Sample {
        pub footprint: u64,
        pub lifetime_max: u64,
        pub rss: u64,
        /// User + system time of the process.
        pub cpu: u64,
        /// User + system time of its children that ended and were waited for.
        pub children_cpu: u64,
    }

    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }

    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RusageInfoV4) -> i32;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }

    /// The times of `rusage_info` are Mach absolute time units, in ns.
    fn ns(ticks: u64) -> u64 {
        static RATIO: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
        let (n, d) = *RATIO.get_or_init(|| {
            let mut t = Timebase { numer: 1, denom: 1 };
            // SAFETY: `t` is a valid, exclusively borrowed `mach_timebase_info`.
            unsafe { mach_timebase_info(&mut t) };
            (u64::from(t.numer), u64::from(t.denom.max(1)))
        });
        (u128::from(ticks) * u128::from(n) / u128::from(d)) as u64
    }

    pub fn of(pid: u32) -> Option<Sample> {
        let mut info = RusageInfoV4 {
            uuid: [0; 16],
            counters: [0; 35],
        };
        // SAFETY: `info` is a valid, exclusively borrowed `rusage_info_v4`
        // (flavor 4) that the call only writes.
        let r = unsafe { proc_pid_rusage(pid as i32, 4, &mut info) };
        (r == 0).then(|| Sample {
            footprint: info.counters[PHYS_FOOTPRINT],
            lifetime_max: info.counters[LIFETIME_MAX],
            rss: info.counters[RESIDENT],
            cpu: ns(info.counters[USER] + info.counters[SYSTEM]),
            children_cpu: ns(info.counters[CHILD_USER] + info.counters[CHILD_SYSTEM]),
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod footprint {
    pub struct Sample {
        pub footprint: u64,
        pub lifetime_max: u64,
        pub rss: u64,
        pub cpu: u64,
        pub children_cpu: u64,
    }
    pub fn of(_pid: u32) -> Option<Sample> {
        None
    }
}

/// The descendant processes of `pid` (DartForge's typed analyses, the official
/// server's processes).
fn descendants(pid: u32) -> Vec<u32> {
    let mut all = Vec::new();
    let mut pending = vec![pid];
    while let Some(p) = pending.pop() {
        let Ok(out) = Command::new("pgrep").arg("-P").arg(p.to_string()).output() else {
            continue;
        };
        for c in String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
        {
            all.push(c);
            pending.push(c);
        }
    }
    all
}

/// The CPU time (ns) the server has used so far: its own, that of its
/// children that ended (each typed analysis of its own) and that of the
/// descendants alive (DartForge's resident analysis process).
fn cpu_of_all(pid: u32) -> u64 {
    let own = footprint::of(pid).map_or(0, |s| s.cpu + s.children_cpu);
    own + descendants(pid)
        .into_iter()
        .filter_map(footprint::of)
        .map(|s| s.cpu)
        .sum::<u64>()
}

/// The CPU time (ns) of the server process and of its children that ended
/// (the queries run there, or in a child of their own that the server waits
/// for: DartForge's reference searches that do not fit).
fn cpu_of(pid: u32) -> u64 {
    footprint::of(pid).map_or(0, |s| s.cpu + s.children_cpu)
}

#[derive(Default)]
struct Memory {
    server_now: AtomicU64,
    child_peak: AtomicU64,
    total_peak: AtomicU64,
    rss_peak: AtomicU64,
    /// Sum and count of the sampled totals (footprint), for the session mean.
    total_sum: AtomicU64,
    samples: AtomicU64,
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

enum Frame {
    Message(Value),
    End,
}

struct Client {
    /// The server's process.
    pid: u32,
    input: std::process::ChildStdin,
    reads: Receiver<Frame>,
    next_id: i64,
    /// Publications seen since the last reset, by URI.
    published: Vec<String>,
    /// The last publication of every URI.
    diagnostics: BTreeMap<String, Value>,
    analyzing: bool,
    /// The server said an analysis started since this was last cleared.
    saw_start: bool,
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
                    self.saw_start |= self.analyzing;
                }
                if f["method"] == "textDocument/publishDiagnostics"
                    && let Some(u) = f["params"]["uri"].as_str()
                {
                    self.published.push(u.to_string());
                    self.diagnostics
                        .insert(u.to_string(), f["params"]["diagnostics"].clone());
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
    /// (`$/analyzerStatus` with `isAnalyzing: false` after `true`), then takes
    /// what the server sent with it. Without a running analysis (it already
    /// ended, e.g. while a burst was being drained), 1 s without one starting
    /// counts as settled.
    fn settle(&mut self, deadline: Instant) {
        let mut started = self.analyzing;
        loop {
            let wait = if started {
                deadline
            } else {
                deadline.min(Instant::now() + Duration::from_secs(1))
            };
            let Some(f) = self.take(wait) else {
                if !started && Instant::now() < deadline {
                    break;
                }
                panic!("the analysis did not settle in time");
            };
            if f["method"] == "$/analyzerStatus" {
                if self.analyzing {
                    started = true;
                } else if started {
                    break;
                }
            }
        }
        // DartForge sends the end of the analysis ahead of its publications.
        let quick = Instant::now() + Duration::from_millis(5);
        while self.take(quick).is_some() {}
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

    /// The time of the change to settled. Without a reported start, the time
    /// is the driver's waiting windows (500 ms for the start, 1 s of
    /// silence), not the analysis: the long session times its loads by the
    /// publications instead ([`Client::edit_timed`]).
    fn settle_after(&mut self, t: Instant) -> Duration {
        self.wait_start();
        self.settle(t + LIMIT);
        t.elapsed()
    }

    /// Waits until the last publication of `uri` has (`present`) or no longer
    /// has a diagnostic starting at `line`; whether it came before
    /// `deadline`.
    fn published_line(&mut self, uri: &str, line: u32, present: bool, deadline: Instant) -> bool {
        let at_line = |d: Option<&Value>| {
            d.and_then(Value::as_array).is_some_and(|d| {
                d.iter()
                    .any(|d| d["range"]["start"]["line"].as_u64() == Some(line.into()))
            })
        };
        loop {
            if at_line(self.diagnostics.get(uri)) == present {
                return true;
            }
            if self.take(deadline).is_none() {
                return false;
            }
        }
    }

    /// After an edit at `t` whose effect on `uri` is a diagnostic starting
    /// at `line` that appears (`present`) or goes: the time to that
    /// publication (`None` if it never came), the time to settled, and
    /// whether the server reported the analysis's start (without it, the
    /// time to settled is the time to the publication).
    fn edit_timed(
        &mut self,
        t: Instant,
        uri: &str,
        line: u32,
        present: bool,
    ) -> (Option<Duration>, Duration, bool) {
        let published = self
            .published_line(uri, line, present, t + LIMIT)
            .then(|| t.elapsed());
        if self.analyzing {
            self.settle(t + LIMIT);
        }
        (published, t.elapsed(), self.saw_start)
    }

    /// Waits until the last publication of every one of `uris` has a
    /// diagnostic that names `marker` (`'marker'` in its message); whether it
    /// came before `deadline`.
    fn published_with(&mut self, uris: &[String], marker: &str, deadline: Instant) -> bool {
        let quoted = format!("'{marker}'");
        let named = |d: Option<&Value>| {
            d.and_then(Value::as_array).is_some_and(|d| {
                d.iter()
                    .any(|d| d["message"].as_str().is_some_and(|m| m.contains(&quoted)))
            })
        };
        loop {
            if uris.iter().all(|u| named(self.diagnostics.get(u))) {
                return true;
            }
            if self.take(deadline).is_none() {
                return false;
            }
        }
    }

    /// Waits for a publication of `uri` with a diagnostic starting at `line`;
    /// the time since `t`. `None` if it does not come before the analysis
    /// settles (and 2 s more).
    fn diagnostic_at(&mut self, uri: &str, line: u32, t: Instant) -> Option<Duration> {
        let mut deadline = t + LIMIT;
        let mut idle_since: Option<Instant> = None;
        loop {
            let f = self.take(deadline)?;
            if f["method"] == "textDocument/publishDiagnostics"
                && f["params"]["uri"] == uri
                && f["params"]["diagnostics"].as_array().is_some_and(|d| {
                    d.iter()
                        .any(|d| d["range"]["start"]["line"].as_u64() == Some(line.into()))
                })
            {
                return Some(t.elapsed());
            }
            if f["method"] == "$/analyzerStatus" {
                if self.analyzing {
                    idle_since = None;
                    deadline = t + LIMIT;
                } else if idle_since.is_none() {
                    idle_since = Some(Instant::now());
                    deadline = Instant::now() + Duration::from_secs(2);
                }
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

const KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "assert",
    "async",
    "await",
    "base",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "dynamic",
    "else",
    "enum",
    "export",
    "extends",
    "extension",
    "external",
    "factory",
    "false",
    "final",
    "finally",
    "for",
    "Function",
    "get",
    "hide",
    "if",
    "implements",
    "import",
    "in",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "new",
    "null",
    "on",
    "operator",
    "part",
    "required",
    "rethrow",
    "return",
    "sealed",
    "set",
    "show",
    "static",
    "super",
    "switch",
    "sync",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "var",
    "void",
    "when",
    "while",
    "with",
    "yield",
];

/// Offsets of `count` identifiers spread over the code of `text` (outside
/// comments, strings and directives), at least three characters long.
fn identifier_positions(text: &str, count: usize) -> Vec<usize> {
    let b = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    let mut directive = false;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line_start = true;
            directive = false;
            i += 1;
            continue;
        }
        if line_start && !c.is_ascii_whitespace() {
            line_start = false;
            let rest = &text[i..];
            directive = ["import ", "export ", "part ", "library "]
                .iter()
                .any(|k| rest.starts_with(k));
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i = text[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
            continue;
        }
        if c == b'\'' || c == b'"' {
            let triple = b.get(i + 1) == Some(&c) && b.get(i + 2) == Some(&c);
            if triple {
                let close = std::str::from_utf8(&[c, c, c]).expect("ascii").to_string();
                i = text[i + 3..]
                    .find(&close)
                    .map_or(b.len(), |e| i + 3 + e + 3);
                continue;
            }
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let word = &text[start..i];
            if !directive && word.len() >= 3 && !KEYWORDS.contains(&word) {
                found.push(start);
            }
            continue;
        }
        i += 1;
    }
    if found.is_empty() || count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|k| found[(k * found.len()) / count + found.len() / (2 * count)])
        .collect()
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

/// Prints the summary of `v` and records the samples (ms) under `label`.
fn summary(results: &mut BTreeMap<String, Value>, label: &str, mut v: Vec<Duration>) {
    if v.is_empty() {
        return;
    }
    v.sort();
    let median = v[v.len() / 2];
    let p90 = v[(v.len() * 9 / 10).min(v.len() - 1)];
    println!(
        "{label}: n={} min {:.1} ms, median {:.1} ms, p90 {:.1} ms, max {:.1} ms",
        v.len(),
        ms(v[0]),
        ms(median),
        ms(p90),
        ms(*v.last().expect("non-empty"))
    );
    results.insert(
        label.to_string(),
        json!(
            v.iter()
                .map(|d| (ms(*d) * 10.0).round() / 10.0)
                .collect::<Vec<_>>()
        ),
    );
}

/// An open document of the session.
struct Document {
    uri: String,
    text: String,
    version: i32,
}

impl Document {
    fn open(c: &mut Client, path: &Path) -> Document {
        let uri = uri_of(path);
        let text = std::fs::read_to_string(path).expect("read the document");
        c.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
        );
        Document {
            uri,
            text,
            version: 1,
        }
    }

    fn insert(&mut self, c: &mut Client, at: usize, s: &str) {
        insert(c, &self.uri, &mut self.text, at, s, &mut self.version);
    }

    fn remove(&mut self, c: &mut Client, start: usize, end: usize) {
        remove(c, &self.uri, &mut self.text, start, end, &mut self.version);
    }
}

/// The package of the main file (the nearest directory with a
/// `pubspec.yaml`): `--root` may be a workspace of several packages.
fn main_package(o: &Options) -> PathBuf {
    o.file
        .ancestors()
        .skip(1)
        .find(|d| d.join("pubspec.yaml").is_file())
        .unwrap_or(&o.root)
        .to_path_buf()
}

/// The directory of the long session's generated files, in the main file's
/// package.
fn generated_dir(o: &Options) -> PathBuf {
    main_package(o).join("lib/df_long_session")
}

/// The undefined name the generated files of wave `wave` carry.
fn wave_marker(wave: usize) -> String {
    format!("dfWave{}", wave % 3)
}

/// Writes the generated files of wave `wave` at once (the session's start).
fn write_wave(o: &Options, wave: usize) {
    for (path, text) in wave_files(o, wave) {
        std::fs::write(&path, text).expect("write a generated file");
    }
}

/// The long session's generated files for wave `wave` (what `build_runner`
/// or `webdev` writes: a `.g.dart` per source, with members that change every
/// wave), each importing the main file, with their texts. Each one carries an
/// undefined name, [`wave_marker`], that changes every wave: both servers
/// must publish every file of the wave again, and the wave's time waits for
/// those publications. The names cycle through three versions, so the set of
/// distinct names does not grow with the session.
fn wave_files(o: &Options, wave: usize) -> Vec<(PathBuf, String)> {
    let dir = generated_dir(o);
    std::fs::create_dir_all(&dir).expect("the generated directory");
    let main = o
        .file
        .strip_prefix(main_package(o).join("lib"))
        .map(|r| format!("../{}", r.display()))
        .unwrap_or_else(|_| o.file.display().to_string());
    (0..o.generated)
        .map(|i| {
            let path = dir.join(format!("gen_{i}.g.dart"));
            let text = format!(
                "// GENERATED CODE - DO NOT MODIFY BY HAND (wave {wave})\n\
                 // ignore_for_file: unused_import\n\
                 import '{main}';\n\n\
                 class Gen{i}W{w} {{\n  final int value;\n  const Gen{i}W{w}(this.value);\n\
                 \x20 Map<String, Object?> toJson() => {{'value': value, 'wave': {wave}}};\n}}\n\n\
                 Gen{i}W{w} gen{i}FromJson(Map<String, Object?> json) => Gen{i}W{w}(json['value'] as int);\n\n\
                 Object? gen{i}Marker() => {marker};\n",
                w = wave % 3,
                marker = wave_marker(wave),
            );
            (path, text)
        })
        .collect()
}

/// A sample of the long session: seconds, RSS and footprint of all
/// processes (bytes), loads so far.
type Point = (f64, u64, u64, u64);

/// RSS and footprint of the server and all its descendants, in bytes.
fn rss_of_all(pid: u32) -> (u64, u64) {
    let mut rss = 0;
    let mut foot = 0;
    for p in std::iter::once(pid).chain(descendants(pid)) {
        if let Some(f) = footprint::of(p) {
            rss += f.rss;
            foot += f.footprint;
        }
    }
    (rss, foot)
}

/// The long session (`--long N`): N rounds of a wave of generated files
/// rewritten on the disk (one `didChangeWatchedFiles` per file), a body edit
/// and its undo in the main file, and every fifth round an API change and its
/// undo in `--api`; RSS and footprint of all processes sampled every 2 s into
/// `--csv`; then a quiet minute. `--long 0`: start, wait for the generated
/// files' publications, rest and report (the freshly started server, for the
/// same files).
///
/// Each load is timed from the change (a wave's last notification) to the
/// publication of the affected files' new diagnostics, and to settled: every
/// generated file with the wave's [`wave_marker`]; the edited file with, or
/// without, the diagnostic of the edited line (an unused local for a body
/// edit, a type error in a public function for an API change). Waiting for
/// `$/analyzerStatus` alone does not time a load: a server that debounces
/// disk changes starts after the driver's short wait for the start, and the
/// official server does not report the analysis of every edit; the time
/// measured was the driver's waiting windows.
fn long_session(o: &Options, c: &mut Client, doc: &mut Document, rounds: usize) {
    let pid = c.pid;
    let started = Instant::now();
    let loads = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let samples: Arc<Mutex<Vec<Point>>> = Arc::default();
    let sampler = {
        let (loads, stop, samples) = (Arc::clone(&loads), Arc::clone(&stop), Arc::clone(&samples));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let (rss, foot) = rss_of_all(pid);
                samples.lock().expect("samples").push((
                    started.elapsed().as_secs_f64(),
                    rss,
                    foot,
                    loads.load(Ordering::Relaxed),
                ));
                std::thread::sleep(Duration::from_secs(2));
            }
        })
    };
    let mut api = o.api.as_ref().map(|a| Document::open(c, a));
    c.settle_after(Instant::now());
    // Time per kind of load, from the change (a wave's last notification)
    // to the publication of the affected files' new diagnostics (every
    // generated file with the wave's marker; the edited file with, or
    // without, the diagnostic on the edited line), and to settled. A load
    // whose start the server did not report is counted apart: its time to
    // settled is the time to the publication.
    let mut kinds: [Vec<Duration>; 6] = Default::default();
    let mut unseen = [0u32; 6];
    let mut missing = [0u32; 6];
    let mut record = |k: usize, (published, settled, seen): (Option<Duration>, Duration, bool)| {
        match published {
            Some(d) => kinds[k].push(d),
            None => missing[k] += 1,
        }
        kinds[k + 1].push(settled);
        if !seen {
            unseen[k + 1] += 1;
        }
    };
    let generated: Vec<String> = wave_files(o, 0).iter().map(|(p, _)| uri_of(p)).collect();
    // The workspace's first analysis has reached the generated files (the
    // official server builds its contexts for seconds without reporting, and
    // on a loaded machine it may still be analyzing when the first analysis
    // looked settled): the rounds start after it.
    {
        let t = Instant::now();
        if !c.published_with(&generated, &wave_marker(0), t + LIMIT) {
            println!(
                "long session {}: the first wave was never published",
                o.label
            );
        }
        if c.analyzing {
            c.settle(t + LIMIT);
        }
        println!(
            "long session {}: the generated files published {:.0} ms after the first analysis",
            o.label,
            ms(t.elapsed())
        );
    }
    for round in 1..=rounds {
        // A wave: every generated file rewritten and reported as it is
        // written, one notification each, spread as a file watcher reports
        // them; timed from the last notification.
        let mut last = Instant::now();
        c.saw_start = false;
        for (path, text) in wave_files(o, round) {
            std::fs::write(&path, text).expect("write a generated file");
            c.notify(
                "workspace/didChangeWatchedFiles",
                json!({"changes": [{"uri": uri_of(&path), "type": 2}]}),
            );
            last = Instant::now();
            std::thread::sleep(Duration::from_millis(5));
        }
        let published = c
            .published_with(&generated, &wave_marker(round), last + LIMIT)
            .then(|| last.elapsed());
        if c.analyzing {
            c.settle(last + LIMIT);
        }
        record(0, (published, last.elapsed(), c.saw_start));
        loads.fetch_add(1, Ordering::Relaxed);
        // A body edit (an unused local: a warning on its line) and its undo.
        let at = body_insertion(&doc.text);
        let statement = "\n    var dfLongSession = 1;";
        c.saw_start = false;
        let t = Instant::now();
        doc.insert(c, at, statement);
        let (line, _) = position(&doc.text, at + 1);
        record(2, c.edit_timed(t, &doc.uri, line, true));
        c.saw_start = false;
        let t = Instant::now();
        doc.remove(c, at, at + statement.len());
        record(2, c.edit_timed(t, &doc.uri, line, false));
        loads.fetch_add(2, Ordering::Relaxed);
        // An API change (a public function with a type error) and its undo.
        if round % 5 == 0
            && let Some(a) = api.as_mut()
        {
            let at = a.text.rfind('}').map_or(a.text.len(), |i| i + 1);
            let declaration = format!("\nint dfLongSessionApi{round}() => '{round}';\n");
            c.saw_start = false;
            let t = Instant::now();
            a.insert(c, at, &declaration);
            let (line, _) = position(&a.text, at + 1);
            record(4, c.edit_timed(t, &a.uri, line, true));
            c.saw_start = false;
            let t = Instant::now();
            a.remove(c, at, at + declaration.len());
            record(4, c.edit_timed(t, &a.uri, line, false));
            loads.fetch_add(2, Ordering::Relaxed);
        }
        if round % 10 == 0 {
            let (rss, foot) = rss_of_all(pid);
            println!(
                "round {round}: {:.0} s, footprint {:.0} MiB, RSS {:.0} MiB",
                started.elapsed().as_secs_f64(),
                mib(foot),
                mib(rss)
            );
        }
    }
    let busy_end = started.elapsed().as_secs_f64();
    let names = [
        "wave to the generated files' diagnostics",
        "wave to settled",
        "body edit to the file's diagnostics",
        "body edit to settled",
        "API change to the file's diagnostics",
        "API change to settled",
    ];
    for (k, kind) in names.iter().enumerate() {
        let n = kinds[k].len();
        if n == 0 {
            continue;
        }
        let total: Duration = kinds[k].iter().sum();
        let mut sorted = kinds[k].clone();
        sorted.sort();
        println!(
            "long session {}: {kind}: {n} loads, {:.1} s in all, mean {:.0} ms, median {:.0} ms, p90 {:.0} ms ({} without a reported start, {} without the publication)",
            o.label,
            total.as_secs_f64(),
            ms(total) / n as f64,
            ms(sorted[n / 2]),
            ms(sorted[(n * 9 / 10).min(n - 1)]),
            unseen[k],
            missing[k],
        );
    }
    std::thread::sleep(Duration::from_secs(60));
    let (rest_rss, rest_foot) = rss_of_all(pid);
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let samples = samples.lock().expect("samples").clone();
    if let Some(path) = &o.csv {
        let mut out = String::from("seconds,rss_mib,footprint_mib,loads\n");
        for (t, rss, foot, l) in &samples {
            out.push_str(&format!("{t:.1},{:.1},{:.1},{l}\n", mib(*rss), mib(*foot)));
        }
        std::fs::write(path, out).expect("write the CSV");
    }
    // Least squares of the footprint over the busy part, after the first tenth
    // (the startup). The footprint, not the RSS: macOS compresses the pages of
    // a loaded machine, and the RSS of an idle process falls with it.
    let busy: Vec<&(f64, u64, u64, u64)> = samples
        .iter()
        .filter(|s| s.0 >= busy_end * 0.1 && s.0 <= busy_end)
        .collect();
    let slope = |x: &dyn Fn(&Point) -> f64| {
        let n = busy.len() as f64;
        if n < 2.0 {
            return 0.0;
        }
        let (sx, sy) = busy
            .iter()
            .fold((0.0, 0.0), |(a, b), s| (a + x(s), b + mib(s.2)));
        let (mx, my) = (sx / n, sy / n);
        let (num, den) = busy.iter().fold((0.0, 0.0), |(a, b), s| {
            (a + (x(s) - mx) * (mib(s.2) - my), b + (x(s) - mx).powi(2))
        });
        if den == 0.0 { 0.0 } else { num / den }
    };
    let per_hour = slope(&|s| s.0 / 3600.0);
    let per_100_loads = slope(&|s| s.3 as f64 / 100.0);
    let peak = samples.iter().map(|s| s.2).max().unwrap_or(0);
    let rss_peak = samples.iter().map(|s| s.1).max().unwrap_or(0);
    println!(
        "long session {}: {rounds} rounds, {} loads, {:.0} s busy; footprint peak {:.1} MiB, slope {:.1} MiB/h ({:.1} MiB per 100 loads), at rest after 60 s {:.1} MiB; RSS peak {:.1} MiB, at rest {:.1} MiB",
        o.label,
        loads.load(Ordering::Relaxed),
        busy_end,
        mib(peak),
        per_hour,
        per_100_loads,
        mib(rest_foot),
        mib(rss_peak),
        mib(rest_rss),
    );
}

/// Inserts `declaration` at `at` of `doc` `reps` times, measuring the time to
/// the diagnostic on its first line and to settled, and removes it each time.
fn diagnostic_edits(
    c: &mut Client,
    results: &mut BTreeMap<String, Value>,
    doc: &mut Document,
    at: usize,
    declaration: &str,
    label: &str,
    reps: usize,
) {
    let mut published = Vec::new();
    let mut settled = Vec::new();
    let mut cpu = Vec::new();
    let mut missing = 0;
    let mut republished = Vec::new();
    for _ in 0..reps {
        c.published.clear();
        let before = cpu_of_all(c.pid);
        let t = Instant::now();
        doc.insert(c, at, declaration);
        // The line of the declaration, after its leading newlines.
        let lead = declaration.len() - declaration.trim_start_matches('\n').len();
        let (line, _) = position(&doc.text, at + lead);
        match c.diagnostic_at(&doc.uri, line, t) {
            Some(d) => published.push(d),
            None => missing += 1,
        }
        if c.analyzing {
            c.settle(t + LIMIT);
        }
        settled.push(t.elapsed());
        cpu.push(Duration::from_nanos(
            cpu_of_all(c.pid).saturating_sub(before),
        ));
        let others = c.published.iter().filter(|u| **u != doc.uri).count();
        republished.push(others);
        doc.remove(c, at, at + declaration.len());
        c.settle_after(Instant::now());
    }
    summary(
        results,
        &format!("{label}: diagnostic published"),
        published,
    );
    summary(results, &format!("{label}: settled"), settled);
    summary(results, &format!("{label}: cpu"), cpu);
    println!("{label}: {missing} without the diagnostic, other files republished {republished:?}");
    results.insert(format!("{label}: missing"), json!(missing));
}

fn main() {
    let o = options();
    if o.long.is_some_and(|n| n > 0) {
        write_wave(&o, 0);
    }
    let mut child = Command::new(&o.command[0])
        .args(&o.command[1..])
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
                let s = footprint::of(pid);
                let server = s.as_ref().map_or(0, |f| f.footprint);
                let mut rss = s.as_ref().map_or(0, |f| f.rss);
                memory.server_now.store(server, Ordering::Relaxed);
                let mut kids = 0;
                for c in descendants(pid) {
                    if let Some(f) = footprint::of(c) {
                        kids += f.footprint;
                        rss += f.rss;
                        let mut p = peaks.lock().expect("peaks");
                        let e = p.entry(c).or_default();
                        *e = (*e).max(f.lifetime_max);
                        memory.child_peak.fetch_max(*e, Ordering::Relaxed);
                    }
                }
                memory
                    .total_peak
                    .fetch_max(server + kids, Ordering::Relaxed);
                memory.rss_peak.fetch_max(rss, Ordering::Relaxed);
                memory
                    .total_sum
                    .fetch_add((server + kids) >> 10, Ordering::Relaxed);
                memory.samples.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(25));
            }
        })
    };
    let mut c = Client {
        pid,
        input,
        reads: rx,
        next_id: 0,
        published: Vec::new(),
        diagnostics: BTreeMap::new(),
        analyzing: false,
        saw_start: false,
    };
    let mut results = BTreeMap::new();
    let root_uri = uri_of(&o.root);
    let started = Instant::now();
    c.request(
        "initialize",
        json!({"processId": std::process::id(), "rootUri": root_uri,
               "workspaceFolders": [{"uri": root_uri, "name": "root"}],
               "capabilities": {"textDocument": {"hover": {"contentFormat": ["markdown"]},
                                                 "rename": {"prepareSupport": true}}}}),
    );
    c.notify("initialized", json!({}));
    let mut main_doc = Document::open(&mut c, &o.file);
    // 1. The first workspace analysis: settled once idle for `QUIET`.
    let deadline = Instant::now() + LIMIT;
    // The official server reports idle once before it starts: only an idle
    // state after a busy one counts.
    let mut idle_at: Option<Duration> = None;
    let mut busy = false;
    loop {
        let wait = match idle_at {
            Some(idle) if !c.analyzing => (started + idle + QUIET).min(deadline),
            _ => deadline,
        };
        match c.take(wait) {
            Some(f) => {
                if f["method"] == "$/analyzerStatus" {
                    busy |= c.analyzing;
                    idle_at = (busy && !c.analyzing).then(|| started.elapsed());
                    if std::env::var_os("EDITING_LATENCY_TRACE").is_some() {
                        println!("status {} at {:.0} ms", c.analyzing, ms(started.elapsed()));
                    }
                }
            }
            None if idle_at.is_some() => break,
            None => panic!("no first analysis in time"),
        }
    }
    let workspace = idle_at.expect("settled");
    let published = c.published.len();
    let with_diagnostics = c
        .diagnostics
        .values()
        .filter(|d| d.as_array().is_some_and(|d| !d.is_empty()))
        .count();
    println!(
        "first analysis: workspace settled at {:.0} ms, {} publications ({} files with diagnostics); server footprint {:.1} MiB",
        ms(workspace),
        published,
        with_diagnostics,
        mib(memory.server_now.load(Ordering::Relaxed))
    );
    results.insert("startup".to_string(), json!(ms(workspace)));
    let startup_cpu = cpu_of_all(pid);
    println!("first analysis: CPU {:.0} ms", startup_cpu as f64 / 1e6);
    results.insert("startup: cpu".to_string(), json!(startup_cpu as f64 / 1e6));
    if let Some(rounds) = o.long {
        long_session(&o, &mut c, &mut main_doc, rounds);
        c.request("shutdown", Value::Null);
        c.notify("exit", Value::Null);
        let _ = child.wait();
        stop.store(true, Ordering::Relaxed);
        let _ = sampler.join();
        return;
    }
    // 2. Typing a statement, waiting for each version to settle.
    let at = body_insertion(&main_doc.text);
    let statement = "    final dfProbe = [1, 2].length;\n";
    let mut latencies = Vec::new();
    let mut cpu = Vec::new();
    let mut typed_at = at;
    for ch in statement.chars().take(o.edits) {
        let before = cpu_of_all(pid);
        let t = Instant::now();
        main_doc.insert(&mut c, typed_at, &ch.to_string());
        typed_at += ch.len_utf8();
        latencies.push(c.settle_after(t));
        cpu.push(Duration::from_nanos(cpu_of_all(pid).saturating_sub(before)));
    }
    summary(&mut results, "keystroke: settled", latencies);
    summary(&mut results, "keystroke: cpu", cpu);
    // 3. A burst without waiting.
    c.published.clear();
    let burst_at = typed_at;
    let mut last = Instant::now();
    for i in 0..o.burst {
        let s = if i % 2 == 0 { "x" } else { "y" };
        last = Instant::now();
        main_doc.insert(&mut c, burst_at + i, s);
        std::thread::sleep(o.interval);
        c.drain();
    }
    c.wait_start();
    c.settle(last + LIMIT);
    let own = c.published.iter().filter(|u| **u == main_doc.uri).count();
    println!(
        "burst of {} edits every {} ms: settled {:.0} ms after the last keystroke ({} publications of the file)",
        o.burst,
        o.interval.as_millis(),
        ms(last.elapsed()),
        own
    );
    results.insert("burst: settled".to_string(), json!(ms(last.elapsed())));
    // Undo the typing and the burst (one edit): the document is the file again.
    main_doc.remove(&mut c, at, burst_at + o.burst);
    c.settle_after(Instant::now());
    // 4. Edits whose diagnostic must be published.
    diagnostic_edits(
        &mut c,
        &mut results,
        &mut main_doc,
        at,
        "    int.parse(0);\n",
        "body edit",
        o.reps,
    );
    let mut docs = vec![main_doc];
    for (path, declaration, label) in [
        (
            &o.private,
            "\nvoid _dfPrivateProbe() {}\n",
            "private API edit",
        ),
        (&o.api, "\nint dfApiProbe() => '';\n", "exported API edit"),
    ] {
        let Some(path) = path else { continue };
        let uri = uri_of(path);
        let index = match docs.iter().position(|d| d.uri == uri) {
            Some(i) => i,
            None => {
                docs.push(Document::open(&mut c, path));
                c.settle_after(Instant::now());
                docs.len() - 1
            }
        };
        let doc = &mut docs[index];
        let end = doc.text.len();
        diagnostic_edits(&mut c, &mut results, doc, end, declaration, label, o.reps);
    }
    let mut main_doc = docs.swap_remove(0);
    // 5. Queries at identifiers of the file.
    let positions = identifier_positions(&main_doc.text, o.positions);
    let uri = main_doc.uri.clone();
    // The latency and the server's CPU time of one request.
    let query = |c: &mut Client, method: &str, text: &str, offset: usize| -> (Duration, Duration) {
        let (line, character) = position(text, offset);
        let at = json!({"line": line, "character": character});
        let params = match method {
            "textDocument/completion" => {
                let (l, ch) = position(text, offset + 2);
                json!({"textDocument": {"uri": uri}, "position": {"line": l, "character": ch}})
            }
            "textDocument/references" => {
                json!({"textDocument": {"uri": uri}, "position": at, "context": {"includeDeclaration": true}})
            }
            "textDocument/codeAction" => {
                json!({"textDocument": {"uri": uri}, "range": {"start": at, "end": at},
                       "context": {"diagnostics": []}})
            }
            _ => json!({"textDocument": {"uri": uri}, "position": at}),
        };
        let before = cpu_of(c.pid);
        let (r, d) = c.request(method, params);
        let cpu = Duration::from_nanos(cpu_of(c.pid).saturating_sub(before));
        if let Some(e) = r.get("error") {
            println!("{method} at {line}:{character}: error {e}");
        }
        if method == "textDocument/references" {
            let n = r["result"].as_array().map_or(0, Vec::len);
            println!("references at {line}:{character}: {n} locations");
        }
        (d, cpu)
    };
    let methods = [
        "textDocument/hover",
        "textDocument/completion",
        "textDocument/definition",
        "textDocument/references",
        "textDocument/prepareRename",
        "textDocument/codeAction",
    ];
    if !positions.is_empty() {
        let edit = "    0;\n";
        for method in methods {
            let name = method.trim_start_matches("textDocument/");
            let mut after = Vec::new();
            let mut after_cpu = Vec::new();
            for r in 0..o.reps {
                main_doc.insert(&mut c, at, edit);
                c.settle_after(Instant::now());
                let p = positions[r % positions.len()];
                let p = if p >= at { p + edit.len() } else { p };
                let (d, cpu) = query(&mut c, method, &main_doc.text.clone(), p);
                after.push(d);
                after_cpu.push(cpu);
                main_doc.remove(&mut c, at, at + edit.len());
                c.settle_after(Instant::now());
            }
            summary(&mut results, &format!("{name}: after edit"), after);
            summary(&mut results, &format!("{name}: after edit cpu"), after_cpu);
            let mut warm = Vec::new();
            let mut warm_cpu = Vec::new();
            for _ in 0..2 {
                for &p in &positions {
                    let (d, cpu) = query(&mut c, method, &main_doc.text.clone(), p);
                    warm.push(d);
                    warm_cpu.push(cpu);
                }
            }
            summary(&mut results, &format!("{name}: warm"), warm);
            summary(&mut results, &format!("{name}: warm cpu"), warm_cpu);
        }
    }
    // 6. Everything is the files again: the final diagnostics.
    c.drain();
    std::thread::sleep(Duration::from_secs(2));
    c.drain();
    let rest = footprint::of(pid).map_or(0, |f| f.footprint);
    let rest_total = rest
        + descendants(pid)
            .into_iter()
            .filter_map(footprint::of)
            .map(|f| f.footprint)
            .sum::<u64>();
    let resident: Vec<(u64, u64)> = descendants(pid)
        .into_iter()
        .filter_map(footprint::of)
        .map(|f| (f.footprint, f.lifetime_max))
        .collect();
    c.request("shutdown", Value::Null);
    let server_peak = footprint::of(pid).map_or(0, |f| f.lifetime_max);
    c.notify("exit", Value::Null);
    let status = child.wait().expect("server exit");
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let samples = memory.samples.load(Ordering::Relaxed).max(1);
    let mean = (memory.total_sum.load(Ordering::Relaxed) / samples) << 10;
    println!(
        "memory: server footprint peak {:.1} MiB, at rest {:.1} MiB, descendant peak {:.1} MiB, all processes: footprint peak (sampled) {:.1} MiB, mean {:.1} MiB, at rest {:.1} MiB, RSS peak {:.1} MiB; exit {status}",
        mib(server_peak),
        mib(rest),
        mib(memory.child_peak.load(Ordering::Relaxed)),
        mib(memory.total_peak.load(Ordering::Relaxed)),
        mib(mean),
        mib(rest_total),
        mib(memory.rss_peak.load(Ordering::Relaxed)),
    );
    for (now, max) in resident {
        println!(
            "descendant process: footprint at rest {:.1} MiB, peak {:.1} MiB",
            mib(now),
            mib(max)
        );
    }
    let total = started.elapsed();
    println!("total {:.1} s", total.as_secs_f64());
    if let Some(path) = &o.json {
        let memory = json!({
            "server_peak_mib": mib(server_peak),
            "server_rest_mib": mib(rest),
            "descendant_peak_mib": mib(memory.child_peak.load(Ordering::Relaxed)),
            "total_peak_mib": mib(memory.total_peak.load(Ordering::Relaxed)),
            "total_mean_mib": mib(mean),
            "total_rest_mib": mib(rest_total),
            "rss_peak_mib": mib(memory.rss_peak.load(Ordering::Relaxed)),
        });
        let out = json!({
            "server": o.label,
            "root": o.root,
            "latency_ms": results,
            "memory": memory,
            "total_s": total.as_secs_f64(),
            "diagnostics": c.diagnostics,
        });
        std::fs::write(path, serde_json::to_vec_pretty(&out).expect("json")).expect("write --json");
    }
    drop(docs);
}
