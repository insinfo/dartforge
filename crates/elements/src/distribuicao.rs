//! O leiaute da distribuição do dartforge: tudo o que o compilador precisa
//! em tempo de execução, ao lado do próprio executável — quem usa o
//! dartforge não instala Rust, LLVM nem o SDK do Dart.
//!
//! ```text
//! <raiz>/bin/dartforge[.exe]
//! <raiz>/lib/dartforge-distribuicao.json   marca e versão da distribuição
//! <raiz>/lib/dart-sdk/lib/…                as bibliotecas do SDK do Dart (fonte)
//! <raiz>/lib/sdk_nativo/…                  a sobreposição do backend nativo
//! <raiz>/lib/runtime/…                     o runtime pré-compilado (staticlib)
//! <raiz>/lib/llvm/bin/…                    o ligador (e o Clang, enquanto for o driver da ligação)
//! ```
//!
//! Numa árvore de desenvolvimento não há marca, e cada consulta cai nas
//! variáveis de ambiente e no repositório (ver quem chama).

use std::path::{Path, PathBuf};

/// O nome do arquivo que marca a raiz de uma distribuição.
pub const MARCA: &str = "dartforge-distribuicao.json";

/// A raiz da distribuição do executável em execução: `DARTFORGE_HOME`, ou o
/// diretório acima do `bin/` do executável quando `lib/` tem a [`MARCA`].
/// `None` numa árvore de desenvolvimento.
pub fn raiz() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_HOME").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let raiz = exe.parent()?.parent()?;
    raiz.join("lib").join(MARCA).is_file().then(|| raiz.to_path_buf())
}

/// `<raiz>/lib/<rel>`, se a distribuição existe e o caminho também.
pub fn em_lib(rel: &str) -> Option<PathBuf> {
    let p = raiz()?.join("lib").join(rel);
    p.exists().then_some(p)
}

/// As bibliotecas do SDK do Dart que a distribuição leva.
pub fn sdk_do_dart() -> Option<PathBuf> {
    em_lib("dart-sdk/lib").filter(|p| p.join("libraries.json").is_file())
}

/// A sobreposição do backend nativo (`sdk_nativo/`).
pub fn sobreposicao_nativa() -> Option<PathBuf> {
    em_lib("sdk_nativo").filter(|p| p.join("libraries.json").is_file())
}

/// Um executável do LLVM da distribuição (`clang`, `ld.lld`…), com a
/// extensão do sistema.
pub fn ferramenta_llvm(nome: &str) -> Option<PathBuf> {
    let arquivo = if cfg!(windows) { format!("{nome}.exe") } else { nome.to_string() };
    em_lib(&format!("llvm/bin/{arquivo}")).filter(|p| p.is_file())
}

/// O diretório de cache do usuário para o dartforge: `%LOCALAPPDATA%` no
/// Windows, `~/Library/Caches` no macOS, `$XDG_CACHE_HOME` ou `~/.cache`
/// nos demais.
pub fn cache_do_usuario() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| Path::new(&h).join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".cache")))
    }?;
    Some(base.join("dartforge"))
}
