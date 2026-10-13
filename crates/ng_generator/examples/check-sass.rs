//! Compares our Sass with that of `sass_builder`, file by file.
//!
//! `build_runner` has already left its output in `.dart_tool/build/generated`;
//! here each `.scss` of the project is compiled by both and the results are
//! compared **normalized** — the intermediate CSS does not need the same
//! spacing, it needs to be the same CSS. The normalization is done by the
//! ngdart shim, which is exactly what consumes this output.
//!
//! ```text
//! cargo run -p dartforge-ng-generator --example check-sass -- C:/MyDartProjects/new_sali/frontend
//! ```
use dartforge_ng_generator::{css, sass};
use std::path::{Path, PathBuf};

fn main() -> std::process::ExitCode {
    let Some(root) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: check-sass <project root>");
        return std::process::ExitCode::FAILURE;
    };
    let root =
        dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&root).unwrap_or(root));
    let generated = root.join(".dart_tool/build/generated");
    let package = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut packages: Vec<PathBuf> = Vec::new();
    if let Ok(e) = std::fs::read_dir(&generated) {
        packages.extend(e.flatten().map(|x| x.path()).filter(|p| p.is_dir()));
    }

    // `--bytes`: compares the CSS of `compilar_com(.., Comprimido)` byte for
    // byte with that of `sass_builder` (without the source map comment),
    // instead of after the shim.
    let bytes = std::env::args().any(|a| a == "--bytes");
    let (mut equal, mut different, mut refused, mut no_official) = (0, 0, 0, 0);
    let mut examples: Vec<String> = Vec::new();
    for scss in files(&root, "scss") {
        // The vendored code (`web/assets/js/...`) is not the project's.
        if scss.to_string_lossy().contains("assets") {
            continue;
        }
        let Ok(rel) = scss.strip_prefix(&root) else {
            continue;
        };
        let official = packages
            .iter()
            .map(|p| p.join(rel).with_extension("css"))
            .find(|p| p.is_file())
            .or_else(|| {
                let alt = generated.join(&package).join(rel).with_extension("css");
                alt.is_file().then_some(alt)
            });
        let Some(official) = official else {
            no_official += 1;
            continue;
        };
        let Ok(source) = std::fs::read_to_string(&scss) else {
            continue;
        };
        if bytes {
            let theirs = std::fs::read_to_string(&official).unwrap_or_default();
            let theirs = theirs.trim_start_matches('\u{feff}');
            let theirs = match theirs.find("\n/*# sourceMappingURL=") {
                Some(i) => &theirs[..i],
                None => theirs,
            };
            match sass::compilar_com(&source, scss.parent(), sass::Estilo::Comprimido) {
                Ok((a, _)) if a == theirs => equal += 1,
                Ok((a, _)) => {
                    different += 1;
                    let i = a
                        .char_indices()
                        .zip(theirs.chars())
                        .find(|((_, x), y)| x != y)
                        .map(|((i, _), _)| i)
                        .unwrap_or(a.len().min(theirs.len()));
                    examples.push(format!(
                        "different: {} (byte {i})\n    official: …{}\n    ours:     …{}",
                        rel.display(),
                        window(theirs, i),
                        window(&a, i)
                    ));
                }
                Err(_) => refused += 1,
            }
            continue;
        }
        let ours = match sass::compilar_em(&source, scss.parent()) {
            Ok(c) => c,
            Err(_) => {
                refused += 1;
                if examples.len() < 30 {
                    examples.push(format!("refused: {}", rel.display()));
                }
                continue;
            }
        };
        let theirs = std::fs::read_to_string(&official).unwrap_or_default();
        // The BOM and the sass_builder source map comment are not CSS.
        let theirs = theirs.trim_start_matches('\u{feff}');
        match (css::shim(&ours), css::shim(theirs)) {
            (Ok(a), Ok(b)) if a == b => equal += 1,
            (Ok(a), Ok(b)) => {
                different += 1;
                // A difference always shows up: it is what breaks the oracle.
                {
                    let i = a
                        .char_indices()
                        .zip(b.chars())
                        .find(|((_, x), y)| x != y)
                        .map(|((i, _), _)| i)
                        .unwrap_or(a.len().min(b.len()));
                    examples.push(format!(
                        "different: {} (byte {i})
    official: …{}
    ours:     …{}",
                        rel.display(),
                        window(&b, i),
                        window(&a, i)
                    ));
                }
            }
            _ => {
                refused += 1;
                if examples.len() < 30 {
                    examples.push(format!("shim refused: {}", rel.display()));
                }
            }
        }
    }
    println!("equal to sass_builder: {equal}");
    println!("different:             {different}");
    println!("refused by us:         {refused}");
    println!("no official .css:      {no_official}");
    for e in &examples {
        println!("  {e}");
    }
    if different == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn window(s: &str, i: usize) -> String {
    let start = i.saturating_sub(40);
    let start = (start..=i).find(|k| s.is_char_boundary(*k)).unwrap_or(i);
    s[start..].chars().take(110).collect()
}

fn files(root: &Path, ext: &str) -> Vec<PathBuf> {
    let mut output = Vec::new();
    let mut stack = vec![root.join("lib"), root.join("web")];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                output.push(p);
            }
        }
    }
    output
}
