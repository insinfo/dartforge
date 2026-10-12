//! Mede o parser novo contra diretórios de fontes Dart.
//!
//! `cargo run -q -p dartforge-frontend --example sintaxe -- C:/tools/dartsdk-3.6.2/lib references/pub`
//!
//! Imprime cada diagnóstico com arquivo, linha e a linha de código, e no fim o
//! total de arquivos aceitos e a contagem por mensagem. Com `--so-resumo` só
//! imprime o resumo.
use std::path::{Path, PathBuf};

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

fn main() {
    let mut raizes = Vec::new();
    let mut so_resumo = false;
    for arg in std::env::args().skip(1) {
        if arg == "--so-resumo" {
            so_resumo = true;
        } else {
            raizes.push(arg);
        }
    }
    let mut arquivos = Vec::new();
    for raiz in &raizes {
        coletar(Path::new(raiz), &mut arquivos);
    }
    arquivos.sort();
    let mut total = 0usize;
    let mut aceitos = 0usize;
    let mut contagem: std::collections::BTreeMap<String, usize> = Default::default();
    let mut nomes = dartforge_intern::Interner::new();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        total += 1;
        let saida = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        if saida.diagnostics.is_empty() {
            aceitos += 1;
            continue;
        }
        for d in saida.diagnostics {
            let inicio = d.span.start.min(fonte.len());
            let linha = fonte[..inicio].matches('\n').count() + 1;
            let ini_linha = fonte[..inicio].rfind('\n').map_or(0, |i| i + 1);
            let fim_linha = fonte[inicio..]
                .find('\n')
                .map_or(fonte.len(), |i| inicio + i);
            let texto = fonte[ini_linha..fim_linha].trim();
            let chave = d
                .message
                .split(", encontrou")
                .next()
                .unwrap_or("")
                .to_string();
            *contagem.entry(chave).or_default() += 1;
            if !so_resumo {
                println!(
                    "{}\n    {}:{}\n    {}",
                    d.message,
                    arquivo.display(),
                    linha,
                    texto.chars().take(160).collect::<String>()
                );
            }
        }
    }
    println!("\n== {aceitos}/{total} arquivos aceitos pelo parser ==");
    let mut ordenado: Vec<_> = contagem.into_iter().collect();
    ordenado.sort_by_key(|a| std::cmp::Reverse(a.1));
    for (mensagem, n) in ordenado.iter().take(40) {
        println!("{n:5}  {mensagem}");
    }
}
