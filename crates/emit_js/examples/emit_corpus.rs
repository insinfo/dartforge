//! Emits each given program and writes its modules, concatenated in order, to
//! one file per program for before/after comparisons. Missing inputs, colliding
//! output names and compilation failures produce a failing exit status.
//!
//! `cargo run --release -p dartforge-emit-js --example emit_corpus -- <out_dir> corpus/js/*.dart`
//!
//! With `DARTFORGE_JS_CHECK_TYPES` set, each compilation also checks the
//! emitter's types against shared inference (`crates/emit_js/src/type_check.rs`).
//! With `EMIT_CORPUS_EXECUTABLE=1`, each program is also written ready for Node
//! in `<out_dir>/<name>/` (`node <out_dir>/<name>/main.mjs`), using the
//! `dart_sdk.js` from `DARTFORGE_DART_SDK_JS`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

fn run(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let usage = "usage: emit_corpus <out_dir> <file.dart>...";
    let out_dir = PathBuf::from(args.next().ok_or(usage)?);
    let mut inputs = Vec::new();
    let mut names = HashSet::new();
    for file in args {
        let path = PathBuf::from(&file);
        if !path.is_file() {
            return Err(format!("input is not a file: {file}"));
        }
        let name = path
            .file_stem()
            .ok_or_else(|| format!("input has no file name: {file}"))?
            .to_string_lossy()
            .into_owned();
        if !names.insert(name.clone()) {
            return Err(format!("multiple inputs produce {name}.js"));
        }
        inputs.push((path, name));
    }
    if inputs.is_empty() {
        return Err(usage.into());
    }
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut failures = 0usize;
    for (path, name) in inputs {
        let output = out_dir.join(format!("{name}.js"));
        let emitted = match dartforge_emit_js::compile(&path, None, None) {
            Ok(emitted) => emitted,
            Err(e) => {
                failures += 1;
                eprintln!("{}: failed: {e}", path.display());
                // A previous run's output must not masquerade as this run's result.
                match std::fs::remove_file(&output) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("{}: {e}", output.display())),
                }
                continue;
            }
        };
        if std::env::var_os("EMIT_CORPUS_EXECUTABLE").is_some() {
            dartforge_emit_js::write(
                &emitted,
                &out_dir.join(&name),
                &dartforge_emit_js::default_dart_sdk_js(),
            )?;
        }
        let mut modules = emitted.modules;
        if modules.is_empty() {
            return Err(format!(
                "{}: compilation produced no modules",
                path.display()
            ));
        }
        modules.sort();
        let mut text = String::new();
        for (module, body) in modules {
            text.push_str(&format!("//// {module}\n{body}"));
        }
        std::fs::write(&output, text).map_err(|e| format!("{}: {e}", output.display()))?;
    }
    if failures > 0 {
        Err(format!("{failures} program(s) did not compile"))
    } else {
        Ok(())
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_missing_and_colliding_inputs_before_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("output");
        assert!(run(std::iter::empty()).unwrap_err().contains("usage:"));
        assert!(
            run([output.display().to_string()].into_iter())
                .unwrap_err()
                .contains("usage:")
        );
        assert!(
            run([
                output.display().to_string(),
                tmp.path().join("missing.dart").display().to_string(),
            ]
            .into_iter())
            .unwrap_err()
            .contains("input is not a file")
        );
        let mut args = vec![output.display().to_string()];
        for directory in ["first", "second"] {
            let directory = tmp.path().join(directory);
            std::fs::create_dir(&directory).unwrap();
            let input = directory.join("same.dart");
            std::fs::write(&input, "void main() {}\n").unwrap();
            args.push(input.display().to_string());
        }
        assert!(
            run(args.into_iter())
                .unwrap_err()
                .contains("multiple inputs")
        );
        assert!(!output.exists());
    }

    #[test]
    #[ignore = "requires Dart SDK sources"]
    fn refused_compilation_fails_and_removes_stale_output() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("invalid.dart");
        std::fs::write(&input, "void main( {\n").unwrap();
        let output = tmp.path().join("output");
        std::fs::create_dir(&output).unwrap();
        let stale = output.join("invalid.js");
        std::fs::write(&stale, "previous successful compilation").unwrap();
        let error = run([output.display().to_string(), input.display().to_string()].into_iter())
            .unwrap_err();
        assert_eq!(error, "1 program(s) did not compile");
        assert!(!stale.exists());
    }
}
