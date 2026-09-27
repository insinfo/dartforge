//! Memória da sessão semântica pelo protocolo: o programa fica retido
//! enquanto a versão vale e cai a cada edição, sem acumular. Binário à
//! parte de propósito: o alocador contador é global, e testes do mesmo
//! binário rodam em paralelo no mesmo processo.

#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

mod comum;

use comum::Projeto;
use serde_json::json;
use std::time::Duration;

#[test]
fn memoria_volta_a_base_depois_de_cada_edicao() {
    let mut p = Projeto::novo("sessao-memoria");
    let texto = |n: usize| format!("int f() => {n};\nint g() => f();\n");
    p.abrir("lib/a.dart", &texto(0));
    // Base: a primeira consulta (índices e caches de tamanho fixo já
    // montados) e a primeira edição (descarta a entrada).
    p.na_posicao("textDocument/hover", "lib/a.dart", 1, 11, json!({}));
    p.mudar("lib/a.dart", 2, &texto(1));
    p.servidor.aguardar_diagnosticos(Duration::from_secs(120));
    let base = dartforge_instrument::live_bytes();
    let mut retido = 0;
    for versao in 3..23 {
        assert_eq!(
            p.na_posicao("textDocument/hover", "lib/a.dart", 1, 11, json!({}))["result"]["contents"],
            "int f()"
        );
        retido = retido.max(dartforge_instrument::live_bytes().saturating_sub(base));
        p.mudar("lib/a.dart", versao, &texto(versao as usize));
        p.servidor.aguardar_diagnosticos(Duration::from_secs(120));
    }
    let depois = dartforge_instrument::live_bytes().saturating_sub(base);
    println!(
        "sessão: retido enquanto válido até {retido} bytes; depois de 20 edições, {depois} bytes acima da base"
    );
    assert!(
        retido > 0,
        "a sessão devia reter o programa entre consultas"
    );
    assert!(
        depois < 64 * 1024,
        "{depois} bytes presos depois das edições"
    );
}
