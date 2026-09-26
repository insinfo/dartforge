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
/// `DARTFORGE_CACHE_NATIVO`, senão `$CARGO_TARGET_DIR/native_cache`, senão
/// `target/native_cache` do repositório que compilou este binário.
pub fn dir_cache_nativo() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_CACHE_NATIVO") {
        return PathBuf::from(d);
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

/// Onde a distribuição guarda as bibliotecas do dartforge: `lib/` ao lado do
/// `bin/` do executável (`<raiz>/bin/dartforge`, `<raiz>/lib/…`), ou
/// `DARTFORGE_LIB`.
pub fn dir_lib_da_distribuicao() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_LIB") {
        return Some(PathBuf::from(d));
    }
    let exe = std::env::current_exe().ok()?;
    let raiz = exe.parent()?.parent()?;
    let lib = raiz.join("lib");
    lib.is_dir().then_some(lib)
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
