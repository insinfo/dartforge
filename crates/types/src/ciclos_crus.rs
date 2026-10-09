//! `DefaultTypesBuilder._breakRawTypeCycles` (`summary2/default_types_builder.dart`):
//! o limite de um parâmetro de tipo que chega à própria declaração por um tipo
//! cru (`class A<T extends void Function<U extends A>()>`) vira `dynamic`, e
//! com ele os limites de todos os parâmetros do caminho (o `S` de
//! `class B<S extends A>` em `class A<T extends B>`). O `_TypesBuilder` lê o
//! limite do nó depois disso (`element.bound = node.bound?.type`), então é o
//! limite do elemento que muda: a instanciação para os limites e a conferência
//! dos argumentos o veem como `dynamic`.
//!
//! O caminho (`_findRawTypePathsToDeclaration`) desce pelos argumentos dos tipos
//! com argumentos, pelo retorno, pelos limites dos parâmetros de tipo e pelos
//! tipos dos parâmetros dos tipos função; um tipo cru de outra declaração segue
//! os limites dos parâmetros dela (uma vez por caminho). Registros não são
//! seguidos. As declarações são lidas em ordem e um limite já quebrado não
//! continua caminho. Só se seguem declarações desta biblioteca (o analyzer
//! segue as do ciclo de bibliotecas em ligação, `getLinkingNode`).

use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypeParameter};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

/// Os parâmetros de tipo (declaração, índice) cujo limite vira `dynamic`.
pub fn limites_quebrados(programa: &Program) -> HashSet<(DeclRef, usize)> {
    let mut quebrados: HashSet<(DeclRef, usize)> = HashSet::new();
    for (ui, unidade) in programa.units.iter().enumerate() {
        let u = UnitId(ui as u32);
        let lib = unidade.library;
        for (di, d) in unidade.ast.decls.iter().enumerate() {
            let Some(tps) = parametros_de(&d.kind) else { continue };
            let alvo = DeclRef { unit: u, decl: ast::DeclId(di as u32) };
            let mut ciclos: Vec<Vec<(DeclRef, usize)>> = Vec::new();
            for (i, tp) in tps.iter().enumerate() {
                let Some(b) = tp.bound else { continue };
                if quebrados.contains(&(alvo, i)) {
                    continue;
                }
                let escopo: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
                let mut busca = Busca { programa, lib, alvo, quebrados: &quebrados, visitados: Vec::new() };
                ciclos.extend(busca.caminhos((alvo, i), u, &unidade.ast, b, &mut escopo.clone()));
            }
            for ciclo in ciclos {
                quebrados.extend(ciclo);
            }
        }
    }
    quebrados
}

/// Os parâmetros de tipo das declarações que o `DefaultTypesBuilder` percorre
/// e que um tipo cru pode nomear (classe, mixin, enum, tipo de extensão,
/// `typedef`); a extensão e as funções não são alvo de tipo cru.
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
    quebrados: &'a HashSet<(DeclRef, usize)>,
    visitados: Vec<DeclRef>,
}

impl Busca<'_> {
    fn declaracao(&self, u: UnitId, nome: &[ast::Name], escopo: &[SymbolId]) -> Option<DeclRef> {
        let b = match nome {
            [n] if escopo.contains(&n.sym) => return None,
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

    /// `_findRawTypePathsToDeclaration(início, t, alvo, visitados)`.
    fn caminhos(
        &mut self,
        inicio: (DeclRef, usize),
        u: UnitId,
        a: &ast::Ast,
        t: ast::TypeId,
        escopo: &mut Vec<SymbolId>,
    ) -> Vec<Vec<(DeclRef, usize)>> {
        let mut saida = Vec::new();
        match &a.ty(t).kind {
            TypeKind::Named { name, args } if args.is_empty() => {
                let Some(d) = self.declaracao(u, name, escopo) else { return saida };
                if d == self.alvo {
                    saida.push(vec![inicio]);
                } else if !self.visitados.contains(&d) {
                    self.visitados.push(d);
                    let a_d = &self.programa.unit(d.unit).ast;
                    if let Some(tps) = parametros_de(&a_d.decl(d.decl).kind) {
                        let escopo_d: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
                        for (j, tp) in tps.iter().enumerate() {
                            let Some(b) = tp.bound else { continue };
                            if self.quebrados.contains(&(d, j)) {
                                continue;
                            }
                            for cauda in self.caminhos((d, j), d.unit, a_d, b, &mut escopo_d.clone()) {
                                let mut c = vec![inicio];
                                c.extend(cauda);
                                saida.push(c);
                            }
                        }
                    }
                    self.visitados.retain(|&x| x != d);
                }
            }
            TypeKind::Named { args, .. } => {
                for &x in args.iter() {
                    saida.extend(self.caminhos(inicio, u, a, x, escopo));
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = escopo.len();
                escopo.extend(type_params.iter().map(|p| p.name.sym));
                if let Some(r) = return_type {
                    saida.extend(self.caminhos(inicio, u, a, *r, escopo));
                }
                for p in type_params.iter() {
                    if let Some(b) = p.bound {
                        saida.extend(self.caminhos(inicio, u, a, b, escopo));
                    }
                }
                self.parametros(inicio, u, a, parameters, escopo, &mut saida);
                escopo.truncate(n);
            }
            TypeKind::Record { .. } | TypeKind::Void => {}
        }
        saida
    }

    /// Os tipos dos parâmetros de um tipo função; o parâmetro no estilo
    /// função (`int f(A x)`) é um tipo função com o retorno escrito.
    fn parametros(
        &mut self,
        inicio: (DeclRef, usize),
        u: UnitId,
        a: &ast::Ast,
        ps: &[Parameter],
        escopo: &mut Vec<SymbolId>,
        saida: &mut Vec<Vec<(DeclRef, usize)>>,
    ) {
        for p in ps {
            let n = escopo.len();
            escopo.extend(p.function_type_params.iter().map(|x| x.name.sym));
            if let Some(t) = p.ty {
                saida.extend(self.caminhos(inicio, u, a, t, escopo));
            }
            if let Some(inner) = &p.function_parameters {
                for tp in p.function_type_params.iter() {
                    if let Some(b) = tp.bound {
                        saida.extend(self.caminhos(inicio, u, a, b, escopo));
                    }
                }
                self.parametros(inicio, u, a, inner, escopo, saida);
            }
            escopo.truncate(n);
        }
    }
}
