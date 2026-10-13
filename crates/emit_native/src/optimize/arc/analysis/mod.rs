//! Domains of the static cycle reduction (§§27–34).
//! It does not plan policies nor transform the HIR yet. Lack of knowledge must
//! keep general ARC; alias sets are not certificates.

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
