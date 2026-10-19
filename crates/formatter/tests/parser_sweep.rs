//! Parser sweep over the `.dart` files of the repository and of the SDK's
//! `lib/` (ignored by default: it depends on the installed SDK and takes a
//! few seconds). `DARTFORGE_SDK_LIB` overrides the default SDK location.
//!
//! ```text
//! cargo test -p dartforge-formatter --test parser_sweep -- --ignored --nocapture
//! ```
//!
//! For every accepted file, checks token coverage ([`verify::check`]); requires
//! the whole `lib/` of SDK 3.6.2 (which `dart format` accepts entirely) to be
//! accepted. The node-by-node comparison with the analyzer lives in the
//! `parser_sweep --dump` example plus the analyzer dump (see the task report).

use std::io;
use std::path::{Path, PathBuf};

use dartforge_formatter::syntax::parser::{parse, verify};

/// Walks every source while keeping directory and metadata failures visible.
fn collect(path: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    if path
        .metadata()
        .map_err(|error| at_path(path, error))?
        .is_dir()
    {
        let inputs = std::fs::read_dir(path).map_err(|error| at_path(path, error))?;
        for entry in inputs {
            let p = entry.map_err(|error| at_path(path, error))?.path();
            if p.file_name()
                .is_some_and(|n| n == "target" || n == ".dart_tool" || n == "references")
            {
                continue;
            }
            collect(&p, output)?;
        }
    } else if path.extension().is_some_and(|e| e == "dart") {
        output.push(path.to_path_buf());
    }
    Ok(())
}

/// Attaches the failing path without discarding the error kind.
fn at_path(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

#[derive(Debug, Default)]
struct Sweep {
    accepted: usize,
    rejected: Vec<String>,
    coverage_failures: Vec<String>,
}

/// Parses a nonempty corpus, checking token coverage for every accepted file.
fn sweep(root: &Path) -> io::Result<Sweep> {
    let mut files = Vec::new();
    collect(root, &mut files)?;
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no Dart sources in {}", root.display()),
        ));
    }
    files.sort();
    let mut result = Sweep::default();
    for p in files {
        let source = std::fs::read_to_string(&p).map_err(|error| at_path(&p, error))?;
        match parse(&source) {
            Ok((t, a)) => {
                result.accepted += 1;
                if let Err(e) = verify::check(&t, &a, &source) {
                    result
                        .coverage_failures
                        .push(format!("{}: {e}", p.display()));
                }
            }
            Err(e) => result
                .rejected
                .push(format!("{}:{}: {}", p.display(), e.offset, e.message)),
        }
    }
    Ok(result)
}

#[test]
#[ignore = "sweeps the repository and the installed SDK"]
fn sweep_of_repository_and_sdk() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Sweep {
        accepted: ok,
        rejected,
        coverage_failures: failures,
    } = sweep(&repo).unwrap();
    eprintln!(
        "repository: {ok} accepted, {} rejected, {} coverage failures",
        rejected.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");

    let home = std::env::var("HOME").unwrap_or_default();
    let sdk = std::env::var_os("DARTFORGE_SDK_LIB")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&home).join("tools/dart/3.6.2/dart-sdk/lib"));
    let Sweep {
        accepted: ok,
        rejected,
        coverage_failures: failures,
    } = sweep(&sdk).unwrap();
    eprintln!(
        "sdk: {ok} accepted, {} rejected, {} coverage failures",
        rejected.len(),
        failures.len()
    );
    assert!(rejected.is_empty(), "{rejected:#?}");
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn missing_empty_and_unreadable_corpora_cannot_pass() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        sweep(&root.path().join("missing")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(
        sweep(root.path()).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    std::fs::write(root.path().join("valid.dart"), "void main() {}").unwrap();
    std::fs::write(root.path().join("invalid.dart"), [0xff]).unwrap();
    let error = sweep(root.path()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("invalid.dart"));
}

#[test]
fn rejected_sources_are_counted_and_accepted_sources_are_checked() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid.dart"), "void main() {}").unwrap();
    std::fs::create_dir(root.path().join("nested")).unwrap();
    std::fs::write(root.path().join("nested/invalid.dart"), "void main( {").unwrap();
    let result = sweep(root.path()).unwrap();
    assert_eq!(result.accepted, 1);
    assert_eq!(result.rejected.len(), 1);
    assert!(result.rejected[0].contains("invalid.dart"));
    assert!(result.coverage_failures.is_empty());
}
