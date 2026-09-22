//! Lista todos os diagnósticos de sintaxe do corpus com a linha de origem.
//!
//! Ferramenta de priorização: `cargo run -p dartforge-compiler --example
//! sintaxe_corpus -- references/pub/pdf-3.13.1 [...]`. Cada linha de saída traz
//! a mensagem, o arquivo, a linha e o texto da linha, para que a lacuna seja
//! lida no contexto real em vez de pela forma da mensagem.
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
    let raizes: Vec<String> = std::env::args().skip(1).collect();
    let mut arquivos = Vec::new();
    for raiz in &raizes {
        coletar(Path::new(raiz), &mut arquivos);
    }
    arquivos.sort();
    let mut total = 0usize;
    let mut aceitos = 0usize;
    let mut contagem: std::collections::BTreeMap<String, usize> = Default::default();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        total += 1;
        let diagnosticos = dartforge_compiler::compile_unit_all_diagnostics(&fonte);
        if diagnosticos.is_empty() {
            aceitos += 1;
            continue;
        }
        for d in diagnosticos {
            let inicio = d.span.start.min(fonte.len());
            let fim = d.span.end.min(fonte.len()).max(inicio);
            let linha = fonte[..inicio].matches('\n').count() + 1;
            let ini_linha = fonte[..inicio].rfind('\n').map_or(0, |i| i + 1);
            let fim_linha = fonte[inicio..].find('\n').map_or(fonte.len(), |i| inicio + i);
            let texto = fonte[ini_linha..fim_linha].trim();
            let trecho = &fonte[inicio..fim];
            let chave = d.message.split(':').next().unwrap_or("").to_string();
            *contagem.entry(chave).or_default() += 1;
            println!(
                "{}\n    {}:{}  «{}»\n    {}",
                d.message,
                arquivo.display(),
                linha,
                trecho.chars().take(40).collect::<String>(),
                texto.chars().take(140).collect::<String>()
            );
        }
    }
    println!("\n== {aceitos}/{total} arquivos sem diagnóstico de sintaxe ==");
    let mut ordenado: Vec<_> = contagem.into_iter().collect();
    ordenado.sort_by(|a, b| b.1.cmp(&a.1));
    for (mensagem, n) in ordenado {
        println!("{n:5}  {mensagem}");
    }
}
