//! `non_const_call_to_literal_constructor` (e a variante `_USING_NEW`)
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.7): o
//! `BestPracticesVerifier._checkForLiteralConstructorUse` da 3.6.2
//! (`analyzer/lib/src/error/best_practices_verifier.dart`).
//!
//! A criação sem `const` de um construtor marcado com o `@literal` do
//! `package:meta` (`fase_resultado::anotacao_do_meta`) que poderia ser
//! `const` relata o nó inteiro com o nome do construtor como escrito
//! (`constructorName.type.qualifiedName` e `.nome`); com `new`, a variante
//! `_USING_NEW`. O "poderia ser `const`" é o `canBeConst` do linter do
//! analyzer: o construtor `const` e a verificação de constantes da criação,
//! como se tivesse `const`, sem erro de constante
//! (`constantes::verificador::criacao_pode_ser_const`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::Motor;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, CreationKeyword, ExprId, ExprKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// Uma criação candidata: a unidade, a expressão, o nome a relatar e se
/// foi escrita com `new`.
struct Candidata {
    unidade: UnitId,
    lib: LibraryId,
    expr: ExprId,
    nome: String,
    com_new: bool,
}

/// Os relatos das bibliotecas `libs` (as próprias, cujos corpos `corpos` tem).
#[allow(clippy::too_many_arguments)]
pub fn construtores_literais(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut out = Vec::new();
    if interner.lookup("literal").is_none() {
        return out;
    }
    // As criações sem `const` de construtor `@literal`.
    let mut candidatas: Vec<Candidata> = Vec::new();
    for &lib in libs {
        for &u in &program.library(lib).units {
            let Some(corpo) = corpos.units.get(u.0 as usize) else { continue };
            let unidade = program.unit(u);
            let a = &unidade.ast;
            for (k, e) in a.exprs.iter().enumerate() {
                let id = ExprId(k as u32);
                let (com_new, nome) = match &e.kind {
                    ExprKind::InstanceCreation { keyword, ty, constructor, .. } => {
                        if matches!(keyword, Some(CreationKeyword::Const)) {
                            continue;
                        }
                        let ast::TypeKind::Named { name, .. } = &a.ty(*ty).kind else { continue };
                        let tipo = name.iter().map(|n| interner.resolve(n.sym)).collect::<Vec<_>>().join(".");
                        let nome = match constructor {
                            Some(c) => format!("{tipo}.{}", interner.resolve(c.sym)),
                            None => tipo,
                        };
                        (matches!(keyword, Some(CreationKeyword::New)), nome)
                    }
                    // A criação sem `new` que a resolução diz ser de
                    // construtor: o nome escrito, sem os argumentos de tipo.
                    ExprKind::Call { target, .. } => {
                        if !matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) {
                            continue;
                        }
                        (false, nome_escrito(interner, a, *target).unwrap_or_default())
                    }
                    _ => continue,
                };
                let Some(Resolved::Constructor(f)) = corpo.get_resolved(id) else { continue };
                let Some(onde) = crate::fase_resultado::unidade_da_funcao(program, *f) else { continue };
                let marcado = crate::fase_resultado::anotacoes_da_funcao(program, *f)
                    .iter()
                    .any(|m| crate::fase_resultado::anotacao_do_meta(program, interner, onde, m, "literal"));
                if marcado {
                    candidatas.push(Candidata { unidade: u, lib, expr: id, nome, com_new });
                }
            }
        }
    }
    if candidatas.is_empty() {
        return out;
    }
    let mut motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    for c in candidatas {
        if !crate::constantes::verificador::criacao_pode_ser_const(&mut motor, c.lib, c.unidade, c.expr) {
            continue;
        }
        let codigo = if c.com_new { w::NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW } else { w::NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR };
        let span = program.unit(c.unidade).ast.expr(c.expr).span;
        out.push((c.unidade, Diagnostic::com_codigo(codigo, span, [c.nome.as_str()])));
    }
    out
}

/// O nome escrito no alvo de uma criação sem `new`: `C`, `p.C`, `C.nome`,
/// `p.C.nome`, sem os argumentos de tipo.
fn nome_escrito(interner: &Interner, a: &ast::Ast, e: ExprId) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
        ExprKind::Property { target, name, .. } => Some(format!("{}.{}", nome_escrito(interner, a, *target)?, interner.resolve(name.sym))),
        ExprKind::TypeArguments { target, .. } => nome_escrito(interner, a, *target),
        _ => None,
    }
}
