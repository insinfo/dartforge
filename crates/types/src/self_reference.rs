//! The analyzer's `TypeAliasSelfReferenceFinder` (`summary2/type_alias.dart`):
//! the `typedef` that reaches itself. Such an alias instantiates as `dynamic`
//! (`TypeAliasElementImpl.instantiate` with `hasSelfReference`,
//! `element.dart:9690-9696`), and the bounds check of the arguments does not
//! run on it (`TYPE_ALIAS_CANNOT_REFERENCE_ITSELF` is reported in `analise`).

use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypedefKind};
use dartforge_intern::SymbolId;

/// The `typedef` declared in `target` reaches itself.
pub fn self_referencing_typedef(program: &Program, target: DeclRef) -> bool {
    let lib = program.unit(target.unit).library;
    let mut finder = SelfReferenceFinder {
        program,
        lib,
        target,
        visited: Vec::new(),
        found: false,
    };
    finder.typedef(target);
    finder.found
}

/// `TypeAliasSelfReferenceFinder` (`summary2/type_alias.dart`): the `typedef`
/// reaches itself through the types it writes and through the bounds of the
/// type parameters of the classes, mixins and `typedef`s it names (enums and
/// extension types are not followed). Only declarations of this library are
/// followed (the analyzer follows those of the library cycle being linked).
struct SelfReferenceFinder<'a> {
    program: &'a Program,
    lib: LibraryId,
    target: DeclRef,
    visited: Vec<DeclRef>,
    found: bool,
}

impl SelfReferenceFinder<'_> {
    fn declaration(
        &self,
        u: UnitId,
        the_name: &[ast::Name],
        scope: &[SymbolId],
    ) -> Option<DeclRef> {
        let b = match the_name {
            [n] if scope.contains(&n.sym) => return None,
            [n] => self.program.lookup_in_unit(u, n.sym),
            [p, n] => self.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
            _ => None,
        }?;
        let (lib, decl) = match b.getter? {
            Element::Class(id) => (self.program.class(id).library, self.program.class(id).decl?),
            Element::Typedef(id) => (
                self.program.typedef(id).library,
                self.program.typedef(id).decl,
            ),
            _ => return None,
        };
        (lib == self.lib).then_some(decl)
    }

    fn typ(&mut self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, scope: &mut Vec<SymbolId>) {
        if self.found {
            return;
        }
        match &ast_.ty(t).kind {
            TypeKind::Named { name, args } => {
                if let Some(d) = self.declaration(u, name, scope) {
                    if d == self.target {
                        self.found = true;
                        return;
                    }
                    if !self.visited.contains(&d) {
                        self.visited.push(d);
                        let ast_d = &self.program.unit(d.unit).ast;
                        match &ast_d.decl(d.decl).kind {
                            DeclKind::Class(c) => {
                                self.type_parameters(d.unit, ast_d, &c.type_params, &mut Vec::new())
                            }
                            DeclKind::Mixin(m) => {
                                self.type_parameters(d.unit, ast_d, &m.type_params, &mut Vec::new())
                            }
                            DeclKind::Typedef(_) => self.typedef(d),
                            _ => {}
                        }
                    }
                }
                for &a in args.iter() {
                    self.typ(u, ast_, a, scope);
                }
            }
            TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let n = scope.len();
                self.type_parameters(u, ast_, type_params, scope);
                self.parameters(u, ast_, parameters, scope);
                if let Some(r) = return_type {
                    self.typ(u, ast_, *r, scope);
                }
                scope.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                for &p in positional.iter().chain(named.iter().map(|(_, t)| t)) {
                    self.typ(u, ast_, p, scope);
                }
            }
            TypeKind::Void => {}
        }
    }

    /// The bounds (`_typeParameterList`), with the names already in scope.
    fn type_parameters(
        &mut self,
        u: UnitId,
        ast_: &ast::Ast,
        tps: &[ast::TypeParameter],
        scope: &mut Vec<SymbolId>,
    ) {
        scope.extend(tps.iter().map(|p| p.name.sym));
        for p in tps {
            if let Some(b) = p.bound {
                self.typ(u, ast_, b, scope);
            }
        }
    }

    /// `_formalParameterList`: the types of the simple parameters and of the
    /// function-typed parameters (not those of `this.x`/`super.x`).
    fn parameters(
        &mut self,
        u: UnitId,
        ast_: &ast::Ast,
        ps: &[Parameter],
        scope: &mut Vec<SymbolId>,
    ) {
        for p in ps {
            if p.this_ || p.super_ {
                continue;
            }
            let n = scope.len();
            scope.extend(p.function_type_params.iter().map(|x| x.name.sym));
            if let Some(t) = p.ty {
                self.typ(u, ast_, t, scope);
            }
            if let Some(inner) = &p.function_parameters {
                self.parameters(u, ast_, inner, scope);
            }
            scope.truncate(n);
        }
    }

    fn typedef(&mut self, d: DeclRef) {
        let ast_ = &self.program.unit(d.unit).ast;
        let DeclKind::Typedef(td) = &ast_.decl(d.decl).kind else {
            return;
        };
        let mut scope = Vec::new();
        self.type_parameters(d.unit, ast_, &td.type_params, &mut scope);
        match &td.kind {
            TypedefKind::Alias(t) => self.typ(d.unit, ast_, *t, &mut scope),
            TypedefKind::Legacy {
                return_type,
                parameters,
            } => {
                self.parameters(d.unit, ast_, parameters, &mut scope);
                if let Some(r) = return_type {
                    self.typ(d.unit, ast_, *r, &mut scope);
                }
            }
        }
    }
}
