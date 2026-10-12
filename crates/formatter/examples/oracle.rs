//! Oráculo do formatador: compara, byte a byte, a saída de
//! `dartforge_formatter::formatar` com a do `dart format` do SDK 3.6.2 sobre
//! um corpus de arquivos `.dart`.
//!
//! ```text
//! cargo run --release -p dartforge-formatter --example oraculo -- \
//!     [--dart <dart>] [--cache <dir>] [--diffs <dir>] [--list] [<dir|arquivo>...]
//! ```
//!
//! Sem caminhos, usa o corpus padrão: os `.dart` do repositório (menos
//! `target/` e `references/`), a `lib/` do SDK e, se existir, o pub cache
//! (`~/tools/dart/pub-cache/hosted/pub.dev`). As referências do Dart ficam
//! em cache por conteúdo (FNV-1a de 64 bits) em `target/oraculo-formatar/`:
//! `<hash>.out` com a saída, `<hash>.err` quando o `dart format` recusa o
//! arquivo. O `dart format -o json` roda em lotes, em paralelo.
//!
//! O placar conta só os arquivos que o `dart format` aceita; arquivos que os
//! dois recusam saem à parte.

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

/// Roda o `dart format -o json` num lote e grava o cache de cada arquivo.
fn references(dart: &str, cache: &Path, lote: &[(PathBuf, u64)]) {
    let mut cmd = Command::new(dart);
    cmd.args(["format", "-o", "json", "--summary", "none"]);
    for (p, _) in lote {
        cmd.arg(p);
    }
    let output = match cmd.output() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("falha ao rodar {dart}: {e}");
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
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("raiz do repositório");
    let matches_here = std::env::var("HOME").unwrap_or_default();
    let mut dart = format!("{matches_here}/tools/dart/3.6.2/dart-sdk/bin/dart");
    let mut cache = repo_root.join("target/oraculo-formatar");
    let mut diffs: Option<PathBuf> = None;
    let mut list_mode = false;
    let mut paths: Vec<PathBuf> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dart" => dart = args.next().expect("--dart <exe>"),
            "--cache" => cache = PathBuf::from(args.next().expect("--cache <dir>")),
            "--diffs" => diffs = Some(PathBuf::from(args.next().expect("--diffs <dir>"))),
            "--lista" => list_mode = true,
            _ => paths.push(PathBuf::from(a)),
        }
    }
    let sets: Vec<(String, Vec<PathBuf>)> = if paths.is_empty() {
        let mut v = vec![
            ("repositório".into(), vec![repo_root.clone()]),
            ("sdk/lib".into(), vec![PathBuf::from(format!("{matches_here}/tools/dart/3.6.2/dart-sdk/lib"))]),
        ];
        let pub_cache = PathBuf::from(format!("{matches_here}/tools/dart/pub-cache/hosted/pub.dev"));
        if pub_cache.is_dir() {
            v.push(("pub-cache".into(), vec![pub_cache]));
        }
        v
    } else {
        vec![("selecionados".into(), paths)]
    };
    fs::create_dir_all(&cache).expect("diretório do cache");
    if let Some(d) = &diffs {
        fs::create_dir_all(d).expect("diretório dos diffs");
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
            eprintln!("{name_}: {} referências novas do dart format…", missing.len());
            let batches: Vec<&[(PathBuf, u64)]> = missing.chunks(150).collect();
            let proximo = Mutex::new(0usize);
            std::thread::scope(|s| {
                for _ in 0..8 {
                    s.spawn(|| {
                        loop {
                            let i = {
                                let mut g = proximo.lock().expect("trava");
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
        let proximo = Mutex::new(0usize);
        let failures = Mutex::new(Vec::<(PathBuf, &'static str)>::new());
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    loop {
                        let i = {
                            let mut g = proximo.lock().expect("trava");
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
                        let mut pl = scoreboard.lock().expect("trava");
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
                                            let _ = fs::write(d.join(format!("{base}.dart.nosso")), &output);
                                            let _ = fs::write(d.join(format!("{base}.dart.dart")), &r);
                                            let _ = fs::write(d.join(format!("{base}.dart.fonte")), &source);
                                            let _ = fs::write(d.join(format!("{base}.caminho")), p.to_string_lossy().as_bytes());
                                        }
                                        Some("diferente")
                                    }
                                    Ok(Err(ErroFormatacao::Sintaxe(_))) => {
                                        pl.rejected += 1;
                                        Some("recusado")
                                    }
                                    Ok(Err(ErroFormatacao::SaidaInesperada { .. })) => {
                                        pl.unexpected_output += 1;
                                        Some("saída inesperada")
                                    }
                                    Err(_) => {
                                        pl.panics += 1;
                                        Some("pânico")
                                    }
                                };
                                if let Some(t) = kind_ {
                                    failures.lock().expect("trava").push((p.clone(), t));
                                }
                            }
                        }
                    }
                });
            }
        });
        let pl = scoreboard.into_inner().expect("placar");
        println!(
            "{name_}: {}/{} idênticos ({:.1}%) — diferentes {}, recusados {}, saída inesperada {}, pânicos {}; o dart recusa {} (nós também: {})",
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
            let mut f = failures.into_inner().expect("falhas");
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
            "total: {}/{} idênticos ({:.1}%)",
            general.equal,
            general.total,
            100.0 * general.equal as f64 / general.total.max(1) as f64
        );
    }
}
