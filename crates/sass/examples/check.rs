//! Checks the compiler against dart-sass 1.102.0 outputs recorded by an
//! oracle (`SassBuilder` reproduced in a Dart program).
//!
//! Usage: `cargo run --release -p dartforge-sass --example check --
//! <manifest> <oracle dir> [diff dir]` (with `SASS_VERSION=1.66.0`, the
//! compatibility mode with dart-sass 1.66.0).
//!
//! The manifest has one line per `.scss`: `<file>\t<package root>\t
//! <package name>\t<package_config.json or ->`. The oracle, for line `i`,
//! writes `i.expanded.css`, `i.expanded.css.map`, `i.compressed.css`,
//! `i.compressed.css.map` (the `.css` as `sass_builder` writes it, with the
//! `sourceMappingURL` comment), or `i.<style>.err` when dart-sass refuses.
use dartforge_sass::sass_builder::{compile, Asset, Disk, Packages};
use dartforge_sass::{OutputStyle, DartSassVersion};

/// The imitated version: `SASS_VERSION` (`1.66.0` or `1.102.0`, the
/// default).
fn version() -> DartSassVersion {
    std::env::var("SASS_VERSION")
        .ok()
        .and_then(|v| DartSassVersion::from_lock(&v))
        .unwrap_or_default()
}
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let manifest = std::fs::read_to_string(&args[1]).expect("manifest");
    let oracle = PathBuf::from(&args[2]);
    let diffs = args.get(3).map(PathBuf::from);
    if let Some(d) = &diffs {
        let _ = std::fs::create_dir_all(d);
    }
    let (mut css_ok, mut map_ok, mut total, mut err_ok) = (0, 0, 0, 0);
    let mut bad = Vec::new();
    for (i, line) in manifest.lines().enumerate() {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 4 {
            continue;
        }
        let (file, root, name, cfg) = (Path::new(c[0]), Path::new(c[1]), c[2], c[3]);
        let mut packages = if cfg == "-" {
            Packages::new()
        } else {
            Packages::from_package_config(Path::new(cfg)).expect("cfg")
        };
        packages.insert(name, root);
        let path = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let input = Asset {
            package: name.into(),
            path,
        };
        let source = std::fs::read_to_string(file).unwrap_or_default();
        for (style, en) in [
            (OutputStyle::Expanded, "expanded"),
            (OutputStyle::Compressed, "compressed"),
        ] {
            total += 1;
            let base = oracle.join(format!("{i}.{en}"));
            let expected_css =
                std::fs::read_to_string(base.with_extension(format!("{en}.css"))).ok();
            let expected_map =
                std::fs::read_to_string(base.with_extension(format!("{en}.css.map"))).ok();
            // Each compilation in a thread with a large stack and a deadline:
            // an infinite loop or deep recursion does not bring the check down.
            let (tx, rx) = std::sync::mpsc::channel();
            {
                let (source, input, packages) = (source.clone(), input.clone(), packages.clone());
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn(move || {
                        let r = std::panic::catch_unwind(|| {
                            compile(&source, &input, &packages, &Disk, style, true, version())
                        })
                        .map(|r| r.map_err(|e| e.to_string()));
                        let _ = tx.send(r);
                    })
                    .expect("thread");
            }
            let r = match rx.recv_timeout(std::time::Duration::from_secs(20)) {
                Ok(r) => r,
                Err(_) => {
                    bad.push(format!("{i} {en} {}: TIMEOUT", file.display()));
                    continue;
                }
            };
            let label = format!("{i} {en} {}", file.display());
            match (r, expected_css) {
                (Ok(Ok(c)), Some(e)) => {
                    // The source map trailer goes in even before the map exists.
                    let mut with_map = c.clone();
                    with_map.map.get_or_insert_with(String::new);
                    let ours = with_map.css_file(&input);
                    let same = ours == e;
                    css_ok += same as usize;
                    let same_map = c.map.as_deref() == expected_map.as_deref();
                    map_ok += same_map as usize;
                    if !same || !same_map {
                        bad.push(format!(
                            "{label}: {}{}",
                            if same { "" } else { "CSS " },
                            if same_map { "" } else { "MAP" }
                        ));
                        if let Some(d) = &diffs {
                            let _ = std::fs::write(d.join(format!("{i}.{en}.ours.css")), &ours);
                            let _ = std::fs::write(d.join(format!("{i}.{en}.official.css")), &e);
                            let _ = std::fs::write(
                                d.join(format!("{i}.{en}.ours.map")),
                                c.map.unwrap_or_default(),
                            );
                            let _ = std::fs::write(
                                d.join(format!("{i}.{en}.official.map")),
                                expected_map.unwrap_or_default(),
                            );
                        }
                    }
                }
                (Ok(Err(_)), None) => {
                    err_ok += 1;
                    css_ok += 1;
                    map_ok += 1;
                }
                (Ok(Err(e)), Some(_)) => bad.push(format!(
                    "{label}: our ERROR: {}",
                    e.lines().next().unwrap_or("")
                )),
                (Ok(Ok(_)), None) => bad.push(format!("{label}: official rejects, ours compiles")),
                (Err(_), _) => bad.push(format!("{label}: PANIC")),
            }
        }
    }
    for r in &bad {
        println!("{r}");
    }
    println!("total {total}: css equal {css_ok}, maps equal {map_ok} (errors in both: {err_ok})");
}
