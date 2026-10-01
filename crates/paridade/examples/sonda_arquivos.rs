//! Compara, arquivo a arquivo, o nosso lado com o oráculo: cada diagnóstico
//! dos dois lados em ordem de offset, marcado `=` (acerto), `~` (acerto com
//! mensagem diferente), `-` (só do oráculo) ou `+` (só nosso).
//!
//! ```text
//! cargo run --release -p dartforge-paridade --example sonda_arquivos -- <grupo> <arquivo relativo>...
//! cargo run --release -p dartforge-paridade --example sonda_arquivos -- --livre <dir> [<arquivo relativo>...]
//! ```
//!
//! Com um grupo do corpus, o oráculo é o gravado. Com `--livre`, `<dir>` vira
//! um pacote de linguagem 3.6 (pubspec e package_config escritos na hora) e o
//! oráculo é o `dart analyze` 3.6.2 rodado agora sobre os arquivos (todos os
//! `.dart` do diretório, se nenhum for dado): serve para conferir um caso
//! mínimo antes de portar uma regra. Os arquivos são analisados juntos, num
//! lote só (como no placar, cada um é a sua biblioteca), sem os filtros de
//! publicação.

use dartforge_paridade::analise::Motor;
use dartforge_paridade::corpus::{self, Grupo};
use dartforge_paridade::oraculo::{self, Registro, SdkOraculo};
use dartforge_paridade::{Execucao, filtros, rodar_nosso};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((primeiro, resto)) = args.split_first() else {
        eprintln!("uso: sonda_arquivos <grupo> <arquivo>... | --livre <dir> [<arquivo>...]");
        std::process::exit(2);
    };
    let (dir, cfg, oraculo, rels) = if primeiro == "--livre" {
        let Some((d, rels)) = resto.split_first() else {
            eprintln!("--livre pede o diretório");
            std::process::exit(2);
        };
        livre(Path::new(d), rels)
    } else {
        do_corpus(primeiro, resto)
    };
    let motor = Motor::descobrir().expect("SDK");
    let fontes: Vec<PathBuf> = rels.iter().map(|r| dir.join(r)).collect();
    let ex = Execucao {
        trabalhadores: 1,
        tamanho_lote: fontes.len().max(1),
        progresso: false,
        isolar: None,
        tempo_max: std::time::Duration::from_secs(600),
    };
    let r = rodar_nosso(&motor, &dir, &fontes, Some(&cfg), &filtros::Opcoes::ler(&dir), &ex);
    let mut soma = [0usize; 4];
    for rel in &rels {
        let rel = rel.replace('\\', "/");
        println!("== {rel}");
        // (offset, comprimento, código, lado) → registro; lado 0 oráculo, 1 nosso.
        let mut linhas: BTreeMap<(usize, usize, String, u8), &Registro> = BTreeMap::new();
        for o in oraculo.iter().filter(|o| o.arquivo == rel) {
            linhas.insert((o.offset, o.length, o.code.clone(), 0), o);
        }
        for n in r.registros.iter().filter(|n| n.arquivo == rel) {
            linhas.insert((n.offset, n.length, n.code.clone(), 1), n);
        }
        let mut vistos = BTreeSet::new();
        for ((off, len, code, lado), reg) in &linhas {
            if !vistos.insert((*off, *len, code.clone())) {
                continue;
            }
            let par = if *lado == 0 { linhas.get(&(*off, *len, code.clone(), 1)) } else { None };
            let (marca, i) = match (lado, par) {
                (0, Some(n)) if n.problem_message == reg.problem_message => ("=", 0),
                (0, Some(_)) => ("~", 1),
                (0, None) => ("-", 2),
                _ => ("+", 3),
            };
            soma[i] += 1;
            println!("{marca} {}:{} (+{len}) {code}: {}", reg.line, reg.column, reg.problem_message);
            if let (0, Some(n)) = (lado, par) {
                if n.problem_message != reg.problem_message {
                    println!("    nosso: {}", n.problem_message);
                }
            }
        }
    }
    println!("total: = {} | ~ {} | - {} | + {}", soma[0], soma[1], soma[2], soma[3]);
    if !r.panicos.is_empty() {
        println!("pânicos: {:?}", r.panicos);
    }
}

type Preparo = (PathBuf, PathBuf, Vec<Registro>, Vec<String>);

/// Um grupo do corpus, com o oráculo gravado.
fn do_corpus(grupo: &str, rels: &[String]) -> Preparo {
    let raiz = corpus::raiz_corpus();
    let raiz = std::path::absolute(&raiz).unwrap_or(raiz);
    let Some((_, g)) = corpus::grupos(&raiz).into_iter().find(|(n, _)| n == grupo) else {
        eprintln!("grupo desconhecido: {grupo}");
        std::process::exit(2);
    };
    let dir = raiz.join(grupo);
    let cfg = corpus::preparar(&dir, grupo, &g).expect("preparar o grupo");
    let (oraculo, _) = oraculo::ler(&dir).expect("oráculo gravado");
    (dir, cfg, oraculo, rels.to_vec())
}

/// Um diretório qualquer, com o oráculo 3.6.2 rodado agora.
fn livre(d: &Path, rels: &[String]) -> Preparo {
    let dir = std::path::absolute(d).expect("diretório");
    let nome = "sonda_livre";
    std::fs::write(dir.join("pubspec.yaml"), format!("name: {nome}\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n"))
        .expect("pubspec");
    let g = Grupo { sdk: SdkOraculo::V362, pacotes: Vec::new(), origem: String::new() };
    let cfg = corpus::preparar(&dir, nome, &g).expect("package_config");
    let rels: Vec<String> = if rels.is_empty() {
        corpus::arquivos_dart(&dir).iter().filter_map(|a| oraculo::relativo(a, &dir)).collect()
    } else {
        rels.to_vec()
    };
    let alvos: Vec<PathBuf> = rels.iter().map(|r| dir.join(r)).collect();
    let cache = std::env::temp_dir().join("dartforge-sonda-cache");
    let diags = oraculo::rodar(SdkOraculo::V362, &dir, &alvos, &cache).expect("dart analyze 3.6.2");
    let regs = diags.iter().filter_map(|j| Registro::de_json(j, &dir)).collect();
    (dir, cfg, regs, rels)
}
