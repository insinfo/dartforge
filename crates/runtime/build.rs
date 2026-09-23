//! A fonte única do runtime nativo, compilada também como módulo deste crate.
//!
//! `src/runtime_main.rs` é compilado de duas formas:
//!
//! * pelo AOT, como texto (`RUNTIME_MAIN`), com `rustc` avulso e `-O`
//!   (`crates/emit_native/src/cache.rs`), virando a `.lib` que o executável
//!   liga — com o `main` C que chama `@dartforge_entry`;
//! * por este crate, como o módulo `abi`, para o JIT (`crates/jit`), que
//!   publica os endereços das funções como símbolos absolutos — **sem** o
//!   `main` C, que colidiria com o `main` de qualquer binário Rust.
//!
//! Este script faz as duas coisas que a segunda forma precisa:
//!
//! 1. liga a cfg `dartforge_runtime_embutido`, que tira o `main` C e a
//!    declaração de `dartforge_entry` (o `rustc` avulso do AOT não a recebe);
//! 2. gera `simbolos.rs`: a tabela `(nome, endereço)` de todo
//!    `#[unsafe(no_mangle)] pub [unsafe] extern "C" fn` do arquivo, exceto o
//!    `main`. Nenhum nome é escrito à mão; tomar o endereço também impede o
//!    linker de descartar as funções do rlib.
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/runtime_main.rs");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_embutido)");
    println!("cargo::rustc-cfg=dartforge_runtime_embutido");

    let texto = std::fs::read_to_string("src/runtime_main.rs").expect("ler src/runtime_main.rs");
    let nomes = nomes_exportados(&texto);
    let mut ordenados = nomes.clone();
    ordenados.sort_unstable();
    ordenados.dedup();
    assert!(ordenados.len() == nomes.len(), "runtime_main.rs define algum símbolo #[unsafe(no_mangle)] duas vezes");

    let mut saida = String::from(
        "// GERADO por crates/runtime/build.rs a partir de src/runtime_main.rs — não editar.\n\n\
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
        saida.push_str(&format!("        (\"{nome}\", crate::abi::{nome} as *const () as usize),\n"));
    }
    saida.push_str("    ]\n}\n");
    let destino = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("simbolos.rs");
    std::fs::write(destino, saida).expect("gravar simbolos.rs");
}

/// Nomes das funções `#[unsafe(no_mangle)]` do harness, exceto o `main` C.
///
/// A forma reconhecida é a que o arquivo usa: o atributo numa linha e, na
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
            .unwrap_or_else(|| panic!("#[unsafe(no_mangle)] seguido de forma não reconhecida em runtime_main.rs: `{seguinte}`"));
        let nome: String = assinatura.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if nome != "main" {
            nomes.push(nome);
        }
    }
    nomes
}
