//! The linter's `canBeConst` (`analyzer/lib/src/lint/linter.dart`) for the
//! instance creations, typed literals and constructor declarations of a
//! unit, computed with the constants engine for the rules
//! `prefer_const_constructors`,
//! `prefer_const_literals_to_create_immutables` and
//! `prefer_const_constructors_in_immutables` (which live with the tree in
//! `dartforge_analysis::lints`, where `approximateContextType` is).
//! Written without compiling or running (2026-10-05).

use crate::constantes::avaliador::Motor;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// What can be `const` in a unit.
#[derive(Default, Debug, Clone)]
pub struct PodemSerConst {
    /// Non-constant instance creations of a `const` constructor.
    pub criacoes: HashSet<ExprId>,
    /// List, set and map literals without `const` (outside a constant
    /// context).
    pub literais: HashSet<ExprId>,
    /// Non-`const` constructors with an empty body.
    pub construtores: HashSet<ast::MemberId>,
}

/// Computes the [`PodemSerConst`] of unit `u`.
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
