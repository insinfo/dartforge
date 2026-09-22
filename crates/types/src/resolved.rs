//! Resolução de nós de identificadores e expressões em corpos para suas declarações e alvos semânticos.
//!
//! Toda referência semântica nos corpos de funções ou inicializadores é registrada
//! na tabela lateral [`UnitBodyTypes`], indexada compactamente por [`ast::ExprId`].

use crate::table::{TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, LibraryId, VariableId};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;

/// Identificador de uma variável local declarada em um bloco ou expressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

/// Referência a um membro de classe ou extensão (função/método ou campo/variável).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberRef {
    Function(FunctionElementId),
    Variable(VariableId),
}

/// Alvo resolvido de uma referência ou identificador dentro de um corpo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resolved {
    /// Variável local declarada em escopo de bloco (`var x`, `int x`, `for (var x...)`).
    Local(LocalId),
    /// Parâmetro formal da função, método ou closure envolvente.
    Parameter { index: u32, name: SymbolId },
    /// Parâmetro de tipo genérico local ou da classe/extensão envolvente.
    TypeParameter(TypeParamId),
    /// Elemento de nível superior (classe, função de topo, variável de topo, typedef, extensão).
    Element(Element),
    /// Membro de classe (campo, getter, setter, método) acessado via receptor explícito ou implícito (`this`).
    Member {
        class: ClassId,
        member: MemberRef,
        via_super: bool,
    },
    /// Prefixo de importação (`import '...' as p; p.x`).
    Prefix(LibraryId),
    /// Acesso a membro em receptor cujo tipo estático é `dynamic`.
    Dynamic,
    /// Membro de extensão resolvido para chamada de método/getter/setter estático de extensão.
    ExtensionMember {
        extension: ExtensionId,
        member: FunctionElementId,
    },
    /// Construtor de classe chamado em instanciação (`new C()`, `C.named()`).
    Constructor(FunctionElementId),
}

/// Tabelas laterais de tipos estáticos e resoluções para uma única unidade de compilação.
///
/// Mantém vetores indexados diretamente por [`ast::ExprId::0 as usize`], sem custos de hash
/// ou ponteiros no heap.
#[derive(Debug, Clone, Default)]
pub struct UnitBodyTypes {
    /// Tipo estático inferido de cada expressão na arena `ast.exprs`.
    pub static_types: Vec<TypeId>,
    /// Alvo resolvido de expressões de identificador, propriedade ou chamada.
    pub resolved: Vec<Option<Resolved>>,
}

impl UnitBodyTypes {
    /// Cria tabelas laterais alocadas com o tamanho exato de expressões da AST da unidade.
    pub fn new(num_exprs: usize, fallback_type: TypeId) -> Self {
        Self {
            static_types: vec![fallback_type; num_exprs],
            resolved: vec![None; num_exprs],
        }
    }

    /// Define o tipo estático de uma expressão.
    pub fn set_type(&mut self, expr: ast::ExprId, ty: TypeId) {
        let idx = expr.0 as usize;
        if idx < self.static_types.len() {
            self.static_types[idx] = ty;
        }
    }

    /// Obtém o tipo estático associado a uma expressão.
    pub fn get_type(&self, expr: ast::ExprId) -> Option<TypeId> {
        self.static_types.get(expr.0 as usize).copied()
    }

    /// Registra a resolução semântica de uma expressão.
    pub fn set_resolved(&mut self, expr: ast::ExprId, target: Resolved) {
        let idx = expr.0 as usize;
        if idx < self.resolved.len() {
            self.resolved[idx] = Some(target);
        }
    }

    /// Obtém o alvo semântico resolvido de uma expressão.
    pub fn get_resolved(&self, expr: ast::ExprId) -> Option<&Resolved> {
        self.resolved.get(expr.0 as usize).and_then(|r| r.as_ref())
    }
}

/// Conjunto de todas as tabelas laterais de corpos de todas as unidades do programa.
#[derive(Debug, Clone, Default)]
pub struct BodyTypes {
    /// Tabelas indexadas pelo índice da unidade em `Program::units`.
    pub units: Vec<UnitBodyTypes>,
}
