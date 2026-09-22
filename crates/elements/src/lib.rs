//! Modelo de elementos e carregamento do grafo de bibliotecas (trilha completa).
//!
//! Segunda fase da meta governante (docs/FRONTEND-ARQUITETURA.md §3): a partir
//! de uma entrada, carrega o fecho transitivo de bibliotecas — projeto,
//! pacotes via `package_config.json` e SDK via `libraries.json` com patches —
//! e constrói o outline: namespaces, elementos e hierarquia por nome. Corpos
//! não são analisados aqui.
#![allow(clippy::too_many_arguments, clippy::collapsible_if)]

pub mod config;
pub mod load;
pub mod model;
pub mod outline;
pub mod sdk;

pub use config::*;
pub use load::*;
pub use model::*;
pub use outline::*;
pub use sdk::*;
