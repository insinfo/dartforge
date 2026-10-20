//! The 3.6.2 `ErrorHandlerVerifier.verifyMethodInvocation`
//! (`analyzer/lib/src/error/error_handler_verifier.dart`), with the
//! `ReturnTypeVerifier` in the context of the `onError` of `catchError`
//! (`return_type_verifier.dart`), docs/ANALYZER-ESPECIFICACAO.md
//! ("`invalid_return_type_for_catch_error`").
//!
//! On a method invocation with a target (the `realTarget`, also that of a
//! cascade) and some argument:
//!
//! * `catchError` on `Future<X>`, with the first positional argument
//!   (without it, nothing): with `T = FutureOr<X>`,
//!   - if the argument is a function expression: its type as an error handler
//!     (`_checkErrorHandlerFunctionType`, with the first parameter checked);
//!     and, outside a generator, each `return` of the closure itself (not of
//!     the inner functions) and the `=> e` body, as in an asynchronous
//!     executable with return type `T` (`T_v = X`): a `return;` without a
//!     value with `X` that is not `void`, `dynamic` or `Null` gives
//!     `RETURN_WITHOUT_VALUE` at the keyword; `return e;` gives
//!     `RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR` when `X` is `void` and
//!     `flatten(S)` is not `void`, `dynamic` or `Null`, when `flatten(S)` is
//!     `void` and `X` is not `void` or `dynamic`, or when `S` is not
//!     assignable to `X` and `flatten(S)` is not a subtype of `X`; the
//!     `=> e` body only when `X` is not `void`;
//!   - otherwise, with a function type: its return type not assignable to `T`
//!     gives `RETURN_TYPE_INVALID_FOR_CATCH_ERROR` at the argument, and the
//!     type as an error handler.
//! * `then` on `Future`, with the named `onError:` of function type; the
//!   `listen` of `Stream`, likewise: the type as a handler (return `void`),
//!   with the first parameter checked only if the value is a function
//!   expression; the report goes on the whole named argument.
//! * `handleError` of `Stream` and `onError` of `StreamSubscription`, with
//!   the first positional argument of function type: likewise.
//!
//! `_checkErrorHandlerFunctionType`: `ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER`
//! when there is no parameter, the first one is named, the first (checked) one
//! does not accept `Object`, the second is named or does not accept
//! `StackTrace`, or there are more than two.
//!
//! "Assignable" is the one without `strict-casts` (the engine does not enable
//! the option).
//! Written without compiling or running (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::{Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codes::{compile_time_error, warning};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, ExprKind, FunctionBody, StmtKind};
use dartforge_intern::Interner;
use std::collections::HashMap;

fn inside(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// `flatten`: `Future<Y>` and `FutureOr<Y>` give `Y` (nullable with them).
fn flatten(table: &mut TypeTable, core: &CoreTypes, t: TypeId) -> TypeId {
    let (y, is_nullable) = match table.get(t) {
        Type::Interface {
            class,
            args,
            nullable,
        } if Some(*class) == core.future_class && args.len() == 1 => (args[0], *nullable),
        Type::FutureOr { arg, nullable } => (*arg, *nullable),
        _ => return t,
    };
    if is_nullable {
        crate::ops::nullable(y, table)
    } else {
        y
    }
}

/// The class `the_name` declared in library `lib`.
fn class_of(
    program: &Program,
    interner: &Interner,
    lib: Option<LibraryId>,
    the_name: &str,
) -> Option<ClassId> {
    let sym = interner.lookup(the_name)?;
    match program.library(lib?).declared.get(&sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// The `realTarget` of each cascade section (the `CascadeTarget` points to
/// the cascade's target).
pub(crate) fn cascade_targets(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade {
            target, sections, ..
        } = &e.kind
        else {
            continue;
        };
        for &s in sections.iter() {
            let mut x = s;
            loop {
                let next_ = match &a.expr(x).kind {
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
                x = next_;
            }
        }
    }
    m
}

struct Verifier<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    stack_trace: Option<TypeId>,
    output: Vec<Diagnostic>,
}

impl Verifier<'_> {
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// `isAssignableTo` without `strict-casts`.
    fn assignable(&mut self, a: TypeId, b: TypeId) -> bool {
        matches!(self.table.get(a), Type::Dynamic) || self.sub(a, b)
    }

    fn format(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// `_checkErrorHandlerFunctionType`.
    fn handler(&mut self, where_: Span, ty: TypeId, expected_return: TypeId, check_first: bool) {
        let Type::Function {
            positional,
            optional,
            named,
            ..
        } = self.table.get(ty).clone()
        else {
            return;
        };
        let report = |v: &mut Self| {
            let args = [v.format(ty), v.format(expected_return)];
            v.output.push(Diagnostic::with_code(
                warning::ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER,
                where_,
                args,
            ));
        };
        // The parameters in order: positional, optional, named.
        let positional_args: Vec<TypeId> =
            positional.iter().chain(optional.iter()).copied().collect();
        let total = positional_args.len() + named.len();
        if total == 0 {
            return report(self);
        }
        let Some(&first) = positional_args.first() else {
            return report(self);
        };
        if check_first {
            let object = self.core.object;
            if !self.sub(object, first) {
                return report(self);
            }
        }
        if total == 2 {
            let Some(&second) = positional_args.get(1) else {
                return report(self);
            };
            if let Some(st) = self.stack_trace
                && !self.sub(st, second)
            {
                report(self)
            }
        } else if total > 2 {
            report(self);
        }
    }
}

/// The reports of unit `u`.
pub fn catch_error_returns(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(u).ast;
    let source = program.unit(u).source.as_str();
    let stack_trace = class_of(program, interner, core.core_library, "StackTrace").map(|c| {
        table.intern(Type::Interface {
            class: c,
            args: Box::new([]),
            nullable: false,
        })
    });
    let stream = class_of(program, interner, core.async_library, "Stream");
    let subscription = class_of(program, interner, core.async_library, "StreamSubscription");
    let mut v = Verifier {
        program,
        interner,
        table,
        core,
        outline,
        stack_trace,
        output: Vec::new(),
    };
    let cascades = cascade_targets(a);
    let ty =
        |e: ExprId, v: &Verifier<'_>| body.get_type(e).filter(|t| !core.is_unknown(&*v.table, *t));
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Call { target, arguments } = &e.kind else {
            continue;
        };
        // The method invocation (not the instance creation without `new`).
        if matches!(
            body.get_resolved(ExprId(k as u32)),
            Some(Resolved::Constructor(_))
        ) {
            continue;
        }
        let ExprKind::Property {
            target: tgt, name, ..
        } = &a.expr(*target).kind
        else {
            continue;
        };
        let tgt = if matches!(a.expr(*tgt).kind, ExprKind::CascadeTarget) {
            match cascades.get(tgt) {
                Some(x) => *x,
                None => continue,
            }
        } else {
            *tgt
        };
        if arguments.args.is_empty() {
            continue;
        }
        let Some(t_target) = ty(tgt, &v) else {
            continue;
        };
        let target_class = match v.table.get(t_target) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let method = interner.resolve(name.sym);
        let first_positional = arguments.args.first().filter(|x| x.name.is_none());
        let named_one = |n: &str| {
            arguments
                .args
                .iter()
                .find(|x| x.name.is_some_and(|m| interner.resolve(m.sym) == n))
        };
        // The function type of the argument.
        let function_type = |x: ExprId, v: &Verifier<'_>| {
            ty(x, v).filter(|t| matches!(v.table.get(*t), Type::Function { .. }))
        };
        let is_function_literal =
            |x: ExprId| matches!(a.expr(x).kind, ExprKind::FunctionExpression(_));
        if method == "catchError" && target_class.is_some() && target_class == core.future_class {
            let Some(callback) = first_positional.map(|x| x.value) else {
                continue;
            };
            let x = match v.table.get(t_target) {
                Type::Interface { args, .. } if args.len() == 1 => args[0],
                _ => continue,
            };
            let expected = v.table.intern(Type::FutureOr {
                arg: x,
                nullable: false,
            });
            if let ExprKind::FunctionExpression(fid) = &a.expr(callback).kind {
                if let Some(t) = function_type(callback, &v) {
                    v.handler(a.expr(callback).span, t, expected, true);
                }
                closure_returns(&mut v, a, source, body, *fid, x, expected);
            } else if let Some(t) = function_type(callback, &v) {
                // `_checkReturnType`.
                let Type::Function { ret, .. } = *v.table.get(t) else {
                    continue;
                };
                if !v.assignable(ret, expected) {
                    let args = [v.format(ret), v.format(expected)];
                    v.output.push(Diagnostic::with_code(
                        warning::RETURN_TYPE_INVALID_FOR_CATCH_ERROR,
                        a.expr(callback).span,
                        args,
                    ));
                }
                v.handler(a.expr(callback).span, t, expected, true);
            }
            continue;
        }
        let empty = core.void_;
        // `then(onError:)` of `Future` and `listen(onError:)` of `Stream`: the
        // report on the whole named argument.
        let of_named =
            (method == "then" && target_class.is_some() && target_class == core.future_class)
                || (method == "listen" && target_class.is_some() && target_class == stream);
        if of_named {
            if let Some(arg) = named_one("onError")
                && let Some(t) = function_type(arg.value, &v)
                && let Some(n) = arg.name
            {
                let where_ = Span {
                    start: n.span.start,
                    end: a.expr(arg.value).span.end,
                };
                v.handler(where_, t, empty, is_function_literal(arg.value));
            }
            continue;
        }
        // `handleError` of `Stream` and `onError` of `StreamSubscription`.
        let of_positional =
            (method == "handleError" && target_class.is_some() && target_class == stream)
                || (method == "onError" && target_class.is_some() && target_class == subscription);
        if of_positional
            && let Some(arg) = first_positional
            && let Some(t) = function_type(arg.value, &v)
        {
            v.handler(
                a.expr(arg.value).span,
                t,
                empty,
                is_function_literal(arg.value),
            );
        }
    }
    v.output
}

/// The `return`s of the closure of the `onError` of `catchError` (not those
/// of the inner functions), in the asynchronous context with return type
/// `expected = FutureOr<X>`.
fn closure_returns(
    v: &mut Verifier<'_>,
    a: &ast::Ast,
    source: &str,
    body: &UnitBodyTypes,
    fid: ast::FunctionId,
    x: TypeId,
    expected: TypeId,
) {
    let f = a.function(fid);
    // A generator does not have its `return`s checked.
    if matches!(
        f.modifier,
        AsyncModifier::AsyncStar | AsyncModifier::SyncStar
    ) {
        return;
    }
    let x_empty_dynamic_or_null = matches!(v.table.get(x), Type::Void | Type::Dynamic | Type::Null);
    let mut returned: Vec<ExprId> = Vec::new();
    match &f.body {
        FunctionBody::Expression(r) => {
            if !matches!(v.table.get(x), Type::Void) {
                returned.push(*r);
            }
        }
        FunctionBody::Block(b) => {
            let region = a.stmt(*b).span;
            let from_inside: Vec<Span> = a
                .functions
                .iter()
                .map(|g| g.span)
                .filter(|&s| inside(s, region) && s != f.span)
                .collect();
            for s in a.stmts.iter() {
                if !inside(s.span, region) || from_inside.iter().any(|&d| inside(s.span, d)) {
                    continue;
                }
                match &s.kind {
                    StmtKind::Return(Some(r)) => returned.push(*r),
                    // `_checkReturnWithoutValue` with `T_v = X`.
                    StmtKind::Return(None) if !x_empty_dynamic_or_null => {
                        let end = if source
                            .get(s.span.start..)
                            .is_some_and(|t| t.starts_with("return"))
                        {
                            s.span.start + 6
                        } else {
                            s.span.end
                        };
                        v.output.push(Diagnostic::with_code(
                            compile_time_error::RETURN_WITHOUT_VALUE,
                            Span {
                                start: s.span.start,
                                end,
                            },
                            std::iter::empty::<&str>(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for r in returned {
        let Some(s) = body.get_type(r) else { continue };
        if v.core.is_unknown(&*v.table, s) {
            continue;
        }
        let flattened = flatten(v.table, v.core, s);
        let empty_dynamic_or_null = |tb: &TypeTable, k: TypeId| {
            matches!(tb.get(k), Type::Void | Type::Dynamic | Type::Null)
        };
        let error = if matches!(v.table.get(x), Type::Void) {
            !empty_dynamic_or_null(&*v.table, flattened)
        } else if matches!(v.table.get(flattened), Type::Void) {
            !matches!(v.table.get(x), Type::Void | Type::Dynamic)
        } else {
            let assignable = v.assignable(s, x);
            let sub = v.sub(flattened, x);
            !assignable && !sub
        };
        if error {
            let args = [v.format(s), v.format(expected)];
            v.output.push(Diagnostic::with_code(
                warning::RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR,
                a.expr(r).span,
                args,
            ));
        }
    }
}
