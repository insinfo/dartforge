//! Bodies of declared functions, constructors (initializers), function
//! expressions (with return inference, `inference.md` "Function literal
//! return type inference"), local functions, untyped top-level variables and
//! metadata.

use super::body::{Body, FunctionCtx, Local};
use super::expr::{self, declare_local, infer, infer_free};
use super::statements;
use super::BodyInferrer;
use crate::codes::*;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody};
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    /// Context of the `return` expressions for a declared return `r`
    /// (`BodyInferenceContext._contextTypeForImposed`, 3.6.2
    /// `body_inference_context.dart:168-214`). A `dynamic` return (written or
    /// implicit) is not imposed (`resolver.dart:2325`, `:3185`,
    /// `function_expression_resolver.dart:46`): the context is `_`. A generator
    /// whose return is not `Iterable<S>`/`Stream<S>` falls, like an async one,
    /// to `FutureOr<futureValueTypeSchema(r)>`.
    pub(crate) fn declared_return_context(&mut self, r: TypeId, m: AsyncModifier) -> TypeId {
        if matches!(self.table.get(r), Type::Dynamic) || self.the_is_unknown(r) {
            return self.core.unknown;
        }
        let element = match m {
            AsyncModifier::None => return r,
            AsyncModifier::Async => None,
            AsyncModifier::SyncStar => self.as_instance_of(r, self.core.iterable_class).map(|a| a[0]),
            AsyncModifier::AsyncStar => self.as_instance_of(r, self.core.stream_class).map(|a| a[0]),
        };
        if let Some(e) = element {
            return e;
        }
        let fv = self.future_value_type_schema(r);
        self.future_or(fv)
    }
}

/// `ErrorVerifier.visitDefaultFormalParameter` (3.6.2,
/// `error_verifier.dart:640-651`): the default value against the parameter's
/// type (`checkForAssignableExpressionAtType`, `INVALID_ASSIGNMENT`; a `void`
/// value is `USE_OF_VOID_RESULT`).
fn assignable_default_value(inf: &mut BodyInferrer<'_>, cx: &Body, d: ast::ExprId, td: TypeId, t: TypeId) {
    if matches!(inf.table.get(t), Type::Void) {
        return;
    }
    expr::verify_assignable_expr(inf, cx, d, td, t, crate::codes::INVALID_ASSIGNMENT.template);
}

/// Declares the parameters of a written list, with the given types; infers
/// the default values. `so_inicializadores` marks `this.x`/`super.x`.
fn declare_parameters(inf: &mut BodyInferrer<'_>, cx: &mut Body, params: &[ast::Parameter], types: &[TypeId], skip_initializers: bool) {
    for (i, p) in params.iter().enumerate() {
        let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
        if let Some(d) = p.default_value {
            let td = infer(inf, cx, d, t);
            assignable_default_value(inf, cx, d, td, t);
        }
        if skip_initializers && (p.this_ || p.super_) {
            continue;
        }
        if let Some(n) = &p.name {
            // A written type that does not resolve: `dynamic` without being `dynamic`.
            if matches!(inf.table.get(t), Type::Dynamic) {
                if let Some(x) = p.ty {
                    if let ast::TypeKind::Named { name, .. } = &inf.program.unit(cx.unit).ast.ty(x).kind {
                        if !name.last().is_some_and(|k| inf.interner.resolve(k.sym) == "dynamic") {
                            inf.invalid_locals.insert((cx.unit, n.span.start));
                        }
                    }
                }
            }
            // `this.x` and `super.x` introduce final locals in the initializer list
            // (`FieldFormalParameterElementImpl.isFinal`,
            // `SuperFormalParameterElementImpl.isFinal`, 3.6.2
            // `element.dart:3992-3995`, `:9282-9285`).
            let final_ = p.final_ || p.this_ || p.super_;
            declare_local(
                inf,
                cx,
                Local { name: n.sym, ty: t, final_, late: false, const_: false, offset: n.span.start, local_function: false },
                true,
            );
        }
    }
}

/// Infers the body of a declared function/method/constructor.
pub(crate) fn infer_declared_function(inf: &mut BodyInferrer<'_>, f: FunctionElementId) {
    let fe = inf.program.function(f);
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let af = inf.program.unit(unit).ast.function(function);
            let data = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Body::to_function(inf, f, unit);
            cx.root = super::body::Root::Funcao(function);
            for &p in data.type_params.iter() {
                let the_name = inf.table.param(p).name;
                cx.declare_type_param(the_name, p);
            }
            cx.push_scope();
            if let Some(ps) = &af.parameters {
                let types: Vec<TypeId> = data.parameters.iter().map(|p| p.ty).collect();
                declare_parameters(inf, &mut cx, ps, &types, false);
            }
            let ret = data.return_type;
            let ctx_ret = inf.declared_return_context(ret, af.modifier);
            let executable = inf.declared_executable(f);
            let legal_return = legal_return_type(inf, unit, af.return_type, af.modifier, ret);
            cx.functions.push(FunctionCtx { closure_imposed: None, modifier: af.modifier, the_return: Some(ret), return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executable, legal_return });
            function_body(inf, &mut cx, &af.body, af.modifier, ret, None);
            // `checkForBodyMayCompleteNormally` at the function/method name.
            // A setter is not checked: the element has a `void` return (the written
            // type is only `NON_VOID_RETURN_FOR_SETTER`).
            if matches!(af.body, FunctionBody::Block(_))
                && cx.flow.reachable
                && af.kind != ast::FunctionKind::Setter
                && let Some(n) = af.name
            {
                body_completes_normally(inf, Some(ret), Some(ctx_ret), af.modifier, n.span);
            }
            cx.functions.pop();
            // A `FunctionDeclaration` (top-level, not a method) with a written return.
            let fe = inf.program.function(f);
            if fe.class.is_none() && fe.extension.is_none() && af.return_type.is_some() {
                unnecessary_set(inf, unit, function, ret);
            }
        }
        FunctionRef::Constructor { unit, member } => {
            let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return };
            let data = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Body::to_function(inf, f, unit);
            cx.root = super::body::Root::Construtor(member);
            if !fe.factory {
                cx.the_static = false;
                if let Some(c) = fe.class {
                    cx.this_type = Some(inf.class_this_type(c));
                }
            }
            let types: Vec<TypeId> = data.parameters.iter().map(|p| p.ty).collect();
            // Scope of the initializers: all parameters.
            cx.push_scope();
            // The primary's header (3.13) is outside the class body: the `this` of
            // a default value there does not have the class's type (`dynamic`; the
            // 3.13.4 oracle does not report `invalid_assignment`).
            let primary = is_primary_constructor(inf.program, f);
            let header_this = if primary { cx.this_type.take() } else { None };
            cx.untyped_this = primary;
            declare_parameters(inf, &mut cx, &ctor.parameters, &types, false);
            cx.untyped_this = false;
            if primary {
                cx.this_type = header_this;
            }
            // A repeated `this` part (or the extension type's) sees the primary's
            // parameters.
            let of_primary = if ctor.primary_body_part { the_primary_parameters(inf, f) } else { Vec::new() };
            for &(n, t, final_, _) in &of_primary {
                super::expr::declare_local(inf, &mut cx, Local { name: n.sym, ty: t, final_, late: false, const_: false, offset: n.span.start, local_function: false }, true);
            }
            // The primary's parameters, protected in the initializers (the body
            // declares new locals).
            if (is_primary_constructor(inf.program, f) || !of_primary.is_empty())
                && let Some(scope) = cx.scopes.last()
            {
                let ids: Vec<_> = scope.iter().filter_map(|(_, n)| match n {
                    super::body::Nome::Local(id) => Some(*id),
                    _ => None,
                }).collect();
                cx.primary_parameters.extend(ids);
            }
            // Initializers do not see `this` (except for access to the initialized field).
            let saved_this = cx.this_type.take();
            let saved_static = cx.the_static;
            cx.the_static = true;
            cx.untyped_this = true;
            for init in ctor.initializers.iter() {
                initializer(inf, &mut cx, fe.class, init, &ctor.parameters, fe.const_, f);
            }
            cx.untyped_this = false;
            // `_checkForRecursiveConstructorRedirect` (`error_verifier.dart:5047-5066`):
            // at the first `this(...)` of a generative constructor in a cycle.
            if !fe.factory
                && !is_primary_constructor(inf.program, f)
                && let Some(sp) = ctor.initializers.iter().find_map(|i| match i {
                    ast::Initializer::Redirect { span, .. } => Some(*span),
                    _ => None,
                })
                && redirects_in_cycle(inf, f)
            {
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::RECURSIVE_CONSTRUCTOR_REDIRECT, sp, &[]);
            }
            if let Some(red) = &ctor.redirect {
                redirecting_factory_target(inf, &mut cx, red, f, ctor);
            }
            if let Some(c) = fe.class {
                implicit_super(inf, c, ctor, unit, member);
                super_parameters(inf, c, ctor, &types);
            }
            // `_checkForValidField` (`error_verifier.dart:5702-5760`): the written
            // type of `this.x` against the field's (subtype, not assignable).
            if let Some(c) = fe.class {
                for (i, p) in ctor.parameters.iter().enumerate() {
                    if !p.this_ || p.super_ || p.ty.is_none() {
                        continue;
                    }
                    let Some(n) = p.name else { continue };
                    let field = inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == n.sym);
                    let Some(v) = field else { continue };
                    if inf.program.variable(v).static_ {
                        continue;
                    }
                    let (Some(&pt), ft) = (types.get(i), inf.variable_type(v)) else { continue };
                    if !inf.sub(pt, ft) {
                        let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                        let sp = dartforge_diagnostics::Span { start: start, end: n.span.end };
                        inf.warning_with_args(
                            dartforge_diagnostics::codes::compile_time_error::FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE,
                            sp,
                            &[crate::exibicao::Arg::Type_(pt), crate::exibicao::Arg::Type_(ft)],
                        );
                    }
                }
            }
            cx.this_type = saved_this;
            cx.the_static = saved_static;
            cx.pop_scope();
            // Scope of the body: parameters without `this.`/`super.`.
            cx.push_scope();
            let mut cx2 = cx;
            for (i, p) in ctor.parameters.iter().enumerate() {
                if p.this_ || p.super_ {
                    continue;
                }
                if let Some(n) = &p.name {
                    let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
                    let id = cx2.declare(Local { name: n.sym, ty: t, final_: p.final_, late: false, const_: false, offset: n.span.start, local_function: false });
                    cx2.flow.initialize(id);
                }
            }
            for &(n, t, final_, initiator) in &of_primary {
                if !initiator {
                    let id = cx2.declare(Local { name: n.sym, ty: t, final_, late: false, const_: false, offset: n.span.start, local_function: false });
                    cx2.flow.initialize(id);
                }
            }
            let v = inf.core.void_;
            let ret = if fe.factory { data.return_type } else { v };
            let ctx_ret = ret;
            // Generative constructor: `return e;` is `return_in_generative_constructor`.
            let executable = if fe.factory { inf.declared_executable(f) } else { None };
            cx2.functions.push(FunctionCtx { closure_imposed: None, modifier: AsyncModifier::None, the_return: Some(ret), return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executable, legal_return: true });
            // `flowEnd(ConstructorDeclaration)`: the analyzer does not trim the
            // constructor at the last statement; the fragment goes to its end.
            let end = inf.program.unit(unit).ast.member(member).span.end;
            function_body(inf, &mut cx2, &ctor.body, AsyncModifier::None, ret, Some(end));
            // A generative constructor with `=> e`: `verifyExpressionFunctionBody`
            // (3.6.2 `return_type_verifier.dart:45-52`, `:134-200`) checks `e` against
            // the constructor's `returnType`, the class's type over its own type
            // parameters (`RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR`).
            // The primary constructor's `this => e` body is another error
            // (`PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY`), without the check.
            // (The unelaborated `this` part, in the extension type, keeps the
            // `primary_body_part` mark; in the class and the enum it is the elaborated k2.)
            let primary = ctor.primary_body_part
                || fe.class.and_then(|c| inf.program.class(c).decl).is_some_and(|d| match &inf.program.unit(d.unit).ast.decl(d.decl).kind {
                    ast::DeclKind::Class(k) => k.primary_constructor == Some(member),
                    ast::DeclKind::Enum(k) => k.primary_constructor == Some(member),
                    _ => false,
                });
            if !fe.factory
                && !primary
                && let FunctionBody::Expression(x) = ctor.body
                && let Some(c) = fe.class
                && let Some(tx) = inf.body_types.units[unit.0 as usize].get_type(x)
            {
                let tc = inf.class_this_type(c);
                let fc = FunctionCtx {
                    closure_imposed: None,
                    modifier: AsyncModifier::None,
                    the_return: Some(tc),
                    return_context: tc,
                    returned: Vec::new(),
                    empty_return: false, valueless_returns: Vec::new(),
                    returned_expressions: Vec::new(),
                    executable: inf.declared_executable(f),
                    legal_return: true,
                };
                verify_expression_return(inf, &cx2, &fc, x, tx);
            }
            // Factory: `atConstructorDeclaration`, from the return type to the end of
            // the name.
            // A factory with a modifier (`NON_SYNC_FACTORY`) is not checked: a
            // generator exits early and an `async` one has an illegal imposed type
            // (`checkForBodyMayCompleteNormally`, 3.6.2 `resolver.dart:542-563`).
            let with_modifier = match ctor.body {
                FunctionBody::Block(b) => {
                    let source = inf.program.unit(unit).source.as_bytes();
                    let mut i = inf.program.unit(unit).ast.stmt(b).span.start;
                    while i > 0 && source[i - 1].is_ascii_whitespace() {
                        i -= 1;
                    }
                    i > 0 && (source[i - 1] == b'*' || source[..i].ends_with(b"async"))
                }
                _ => false,
            };
            if fe.factory && matches!(ctor.body, FunctionBody::Block(_)) && cx2.flow.reachable && !with_modifier {
                let where_ = dartforge_diagnostics::Span { start: ctor.class_name.span.start, end: ctor.name.map_or(ctor.class_name.span.end, |n| n.span.end) };
                body_completes_normally(inf, Some(ret), Some(ret), AsyncModifier::None, where_);
            }
        }
        FunctionRef::None => {}
    }
}

/// A constructor initializer.
fn initializer(inf: &mut BodyInferrer<'_>, cx: &mut Body, cls: Option<ClassId>, init: &ast::Initializer, params: &[ast::Parameter], const_constructor: bool, current: FunctionElementId) {
    let u = inf.core.unknown;
    match init {
        ast::Initializer::Field { name, value, .. } => {
            let field = cls.and_then(|c| inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == name.sym));
            let t = field.map(|v| inf.variable_type(v)).unwrap_or(u);
            let tv = infer(inf, cx, *value, t);
            // `checkForFieldInitializerNotAssignable`
            // (`error_detection_helpers.dart:168-205`): at the expression; in a
            // `const` constructor, the `CONST_` variant.
            if field.is_some() {
                if inf.assignable(tv, t) {
                    if !matches!(inf.table.get(t), Type::Void) {
                        expr::void_use(inf, cx, *value, tv);
                    }
                } else {
                    use dartforge_diagnostics::codes::compile_time_error as c;
                    let code = if const_constructor { c::CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE } else { c::FIELD_INITIALIZER_NOT_ASSIGNABLE };
                    let sp = inf.span_expr(cx.unit, *value);
                    let since = inf.diagnostics.len();
                    inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Type_(tv), crate::exibicao::Arg::Type_(t)]);
                    inf.attach_non_promotion(since, cx, Some(*value), sp);
                }
            }
        }
        ast::Initializer::Super { span, constructor, arguments } => {
            super_initializer(inf, cx, cls, *span, *constructor, arguments, params);
        }
        ast::Initializer::Redirect { span, constructor, arguments } => {
            redirecting_initializer(inf, cls, *span, *constructor, current);
            let tgt = cls.map(|c| {
                let ps = inf.outline.classes[c.0 as usize].type_params.clone();
                let args: Vec<TypeId> = ps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                (c, args.into_boxed_slice())
            });
            call_constructor_of(inf, cx, tgt, *constructor, arguments, &[], *span);
        }
        ast::Initializer::Assert { condition, message, .. } => {
            expr::assert_condition(inf, cx, *condition);
            if let Some(m) = message {
                infer_free(inf, cx, *m);
            }
        }
    }
}

/// What the type name of a redirection target (`= X`, `= p.X`, `= X.n`,
/// `= X<T>.n`) designates, as `NamedTypeResolver` resolves it with
/// `redirectedConstructor_namedType`
/// (`an611:src/dart/resolver/named_type_resolver.dart:84-133`, `:230-312`).
enum TargetType {
    /// Class, enum, extension type or alias that expands to one of them.
    Classe(ClassId),
    /// A type without constructors (type parameter, `Never`, function type
    /// alias): the `staticElement` stays null and the type is the `{1}` of
    /// `redirect_to_missing_constructor`.
    NoConstructors(TypeId),
    /// `dynamic` (or an alias of it): nothing to report.
    Dinamico,
    /// Alias that expands to a type parameter (`_verifyTypeAliasForContext`).
    ParameterAlias,
    /// `reportNullOrNonTypeElement`: `redirect_to_non_class` with the name.
    NonClass(dartforge_intern::SymbolId),
    /// `p.X` with `p` neither a prefix nor a type.
    ShadowedPrefix,
    /// Undefined but silenced (`shouldIgnoreUndefinedNamedType`) or syntax that
    /// does not get to be a target.
    Nada,
}

/// The written target of a redirecting factory, already resolved.
struct WrittenTarget {
    ty: TargetType,
    /// The constructor's name (`None` = unnamed; `new` already becomes unnamed).
    constructor: Option<dartforge_intern::SymbolId>,
    /// `NamedType.qualifiedName` with the constructor's `.the_name`, as written (the
    /// `{0}` of `redirect_to_missing_constructor`).
    cited_name: String,
    /// `_getErrorRange`: from the prefix to the type name, without type
    /// arguments.
    type_range: dartforge_diagnostics::Span,
    /// The prefix's token (for `prefix_shadowed_by_local_declaration`).
    prefix: Option<ast::Name>,
}

/// The name `the_name` in the scope of the constructor of `cls` (its
/// parameters, the class's type parameters, its members and the unit's
/// scope), like the `nameScope` of `ResolutionVisitor.visitConstructorDeclaration`
/// (`an611:src/dart/resolver/resolution_visitor.dart:356-382`: the parameters
/// are defined before `_resolveRedirectedConstructor`).
enum InScope {
    Elemento(Element),
    TypeParam(TypeParamId),
    Prefixo,
    Builtin,
    /// Parameter, member or enum constant: not a type.
    Other,
    None_,
}

fn name_in_constructor_scope(inf: &BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, params: &[ast::Parameter], the_name: dartforge_intern::SymbolId) -> InScope {
    if params.iter().any(|p| p.name.is_some_and(|n| n.sym == the_name)) {
        return InScope::Other;
    }
    if let Some(c) = cls {
        if let Some(&p) = inf.outline.classes[c.0 as usize].type_params.iter().find(|&&p| inf.table.param(p).name == the_name) {
            return InScope::TypeParam(p);
        }
        let setter_key = inf.setter_key(the_name);
        if inf.lexical_declared_member(Some(c), None, the_name, setter_key).is_some() {
            return InScope::Other;
        }
        let ce = inf.program.class(c);
        if ce.enum_constants.iter().chain(ce.fields.iter()).any(|&v| inf.program.variable(v).name == the_name) {
            return InScope::Other;
        }
    }
    if let Some(b) = inf.program.lookup_na_unidade(unit, the_name)
        && let Some(el) = b.getter.or(b.setter)
    {
        if let Element::Prefix(..) = el {
            return InScope::Prefixo;
        }
        return InScope::Elemento(el);
    }
    if inf.program.prefixos_na_unidade(unit).contains_key(&the_name) {
        return InScope::Prefixo;
    }
    if matches!(inf.interner.resolve(the_name), "dynamic" | "Never") {
        return InScope::Builtin;
    }
    InScope::None_
}

/// The type that a type element designates (`_resolveToElement`).
fn element_type(inf: &mut BodyInferrer<'_>, el: Element, the_name: dartforge_intern::SymbolId) -> TargetType {
    match el {
        Element::Class(c) => TargetType::Classe(c),
        Element::Typedef(td) => {
            let tgt = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(tgt).clone() {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => TargetType::Classe(class),
                Type::TypeParameter { .. } => TargetType::ParameterAlias,
                _ if inf.e_dynamic(tgt) => TargetType::Dinamico,
                _ if inf.table.e_invalido(tgt) => TargetType::Nada,
                _ => TargetType::NoConstructors(tgt),
            }
        }
        _ => TargetType::NonClass(the_name),
    }
}

/// Resolves the written target of `= …` in the constructor's scope.
fn resolve_written_target(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, params: &[ast::Parameter], red: &ast::RedirectTarget) -> Option<WrittenTarget> {
    let (names, written): (Vec<ast::Name>, Option<ast::Name>) = {
        let node = inf.program.unit(unit).ast.ty(red.ty);
        let ast::TypeKind::Named { name, .. } = &node.kind else { return None };
        (name.to_vec(), red.constructor)
    };
    let without_new = |inf: &BodyInferrer<'_>, s: Option<dartforge_intern::SymbolId>| match s {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    // (prefix, type name, constructor name)
    let (prefix, typ, ctor): (Option<ast::Name>, ast::Name, Option<ast::Name>) = match (names.as_slice(), written) {
        ([a], c) => (None, *a, c),
        ([a, b], None) => {
            // `A.b`: the parser does not tell `prefix.Tipo` from
            // `Tipo.the_constructor`; the resolver rewrites to the second when the first
            // designates a class or alias.
            match name_in_constructor_scope(inf, unit, cls, params, a.sym) {
                InScope::Elemento(Element::Class(_) | Element::Typedef(_)) => (None, *a, Some(*b)),
                _ => (Some(*a), *b, None),
            }
        }
        ([a, b], Some(c)) => (Some(*a), *b, Some(c)),
        ([a, b, c], None) => (Some(*a), *b, Some(*c)),
        _ => return None,
    };
    let the_constructor = without_new(inf, ctor.map(|n| n.sym));
    let mut cited_name: String = match prefix {
        Some(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(typ.sym)),
        None => inf.interner.resolve(typ.sym).to_string(),
    };
    if let Some(c) = ctor {
        cited_name.push('.');
        cited_name.push_str(inf.interner.resolve(c.sym));
    }
    let type_range = dartforge_diagnostics::Span { start: prefix.map_or(typ.span.start, |p| p.span.start), end: typ.span.end };
    let target_type = match prefix {
        None => match name_in_constructor_scope(inf, unit, cls, params, typ.sym) {
            InScope::Elemento(el) => element_type(inf, el, typ.sym),
            InScope::TypeParam(p) => TargetType::NoConstructors(inf.table.intern(Type::TypeParameter { param: p, nullable: false })),
            InScope::Builtin if inf.interner.resolve(typ.sym) == "dynamic" => TargetType::Dinamico,
            InScope::Builtin => TargetType::NoConstructors(inf.core.never),
            InScope::Prefixo | InScope::Other => TargetType::NonClass(typ.sym),
            InScope::None_ => {
                if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, None, typ.sym) {
                    TargetType::Nada
                } else {
                    TargetType::NonClass(typ.sym)
                }
            }
        },
        Some(p) => match name_in_constructor_scope(inf, unit, cls, params, p.sym) {
            InScope::Prefixo => match inf.program.lookup_prefixed_na_unidade(unit, p.sym, typ.sym).and_then(|b| b.getter) {
                Some(el) => element_type(inf, el, typ.sym),
                None if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, Some(p.sym), typ.sym) => TargetType::Nada,
                None => TargetType::NonClass(typ.sym),
            },
            InScope::None_ => {
                if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, Some(p.sym), typ.sym) {
                    TargetType::Nada
                } else {
                    TargetType::NonClass(typ.sym)
                }
            }
            // `p.X.n` with `p` a class: syntax that does not get to be a target.
            InScope::Elemento(Element::Class(_) | Element::Typedef(_)) => TargetType::Nada,
            _ => TargetType::ShadowedPrefix,
        },
    };
    Some(WrittenTarget { ty: target_type, constructor: the_constructor, cited_name, type_range, prefix })
}

/// The constructor that a target designates (`lookUpConstructor`: with the
/// accessibility test for private ones).
#[derive(Clone, Copy)]
enum Construtor {
    Declared(FunctionElementId),
    /// The synthetic unnamed constructor of an abstract class or enum without a
    /// declared constructor (the model only creates the one of concrete
    /// classes), or the primary of an extension type.
    Implicit,
}

fn find_constructor(inf: &BodyInferrer<'_>, tgt: ClassId, the_name: Option<dartforge_intern::SymbolId>, lib: dartforge_elements::model::LibraryId) -> Option<Construtor> {
    let key = the_name.or(inf.sym.empty)?;
    if inf.interner.resolve(key).starts_with('_') && inf.program.class(tgt).library != lib {
        return None;
    }
    match inf.constructor_or_primary(tgt, key) {
        Some(Some(f)) => Some(Construtor::Declared(f)),
        Some(None) => Some(Construtor::Implicit),
        None => {
            if the_name.is_some() {
                return None;
            }
            // The synthetic one that the model does not create, also forwarded
            // through a mixin application.
            let mut c = tgt;
            loop {
                let ce = inf.program.class(c);
                match ce.kind {
                    ClassKind::MixinApplication => c = ce.supertype_class?,
                    ClassKind::Class | ClassKind::Enum if ce.constructors.is_empty() => return Some(Construtor::Implicit),
                    _ => return None,
                }
            }
        }
    }
}

/// The synthetic unnamed one that the model does not create (abstract class
/// without a declared constructor, also through mixin applications).
pub(crate) fn without_implicit_name(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let mut current = c;
    for _ in 0..16 {
        let ce = inf.program.class(current);
        match ce.kind {
            ClassKind::MixinApplication => match ce.supertype_class {
                Some(s) => current = s,
                None => return false,
            },
            ClassKind::Class | ClassKind::Enum => return ce.constructors.is_empty(),
            _ => return false,
        }
    }
    false
}

/// `isFactory`/`isConst` of a found constructor.
fn factory_and_const(inf: &BodyInferrer<'_>, tgt: ClassId, c: Construtor) -> (bool, bool) {
    match c {
        Construtor::Declared(f) => {
            let fe = inf.program.function(f);
            (fe.factory, fe.const_)
        }
        // An enum's synthetic one is `const`; a class's is not. The primary of
        // an extension type is `const` with `extension type const`.
        Construtor::Implicit => {
            let ce = inf.program.class(tgt);
            match ce.kind {
                ClassKind::Enum => (false, true),
                ClassKind::ExtensionType => {
                    let const_ = ce.decl.is_some_and(|d| matches!(&inf.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_));
                    (false, const_)
                }
                _ => (false, false),
            }
        }
    }
}

/// The function type of a constructor, with the return replaced by the
/// class's `this` type (for a constructor forwarded from a mixin application,
/// the application's).
fn constructor_type(inf: &mut BodyInferrer<'_>, tgt: ClassId, c: Construtor) -> TypeId {
    let this = inf.class_this_type(tgt);
    let sig = match c {
        Construtor::Declared(f) if !matches!(inf.program.function(f).node, FunctionRef::None) => inf.outline.functions[f.0 as usize].signature,
        Construtor::Implicit if inf.program.class(tgt).kind == ClassKind::ExtensionType => inf.primary_signature(tgt),
        _ => inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: this,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        }),
    };
    // The class's type parameters stay free (`ConstructorElement.type` has
    // no type parameters of its own).
    match inf.table.get(sig).clone() {
        Type::Function { positional, optional, named, nullable, .. } => {
            inf.table.intern(Type::Function { type_params: Box::new([]), ret: this, positional, optional, named, nullable })
        }
        _ => sig,
    }
}

/// `redirectedConstructor?.declaration` of a declared constructor: the
/// target of `= …` or of the `this(...)` (the first one), when it is a
/// declared constructor.
pub(crate) fn redirected_constructor(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> Option<FunctionElementId> {
    let fe = inf.program.function(f);
    let FunctionRef::Constructor { unit, member } = fe.node else { return None };
    let cls = fe.class;
    let lib = fe.library;
    let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return None };
    if let Some(red) = &ctor.redirect {
        let tgt = resolve_written_target(inf, unit, cls, &ctor.parameters, red)?;
        let TargetType::Classe(c) = tgt.ty else { return None };
        return match find_constructor(inf, c, tgt.constructor, lib)? {
            Construtor::Declared(g) => Some(g),
            Construtor::Implicit => None,
        };
    }
    if ctor.factory || is_primary_constructor(inf.program, f) {
        return None;
    }
    let the_name = ctor.initializers.iter().find_map(|i| match i {
        ast::Initializer::Redirect { constructor, .. } => Some(*constructor),
        _ => None,
    })?;
    let c = cls?;
    let key = match the_name.map(|n| n.sym) {
        Some(s) if Some(s) == inf.sym.new_ => inf.sym.empty,
        Some(s) => Some(s),
        None => inf.sym.empty,
    }?;
    inf.constructor_of(c, key)
}

/// The parameters of the primary constructor of `f`'s declaration, when `f`
/// is a `this` part that did not become the k2: the second part onwards of a
/// class and enum, and the extension type's part (which is not elaborated).
/// In 3.13.4 every `PrimaryConstructorBody` resolves in the primary's scope
/// (`scope_context.dart:406-430`: `ConstructorInitializerScope` in the
/// initializers, `PrimaryParameterScope` in the body), with the parameters
/// visible. Yields (name, type, final, `this.`/`super.`/declaring).
fn the_primary_parameters(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> Vec<(ast::Name, TypeId, bool, bool)> {
    let program = inf.program;
    let Some(c) = program.function(f).class else { return Vec::new() };
    let Some(d) = program.class(c).decl else { return Vec::new() };
    let ast = &program.unit(d.unit).ast;
    let k2 = match &ast.decl(d.decl).kind {
        ast::DeclKind::Class(x) => x.primary_constructor,
        ast::DeclKind::Enum(x) => x.primary_constructor,
        ast::DeclKind::ExtensionType(x) => {
            let n = x.representation_name;
            let field = program.class(c).fields.iter().copied().find(|&v| program.variable(v).name == n.sym && !program.variable(v).static_);
            let t = match field {
                Some(v) => inf.variable_type(v),
                None => inf.core.dynamic_,
            };
            return vec![(n, t, true, true)];
        }
        _ => None,
    };
    let Some(k2) = k2 else { return Vec::new() };
    let Some(g) = program
        .class(c)
        .constructors
        .values()
        .copied()
        .find(|&g| matches!(program.function(g).node, FunctionRef::Constructor { unit, member } if unit == d.unit && member == k2))
    else {
        return Vec::new();
    };
    if g == f {
        return Vec::new();
    }
    let ast::MemberKind::Constructor(kc) = &ast.member(k2).kind else { return Vec::new() };
    let types: Vec<TypeId> = inf.outline.functions[g.0 as usize].parameters.iter().map(|p| p.ty).collect();
    kc.parameters
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let n = p.name?;
            let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
            Some((n, t, p.final_ || p.this_ || p.super_, p.this_ || p.super_))
        })
        .collect()
}

/// `f` is the (elaborated) primary constructor of a class or enum: its
/// redirection is `PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` and does not enter
/// the `RECURSIVE_CONSTRUCTOR_REDIRECT` chain.
fn is_primary_constructor(program: &dartforge_elements::model::Program, f: FunctionElementId) -> bool {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return false };
    program.unit(unit).ast.decls.iter().any(|d| match &d.kind {
        ast::DeclKind::Class(x) => x.primary_constructor == Some(member),
        ast::DeclKind::Enum(x) => x.primary_constructor == Some(member),
        _ => false,
    })
}

/// `_hasRedirectingFactoryConstructorCycle`
/// (`an611:src/generated/error_verifier.dart:6327-6338`): `f` comes back to
/// itself through the chain of redirections (a cycle ahead of it does not
/// count).
pub(crate) fn redirects_in_cycle(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(f);
    while let Some(g) = current {
        if !seen.insert(g) {
            return g == f;
        }
        current = redirected_constructor(inf, g);
    }
    false
}

/// The `visitConstructorDeclaration` rules for the redirecting factory
/// `= tgt` (`an611:src/generated/error_verifier.dart:589-605`), in the
/// analyzer's order:
///
/// 1. `_checkForRedirectingConstructorErrorCodes` (`:5094-5137`):
///    `redirect_to_non_const_constructor`,
///    `redirect_to_abstract_class_constructor` and
///    `invalid_reference_to_generative_enum_constructor`;
/// 2. `_checkForRecursiveFactoryRedirect` (`:5073-5089`) or, without a cycle,
///    `_checkForAllRedirectConstructorErrorCodes` (`:2025-2075`):
///    `redirect_to_missing_constructor`, `redirect_to_invalid_return_type` and
///    `redirect_to_invalid_function_type`.
///
/// Before that, those from resolving the target's type
/// (`redirect_to_non_class`,
/// `redirect_to_type_alias_expands_to_type_parameter`,
/// `prefix_shadowed_by_local_declaration`).
fn redirecting_factory_target(inf: &mut BodyInferrer<'_>, cx: &mut Body, red: &ast::RedirectTarget, f_current: FunctionElementId, ctor: &ast::Constructor) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    let fe = inf.program.function(f_current);
    let (owner, lib, current_const) = (fe.class, fe.library, fe.const_);
    let Some(tgt) = resolve_written_target(inf, cx.unit, owner, &ctor.parameters, red) else { return };
    let cls = match tgt.ty {
        TargetType::Classe(c) => c,
        TargetType::NonClass(the_name) => {
            let text = inf.interner.resolve(the_name).to_string();
            inf.warning_with_code(ce::REDIRECT_TO_NON_CLASS, tgt.type_range, &[&text]);
            return;
        }
        TargetType::ParameterAlias => {
            inf.warning_with_code(ce::REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, tgt.type_range, &[]);
            return;
        }
        TargetType::ShadowedPrefix => {
            if let Some(p) = tgt.prefix {
                let text = inf.interner.resolve(p.sym).to_string();
                inf.warning_with_code(ce::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, p.span, &[&text]);
            }
            return;
        }
        TargetType::NoConstructors(t) => {
            if !redirects_in_cycle(inf, f_current) {
                let displayed = inf.table.format(t, inf.interner, inf.program);
                inf.warning_with_code(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&tgt.cited_name, &displayed]);
            }
            return;
        }
        TargetType::Dinamico | TargetType::Nada => return,
    };
    // The target's written type arguments (`= B<T>.the_name`), in the
    // constructor's scope: the `NamedType.type` that `TypeArgumentsVerifier`
    // reads.
    let written_ones: Vec<ast::TypeId> = match &inf.program.unit(cx.unit).ast.ty(red.ty).kind {
        ast::TypeKind::Named { args, .. } => args.to_vec(),
        _ => Vec::new(),
    };
    let written_args: Vec<TypeId> = written_ones.iter().map(|&x| inf.type_argument_type(cx, x)).collect();
    let n_params = inf.outline.classes[cls.0 as usize].type_params.len();
    if !written_ones.is_empty() && written_ones.len() != n_params {
        let text = inf.interner.resolve(inf.program.class(cls).name).to_string();
        let sp = inf.program.unit(cx.unit).ast.ty(red.ty).span;
        inf.report_type_arguments(&text, n_params, written_ones.len(), sp);
    }
    let finding = find_constructor(inf, cls, tgt.constructor, lib);
    // 1. `_checkForRedirectingConstructorErrorCodes`.
    if let Some(c) = finding {
        let (factory, const_target) = factory_and_const(inf, cls, c);
        if current_const && !const_target {
            inf.warning_with_code(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, red.span, &[]);
        }
        let ce_target = inf.program.class(cls);
        if matches!(ce_target.kind, ClassKind::Class | ClassKind::MixinApplication) && ce_target.modifiers.abstract_ && !factory {
            let mut the_name = owner.map(|d| inf.interner.resolve(inf.program.class(d).name).to_string()).unwrap_or_default();
            if let Some(n) = ctor.name {
                the_name.push('.');
                the_name.push_str(inf.interner.resolve(n.sym));
            }
            let target_class = inf.interner.resolve(ce_target.name).to_string();
            inf.warning_with_code(ce::REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR, red.span, &[&the_name, &target_class]);
        }
        // `_checkForInvalidGenerativeConstructorReference`.
        if inf.program.class(cls).kind == ClassKind::Enum && !factory {
            inf.warning(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), red.span);
        }
    }
    // 2. Cycle, or the target's rules.
    if finding.is_some_and(|c| matches!(c, Construtor::Declared(_))) && redirects_in_cycle(inf, f_current) {
        inf.warning_with_code(ce::RECURSIVE_FACTORY_REDIRECT, red.span, &[]);
        return;
    }
    let Some(c) = finding else {
        // `{1}`: the target's type as a `DartType` (that of the class
        // instantiated with its own parameters, which is what inference gives
        // without written arguments).
        let typ = inf.class_this_type(cls);
        let displayed = inf.table.format(typ, inf.interner, inf.program);
        inf.warning_with_code(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&tgt.cited_name, &displayed]);
        return;
    };
    let Some(owner) = owner else { return };
    // The target's type (`NamedTypeResolver._instantiateElement`, 3.6.2
    // `named_type_resolver.dart:202-262`): with written arguments, the class
    // instantiated with them (`dynamic` for all if the count is wrong);
    // without them, `_inferRedirectedConstructor` (`:172-200`): the class
    // itself or one without type parameters takes the `thisType`; otherwise
    // the arguments come from inferring `C<X…> <: Dona<T…>` (the owner's
    // `thisType` as context), through the final types. A written alias
    // (`= Alias.the_name`) does not infer: it is the alias's instantiation (with
    // the written arguments, or by the bounds, `instantiateTypeAliasToBounds`),
    // the type of the annotation itself (its reports already came out when
    // resolving the target).
    let target_params = inf.outline.classes[cls.0 as usize].type_params.clone();
    let written_via_alias = match &inf.program.unit(cx.unit).ast.ty(red.ty).kind {
        ast::TypeKind::Named { name, .. } => {
            let binding = if name.len() == 2 {
                inf.program.lookup_prefixed_na_unidade(cx.unit, name[0].sym, name[1].sym)
            } else {
                name.first().and_then(|n| inf.program.lookup_na_unidade(cx.unit, n.sym))
            };
            matches!(binding.and_then(|b| b.getter), Some(Element::Typedef(_)))
        }
        _ => false,
    };
    let the_target_args: Vec<TypeId> = if written_via_alias && !target_params.is_empty() {
        let (n_diag, n_units) = (inf.diagnostics.len(), inf.warning_units.len());
        let t = inf.type_from_annotation(cx, red.ty);
        inf.diagnostics.truncate(n_diag);
        inf.warning_units.truncate(n_units);
        match inf.table.get(t).clone() {
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } if class == cls => args.to_vec(),
            _ => return,
        }
    } else if target_params.is_empty() || (written_ones.is_empty() && cls == owner) {
        target_params.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect()
    } else if !written_ones.is_empty() {
        if written_args.len() == target_params.len() {
            written_args
        } else {
            vec![inf.core.dynamic_; target_params.len()]
        }
    } else {
        let this_target = inf.class_this_type(cls);
        let owner_this = inf.class_this_type(owner);
        let mut gi = crate::constraints::GenericInferrer::new(&target_params);
        let mut env = inf.env();
        gi.constrain_return(this_target, owner_this, &mut env);
        let r = gi.choose_final(&mut env);
        drop(env);
        r
    };
    let target_map = inf.map(&target_params, &the_target_args);
    let of = constructor_type(inf, cls, c);
    let of = inf.subst(of, &target_map);
    let for_ = constructor_type(inf, owner, Construtor::Declared(f_current));
    let (Type::Function { ret: ret_of, .. }, Type::Function { ret: ret_for, .. }) = (inf.table.get(of).clone(), inf.table.get(for_).clone()) else { return };
    if !inf.assignable(ret_of, ret_for) {
        let (a, b) = (inf.table.format(ret_of, inf.interner, inf.program), inf.table.format(ret_for, inf.interner, inf.program));
        inf.warning_with_code(ce::REDIRECT_TO_INVALID_RETURN_TYPE, red.span, &[&a, &b]);
    } else if !inf.sub(of, for_) {
        let (a, b) = (inf.table.format(of, inf.interner, inf.program), inf.table.format(for_, inf.interner, inf.program));
        inf.warning_with_code(ce::REDIRECT_TO_INVALID_FUNCTION_TYPE, red.span, &[&a, &b]);
    }
}

/// The analyzer's `enclosingClass.supertype` for a class
/// (`an611:src/summary2/types_builder.dart:117-127` and
/// `library_builder.dart:732-743`): the written `extends` when it is a class
/// (not an enum, extension type, mixin, `Function`, `Null` or nullable);
/// otherwise `Object`. Nothing for `Object` itself, for enums (the supertype
/// is `Enum`, whose synthetic unnamed constructor is never missing) and for
/// what is not a class. With mixins the supertype is still the `extends`'s
/// (the analyzer does not desugar the application).
fn analyzer_supertype(inf: &mut BodyInferrer<'_>, c: ClassId) -> Option<(ClassId, TypeId)> {
    if inf.program.class(c).kind != ClassKind::Class {
        return None;
    }
    let object = inf.core.object_class?;
    if c == object {
        return None;
    }
    if let Some(t) = inf.outline.classes[c.0 as usize].supertype
        && let Type::Interface { class: s, nullable: false, .. } = inf.table.get(t).clone()
        && matches!(inf.program.class(s).kind, ClassKind::Class | ClassKind::MixinApplication)
        && Some(s) != inf.core.function_class
        && Some(s) != inf.core.null_class
    {
        return Some((s, t));
    }
    Some((object, inf.core.object))
}

/// `shouldIgnoreUndefinedNamedType` of the written `extends` of `c`.
fn ignored_extends(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let Some((u, t)) = inf.program.class(c).supertype else { return false };
    let ast::TypeKind::Named { name, .. } = &inf.program.unit(u).ast.ty(t).kind else { return false };
    let (prefix, the_name) = match name.as_ref() {
        [a] => (None, a.sym),
        [p, a] => (Some(p.sym), a.sym),
        _ => return false,
    };
    crate::scope::should_ignore_undefined(inf.program, inf.interner, u, prefix, the_name)
}

/// `constructors.every((c) => c.isFactory)` of class `s` (the synthetic one
/// is generative; those of a mixin application are the generative ones
/// forwarded from the base, and an empty list passes).
fn only_factories(inf: &BodyInferrer<'_>, s: ClassId) -> bool {
    let ce = inf.program.class(s);
    if ce.kind == ClassKind::MixinApplication {
        let mut base = ce.supertype_class;
        while let Some(b) = base {
            let be = inf.program.class(b);
            if be.kind != ClassKind::MixinApplication {
                return !(be.constructors.is_empty() || be.constructors.values().any(|&f| !inf.program.function(f).factory));
            }
            base = be.supertype_class;
        }
        return true;
    }
    !ce.constructors.is_empty() && ce.constructors.values().all(|&f| inf.program.function(f).factory)
}

/// The parameters of a found constructor: the AST node (with the unit) and
/// the outline types, in the same order.
fn constructor_parameters<'a>(inf: &BodyInferrer<'a>, k: Construtor) -> Option<(UnitId, &'a ast::Constructor, Vec<crate::resolve::ParameterTypeData>)> {
    let Construtor::Declared(f) = k else { return None };
    let the_program: &'a dartforge_elements::model::Program = inf.program;
    let FunctionRef::Constructor { unit, member } = the_program.function(f).node else { return None };
    let ast::MemberKind::Constructor(ctor) = &the_program.unit(unit).ast.member(member).kind else { return None };
    Some((unit, ctor, inf.outline.functions[f.0 as usize].parameters.to_vec()))
}

/// The required positionals and the names of the `required` named ones.
fn constructor_required(inf: &BodyInferrer<'_>, k: Construtor) -> (usize, Vec<dartforge_intern::SymbolId>) {
    let Some((_, ctor, _)) = constructor_parameters(inf, k) else { return (0, Vec::new()) };
    let positional_args = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
    let named_args = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named && p.required).filter_map(|p| p.external_name().map(|n| n.sym)).collect();
    (positional_args, named_args)
}

/// `ConstructorElement.getDisplayString()`
/// (`an611:src/dart/element/display_string_builder.dart:78-93`, `:405-461`,
/// `:531-553`): `Tipo Classe.the_name(int a, [int b = 0], {required int c})`, with
/// the constructor's type and parameters as seen by the type `typ` (the
/// substituted `ConstructorMember`).
/// `without_return`: the element's display in 3.13.4 does not include the return
/// type (`'A.named()'`, not `'A A.named()'`).
fn display_constructor(inf: &mut BodyInferrer<'_>, tgt: ClassId, k: Construtor, typ: TypeId, without_return: bool) -> String {
    let mut s = String::new();
    if !without_return {
        s.push_str(&inf.table.format(typ, inf.interner, inf.program));
        s.push(' ');
    }
    s.push_str(inf.interner.resolve(inf.program.class(tgt).name));
    if let Construtor::Declared(f) = k {
        let the_name = inf.program.function(f).name;
        if Some(the_name) != inf.sym.empty {
            s.push('.');
            s.push_str(inf.interner.resolve(the_name));
        }
    }
    s.push('(');
    let map = match inf.table.get(typ).clone() {
        Type::Interface { args, .. } => {
            let ps = inf.outline.classes[tgt.0 as usize].type_params.clone();
            (ps.len() == args.len()).then(|| inf.map(&ps, &args))
        }
        _ => None,
    };
    if let Some((unit, ctor, types)) = constructor_parameters(inf, k) {
        let the_program: &dartforge_elements::model::Program = inf.program;
        let source = &the_program.unit(unit).source;
        let mut pieces: Vec<(ast::ParameterKind, String)> = Vec::new();
        for (i, p) in ctor.parameters.iter().enumerate() {
            let mut t = types.get(i).map_or(inf.core.dynamic_, |d| d.ty);
            if let Some(m) = &map {
                t = inf.subst(t, m);
            }
            let mut text = String::new();
            if p.kind == ast::ParameterKind::Named && p.required {
                text.push_str("required ");
            }
            text.push_str(&inf.table.format(t, inf.interner, inf.program));
            text.push(' ');
            if let Some(n) = p.external_name() {
                text.push_str(inf.interner.resolve(n.sym));
            }
            if let Some(d) = p.default_value {
                let sp = the_program.unit(unit).ast.expr(d).span;
                text.push_str(" = ");
                text.push_str(&source[sp.start..sp.end]);
            }
            pieces.push((p.kind, text));
        }
        let mut last: Option<ast::ParameterKind> = None;
        let mut closure = "";
        for (i, (type_p, text)) in pieces.iter().enumerate() {
            if i != 0 {
                s.push_str(", ");
            }
            if last != Some(*type_p) {
                s.push_str(closure);
                let (open, closes) = match type_p {
                    ast::ParameterKind::Required => ("", ""),
                    ast::ParameterKind::Optional => ("[", "]"),
                    ast::ParameterKind::Named => ("{", "}"),
                };
                s.push_str(open);
                closure = closes;
                last = Some(*type_p);
            }
            s.push_str(text);
        }
        s.push_str(closure);
    }
    s.push(')');
    s
}

/// `super(...)`/`super.n(...)`: `ElementResolver.visitSuperConstructorInvocation`
/// (`an611:src/generated/element_resolver.dart:338-404`):
/// `undefined_constructor_in_initializer(_default)` without the constructor (or a
/// private one from another library) and `non_generative_constructor` with a
/// factory in a superclass that has some generative one; then the arguments
/// against the constructor (except with the `extends` silenced by
/// `shouldIgnoreUndefinedNamedType`).
fn super_initializer(inf: &mut BodyInferrer<'_>, cx: &mut Body, cls: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, arguments: &ast::Arguments, params: &[ast::Parameter]) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    let sup = cls.and_then(|c| analyzer_supertype(inf, c).map(|s| (c, s)));
    let Some((c, (s, st))) = sup else {
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    };
    let key = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    let lib = inf.program.class(c).library;
    let Some(k) = find_constructor(inf, s, key, lib) else {
        let displayed = inf.table.format(st, inf.interner, inf.program);
        match constructor {
            Some(n) => {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER, span, &[&displayed, &the_name]);
            }
            None => inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, span, &[&displayed]),
        }
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    };
    if factory_and_const(inf, s, k).0 && !only_factories(inf, s) {
        let v313 = inf.program.referencia(cx.unit) == dartforge_diagnostics::AnalyzerReference::V3_13;
        let displayed = display_constructor(inf, s, k, st, v313);
        inf.warning_with_code(ce::NON_GENERATIVE_CONSTRUCTOR, span, &[&displayed]);
    }
    if ignored_extends(inf, c) {
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    }
    let tgt = match inf.table.get(st).clone() {
        Type::Interface { class, args, .. } => Some((class, args)),
        _ => None,
    };
    call_constructor_of(inf, cx, tgt, constructor, arguments, params, span);
}

/// `this(...)`/`this.n(...)` (`_checkForConflictingInitializerErrorCodes`,
/// `an611:src/generated/error_verifier.dart:2705-2733`): outside a `factory`,
/// `redirect_generative_to_missing_constructor` without the target (looked up
/// only in the class itself) and
/// `redirect_generative_to_non_generative_constructor` with a factory; and
/// `redirect_to_non_const_constructor` at the name (or at the `this`) when the
/// current one is `const` and the target is not.
///
/// In the body of the primary constructor, 3.13.4 only reports
/// `PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` (the parser) and exits
/// (`_checkForConflictingPrimaryConstructorInitializers`,
/// `error_verifier.dart:3984-3999`): none of these.
fn redirecting_initializer(inf: &mut BodyInferrer<'_>, cls: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, current: FunctionElementId) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    let Some(c) = cls else { return };
    if is_primary_constructor(inf.program, current) {
        return;
    }
    let key = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => inf.sym.empty,
        Some(x) => Some(x),
        None => inf.sym.empty,
    };
    let finding = key.and_then(|k| inf.constructor_or_primary(c, k)).map(|f| match f {
        Some(f) => Construtor::Declared(f),
        None => Construtor::Implicit,
    });
    let fe = inf.program.function(current);
    let (current_factory, current_const) = (fe.factory, fe.const_);
    if !current_factory {
        match finding {
            None => {
                let class_name = inf.interner.resolve(inf.program.class(c).name).to_string();
                let mut the_name = class_name.clone();
                if let Some(n) = constructor {
                    the_name.push('.');
                    the_name.push_str(inf.interner.resolve(n.sym));
                }
                inf.warning_with_code(ce::REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR, span, &[&the_name, &class_name]);
            }
            Some(k) if factory_and_const(inf, c, k).0 => {
                inf.warning_with_code(ce::REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR, span, &[]);
            }
            Some(_) => {}
        }
    }
    if let Some(k) = finding
        && current_const
        && !factory_and_const(inf, c, k).1
    {
        let sp = constructor.map_or(dartforge_diagnostics::Span { start: span.start, end: span.start + 4 }, |n| n.span);
        inf.warning_with_code(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, sp, &[]);
    }
}

/// `SuperConstructorResolver._constructor`
/// (`an611:src/summary2/super_constructor_resolver.dart:28-62`): the
/// `superConstructor` is the supertype's constructor with the name of the
/// `super(...)` (or the unnamed one), compared literally (`super.new()` does
/// not find it), without an accessibility test and including factories; only
/// for classes (an enum has none).
/// The superclass chain of `c` comes back to `c` (inheritance cycle).
fn in_superclass_cycle(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let mut current = inf.program.class(c).supertype_class;
    for _ in 0..64 {
        match current {
            Some(k) if k == c => return true,
            Some(k) => current = inf.program.class(k).supertype_class,
            None => return false,
        }
    }
    false
}

fn super_constructor(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor) -> Option<(ClassId, TypeId, Construtor)> {
    let the_name = ctor
        .initializers
        .iter()
        .find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        })
        .flatten();
    let (s, st) = analyzer_supertype(inf, c)?;
    let key = the_name.or(inf.sym.empty)?;
    if Some(key) == inf.sym.new_ {
        return None;
    }
    let mut current = s;
    let mut passed_on = false;
    for _ in 0..16 {
        let ce = inf.program.class(current);
        if ce.kind == ClassKind::MixinApplication {
            current = ce.supertype_class?;
            passed_on = true;
            continue;
        }
        if let Some(&f) = ce.constructors.get(&key) {
            if passed_on && inf.program.function(f).factory {
                return None;
            }
            if matches!(inf.program.function(f).node, FunctionRef::None) {
                return Some((s, st, Construtor::Implicit));
            }
            return Some((s, st, Construtor::Declared(f)));
        }
        return (Some(key) == inf.sym.empty && ce.constructors.is_empty() && ce.kind == ClassKind::Class).then_some((s, st, Construtor::Implicit));
    }
    None
}

/// `ErrorVerifier.visitSuperFormalParameter`
/// (`an611:src/generated/error_verifier.dart:1457-1503`) on the `super.x` of a
/// generative constructor that is neither redirecting nor `external` (the
/// other places are `invalid_super_formal_parameter_location`, in `analise`;
/// in an extension type, `extension_type_constructor_with_super_formal_parameter`):
/// without the associated parameter (a positional one by its index among the
/// `super.` ones, a named one by name) `super_formal_parameter_without_associated_*`;
/// with it and the written type outside its type,
/// `super_formal_parameter_type_is_not_subtype_of_associated`. At the name.
fn super_parameters(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor, types: &[TypeId]) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    if ctor.factory || ctor.external || ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return;
    }
    if inf.program.class(c).kind == ClassKind::ExtensionType || !ctor.parameters.iter().any(|p| p.super_) {
        return;
    }
    // A superclass in an inheritance cycle is cut (it becomes `Object`), and an
    // enum does not get to a constructor with parameters: in both, the
    // associated constructor has no positionals, and each positional `super.x`
    // reports.
    let cycle = in_superclass_cycle(inf, c);
    let without_parameters = cycle || inf.program.class(c).kind == ClassKind::Enum;
    let associated = if cycle { None } else { super_constructor(inf, c, ctor) };
    // In 3.13.4 (`error_verifier.dart:2391-2408` of the main checkout), without
    // the `superConstructor` the check stops; in 3.6.2 each `super.x` reports.
    let unit_id = inf.program.class(c).decl.map(|d| d.unit);
    let v313 = unit_id.is_some_and(|u| inf.program.referencia(u) == dartforge_diagnostics::AnalyzerReference::V3_13);
    if associated.is_none() && v313 && !without_parameters {
        return;
    }
    // The parameters of the associated constructor, as seen by the supertype.
    let (positional_args, named_args): (Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId)>) = match associated {
        Some((s, st, k)) => {
            let map = match inf.table.get(st).clone() {
                Type::Interface { args, .. } => {
                    let ps = inf.outline.classes[s.0 as usize].type_params.clone();
                    (ps.len() == args.len()).then(|| inf.map(&ps, &args))
                }
                _ => None,
            };
            let mut pos = Vec::new();
            let mut the_named = Vec::new();
            if let Some((_, k_ast, data)) = constructor_parameters(inf, k) {
                for (i, p) in k_ast.parameters.iter().enumerate() {
                    let mut t = data.get(i).map_or(inf.core.dynamic_, |d| d.ty);
                    if let Some(m) = &map {
                        t = inf.subst(t, m);
                    }
                    match p.kind {
                        ast::ParameterKind::Named => {
                            if let Some(n) = p.external_name() {
                                the_named.push((n.sym, t));
                            }
                        }
                        _ => pos.push(t),
                    }
                }
            }
            (pos, the_named)
        }
        None => (Vec::new(), Vec::new()),
    };
    // The positional's index is the element's `indexIn` (`element.dart:9318-9323`):
    // in 3.6.2, two same-named `super._` are the same element by location, and the
    // second gets the first's index.
    let super_names: Vec<Option<dartforge_intern::SymbolId>> =
        ctor.parameters.iter().filter(|p| p.super_).map(|p| p.name.map(|n| n.sym)).collect();
    let mut idx = 0usize;
    for (i, p) in ctor.parameters.iter().enumerate() {
        if !p.super_ {
            continue;
        }
        let element_index = match p.name {
            Some(n) if !v313 => super_names.iter().position(|s| *s == Some(n.sym)).unwrap_or(idx),
            _ => idx,
        };
        let Some(n) = p.name else {
            idx += 1;
            continue;
        };
        let tgt = if p.kind == ast::ParameterKind::Named {
            let external = p.external_name().map_or(n.sym, |x| x.sym);
            named_args.iter().find(|(m, _)| *m == external).map(|(_, t)| *t)
        } else {
            positional_args.get(element_index).copied()
        };
        idx += 1;
        let Some(associated_type) = tgt else {
            let code = if p.kind == ast::ParameterKind::Named { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED } else { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL };
            inf.warning_with_code(code, n.span, &[]);
            continue;
        };
        // Without a written type the type is the inherited one: it never fires.
        if p.ty.is_none() && p.function_parameters.is_none() {
            continue;
        }
        let Some(&own) = types.get(i) else { continue };
        if !inf.sub(own, associated_type) {
            inf.warning_with_args(
                ce::SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED,
                n.span,
                &[crate::exibicao::Arg::Type_(own), crate::exibicao::Arg::Type_(associated_type)],
            );
        }
    }
}

/// The `errorRange` of `_checkForUndefinedConstructorInInitializerImplicit`. In
/// 3.6.2, the class name (`returnType`). In 3.13.4 (`error_verifier.dart:2124-2129`,
/// `:7997-8002` of the main checkout): an ordinary constructor goes from the type
/// name (or the `new`) to its name (`ConstructorDeclaration.errorRange`); the
/// primary, the `this` of the body part when there is one, otherwise from the first
/// token (the `const`, if written) to the constructor name
/// (`PrimaryConstructorDeclaration.errorRange`).
fn implicit_super_range(inf: &BodyInferrer<'_>, ctor: &ast::Constructor, unit: UnitId, member: ast::MemberId) -> dartforge_diagnostics::Span {
    use dartforge_diagnostics::Span;
    if inf.program.referencia(unit) != dartforge_diagnostics::AnalyzerReference::V3_13 {
        return ctor.class_name.span;
    }
    let u = inf.program.unit(unit);
    let primary = u.ast.decls.iter().any(|d| match &d.kind {
        ast::DeclKind::Class(x) => x.primary_constructor == Some(member),
        ast::DeclKind::Enum(x) => x.primary_constructor == Some(member),
        _ => false,
    });
    if !primary {
        let end = ctor.name.map_or(ctor.class_name.span.end, |n| n.span.end);
        return Span { start: ctor.class_name.span.start, end: end };
    }
    let ms = u.ast.member(member).span;
    if ms.start != ctor.class_name.span.start && u.source[ms.start..].starts_with("this") {
        return Span { start: ms.start, end: ms.start + 4 };
    }
    let start = if ctor.const_ {
        let before = u.source[..ctor.class_name.span.start].trim_end();
        if before.ends_with("const") { before.len() - "const".len() } else { ctor.class_name.span.start }
    } else {
        ctor.class_name.span.start
    };
    let end = match ctor.name {
        Some(n) => n.span.end,
        None if start < ctor.class_name.span.start => start + "const".len(),
        None => ctor.class_name.span.end,
    };
    Span { start: start, end: end }
}

/// `_checkForUndefinedConstructorInInitializerImplicit`
/// (`an611:src/generated/error_verifier.dart:5444-5551`): a generative
/// constructor, not `external`, without `super(...)` or `this(...)`, in a
/// class whose superclass has some generative one: its unnamed one is
/// missing (`undefined_constructor_in_initializer_default`), is a factory
/// (`non_generative_constructor`) or requires more arguments than the
/// `super.x` give (`implicit_super_initializer_missing_arguments`; without
/// `super-parameters`, `no_default_super_constructor`). At the class name
/// (up to the constructor name in the last two).
fn implicit_super(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor, unit: UnitId, member: ast::MemberId) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    if ctor.factory || ctor.external {
        return;
    }
    if ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Super { .. } | ast::Initializer::Redirect { .. })) {
        return;
    }
    let Some((s, st)) = analyzer_supertype(inf, c) else { return };
    if only_factories(inf, s) {
        return;
    }
    let typ = implicit_super_range(inf, ctor, unit, member);
    let lib = inf.program.class(c).library;
    // `superElement.unnamedConstructor`: without the accessibility test.
    let Some(k) = find_constructor(inf, s, None, inf.program.class(s).library) else {
        let the_name = inf.interner.resolve(inf.program.class(s).name).to_string();
        inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, typ, &[&the_name]);
        return;
    };
    if factory_and_const(inf, s, k).0 {
        // `{0}` = the element: `superElement.unnamedConstructor`, without
        // substitution (the type is the class's over its own parameters).
        let own = inf.class_this_type(s);
        let v313 = inf.program.referencia(unit) == dartforge_diagnostics::AnalyzerReference::V3_13;
        let displayed = display_constructor(inf, s, k, own, v313);
        inf.warning_with_code(ce::NON_GENERATIVE_CONSTRUCTOR, typ, &[&displayed]);
        return;
    }
    let (positional_args, mut named_args) = constructor_required(inf, k);
    let range = dartforge_diagnostics::Span { start: typ.start, end: ctor.name.map_or(typ.end, |n| n.span.end) };
    let displayed = inf.table.format(st, inf.interner, inf.program);
    let super_parameters_of = inf.program.library(lib).features.version() >= dartforge_frontend::features::LanguageVersion::new(2, 17);
    if !super_parameters_of {
        if positional_args != 0 || !named_args.is_empty() {
            inf.warning_with_code(ce::NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT, range, &[&displayed]);
        }
        return;
    }
    let super_positional_args = ctor.parameters.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
    let super_named_args: Vec<dartforge_intern::SymbolId> = ctor.parameters.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.external_name().map(|n| n.sym)).collect();
    named_args.retain(|n| !super_named_args.contains(n));
    if positional_args > super_positional_args || !named_args.is_empty() {
        inf.warning_with_code(ce::IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS, range, &[&displayed]);
    }
}

/// `super(...)`/`this(...)`: invokes the constructor of the instantiated
/// class. The current constructor's `super.x` parameters are implicit
/// arguments (positional ones after the explicit ones; named ones by name).
fn call_constructor_of(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: Option<(ClassId, Box<[TypeId]>)>, the_name: Option<ast::Name>, args: &ast::Arguments, params: &[ast::Parameter], span: dartforge_diagnostics::Span) {
    let u = inf.core.unknown;
    let key = the_name.map(|n| n.sym).or(inf.sym.empty);
    let f = tgt.as_ref().and_then(|(c, _)| key.and_then(|k| inf.program.class(*c).constructors.get(&k).copied()));
    match (tgt, f) {
        (Some((c, targs)), Some(f)) => {
            let sig = inf.outline.functions[f.0 as usize].signature;
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            let sig = if ps.len() == targs.len() {
                let map = inf.map(&ps, &targs);
                inf.subst(sig, &map)
            } else {
                sig
            };
            // Arity: the `super.x` count as implicit arguments
            // (`verifySuperFormalParameters`); the cited name is the constructor's,
            // or `<typ of return_>.new`; the entity of the required named one is
            // the whole invocation.
            let cited_name = match the_name {
                Some(n) => inf.interner.resolve(n.sym).to_string(),
                None => {
                    let ret = match inf.table.get(sig) {
                        Type::Function { ret, .. } => *ret,
                        _ => sig,
                    };
                    format!("{}.new", inf.table.format(ret, inf.interner, inf.program))
                }
            };
            let super_positional_args = params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
            // `POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT`
            // (`verifySuperFormalParameters`,
            // an611:src/error/super_formal_parameters_verifier.dart:22-33): at the
            // name of each positional `super.x`, if the `super(...)` has a positional
            // argument.
            if args.args.iter().any(|a| a.name.is_none()) {
                for p in params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named) {
                    if let Some(n) = p.name {
                        inf.warning_with_code(
                            dartforge_diagnostics::codes::compile_time_error::POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT,
                            n.span,
                            &[],
                        );
                    }
                }
            }
            let super_named_args: Vec<_> =
                params.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
            inf.arity_target = Some(super::calls::ArityTarget {
                name: Some(cited_name),
                entity: Some(span),
                super_positional_args,
                super_named_args,
            });
            super::calls::invoke(inf, cx, sig, args, u, None);
            inf.arity_target = None;
        }
        _ => {
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
        }
    }
}

/// Infers a body (block or expression) with the return already in the
/// context. `end`: where the body's dead fragment ends (the default is the
/// block's last statement).
fn function_body(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    body: &FunctionBody,
    m: AsyncModifier,
    ret: TypeId,
    end: Option<usize>,
) {
    let _ = (m, ret);
    match body {
        FunctionBody::Expression(e) => {
            let ctx = cx.functions.last().map(|f| f.return_context).unwrap_or(inf.core.unknown);
            let t = infer(inf, cx, *e, ctx);
            if let Some(fc) = cx.functions.last().cloned() {
                verify_expression_return(inf, cx, &fc, *e, t);
            }
        }
        FunctionBody::Block(s) => {
            let end = end.unwrap_or_else(|| statements::flow_end(inf, cx, *s));
            statements::enter_flow(cx, end);
            statements::infer_statement(inf, cx, *s);
            statements::exit_flow(cx);
        }
        _ => {}
    }
}

/// `ReturnTypeVerifier.verifyReturnType` (`an611:src/error/return_type_verifier.dart:79-130`,
/// `_isLegalReturnType` `:297-322`): with a written return, `async` requires
/// `Future<Never>` to be a subtype of it, `async*` `Stream<Never>`, `sync*`
/// `Iterable<Never>`, and a generator cannot be `void`; the report goes on the
/// written type. Returns `hasLegalReturnType`.
fn legal_return_type(inf: &mut BodyInferrer<'_>, unit: UnitId, written: Option<ast::TypeId>, m: AsyncModifier, ret: TypeId) -> bool {
    use dartforge_diagnostics::codes::compile_time_error as ce;
    let Some(written) = written else { return true };
    let (cls, code) = match m {
        AsyncModifier::None => return true,
        AsyncModifier::Async => (inf.core.future_class, ce::ILLEGAL_ASYNC_RETURN_TYPE),
        AsyncModifier::AsyncStar => (inf.core.stream_class, ce::ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE),
        AsyncModifier::SyncStar => (inf.core.iterable_class, ce::ILLEGAL_SYNC_GENERATOR_RETURN_TYPE),
    };
    let generator = matches!(m, AsyncModifier::AsyncStar | AsyncModifier::SyncStar);
    let illegal = if generator && matches!(inf.table.get(ret), Type::Void) {
        true
    } else {
        let n = inf.core.never;
        let piso = inf.iface(cls, vec![n]);
        !inf.sub(piso, ret)
    };
    if illegal {
        let sp = inf.program.unit(unit).ast.ty(written).span;
        inf.warning_with_code(code, sp, &[]);
    }
    !illegal
}

/// `ResolverVisitor.checkForBodyMayCompleteNormally`
/// (`an611:src/generated/resolver.dart:522-603`), with the end of the block
/// body reachable: without a return context, nothing (`catchError` is another
/// rule); a generator, nothing; `async` with the imposed type outside
/// `Future<Never>`, nothing; a potentially non-nullable context gives
/// `BODY_MIGHT_COMPLETE_NORMALLY`; a nullable one whose base (`futureOrBase`)
/// is not `dynamic`, `_`, `void` or `Null`,
/// `BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE`.
fn body_completes_normally(inf: &mut BodyInferrer<'_>, imposed: Option<TypeId>, context: Option<TypeId>, m: AsyncModifier, where_: dartforge_diagnostics::Span) {
    use dartforge_diagnostics::codes::{compile_time_error as ce, warning as w};
    let Some(rt) = context else { return };
    if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) || inf.the_is_unknown(rt) {
        return;
    }
    if m == AsyncModifier::Async
        && let Some(i) = imposed
    {
        let n = inf.core.never;
        let piso = inf.future(n);
        if !inf.sub(piso, i) {
            return;
        }
    }
    // `isPotentiallyNonNullable` is `!isNullable` (type_system.dart:1242-1284):
    // `dynamic`, invalid, the unknown `_`, `void` and `Null` are nullable,
    // and `FutureOr<T>` is nullable if `T` is (`FutureOr<_>`, the context of
    // a closure passed to `Future.delayed`, is nullable).
    fn is_nullable(inf: &mut BodyInferrer<'_>, t: TypeId, bottom: u32) -> bool {
        if bottom > 32 {
            return false;
        }
        if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.the_is_unknown(t) || matches!(inf.table.get(t), Type::Void | Type::Null) {
            return true;
        }
        match inf.table.get(t).clone() {
            Type::FutureOr { arg, nullable } => nullable || is_nullable(inf, arg, bottom + 1),
            Type::Intersection { bound, .. } => is_nullable(inf, bound, bottom + 1),
            _ => inf.the_is_nullable(t),
        }
    }
    if !is_nullable(inf, rt, 0) {
        inf.warning_with_args(ce::BODY_MIGHT_COMPLETE_NORMALLY, where_, &[crate::exibicao::Arg::Type_(rt)]);
        return;
    }
    let mut base = rt;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.table.e_invalido(base) || inf.the_is_unknown(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.warning_with_args(w::BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE, where_, &[crate::exibicao::Arg::Type_(rt)]);
}

/// `_checkForFutureCatchErrorOnError` (`an611:src/generated/resolver.dart:3971-4006`):
/// a function expression without a return context, a direct positional argument
/// of `x.catchError(...)` with `x` a `Future<T>`: the `{` reports
/// `BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR` with the base of `FutureOr<T>`.
fn catch_error_without_return(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, where_: dartforge_diagnostics::Span) {
    let dartforge_frontend::parents::Parent::Expr(call) = inf.parent_of(cx.unit, e) else { return };
    let a = &inf.program.unit(cx.unit).ast;
    let ast::ExprKind::Call { target, arguments } = &a.expr(call).kind else { return };
    if !arguments.args.iter().any(|x| x.value == e && x.name.is_none()) {
        return;
    }
    let ast::ExprKind::Property { target: tgt, name, .. } = &a.expr(*target).kind else { return };
    if inf.interner.resolve(name.sym) != "catchError" {
        return;
    }
    let Some(recv) = inf.body_types.units[cx.unit.0 as usize].static_types.get(tgt.0 as usize).copied() else { return };
    if !matches!(inf.table.get(recv), Type::Interface { .. }) {
        return;
    }
    let Some(args) = inf.as_instance_of(recv, inf.core.future_class) else { return };
    let Some(&t) = args.first() else { return };
    let mut base = t;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.the_is_unknown(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.warning_with_args(dartforge_diagnostics::codes::warning::BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR, where_, &[crate::exibicao::Arg::Type_(base)]);
}

/// Type of an untyped top-level variable/field: an override (a field that
/// overrides a getter) or the initializer (`Null` becomes `dynamic`).
pub(crate) fn infer_untyped_variable_type(inf: &mut BodyInferrer<'_>, vid: VariableId) -> TypeId {
    let v = inf.program.variable(vid);
    // Override of an inherited member (instance fields).
    if !v.static_ {
        if let Some(c) = v.class {
            if let Some(t) = overlay_type(inf, c, vid) {
                if let Some((unit, init)) = inf.initializer(vid) {
                    let mut cx = Body::to_variable(inf, vid, unit);
                    infer(inf, &mut cx, init, t);
                }
                return t;
            }
        }
    }
    match inf.initializer(vid) {
        Some((unit, init)) => {
            let mut cx = Body::to_variable(inf, vid, unit);
            let u = inf.core.unknown;
            let t = infer(inf, &mut cx, init, u);
            if matches!(inf.table.get(t), Type::Null) {
                inf.core.dynamic_
            } else {
                t
            }
        }
        None => match v.node {
            // The constant's type comes from the outline (the written instantiation
            // or the bounds'); the class's `this` type (`E<T>`) would leak the `T`.
            VariableRef::EnumConstant { unit, decl, index } => {
                if let Some(t) = enum_constant_type(inf, vid, unit, decl, index) {
                    return t;
                }
                let d = &inf.outline.variables[vid.0 as usize];
                match d.declared_type.or(d.inferred) {
                    Some(t) => t,
                    None => v.class.map(|c| inf.class_this_type(c)).unwrap_or(inf.core.dynamic_),
                }
            }
            _ => inf.core.dynamic_,
        },
    }
}

/// Enum `c` has the synthetic unnamed constructor: none declared is
/// generative or unnamed (`buildEnumSyntheticConstructors`).
fn enum_has_synthetic(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let k = inf.program.class(c);
    !k.constructors.iter().any(|(the_name, &f)| !inf.program.function(f).factory || inf.interner.resolve(*the_name).is_empty())
}

/// The implicit type of a generic enum constant without written type
/// arguments: that of the creation `E.the_name(args)` inferred without a context
/// (the synthetic initialization of `ElementBuilder`, typed like an untyped
/// field). The inference is speculative: its warnings come out when the enum
/// declaration is visited.
fn enum_constant_type(inf: &mut BodyInferrer<'_>, vid: VariableId, unit: UnitId, decl: ast::DeclId, index: usize) -> Option<TypeId> {
    let c = inf.program.variable(vid).class?;
    let program = inf.program;
    let ast::DeclKind::Enum(en) = &program.unit(unit).ast.decl(decl).kind else { return None };
    let k = en.constants.get(index)?;
    let args = k.arguments.as_ref()?;
    let key = match k.constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => inf.sym.empty,
        Some(x) => Some(x),
        None => inf.sym.empty,
    };
    let f = key.and_then(|ch| program.class(c).constructors.get(&ch).copied())?;
    let before = inf.diagnostics.len();
    let saved_target = inf.arity_target.take();
    let mut cx = Body::new(inf, unit, Some(c), None, true);
    let u = inf.core.unknown;
    let t = super::calls::build(inf, &mut cx, None, c, Some(f), None, args, u);
    inf.arity_target = saved_target;
    inf.diagnostics.truncate(before);
    inf.warning_units.truncate(before);
    match inf.table.get(t) {
        Type::Interface { class, .. } if *class == c => Some(t),
        _ => None,
    }
}

/// The type of an untyped instance field through override inference
/// (`_inferAccessorOrField` with `field`): the getters and setters it
/// overrides in the direct superinterfaces (`getOverridden2`), through the
/// combined signature. Only getters: the combined getter's return; only
/// setters: the combined setter's parameter; both: the getter's for a
/// `final` field, and for a non-`final` one the type only when both agree.
/// With nothing (or no agreement), the initializer decides.
fn overlay_type(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let before = inf.declared_untyped.replace(c);
    let r = overlay_type_at(inf, c, vid);
    inf.declared_untyped = before;
    if inf.provisional_inheritance && before.is_none() {
        inf.inheritance = crate::heranca::Heranca::default();
        inf.provisional_inheritance = false;
    }
    r
}

fn overlay_type_at(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let v = inf.program.variable(vid);
    let final_ = v.final_ || v.const_;
    let (getters, setters, n_getter, n_setter) = inf.overridden_getters_and_setters(c, v.library, v.name);
    match (getters.is_empty(), setters.is_empty(), n_setter) {
        (false, true, _) => Some(inf.combined_getter_type(c, &getters, n_getter)),
        (true, false, Some(ns)) => Some(inf.combined_setter_type(c, &setters, ns)),
        (false, false, Some(ns)) => {
            let g = inf.combined_getter_type(c, &getters, n_getter);
            if final_ {
                return Some(g);
            }
            let s = inf.combined_setter_type(c, &setters, ns);
            (inf.table.canonico(g) == inf.table.canonico(s)).then_some(g)
        }
        _ => None,
    }
}

/// Extracts the function type from a context (through `?` and `FutureOr`).
fn context_function(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<TypeId> {
    if inf.the_is_unknown(ctx) {
        return None;
    }
    let t = inf.non_null(ctx);
    match inf.table.get(t).clone() {
        Type::Function { .. } => Some(t),
        Type::FutureOr { arg, .. } => context_function(inf, arg),
        _ => None,
    }
}

/// Function expression (closure) in context `ctx`.
pub(crate) fn function_expression(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId, ctx: TypeId, e: ExprId) -> TypeId {
    // `wasFunctionTypeSupplied` (`function_expression_resolver.dart:35`): the
    // context, as it arrived, is a function type.
    if matches!(inf.table.get(ctx), Type::Function { .. }) {
        inf.body_types.units[cx.unit.0 as usize].with_function_type.insert(fid);
    }
    let (t, _) = function_literal(inf, cx, fid, ctx, None, Some(e));
    t
}

/// Infers a literal or local function. `local` is the id of the local
/// function (declared before, for recursive calls).
fn function_literal(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId, ctx: TypeId, local: Option<crate::resolved::LocalId>, expression: Option<ExprId>) -> (TypeId, ()) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let u = inf.core.unknown;
    let ctx_fn = context_function(inf, ctx);
    // Write capture: outer locals written inside the closure lose their
    // promotions (here and after it).
    let written_ones = statements::names_written_in_function(inf, cx.unit, af);
    let mut captured: Vec<crate::resolved::LocalId> = Vec::new();
    for n in &written_ones {
        if let Some(id) = statements::write_local(cx, *n) {
            captured.push(id);
        }
    }
    let flow_before = cx.flow.clone();
    let mut flow_inside = cx.flow.clone();
    for &id in &captured {
        flow_inside.capture(id);
    }
    for e in cx.written_in_closure.clone().iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.capture(id);
        }
    }
    if cx.written_in_body.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.written_in_body = Some(match cx.root {
            // The whole member, including writes to its own parameters (R-FLU-07
            // rule 2; `base ??= const {}` and then the closure).
            super::body::Root::Funcao(f) => statements::separately_written_names(inf, cx.unit, &a.function(f).body),
            super::body::Root::Construtor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => statements::separately_written_names(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::body::Root::Nothing => Default::default(),
        });
    }
    // `functionExpression_begin`: `conservativeJoin(anywhere.written,
    // anywhere.captured)`: a write anywhere in the member loses the
    // promotions inside the literal but stays promotable; a write inside
    // some literal of the member (before or after this one) is captured and
    // does not promote (R-FLU-07 rule 2).
    let (outside, inside) = cx.written_in_body.clone().unwrap_or_default();
    for e in outside.iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.conservative_join(&[id], &[]);
        }
    }
    for e in inside.iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.conservative_join(&[], &[id]);
        }
    }
    // Only a write to a variable from outside the function is a capture:
    // that of its own parameters and locals (a declaration found in its scan)
    // is not.
    cx.written_in_closure.extend(written_ones.iter().copied().filter(|e| e.1.is_none()));
    cx.flow = flow_inside;
    cx.push_scope();
    // Type parameters of the function literal.
    let mut tps: Vec<TypeParamId> = Vec::new();
    for tp in af.type_params.iter() {
        let p = inf.table.alloc_type_param(tp.name.sym, crate::table::TypeParamOwner::GenericFunctionType, inf.core.object_nullable, crate::table::Variance::Unspecified);
        inf.table.without_default_type.insert(p);
        cx.declare_type_param(tp.name.sym, p);
        tps.push(p);
    }
    for (tp, &p) in af.type_params.iter().zip(tps.iter()) {
        if let Some(b) = tp.bound {
            let bt = inf.type_from_annotation(cx, b);
            inf.table.set_type_param_bound(p, bt);
        }
    }
    // Function context with the type parameters renamed to ours.
    let (cpos, copt, cnamed, cret) = match ctx_fn.map(|t| inf.table.get(t).clone()) {
        Some(Type::Function { type_params, ret, positional, optional, named, .. }) => {
            let map: HashMap<TypeParamId, TypeId> = if type_params.len() == tps.len() {
                type_params.iter().copied().zip(tps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false }))).collect()
            } else {
                HashMap::new()
            };
            let s = |inf: &mut BodyInferrer<'_>, t: TypeId| inf.subst(t, &map);
            let pos: Vec<TypeId> = positional.iter().map(|&t| s(inf, t)).collect();
            let opt: Vec<TypeId> = optional.iter().map(|&t| s(inf, t)).collect();
            let named: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|(n, t, r)| (*n, s(inf, *t), *r)).collect();
            let r = s(inf, ret);
            (pos, opt, named, Some(r))
        }
        _ => (Vec::new(), Vec::new(), Vec::new(), None),
    };
    let all_ctx: Vec<TypeId> = cpos.iter().chain(copt.iter()).copied().collect();
    let mut pos = Vec::new();
    let mut opt = Vec::new();
    let mut named = Vec::new();
    let mut ipos = 0usize;
    // The annotations of the parameters of a local or literal function, in the
    // outer scope (`AnnotationResolver`'s validation with the visible locals).
    if let Some(ps) = &af.parameters {
        fn annotations<'b>(ps: &'b [ast::Parameter], output: &mut Vec<&'b ast::Annotation>) {
            for q in ps.iter() {
                output.extend(q.metadata.iter());
                if let Some(fs) = &q.function_parameters {
                    annotations(fs, output);
                }
            }
        }
        let mut all = Vec::new();
        annotations(ps, &mut all);
        for m in all {
            super::statements::validate_local_annotation(inf, cx, m);
        }
    }
    if let Some(ps) = &af.parameters {
        for p in ps.iter() {
            // `ResolutionVisitor.visitFieldFormalParameter`/
            // `visitSuperFormalParameter` without an `ElementWalker` (local function,
            // closure; 3.6.2 `resolution_visitor.dart:614-635`, `:1263`): a required
            // `this.x`/`super.x` is only closed, it does not enter the parameters
            // (neither the type nor the scope); an optional one enters through the
            // `DefaultFormalParameter` (`:440-467`).
            if p.kind == ast::ParameterKind::Required && (p.this_ || p.super_) {
                continue;
            }
            let scope = cx.visible_type_parameters();
            // Inside a static member (`_inStaticMethod` goes up through the elements
            // enclosing the closure), the class's type parameter in the parameter's
            // type is `TYPE_PARAMETER_REFERENCED_BY_STATIC`.
            let static_before = std::mem::replace(&mut inf.in_static_member, cx.static_member);
            let written = inf.written_parameter_type(cx.unit, cx.lib, p, &scope);
            inf.in_static_member = static_before;
            let do_ctx = match p.kind {
                ast::ParameterKind::Named => p.name.and_then(|n| cnamed.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t)),
                _ => {
                    let t = all_ctx.get(ipos).copied();
                    ipos += 1;
                    t
                }
            };
            let t = match written {
                Some(t) => t,
                None => match do_ctx {
                    Some(k) => {
                        let s = inf.greatest_closure_of(k);
                        let n = inf.core.null;
                        if inf.sub(s, n) {
                            inf.core.object_nullable
                        } else {
                            s
                        }
                    }
                    None => inf.core.dynamic_,
                },
            };
            if let Some(d) = p.default_value {
                let td = infer(inf, cx, d, t);
                assignable_default_value(inf, cx, d, td, t);
            } else if p.kind != ast::ParameterKind::Required
                && !p.required
                && let Some(the_name) = p.name
                && !inf.the_is_nullable(t)
            {
                // `_checkUseOfDefaultValuesInParameters` (3.6.2,
                // `error_verifier.dart:6180-6250`) also in function expressions and local
                // functions: an optional one without a default whose type is potentially
                // non-nullable (the element's, inferred from the context if not written).
                use dartforge_diagnostics::codes::compile_time_error as ce;
                let annotated = p.metadata.iter().any(|a| a.name.last().is_some_and(|n| inf.interner.resolve(n.sym) == "required"));
                if annotated {
                    inf.warning_with_code(ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION, the_name.span, &[]);
                } else {
                    let code = if p.kind == ast::ParameterKind::Optional { ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL } else { ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER };
                    let text = inf.interner.resolve(the_name.sym).to_string();
                    inf.warning_with_code(code, the_name.span, &[&text]);
                }
            }
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
            if let Some(n) = &p.name {
                declare_local(
                    inf,
                    cx,
                    Local { name: n.sym, ty: t, final_: p.final_, late: false, const_: false, offset: n.span.start, local_function: false },
                    true,
                );
            }
        }
    }
    // Return: written, or inferred with the schema imposed by the context.
    let written_ret = af.return_type.map(|r| inf.type_from_annotation(cx, r));
    let m = af.modifier;
    // The whole return context (a generator's runtime type uses it).
    let full_context = cret;
    let (ctx_ret, declared) = match written_ret {
        Some(r) => (inf.declared_return_context(r, m), Some(r)),
        None => {
            let s = cret.unwrap_or(u);
            let k = match m {
                AsyncModifier::None => s,
                AsyncModifier::AsyncStar => inf.as_schema_instance_of(s, inf.core.stream_class).unwrap_or(u),
                AsyncModifier::SyncStar => inf.as_schema_instance_of(s, inf.core.iterable_class).unwrap_or(u),
                AsyncModifier::Async => {
                    let fv = inf.future_value_type_schema(s);
                    inf.future_or(fv)
                }
            };
            (k, None)
        }
    };
    // A local function with a written return: the type is available for
    // recursion.
    if let (Some(id), Some(r)) = (local, declared) {
        let ft = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        cx.locals[id.0 as usize].ty = ft;
        cx.locals[id.0 as usize].local_function = false;
    }
    // A named local function: "function 'f'"; a closure: another rule (not
    // here).
    let executable = match (local, af.name) {
        (Some(_), Some(n)) => Some(super::body::Executable {
            kind: super::body::ExecutableKind::Funcao,
            nome: inf.interner.resolve(n.sym).to_string(),
        }),
        _ => None,
    };
    // `verifyReturnType` of a named local function (`visitFunctionDeclaration`).
    let legal_return = match (local, declared) {
        (Some(_), Some(r)) => legal_return_type(inf, cx.unit, af.return_type, m, r),
        _ => true,
    };
    // The function literal's `imposedType`: the return of the context's
    // function type, except `dynamic` and `_` (a named local function uses
    // the declared one).
    let closure_imposed = if local.is_none() && written_ret.is_none() {
        full_context.filter(|&r| !inf.e_dynamic(r) && !inf.the_is_unknown(r))
    } else {
        None
    };
    cx.functions.push(FunctionCtx { closure_imposed, modifier: m, the_return: declared, return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executable, legal_return });
    let saved_jumps = std::mem::take(&mut cx.jumps);
    let saved_cascades = std::mem::take(&mut cx.cascades);
    let saved_targets = std::mem::take(&mut cx.cascade_targets);
    let saved_bases = std::mem::take(&mut cx.cascade_bases);
    let (body_t, completes) = match &af.body {
        FunctionBody::Expression(e) => {
            let t = infer(inf, cx, *e, ctx_ret);
            if let Some(fc) = cx.functions.last().cloned() {
                verify_expression_return(inf, cx, &fc, *e, t);
            }
            if let Some(f) = cx.functions.last_mut() {
                f.returned_expressions.push((*e, t));
            }
            (Some(t), false)
        }
        FunctionBody::Block(s) => {
            let end = statements::flow_end(inf, cx, *s);
            statements::enter_flow(cx, end);
            statements::infer_statement(inf, cx, *s);
            statements::exit_flow(cx);
            let reachable = cx.flow.reachable;
            if reachable {
                // `checkForBodyMayCompleteNormally`: a local function at the name, a
                // function expression at the `{`; the imposed type is the written return
                // or the context's.
                let imposed = declared.or(cret);
                let where_ = match (local, af.name) {
                    (Some(_), Some(n)) => n.span,
                    _ => {
                        let begin = inf.program.unit(cx.unit).ast.stmt(*s).span.start;
                        dartforge_diagnostics::Span { start: begin, end: begin + 1 }
                    }
                };
                body_completes_normally(inf, imposed, imposed.map(|_| ctx_ret), m, where_);
                if imposed.is_none()
                    && local.is_none()
                    && let Some(ex) = expression
                {
                    catch_error_without_return(inf, cx, ex, where_);
                }
            }
            (None, reachable)
        }
        _ => (None, false),
    };
    cx.jumps = saved_jumps;
    cx.cascades = saved_cascades;
    cx.cascade_targets = saved_targets;
    cx.cascade_bases = saved_bases;
    let fc = cx.functions.pop().unwrap();
    cx.pop_scope();
    cx.flow = flow_before;
    for &id in &captured {
        cx.flow.capture(id);
    }
    let mut generator_execution = None;
    let ret = match declared {
        Some(r) => r,
        None => {
            // Type actually returned.
            let mut t = match body_t {
                Some(t) => t,
                None => {
                    if completes && !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                        inf.core.null
                    } else {
                        inf.core.never
                    }
                }
            };
            let generator = matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
            if body_t.is_none() {
                for &r in &fc.returned {
                    t = inf.up(r, t);
                }
                // `return;` contributes `Null`, also in a generator: analyzer 3.6.2
                // (oracle) joins the `return`s with the generator's `yield`s (the rule
                // that ignores `return;` in a generator came later).
                if fc.empty_return {
                    let n = inf.core.null;
                    t = inf.up(n, t);
                }
                // A generator without `yield` or `return`: element `dynamic` (analyzer
                // 3.6.2), not `Never`.
                if generator && fc.returned.is_empty() && !fc.empty_return {
                    t = inf.core.dynamic_;
                }
            } else if m == AsyncModifier::Async {
                t = inf.flatten(t);
            }
            let r = inf.greatest_closure_of(ctx_ret);
            let wrap = |inf: &mut BodyInferrer<'_>, t: TypeId| {
                let s = if matches!(inf.table.get(r), Type::Void)
                    || (m == AsyncModifier::Async && matches!(inf.table.get(r), Type::FutureOr { arg, .. } if matches!(inf.table.get(*arg), Type::Void)))
                {
                    inf.core.void_
                } else if inf.sub(t, r) {
                    t
                } else {
                    r
                };
                match m {
                    AsyncModifier::Async => {
                        let f = inf.flatten(s);
                        inf.future(f)
                    }
                    AsyncModifier::AsyncStar => inf.flow_of(s),
                    AsyncModifier::SyncStar => inf.iterable(s),
                    AsyncModifier::None => s,
                }
            };
            let static_ = wrap(inf, t);
            // `ReturnTypeVerifier` checks every `return` of the closure against its
            // return type (the inferred one, already adjusted to the context); a
            // `=> e` with a flattened `void` return accepts any expression
            // (`verifyExpressionFunctionBody`, `return_type_verifier.dart:45-52`).
            let flattened = if m == AsyncModifier::Async { inf.flatten(static_) } else { static_ };
            let void_arrow = matches!(af.body, FunctionBody::Expression(_)) && matches!(inf.table.get(flattened), Type::Void);
            if fc.executable.is_none() && !generator && !void_arrow {
                closure_returns(inf, cx, &fc, static_, static_);
            }
            // `_checkReturnWithoutValue` (`return_type_verifier.dart:278-295`) with
            // the closure's return type, the inferred one: `return;` where it (the
            // future value, in `async`) is not `void`, `dynamic` or `Null`.
            if fc.executable.is_none() && !generator && !fc.valueless_returns.is_empty() {
                let tv = if m == AsyncModifier::Async { inf.future_value_type(static_) } else { static_ };
                if !matches!(inf.table.get(tv), Type::Void | Type::Dynamic | Type::Null) {
                    for &sp in &fc.valueless_returns {
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::RETURN_WITHOUT_VALUE, sp, &[]);
                    }
                }
            }
            // The generator's runtime type (the CFE rule, checked against the 3.6.2
            // and 3.13.4 VMs): the element is the upper bound of the `yield`s (the
            // `return;` does not count; `Null` with none), wrapped
            // (`Iterable`/`Stream`); the CFE compares that wrapped type with the
            // context's ELEMENT and, if it is not a subtype of it, the return is the
            // context's whole return type (`Iterable<num>? Function()` gives
            // `Iterable<num>?`).
            if generator && body_t.is_none() {
                let mut e = if fc.returned.is_empty() { inf.core.null } else { inf.core.never };
                for &y in &fc.returned {
                    e = inf.up(y, e);
                }
                let wrapped = match m {
                    AsyncModifier::AsyncStar => inf.flow_of(e),
                    _ => inf.iterable(e),
                };
                let execution = if inf.sub(wrapped, r) {
                    wrapped
                } else {
                    let context = full_context.unwrap_or(inf.core.unknown);
                    inf.greatest_closure_of(context)
                };
                if execution != static_ {
                    generator_execution = Some(execution);
                }
            }
            static_
        }
    };
    if let Some(r) = generator_execution {
        let fe = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        inf.body_types.units[cx.unit.0 as usize].function_runtime_types.insert(fid, fe);
    }
    let ft = inf.table.intern(Type::Function {
        type_params: tps.into_boxed_slice(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: opt.into_boxed_slice(),
        named: named.into_boxed_slice(),
        nullable: false,
    });
    (ft, ())
}

impl<'a> BodyInferrer<'a> {
    /// `S` is `C<S1>` (without subtypes): `S1`.
    pub(crate) fn as_schema_instance_of(&mut self, s: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        match self.table.get(s) {
            Type::Interface { class, args, .. } if *class == c && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    }
}

/// Local function declaration.
pub(crate) fn the_local_function(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let Some(the_name) = af.name else { return };
    let d = inf.core.dynamic_;
    let id = cx.declare(Local { name: the_name.sym, ty: d, final_: true, late: false, const_: false, offset: the_name.span.start, local_function: true });
    cx.local_functions.insert(id);
    cx.flow.initialize(id);
    let u = inf.core.unknown;
    let (t, _) = function_literal(inf, cx, fid, u, Some(id), None);
    cx.locals[id.0 as usize].ty = t;
    cx.locals[id.0 as usize].local_function = false;
    inf.body_types.units[cx.unit.0 as usize].set_local_type(the_name.span.start, t);
    // A local function with a written return (`parent.returnType?.type`).
    if let Some(rt) = inf.program.unit(cx.unit).ast.function(fid).return_type
        && let Some(&return_) = inf.body_types.units[cx.unit.0 as usize].tipos_de_anotacoes.get(&rt)
    {
        unnecessary_set(inf, cx.unit, fid, return_);
    }
}

/// `return_` is `void`, `Future<void>` or `FutureOr<void>`.
fn void_return(inf: &BodyInferrer<'_>, t: TypeId) -> bool {
    match inf.table.get(t) {
        Type::Void => true,
        Type::Interface { class, args, .. } if Some(*class) == inf.core.future_class => args.len() == 1 && matches!(inf.table.get(args[0]), Type::Void),
        Type::FutureOr { arg, .. } => matches!(inf.table.get(*arg), Type::Void),
        _ => false,
    }
}

/// `UNNECESSARY_SET_LITERAL` (`_checkForUnnecessarySetLiteral`,
/// `an611:src/error/best_practices_verifier.dart:1283-1318`): the body `=> {…}`
/// is a set literal and the return (that of the parameter receiving the
/// closure, or the declaration's written one) is `void`, `Future<void>` or
/// `FutureOr<void>`; at the literal.
pub(crate) fn unnecessary_set(inf: &mut BodyInferrer<'_>, unit: UnitId, fid: ast::FunctionId, return_: TypeId) {
    let a = &inf.program.unit(unit).ast;
    let ast::FunctionBody::Expression(e) = a.function(fid).body else { return };
    if !matches!(a.expr(e).kind, ast::ExprKind::SetOrMap { .. }) || !void_return(inf, return_) {
        return;
    }
    let Some(t) = inf.body_types.units[unit.0 as usize].get_type(e) else { return };
    if matches!(inf.table.get(t), Type::Interface { class, .. } if Some(*class) == inf.core.set_class) {
        let sp = inf.span_expr(unit, e);
        inf.warning_with_code(dartforge_diagnostics::codes::warning::UNNECESSARY_SET_LITERAL, sp, &[]);
    }
}

/// Metadata (annotations) and enum constant arguments of a unit.
pub(crate) fn infer_unit_metadata(inf: &mut BodyInferrer<'_>, unit: UnitId) {
    let a = &inf.program.unit(unit).ast;
    // The annotations of the directives (`@Timeout(Duration(seconds: 9)) library;`,
    // or before the first `import`): in the library's scope, like those of
    // the declarations (`ResolutionVisitor.visitDirective`).
    for d in inf.program.unit(unit).unit.directives.iter() {
        for m in d.metadata.iter() {
            annotation(inf, unit, None, None, m);
        }
    }
    for (di, d) in a.decls.iter().enumerate() {
        let (cls, _) = decl_class(inf, unit, d);
        // A declaration's annotation is resolved in the scope outside it (the
        // library's): the container's members are not in it.
        for m in d.metadata.iter() {
            annotation(inf, unit, None, None, m);
        }
        // The annotations of the type parameters: in the type parameters' scope,
        // whose parent is the library's (the container's members are not).
        let tps: &[ast::TypeParameter] = match &d.kind {
            ast::DeclKind::Class(x) => &x.type_params,
            ast::DeclKind::Mixin(x) => &x.type_params,
            ast::DeclKind::Enum(x) => &x.type_params,
            ast::DeclKind::Extension(x) => &x.type_params,
            ast::DeclKind::ExtensionType(x) => &x.type_params,
            ast::DeclKind::Typedef(x) => &x.type_params,
            ast::DeclKind::Function(_) | ast::DeclKind::Variables(_) => &[],
        };
        // The elements of the declaration's type parameters, which are in the
        // scope of their annotations (`@A(T) U` resolves `T` to the type
        // parameter; evaluated, it is `CONST_TYPE_PARAMETER`).
        let ids: Vec<TypeParamId> = if tps.iter().any(|tp| !tp.metadata.is_empty()) {
            let decl = ast::DeclId(di as u32);
            let same = |r: &dartforge_elements::model::DeclRef| r.unit == unit && r.decl == decl;
            match &d.kind {
                ast::DeclKind::Extension(_) => inf
                    .program
                    .extensions
                    .iter()
                    .position(|x| same(&x.decl))
                    .and_then(|i| inf.outline.extensions.get(i))
                    .map(|x| x.type_params.to_vec())
                    .unwrap_or_default(),
                ast::DeclKind::Typedef(_) => inf
                    .program
                    .typedefs
                    .iter()
                    .position(|x| same(&x.decl))
                    .and_then(|i| inf.outline.typedefs.get(i))
                    .map(|x| x.type_params.to_vec())
                    .unwrap_or_default(),
                _ => cls.and_then(|c| inf.outline.classes.get(c.0 as usize)).map(|x| x.type_params.to_vec()).unwrap_or_default(),
            }
        } else {
            Vec::new()
        };
        let scope: Vec<(dartforge_intern::SymbolId, TypeParamId)> =
            if ids.len() == tps.len() { tps.iter().map(|tp| tp.name.sym).zip(ids.iter().copied()).collect() } else { Vec::new() };
        for tp in tps.iter() {
            for m in tp.metadata.iter() {
                validate_annotation(inf, unit, None, m);
                unvalidated_annotation_with(inf, unit, None, None, m, &scope);
            }
        }
        match &d.kind {
            ast::DeclKind::Enum(en) => {
                let Some(c) = cls else { continue };
                for k in en.constants.iter() {
                    for m in k.metadata.iter() {
                        annotation(inf, unit, Some(c), None, m);
                    }
                    // `ResolverVisitor.visitEnumConstantDeclaration`
                    // (`an611:src/generated/resolver.dart:2483-2520`), before the arguments:
                    // the constructor of the synthetic creation is a factory, or does not
                    // exist (the synthetic `const` one only without a declared constructor).
                    let key = match k.constructor.map(|n| n.sym) {
                        Some(x) if Some(x) == inf.sym.new_ => inf.sym.empty,
                        Some(x) => Some(x),
                        None => inf.sym.empty,
                    };
                    {
                        use dartforge_diagnostics::codes::compile_time_error as ce;
                        let ce_enum = inf.program.class(c);
                        let declared = key.and_then(|ch| ce_enum.constructors.get(&ch).copied());
                        // `buildEnumSyntheticConstructors` (`library_builder.dart:238-260`): the
                        // synthetic unnamed one exists when no constructor is generative or
                        // unnamed (only named factories count as absence).
                        let synthetic = key == inf.sym.empty && enum_has_synthetic(inf, c);
                        match declared {
                            Some(f) if inf.program.function(f).factory => {
                                let sp = k.constructor.map_or(k.name.span, |n| n.span);
                                inf.warning_with_code(ce::ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR, sp, &[]);
                            }
                            Some(_) => {}
                            None if synthetic => {}
                            None => match k.constructor {
                                Some(n) => {
                                    let the_name = inf.interner.resolve(n.sym).to_string();
                                    inf.warning_with_code(ce::UNDEFINED_ENUM_CONSTRUCTOR_NAMED, n.span, &[&the_name]);
                                }
                                None => inf.warning_with_code(ce::UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED, k.name.span, &[]),
                            },
                        }
                        // `TypeArgumentsVerifier.checkEnumConstantDeclaration`
                        // (`type_arguments_verifier.dart:93-112`): with the constructor
                        // resolved, a written `<…>` list with a count different from the enum's.
                        let resolved = declared.is_some() || synthetic;
                        let the_expected = inf.program.class(c).type_params.len();
                        if resolved
                            && let (Some(&first), Some(&last)) = (k.type_args.first(), k.type_args.last())
                            && k.type_args.len() != the_expected
                        {
                            let source = &inf.program.unit(unit).source;
                            let arg_start = a.ty(first).span.start;
                            let arg_end = a.ty(last).span.end;
                            let start = source[..arg_start].rfind('<').unwrap_or(arg_start);
                            let end = source[arg_end..].find('>').map_or(arg_end, |i| arg_end + i + 1);
                            let (e, d) = (the_expected.to_string(), k.type_args.len().to_string());
                            inf.warning_with_code(ce::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM, dartforge_diagnostics::Span { start: start, end: end }, &[&e, &d]);
                        }
                    }
                    if let Some(args) = &k.arguments {
                        let mut cx = Body::new(inf, unit, Some(c), None, true);
                        let f = key.and_then(|ch| inf.program.class(c).constructors.get(&ch).copied());
                        if let Some(f) = f {
                            let explicit = if k.type_args.is_empty() {
                                None
                            } else {
                                Some(k.type_args.iter().map(|&t| inf.type_argument_type(&cx, t)).collect())
                            };
                            let u = inf.core.unknown;
                            let enum_name = inf.interner.resolve(en.name.sym).to_string();
                            super::calls::define_target(inf, Some(enum_name), k.name.span);
                            super::calls::build(inf, &mut cx, None, c, Some(f), explicit, args, u);
                            inf.arity_target = None;
                            // `TypeArgumentsVerifier`'s `checkEnumConstantDeclaration`: the
                            // constructor's type arguments (written or inferred), at the written
                            // argument or at the name.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if let Some(types) = inf.body_types.units[unit.0 as usize].instanciacao(args.span.start).map(|x| x.to_vec())
                                && types.len() == params.len()
                            {
                                let nodes = k.type_args.to_vec();
                                super::calls::check_explicit_bounds(inf, unit, &params, &types, &nodes, k.name.span, None);
                            }
                        } else if key == inf.sym.empty && enum_has_synthetic(inf, c) {
                            // Implicit unnamed constructor `const E()`: the arguments go through its
                            // arity (no parameters).
                            let ret = inf.class_this_type(c);
                            let sig = inf.table.intern(Type::Function {
                                type_params: Box::new([]),
                                ret,
                                positional: Box::new([]),
                                optional: Box::new([]),
                                named: Box::new([]),
                                nullable: false,
                            });
                            let enum_name = inf.interner.resolve(en.name.sym).to_string();
                            super::calls::define_target(inf, Some(enum_name), k.name.span);
                            let u = inf.core.unknown;
                            super::calls::invoke(inf, &mut cx, sig, args, u, None);
                            inf.arity_target = None;
                            // `checkEnumConstantDeclaration`: the written arguments of the implicit
                            // constructor.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if !k.type_args.is_empty() && k.type_args.len() == params.len() {
                                let nodes = k.type_args.to_vec();
                                let types: Vec<TypeId> = nodes.iter().map(|&t| inf.type_argument_type(&cx, t)).collect();
                                super::calls::check_explicit_bounds(inf, unit, &params, &types, &nodes, k.name.span, None);
                            }
                        } else {
                            for x in args.args.iter() {
                                infer_free(inf, &mut cx, x.value);
                            }
                        }
                    } else if let Some(f) = inf.sym.empty.and_then(|v| inf.program.class(c).constructors.get(&v).copied()) {
                        let enum_name = inf.interner.resolve(en.name.sym).to_string();
                        super::calls::arity_without_arguments(inf, f, enum_name, k.name.span);
                    }
                }
            }
            _ => {}
        }
    }
    // The annotations of the parameters of a written function type
    // (`void Function([@A() int p])`): `AnnotationResolver` visits every
    // annotation in the tree.
    for ty in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &ty.kind {
            for p in parameters.iter() {
                for m in p.metadata.iter() {
                    annotation(inf, unit, None, None, m);
                }
            }
        }
    }
    for mb in a.members.iter() {
        for m in mb.metadata.iter() {
            // Owner class unknown here: resolves in the library's scope.
            annotation(inf, unit, None, None, m);
        }
        if let ast::MemberKind::Constructor(ctor) = &mb.kind {
            for p in ctor.parameters.iter() {
                for m in p.metadata.iter() {
                    annotation(inf, unit, None, None, m);
                }
            }
        }
    }
    // Top-level functions and methods (local ones and literals see locals
    // that the annotation may cite: they stay without name validation).
    let the_declared: std::collections::HashMap<u32, (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>)> = inf
        .program
        .functions
        .iter()
        .filter_map(|fe| match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit => Some((function.0, (fe.class, fe.extension))),
            _ => None,
        })
        .collect();
    for (i, f) in a.functions.iter().enumerate() {
        let validate = the_declared.contains_key(&(i as u32));
        // A method's type parameters see the container's scope: the class's, or
        // the extension's, with its static members (`@A(foo) T` in a method of an
        // extension with `static foo()`).
        let (owner, owner_extension) = the_declared.get(&(i as u32)).copied().unwrap_or((None, None));
        // The function's own type parameters are also in scope.
        let scope: Vec<(dartforge_intern::SymbolId, TypeParamId)> = if f.type_params.iter().any(|tp| !tp.metadata.is_empty()) {
            let fid = inf.program.functions.iter().position(|fe| {
                matches!(fe.node, dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit && function.0 == i as u32)
            });
            match fid.and_then(|x| inf.outline.functions.get(x)) {
                Some(data) if data.type_params.len() == f.type_params.len() => {
                    f.type_params.iter().map(|tp| tp.name.sym).zip(data.type_params.iter().copied()).collect()
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        for tp in f.type_params.iter() {
            for m in tp.metadata.iter() {
                if validate {
                    validate_annotation_at(inf, unit, owner, owner_extension, m);
                    unvalidated_annotation_with(inf, unit, owner, owner_extension, m, &scope);
                } else {
                    unvalidated_annotation_with(inf, unit, None, None, m, &scope);
                }
            }
        }
        if let Some(ps) = &f.parameters {
            for p in ps.iter() {
                // The parameters of an old-style function parameter
                // (`void f(void cb([@A() int p]))`) are also visited.
                if validate {
                    fn nested(inf: &mut BodyInferrer<'_>, unit: UnitId, ps: &[ast::Parameter]) {
                        for q in ps.iter() {
                            for m in q.metadata.iter() {
                                annotation(inf, unit, None, None, m);
                            }
                            if let Some(fs) = &q.function_parameters {
                                nested(inf, unit, fs);
                            }
                        }
                    }
                    if let Some(fs) = &p.function_parameters {
                        nested(inf, unit, fs);
                    }
                }
                for m in p.metadata.iter() {
                    if validate {
                        annotation(inf, unit, None, None, m);
                    } else {
                        unvalidated_annotation(inf, unit, None, m);
                    }
                }
            }
        }
    }
}

fn decl_class(inf: &BodyInferrer<'_>, unit: UnitId, d: &ast::Decl) -> (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>) {
    let lib = inf.program.unit(unit).library;
    let the_name = match &d.kind {
        ast::DeclKind::Class(c) => Some(c.name.sym),
        ast::DeclKind::Mixin(m) => Some(m.name.sym),
        ast::DeclKind::Enum(e) => Some(e.name.sym),
        ast::DeclKind::ExtensionType(e) => Some(e.name.sym),
        _ => None,
    };
    match the_name.and_then(|n| inf.program.library(lib).declared.get(&n)).and_then(|b| b.getter) {
        Some(Element::Class(c)) => (Some(c), None),
        _ => (None, None),
    }
}

/// What a name in an annotation is, for `AnnotationResolver`.
enum InAnnotation {
    Nada,
    Prefixo,
    Classe(ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
    Alias(dartforge_elements::model::TypedefId),
    /// Getter: the implicit one of a variable (`Some(const)`) or a written one (`None`).
    Getter(Option<bool>),
    Outro,
}

fn classify_in_annotation(inf: &BodyInferrer<'_>, b: Option<dartforge_elements::model::Binding>) -> InAnnotation {
    use dartforge_elements::model::FunctionKind as K;
    match b.and_then(|b| b.getter) {
        None => InAnnotation::Nada,
        Some(Element::Prefix(..)) => InAnnotation::Prefixo,
        Some(Element::Class(c)) => InAnnotation::Classe(c),
        Some(Element::Extension(x)) => InAnnotation::Extensao(x),
        Some(Element::Typedef(td)) => InAnnotation::Alias(td),
        Some(Element::Variable(v)) => InAnnotation::Getter(Some(inf.program.variable(v).const_)),
        Some(Element::Function(f)) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (K::ImplicitAccessor, Some(v)) => InAnnotation::Getter(Some(inf.program.variable(v).const_)),
                (K::Getter, _) => InAnnotation::Getter(None),
                _ => InAnnotation::Outro,
            }
        }
    }
}

/// The static getter `the_name` of a class or extension (`getGetter`):
/// `Some(Some(const))` for the implicit one of a field (or enum constant, or
/// `values`), `Some(None)` for a written getter.
fn static_getter(
    inf: &BodyInferrer<'_>,
    cls: Option<ClassId>,
    ext: Option<dartforge_elements::model::ExtensionId>,
    the_name: dartforge_intern::SymbolId,
) -> Option<Option<bool>> {
    use dartforge_elements::model::FunctionKind as K;
    let (statics, fields) = match (cls, ext) {
        (Some(c), _) => {
            let k = inf.program.class(c);
            if k.enum_constants.iter().any(|&v| inf.program.variable(v).name == the_name) {
                return Some(Some(true));
            }
            if k.kind == dartforge_elements::model::ClassKind::Enum && inf.interner.resolve(the_name) == "values" {
                return Some(Some(true));
            }
            (&k.static_members, k.fields.clone())
        }
        (None, Some(x)) => {
            let e = inf.program.extension(x);
            (&e.static_members, e.fields.clone())
        }
        _ => return None,
    };
    if let Some(&f) = statics.get(&the_name) {
        let fe = inf.program.function(f);
        return match (fe.kind, fe.variable) {
            (K::ImplicitAccessor, Some(v)) => Some(Some(inf.program.variable(v).const_)),
            (K::Getter, _) => Some(None),
            _ => None,
        };
    }
    fields
        .iter()
        .find(|&&v| inf.program.variable(v).name == the_name && inf.program.variable(v).static_)
        .map(|&v| Some(inf.program.variable(v).const_))
}

/// `AnnotationResolver` (`an611:src/dart/resolver/annotation_resolver.dart:
/// 30-420`): `UNDEFINED_ANNOTATION` at a name that does not resolve,
/// `INVALID_ANNOTATION` at an annotation that is neither a reference to a
/// constant nor an invocation of a constant constructor. Names are resolved in
/// the library's scope (the class's, only when it is known).
pub(crate) fn validate_annotation(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, m: &ast::Annotation) {
    validate_annotation_at(inf, unit, cls, None, m);
}

/// [`validate_annotation`] in the scope of a class or extension (its static
/// members).
pub(crate) fn validate_annotation_at(
    inf: &mut BodyInferrer<'_>,
    unit: UnitId,
    cls: Option<ClassId>,
    extension: Option<dartforge_elements::model::ExtensionId>,
    m: &ast::Annotation,
) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let span = m.span;
    let Some(&n1) = m.name.first() else { return };
    let args = m.arguments.is_some();
    let n2 = m.name.get(1).copied();
    // Static constants of the class (or extension) the annotation is in.
    if m.name.len() == 1 {
        if let Some(g) = static_getter(inf, cls, extension, n1.sym) {
            if g != Some(true) || args {
                inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
            }
            return;
        }
    }
    let b1 = inf.program.lookup_na_unidade(unit, n1.sym);
    let mut e1 = classify_in_annotation(inf, b1);
    if matches!(e1, InAnnotation::Nada) && inf.program.prefixos_na_unidade(unit).contains_key(&n1.sym) {
        e1 = InAnnotation::Prefixo;
    }
    // For a prefix: the second name resolved in it, and the third is the
    // member.
    let (tgt, mem, target_name) = match e1 {
        InAnnotation::Prefixo => {
            let Some(n2) = n2 else {
                inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
                return;
            };
            let b2 = inf.program.lookup_prefixed_na_unidade(unit, n1.sym, n2.sym);
            match classify_in_annotation(inf, b2) {
                InAnnotation::Prefixo | InAnnotation::Outro => {
                    inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
                    return;
                }
                x => (x, m.name.get(2).copied(), n2),
            }
        }
        x => (x, n2, n1),
    };
    let invalid = match tgt {
        InAnnotation::Nada => {
            let t = inf.interner.resolve(target_name.sym).to_string();
            inf.warning_with_code(c::UNDEFINED_ANNOTATION, span, &[&t]);
            return;
        }
        InAnnotation::Classe(k) => {
            let kind = inf.program.class(k).kind;
            let is_class = matches!(kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication);
            // `C.new` is the unnamed constructor.
            let ctor = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| {
                let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                let key = n.map(|n| n.sym).or(inf.sym.empty);
                key.and_then(|ch| inf.program.class(k).constructors.get(&ch).copied()).is_some()
            };
            if kind == dartforge_elements::model::ClassKind::ExtensionType {
                // The (primary) constructor of an extension type: the model does not put
                // it in the table by name; no report.
                false
            } else if is_class && args {
                let found = ctor(inf, mem);
                // The constructor exists and is not `const`
                // (`constant_verifier.dart:103-115`): on the whole annotation.
                if found {
                    let n = mem.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let key = n.map(|n| n.sym).or(inf.sym.empty);
                    let e_const = key
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_);
                    if !e_const {
                        inf.warning_with_code(c::NON_CONSTANT_ANNOTATION_CONSTRUCTOR, span, &[]);
                    }
                }
                !found
            } else {
                // Without an argument list, an annotation that resolves to a `const`
                // constructor is `NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS`
                // (`constant_verifier.dart:103-128`), on the whole annotation; a
                // constructor that is not `const` has its own code.
                let const_constructor = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| -> bool {
                    let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let key = n.map(|n| n.sym).or(inf.sym.empty);
                    key
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_)
                };
                let of_member = match mem {
                    Some(n) => match static_getter(inf, Some(k), None, n.sym) {
                        Some(g) => Some(g != Some(true) || args),
                        None => None,
                    },
                    None => None,
                };
                match of_member {
                    Some(invalid) => invalid,
                    None => {
                        let found = ctor(inf, mem);
                        if found && !args && is_class && const_constructor(inf, mem) {
                            inf.warning_with_code(c::NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS, span, &[]);
                        }
                        !found
                    }
                }
            }
        }
        InAnnotation::Extensao(x) => match mem.and_then(|n| static_getter(inf, None, Some(x), n.sym)) {
            Some(g) => g != Some(true) || args,
            None => true,
        },
        InAnnotation::Getter(g) => g != Some(true) || args,
        // Alias (`_typeAliasConstructorInvocation`/`_typeAliasGetter`, 3.6.2
        // `annotation_resolver.dart:274-285`, `:307-318`, `:383-413`): with
        // arguments and a class target, it is a creation (the constructor decides,
        // without a report here); otherwise the target's getter by the second
        // name: without it (`@V`, or a function alias), `INVALID_ANNOTATION`.
        InAnnotation::Alias(td) => {
            let tgt = inf.outline.typedefs.get(td.0 as usize).map(|d| d.target_type);
            let target_class = tgt.and_then(|t| match inf.table.get(t) {
                Type::Interface { class, .. } => Some(*class),
                _ => None,
            });
            if args && target_class.is_some() {
                false
            } else {
                match (target_class, mem) {
                    (Some(k), Some(n)) => match static_getter(inf, Some(k), None, n.sym) {
                        Some(g) => g != Some(true) || args,
                        None => true,
                    },
                    _ => true,
                }
            }
        }
        InAnnotation::Prefixo | InAnnotation::Outro => true,
    };
    if invalid {
        inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
    }
}

/// An annotation: `@x`, `@C(args)`, `@C.the_name(args)`, `@p.C(args)`.
fn annotation(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, _ext: Option<dartforge_elements::model::ExtensionId>, m: &ast::Annotation) {
    validate_annotation(inf, unit, cls, m);
    unvalidated_annotation(inf, unit, cls, m);
}

/// The annotation's arguments (inference), without name validation.
fn unvalidated_annotation(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, m: &ast::Annotation) {
    unvalidated_annotation_with(inf, unit, cls, None, m, &[]);
}

/// Like [`unvalidated_annotation`], with extra type parameters in scope
/// (those of the declaration whose type parameter the annotation marks).
fn unvalidated_annotation_with(
    inf: &mut BodyInferrer<'_>,
    unit: UnitId,
    cls: Option<ClassId>,
    extension: Option<dartforge_elements::model::ExtensionId>,
    m: &ast::Annotation,
    types: &[(dartforge_intern::SymbolId, TypeParamId)],
) {
    let Some(args) = &m.arguments else { return };
    let mut cx = Body::new(inf, unit, cls, extension, true);
    for &(the_name, p) in types {
        cx.declare_type_param(the_name, p);
    }
    let names: Vec<dartforge_intern::SymbolId> = m.name.iter().map(|n| n.sym).collect();
    // Class and constructor.
    let (c, ctor) = match names.as_slice() {
        [c] => (inf.program.lookup_na_unidade(unit, *c).and_then(|b| b.getter), None),
        [a, b] => match inf.program.lookup_na_unidade(unit, *a).and_then(|x| x.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
            _ => (inf.program.lookup_prefixed_na_unidade(unit, *a, *b).and_then(|x| x.getter), None),
        },
        [p, c, n] => (inf.program.lookup_prefixed_na_unidade(unit, *p, *c).and_then(|x| x.getter), Some(*n)),
        _ => (None, None),
    };
    let u = inf.core.unknown;
    // `@B<T>()` with `B` a class alias: the arguments against the alias's
    // type parameters (`AnnotationInferrer` with the alias's constructor).
    let c_original = c;
    let (c, params_do_alias) = match c {
        Some(Element::Typedef(td)) => {
            let tgt = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(tgt).clone() {
                Type::Interface { class, .. } => (Some(Element::Class(class)), Some(inf.outline.typedefs[td.0 as usize].type_params.to_vec())),
                _ => (c, None),
            }
        }
        _ => (c, None),
    };
    if let Some(Element::Class(c)) = c {
        let key = ctor.or(inf.sym.empty);
        if let Some(f) = key.and_then(|k| inf.program.class(c).constructors.get(&k).copied()) {
            let explicit: Option<Vec<TypeId>> = if m.type_args.is_empty() {
                None
            } else {
                Some(m.type_args.iter().map(|&t| inf.type_argument_type(&cx, t)).collect())
            };
            // `AnnotationInferrer`'s `_needsTypeArgumentBoundsCheck`.
            if let Some(ex) = &explicit {
                let params = params_do_alias.clone().unwrap_or_else(|| inf.outline.classes[c.0 as usize].type_params.to_vec());
                // `_reportWrongNumberOfTypeArguments` (`invocation_inferrer.dart:316-327`)
                // with `AnnotationInferrer`'s code: `{0}` is the constructor's raw function
                // type, at the `<…>` list.
                if ex.len() != params.len()
                    && let Some(sp) = super::calls::type_list_range(inf, unit, &m.type_args)
                {
                    // The constructor's type as a generic function over the class's type
                    // parameters (or the alias's, with its target).
                    let mut raw = inf.constructor_signature(c, f);
                    if params_do_alias.is_some()
                        && let Some(Element::Typedef(td)) = c_original
                        && let Type::Interface { args: target_args, .. } = inf.table.get(inf.outline.typedefs[td.0 as usize].target_type).clone()
                    {
                        let formals = inf.outline.classes[c.0 as usize].type_params.to_vec();
                        let map = inf.map(&formals, &target_args);
                        raw = inf.subst(raw, &map);
                    }
                    if let Type::Function { ret, positional, optional, named, .. } = inf.table.get(raw).clone() {
                        raw = inf.table.intern(Type::Function { type_params: params.clone().into_boxed_slice(), ret, positional, optional, named, nullable: false });
                    }
                    let text = inf.table.format(raw, inf.interner, inf.program);
                    let (n, d) = (params.len().to_string(), ex.len().to_string());
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS, sp, &[&text, &n, &d]);
                }
                if ex.len() == params.len() {
                    let (ex, nodes) = (ex.clone(), m.type_args.to_vec());
                    super::calls::check_explicit_bounds(inf, unit, &params, &ex, &nodes, m.span, None);
                }
            }
            if params_do_alias.is_some() {
                for x in args.args.iter() {
                    infer_free(inf, &mut cx, x.value);
                }
                return;
            }
            // `_reportNotEnoughPositionalArguments` with `Annotation`: the
            // `identifier` of the prefixed name (`@A.the_name` → `the_name`, `@p.A` → `A`),
            // or `<the_name>.new`; `MISSING_REQUIRED_ARGUMENT` goes at the constructor's
            // identifier, or at the class's.
            let prefix0 = m.name.first().is_some_and(|n| inf.program.prefixos_na_unidade(unit).contains_key(&n.sym));
            let cited_name = match m.name.as_slice() {
                [a] => format!("{}.new", inf.interner.resolve(a.sym)),
                [a, b] if !m.type_args.is_empty() && !prefix0 => {
                    let _ = b;
                    format!("{}.new", inf.interner.resolve(a.sym))
                }
                [_, b, ..] => inf.interner.resolve(b.sym).to_string(),
                [] => String::new(),
            };
            let entity = match m.name.as_slice() {
                [a] => a.span,
                [_, b] => b.span,
                [_, _, c] => c.span,
                _ => m.span,
            };
            super::calls::define_target(inf, Some(cited_name), entity);
            super::calls::build(inf, &mut cx, None, c, Some(f), explicit, args, u);
            inf.arity_target = None;
            return;
        }
    }
    for x in args.args.iter() {
        infer_free(inf, &mut cx, x.value);
    }
}

/// `_checkReturnExpression` of a closure whose return is the context's `r`
/// (wrapped: `displayed`): `RETURN_OF_INVALID_TYPE_FROM_CLOSURE`
/// (`an611:src/error/return_type_verifier.dart:150-170`), with the returned
/// type and the closure's.
fn closure_returns(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, r: TypeId, displayed: TypeId) {
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    for &(e, s) in &fc.returned_expressions {
        let error = match fc.modifier {
            AsyncModifier::None => {
                if matches!(inf.table.get(s), Type::Void) {
                    !e_void_dyn(inf, r)
                } else {
                    !inf.assignable(s, r)
                }
            }
            AsyncModifier::Async => {
                let tv = inf.flatten(r);
                let fs = inf.flatten(s);
                if matches!(inf.table.get(tv), Type::Void) {
                    !matches!(inf.table.get(fs), Type::Void | Type::Dynamic | Type::Null)
                } else if matches!(inf.table.get(fs), Type::Void) {
                    !e_void_dyn(inf, tv)
                } else {
                    !inf.assignable(s, tv) && !inf.sub(fs, tv)
                }
            }
            _ => false,
        };
        if error {
            let sp = inf.span_expr(cx.unit, e);
            let of = inf.table.format(s, inf.interner, inf.program);
            let for_ = inf.table.format(displayed, inf.interner, inf.program);
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::RETURN_OF_INVALID_TYPE_FROM_CLOSURE, sp, &[&of, &for_]);
        }
    }
}

/// `=> e`: the analyzer's `verifyExpressionFunctionBody`: a return (after
/// `flatten` in `async`) of `void` accepts any expression; the rest is the
/// `return e;` rule ([`verify_return`]).
fn verify_expression_return(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, e: ExprId, t: TypeId) {
    let Some(r) = fc.the_return else { return };
    let flattened = if fc.modifier == AsyncModifier::None { r } else { inf.flatten(r) };
    if matches!(inf.table.get(flattened), Type::Void) {
        return;
    }
    verify_return(inf, cx, fc, e, t);
}

/// `ReturnTypeVerifier`'s `_checkReturnExpression`: `return e;` (or `=> e`)
/// with `e` of type `S` in an executable with declared return `T`. In
/// generators, closures and generative constructors nothing comes out of
/// here.
pub(crate) fn verify_return(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, e: ExprId, s: TypeId) {
    // `_checkReturnExpression`: with an illegal declared return, nothing.
    if !fc.legal_return {
        return;
    }
    let (Some(t), Some(exe)) = (fc.the_return, fc.executable.as_ref()) else { return };
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    let e_void_dyn_null = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic | Type::Null);
    let error = match fc.modifier {
        AsyncModifier::None => {
            if matches!(inf.table.get(t), Type::Void) && !e_void_dyn_null(inf, s) {
                true
            } else if matches!(inf.table.get(s), Type::Void) {
                !e_void_dyn(inf, t)
            } else if let Some(field) = single_record_field(inf, cx, t, s, e)
                && inf.assignable(field, s)
            {
                // `(int,) f() => (1);`: the parenthesis was meant to be a record.
                let sp = inf.span_expr(cx.unit, e);
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA,
                    sp,
                    &[],
                );
                return;
            } else {
                !inf.assignable(s, t)
            }
        }
        AsyncModifier::Async => {
            let tv = inf.future_value_type(t);
            let fs = inf.flatten(s);
            if matches!(inf.table.get(tv), Type::Void) && !e_void_dyn_null(inf, fs) {
                true
            } else if matches!(inf.table.get(fs), Type::Void) {
                !e_void_dyn(inf, tv)
            } else {
                !inf.assignable(s, tv) && !inf.sub(fs, tv)
            }
        }
        AsyncModifier::SyncStar | AsyncModifier::AsyncStar => false,
    };
    if !error {
        return;
    }
    use super::body::ExecutableKind as E;
    use dartforge_diagnostics::codes::compile_time_error as c;
    let code = match exe.kind {
        E::Funcao => c::RETURN_OF_INVALID_TYPE_FROM_FUNCTION,
        E::Method => c::RETURN_OF_INVALID_TYPE_FROM_METHOD,
        E::Construtor => c::RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR,
    };
    let sp = inf.span_expr(cx.unit, e);
    let of = inf.table.format(s, inf.interner, inf.program);
    let for_ = inf.table.format(t, inf.interner, inf.program);
    let the_name = exe.nome.clone();
    inf.warning_with_code(code, sp, &[&of, &for_, &the_name]);
}

/// `T` is a record with a single positional field, `S` is not a record and
/// `e` is a parenthesized expression: the field's type.
fn single_record_field(inf: &BodyInferrer<'_>, cx: &Body, t: TypeId, s: TypeId, e: ExprId) -> Option<TypeId> {
    let Type::Record { positional, named, .. } = inf.table.get(t) else { return None };
    if positional.len() != 1 || !named.is_empty() || matches!(inf.table.get(s), Type::Record { .. }) {
        return None;
    }
    let field = positional[0];
    matches!(inf.program.unit(cx.unit).ast.expr(e).kind, ast::ExprKind::Parenthesized(_)).then_some(field)
}

impl<'a> BodyInferrer<'a> {
    /// **futureValueType**(`T`) of a declared return: `Future<S>`, `FutureOr<S>`
    /// (nullable or not) give `S`; `void` and `dynamic` stay; the rest is
    /// `Object?`.
    pub(crate) fn future_value_type(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Void | Type::Dynamic => t,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Kind and display name of a declared executable.
    pub(crate) fn declared_executable(&self, f: FunctionElementId) -> Option<super::body::Executable> {
        use super::body::{ExecutableKind as E, Executable};
        let fe = self.program.function(f);
        let the_name = self.interner.resolve(fe.name);
        if matches!(fe.node, FunctionRef::Constructor { .. }) {
            let cls = self.interner.resolve(self.program.class(fe.class?).name).to_string();
            let the_name = if the_name.is_empty() || the_name == "new" { cls } else { format!("{cls}.{the_name}") };
            return Some(Executable { kind: E::Construtor, nome: the_name });
        }
        let the_kind = match fe.kind {
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => E::Funcao,
            _ if fe.class.is_some() || fe.extension.is_some() => E::Method,
            _ => E::Funcao,
        };
        // A setter is displayed without the key's `=`; `operator ==` stays as is.
        let the_name = if fe.kind == FunctionKind::Setter { the_name.strip_suffix('=').unwrap_or(the_name) } else { the_name };
        Some(Executable { kind: the_kind, nome: the_name.to_string() })
    }
}
