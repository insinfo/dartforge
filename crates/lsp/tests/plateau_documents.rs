//! Server memory plateau: K open documents, each edited K times.
//!
//! It is the plateau test of the old compiler (`plato_memoria.rs`, preserved
//! in the `exploracao-initial` branch) applied to the owner of the LSP
//! documents, and it answers the failure observed in Dart's LSP: memory that
//! only grows with each edit until the process has to be killed. The
//! `DocumentStore` promises that the arrival of version N discards N−1 in
//! place and that closing removes everything; the counting allocator turns
//! the promise into an assertion.
//!
//! Two axes, because they fail for different reasons: **edits** (retention
//! per version, the analyzer's failure mode) and **documents** (the floor
//! grows with the number of open documents, and must return to the initial
//! value when all of them close).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::DocumentStore;

/// Fixed-width text per document and revision: the revision always takes
/// three digits so the retained text has the same size in every version and
/// the plateau does not hide content variation behind the tolerance.
fn text(document: usize, revision: usize) -> String {
    format!(
        "int f{document}(int n) {{ var soma = {:03}; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\nvoid main() {{ print(f{document}(3)); }}\n",
        revision % 1000
    )
}

fn uri(document: usize) -> String {
    format!("file:///plato/doc{document}.dart")
}

/// Opens K documents and applies K rounds of edits to each, diagnosing the
/// current text as an editor would; returns the live bytes after each round.
/// The diagnosis is transient and dropped at the end of the iteration.
fn edit(docs: &mut DocumentStore, k: usize) -> Vec<usize> {
    for document in 0..k {
        docs.open(uri(document), 1, text(document, 0));
    }
    let mut live = Vec::with_capacity(k);
    for round in 1..=k {
        for document in 0..k {
            let target = uri(document);
            assert!(docs.update(&target, round as i32 + 1, text(document, round)));
            let diagnostics = docs.diagnose_open(&target);
            assert!(diagnostics.is_empty(), "{target}: {diagnostics:?}");
        }
        live.push(dartforge_instrument::live_bytes());
    }
    live
}

/// `live_bytes` stabilizes over the rounds: the maximum of the second half
/// does not exceed the maximum of the reference plateau beyond the
/// tolerance.
///
/// The tolerance (8 KiB) covers the noise of the harness and of the
/// `HashMap`. Retaining one version per edit would cost the text size times
/// K×K edits — with K = 24 that is 576 texts of ~120 bytes, ~69 KiB, well
/// above the tolerance.
fn k_documents_edited_k_times_settle_on_plateau() {
    const K: usize = 24;
    let mut docs = DocumentStore::new();
    let live = edit(&mut docs, K);
    assert_eq!(live.len(), K);
    let reference = live[4..8].iter().max().copied().unwrap();
    let after = live[8..].iter().max().copied().unwrap();
    assert!(
        after <= reference.saturating_add(8 * 1024),
        "memory grew with the edits: plateau {reference} bytes, tail {after} bytes"
    );
    assert_eq!(docs.len(), K, "each document costs exactly one entry");
}

/// Closing every document returns memory to the level before opening:
/// nothing of the text, the versions or the diagnostics survives `close`.
fn closing_all_returns_to_initial_level() {
    const K: usize = 16;
    let mut docs = DocumentStore::new();
    let before = dartforge_instrument::live_bytes();
    let _ = edit(&mut docs, K);
    let open = dartforge_instrument::live_bytes();
    assert!(
        open > before + K * 100,
        "K open documents must cost at least their text: {before} → {open}"
    );
    for document in 0..K {
        assert!(docs.close(&uri(document)));
    }
    assert!(docs.is_empty());
    let after = dartforge_instrument::live_bytes();
    // The empty `HashMap` keeps the allocated capacity: for K entries hashbrown
    // reserves the next power of two above K/0.875 buckets. Each entry now
    // costs text + line table + version (the server announces incremental sync
    // and needs the table per document), ~80 bytes per bucket against ~56 for
    // the text-only version: 32 buckets give ~2.5 KiB with K = 16, budgeted as
    // K*192. What must not remain is the documents' text, which dominates the
    // cost and is measured by the final assertion after the `drop`.
    assert!(
        after <= before.saturating_add(K * 192),
        "closing did not release the documents: before {before}, after {after}"
    );
    drop(docs);
    assert!(dartforge_instrument::live_bytes() <= before.saturating_add(64));
}

/// A single test with both phases in sequence: the counting allocator is
/// global to the process, and the harness runs `#[test]` functions in
/// parallel threads and frees the structures of a finished test (captured
/// output, result) at a nondeterministic time. With two `#[test]`s, the
/// second would measure those frees as its own — a `Mutex` between them
/// does not help, because the freeing happens outside the locked region.
#[test]
fn documents_plateau() {
    k_documents_edited_k_times_settle_on_plateau();
    closing_all_returns_to_initial_level();
}
