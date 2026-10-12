//! O parser novo aceita 100% do `lib/` do SDK 3.6.2 e do corpus de pub.dev.
//!
//! Este é o critério de aceite do passo 1 da meta governante (PLANO.md). O
//! teste é ignorado por padrão porque depende de arquivos fora do repositório:
//! `DARTFORGE_SDK_LIB` (padrão `C:/tools/dartsdk-3.6.2/lib`) e
//! `references/pub` na raiz do workspace.
use std::path::{Path, PathBuf};

fn collect(dir: &Path, output: &mut Vec<PathBuf>) {
    let Ok(inputs) = std::fs::read_dir(dir) else {
        return;
    };
    for input in inputs.flatten() {
        let path = input.path();
        if path.is_dir() {
            collect(&path, output);
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
}

/// A versão de linguagem do pacote dono de `file`: o `x.y` do limite
/// inferior de `sdk:` no `pubspec.yaml` mais próximo (`^3.8.0`,
/// `'>=3.8.0 <4.0.0'`), como o `languageVersion` que o `pub get` grava.
fn package_version(file: &Path) -> Option<dartforge_frontend::LanguageVersion> {
    let pubspec = file
        .ancestors()
        .skip(1)
        .map(|d| d.join("pubspec.yaml"))
        .find(|p| p.is_file())?;
    let texto = std::fs::read_to_string(pubspec).ok()?;
    let line = texto.lines().find(|l| l.trim_start().starts_with("sdk:"))?;
    let value = line
        .trim_start()
        .trim_start_matches("sdk:")
        .trim()
        .trim_matches(['\'', '"']);
    let inferior = value
        .trim_start_matches('^')
        .trim_start_matches(">=")
        .trim();
    let mut partes = inferior.split(['.', ' ']);
    let greater: u16 = partes.next()?.parse().ok()?;
    let less: u16 = partes.next()?.parse().ok()?;
    Some(dartforge_frontend::LanguageVersion::new(greater, less))
}

/// `by_package`: a versão de cada arquivo sem marcador vem do pubspec do
/// pacote. No SDK não: as bibliotecas `dart:` são lidas na versão corrente
/// do SDK, mesmo com o `pubspec.yaml` de desenvolvimento que algumas pastas
/// trazem (`_internal/js_runtime`: `sdk: '>=2.2.2'`).
fn measure(root: &Path, by_package: bool) -> (usize, usize, Vec<String>) {
    let mut files = Vec::new();
    collect(root, &mut files);
    files.sort();
    let mut names = dartforge_intern::Interner::new();
    let mut total = 0;
    let mut accepted = 0;
    let mut failures = Vec::new();
    for file in &files {
        let Ok(fonte) = std::fs::read_to_string(file) else {
            continue;
        };
        total += 1;
        // A versão de cada arquivo (docs/VERSOES-LINGUAGEM.md §2): o
        // marcador; senão a do pacote, que o `pub` tira do limite inferior
        // de `environment: sdk:` no pubspec; senão o piso 3.6 (o SDK).
        let versao = dartforge_frontend::features::marcador_versao(&fonte)
            .map(|m| m.0)
            .or_else(|| if by_package { package_version(file) } else { None })
            .unwrap_or(dartforge_frontend::LanguageVersion::PISO);
        let features = dartforge_frontend::LibraryFeatures::new(versao, &[]);
        let output = dartforge_frontend::parser::parse_com(&fonte, &mut names, features);
        if output.diagnostics.is_empty() {
            accepted += 1;
        } else {
            let d = &output.diagnostics[0];
            let line = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            failures.push(format!("{}:{}: {}", file.display(), line, d.message));
        }
    }
    (accepted, total, failures)
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn whole_sdk_lib_is_accepted() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let (accepted, total, failures) = measure(Path::new(&sdk), false);
    assert!(total > 0, "SDK não encontrado em {sdk}");
    assert_eq!(
        accepted,
        total,
        "arquivos do SDK recusados:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn whole_pub_corpus_is_accepted() {
    // `DARTFORGE_PUB_CORPUS` aponta outro clone (numa worktree, o do checkout
    // principal: `references/` fica fora do git).
    let root = std::env::var("DARTFORGE_PUB_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub"));
    let (accepted, total, failures) = measure(&root, true);
    assert!(total > 0, "corpus não encontrado em {}", root.display());
    assert_eq!(
        accepted,
        total,
        "arquivos do corpus recusados:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2"]
fn whole_sdk_lib_lexes() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let mut files = Vec::new();
    collect(Path::new(&sdk), &mut files);
    let mut failures = Vec::new();
    for file in &files {
        let Ok(fonte) = std::fs::read_to_string(file) else {
            continue;
        };
        if let Err(d) = dartforge_frontend::lexer::lex(&fonte) {
            let line = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            failures.push(format!("{}:{}: {}", file.display(), line, d.message));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
