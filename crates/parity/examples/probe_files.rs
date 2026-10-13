//! Compares our side with the oracle, file by file: every diagnostic of both
//! sides in offset order, marked `=` (hit), `~` (hit with a different
//! message), `-` (oracle only) or `+` (ours only).
//!
//! ```text
//! cargo run --release -p dartforge-parity --example probe_files -- <group> <relative file>...
//! cargo run --release -p dartforge-parity --example probe_files -- --livre <dir> [<relative file>...]
//! ```
//!
//! With a corpus group, the oracle is the recorded one. With `--livre`, `<dir>`
//! becomes a language 3.6 package (pubspec and package_config written on the
//! spot) and the oracle is `dart analyze` 3.6.2 run now over the files (every
//! `.dart` in the directory, if none is given): useful to check a minimal case
//! before porting a rule. The files are analyzed together, in a single batch
//! (as in the scoreboard, each one is its own library), without the
//! publication filters.

use dartforge_parity::analysis::Engine;
use dartforge_parity::corpus::{self, Group};
use dartforge_parity::oracle::{self, Record, OracleSdk};
use dartforge_parity::{Execution, filters, run_ours};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((first, rest)) = args.split_first() else {
        eprintln!("usage: probe_files <group> <file>... | --livre <dir> [<file>...]");
        std::process::exit(2);
    };
    let (dir, cfg, oracle, rels) = if first == "--livre" {
        let Some((d, rels)) = rest.split_first() else {
            eprintln!("--livre needs the directory");
            std::process::exit(2);
        };
        free(Path::new(d), rels)
    } else {
        from_corpus(first, rest)
    };
    let engine = Engine::discover().expect("SDK");
    let sources: Vec<PathBuf> = rels.iter().map(|r| dir.join(r)).collect();
    let ex = Execution {
        workers: 1,
        batch_size: sources.len().max(1),
        progress: false,
        isolate: None,
        max_time: std::time::Duration::from_secs(600),
    };
    let r = run_ours(&engine, &dir, &sources, Some(&cfg), &filters::AnalysisOptions::read(&dir), &ex);
    let mut sum = [0usize; 4];
    for rel in &rels {
        let rel = rel.replace('\\', "/");
        println!("== {rel}");
        // (offset, length, code, side) → record; side 0 oracle, 1 ours.
        let mut lines: BTreeMap<(usize, usize, String, u8), &Record> = BTreeMap::new();
        for o in oracle.iter().filter(|o| o.file == rel) {
            lines.insert((o.offset, o.length, o.code.clone(), 0), o);
        }
        for n in r.records.iter().filter(|n| n.file == rel) {
            lines.insert((n.offset, n.length, n.code.clone(), 1), n);
        }
        let mut seen = BTreeSet::new();
        for ((off, len, code, side), reg) in &lines {
            if !seen.insert((*off, *len, code.clone())) {
                continue;
            }
            let pair = if *side == 0 { lines.get(&(*off, *len, code.clone(), 1)) } else { None };
            let (mark, i) = match (side, pair) {
                (0, Some(n)) if n.problem_message == reg.problem_message => ("=", 0),
                (0, Some(_)) => ("~", 1),
                (0, None) => ("-", 2),
                _ => ("+", 3),
            };
            sum[i] += 1;
            println!("{mark} {}:{} (+{len}) {code}: {}", reg.line, reg.column, reg.problem_message);
            if let (0, Some(n)) = (side, pair) {
                if n.problem_message != reg.problem_message {
                    println!("    ours: {}", n.problem_message);
                }
            }
        }
    }
    println!("total: = {} | ~ {} | - {} | + {}", sum[0], sum[1], sum[2], sum[3]);
    if !r.panics.is_empty() {
        println!("panics: {:?}", r.panics);
    }
}

type Setup = (PathBuf, PathBuf, Vec<Record>, Vec<String>);

/// A corpus group, with the recorded oracle.
fn from_corpus(group: &str, rels: &[String]) -> Setup {
    let root = corpus::corpus_root();
    let root = std::path::absolute(&root).unwrap_or(root);
    let Some((_, g)) = corpus::groups(&root).into_iter().find(|(n, _)| n == group) else {
        eprintln!("unknown group: {group}");
        std::process::exit(2);
    };
    let dir = root.join(group);
    let cfg = corpus::prepare(&dir, group, &g).expect("prepare the group");
    let (oracle, _) = oracle::read(&dir).expect("recorded oracle");
    (dir, cfg, oracle, rels.to_vec())
}

/// Any directory, with the 3.6.2 oracle run now.
fn free(d: &Path, rels: &[String]) -> Setup {
    let dir = std::path::absolute(d).expect("directory");
    let name = "sonda_livre";
    std::fs::write(dir.join("pubspec.yaml"), format!("name: {name}\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n"))
        .expect("pubspec");
    let g = Group { sdk: OracleSdk::V362, packages: Vec::new(), origin: String::new() };
    let cfg = corpus::prepare(&dir, name, &g).expect("package_config");
    let rels: Vec<String> = if rels.is_empty() {
        corpus::dart_files(&dir).iter().filter_map(|a| oracle::relative(a, &dir)).collect()
    } else {
        rels.to_vec()
    };
    let targets: Vec<PathBuf> = rels.iter().map(|r| dir.join(r)).collect();
    let cache = std::env::temp_dir().join("dartforge-sonda-cache");
    let diags = oracle::run(OracleSdk::V362, &dir, &targets, &cache).expect("dart analyze 3.6.2");
    let regs = diags.iter().filter_map(|j| Record::from_json(j, &dir)).collect();
    (dir, cfg, regs, rels)
}
