//! O `canBeConst` do linter (`analyzer/lib/src/lint/linter.dart`) das
//! criações de instância, dos literais tipados e das declarações de
//! construtor de uma unidade, calculado com o motor de constantes para as
//! regras `prefer_const_constructors`,
//! `prefer_const_literals_to_create_immutables` e
//! `prefer_const_constructors_in_immutables` (que vivem com a árvore em
//! `dartforge_analysis::lints`, onde está o `approximateContextType`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::Motor;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// O que pode ser `const` numa unidade.
#[derive(Default, Debug, Clone)]
pub struct PodemSerConst {
    /// Criações de instância não constantes de construtor `const`.
    pub criacoes: HashSet<ExprId>,
    /// Literais de lista, conjunto e mapa sem `const` (fora de contexto
    /// constante).
    pub literais: HashSet<ExprId>,
    /// Construtores não `const` de corpo vazio.
    pub construtores: HashSet<ast::MemberId>,
}

/// Calcula o [`PodemSerConst`] da unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn podem_ser_const(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
) -> PodemSerConst {
    let mut r = PodemSerConst::default();
    let unit = program.unit(u);
    let a = &unit.ast;
    let lib = unit.library;
    let Some(body) = bodies.units.get(u.0 as usize) else { return r };
    let pais = crate::lints_tipados::pais_da_unidade(program, u);
    let mut creations: Vec<ExprId> = Vec::new();
    let mut literals: Vec<ExprId> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            ExprKind::InstanceCreation { keyword, .. } => {
                if *keyword == Some(ast::CreationKeyword::Const) || pais.em_contexto_constante(a, id) {
                    continue;
                }
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id)
                    && program.function(*f).const_
                {
                    creations.push(id);
                }
            }
            ExprKind::Call { .. } => {
                if pais.em_contexto_constante(a, id) {
                    continue;
                }
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id)
                    && program.function(*f).const_
                {
                    creations.push(id);
                }
            }
            ExprKind::List { const_: false, .. } | ExprKind::SetOrMap { const_: false, .. } => {
                if !pais.em_contexto_constante(a, id) {
                    literals.push(id);
                }
            }
            _ => {}
        }
    }
    let constructors: Vec<ast::MemberId> = a
        .members
        .iter()
        .enumerate()
        .filter_map(|(k, m)| match &m.kind {
            MemberKind::Constructor(c) if !c.const_ && matches!(c.body, ast::FunctionBody::Empty) => Some(ast::MemberId(k as u32)),
            _ => None,
        })
        .collect();
    if creations.is_empty() && literals.is_empty() && constructors.is_empty() {
        return r;
    }
    let mut engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
    for e in creations {
        if crate::constantes::verifier::creation_can_be_const(&mut engine, lib, u, e) {
            r.criacoes.insert(e);
        }
    }
    for e in literals {
        if crate::constantes::verifier::literal_can_be_const(&mut engine, lib, u, e) {
            r.literais.insert(e);
        }
    }
    for m in constructors {
        if crate::constantes::verifier::constructor_can_be_const(&mut engine, lib, u, m) {
            r.construtores.insert(m);
        }
    }
    r
}
