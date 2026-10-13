//! Emits each given program and writes all modules of each one, concatenated in
//! order, into a single file per program — to compare the emitter's output before
//! and after a change (`diff -r`), without Node.
//!
//! `cargo run --release -p dartforge-emit-js --example emit_corpus -- <out_dir> corpus/js/*.dart`
//!
//! With `DARTFORGE_JS_CONFERIR_TIPOS` set, each compilation also checks the
//! emitter's types against the shared inference (`crates/emit_js/src/type_check.rs`).
//! With `EMITIR_CORPUS_EXECUTAVEL=1`, each program is also written ready for Node
//! in `<out_dir>/<name>/` (`node <out_dir>/<name>/main.mjs`), with the
//! `dart_sdk.js` from `DARTFORGE_DART_SDK_JS`.

fn main() {
    let mut args = std::env::args().skip(1);
    let out_dir = std::path::PathBuf::from(
        args.next()
            .expect("usage: emit_corpus <out_dir> <file.dart>..."),
    );
    std::fs::create_dir_all(&out_dir).expect("create output directory");
    let mut failures = 0usize;
    for file in args {
        let path = std::path::PathBuf::from(&file);
        let name = path
            .file_stem()
            .expect("file name")
            .to_string_lossy()
            .to_string();
        let text = match dartforge_emit_js::compile(&path, None, None) {
            Ok(e) => {
                if std::env::var_os("EMITIR_CORPUS_EXECUTAVEL").is_some() {
                    let dir = out_dir.join(&name);
                    dartforge_emit_js::write(&e, &dir, &dartforge_emit_js::default_dart_sdk_js())
                        .expect("write the runnable program");
                }
                let mut modules = e.modules;
                modules.sort();
                let mut t = String::new();
                for (m, body) in modules {
                    t.push_str(&format!("//// {m}\n{body}"));
                }
                t
            }
            Err(e) => {
                failures += 1;
                eprintln!("{file}: failed: {e}");
                format!("//// ERRO\n{e}\n")
            }
        };
        std::fs::write(out_dir.join(format!("{name}.js")), text).expect("write output");
    }
    if failures > 0 {
        eprintln!("{failures} program(s) did not compile");
    }
}
