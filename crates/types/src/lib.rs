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
pub mod auto_referencia;
pub mod raw_cycles;
pub mod a_main;
pub mod anotacoes;
pub mod boas_praticas;
pub mod bounds;
pub mod codes;
pub mod constant;
pub mod constantes;
pub mod constraints;
pub mod despejo;
pub mod exibicao;
pub mod fase_acesso;
pub mod fase_deprecado;
pub mod fase_heranca;
pub mod fase_ffi;
pub mod fase_genericos;
pub mod fase_override;
pub mod fase_requeridos;
pub mod fase_resultado;
pub mod fase_sdk;
pub mod fase_super;
pub mod hierarchy;
pub mod heranca;
pub mod inference;
pub mod limites;
pub mod lints_tipados;
pub mod lints_tipados2;
pub mod lints_tipados3;
pub mod lints_tipados4;
pub mod lints_tipados5;
pub mod lints_tipados6;
pub mod lints_tipados7;
pub mod lints_tipados8;
pub mod lints_tipados9;
pub mod fase_catch_error;
pub mod fase_nao_guardar;
pub mod fase_nao_usados;
pub mod fase_literal;
pub mod fase_mixins;
pub mod fase_estrita;
pub mod ops;
pub mod parametros;
pub mod field_promotion;
pub mod redirection;
pub mod resolve;
pub mod resolved;
pub mod scope;
pub mod sobrescritas;
pub mod subtyping;
pub mod table;
pub mod tipos_de_extensao;
pub mod variancia;

pub use codes::*;
pub use constant::{ConstValue, ConstantEvaluator};
pub use constraints::GenericInferrer;
pub use hierarchy::{build_class_hierarchy, ClassHierarchy, ClassHierarchyData};
pub use inference::{corpo_no_offset, corpos_da_unidade, extensoes_aplicaveis, extensoes_aplicaveis_dentre, inferir_corpos, BodyInferrer, BodyRef, InferredBodies};
pub use ops::{erase_extension_type, glb, lub, non_nullable, normalize, nullable, substitute};
pub use resolve::{
    ClassTypeData, ExtensionTypeData, FunctionTypeData, OutlineResolver, OutlineTypes,
    ParameterTypeData, TypedefTypeData, VariableTypeData,
};
pub use resolved::{BodyTypes, EscopoSondado, LocalId, VisibleLocal, MemberRef, Resolved, UnitBodyTypes};
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
pub fn resolve_outline_com_unidades(
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

/// Like [`infer_bodies_das_bibliotecas`], recording the declaration of the
/// local that each local read resolves to
/// (`UnitBodyTypes::declaracoes_de_locais`), which the evaluation of local
/// constants uses ([`constantes`]).
pub fn infer_library_bodies_with_locals(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
) -> (BodyTypes, Vec<Diagnostic>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.apenas_bibliotecas = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.registrar_locais = true;
    inferrer.infer_all()
}

/// Like [`infer_bodies_das_bibliotecas`] (with `record_locals`, as in
/// [`infer_library_bodies_with_locals`]), with the unit of each
/// diagnostic, in the same order (`None` when no body is being inferred).
pub fn infer_bodies_das_bibliotecas_com_unidades(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
    record_locals: bool,
) -> (BodyTypes, Vec<Diagnostic>, Vec<Option<dartforge_elements::model::UnitId>>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.apenas_bibliotecas = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.registrar_locais = record_locals;
    inferrer.infer_all_with_units()
}

/// Like [`infer_program_bodies`], inferring the bodies **only** of the given
/// libraries, including the SDK ones when requested (the native backend
/// compiles the SDK from source). Variables of the other libraries are
/// inferred on demand, when someone reads their type, without their
/// warnings.
pub fn infer_bodies_das_bibliotecas(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    libraries: &[dartforge_elements::model::LibraryId],
) -> (BodyTypes, Vec<Diagnostic>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.apenas_bibliotecas = Some(libraries.iter().map(|l| l.0).collect());
    inferrer.infer_all()
}
