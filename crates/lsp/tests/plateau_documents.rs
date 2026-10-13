//! Platô de memória do servidor: K documentos abertos, cada um editado K vezes.
//!
//! É o teste de platô do compilador antigo (`plato_memoria.rs`, preservado na
//! branch `exploracao-initial`) aplicado ao dono dos documentos do LSP, e responde à falha observada no
//! LSP do Dart: memória que só sobe a cada edição até ser preciso matar o
//! processo. O `DocumentStore` promete que a chegada da versão N descarta a
//! N−1 no lugar e que fechar remove tudo; o alocador contador é o que
//! transforma a promessa em afirmação.
//!
//! Dois eixos, porque falham por causas diferentes: **edições** (retenção por
//! versão, o modo de falha do analyzer) e **documentos** (o piso cresce com o
//! número de abertos, e tem de voltar ao valor inicial quando todos fecham).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::DocumentStore;

/// Texto de largura fixa por documento e revisão: a revisão ocupa sempre três
/// dígitos para que o texto retido tenha o mesmo tamanho em toda versão e o
/// platô não esconda variação de conteúdo atrás de tolerância.
fn text(document: usize, revision: usize) -> String {
    format!(
        "int f{document}(int n) {{ var soma = {:03}; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\nvoid main() {{ print(f{document}(3)); }}\n",
        revision % 1000
    )
}

fn uri(document: usize) -> String {
    format!("file:///plato/doc{document}.dart")
}

/// Abre K documentos e aplica K rodadas de edição a cada um, diagnosticando
/// o texto vigente como um editor faria; devolve os bytes vivos após cada
/// rodada. O diagnóstico é transitório e cai no fim da iteração.
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

/// `live_bytes` estabiliza ao longo das rodadas: o máximo da segunda metade
/// não supera o máximo do platô de referência além da tolerância.
///
/// A tolerância (8 KiB) cobre o ruído do harness e do `HashMap`. Reter uma
/// versão por edição custaria o tamanho do texto vezes K×K edições — com
/// K = 24 são 576 textos de ~120 bytes, ~69 KiB, bem acima da tolerância.
fn k_documents_edited_k_times_settle_on_plateau() {
    const K: usize = 24;
    let mut docs = DocumentStore::new();
    let live = edit(&mut docs, K);
    assert_eq!(live.len(), K);
    let reference = live[4..8].iter().max().copied().unwrap();
    let after = live[8..].iter().max().copied().unwrap();
    assert!(
        after <= reference.saturating_add(8 * 1024),
        "memória cresceu com as edições: platô {reference} bytes, cauda {after} bytes"
    );
    assert_eq!(docs.len(), K, "cada documento custa exatamente uma entrada");
}

/// Fechar todos os documentos devolve a memória ao nível de antes de abrir:
/// nada do texto, das versões ou dos diagnósticos sobrevive ao `close`.
fn closing_all_returns_to_initial_level() {
    const K: usize = 16;
    let mut docs = DocumentStore::new();
    let before = dartforge_instrument::live_bytes();
    let _ = edit(&mut docs, K);
    let open = dartforge_instrument::live_bytes();
    assert!(
        open > before + K * 100,
        "K documentos abertos devem custar ao menos o texto deles: {before} → {open}"
    );
    for document in 0..K {
        assert!(docs.close(&uri(document)));
    }
    assert!(docs.is_empty());
    let after = dartforge_instrument::live_bytes();
    // O `HashMap` vazio conserva a capacidade alocada: para K entradas o
    // hashbrown reserva a próxima potência de dois acima de K/0,875 buckets.
    // Cada entrada agora custa texto + tabela de linhas + versão (o servidor
    // anuncia sincronização incremental e precisa da tabela por documento),
    // ~80 bytes por bucket contra ~56 da versão só-texto: 32 buckets dão
    // ~2,5 KiB com K = 16, orçados em K*192. O que não pode sobrar é o texto
    // dos documentos, que domina o custo e é medido pela afirmação final
    // após o `drop`.
    assert!(
        after <= before.saturating_add(K * 192),
        "fechar não liberou os documentos: antes {before}, depois {after}"
    );
    drop(docs);
    assert!(dartforge_instrument::live_bytes() <= before.saturating_add(64));
}

/// Um único teste com as duas fases em sequência: o alocador contador é
/// global ao processo, e o harness roda funções `#[test]` em threads
/// paralelas e libera as estruturas de um teste terminado (captura de saída,
/// resultado) em momento não determinístico. Com dois `#[test]`, o segundo
/// media essas liberações como se fossem suas — um `Mutex` entre eles não
/// resolve, porque a liberação acontece fora da região travada.
#[test]
fn documents_plateau() {
    k_documents_edited_k_times_settle_on_plateau();
    closing_all_returns_to_initial_level();
}
