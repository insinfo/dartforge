//! `DefaultTypesBuilder._breakRawTypeCycles` (`summary2/default_types_builder.dart`):
//! the bound of a type parameter that reaches its own declaration through a raw
//! type (`class A<T extends void Function<U extends A>()>`) becomes `dynamic`, and
//! with it the bounds of all parameters on the path (the `S` of
//! `class B<S extends A>` in `class A<T extends B>`). The `_TypesBuilder` reads the
//! node's bound after that (`element.bound = node.bound?.type`), so it is the
//! element's bound that changes: instantiation to bounds and the argument check
//! see it as `dynamic`.
//!
//! The path (`_findRawTypePathsToDeclaration`) descends through the arguments of
//! types with arguments, through the return type, through the bounds of type
//! parameters and through the parameter types of function types; a raw type of
//! another declaration follows the bounds of its parameters (once per path).
//! Records are not followed. Declarations are read in order and an already broken
//! bound does not continue a path. Only declarations of this library are followed
//! (the analyzer follows those of the library cycle being linked,
//! `getLinkingNode`).

use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypeParameter};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

/// The type parameters (declaration, index) whose bound becomes `dynamic`.
pub fn broken_bounds(program: &Program) -> HashSet<(DeclRef, usize)> {
    let mut broken: HashSet<(DeclRef, usize)> = HashSet::new();
    for (ui, unit) in program.units.iter().enumerate() {
        let u = UnitId(ui as u32);
        let lib = unit.library;
        for (di, d) in unit.ast.decls.iter().enumerate() {
            let Some(tps) = parametros_de(&d.kind) else { continue };
            let target = DeclRef { unit: u, decl: ast::DeclId(di as u32) };
            let mut cycles: Vec<Vec<(DeclRef, usize)>> = Vec::new();
            for (i, tp) in tps.iter().enumerate() {
                let Some(b) = tp.bound else { continue };
                if broken.contains(&(target, i)) {
                    continue;
                }
                let scope: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
                let mut search = Busca { programa: program, lib, alvo: target, broken: &broken, visited: Vec::new() };
                cycles.extend(search.caminhos((target, i), u, &unit.ast, b, &mut scope.clone()));
            }
            for cycle in cycles {
                broken.extend(cycle);
            }
        }
    }
    broken
}

/// The type parameters of the declarations that `DefaultTypesBuilder` walks
/// and that a raw type can name (class, mixin, enum, extension type,
/// `typedef`); extensions and functions are not targets of a raw type.
fn parametros_de(k: &DeclKind) -> Option<&[TypeParameter]> {
    match k {
        DeclKind::Class(x) => Some(&x.type_params),
        DeclKind::Mixin(x) => Some(&x.type_params),
        DeclKind::Enum(x) => Some(&x.type_params),
        DeclKind::ExtensionType(x) => Some(&x.type_params),
        DeclKind::Typedef(x) => Some(&x.type_params),
        _ => None,
    }
}

struct Busca<'a> {
    programa: &'a Program,
    lib: LibraryId,
    alvo: DeclRef,
    broken: &'a HashSet<(DeclRef, usize)>,
    visited: Vec<DeclRef>,
}

impl Busca<'_> {
    fn declaracao(&self, u: UnitId, the_name: &[ast::Name], scope: &[SymbolId]) -> Option<DeclRef> {
        let b = match the_name {
            [n] if scope.contains(&n.sym) => return None,
            [n] => self.programa.lookup_na_unidade(u, n.sym),
            [p, n] => self.programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        }?;
        let (lib, decl) = match b.getter? {
            Element::Class(id) => (self.programa.class(id).library, self.programa.class(id).decl?),
            Element::Typedef(id) => (self.programa.typedef(id).library, self.programa.typedef(id).decl),
            _ => return None,
        };
        (lib == self.lib).then_some(decl)
    }

    /// `_findRawTypePathsToDeclaration(início, t, target, visited)`.
    fn caminhos(
        &mut self,
        start: (DeclRef, usize),
        u: UnitId,
        a: &ast::Ast,
        t: ast::TypeId,
        scope: &mut Vec<SymbolId>,
    ) -> Vec<Vec<(DeclRef, usize)>> {
        let mut output = Vec::new();
        match &a.ty(t).kind {
            TypeKind::Named { name, args } if args.is_empty() => {
                let Some(d) = self.declaracao(u, name, scope) else { return output };
                if d == self.alvo {
                    output.push(vec![start]);
                } else if !self.visited.contains(&d) {
                    self.visited.push(d);
                    let a_d = &self.programa.unit(d.unit).ast;
                    if let Some(tps) = parametros_de(&a_d.decl(d.decl).kind) {
                        let scope_d: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
                        for (j, tp) in tps.iter().enumerate() {
                            let Some(b) = tp.bound else { continue };
                            if self.broken.contains(&(d, j)) {
                                continue;
                            }
                            for tail in self.caminhos((d, j), d.unit, a_d, b, &mut scope_d.clone()) {
                                let mut c = vec![start];
                                c.extend(tail);
                                output.push(c);
                            }
                        }
                    }
                    self.visited.retain(|&x| x != d);
                }
            }
            TypeKind::Named { args, .. } => {
                for &x in args.iter() {
                    output.extend(self.caminhos(start, u, a, x, scope));
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = scope.len();
                scope.extend(type_params.iter().map(|p| p.name.sym));
                if let Some(r) = return_type {
                    output.extend(self.caminhos(start, u, a, *r, scope));
                }
                for p in type_params.iter() {
                    if let Some(b) = p.bound {
                        output.extend(self.caminhos(start, u, a, b, scope));
                    }
                }
                self.parametros(start, u, a, parameters, scope, &mut output);
                scope.truncate(n);
            }
            TypeKind::Record { .. } | TypeKind::Void => {}
        }
        output
    }

    /// The parameter types of a function type; a function-style parameter
    /// (`int f(A x)`) is a function type with the written return type.
    fn parametros(
        &mut self,
        start: (DeclRef, usize),
        u: UnitId,
        a: &ast::Ast,
        ps: &[Parameter],
        scope: &mut Vec<SymbolId>,
        output: &mut Vec<Vec<(DeclRef, usize)>>,
    ) {
        for p in ps {
            let n = scope.len();
            scope.extend(p.function_type_params.iter().map(|x| x.name.sym));
            if let Some(t) = p.ty {
                output.extend(self.caminhos(start, u, a, t, scope));
            }
            if let Some(inner) = &p.function_parameters {
                for tp in p.function_type_params.iter() {
                    if let Some(b) = tp.bound {
                        output.extend(self.caminhos(start, u, a, b, scope));
                    }
                }
                self.parametros(start, u, a, inner, scope, output);
            }
            scope.truncate(n);
        }
    }
}
