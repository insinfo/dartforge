//! A fonte única do runtime: o que o AOT compila e o que o JIT usa são o mesmo
//! código, com a mesma semântica.

/// O texto que o AOT compila contém, sem alteração, os arquivos que este
/// crate compila como `heap` e `abi`: o `heap.rs` e, em seguida, todos os
/// fragmentos concatenados na ordem de `FRAGMENTOS`.
#[test]
fn aot_compiles_the_same_files() {
    let text = dartforge_runtime::RUNTIME_MAIN;
    assert!(text.contains(include_str!("../src/heap.rs")));
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut concatenated = String::new();
    for f in dartforge_runtime::simbolos::FRAGMENTOS {
        concatenated.push_str(
            &std::fs::read_to_string(src.join(format!("{f}.rs"))).expect("ler fragmento"),
        );
    }
    assert!(text.ends_with(&concatenated));
}

/// A tabela gerada tem nomes únicos, endereços reais e não tem o `main` C.
#[test]
fn the_table_covers_runtime_symbols() {
    let tabela = dartforge_runtime::simbolos::tabela();
    assert_eq!(tabela.len(), dartforge_runtime::simbolos::NOMES.len());
    assert!(tabela.len() > 100, "{}", tabela.len());
    let mut names: Vec<&str> = tabela.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), tabela.len());
    assert!(
        tabela
            .iter()
            .all(|(n, a)| n.starts_with("dartforge_") && *a != 0)
    );
    assert!(!names.contains(&"main"));
    // Cada nome anunciado aparece no texto que o AOT compila.
    for name in &names {
        assert!(
            dartforge_runtime::RUNTIME_MAIN.contains(&format!("fn {name}(")),
            "{name} não está no RUNTIME_MAIN"
        );
    }
}

/// @Native resolve todo o catálogo reservado na instância que atende a chamada,
/// mesmo quando o executável não exporta suas funções para o lookup do processo.
#[test]
#[allow(unsafe_code)] // A prova chama a ABI cujo contrato exige bytes válidos.
fn native_resolver_and_its_cache_keep_this_runtimes_addresses() {
    for (name, expected) in dartforge_runtime::simbolos::tabela() {
        for _ in 0..2 {
            // SAFETY: nome é uma string estática UTF-8 e vive durante a chamada.
            let address = unsafe {
                dartforge_runtime::abi::dartforge_ffi_simbolo_nativo(
                    name.as_ptr(),
                    name.len() as i64,
                )
            };
            assert_eq!(address as usize, expected, "{name}");
        }
    }
}

/// O perfil deste pacote é o do AOT (`rustc -O`): estouro de inteiro é
/// modular, não `panic`. A sobreposição `[profile.dev.package.dartforge-runtime]`
/// do `Cargo.toml` da raiz vale para todos os alvos do pacote, inclusive este
/// teste; sem ela, o perfil `test` (que herda de `dev`) entraria em pânico aqui,
/// e o runtime do JIT divergiria do AOT em toda conta que estoura.
#[test]
fn overflow_wraps_as_in_aot() {
    let r = std::panic::catch_unwind(|| std::hint::black_box(i64::MAX) + std::hint::black_box(1));
    assert_eq!(r.ok(), Some(i64::MIN));
}
