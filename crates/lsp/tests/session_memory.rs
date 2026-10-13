//! Semantic session memory through the protocol: the program stays retained
//! while the version is valid and drops at each edit, without accumulating.
//! A separate binary on purpose: the counting allocator is global, and tests
//! of the same binary run in parallel in the same process.

#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

mod common;

use common::Project;
use serde_json::json;
use std::time::Duration;

#[test]
fn memory_returns_to_baseline_after_each_edit() {
    let mut p = Project::new("sessao-memoria");
    let text = |n: usize| format!("int f() => {n};\nint g() => f();\n");
    p.open("lib/a.dart", &text(0));
    // Baseline: the first query (indexes and fixed-size caches already built)
    // and the first edit (discards the entry).
    p.in_position("textDocument/hover", "lib/a.dart", 1, 11, json!({}));
    p.change("lib/a.dart", 2, &text(1));
    p.server.await_diagnostics(Duration::from_secs(120));
    let base = dartforge_instrument::live_bytes();
    let mut retained = 0;
    for version in 3..23 {
        assert_eq!(
            p.in_position("textDocument/hover", "lib/a.dart", 1, 11, json!({}))["result"]["contents"],
            "int f()\nType: int Function()"
        );
        retained = retained.max(dartforge_instrument::live_bytes().saturating_sub(base));
        p.change("lib/a.dart", version, &text(version as usize));
        p.server.await_diagnostics(Duration::from_secs(120));
    }
    let after = dartforge_instrument::live_bytes().saturating_sub(base);
    println!(
        "sessão: retido enquanto válido até {retained} bytes; depois de 20 edições, {after} bytes acima da base"
    );
    assert!(
        retained > 0,
        "a sessão devia reter o programa entre consultas"
    );
    assert!(
        after < 64 * 1024,
        "{after} bytes presos depois das edições"
    );
}
