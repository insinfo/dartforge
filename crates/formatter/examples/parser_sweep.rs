//! Sweep of the formatter's parser over `.dart` files.
//!
//! ```text
//! cargo run --release -p dartforge-formatter --example parser_sweep -- \
//!     [--dump] [--list] [<dir|file>...]
//! ```
//!
//! For each file, runs [`parse`] and, if parsing succeeded, the token
//! coverage check ([`verify::check`]). Prints the number of accepted files,
//! rejected files and files with coverage failures, and the total time. With
//! `--list`, paths come from standard input, one per line. With `--dump`,
//! writes to standard output, for each file, `### path` followed by
//! `ERR <message>` or the tree dump ([`verify::dump`]), in the format of the
//! analyzer oracle.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use dartforge_formatter::syntax::parser::{parse, verify};

fn collect(path: &Path, output: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(inputs) = std::fs::read_dir(path) else {
            return;
        };
        let mut v: Vec<_> = inputs.flatten().map(|e| e.path()).collect();
        v.sort();
        for p in v {
            if p.file_name()
                .is_some_and(|n| n == "target" || n == ".dart_tool" || n == ".git")
            {
                continue;
            }
            collect(&p, output);
        }
    } else if path.extension().is_some_and(|e| e == "dart") {
        output.push(path.to_path_buf());
    }
}

fn main() {
    let mut dump = false;
    let mut list = false;
    let mut paths = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--dump" => dump = true,
            "--list" => list = true,
            _ => collect(Path::new(&a), &mut paths),
        }
    }
    if list {
        for l in std::io::stdin().lock().lines().map_while(Result::ok) {
            if !l.is_empty() {
                paths.push(PathBuf::from(l));
            }
        }
    }
    let output = std::io::stdout();
    let mut output = output.lock();
    let (mut ok, mut rejected, mut failures) = (0usize, 0usize, 0usize);
    let start = Instant::now();
    for p in &paths {
        let Ok(source) = std::fs::read_to_string(p) else {
            continue;
        };
        let r = parse(&source);
        if dump {
            let _ = writeln!(output, "### {}", p.display());
        }
        match r {
            Ok((tokens, tree)) => {
                ok += 1;
                if let Err(e) = verify::check(&tokens, &tree, &source) {
                    failures += 1;
                    eprintln!("COBERTURA {}: {e}", p.display());
                }
                if dump {
                    let _ = write!(output, "{}", verify::dump(&tokens, &tree));
                }
            }
            Err(e) => {
                rejected += 1;
                if dump {
                    let _ = writeln!(output, "ERR {} @{}", e.message, e.offset);
                } else {
                    eprintln!("REJECTED {}:{}: {}", p.display(), e.offset, e.message);
                }
            }
        }
    }
    eprintln!(
        "{} files: {ok} accepted, {rejected} rejected, {failures} coverage failures, {:.2?}",
        paths.len(),
        start.elapsed()
    );
}
