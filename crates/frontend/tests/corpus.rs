//! The new parser accepts 100% of the SDK 3.6.2 `lib/` and of the pub.dev
//! corpus.
//!
//! This is the acceptance criterion of step 1 of the governing goal
//! (PLANO.md). The external acceptance tests are ignored by default because they
//! depend on files outside the repository: `DARTFORGE_SDK_LIB` (default
//! `C:/tools/dartsdk-3.6.2/lib`) and `references/pub` at the workspace root.
//! Missing inputs and read failures must fail instead of shrinking the corpus.
use std::io;
use std::path::{Path, PathBuf};

/// Enumerates Dart files recursively, preserving errors from every directory.
fn collect(dir: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    let inputs = std::fs::read_dir(dir).map_err(|error| at_path(dir, error))?;
    for input in inputs {
        let input = input.map_err(|error| at_path(dir, error))?;
        let path = input.path();
        let metadata = path.metadata().map_err(|error| at_path(&path, error))?;
        if metadata.is_dir() {
            collect(&path, output)?;
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
    Ok(())
}

/// Keeps the failing path in diagnostics without losing the I/O error kind.
fn at_path(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

/// Requires a nonempty corpus for both parser and lexer acceptance checks.
fn source_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect(root, &mut files)?;
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no Dart sources in {}", root.display()),
        ));
    }
    files.sort();
    Ok(files)
}

/// Reads a complete UTF-8 source; unreadable input cannot count as acceptance.
fn read_source(path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path).map_err(|error| at_path(path, error))
}

#[derive(Debug)]
struct Acceptance {
    accepted: usize,
    total: usize,
    failures: Vec<String>,
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
    let text = std::fs::read_to_string(pubspec).ok()?;
    let line = text.lines().find(|l| l.trim_start().starts_with("sdk:"))?;
    let value = line
        .trim_start()
        .trim_start_matches("sdk:")
        .trim()
        .trim_matches(['\'', '"']);
    let lower = value
        .trim_start_matches('^')
        .trim_start_matches(">=")
        .trim();
    let mut parts = lower.split(['.', ' ']);
    let greater: u16 = parts.next()?.parse().ok()?;
    let less: u16 = parts.next()?.parse().ok()?;
    Some(dartforge_frontend::LanguageVersion::new(greater, less))
}

/// `by_package`: the version of each file without a marker comes from the
/// package's pubspec. Not in the SDK: the `dart:` libraries are read at the
/// SDK's current version, even with the development `pubspec.yaml` some
/// folders carry (`_internal/js_runtime`: `sdk: '>=2.2.2'`).
fn measure(root: &Path, by_package: bool) -> io::Result<Acceptance> {
    let files = source_files(root)?;
    let mut names = dartforge_intern::Interner::new();
    let mut accepted = 0;
    let mut failures = Vec::new();
    for file in &files {
        let source = read_source(file)?;
        // The version of each file (docs/VERSOES-LINGUAGEM.md §2): the marker;
        // otherwise the package's, which `pub` takes from the lower bound of
        // `environment: sdk:` in the pubspec; otherwise the 3.6 floor (the SDK).
        let version = dartforge_frontend::features::version_marker(&source)
            .map(|m| m.0)
            .or_else(|| {
                if by_package {
                    package_version(file)
                } else {
                    None
                }
            })
            .unwrap_or(dartforge_frontend::LanguageVersion::FLOOR);
        let features = dartforge_frontend::LibraryFeatures::new(version, &[]);
        let output = dartforge_frontend::parser::parse_with(&source, &mut names, features);
        if output.diagnostics.is_empty() {
            accepted += 1;
        } else {
            let d = &output.diagnostics[0];
            let line = source[..d.span.start.min(source.len())]
                .matches('\n')
                .count()
                + 1;
            failures.push(format!("{}:{}: {}", file.display(), line, d.message));
        }
    }
    Ok(Acceptance {
        accepted,
        total: files.len(),
        failures,
    })
}

#[test]
#[ignore = "depends on SDK 3.6.2 and on the corpus in references/pub"]
fn whole_sdk_lib_is_accepted() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let Acceptance {
        accepted,
        total,
        failures,
    } = measure(Path::new(&sdk), false).unwrap();
    eprintln!("SDK acceptance: {accepted}/{total}");
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
    let Acceptance {
        accepted,
        total,
        failures,
    } = measure(&root, true).unwrap();
    eprintln!("Pub corpus acceptance: {accepted}/{total}");
    assert_eq!(
        accepted,
        total,
        "corpus files rejected:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "depends on SDK 3.6.2"]
fn whole_sdk_lib_lexes() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let files = source_files(Path::new(&sdk)).unwrap();
    let mut failures = Vec::new();
    for file in &files {
        let source = read_source(file).unwrap();
        if let Err(d) = dartforge_frontend::lexer::lex(&source) {
            let line = source[..d.span.start.min(source.len())]
                .matches('\n')
                .count()
                + 1;
            failures.push(format!("{}:{}: {}", file.display(), line, d.message));
        }
    }
    eprintln!("SDK lexer: {} files", files.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn missing_and_empty_corpora_cannot_pass() {
    let root = tempfile::tempdir().unwrap();
    for path in [root.path().join("missing"), root.path().to_path_buf()] {
        assert_eq!(
            source_files(&path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(measure(&path, false).is_err());
    }
    std::fs::write(root.path().join("README.txt"), "No Dart sources").unwrap();
    assert!(source_files(root.path()).is_err());
}

#[test]
fn unreadable_sources_are_errors_instead_of_smaller_corpora() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid.dart"), "void main() {}").unwrap();
    let invalid = root.path().join("invalid.dart");
    std::fs::write(&invalid, [0xff]).unwrap();
    let error = measure(root.path(), false).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("invalid.dart"));
    assert!(read_source(&invalid).is_err());
}

#[test]
fn acceptance_counts_nested_sources_and_rejected_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid.dart"), "void main() {}").unwrap();
    std::fs::create_dir(root.path().join("nested")).unwrap();
    std::fs::write(root.path().join("nested/invalid.dart"), "class {").unwrap();
    let result = measure(root.path(), false).unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.accepted, 1);
    assert_eq!(result.failures.len(), 1);
    assert!(result.failures[0].contains("invalid.dart"));
}
