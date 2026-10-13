//! Measures the resident session: first compilation, body edit, public API
//! edit and the memory plateau after N edits.
//!
//! `cargo run -q --release -p dartforge-dev --example measure -- <input.dart> [package_config.json] [file_to_edit] [N]`
//!
//! The edited file is restored at the end (the example truncates what it
//! appended).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::{Report, Session};
use std::io::Write;
use std::path::{Path, PathBuf};

const MB: f64 = 1_048_576.0;

/// Appends text to the end of the file and returns the previous size, to
/// restore it afterwards.
fn append(path: &Path, text: &str) -> std::io::Result<u64> {
    let before = std::fs::metadata(path)?.len();
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    f.write_all(text.as_bytes())?;
    f.flush()?;
    Ok(before)
}

fn restore(path: &Path, size: u64) {
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_len(size);
    }
}

fn row(name: &str, r: &Report) {
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{name:<22}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>10.1}{:>8}{:>9}{:>8}",
        ms(r.load),
        ms(r.outline),
        ms(r.bodies),
        ms(r.emit),
        ms(r.hashes),
        ms(r.write),
        ms(r.total()),
        r.reanalyzed_units,
        r.written_modules,
        r.body_changed.len(),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = PathBuf::from(
        args.first().cloned().unwrap_or_else(|| "C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart".into()),
    );
    let packages = args.get(1).map(PathBuf::from).or_else(|| {
        let p = PathBuf::from("C:/MyDartProjects/new_sali/core/.dart_tool/package_config.json");
        p.exists().then_some(p)
    });
    let target = args.get(2).map(PathBuf::from).unwrap_or_else(|| input.clone());
    let edits: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(20);
    // `DARTFORGE_DEV_SAIDA` lets the checker point the session at the same
    // directory that the full `compile-js` wrote.
    let output = std::env::var_os("DARTFORGE_DEV_SAIDA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/dev-measure"));
    if !input.exists() {
        eprintln!("input does not exist: {}", input.display());
        return;
    }

    let mut session = match Session::new(&input, None, packages.as_deref(), &output) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    println!("== resident session — {}", input.display());
    println!(
        "{:<22}{:>9}{:>9}{:>9}{:>9}{:>9}{:>9}{:>10}{:>8}{:>9}{:>8}",
        "compilation", "load", "outline", "bodies", "emission", "hashes", "write", "total ms", "reanal", "written", "body≠"
    );

    let r = session.compile().expect("first compilation");
    row("first", &r);
    let detail = |n: &str, r: &Report| {
        let c = &r.load_detail;
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        println!(
            "   {n}: read+lex {:.1} par / {:.1} serial, parse {:.1}, directives {:.1}, outline {:.1}+{:.1}+{:.1}+{:.1} ms ({} files read)",
            ms(c.parallel_read_lex), ms(c.read), ms(c.parse), ms(c.directives),
            ms(c.outline_declarations), ms(c.outline_reexports), ms(c.outline_scopes), ms(c.outline_supertypes),
            c.files_read
        );
    };
    detail("load of the first", &r);
    let live_after_first = dartforge_instrument::live_bytes();

    // 1. Body edit. Two functions are appended and then a `print` is
    // inserted **into the body of the first one**: it is the real edit (a
    // keystroke inside a method) and shifts everything that comes after,
    // which is where the API hash by text range used to slip.
    let base = append(&target, "\nvoid _forgeA() { print('a'); }\nvoid _forgeB() { print('b'); }\n").expect("edit");
    session.file_changed(&target);
    session.compile().expect("preparation of the body edit");
    restore(&target, base);
    let size = append(
        &target,
        "\nvoid _forgeA() { print('a'); print('x'); }\nvoid _forgeB() { print('b'); }\n",
    )
    .expect("edit");
    session.file_changed(&target);
    let r = session.compile().expect("body edit");
    row("body edit", &r);
    detail("load of the body edit", &r);
    println!(
        "   body changed: {} | API changed: {} (+{} dependents) | reemitted libraries: {} | reemitted modules: {} | emission context {:.1} ms",
        r.body_changed.len(),
        r.api_changed.len(),
        r.invalidated_dependents,
        r.reemitted_libraries,
        r.reemitted_modules,
        r.emit_context.as_secs_f64() * 1000.0
    );
    let api_changed_in_body = r.api_changed.len();
    restore(&target, base);
    let _ = size;

    // 2. Public API edit: a new public top-level function.
    let size = append(&target, "\nvoid forgeApi0() { print('api 0'); }\n").expect("edit");
    session.file_changed(&target);
    let r = session.compile().expect("API edit");
    row("API edit", &r);
    println!(
        "   API changed: {} library(ies), {} dependent(s) invalidated | reemitted libraries: {} | reemitted modules: {} | emission context {:.1} ms",
        r.api_changed.len(),
        r.invalidated_dependents,
        r.reemitted_libraries,
        r.reemitted_modules,
        r.emit_context.as_secs_f64() * 1000.0
    );
    restore(&target, size);

    // 3. Plateau: N body edits in a row.
    println!("\n== memory plateau ({edits} body edits)");
    dartforge_instrument::reset_peak();
    let mut live = Vec::with_capacity(edits);
    let mut sum_ms = 0.0;
    for i in 0..edits {
        let size = append(&target, &format!("\nvoid _forgePlateau{i}() {{ print('p{i}'); }}\n")).expect("edit");
        session.file_changed(&target);
        let r = session.compile().expect("plateau compilation");
        sum_ms += r.total().as_secs_f64() * 1000.0;
        restore(&target, size);
        let v = dartforge_instrument::live_bytes();
        live.push(v);
        if i < 3 || i + 1 == edits {
            println!(
                "  edit {:>2}: live {:>8.2} MB | units in cache {} | retained by the session {:.2} MB | {:.0} ms",
                i + 1,
                v as f64 / MB,
                session.cached_units(),
                session.retained_bytes() as f64 / MB,
                r.total().as_secs_f64() * 1000.0
            );
        }
    }
    let first = live[0] as f64 / MB;
    let last = *live.last().unwrap() as f64 / MB;
    // `scripts/custo-zero.ps1` reads the average from this line with the
    // regex `m\S*dia (\d+(?:\.\d+)?) ms/edi`, against binaries of older
    // commits too: the line keeps its original wording.
    println!(
        "vivos após 1ª edição {first:.2} MB → após {edits}ª {last:.2} MB (crescimento {:+.2} MB); pico {:.2} MB; média {:.2} ms/edição; vivos após a primeira compilação {:.2} MB",
        last - first,
        dartforge_instrument::peak_bytes() as f64 / MB,
        sum_ms / edits as f64,
        live_after_first as f64 / MB
    );
    if api_changed_in_body != 0 {
        println!("warning: the body edit touched the API — the API hash is not separating body from signature");
    }
}
