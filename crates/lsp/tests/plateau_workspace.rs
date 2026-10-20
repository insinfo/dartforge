//! Server memory with a workspace folder open and the typed analysis running.
//!
//! The regression this catches (`docs/LSP.md`, "Memória"): with the workspace
//! root open, the typed worker analyzed every package in the server process,
//! and the resident memory stayed at the peak of that analysis (hundreds of
//! MiB on `new_sali`, 97% of it pages the allocator kept after the analysis
//! dropped). The binary now runs each analysis in a child process
//! ([`dartforge_lsp::Server::analyze_in_child_processes`]); the server keeps
//! the documents and the published diagnostics.
//!
//! Measured with the counting allocator, over the real typed flow (the same
//! workspace package analyzed in the worker thread and in child processes):
//!
//! * the server's peak during the workspace analysis in child processes is a
//!   fraction of the in-thread one (the program, trees and tables never come
//!   into the server);
//! * rounds of open, edit (waiting for each typed publication) and close end at
//!   the same live level: nothing grows with the number of analyses.
//!
//! A separate binary with a single `#[test]`: the counting allocator is global
//! to the process (see `plateau_documents.rs`).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

mod common;

use common::Project;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(120);

/// Files of the workspace package.
const FILES: usize = 40;

/// The text of file `i`: a class, a function that uses the previous file and,
/// in every fifth file, a typed error (`non_bool_condition`).
fn source(i: usize, revision: usize) -> String {
    let import = if i > 0 {
        format!("import 'f{}.dart';\n", i - 1)
    } else {
        String::new()
    };
    let previous = if i > 0 {
        format!("  var p = C{}(); p.m(); // {revision:03}\n", i - 1)
    } else {
        format!("  // {revision:03}\n")
    };
    let error = if i.is_multiple_of(5) {
        "  if (1) {}\n"
    } else {
        ""
    };
    format!(
        "{import}class C{i} {{\n  int field = {i};\n  int m() => field + 1;\n  String get name => 'c{i}';\n}}\n\nvoid f{i}() {{\n{previous}{error}  final list = <int>[1, 2, 3];\n  for (final x in list) {{\n    print(x);\n  }}\n}}\n"
    )
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// A project with the workspace package written to disk.
fn workspace(name: &str) -> Project {
    let p = Project::new(name);
    for i in 0..FILES {
        p.write(&format!("lib/f{i}.dart"), &source(i, 0));
    }
    p
}

/// `initialized` (the workspace folder is analyzed and published), then waits
/// for the analysis; returns the number of files published.
fn analyze_workspace(p: &mut Project) -> usize {
    p.server
        .receive(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    let mut outputs = p.server.pump();
    outputs.extend(p.server.await_diagnostics(LIMIT));
    outputs
        .iter()
        .filter(|m| m["method"] == "textDocument/publishDiagnostics")
        .count()
}

/// One round: opens ten files, edits each four times waiting for the typed
/// publication, and closes them.
fn round(p: &mut Project, revision: &mut usize) {
    for i in 0..10 {
        let uri = p.uri(&format!("lib/f{i}.dart"));
        p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source(i, *revision)}
        }}));
        p.server.pump();
    }
    p.server.await_diagnostics(LIMIT);
    for v in 2..=5 {
        for i in 0..10 {
            *revision += 1;
            p.change(&format!("lib/f{i}.dart"), v, &source(i, *revision % 1000));
        }
        p.server.await_diagnostics(LIMIT);
    }
    for i in 0..10 {
        let uri = p.uri(&format!("lib/f{i}.dart"));
        p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}));
        p.server.pump();
    }
    p.server.await_diagnostics(LIMIT);
}

#[test]
fn workspace_analysis_stays_out_of_the_server() {
    // The analysis in the worker thread: its peak is the program of the package.
    let mut in_thread = workspace("plateau-workspace-thread");
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let published = analyze_workspace(&mut in_thread);
    let thread_peak = dartforge_instrument::peak_bytes() - base;
    assert!(
        published >= FILES / 5,
        "the workspace errors were not published ({published})"
    );
    drop(in_thread);

    // The same package with each analysis in a child process.
    let mut p = workspace("plateau-workspace-child");
    p.server
        .analyze_in_child_processes(PathBuf::from(env!("CARGO_BIN_EXE_dartforge-lsp")));
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let published = analyze_workspace(&mut p);
    let child_peak = dartforge_instrument::peak_bytes() - base;
    assert!(
        published >= FILES / 5,
        "the workspace errors were not published ({published})"
    );
    eprintln!(
        "workspace analysis: server peak {:.2} MiB in the thread, {:.2} MiB with child processes",
        mib(thread_peak),
        mib(child_peak)
    );
    assert!(
        child_peak * 4 < thread_peak,
        "with child processes the server's peak ({:.2} MiB) should be a fraction of the in-thread analysis' ({:.2} MiB)",
        mib(child_peak),
        mib(thread_peak)
    );

    // Rounds of open, edit and close: the same live level after each one.
    let mut revision = 0;
    round(&mut p, &mut revision);
    let plateau = dartforge_instrument::live_bytes();
    let mut levels = Vec::new();
    for _ in 0..3 {
        round(&mut p, &mut revision);
        levels.push(dartforge_instrument::live_bytes());
    }
    eprintln!("live after each round: {:?} (plateau {plateau})", levels);
    for level in &levels {
        assert!(
            level.abs_diff(plateau) <= 64 * 1024,
            "the live level moved with the rounds: {levels:?} against {plateau}"
        );
    }
}
