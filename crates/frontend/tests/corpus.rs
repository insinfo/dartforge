//! The new parser accepts 100% of the SDK 3.6.2 `lib/` and of the pub.dev
//! corpus.
//!
//! This is the acceptance criterion of step 1 of the governing goal
//! (PLANO.md). The test is ignored by default because it depends on files
//! outside the repository: `DARTFORGE_SDK_LIB` (default
//! `C:/tools/dartsdk-3.6.2/lib`) and `references/pub` at the workspace root.
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

/// The language version of the package that owns `file`: the `x.y` of the
/// lower bound of `sdk:` in the nearest `pubspec.yaml` (`^3.8.0`,
/// `'>=3.8.0 <4.0.0'`), like the `languageVersion` that `pub get` writes.
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

/// `by_package`: the version of each file without a marker comes from the
/// package's pubspec. Not in the SDK: the `dart:` libraries are read at the
/// SDK's current version, even with the development `pubspec.yaml` some
/// folders carry (`_internal/js_runtime`: `sdk: '>=2.2.2'`).
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
        // The version of each file (docs/VERSOES-LINGUAGEM.md §2): the marker;
        // otherwise the package's, which `pub` takes from the lower bound of
        // `environment: sdk:` in the pubspec; otherwise the 3.6 floor (the SDK).
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
#[ignore = "depends on SDK 3.6.2 and on the corpus in references/pub"]
fn whole_sdk_lib_is_accepted() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let (accepted, total, failures) = measure(Path::new(&sdk), false);
    assert!(total > 0, "SDK not found at {sdk}");
    assert_eq!(
        accepted,
        total,
        "SDK files rejected:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "depends on SDK 3.6.2 and on the corpus in references/pub"]
fn whole_pub_corpus_is_accepted() {
    // `DARTFORGE_PUB_CORPUS` points to another clone (in a worktree, the main
    // checkout's: `references/` is outside git).
    let root = std::env::var("DARTFORGE_PUB_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub"));
    let (accepted, total, failures) = measure(&root, true);
    assert!(total > 0, "corpus not found at {}", root.display());
    assert_eq!(
        accepted,
        total,
        "corpus files rejected:\n{}",
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
