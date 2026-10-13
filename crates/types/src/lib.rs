//! # dartforge-types — The DartForge type system
//!
//! Implementation of the hash-consed structural representation of Dart 3 types,
//! resolution of type annotations in program outlines, the transitive closure of
//! the hierarchy of instantiated supertypes, strict algorithmic subtyping following
//! the normative rules (`subtyping.md`), and the core type operations.
//!
//! ## Memory discipline (docs/FRONTEND-ARQUITETURA.md §2)
//!
//! - **Hash-consing**: Every structurally equal instance shares the same [`TypeId`].
//! - **No cyclic pointer graphs**: Using `Rc<RefCell<...>>` is forbidden. Relations
//!   are kept through compact [`TypeId`] and [`TypeParamId`] indices.
//! - **Side tables**: Resolution results live in external structures indexed by
//!   element IDs ([`OutlineTypes`]), preserving the immutability of the AST.
//! - **Memory tracking**: The [`TypeTable`] continuously computes its `payload_bytes`
//!   for LSP plateau monitoring and compiler measurements.

/// The `typedef` that reaches itself (`TypeAliasSelfReferenceFinder`).
pub mod self_reference;
pub mod raw_cycles;
pub mod a_main;
pub mod annotations;
pub mod best_practices;
pub mod bounds;
pub mod codes;
pub mod constant;
pub mod const_eval;
pub mod constraints;
pub mod dump;
pub mod display;
pub mod phase_access;
pub mod phase_deprecated;
pub mod phase_inheritance;
pub mod phase_ffi;
pub mod phase_generics;
pub mod phase_override;
pub mod phase_required;
pub mod phase_result;
pub mod phase_sdk;
pub mod phase_super;
pub mod hierarchy;
pub mod inheritance;
pub mod inference;
pub mod bound_checks;
pub mod typed_lints;
pub mod typed_lints2;
pub mod typed_lints3;
pub mod typed_lints4;
pub mod typed_lints5;
pub mod typed_lints6;
pub mod typed_lints7;
pub mod typed_lints8;
pub mod typed_lints9;
pub mod phase_catch_error;
pub mod phase_do_not_store;
pub mod phase_unused;
pub mod phase_literal;
pub mod phase_mixins;
pub mod phase_strict;
pub mod ops;
pub mod parameters;
pub mod field_promotion;
pub mod redirection;
pub mod resolve;
pub mod resolved;
pub mod scope;
pub mod override_checks;
pub mod subtyping;
pub mod table;
pub mod extension_types;
pub mod variance;

pub use codes::*;
pub use constant::{ConstValue, ConstantEvaluator};
pub use constraints::GenericInferrer;
pub use hierarchy::{build_class_hierarchy, ClassHierarchy, ClassHierarchyData};
pub use inference::{body_at_offset, unit_bodies, applicable_extensions, applicable_extensions_among, infer_bodies, BodyInferrer, BodyRef, InferredBodies, SharedInference};
pub use ops::{erase_extension_type, glb, lub, non_nullable, normalize, nullable, substitute};
pub use resolve::{
    ClassTypeData, ExtensionTypeData, FunctionTypeData, OutlineResolver, OutlineTypes,
    ParameterTypeData, TypedefTypeData, VariableTypeData,
};
pub use resolved::{BodyTypes, ProbedScope, LocalId, VisibleLocal, MemberRef, Resolved, UnitBodyTypes};
pub use scope::ordered_supertypes;
pub use subtyping::{is_subtype, SubtypeEnv};
pub use table::{
    CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeParameterData, TypeTable, Variance,
};

use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;

/// Entry point for resolving the outline type annotations of a [`Program`].
///
/// Resolves all written annotations of classes, functions, variables and typedefs,
/// instantiates the transitive class hierarchy, and returns the side tables
/// ([`OutlineTypes`]) and the accumulated diagnostics.
pub fn resolve_outline(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
) -> (OutlineTypes, Vec<Diagnostic>) {
    let resolver = OutlineResolver::new(program, interner, table, core);
    resolver.resolve_all()
}

/// Like [`resolve_outline`], with the unit (file) of each diagnostic, in the
/// same order: a caller analyzing several files at once does not need to
/// guess the file from the range (two files may have the same one).
pub fn resolve_outline_with_units(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
) -> (OutlineTypes, Vec<Diagnostic>, Vec<dartforge_elements::model::UnitId>) {
    OutlineResolver::new(program, interner, table, core).resolve_all_with_units()
}

/// Entry point for type inference and body resolution in a [`Program`].
///
/// Walks all bodies of functions, methods and initializers, filling the side tables
/// [`BodyTypes`] and resolving the types of expressions, members and lexical scopes.
pub fn infer_program_bodies(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
) -> (BodyTypes, Vec<Diagnostic>) {
    let inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.infer_all()
}

/// Like [`infer_library_bodies`], recording the declaration of the
/// local that each local read resolves to
/// (`UnitBodyTypes::local_declarations`), which the evaluation of local
/// constants uses ([`const_eval`]).
pub fn infer_library_bodies_with_locals(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
) -> (BodyTypes, Vec<Diagnostic>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.only_libraries = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.record_locals = true;
    inferrer.infer_all()
}

/// Like [`infer_library_bodies`] (with `record_locals`, as in
/// [`infer_library_bodies_with_locals`]), with the unit of each
/// diagnostic, in the same order (`None` when no body is being inferred).
pub fn infer_library_bodies_with_units(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
    record_locals: bool,
) -> (BodyTypes, Vec<Diagnostic>, Vec<Option<dartforge_elements::model::UnitId>>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.only_libraries = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.record_locals = record_locals;
    inferrer.infer_all_with_units()
}

/// Like [`infer_library_bodies_with_units`], sharing with the other passes
/// over the same program, table and outline what does not depend on the pass
/// ([`SharedInference`]): the engine's one-pass-per-library loop.
#[allow(clippy::too_many_arguments)]
pub fn infer_library_bodies_shared(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
    record_locals: bool,
    shared: &SharedInference,
) -> (BodyTypes, Vec<Diagnostic>, Vec<Option<dartforge_elements::model::UnitId>>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.only_libraries = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.record_locals = record_locals;
    inferrer.shared = Some(shared);
    inferrer.infer_all_with_units()
}

/// Like [`infer_program_bodies`], inferring the bodies **only** of the given
/// libraries, including the SDK ones when requested (the native backend
/// compiles the SDK from source). Variables of the other libraries are
/// inferred on demand, when someone reads their type, without their
/// warnings.
pub fn infer_library_bodies(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
) -> (BodyTypes, Vec<Diagnostic>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.only_libraries = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.infer_all()
}
