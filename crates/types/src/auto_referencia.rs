//! O `TypeAliasSelfReferenceFinder` do analyzer (`summary2/type_alias.dart`):
//! o `typedef` que chega a si mesmo. Um alias assim instancia como `dynamic`
//! (`TypeAliasElementImpl.instantiate` com `hasSelfReference`,
//! `element.dart:9690-9696`), e a conferência de limites dos argumentos não
//! roda nele (o `TYPE_ALIAS_CANNOT_REFERENCE_ITSELF` sai no `analise`).

use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypedefKind};
use dartforge_intern::SymbolId;

/// O `typedef` declarado em `alvo` chega a si mesmo.
pub fn typedef_auto_referente(program: &Program, target: DeclRef) -> bool {
    let lib = program.unit(target.unit).library;
    let mut finder = SelfReferenceFinder { programa: program, lib, alvo: target, visited: Vec::new(), found: false };
    finder.typedef(target);
    finder.found
}

/// `TypeAliasSelfReferenceFinder` (`summary2/type_alias.dart`): o `typedef`
/// chega a si mesmo pelos tipos que escreve e pelos limites dos parâmetros de
/// tipo das classes, mixins e `typedef` que nomeia (enum e tipo de extensão
/// não são seguidos). Só se seguem declarações desta biblioteca (o analyzer
/// segue as do ciclo de bibliotecas em ligação).
struct SelfReferenceFinder<'a> {
    programa: &'a Program,
    lib: LibraryId,
    alvo: DeclRef,
    visited: Vec<DeclRef>,
    found: bool,
}

impl SelfReferenceFinder<'_> {
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

    fn tipo(&mut self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, scope: &mut Vec<SymbolId>) {
        if self.found {
            return;
        }
        match &ast_.ty(t).kind {
            TypeKind::Named { name, args } => {
                if let Some(d) = self.declaracao(u, name, scope) {
                    if d == self.alvo {
                        self.found = true;
                        return;
                    }
                    if !self.visited.contains(&d) {
                        self.visited.push(d);
                        let ast_d = &self.programa.unit(d.unit).ast;
                        match &ast_d.decl(d.decl).kind {
                            DeclKind::Class(c) => self.parametros_de_tipo(d.unit, ast_d, &c.type_params, &mut Vec::new()),
                            DeclKind::Mixin(m) => self.parametros_de_tipo(d.unit, ast_d, &m.type_params, &mut Vec::new()),
                            DeclKind::Typedef(_) => self.typedef(d),
                            _ => {}
                        }
                    }
                }
                for &a in args.iter() {
                    self.tipo(u, ast_, a, scope);
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = scope.len();
                self.parametros_de_tipo(u, ast_, type_params, scope);
                self.parametros(u, ast_, parameters, scope);
                if let Some(r) = return_type {
                    self.tipo(u, ast_, *r, scope);
                }
                scope.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                for &p in positional.iter().chain(named.iter().map(|(_, t)| t)) {
                    self.tipo(u, ast_, p, scope);
                }
            }
            TypeKind::Void => {}
        }
    }

    /// Os limites (`_typeParameterList`), com os nomes já no escopo.
    fn parametros_de_tipo(&mut self, u: UnitId, ast_: &ast::Ast, tps: &[ast::TypeParameter], scope: &mut Vec<SymbolId>) {
        scope.extend(tps.iter().map(|p| p.name.sym));
        for p in tps {
            if let Some(b) = p.bound {
                self.tipo(u, ast_, b, scope);
            }
        }
    }

    /// `_formalParameterList`: os tipos dos parâmetros simples e das
    /// funções-parâmetro (não os de `this.x`/`super.x`).
    fn parametros(&mut self, u: UnitId, ast_: &ast::Ast, ps: &[Parameter], scope: &mut Vec<SymbolId>) {
        for p in ps {
            if p.this_ || p.super_ {
                continue;
            }
            let n = scope.len();
            scope.extend(p.function_type_params.iter().map(|x| x.name.sym));
            if let Some(t) = p.ty {
                self.tipo(u, ast_, t, scope);
            }
            if let Some(inner) = &p.function_parameters {
                self.parametros(u, ast_, inner, scope);
            }
            scope.truncate(n);
        }
    }

    fn typedef(&mut self, d: DeclRef) {
        let ast_ = &self.programa.unit(d.unit).ast;
        let DeclKind::Typedef(td) = &ast_.decl(d.decl).kind else { return };
        let mut scope = Vec::new();
        self.parametros_de_tipo(d.unit, ast_, &td.type_params, &mut scope);
        match &td.kind {
            TypedefKind::Alias(t) => self.tipo(d.unit, ast_, *t, &mut scope),
            TypedefKind::Legacy { return_type, parameters } => {
                self.parametros(d.unit, ast_, parameters, &mut scope);
                if let Some(r) = return_type {
                    self.tipo(d.unit, ast_, *r, &mut scope);
                }
            }
        }
    }
}
