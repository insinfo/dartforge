//! O runtime nativo do AOT e o diretório dos caches do backend.
//!
//! O runtime é a `staticlib` de `crates/runtime_estatico`, compilada pelo
//! `build.rs` deste crate com o `cargo` do build (duas variantes: a do
//! executável, com o `main` C, e a da biblioteca compartilhada do SDK da
//! fonte). Vem pronta com o dartforge: em `lib/` da distribuição, ou no
//! diretório do build numa árvore de desenvolvimento. Quem usa o dartforge
//! não compila Rust. O nome leva o hash do fonte do runtime e do
//! `Cargo.lock`: só o runtime deste compilador casa.

use std::path::{Path, PathBuf};

/// Diretório dos caches do backend nativo (objetos, SDK da fonte):
/// `DARTFORGE_CACHE_NATIVO`; na distribuição, o cache do usuário
/// (`~/.cache/dartforge/nativo`…); numa árvore de desenvolvimento,
/// `$CARGO_TARGET_DIR/native_cache` ou `target/native_cache` do repositório
/// que compilou este binário.
pub fn dir_cache_nativo() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_CACHE_NATIVO") {
        return PathBuf::from(d);
    }
    if dartforge_elements::distribuicao::raiz().is_some()
        && let Some(c) = dartforge_elements::distribuicao::cache_do_usuario()
    {
        return c.join("nativo");
    }
    if let Some(t) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(t).join("native_cache");
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/native_cache")
}

pub struct RuntimeCache {
    pub lib_path: PathBuf,
}

impl RuntimeCache {
    /// O runtime do executável (com o `main` C).
    pub fn get_or_compile() -> Result<Self, String> {
        runtime_precompilado(env!("DARTFORGE_RUNTIME_PRINCIPAL")).map(|lib_path| Self { lib_path })
    }

    /// A variante do runtime que vai para a DLL do SDK da fonte (P5c): sem o
    /// `main` C (cfg `dartforge_runtime_dll`); o `main` é o do executável.
    pub fn para_dll() -> Result<Self, String> {
        runtime_precompilado(env!("DARTFORGE_RUNTIME_DLL")).map(|lib_path| Self { lib_path })
    }
}

/// Onde a distribuição guarda o runtime pré-compilado: `lib/runtime/`
/// (`dartforge_elements::distribuicao`), ou `DARTFORGE_LIB`.
pub fn dir_lib_da_distribuicao() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_LIB") {
        return Some(PathBuf::from(d));
    }
    dartforge_elements::distribuicao::em_lib("runtime")
}

/// O runtime pré-compilado `nome`: na distribuição, senão no diretório do
/// build.
fn runtime_precompilado(nome: &str) -> Result<PathBuf, String> {
    let candidatos: Vec<PathBuf> = [dir_lib_da_distribuicao(), Some(PathBuf::from(env!("DARTFORGE_RUNTIME_DIR_DO_BUILD")))]
        .into_iter()
        .flatten()
        .collect();
    candidatos.iter().map(|d| d.join(nome)).find(|p| p.is_file()).ok_or_else(|| {
        let onde: Vec<String> = candidatos.iter().map(|d| d.display().to_string()).collect();
        format!(
            "o runtime {nome} não está na distribuição (procurado em {}); este dartforge foi compilado com DARTFORGE_RUNTIME_SEM_PRECOMPILAR=1?",
            onde.join(", ")
        )
    })
}
