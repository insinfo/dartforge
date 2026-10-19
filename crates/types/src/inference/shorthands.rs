//! Dot shorthands (Dart 3.10, `docs/VERSOES-LINGUAGEM.md` §4.4): `.id`,
//! `.id(args)`, `.new(args)` and `const .id(args)` resolved in the
//! declaration that the **context** of the selector chain denotes.
//!
//! The context is that of the whole chain: the outermost node of a chain
//! whose root is `.id` records its context for the root
//! ([`record_chain`]); `e == .x` records the type of `e` first
//! ([`record_equality`]). Only the first record counts.
//!
//! Like analyzer 3.13.4, shorthands are resolved even in a library without
//! the feature (the parser already reported `experiment_not_enabled`), and
//! the resolution errors are those of `ResolverVisitor`: without a
//! declaration in the context, an access `.id` (or a creation
//! `const .id(…)`) is `DOT_SHORTHAND_MISSING_CONTEXT` at the node; an
//! invocation `.id(…)` is `DOT_SHORTHAND_UNDEFINED_INVOCATION` at the name,
//! with the context displayed (`_` when there is none); with the declaration
//! and without the member, `DOT_SHORTHAND_UNDEFINED_GETTER`/`_INVOCATION` at
//! the name.

use super::BodyInferrer;
use super::body::Body;
use super::expr::resolver;
use crate::resolved::Resolved;
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;

/// Outer node of a `.id…` chain: records its context for the root.
pub(crate) fn record_chain(inf: &BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) {
    let a = &inf.program.unit(cx.unit).ast;
    if !matches!(
        a.expr(e).kind,
        ExprKind::Property { .. }
            | ExprKind::Call { .. }
            | ExprKind::Index { .. }
            | ExprKind::TypeArguments { .. }
            | ExprKind::Unary { .. }
    ) {
        return;
    }
    if let Some(root) = a.shorthand_root(e) {
        cx.shorthand_context.entry(root.0).or_insert(ctx);
    }
}

/// `e == .x` / `e != .x`: the shorthand on the right uses the type of `e`.
pub(crate) fn record_equality(
    inf: &BodyInferrer<'_>,
    cx: &mut Body,
    right: ExprId,
    left: TypeId,
) {
    if let Some(r) = inf.program.unit(cx.unit).ast.shorthand_root(right) {
        cx.shorthand_context.insert(r.0, left);
    }
}

/// The context without the outer `FutureOr`s (`futureOrBase`), for display.
fn future_or_base(inf: &BodyInferrer<'_>, mut t: TypeId) -> TypeId {
    while let Type::FutureOr { arg, .. } = inf.table.get(t) {
        t = *arg;
    }
    t
}

/// The context displayed in the message: without `FutureOr` and without the
/// `?` (analyzer 3.13.4 shows `Object` for the context `Object?`).
fn display(inf: &mut BodyInferrer<'_>, t: TypeId) -> String {
    let base = future_or_base(inf, t);
    let base = inf.non_null(base);
    // The analyzer passes the text ready-made (`getDisplayString()`, without
    // the alias).
    inf.table.format_without_alias(base, inf.interner, inf.program)
}

/// The declaration `D` that the context denotes (spec 3.10, "Declaration
/// denoted by a type scheme"): `C`/`C<…>` of a class, mixin, enum or extension
/// type; `S?` and `FutureOr<S>` denote what `S` denotes. `Future<S>` falls
/// back to `S` when `Future` does not have the member (the context of the
/// `=>` of an `async` function).
///
/// A private declaration from another library is not denoted (analyzer
/// 3.13.4 treats the context as absent).
fn declaration(inf: &BodyInferrer<'_>, cx: &Body, ctx: TypeId, the_name: SymbolId) -> Option<ClassId> {
    let mut t = ctx;
    let d = loop {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } => {
                let c = *class;
                if Some(c) == inf.core.future_class && args.len() == 1 && !has_member(inf, c, the_name)
                {
                    t = args[0];
                    continue;
                }
                break c;
            }
            Type::ExtensionType { decl, .. } => break *decl,
            Type::FutureOr { arg, .. } => t = *arg,
            _ => return None,
        }
    };
    let class = inf.program.class(d);
    if class.library != cx.lib && inf.interner.resolve(class.name).starts_with('_') {
        return None;
    }
    Some(d)
}

/// `D` has the constructor with key `k`: declared, the primary of an
/// extension type, or the implicit unnamed one of a class without
/// constructors.
fn has_constructor(inf: &BodyInferrer<'_>, d: ClassId, k: SymbolId) -> bool {
    if inf.constructor_or_primary(d, k).is_some() {
        return true;
    }
    let class = inf.program.class(d);
    Some(k) == inf.sym.empty
        && class.constructors.is_empty()
        && matches!(class.kind, ClassKind::Class | ClassKind::MixinApplication)
}

/// The key of constructor `the_name` (`new` = the unnamed one).
fn constructor_key(inf: &BodyInferrer<'_>, the_name: SymbolId) -> Option<SymbolId> {
    if Some(the_name) == inf.sym.new_ {
        inf.sym.empty
    } else {
        Some(the_name)
    }
}

/// Static member, enum constant or constructor `the_name` of `d`.
fn has_member(inf: &BodyInferrer<'_>, d: ClassId, the_name: SymbolId) -> bool {
    let class = inf.program.class(d);
    class.static_members.contains_key(&the_name)
        || class
            .enum_constants
            .iter()
            .any(|v| inf.program.variable(*v).name == the_name)
        || constructor_key(inf, the_name).is_some_and(|k| has_constructor(inf, d, k))
}

/// The type of `D.the_name` as a value: static getter/field, enum constant,
/// static method (tear-off) or constructor (tear-off).
fn member_type(inf: &mut BodyInferrer<'_>, d: ClassId, the_name: SymbolId) -> Option<TypeId> {
    let class = inf.program.class(d);
    if let Some(&fid) = class.static_members.get(&the_name) {
        let f = inf.program.function(fid);
        return Some(match (f.kind, f.variable) {
            (FunctionKind::ImplicitAccessor, Some(vid)) => inf.variable_type(vid),
            (FunctionKind::Getter, _) => inf.outline.functions[fid.0 as usize].return_type,
            _ => inf.outline.functions[fid.0 as usize].signature,
        });
    }
    if class
        .enum_constants
        .iter()
        .any(|v| inf.program.variable(*v).name == the_name)
    {
        return Some(inf.table.intern(Type::Interface {
            class: d,
            args: Box::new([]),
            nullable: false,
        }));
    }
    let k = constructor_key(inf, the_name)?;
    match inf.constructor_or_primary(d, k) {
        Some(Some(fid)) => Some(inf.outline.functions[fid.0 as usize].signature),
        Some(None) => Some(inf.primary_signature(d)),
        None => has_constructor(inf, d, k).then_some(inf.core.dynamic_),
    }
}

/// `.the_name` without a call: getter, field, enum constant or tear-off.
pub(crate) fn value_(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    the_name: SymbolId,
    ctx: TypeId,
) -> TypeId {
    let ctx = cx.shorthand_context.remove(&e.0).unwrap_or(ctx);
    let reported = cx.reported_shorthands.remove(&e.0);
    match declaration(inf, cx, ctx, the_name) {
        Some(d) => {
            resolver(inf, cx, e, Resolved::Element(Element::Class(d)));
            // `PropertyElementResolver.resolveDotShorthand` (main checkout,
            // `property_element_resolver.dart:281-305`): the tear-off of a
            // generative constructor (declared, or the implicit one of `.new`
            // without constructors) of an abstract class, at the node.
            let k = inf.program.class(d);
            if !reported && k.kind == dartforge_elements::model::ClassKind::Class && k.modifiers.abstract_ {
                let generator = match constructor_key(inf, the_name).and_then(|c| k.constructors.get(&c)) {
                    Some(&f) => !inf.program.function(f).factory,
                    None => Some(the_name) == inf.sym.new_ && k.constructors.is_empty(),
                };
                if generator {
                    let span = inf.program.unit(cx.unit).ast.expr(e).span;
                    inf.warning_with_code(
                        dartforge_diagnostics::codes::compile_time_error::TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS,
                        span,
                        &[],
                    );
                }
            }
            match member_type(inf, d, the_name) {
                Some(t) => t,
                None => {
                    if !reported {
                        let (text, span) = name_and_span(inf, cx, e);
                        let ty = display(inf, ctx);
                        report(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_GETTER", span, &[&text, &ty]);
                    }
                    inf.core.dynamic_
                }
            }
        }
        None => {
            if !reported {
                let span = inf.program.unit(cx.unit).ast.expr(e).span;
                report(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
            }
            inf.core.dynamic_
        }
    }
}

/// Reports by the code's unique name (all three are from the 3.13.4
/// supplement of `dartforge_diagnostics`' table).
fn report(inf: &mut BodyInferrer<'_>, unique: &str, span: dartforge_diagnostics::Span, args: &[&str]) {
    if let Some(code) = dartforge_diagnostics::Code::by_unique(unique) {
        inf.warning_with_code(code, span, args);
    }
}

/// The text and the range of the name of a `DotShorthand`.
fn name_and_span(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> (String, dartforge_diagnostics::Span) {
    match &inf.program.unit(cx.unit).ast.expr(e).kind {
        ExprKind::DotShorthand { name, .. } => (inf.interner.resolve(name.sym).to_string(), name.span),
        _ => (String::new(), inf.program.unit(cx.unit).ast.expr(e).span),
    }
}

/// `.the_name(args)` / `const .the_name(args)` that is a construction: the
/// constructor of `D`, with the type arguments from the call's context.
/// `None` when it is not a construction (a static method goes through the
/// call's common path, which types the target through [`value_`]).
pub(crate) fn construction(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    ctx: TypeId,
) -> Option<TypeId> {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, .. } = &a.expr(e).kind else {
        return None;
    };
    let tgt = *target;
    let ExprKind::DotShorthand { name, const_ } = &a.expr(tgt).kind else {
        return None;
    };
    let (the_name, const_) = (name.sym, *const_);
    let ctx_chain = cx.shorthand_context.get(&tgt.0).copied().unwrap_or(ctx);
    let Some(d) = declaration(inf, cx, ctx_chain, the_name) else {
        // Without a declaration in the context: the creation `const .id(…)` has
        // no context; the invocation `.id(…)` does not find the member in `_`.
        if const_ {
            let span = inf.program.unit(cx.unit).ast.expr(e).span;
            report(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
        } else {
            let (text, span) = name_and_span(inf, cx, tgt);
            let ty = display(inf, ctx_chain);
            report(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&text, &ty]);
        }
        cx.reported_shorthands.insert(tgt.0);
        return None;
    };
    let class = inf.program.class(d);
    if class.static_members.contains_key(&the_name) {
        return None;
    }
    // `ConstructorInvocationResolver.resolveDotShorthand` (main checkout,
    // `constructor_invocation_resolver.dart:60-95`): `const .id(…)` without
    // an accessible constructor `id` in the context's declaration is
    // `CONST_WITH_UNDEFINED_CONSTRUCTOR` at the name, with the class name.
    let accessible = |inf: &BodyInferrer<'_>, f: dartforge_elements::model::FunctionElementId| {
        let fe = inf.program.function(f);
        !inf.interner.resolve(fe.name).starts_with('_') || fe.library == cx.lib
    };
    let finding = constructor_key(inf, the_name).and_then(|k| class.constructors.get(&k)).copied();
    let finding = if const_ { finding.filter(|&f| accessible(inf, f)) } else { finding };
    let Some(fid) = finding else {
        // `.new()` of an abstract class without a declared constructor: the
        // implicit constructor is generative (`resolveDotShorthand`,
        // `:107-112`), at the node.
        let k = inf.program.class(d);
        if !const_
            && Some(the_name) == inf.sym.new_
            && k.kind == dartforge_elements::model::ClassKind::Class
            && k.modifiers.abstract_
            && k.constructors.is_empty()
        {
            let span = inf.program.unit(cx.unit).ast.expr(e).span;
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
            // The target goes through the common path (`value`), which is not a
            // tear-off.
            cx.reported_shorthands.insert(tgt.0);
        }
        if const_ {
            let (text, span) = name_and_span(inf, cx, tgt);
            let cls = inf.interner.resolve(inf.program.class(d).name).to_string();
            report(inf, "CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR", span, &[&cls, &text]);
            cx.reported_shorthands.insert(tgt.0);
        } else if !has_member(inf, d, the_name) {
            let (text, span) = name_and_span(inf, cx, tgt);
            let ty = display(inf, ctx_chain);
            report(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&text, &ty]);
            cx.reported_shorthands.insert(tgt.0);
        }
        return None;
    };
    // `resolveDotShorthand` (main checkout,
    // `constructor_invocation_resolver.dart:96-117`): an abstract class with
    // a non-factory constructor is `INSTANTIATE_ABSTRACT_CLASS` at the whole
    // node; otherwise the written type arguments are
    // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR` at the list.
    {
        let program = inf.program;
        let ExprKind::Call { arguments, .. } = &program.unit(cx.unit).ast.expr(e).kind else {
            return None;
        };
        let k = inf.program.class(d);
        if k.kind == dartforge_elements::model::ClassKind::Class && k.modifiers.abstract_ && !inf.program.function(fid).factory {
            let span = program.unit(cx.unit).ast.expr(e).span;
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
        } else if let Some(sp) = super::calls::type_list_range(inf, cx.unit, &arguments.type_args) {
            let (text, _) = name_and_span(inf, cx, tgt);
            let cls = inf.interner.resolve(k.name).to_string();
            report(inf, "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR", sp, &[&cls, &text]);
        }
    }
    // The constructor invocation through the creation's common path
    // (`InstanceCreationExpression` with the context type): the type
    // arguments from the context and the argument check (arity, types).
    cx.shorthand_context.remove(&tgt.0);
    resolver(inf, cx, tgt, Resolved::Element(Element::Class(d)));
    let program = inf.program;
    let ExprKind::Call { arguments, .. } = &program.unit(cx.unit).ast.expr(e).kind else {
        return None;
    };
    // The entity and the name for arity errors: the constructor's name
    // (`'new'`, `'named'`).
    let (text, span) = name_and_span(inf, cx, tgt);
    super::calls::define_target(inf, Some(text), span);
    let r = super::calls::build(inf, cx, Some(e), d, Some(fid), None, arguments, ctx);
    inf.arity_target = None;
    Some(r)
}
