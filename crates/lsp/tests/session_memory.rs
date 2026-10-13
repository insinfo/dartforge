//! Memória da sessão semântica pelo protocolo: o programa fica retido
//! enquanto a versão vale e cai a cada edição, sem acumular. Binário à
//! parte de propósito: o alocador contador é global, e testes do mesmo
//! binário rodam em paralelo no mesmo processo.

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
    // Base: a primeira consulta (índices e caches de tamanho fixo já
    // montados) e a primeira edição (descarta a entrada).
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
