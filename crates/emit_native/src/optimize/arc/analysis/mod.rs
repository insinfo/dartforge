//! Domínios da redução estática de ciclos (§§27–34).
//! Ainda não planeja políticas nem transforma a HIR. Desconhecimento deve
//! conservar ARC geral; conjuntos de aliases não constituem certificados.

pub mod model;
pub mod points_to;
pub mod hir;
pub(crate) mod origins;
pub mod module;
pub mod escape;
pub mod summaries;
pub mod graph;
mod calls;
mod writes;
