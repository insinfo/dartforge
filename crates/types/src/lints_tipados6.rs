//! O `canBeConst` do linter (`analyzer/lib/src/lint/linter.dart`) das
//! criações de instância, dos literais tipados e das declarações de
//! construtor de uma unidade, calculado com o motor de constantes para as
//! regras `prefer_const_constructors`,
//! `prefer_const_literals_to_create_immutables` e
//! `prefer_const_constructors_in_immutables` (que vivem com a árvore em
//! `dartforge_analise::lints`, onde está o `approximateContextType`).
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
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    u: UnitId,
) -> PodemSerConst {
    let mut r = PodemSerConst::default();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let lib = unidade.library;
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return r };
    let pais = crate::lints_tipados::pais_da_unidade(program, u);
    let mut criacoes: Vec<ExprId> = Vec::new();
    let mut literais: Vec<ExprId> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            ExprKind::InstanceCreation { keyword, .. } => {
                if *keyword == Some(ast::CreationKeyword::Const) || pais.em_contexto_constante(a, id) {
                    continue;
                }
                if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id)
                    && program.function(*f).const_
                {
                    criacoes.push(id);
                }
            }
            ExprKind::Call { .. } => {
                if pais.em_contexto_constante(a, id) {
                    continue;
                }
                if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id)
                    && program.function(*f).const_
                {
                    criacoes.push(id);
                }
            }
            ExprKind::List { const_: false, .. } | ExprKind::SetOrMap { const_: false, .. } => {
                if !pais.em_contexto_constante(a, id) {
                    literais.push(id);
                }
            }
            _ => {}
        }
    }
    let construtores: Vec<ast::MemberId> = a
        .members
        .iter()
        .enumerate()
        .filter_map(|(k, m)| match &m.kind {
            MemberKind::Constructor(c) if !c.const_ && matches!(c.body, ast::FunctionBody::Empty) => Some(ast::MemberId(k as u32)),
            _ => None,
        })
        .collect();
    if criacoes.is_empty() && literais.is_empty() && construtores.is_empty() {
        return r;
    }
    let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    for e in criacoes {
        if crate::constantes::verificador::criacao_pode_ser_const(&mut motor, lib, u, e) {
            r.criacoes.insert(e);
        }
    }
    for e in literais {
        if crate::constantes::verificador::literal_pode_ser_const(&mut motor, lib, u, e) {
            r.literais.insert(e);
        }
    }
    for m in construtores {
        if crate::constantes::verificador::construtor_pode_ser_const(&mut motor, lib, u, m) {
            r.construtores.insert(m);
        }
    }
    r
}
