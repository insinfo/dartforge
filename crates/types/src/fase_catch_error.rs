//! `invalid_return_type_for_catch_error` (`WarningCode.RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR`,
//! docs/ANALYZER-ESPECIFICACAO.md, "`invalid_return_type_for_catch_error`"):
//! o `ErrorHandlerVerifier._checkFutureCatchErrorOnError`
//! (`analyzer/lib/src/error/error_handler_verifier.dart`) da 3.6.2.
//!
//! Em `f.catchError(<closure>, …)` com `f` do tipo `Future<X>` e a closure
//! como primeiro argumento posicional, cada `return e;` da closure (não das
//! funções de dentro dela) e o corpo `=> e` são conferidos como num
//! executável assíncrono de retorno `T = FutureOr<X>`
//! (`ReturnTypeVerifier._checkReturnExpression`, ramo assíncrono, com
//! `T_v = futureValueType(T) = X`):
//!
//! * `X` é `void` e `flatten(S)` não é `void`, `dynamic` nem `Null`;
//! * `flatten(S)` é `void` e `X` não é `void` nem `dynamic`;
//! * senão, `S` não é atribuível a `X` e `flatten(S)` não é subtipo de `X`.
//!
//! O corpo `=> e` não é conferido quando `flatten(T)` é `void`. O relato vai
//! na expressão, com `S` e `T`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::{Diagnostic, Span, codigos::warning};
use dartforge_elements::model::{Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind, FunctionBody, StmtKind};
use dartforge_intern::Interner;

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// `flatten`: `Future<Y>` e `FutureOr<Y>` dão `Y` (anulável com eles).
fn achatar(table: &mut TypeTable, core: &CoreTypes, t: TypeId) -> TypeId {
    let (y, anulavel) = match table.get(t) {
        Type::Interface { class, args, nullable } if Some(*class) == core.future_class && args.len() == 1 => (args[0], *nullable),
        Type::FutureOr { arg, nullable } => (*arg, *nullable),
        _ => return t,
    };
    if anulavel { crate::ops::nullable(y, table) } else { y }
}

/// Os relatos da unidade `u`.
pub fn retornos_de_catch_error(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Diagnostic> {
    let mut saida = Vec::new();
    let a = &program.unit(u).ast;
    let Some(catch_error) = interner.lookup("catchError") else { return saida };
    for e in a.exprs.iter() {
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
        if name.sym != catch_error {
            continue;
        }
        let Some(primeiro) = arguments.args.first() else { continue };
        if primeiro.name.is_some() {
            continue;
        }
        let ExprKind::FunctionExpression(fid) = &a.expr(primeiro.value).kind else { continue };
        // O alvo é `Future<X>`.
        let Some(t_alvo) = corpo.get_type(*alvo) else { continue };
        let x = match table.get(t_alvo) {
            Type::Interface { class, args, .. } if Some(*class) == core.future_class && args.len() == 1 => args[0],
            _ => continue,
        };
        let t = table.intern(Type::FutureOr { arg: x, nullable: false });
        let f = a.function(*fid);
        // As expressões retornadas pela própria closure.
        let mut retornadas: Vec<ExprId> = Vec::new();
        match &f.body {
            FunctionBody::Expression(r) => {
                if !matches!(table.get(x), Type::Void) {
                    retornadas.push(*r);
                }
            }
            FunctionBody::Block(b) => {
                let regiao = a.stmt(*b).span;
                // As funções de dentro (não se olham os `return` delas).
                let de_dentro: Vec<Span> = a
                    .functions
                    .iter()
                    .map(|g| g.span)
                    .filter(|&s| dentro(s, regiao) && s != f.span)
                    .collect();
                for s in a.stmts.iter() {
                    if let StmtKind::Return(Some(r)) = &s.kind
                        && dentro(s.span, regiao)
                        && !de_dentro.iter().any(|&d| dentro(s.span, d))
                    {
                        retornadas.push(*r);
                    }
                }
            }
            _ => {}
        }
        for r in retornadas {
            let Some(s) = corpo.get_type(r) else { continue };
            if core.is_unknown(table, s) {
                continue;
            }
            let achatado = achatar(table, core, s);
            let vazio_dinamico_ou_nulo = |tb: &TypeTable, k: TypeId| matches!(tb.get(k), Type::Void | Type::Dynamic | Type::Null);
            let erro = if matches!(table.get(x), Type::Void) {
                !vazio_dinamico_ou_nulo(table, achatado)
            } else if matches!(table.get(achatado), Type::Void) {
                !matches!(table.get(x), Type::Void | Type::Dynamic)
            } else {
                let atribuivel = matches!(table.get(s), Type::Dynamic) || {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(s, x, &mut env)
                };
                let sub = {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(achatado, x, &mut env)
                };
                !atribuivel && !sub
            };
            if erro {
                let de = table.format(s, interner, program);
                let para = table.format(t, interner, program);
                saida.push(Diagnostic::com_codigo(warning::RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR, a.expr(r).span, [de, para]));
            }
        }
    }
    saida
}
