//! Oráculo do formatador: compara, byte a byte, a saída de
//! `dartforge_formatar::formatar` com a do `dart format` do SDK 3.6.2 sobre
//! um corpus de arquivos `.dart`.
//!
//! ```text
//! cargo run --release -p dartforge-formatar --example oraculo -- \
//!     [--dart <dart>] [--cache <dir>] [--diffs <dir>] [--lista] [<dir|arquivo>...]
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

use dartforge_formatar::{ErroFormatacao, Opcoes, formatar};

fn fnv(dados: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in dados {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn coletar(raiz: &Path, saida: &mut Vec<PathBuf>) {
    if raiz.is_file() {
        if raiz.extension().is_some_and(|e| e == "dart") {
            saida.push(raiz.to_path_buf());
        }
        return;
    }
    let Ok(entradas) = fs::read_dir(raiz) else {
        return;
    };
    let mut filhos: Vec<PathBuf> = entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
    filhos.sort();
    for f in filhos {
        let nome = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if nome.starts_with('.') || nome == "target" || nome == "references" || nome == "node_modules" {
            continue;
        }
        if f.is_symlink() {
            continue;
        }
        coletar(&f, saida);
    }
}

/// Roda o `dart format -o json` num lote e grava o cache de cada arquivo.
fn referencias(dart: &str, cache: &Path, lote: &[(PathBuf, u64)]) {
    let mut cmd = Command::new(dart);
    cmd.args(["format", "-o", "json", "--summary", "none"]);
    for (p, _) in lote {
        cmd.arg(p);
    }
    let saida = match cmd.output() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("falha ao rodar {dart}: {e}");
            return;
        }
    };
    let texto = String::from_utf8_lossy(&saida.stdout);
    let mut por_caminho: BTreeMap<String, String> = BTreeMap::new();
    for linha in texto.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(linha) else {
            continue;
        };
        if let (Some(p), Some(s)) = (v["path"].as_str(), v["source"].as_str()) {
            por_caminho.insert(p.to_string(), s.to_string());
        }
    }
    for (p, h) in lote {
        let chave = p.to_string_lossy().to_string();
        match por_caminho.get(&chave) {
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
struct Placar {
    total: usize,
    iguais: usize,
    diferentes: usize,
    recusados: usize,
    panicos: usize,
    saida_inesperada: usize,
    ambos_recusam: usize,
    so_dart_recusa: usize,
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut args = std::env::args().skip(1);
    let raiz_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("raiz do repositório");
    let casa = std::env::var("HOME").unwrap_or_default();
    let mut dart = format!("{casa}/tools/dart/3.6.2/dart-sdk/bin/dart");
    let mut cache = raiz_repo.join("target/oraculo-formatar");
    let mut diffs: Option<PathBuf> = None;
    let mut listar = false;
    let mut caminhos: Vec<PathBuf> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dart" => dart = args.next().expect("--dart <exe>"),
            "--cache" => cache = PathBuf::from(args.next().expect("--cache <dir>")),
            "--diffs" => diffs = Some(PathBuf::from(args.next().expect("--diffs <dir>"))),
            "--lista" => listar = true,
            _ => caminhos.push(PathBuf::from(a)),
        }
    }
    let conjuntos: Vec<(String, Vec<PathBuf>)> = if caminhos.is_empty() {
        let mut v = vec![
            ("repositório".into(), vec![raiz_repo.clone()]),
            ("sdk/lib".into(), vec![PathBuf::from(format!("{casa}/tools/dart/3.6.2/dart-sdk/lib"))]),
        ];
        let pub_cache = PathBuf::from(format!("{casa}/tools/dart/pub-cache/hosted/pub.dev"));
        if pub_cache.is_dir() {
            v.push(("pub-cache".into(), vec![pub_cache]));
        }
        v
    } else {
        vec![("selecionados".into(), caminhos)]
    };
    fs::create_dir_all(&cache).expect("diretório do cache");
    if let Some(d) = &diffs {
        fs::create_dir_all(d).expect("diretório dos diffs");
    }

    let mut geral = Placar::default();
    for (nome, raizes) in &conjuntos {
        let mut arquivos = Vec::new();
        for r in raizes {
            coletar(r, &mut arquivos);
        }
        let com_hash: Vec<(PathBuf, u64, Vec<u8>)> = arquivos
            .into_iter()
            .filter_map(|p| fs::read(&p).ok().map(|d| (p, fnv(&d), d)))
            .collect();
        let faltando: Vec<(PathBuf, u64)> = com_hash
            .iter()
            .filter(|(_, h, _)| !cache.join(format!("{h:016x}.out")).exists() && !cache.join(format!("{h:016x}.err")).exists())
            .map(|(p, h, _)| (p.clone(), *h))
            .collect();
        if !faltando.is_empty() {
            eprintln!("{nome}: {} referências novas do dart format…", faltando.len());
            let lotes: Vec<&[(PathBuf, u64)]> = faltando.chunks(150).collect();
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
                            let Some(lote) = lotes.get(i) else {
                                break;
                            };
                            referencias(&dart, &cache, lote);
                        }
                    });
                }
            });
        }

        let placar = Mutex::new(Placar::default());
        let proximo = Mutex::new(0usize);
        let falhas = Mutex::new(Vec::<(PathBuf, &'static str)>::new());
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
                        let Some((p, h, dados)) = com_hash.get(i) else {
                            break;
                        };
                        let referencia = fs::read_to_string(cache.join(format!("{h:016x}.out"))).ok();
                        let fonte = String::from_utf8_lossy(dados).to_string();
                        let nosso = std::panic::catch_unwind(|| formatar(&fonte, &Opcoes::default()));
                        let mut pl = placar.lock().expect("trava");
                        match (referencia, nosso) {
                            (None, Ok(Err(_))) => pl.ambos_recusam += 1,
                            (None, _) => pl.so_dart_recusa += 1,
                            (Some(r), resultado) => {
                                pl.total += 1;
                                let tipo = match resultado {
                                    Ok(Ok(saida)) if saida == r => {
                                        pl.iguais += 1;
                                        None
                                    }
                                    Ok(Ok(saida)) => {
                                        pl.diferentes += 1;
                                        if let Some(d) = &diffs {
                                            let base = format!("{h:016x}");
                                            let _ = fs::write(d.join(format!("{base}.dart.nosso")), &saida);
                                            let _ = fs::write(d.join(format!("{base}.dart.dart")), &r);
                                            let _ = fs::write(d.join(format!("{base}.dart.fonte")), &fonte);
                                            let _ = fs::write(d.join(format!("{base}.caminho")), p.to_string_lossy().as_bytes());
                                        }
                                        Some("diferente")
                                    }
                                    Ok(Err(ErroFormatacao::Sintaxe(_))) => {
                                        pl.recusados += 1;
                                        Some("recusado")
                                    }
                                    Ok(Err(ErroFormatacao::SaidaInesperada { .. })) => {
                                        pl.saida_inesperada += 1;
                                        Some("saída inesperada")
                                    }
                                    Err(_) => {
                                        pl.panicos += 1;
                                        Some("pânico")
                                    }
                                };
                                if let Some(t) = tipo {
                                    falhas.lock().expect("trava").push((p.clone(), t));
                                }
                            }
                        }
                    }
                });
            }
        });
        let pl = placar.into_inner().expect("placar");
        println!(
            "{nome}: {}/{} idênticos ({:.1}%) — diferentes {}, recusados {}, saída inesperada {}, pânicos {}; o dart recusa {} (nós também: {})",
            pl.iguais,
            pl.total,
            100.0 * pl.iguais as f64 / pl.total.max(1) as f64,
            pl.diferentes,
            pl.recusados,
            pl.saida_inesperada,
            pl.panicos,
            pl.so_dart_recusa + pl.ambos_recusam,
            pl.ambos_recusam
        );
        if listar {
            let mut f = falhas.into_inner().expect("falhas");
            f.sort();
            for (p, t) in f {
                println!("  {t}: {}", p.display());
            }
        }
        geral.total += pl.total;
        geral.iguais += pl.iguais;
    }
    if conjuntos.len() > 1 {
        println!(
            "total: {}/{} idênticos ({:.1}%)",
            geral.iguais,
            geral.total,
            100.0 * geral.iguais as f64 / geral.total.max(1) as f64
        );
    }
}
