//! DartForge's code generation engine — the replacement for `build_runner`
//! in the development cycle. Contract: `docs/BUILD-MOTOR.md`; Dart executor
//! protocol: `docs/BUILD-PROTOCOLO.md`.
//!
//! The engine asks two questions — **when** (phase plan, schedule,
//! fingerprint) and **what** (executors) — and publishes the result in an
//! in-memory `Generation` (`dartforge_elements::generated`).
//!
//! **Zero cost for those who do not use it** (PLANO.md, governing rule):
//! nothing in this crate is built if no resolved package defines builders
//! ([`detect`]); [`instance_count`] counts the engines created, for the gate.
pub mod client;
pub mod config;
pub mod query;
pub mod executor_deps;
pub mod descriptor;
pub mod equivalent;
pub mod executor;
pub mod native_executor;
pub mod extensions;
pub mod triggers;
pub mod glob;
pub mod graph;
pub mod command_line;
pub mod engine;
#[cfg(feature = "native-generators")]
pub mod natives;
pub mod oracle;
pub mod packages;
pub mod profile;
pub mod persistence;
pub mod plan;
pub mod value;
pub mod vm;

pub use engine::{Update, UpdateContext, Demand, Engine, EngineOptions, Scoreboard, EngineReport};

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

/// Does the project use builders? Some resolved package defines builders in
/// its `build.yaml` (`builders:`/`post_process_builders:`), or `build_runner`
/// is resolved. The name of the official orchestrator is not proof of
/// absence (DF-BUILD-001): a local builder using `package:build`, without
/// `build_runner`, also generates. It costs one `stat` per package plus
/// reading the few `build.yaml` files that exist; with none, nothing is
/// created.
pub fn detect(cfg: &PackageConfig) -> bool {
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
