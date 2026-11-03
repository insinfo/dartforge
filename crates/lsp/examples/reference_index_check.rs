//! Compares the references the resolved-reference index gives with those of
//! the search without it, at every identifier of the project's files (one
//! position per target and file): the acceptance test of the index
//! (docs/LSP.md, "Décima quinta rodada").
//!
//! `cargo run -q --release -p dartforge-lsp --example reference_index_check -- <project> [file.dart…]`
//!
//! Every `.dart` file of `lib/` and `bin/` is opened; without files after the
//! project, all of them are checked. The SDK is found as the server finds it
//! (`DARTFORGE_SDK_LIB`, …). With `DARTFORGE_INDEX_CHECK_RESIDENT=1` the
//! checked files' records come from a resident's analyses, as after an edit
//! in the editor (docs/LSP.md, "Décima sexta rodada").
use dartforge_lsp::{DocumentStore, SemanticAnalyzer};
use std::path::{Path, PathBuf};

fn dart_in(d: &Path, output: &mut Vec<PathBuf>) {
    let Ok(r) = std::fs::read_dir(d) else { return };
    for e in r.flatten() {
        let p = e.path();
        if p.is_dir() {
            dart_in(&p, output);
        } else if p.extension().is_some_and(|x| x == "dart") {
            output.push(p);
        }
    }
}

fn uri_of(p: &Path) -> String {
    url::Url::from_file_path(dartforge_elements::config::without_verbatim(
        p.to_path_buf(),
    ))
    .expect("file URI")
    .to_string()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(project) = args.next() else {
        eprintln!("usage: reference_index_check <project> [file.dart…]");
        std::process::exit(2);
    };
    let root = std::fs::canonicalize(&project).expect("project");
    let mut files = Vec::new();
    dart_in(&root.join("lib"), &mut files);
    dart_in(&root.join("bin"), &mut files);
    files.sort();
    let mut documents = DocumentStore::new();
    for f in &files {
        let text = std::fs::read_to_string(f).expect("read");
        documents.open(uri_of(f), 1, text);
    }
    let chosen: Vec<PathBuf> = args
        .map(|a| std::fs::canonicalize(root.join(a)).expect("file"))
        .collect();
    let uris: Vec<String> = if chosen.is_empty() {
        files.iter().map(|f| uri_of(f)).collect()
    } else {
        chosen.iter().map(|f| uri_of(f)).collect()
    };
    let mut analyzer = SemanticAnalyzer::discover();
    print!("{}", analyzer.check_reference_index(&documents, &uris));
}
