//! Cache de unidades já analisadas, para uma sessão residente (`dartforge dev`).
//!
//! Entre duas compilações a maior parte dos arquivos não mudou: no
//! `new_sali/core` são 2.423 unidades (SDK incluído) e uma edição toca uma.
//! Guardando `(fonte, AST, CompilationUnit)` por caminho, a compilação
//! seguinte reanalisa só o que foi invalidado.
//!
//! **Quem decide o que mudou é o chamador**, não o carregador: perguntar ao
//! sistema de arquivos a cada unidade custava um `stat` por arquivo dentro da
//! carga (2.423 no `new_sali`, 7–20 s de relógio no Windows com antivírus).
//! A sessão chama [`CacheUnidades::alterados`] — que faz a varredura **em
//! paralelo** — ou [`CacheUnidades::invalidar`] quando já sabe o arquivo.
//!
//! Condição de uso: o mesmo [`dartforge_intern::Interner`] das análises
//! anteriores, porque os nós guardam `SymbolId` daquela arena.
use crate::model::{Program, Unit, UnitRole};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Identidade barata de um arquivo: o que o `mtime` e o tamanho dizem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marca {
    /// Modificação em nanossegundos desde a época (0 quando indisponível).
    pub mtime_ns: u128,
    pub tamanho: u64,
}

impl Marca {
    /// Lê a marca de um caminho; `None` se o arquivo não existe.
    pub fn ler(path: &Path) -> Option<Marca> {
        let md = std::fs::metadata(path).ok()?;
        let mtime_ns = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Some(Marca { mtime_ns, tamanho: md.len() })
    }
}

/// Unidades guardadas entre compilações, por caminho.
#[derive(Default)]
pub struct CacheUnidades {
    unidades: HashMap<PathBuf, Unit>,
    /// Marca do arquivo quando a unidade foi analisada.
    marcas: HashMap<PathBuf, Marca>,
    /// Unidades reaproveitadas na carga corrente.
    pub reaproveitadas: usize,
}

impl CacheUnidades {
    pub fn nova() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.unidades.len()
    }

    pub fn is_empty(&self) -> bool {
        self.unidades.is_empty()
    }

    /// Esquece uma unidade (arquivo editado ou apagado).
    pub fn invalidar(&mut self, path: &Path) {
        self.unidades.remove(path);
        self.marcas.remove(path);
    }

    pub fn limpar(&mut self) {
        self.unidades.clear();
        self.marcas.clear();
    }

    /// Bytes de fonte retidos — a parcela que cresce com o projeto, não com
    /// o número de edições (o exemplo `medir` vigia isso).
    pub fn bytes_fonte(&self) -> usize {
        self.unidades.values().map(|u| u.source.len()).sum()
    }

    /// Zera os contadores da carga (o carregador chama ao começar).
    pub fn iniciar_carga(&mut self) {
        self.reaproveitadas = 0;
    }

    /// Caminhos guardados cujo arquivo mudou (ou sumiu) desde a análise.
    ///
    /// É a varredura do observador do `dartforge dev`: um `stat` por unidade,
    /// **em paralelo** (no Windows cada `stat` passa pelo antivírus e em série
    /// custaria segundos), sem ler conteúdo.
    pub fn alterados(&self) -> Vec<PathBuf> {
        let caminhos: Vec<(&PathBuf, Marca)> = self.marcas.iter().map(|(p, m)| (p, *m)).collect();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        if caminhos.len() < 32 || threads < 2 {
            return caminhos
                .into_iter()
                .filter(|(p, m)| Marca::ler(p).is_none_or(|atual| atual != *m))
                .map(|(p, _)| p.clone())
                .collect();
        }
        let proximo = std::sync::atomic::AtomicUsize::new(0);
        let lotes: Vec<Vec<PathBuf>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut meus = Vec::new();
                        loop {
                            let i = proximo.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((p, m)) = caminhos.get(i) else { break };
                            if Marca::ler(p).is_none_or(|atual| atual != *m) {
                                meus.push((*p).clone());
                            }
                        }
                        meus
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        lotes.into_iter().flatten().collect()
    }

    /// Caminhos guardados (o que a sessão observa).
    pub fn caminhos(&self) -> Vec<PathBuf> {
        self.unidades.keys().cloned().collect()
    }

    /// Se há unidade guardada para `path` (sem tocar no disco).
    pub fn contem(&self, path: &Path) -> bool {
        self.unidades.contains_key(path)
    }

    /// Retira a unidade de `path`, se houver. Não confere o disco: o que foi
    /// invalidado já saiu daqui.
    pub(crate) fn tirar(&mut self, path: &Path) -> Option<Unit> {
        let u = self.unidades.remove(path);
        if u.is_some() {
            self.reaproveitadas += 1;
        }
        u
    }

    /// Registra a marca de um arquivo recém-lido (um `stat`, no ato da
    /// leitura — é a única consulta ao disco que a carga faz por arquivo novo).
    pub(crate) fn anotar_marca(&mut self, path: &Path) {
        if let Some(m) = Marca::ler(path) {
            self.marcas.insert(path.to_path_buf(), m);
        }
    }

    /// Recolhe as unidades de um [`Program`] que não serve mais — inclusive as
    /// do SDK, que também não mudam entre edições.
    pub fn recolher(&mut self, program: Program) {
        let Program { units, .. } = program;
        for u in units {
            let Some(path) = u.path.clone() else { continue };
            self.unidades.insert(path, u);
        }
    }
}

/// Papel e URI esperados de uma unidade reaproveitada, conferidos pelo
/// carregador antes de aceitá-la (o mesmo arquivo pode entrar como parte de
/// outra biblioteca depois de uma edição de diretivas).
pub(crate) fn serve(u: &Unit, uri: &str, role: UnitRole) -> bool {
    u.uri == uri && u.role == role
}
