//! Mede memória e tempo do front-end novo sobre um projeto inteiro, com todas
//! as árvores retidas — o que um servidor de linguagem faz com o projeto aberto.
//!
//! `cargo run -q --release -p dartforge-frontend --example memoria -- C:/MyDartProjects/new_sali`
//!
//! Imprime bytes de fonte, bytes vivos com tudo retido, pico, razão vivo/fonte,
//! alocações e tempo. A referência que este número enfrenta está no PLANO.md:
//! o LSP do Dart chega a ~6 GB e o `webdev` a ~10 GB neste mesmo projeto.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use std::path::{Path, PathBuf};
use std::time::Instant;

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            if caminho
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

fn main() {
    let mut arquivos = Vec::new();
    for raiz in std::env::args().skip(1) {
        coletar(Path::new(&raiz), &mut arquivos);
    }
    arquivos.sort();
    let inicio = Instant::now();
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let alocacoes_base = dartforge_instrument::allocation_count();
    let mut nomes = dartforge_intern::Interner::new();
    let mut retidas = Vec::with_capacity(arquivos.len());
    let mut bytes_fonte = 0usize;
    let mut recusados = 0usize;
    let mut tokens_total = 0usize;
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        bytes_fonte += fonte.len();
        let saida = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        if !saida.diagnostics.is_empty() {
            recusados += 1;
        }
        tokens_total += saida.ast.exprs.len();
        // Fonte e árvore ficam vivas, como num editor com o projeto aberto.
        retidas.push((fonte, saida));
    }
    let tempo = inicio.elapsed();
    let vivos = dartforge_instrument::live_bytes() - base;
    let pico = dartforge_instrument::peak_bytes() - base;
    let alocacoes = dartforge_instrument::allocation_count() - alocacoes_base;
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    println!(
        "arquivos: {} ({} recusados pelo parser)",
        retidas.len(),
        recusados
    );
    println!("fonte: {:.2} MiB", mib(bytes_fonte));
    println!(
        "vivo com tudo retido: {:.2} MiB ({:.2}x a fonte)",
        mib(vivos),
        vivos as f64 / bytes_fonte as f64
    );
    println!("pico: {:.2} MiB", mib(pico));
    println!(
        "nós de expressão: {tokens_total}; símbolos internados: {} ({:.2} MiB)",
        nomes.len(),
        mib(nomes.payload_bytes())
    );
    println!(
        "alocações: {alocacoes}; tempo: {:.0} ms ({:.1} MiB/s)",
        tempo.as_secs_f64() * 1e3,
        mib(bytes_fonte) / tempo.as_secs_f64()
    );
    drop(retidas);
    println!(
        "após descartar tudo: {} bytes acima da base",
        dartforge_instrument::live_bytes().saturating_sub(base)
    );
}
