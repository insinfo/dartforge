//! A fonte única do runtime: o que o AOT compila e o que o JIT usa são o mesmo
//! código, com a mesma semântica.

/// O texto que o AOT compila contém, sem alteração, os arquivos que este
/// crate compila como `heap` e `abi`: o `heap.rs` e, em seguida, todos os
/// fragmentos concatenados na ordem de `FRAGMENTOS`.
#[test]
fn o_aot_compila_os_mesmos_arquivos() {
    let texto = dartforge_runtime::RUNTIME_MAIN;
    assert!(texto.contains(include_str!("../src/heap.rs")));
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut concatenado = String::new();
    for f in dartforge_runtime::simbolos::FRAGMENTOS {
        concatenado.push_str(
            &std::fs::read_to_string(src.join(format!("{f}.rs"))).expect("ler fragmento"),
        );
    }
    assert!(texto.ends_with(&concatenado));
}

/// A tabela gerada tem nomes únicos, endereços reais e não tem o `main` C.
#[test]
fn a_tabela_cobre_os_simbolos_do_runtime() {
    let tabela = dartforge_runtime::simbolos::tabela();
    assert_eq!(tabela.len(), dartforge_runtime::simbolos::NOMES.len());
    assert!(tabela.len() > 100, "{}", tabela.len());
    let mut nomes: Vec<&str> = tabela.iter().map(|(n, _)| *n).collect();
    nomes.sort_unstable();
    nomes.dedup();
    assert_eq!(nomes.len(), tabela.len());
    assert!(
        tabela
            .iter()
            .all(|(n, a)| n.starts_with("dartforge_") && *a != 0)
    );
    assert!(!nomes.contains(&"main"));
    // Cada nome anunciado aparece no texto que o AOT compila.
    for nome in &nomes {
        assert!(
            dartforge_runtime::RUNTIME_MAIN.contains(&format!("fn {nome}(")),
            "{nome} não está no RUNTIME_MAIN"
        );
    }
}

/// @Native resolve todo o catálogo reservado na instância que atende a chamada,
/// mesmo quando o executável não exporta suas funções para o lookup do processo.
#[test]
#[allow(unsafe_code)] // A prova chama a ABI cujo contrato exige bytes válidos.
fn o_resolver_nativo_e_seu_cache_conservam_os_enderecos_deste_runtime() {
    for (nome, esperado) in dartforge_runtime::simbolos::tabela() {
        for _ in 0..2 {
            // SAFETY: nome é uma string estática UTF-8 e vive durante a chamada.
            let endereco = unsafe {
                dartforge_runtime::abi::dartforge_ffi_simbolo_nativo(
                    nome.as_ptr(),
                    nome.len() as i64,
                )
            };
            assert_eq!(endereco as usize, esperado, "{nome}");
        }
    }
}

/// O perfil deste pacote é o do AOT (`rustc -O`): estouro de inteiro é
/// modular, não `panic`. A sobreposição `[profile.dev.package.dartforge-runtime]`
/// do `Cargo.toml` da raiz vale para todos os alvos do pacote, inclusive este
/// teste; sem ela, o perfil `test` (que herda de `dev`) entraria em pânico aqui,
/// e o runtime do JIT divergiria do AOT em toda conta que estoura.
#[test]
fn estouro_e_modular_como_no_aot() {
    let r = std::panic::catch_unwind(|| std::hint::black_box(i64::MAX) + std::hint::black_box(1));
    assert_eq!(r.ok(), Some(i64::MIN));
}
