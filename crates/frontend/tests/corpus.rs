//! O parser novo aceita 100% do `lib/` do SDK 3.6.2 e do corpus de pub.dev.
//!
//! Este é o critério de aceite do passo 1 da meta governante (PLANO.md). O
//! teste é ignorado por padrão porque depende de arquivos fora do repositório:
//! `DARTFORGE_SDK_LIB` (padrão `C:/tools/dartsdk-3.6.2/lib`) e
//! `references/pub` na raiz do workspace.
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

fn medir(raiz: &Path) -> (usize, usize, Vec<String>) {
    let mut arquivos = Vec::new();
    coletar(raiz, &mut arquivos);
    arquivos.sort();
    let mut nomes = dartforge_intern::Interner::new();
    let mut total = 0;
    let mut aceitos = 0;
    let mut falhas = Vec::new();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        total += 1;
        let saida = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        if saida.diagnostics.is_empty() {
            aceitos += 1;
        } else {
            let d = &saida.diagnostics[0];
            let linha = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            falhas.push(format!("{}:{}: {}", arquivo.display(), linha, d.message));
        }
    }
    (aceitos, total, falhas)
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn sdk_lib_inteiro_e_aceito() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let (aceitos, total, falhas) = medir(Path::new(&sdk));
    assert!(total > 0, "SDK não encontrado em {sdk}");
    assert_eq!(
        aceitos,
        total,
        "arquivos do SDK recusados:\n{}",
        falhas.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn corpus_pub_inteiro_e_aceito() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub");
    let (aceitos, total, falhas) = medir(&raiz);
    assert!(total > 0, "corpus não encontrado em {}", raiz.display());
    assert_eq!(
        aceitos,
        total,
        "arquivos do corpus recusados:\n{}",
        falhas.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2"]
fn sdk_lib_inteiro_lexa() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let mut arquivos = Vec::new();
    coletar(Path::new(&sdk), &mut arquivos);
    let mut falhas = Vec::new();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        if let Err(d) = dartforge_frontend::lexer::lex(&fonte) {
            let linha = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            falhas.push(format!("{}:{}: {}", arquivo.display(), linha, d.message));
        }
    }
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
}
