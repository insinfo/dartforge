//! Editor-like sessions against the real `dartforge-lsp` binary, over the
//! wire: what VS Code does in a working day, compressed.
//!
//! * `multi_package_editing_session`: a two-package project (an app with a
//!   path dependency) and a minimal SDK on disk; the SDK report after
//!   `initialized`, `$/setTrace`, hover, definition across packages,
//!   references, completion, typing character by character through syntax
//!   errors with completion and hover in between, rename, formatting and the
//!   other queries, a `package_config.json` change announced through
//!   `workspace/didChangeWatchedFiles`, rounds of edits with the server's
//!   resident memory measured, `didClose`, `shutdown` and `exit`. No response
//!   may be an internal error, nothing may be shown as an error, `stderr` may
//!   not report panics or ignored notifications, and the process must exit
//!   with 0.
//! * `missing_sdk_is_reported_once`: without any SDK, one
//!   `window/showMessage` warning naming `dartforge.sdkPath`.
//! * `sdk_path_option_overrides_discovery`: `initializationOptions.sdkPath`.
//! * `real_project_session`: the same pressure on a real project named by
//!   `DARTFORGE_LSP_EDITOR_PROJECT` (a package with a resolved
//!   `.dart_tool/package_config.json`), with the SDK the binary discovers;
//!   skipped when the variable is not set. Nothing is written to that project.

mod common;

use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufReader, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Maximum wait for one response (a debug binary loading a real SDK is slow).
const TIME_LIMIT: Duration = Duration::from_secs(120);

/// The Dart server's `UnhandledError`: what a contained panic answers.
const UNHANDLED: i64 = -32001;

enum Read {
    Frame(Value),
    End,
}

/// The server in a child process, with `stderr` collected.
struct Session {
    child: Child,
    input: Option<ChildStdin>,
    reads: Receiver<Read>,
    /// Notifications and responses not consumed yet.
    pending: Vec<Value>,
    /// Every notification seen, for the final checks.
    notifications: Vec<Value>,
    stderr: Arc<Mutex<String>>,
    next_id: i64,
    /// `(method, error)` of every request answered with an error.
    errors: Vec<(String, Value)>,
}

impl Session {
    fn start(variables: &[(&str, Option<&str>)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge-lsp"));
        command
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in variables {
            match value {
                Some(v) => command.env(key, v),
                None => command.env_remove(key),
            };
        }
        let mut child = command.spawn().expect("spawn dartforge-lsp");
        let input = child.stdin.take();
        let output = child.stdout.take().expect("stdout");
        let mut error_output = child.stderr.take().expect("stderr");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            while let Ok(Some(f)) = read_message(&mut reader) {
                if tx.send(Read::Frame(f)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Read::End);
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&stderr);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(n) = error_output.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                sink.lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&buffer[..n]));
            }
        });
        Self {
            child,
            input,
            reads: rx,
            pending: Vec::new(),
            notifications: Vec::new(),
            stderr,
            next_id: 100,
            errors: Vec::new(),
        }
    }

    fn send(&mut self, message: &Value) {
        let input = self.input.as_mut().expect("stdin open");
        input.write_all(&frame(message)).expect("write to stdin");
        input.flush().expect("flush stdin");
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Takes one frame: answers the server's requests (registration,
    /// configuration, progress) like a client, records notifications.
    fn take(&mut self, deadline: Instant, what: &str) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match self.reads.recv_timeout(remaining) {
            Ok(Read::Frame(f)) => {
                if f.get("method").is_some() && f.get("id").is_some() {
                    let result = if f["method"] == "workspace/configuration" {
                        let n = f["params"]["items"].as_array().map_or(1, Vec::len);
                        json!(vec![json!({}); n])
                    } else {
                        Value::Null
                    };
                    let id = f["id"].clone();
                    self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
                } else {
                    if f.get("id").is_none() {
                        self.notifications.push(f.clone());
                    }
                    self.pending.push(f);
                }
            }
            Ok(Read::End) => panic!(
                "the server closed stdout while waiting for {what}; stderr:\n{}",
                self.stderr()
            ),
            Err(_) => panic!(
                "timed out waiting for {what} (the server hangs?); stderr:\n{}",
                self.stderr()
            ),
        }
    }

    fn wait_for(&mut self, what: &str, mut condition: impl FnMut(&Value) -> bool) -> Value {
        let deadline = Instant::now() + TIME_LIMIT;
        loop {
            if let Some(i) = self.pending.iter().position(&mut condition) {
                return self.pending.remove(i);
            }
            self.take(deadline, what);
        }
    }

    /// Sends a request and waits for its response; records errors.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send(&message);
        let response = self.wait_for(&format!("{method} (id {id})"), |m| {
            m["id"] == json!(id) && m.get("method").is_none()
        });
        if let Some(e) = response.get("error") {
            self.errors
                .push((format!("{method} {}", message["params"]), e.clone()));
        }
        // Drop the publications that piled up, so the queue stays small.
        self.pending
            .retain(|m| m["method"] != "textDocument/publishDiagnostics");
        response
    }

    fn at(
        &mut self,
        method: &str,
        uri: &str,
        (line, character): (u32, u32),
        extra: Value,
    ) -> Value {
        let mut params = json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}});
        if let Value::Object(more) = extra {
            for (k, v) in more {
                params[k] = v;
            }
        }
        self.request(method, params)
    }

    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }

    /// Resident memory of the server process, in KiB (`ps`; `None` where
    /// unavailable).
    fn resident_kib(&self) -> Option<u64> {
        if cfg!(windows) {
            return None;
        }
        let out = Command::new("ps")
            .args(["-o", "rss=", "-p", &self.child.id().to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    /// `shutdown` + `exit`; returns the exit code (the process must end in time).
    fn finish(mut self) -> (i32, String, Vec<Value>, Vec<(String, Value)>) {
        let r = self.request("shutdown", Value::Null);
        assert_eq!(r["result"], Value::Null, "shutdown answers null: {r}");
        self.notify("exit", Value::Null);
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(30);
        let code = loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                break status.code().unwrap_or(-1);
            }
            assert!(
                Instant::now() < deadline,
                "the server did not exit after `exit`"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        // Give the stderr reader a moment to drain.
        std::thread::sleep(Duration::from_millis(50));
        let stderr = self.stderr();
        while let Ok(Read::Frame(f)) = self.reads.try_recv() {
            if f.get("id").is_none() {
                self.notifications.push(f);
            }
        }
        (code, stderr, self.notifications, self.errors)
    }
}

/// The capabilities of a VS Code-like client.
fn client_capabilities() -> Value {
    json!({
        "workspace": {
            "applyEdit": true,
            "configuration": true,
            "workspaceEdit": {"documentChanges": true, "resourceOperations": ["create", "rename", "delete"]},
            "didChangeWatchedFiles": {"dynamicRegistration": true},
            "didChangeConfiguration": {"dynamicRegistration": true},
            "workspaceFolders": true,
        },
        "window": {"workDoneProgress": true},
        "textDocument": {
            "synchronization": {"dynamicRegistration": true, "didSave": true},
            "hover": {"contentFormat": ["markdown", "plaintext"]},
            "completion": {"completionItem": {"snippetSupport": true, "documentationFormat": ["markdown"]}},
            "rename": {"prepareSupport": true},
            "documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
            "codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}},
            "semanticTokens": {"requests": {"full": true}, "tokenTypes": [], "tokenModifiers": [], "formats": ["relative"]},
        },
    })
}

fn uri_of(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

/// `initialize` + `initialized` + `$/setTrace`.
fn handshake(s: &mut Session, root: Option<&Path>, options: Value) -> Value {
    let root_uri = root.map(uri_of);
    let folders = root_uri
        .as_ref()
        .map(|u| json!([{"uri": u, "name": "root"}]))
        .unwrap_or(Value::Null);
    let init = s.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "workspaceFolders": folders,
               "capabilities": client_capabilities(), "initializationOptions": options, "trace": "off"}),
    );
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    s.notify("initialized", json!({}));
    s.notify("$/setTrace", json!({"value": "messages"}));
    init
}

fn log_messages(notifications: &[Value], method: &str) -> Vec<String> {
    notifications
        .iter()
        .filter(|n| n["method"] == method)
        .map(|n| n["params"]["message"].as_str().unwrap_or("").to_string())
        .collect()
}

/// The checks every session ends with.
fn assert_healthy(code: i32, stderr: &str, notifications: &[Value], errors: &[(String, Value)]) {
    assert_eq!(code, 0, "exit after shutdown is 0; stderr:\n{stderr}");
    let internal: Vec<_> = errors
        .iter()
        .filter(|(_, e)| e["code"] == json!(UNHANDLED))
        .collect();
    let logged: Vec<_> = notifications
        .iter()
        .filter(|n| n["method"] == "window/logMessage" && n["params"]["type"] == 1)
        .map(|n| n["params"]["message"].as_str().unwrap_or(""))
        .collect();
    assert!(
        internal.is_empty(),
        "internal errors: {internal:?}\nlogged errors: {logged:?}\nstderr:\n{stderr}"
    );
    let shown_errors: Vec<_> = notifications
        .iter()
        .filter(|n| n["method"] == "window/showMessage" && n["params"]["type"] == 1)
        .collect();
    assert!(shown_errors.is_empty(), "errors shown: {shown_errors:?}");
    assert!(!stderr.contains("panic"), "a panic was logged:\n{stderr}");
    assert!(
        !stderr.contains("notification ignored"),
        "ignored notifications logged:\n{stderr}"
    );
}

/// A minimal SDK (the `dart:core` of the semantic tests) in `root/sdk`.
fn write_sdk(root: &Path) -> PathBuf {
    let lib = root.join("sdk/lib");
    fs::create_dir_all(lib.join("core")).unwrap();
    fs::write(
        lib.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
    )
    .unwrap();
    fs::write(lib.join("core/core.dart"), common::CORE).unwrap();
    fs::write(root.join("sdk/version"), "3.6.2\n").unwrap();
    root.join("sdk")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/lsp-editor-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Scratch(fs::canonicalize(root).unwrap())
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // The typed analysis caches the minimal SDK by path: remove that too.
        if let Ok(sdk) =
            dartforge_elements::sdk::SdkLayout::load(&self.0.join("sdk/lib"), "dartdevc")
        {
            let _ = fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SHAPES: &str = "abstract class Shape {\n  double area();\n  String get label => 'shape';\n}\n\nclass Circle extends Shape {\n  final double radius;\n  Circle(this.radius);\n  @override\n  double area() => radius * radius * 3;\n}\n\nclass Square extends Shape {\n  final double side;\n  Square(this.side);\n  @override\n  double area() => side * side;\n}\n";

const SERVICE: &str = "import 'package:util/util.dart';\nimport 'model.dart';\n\nclass Service {\n  final List<Shape> shapes = [];\n\n  void add(Shape shape) {\n    shapes.add(shape);\n  }\n\n  double total() {\n    var sum = 0.0;\n    return sum;\n  }\n\n  Report report() => Report(shapes.length);\n}\n\nShape unit() => Circle(1);\n";

const MODEL: &str = "class Report {\n  final int count;\n  const Report(this.count);\n  String describe() => 'shapes: $count';\n}\n";

const MAIN: &str = "import 'package:app/src/service.dart';\nimport 'package:util/util.dart';\n\nvoid main() {\n  final service = Service();\n  service.add(Circle(2));\n  service.add(Square(3));\n  print(service.total());\n  print(service.report().describe());\n}\n";

/// Line and UTF-16 column of the `n`-th occurrence of `needle` (plus `delta` columns).
fn position(text: &str, needle: &str, n: usize, delta: u32) -> (u32, u32) {
    let at = text
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("no {needle:?} #{n}"))
        .0;
    let before = &text[..at];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    (line, column + delta)
}

fn hover_text(response: &Value) -> String {
    let c = &response["result"]["contents"];
    c["value"].as_str().or(c.as_str()).unwrap_or("").to_string()
}

#[test]
fn multi_package_editing_session() {
    let dir = Scratch::new("multi");
    let sdk = write_sdk(&dir.0);
    dir.write(
        "util/pubspec.yaml",
        "name: util\nenvironment:\n  sdk: ^3.6.0\n",
    );
    dir.write(
        "util/lib/util.dart",
        "export 'src/shapes.dart';\nexport 'src/strings.dart';\n",
    );
    dir.write("util/lib/src/shapes.dart", SHAPES);
    dir.write(
        "util/lib/src/strings.dart",
        "extension Shout on String {\n  String shout() => toUpperCase();\n}\n",
    );
    dir.write(
        "app/pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  util:\n    path: ../util\n",
    );
    let config = |extra: &str| {
        format!(
            r#"{{"configVersion":2,"packages":[{{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}},{{"name":"util","rootUri":"../../util","packageUri":"lib/","languageVersion":"3.6"}}{extra}]}}"#
        )
    };
    dir.write("app/.dart_tool/package_config.json", &config(""));
    let service_path = dir.write("app/lib/src/service.dart", SERVICE);
    let model_path = dir.write("app/lib/src/model.dart", MODEL);
    let main_path = dir.write("app/bin/main.dart", MAIN);
    let app = dir.0.join("app");
    let (service, model, main) = (
        uri_of(&service_path),
        uri_of(&model_path),
        uri_of(&main_path),
    );
    let shapes_uri = uri_of(&dir.0.join("util/lib/src/shapes.dart"));

    let sdk_lib = sdk.join("lib");
    let mut s = Session::start(&[
        ("DARTFORGE_SDK_LIB", Some(sdk_lib.to_str().unwrap())),
        ("DARTFORGE_LSP_DEBUG", None),
        ("RUST_LOG", None),
    ]);
    handshake(&mut s, Some(&app), json!({}));
    let used = s.wait_for("the SDK log", |m| {
        m["method"] == "window/logMessage"
            && m["params"]["message"]
                .as_str()
                .is_some_and(|t| t.starts_with("Using the Dart SDK"))
    });
    let used = used["params"]["message"].as_str().unwrap().to_string();
    assert!(
        used.contains("DARTFORGE_SDK_LIB") && used.contains("3.6.2"),
        "{used}"
    );

    let mut versions = std::collections::HashMap::new();
    for (uri, text) in [(&service, SERVICE), (&model, MODEL), (&main, MAIN)] {
        s.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
        );
        versions.insert(uri.clone(), 1);
    }

    // Hover and definition across packages.
    let h = s.at(
        "textDocument/hover",
        &main,
        position(MAIN, "Circle(2)", 0, 1),
        json!({}),
    );
    assert!(hover_text(&h).contains("Circle"), "{h}");
    let d = s.at(
        "textDocument/definition",
        &main,
        position(MAIN, "Circle(2)", 0, 1),
        json!({}),
    );
    let target = d["result"]
        .as_array()
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_else(|| d["result"].clone());
    assert_eq!(target["uri"], json!(shapes_uri), "{d}");
    let d = s.at(
        "textDocument/definition",
        &main,
        position(MAIN, "describe", 0, 2),
        json!({}),
    );
    assert!(d["result"].to_string().contains("model.dart"), "{d}");

    // References to `Report` in the app.
    let r = s.at(
        "textDocument/references",
        &model,
        position(MODEL, "Report", 0, 1),
        json!({"context": {"includeDeclaration": true}}),
    );
    let references = r["result"].as_array().map_or(0, Vec::len);
    assert!(references >= 2, "{r}");

    // Completion after `shape.`
    let mut text = SERVICE.to_string();
    let mut version = 1;
    let edit = |s: &mut Session,
                text: &mut String,
                version: &mut i32,
                at: usize,
                remove: usize,
                insert: &str| {
        let start = position_of_offset(text, at);
        let end = position_of_offset(text, at + remove);
        text.replace_range(at..at + remove, insert);
        *version += 1;
        s.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": service, "version": *version},
                   "contentChanges": [{"range": {"start": {"line": start.0, "character": start.1},
                                                  "end": {"line": end.0, "character": end.1}},
                                       "text": insert}]}),
        );
    };

    // Typing a statement character by character inside `total()`, with
    // completion, hover and semantic tokens in between: most intermediate
    // texts do not parse.
    let typed = "    for (final s in shapes) {\n      sum += s.area();\n    }\n";
    let mut cursor = text.find("    return sum;").unwrap();
    for (i, ch) in typed.chars().enumerate() {
        let mut buffer = [0u8; 4];
        edit(
            &mut s,
            &mut text,
            &mut version,
            cursor,
            0,
            ch.encode_utf8(&mut buffer),
        );
        cursor += ch.len_utf8();
        let here = position_of_offset(&text, cursor);
        if i % 3 == 0 {
            let c = s.at("textDocument/completion", &service, here, json!({}));
            assert!(c.get("result").is_some() || c.get("error").is_some());
        }
        if i % 7 == 0 {
            s.at("textDocument/hover", &service, here, json!({}));
            s.at("textDocument/signatureHelp", &service, here, json!({}));
        }
        if i % 11 == 0 {
            s.request(
                "textDocument/semanticTokens/full",
                json!({"textDocument": {"uri": service}}),
            );
            s.request(
                "textDocument/codeAction",
                json!({"textDocument": {"uri": service},
                       "range": {"start": {"line": here.0, "character": 0}, "end": {"line": here.0, "character": here.1}},
                       "context": {"diagnostics": []}}),
            );
        }
    }
    assert!(text.contains(typed));
    let dot = text.find("s.area()").unwrap() + 2;
    let c = s.at(
        "textDocument/completion",
        &service,
        position_of_offset(&text, dot),
        json!({}),
    );
    let labels: Vec<String> = c["result"]["items"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|i| i["label"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        labels.iter().any(|l| l.starts_with("area")),
        "completion after `s.`: {labels:?}"
    );

    // A broken paste and its undo.
    let junk = "class {{{ ( 'unterminated\n";
    let at = text.find("class Service").unwrap();
    edit(&mut s, &mut text, &mut version, at, 0, junk);
    s.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": service}}),
    );
    s.request(
        "textDocument/foldingRange",
        json!({"textDocument": {"uri": service}}),
    );
    s.at(
        "textDocument/hover",
        &service,
        position_of_offset(&text, at + 3),
        json!({}),
    );
    edit(&mut s, &mut text, &mut version, at, junk.len(), "");
    assert!(text.starts_with("import"));

    // Rename `Report` (declared in model.dart, used in service.dart and main.dart).
    let prepare = s.at(
        "textDocument/prepareRename",
        &model,
        position(MODEL, "Report", 0, 1),
        json!({}),
    );
    assert!(prepare["result"].is_object(), "{prepare}");
    let rename = s.at(
        "textDocument/rename",
        &model,
        position(MODEL, "Report", 0, 1),
        json!({"newName": "Summary"}),
    );
    let edit_text = rename["result"].to_string();
    assert!(
        edit_text.contains("model.dart") && edit_text.contains("service.dart"),
        "{rename}"
    );

    // Formatting a badly formatted document.
    let ugly = "class   Report{final int count;const Report(this.count);\nString describe()=>'shapes: $count';}\n";
    versions.insert(model.clone(), 2);
    s.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": model, "version": 2}, "contentChanges": [{"text": ugly}]}),
    );
    let f = s.request(
        "textDocument/formatting",
        json!({"textDocument": {"uri": model}, "options": {"tabSize": 2, "insertSpaces": true}}),
    );
    assert!(f["result"].as_array().is_some_and(|l| !l.is_empty()), "{f}");

    // The other queries an editor sends while browsing.
    for (method, params) in [
        (
            "textDocument/documentHighlight",
            json!({"textDocument": {"uri": main}, "position": {"line": 4, "character": 10}}),
        ),
        (
            "textDocument/inlayHint",
            json!({"textDocument": {"uri": main}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 9, "character": 0}}}),
        ),
        (
            "textDocument/documentLink",
            json!({"textDocument": {"uri": main}}),
        ),
        (
            "textDocument/implementation",
            json!({"textDocument": {"uri": shapes_uri}, "position": {"line": 0, "character": 16}}),
        ),
        (
            "textDocument/typeDefinition",
            json!({"textDocument": {"uri": main}, "position": {"line": 4, "character": 10}}),
        ),
        ("workspace/symbol", json!({"query": "Circ"})),
        (
            "textDocument/codeLens",
            json!({"textDocument": {"uri": main}}),
        ),
    ] {
        s.request(method, params);
    }

    // A new package appears: `package_config.json` changes on disk and the
    // client announces it. The open file that imports it must resolve.
    dir.write("extra/pubspec.yaml", "name: extra\n");
    dir.write(
        "extra/lib/extra.dart",
        "/// From the extra package.\nint extraValue() => 42;\n",
    );
    dir.write(
        "app/.dart_tool/package_config.json",
        &config(r#",{"name":"extra","rootUri":"../../extra","packageUri":"lib/","languageVersion":"3.6"}"#),
    );
    s.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [
            {"uri": uri_of(&app.join(".dart_tool/package_config.json")), "type": 2},
            {"uri": uri_of(&app.join("pubspec.yaml")), "type": 2},
        ]}),
    );
    let main2 = format!("import 'package:extra/extra.dart';\n{MAIN}").replace(
        "  print(service.total());",
        "  print(service.total() + extraValue());",
    );
    s.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": main, "version": 2}, "contentChanges": [{"text": main2}]}),
    );
    let h = s.at(
        "textDocument/hover",
        &main,
        position(&main2, "extraValue()", 0, 2),
        json!({}),
    );
    assert!(
        hover_text(&h).contains("extraValue"),
        "the new package resolves after the watched-files change: {h}"
    );

    // Rounds of edits: the resident memory must not grow with their number.
    let mut samples = Vec::new();
    for round in 0..8 {
        for k in 0..25 {
            let body = format!("    sum += {round}.{k};\n");
            let at = text.find("    return sum;").unwrap();
            edit(&mut s, &mut text, &mut version, at, 0, &body);
            if k % 5 == 0 {
                let here = position_of_offset(&text, at + 6);
                s.at("textDocument/hover", &service, here, json!({}));
                s.at("textDocument/completion", &service, here, json!({}));
            }
            let len = body.len();
            edit(&mut s, &mut text, &mut version, at, len, "");
        }
        s.request(
            "textDocument/semanticTokens/full",
            json!({"textDocument": {"uri": service}}),
        );
        if let Some(kib) = s.resident_kib() {
            samples.push(kib);
        }
    }
    println!("resident KiB after each round: {samples:?}");
    if samples.len() == 8 {
        let base = samples[2];
        let last = *samples.last().unwrap();
        assert!(
            last <= base + 64 * 1024,
            "resident memory grew {} MiB across rounds of edits: {samples:?}",
            (last.saturating_sub(base)) / 1024
        );
    }

    for uri in [&service, &model, &main] {
        s.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": uri}}),
        );
    }
    s.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": service}, "position": {"line": 0, "character": 0}}),
    );

    let (code, stderr, notifications, errors) = s.finish();
    assert_healthy(code, &stderr, &notifications, &errors);
    assert!(
        log_messages(&notifications, "window/showMessage").is_empty(),
        "nothing is shown with a working SDK: {:?}",
        log_messages(&notifications, "window/showMessage")
    );
}

/// A document opened before `pub get` takes the tool's current language
/// version (3.13, where a `final` parameter is an error); when
/// `package_config.json` appears with `languageVersion: 3.6` and the client
/// announces it, the open document is diagnosed again in 3.6 without being
/// reopened.
#[test]
fn package_config_change_reaches_open_documents() {
    let dir = Scratch::new("config");
    let sdk = write_sdk(&dir.0);
    dir.write("app/pubspec.yaml", "name: app\n");
    let text = "void f(final int x) {\n  print(x);\n}\n";
    let file = dir.write("app/lib/a.dart", text);
    let app = dir.0.join("app");
    let uri = uri_of(&file);
    let sdk_lib = sdk.join("lib");
    let mut s = Session::start(&[
        ("DARTFORGE_SDK_LIB", Some(sdk_lib.to_str().unwrap())),
        ("DARTFORGE_LSP_TYPED", Some("0")),
    ]);
    handshake(&mut s, Some(&app), json!({}));
    s.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
    );
    let first = s.wait_for("the first publication", |m| {
        m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == json!(uri)
    });
    assert!(
        first["params"]["diagnostics"]
            .as_array()
            .is_some_and(|l| !l.is_empty()),
        "3.13 refuses the `final` parameter: {first}"
    );
    dir.write(
        "app/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    );
    s.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri_of(&app.join(".dart_tool/package_config.json")), "type": 1}]}),
    );
    let second = s.wait_for("the publication after the configuration change", |m| {
        m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == json!(uri)
    });
    assert_eq!(
        second["params"]["diagnostics"],
        json!([]),
        "3.6 accepts the `final` parameter: {second}"
    );
    let (code, stderr, notifications, errors) = s.finish();
    assert_healthy(code, &stderr, &notifications, &errors);
}

/// Generated code nests deeply: a 30 000-term `+` chain overflowed the 8 MiB
/// main thread of a release build, 5 000 terms that of a debug build (a stack
/// overflow aborts; no `catch_unwind` contains it). The dispatcher now runs on
/// a thread with a large stack.
#[test]
fn deeply_nested_code_does_not_abort_the_server() {
    let dir = Scratch::new("deep");
    let sdk = write_sdk(&dir.0);
    let text = format!(
        "var x = {};\nvoid main() {{ print(x); }}\n",
        vec!["1"; 5_000].join(" + ")
    );
    let file = dir.write("a.dart", &text);
    let uri = uri_of(&file);
    let sdk_lib = sdk.join("lib");
    let mut s = Session::start(&[
        ("DARTFORGE_SDK_LIB", Some(sdk_lib.to_str().unwrap())),
        ("DARTFORGE_LSP_TYPED", Some("0")),
    ]);
    handshake(&mut s, None, json!({}));
    s.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}),
    );
    s.at("textDocument/hover", &uri, (1, 21), json!({}));
    s.at("textDocument/completion", &uri, (1, 21), json!({}));
    for method in [
        "textDocument/semanticTokens/full",
        "textDocument/documentSymbol",
        "textDocument/foldingRange",
    ] {
        s.request(method, json!({"textDocument": {"uri": uri}}));
    }
    let (code, stderr, notifications, errors) = s.finish();
    assert_healthy(code, &stderr, &notifications, &errors);
}

/// (line, UTF-16 column) of a byte offset.
fn position_of_offset(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    (line, column)
}

/// Variables that hide every SDK the machine may have, except well-known
/// install locations (which the test then tolerates).
fn without_sdk(home: &Path) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("DARTFORGE_SDK_LIB", None),
        ("DART_SDK", None),
        ("FLUTTER_ROOT", None),
        ("DARTFORGE_HOME", None),
        ("PATH", Some(String::new())),
        ("HOME", Some(home.display().to_string())),
        ("USERPROFILE", Some(home.display().to_string())),
    ]
}

#[test]
fn missing_sdk_is_reported_once() {
    let dir = Scratch::new("missing");
    let home = dir.0.join("home");
    fs::create_dir_all(&home).unwrap();
    let variables = without_sdk(&home);
    let borrowed: Vec<(&str, Option<&str>)> =
        variables.iter().map(|(k, v)| (*k, v.as_deref())).collect();
    let mut s = Session::start(&borrowed);
    handshake(&mut s, None, json!({}));
    let uri = uri_of(&dir.0.join("a.dart"));
    s.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": "void main() { print(1); }\n"}}),
    );
    let symbols = s.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": uri}}),
    );
    assert!(
        symbols["result"].is_array(),
        "syntactic features still answer: {symbols}"
    );
    s.notify("$/setTrace", json!({"value": "off"}));
    s.notify("$/someFutureNotification", json!({}));
    let (code, stderr, notifications, errors) = s.finish();
    assert_healthy(code, &stderr, &notifications, &errors);
    let logs = log_messages(&notifications, "window/logMessage");
    if logs
        .iter()
        .any(|l| l.contains("a well-known install location"))
    {
        eprintln!("an SDK is installed in a well-known location; nothing more to check");
        return;
    }
    let shown = log_messages(&notifications, "window/showMessage");
    assert_eq!(shown.len(), 1, "one warning: {shown:?}");
    assert!(shown[0].contains("dartforge.sdkPath"), "{}", shown[0]);
    assert!(
        logs.iter().any(|l| l.starts_with("No Dart SDK found")),
        "{logs:?}"
    );
}

#[test]
fn sdk_path_option_overrides_discovery() {
    let dir = Scratch::new("option");
    let sdk = write_sdk(&dir.0);
    let file = dir.write("a.dart", "int twice(int x) => x + x;\nvar y = twice(2);\n");
    let mut s = Session::start(&[("DARTFORGE_SDK_LIB", Some("/no/such/sdk/lib"))]);
    handshake(&mut s, None, json!({"sdkPath": sdk.to_str().unwrap()}));
    let uri = uri_of(&file);
    s.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1,
                                "text": "int twice(int x) => x + x;\nvar y = twice(2);\n"}}),
    );
    let h = s.at("textDocument/hover", &uri, (1, 9), json!({}));
    assert!(hover_text(&h).contains("twice"), "{h}");
    let (code, stderr, notifications, errors) = s.finish();
    assert_healthy(code, &stderr, &notifications, &errors);
    let logs = log_messages(&notifications, "window/logMessage");
    assert!(
        logs.iter()
            .any(|l| l.starts_with("Using the Dart SDK")
                && l.contains("sdkPath initialization option")),
        "{logs:?}"
    );
    assert!(log_messages(&notifications, "window/showMessage").is_empty());
}

/// Identifier positions of `text` worth querying: the start of every
/// `.name` and of names after `new `/`= `/`(`, at most `limit`.
fn interesting_positions(text: &str, limit: usize) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for i in 1..bytes.len() {
        let previous = bytes[i - 1];
        if (previous == b'.' || previous == b'(' || previous == b' ')
            && bytes[i].is_ascii_alphabetic()
        {
            out.push(i);
        }
    }
    let step = (out.len() / limit.max(1)).max(1);
    out.into_iter().step_by(step).take(limit).collect()
}

#[test]
fn real_project_session() {
    let Ok(project) = std::env::var("DARTFORGE_LSP_EDITOR_PROJECT") else {
        eprintln!("DARTFORGE_LSP_EDITOR_PROJECT not set; test skipped");
        return;
    };
    let root = fs::canonicalize(&project).expect("project folder");
    let mut files = Vec::new();
    collect_dart(&root.join("lib"), &mut files);
    collect_dart(&root.join("bin"), &mut files);
    files.sort();
    let per_file = 6;
    let max_files: usize = std::env::var("DARTFORGE_LSP_EDITOR_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    files.truncate(max_files);
    assert!(!files.is_empty(), "no .dart files in {}", root.display());

    let mut s = Session::start(&[]);
    handshake(&mut s, Some(&root), json!({}));
    let started = Instant::now();
    let mut samples = Vec::new();
    for (n, file) in files.iter().enumerate() {
        let Ok(original) = fs::read_to_string(file) else {
            continue;
        };
        let uri = uri_of(file);
        s.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": original}}),
        );
        for at in interesting_positions(&original, per_file) {
            let here = position_of_offset(&original, at);
            s.at("textDocument/hover", &uri, here, json!({}));
            s.at("textDocument/definition", &uri, here, json!({}));
            s.at("textDocument/completion", &uri, here, json!({}));
        }
        s.request(
            "textDocument/semanticTokens/full",
            json!({"textDocument": {"uri": uri}}),
        );
        s.request(
            "textDocument/formatting",
            json!({"textDocument": {"uri": uri}, "options": {"tabSize": 2, "insertSpaces": true}}),
        );
        // Typing a broken line in the middle of the file, then removing it.
        let mut text = original.clone();
        let middle = text[..text.len() / 2].rfind('\n').map_or(0, |i| i + 1);
        let typed = "  final x = foo.bar(baz, [1, 2";
        let mut version = 1;
        for (i, ch) in typed.char_indices() {
            let at = middle + i;
            let p = position_of_offset(&text, at);
            text.insert(at, ch);
            version += 1;
            s.notify(
                "textDocument/didChange",
                json!({"textDocument": {"uri": uri, "version": version},
                       "contentChanges": [{"range": {"start": {"line": p.0, "character": p.1}, "end": {"line": p.0, "character": p.1}}, "text": ch.to_string()}]}),
            );
            if i % 4 == 0 {
                let here = position_of_offset(&text, at + 1);
                s.at("textDocument/completion", &uri, here, json!({}));
            }
        }
        version += 1;
        s.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": original}]}),
        );
        if n % 3 == 0 {
            s.notify(
                "textDocument/didClose",
                json!({"textDocument": {"uri": uri}}),
            );
        }
        if let Some(kib) = s.resident_kib() {
            samples.push(kib);
        }
    }
    s.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri_of(&root.join(".dart_tool/package_config.json")), "type": 2}]}),
    );
    s.request("workspace/symbol", json!({"query": "a"}));
    println!(
        "{} files in {:.1} s; resident KiB after each file: {samples:?}",
        files.len(),
        started.elapsed().as_secs_f64()
    );
    let (code, stderr, notifications, errors) = s.finish();
    let others: Vec<_> = errors
        .iter()
        .filter(|(_, e)| e["code"] != json!(UNHANDLED))
        .map(|(m, e)| format!("{m}: {e}"))
        .collect();
    println!("non-internal error responses: {others:?}");
    assert_healthy(code, &stderr, &notifications, &errors);
}

fn collect_dart(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_dart(&p, out);
        } else if p.extension().is_some_and(|x| x == "dart") {
            out.push(p);
        }
    }
}

/// A map entry typed inside a set literal (`{1, 2` typed before a line
/// holding `key: value`, met in a real project): the set's single type
/// parameter was indexed as a map's second one. The panic was contained, but
/// every `didChange` and query on the file failed until the text changed. In
/// the analyzer, the entry contributes `dynamic` as the element type
/// (`_toSetType`, `_computeElementType`), in sets and lists alike.
#[test]
fn map_entry_inside_a_set_or_list_literal_is_inferred() {
    let mut p = common::Project::new("editor-map-entry-in-set");
    let variants = [
        // Both shapes required: `ambiguous_set_or_map_literal_both`, `dynamic`.
        (
            "void g() {\n  var x = {1, 'a': 3};\n  print(x);\n}\n",
            "dynamic x",
        ),
        (
            "void g() {\n  var x = {1, 2\n    'a': 3};\n  print(x);\n}\n",
            "dynamic x",
        ),
        (
            "void g() {\n  var x = <int>{1, 'a': 3};\n  print(x);\n}\n",
            "Set<int> x",
        ),
        (
            "void g() {\n  var x = [1, 'a': 3];\n  print(x);\n}\n",
            "List<",
        ),
    ];
    for (n, (text, expected)) in variants.iter().enumerate() {
        let file = format!("lib/v{n}.dart");
        p.open(&file, text);
        let h = p.in_position("textDocument/hover", &file, 1, 6, json!({}));
        assert!(h.get("error").is_none(), "variant {n}: {h}");
        assert!(hover_text(&h).contains(expected), "variant {n}: {h}");
        let c = p.in_position("textDocument/completion", &file, 2, 8, json!({}));
        assert!(c.get("error").is_none(), "variant {n}: {c}");
    }
}

/// An analyzer that fails on demand, for the panic containment.
struct Failing;

impl dartforge_lsp::Analyzer for Failing {
    fn diagnose(&mut self, _uri: &str, text: &str) -> Vec<dartforge_diagnostics::Diagnostic> {
        assert!(!text.contains("boom"), "diagnose failed");
        Vec::new()
    }

    fn hover(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<dartforge_lsp::Hover> {
        panic!("hover failed")
    }
}

/// A panic in a request answers `UnhandledError` and the session goes on; a
/// panic in a notification is logged every time but shown to the user once
/// per method, not once per keystroke.
#[test]
fn panics_are_contained_and_shown_once() {
    let mut server = dartforge_lsp::Server::with_analyzer(Failing);
    server.receive(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}}),
    );
    server.receive(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
    let uri = "file:///failing/a.dart";
    server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": "boom"}}}));
    for version in 2..6 {
        server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": "boom boom"}]}}));
    }
    server.receive(
        json!({"jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 1}}}),
    );
    server.receive(
        json!({"jsonrpc": "2.0", "id": 3, "method": "textDocument/documentSymbol",
        "params": {"textDocument": {"uri": uri}}}),
    );
    let mut outputs = Vec::new();
    while server.has_pending() {
        outputs.extend(server.pump());
    }
    let count = |method: &str, ty: i64| {
        outputs
            .iter()
            .filter(|m| m["method"] == method && m["params"]["type"] == ty)
            .count()
    };
    assert_eq!(
        count("window/showMessage", 1),
        2,
        "didOpen and didChange, once each: {outputs:?}"
    );
    assert_eq!(
        count("window/logMessage", 1),
        6,
        "every failure is logged: {outputs:?}"
    );
    let hover = outputs
        .iter()
        .find(|m| m["id"] == 2)
        .expect("hover answered");
    assert_eq!(hover["error"]["code"], json!(UNHANDLED), "{hover}");
    let symbols = outputs
        .iter()
        .find(|m| m["id"] == 3)
        .expect("symbols answered");
    assert!(
        symbols["result"].is_array(),
        "the session goes on: {symbols}"
    );
}
