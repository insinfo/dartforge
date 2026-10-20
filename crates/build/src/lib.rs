//! DartForge's code generation engine — the replacement for `build_runner`
//! in the development cycle. Contract: `docs/BUILD-MOTOR.md`; Dart executor
//! protocol: `docs/BUILD-PROTOCOLO.md`.
//!
//! The engine asks two questions — **when** (phase plan, schedule,
//! fingerprint) and **what** (executors) — and publishes the result in an
//! in-memory `Generation` (`dartforge_elements::generated`).
//!
//! **Zero cost for those who do not use it** (PLANO.md, governing rule):
//! valid projects without definitions or a runner trigger construct no engine
//! ([`detect`]); [`instance_count`] counts the engines created, for the gate.
pub mod client;
pub mod command_line;
pub mod config;
pub mod descriptor;
mod detection;
pub mod engine;
pub mod equivalent;
pub mod executor;
pub mod executor_deps;
pub mod extensions;
pub mod glob;
pub mod graph;
pub mod native_executor;
#[cfg(feature = "native-generators")]
pub mod natives;
pub mod oracle;
pub mod packages;
pub mod persistence;
pub mod plan;
pub mod profile;
pub mod query;
pub mod triggers;
pub mod value;
pub mod vm;

pub use engine::{Demand, Engine, EngineOptions, EngineReport, Scoreboard, Update, UpdateContext};

use dartforge_elements::config::PackageConfig;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Engine version: it goes into the fingerprint of every action.
pub const VERSION: &str = concat!("dartforge-build/", env!("CARGO_PKG_VERSION"), "/1");

static INSTANCES: AtomicUsize = AtomicUsize::new(0);

/// How many engines this process has built so far (the zero-cost gate requires
/// 0 in a project without builders).
pub fn instance_count() -> usize {
    INSTANCES.load(Ordering::Relaxed)
}

pub(crate) fn count_instance() {
    INSTANCES.fetch_add(1, Ordering::Relaxed);
}

/// Whether the resolved project requires the generation engine.
/// A nonempty builder section or resolved `build_runner` enables the engine.
/// Unreadable files, invalid YAML or invalid definition-section shapes also
/// require validation; they cannot establish that builders are absent. Other
/// schema rules are checked by [`Engine::new`], not by this predicate.
///
/// No engine is constructed here. Only existing files are parsed, using the
/// same first-document semantics as [`config::load_yaml`]. Quoted keys, flow
/// mappings and indentation do not change detection; empty sections and nested
/// target options do not define builders.
///
/// ```
/// use dartforge_build::detect;
/// use dartforge_elements::config::PackageConfig;
/// assert!(!detect(&PackageConfig::default()));
/// ```
pub fn detect(cfg: &PackageConfig) -> bool {
    cfg.packages.contains_key("build_runner")
        || cfg
            .root_dirs
            .iter()
            .any(|(_, dir)| detection::requires_engine(&dir.join("build.yaml")))
}

/// Directory of the root package: the one of the nearest `pubspec.yaml` above
/// `path` (a file or a directory).
pub fn package_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|d| d.join("pubspec.yaml").is_file())
        .map(Path::to_path_buf)
}

/// The plan of a project's `build.dart` (the list of applications), together
/// with the package graph and the configurations read.
pub fn project_plan(
    root: &Path,
    cfg: &PackageConfig,
) -> Result<(packages::PackageGraph, plan::Configs, plan::Plan), String> {
    let graph = packages::PackageGraph::build(root, cfg, None)?;
    let configs = plan::Configs::read(&graph, false)?;
    let plan = plan::Plan::from_script(&graph, &configs)?;
    Ok((graph, configs, plan))
}
