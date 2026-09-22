//! Platô de memória do servidor: K documentos abertos, cada um editado K vezes.
//!
//! É o teste de platô do compilador (`crates/compiler/tests/plato_memoria.rs`)
//! aplicado ao dono dos documentos do LSP, e responde à falha observada no
//! LSP do Dart: memória que só sobe a cada edição até ser preciso matar o
//! processo. O `DocumentStore` promete que a chegada da versão N descarta a
//! N−1 no lugar e que fechar remove tudo; o alocador contador é o que
//! transforma a promessa em afirmação.
//!
//! Dois eixos, porque falham por causas diferentes: **edições** (retenção por
//! versão, o modo de falha do analyzer) e **documentos** (o piso cresce com o
//! número de abertos, e tem de voltar ao valor inicial quando todos fecham).
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::DocumentStore;

/// Texto de largura fixa por documento e revisão: a revisão ocupa sempre três
/// dígitos para que o texto retido tenha o mesmo tamanho em toda versão e o
/// platô não esconda variação de conteúdo atrás de tolerância.
fn texto(documento: usize, revisao: usize) -> String {
    format!(
        "int f{documento}(int n) {{ var soma = {:03}; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\nvoid main() {{ print(f{documento}(3)); }}\n",
        revisao % 1000
    )
}

fn uri(documento: usize) -> String {
    format!("file:///plato/doc{documento}.dart")
}

/// Abre K documentos e aplica K rodadas de edição a cada um, diagnosticando
/// o texto vigente como um editor faria; devolve os bytes vivos após cada
/// rodada. O diagnóstico é transitório e cai no fim da iteração.
fn editar(docs: &mut DocumentStore, k: usize) -> Vec<usize> {
    for documento in 0..k {
        docs.open(uri(documento), 1, texto(documento, 0));
    }
    let mut vivos = Vec::with_capacity(k);
    for rodada in 1..=k {
        for documento in 0..k {
            let alvo = uri(documento);
            assert!(docs.update(&alvo, rodada as i32 + 1, texto(documento, rodada)));
            let diagnosticos = docs.diagnose_open(&alvo);
            assert!(diagnosticos.is_empty(), "{alvo}: {diagnosticos:?}");
        }
        vivos.push(dartforge_instrument::live_bytes());
    }
    vivos
}

/// `live_bytes` estabiliza ao longo das rodadas: o máximo da segunda metade
/// não supera o máximo do platô de referência além da tolerância.
///
/// A tolerância (8 KiB) cobre o ruído do harness e do `HashMap`. Reter uma
/// versão por edição custaria o tamanho do texto vezes K×K edições — com
/// K = 24 são 576 textos de ~120 bytes, ~69 KiB, bem acima da tolerância.
fn k_documentos_editados_k_vezes_estabilizam_em_plato() {
    const K: usize = 24;
    let mut docs = DocumentStore::new();
    let vivos = editar(&mut docs, K);
    assert_eq!(vivos.len(), K);
    let referencia = vivos[4..8].iter().max().copied().unwrap();
    let depois = vivos[8..].iter().max().copied().unwrap();
    assert!(
        depois <= referencia.saturating_add(8 * 1024),
        "memória cresceu com as edições: platô {referencia} bytes, cauda {depois} bytes"
    );
    assert_eq!(docs.len(), K, "cada documento custa exatamente uma entrada");
}

/// Fechar todos os documentos devolve a memória ao nível de antes de abrir:
/// nada do texto, das versões ou dos diagnósticos sobrevive ao `close`.
fn fechar_todos_devolve_ao_nivel_inicial() {
    const K: usize = 16;
    let mut docs = DocumentStore::new();
    let antes = dartforge_instrument::live_bytes();
    let _ = editar(&mut docs, K);
    let aberto = dartforge_instrument::live_bytes();
    assert!(
        aberto > antes + K * 100,
        "K documentos abertos devem custar ao menos o texto deles: {antes} → {aberto}"
    );
    for documento in 0..K {
        assert!(docs.close(&uri(documento)));
    }
    assert!(docs.is_empty());
    let depois = dartforge_instrument::live_bytes();
    // O `HashMap` vazio conserva a capacidade alocada: para K entradas o
    // hashbrown reserva a próxima potência de dois acima de K/0,875 buckets
    // de 56 bytes (chave `String` + entrada), 1.792 bytes com K = 16. O que
    // não pode sobrar é o texto dos documentos, que domina o custo e é medido
    // pela afirmação final após o `drop`.
    assert!(
        depois <= antes.saturating_add(K * 128),
        "fechar não liberou os documentos: antes {antes}, depois {depois}"
    );
    drop(docs);
    assert!(dartforge_instrument::live_bytes() <= antes.saturating_add(64));
}

/// Um único teste com as duas fases em sequência: o alocador contador é
/// global ao processo, e o harness roda funções `#[test]` em threads
/// paralelas e libera as estruturas de um teste terminado (captura de saída,
/// resultado) em momento não determinístico. Com dois `#[test]`, o segundo
/// media essas liberações como se fossem suas — um `Mutex` entre eles não
/// resolve, porque a liberação acontece fora da região travada.
#[test]
fn plato_de_documentos() {
    k_documentos_editados_k_vezes_estabilizam_em_plato();
    fechar_todos_devolve_ao_nivel_inicial();
}
