//! # dartforge-types — Sistema de tipos do DartForge
//!
//! Implementação da representação estrutural hash-consed de tipos em Dart 3,
//! resolução de anotações de tipo do outline de programas, fechamento transitivo
//! da hierarquia de supertipos instanciados, subtipagem algorítmica estrita segundo
//! as regras normativas (`subtyping.md`) e operações centrais de tipos.
//!
//! ## Disciplina de Memória (docs/FRONTEND-ARQUITETURA.md §2)
//!
//! - **Hash-consing**: Toda instância estruturalmente igual compartilha o mesmo [`TypeId`].
//! - **Sem grafos cíclicos de ponteiros**: Proibido uso de `Rc<RefCell<...>>`. Relações
//!   são mantidas via índices compactos [`TypeId`] e [`TypeParamId`].
//! - **Tabelas laterais**: Resultados da resolução residem em estruturas externas indexadas
//!   pelos IDs dos elementos ([`OutlineTypes`]), preservando a imutabilidade da AST.
//! - **Rastreio de memória**: A [`TypeTable`] computa continuamente seu `payload_bytes`
//!   para vigilância de platô do LSP e medições do compilador.

pub mod bounds;
pub mod codes;
pub mod constant;
pub mod constraints;
pub mod despejo;
pub mod hierarchy;
pub mod inferencia;
pub mod ops;
pub mod resolve;
pub mod resolved;
pub mod scope;
pub mod subtyping;
pub mod table;

pub use codes::*;
pub use constant::{ConstValue, ConstantEvaluator};
pub use constraints::GenericInferrer;
pub use hierarchy::{build_class_hierarchy, ClassHierarchy, ClassHierarchyData};
pub use inferencia::BodyInferrer;
pub use ops::{erase_extension_type, glb, lub, non_nullable, normalize, nullable, substitute};
pub use resolve::{
    ClassTypeData, ExtensionTypeData, FunctionTypeData, OutlineResolver, OutlineTypes,
    ParameterTypeData, TypedefTypeData, VariableTypeData,
};
pub use resolved::{BodyTypes, LocalId, MemberRef, Resolved, UnitBodyTypes};
pub use scope::supertipos_ordenados;
pub use subtyping::{is_subtype, SubtypeEnv};
pub use table::{
    CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeParameterData, TypeTable, Variance,
};

use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;

/// Ponto de entrada para a resolução de anotações de tipo do outline de um [`Program`].
///
/// Resolve todas as anotações escritas de classes, funções, variáveis e typedefs,
/// instancia a hierarquia transitiva de classes e retorna as tabelas laterais
/// ([`OutlineTypes`]) e os diagnósticos acumulados.
pub fn resolve_outline(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
) -> (OutlineTypes, Vec<Diagnostic>) {
    let resolver = OutlineResolver::new(program, interner, table, core);
    resolver.resolve_all()
}

/// Ponto de entrada para a inferência de tipos e resolução de corpos em um [`Program`].
///
/// Percorre todos os corpos de funções, métodos e inicializadores, preenchendo as tabelas
/// laterais [`BodyTypes`] e resolvendo tipos de expressões, membros e escopos léxicos.
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

/// Como [`infer_program_bodies`], inferindo os corpos **só** das bibliotecas
/// dadas — inclusive as do SDK, quando pedidas (o backend nativo compila o
/// SDK da fonte). As variáveis das outras bibliotecas são inferidas sob
/// demanda, quando alguém lê o tipo, sem os avisos delas.
pub fn infer_bodies_das_bibliotecas(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut OutlineTypes,
    bibliotecas: &[dartforge_elements::model::LibraryId],
) -> (BodyTypes, Vec<Diagnostic>) {
    let mut inferrer = BodyInferrer::new(program, interner, table, core, outline);
    inferrer.apenas_bibliotecas = Some(bibliotecas.iter().map(|l| l.0).collect());
    inferrer.infer_all()
}
