//! Contexto de compilação da crate `dartforge-emit-native`.

use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::{BodyTypes, Resolved};
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};


pub struct Context<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
    pub entry_lib: Option<LibraryId>,
}

impl<'a> Context<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Self {
        Self {
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            entry_lib: program.entry,
        }
    }

    pub fn symbol_name(&self, sym: SymbolId) -> &str {
        self.interner.resolve(sym)
    }

    pub fn get_type(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_type(expr)
        } else {
            None
        }
    }

    pub fn get_resolved(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<&Resolved> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_resolved(expr)
        } else {
            None
        }
    }

    pub fn is_int(&self, ty: TypeId) -> bool {
        if self.core.int_class.is_some() && ty == self.core.int {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "int"
            }
            _ => false,
        }
    }

    pub fn is_double(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "double"
            }
            _ => false,
        }
    }

    pub fn is_bool(&self, ty: TypeId) -> bool {
        if self.core.bool_class.is_some() && ty == self.core.bool_ {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "bool"
            }
            _ => false,
        }
    }

    pub fn is_string(&self, ty: TypeId) -> bool {
        if self.core.string_class.is_some() && ty == self.core.string {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "String"
            }
            _ => false,
        }
    }

    pub fn is_list(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "List"
            }
            _ => false,
        }
    }

    pub fn is_map(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "Map"
            }
            _ => false,
        }
    }

    pub fn is_void(&self, ty: TypeId) -> bool {
        ty == self.core.void_
    }

    /// Representação de um tipo Dart (R1, `docs/NATIVO-PLANO.md` §6.2).
    ///
    /// Só `int`, `double` e `bool` **não anuláveis** são escalares; qualquer
    /// tipo anulável (`int?` inclusive), `num`, `Object`, `dynamic` e
    /// parâmetros de tipo são `Ref`. `Type` (não anulável) é o id de classe
    /// que o runtime usa nos testes de tipo, um `I64` — provisório, até os
    /// objetos `Type` existirem no heap.
    pub fn to_hir_type(&self, ty: TypeId) -> crate::hir::Type {
        if self.is_void(ty) {
            return crate::hir::Type::Void;
        }
        let Type::Interface { class, nullable, .. } = self.table.get(ty) else {
            return crate::hir::Type::Ref;
        };
        let classe = &self.program.classes[class.0 as usize];
        if *nullable || !self.program.library(classe.library).is_sdk {
            return crate::hir::Type::Ref;
        }
        match self.symbol_name(classe.name) {
            "int" | "Type" => crate::hir::Type::I64,
            "double" => crate::hir::Type::F64,
            "bool" => crate::hir::Type::I1,
            _ => crate::hir::Type::Ref,
        }
    }

    /// Tipo declarado (ou inferido) da variável local cujo nome começa em
    /// `offset` (R6).
    pub fn tipo_local(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.bodies.units.get(unit.0 as usize)?.tipo_local(offset)
    }
}

