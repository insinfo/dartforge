//! Contexto de compilação da crate `dartforge-emit-native`.

use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId, Program, UnitId, VariableId};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::{BodyTypes, Resolved};
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};
use std::collections::HashMap;

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
}

