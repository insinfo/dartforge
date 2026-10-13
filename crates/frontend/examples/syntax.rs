//! Measures the new parser against directories of Dart sources.
//!
//! `cargo run -q -p dartforge-frontend --example syntax -- C:/tools/dartsdk-3.6.2/lib references/pub`
//!
//! Prints each diagnostic with file, line and the line of code, and at the
//! end the total of accepted files and the count per message. With
//! `--summary-only` it only prints the summary.
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

fn main() {
    let mut roots = Vec::new();
    let mut summary_only = false;
    for arg in std::env::args().skip(1) {
        if arg == "--summary-only" {
            summary_only = true;
        } else {
            roots.push(arg);
        }
    }
    let mut files = Vec::new();
    for root in &roots {
        collect(Path::new(root), &mut files);
    }
    files.sort();
    let mut total = 0usize;
    let mut accepted = 0usize;
    let mut count: std::collections::BTreeMap<String, usize> = Default::default();
    let mut names = dartforge_intern::Interner::new();
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        total += 1;
        let output = dartforge_frontend::parser::parse(&source, &mut names);
        if output.diagnostics.is_empty() {
            accepted += 1;
            continue;
        }
        for d in output.diagnostics {
            let start_offset = d.span.start.min(source.len());
            let line = source[..start_offset].matches('\n').count() + 1;
            let line_start = source[..start_offset].rfind('\n').map_or(0, |i| i + 1);
            let line_end = source[start_offset..]
                .find('\n')
                .map_or(source.len(), |i| start_offset + i);
            let text = source[line_start..line_end].trim();
            let key = d
                .message
                .split(", encontrou")
                .next()
                .unwrap_or("")
                .to_string();
            *count.entry(key).or_default() += 1;
            if !summary_only {
                println!(
                    "{}\n    {}:{}\n    {}",
                    d.message,
                    file.display(),
                    line,
                    text.chars().take(160).collect::<String>()
                );
            }
        }
    }
    println!("\n== {accepted}/{total} files accepted by the parser ==");
    let mut sorted: Vec<_> = count.into_iter().collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
    for (message, n) in sorted.iter().take(40) {
        println!("{n:5}  {message}");
    }
}
