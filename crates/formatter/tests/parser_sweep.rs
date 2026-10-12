//! Varredura do parser sobre os `.dart` do repositório e da `lib/` do SDK
//! (ignorada por padrão: depende do SDK instalado e leva alguns segundos).
//!
//! ```text
//! cargo test -p dartforge-formatter --test parser_varredura -- --ignored --nocapture
//! ```
//!
//! Confere, para todo arquivo aceito, a cobertura de tokens
//! ([`verify::check`]); exige que toda a `lib/` do SDK 3.6.2 (que o
//! `dart format` aceita por inteiro) seja aceita. A comparação nó a nó com o
//! analyzer fica no exemplo `parser_varredura --dump` mais o despejo do
//! analyzer (ver o relatório da tarefa).

use std::path::{Path, PathBuf};

use dartforge_formatter::syntax::parser::{parse, verify};

fn collect(path: &Path, output: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(inputs) = std::fs::read_dir(path) else {
            return;
        };
        for e in inputs.flatten() {
            let p = e.path();
            if p.file_name()
                .is_some_and(|n| n == "target" || n == ".dart_tool" || n == "references")
            {
                continue;
            }
            collect(&p, output);
        }
    } else if path.extension().is_some_and(|e| e == "dart") {
        output.push(path.to_path_buf());
    }
}

/// (aceitos, recusados, falhas de cobertura com mensagens).
fn sweep(root: &Path) -> (usize, Vec<String>, Vec<String>) {
    let mut files = Vec::new();
    collect(root, &mut files);
    let (mut ok, mut rejected, mut failures) = (0, Vec::new(), Vec::new());
    for p in files {
        let Ok(source) = std::fs::read_to_string(&p) else {
            continue;
        };
        match parse(&source) {
            Ok((t, a)) => {
                ok += 1;
                if let Err(e) = verify::check(&t, &a, &source) {
                    failures.push(format!("{}: {e}", p.display()));
                }
            }
            Err(e) => rejected.push(format!("{}:{}: {}", p.display(), e.posicao, e.mensagem)),
        }
    }
    (ok, rejected, failures)
}

#[test]
#[ignore = "varre o repositório e o SDK instalado"]
fn sweep_of_repository_and_sdk() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, rejected, failures) = sweep(&repo);
    eprintln!(
        "repositório: {ok} aceitos, {} recusados, {} falhas de cobertura",
        rejected.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");

    let home = std::env::var("HOME").unwrap_or_default();
    let sdk = Path::new(&home).join("tools/dart/3.6.2/dart-sdk/lib");
    if sdk.is_dir() {
        let (ok, rejected, failures) = sweep(&sdk);
        eprintln!(
            "sdk: {ok} aceitos, {} recusados, {} falhas de cobertura",
            rejected.len(),
            failures.len()
        );
        assert!(rejected.is_empty(), "{rejected:#?}");
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
