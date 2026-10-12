//! DartForge's code generation engine — the replacement for `build_runner`
//! in the development cycle. Contract: `docs/BUILD-MOTOR.md`; Dart executor
//! protocol: `docs/BUILD-PROTOCOLO.md`.
//!
//! The engine asks two questions — **when** (phase plan, schedule,
//! fingerprint) and **what** (executors) — and publishes the result in an
//! in-memory `Geracao` (`dartforge_elements::gerado`).
//!
//! **Zero cost for those who do not use it** (PLANO.md, governing rule):
//! nothing in this crate is built if no resolved package defines builders
//! ([`detectar`]); [`instancias`] counts the engines created, for the gate.
pub mod cliente;
pub mod config;
pub mod consulta;
pub mod dependencias_executor;
pub mod descritor;
pub mod equivalente;
pub mod executor;
pub mod executor_nativo;
pub mod extensoes;
pub mod gatilhos;
pub mod glob;
pub mod grafo;
pub mod linha_de_comando;
pub mod motor;
#[cfg(feature = "nativos")]
pub mod nativos;
pub mod oraculo;
pub mod pacotes;
pub mod perfil;
pub mod persistencia;
pub mod plano;
pub mod valor;
pub mod vm;

pub use motor::{Atualizacao, Contexto, Demanda, Motor, OpcoesMotor, Placar, RelMotor};

use dartforge_elements::config::PackageConfig;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Engine version: it goes into the fingerprint of every action.
pub const VERSAO: &str = concat!("dartforge-build/", env!("CARGO_PKG_VERSION"), "/1");

static INSTANCES: AtomicUsize = AtomicUsize::new(0);

/// How many engines this process has built so far (the zero-cost gate requires
/// 0 in a project without builders).
pub fn instancias() -> usize {
    INSTANCES.load(Ordering::Relaxed)
}

pub(crate) fn contar_instancia() {
    INSTANCES.fetch_add(1, Ordering::Relaxed);
}

/// Does the project use builders? Some resolved package defines builders in
/// its `build.yaml` (`builders:`/`post_process_builders:`), or `build_runner`
/// is resolved. The name of the official orchestrator is not proof of
/// absence (DF-BUILD-001): a local builder using `package:build`, without
/// `build_runner`, also generates. It costs one `stat` per package plus
/// reading the few `build.yaml` files that exist; with none, nothing is
/// created.
pub fn detectar(cfg: &PackageConfig) -> bool {
    cfg.packages.contains_key("build_runner")
        || cfg
            .root_dirs
            .iter()
            .any(|(_, dir)| defines_builders(&dir.join("build.yaml")))
}

/// Does the `build.yaml` declare builders? Checked through the YAML top-level
/// keys (no indentation), without building the whole configuration.
fn defines_builders(build_yaml: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(build_yaml) else {
        return false;
    };
    text.lines().any(|l| {
        let key = l.split('#').next().unwrap_or_default().trim_end();
        matches!(key, "builders:" | "post_process_builders:")
    })
}

/// Directory of the root package: the one of the nearest `pubspec.yaml` above
/// `path` (a file or a directory).
pub fn raiz_do_pacote(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|d| d.join("pubspec.yaml").is_file())
        .map(Path::to_path_buf)
}

/// The plan of a project's `build.dart` (the list of applications), together
/// with the package graph and the configurations read.
pub fn plano_do_projeto(
    root: &Path,
    cfg: &PackageConfig,
) -> Result<(pacotes::GrafoPacotes, plano::Configs, plano::Plano), String> {
    let graph = pacotes::GrafoPacotes::montar(root, cfg, None)?;
    let configs = plano::Configs::ler(&graph, false)?;
    let plan = plano::Plano::do_script(&graph, &configs)?;
    Ok((graph, configs, plan))
}
