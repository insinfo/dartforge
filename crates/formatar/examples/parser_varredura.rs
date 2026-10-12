//! Varredura do parser do formatador sobre arquivos `.dart`.
//!
//! ```text
//! cargo run --release -p dartforge-formatar --example parser_varredura -- \
//!     [--despejar] [--lista] [<dir|arquivo>...]
//! ```
//!
//! Para cada arquivo, roda [`analisar`] e, se a análise teve sucesso, a
//! conferência de cobertura de tokens ([`verificar::conferir`]). Imprime a
//! contagem de arquivos aceitos, recusados e com falha de cobertura, e o
//! tempo total. Com `--lista`, os caminhos vêm da entrada padrão, um por
//! linha. Com `--despejar`, escreve na saída padrão, para cada arquivo,
//! `### caminho` seguido de `ERR <mensagem>` ou do despejo da árvore
//! ([`verificar::despejar`]), no formato do oráculo do analyzer.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use dartforge_formatar::sintaxe::parser::{analisar, verificar};

fn coletar(caminho: &Path, saida: &mut Vec<PathBuf>) {
    if caminho.is_dir() {
        let Ok(entradas) = std::fs::read_dir(caminho) else {
            return;
        };
        let mut v: Vec<_> = entradas.flatten().map(|e| e.path()).collect();
        v.sort();
        for p in v {
            if p.file_name()
                .is_some_and(|n| n == "target" || n == ".dart_tool" || n == ".git")
            {
                continue;
            }
            coletar(&p, saida);
        }
    } else if caminho.extension().is_some_and(|e| e == "dart") {
        saida.push(caminho.to_path_buf());
    }
}

fn main() {
    let mut despejar = false;
    let mut lista = false;
    let mut caminhos = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--despejar" => despejar = true,
            "--lista" => lista = true,
            _ => coletar(Path::new(&a), &mut caminhos),
        }
    }
    if lista {
        for l in std::io::stdin().lock().lines().map_while(Result::ok) {
            if !l.is_empty() {
                caminhos.push(PathBuf::from(l));
            }
        }
    }
    let saida = std::io::stdout();
    let mut saida = saida.lock();
    let (mut ok, mut recusados, mut falhas) = (0usize, 0usize, 0usize);
    let inicio = Instant::now();
    for p in &caminhos {
        let Ok(fonte) = std::fs::read_to_string(p) else {
            continue;
        };
        let r = analisar(&fonte);
        if despejar {
            let _ = writeln!(saida, "### {}", p.display());
        }
        match r {
            Ok((tokens, arvore)) => {
                ok += 1;
                if let Err(e) = verificar::conferir(&tokens, &arvore, &fonte) {
                    falhas += 1;
                    eprintln!("COBERTURA {}: {e}", p.display());
                }
                if despejar {
                    let _ = write!(saida, "{}", verificar::despejar(&tokens, &arvore));
                }
            }
            Err(e) => {
                recusados += 1;
                if despejar {
                    let _ = writeln!(saida, "ERR {} @{}", e.mensagem, e.posicao);
                } else {
                    eprintln!("RECUSADO {}:{}: {}", p.display(), e.posicao, e.mensagem);
                }
            }
        }
    }
    eprintln!(
        "{} arquivos: {ok} aceitos, {recusados} recusados, {falhas} falhas de cobertura, {:.2?}",
        caminhos.len(),
        inicio.elapsed()
    );
}
