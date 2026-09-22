//! `dartforge-diferencial [opções]` — relatório do corpus.
//! `dartforge-diferencial contrato [-o docs/CONTRATO-DDC.md]` — gera o documento do contrato.

use std::path::PathBuf;
use std::time::Duration;

use dartforge_diferencial::{Ambiente, Opcoes, contrato, executar_corpus, listar, relatorio};

const USO: &str = "uso:
  dartforge-diferencial [--nativo] [--corpus DIR] [--filtro TEXTO] [--sem-forge] [--sem-cache] [--jobs N] [--limite SEG] [--silencioso]
      roda dart run × [ddc+node ou nativo] × dartforge em cada programa e imprime o relatório
      (código 0 se todos batem; 1 se algum falha)
  dartforge-diferencial contrato [--corpus DIR] [-o ARQUIVO]
      compila cada programa com o dartdevc e escreve docs/CONTRATO-DDC.md
  dartforge-diferencial verificar [--corpus DIR] [--filtro TEXTO]
      só os oráculos: cada programa tem de rodar no dart run e bater com ddc+node";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut amb = Ambiente::detectar();
    let mut corpus = amb.raiz.join("corpus/js");
    let mut filtro: Option<String> = None;
    let mut op = Opcoes::default();
    let mut saida_doc = amb.raiz.join("docs/CONTRATO-DDC.md");
    let mut silencioso = false;
    let mut modo = "relatorio";
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "contrato" | "verificar" => modo = if args[i] == "contrato" { "contrato" } else { "verificar" },
            "--corpus" => {
                i += 1;
                corpus = PathBuf::from(&args[i]);
            }
            "--filtro" => {
                i += 1;
                filtro = Some(args[i].clone());
            }
            "-o" => {
                i += 1;
                saida_doc = PathBuf::from(&args[i]);
            }
            "--jobs" => {
                i += 1;
                op.threads = args[i].parse().expect("--jobs N");
            }
            "--limite" => {
                i += 1;
                amb.limite = Duration::from_secs(args[i].parse().expect("--limite SEG"));
            }
            "--sem-forge" => op.com_forge = false,
            "--nativo" => op.nativo = true,
            "--sem-cache" => amb.usar_cache = false,
            "--silencioso" => silencioso = true,
            "-h" | "--help" => {
                println!("{USO}");
                return;
            }
            outro => {
                eprintln!("argumento desconhecido: {outro}\n{USO}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let programas = listar(&corpus, filtro.as_deref());
    if programas.is_empty() {
        eprintln!("nenhum programa em {}", corpus.display());
        std::process::exit(2);
    }
    match modo {
        "contrato" => {
            let doc = contrato::gerar(&amb, &programas);
            std::fs::write(&saida_doc, doc).expect("escrever o documento");
            println!("{} programas → {}", programas.len(), saida_doc.display());
        }
        _ => {
            if modo == "verificar" {
                op.com_forge = false;
            }
            if op.com_forge {
                match &amb.dartforge_bin {
                    Some(b) => eprintln!("dartforge: {}", b.display()),
                    None => eprintln!("dartforge: binário não encontrado; usando `cargo run -p dartforge-cli`"),
                }
            }
            let inicio = std::time::Instant::now();
            let resultados = executar_corpus(&amb, &programas, op, |r| {
                if silencioso {
                    return;
                }
                let estado = if r.forge.is_some() {
                    if r.ok() { "ok" } else { "FALHA" }
                } else if r.dart.codigo != 0 {
                    "DART!"
                } else if r.nativo {
                    "DART"
                } else if r.ddc_vs_dart().is_none() == r.programa.diverge_ddc.is_none() {
                    "ok"
                } else {
                    "DDC≠VM"
                };
                eprintln!("  {estado:<6} {}", r.programa.nome);
            });
            print!("{}", relatorio(&resultados));
            println!("({} programas em {:.1} s)", resultados.len(), inicio.elapsed().as_secs_f64());
            let todos_ok = if op.com_forge {
                resultados.iter().all(|r| r.ok())
            } else if op.nativo {
                resultados.iter().all(|r| r.dart.codigo == 0)
            } else {
                resultados.iter().all(|r| r.dart.codigo == 0 && (r.ddc_vs_dart().is_none() != r.programa.diverge_ddc.is_some()))
            };
            if !todos_ok {
                std::process::exit(1);
            }
        }
    }
}
