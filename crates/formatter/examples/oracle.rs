//! Formatter oracle: compares, byte for byte, the output of
//! `dartforge_formatter::formatar` with that of `dart format` from SDK 3.6.2
//! over a corpus of `.dart` files.
//!
//! ```text
//! cargo run --release -p dartforge-formatter --example oracle -- \
//!     [--dart <dart>] [--cache <dir>] [--diffs <dir>] [--list] [<dir|file>...]
//! ```
//!
//! Without paths, uses the default corpus: the repository's `.dart` files
//! (except `target/` and `references/`), the SDK's `lib/` and, if present,
//! the pub cache (`~/tools/dart/pub-cache/hosted/pub.dev`). Dart's reference
//! outputs are cached by content (64-bit FNV-1a) in
//! `target/formatter-oracle/`: `<hash>.out` with the output, `<hash>.err`
//! when `dart format` rejects the file. `dart format -o json` runs in
//! batches, in parallel.
//!
//! The score counts only the files `dart format` accepts; files both reject
//! are reported separately.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use dartforge_formatter::{ErroFormatacao, Opcoes, formatar};

fn fnv(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn collect(root: &Path, output: &mut Vec<PathBuf>) {
    if root.is_file() {
        if root.extension().is_some_and(|e| e == "dart") {
            output.push(root.to_path_buf());
        }
        return;
    }
    let Ok(inputs) = fs::read_dir(root) else {
        return;
    };
    let mut children: Vec<PathBuf> = inputs.filter_map(|e| e.ok().map(|e| e.path())).collect();
    children.sort();
    for f in children {
        let name_ = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name_.starts_with('.') || name_ == "target" || name_ == "references" || name_ == "node_modules" {
            continue;
        }
        if f.is_symlink() {
            continue;
        }
        collect(&f, output);
    }
}

/// Runs `dart format -o json` on a batch and writes each file's cache entry.
fn references(dart: &str, cache: &Path, lote: &[(PathBuf, u64)]) {
    let mut cmd = Command::new(dart);
    cmd.args(["format", "-o", "json", "--summary", "none"]);
    for (p, _) in lote {
        cmd.arg(p);
    }
    let output = match cmd.output() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to run {dart}: {e}");
            return;
        }
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut by_path: BTreeMap<String, String> = BTreeMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let (Some(p), Some(s)) = (v["path"].as_str(), v["source"].as_str()) {
            by_path.insert(p.to_string(), s.to_string());
        }
    }
    for (p, h) in lote {
        let key = p.to_string_lossy().to_string();
        match by_path.get(&key) {
            Some(s) => {
                let _ = fs::write(cache.join(format!("{h:016x}.out")), s);
            }
            None => {
                let _ = fs::write(cache.join(format!("{h:016x}.err")), "");
            }
        }
    }
}

#[derive(Default)]
struct Scoreboard {
    total: usize,
    equal: usize,
    different: usize,
    rejected: usize,
    panics: usize,
    unexpected_output: usize,
    both_reject: usize,
    only_dart_rejects: usize,
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut args = std::env::args().skip(1);
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repository root");
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dart = format!("{home}/tools/dart/3.6.2/dart-sdk/bin/dart");
    let mut cache = repo_root.join("target/formatter-oracle");
    let mut diffs: Option<PathBuf> = None;
    let mut list_mode = false;
    let mut paths: Vec<PathBuf> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dart" => dart = args.next().expect("--dart <exe>"),
            "--cache" => cache = PathBuf::from(args.next().expect("--cache <dir>")),
            "--diffs" => diffs = Some(PathBuf::from(args.next().expect("--diffs <dir>"))),
            "--list" => list_mode = true,
            _ => paths.push(PathBuf::from(a)),
        }
    }
    let sets: Vec<(String, Vec<PathBuf>)> = if paths.is_empty() {
        let mut v = vec![
            ("repository".into(), vec![repo_root.clone()]),
            ("sdk/lib".into(), vec![PathBuf::from(format!("{home}/tools/dart/3.6.2/dart-sdk/lib"))]),
        ];
        let pub_cache = PathBuf::from(format!("{home}/tools/dart/pub-cache/hosted/pub.dev"));
        if pub_cache.is_dir() {
            v.push(("pub-cache".into(), vec![pub_cache]));
        }
        v
    } else {
        vec![("selected".into(), paths)]
    };
    fs::create_dir_all(&cache).expect("cache directory");
    if let Some(d) = &diffs {
        fs::create_dir_all(d).expect("diffs directory");
    }

    let mut general = Scoreboard::default();
    for (name_, roots) in &sets {
        let mut files = Vec::new();
        for r in roots {
            collect(r, &mut files);
        }
        let with_hash: Vec<(PathBuf, u64, Vec<u8>)> = files
            .into_iter()
            .filter_map(|p| fs::read(&p).ok().map(|d| (p, fnv(&d), d)))
            .collect();
        let missing: Vec<(PathBuf, u64)> = with_hash
            .iter()
            .filter(|(_, h, _)| !cache.join(format!("{h:016x}.out")).exists() && !cache.join(format!("{h:016x}.err")).exists())
            .map(|(p, h, _)| (p.clone(), *h))
            .collect();
        if !missing.is_empty() {
            eprintln!("{name_}: {} new dart format references…", missing.len());
            let batches: Vec<&[(PathBuf, u64)]> = missing.chunks(150).collect();
            let next_batch = Mutex::new(0usize);
            std::thread::scope(|s| {
                for _ in 0..8 {
                    s.spawn(|| {
                        loop {
                            let i = {
                                let mut g = next_batch.lock().expect("lock");
                                let i = *g;
                                *g += 1;
                                i
                            };
                            let Some(lote) = batches.get(i) else {
                                break;
                            };
                            references(&dart, &cache, lote);
                        }
                    });
                }
            });
        }

        let scoreboard = Mutex::new(Scoreboard::default());
        let next_batch = Mutex::new(0usize);
        let failures = Mutex::new(Vec::<(PathBuf, &'static str)>::new());
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    loop {
                        let i = {
                            let mut g = next_batch.lock().expect("lock");
                            let i = *g;
                            *g += 1;
                            i
                        };
                        let Some((p, h, data)) = with_hash.get(i) else {
                            break;
                        };
                        let reference = fs::read_to_string(cache.join(format!("{h:016x}.out"))).ok();
                        let source = String::from_utf8_lossy(data).to_string();
                        let ours = std::panic::catch_unwind(|| formatar(&source, &Opcoes::default()));
                        let mut pl = scoreboard.lock().expect("lock");
                        match (reference, ours) {
                            (None, Ok(Err(_))) => pl.both_reject += 1,
                            (None, _) => pl.only_dart_rejects += 1,
                            (Some(r), result) => {
                                pl.total += 1;
                                let kind_ = match result {
                                    Ok(Ok(output)) if output == r => {
                                        pl.equal += 1;
                                        None
                                    }
                                    Ok(Ok(output)) => {
                                        pl.different += 1;
                                        if let Some(d) = &diffs {
                                            let base = format!("{h:016x}");
                                            let _ = fs::write(d.join(format!("{base}.dart.ours")), &output);
                                            let _ = fs::write(d.join(format!("{base}.dart.dart")), &r);
                                            let _ = fs::write(d.join(format!("{base}.dart.source")), &source);
                                            let _ = fs::write(d.join(format!("{base}.path")), p.to_string_lossy().as_bytes());
                                        }
                                        Some("different")
                                    }
                                    Ok(Err(ErroFormatacao::Sintaxe(_))) => {
                                        pl.rejected += 1;
                                        Some("rejected")
                                    }
                                    Ok(Err(ErroFormatacao::SaidaInesperada { .. })) => {
                                        pl.unexpected_output += 1;
                                        Some("unexpected output")
                                    }
                                    Err(_) => {
                                        pl.panics += 1;
                                        Some("panic")
                                    }
                                };
                                if let Some(t) = kind_ {
                                    failures.lock().expect("lock").push((p.clone(), t));
                                }
                            }
                        }
                    }
                });
            }
        });
        let pl = scoreboard.into_inner().expect("score");
        println!(
            "{name_}: {}/{} identical ({:.1}%) — different {}, rejected {}, unexpected output {}, panics {}; dart rejects {} (we do too: {})",
            pl.equal,
            pl.total,
            100.0 * pl.equal as f64 / pl.total.max(1) as f64,
            pl.different,
            pl.rejected,
            pl.unexpected_output,
            pl.panics,
            pl.only_dart_rejects + pl.both_reject,
            pl.both_reject
        );
        if list_mode {
            let mut f = failures.into_inner().expect("failures");
            f.sort();
            for (p, t) in f {
                println!("  {t}: {}", p.display());
            }
        }
        general.total += pl.total;
        general.equal += pl.equal;
    }
    if sets.len() > 1 {
        println!(
            "total: {}/{} identical ({:.1}%)",
            general.equal,
            general.total,
            100.0 * general.equal as f64 / general.total.max(1) as f64
        );
    }
}
