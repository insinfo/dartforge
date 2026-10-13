//! Check of the CSS shim (`css::shim`, the port of `shimShadowCss`) against
//! the official compiler, byte for byte. Both tests depend on files from
//! outside the repository and are `#[ignore]`:
//!
//! - `project_stylesheets_equal_to_official`: walks the output of an official
//!   build (`DARTFORGE_SHIM_GERADO`, the `.dart_tool/build/generated`) and, for
//!   each `X.css.shim.dart`, generates ours with [`gerar_folha`] from the
//!   `X.css` next to it (output of `sass_builder`) or, if there is none, from
//!   the `X.css` in the package (`DARTFORGE_SHIM_RAIZES`, `package=dir`
//!   separated by `,`).
//!
//!   ```text
//!   DARTFORGE_SHIM_GERADO=…/example/.dart_tool/build/generated \
//!   DARTFORGE_SHIM_RAIZES=limitless_ui=…/limitless_ui,limitless_ui_example=…/example \
//!   cargo test --release -p dartforge-ng-generator --test shim_check -- --ignored --nocapture
//!   ```
//!
//! - `shim_equal_to_dart_oracle`: reads the output of a Dart program that calls
//!   the official `shimShadowCss` on a list of `.css` files
//!   (`DARTFORGE_SHIM_ORACULO`; records `=== path`, `OK`/`ERR`, the text and
//!   `\0`) and compares: equal output, or a refusal where the official one
//!   throws.
use std::path::{Path, PathBuf};

use dartforge_ng_generator::{Pacote, css, gerar_folha, shadow_css};

fn files(dir: &Path, suffix: &str, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, suffix, output);
        } else if p.to_string_lossy().ends_with(suffix) {
            output.push(p);
        }
    }
}

/// The first position where `a` and `b` differ, with an excerpt of each.
fn difference(a: &str, b: &str) -> String {
    let i = a
        .char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map(|((i, _), _)| i)
        .unwrap_or(a.len().min(b.len()));
    let excerpt = |s: &str| {
        let start = s.floor_char_boundary(i.saturating_sub(60));
        let end = s.ceil_char_boundary((i + 80).min(s.len()));
        s[start..end].to_string()
    };
    format!(
        "byte {i}\n  official: {}\n  ours:     {}",
        excerpt(a),
        excerpt(b)
    )
}

#[test]
#[ignore]
fn project_stylesheets_equal_to_official() {
    let Ok(generated) = std::env::var("DARTFORGE_SHIM_GERADO") else {
        eprintln!("set DARTFORGE_SHIM_GERADO");
        return;
    };
    let generated = PathBuf::from(generated);
    let roots: Vec<(String, PathBuf)> = std::env::var("DARTFORGE_SHIM_RAIZES")
        .unwrap_or_default()
        .split(',')
        .filter_map(|pair| {
            let (n, d) = pair.split_once('=')?;
            Some((n.to_string(), PathBuf::from(d)))
        })
        .collect();
    let mut shims = Vec::new();
    files(&generated, ".css.shim.dart", &mut shims);
    shims.sort();
    let (mut equal, mut different, mut refused) = (0, Vec::new(), Vec::new());
    for shim in &shims {
        let official = std::fs::read_to_string(shim)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let text = shim.to_string_lossy();
        let mut css = PathBuf::from(text.trim_end_matches(".shim.dart"));
        if !css.is_file() {
            // A stylesheet of the package itself: `generated/<package>/<path>`.
            let relative = css.strip_prefix(&generated).map(Path::to_path_buf);
            if let Ok(rel) = relative {
                let mut parts = rel.components();
                let package = parts
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().to_string());
                if let Some((_, root)) = roots.iter().find(|(n, _)| Some(n) == package.as_ref()) {
                    css = root.join(parts.as_path());
                }
            }
        }
        let package = Pacote::default();
        let outputs = gerar_folha(&package, &css);
        let ours = outputs
            .into_iter()
            .find(|(p, _)| p.to_string_lossy().ends_with(".shim.dart"))
            .map(|(_, r)| r);
        match ours {
            Some(Ok(t)) if t == official => equal += 1,
            Some(Ok(t)) => different.push((shim.clone(), difference(&official, &t))),
            Some(Err(r)) => refused.push((shim.clone(), format!("{r:?}"))),
            None => refused.push((shim.clone(), "no output".into())),
        }
    }
    for (p, d) in &different {
        eprintln!("DIFFERENT {}\n{d}", p.display());
    }
    for (p, r) in &refused {
        eprintln!("REFUSED {}: {r}", p.display());
    }
    println!(
        "project shim: {} stylesheets, {equal} equal, {} different, {} refused",
        shims.len(),
        different.len(),
        refused.len()
    );
    assert!(different.is_empty() && refused.is_empty());
}

#[test]
#[ignore]
fn shim_equal_to_dart_oracle() {
    let Ok(oracle) = std::env::var("DARTFORGE_SHIM_ORACULO") else {
        eprintln!("set DARTFORGE_SHIM_ORACULO");
        return;
    };
    let text = std::fs::read_to_string(&oracle).expect("oracle");
    let (mut total, mut equal, mut equal_errors) = (0, 0, 0);
    let mut different = Vec::new();
    let mut reasons = std::collections::BTreeMap::new();
    for record in text.split("\0\n") {
        let Some(rest) = record.strip_prefix("=== ") else {
            continue;
        };
        let Some((path, rest)) = rest.split_once('\n') else {
            continue;
        };
        let Ok(css) = std::fs::read_to_string(path) else {
            continue;
        };
        // The oracle's `readAsStringSync` (`utf8.decode`, like the builder's
        // `readAsString`) strips the leading BOM.
        let css = css.strip_prefix('\u{FEFF}').unwrap_or(&css);
        total += 1;
        let ours = css::shim(css);
        if let Some(expected) = rest.strip_prefix("OK\n") {
            let expected = expected.strip_suffix('\n').unwrap_or(expected);
            match ours {
                Ok(t) if t == expected => equal += 1,
                Ok(t) => different.push(format!("{path}\n{}", difference(expected, &t))),
                Err(_) => different.push(format!("{path}\n  official generates, we refuse")),
            }
        } else {
            match ours {
                Err(_) => {
                    equal_errors += 1;
                    let our_reason =
                        shadow_css::shim_shadow_css(css, "_ngcontent-%ID%", "_nghost-%ID%")
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_default();
                    let official = rest.lines().nth(1).unwrap_or("").to_string();
                    *reasons.entry((official, our_reason)).or_insert(0usize) += 1;
                }
                Ok(_) => different.push(format!(
                    "{path}\n  official throws ({}), we generate",
                    rest.trim()
                )),
            }
        }
    }
    for d in &different {
        eprintln!("DIFFERENT {d}");
    }
    for ((official, ours), n) in &reasons {
        println!("  official exception {official:<20} ours {ours:<45} {n}");
    }
    println!(
        "dart oracle: {total} stylesheets, {equal} equal, {equal_errors} refused as in the official one, {} different",
        different.len()
    );
    assert!(different.is_empty());
}
