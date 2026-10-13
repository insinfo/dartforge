//! The DartForge macro host (docs/MACROS-PROTOCOLO.md).
//!
//! * [`applications`] — cheap detection (zero cost without a `macro` class) and
//!   the order of the applications, the one of CFE 3.6.2;
//! * [`model`] and [`queries`] — what the macro sees of the program, served
//!   from `elements` with identifiers that are stable across phases;
//! * [`assembly`] — the augmentation library with Rules 1–4, byte for byte
//!   the one CFE 3.6.2 produces;
//! * [`protocol`] and [`executor`] — the `macro.*` service of `dfexec/1` (the
//!   same protocol and executor as the builders, docs/BUILD-PROTOCOLO.md) and
//!   the [`executor::MacroExecutor`] trait;
//! * [`session`] — the three phases with the program reloaded between them
//!   (the diff-based reload reuses the units that did not change);
//! * [`cache`] — the expansion cache: an application whose request and whose
//!   queries answer the same is reused without talking to the executor;
//! * [`vm`] — the **materialization** executor: the same macro API (pure
//!   Dart, `packages/macros`) on a Dart VM, to write the augmentation that the
//!   official SDK accepts with the experimental flag
//!   (docs/MACROS-COMPATIBILIDADE.md). The JS backend uses it provisionally
//!   until the native executor is ready.
#![allow(clippy::too_many_arguments)]

pub mod applications;
pub mod assembly;
pub mod cache;
pub mod executor;
pub mod model;
pub mod native;
pub mod protocol;
pub mod queries;
pub mod session;
pub mod vm;

pub use session::{Expansion, GeneratedText, Measurement, Output, pending_applications, apply, apply_incremental, sessions};

/// Does the program apply any macro? A look at
/// [`Program::classes_macro`](dartforge_elements::model::Program), which the
/// outline fills in the same pass that creates the elements: without a
/// `macro` class, no annotation is looked at (zero-cost rule, item 1).
pub fn has_macros(program: &dartforge_elements::model::Program) -> bool {
    !program.classes_macro.is_empty()
}
