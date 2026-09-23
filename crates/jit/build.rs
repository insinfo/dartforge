//! Liga o crate à biblioteca **compartilhada** da API C do LLVM e gera o
//! runtime embutido **provisório**.
//!
//! # Ligação com o LLVM
//!
//! `llvm-sys` entra com a feature `no-llvm-linking`: dele aproveitamos as
//! assinaturas `extern "C"` e os invólucros de `LLVMInitializeNative*`, mas a
//! ligação é feita aqui, contra `LLVM-C`.
//!
//! A distribuição oficial `clang+llvm-22.1.8-x86_64-pc-windows-msvc` traz as
//! bibliotecas estáticas compiladas com a CRT **estática** (`libcmt`), enquanto
//! o Rust usa a CRT dinâmica (`msvcrt`). Ligar as duas produz
//! `LINK : warning LNK4098: defaultlib 'libcmt.lib' conflita` e um binário com
//! **dois heaps**. Isso não é teórico: com a ligação estática, uma mensagem de
//! erro devolvida por `LLVMParseIRInContext2` é lida corretamente e derruba o
//! processo com `STATUS_ACCESS_VIOLATION` no `LLVMDisposeMessage` seguinte —
//! alocada por uma CRT, liberada pela outra. O mesmo pacote também não permite
//! `llvm-config --link-shared`, que procura um `LLVM-22.dll` inexistente.
//!
//! Com `LLVM-C.dll`, alocação e liberação acontecem as duas dentro da DLL, com
//! a CRT dela. O preço é uma dependência de execução: a DLL precisa estar
//! alcançável pelo carregador. Veja `docs/JIT.md`.
//!
//! # Runtime embutido provisório
//!
//! O código JIT chama os símbolos `dartforge_*` do runtime Rust. A fonte desses
//! símbolos é **uma só**: `crates/runtime/src/runtime_main.rs`, a mesma que o
//! AOT compila com `rustc` avulso (`crates/emit_native/src/cache.rs`). Hoje esse
//! arquivo não é módulo do crate `dartforge-runtime` — ele só existe como texto
//! em `RUNTIME_MAIN` —, e a mudança que o torna módulo (plano do JIT, §3.1)
//! espera o merge do trabalho em curso no runtime.
//!
//! Até lá, este script:
//!
//! 1. lê `runtime_main.rs` **sem alterá-lo** e grava em `OUT_DIR` uma cópia
//!    com três substituições mecânicas: o `main` C do harness vira
//!    `dartforge_jit_main_provisorio` (senão colidiria com o `main` de qualquer
//!    binário Rust), e a entrada `dartforge_entry` que ele chama vira
//!    `dartforge_jit_entrada_provisoria`, definida em `src/ffi.rs`;
//! 2. gera a tabela `(nome, endereço)` a partir dos `#[unsafe(no_mangle)]` do
//!    mesmo arquivo — nenhum nome é escrito à mão.
//!
//! Cada substituição exige **exatamente uma** ocorrência; se o runtime mudar de
//! forma, o build falha dizendo o quê, em vez de gerar um runtime diferente em
//! silêncio. Quando a fonte única entrar no `crates/runtime`, este bloco some e
//! a tabela passa a vir de `dartforge_runtime::simbolos`.
use std::path::{Path, PathBuf};

/// Emite as diretivas de ligação, gera o runtime provisório e as dependências
/// de reexecução do script.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_DIR");
    println!("cargo::rerun-if-env-changed=LLVM_SYS_221_PREFIX");

    let prefix = prefix();
    let libdir = prefix.join("lib");
    if !libdir.is_dir() {
        println!(
            "cargo::warning=diretório de bibliotecas do LLVM não encontrado: {}",
            libdir.display()
        );
    }
    println!("cargo::rustc-link-search=native={}", libdir.display());
    println!("cargo::rustc-link-lib=dylib={}", shared_library_name());

    gerar_runtime_provisorio();
}

/// Prefixo da distribuição completa do LLVM 22.1.x.
///
/// `DARTFORGE_LLVM_DIR` é o nome preferido do projeto, no mesmo espírito de
/// `DARTFORGE_CLANG` em `crates/native`. `LLVM_SYS_221_PREFIX` é aceito porque é
/// o nome que o `build.rs` do `llvm-sys` já exige, e manter as duas apontando
/// para lugares diferentes só produziria confusão.
fn prefix() -> PathBuf {
    for variable in ["DARTFORGE_LLVM_DIR", "LLVM_SYS_221_PREFIX"] {
        if let Some(value) = std::env::var_os(variable) {
            let path = PathBuf::from(value);
            if path.as_os_str().is_empty() {
                continue;
            }
            return path;
        }
    }
    PathBuf::from(FALLBACK_PREFIX)
}

/// Instalação verificada nesta máquina, usada quando nada foi configurado.
const FALLBACK_PREFIX: &str = if cfg!(windows) {
    "D:/DartSDKs/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
} else {
    "/usr/lib/llvm-22"
};

/// Nome da biblioteca compartilhada da API C, tal como cada pacote a publica.
///
/// Windows publica `bin/LLVM-C.dll` com a import library `lib/LLVM-C.lib`. Os
/// pacotes Unix publicam a biblioteca completa como `libLLVM-22.so`/`.dylib`, e
/// a API C está dentro dela — não há `libLLVM-C` separada.
fn shared_library_name() -> &'static str {
    if Path::new(&prefix()).join("lib/LLVM-C.lib").is_file() || cfg!(windows) {
        "LLVM-C"
    } else {
        "LLVM-22"
    }
}

/// Substituições aplicadas à cópia do harness, cada uma exigida uma única vez.
///
/// São as três linhas que amarram `runtime_main.rs` a um executável AOT: a
/// declaração da entrada emitida, a chamada a ela e o `main` C.
const SUBSTITUICOES: &[(&str, &str)] = &[
    ("fn dartforge_entry();", "fn dartforge_jit_entrada_provisoria();"),
    (
        "unsafe { dartforge_entry() };",
        "unsafe { dartforge_jit_entrada_provisoria() };",
    ),
    (
        "pub extern \"C\" fn main() -> i32 {",
        "pub extern \"C\" fn dartforge_jit_main_provisorio() -> i32 {",
    ),
];

/// Gera `runtime_provisorio.rs` e `simbolos_provisorios.rs` em `OUT_DIR`.
fn gerar_runtime_provisorio() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let fonte = manifest.join("../runtime/src/runtime_main.rs");
    println!("cargo::rerun-if-changed={}", fonte.display());
    let texto = std::fs::read_to_string(&fonte)
        .unwrap_or_else(|erro| panic!("não foi possível ler {}: {erro}", fonte.display()));

    let mut copia = texto.clone();
    for (antes, depois) in SUBSTITUICOES {
        let ocorrencias = copia.matches(antes).count();
        assert!(
            ocorrencias == 1,
            "runtime_main.rs mudou de forma: `{antes}` aparece {ocorrencias} vez(es), e o runtime \
             embutido provisório do JIT exige exatamente uma. Ajuste SUBSTITUICOES em \
             crates/jit/build.rs — ou, se a fonte única já entrou em crates/runtime, remova este \
             bloco (plano do JIT, §3.1)."
        );
        copia = copia.replacen(antes, depois, 1);
    }

    let nomes = nomes_exportados(&texto);
    let mut ordenados = nomes.clone();
    ordenados.sort_unstable();
    ordenados.dedup();
    assert!(
        ordenados.len() == nomes.len(),
        "runtime_main.rs define algum símbolo #[unsafe(no_mangle)] duas vezes"
    );

    let saida = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let cabecalho = format!(
        "// GERADO por crates/jit/build.rs a partir de {} — não editar.\n\
         // PROVISÓRIO até a fonte única do runtime (plano do JIT, §3.1).\n",
        fonte.display()
    );
    std::fs::write(
        saida.join("runtime_provisorio.rs"),
        format!("{cabecalho}use dartforge_runtime::heap;\n{copia}"),
    )
    .expect("gravar runtime_provisorio.rs");

    let mut tabela = cabecalho.clone();
    tabela.push_str("/// Nomes do runtime publicados na sessão, na ordem da fonte.\n");
    tabela.push_str("pub(crate) const RUNTIME_SYMBOLS: &[&str] = &[\n");
    for nome in &nomes {
        tabela.push_str(&format!("    \"{nome}\",\n"));
    }
    tabela.push_str("];\n\n");
    tabela.push_str("/// Endereço de cada função do runtime, na mesma ordem de [`RUNTIME_SYMBOLS`].\n");
    tabela.push_str("fn runtime_symbol_addresses() -> Vec<(&'static str, usize)> {\n    vec![\n");
    for nome in &nomes {
        tabela.push_str(&format!(
            "        (\"{nome}\", runtime_provisorio::{nome} as *const () as usize),\n"
        ));
    }
    tabela.push_str("    ]\n}\n");
    std::fs::write(saida.join("simbolos_provisorios.rs"), tabela).expect("gravar simbolos_provisorios.rs");
}

/// Nomes das funções `#[unsafe(no_mangle)]` do harness, exceto o `main` C.
///
/// A forma reconhecida é a única que o arquivo usa: o atributo numa linha e,
/// na linha seguinte, `pub [unsafe] extern "C" fn nome(`. Um atributo seguido
/// de outra coisa derruba o build, porque significaria um símbolo que a tabela
/// não saberia publicar.
fn nomes_exportados(texto: &str) -> Vec<String> {
    let mut nomes = Vec::new();
    let mut linhas = texto.lines();
    while let Some(linha) = linhas.next() {
        if linha.trim() != "#[unsafe(no_mangle)]" {
            continue;
        }
        let seguinte = linhas.next().unwrap_or("").trim();
        let assinatura = seguinte
            .strip_prefix("pub unsafe extern \"C\" fn ")
            .or_else(|| seguinte.strip_prefix("pub extern \"C\" fn "))
            .unwrap_or_else(|| {
                panic!("#[unsafe(no_mangle)] seguido de forma não reconhecida em runtime_main.rs: `{seguinte}`")
            });
        let nome: String = assinatura
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if nome != "main" {
            nomes.push(nome);
        }
    }
    nomes
}
