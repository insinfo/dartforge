//! Domains of the static cycle reduction (§§27–34).
//! It does not plan policies nor transform the HIR yet. Lack of knowledge must
//! keep general ARC; alias sets are not certificates.

mod calls;
pub mod escape;
pub mod graph;
pub mod hir;
pub mod model;
pub mod module;
pub(crate) mod origins;
pub mod points_to;
pub mod summaries;
mod writes;
