//! Parser sweep over the `.dart` files of the repository and of the SDK's
//! `lib/` (ignored by default: it depends on the installed SDK and takes a
//! few seconds).
//!
//! ```text
//! cargo test -p dartforge-formatter --test parser_sweep -- --ignored --nocapture
//! ```
//!
//! For every accepted file, checks token coverage ([`verify::check`]); requires
//! the whole `lib/` of SDK 3.6.2 (which `dart format` accepts entirely) to be
//! accepted. The node-by-node comparison with the analyzer lives in the
//! `parser_sweep --dump` example plus the analyzer dump (see the task report).

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

/// (accepted, rejected, coverage failures with messages).
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
#[ignore = "sweeps the repository and the installed SDK"]
fn sweep_of_repository_and_sdk() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, rejected, failures) = sweep(&repo);
    eprintln!(
        "repository: {ok} accepted, {} rejected, {} coverage failures",
        rejected.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");

    let home = std::env::var("HOME").unwrap_or_default();
    let sdk = Path::new(&home).join("tools/dart/3.6.2/dart-sdk/lib");
    if sdk.is_dir() {
        let (ok, rejected, failures) = sweep(&sdk);
        eprintln!(
            "sdk: {ok} accepted, {} rejected, {} coverage failures",
            rejected.len(),
            failures.len()
        );
        assert!(rejected.is_empty(), "{rejected:#?}");
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
