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

pub mod hierarchy;
pub mod ops;
pub mod resolve;
pub mod subtyping;
pub mod table;

pub use hierarchy::{build_class_hierarchy, ClassHierarchy, ClassHierarchyData};
pub use ops::{erase_extension_type, glb, lub, non_nullable, normalize, nullable, substitute};
pub use resolve::{
    ClassTypeData, ExtensionTypeData, FunctionTypeData, OutlineResolver, OutlineTypes,
    ParameterTypeData, TypedefTypeData, VariableTypeData,
};
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

