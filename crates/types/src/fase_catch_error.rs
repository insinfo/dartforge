//! O `ErrorHandlerVerifier.verifyMethodInvocation` da 3.6.2
//! (`analyzer/lib/src/error/error_handler_verifier.dart`), com o
//! `ReturnTypeVerifier` no contexto do `onError` de `catchError`
//! (`return_type_verifier.dart`), docs/ANALYZER-ESPECIFICACAO.md
//! ("`invalid_return_type_for_catch_error`").
//!
//! Na invocação de método com alvo (o `realTarget`, também o de uma
//! cascata) e algum argumento:
//!
//! * `catchError` sobre `Future<X>`, com o primeiro argumento posicional
//!   (sem isso, nada): com `T = FutureOr<X>`,
//!   - se o argumento é uma expressão de função: o tipo dela como manipulador
//!     de erro (`_checkErrorHandlerFunctionType`, com o primeiro parâmetro
//!     conferido); e, fora de gerador, cada `return` da própria closure (não
//!     das funções de dentro) e o corpo `=> e`, como num executável
//!     assíncrono de retorno `T` (`T_v = X`): o `return;` sem valor com `X`
//!     que não é `void`, `dynamic` nem `Null` dá `RETURN_WITHOUT_VALUE` na
//!     palavra; o `return e;` dá `RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR`
//!     quando `X` é `void` e `flatten(S)` não é `void`, `dynamic` nem `Null`,
//!     quando `flatten(S)` é `void` e `X` não é `void` nem `dynamic`, ou
//!     quando `S` não é atribuível a `X` nem `flatten(S)` é subtipo de `X`;
//!     o corpo `=> e` só quando `X` não é `void`;
//!   - senão, com um tipo de função: o retorno dele não atribuível a `T` dá
//!     `RETURN_TYPE_INVALID_FOR_CATCH_ERROR` no argumento, e o tipo como
//!     manipulador de erro.
//! * `then` sobre `Future`, com o nomeado `onError:` de tipo de função; o
//!   `listen` de `Stream`, idem: o tipo como manipulador (retorno `void`),
//!   com o primeiro parâmetro conferido só se o valor é expressão de função;
//!   o relato vai no argumento nomeado inteiro.
//! * `handleError` de `Stream` e `onError` de `StreamSubscription`, com o
//!   primeiro argumento posicional de tipo de função: idem.
//!
//! `_checkErrorHandlerFunctionType`: `ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER`
//! quando não há parâmetro, o primeiro é nomeado, o primeiro (conferido) não
//! aceita `Object`, o segundo é nomeado ou não aceita `StackTrace`, ou há
//! mais de dois.
//!
//! O "atribuível" é o sem `strict-casts` (o motor não liga a opção).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::{Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codigos::{compile_time_error, warning};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, ExprKind, FunctionBody, StmtKind};
use dartforge_intern::Interner;
use std::collections::HashMap;

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

/// A classe `nome` declarada na biblioteca `lib`.
fn classe_de(program: &Program, interner: &Interner, lib: Option<LibraryId>, nome: &str) -> Option<ClassId> {
    let sym = interner.lookup(nome)?;
    match program.library(lib?).declared.get(&sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// O `realTarget` de cada seção de cascata (o `CascadeTarget` aponta para o
/// alvo da cascata).
fn alvos_de_cascata(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &s in sections.iter() {
            let mut x = s;
            loop {
                let proximo = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        m.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = proximo;
            }
        }
    }
    m
}

struct Verificador<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    stack_trace: Option<TypeId>,
    saida: Vec<Diagnostic>,
}

impl Verificador<'_> {
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// `isAssignableTo` sem `strict-casts`.
    fn atribuivel(&mut self, a: TypeId, b: TypeId) -> bool {
        matches!(self.table.get(a), Type::Dynamic) || self.sub(a, b)
    }

    fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// `_checkErrorHandlerFunctionType`.
    fn manipulador(&mut self, onde: Span, tipo: TypeId, retorno_esperado: TypeId, conferir_primeiro: bool) {
        let Type::Function { positional, optional, named, .. } = self.table.get(tipo).clone() else { return };
        let relatar = |v: &mut Self| {
            let args = [v.formatar(tipo), v.formatar(retorno_esperado)];
            v.saida.push(Diagnostic::com_codigo(warning::ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER, onde, args));
        };
        // Os parâmetros na ordem: posicionais, opcionais, nomeados.
        let posicionais: Vec<TypeId> = positional.iter().chain(optional.iter()).copied().collect();
        let total = posicionais.len() + named.len();
        if total == 0 {
            return relatar(self);
        }
        let Some(&primeiro) = posicionais.first() else { return relatar(self) };
        if conferir_primeiro {
            let objeto = self.core.object;
            if !self.sub(objeto, primeiro) {
                return relatar(self);
            }
        }
        if total == 2 {
            let Some(&segundo) = posicionais.get(1) else { return relatar(self) };
            if let Some(st) = self.stack_trace
                && !self.sub(st, segundo)
            {
                return relatar(self);
            }
        } else if total > 2 {
            relatar(self);
        }
    }
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
    let a = &program.unit(u).ast;
    let fonte = program.unit(u).source.as_str();
    let stack_trace = classe_de(program, interner, core.core_library, "StackTrace").map(|c| table.intern(Type::Interface { class: c, args: Box::new([]), nullable: false }));
    let stream = classe_de(program, interner, core.async_library, "Stream");
    let subscription = classe_de(program, interner, core.async_library, "StreamSubscription");
    let mut v = Verificador { program, interner, table, core, outline, stack_trace, saida: Vec::new() };
    let cascatas = alvos_de_cascata(a);
    let tipo = |e: ExprId, v: &Verificador<'_>| corpo.get_type(e).filter(|t| !core.is_unknown(&*v.table, *t));
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        // A invocação de método (não a criação de instância sem `new`).
        if matches!(corpo.get_resolved(ExprId(k as u32)), Some(Resolved::Constructor(_))) {
            continue;
        }
        let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
        let alvo = if matches!(a.expr(*alvo).kind, ExprKind::CascadeTarget) {
            match cascatas.get(alvo) {
                Some(x) => *x,
                None => continue,
            }
        } else {
            *alvo
        };
        if arguments.args.is_empty() {
            continue;
        }
        let Some(t_alvo) = tipo(alvo, &v) else { continue };
        let classe_do_alvo = match v.table.get(t_alvo) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let metodo = interner.resolve(name.sym);
        let primeiro_posicional = arguments.args.first().filter(|x| x.name.is_none());
        let nomeado = |n: &str| arguments.args.iter().find(|x| x.name.is_some_and(|m| interner.resolve(m.sym) == n));
        // O tipo de função do argumento.
        let tipo_de_funcao = |x: ExprId, v: &Verificador<'_>| tipo(x, v).filter(|t| matches!(v.table.get(*t), Type::Function { .. }));
        let e_funcao_literal = |x: ExprId| matches!(a.expr(x).kind, ExprKind::FunctionExpression(_));
        if metodo == "catchError" && classe_do_alvo.is_some() && classe_do_alvo == core.future_class {
            let Some(callback) = primeiro_posicional.map(|x| x.value) else { continue };
            let x = match v.table.get(t_alvo) {
                Type::Interface { args, .. } if args.len() == 1 => args[0],
                _ => continue,
            };
            let esperado = v.table.intern(Type::FutureOr { arg: x, nullable: false });
            if let ExprKind::FunctionExpression(fid) = &a.expr(callback).kind {
                if let Some(t) = tipo_de_funcao(callback, &v) {
                    v.manipulador(a.expr(callback).span, t, esperado, true);
                }
                retornos_da_closure(&mut v, a, fonte, corpo, *fid, x, esperado);
            } else if let Some(t) = tipo_de_funcao(callback, &v) {
                // `_checkReturnType`.
                let Type::Function { ret, .. } = *v.table.get(t) else { continue };
                if !v.atribuivel(ret, esperado) {
                    let args = [v.formatar(ret), v.formatar(esperado)];
                    v.saida.push(Diagnostic::com_codigo(warning::RETURN_TYPE_INVALID_FOR_CATCH_ERROR, a.expr(callback).span, args));
                }
                v.manipulador(a.expr(callback).span, t, esperado, true);
            }
            continue;
        }
        let vazio = core.void_;
        // `then(onError:)` de `Future` e `listen(onError:)` de `Stream`: o
        // relato no argumento nomeado inteiro.
        let do_nomeado = (metodo == "then" && classe_do_alvo.is_some() && classe_do_alvo == core.future_class)
            || (metodo == "listen" && classe_do_alvo.is_some() && classe_do_alvo == stream);
        if do_nomeado {
            if let Some(arg) = nomeado("onError")
                && let Some(t) = tipo_de_funcao(arg.value, &v)
                && let Some(n) = arg.name
            {
                let onde = Span { start: n.span.start, end: a.expr(arg.value).span.end };
                v.manipulador(onde, t, vazio, e_funcao_literal(arg.value));
            }
            continue;
        }
        // `handleError` de `Stream` e `onError` de `StreamSubscription`.
        let do_posicional = (metodo == "handleError" && classe_do_alvo.is_some() && classe_do_alvo == stream)
            || (metodo == "onError" && classe_do_alvo.is_some() && classe_do_alvo == subscription);
        if do_posicional
            && let Some(arg) = primeiro_posicional
            && let Some(t) = tipo_de_funcao(arg.value, &v)
        {
            v.manipulador(a.expr(arg.value).span, t, vazio, e_funcao_literal(arg.value));
        }
    }
    v.saida
}

/// Os `return` da closure do `onError` de `catchError` (não os das funções de
/// dentro), no contexto assíncrono de retorno `esperado = FutureOr<X>`.
fn retornos_da_closure(v: &mut Verificador<'_>, a: &ast::Ast, fonte: &str, corpo: &UnitBodyTypes, fid: ast::FunctionId, x: TypeId, esperado: TypeId) {
    let f = a.function(fid);
    // O gerador não tem os `return` conferidos.
    if matches!(f.modifier, AsyncModifier::AsyncStar | AsyncModifier::SyncStar) {
        return;
    }
    let x_vazio_dinamico_ou_nulo = matches!(v.table.get(x), Type::Void | Type::Dynamic | Type::Null);
    let mut retornadas: Vec<ExprId> = Vec::new();
    match &f.body {
        FunctionBody::Expression(r) => {
            if !matches!(v.table.get(x), Type::Void) {
                retornadas.push(*r);
            }
        }
        FunctionBody::Block(b) => {
            let regiao = a.stmt(*b).span;
            let de_dentro: Vec<Span> = a.functions.iter().map(|g| g.span).filter(|&s| dentro(s, regiao) && s != f.span).collect();
            for s in a.stmts.iter() {
                if !dentro(s.span, regiao) || de_dentro.iter().any(|&d| dentro(s.span, d)) {
                    continue;
                }
                match &s.kind {
                    StmtKind::Return(Some(r)) => retornadas.push(*r),
                    // `_checkReturnWithoutValue` com `T_v = X`.
                    StmtKind::Return(None) if !x_vazio_dinamico_ou_nulo => {
                        let fim = if fonte.get(s.span.start..).is_some_and(|t| t.starts_with("return")) { s.span.start + 6 } else { s.span.end };
                        v.saida.push(Diagnostic::com_codigo(compile_time_error::RETURN_WITHOUT_VALUE, Span { start: s.span.start, end: fim }, std::iter::empty::<&str>()));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for r in retornadas {
        let Some(s) = corpo.get_type(r) else { continue };
        if v.core.is_unknown(&*v.table, s) {
            continue;
        }
        let achatado = achatar(v.table, v.core, s);
        let vazio_dinamico_ou_nulo = |tb: &TypeTable, k: TypeId| matches!(tb.get(k), Type::Void | Type::Dynamic | Type::Null);
        let erro = if matches!(v.table.get(x), Type::Void) {
            !vazio_dinamico_ou_nulo(&*v.table, achatado)
        } else if matches!(v.table.get(achatado), Type::Void) {
            !matches!(v.table.get(x), Type::Void | Type::Dynamic)
        } else {
            let atribuivel = v.atribuivel(s, x);
            let sub = v.sub(achatado, x);
            !atribuivel && !sub
        };
        if erro {
            let args = [v.formatar(s), v.formatar(esperado)];
            v.saida.push(Diagnostic::com_codigo(warning::RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR, a.expr(r).span, args));
        }
    }
}
