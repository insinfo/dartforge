//! A fonte única do runtime nativo, compilada também como módulo deste crate.
//!
//! O runtime que o código gerado chama é a concatenação, na ordem de
//! [`FRAGMENTOS`], dos arquivos `src/<fragmento>.rs` (um por tema: núcleo,
//! raízes do coletor, exceções, saída, strings, coleções, closures). Os
//! fragmentos não são módulos Rust: são pedaços de UM programa, que se enxergam
//! sem `use`. A lista é uma só, e é esta; o texto é compilado de duas formas:
//!
//! * pelo AOT, como texto (`RUNTIME_MAIN`), com `rustc` avulso e `-O`
//!   (`crates/emit_native/src/cache.rs`), virando a `.lib` que o executável
//!   liga — com o `main` C que chama `@dartforge_entry`;
//! * por este crate, como o módulo `abi`, para o JIT (`crates/jit`), que
//!   publica os endereços das funções como símbolos absolutos — **sem** o
//!   `main` C, que colidiria com o `main` de qualquer binário Rust.
//!
//! Este script:
//!
//! 1. grava em `OUT_DIR` o texto concatenado (`runtime_main.rs`, que o
//!    `RUNTIME_MAIN` inclui) e o corpo do módulo `abi` (`abi.rs`: um
//!    `include!` por fragmento, na mesma ordem, para os erros de compilação
//!    apontarem o arquivo de verdade);
//! 2. liga a cfg `dartforge_runtime_embutido`, que tira o `main` C e a
//!    declaração de `dartforge_entry` (o `rustc` avulso do AOT não a recebe);
//! 3. gera `simbolos.rs`: a tabela `(nome, endereço)` de todo
//!    `#[unsafe(no_mangle)] pub [unsafe] extern "C" fn` de **todos** os
//!    fragmentos, exceto o `main`. Nenhum nome é escrito à mão; tomar o
//!    endereço também impede o linker de descartar as funções do rlib.
//!
//! Um arquivo novo em `src/` que não seja `lib.rs`, `heap.rs` nem fragmento
//! listado derruba o build: um fragmento esquecido fora da lista seria código
//! do runtime que nenhum dos dois perfis compila.
use std::path::PathBuf;

/// Os fragmentos do runtime, na ordem de concatenação. Acrescentar um
/// fragmento é acrescentar o nome aqui (e o arquivo em `src/`).
const FRAGMENTOS: &[&str] = &[
    "nucleo",
    "gc_raizes",
    "excecoes",
    "saida",
    "strings",
    "colecoes",
    "closures",
    // δ (P5b): os natives do SDK da fonte (crates/emit_native/src/nativos.rs).
    "nativos_numeros",
    "nativos_strings",
    // Relógio, fuso horário e entropia.
    "nativos_sistema",
    // O motor de expressões regulares do `RegExp`.
    "regexp",
    // P2 (α): operadores sobre dynamic/num (tapa-buraco até P5).
    "despacho",
    // δ (P5c): tabelas de métodos e busca por seletor do SDK da fonte.
    "seletores",
    "nativos_listas",
    // As listas tipadas do `typed_data_patch.dart` da VM.
    "typed_data",
    // Os tipos SIMD (`Float32x4`, `Int32x4`, `Float64x2`).
    "simd",
    // P6: o laço de eventos (microtarefas e timers).
    "eventos",
    // Portas e a fila de mensagens do isolado.
    "portas",
    // Os isolados (`Isolate.spawn`, a porta de controle).
    "isolados",
    // O dart:ffi (bibliotecas dinâmicas, memória nativa, trampolins de chamada).
    "ffi",
    // O `dart:io` da VM: arquivos, diretórios, o IOService e a plataforma.
    "io_arquivos",
    "io_diretorios",
    "io_servico",
    "io_plataforma",
    // O manipulador de eventos, os soquetes e os processos.
    "io_eventos",
    "io_soquetes",
    "io_soquetes_unix",
    "io_processos",
    // O mesmo no Windows: a porta de conclusão, o Winsock e os processos.
    "io_windows_eventos",
    "io_windows_soquetes",
    "io_windows_processos",
    // A TLS do `dart:io` (`SecureSocket`, `SecurityContext`), sobre o rustls.
    "tls",
    // `dart:developer` e a timeline no perfil de produção.
    "nativos_desenvolvedor",
    // RTI: tipos em tempo de execução.
    "tipos",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_dll)");
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_embutido)");
    // As features escolhem o perfil do módulo `abi`: sem nenhuma, o JIT
    // (sem o `main` C); `aot`, a `staticlib` do executável (com o `main`);
    // `dll`, a da biblioteca compartilhada do SDK da fonte (sem o `main`).
    // As duas `staticlib` saem de `crates/runtime_estatico`.
    let aot = std::env::var_os("CARGO_FEATURE_AOT").is_some();
    let dll = std::env::var_os("CARGO_FEATURE_DLL").is_some();
    assert!(!(aot && dll), "as features aot e dll do runtime são exclusivas");
    if dll {
        println!("cargo::rustc-cfg=dartforge_runtime_dll");
    } else if !aot {
        println!("cargo::rustc-cfg=dartforge_runtime_embutido");
    }

    let manifesto =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifesto.join("src");
    conferir_lista(&src);

    let mut texto = String::new();
    let mut abi = String::from("// GERADO por crates/runtime/build.rs — não editar.\n");
    for f in FRAGMENTOS {
        let caminho = src.join(format!("{f}.rs"));
        let conteudo = std::fs::read_to_string(&caminho)
            .unwrap_or_else(|e| panic!("ler {}: {e}", caminho.display()));
        assert!(
            conteudo.ends_with('\n'),
            "{} não termina em fim de linha",
            caminho.display()
        );
        texto.push_str(&conteudo);
        let literal = format!("{:?}", caminho.to_str().expect("caminho UTF-8"));
        abi.push_str(&format!("include!({literal});\n"));
    }

    let nomes = nomes_exportados(&texto);
    let mut ordenados = nomes.clone();
    ordenados.sort_unstable();
    ordenados.dedup();
    assert!(
        ordenados.len() == nomes.len(),
        "o runtime define algum símbolo #[unsafe(no_mangle)] duas vezes"
    );

    let mut saida = String::from(
        "// GERADO por crates/runtime/build.rs a partir dos fragmentos de src/ — não editar.\n\n\
         /// Os fragmentos do runtime, na ordem em que são concatenados.\n\
         pub const FRAGMENTOS: &[&str] = &[\n",
    );
    for f in FRAGMENTOS {
        saida.push_str(&format!("    \"{f}\",\n"));
    }
    saida.push_str(
        "];\n\n\
         /// Nomes dos símbolos que o runtime exporta ao código gerado, na ordem da fonte.\n\
         pub const NOMES: &[&str] = &[\n",
    );
    for nome in &nomes {
        saida.push_str(&format!("    \"{nome}\",\n"));
    }
    saida.push_str(
        "];\n\n/// Endereço de cada função do runtime neste processo, na ordem de [`NOMES`].\n\
         pub fn tabela() -> Vec<(&'static str, usize)> {\n    vec![\n",
    );
    for nome in &nomes {
        saida.push_str(&format!(
            "        (\"{nome}\", crate::abi::{nome} as *const () as usize),\n"
        ));
    }
    saida.push_str("    ]\n}\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("simbolos.rs"), saida).expect("gravar simbolos.rs");
    std::fs::write(out.join("runtime_main.rs"), texto).expect("gravar runtime_main.rs");
    std::fs::write(out.join("abi.rs"), abi).expect("gravar abi.rs");
}

/// Todo `src/*.rs` é `lib.rs`, `heap.rs` ou um fragmento listado.
fn conferir_lista(src: &std::path::Path) {
    let entradas = std::fs::read_dir(src).unwrap_or_else(|e| panic!("ler {}: {e}", src.display()));
    for entrada in entradas {
        let caminho = entrada.expect("entrada de src/").path();
        if caminho.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let nome = caminho.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            nome == "lib" || nome == "heap" || FRAGMENTOS.contains(&nome),
            "{} não é fragmento do runtime: acrescente \"{nome}\" a FRAGMENTOS em crates/runtime/build.rs",
            caminho.display()
        );
    }
}

/// Nomes das funções `#[unsafe(no_mangle)]` do runtime, exceto o `main` C.
///
/// A forma reconhecida é a que os fragmentos usam: o atributo numa linha e, na
/// seguinte, `pub [unsafe] extern "C" fn nome(`. Entre as duas pode haver
/// outros atributos (`#[cfg(...)]`). Qualquer outra forma derruba o build,
/// porque seria um símbolo que a tabela não saberia publicar.
fn nomes_exportados(texto: &str) -> Vec<String> {
    let mut nomes = Vec::new();
    let mut linhas = texto.lines();
    while let Some(linha) = linhas.next() {
        if linha.trim() != "#[unsafe(no_mangle)]" {
            continue;
        }
        let seguinte = loop {
            match linhas.next() {
                Some(l) if l.trim().starts_with("#[") => continue,
                Some(l) => break l.trim(),
                None => break "",
            }
        };
        let assinatura = seguinte
            .strip_prefix("pub unsafe extern \"C\" fn ")
            .or_else(|| seguinte.strip_prefix("pub extern \"C\" fn "))
            .unwrap_or_else(|| {
                panic!(
                    "#[unsafe(no_mangle)] seguido de forma não reconhecida no runtime: `{seguinte}`"
                )
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
