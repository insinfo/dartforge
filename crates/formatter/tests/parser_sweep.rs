//! Varredura do parser sobre os `.dart` do repositório e da `lib/` do SDK
//! (ignorada por padrão: depende do SDK instalado e leva alguns segundos).
//!
//! ```text
//! cargo test -p dartforge-formatter --test parser_varredura -- --ignored --nocapture
//! ```
//!
//! Confere, para todo arquivo aceito, a cobertura de tokens
//! ([`verify::conferir`]); exige que toda a `lib/` do SDK 3.6.2 (que o
//! `dart format` aceita por inteiro) seja aceita. A comparação nó a nó com o
//! analyzer fica no exemplo `parser_varredura --despejar` mais o despejo do
//! analyzer (ver o relatório da tarefa).

use std::path::{Path, PathBuf};

use dartforge_formatter::syntax::parser::{analisar, verify};

fn coletar(caminho: &Path, saida: &mut Vec<PathBuf>) {
    if caminho.is_dir() {
        let Ok(entradas) = std::fs::read_dir(caminho) else {
            return;
        };
        for e in entradas.flatten() {
            let p = e.path();
            if p.file_name()
                .is_some_and(|n| n == "target" || n == ".dart_tool" || n == "references")
            {
                continue;
            }
            coletar(&p, saida);
        }
    } else if caminho.extension().is_some_and(|e| e == "dart") {
        saida.push(caminho.to_path_buf());
    }
}

/// (aceitos, recusados, falhas de cobertura com mensagens).
fn varrer(raiz: &Path) -> (usize, Vec<String>, Vec<String>) {
    let mut arquivos = Vec::new();
    coletar(raiz, &mut arquivos);
    let (mut ok, mut recusados, mut falhas) = (0, Vec::new(), Vec::new());
    for p in arquivos {
        let Ok(fonte) = std::fs::read_to_string(&p) else {
            continue;
        };
        match analisar(&fonte) {
            Ok((t, a)) => {
                ok += 1;
                if let Err(e) = verify::conferir(&t, &a, &fonte) {
                    falhas.push(format!("{}: {e}", p.display()));
                }
            }
            Err(e) => recusados.push(format!("{}:{}: {}", p.display(), e.posicao, e.mensagem)),
        }
    }
    (ok, recusados, falhas)
}

#[test]
#[ignore = "varre o repositório e o SDK instalado"]
fn varredura_do_repositorio_e_do_sdk() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, recusados, falhas) = varrer(&repo);
    eprintln!(
        "repositório: {ok} aceitos, {} recusados, {} falhas de cobertura",
        recusados.len(),
        falhas.len()
    );
    assert!(falhas.is_empty(), "{falhas:#?}");

    let home = std::env::var("HOME").unwrap_or_default();
    let sdk = Path::new(&home).join("tools/dart/3.6.2/dart-sdk/lib");
    if sdk.is_dir() {
        let (ok, recusados, falhas) = varrer(&sdk);
        eprintln!(
            "sdk: {ok} aceitos, {} recusados, {} falhas de cobertura",
            recusados.len(),
            falhas.len()
        );
        assert!(recusados.is_empty(), "{recusados:#?}");
        assert!(falhas.is_empty(), "{falhas:#?}");
    }
}
