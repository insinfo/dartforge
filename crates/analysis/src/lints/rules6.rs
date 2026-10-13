//! The sixth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `control_flow_in_finally`, `throw_in_finally`, `avoid_final_parameters`,
//! `avoid_void_async`, `avoid_returning_null_for_void`,
//! `prefer_asserts_with_message`, `unnecessary_library_directive`,
//! `no_self_assignments`, `avoid_annotating_with_dynamic`,
//! `unnecessary_constructor_name`, `unnecessary_breaks`,
//! `prefer_expression_function_bodies`, `combinators_ordering`,
//! `prefer_null_aware_operators`, `one_member_abstracts` and
//! `avoid_private_typedef_functions`.
//!
//! With the same data as the original (those that need the element or the
//! type only report with the unit's semantics):
//! - `avoid_returning_null_for_void`: the return type of the element of the
//!   nearest function (literal, declared or method), inferred or inherited
//!   (`super::function_return`), `void`, or `Future<void>` in an async body.
//! - `unnecessary_breaks`: only with patterns (the library at 3.0 or later).
//! - `unnecessary_constructor_name`: a creation without `new`/`const` is the
//!   call resolved to a constructor.
//! - `avoid_private_typedef_functions`: the `NamedType`s with the same name in
//!   all the library's units, also type literals.
//! - `avoid_annotating_with_dynamic`: the annotation's resolved type
//!   (`TypeAnnotation.type`, also through an alias), `this.x`/`super.x` with
//!   a list, and the augmentation exemption.
//! - `unnecessary_library_directive`: the documentation through `findDartDoc`.
//! - `prefer_null_aware_operators`: the nodes' `toString`.
//! Written without compiling or running (2026-10-05).

use super::walk::{walk, Node};
use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, AssignOp, Ast, AsyncModifier, BinaryOp, Combinator, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionId, FunctionKind,
    Initializer, MemberId, MemberKind, StmtKind, TypeKind, TypedefKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// What leaves the `finally`: `break`/`continue` (with the label, if any) or
/// `return`/`throw`.
enum Flow {
    Break(Option<SymbolId>),
    Continue(Option<SymbolId>),
    Exit,
}

/// `ControlFlowInFinallyBlockReporter.reportIfFinallyAncestorExists`: the node
/// is in the `finally` of the nearest `try`, and nothing inside the `finally`
/// contains it: neither a function (for `return` and `throw`), nor the target
/// of the jump (for `break` and `continue`).
fn escapes_finally(a: &Ast, stack: &[Node], flow: &Flow) -> bool {
    let Some(t) = stack.iter().rposition(|n| matches!(n, Node::Stmt(s) if matches!(a.stmt(*s).kind, StmtKind::Try { .. }))) else {
        return false;
    };
    let Node::Stmt(try_stmt) = stack[t] else { return false };
    let StmtKind::Try { finally_: Some(end), .. } = &a.stmt(try_stmt).kind else { return false };
    if stack.get(t + 1) != Some(&Node::Stmt(*end)) {
        return false;
    }
    let inner = &stack[t + 2..];
    if inner.iter().any(|n| matches!(n, Node::Function(_))) {
        return false;
    }
    let label = match flow {
        Flow::Exit => return true,
        Flow::Break(r) | Flow::Continue(r) => *r,
    };
    let is_break = matches!(flow, Flow::Break(_));
    let is_target = |n: &Node| {
        let Node::Stmt(s) = n else { return false };
        match (&a.stmt(*s).kind, label) {
            (StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. }, None) => true,
            (StmtKind::Switch { .. }, None) => is_break,
            (StmtKind::Labeled { labels, .. }, Some(r)) => labels.iter().any(|l| l.sym == r),
            (StmtKind::Switch { cases, .. }, Some(r)) => cases.iter().any(|k| k.labels.iter().any(|l| l.sym == r)),
            _ => false,
        }
    };
    !inner.iter().any(is_target)
}

/// The range of the `=> e;` body (the `ExpressionFunctionBody`): from the
/// `async`, if any, or from the `=>`, to the `;`.
fn arrow_body(a: &Ast, source: &str, e: ExprId) -> Span {
    let s = a.expr(e).span;
    let before = source.get(..s.start).unwrap_or("").trim_end();
    let mut start = s.start;
    if before.ends_with("=>") {
        start = before.len() - 2;
        let more = before[..start].trim_end();
        if more.ends_with("async") {
            start = more.len() - 5;
        }
    }
    let after = source.get(s.end..).unwrap_or("");
    let trimmed = after.trim_start();
    let end = if trimmed.starts_with(';') { s.end + (after.len() - trimmed.len()) + 1 } else { s.end };
    Span { start: start, end: end }
}

/// The range of the block body (the `BlockFunctionBody`): it starts at the
/// word `async`, `async*` or `sync*`, if any.
fn block_body(source: &str, block: Span, modifier: AsyncModifier) -> Span {
    if modifier == AsyncModifier::None {
        return block;
    }
    let mut before = source.get(..block.start).unwrap_or("").trim_end();
    if let Some(stripped) = before.strip_suffix('*') {
        before = stripped.trim_end();
    }
    let word = if modifier == AsyncModifier::SyncStar { "sync" } else { "async" };
    if before.ends_with(word) { Span { start: before.len() - word.len(), end: block.end } } else { block }
}

/// `Identifier`: `x` or `p.x` (the `PrefixedIdentifier`).
fn identifier(a: &Ast, e: ExprId) -> Option<(Option<SymbolId>, SymbolId)> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some((None, n.sym)),
        ExprKind::Property { target, name, null_aware: false } => match &a.expr(*target).kind {
            ExprKind::Identifier(p) => Some((Some(p.sym), name.sym)),
            _ => None,
        },
        _ => None,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let method_fns: HashSet<FunctionId> = a
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            MemberKind::Method(f) => Some(*f),
            _ => None,
        })
        .collect();
    let is_null = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Null);
    // The `toString` of an expression.
    let source_of = |e: ExprId| -> String { dartforge_frontend::fonte::de_expr(a, source, interner, e) };

    // `prefer_asserts_with_message`: the statement and the initializer.
    if enabled("prefer_asserts_with_message") {
        for s in a.stmts.iter() {
            if matches!(s.kind, StmtKind::Assert { message: None, .. }) {
                report(&c::PREFER_ASSERTS_WITH_MESSAGE, s.span, &[]);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Assert { span, message: None, .. } = i {
                        report(&c::PREFER_ASSERTS_WITH_MESSAGE, *span, &[]);
                    }
                }
            }
        }
    }
    // `no_self_assignments`: `x = x` and `p.x = p.x`.
    if enabled("no_self_assignments") {
        for e in a.exprs.iter() {
            if let ExprKind::Assign { op: AssignOp::Assign, target, value } = &e.kind
                && let (Some(x), Some(y)) = (identifier(a, *target), identifier(a, *value))
                && x == y
            {
                report(&c::NO_SELF_ASSIGNMENTS, e.span, &[]);
            }
        }
    }
    // `avoid_final_parameters`: the parameters of constructors, methods and
    // functions (declared or literal); not those inside a function parameter.
    if enabled("avoid_final_parameters") {
        let mut check = |ps: &[ast::Parameter]| {
            for p in ps.iter().filter(|p| p.final_) {
                report(&c::AVOID_FINAL_PARAMETERS, p.span, &[]);
            }
        };
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                check(&ps[..]);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                check(&k.parameters[..]);
            }
        }
    }
    // `avoid_void_async`: an `async` (non-generator) function or method with a
    // written `void` return; a top-level or local `main` is exempt.
    if enabled("avoid_void_async") {
        for (i, f) in a.functions.iter().enumerate() {
            let Some(name) = f.name else { continue };
            // `returnType.type is VoidType`: the written `void`, or an alias
            // of it (through the unit's semantics).
            let is_void = |t: ast::TypeId| {
                matches!(a.ty(t).kind, TypeKind::Void)
                    || sem.is_some_and(|s| super::written_type(s, t).is_some_and(|x| matches!(s.table.get(x), dartforge_types::table::Type::Void)))
            };
            if f.modifier != AsyncModifier::Async || !f.return_type.is_some_and(is_void) {
                continue;
            }
            if !method_fns.contains(&FunctionId(i as u32)) && interner.resolve(name.sym) == "main" {
                continue;
            }
            report(&c::AVOID_VOID_ASYNC, name.span, &[]);
        }
    }
    // `avoid_annotating_with_dynamic`: every plain, `this.x` or `super.x`
    // parameter with the type `dynamic` written, in any list.
    if enabled("avoid_annotating_with_dynamic") {
        fn collect_lists<'x>(list: &'x [ast::Parameter], out: &mut Vec<&'x [ast::Parameter]>) {
            out.push(list);
            for p in list {
                if let Some(inner) = &p.function_parameters {
                    collect_lists(inner, out);
                }
            }
        }
        let mut all_lists: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                collect_lists(ps, &mut all_lists);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                collect_lists(&k.parameters, &mut all_lists);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { parameters, .. } = &t.kind {
                collect_lists(parameters, &mut all_lists);
            }
        }
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                collect_lists(parameters, &mut all_lists);
            }
        }
        // `inAugmentation`: the declaration (function, method, constructor,
        // field, top-level variable, typedef) that contains the list is `augment`.
        let mut augmented: Vec<Span> = a.decls.iter().filter(|d| d.augment).map(|d| d.span).collect();
        augmented.extend(a.members.iter().filter(|m| m.augment).map(|m| m.span));
        let in_augmentation = |s: Span| augmented.iter().any(|x| x.start <= s.start && s.end <= x.end);
        // The written type is a `NamedType` whose type is `dynamic` (needs the
        // unit's semantics).
        let is_dynamic = |t: ast::TypeId| {
            matches!(a.ty(t).kind, TypeKind::Named { .. })
                && sem.is_some_and(|s| super::written_type(s, t).is_some_and(|x| matches!(s.table.get(x), dartforge_types::table::Type::Dynamic)))
        };
        for list in all_lists {
            // The plain parameter, `this.x` and `super.x` (also with a list):
            // not the function parameter.
            for p in list.iter().filter(|p| p.function_parameters.is_none() || p.this_ || p.super_) {
                let Some(t) = p.ty else { continue };
                if !is_dynamic(t) || in_augmentation(p.span) {
                    continue;
                }
                let ty = a.ty(t);
                // The node is the parameter without the default value.
                let end = match (p.default_value, p.name) {
                    (None, _) => p.span.end,
                    (Some(_), Some(n)) if p.function_parameters.is_some() => dartforge_frontend::fonte::fim_do_parametro_funcao(source, n.span.end),
                    (Some(_), Some(n)) => n.span.end.max(ty.span.end),
                    (Some(_), None) => ty.span.end,
                };
                report(&c::AVOID_ANNOTATING_WITH_DYNAMIC, Span { start: p.span.start, end: end }, &[]);
            }
        }
    }
    // `unnecessary_library_directive`: no `part`, no documentation and no
    // annotation.
    if enabled("unnecessary_library_directive") && !u.unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::Part { .. })) {
        // `sortedCommentAndAnnotations`: the documentation (`_findComment`)
        // and the metadata.
        let comments = dartforge_frontend::comentarios::Comentarios::de(source);
        for d in u.unit.directives.iter() {
            if !matches!(d.kind, DirectiveKind::Library { .. }) || !d.metadata.is_empty() {
                continue;
            }
            if comments.dart_doc(source, d.span.start).is_none() {
                report(&c::UNNECESSARY_LIBRARY_DIRECTIVE, d.span, &[]);
            }
        }
    }
    // `unnecessary_constructor_name`: `.new` in the declaration, in an
    // extension type's primary constructor and in the creation.
    if enabled("unnecessary_constructor_name") {
        let is_new = |n: ast::Name| interner.resolve(n.sym) == "new";
        // The constructors of an extension type with an unnamed primary are
        // not checked.
        let mut exempt: HashSet<MemberId> = HashSet::new();
        for d in a.decls.iter() {
            if let DeclKind::ExtensionType(x) = &d.kind {
                match x.constructor {
                    Some(n) if is_new(n) => report(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]),
                    Some(_) => {}
                    None => exempt.extend(x.members.iter().copied()),
                }
            }
        }
        for (i, m) in a.members.iter().enumerate() {
            if let MemberKind::Constructor(k) = &m.kind
                && let Some(n) = k.name
                && is_new(n)
                && !exempt.contains(&MemberId(i as u32))
            {
                report(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]);
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            match &e.kind {
                ExprKind::InstanceCreation { constructor: Some(n), .. } if is_new(*n) => report(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]),
                // The creation without `new`/`const` (`C.new(…)`): the call
                // resolved to a constructor (needs the unit's semantics).
                ExprKind::Call { target, .. } => {
                    if let ExprKind::Property { name, null_aware: false, .. } = &a.expr(*target).kind
                        && is_new(*name)
                        && let Some(s) = sem
                        && matches!(s.body.get_resolved(ExprId(k as u32)), Some(dartforge_types::resolved::Resolved::Constructor(_)))
                    {
                        report(&c::UNNECESSARY_CONSTRUCTOR_NAME, name.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }
    // `unnecessary_breaks`: the unlabeled `break` that closes a case of more
    // than one statement.
    if enabled("unnecessary_breaks") && sem.is_some_and(|s| super::version_at_least(s, 3, 0)) {
        for s in a.stmts.iter() {
            let StmtKind::Switch { cases, .. } = &s.kind else { continue };
            for k in cases.iter() {
                if k.body.len() > 1
                    && let Some(&last) = k.body.last()
                    && matches!(a.stmt(last).kind, StmtKind::Break(None))
                {
                    report(&c::UNNECESSARY_BREAKS, a.stmt(last).span, &[]);
                }
            }
        }
    }
    // `prefer_expression_function_bodies`: a block with a single `return e;`.
    if enabled("prefer_expression_function_bodies") {
        let mut check = |body: &FunctionBody, modifier: AsyncModifier| {
            if let FunctionBody::Block(s) = body
                && let StmtKind::Block(stmts) = &a.stmt(*s).kind
                && let [single] = &stmts[..]
                && matches!(a.stmt(*single).kind, StmtKind::Return(Some(_)))
            {
                report(&c::PREFER_EXPRESSION_FUNCTION_BODIES, block_body(source, a.stmt(*s).span, modifier), &[]);
            }
        };
        for f in a.functions.iter() {
            check(&f.body, f.modifier);
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                check(&k.body, AsyncModifier::None);
            }
        }
    }
    // `combinators_ordering`: the names of each `show`/`hide` in order.
    if enabled("combinators_ordering") {
        for d in u.unit.directives.iter() {
            let combinators = match &d.kind {
                DirectiveKind::Import { combinators, .. } | DirectiveKind::Export { combinators, .. } => combinators,
                _ => continue,
            };
            for k in combinators.iter() {
                let (word, names) = match k {
                    Combinator::Show(n) => ("show", n),
                    Combinator::Hide(n) => ("hide", n),
                };
                let (Some(first), Some(last)) = (names.first(), names.last()) else { continue };
                if names.windows(2).all(|w| interner.resolve(w[0].sym) <= interner.resolve(w[1].sym)) {
                    continue;
                }
                let start = source
                    .get(..first.span.start)
                    .and_then(|t| t.rfind(word))
                    .filter(|&i| i >= d.span.start)
                    .unwrap_or(first.span.start);
                report(&c::COMBINATORS_ORDERING, Span { start: start, end: last.span.end }, &[]);
            }
        }
    }
    // `prefer_null_aware_operators`: `a == null ? null : a.b` and
    // `a != null ? a.b : null`.
    if enabled("prefer_null_aware_operators") {
        for e in a.exprs.iter() {
            let ExprKind::Conditional { condition, then, else_ } = &e.kind else { continue };
            let ExprKind::Binary { op, left, right } = &a.expr(*condition).kind else { continue };
            let tested = if is_null(*left) {
                *right
            } else if is_null(*right) {
                *left
            } else {
                continue;
            };
            let mut result = match op {
                BinaryOp::Eq if is_null(*then) => *else_,
                BinaryOp::NotEq if is_null(*else_) => *then,
                _ => continue,
            };
            let tested_text = source_of(tested);
            loop {
                // The prefix, the target of the method call, the operand of
                // `!` or the target of the access.
                let next = match &a.expr(result).kind {
                    ExprKind::Property { target, .. } => Some(*target),
                    ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                        ExprKind::Property { target, .. } => Some(*target),
                        ExprKind::TypeArguments { target, .. } => match &a.expr(*target).kind {
                            ExprKind::Property { target, .. } => Some(*target),
                            _ => None,
                        },
                        _ => None,
                    },
                    ExprKind::Unary { op: UnaryOp::NullAssert, operand } => Some(*operand),
                    _ => None,
                };
                let Some(p) = next else { break };
                if source_of(p) == tested_text {
                    report(&c::PREFER_NULL_AWARE_OPERATORS, e.span, &[]);
                    break;
                }
                result = p;
            }
        }
    }
    // `one_member_abstracts`: an abstract class without clauses, without a
    // field or accessor, with a single method, and that one abstract.
    if enabled("one_member_abstracts") {
        for d in a.decls.iter().filter(|d| !d.augment) {
            let DeclKind::Class(x) = &d.kind else { continue };
            if !x.modifiers.abstract_ || x.modifiers.macro_ || x.mixin_application || x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty() {
                continue;
            }
            let mut has_field = false;
            let mut methods: Vec<&ast::Function> = Vec::new();
            for &m in x.members.iter() {
                match &a.member(m).kind {
                    MemberKind::Field(_) => has_field = true,
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        match f.kind {
                            FunctionKind::Getter | FunctionKind::Setter => has_field = true,
                            FunctionKind::Function | FunctionKind::Operator => methods.push(f),
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if let (false, [single]) = (has_field, &methods[..])
                && matches!(single.body, FunctionBody::Empty)
                && !single.external
                && !single.static_
            {
                report(&c::ONE_MEMBER_ABSTRACTS, x.name.span, &[single.name.map_or("", |n| interner.resolve(n.sym))]);
            }
        }
    }
    // `avoid_private_typedef_functions`: the private function type alias
    // mentioned at most once.
    if enabled("avoid_private_typedef_functions")
        && let Some(s) = sem
    {
        // The library's units (`context.allUnits`).
        let units = &s.program.library(s.program.unit(s.unit).library).units;
        for d in a.decls.iter() {
            let DeclKind::Typedef(x) = &d.kind else { continue };
            let eligible = match &x.kind {
                TypedefKind::Legacy { .. } => true,
                TypedefKind::Alias(t) => x.type_params.is_empty() && matches!(a.ty(*t).kind, TypeKind::Function { .. }),
            };
            if !eligible || !interner.resolve(x.name.sym).starts_with('_') {
                continue;
            }
            // `_CountVisitor`: the `NamedType`s with the same name in all the
            // units, also a type literal (which resolution turns into a
            // `TypeLiteral` with a `NamedType`).
            let mut uses = 0usize;
            for &uu in units.iter() {
                let au = &s.program.unit(uu).ast;
                uses += au.types.iter().filter(|t| matches!(&t.kind, TypeKind::Named { name, .. } if name.last().is_some_and(|n| n.sym == x.name.sym))).count();
                if let Some(cu) = s.bodies.units.get(uu.0 as usize) {
                    uses += au
                        .exprs
                        .iter()
                        .enumerate()
                        .filter(|(k, ex)| {
                            matches!(&ex.kind, ExprKind::Identifier(n) if n.sym == x.name.sym)
                                && matches!(
                                    cu.get_resolved(ExprId(*k as u32)),
                                    Some(dartforge_types::resolved::Resolved::Element(dartforge_elements::model::Element::Typedef(_)))
                                )
                        })
                        .count();
                }
            }
            if uses <= 1 {
                report(&c::AVOID_PRIVATE_TYPEDEF_FUNCTIONS, x.name.span, &[]);
            }
        }
    }

    // The rules that ask about the ancestors.
    let (in_finally, throw_in_finally, null_for_void_on) = (enabled("control_flow_in_finally"), enabled("throw_in_finally"), enabled("avoid_returning_null_for_void"));
    if in_finally || throw_in_finally || null_for_void_on {
        // The code of `return null`: that of the nearest function (literal,
        // declared or method), if the element's return type is `void` (or
        // `Future<void>` in an async body). Needs the unit's semantics.
        let null_code = |stack: &[Node], own: Option<FunctionId>| -> Option<&'static CodigoLint> {
            let s = sem?;
            let f = own.or_else(|| {
                stack.iter().rev().find_map(|n| match n {
                    Node::Function(f) => Some(*f),
                    _ => None,
                })
            })?;
            let function = a.function(f);
            let ret = super::function_return(s, f)?;
            let is_async = matches!(function.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar);
            let is_void = |t: dartforge_types::table::TypeId| matches!(s.table.get(t), dartforge_types::table::Type::Void);
            let returns_void = match s.table.get(ret) {
                dartforge_types::table::Type::Void => !is_async,
                dartforge_types::table::Type::Interface { class, args, .. } => {
                    is_async && Some(*class) == s.core.future_class && args.first().is_some_and(|x| is_void(*x))
                }
                _ => false,
            };
            if !returns_void {
                return None;
            }
            Some(if method_fns.contains(&f) {
                &c::AVOID_RETURNING_NULL_FOR_VOID_FROM_METHOD
            } else {
                &c::AVOID_RETURNING_NULL_FOR_VOID_FROM_FUNCTION
            })
        };
        walk(u, &mut |node, stack| match node {
            Node::Stmt(s) => {
                let n = a.stmt(s);
                match &n.kind {
                    StmtKind::Break(r) if in_finally => {
                        if escapes_finally(a, stack, &Flow::Break(r.map(|x| x.sym))) {
                            report(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["break"]);
                        }
                    }
                    StmtKind::Continue(r) if in_finally => {
                        if escapes_finally(a, stack, &Flow::Continue(r.map(|x| x.sym))) {
                            report(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["continue"]);
                        }
                    }
                    StmtKind::Return(valor) => {
                        if in_finally && escapes_finally(a, stack, &Flow::Exit) {
                            report(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["return"]);
                        }
                        if null_for_void_on
                            && valor.is_some_and(is_null)
                            && let Some(code) = null_code(stack, None)
                        {
                            report(code, n.span, &[]);
                        }
                    }
                    _ => {}
                }
            }
            Node::Expr(e) => {
                if throw_in_finally && matches!(a.expr(e).kind, ExprKind::Throw(_)) && escapes_finally(a, stack, &Flow::Exit) {
                    report(&c::THROW_IN_FINALLY, a.expr(e).span, &["throw"]);
                }
            }
            Node::Function(f) => {
                if null_for_void_on
                    && let FunctionBody::Expression(v) = &a.function(f).body
                    && is_null(*v)
                    && let Some(code) = null_code(stack, Some(f))
                {
                    report(code, arrow_body(a, source, *v), &[]);
                }
            }
            _ => {}
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn findings(source: &str) -> Vec<(&'static str, String)> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        let mut reports = run(u, &names, &|_| true, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| (r.code.unico, source[r.span.start..r.span.end].to_string())).collect()
    }

    fn only(rule: &str, source: &str) -> Vec<String> {
        findings(source).into_iter().filter(|(c, _)| c.starts_with(rule)).map(|(_, t)| t).collect()
    }

    #[test]
    fn flow_in_finally() {
        let source = "int f(List<int> l) {\n  for (var x in l) {\n    try {\n    } finally {\n      if (x > 0) break;\n      for (;;) {\n        break;\n      }\n      l.forEach((y) {\n        return;\n      });\n      return 1;\n    }\n  }\n  return 0;\n}\n";
        assert_eq!(only("control_flow_in_finally", source), vec!["break;".to_string(), "return 1;".to_string()]);
        let source = "void f() {\n  try {\n    throw 1;\n  } finally {\n    throw 2;\n  }\n}\n";
        assert_eq!(only("throw_in_finally", source), vec!["throw 2".to_string()]);
        // The nearest `try` has no `finally`: nothing.
        let source = "void f() {\n  try {\n  } finally {\n    try {\n      return;\n    } catch (e) {}\n  }\n}\n";
        assert!(only("control_flow_in_finally", source).is_empty());
    }

    #[test]
    fn null_for_void() {
        let source = "void f() {\n  return null;\n}\nFuture<void> g() async => null;\nclass A {\n  void m() => null;\n  set s(int v) {\n    return null;\n  }\n  int n() {\n    return null;\n  }\n}\n";
        // The return type is the element's: without the unit's semantics,
        // nothing.
        assert!(findings(source).into_iter().all(|(c, _)| !c.starts_with("avoid_returning_null_for_void")));
    }

    #[test]
    fn declarations() {
        assert_eq!(only("avoid_void_async", "void main() async {}\nvoid f() async {}\nFuture<void> g() async {}\n"), vec!["f".to_string()]);
        assert_eq!(only("avoid_final_parameters", "void f(final int a, int b) {}\n"), vec!["final int a".to_string()]);
        // The resolved type needs the unit's semantics.
        assert!(only("avoid_annotating_with_dynamic", "void f(dynamic a, [dynamic b = 1]) {}\n").is_empty());
        assert_eq!(only("one_member_abstracts", "abstract class A {\n  void m();\n}\nabstract class B {\n  void m();\n  int get x;\n}\n"), vec!["A".to_string()]);
        // The count is over the whole library: needs the unit's semantics.
        assert!(only("avoid_private_typedef_functions", "typedef _F = void Function();\n_F? c;\n").is_empty());
        // The creation without `new` needs the semantics; the declaration does not.
        assert_eq!(only("unnecessary_constructor_name", "class A {\n  A.new();\n}\nvar a = A.new();\nvar t = A.new;\n"), vec!["new".to_string()]);
        assert_eq!(only("unnecessary_library_directive", "library a;\n"), vec!["library a;".to_string()]);
        assert!(only("unnecessary_library_directive", "/// Doc.\nlibrary a;\n").is_empty());
        assert_eq!(only("combinators_ordering", "import 'dart:math' show max, min;\nimport 'dart:async' hide Timer, Future;\n"), vec!["hide Timer, Future".to_string()]);
    }

    #[test]
    fn statements_and_expressions() {
        assert_eq!(only("prefer_asserts_with_message", "void f(int x) {\n  assert(x > 0);\n  assert(x > 0, 'm');\n}\n"), vec!["assert(x > 0);".to_string()]);
        assert_eq!(only("no_self_assignments", "class A {\n  int x = 0;\n  void m(int x, A o) {\n    x = x;\n    this.x = x;\n    o.x = o.x;\n  }\n}\n"), vec!["x = x".to_string(), "o.x = o.x".to_string()]);
        // The language version needs the unit's semantics.
        assert!(only("unnecessary_breaks", "void f(int x) {\n  switch (x) {\n    case 1:\n      f(2);\n      break;\n    case 2:\n      break;\n  }\n}\n").is_empty());
        assert_eq!(only("prefer_expression_function_bodies", "int f() {\n  return 1;\n}\nFuture<int> g() async {\n  return 1;\n}\nvoid h() {\n  return;\n}\n"), vec![
            "{\n  return 1;\n}".to_string(),
            "async {\n  return 1;\n}".to_string()
        ]);
        assert_eq!(
            only("prefer_null_aware_operators", "int? f(String? a) => a == null ? null : a.length;\nint? g(String? a) => a != null ? a.length : null;\nString? h(String? a) => a == null ? null : a;\n"),
            vec!["a == null ? null : a.length".to_string(), "a != null ? a.length : null".to_string()]
        );
    }
}
