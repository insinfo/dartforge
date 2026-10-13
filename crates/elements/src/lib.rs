//! Element model and loading of the library graph (complete track).
//!
//! Second phase of the governing goal (docs/FRONTEND-ARQUITETURA.md §3):
//! starting from an entry point, loads the transitive closure of libraries —
//! project, packages via `package_config.json` and the SDK via
//! `libraries.json` with patches — and builds the outline: namespaces,
//! elements and the hierarchy by name. Bodies are not analyzed here.
#![allow(clippy::too_many_arguments, clippy::collapsible_if)]

pub mod augmentation;
pub mod config;
pub mod distribution;
pub mod generated;
pub mod incremental;
pub mod load;
pub mod model;
pub mod outline;
pub mod sdk;
pub mod sdk_cache;
pub mod unit_cache;

pub use config::*;
pub use load::*;
pub use model::*;
pub use outline::*;
pub use sdk::*;
pub use sdk_cache::SdkCache;
pub use unit_cache::AnalyzedUnitCache;
