//! Expression inference: context (schema) downwards, static type upwards
//! (`inference.md`, "Expression inference").
//!
//! Chains with `?.` do *null-shorting*: the inner nodes of the chain get the
//! non-nullable type and only the node that ends the chain gets the `?`
//! ([`infer_at`] returns whether the chain is short-circuiting).

use super::BodyInferrer;
use super::body::{Base, Body, Local, MemberName};
use super::calls;
use super::collections;
use super::flow::Flow;
use super::functions;
use super::members::{InterfaceMember, Search};
use super::patterns;
use super::shorthands;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infers `e` in context `ctx` (the unknown `_` = no context).
pub(crate) fn infer(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> TypeId {
    // `TypeAnalyzer.analyzeExpression` (`type_analyzer.dart:557-561`): the
    // `dynamic` schema becomes `_` (`InvalidType` does not).
    let ctx = if matches!(inf.table.get(ctx), Type::Dynamic) && !inf.table.is_invalid(ctx) {
        inf.core.unknown
    } else {
        ctx
    };
    let mark = cx.chains.len();
    let (t, short_circuit) = infer_at(inf, cx, e, ctx, false);
    close_chain(inf, cx, mark);
    let t = if short_circuit {
        let t = inf.nullable(t);
        record_type(inf, cx, e, t);
        t
    } else {
        t
    };
    record_implicit_call(inf, cx, e, t, ctx);
    t
}

/// `_insertImplicitCallReference` (`resolver.dart:4085-4142`) in the
/// expression kinds whose `visit…` calls it: with `getImplicitCallMethod`
/// (`error_detection_helpers.dart:312-334`), the expression becomes `.call`.
fn record_implicit_call(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, ctx: TypeId) {
    let inserting_kind = matches!(
        &ast(inf, cx).expr(e).kind,
        ExprKind::As { .. }
            | ExprKind::Assign { .. }
            | ExprKind::Await(_)
            | ExprKind::Binary { .. }
            | ExprKind::Cascade { .. }
            | ExprKind::Call { .. }
            | ExprKind::InstanceCreation { .. }
            | ExprKind::Index { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Identifier(_)
            | ExprKind::Property { .. }
            | ExprKind::This
    );
    if !inserting_kind || inf.the_is_unknown(ctx) {
        return;
    }
    // `acceptsFunctionType(context)`: a function type, `Function`, or a
    // `FutureOr` of one of them.
    let mut context = ctx;
    for _ in 0..8 {
        match inf.table.get(context).clone() {
            Type::FutureOr { arg, .. } => context = arg,
            _ => break,
        }
    }
    let accepts =
        matches!(inf.table.get(context), Type::Function { .. }) || context == inf.core.function;
    if !accepts {
        return;
    }
    // Non-nullable type parameter: the bound.
    let mut typ = t;
    let mut seen = std::collections::HashSet::new();
    loop {
        match inf.table.get(typ).clone() {
            Type::TypeParameter { param, nullable } => {
                if nullable || !seen.insert(param) {
                    return;
                }
                typ = inf.table.param(param).bound;
            }
            Type::Intersection { bound, .. } => typ = bound,
            _ => break,
        }
    }
    if !matches!(
        inf.table.get(typ),
        Type::Interface {
            nullable: false,
            ..
        }
    ) || typ == inf.core.function
    {
        return;
    }
    let Some(call) = inf.sym.call else { return };
    if let Some(m) = inf.interface_member(typ, call, false).filter(|m| m.method) {
        inf.body_types.units[cx.unit.0 as usize]
            .implicit_calls
            .insert(e);
        // A generic `call` in a function context: the instantiation from the
        // context, with the `COULD_NOT_INFER`s at the expression.
        let _ = instantiate_in_context_with(inf, cx, e, m.ty, context, false);
    }
}

/// End of a chain with `?.`: the receiver's promotion only held inside it;
/// afterwards, the flow is the join of "was null" (the one from before the
/// first `?.`) with having gone through the chain.
fn close_chain(inf: &mut BodyInferrer<'_>, cx: &mut Body, mark: usize) {
    if cx.chains.len() > mark {
        let before = cx.chains[mark].clone();
        cx.chains.truncate(mark);
        let after = cx.flow.clone();
        cx.flow = inf.join(&before, &after);
    }
}

/// Infers without a context.
pub(crate) fn infer_free(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> TypeId {
    let u = inf.core.unknown;
    infer(inf, cx, e, u)
}

pub(crate) fn record_type(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId) {
    inf.body_types.units[cx.unit.0 as usize].set_type(e, t);
}

pub(crate) fn resolver(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, r: Resolved) {
    let table = &mut inf.body_types.units[cx.unit.0 as usize];
    if inf.record_locals
        && let Resolved::Local(id) = &r
    {
        table.local_declarations.insert(e, cx.local(*id).offset);
    }
    table.set_resolved(e, r);
}

/// Captures the lexical scope when `the_name` is the identifier probed by the
/// LSP.
pub(crate) fn probe_scope_at(inf: &mut BodyInferrer<'_>, cx: &Body, the_name: ast::Name) {
    if inf.scope_probe == Some((cx.unit, the_name.span.start)) && inf.probed_scope.is_none() {
        inf.probed_scope = Some(cx.visible_scope());
    }
}

fn ast<'p>(inf: &BodyInferrer<'p>, cx: &Body) -> &'p ast::Ast {
    &inf.program.unit(cx.unit).ast
}

/// What a simple name denotes at the current point.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NameRef {
    Local(LocalId),
    TypeParam(crate::table::TypeParamId),
    /// Built-in pseudo-type (`dynamic`, `Never`), without an element in the
    /// library's namespace: as a value, it is a type literal (`Type`).
    BuiltinType,
    Element(Element),
    /// Member declared in the body of the enclosing class/extension.
    LexicalMember(dartforge_elements::model::FunctionElementId, bool),
    ConstantEnum(dartforge_elements::model::VariableId),
    Prefix,
    /// Not found lexically, but there is a `this`: `this.the_name`.
    ImplicitThis,
    /// A block local declared after this use (the name in the declaration).
    Forward(dartforge_diagnostics::Span),
    None_,
}

/// Lexical lookup of a name (`setter` = write context).
pub(crate) fn resolve_name(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    the_name: SymbolId,
    setter: bool,
) -> NameRef {
    match cx.lookup(the_name) {
        Some(MemberName::Local(id)) => return NameRef::Local(id),
        Some(MemberName::TypeParam(p)) => return NameRef::TypeParam(p),
        Some(MemberName::Forward(s)) => return NameRef::Forward(s),
        None => {}
    }
    // Members declared in the body of the class/extension.
    if cx.class.is_some() || cx.extension.is_some() {
        // The container's scope keeps getter and setter under the base name: a
        // read that only finds the setter stops there (`LexicalLookup.resolveGetter`).
        let setter_key = inf.setter_key(the_name);
        let order: Vec<Option<SymbolId>> = if setter {
            vec![setter_key, Some(the_name)]
        } else {
            vec![Some(the_name), setter_key]
        };
        for the_key in order.into_iter().flatten() {
            if let Some((f, static_)) =
                inf.lexical_declared_member(cx.class, cx.extension, the_key, None)
            {
                return NameRef::LexicalMember(f, static_);
            }
        }
        if let Some(c) = cx.class
            && let Some(&v) = inf
                .program
                .class(c)
                .enum_constants
                .iter()
                .find(|&&v| inf.program.variable(v).name == the_name)
        {
            return NameRef::ConstantEnum(v);
        }
        if let Some(x) = cx.extension
            && let Some(&v) = inf
                .program
                .extension(x)
                .fields
                .iter()
                .find(|&&v| inf.program.variable(v).name == the_name)
        {
            return NameRef::Element(Element::Variable(v));
        }
    }
    if let Some(b) = inf.program.lookup_in_unit(cx.unit, the_name) {
        let el = if setter {
            b.setter.or(b.getter)
        } else {
            b.getter.or(b.setter)
        };
        if let Some(el) = el {
            if let Element::Prefix(..) = el {
                return NameRef::Prefix;
            }
            return NameRef::Element(el);
        }
    }
    if inf
        .program
        .prefixes_in_unit(cx.unit)
        .contains_key(&the_name)
    {
        return NameRef::Prefix;
    }
    if matches!(inf.interner.resolve(the_name), "dynamic" | "Never") {
        return NameRef::BuiltinType;
    }
    if cx.this_type.is_some() && !cx.the_static {
        return NameRef::ImplicitThis;
    }
    NameRef::None_
}

/// The `thisType` of `ResolverVisitor` (`_setupThisType`,
/// `an611:src/generated/resolver.dart:4180-4193`): the class's `this` type
/// or the enclosing extension's extended type, in **any** member: static,
/// factory, field or constructor initializer. The implicit `this` lookup of
/// a name that does not resolve lexically uses that type even where `this`
/// is not available (the error then becomes the instance member access
/// one, not the undefined name one).
fn analyzer_this_type(inf: &mut BodyInferrer<'_>, cx: &Body) -> Option<TypeId> {
    if cx.this_type.is_some() {
        return cx.this_type;
    }
    if let Some(c) = cx.class {
        return Some(inf.class_this_type(c));
    }
    cx.extension
        .map(|x| inf.outline.extensions[x.0 as usize].on)
}

/// What the analyzer's lexical lookup finds for a **read** of a name
/// (`LexicalLookup.resolveGetter`, `an611:src/dart/resolver/lexical_lookup.dart:17-33`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lexical {
    /// Getter, method, variable, type, prefix…: the read resolves.
    Getter,
    /// Only an instance setter declared in the container: the lookup goes on
    /// through the implicit `this`.
    InstanceSetter,
    /// Only a setter that is not an instance one (top-level, static, imported):
    /// recovery: the read has no element, and the lookup stops here.
    StandaloneSetter,
    /// Nothing in the lexical scope.
    Nothing,
}

fn lexical_read_lookup(inf: &mut BodyInferrer<'_>, cx: &Body, the_name: SymbolId) -> Lexical {
    match resolve_name(inf, cx, the_name, false) {
        NameRef::None_ | NameRef::ImplicitThis => Lexical::Nothing,
        NameRef::LexicalMember(f, static_) => {
            if inf.program.function(f).kind == FunctionKind::Setter {
                // `noGetterIsPossible` (3.6.2 `method_invocation_resolver.dart:616-628`):
                // a static setter or one declared in an extension (also an instance one)
                // is the accessed property; no getter is looked for.
                if static_ || inf.program.function(f).extension.is_some() {
                    Lexical::StandaloneSetter
                } else {
                    Lexical::InstanceSetter
                }
            } else {
                Lexical::Getter
            }
        }
        NameRef::Element(Element::Function(f))
            if inf.program.function(f).kind == FunctionKind::Setter =>
        {
            // Top-level setter (declared or imported) without a getter of the same
            // name.
            match inf.program.lookup_in_unit(cx.unit, the_name) {
                Some(b) if b.getter.is_none() => Lexical::StandaloneSetter,
                _ => Lexical::Getter,
            }
        }
        _ => Lexical::Getter,
    }
}

/// What the implicit `this` lookup finds (`ThisLookup`,
/// `an611:src/dart/resolver/this_lookup.dart:20-83`, which calls
/// `TypePropertyResolver.resolve` with the static recovery of
/// `_lookupInterfaceType`, `an611:src/dart/resolver/type_property_resolver.dart:251-281`).
#[derive(Debug, Clone, Copy)]
enum ThroughThis {
    /// Instance member (of the interface or of an applicable extension).
    Instance,
    /// Static member of the class or of a superclass/mixin (recovery):
    /// `(declaring class, is method)`.
    Static(ClassId, bool),
    Missing,
    /// A nullable receiver or one that is not an interface: the analyzer goes
    /// other ways (null errors); we report nothing.
    Uncertain,
}

fn lookup_through_this(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    this: TypeId,
    the_name: SymbolId,
    setter: bool,
) -> ThroughThis {
    let cls = match inf.table.get(this) {
        Type::Interface {
            class,
            nullable: false,
            ..
        } => *class,
        Type::ExtensionType {
            decl,
            nullable: false,
            ..
        } => *decl,
        _ => return ThroughThis::Uncertain,
    };
    match inf.lookup_member(cx.lib, this, the_name, setter) {
        Search::Finding(_) => ThroughThis::Instance,
        Search::Dynamic | Search::NeverType => ThroughThis::Uncertain,
        Search::Missing => match inf.static_recovery(cx.lib, cls, the_name, setter) {
            Some((owner, method)) => ThroughThis::Static(owner, method),
            None => ThroughThis::Missing,
        },
    }
}

/// The error of accessing an instance member without `this`
/// (`_checkForInvalidInstanceMemberAccess`,
/// `an611:src/generated/error_verifier.dart:3976-4038`): a static method
/// (and what it nests), a factory constructor, a constructor initializer
/// (field ones are left out: see the `Raiz::Nothing` arm). `None` where `this`
/// is available.
fn instance_without_this_error(
    inf: &BodyInferrer<'_>,
    cx: &Body,
) -> Option<dartforge_diagnostics::Code> {
    use dartforge_diagnostics::codes::compile_time_error as c;
    if cx.this_type.is_some() && !cx.the_static {
        return None;
    }
    if cx.class.is_none() && cx.extension.is_none() {
        return None;
    }
    if cx.annotation_access.is_some() {
        return cx.annotation_access;
    }
    match cx.root {
        super::body::Root::Function(_) if cx.static_member => {
            Some(c::INSTANCE_MEMBER_ACCESS_FROM_STATIC)
        }
        super::body::Root::Function(_) => None,
        super::body::Root::Constructor(m) => match &inf.program.unit(cx.unit).ast.member(m).kind {
            ast::MemberKind::Constructor(k) if k.factory => {
                Some(c::INSTANCE_MEMBER_ACCESS_FROM_FACTORY)
            }
            ast::MemberKind::Constructor(_) => Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER),
            _ => None,
        },
        // Initializer of a non-`late` instance field or of a static variable
        // (3.6.2 `error_verifier.dart:3976-4038`); enum constant arguments and
        // annotations use the same body without a root and are left out.
        super::body::Root::Nothing if cx.variable_initializer => {
            Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER)
        }
        super::body::Root::Nothing => None,
    }
}

/// Reports an instance member access where `this` is not available (see
/// [`instance_without_this_error`]).
fn warn_instance_without_this(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    if let Some(code) = instance_without_this_error(inf, cx) {
        let the_name = inf.interner.resolve(n.sym).to_string();
        if code.info().name == "implicit_this_reference_in_initializer" {
            inf.warning_with_code(code, n.span, &[&the_name]);
        } else {
            inf.warning_with_code(code, n.span, &[]);
        }
    }
}

/// Unqualified reference to an inherited static member
/// (`_checkForUnqualifiedReferenceToNonLocalStaticMember`,
/// `an611:src/generated/error_verifier.dart:5660-5700`, and
/// `_reportInstanceAccessToStaticMember`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:208-227`).
fn warn_unqualified_static(inf: &mut BodyInferrer<'_>, cx: &Body, owner: ClassId, n: ast::Name) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let code = if cx.extension.is_some() {
        c::UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE
    } else {
        c::UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER
    };
    let the_name = inf
        .interner
        .resolve(inf.program.class(owner).name)
        .to_string();
    inf.warning_with_code(code, n.span, &[&the_name]);
}

/// `p.loadLibrary` of a `deferred` import prefix (`PrefixScope.lookup`,
/// `an611:src/dart/element/scope.dart:576-583`): the deferred library's
/// synthetic `Future<dynamic> Function()` function.
pub(crate) fn load_library(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    prefix: SymbolId,
    the_name: SymbolId,
) -> Option<TypeId> {
    if inf.interner.resolve(the_name) != "loadLibrary" {
        return None;
    }
    let lib = inf.program.library(cx.lib);
    let deferred = lib.imports.iter().any(|im| {
        im.prefix == Some(prefix)
            && im.deferred
            && !inf.program.library(im.library).units.is_empty()
    });
    if !deferred {
        return None;
    }
    let ret = match inf.core.future_class {
        Some(f) => {
            let d = inf.core.dynamic_;
            inf.table.intern(Type::Interface {
                class: f,
                args: Box::new([d]),
                nullable: false,
            })
        }
        None => inf.core.dynamic_,
    };
    Some(inf.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: Box::new([]),
        optional: Box::new([]),
        named: Box::new([]),
        nullable: false,
    }))
}

/// `p.the_name` that the prefix does not have (`_resolveTargetPrefixElement`,
/// `an611:src/dart/resolver/property_element_resolver.dart:744-786`):
/// `UNDEFINED_PREFIXED_NAME`, except the ignored name of an import that
/// does not exist.
pub(crate) fn warn_undefined_prefixed_name(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    prefix: SymbolId,
    name: ast::Name,
) {
    if crate::scope::should_ignore_undefined(
        inf.program,
        inf.interner,
        cx.unit,
        Some(prefix),
        name.sym,
    ) {
        return;
    }
    let the_name = inf.interner.resolve(name.sym).to_string();
    let p = inf.interner.resolve(prefix).to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_PREFIXED_NAME,
        name.span,
        &[&the_name, &p],
    );
}

/// The read of a simple name whose lexical lookup found no getter
/// (`SimpleIdentifierResolver._resolve1`,
/// `an611:src/dart/resolver/simple_identifier_resolver.dart:209-224`, with the
/// lookup of `PropertyElementResolver.resolveSimpleIdentifier`,
/// `an611:src/dart/resolver/property_element_resolver.dart:243-272`): the
/// implicit `this` may still find the member (the error is then the access
/// without `this` or the unqualified static one); otherwise `await` in a
/// function body, the ignored name of an import that does not exist, or an
/// undefined name.
fn undefined_read_name(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name, lexical: Lexical) {
    // `if (node.isSynthetic) return null;` (`_resolve1`): the parser already
    // reported the missing identifier.
    if is_synthetic_name(inf, n) {
        return;
    }
    if lexical != Lexical::StandaloneSetter
        && let Some(this) = analyzer_this_type(inf, cx)
    {
        match lookup_through_this(inf, cx, this, n.sym, false) {
            ThroughThis::Instance => {
                warn_instance_without_this(inf, cx, n);
                return;
            }
            ThroughThis::Static(owner, method) => {
                // Static method tear-off: `ErrorVerifier` does not report.
                if !method {
                    warn_unqualified_static(inf, cx, owner, n);
                }
                return;
            }
            ThroughThis::Uncertain => return,
            ThroughThis::Missing => {}
        }
    }
    undefined_name_without_this(inf, cx, n);
}

/// `PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD` (`ScopeResolverVisitor.visitSimpleIdentifier`,
/// `an611:src/generated/resolver.dart:5186-5193`): the identifier in a write
/// context (`=`, `op=`, `++`/`--`) is a variable of the guarded pattern whose
/// `when` clause is being analyzed (also inside closures and the guard's
/// `if-case`).
/// `_checkForAssignmentToPrimaryConstructorParameter` (3.13,
/// `error_verifier.dart:3302-3327` of the main checkout): a write to a primary
/// constructor parameter inside its initializers or an instance field's
/// (also in a function inside them), at the written name.
pub(crate) fn write_in_primary(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    id: LocalId,
    span: dartforge_diagnostics::Span,
) {
    if cx.primary_parameters.contains(&id) {
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER, span, &[]);
    }
}

pub(crate) fn write_in_guard(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    id: LocalId,
    span: dartforge_diagnostics::Span,
) {
    if cx.variables_in_guard.contains(&id) {
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD, span, &[]);
    }
}

/// The reference (read or write) to an inconsistent join variable of cases
/// that share the body (`ResolverVisitor.finishJoinedPatternVariable`,
/// `an611:src/generated/resolver.dart:878-915`), with the name.
pub(crate) fn join_reference(inf: &mut BodyInferrer<'_>, cx: &Body, id: LocalId, n: ast::Name) {
    if let Some(&code) = cx.inconsistent_joins.get(&id) {
        let the_name = inf.interner.resolve(n.sym).to_string();
        inf.warning_with_code(code, n.span, &[&the_name]);
    }
}

/// The empty name the parser's recovery inserts in place of a missing
/// identifier (a synthetic token in the analyzer).
fn is_synthetic_name(inf: &BodyInferrer<'_>, n: ast::Name) -> bool {
    n.span.start == n.span.end && inf.interner.resolve(n.sym).is_empty()
}

/// The end of [`undefined_read_name`]: `await` in a function body, an
/// ignored name or `UNDEFINED_IDENTIFIER`.
pub(crate) fn undefined_name_without_this(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    let text = inf.interner.resolve(n.sym).to_string();
    let in_function = !matches!(cx.root, super::body::Root::Nothing) || !cx.functions.is_empty();
    if text == "await" && in_function {
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::UNDEFINED_IDENTIFIER_AWAIT,
            n.span,
            &[],
        );
        return;
    }
    if crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, n.sym) {
        return;
    }
    // The prefix of an import whose target does not exist (`import 'dart:foo' as foo;`):
    // the `PrefixElement` exists anyway, and `foo.bar` is a prefixed name that
    // does not resolve, without a report here.
    let lib = inf.program.unit(cx.unit).library;
    let is_prefix = inf.program.library(lib).units.iter().any(|&u| {
        inf.program.unit(u).unit.directives.iter().any(|d| matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(p), .. } if p.sym == n.sym))
    });
    if is_prefix {
        return;
    }
    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, text);
    inf.warning(msg, n.span);
}

/// The invocation `the_name(args)` without a target (`MethodInvocationResolver._resolveReceiverNull`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:559-657`) when the
/// lexical lookup finds no getter: without `this`, `UNDEFINED_FUNCTION`; with
/// `this`, the lookup on it (an instance member found follows the common path;
/// an inherited static, unqualified static; nothing, `UNDEFINED_METHOD` with the
/// class name). Returns whether it handled the invocation (and reported what
/// there was to report): the caller infers the arguments without a context and
/// gives `dynamic`.
pub(crate) fn undefined_targetless_invocation(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    n: ast::Name,
) -> bool {
    use dartforge_diagnostics::codes::compile_time_error as c;
    if cx.wildcard == Some(n.sym) {
        return false;
    }
    let lexical = lexical_read_lookup(inf, cx, n.sym);
    if lexical == Lexical::Getter {
        return false;
    }
    let Some(this) = analyzer_this_type(inf, cx) else {
        if !crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, n.sym) {
            let the_name = inf.interner.resolve(n.sym).to_string();
            inf.warning_with_code(c::UNDEFINED_FUNCTION, n.span, &[&the_name]);
        }
        return true;
    };
    let type_name = |inf: &BodyInferrer<'_>| -> String {
        match inf.table.get(this) {
            Type::Interface { class, .. } => inf
                .interner
                .resolve(inf.program.class(*class).name)
                .to_string(),
            Type::ExtensionType { decl, .. } => inf
                .interner
                .resolve(inf.program.class(*decl).name)
                .to_string(),
            Type::Function { .. } => "Function".to_string(),
            _ => "<unknown>".to_string(),
        }
    };
    if lexical == Lexical::StandaloneSetter {
        // Top-level, extension or static setter: no getter is possible.
        let the_name = inf.interner.resolve(n.sym).to_string();
        let typ = type_name(inf);
        inf.warning_with_code(c::UNDEFINED_METHOD, n.span, &[&the_name, &typ]);
        return true;
    }
    // A potentially nullable `this` where it is available (an extension on a
    // nullable type): the common path resolves through `Object`/extensions or
    // reports the unchecked use (`ThisLookup` → `TypePropertyResolver`).
    // (An extension type is only nullable through `?`, `type_property_resolver.dart:91-96`.)
    if let Some(t) = cx.this_type
        && !cx.the_static
        && !inf.is_dynamic(t)
        && !inf.is_non_nullable(t)
        && !matches!(
            inf.table.get(t),
            Type::ExtensionType {
                nullable: false,
                ..
            }
        )
    {
        return false;
    }
    inf.extension_ambiguity = None;
    match lookup_through_this(inf, cx, this, n.sym, false) {
        // Ambiguous extensions (`_resolveReceiverNull` → `TypePropertyResolver`):
        // only the ambiguity, at the name.
        ThroughThis::Uncertain if inf.extension_ambiguity.is_some() => {
            inf.report_extension_ambiguity(n.span);
            true
        }
        // Instance member: where `this` is available, the common path resolves it.
        ThroughThis::Instance if cx.this_type.is_some() && !cx.the_static => false,
        ThroughThis::Instance => {
            warn_instance_without_this(inf, cx, n);
            true
        }
        ThroughThis::Static(owner, _) => {
            warn_unqualified_static(inf, cx, owner, n);
            true
        }
        ThroughThis::Uncertain => true,
        ThroughThis::Missing => {
            let the_name = inf.interner.resolve(n.sym).to_string();
            let typ = type_name(inf);
            inf.warning_with_code(c::UNDEFINED_METHOD, n.span, &[&the_name, &typ]);
            true
        }
    }
}

/// A type reference used as a receiver (`C.m`, `p.C.m`, `C<T>.m`).
#[derive(Debug, Clone)]
pub(crate) enum TypeRef {
    Class(ClassId, Option<Vec<ast::TypeId>>),
    /// Type alias that names a class (`typedef A = B<int>`).
    /// Typedef for a class: the class's arguments (`None`: the typedef only
    /// renames the parameters, and the arguments are the explicit ones or
    /// inferred like the class's).
    Alias(
        ClassId,
        Option<Vec<TypeId>>,
        dartforge_elements::model::TypedefId,
    ),
    Extension(ExtensionId),
}

/// If `e` names a class/extension/typedef (not a value), which one.
pub(crate) fn type_reference(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<TypeRef> {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Element(el) => el,
            _ => return None,
        },
        ExprKind::Property {
            target,
            name,
            null_aware: false,
        } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else {
                return None;
            };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix) {
                return None;
            }
            inf.program
                .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)?
                .getter?
        }
        ExprKind::TypeArguments { target, type_args } => {
            let targs = type_args.to_vec();
            return match type_reference(inf, cx, *target)? {
                TypeRef::Class(c, None) => Some(TypeRef::Class(c, Some(targs))),
                TypeRef::Alias(_, _, td) => {
                    let ex: Vec<TypeId> = targs
                        .iter()
                        .map(|&t| inf.type_argument_type(cx, t))
                        .collect();
                    alias_of(inf, td, Some(ex))
                }
                _ => None,
            };
        }
        _ => return None,
    };
    match el {
        Element::Class(c) => Some(TypeRef::Class(c, None)),
        Element::Extension(x) => Some(TypeRef::Extension(x)),
        Element::Typedef(td) => alias_of(inf, td, None),
        _ => None,
    }
}

/// `e` names (with or without a prefix) a generic type alias whose target is
/// neither a class nor an extension type.
pub(crate) fn alias_without_class(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Element(el) => el,
            _ => return false,
        },
        ExprKind::Property {
            target,
            name,
            null_aware: false,
        } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else {
                return false;
            };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix) {
                return false;
            }
            match inf
                .program
                .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
                .and_then(|b| b.getter)
            {
                Some(el) => el,
                None => return false,
            }
        }
        _ => return false,
    };
    let Element::Typedef(td) = el else {
        return false;
    };
    let d = &inf.outline.typedefs[td.0 as usize];
    !d.type_params.is_empty()
        && !matches!(
            inf.table.get(d.target_type),
            Type::Interface { .. } | Type::ExtensionType { .. }
        )
}

/// `e` (a simple identifier) names a type alias whose target is neither a
/// class nor an extension type (generic or not): its `the_name(…)` is a
/// `MethodInvocation` of an element that is not a function
/// (`_resolveReceiverNull`, `method_invocation_resolver.dart:599`).
pub(crate) fn non_class_type_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind else {
        return false;
    };
    let NameRef::Element(Element::Typedef(td)) = resolve_name(inf, cx, n.sym, false) else {
        return false;
    };
    let d = &inf.outline.typedefs[td.0 as usize];
    // `Null` and `FutureOr` are `InterfaceType` in the analyzer.
    !matches!(
        inf.table.get(d.target_type),
        Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. }
    )
}

/// `e` names a function type alias.
pub(crate) fn function_type_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    named_alias_target(inf, cx, e)
        .is_some_and(|t| matches!(inf.table.get(t), Type::Function { .. }))
}

/// The access `tgt.the_name` with `tgt` the instantiated type literal of a
/// function type alias (`Fn<int>.foo`, `Fn<int>.foo = 1`): infers the target
/// and reports `UNDEFINED_GETTER_ON_FUNCTION_TYPE` (read) or
/// `UNDEFINED_SETTER_ON_FUNCTION_TYPE` (write) at the name, with the type's
/// name as written. Says whether it reported.
pub(crate) fn function_type_literal(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    tgt: ExprId,
    the_name: ast::Name,
    read: bool,
) -> bool {
    let ExprKind::TypeArguments { target: base, .. } = &ast(inf, cx).expr(tgt).kind else {
        return false;
    };
    let base = *base;
    if !function_type_alias(inf, cx, base) {
        return false;
    }
    infer_free(inf, cx, tgt);
    let sp_base = inf.span_expr(cx.unit, base);
    let qualified = inf.program.unit(cx.unit).source[sp_base.start..sp_base.end].to_string();
    let text = inf.interner.resolve(the_name.sym).to_string();
    let code = if read {
        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_GETTER_ON_FUNCTION_TYPE
    } else {
        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_SETTER_ON_FUNCTION_TYPE
    };
    inf.warning_with_code(code, the_name.span, &[&text, &qualified]);
    true
}

/// `e` names an alias whose target is a type parameter (`typedef T<X> = X`).
pub(crate) fn type_parameter_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    named_alias_target(inf, cx, e)
        .is_some_and(|t| matches!(inf.table.get(t), Type::TypeParameter { .. }))
}

/// The target of the alias that `e` (`T` or `p.T`) names.
fn named_alias_target(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<TypeId> {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Element(el) => el,
            _ => return None,
        },
        ExprKind::Property { target, name, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Identifier(p) => inf
                .program
                .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
                .and_then(|b| b.getter)?,
            _ => return None,
        },
        _ => return None,
    };
    let Element::Typedef(td) = el else {
        return None;
    };
    Some(inf.outline.typedefs[td.0 as usize].target_type)
}

/// `e` names an alias whose target is `Null` or `FutureOr<…>` (classes for
/// the analyzer, which rewrites `T<X>.the_name()` as a creation).
pub(crate) fn alias_of_null_or_futureor(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Element(el) => el,
            _ => return false,
        },
        ExprKind::Property { target, name, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Identifier(p) => match inf
                .program
                .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
                .and_then(|b| b.getter)
            {
                Some(el) => el,
                None => return false,
            },
            _ => return false,
        },
        _ => return false,
    };
    let Element::Typedef(td) = el else {
        return false;
    };
    matches!(
        inf.table
            .get(inf.outline.typedefs[td.0 as usize].target_type),
        Type::Null | Type::FutureOr { .. }
    )
}

/// Class and arguments of a typedef used as a class, with the typedef's
/// explicit arguments (or without them).
pub(crate) fn alias_of(
    inf: &mut BodyInferrer<'_>,
    td: dartforge_elements::model::TypedefId,
    explicit: Option<Vec<TypeId>>,
) -> Option<TypeRef> {
    let tgt = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let (class, args) = match inf.table.get(tgt).clone() {
        Type::Interface { class, args, .. }
        | Type::ExtensionType {
            decl: class, args, ..
        } => (class, args),
        // `Null` and `FutureOr<T>` are `InterfaceType` in the analyzer.
        Type::Null => (inf.core.null_class?, Box::new([]) as Box<[TypeId]>),
        Type::FutureOr { arg, .. } => {
            let lib = inf.core.async_library?;
            let the_name = inf.interner.lookup("FutureOr")?;
            let Some(Element::Class(c)) = inf
                .program
                .library(lib)
                .declared
                .get(&the_name)
                .and_then(|b| b.getter)
            else {
                return None;
            };
            (c, Box::new([arg]) as Box<[TypeId]>)
        }
        _ => return None,
    };
    if params.is_empty() {
        return Some(TypeRef::Alias(class, Some(args.to_vec()), td));
    }
    let inst = match explicit {
        Some(ex) if ex.len() == params.len() => ex,
        _ => {
            // A typedef that only renames (`typedef M<K, V> = _M<K, V>`): the
            // arguments are inferred like the class's.
            let renames = args.len() == params.len()
                && args.iter().zip(params.iter()).all(|(&a, &p)| matches!(inf.table.get(a), Type::TypeParameter { param, nullable: false } if *param == p));
            if renames {
                return Some(TypeRef::Alias(class, None, td));
            }
            inf.instantiate_to_bounds(&params)
        }
    };
    let map = inf.map(&params, &inst);
    let args = args.iter().map(|a| inf.subst(*a, &map)).collect();
    Some(TypeRef::Alias(class, Some(args), td))
}

/// Records the nodes of a type reference (type `Type`, resolution of the element).
fn record_type_ref(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let a = ast(inf, cx);
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(b) = inf.program.lookup_in_unit(cx.unit, n.sym)
                && let Some(el) = b.getter
            {
                resolver(inf, cx, e, Resolved::Element(el));
            }
            record_type(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            if let ExprKind::Identifier(p) = &a.expr(*target).kind {
                resolver(inf, cx, *target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf
                    .program
                    .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
                    .and_then(|b| b.getter)
                {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            record_type(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            record_type_ref(inf, cx, t);
            record_type(inf, cx, e, tt);
        }
        _ => {}
    }
}

/// Type of a local variable read (with the definite assignment check).
fn read_local(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    id: LocalId,
    span: dartforge_diagnostics::Span,
) -> TypeId {
    let l = cx.local(id).clone();
    // `checkReadOfNotAssignedLocalVariable`
    // (`an611:src/generated/resolver.dart:643-690`). Also in unreachable
    // code, which inherits the state of the paths leading to it
    // (`return; use(x);` reports).
    if !l.local_function {
        let the_name = inf.interner.resolve(l.name).to_string();
        if l.late {
            if cx.flow.unassigned(id) {
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE,
                    span,
                    &[&the_name],
                );
            }
        } else if !cx.flow.assigned(id) {
            if l.final_ {
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL,
                    span,
                    &[&the_name],
                );
            } else if !inf.the_is_nullable(l.ty) && !inf.is_dynamic(l.ty) {
                // `isPotentiallyNonNullable`: `Null` is not a subtype of the type
                // (a type parameter bounded by `Object?` and an extension type
                // without `Object` too).
                let msg = format!(
                    "{}: '{}'",
                    DEFINITELY_UNASSIGNED_VARIABLE.template, the_name
                );
                inf.warning(msg, span);
            }
        }
    }
    cx.flow.current_type(id, l.ty)
}

/// `EXTENSION_AS_EXPRESSION` (`simple_identifier_resolver.dart:315-323`,
/// `prefixed_identifier_resolver.dart:160-184`): the name (or `p.E`) of an
/// extension used as a value, outside a method or property access target.
/// With the report the node becomes `dynamic`.
fn extension_as_expression(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    sp: dartforge_diagnostics::Span,
) -> bool {
    if let dartforge_frontend::parents::Parent::Expr(p) = inf.parent_of(cx.unit, e) {
        match &ast(inf, cx).expr(p).kind {
            ExprKind::Property { target, .. } | ExprKind::Call { target, .. } if *target == e => {
                return false;
            }
            _ => {}
        }
    }
    let text = inf.program.unit(cx.unit).source[sp.start..sp.end].to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::EXTENSION_AS_EXPRESSION,
        sp,
        &[&text],
    );
    true
}

/// The name with which `_resolve` reports the type argument count of a
/// tear-off: that of a declared function, method or operator (also a local
/// function); `None` for a variable, getter or other expression.
fn referenced_function_name(inf: &BodyInferrer<'_>, cx: &Body, tgt: ExprId) -> Option<String> {
    use crate::resolved::{MemberRef, Resolved};
    let function = |f: dartforge_elements::model::FunctionElementId| {
        let fe = inf.program.function(f);
        matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator)
            .then(|| inf.interner.resolve(fe.name).to_string())
    };
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(tgt)? {
        Resolved::Element(Element::Function(f)) => function(*f),
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        } => function(*f),
        Resolved::ExtensionMember { member, .. } => function(*member),
        // The local function (declared before: the `funcao_local` mark goes
        // away after it is inferred, and the `funcoes_locais` set remains).
        Resolved::Local(id) if cx.local(*id).local_function || cx.local_functions.contains(id) => {
            Some(inf.interner.resolve(cx.local(*id).name).to_string())
        }
        _ => None,
    }
}

/// Type of a read of a top-level element.
fn read_element(inf: &mut BodyInferrer<'_>, el: Element) -> TypeId {
    match el {
        Element::Class(_) | Element::Typedef(_) | Element::Extension(_) => inf.core.type_,
        Element::Variable(v) => inf.variable_type(v),
        Element::Function(f) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
                (FunctionKind::Getter, _) => inf.outline.functions[f.0 as usize].return_type,
                (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize]
                    .parameters
                    .first()
                    .map(|p| p.ty)
                    .unwrap_or(inf.core.dynamic_),
                _ => inf.outline.functions[f.0 as usize].signature,
            }
        }
        Element::Prefix(..) => inf.core.dynamic_,
    }
}

pub(crate) fn lexical_member_resolved(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    f: dartforge_elements::model::FunctionElementId,
    static_: bool,
) -> Resolved {
    let fe = inf.program.function(f);
    if let Some(x) = fe.extension.or(cx.extension)
        && fe.class.is_none()
    {
        return Resolved::ExtensionMember {
            extension: x,
            member: f,
        };
    }
    let class = fe.class.or(cx.class).unwrap_or(ClassId(0));
    let member = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) if static_ => MemberRef::Variable(v),
        _ => MemberRef::Function(f),
    };
    Resolved::Member {
        class,
        member,
        via_super: false,
    }
}

/// `referencedBeforeDeclaration` (`diagnostic_factory.dart:342`): the
/// context is the declaration, at its name.
fn warning_before_declaration(
    inf: &mut BodyInferrer<'_>,
    n: ast::Name,
    msg: String,
    decl: dartforge_diagnostics::Span,
) {
    let text = format!(
        "The declaration of '{}' is here.",
        inf.interner.resolve(n.sym)
    );
    inf.diagnostics
        .push(dartforge_diagnostics::Diagnostic::new(msg, n.span).with_context(decl, text));
    inf.warning_units.push(inf.current_unit);
}

/// Identifier as a value.
fn identifier(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, n: ast::Name) -> TypeId {
    probe_scope_at(inf, cx, n);
    if let Some(&(_, decl)) = cx.pattern_hidden.iter().find(|(s, _)| *s == n.sym) {
        let msg = format!(
            "{}: '{}'",
            REFERENCED_BEFORE_DECLARATION.template,
            inf.interner.resolve(n.sym)
        );
        warning_before_declaration(inf, n, msg, decl);
        return inf.core.dynamic_;
    }
    match resolve_name(inf, cx, n.sym, false) {
        NameRef::Local(id) => {
            resolver(inf, cx, e, Resolved::Local(id));
            join_reference(inf, cx, id, n);
            if inf.invalid_locals.contains(&(cx.unit, cx.local(id).offset)) {
                inf.body_types.units[cx.unit.0 as usize]
                    .invalid_types
                    .insert(e);
            }
            if n.span.start < cx.local(id).offset && !cx.local(id).local_function {
                let msg = format!(
                    "{}: '{}'",
                    REFERENCED_BEFORE_DECLARATION.template,
                    inf.interner.resolve(n.sym)
                );
                let decl = dartforge_diagnostics::Span {
                    start: cx.local(id).offset,
                    end: cx.local(id).offset + (n.span.end - n.span.start),
                };
                warning_before_declaration(inf, n, msg, decl);
            }
            let t = read_local(inf, cx, id, n.span);
            // `whyNotPromoted` of this read (the variable's non-promotion history at
            // this point).
            inf.record_local_non_promotion(cx, e, id);
            t
        }
        NameRef::TypeParam(p) => {
            resolver(inf, cx, e, Resolved::TypeParameter(p));
            // `_checkForTypeParameterReferencedByStatic` of
            // `visitSimpleIdentifier` (3.6.2, `error_verifier.dart:5419-5434`).
            if cx.static_member && crate::resolve::class_param(inf.table, p) {
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, n.span, &[]);
            }
            inf.core.type_
        }
        NameRef::BuiltinType => inf.core.type_,
        NameRef::Element(_) if ambiguous_import(inf, cx, None, n) => {
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::Element(el) => {
            resolver(inf, cx, e, Resolved::Element(el));
            // Only a top-level setter with that name: the read has no element.
            if lexical_read_lookup(inf, cx, n.sym) == Lexical::StandaloneSetter {
                undefined_read_name(inf, cx, n, Lexical::StandaloneSetter);
                return inf.table.invalid(inf.core.dynamic_);
            }
            if let Element::Extension(_) = el
                && extension_as_expression(inf, cx, e, n.span)
            {
                return inf.core.dynamic_;
            }
            read_element(inf, el)
        }
        NameRef::LexicalMember(f, static_) => {
            let r = lexical_member_resolved(inf, cx, f, static_);
            resolver(inf, cx, e, r);
            let only_setter = inf.program.function(f).kind == FunctionKind::Setter;
            // `isInstanceMember` (3.6.2 `extensions.dart:109-119`) only applies to a
            // member of an `InterfaceElement`: an extension member found in the lexical
            // scope is the element read (the getter) or the recovery (the setter,
            // without the implicit `this` lookup: `undefined_identifier`, not the
            // ambiguity with another extension's getter;
            // `static_extension_internal_basename_shadowing_error_test.dart:276`).
            let of_extension = inf.program.function(f).extension.is_some();
            if !static_ && !of_extension {
                // Instance member declared here: through the `this` type (the
                // substitution is the identity).
                if let Some(this) = cx.this_type
                    && let Search::Finding(m) = inf.lookup_member(cx.lib, this, n.sym, false)
                {
                    // Only the setter was found here, the read is `this.the_name`: the element
                    // read is the interface's (inherited) getter, not the setter (the
                    // backends invoke the node's `Resolved`).
                    if only_setter {
                        resolver(inf, cx, e, m.resolved.clone());
                    }
                    return field_read(inf, cx, e, Base::This, m.ty);
                }
            }
            if only_setter {
                // Read of a name that only has a setter in the container: an instance
                // one still looks through the implicit `this`; a static one does not.
                let lexical = if static_ || of_extension {
                    Lexical::StandaloneSetter
                } else {
                    Lexical::InstanceSetter
                };
                undefined_read_name(inf, cx, n, lexical);
                return inf.table.invalid(inf.core.dynamic_);
            }
            if !static_ {
                warn_instance_without_this(inf, cx, n);
            }
            inf.declared_member_type(f, false).0
        }
        NameRef::ConstantEnum(v) => {
            let c = inf.program.variable(v).class;
            if let Some(c) = c {
                resolver(
                    inf,
                    cx,
                    e,
                    Resolved::Member {
                        class: c,
                        member: MemberRef::Variable(v),
                        via_super: false,
                    },
                );
            }
            inf.variable_type(v)
        }
        NameRef::Prefix => {
            // `SimpleIdentifierResolver._resolve1` (3.6.2,
            // `simple_identifier_resolver.dart:199-205`): the prefix outside `p.x`
            // and `p.f()` (argument, `p?.x`, `p[0]`, cascade, `p()`).
            resolver(inf, cx, e, Resolved::Prefix(cx.lib));
            prefix_without_dot(inf, n);
            // `_resolve2` (`simple_identifier_resolver.dart:269-276`): outside
            // `p.x`/`p.f()` the prefix's type is `InvalidType`.
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::ImplicitThis => {
            let this = cx.this_type.unwrap();
            // `ThisLookup.lookupGetter` with a potentially nullable `this` (an
            // extension on a nullable type): `TypePropertyResolver` reports the
            // unchecked use, with the code chosen by the name's parent, and recovers
            // through the interface of the type without `?`.
            if inf.requires_null_check(cx.lib, this, n.sym, false) {
                let code = nullable_this_code(inf, cx, e);
                let the_name = inf.interner.resolve(n.sym).to_string();
                let since = inf.diagnostics.len();
                inf.null_warning(this, code, n.span, &[&the_name]);
                inf.attach_non_promotion(since, cx, None, n.span);
                let nn = inf.non_null(this);
                return match inf.lookup_member(cx.lib, nn, n.sym, false) {
                    Search::Finding(m) => {
                        resolver(inf, cx, e, m.resolved.clone());
                        m.ty
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.lookup_member(cx.lib, this, n.sym, false) {
                Search::Finding(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    field_read(inf, cx, e, Base::This, m.ty)
                }
                // Ambiguous extensions (`ThisLookup.lookupGetter` →
                // `TypePropertyResolver`): the ambiguity at the name, and without a
                // getter `UNDEFINED_IDENTIFIER` too
                // (`simple_identifier_resolver.dart:208-221`).
                Search::Dynamic if inf.extension_ambiguity.is_some() => {
                    inf.report_extension_ambiguity(n.span);
                    undefined_name_without_this(inf, cx, n);
                    inf.table.invalid(inf.core.dynamic_)
                }
                Search::Dynamic => inf.core.dynamic_,
                Search::NeverType => inf.core.never,
                Search::Missing => {
                    undefined_read_name(inf, cx, n, Lexical::Nothing);
                    inf.table.invalid(inf.core.dynamic_)
                }
            }
        }
        NameRef::Forward(decl) => {
            let msg = format!(
                "{}: '{}'",
                REFERENCED_BEFORE_DECLARATION.template,
                inf.interner.resolve(n.sym)
            );
            warning_before_declaration(inf, n, msg, decl);
            // `checkReadOfNotAssignedLocalVariable` (3.6.2 `resolver.dart:643-679`):
            // a local not yet assigned, if `final` (not `late`; `const` does not
            // count), is `READ_POTENTIALLY_UNASSIGNED_FINAL` at the name.
            let forward_final =
                inf.program
                    .unit(cx.unit)
                    .ast
                    .stmts
                    .iter()
                    .any(|st| match &st.kind {
                        ast::StmtKind::Variables(vl) => {
                            vl.final_
                                && !vl.const_
                                && !vl.late
                                && vl.variables.iter().any(|v| v.name.span == decl)
                        }
                        _ => false,
                    });
            if forward_final {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL, n.span, &[&the_name]);
            }
            // The analyzer resolves the name to the local declared ahead (the
            // block's scope already has it): `ReferenceFinder` sees the dependency
            // (`const x = [x];` is a cycle).
            if inf.record_locals {
                inf.body_types.units[cx.unit.0 as usize]
                    .local_declarations
                    .insert(e, decl.start);
            }
            // The element's type at this point: `ResolutionVisitor` already set the
            // written type, or `dynamic` without a type (`resolution_visitor.dart:1397`);
            // inference of the implicit one only comes at the declaration.
            // `Function f = () { x = f; };` gives an `invalid_assignment` of `Function`
            // (`ref_before_declaration_test.dart:71`). The annotation's warnings are the
            // declaration's, reported there.
            let written =
                inf.program
                    .unit(cx.unit)
                    .ast
                    .stmts
                    .iter()
                    .find_map(|st| match &st.kind {
                        ast::StmtKind::Variables(vl)
                            if vl.variables.iter().any(|v| v.name.span == decl) =>
                        {
                            Some(vl.ty)
                        }
                        _ => None,
                    });
            match written.flatten() {
                Some(t) => {
                    let (nd, nu) = (inf.diagnostics.len(), inf.warning_units.len());
                    let r = inf.type_from_annotation(cx, t);
                    inf.diagnostics.truncate(nd);
                    inf.warning_units.truncate(nu);
                    r
                }
                None => inf.core.dynamic_,
            }
        }
        // Only wildcards declare `_` here (3.7): using `_` is an error
        // (`Undefined name '_'` in the CFE).
        NameRef::None_ if cx.wildcard == Some(n.sym) => {
            inf.language_error(cx.unit, n.span, WILDCARD_DOES_NOT_BIND.template.to_string());
            inf.core.dynamic_
        }
        NameRef::None_ => {
            undefined_read_name(inf, cx, n, Lexical::Nothing);
            inf.table.invalid(inf.core.dynamic_)
        }
    }
}

/// The code of the unchecked use through the implicit `this`
/// (`type_property_resolver.dart:107-140`): by the name's parent: a method
/// invocation, a binary operator (a cascade parent counts through its first
/// section), otherwise a property access.
fn nullable_this_code(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
) -> dartforge_diagnostics::Code {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let a = ast(inf, cx);
    let parent = match inf.parent_of(cx.unit, e) {
        dartforge_frontend::parents::Parent::Expr(p) => p,
        _ => return c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
    };
    // Cascade target: the first section counts (`..m()` is an invocation).
    if let ExprKind::Cascade { sections, .. } = &a.expr(parent).kind {
        return match sections.first().map(|&s| &a.expr(s).kind) {
            Some(ExprKind::Call { .. }) => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
            _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
        };
    }
    match &a.expr(parent).kind {
        ExprKind::Binary { .. } => c::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE,
        // A write through the implicit `this` (`the_name += 1`, `the_name ??= 1`,
        // `the_name++`): the method variant, like that of `lookupSetter`
        // (checked against the 3.6.2 oracle).
        ExprKind::Assign { target, .. } if *target == e => {
            c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        }
        ExprKind::Unary {
            op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec,
            operand,
        } if *operand == e => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
        // `the_name()`: the name's `MethodInvocation`.
        ExprKind::Call { target, .. } if *target == e => {
            c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        }
        // `the_name.m()`: the name is the `MethodInvocation`'s target.
        ExprKind::Property { target, .. } if *target == e => match inf.parent_of(cx.unit, parent) {
            dartforge_frontend::parents::Parent::Expr(q) if matches!(&a.expr(q).kind, ExprKind::Call { target, .. } if *target == parent) => {
                c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
            }
            _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
        },
        _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
    }
}

/// The promotion target that `e` denotes, without parentheses: the local
/// variable, the property (`_PropertyReference`: the stable synthetic of a
/// promotable one, or the generation of this read of a non-promotable one)
/// or `this`.
pub(crate) fn promotion_target(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
) -> Option<LocalId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            promotion_target(inf, cx, i)
        }
        ExprKind::Identifier(n) => match cx.lookup(n.sym) {
            Some(MemberName::Local(id)) if !cx.local(id).local_function => Some(id),
            Some(_) => None,
            // Implicit `_x`: `this._x`.
            None => property_target(inf, cx, e, Base::This),
        },
        ExprKind::Property {
            target,
            null_aware: false,
            ..
        } => {
            let t = *target;
            let base = property_base(inf, cx, t)?;
            let before = force_cascade_version(inf, cx, t);
            let r = property_target(inf, cx, e, base);
            cx.forced_version = before;
            r
        }
        ExprKind::This => this_local(inf, cx),
        _ => None,
    }
}

/// The base of a property read from `t` (the `PropertyTarget` with an SSA
/// node): `this`, `super`, a local (not `late`, not a local function) or
/// another (stable) promotable property.
fn property_base(inf: &mut BodyInferrer<'_>, cx: &mut Body, t: ExprId) -> Option<Base> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(t).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            property_base(inf, cx, i)
        }
        ExprKind::This => Some(Base::This),
        ExprKind::Super => Some(Base::Super),
        // A cascade section reads from the cascade target, whose reference is
        // that of the target expression (`cascadeExpression_afterTarget` keeps
        // `_getExpressionReference(target)`): `c?.._field` sees the promotion
        // of `c._field`.
        ExprKind::CascadeTarget => cx.cascade_bases.last().copied().flatten().map(|(b, _)| b),
        // The cascade's value is the target's (the same reference).
        ExprKind::Cascade { target, .. } => {
            let tgt = *target;
            property_base(inf, cx, tgt)
        }
        ExprKind::Identifier(n) => match cx.lookup(n.sym) {
            Some(MemberName::Local(id)) if !cx.local(id).late && !cx.local(id).local_function => {
                Some(Base::Local(id))
            }
            Some(_) => None,
            None => stable_property(inf, cx, t, Base::This).map(Base::Local),
        },
        ExprKind::Property {
            target,
            null_aware: false,
            ..
        } => {
            let t2 = *target;
            let b = property_base(inf, cx, t2)?;
            let before = force_cascade_version(inf, cx, t2);
            let r = stable_property(inf, cx, t, b).map(Base::Local);
            cx.forced_version = before;
            r
        }
        _ => None,
    }
}

/// With `t` the cascade target, forces its base's version to that of the
/// start of the cascade; returns the previously forced one, for restoring.
fn force_cascade_version(inf: &BodyInferrer<'_>, cx: &mut Body, t: ExprId) -> Option<(Base, u32)> {
    let before = cx.forced_version;
    if matches!(ast(inf, cx).expr(t).kind, ExprKind::CascadeTarget)
        && let Some(Some(bv)) = cx.cascade_bases.last().copied()
    {
        cx.forced_version = Some(bv);
    }
    before
}

/// The instance member (getter, field or method, of a class or of an
/// extension) that the read `e` reaches, with the written name: the
/// `propertyMember` of `propertyGet`.
fn read_member(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
) -> Option<(FunctionElementId, SymbolId)> {
    let the_name = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => n.sym,
        ExprKind::Property { name, .. } => name.sym,
        _ => return None,
    };
    let f = match inf.body_types.units[cx.unit.0 as usize].get_resolved(e)? {
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        _ => return None,
    };
    if inf.program.function(f).static_ {
        return None;
    }
    Some((f, the_name))
}

/// The stable synthetic of a promotable property read in `e` (`None` if it
/// is not promotable).
fn stable_property(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    base: Base,
) -> Option<LocalId> {
    let (f, the_name) = read_member(inf, cx, e)?;
    if !inf.promotable_property(cx.lib, f) {
        return None;
    }
    let the_key = (base, cx.base_version(base), the_name);
    if let Some(&id) = cx.fields.get(&the_key) {
        cx.ensure_model(id);
        return Some(id);
    }
    // The declared type as seen by the receiver: that of the read before any
    // promotion (the first read, which did not have the synthetic yet).
    let t = inf.body_types.units[cx.unit.0 as usize]
        .get_type(e)
        .unwrap_or(inf.core.dynamic_);
    let id = cx.declare_synthetic(Local {
        name: the_name,
        ty: t,
        final_: true,
        late: false,
        const_: false,
        offset: 0,
        local_function: false,
    });
    cx.fields.insert(the_key, id);
    Some(id)
}

/// The property read in `e` as a promotion target: a promotable one through
/// the stable synthetic; a non-promotable one gets a new generation (the
/// fresh SSA node of each access), which only the why-not-promoted of the
/// following reads consults.
fn property_target(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    base: Base,
) -> Option<LocalId> {
    if let Some(id) = stable_property(inf, cx, e, base) {
        return Some(id);
    }
    let (_, the_name) = read_member(inf, cx, e)?;
    if let Some(&id) = cx.read_generation.get(&e) {
        cx.ensure_model(id);
        return Some(id);
    }
    let the_key = (base, cx.base_version(base), the_name);
    let t = inf.body_types.units[cx.unit.0 as usize]
        .get_type(e)
        .unwrap_or(inf.core.dynamic_);
    let id = cx.declare_synthetic(Local {
        name: the_name,
        ty: t,
        final_: true,
        late: false,
        const_: false,
        offset: 0,
        local_function: false,
    });
    cx.generations.entry(the_key).or_default().push(id);
    cx.read_generation.insert(e, id);
    Some(id)
}

/// The synthetic of `this` (created the first time).
pub(crate) fn this_local(inf: &mut BodyInferrer<'_>, cx: &mut Body) -> Option<LocalId> {
    let typ = cx.this_type?;
    if let Some(id) = cx.local_this {
        cx.ensure_model(id);
        return Some(id);
    }
    let the_name = inf.sym.this_?;
    let id = cx.declare_synthetic(Local {
        name: the_name,
        ty: typ,
        final_: true,
        late: false,
        const_: false,
        offset: 0,
        local_function: false,
    });
    cx.local_this = Some(id);
    Some(id)
}

/// The read type of property `e` (`tgt.the_name`, already resolved) with
/// declared type `t`, with the property's promotion when the target is a
/// promotable base: the function of `this._f()` (a
/// `FunctionExpressionInvocation` of the `PropertyAccess`).
pub(crate) fn property_read_type(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    t: TypeId,
) -> TypeId {
    let ExprKind::Property {
        target,
        null_aware: false,
        ..
    } = &inf.program.unit(cx.unit).ast.expr(e).kind
    else {
        return t;
    };
    let tgt = *target;
    match property_base(inf, cx, tgt) {
        Some(base) => field_read(inf, cx, e, base, t),
        None => t,
    }
}

/// Read type of a property (`propertyGet` + `_handleProperty`): the
/// promoted one, when the property is promotable and the promoted type is a
/// subtype of the unpromoted one; a read of a non-promotable one records its
/// why-not-promoted.
pub(crate) fn field_read(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    base: Base,
    t: TypeId,
) -> TypeId {
    let Some((f, the_name)) = read_member(inf, cx, e) else {
        return t;
    };
    // A cascade section reads from the target's temporary, with the SSA
    // node from the start of the cascade: a write to the local during the
    // cascade does not remove the promotion from the following sections
    // (`c?.._f.g([c = C()]).._f`).
    let version = match &ast(inf, cx).expr(e).kind {
        ExprKind::Property { target, .. }
            if matches!(ast(inf, cx).expr(*target).kind, ExprKind::CascadeTarget) =>
        {
            match cx.cascade_bases.last().copied().flatten() {
                Some((b, v)) if b == base => v,
                _ => cx.base_version(base),
            }
        }
        _ => cx.base_version(base),
    };
    let the_key = (base, version, the_name);
    if inf.promotable_property(cx.lib, f) {
        return match cx.fields.get(&the_key) {
            Some(&id) => {
                let p = cx.flow.current_type(id, t);
                if p != t && !inf.sub(p, t) { t } else { p }
            }
            None => t,
        };
    }
    inf.record_property_non_promotion(cx, e, the_key, f);
    t
}

/// Coercion of a generic tear-off to a non-generic function context
/// (implicit instantiation).
fn instantiate_in_context(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    t: TypeId,
    ctx: TypeId,
) -> TypeId {
    instantiate_in_context_with(inf, cx, e, t, ctx, true)
}

/// [`instantiate_in_context`]; `record_type` records the tear-off's
/// instantiation (the implicit reference to `call` is not a tear-off of the
/// node).
fn instantiate_in_context_with(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    t: TypeId,
    ctx: TypeId,
    record_type: bool,
) -> TypeId {
    let Type::Function {
        type_params,
        ret,
        positional,
        optional,
        named,
        nullable,
    } = inf.table.get(t).clone()
    else {
        return t;
    };
    if type_params.is_empty() || inf.the_is_unknown(ctx) {
        return t;
    }
    let k = inf.greatest_closure_of(ctx);
    let k = inf.non_null(k);
    let Type::Function {
        type_params: tp_ctx,
        ..
    } = inf.table.get(k).clone()
    else {
        return t;
    };
    if !tp_ctx.is_empty() {
        return t;
    }
    let without = inf.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: positional.clone(),
        optional: optional.clone(),
        named: named.clone(),
        nullable,
    });
    // `inferFunctionTypeInstantiation` (3.6.2 `type_system.dart`): the
    // `GenericInferrer` with the reporter and the expression as entity
    // (`constrainGenericFunctionInContext`, origin "Function type declared
    // as … used where … is required."); `chooseFinalTypes` reports
    // `COULD_NOT_INFER`.
    let mut gi = crate::constraints::GenericInferrer::new(&type_params);
    gi.with_origin(crate::constraints::Origin::Function {
        declared: t,
        context: k,
    });
    gi.generic_metadata = inf
        .program
        .library(cx.lib)
        .features
        .has(dartforge_frontend::Feature::GenericMetadata);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    gi.constrain_return(without, k, &mut env);
    let args = gi.choose_final(&mut env);
    let failures = gi.failures(&args, &mut env, interner, program);
    let r = crate::constraints::instantiate_function(t, &args, &mut env);
    drop(env);
    let sp = inf.span_expr(cx.unit, e);
    for (the_name, suffix) in failures {
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::COULD_NOT_INFER,
            sp,
            &[&the_name, &suffix],
        );
    }
    if record_type {
        inf.body_types.units[cx.unit.0 as usize]
            .tearoff_instantiations
            .insert(e, args.into_boxed_slice());
    }
    r
}

/// Infers a node; returns `(typ, short_circuit)` where `short_circuit` says there is a `?.`
/// in the chain below (the caller that ends the chain makes the type
/// nullable).
pub(crate) fn infer_at(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    ctx: TypeId,
    _chain: bool,
) -> (TypeId, bool) {
    let a = ast(inf, cx);
    let expr = a.expr(e);
    let span = expr.span;
    // `checkUnreachableNode` (`NullSafetyDeadCodeVerifier.visitNode`): the
    // first expression visited in an unreachable flow is the first dead
    // node; the fragment goes to the end of the current basic block (or of
    // the block's last statement).
    if !cx.flow.reachable && cx.dead_code.is_none() {
        let end = cx
            .flow_ends
            .last()
            .copied()
            .or(cx.block_ends.last().copied())
            .unwrap_or(span.end)
            .max(span.end);
        inf.warning(
            DEAD_CODE.template.to_string(),
            dartforge_diagnostics::Span {
                start: span.start,
                end,
            },
        );
        cx.dead_code = Some(cx.flow_ends.len());
    }
    let mut short_circuit = false;
    shorthands::record_chain(inf, cx, e, ctx);
    let t = match &expr.kind {
        ExprKind::DotShorthand { name, .. } => shorthands::value_(inf, cx, e, name.sym, ctx),
        ExprKind::Int(_) => {
            let s = inf.greatest_closure_of(ctx);
            let (int, double) = (inf.core.int, inf.core.double);
            let typ = if !inf.the_is_unknown(ctx) && inf.sub(double, s) && !inf.sub(int, s) {
                double
            } else {
                int
            };
            let the_negated = cx.negated_literal == Some(e);
            if the_negated {
                cx.negated_literal = None;
            }
            super::integers::literal_out_of_range(inf, cx, e, typ, the_negated);
            typ
        }
        ExprKind::Double(_) => inf.core.double,
        ExprKind::Bool(_) => inf.core.bool_,
        ExprKind::Null => inf.core.null,
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(i) = p {
                    let t = infer_free(inf, cx, *i);
                    void_use(inf, cx, *i, t);
                    dead_after_interpolation(inf, cx, *i);
                }
            }
            inf.core.string
        }
        ExprKind::Symbol(_) => inf.core.symbol,
        // `$this` in an interpolation arrives as an identifier.
        ExprKind::Identifier(n) if Some(n.sym) == inf.sym.this_ => {
            cx.this_type.unwrap_or(inf.core.dynamic_)
        }
        ExprKind::Identifier(n) => {
            let t = identifier(inf, cx, e, *n);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        // `ResolverVisitor`'s `_thisType` applies to the whole body of the
        // declaration, also in static members, field initializers and
        // `factory`s (the `this` there is `INVALID_REFERENCE_TO_THIS`, but has
        // its type).
        ExprKind::This => match cx.this_type {
            Some(t) => t,
            None if !cx.untyped_this => analyzer_this_type(inf, cx).unwrap_or(inf.core.dynamic_),
            None => inf.core.dynamic_,
        },
        // `visitSuperExpression` (3.6.2 `static_type_analyzer.dart:240-252`):
        // without `this` or inside an `extension`, the type is invalid.
        ExprKind::Super => match cx.this_type {
            Some(t) if cx.extension.is_none() => t,
            _ => inf.table.invalid(inf.core.dynamic_),
        },
        ExprKind::Parenthesized(i) => {
            let mark = cx.chains.len();
            let (t, c) = infer_at(inf, cx, *i, ctx, false);
            close_chain(inf, cx, mark);
            let t = if c { inf.nullable(t) } else { t };
            record_type(inf, cx, *i, t);
            t
        }
        ExprKind::List { .. } | ExprKind::SetOrMap { .. } => collections::literal(inf, cx, e, ctx),
        ExprKind::Record {
            positional, named, ..
        } => {
            let k = inf.greatest_closure_if_known(ctx);
            let k = inf.non_null(k);
            let (ctx_pos, ctx_named) = match inf.table.get(k).clone() {
                Type::Record {
                    positional: p,
                    named: n,
                    ..
                } if p.len() == positional.len() && n.len() == named.len() => (Some(p), Some(n)),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let positional = positional.to_vec();
            let named: Vec<(ast::Name, ExprId)> = named.to_vec();
            // `_resolveField` (3.6.2 `record_literal_resolver.dart:134-158`): a
            // `void` field reports at the field (a named one, from the name to the
            // value).
            let mut pos = Vec::new();
            for (i, p) in positional.iter().enumerate() {
                let c = ctx_pos.as_ref().map(|v| v[i]).unwrap_or(u);
                let tp = infer(inf, cx, *p, c);
                let tp = dynamic_cast_in_field(inf, tp, c);
                if matches!(inf.table.get(tp), Type::Void) {
                    let sp = inf.span_expr(cx.unit, *p);
                    inf.warning_with_code(
                        dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
                        sp,
                        &[],
                    );
                }
                pos.push(tp);
            }
            let mut nm = Vec::new();
            for (n, x) in named.iter() {
                let c = ctx_named
                    .as_ref()
                    .and_then(|v| v.iter().find(|(s, _)| *s == n.sym).map(|(_, t)| *t))
                    .unwrap_or(u);
                let tx = infer(inf, cx, *x, c);
                let tx = dynamic_cast_in_field(inf, tx, c);
                if matches!(inf.table.get(tx), Type::Void) {
                    let end = inf.span_expr(cx.unit, *x).end;
                    let sp = dartforge_diagnostics::Span {
                        start: n.span.start,
                        end,
                    };
                    inf.warning_with_code(
                        dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
                        sp,
                        &[],
                    );
                }
                nm.push((n.sym, tx));
            }
            inf.table.intern(Type::Record {
                positional: pos.into_boxed_slice(),
                named: nm.into_boxed_slice(),
                nullable: false,
            })
        }
        ExprKind::InstanceCreation { .. } => {
            let t = calls::instantiation(inf, cx, e, ctx);
            null_free_api(inf, cx, e, t);
            t
        }
        // `insertGenericFunctionInstantiation` (3.6.2 `resolver.dart:1171`)
        // after `visitFunctionExpression` (`:2833`), the invocations (`:2855`,
        // `:3244`), the index (`:3035`), the operators (`:2094`, `:3414`,
        // `:3455`), `as` (`:1867`), the assignment (`:1933`) and `await`
        // (`:2081`), besides identifiers and properties.
        ExprKind::FunctionExpression(f) => {
            let t = functions::function_expression(inf, cx, *f, ctx, e);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Property {
            target,
            name,
            null_aware,
        } => {
            let (t, c) = property(inf, cx, e, *target, *name, *null_aware);
            short_circuit = c;
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Index {
            target,
            index,
            null_aware,
        } => {
            let (t, c) = read_index(inf, cx, e, *target, *index, *null_aware, ctx);
            short_circuit = c;
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Call { .. } => {
            if let Some(t) = shorthands::construction(inf, cx, e, ctx) {
                t
            } else {
                let (t, c) = calls::the_call(inf, cx, e, ctx);
                short_circuit = c;
                null_free_api(inf, cx, e, t);
                instantiate_in_context(inf, cx, e, t, ctx)
            }
        }
        ExprKind::TypeArguments { target, type_args } => {
            if type_reference(inf, cx, e).is_some() {
                check_literal_arguments(inf, cx, e, false);
                // The `NamedType.type` of each argument (the constant verifier reads
                // that of `const (List<T>)`); the reports already came out in the check
                // above.
                let before = inf.diagnostics.len();
                for &x in type_args.iter() {
                    inf.type_argument_type(cx, x);
                }
                inf.diagnostics.truncate(before);
                inf.warning_units.truncate(before);
                record_type_ref(inf, cx, e);
                inf.core.type_
            } else if alias_without_class(inf, cx, *target) {
                // `Cb<String>` with `Cb` an alias of a type that is not a class
                // (`typedef Cb<T> = void Function()`): also a `TypeLiteral`.
                check_literal_arguments(inf, cx, e, false);
                for &x in type_args.iter() {
                    inf.type_argument_type(cx, x);
                }
                record_type_ref(inf, cx, e);
                inf.core.type_
            } else {
                // `C.the_name<T>` (constructor tear-off with arguments):
                // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR`, and the instantiation
                // goes on over the tear-off's type.
                if let ExprKind::Property {
                    target: r, name, ..
                } = &ast(inf, cx).expr(*target).kind
                    && let Some(rt) = type_reference(inf, cx, *r)
                    && let TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) = rt
                {
                    let the_key = if Some(name.sym) == inf.sym.new_ {
                        inf.sym.empty
                    } else {
                        Some(name.sym)
                    };
                    if the_key
                        .and_then(|k| inf.constructor_or_primary(c, k))
                        .is_some()
                    {
                        let written = ast(inf, cx).expr(*r).span;
                        super::calls::types_in_constructor(inf, cx.unit, written, *name, type_args);
                        // The `FunctionReference` of a `ConstructorReference` becomes
                        // `InvalidType` (`function_reference_resolver.dart:250-252`).
                        infer_free(inf, cx, *target);
                        for &x in type_args.iter() {
                            inf.type_argument_type(cx, x);
                        }
                        return (inf.table.invalid(inf.core.dynamic_), false);
                    }
                }
                // `p.f<T>` with `p` not resolving in the scope
                // (`_resolvePrefixedIdentifierFunction`, 3.6.2
                // `function_reference_resolver.dart:413-426`): at the prefix, with the
                // whole prefixed name (`unresolved.Class<int>.named()`).
                if let ExprKind::Property {
                    target: r,
                    name,
                    null_aware: false,
                } = &ast(inf, cx).expr(*target).kind
                    && let ExprKind::Identifier(p) = &ast(inf, cx).expr(*r).kind
                    && matches!(
                        resolve_name(inf, cx, p.sym, false),
                        NameRef::None_ | NameRef::ImplicitThis
                    )
                    && !crate::scope::should_ignore_undefined(
                        inf.program,
                        inf.interner,
                        cx.unit,
                        None,
                        p.sym,
                    )
                {
                    let (p, name) = (*p, *name);
                    let text = format!(
                        "{}.{}",
                        inf.interner.resolve(p.sym),
                        inf.interner.resolve(name.sym)
                    );
                    inf.warning(
                        format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, text),
                        p.span,
                    );
                    for &x in type_args.iter() {
                        inf.type_argument_type(cx, x);
                    }
                    return (inf.table.invalid(inf.core.dynamic_), false);
                }
                // `m<T>` with `m` an inherited static (or one of the extended type)
                // found through the implicit `this` (`_resolveSimpleIdentifierFunction`,
                // 3.6.2 `function_reference_resolver.dart:657-694`): the
                // `FunctionReference` reports the unqualified access, which the tear-off
                // without type arguments does not report.
                if let ExprKind::Identifier(n) = &ast(inf, cx).expr(*target).kind
                    && matches!(
                        resolve_name(inf, cx, n.sym, false),
                        NameRef::None_ | NameRef::ImplicitThis
                    )
                    && let Some(this) = analyzer_this_type(inf, cx)
                    && let ThroughThis::Static(owner, true) =
                        lookup_through_this(inf, cx, this, n.sym, false)
                {
                    let n = *n;
                    warn_unqualified_static(inf, cx, owner, n);
                }
                let t = infer_free(inf, cx, *target);
                let targs: Vec<TypeId> = type_args
                    .iter()
                    .map(|&x| inf.type_argument_type(cx, x))
                    .collect();
                // `_resolve`: a type parameter counts through its bound.
                let raw = match inf.table.get(t).clone() {
                    Type::TypeParameter { param, .. } => inf.table.param(param).bound,
                    _ => t,
                };
                match inf.table.get(raw).clone() {
                    Type::Function { type_params, .. } if type_params.len() == targs.len() => {
                        // `ErrorVerifier`'s `checkFunctionReference`.
                        let nodes = type_args.to_vec();
                        let sp = inf.span_expr(cx.unit, e);
                        let visible: Vec<crate::table::TypeParamId> = cx.visible_type_parameters().into_values().collect();
                        calls::check_explicit_bounds(inf, cx.unit, &type_params, &targs, &nodes, sp, Some(&visible));
                        let mut env = inf.env();
                        crate::constraints::instantiate_function(raw, &targs, &mut env)
                    }
                    Type::Function { type_params, .. } => {
                        // `_checkTypeArguments` (`function_reference_resolver.dart:115-150`):
                        // with the name of a declared function or method,
                        // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION`; for a variable, getter or
                        // expression, the anonymous variant. The arguments become `dynamic`.
                        if let Some(sp) = super::calls::type_list_range(inf, cx.unit, type_args) {
                            let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                            match referenced_function_name(inf, cx, *target) {
                                Some(the_name) => inf.warning_with_code(
                                    dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                    sp,
                                    &[&the_name, &n, &d],
                                ),
                                // The function's type goes after: only the 3.13.4 text uses it
                                // (`The type of this function is …`).
                                None => {
                                    let typ = inf.table.format(raw, inf.interner, inf.program);
                                    inf.warning_with_code(
                                        dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION,
                                        sp,
                                        &[&n, &d, &typ],
                                    )
                                }
                            }
                        }
                        let dynamic = vec![inf.core.dynamic_; type_params.len()];
                        let mut env = inf.env();
                        crate::constraints::instantiate_function(raw, &dynamic, &mut env)
                    }
                    _ if inf.table.is_invalid(raw) => raw,
                    // `p.the_name<…>` with `the_name` not resolving in the prefix (the missing
                    // import, `shouldIgnoreUndefined`): the prefixed name's resolver stops
                    // without reaching the reference's `_resolve`, and the type is invalid,
                    // without a report.
                    _ if matches!(&ast(inf, cx).expr(*target).kind, ExprKind::Property { target: p, .. }
                        if matches!(inf.body_types.units[cx.unit.0 as usize].get_resolved(*p), Some(Resolved::Prefix(_))))
                        && inf.body_types.units[cx.unit.0 as usize].get_resolved(*target).is_none() =>
                    {
                        inf.table.invalid(inf.core.dynamic_)
                    }
                    // `node<…>` of an object with a `call` method: `node.call<…>`
                    // (`_resolveAsImplicitCallReference`, 3.6.2
                    // `function_reference_resolver.dart:287-306`): the count is checked
                    // against the type parameters of `call`, with the name `call`, and the
                    // type is the instantiated method's.
                    Type::Interface { .. }
                        if inf.sym.call.is_some_and(|call| matches!(inf.lookup_member(cx.lib, raw, call, false), super::members::Search::Finding(m) if m.method)) =>
                    {
                        let call = inf.sym.call.expect("call");
                        let call_type = match inf.lookup_member(cx.lib, raw, call, false) {
                            super::members::Search::Finding(m) => m.ty,
                            _ => unreachable!(),
                        };
                        match inf.table.get(call_type).clone() {
                            Type::Function { type_params, .. } => {
                                let used = if type_params.len() == targs.len() {
                                    targs.clone()
                                } else {
                                    if let Some(sp) = super::calls::type_list_range(inf, cx.unit, type_args) {
                                        let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                                        inf.warning_with_code(
                                            dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                            sp,
                                            &["call", &n, &d],
                                        );
                                    }
                                    vec![inf.core.dynamic_; type_params.len()]
                                };
                                let mut env = inf.env();
                                crate::constraints::instantiate_function(call_type, &used, &mut env)
                            }
                            _ => inf.core.dynamic_,
                        }
                    }
                    _ => {
                        // `DISALLOWED_TYPE_INSTANTIATION_EXPRESSION`
                        // (`function_reference_resolver.dart:270-278`), with constructor
                        // tear-offs (2.15).
                        let version = inf.program.library(cx.lib).features.version();
                        if version >= dartforge_frontend::features::LanguageVersion::new(2, 15) {
                            let sp = inf.span_expr(cx.unit, *target);
                            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::DISALLOWED_TYPE_INSTANTIATION_EXPRESSION, sp, &[]);
                            inf.table.invalid(inf.core.dynamic_)
                        } else {
                            inf.core.dynamic_
                        }
                    }
                }
            }
        }
        ExprKind::Unary { op, operand } => {
            let t = unary(inf, cx, e, *op, *operand, ctx, &mut short_circuit);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Binary { op, left, right } => {
            let t = binary(inf, cx, e, *op, *left, *right, ctx);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Conditional {
            condition,
            then,
            else_,
        } => {
            let (then, else_) = (*then, *else_);
            let (vf, ff) = checked_condition(inf, cx, *condition);
            let before = cx.flow.clone();
            cx.flow = vf;
            let t1 = flow_operand(inf, cx, then, None, |inf, cx| infer(inf, cx, then, ctx));
            let after1 = std::mem::replace(&mut cx.flow, ff);
            let t2 = flow_operand(inf, cx, else_, None, |inf, cx| infer(inf, cx, else_, ctx));
            let after2 = std::mem::replace(&mut cx.flow, before);
            cx.flow = inf.join(&after1, &after2);
            upper_bound_in_context(inf, t1, t2, ctx)
        }
        ExprKind::Is { value, ty, negated } => {
            let (vf, ff) = type_test(inf, cx, *value, *ty, *negated, span);
            cx.flow = inf.join(&vf, &ff);
            inf.core.bool_
        }
        ExprKind::As { value, ty } => {
            let v = infer_free(inf, cx, *value);
            let t = inf.type_in_context(cx, *ty, crate::resolve::TypeContext::As);
            let equal = v == t
                || crate::ops::without_display(v, inf.table)
                    == crate::ops::without_display(t, inf.table);
            if equal && !inf.is_dynamic(v) && !inf.table.is_invalid(v) && !inf.table.is_invalid(t) {
                inf.warning(UNNECESSARY_CAST.template.to_string(), span);
            }
            // `CAST_FROM_NULL_ALWAYS_FAILS` (`visitAsExpression`,
            // `best_practices_verifier.dart:135-150`).
            let non_nullable = inf.is_non_nullable(t);
            if non_nullable && is_core_null(inf, v) {
                inf.warning_with_code(
                    dartforge_diagnostics::codes::warning::CAST_FROM_NULL_ALWAYS_FAILS,
                    span,
                    &[],
                );
            }
            // `CAST_FROM_NULLABLE_ALWAYS_FAILS` (`ResolverVisitor.visitAsExpression`,
            // `resolver.dart:1871-1887`): the operand is the identifier of a local
            // or type parameter declared nullable, definitely unassigned.
            if non_nullable
                && !is_core_null(inf, v)
                && let ExprKind::Identifier(n) = &ast(inf, cx).expr(*value).kind
                && let Some(Resolved::Local(id)) = inf.body_types.units[cx.unit.0 as usize]
                    .get_resolved(*value)
                    .cloned()
            {
                let n = *n;
                let declared = cx.local(id).ty;
                if inf.the_is_nullable(declared) && cx.flow.unassigned(id) {
                    let the_name = inf.interner.resolve(n.sym).to_string();
                    inf.warning_with_code(
                        dartforge_diagnostics::codes::warning::CAST_FROM_NULLABLE_ALWAYS_FAILS,
                        n.span,
                        &[&the_name],
                    );
                }
            }
            if let Some(id) = promotion_target(inf, cx, *value) {
                let decl = cx.local(id).ty;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                inf.promote(&mut f, id, decl, t);
                cx.flow = f;
            }
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Assign { op, target, value } => {
            let t = assignment(inf, cx, e, *op, *target, *value, &mut short_circuit);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::PatternAssign { pattern, value } => {
            patterns::pattern_assignment(inf, cx, *pattern, *value)
        }
        ExprKind::Cascade {
            target,
            sections,
            null_aware,
        } => {
            let t = infer(inf, cx, *target, ctx);
            void_use(inf, cx, *target, t);
            // `EXTENSION_OVERRIDE_WITH_CASCADE`
            // (`method_invocation_resolver.dart:431-437`,
            // `property_element_resolver.dart:606-612`): at the extension name of
            // the `E(x)` that is the cascade target (one report per cascade).
            if cx.overlays.contains_key(target)
                && let ExprKind::Call { target: f, .. } = &ast(inf, cx).expr(*target).kind
            {
                let the_name = match &ast(inf, cx).expr(*f).kind {
                    ExprKind::Identifier(n) => Some(n.span),
                    ExprKind::Property { name, .. } => Some(name.span),
                    ExprKind::TypeArguments { target: g, .. } => {
                        match &ast(inf, cx).expr(*g).kind {
                            ExprKind::Identifier(n) => Some(n.span),
                            ExprKind::Property { name, .. } => Some(name.span),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(sp) = the_name {
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::EXTENSION_OVERRIDE_WITH_CASCADE, sp, &[]);
                }
            }
            // `c?..x` with `c` non-nullable: reported through the first section
            // (`ErrorVerifier`'s `visitPropertyAccess`/`visitMethodInvocation`/
            // `visitIndexExpression`).
            if *null_aware
                && let Some(&first) = sections.first()
                && first_section_without_rewrite(inf, cx, first, t)
            {
                unnecessary_null_operator(inf, cx, *target, t);
            }
            // `promoteToNonNull`, as with `?.`: a type parameter with a nullable
            // bound becomes `T & NonNull(bnd)` (`nn?..toRadixString(16)`).
            let r = if *null_aware {
                match inf.table.get(t) {
                    Type::TypeParameter {
                        nullable: false, ..
                    } => inf.non_null_promotion(t),
                    _ => inf.non_null(t),
                }
            } else {
                t
            };
            cx.cascades.push(r);
            cx.cascade_targets.push(*target);
            // A target that is not a reference (`getC()?..`): the cascade's
            // temporary is a new SSA node, whose properties promote between the
            // sections.
            // A local captured by a write does not promote, but the temporary does:
            // it gets its own base.
            let captured = |cx: &Body, b: Base| matches!(b, Base::Local(id) if cx.flow.model(id).is_some_and(|m| m.captured));
            let bv = match property_base(inf, cx, *target).filter(|&b| !captured(cx, b)) {
                Some(b) => Some((b, cx.base_version(b))),
                None => inf.sym.empty.map(|the_name| {
                    let id = cx.declare_synthetic(Local {
                        name: the_name,
                        ty: r,
                        final_: true,
                        late: false,
                        const_: false,
                        offset: 0,
                        local_function: false,
                    });
                    (Base::Local(id), cx.base_version(Base::Local(id)))
                }),
            };
            cx.cascade_bases.push(bv);
            // `nullAwareAccess_rightBegin` on the target's temporary
            // (`tryMarkNonNullable`): with a `Null` target, the non-null is `Never`
            // and the sections become unreachable (the first is the dead node); at
            // the end, the join with the null-shortcut path is reachable again.
            let shorthand = if *null_aware && matches!(inf.table.get(r), Type::Never) {
                let s = cx.flow.clone();
                cx.flow = s.unreachable();
                Some(s)
            } else {
                None
            };
            let secs = sections.to_vec();
            for s in secs {
                // The section (the cascade's `MethodInvocation`/`PropertyAccess`/
                // `IndexExpression`) starts at the `..`/`?..` operator: that is where
                // the dead node is.
                if !cx.flow.reachable && cx.dead_code.is_none() {
                    let sp = inf.span_expr(cx.unit, s);
                    let source = inf.program.unit(cx.unit).source.as_str();
                    let before = source[..sp.start].trim_end();
                    let start = if before.ends_with("?..") {
                        before.len() - 3
                    } else if before.ends_with("..") {
                        before.len() - 2
                    } else {
                        sp.start
                    };
                    let end = cx
                        .flow_ends
                        .last()
                        .copied()
                        .or(cx.block_ends.last().copied())
                        .unwrap_or(sp.end)
                        .max(sp.end);
                    inf.warning(
                        DEAD_CODE.template.to_string(),
                        dartforge_diagnostics::Span { start, end },
                    );
                    cx.dead_code = Some(cx.flow_ends.len());
                }
                infer_free(inf, cx, s);
            }
            if let Some(s) = shorthand {
                let after = cx.flow.clone();
                cx.flow = inf.join(&s, &after);
            }
            cx.cascade_bases.pop();
            cx.cascade_targets.pop();
            cx.cascades.pop();
            t
        }
        ExprKind::CascadeTarget => cx.cascades.last().copied().unwrap_or(inf.core.dynamic_),
        ExprKind::Await(i) => {
            let k1 = await_context(inf, ctx);
            let t1 = infer(inf, cx, *i, k1);
            void_use(inf, cx, *i, t1);
            // `AWAIT_OF_INCOMPATIBLE_TYPE` at the `await`
            // (an611:src/generated/error_verifier.dart:2214-2223).
            if incompatible_with_await(inf, t1, 0) {
                let begin = inf.span_expr(cx.unit, e).start;
                let sp = dartforge_diagnostics::Span {
                    start: begin,
                    end: begin + 5,
                };
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::AWAIT_OF_INCOMPATIBLE_TYPE,
                    sp,
                    &[],
                );
            }
            let t = inf.flatten(t1);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Throw(i) => {
            let t = infer_free(inf, cx, *i);
            void_use(inf, cx, *i, t);
            // `THROW_OF_INVALID_TYPE` (an611:src/generated/error_verifier.dart:5336-5347):
            // not assignable to `Object`; `void` reports too.
            let object = inf.core.object;
            if !inf.assignable(t, object) {
                let tt = inf.table.format(t, inf.interner, inf.program);
                let sp = inf.span_expr(cx.unit, *i);
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::THROW_OF_INVALID_TYPE,
                    sp,
                    &[&tt],
                );
            }
            cx.flow.reachable = false;
            inf.core.never
        }
        ExprKind::Rethrow => {
            cx.flow.reachable = false;
            inf.core.never
        }
        ExprKind::Switch { value, cases } => {
            patterns::switch_expression(inf, cx, *value, cases, ctx)
        }
    };
    record_type(inf, cx, e, t);
    if matches!(inf.table.get(t), Type::Never) {
        cx.flow.reachable = false;
    }
    (t, short_circuit)
}

impl<'a> BodyInferrer<'a> {
    /// Greatest closure if the context is not `_` (otherwise `_` itself).
    pub(crate) fn greatest_closure_if_known(&mut self, ctx: TypeId) -> TypeId {
        if self.the_is_unknown(ctx) {
            ctx
        } else {
            self.greatest_closure_of(ctx)
        }
    }
}

/// `K1` of `await e` in context `K` (`inference.md`, "Await expressions").
fn await_context(inf: &mut BodyInferrer<'_>, k: TypeId) -> TypeId {
    if inf.the_is_unknown(k) {
        return inf.future_or(k);
    }
    match inf.table.get(k) {
        Type::FutureOr { .. } => k,
        Type::Dynamic => {
            let u = inf.core.unknown;
            inf.future_or(u)
        }
        _ => inf.future_or(k),
    }
}

/// UP of the branches with the `inference-update-3` rule: if UP does not fit
/// the context but both branches do, the type is the context.
pub(crate) fn upper_bound_in_context(
    inf: &mut BodyInferrer<'_>,
    t1: TypeId,
    t2: TypeId,
    ctx: TypeId,
) -> TypeId {
    let t = inf.up(t1, t2);
    if inf.the_is_unknown(ctx) {
        return t;
    }
    let s = inf.greatest_closure_of(ctx);
    if inf.sub(t, s) {
        t
    } else if inf.sub(t1, s) && inf.sub(t2, s) {
        s
    } else {
        t
    }
}

/// `checkForUseOfVoidResult`: an expression of type `void` used where a
/// value is expected. The range is the method name in a method invocation
/// (`f()` with `f` a function or method, `a.m()`); otherwise, the whole
/// expression. Returns whether it reported.
pub(crate) fn void_use(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Void) {
        return false;
    }
    let a = ast(inf, cx);
    let types = &inf.body_types.units[cx.unit.0 as usize];
    let method = |x: ExprId| -> bool {
        match types.get_resolved(x) {
            Some(Resolved::Element(dartforge_elements::model::Element::Function(_))) => true,
            Some(Resolved::Member {
                member: crate::resolved::MemberRef::Function(f),
                ..
            })
            | Some(Resolved::ExtensionMember { member: f, .. }) => !matches!(
                inf.program.function(*f).kind,
                FunctionKind::Getter | FunctionKind::ImplicitAccessor
            ),
            Some(Resolved::Local(id)) => cx.local_functions.contains(id),
            _ => false,
        }
    };
    let sp = match &a.expr(e).kind {
        ExprKind::Call { target, .. } => match &a.expr(*target).kind {
            ExprKind::Property { name, .. } if method(*target) => name.span,
            ExprKind::Identifier(n) if method(*target) => n.span,
            _ => a.expr(e).span,
        },
        _ => a.expr(e).span,
    };
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
        sp,
        &[],
    );
    true
}

/// `isIncompatibleWithAwait` (an611:src/dart/element/type_system.dart:971-1001):
/// `S?` with `S` incompatible; an extension type that is not a subtype of
/// `Future<Object?>`; a type parameter (promoted or not) through its bound.
fn incompatible_with_await(inf: &mut BodyInferrer<'_>, t: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return false;
    }
    match inf.table.get(t).clone() {
        Type::ExtensionType { nullable, .. } => {
            if nullable {
                let nn = inf.non_null(t);
                return incompatible_with_await(inf, nn, prof + 1);
            }
            let oq = inf.core.object_nullable;
            let fut = inf.future(oq);
            !inf.sub(t, fut)
        }
        Type::Intersection { bound, .. } => incompatible_with_await(inf, bound, prof + 1),
        Type::TypeParameter { param, .. } => {
            let b = inf.table.param(param).bound;
            if b == t {
                return false;
            }
            incompatible_with_await(inf, b, prof + 1)
        }
        _ => false,
    }
}

/// `getErrorNode` of `checkForAssignableExpressionAtType`
/// (an611:src/generated/error_detection_helpers.dart:83-91): goes down
/// through parentheses and the cascade target.
pub(crate) fn without_parentheses(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> ExprId {
    let a = ast(inf, cx);
    let mut x = e;
    loop {
        match &a.expr(x).kind {
            ExprKind::Parenthesized(i) => x = *i,
            ExprKind::Cascade { target, .. } => x = *target,
            _ => return x,
        }
    }
}

/// `checkForAssignableExpressionAtType`: with a non-`void` target, a `void`
/// value is `use_of_void_result`; otherwise, assignability.
pub(crate) fn verify_assignable_expr(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    of: TypeId,
    for_: TypeId,
    template: &str,
) {
    verify_assignable_expr_at(inf, cx, e, of, for_, template, true);
}

/// Like [`verify_assignable_expr`]; `unwrap` applies `getErrorNode`
/// (false on the right side of an assignment, which the analyzer reports as
/// written: `assignment_expression_resolver.dart:148-154`).
pub(crate) fn verify_assignable_expr_at(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    of: TypeId,
    for_: TypeId,
    template: &str,
    unwrap: bool,
) {
    if !matches!(inf.table.get(for_), Type::Void) && void_use(inf, cx, e, of) {
        return;
    }
    if inf.assignable(of, for_) {
        return;
    }
    // A record with one positional field written `(x)` without a comma
    // (an611:src/generated/error_detection_helpers.dart:93-108): at the
    // parenthesis.
    if let Type::Record {
        positional, named, ..
    } = inf.table.get(for_).clone()
        && positional.len() == 1
        && named.is_empty()
        && !matches!(inf.table.get(of), Type::Record { .. })
        && matches!(ast(inf, cx).expr(e).kind, ExprKind::Parenthesized(_))
        && inf.assignable(positional[0], of)
    {
        let sp = inf.span_expr(cx.unit, e);
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA, sp, &[]);
        return;
    }
    let no = if unwrap {
        without_parentheses(inf, cx, e)
    } else {
        e
    };
    let sp = inf.span_expr(cx.unit, no);
    // `_insertImplicitCallReference` (`resolver.dart:4085-4142`): the
    // expression became the implicit reference to `call`, whose type is the
    // method's as instantiated by the context; that is what the message
    // shows.
    let the_implicit = {
        let table = &inf.body_types.units[cx.unit.0 as usize].implicit_calls;
        table.contains(&e) || table.contains(&no)
    };
    let of = match the_implicit
        .then(|| inf.implicit_call_type(of, for_))
        .flatten()
    {
        Some(f) => inf.instantiate_function_from_context(f, for_),
        None => of,
    };
    // `checkForAssignableExpressionAtType`: the error carries the
    // expression's why-not-promoted
    // (`computeWhyNotPromotedMessages(expression, …)`).
    let since = inf.diagnostics.len();
    let entity = inf.span_expr(cx.unit, e);
    if template == ARGUMENT_TYPE_NOT_ASSIGNABLE.template {
        // `{2}`: only between records (`error_detection_helpers.dart:109-139`).
        let mut info: Vec<String> = Vec::new();
        if let (
            Type::Record {
                positional: pa,
                named: at,
                ..
            },
            Type::Record {
                positional: pe,
                named: ne,
                ..
            },
        ) = (inf.table.get(of).clone(), inf.table.get(for_).clone())
        {
            if !pe.is_empty() && pa.len() != pe.len() {
                info.push(format!(
                    "Expected {} positional arguments, but got {} instead.",
                    pe.len(),
                    pa.len()
                ));
            }
            if !ne.is_empty() && at.len() != ne.len() {
                info.push(format!(
                    "Expected {} named arguments, but got {} instead.",
                    ne.len(),
                    at.len()
                ));
            }
            if !ne.is_empty() {
                let mut named_args: Vec<(String, TypeId)> = at
                    .iter()
                    .map(|(n, t)| (inf.interner.resolve(*n).to_string(), *t))
                    .collect();
                named_args.sort_by(|a, b| a.0.cmp(&b.0));
                for (the_name, t) in named_args {
                    if !ne
                        .iter()
                        .any(|(n2, t2)| inf.interner.resolve(*n2) == the_name && *t2 == t)
                    {
                        let tt = inf.table.format_without_alias(t, inf.interner, inf.program);
                        info.push(format!(
                            "Unexpected named argument `{the_name}` with type `{tt}`."
                        ));
                    }
                }
            }
        }
        let a3 = info.join(" ");
        inf.warning_with_args(
            dartforge_diagnostics::codes::compile_time_error::ARGUMENT_TYPE_NOT_ASSIGNABLE,
            sp,
            &[
                crate::display::Arg::Type_(of),
                crate::display::Arg::Type_(for_),
                crate::display::Arg::Text(a3.into()),
            ],
        );
        inf.attach_non_promotion(since, cx, Some(e), entity);
        return;
    }
    inf.verify_assignable(of, for_, sp, template);
    inf.attach_non_promotion(since, cx, Some(e), entity);
}

/// An operand that closes a basic block (the `flowEnd` of
/// `NullSafetyDeadCodeVerifier`, `an611:src/error/dead_code_verifier.dart:213-330`):
/// the branches of `?:` and the right side of `&&`/`||`. If it starts
/// unreachable and no dead fragment is open, it is the first dead node and the
/// fragment goes from it (or from the operator, `start`, when the parent is a
/// binary) to its end; the nodes inside are part of the same fragment.
fn flow_operand<R>(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    start: Option<usize>,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body) -> R,
) -> R {
    let sp = inf.span_expr(cx.unit, e);
    let open = !cx.flow.reachable && cx.dead_code.is_none();
    // `f()` with `f` a local of a function type: the analyzer replaces the
    // node (`MethodInvocation` → `FunctionExpressionInvocation`), and this
    // operand's `flowEnd` no longer finds it (`_containsFirstDeadNode`): the
    // fragment only closes at the end of the outer basic block.
    // `p.y` with `p` a local of a record type: `PrefixedIdentifierResolver`
    // replaces the node with a `PropertyAccess`, with the same effect.
    let rewritten = open
        && (local_call(inf, cx, e) || local_record_property(inf, cx, e))
        && !cx.flow_ends.is_empty();
    if open {
        // With a binary parent, the `flowEnd` that closes the fragment replaces
        // the node with the (new) `rightOperand`: the end is the operand's
        // either way.
        let end = if rewritten && start.is_none() {
            cx.flow_ends.last().copied().unwrap_or(sp.end).max(sp.end)
        } else {
            sp.end
        };
        let span = dartforge_diagnostics::Span {
            start: start.unwrap_or(sp.start),
            end,
        };
        inf.warning(DEAD_CODE.template.to_string(), span);
        if rewritten {
            cx.dead_code = Some(cx.flow_ends.len());
        }
    }
    super::statements::enter_flow(cx, sp.end);
    if open && !rewritten {
        cx.dead_code = Some(cx.flow_ends.len());
    }
    let r = f(inf, cx);
    super::statements::exit_flow(cx);
    r
}

/// The first section of a cascade `c?..s` is an access that the analyzer
/// does not rewrite: property, index, or **method** invocation. `..g()` with
/// `g` a getter (or field) becomes an expression invocation, and the `?..`
/// comes out without a report.
fn first_section_without_rewrite(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    section: ExprId,
    t: TypeId,
) -> bool {
    let a = ast(inf, cx);
    // Goes down to the access applied to the cascade target, remembering
    // the parent.
    let mut parent: Option<ExprId> = None;
    let mut x = section;
    loop {
        let tgt = match &a.expr(x).kind {
            ExprKind::Call { target, .. }
            | ExprKind::Property { target, .. }
            | ExprKind::Index { target, .. } => *target,
            ExprKind::Assign { target, .. } => *target,
            _ => return true,
        };
        if matches!(a.expr(tgt).kind, ExprKind::CascadeTarget) {
            break;
        }
        parent = Some(x);
        x = tgt;
    }
    let ExprKind::Property { name, .. } = &a.expr(x).kind else {
        return true;
    };
    let invoked = parent
        .is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Call { target, .. } if *target == x));
    if !invoked {
        return true;
    }
    let the_name = name.sym;
    let nn = inf.non_null(t);
    !matches!(inf.lookup_member(cx.lib, nn, the_name, false), Search::Finding(m) if !m.method)
}

/// `e` is `p.y` (a `PrefixedIdentifier`) with `p` a local whose type, through
/// its bound, is a record (`prefixed_identifier_resolver.dart:31-46`).
fn local_record_property(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let ExprKind::Property {
        target,
        null_aware: false,
        ..
    } = &a.expr(e).kind
    else {
        return false;
    };
    let ExprKind::Identifier(n) = &a.expr(*target).kind else {
        return false;
    };
    let Some(super::body::MemberName::Local(id)) = cx.lookup(n.sym) else {
        return false;
    };
    let t = cx.flow.current_type(id, cx.local(id).ty);
    let t = inf.resolve_to_bound(t);
    matches!(inf.table.get(t), Type::Record { .. })
}

/// `e` is `f(...)` with `f` a local variable (not a local function).
fn local_call(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let ExprKind::Call { target, .. } = &a.expr(e).kind else {
        return false;
    };
    let ExprKind::Identifier(n) = &a.expr(*target).kind else {
        return false;
    };
    match cx.lookup(n.sym) {
        Some(super::body::MemberName::Local(id)) => !cx.local_functions.contains(&id),
        _ => false,
    }
}

/// `RECEIVER_OF_TYPE_NEVER` (`binary_expression_resolver.dart:422-428`,
/// `method_invocation_resolver.dart:543`, `property_element_resolver.dart:92`,
/// `function_expression_invocation_resolver.dart:66`): the receiver of an
/// operator, index, method or invocation has type `Never` (not `Never?`).
/// Reports at the receiver and says whether it reported.
pub(crate) fn never_receiver(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Never)
        || matches!(ast(inf, cx).expr(r).kind, ExprKind::Super)
    {
        return false;
    }
    // In a cascade section, the receiver is the cascade target
    // (`realTarget`), with its static type: `?..` does not promote here
    // (`null?..m()` has receiver `Null`, not `Never`; the non-null is left
    // to `TypePropertyResolver`).
    let r = if matches!(ast(inf, cx).expr(r).kind, ExprKind::CascadeTarget) {
        let tgt = cx.cascade_targets.last().copied().unwrap_or(r);
        let raw = inf.body_types.units[cx.unit.0 as usize].get_type(tgt);
        if !raw.is_some_and(|b| matches!(inf.table.get(b), Type::Never)) {
            return false;
        }
        tgt
    } else {
        r
    };
    let sp = inf.span_expr(cx.unit, r);
    inf.warning_with_code(
        dartforge_diagnostics::codes::warning::RECEIVER_OF_TYPE_NEVER,
        sp,
        &[],
    );
    true
}

/// `RecordLiteralResolver._resolveField` (3.6.2 `record_literal_resolver.dart:135-148`):
/// a `dynamic` field with a known context takes the context's greatest closure,
/// when `dynamic` is not a subtype of it (the implicit cast).
fn dynamic_cast_in_field(inf: &mut BodyInferrer<'_>, t: TypeId, context: TypeId) -> TypeId {
    if context == inf.core.unknown || !matches!(inf.table.get(t), Type::Dynamic) {
        return t;
    }
    let closure = inf.greatest_closure_of(context);
    if inf.sub(t, closure) { t } else { closure }
}

/// `DEAD_NULL_AWARE_EXPRESSION` (`_checkForDeadNullCoalesce`,
/// `an611:src/generated/error_verifier.dart:3045-3052`): in `a ?? b` and
/// `a ??= b`, with `a` strictly non-nullable, the right side.
fn warn_dead_null(inf: &mut BodyInferrer<'_>, cx: &Body, the_left: TypeId, the_right: ExprId) {
    if strictly_non_nullable(inf, the_left) {
        let sp = inf.span_expr(cx.unit, the_right);
        inf.warning_with_code(
            dartforge_diagnostics::codes::static_warning::DEAD_NULL_AWARE_EXPRESSION,
            sp,
            &[],
        );
    }
}

/// `t` is strictly non-nullable (`isStrictlyNonNullable`): not `dynamic`,
/// `void`, `Null` or nullable; a type parameter through its bound; an
/// extension type only with `implements` (here, never: on the safe side).
pub(crate) fn strictly_non_nullable(inf: &mut BodyInferrer<'_>, t: TypeId) -> bool {
    // `TypeSystemImpl.isStrictlyNonNullable` (3.6.2 `type_system.dart:1290-1310`).
    let mut t = t;
    for _ in 0..64 {
        if inf.table.is_invalid(t) || inf.the_is_unknown(t) {
            return false;
        }
        let ty = inf.table.get(t).clone();
        if matches!(ty, Type::Dynamic | Type::Void | Type::Null) || ty.is_declared_nullable() {
            return false;
        }
        match ty {
            Type::FutureOr { arg, .. } => t = arg,
            // An extension type only with `implements` (`type.interfaces`).
            Type::ExtensionType { decl, .. } => {
                return !inf.outline.classes[decl.0 as usize].interfaces.is_empty();
            }
            // `TypeParameterType.bound`: the promoted one, otherwise the declared one.
            Type::TypeParameter { param, .. } => {
                let b = inf.table.param(param).bound;
                if b == t {
                    return true;
                }
                t = b;
            }
            Type::Intersection { bound, .. } => t = bound,
            _ => return true,
        }
    }
    true
}

/// `_checkForUnnecessaryNullAware` for `?.` and `?[`: a strictly
/// non-nullable receiver. The range is the operator (`?.`, or the whole
/// `?[`); with an earlier `?.`/`?[` in the same chain, it is the
/// `…_AFTER_SHORT_CIRCUIT`.
fn unnecessary_null_operator(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, t: TypeId) {
    if matches!(ast(inf, cx).expr(r).kind, ExprKind::Super) || !strictly_non_nullable(inf, t) {
        return;
    }
    report_null_operator(inf, cx, r, true);
}

/// `C?.x`, `E?.m()`, `Alias?.x`: the receiver is a type literal (without a
/// static type in the analyzer, a `targetElement` of a class, extension or
/// alias, `error_verifier.dart:5613-5627`): the operator is unnecessary.
pub(crate) fn null_operator_on_type(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId) {
    let simple = match &ast(inf, cx).expr(r).kind {
        ExprKind::Identifier(_) => true,
        ExprKind::Property { target, .. } => {
            matches!(ast(inf, cx).expr(*target).kind, ExprKind::Identifier(_))
        }
        _ => false,
    };
    if simple {
        report_null_operator(inf, cx, r, false);
    }
}

/// The `INVALID_NULL_AWARE_OPERATOR` at the operator that follows `r`
/// (`?.`, `?..` or `?[`); `chain`: with an earlier `?.` in the same chain,
/// it is the `…_AFTER_SHORT_CIRCUIT`.
fn report_null_operator(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, chain: bool) {
    use dartforge_diagnostics::codes::static_warning as w;
    let end = inf.span_expr(cx.unit, r).end;
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(end..).unwrap_or("");
    let pos = end + skip_whitespace_and_comments(rest);
    let after = source.get(pos..).unwrap_or("");
    let (size, args): (usize, [&str; 2]) = if after.starts_with("?..") {
        (3, ["?..", ".."])
    } else if after.starts_with("?.") {
        (2, ["?.", "."])
    } else if after.starts_with('?') {
        // `?[`: from the `?` to the `[` (with whatever is between them).
        let Some(i) = after.find('[') else { return };
        (i + 1, ["?[", "["])
    } else {
        return;
    };
    let sp = dartforge_diagnostics::Span {
        start: pos,
        end: pos + size,
    };
    // `invalidNullAwareAfterShortCircuit`: the context is the operator that
    // short-circuits.
    match (chain && args[0] != "?..")
        .then(|| previous_short_circuit_operator(inf, cx, r))
        .flatten()
    {
        Some((previous, lexeme)) => inf.warning_with_context(
            w::INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT,
            sp,
            &args,
            vec![(
                None,
                previous,
                format!("The operator '{lexeme}' is causing the short circuiting."),
            )],
        ),
        None => inf.warning_with_code(w::INVALID_NULL_AWARE_OPERATOR, sp, &args),
    }
}

/// `...?e` with `e` strictly non-nullable: at the `...?`.
pub(crate) fn unnecessary_null_spread(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    the_value: ExprId,
    t: TypeId,
) {
    use dartforge_diagnostics::codes::static_warning as w;
    if !strictly_non_nullable(inf, t) {
        return;
    }
    let begin = inf.span_expr(cx.unit, the_value).start;
    let source = &inf.program.unit(cx.unit).source;
    let before = source.get(..begin).unwrap_or("").trim_end();
    if let Some(pos) = before.strip_suffix("...?").map(str::len) {
        let sp = dartforge_diagnostics::Span {
            start: pos,
            end: pos + 4,
        };
        inf.warning_with_code(w::INVALID_NULL_AWARE_OPERATOR, sp, &["...?", "..."]);
    }
}

/// `previousShortCircuitingOperator` (`error_verifier.dart:5583-5605`): the
/// `?.`/`?` (of `?[`) operator of `r` that short-circuits the chain, the
/// deepest first; only through accesses that are themselves `?.`/`?[` (a `.`
/// in between ends the search). The token's range and lexeme.
fn previous_short_circuit_operator(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    r: ExprId,
) -> Option<(dartforge_diagnostics::Span, &'static str)> {
    let source = &inf.program.unit(cx.unit).source;
    let token_after = |tgt: ExprId,
                       lexeme: &'static str|
     -> Option<(dartforge_diagnostics::Span, &'static str)> {
        let end = inf.span_expr(cx.unit, tgt).end;
        let pos = end + skip_whitespace_and_comments(source.get(end..).unwrap_or(""));
        source.get(pos..)?.starts_with(lexeme).then_some((
            dartforge_diagnostics::Span {
                start: pos,
                end: pos + lexeme.len(),
            },
            lexeme,
        ))
    };
    match &ast(inf, cx).expr(r).kind {
        ExprKind::Property {
            target,
            null_aware: true,
            ..
        } => {
            previous_short_circuit_operator(inf, cx, *target).or_else(|| token_after(*target, "?."))
        }
        ExprKind::Index {
            target,
            null_aware: true,
            ..
        } => {
            previous_short_circuit_operator(inf, cx, *target).or_else(|| token_after(*target, "?"))
        }
        ExprKind::Call { target, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Property {
                target: t2,
                null_aware: true,
                ..
            } => previous_short_circuit_operator(inf, cx, *t2).or_else(|| token_after(*t2, "?.")),
            _ => None,
        },
        _ => None,
    }
}

/// Infers the receiver of an access (`r.x`, `r[i]`, `r.m()`), handling `?.`
/// and *null-shorting*. Returns the receiver's type for the lookup and
/// whether there is a short circuit.
pub(crate) fn the_receiver(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    r: ExprId,
    null_aware: bool,
) -> (TypeId, bool) {
    let u = inf.core.unknown;
    let (t, c) = infer_at(inf, cx, r, u, true);
    if null_aware {
        unnecessary_null_operator(inf, cx, r, t);
        // Promotes the receiver inside the chain; `close_chain` undoes it at
        // the end.
        cx.chains.push(cx.flow.clone());
        if let Some(id) = promotion_target(inf, cx, r) {
            let decl = cx.local(id).ty;
            let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
            inf.promote_non_null(&mut f, id, decl);
            cx.flow = f;
        }
        // `promoteToNonNull` of the target (a type parameter with a nullable
        // bound becomes `T & NonNull(bnd)`: `nn?.toRadixString(16)` with
        // `NN extends int?`).
        let nn = match inf.table.get(t) {
            Type::TypeParameter {
                nullable: false, ..
            } => inf.non_null_promotion(t),
            _ => inf.non_null(t),
        };
        (nn, true)
    } else {
        (t, c)
    }
}

/// Member lookup on the receiver `target`: an explicit extension override
/// (`E(x).m`) consults only the extension; otherwise the normal lookup.
pub(crate) fn lookup_target_member(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    target: ExprId,
    recv: TypeId,
    the_name: SymbolId,
    setter: bool,
) -> Search {
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        return match inf.explicit_extension_member(x, &args, the_name, setter) {
            Some(m) => Search::Finding(m),
            None => Search::Missing,
        };
    }
    inf.lookup_member(cx.lib, recv, the_name, setter)
}

/// `target.name` (read).
/// `startNullAwarePropertyAccess`/`visitMethodInvocation` (3.6.2,
/// `resolver.dart:1777-1794`, `:3215-3226`): `?.` on a type literal only
/// stops short-circuiting when the target is a `SimpleIdentifier` that names
/// an `InterfaceElement` (`C?.x` is `C.x`); `p.C?.x`, `C<int>?.x`, an alias or
/// an extension short-circuit (the result becomes nullable).
pub(crate) fn shortening_waived(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    target: ExprId,
    rt: &TypeRef,
) -> bool {
    matches!(ast(inf, cx).expr(target).kind, ExprKind::Identifier(_))
        && matches!(rt, TypeRef::Class(..))
}

/// `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT` at the prefix's name.
pub(crate) fn prefix_without_dot(inf: &mut BodyInferrer<'_>, n: ast::Name) {
    let the_name = inf.interner.resolve(n.sym).to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT,
        n.span,
        &[&the_name],
    );
}

fn property(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    target: ExprId,
    name: ast::Name,
    null_aware: bool,
) -> (TypeId, bool) {
    let a = ast(inf, cx);
    // `p.the_name` with an import prefix.
    if let ExprKind::Identifier(p) = &a.expr(target).kind
        && matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix)
    {
        // `p?.x` is a `PropertyAccess`, not a `PrefixedIdentifier`: the prefix
        // does not count there (`_isValidAsPrefix`).
        if null_aware {
            prefix_without_dot(inf, *p);
        }
        resolver(inf, cx, target, Resolved::Prefix(cx.lib));
        if ambiguous_import(inf, cx, Some(p.sym), name) {
            return (inf.table.invalid(inf.core.dynamic_), false);
        }
        match inf
            .program
            .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
            .and_then(|b| b.getter.or(b.setter))
        {
            Some(el) => {
                resolver(inf, cx, e, Resolved::Element(el));
                if let Element::Extension(_) = el {
                    let sp = inf.span_expr(cx.unit, e);
                    if extension_as_expression(inf, cx, e, sp) {
                        return (inf.core.dynamic_, false);
                    }
                }
                return (read_element(inf, el), false);
            }
            None => {
                if let Some(t) = load_library(inf, cx, p.sym, name.sym) {
                    return (t, false);
                }
                warn_undefined_prefixed_name(inf, cx, p.sym, name);
                return (inf.core.dynamic_, false);
            }
        }
    }
    // `Fn<int>.foo` with `Fn` a function type alias (3.6.2
    // `property_element_resolver.dart:459-477`): no resolution is possible,
    // `UNDEFINED_GETTER_ON_FUNCTION_TYPE` at the name.
    if function_type_literal(inf, cx, target, name, true) {
        return (inf.table.invalid(inf.core.dynamic_), false);
    }
    // `C.x`: static, enum constant or constructor tear-off.
    if let Some(rt) = type_reference(inf, cx, target) {
        record_type_ref(inf, cx, target);
        if null_aware {
            null_operator_on_type(inf, cx, target);
        }
        let short_circuit = null_aware && !shortening_waived(inf, cx, target, &rt);
        return (static_access(inf, cx, e, rt, name), short_circuit);
    }
    // `super.x`. With `super?.x` (`INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER`
    // in the parser), the access is still null-aware: the type comes out
    // nullable at the end of the chain (`int y = super?.x` is an
    // `INVALID_ASSIGNMENT` of `int?`).
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.this_type.unwrap_or(inf.core.dynamic_);
        record_type(inf, cx, target, this);
        return (super_member(inf, cx, e, name, SuperUse::Read), null_aware);
    }
    let (recv, short_circuit) = the_receiver(inf, cx, target, null_aware);
    // `x.new` with `x` a value: `TypePropertyResolver` never finds `new`
    // (`type_property_resolver.dart:75-79`), neither on `dynamic` nor on a
    // nullable type.
    if !cx.overlays.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
        let typ = inf.table.format(recv, inf.interner, inf.program);
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::UNDEFINED_GETTER,
            name.span,
            &["new", &typ],
        );
        return (inf.table.invalid(inf.core.dynamic_), short_circuit);
    }
    if let Some((x, args)) = cx.overlays.get(&target).cloned()
        && inf
            .explicit_extension_member(x, &args, name.sym, false)
            .is_none()
        && inf.extension_static_member(x, name.sym, false).is_some()
    {
        inf.warning(
            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER
                .template
                .to_string(),
            name.span,
        );
        return (inf.core.dynamic_, short_circuit);
    }
    let base = if null_aware {
        None
    } else {
        property_base(inf, cx, target)
    };
    // `void`: the value cannot be used (analyzer: `use_of_void_result`, at the name).
    // Not for an `E(x)` target: the member comes from the extension, and the
    // argument's `void` was already reported at the argument
    // (`extension_member_resolver.dart:227-232`).
    if !cx.overlays.contains_key(&target) && matches!(inf.table.get(recv), Type::Void) {
        // In `x..p`, the report is at the cascade target (made there).
        if !matches!(a.expr(target).kind, ExprKind::CascadeTarget) {
            inf.warning_with_code(
                dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
                name.span,
                &[],
            );
        }
        return (inf.core.dynamic_, short_circuit);
    }
    // A potentially nullable receiver without the member on `Object` nor in
    // an extension: `unchecked_use_of_nullable_value` at the name, and the
    // read goes on through the non-nullable type only to recover the type
    // (without `undefined_getter`).
    let check_null = !cx.overlays.contains_key(&target)
        && inf.requires_null_check(cx.lib, recv, name.sym, false);
    if check_null {
        let the_name = inf.interner.resolve(name.sym).to_string();
        let since = inf.diagnostics.len();
        inf.null_warning(
            recv,
            dartforge_diagnostics::codes::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&the_name],
        );
        // `computeWhyNotPromotedMessages(nameErrorEntity, whyNotPromoted(receiver))`.
        inf.attach_non_promotion(since, cx, Some(target), name.span);
    }
    // `f.call` read with `f` of `dart:core`'s `Function` type: the
    // `functionTypeCallType` is the type itself (`property_element_resolver.dart:
    // 440-445`); only the invocation `f.call()` comes out dynamic.
    if !check_null
        && Some(name.sym) == inf.sym.call
        && matches!(inf.table.get(recv), Type::Interface { class, nullable: false, .. } if Some(*class) == inf.core.function_class)
    {
        return (recv, short_circuit);
    }
    let search = lookup_target_member(inf, cx, target, recv, name.sym, false);
    inf.report_extension_ambiguity(name.span);
    let t = match search {
        Search::Finding(m) => {
            resolver(inf, cx, e, m.resolved.clone());
            match base {
                Some(b) => field_read(inf, cx, e, b, m.ty),
                None => m.ty,
            }
        }
        Search::Missing if check_null => {
            match inf.instance_access_to_static(cx.lib, recv, name.sym, false, name.span) {
                Some(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    m.ty
                }
                None => inf.core.dynamic_,
            }
        }
        // An `InvalidType` receiver: no member is looked up, and the result is
        // the invalid one itself (`property_element_resolver`, T4.1 item 2).
        Search::Dynamic if inf.table.is_invalid(recv) => {
            resolver(inf, cx, e, Resolved::Dynamic);
            recv
        }
        Search::Dynamic => {
            resolver(inf, cx, e, Resolved::Dynamic);
            // `Object`'s `hashCode`, `runtimeType`, `toString`… also apply on `dynamic`.
            let o = inf.core.object;
            match inf.interface_member(o, name.sym, false) {
                Some(m) => m.ty,
                None => inf.core.dynamic_,
            }
        }
        // A `Never` receiver (analyzer 3.6.2): in the form `id.x`
        // (`PrefixedIdentifier`) the result is `Never`; in the others
        // (`PropertyAccess`, `failure().x`) only `Object`'s members apply, and a
        // name that is not `Object`'s gets the recovery type (`InvalidType`,
        // which behaves like `dynamic`).
        Search::NeverType => {
            let prefixed = !null_aware && matches!(a.expr(target).kind, ExprKind::Identifier(_));
            let o = inf.core.object;
            match inf.interface_member(o, name.sym, false) {
                _ if prefixed => inf.core.never,
                Some(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    m.ty
                }
                None => {
                    inf.body_types.units[cx.unit.0 as usize]
                        .invalid_types
                        .insert(e);
                    inf.table.invalid(inf.core.dynamic_)
                }
            }
        }
        Search::Missing => {
            if let Some((x, _)) = cx.overlays.get(&target).cloned() {
                let extension = inf
                    .program
                    .extension(x)
                    .name
                    .map(|n| inf.interner.resolve(n))
                    .unwrap_or("");
                let msg = format!(
                    "{}: '{}' in '{}'",
                    UNDEFINED_EXTENSION_GETTER.template,
                    inf.interner.resolve(name.sym),
                    extension
                );
                inf.warning(msg, name.span);
                return (inf.core.dynamic_, short_circuit);
            }
            if let Some(m) = inf.instance_access_to_static(cx.lib, recv, name.sym, false, name.span)
            {
                resolver(inf, cx, e, m.resolved.clone());
                return (m.ty, short_circuit);
            }
            let msg = format!(
                "{}: getter '{}' not defined for the type '{}'",
                UNDEFINED_GETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.warning(msg, name.span);
            // A read without an element on a target that is not dynamic: the
            // recovery type (`generated/resolver.dart:1668`).
            inf.table.invalid(inf.core.dynamic_)
        }
    };
    (t, short_circuit)
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` of a class or alias instantiated as an
/// expression (`e` is the `TypeArguments`). As a type literal
/// (`_resolveDirectTypeLiteral`/`_resolveTypeAlias`,
/// `function_reference_resolver.dart:115-150, 316-327, 821-835`), at the
/// argument list; with `in_named_type` (the `NamedType` of `C<T>.x` and of
/// the implicit creation, `NamedTypeResolver._buildTypeArguments`), from the
/// name to the `>`. The name is the written one (the alias's, if it is one).
pub(crate) fn check_literal_arguments(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    in_named_type: bool,
) {
    let a = ast(inf, cx);
    let ExprKind::TypeArguments { target, type_args } = &a.expr(e).kind else {
        return;
    };
    let (target, n_args) = (*target, type_args.len());
    let last_end = type_args.last().map(|&t| a.ty(t).span.end);
    let (el, the_name) = match &a.expr(target).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Element(el) => (el, *n),
            _ => return,
        },
        ExprKind::Property {
            target: p,
            name,
            null_aware: false,
        } => {
            let ExprKind::Identifier(p) = &a.expr(*p).kind else {
                return;
            };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix) {
                return;
            }
            let Some(el) = inf
                .program
                .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
                .and_then(|b| b.getter)
            else {
                return;
            };
            (el, *name)
        }
        _ => return,
    };
    let parameters = match el {
        Element::Class(c) => inf.outline.classes[c.0 as usize].type_params.len(),
        Element::Typedef(td) => inf.outline.typedefs[td.0 as usize].type_params.len(),
        _ => return,
    };
    if parameters == n_args {
        return;
    }
    let source = &inf.program.unit(cx.unit).source;
    let tgt = a.expr(target).span;
    let after_name = tgt.end;
    let open = source
        .get(after_name..)
        .and_then(|r| r.find('<'))
        .map(|i| after_name + i)
        .unwrap_or(after_name);
    let args_end = last_end.unwrap_or(open + 1);
    let closes = source
        .get(args_end..)
        .and_then(|r| r.find('>'))
        .map(|i| args_end + i + 1)
        .unwrap_or(args_end);
    let span = if in_named_type {
        dartforge_diagnostics::Span {
            start: tgt.start,
            end: closes,
        }
    } else {
        dartforge_diagnostics::Span {
            start: open,
            end: closes,
        }
    };
    let (text, p, n) = (
        inf.interner.resolve(the_name.sym).to_string(),
        parameters.to_string(),
        n_args.to_string(),
    );
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS,
        span,
        &[&text, &p, &n],
    );
}

/// Static access `C.x` / `E.x` / `C.new`.
fn static_access(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    rt: TypeRef,
    name: ast::Name,
) -> TypeId {
    let explicit_instance = explicit_instantiation_receiver(inf, cx, e);
    if explicit_instance && let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind {
        check_literal_arguments(inf, cx, *target, true);
    }
    match rt {
        TypeRef::Extension(x) => match inf.extension_static_member(x, name.sym, false) {
            Some(m) => {
                resolver(inf, cx, e, m.resolved.clone());
                m.ty
            }
            None => {
                if inf
                    .program
                    .extension(x)
                    .instance_members
                    .contains_key(&name.sym)
                {
                    let msg = format!(
                        "{}: '{}'",
                        STATIC_ACCESS_TO_INSTANCE_MEMBER.template,
                        inf.interner.resolve(name.sym)
                    );
                    inf.warning(msg, name.span);
                    return inf.core.dynamic_;
                }
                let extension = inf
                    .program
                    .extension(x)
                    .name
                    .map(|n| inf.interner.resolve(n))
                    .unwrap_or("");
                let msg = format!(
                    "{}: '{}' in '{}'",
                    UNDEFINED_EXTENSION_GETTER.template,
                    inf.interner.resolve(name.sym),
                    extension
                );
                inf.warning(msg, name.span);
                inf.core.dynamic_
            }
        },
        TypeRef::Class(c, targs) => {
            // A private name from another library is not found (`Enum2._A` outside
            // the enum's library: `undefined_enum_constant`).
            let the_visible = !inf.interner.resolve(name.sym).starts_with('_')
                || inf.program.class(c).library == cx.lib;
            if let Some(m) = inf
                .static_member(c, name.sym, false)
                .filter(|_| the_visible)
            {
                if explicit_instance {
                    warn_static_instantiation(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.ty;
            }
            let args = targs.map(|v| {
                v.iter()
                    .map(|&t| inf.type_argument_type(cx, t))
                    .collect::<Vec<_>>()
            });
            constructor_tearoff(inf, cx, e, c, args, name, true, false)
        }
        TypeRef::Alias(c, args, td) => {
            let the_visible = !inf.interner.resolve(name.sym).starts_with('_')
                || inf.program.class(c).library == cx.lib;
            if let Some(m) = inf
                .static_member(c, name.sym, false)
                .filter(|_| the_visible)
            {
                if explicit_instance {
                    warn_static_instantiation(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.ty;
            }
            if !explicit_instance
                && args.is_some()
                && !inf.outline.typedefs[td.0 as usize].type_params.is_empty()
            {
                return generic_alias_tearoff(inf, cx, e, c, td, name);
            }
            // Explicitly instantiated `X<T>.m`: the same
            // `CLASS_INSTANTIATION_ACCESS_TO_MEMBER` as the class.
            constructor_tearoff(inf, cx, e, c, args, name, explicit_instance, true)
        }
    }
}

fn explicit_instantiation_receiver(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind else {
        return false;
    };
    matches!(
        &ast(inf, cx).expr(*target).kind,
        ExprKind::TypeArguments { .. }
    )
}

/// Only members declared in the class itself; the inherited lookup requires
/// resolving substitutions and precedence before diagnosing.
fn visible_direct_instance_member(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    cls: ClassId,
    name: ast::Name,
    write: bool,
) -> bool {
    if inf.interner.resolve(name.sym).starts_with('_') && inf.program.class(cls).library != cx.lib {
        return false;
    }
    let members = &inf.program.class(cls).instance_members;
    members.contains_key(&name.sym)
        || (write
            && inf
                .setter_key(name.sym)
                .is_some_and(|the_key| members.contains_key(&the_key)))
}

fn warn_static_access_to_instance(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    cls: ClassId,
    name: ast::Name,
    write: bool,
) -> bool {
    let found = visible_direct_instance_member(inf, cx, cls, name, write);
    if found {
        let msg = format!(
            "{}: '{}'",
            STATIC_ACCESS_TO_INSTANCE_MEMBER.template,
            inf.interner.resolve(name.sym)
        );
        inf.warning(msg, name.span);
    }
    found
}

fn warn_class_instantiation(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    expr: ExprId,
    cls: ClassId,
    name: ast::Name,
    write: bool,
) -> bool {
    if !visible_direct_instance_member(inf, cx, cls, name, write) {
        return false;
    }
    let msg = format!(
        "{}: '{}'",
        CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template,
        inf.interner.resolve(name.sym)
    );
    let span = ast(inf, cx).expr(expr).span;
    inf.warning(msg, span);
    true
}

fn warn_static_instantiation(inf: &mut BodyInferrer<'_>, cx: &Body, expr: ExprId, name: ast::Name) {
    let msg = format!(
        "{}: '{}'",
        CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER.template,
        inf.interner.resolve(name.sym)
    );
    let span = ast(inf, cx).expr(expr).span;
    inf.warning(msg, span);
}

fn warn_unknown_instantiation(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    expr: ExprId,
    cls: ClassId,
    name: ast::Name,
) {
    // The synthetic recovery identifier (`A<int>.;`) is not resolved.
    if name.span.start == name.span.end {
        return;
    }
    let cls = inf.interner.resolve(inf.program.class(cls).name);
    let mem = inf.interner.resolve(name.sym);
    let msg = format!(
        "{}: '{cls}', '{mem}'",
        CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER.template
    );
    let span = ast(inf, cx).expr(expr).span;
    inf.warning(msg, span);
}

/// `C.the_name` / `C.new` as a value: the constructor's function type (generic
/// over the class's parameters if not instantiated).
/// `of_alias`: the receiver is a type alias; instantiated (`X<T>.new`),
/// `AstRewriter` does not rewrite it into a constructor reference, and the
/// missing `new` is an unknown member of the instantiation like any other
/// name.
#[allow(clippy::too_many_arguments)]
fn constructor_tearoff(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    c: ClassId,
    args: Option<Vec<TypeId>>,
    name: ast::Name,
    diagnose_absence: bool,
    of_alias: bool,
) -> TypeId {
    let explicit_instance = explicit_instantiation_receiver(inf, cx, e);
    let the_key = if Some(name.sym) == inf.sym.new_ {
        inf.sym.empty
    } else {
        Some(name.sym)
    };
    let Some(the_key) = the_key else {
        return inf.core.dynamic_;
    };
    // The primary of an extension type (`E.new`, `E.the_name`) is not an element:
    // `Some(None)`, with the signature `(Representação) -> E`.
    let primary = inf.constructor_or_primary(c, the_key) == Some(None);
    let constructor = inf.constructor_of(c, the_key);
    // Generative enum constructors only create the enum's own constants.
    // The implicit unnamed one does not appear in the table, but `E.new` is
    // also a reference to it. Factories declared in the enum can be
    // referenced.
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum
        && (constructor.is_some_and(|f| !inf.program.function(f).factory)
            || (constructor.is_none() && Some(name.sym) == inf.sym.new_))
    {
        inf.warning(
            INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR
                .template
                .to_string(),
            ast(inf, cx).expr(e).span,
        );
        return inf.core.dynamic_;
    }
    // `ConstructorReferenceResolver.resolve`
    // (`an611:src/dart/resolver/constructor_reference_resolver.dart:31-41`):
    // a generative one (also the synthetic unnamed one, which the model does
    // not create) of an abstract class, on the whole `ConstructorReference`.
    let abstract_ = {
        let ce = inf.program.class(c);
        matches!(
            ce.kind,
            dartforge_elements::model::ClassKind::Class
                | dartforge_elements::model::ClassKind::MixinApplication
        ) && ce.modifiers.abstract_
    };
    let synthetic = constructor.is_none()
        && !primary
        && Some(the_key) == inf.sym.empty
        && super::functions::without_implicit_name(inf, c);
    if abstract_ && (synthetic || constructor.is_some_and(|f| !inf.program.function(f).factory)) {
        let sp = ast(inf, cx).expr(e).span;
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS, sp, &[]);
    }
    if synthetic {
        let ret = inf.class_this_type(c);
        let sig = inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        });
        return tearoff_with_arguments(inf, c, sig, args);
    }
    let Some(f) = constructor else {
        if primary {
            let sig = inf.primary_signature(c);
            return tearoff_with_arguments(inf, c, sig, args);
        }
        // `C<T>.new` is still a reference to the unnamed constructor. Its
        // absence has a diagnostic at the `new` token, even when the receiver
        // was instantiated explicitly.
        if diagnose_absence && Some(name.sym) == inf.sym.new_ && !(of_alias && explicit_instance) {
            let cls = inf.interner.resolve(inf.program.class(c).name);
            let msg = if explicit_instance {
                format!(
                    "{}: '{cls}', 'new'",
                    NEW_WITH_UNDEFINED_CONSTRUCTOR.template
                )
            } else {
                format!(
                    "{}: '{cls}'",
                    NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT.template
                )
            };
            inf.warning(msg, name.span);
            return inf.core.dynamic_;
        }
        if explicit_instance && warn_class_instantiation(inf, cx, e, c, name, false) {
            return inf.core.dynamic_;
        }
        if explicit_instance && diagnose_absence {
            warn_unknown_instantiation(inf, cx, e, c, name);
            return inf.core.dynamic_;
        }
        if !explicit_instance && warn_static_access_to_instance(inf, cx, c, name, false) {
            return inf.core.dynamic_;
        }
        // `PropertyElementResolver` (`property_element_resolver.dart:696-705`):
        // in an enum the code is `UNDEFINED_ENUM_CONSTANT`.
        if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum {
            let (n, en) = (
                inf.interner.resolve(name.sym).to_string(),
                inf.interner.resolve(inf.program.class(c).name).to_string(),
            );
            inf.warning_with_code(
                dartforge_diagnostics::codes::compile_time_error::UNDEFINED_ENUM_CONSTANT,
                name.span,
                &[&n, &en],
            );
            return inf.core.dynamic_;
        }
        // `[propertyName.name, typeReference.name]`
        // (`property_element_resolver.dart:697-705`): the class name, also
        // through the alias.
        let (n, cls) = (
            inf.interner.resolve(name.sym).to_string(),
            inf.interner.resolve(inf.program.class(c).name).to_string(),
        );
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::UNDEFINED_GETTER,
            name.span,
            &[&n, &cls],
        );
        return inf.core.dynamic_;
    };
    if inf.program.function(f).class == Some(c) {
        resolver(inf, cx, e, Resolved::Constructor(f));
    }
    let sig = inf.constructor_signature(c, f);
    tearoff_with_arguments(inf, c, sig, args)
}

/// `F.new`/`F.the_name` through a generic alias that does more than forward the
/// parameters (`typedef F<T> = C<int, T>`), without type arguments: the
/// generic function over the alias's parameters, `C<int, T> Function<T>(…)`
/// (like the analyzer and the CFE), which the context's implicit
/// instantiation closes later.
fn generic_alias_tearoff(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    c: ClassId,
    td: dartforge_elements::model::TypedefId,
    name: ast::Name,
) -> TypeId {
    let tgt = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let target_args = match inf.table.get(tgt).clone() {
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args,
        _ => return inf.core.dynamic_,
    };
    let new_: Vec<crate::table::TypeParamId> = params
        .iter()
        .map(|&p| {
            let d = inf.table.param(p).clone();
            inf.table.alloc_type_param(
                d.name,
                crate::table::TypeParamOwner::GenericFunctionType,
                d.bound,
                d.variance,
            )
        })
        .collect();
    let types: Vec<TypeId> = new_
        .iter()
        .map(|&p| {
            inf.table.intern(Type::TypeParameter {
                param: p,
                nullable: false,
            })
        })
        .collect();
    let map = inf.map(&params, &types);
    for (&p, &o) in new_.iter().zip(params.iter()) {
        let b = inf.table.param(p).bound;
        let b = inf.subst(b, &map);
        inf.table.set_type_param_bound(p, b);
        inf.table.param_mut(p).explicit = inf.table.param(o).explicit;
    }
    let args: Vec<TypeId> = target_args.iter().map(|a| inf.subst(*a, &map)).collect();
    let t = constructor_tearoff(inf, cx, e, c, Some(args), name, false, true);
    match inf.table.get(t).clone() {
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } if type_params.is_empty() => inf.table.intern(Type::Function {
            type_params: new_.into_boxed_slice(),
            ret,
            positional,
            optional,
            named,
            nullable,
        }),
        _ => t,
    }
}

/// The type of the tear-off of a constructor of `c` with signature `sig`:
/// instantiated with `args`, or generic over new parameters in place of the
/// class's.
fn tearoff_with_arguments(
    inf: &mut BodyInferrer<'_>,
    c: ClassId,
    sig: TypeId,
    args: Option<Vec<TypeId>>,
) -> TypeId {
    let params = inf.outline.classes[c.0 as usize].type_params.clone();
    match args {
        Some(args) if args.len() == params.len() => {
            let map = inf.map(&params, &args);
            inf.subst(sig, &map)
        }
        _ if params.is_empty() => sig,
        _ => {
            // Generic: `C<T> Function<T>(...)` with the parameters of **the class
            // itself** as formals (`ConstructorElementToInfer.asType`,
            // `invocation_inference_helper.dart:45-54`: `typeElement.typeParameters`,
            // without a copy). In a context that mentions the same parameters
            // (`[A<T> Function() fn = A.new]` inside `A<T>`) the tear-off's type and
            // the context's are equal, `trySubtypeMatch` closes without a constraint
            // (`P == Q`) and `T` goes to `dynamic`: `A<dynamic> Function()` is not
            // assignable (oracle `ConstWithTypeParametersConstructorTearo_9751514d`).
            match inf.table.get(sig).clone() {
                Type::Function {
                    ret,
                    positional,
                    optional,
                    named,
                    nullable,
                    ..
                } => inf.table.intern(Type::Function {
                    type_params: params.clone(),
                    ret,
                    positional,
                    optional,
                    named,
                    nullable,
                }),
                _ => sig,
            }
        }
    }
}

/// How `super.the_name` is used: the code for a name that does not exist
/// changes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuperUse {
    Read,
    Write,
    Invocation,
}

/// The classes of the current class's `super` chain, in lookup order: its
/// mixins (the last first), the superclass, its mixins… without the
/// interfaces (the analyzer's `getInheritedConcreteMap`). In a mixin, the
/// `on` constraints and their supertypes.
fn super_chain(inf: &BodyInferrer<'_>, c: ClassId) -> Vec<ClassId> {
    let mut v = Vec::new();
    let cls = inf.program.class(c);
    if cls.kind == dartforge_elements::model::ClassKind::Mixin {
        for &o in &cls.on_classes {
            for (sup, _) in crate::scope::ordered_supertypes(inf.program, &inf.outline.hierarchy, o)
            {
                if !v.contains(&sup) {
                    v.push(sup);
                }
            }
            if !v.contains(&o) {
                v.insert(0, o);
            }
        }
        // Without `on`, the constraint is `Object`.
        if let Some(o) = inf.core.object_class
            && !v.contains(&o)
        {
            v.push(o);
        }
        return v;
    }
    for &m in cls.mixin_classes.iter().rev() {
        v.push(m);
    }
    let mut current = cls.supertype_class;
    while let Some(k) = current {
        if v.contains(&k) {
            break;
        }
        v.push(k);
        for &m in inf.program.class(k).mixin_classes.iter().rev() {
            if !v.contains(&m) {
                v.push(m);
            }
        }
        current = inf.program.class(k).supertype_class;
    }
    // Every chain ends at `Object` (the model may not link an enum's
    // superclass).
    if let Some(o) = inf.core.object_class
        && !v.contains(&o)
        && c != o
    {
        v.push(o);
    }
    v
}

/// The member `the_key` that `super` reaches: the current class's
/// `getMember2(forSuper: true)` (the `superImplemented.last` of
/// `InheritanceManager3`). `Err(Some(..))`: only the one inherited through
/// the interface (`getInherited2`), which the analyzer uses for recovery with
/// `ABSTRACT_SUPER_MEMBER_REFERENCE`; `Err(None)`: none.
#[allow(clippy::type_complexity)]
pub(crate) fn member_reached_through_super(
    inf: &mut BodyInferrer<'_>,
    c: ClassId,
    the_key: SymbolId,
) -> Result<
    (ClassId, dartforge_elements::model::FunctionElementId),
    Option<(ClassId, dartforge_elements::model::FunctionElementId)>,
> {
    let owner = |inf: &BodyInferrer<'_>, m: &crate::inheritance::InterfaceMember| {
        inf.program.function(m.function).class.unwrap_or(m.class)
    };
    if let Some(m) = inf.inheritance_member(c, the_key, false, true) {
        return Ok((owner(inf, &m), m.function));
    }
    match inf.inherited_from_inheritance(c, the_key) {
        Some(m) => Err(Some((owner(inf, &m), m.function))),
        None => Err(None),
    }
}

/// `super.the_name` (read, write or invocation): the concrete member of the super
/// chain; only abstract → `ABSTRACT_SUPER_MEMBER_REFERENCE` (and its type);
/// none → `UNDEFINED_SUPER_{GETTER,SETTER,METHOD}`
/// (`an611:src/dart/resolver/property_element_resolver.dart:790-880`,
/// `method_invocation_resolver.dart:744-788`). `Object`'s members count
/// (every chain ends at it).
pub(crate) fn super_member(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    name: ast::Name,
    use_: SuperUse,
) -> TypeId {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let setter = use_ == SuperUse::Write;
    let Some(cls) = cx.class else {
        return inf.core.dynamic_;
    };
    let Some(this) = cx.this_type else {
        return inf.core.dynamic_;
    };
    // `super` in an extension type is not valid (`SuperContext`): another
    // error, without resolution.
    if inf.program.class(cls).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return inf.core.dynamic_;
    }
    // Without the setter's key in the interner, no declaration has that
    // setter: the lookup finds nothing and goes on to the report.
    let the_key = if setter {
        inf.setter_key(name.sym)
    } else {
        Some(name.sym)
    };
    let finding = match the_key
        .map(|k| member_reached_through_super(inf, cls, k))
        .unwrap_or(Err(None))
    {
        Ok(x) => Some(x),
        Err(Some((sup, f))) => {
            let fe = &inf.program.functions[f.0 as usize];
            let typ = match fe.kind {
                dartforge_elements::model::FunctionKind::Getter => "getter",
                dartforge_elements::model::FunctionKind::Setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor if setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor => "getter",
                _ => "method",
            };
            let the_name = inf.interner.resolve(name.sym).to_string();
            inf.warning_with_code(
                c::ABSTRACT_SUPER_MEMBER_REFERENCE,
                name.span,
                &[typ, &the_name],
            );
            Some((sup, f))
        }
        Err(None) => None,
    };
    if let Some((sup, f)) = finding {
        let (t, _) = inf.declared_member_type(f, setter);
        let t = inf.substitute_from_owner(this, cls, sup, t);
        resolver(
            inf,
            cx,
            e,
            Resolved::Member {
                class: sup,
                member: MemberRef::Function(f),
                via_super: true,
            },
        );
        // `super._x`: the property of the `_superSsaNode`.
        if use_ == SuperUse::Read {
            return field_read(inf, cx, e, Base::Super, t);
        }
        return t;
    }
    // `late final x;` without an initializer has an implicit setter (the
    // element model does not create it): `super.x = v` reaches it.
    if setter {
        for k in super_chain(inf, cls) {
            let field = inf.program.class(k).fields.iter().copied().find(|&v| {
                let ve = inf.program.variable(v);
                ve.name == name.sym && ve.late && ve.final_ && !ve.static_
            });
            if let Some(v) = field {
                return inf.variable_type(v);
            }
        }
    }
    // `Object` (a class's chain always ends at it).
    let o = inf.core.object;
    if let Some(m) = inf.interface_member(o, name.sym, setter) {
        return m.ty;
    }
    let the_name = inf.interner.resolve(name.sym).to_string();
    match use_ {
        SuperUse::Invocation => {
            let owner = inf
                .interner
                .resolve(inf.program.class(cls).name)
                .to_string();
            inf.warning_with_code(c::UNDEFINED_SUPER_METHOD, name.span, &[&the_name, &owner]);
        }
        SuperUse::Read | SuperUse::Write => {
            let typ = inf.table.format(this, inf.interner, inf.program);
            let code = if setter {
                c::UNDEFINED_SUPER_SETTER
            } else {
                c::UNDEFINED_SUPER_GETTER
            };
            inf.warning_with_code(code, name.span, &[&the_name, &typ]);
        }
    }
    inf.core.dynamic_
}

/// The operator `op` as seen through `super` in the current class: the first
/// one declared in the supertypes, in the order of [`super_member`],
/// instantiated as the current class sees it; otherwise `Object`'s.
pub(crate) fn lookup_super_operator(inf: &mut BodyInferrer<'_>, cx: &Body, op: SymbolId) -> Search {
    let (Some(c), Some(this)) = (cx.class, cx.this_type) else {
        return Search::Dynamic;
    };
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return Search::Dynamic;
    }
    // Only the concrete one (`TypePropertyResolver`'s
    // `getMember(forSuper: true)`).
    if let Ok((sup, f)) = member_reached_through_super(inf, c, op) {
        let (t, _) = inf.declared_member_type(f, false);
        let typ = inf.substitute_from_owner(this, c, sup, t);
        return Search::Finding(InterfaceMember {
            resolved: Resolved::Member {
                class: sup,
                member: MemberRef::Function(f),
                via_super: true,
            },
            ty: typ,
            method: true,
            function: Some(f),
            of_extension: false,
        });
    }
    let o = inf.core.object;
    match inf.interface_member(o, op, false) {
        Some(m) => Search::Finding(m),
        None => Search::Missing,
    }
}

/// Symbol of the binary operator.
fn operator_text(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::Eq | BinaryOp::NotEq => "==",
        _ => return None,
    })
}

fn operator_symbol(inf: &BodyInferrer<'_>, op: BinaryOp) -> Option<SymbolId> {
    inf.interner.lookup(operator_text(op)?)
}

fn skip_whitespace_and_comments(snippet: &str) -> usize {
    let bytes = snippet.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
        } else if bytes.get(pos..pos + 2) == Some(b"/*") {
            pos += 2;
            while pos + 1 < bytes.len() && &bytes[pos..pos + 2] != b"*/" {
                pos += 1;
            }
            pos = (pos + 2).min(bytes.len());
        } else if bytes.get(pos..pos + 2) == Some(b"//") {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
        } else {
            break;
        }
    }
    pos
}

fn index_span(
    inf: &BodyInferrer<'_>,
    cx: &Body,
    tgt: ExprId,
    target: ExprId,
) -> dartforge_diagnostics::Span {
    let todo = inf.span_expr(cx.unit, tgt);
    let start = inf.span_expr(cx.unit, target).end;
    let between = inf
        .program
        .unit(cx.unit)
        .source
        .get(start..todo.end)
        .unwrap_or("");
    let pos = skip_whitespace_and_comments(between);
    let bracket = start + pos + between.get(pos..).unwrap_or("").find('[').unwrap_or(0);
    dartforge_diagnostics::Span {
        start: bracket,
        end: todo.end,
    }
}

fn warn_extension_operator(
    inf: &mut BodyInferrer<'_>,
    x: ExtensionId,
    the_name: &str,
    span: dartforge_diagnostics::Span,
) {
    let extension = inf
        .program
        .extension(x)
        .name
        .map(|n| inf.interner.resolve(n))
        .unwrap_or("");
    let msg = format!(
        "{}: '{}' in '{}'",
        UNDEFINED_EXTENSION_OPERATOR.template, the_name, extension
    );
    inf.warning(msg, span);
}

fn read_index(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    tgt: ExprId,
    target: ExprId,
    index: ExprId,
    null_aware: bool,
    ctx: TypeId,
) -> (TypeId, bool) {
    let (recv, short_circuit) = the_receiver(inf, cx, target, null_aware);
    // `resolveIndexExpression` (3.6.2 `property_element_resolver.dart:79-97`):
    // the target through its bound (`resolveToBound`) and the `Never` before
    // the `?[`.
    let raw = inf.body_types.units[cx.unit.0 as usize]
        .get_type(target)
        .unwrap_or(recv);
    let bnd = inf.resolve_to_bound(raw);
    if !cx.overlays.contains_key(&target) && never_receiver(inf, cx, target, bnd) {
        infer_free(inf, cx, index);
        return (inf.core.never, short_circuit);
    }
    let op = inf.sym.index;
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        if let Some((s, m)) = op.and_then(|s| {
            inf.explicit_extension_member(x, &args, s, false)
                .map(|m| (s, m))
        }) {
            return (
                binary_operator_with_member(inf, cx, recv, s, (index, ctx), Some(tgt), m).0,
                short_circuit,
            );
        }
        infer_free(inf, cx, index);
        let span = index_span(inf, cx, tgt, target);
        warn_extension_operator(inf, x, "[]", span);
        return (inf.core.dynamic_, short_circuit);
    }
    let undefined = index_span(inf, cx, tgt, target);
    // A `void` target (through the bound): `USE_OF_VOID_RESULT` at the `[…]`
    // (`property_element_resolver.dart:81-90`, `_reportUnresolvedIndex`).
    let bnd = {
        let mut b = recv;
        for _ in 0..16 {
            match inf.table.get(b) {
                Type::TypeParameter {
                    param,
                    nullable: false,
                } => {
                    let l = inf.table.param(*param).bound;
                    if l == b {
                        break;
                    }
                    b = l;
                }
                Type::Intersection { bound, .. } => b = *bound,
                _ => break,
            }
        }
        b
    };
    if matches!(inf.table.get(bnd), Type::Void) {
        infer_free(inf, cx, index);
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
            undefined,
            &[],
        );
        return (inf.core.dynamic_, short_circuit);
    }
    let token = dartforge_diagnostics::Span {
        start: undefined.start,
        end: undefined.start + 1,
    };
    let super_ = matches!(ast(inf, cx).expr(target).kind, ExprKind::Super);
    let positions = OperatorPositions {
        token,
        undefined,
        compound: false,
        super_,
        numeric_target: None,
    };
    (
        binary_operator(inf, cx, recv, op, (index, ctx), positions, Some(tgt)).0,
        short_circuit,
    )
}

/// Where the analyzer reports an operator's errors: `token` for the use
/// without a null check (the operator; in `a[i]`, the `[`), `undefined` for
/// `undefined_operator` (the operator; in `a[i]`, the `[i]`).
#[derive(Clone, Copy)]
pub(crate) struct OperatorPositions {
    pub token: dartforge_diagnostics::Span,
    pub undefined: dartforge_diagnostics::Span,
    /// Compound assignment (`x += 1`): the analyzer speaks of "method '+'", and
    /// the missing operator is not reported here when the target is a method
    /// (`assignment_to_method`) or `void` (`use_of_void_result`).
    pub compound: bool,
    /// The receiver is `super` (`undefined_super_operator`, another rule).
    pub super_: bool,
    /// The target of the numeric refinement of the operand's context, when it
    /// is not the receiver: in a compound assignment, the write type
    /// (`_computeRhsContext`, assignment_expression_resolver.dart:184-201).
    pub numeric_target: Option<TypeId>,
}

/// Dart operators, from the longest to the shortest.
const OPERATORS: &[&str] = &[
    ">>>=", "~/=", ">>=", "<<=", "??=", ">>>", "~/", ">>", "<<", "<=", ">=", "==", "!=", "&&",
    "||", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "??", "+", "-", "*", "/",
    "%", "<", ">", "&", "|", "^", "~",
];

/// The operator token right after `after_` (skipping whitespace and
/// comments).
fn operator_token(inf: &BodyInferrer<'_>, cx: &Body, after_: usize) -> dartforge_diagnostics::Span {
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(after_..).unwrap_or("");
    let pos = after_ + skip_whitespace_and_comments(rest);
    let text = source.get(pos..).unwrap_or("");
    let n = OPERATORS
        .iter()
        .find(|o| text.starts_with(**o))
        .map_or(0, |o| o.len());
    dartforge_diagnostics::Span {
        start: pos,
        end: pos + n,
    }
}

/// Invocation of a one-argument operator (`a + b`, `a[i]`): looks up the
/// member on the receiver, infers the argument with the parameter's context
/// (with the numeric refinement) and returns `(typ, mem)`.
pub(crate) fn binary_operator(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    recv: TypeId,
    op: Option<SymbolId>,
    (arg, ctx): (ExprId, TypeId),
    positions: OperatorPositions,
    no: Option<ExprId>,
) -> (TypeId, Option<InterfaceMember>) {
    let Some(op) = op else {
        infer_free(inf, cx, arg);
        return (inf.core.dynamic_, None);
    };
    // A potentially nullable receiver: the analyzer reports the operator's
    // invocation (`[]`/`[]=` as a method invocation) without a null check.
    let check_null = inf.requires_null_check(cx.lib, recv, op, false);
    if check_null {
        let text = inf.interner.resolve(op).to_string();
        let code = if text == "[]" || text == "[]=" || positions.compound {
            dartforge_diagnostics::codes::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        } else {
            dartforge_diagnostics::codes::compile_time_error::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE
        };
        let since = inf.diagnostics.len();
        inf.null_warning(recv, code, positions.token, &[&text]);
        // The operator's receiver (`leftOperand`, the index's target, the left
        // side of the compound one).
        let receiver = no.and_then(|n| match &ast(inf, cx).expr(n).kind {
            ExprKind::Binary { left, .. } => Some(*left),
            ExprKind::Index { target, .. } => Some(*target),
            ExprKind::Assign { target, .. } => Some(*target),
            _ => None,
        });
        if let Some(r) = receiver {
            inf.attach_non_promotion(since, cx, Some(r), positions.token);
        }
    }
    // `super[i]`, `super + x`: the operator is the superclass's (like the
    // `super.m` of [`super_member`]), not the one the class itself overrides.
    let search = if positions.super_ {
        lookup_super_operator(inf, cx, op)
    } else {
        inf.lookup_member(cx.lib, recv, op, false)
    };
    // The operator's `AMBIGUOUS_EXTENSION_MEMBER_ACCESS`: on the whole
    // binary; for an index, at the target (`extension_member_resolver.dart:115-128`).
    {
        let sp = match no.map(|n| &ast(inf, cx).expr(n).kind) {
            Some(ExprKind::Index { target, .. }) => inf.span_expr(cx.unit, *target),
            // The compound one (`a += 0`): at the operator.
            Some(ExprKind::Assign { .. }) => positions.token,
            Some(_) => inf.span_expr(cx.unit, no.unwrap()),
            None => positions.token,
        };
        inf.report_extension_ambiguity(sp);
    }
    match search {
        Search::Finding(m) => binary_operator_with_target_member(
            inf,
            cx,
            recv,
            positions.numeric_target.unwrap_or(recv),
            op,
            arg,
            ctx,
            no,
            m,
        ),
        Search::Missing if check_null => {
            infer_free(inf, cx, arg);
            (inf.core.dynamic_, None)
        }
        Search::Dynamic => {
            infer_free(inf, cx, arg);
            // An operator on `InvalidType`: the result is invalid
            // (`binary_expression_resolver.dart:332-335`).
            (
                if inf.table.is_invalid(recv) {
                    recv
                } else {
                    inf.core.dynamic_
                },
                None,
            )
        }
        Search::NeverType => {
            infer_free(inf, cx, arg);
            (inf.core.never, None)
        }
        Search::Missing => {
            infer_free(inf, cx, arg);
            if positions.super_ {
                // `UNDEFINED_SUPER_OPERATOR` with the type of `super` (that of `this`).
                let text = inf.interner.resolve(op).to_string();
                let typ = inf.table.format(recv, inf.interner, inf.program);
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::UNDEFINED_SUPER_OPERATOR,
                    positions.undefined,
                    &[&text, &typ],
                );
                return (inf.core.dynamic_, None);
            }
            if matches!(inf.table.get(recv), Type::Void | Type::Function { .. }) {
                return (inf.core.dynamic_, None);
            }
            let text = inf.interner.resolve(op).to_string();
            let typ = inf.table.format(recv, inf.interner, inf.program);
            inf.warning_with_code(
                dartforge_diagnostics::codes::compile_time_error::UNDEFINED_OPERATOR,
                positions.undefined,
                &[&text, &typ],
            );
            // An operator the type does not have: the result is `InvalidType`
            // (`binary_expression_resolver.dart:308`).
            (inf.table.invalid(inf.core.dynamic_), None)
        }
    }
}

fn binary_operator_with_member(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    recv: TypeId,
    op: SymbolId,
    (arg, ctx): (ExprId, TypeId),
    no: Option<ExprId>,
    m: InterfaceMember,
) -> (TypeId, Option<InterfaceMember>) {
    binary_operator_with_target_member(inf, cx, recv, recv, op, arg, ctx, no, m)
}

/// Like [`binary_operator_with_member`], with the target of the numeric
/// refinement of the operand's context separate from the receiver.
#[allow(clippy::too_many_arguments)]
fn binary_operator_with_target_member(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    recv: TypeId,
    numeric_target: TypeId,
    op: SymbolId,
    arg: ExprId,
    ctx: TypeId,
    no: Option<ExprId>,
    m: InterfaceMember,
) -> (TypeId, Option<InterfaceMember>) {
    if let Some(n) = no {
        resolver(inf, cx, n, m.resolved.clone());
    }
    let (param, ret) = match inf.table.get(m.ty).clone() {
        Type::Function {
            positional,
            optional,
            ret,
            ..
        } => (positional.first().or(optional.first()).copied(), ret),
        _ => (None, inf.core.dynamic_),
    };
    let ctx_arg = match param {
        Some(p) => numeric_context(inf, numeric_target, &m, op, ctx, p),
        None => inf.core.unknown,
    };
    let ta = infer(inf, cx, arg, ctx_arg);
    if let Some(p) = param {
        verify_assignable_expr(inf, cx, arg, ta, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
    }
    let t = refine_numeric(inf, recv, &m, op, &[ta], ret);
    (t, Some(m))
}

fn is_refinable_numeric(inf: &BodyInferrer<'_>, m: &InterfaceMember, op: SymbolId) -> bool {
    if m.of_extension {
        return false;
    }
    let s = inf.interner.resolve(op);
    matches!(s, "+" | "-" | "*" | "%" | "remainder")
}

/// Context of the argument of a numeric `e1 op e2` (`_refineNumericInvocationContext`).
pub(crate) fn numeric_context(
    inf: &mut BodyInferrer<'_>,
    t: TypeId,
    m: &InterfaceMember,
    op: SymbolId,
    ctx: TypeId,
    current: TypeId,
) -> TypeId {
    if !is_refinable_numeric(inf, m, op) && Some(op) != inf.sym.clamp {
        return current;
    }
    let num_q = inf.nullable(inf.core.num);
    if !inf.sub(t, num_q) {
        return current;
    }
    let c = inf.greatest_closure_of(ctx);
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let int_q = inf.nullable(int);
    let double_q = inf.nullable(double);
    if inf.sub(int, c) && !inf.sub(num, c) && inf.sub(t, int_q) {
        return int;
    }
    let clamp = Some(op) == inf.sym.clamp;
    if inf.sub(double, c)
        && !inf.sub(num, c)
        && (if clamp {
            inf.sub(t, double_q)
        } else {
            !inf.sub(t, double_q)
        })
    {
        return double;
    }
    num
}

/// Type of a numeric `e1 op e2` (`_refineNumericInvocationTypeNullSafe`).
pub(crate) fn refine_numeric(
    inf: &mut BodyInferrer<'_>,
    t: TypeId,
    m: &InterfaceMember,
    op: SymbolId,
    args: &[TypeId],
    current: TypeId,
) -> TypeId {
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let num_q = inf.nullable(num);
    let int_q = inf.nullable(int);
    let double_q = inf.nullable(double);
    if is_refinable_numeric(inf, m, op) && args.len() == 1 && inf.sub(t, num_q) {
        let s = args[0];
        if inf.sub(t, double_q) {
            return double;
        }
        let bottom = inf.is_bottom(s);
        if !bottom && inf.sub(s, double_q) {
            return double;
        }
        if !bottom && inf.sub(t, int_q) && inf.sub(s, int_q) {
            return int;
        }
        return num;
    }
    if Some(op) == inf.sym.clamp && !m.of_extension && args.len() == 2 && inf.sub(t, num_q) {
        let (t2, t3) = (args[0], args[1]);
        if inf.is_bottom(t2) || inf.is_bottom(t3) {
            return current;
        }
        if inf.sub(t, int_q) && inf.sub(t2, int_q) && inf.sub(t3, int_q) {
            return int;
        }
        if inf.sub(t, double_q) && inf.sub(t2, double_q) && inf.sub(t3, double_q) {
            return double;
        }
        return num;
    }
    current
}

/// Binary operators.
fn binary(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    ctx: TypeId,
) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        // `BinaryExpressionResolver.resolve` (`binary_expression_resolver.dart:69-77`):
        // an operator that is not binary, at the token; `_resolveUnsupportedOperator`
        // visits both sides without a context and the type is `InvalidType`.
        BinaryOp::NotBinary => {
            let left_end = inf.span_expr(cx.unit, left).end;
            let token = operator_token(inf, cx, left_end);
            inf.warning_with_code(
                dartforge_diagnostics::codes::compile_time_error::NOT_BINARY_OPERATOR,
                token,
                &["~"],
            );
            infer_free(inf, cx, left);
            infer_free(inf, cx, right);
            inf.table.invalid(inf.core.dynamic_)
        }
        BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq => {
            let (vf, ff) = binary_condition(inf, cx, e, op, left, right);
            cx.flow = inf.join(&vf, &ff);
            // `x == y` with `x: Never`: the receiver never exists, and the `==`
            // invocation has type `Never` (like any member of `Never`).
            if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                && inf.body_types.units[cx.unit.0 as usize]
                    .get_type(left)
                    .is_some_and(|t| matches!(inf.table.get(t), Type::Never))
            {
                return inf.core.never;
            }
            inf.core.bool_
        }
        BinaryOp::IfNull => {
            let k_q = if inf.the_is_unknown(ctx) {
                ctx
            } else {
                inf.nullable(ctx)
            };
            let t1 = infer(inf, cx, left, k_q);
            void_use(inf, cx, left, t1);
            dead_in_right_operand(inf, cx, left, right);
            warn_dead_null(inf, cx, t1, right);
            let j = if inf.the_is_unknown(ctx) || inf.is_dynamic(ctx) {
                t1
            } else {
                ctx
            };
            // Branch where `e1` is not null: `e1` promotes to non-null; in the
            // other nothing is promoted. The join of both applies afterwards, so
            // `y ?? (throw 0)` leaves `y` promoted.
            let mut before = cx.flow.clone();
            if let Some(id) = promotion_target(inf, cx, left) {
                let decl = cx.local(id).ty;
                inf.promote_non_null(&mut before, id, decl);
            }
            let t2 = infer(inf, cx, right, j);
            let after = cx.flow.clone();
            cx.flow = inf.join(&before, &after);
            let nn = inf.non_null(t1);
            upper_bound_in_context(inf, nn, t2, ctx)
        }
        _ => {
            let u = inf.core.unknown;
            let l = infer(inf, cx, left, u);
            void_use(inf, cx, left, l);
            dead_in_right_operand(inf, cx, left, right);
            if never_receiver(inf, cx, left, l) {
                infer_free(inf, cx, right);
                return inf.core.never;
            }
            if matches!(
                inf.program.unit(cx.unit).ast.expr(left).kind,
                ExprKind::Super
            ) {
                let this = cx.this_type.unwrap_or(inf.core.dynamic_);
                record_type(inf, cx, left, this);
            }
            let sym = operator_symbol(inf, op);
            if let Some((x, args)) = cx.overlays.get(&left).cloned()
                && let Some(text) = operator_text(op)
            {
                if let Some((s, m)) = sym.and_then(|s| {
                    inf.explicit_extension_member(x, &args, s, false)
                        .map(|m| (s, m))
                }) {
                    return binary_operator_with_member(inf, cx, l, s, (right, ctx), Some(e), m).0;
                }
                let start = inf.span_expr(cx.unit, left).end;
                let end = inf.span_expr(cx.unit, right).start;
                let snippet = inf
                    .program
                    .unit(cx.unit)
                    .source
                    .get(start..end)
                    .unwrap_or("");
                let pos = skip_whitespace_and_comments(snippet);
                let extension = inf
                    .program
                    .extension(x)
                    .name
                    .map(|n| inf.interner.resolve(n))
                    .unwrap_or("");
                let msg = format!(
                    "{}: '{}' in '{}'",
                    UNDEFINED_EXTENSION_OPERATOR.template, text, extension
                );
                if snippet
                    .get(pos..)
                    .is_some_and(|rest| rest.starts_with(text))
                {
                    let offset = start + pos;
                    inf.warning(
                        msg,
                        dartforge_diagnostics::Span {
                            start: offset,
                            end: offset + text.len(),
                        },
                    );
                }
                infer_free(inf, cx, right);
                return inf.core.dynamic_;
            }
            let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
            let _ = span;
            let super_ = matches!(ast(inf, cx).expr(left).kind, ExprKind::Super);
            let positions = OperatorPositions {
                token,
                undefined: token,
                compound: false,
                super_,
                numeric_target: None,
            };
            let (t, _) = binary_operator(inf, cx, l, sym, (right, ctx), positions, Some(e));
            t
        }
    }
}

/// The right operand of a binary that starts unreachable: the first dead node
/// has a `BinaryExpression` parent, and the fragment goes from the operator
/// to the end of the right operand (`dead_code_verifier.dart:316-318`).
fn dead_in_right_operand(inf: &mut BodyInferrer<'_>, cx: &mut Body, left: ExprId, right: ExprId) {
    if cx.flow.reachable || cx.dead_code.is_some() {
        return;
    }
    let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
    let end = inf.span_expr(cx.unit, right).end;
    inf.warning(
        DEAD_CODE.template.to_string(),
        dartforge_diagnostics::Span {
            start: token.start,
            end,
        },
    );
    cx.dead_code = Some(cx.flow_ends.len());
}

/// An interpolated `${e}` that ends the flow (`'${throw x} …'`): the
/// `InterpolationString` that always follows it (from just after the `}`) is
/// the first dead node (`checkUnreachableNode` of `visitNode`), and the
/// fragment runs to the end of the current basic block.
fn dead_after_interpolation(inf: &mut BodyInferrer<'_>, cx: &mut Body, i: ExprId) {
    if cx.flow.reachable || cx.dead_code.is_some() {
        return;
    }
    let after = inf.span_expr(cx.unit, i).end;
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(after..).unwrap_or("");
    let pos = after + skip_whitespace_and_comments(rest);
    if !source.get(pos..).unwrap_or("").starts_with('}') {
        return;
    }
    let start = pos + 1;
    let end = cx
        .flow_ends
        .last()
        .copied()
        .or(cx.block_ends.last().copied())
        .unwrap_or(start)
        .max(start);
    inf.warning(
        DEAD_CODE.template.to_string(),
        dartforge_diagnostics::Span { start, end },
    );
    cx.dead_code = Some(cx.flow_ends.len());
}

/// Unary operators (`!`, `-`, `~`, `++`, `--`, postfix `!`).
fn unary(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    op: UnaryOp,
    operand: ExprId,
    ctx: TypeId,
    short_circuit: &mut bool,
) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        UnaryOp::Not => {
            let (vf, ff) = the_condition(inf, cx, operand);
            // Outside a condition, `!e` also requires `bool` (and not `void`).
            verify_bool(inf, cx, operand, BoolUse::Negation);
            cx.flow = inf.join(&vf, &ff);
            inf.core.bool_
        }
        UnaryOp::NullAssert => {
            // The operand of `e!` gets the nullable context (`K?`): in
            // `_name = _becomeParentOf(name)!`, `T` comes out as `IdentifierImpl?`.
            let k = if inf.the_is_unknown(ctx) {
                ctx
            } else {
                inf.nullable(ctx)
            };
            let (t, c) = infer_at(inf, cx, operand, k, true);
            *short_circuit = c;
            // `NULL_CHECK_ALWAYS_FAILS` (`visitPostfixExpression`,
            // `best_practices_verifier.dart:686-697`): `e!` with `e` of type `Null`.
            if is_core_null(inf, t) {
                inf.warning_with_code(
                    dartforge_diagnostics::codes::warning::NULL_CHECK_ALWAYS_FAILS,
                    span,
                    &[],
                );
            }
            // `unnecessary_non_null_assertion` (at the `!`): only with an operand
            // that is certainly non-nullable (`T extends Object?` is not).
            if !c && strictly_non_nullable(inf, t) {
                let sp = dartforge_diagnostics::Span {
                    start: span.end.saturating_sub(1),
                    end: span.end,
                };
                inf.warning_with_code(
                    dartforge_diagnostics::codes::static_warning::UNNECESSARY_NON_NULL_ASSERTION,
                    sp,
                    &[],
                );
            }
            if let Some(id) = promotion_target(inf, cx, operand) {
                let decl = cx.local(id).ty;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                inf.promote_non_null(&mut f, id, decl);
                cx.flow = f;
            }
            // `visitPostfixExpression` with `!`: `checkForUseOfVoidResult(node)`
            // (3.6.2 `error_verifier.dart:1310-1316`), on the whole `e!`.
            let r = inf.non_null_promotion(t);
            if matches!(inf.table.get(r), Type::Void) {
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
                    span,
                    &[],
                );
            }
            r
        }
        UnaryOp::Neg | UnaryOp::BitNot => {
            // `-1` with a literal: the literal gets the context (`double x = -1`).
            let literal = op == UnaryOp::Neg
                && matches!(
                    inf.program.unit(cx.unit).ast.expr(operand).kind,
                    ExprKind::Int(_)
                );
            let c = if literal { ctx } else { inf.core.unknown };
            if literal && op == UnaryOp::Neg {
                cx.negated_literal = Some(operand);
            }
            let t = infer(inf, cx, operand, c);
            void_use(inf, cx, operand, t);
            let sym = if op == UnaryOp::Neg {
                inf.sym.unary_minus
            } else {
                inf.sym.til
            };
            if let Some((x, args)) = cx.overlays.get(&operand).cloned() {
                if let Some(m) = sym.and_then(|s| inf.explicit_extension_member(x, &args, s, false))
                {
                    resolver(inf, cx, e, m.resolved.clone());
                    return match inf.table.get(m.ty) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    };
                }
                let operator = if op == UnaryOp::Neg { "unary-" } else { "~" };
                let extension = inf
                    .program
                    .extension(x)
                    .name
                    .map(|n| inf.interner.resolve(n))
                    .unwrap_or("");
                let msg = format!(
                    "{}: '{}' in '{}'",
                    UNDEFINED_EXTENSION_OPERATOR.template, operator, extension
                );
                inf.warning(
                    msg,
                    dartforge_diagnostics::Span {
                        start: span.start,
                        end: span.start + 1,
                    },
                );
                return inf.core.dynamic_;
            }
            let Some(sym) = sym else {
                return inf.core.dynamic_;
            };
            let the_name = if op == UnaryOp::Neg { "unary-" } else { "~" };
            let sp_op = dartforge_diagnostics::Span {
                start: span.start,
                end: span.start + 1,
            };
            let null = null_in_unary(inf, cx, t, sym, the_name, sp_op, operand);
            let search = inf.lookup_member(cx.lib, t, sym, false);
            // The prefix's extension ambiguity, at the operand.
            let sp_operand = inf.span_expr(cx.unit, operand);
            inf.report_extension_ambiguity(sp_operand);
            match search {
                Search::Finding(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    match inf.table.get(m.ty) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    }
                }
                Search::NeverType => inf.core.never,
                Search::Missing if !null => {
                    undefined_unary_operator(inf, t, the_name, sp_op);
                    inf.table.invalid(inf.core.dynamic_)
                }
                _ => inf.core.dynamic_,
            }
        }
        UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
            let prefix = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
            let bop = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) {
                BinaryOp::Add
            } else {
                BinaryOp::Sub
            };
            let (read, write, local) = read_for_write(inf, cx, operand, short_circuit);
            // `_resolve1` of `PostfixExpressionResolver` and that of
            // `PrefixExpressionResolver` (3.6.2 `:127-132`, `:177-183`): a `Never`
            // read is `RECEIVER_OF_TYPE_NEVER` at the operand, without the operator,
            // and the type is `Never`.
            if matches!(inf.table.get(read), Type::Never) {
                never_receiver(inf, cx, operand, read);
                return inf.core.never;
            }
            let sym = operator_symbol(inf, bop);
            let int = inf.core.int;
            if let Some(s) = sym {
                // The `++`/`--` token, before or after the operand.
                let sp_op = if prefix {
                    dartforge_diagnostics::Span {
                        start: span.start,
                        end: span.start + 2,
                    }
                } else {
                    dartforge_diagnostics::Span {
                        start: span.end.saturating_sub(2),
                        end: span.end,
                    }
                };
                let the_name = if bop == BinaryOp::Add { "+" } else { "-" };
                let null = null_in_unary(inf, cx, read, s, the_name, sp_op, operand);
                // `C++` with `C` a type: only the write's `ASSIGNMENT_TO_TYPE`.
                let of_type = type_reference(inf, cx, operand).is_some()
                    || matches!(&ast(inf, cx).expr(operand).kind, ExprKind::Identifier(n)
                        if matches!(resolve_name(inf, cx, n.sym, false), NameRef::BuiltinType | NameRef::TypeParam(_)));
                if !null
                    && !of_type
                    && matches!(inf.lookup_member(cx.lib, read, s, false), Search::Missing)
                {
                    undefined_unary_operator(inf, read, the_name, sp_op);
                }
            }
            let res = match sym.map(|s| inf.lookup_member(cx.lib, read, s, false)) {
                Some(Search::Finding(m)) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    let (ret, param) = match inf.table.get(m.ty) {
                        Type::Function {
                            ret, positional, ..
                        } => (*ret, positional.first().copied()),
                        _ => (inf.core.dynamic_, None),
                    };
                    let r = refine_numeric(inf, read, &m, sym.unwrap(), &[int], ret);
                    // `++a`/`a++`: the implicit `1` (`int`) against the operator's parameter,
                    // reported at the operand
                    // (`_checkForIntNotAssignable`, an611:src/generated/error_verifier.dart:3899-3906);
                    // the result against the write type, on the whole node
                    // (`_checkForInvalidAssignmentIncDec`,
                    // an611:src/dart/resolver/prefix_expression_resolver.dart:96-110).
                    if !inf.is_dynamic(read) && !matches!(inf.table.get(read), Type::Void) {
                        if let Some(p) = param {
                            let sp = inf.span_expr(cx.unit, without_parentheses(inf, cx, operand));
                            inf.verify_assignable(
                                int,
                                p,
                                sp,
                                ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                            );
                        }
                        // A getter and setter of different types (`int get x`,
                        // `set x(String)`): the 3.6.2 oracle does not report (checked against
                        // the corpus, `InvalidAssignment__postfixExpression_in_*`).
                        // The operator's return (`operatorReturnType`), without the null
                        // short-circuit's `?` (`x?[0]++`).
                        if !inf.is_dynamic(write) && (local.is_some() || read == write) {
                            inf.verify_assignable(r, write, span, INVALID_ASSIGNMENT.template);
                        }
                    }
                    r
                }
                Some(Search::NeverType) => inf.core.never,
                _ => inf.core.dynamic_,
            };
            if let Some(id) = local {
                // The write rule sees the state from before the write.
                let target_span = inf.span_expr(cx.unit, operand);
                check_final_local(inf, cx, id, target_span);
                let decl = cx.local(id).ty;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                let reason = super::flow::NonPromotionReason::Write {
                    name: cx.local(id).name,
                    span,
                };
                inf.assign_flow(&mut f, id, decl, res, Some(reason));
                cx.flow = f;
            }
            let _ = write;
            if prefix { res } else { read }
        }
    }
}

/// A prefix or postfix operator (`-x`, `~x`, `++x`, `x--`) on a potentially
/// nullable receiver: `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` at the
/// operator token, through `TypePropertyResolver` (a pure `Null` is
/// `INVALID_USE_OF_NULL_VALUE`), with the operand's non-promotion messages.
fn null_in_unary(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    recv: TypeId,
    sym: SymbolId,
    the_name: &str,
    sp: dartforge_diagnostics::Span,
    operand: ExprId,
) -> bool {
    // `void` is potentially nullable and is not bounded by `dynamic`:
    // `TypePropertyResolver` (`type_property_resolver.dart:80-150`) reports
    // an operator that neither `Object` nor an extension has (besides the
    // `USE_OF_VOID_RESULT` at the operand).
    let requires = if matches!(inf.table.get(recv), Type::Void) {
        let o = inf.core.object;
        inf.interface_member(o, sym, false).is_none()
            && inf.extension_member(cx.lib, recv, sym, false).is_none()
    } else {
        inf.requires_null_check(cx.lib, recv, sym, false)
    };
    if !requires {
        return false;
    }
    let since = inf.diagnostics.len();
    inf.null_warning(recv, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &[the_name]);
    inf.attach_non_promotion(since, cx, Some(operand), sp);
    true
}

/// A prefix/postfix operator that the operand's type does not have
/// (`TypePropertyResolver` without a member): `UNDEFINED_OPERATOR` at the operator
/// token (`prefix_expression_resolver.dart`, `postfix_expression_resolver.dart`).
fn undefined_unary_operator(
    inf: &mut BodyInferrer<'_>,
    recv: TypeId,
    the_name: &str,
    sp: dartforge_diagnostics::Span,
) {
    if matches!(inf.table.get(recv), Type::Void | Type::Function { .. })
        || inf.table.is_invalid(recv)
    {
        return;
    }
    let typ = inf.table.format(recv, inf.interner, inf.program);
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_OPERATOR,
        sp,
        &[the_name, &typ],
    );
}

pub(crate) fn check_final_local(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    id: LocalId,
    span: dartforge_diagnostics::Span,
) {
    let l = cx.local(id);
    if cx.local_functions.contains(&id) {
        inf.warning(ASSIGNMENT_TO_FUNCTION.template.to_string(), span);
    } else if l.const_ {
        inf.warning(ASSIGNMENT_TO_CONST.template.to_string(), span);
    } else if l.final_ && l.late {
        if cx.flow.assigned(id) {
            inf.warning_with_code(
                dartforge_diagnostics::codes::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED,
                span,
                &[],
            );
        }
    } else if l.final_ && !cx.flow.unassigned(id) {
        let msg = format!(
            "{}: '{}'",
            ASSIGNMENT_TO_FINAL_LOCAL.template,
            inf.interner.resolve(l.name)
        );
        inf.warning(msg, span);
    }
}

fn warn_write_to_method(inf: &mut BodyInferrer<'_>, the_name: ast::Name) {
    inf.warning(ASSIGNMENT_TO_METHOD.template.to_string(), the_name.span);
}

/// Diagnostic for when the recovery of a write finds only the getter.
/// An explicit getter is a property without a setter; an implicit getter is a
/// `final` or `const` field. A `late final` without an initializer still accepts
/// writes.
fn warn_member_without_setter(
    inf: &mut BodyInferrer<'_>,
    the_name: ast::Name,
    f: dartforge_elements::model::FunctionElementId,
) -> bool {
    let fe = inf.program.function(f);
    match fe.kind {
        FunctionKind::Getter => {
            let owner = fe
                .class
                .map(|c| inf.program.class(c).name)
                .or_else(|| fe.extension.and_then(|e| inf.program.extension(e).name));
            let Some(owner) = owner else { return false };
            let msg = format!(
                "{}: '{}' in class '{}'",
                ASSIGNMENT_TO_FINAL_NO_SETTER.template,
                inf.interner.resolve(the_name.sym),
                inf.interner.resolve(owner)
            );
            inf.warning(msg, the_name.span);
            true
        }
        FunctionKind::ImplicitAccessor => {
            let Some(v) = fe.variable else { return false };
            let ve = inf.program.variable(v);
            if ve.const_ {
                inf.warning(ASSIGNMENT_TO_CONST.template.to_string(), the_name.span);
            } else if ve.final_ && !(ve.late && inf.initializer(v).is_none()) {
                let msg = format!(
                    "{}: '{}'",
                    ASSIGNMENT_TO_FINAL.template,
                    inf.interner.resolve(the_name.sym)
                );
                inf.warning(msg, the_name.span);
            } else {
                return false;
            }
            true
        }
        _ => false,
    }
}

/// For `x op= e` / `x++`: reads the target, returning `(typ lido, typ of write, local)`.
/// `short_circuit` becomes true when the target is in a `?.`/`?[` chain (the
/// expression's result is then nullable: `a?.x += 1` is null with `a`
/// null).
fn read_for_write(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    tgt: ExprId,
    short_circuit_out: &mut bool,
) -> (TypeId, TypeId, Option<LocalId>) {
    let a = ast(inf, cx);
    match &a.expr(tgt).kind {
        ExprKind::Identifier(n) => {
            let n = *n;
            match resolve_name(inf, cx, n.sym, true) {
                NameRef::Local(id) => {
                    resolver(inf, cx, tgt, Resolved::Local(id));
                    join_reference(inf, cx, id, n);
                    write_in_guard(inf, cx, id, n.span);
                    write_in_primary(inf, cx, id, n.span);
                    let t = read_local(inf, cx, id, n.span);
                    let decl = cx.local(id).ty;
                    (t, decl, Some(id))
                }
                _ => {
                    let read = identifier(inf, cx, tgt, n);
                    let write = name_write_type(inf, cx, tgt, n);
                    (read, write, None)
                }
            }
        }
        ExprKind::Property {
            target,
            name,
            null_aware,
        } => {
            let (target, name, null_aware) = (*target, *name, *null_aware);
            let (read, short_circuit_read) = property(inf, cx, tgt, target, name, null_aware);
            let recv_read = inf.body_types.units[cx.unit.0 as usize]
                .get_type(target)
                .map(|t| {
                    (
                        if null_aware { inf.non_null(t) } else { t },
                        short_circuit_read,
                    )
                });
            let mut short_circuit = false;
            let write = property_write(
                inf,
                cx,
                tgt,
                (target, name, null_aware),
                &mut short_circuit,
                recv_read,
            );
            *short_circuit_out = short_circuit_read || short_circuit;
            record_type(inf, cx, tgt, read);
            (read, write, None)
        }
        ExprKind::Index {
            target,
            index,
            null_aware,
        } => {
            let (target, index, null_aware) = (*target, *index, *null_aware);
            let u = inf.core.unknown;
            let (t, c) = read_index(inf, cx, tgt, target, index, null_aware, u);
            *short_circuit_out = c;
            // The write type (`setWriteElement` with `IndexExpression`, 3.6.2
            // `resolver.dart:1722-1728`): the 2nd parameter of `[]=`; without it,
            // `InvalidType` (`dynamic` on a `dynamic` target).
            let second = |inf: &BodyInferrer<'_>, f: TypeId| match inf.table.get(f) {
                Type::Function { positional, .. } if positional.len() == 2 => Some(positional[1]),
                _ => None,
            };
            let mut write = inf.table.invalid(inf.core.dynamic_);
            if let Some((x, args)) = cx.overlays.get(&target).cloned() {
                match inf
                    .sym
                    .set_index
                    .and_then(|s| inf.explicit_extension_member(x, &args, s, false))
                {
                    Some(m) => {
                        if let Some(p1) = second(inf, m.ty) {
                            write = p1;
                        }
                    }
                    None => {
                        let span = index_span(inf, cx, tgt, target);
                        warn_extension_operator(inf, x, "[]=", span);
                    }
                }
            } else if let Some(op) = inf.sym.set_index
                && matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `super[i]++`: the `[]=` also through the `super` chain.
                match lookup_super_operator(inf, cx, op) {
                    Search::Missing => {
                        let span = index_span(inf, cx, tgt, target);
                        let this = cx.this_type.unwrap_or(inf.core.dynamic_);
                        let typ = inf.table.format(this, inf.interner, inf.program);
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::UNDEFINED_SUPER_OPERATOR, span, &["[]=", &typ]);
                    }
                    Search::Finding(m) => {
                        if let Some(p1) = second(inf, m.ty) {
                            write = p1;
                        }
                    }
                    _ => {}
                }
            } else if let Some(op) = inf.sym.set_index
                && let Some(recv) = inf.body_types.units[cx.unit.0 as usize].get_type(target)
                && !matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `x[i]++`: the `[]=` is also looked up (the `writeElement` of
                // `PropertyElementResolver`); missing, `UNDEFINED_OPERATOR` at the
                // `[…]` (`_reportUnresolvedIndex`).
                let r = if null_aware { inf.non_null(recv) } else { recv };
                let invalid = inf.table.is_invalid(r)
                    || matches!(inf.table.get(r), Type::Dynamic | Type::Never | Type::Void);
                if matches!(inf.table.get(r), Type::Dynamic) && !inf.table.is_invalid(r) {
                    write = inf.core.dynamic_;
                }
                if !invalid
                    && let Search::Finding(m) = inf.lookup_member(cx.lib, r, op, false)
                    && let Some(p1) = second(inf, m.ty)
                {
                    write = p1;
                }
                if !invalid && !inf.requires_null_check(cx.lib, r, op, false) {
                    match inf.lookup_member(cx.lib, r, op, false) {
                        Search::Missing => {
                            let span = index_span(inf, cx, tgt, target);
                            let typ = inf.table.format(r, inf.interner, inf.program);
                            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::UNDEFINED_OPERATOR, span, &["[]=", &typ]);
                        }
                        // `checkIndexExpressionIndex` with the `writeElement` (3.6.2
                        // `error_detection_helpers.dart:257-287`, `resolver.dart:1443`): the
                        // index also against the first parameter of `[]=`.
                        Search::Finding(m) if m.method => {
                            if let Type::Function { positional, .. } = inf.table.get(m.ty).clone()
                                && let Some(&p0) = positional.first()
                                && let Some(ti) =
                                    inf.body_types.units[cx.unit.0 as usize].get_type(index)
                            {
                                let sp = inf.span_expr(cx.unit, index);
                                inf.verify_assignable(
                                    ti,
                                    p0,
                                    sp,
                                    ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            record_type(inf, cx, tgt, t);
            (t, write, None)
        }
        _ => {
            let t = infer_free(inf, cx, tgt);
            (t, t, None)
        }
    }
}

/// `_checkForAmbiguousImport` (3.6.2 `error_verifier.dart:2116-2130`): a name
/// (with the prefix) that two imports bring in with different elements is
/// `AMBIGUOUS_IMPORT` at the name; the element is the
/// `MultiplyDefinedElement` and the type is invalid. Says whether it
/// reported.
pub(crate) fn ambiguous_import(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    prefix: Option<SymbolId>,
    the_name: ast::Name,
) -> bool {
    let b = match prefix {
        Some(p) => inf
            .program
            .lookup_prefixed_in_unit(cx.unit, p, the_name.sym),
        None => inf.program.lookup_in_unit(cx.unit, the_name.sym),
    };
    if !b.is_some_and(|b| b.ambiguous) {
        return false;
    }
    let list = crate::scope::ambiguous_libraries(inf.program, cx.unit, prefix, the_name.sym);
    let text = inf.interner.resolve(the_name.sym).to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::AMBIGUOUS_IMPORT,
        the_name.span,
        &[&text, &list],
    );
    true
}

/// Write type (setter) of a non-local name.
fn name_write_type(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, n: ast::Name) -> TypeId {
    match resolve_name(inf, cx, n.sym, true) {
        NameRef::Local(id) => cx.local(id).ty,
        NameRef::Element(_) if ambiguous_import(inf, cx, None, n) => {
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::Element(el) => {
            resolver(inf, cx, tgt, Resolved::Element(el));
            match el {
                // `setWriteElement` (3.6.2 `resolver.dart:1709-1747`): the write type
                // only comes from a setter (the synthetic one, through the variable)
                // or from a `VariableElement`; a `final`/`const` top-level or static
                // variable has no setter, the write falls back to the recovery getter
                // and the type is `InvalidType` (without `INVALID_ASSIGNMENT`).
                Element::Variable(v) => {
                    let ve = inf.program.variable(v);
                    if ve.const_ {
                        inf.warning(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                        inf.table.invalid(inf.core.dynamic_)
                    } else if ve.final_ && !(ve.late && inf.initializer(v).is_none()) {
                        let msg = format!(
                            "{}: '{}'",
                            ASSIGNMENT_TO_FINAL.template,
                            inf.interner.resolve(n.sym)
                        );
                        inf.warning(msg, n.span);
                        inf.table.invalid(inf.core.dynamic_)
                    } else {
                        inf.variable_type(v)
                    }
                }
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
                        (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize]
                            .parameters
                            .first()
                            .map(|p| p.ty)
                            .unwrap_or(inf.core.dynamic_),
                        (FunctionKind::Getter, _) => {
                            let msg = format!(
                                "{}: '{}'",
                                ASSIGNMENT_TO_FINAL.template,
                                inf.interner.resolve(n.sym)
                            );
                            inf.warning(msg, n.span);
                            inf.table.invalid(inf.core.dynamic_)
                        }
                        (FunctionKind::Function, _) => {
                            inf.warning(ASSIGNMENT_TO_FUNCTION.template.to_string(), n.span);
                            inf.core.dynamic_
                        }
                        _ => inf.core.dynamic_,
                    }
                }
                Element::Class(_) | Element::Typedef(_) => {
                    inf.warning(ASSIGNMENT_TO_TYPE.template.to_string(), n.span);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        NameRef::TypeParam(_) | NameRef::BuiltinType => {
            inf.warning(ASSIGNMENT_TO_TYPE.template.to_string(), n.span);
            inf.core.dynamic_
        }
        // `AssignmentVerifier.verify` (`assignment_verifier.dart:66-71`): the
        // write's recovery is the prefix.
        NameRef::Prefix => {
            prefix_without_dot(inf, n);
            inf.core.dynamic_
        }
        NameRef::LexicalMember(f, static_) => {
            let r = lexical_member_resolved(inf, cx, f, static_);
            resolver(inf, cx, tgt, r);
            let fe = inf.program.function(f);
            // An instance setter written where `this` is not available (static
            // method, factory, initializer): the write element is it
            // (`setWriteElement`) and the error is the access without `this`
            // (`_checkForInvalidInstanceMemberAccess`).
            if !static_ && instance_without_this_error(inf, cx).is_some() {
                let write_type = match (fe.kind, fe.variable) {
                    (FunctionKind::Setter, _) => Some(
                        inf.outline.functions[f.0 as usize]
                            .parameters
                            .first()
                            .map(|p| p.ty)
                            .unwrap_or(inf.core.dynamic_),
                    ),
                    (FunctionKind::ImplicitAccessor, Some(v))
                        if !inf.program.variable(v).final_ && !inf.program.variable(v).const_ =>
                    {
                        Some(inf.variable_type(v))
                    }
                    _ => None,
                };
                if let Some(t) = write_type {
                    warn_instance_without_this(inf, cx, n);
                    return t;
                }
            }
            if fe.kind == FunctionKind::Function && fe.class.is_some() {
                warn_write_to_method(inf, n);
                // `setWriteElement`: the write type only comes from a setter or a
                // variable; from a method it is `InvalidType`.
                return inf.table.invalid(inf.core.dynamic_);
            }
            // `LexicalLookup.resolveSetter` (3.6.2 `lexical_lookup.dart`): a getter
            // found without a setter only goes on to the implicit `this`
            // (`ThisLookup.lookupSetter`, where an inherited or extension setter may
            // appear) when it `isInstanceMember`, an instance member of an
            // `InterfaceElement` (`extensions.dart:109-119`). Static and extension ones
            // are the recovery: `static final int x` in a class whose superclass has an
            // extension setter `x` gives
            // `ASSIGNMENT_TO_FINAL` (`static_extension_internal_basename_shadowing_error_test.dart:402`).
            let follows_through_this = !static_ && fe.extension.is_none();
            if let (FunctionKind::ImplicitAccessor, Some(v)) = (fe.kind, fe.variable) {
                let ve = inf.program.variable(v);
                if (ve.final_ || ve.const_) && ve.setter.is_none() && follows_through_this {
                    // A final field without a setter: there may be an inherited setter.
                    if let Some(this) = cx.this_type
                        && let Search::Finding(m) = inf.lookup_member(cx.lib, this, n.sym, true)
                    {
                        return m.ty;
                    }
                }
            }
            if follows_through_this
                && let Some(this) = cx.this_type
                && let Search::Finding(m) = inf.lookup_member(cx.lib, this, n.sym, true)
            {
                return m.ty;
            }
            warn_member_without_setter(inf, n, f);
            // Without a setter, the write element is the recovery one (the getter):
            // `InvalidType` (`setWriteElement`).
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::ImplicitThis => {
            let this = cx.this_type.unwrap();
            // `ThisLookup.lookupSetter` with a potentially nullable `this`: 3.6.2
            // reports the method variant (`The method 'foo' can't be
            // unconditionally invoked`) on every write through the implicit `this`
            // (`=`, compound, `??=`, `++`), checked against the oracle.
            if inf.requires_null_check(cx.lib, this, n.sym, true) {
                let the_name = inf.interner.resolve(n.sym).to_string();
                let since = inf.diagnostics.len();
                inf.null_warning(this, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, n.span, &[&the_name]);
                inf.attach_non_promotion(since, cx, None, n.span);
                let nn = inf.non_null(this);
                return match inf.lookup_member(cx.lib, nn, n.sym, true) {
                    Search::Finding(m) => {
                        resolver(inf, cx, tgt, m.resolved.clone());
                        m.ty
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.lookup_member(cx.lib, this, n.sym, true) {
                Search::Finding(m) => {
                    resolver(inf, cx, tgt, m.resolved.clone());
                    m.ty
                }
                Search::Missing => {
                    if let Search::Finding(getter) = inf.lookup_member(cx.lib, this, n.sym, false)
                        && let Some(f) = getter.function
                        && warn_member_without_setter(inf, n, f)
                    {
                        resolver(inf, cx, tgt, getter.resolved);
                        return getter.ty;
                    }
                    undefined_written_name(inf, cx, n);
                    inf.core.dynamic_
                }
                // Ambiguous extensions (`ThisLookup.lookupSetter`): the ambiguity and,
                // without a setter, the undefined name.
                Search::Dynamic if inf.extension_ambiguity.is_some() => {
                    inf.report_extension_ambiguity(n.span);
                    undefined_name_without_this(inf, cx, n);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        NameRef::ConstantEnum(v) => inf.variable_type(v),
        // Only wildcards declare `_` here (3.7): using `_` is an error
        // (`Undefined name '_'` in the CFE).
        NameRef::None_ if cx.wildcard == Some(n.sym) => {
            inf.language_error(cx.unit, n.span, WILDCARD_DOES_NOT_BIND.template.to_string());
            inf.core.dynamic_
        }
        NameRef::None_ => {
            undefined_written_name(inf, cx, n);
            inf.core.dynamic_
        }
        _ => inf.core.dynamic_,
    }
}

/// A write to a simple name without a setter in the lexical scope
/// (`AssignmentVerifier.verify`, `an611:src/error/assignment_verifier.dart:28-113`,
/// with `ThisLookup.lookupSetter`): the implicit `this` may still find the
/// setter (the error is then the access without `this` or the unqualified
/// static one) or a recovery getter (the error is a different one: write to a
/// final, to a method…); otherwise, undefined name, without the exception for
/// imports that do not exist, which the write does not consult.
fn undefined_written_name(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    if is_synthetic_name(inf, n) {
        return;
    }
    if let Some(this) = analyzer_this_type(inf, cx) {
        match lookup_through_this(inf, cx, this, n.sym, true) {
            ThroughThis::Instance => {
                warn_instance_without_this(inf, cx, n);
                return;
            }
            ThroughThis::Static(owner, _) => {
                warn_unqualified_static(inf, cx, owner, n);
                return;
            }
            ThroughThis::Uncertain => return,
            ThroughThis::Missing => {}
        }
        if !matches!(
            lookup_through_this(inf, cx, this, n.sym, false),
            ThroughThis::Missing
        ) {
            return;
        }
    }
    let msg = format!(
        "{}: '{}'",
        UNDEFINED_IDENTIFIER.template,
        inf.interner.resolve(n.sym)
    );
    inf.warning(msg, n.span);
}

/// Assignments: `=`, compound ones and `??=`.
fn assignment(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    op: AssignOp,
    tgt: ExprId,
    the_value: ExprId,
    short_circuit: &mut bool,
) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    let a = ast(inf, cx);
    let target_kind = &a.expr(tgt).kind;
    match op {
        AssignOp::Assign => {
            // Write type and context.
            let (write, context, local) = match target_kind {
                ExprKind::Identifier(n) => {
                    let n = *n;
                    match resolve_name(inf, cx, n.sym, true) {
                        NameRef::Local(id) => {
                            resolver(inf, cx, tgt, Resolved::Local(id));
                            join_reference(inf, cx, id, n);
                            write_in_guard(inf, cx, id, n.span);
                            write_in_primary(inf, cx, id, n.span);
                            let l = cx.local(id).clone();
                            if cx.local_functions.contains(&id) {
                                inf.warning(ASSIGNMENT_TO_FUNCTION.template.to_string(), n.span);
                                (inf.core.dynamic_, inf.core.dynamic_, None)
                            } else {
                                // `assignment_expression_resolver.dart:360-385`:
                                // an already assigned `late final` and a possibly assigned `final`.
                                if l.const_ {
                                    if !cx.flow.unassigned(id) {
                                        inf.warning(
                                            ASSIGNMENT_TO_CONST.template.to_string(),
                                            n.span,
                                        );
                                    }
                                } else if l.final_ && l.late {
                                    if cx.flow.assigned(id) {
                                        inf.warning_with_code(
                                            dartforge_diagnostics::codes::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED,
                                            n.span,
                                            &[],
                                        );
                                    }
                                } else if l.final_ && !cx.flow.unassigned(id) {
                                    let msg = format!(
                                        "{}: '{}'",
                                        ASSIGNMENT_TO_FINAL_LOCAL.template,
                                        inf.interner.resolve(l.name)
                                    );
                                    inf.warning(msg, n.span);
                                }
                                let current = cx.flow.current_type(id, l.ty);
                                (l.ty, current, Some(id))
                            }
                        }
                        _ => {
                            let t = name_write_type(inf, cx, tgt, n);
                            (t, t, None)
                        }
                    }
                }
                ExprKind::Property {
                    target,
                    name,
                    null_aware,
                } => {
                    let (target, name, null_aware) = (*target, *name, *null_aware);
                    let t = property_write(
                        inf,
                        cx,
                        tgt,
                        (target, name, null_aware),
                        short_circuit,
                        None,
                    );
                    (t, t, None)
                }
                ExprKind::Index {
                    target,
                    index,
                    null_aware,
                } => {
                    let (target, index, null_aware) = (*target, *index, *null_aware);
                    let (recv, c) = the_receiver(inf, cx, target, null_aware);
                    *short_circuit = c;
                    let raw = inf.body_types.units[cx.unit.0 as usize]
                        .get_type(target)
                        .unwrap_or(recv);
                    let bnd = inf.resolve_to_bound(raw);
                    if never_receiver(inf, cx, target, bnd) {
                        infer_free(inf, cx, index);
                        let d = inf.core.dynamic_;
                        (d, d, None)
                    } else {
                        let t = index_write(inf, cx, tgt, recv, index, span);
                        (t, t, None)
                    }
                }
                _ => {
                    let t = infer_free(inf, cx, tgt);
                    (t, t, None)
                }
            };
            inf.body_types.units[cx.unit.0 as usize]
                .write_types
                .insert(e, write);
            // `flow.write(…, rhs)` (`_write`, `flow_analysis.dart:6133-6149`): writing
            // a condition into a local keeps its true/false models in the SSA node,
            // like the initializer (§7.10).
            let (tv, saved_condition) = if local.is_some() && is_condition_form(inf, cx, the_value)
            {
                let (yes, not_) = the_condition(inf, cx, the_value);
                cx.flow = inf.join(&yes, &not_);
                let t = inf.body_types.units[cx.unit.0 as usize]
                    .get_type(the_value)
                    .unwrap_or(inf.core.bool_);
                (t, Some((yes, not_)))
            } else {
                (infer(inf, cx, the_value, context), None)
            };
            if let Some(id) = local {
                let decl = cx.local(id).ty;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                let reason = super::flow::NonPromotionReason::Write {
                    name: cx.local(id).name,
                    span: inf.span_expr(cx.unit, e),
                };
                inf.assign_flow(&mut f, id, decl, tv, Some(reason));
                cx.flow = f;
                cx.forget_fields_of(id);
                if let (Some((yes, not_)), Some(version)) = (saved_condition, cx.flow.version(id)) {
                    cx.conditions.insert(id, (yes, not_, version));
                }
            }
            if !inf.is_dynamic(write) {
                verify_assignable_expr_at(
                    inf,
                    cx,
                    the_value,
                    tv,
                    write,
                    INVALID_ASSIGNMENT.template,
                    false,
                );
            } else {
                void_use(inf, cx, the_value, tv);
            }
            tv
        }
        AssignOp::Compound(bop) => {
            let (read, write, local) = read_for_write(inf, cx, tgt, short_circuit);
            inf.body_types.units[cx.unit.0 as usize]
                .write_types
                .insert(e, write);
            // `x += 1` / `x ??= 3` with `x` of type `void`: at the operator.
            if matches!(inf.table.get(read), Type::Void) {
                let token = operator_token(inf, cx, inf.span_expr(cx.unit, tgt).end);
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
                    token,
                    &[],
                );
            }
            if bop == BinaryOp::IfNull {
                // `checkFinalAlreadyAssigned(left)` before the right side (3.6.2
                // `assignment_expression_resolver.dart:79`), with the state from
                // before the write.
                if let Some(id) = local {
                    let target_span = inf.span_expr(cx.unit, tgt);
                    check_final_local(inf, cx, id, target_span);
                }
                // The right side only runs if the target is null: what it promotes or
                // assigns does not hold afterwards (`origin ??= element!.library;`
                // does not promote `element`).
                let before = cx.flow.clone();
                // An index without a resolved `[]` (`Never` receiver) has an invalid
                // `readType` (3.6.2 `resolver.dart:1665-1672`): `isStrictlyNonNullable`
                // does not apply.
                // The same for a name that is neither a getter nor a variable (class,
                // alias, function, method, type parameter: `C ??= null`).
                let without_read = match &ast(inf, cx).expr(tgt).kind {
                    ExprKind::Index {
                        target,
                        null_aware: false,
                        ..
                    } => inf.body_types.units[cx.unit.0 as usize]
                        .get_type(*target)
                        .is_some_and(|r| {
                            matches!(inf.table.get(inf.resolve_to_bound(r)), Type::Never)
                        }),
                    ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                        let is_method = |inf: &BodyInferrer<'_>, f: dartforge_elements::model::FunctionElementId| {
                            matches!(inf.program.function(f).kind, FunctionKind::Function)
                        };
                        match inf.body_types.units[cx.unit.0 as usize]
                            .get_resolved(tgt)
                            .cloned()
                        {
                            Some(Resolved::Element(Element::Function(f))) => is_method(inf, f),
                            Some(Resolved::Element(Element::Variable(_))) => false,
                            Some(Resolved::Element(_)) => true,
                            Some(Resolved::Member {
                                member: MemberRef::Function(f),
                                ..
                            }) => is_method(inf, f),
                            Some(Resolved::ExtensionMember { member: f, .. }) => is_method(inf, f),
                            Some(
                                Resolved::TypeParameter(_)
                                | Resolved::Constructor(_)
                                | Resolved::Prefix(_),
                            ) => true,
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if !without_read {
                    warn_dead_null(inf, cx, read, the_value);
                }
                let tv = infer(inf, cx, the_value, write);
                if !matches!(inf.table.get(write), Type::Void) {
                    void_use(inf, cx, the_value, tv);
                }
                // `x ??= e`: the type of `e` against the write type, at the right side
                // (`_checkForInvalidAssignment`,
                // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 263-270).
                if !inf.is_dynamic(write) && !matches!(inf.table.get(tv), Type::Void) {
                    let sp = inf.span_expr(cx.unit, the_value);
                    inf.verify_assignable(tv, write, sp, INVALID_ASSIGNMENT.template);
                }
                let nn = inf.non_null(read);
                let t = inf.up(nn, tv);
                // The write only happens on the null branch (`ifNullExpression_
                // rightBegin` … `write` … `ifNullExpression_end`): after the join the
                // local is only potentially assigned if it was not before.
                if let Some(id) = local {
                    let decl = cx.local(id).ty;
                    let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                    let reason = super::flow::NonPromotionReason::Write {
                        name: cx.local(id).name,
                        span: inf.span_expr(cx.unit, e),
                    };
                    inf.assign_flow(&mut f, id, decl, tv, Some(reason));
                    cx.flow = f;
                }
                let after = cx.flow.clone();
                // On the branch without the right side the target is not null.
                let mut before = before;
                if let Some(id) = local {
                    let decl = cx.local(id).ty;
                    inf.promote_non_null(&mut before, id, decl);
                }
                cx.flow = inf.join(&before, &after);
                return t;
            }
            let sym = operator_symbol(inf, bop);
            let token = operator_token(inf, cx, inf.span_expr(cx.unit, tgt).end);
            // The right side's context: `refineNumericInvocationContext` with the
            // write type as target and as context.
            let positions = OperatorPositions {
                token,
                undefined: token,
                compound: true,
                super_: false,
                numeric_target: Some(write),
            };
            let (t, _) =
                binary_operator(inf, cx, read, sym, (the_value, write), positions, Some(e));
            // `x op= e`: the operator's return against the write type, at the right
            // side (`_resolveTypes` + `_checkForInvalidAssignment`,
            // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 272-290).
            if !inf.is_dynamic(write)
                && !inf.is_dynamic(read)
                && !matches!(inf.table.get(t), Type::Void)
                && (local.is_some() || read == write)
            {
                let sp = inf.span_expr(cx.unit, the_value);
                inf.verify_assignable(t, write, sp, INVALID_ASSIGNMENT.template);
            }
            if let Some(id) = local {
                // The write rule sees the state from before the write.
                let target_span = inf.span_expr(cx.unit, tgt);
                check_final_local(inf, cx, id, target_span);
                let decl = cx.local(id).ty;
                let mut f = std::mem::replace(&mut cx.flow, Flow::reachable());
                let reason = super::flow::NonPromotionReason::Write {
                    name: cx.local(id).name,
                    span: inf.span_expr(cx.unit, e),
                };
                inf.assign_flow(&mut f, id, decl, t, Some(reason));
                cx.flow = f;
            }
            t
        }
    }
}

/// `r.x = …`: the setter's type.
fn property_write(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    tgt: ExprId,
    (target, name, null_aware): (ExprId, ast::Name, bool),
    short_circuit: &mut bool,
    recv_read: Option<(TypeId, bool)>,
) -> TypeId {
    // `Fn<int>.foo = 1`: a setter with no possible resolution; in a compound
    // one only the getter reports (`hasRead`), already at the read.
    if let ExprKind::TypeArguments { target: base, .. } = &ast(inf, cx).expr(target).kind
        && function_type_alias(inf, cx, *base)
    {
        if recv_read.is_none() {
            function_type_literal(inf, cx, target, name, false);
        }
        return inf.table.invalid(inf.core.dynamic_);
    }
    let a = ast(inf, cx);
    if let ExprKind::Identifier(p) = &a.expr(target).kind
        && matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix)
    {
        if null_aware {
            prefix_without_dot(inf, *p);
        }
        resolver(inf, cx, target, Resolved::Prefix(cx.lib));
        if ambiguous_import(inf, cx, Some(p.sym), name) {
            return inf.table.invalid(inf.core.dynamic_);
        }
        if let Some(el) = inf
            .program
            .lookup_prefixed_in_unit(cx.unit, p.sym, name.sym)
            .and_then(|b| b.setter.or(b.getter))
        {
            resolver(inf, cx, tgt, Resolved::Element(el));
            return match el {
                Element::Variable(v) => inf.variable_type(v),
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
                        _ => inf.outline.functions[f.0 as usize]
                            .parameters
                            .first()
                            .map(|p| p.ty)
                            .unwrap_or(inf.core.dynamic_),
                    }
                }
                _ => inf.core.dynamic_,
            };
        }
        return inf.core.dynamic_;
    }
    if null_aware && type_reference(inf, cx, target).is_some() {
        null_operator_on_type(inf, cx, target);
    }
    if let Some(rt) = type_reference(inf, cx, target) {
        record_type_ref(inf, cx, target);
        if null_aware && !shortening_waived(inf, cx, target, &rt) {
            *short_circuit = true;
        }
        let explicit_instance = matches!(
            &ast(inf, cx).expr(target).kind,
            ExprKind::TypeArguments { .. }
        );
        if explicit_instance && let TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) = &rt {
            if inf.static_member(*c, name.sym, true).is_some()
                || inf.static_member(*c, name.sym, false).is_some()
            {
                warn_static_instantiation(inf, cx, tgt, name);
                return inf.core.dynamic_;
            }
            if warn_class_instantiation(inf, cx, tgt, *c, name, true) {
                return inf.core.dynamic_;
            }
        }
        let m = match &rt {
            TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) => inf.static_member(*c, name.sym, true),
            TypeRef::Extension(x) => inf.extension_static_member(*x, name.sym, true),
        };
        return match m {
            Some(m) => {
                // A private static setter from another library: found, but
                // inaccessible (`PRIVATE_SETTER`, 3.6.2
                // `property_element_resolver.dart:710-719`).
                if let TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) = &rt
                    && inf.interner.resolve(name.sym).starts_with('_')
                    && inf.program.class(*c).library != cx.lib
                {
                    let text = inf.interner.resolve(name.sym).to_string();
                    inf.warning_with_code(
                        dartforge_diagnostics::codes::compile_time_error::PRIVATE_SETTER,
                        name.span,
                        &[&text],
                    );
                }
                resolver(inf, cx, tgt, m.resolved.clone());
                m.ty
            }
            None => {
                if let TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) = &rt
                    && inf.static_member(*c, name.sym, false).is_none()
                {
                    // An instance method does not declare a setter. In `C.m = v`, the
                    // analyzer looks for the static setter and reports its absence, while
                    // `C.m` as a read is an improper static access to the instance method.
                    let cls = inf.program.class(*c);
                    let instance_setter = inf
                        .setter_key(name.sym)
                        .is_some_and(|the_key| cls.instance_members.contains_key(&the_key));
                    let instance_method = cls
                        .instance_members
                        .get(&name.sym)
                        .is_some_and(|&f| inf.program.function(f).kind == FunctionKind::Function);
                    if !explicit_instance
                        && !instance_setter
                        && instance_method
                        && (!inf.interner.resolve(name.sym).starts_with('_')
                            || cls.library == cx.lib)
                    {
                        let msg = format!(
                            "{}: setter '{}' not defined for the type '{}'",
                            UNDEFINED_SETTER.template,
                            inf.interner.resolve(name.sym),
                            inf.interner.resolve(cls.name),
                        );
                        inf.warning(msg, name.span);
                        return inf.core.dynamic_;
                    }
                    if !explicit_instance && warn_static_access_to_instance(inf, cx, *c, name, true)
                    {
                        return inf.core.dynamic_;
                    }
                }
                if let TypeRef::Extension(x) = rt
                    && inf
                        .explicit_extension_member(x, &[], name.sym, true)
                        .is_some()
                {
                    let msg = format!(
                        "{}: '{}'",
                        STATIC_ACCESS_TO_INSTANCE_MEMBER.template,
                        inf.interner.resolve(name.sym)
                    );
                    inf.warning(msg, name.span);
                    return inf.core.dynamic_;
                }
                let getter = match rt {
                    TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) => {
                        inf.static_member(c, name.sym, false)
                    }
                    TypeRef::Extension(x) => inf.extension_static_member(x, name.sym, false),
                };
                let missing_getter = getter.is_none();
                if let Some(getter) = getter
                    && let Some(f) = getter.function
                    && warn_member_without_setter(inf, name, f)
                {
                    resolver(inf, cx, tgt, getter.resolved);
                    return getter.ty;
                }
                if missing_getter && let TypeRef::Extension(x) = rt {
                    let extension = inf
                        .program
                        .extension(x)
                        .name
                        .map(|n| inf.interner.resolve(n))
                        .unwrap_or("");
                    let msg = format!(
                        "{}: '{}' in '{}'",
                        UNDEFINED_EXTENSION_SETTER.template,
                        inf.interner.resolve(name.sym),
                        extension
                    );
                    inf.warning(msg, name.span);
                }
                // `_resolveTargetInterfaceElement` with neither setter nor getter
                // (`augmented.getGetter`, which also sees instance ones): `AssignmentVerifier`
                // reports `UNDEFINED_SETTER` with the class's `thisType` (3.6.2
                // `property_element_resolver.dart:708-731`, `assignment_verifier.dart:96-106`);
                // the superclass's statics do not count (`C.s = 1`), and neither does
                // `C.new = 1`.
                if missing_getter
                    && !explicit_instance
                    && let TypeRef::Class(c, _) = rt
                {
                    let cls = inf.program.class(c);
                    let of_instance = cls.instance_members.contains_key(&name.sym)
                        || inf
                            .setter_key(name.sym)
                            .is_some_and(|ch| cls.instance_members.contains_key(&ch));
                    if !of_instance && name.span.start != name.span.end {
                        let this = inf.class_this_type(c);
                        let msg = format!(
                            "{}: setter '{}' not defined for the type '{}'",
                            UNDEFINED_SETTER.template,
                            inf.interner.resolve(name.sym),
                            inf.table.format(this, inf.interner, inf.program),
                        );
                        inf.warning(msg, name.span);
                    }
                }
                inf.core.dynamic_
            }
        };
    }
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.this_type.unwrap_or(inf.core.dynamic_);
        record_type(inf, cx, target, this);
        return super_member(inf, cx, tgt, name, SuperUse::Write);
    }
    // In a compound write (`x.p += 1`) the receiver was already read, and the
    // null check came out at the read (a single resolution in the analyzer).
    let compound = recv_read.is_some();
    let (recv, c) = recv_read.unwrap_or_else(|| the_receiver(inf, cx, target, null_aware));
    *short_circuit = c;
    // A `void` receiver (`property_element_resolver.dart:449-455`): at the name.
    if !cx.overlays.contains_key(&target) && matches!(inf.table.get(recv), Type::Void) {
        inf.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT,
            name.span,
            &[],
        );
        return inf.core.dynamic_;
    }
    // `x.new = v` (`type_property_resolver.dart:75-79`): no `new` setter.
    if !cx.overlays.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
        let msg = format!(
            "{}: setter '{}' not defined for the type '{}'",
            UNDEFINED_SETTER.template,
            "new",
            inf.table.format(recv, inf.interner, inf.program),
        );
        inf.warning(msg, name.span);
        return inf.core.dynamic_;
    }
    // `E(the_value).m` forces the named extension: without a setter it emits
    // `undefined_extension_setter`, even if the extension has a getter `m`.
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        if let Some(m) = inf.explicit_extension_member(x, &args, name.sym, true) {
            resolver(inf, cx, tgt, m.resolved);
            return m.ty;
        }
        if inf.extension_static_member(x, name.sym, true).is_some() {
            // `+=` resolves the read and the write of the same name. The analyzer
            // reports the static access once, even when both sides resolve.
            let already_reported =
                inf.diagnostics
                    .iter()
                    .zip(&inf.warning_units)
                    .any(|(d, unit)| {
                        *unit == inf.current_unit
                            && d.span == name.span
                            && d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template
                    });
            if !already_reported {
                inf.warning(
                    EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER
                        .template
                        .to_string(),
                    name.span,
                );
            }
            return inf.core.dynamic_;
        }
        let extension = inf
            .program
            .extension(x)
            .name
            .map(|n| inf.interner.resolve(n))
            .unwrap_or("");
        let msg = format!(
            "{}: '{}' in '{}'",
            UNDEFINED_EXTENSION_SETTER.template,
            inf.interner.resolve(name.sym),
            extension
        );
        inf.warning(msg, name.span);
        return inf.core.dynamic_;
    }
    // A nullable receiver without the getter or the setter on `Object` or in an
    // extension of the nullable type: `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE`
    // (the `_hasGetterOrSetter` of `TypePropertyResolver.resolve`,
    // an362:src/dart/resolver/type_property_resolver.dart:99-180), and then no
    // `undefined_setter`; resolution goes on through the non-nullable type.
    let check_null = !compound
        && inf.requires_null_check(cx.lib, recv, name.sym, false)
        && inf.requires_null_check(cx.lib, recv, name.sym, true);
    if check_null {
        let the_name = inf.interner.resolve(name.sym).to_string();
        let since = inf.diagnostics.len();
        inf.null_warning(
            recv,
            dartforge_diagnostics::codes::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&the_name],
        );
        inf.attach_non_promotion(since, cx, Some(target), name.span);
    }
    // A method of the class's interface takes precedence over a same-named extension setter.
    if let Some(m) = inf.interface_member(recv, name.sym, false)
        && m.method
        && m.function
            .is_some_and(|f| inf.program.function(f).kind == FunctionKind::Function)
    {
        warn_write_to_method(inf, name);
        resolver(inf, cx, tgt, m.resolved);
        // `setWriteElement`: a write to a method has type `InvalidType`.
        return inf.table.invalid(inf.core.dynamic_);
    }
    let search = inf.lookup_member(cx.lib, recv, name.sym, true);
    inf.report_extension_ambiguity(name.span);
    match search {
        Search::Finding(m) => {
            resolver(inf, cx, tgt, m.resolved.clone());
            m.ty
        }
        Search::Missing => {
            // The recovered static setter (`lookupStaticSetter`).
            if let Some(m) = inf.instance_access_to_static(cx.lib, recv, name.sym, true, name.span)
            {
                resolver(inf, cx, tgt, m.resolved);
                return m.ty;
            }
            if check_null {
                return inf.core.dynamic_;
            }
            // The analyzer recovers the getter when the write does not find a setter
            // (`AssignmentVerifier.verify` with `result.getter`, which may be the
            // recovered static one). A getter declared in a class has its own
            // diagnostic; without a getter, it is still an undefined setter.
            let getter = match inf.lookup_member(cx.lib, recv, name.sym, false) {
                Search::Finding(getter) => Some(getter),
                Search::Missing => {
                    match inf.receiver_static_recovery(cx.lib, recv, name.sym, false) {
                        Some((owner, true)) => {
                            let m = inf.static_member(owner, name.sym, false);
                            warn_write_to_method(inf, name);
                            if let Some(m) = m {
                                resolver(inf, cx, tgt, m.resolved);
                                return inf.table.invalid(inf.core.dynamic_);
                            }
                            return inf.core.dynamic_;
                        }
                        Some((owner, false)) => inf.static_member(owner, name.sym, false),
                        None => None,
                    }
                }
                _ => None,
            };
            if let Some(getter) = getter {
                if let Some(f) = getter.function {
                    if warn_member_without_setter(inf, name, f) {
                        resolver(inf, cx, tgt, getter.resolved);
                        return getter.ty;
                    }
                } else if let Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                } = getter.resolved
                {
                    // An enum constant (`static const`): `ASSIGNMENT_TO_CONST`.
                    if inf.program.variable(v).const_ {
                        inf.warning(ASSIGNMENT_TO_CONST.template.to_string(), name.span);
                        resolver(inf, cx, tgt, getter.resolved);
                        return getter.ty;
                    }
                }
            }
            let msg = format!(
                "{}: setter '{}' not defined for the type '{}'",
                UNDEFINED_SETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.warning(msg, name.span);
            inf.core.dynamic_
        }
        _ => {
            resolver(inf, cx, tgt, Resolved::Dynamic);
            inf.core.dynamic_
        }
    }
}

/// `r[i] = …`: the value's type in `[]=`.
fn index_write(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    tgt: ExprId,
    recv: TypeId,
    index: ExprId,
    span: dartforge_diagnostics::Span,
) -> TypeId {
    let target = match &ast(inf, cx).expr(tgt).kind {
        ExprKind::Index { target, .. } => Some(*target),
        _ => None,
    };
    let search = if let Some(target) = target
        && let Some((x, args)) = cx.overlays.get(&target).cloned()
    {
        match inf
            .sym
            .set_index
            .and_then(|op| inf.explicit_extension_member(x, &args, op, false))
        {
            Some(m) => Search::Finding(m),
            None => {
                infer_free(inf, cx, index);
                let sp = index_span(inf, cx, tgt, target);
                warn_extension_operator(inf, x, "[]=", sp);
                return inf.core.dynamic_;
            }
        }
    } else {
        let Some(op) = inf.sym.set_index else {
            infer_free(inf, cx, index);
            return inf.core.dynamic_;
        };
        // `resolveIndexExpression` (`property_element_resolver.dart:80-115`):
        // `TypePropertyResolver` with the name `[]` (which looks for `[]` and
        // `[]=`); a potentially nullable receiver is
        // `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` `['[]']` at the `[`
        // (the `IndexExpression` is a `MethodReferenceExpression`).
        if let Some(t) = target
            && let Some(the_read) = inf.interner.lookup("[]")
            && inf.requires_null_check(cx.lib, recv, the_read, false)
            && inf.requires_null_check(cx.lib, recv, op, false)
        {
            let start = index_span(inf, cx, tgt, t).start;
            let sp = dartforge_diagnostics::Span {
                start,
                end: start + 1,
            };
            let since = inf.diagnostics.len();
            inf.null_warning(recv, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &["[]"]);
            inf.attach_non_promotion(since, cx, Some(t), sp);
        }
        // `super[i] = v`: the `[]=` of the `super` chain.
        if target.is_some_and(|t| matches!(ast(inf, cx).expr(t).kind, ExprKind::Super)) {
            lookup_super_operator(inf, cx, op)
        } else {
            let found = inf.lookup_member(cx.lib, recv, op, false);
            // Two extensions, one with `[]` and the other with `[]=`, neither
            // more specific: `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` with the name the
            // `TypePropertyResolver` looked up (`[]`), at the target.
            if let Some(t) = target
                && let Some(amb) = inf.extension_ambiguity.as_mut()
            {
                amb.0 = "[]".to_string();
                let sp = inf.span_expr(cx.unit, t);
                inf.report_extension_ambiguity(sp);
            }
            found
        }
    };
    match search {
        Search::Finding(m) => {
            resolver(inf, cx, tgt, m.resolved.clone());
            let (pi, pv) = match inf.table.get(m.ty).clone() {
                Type::Function { positional, .. } => {
                    (positional.first().copied(), positional.get(1).copied())
                }
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let ti = infer(inf, cx, index, pi.unwrap_or(u));
            if let Some(p) = pi {
                verify_assignable_expr(
                    inf,
                    cx,
                    index,
                    ti,
                    p,
                    ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                );
            }
            pv.unwrap_or(inf.core.dynamic_)
        }
        Search::Missing => {
            infer_free(inf, cx, index);
            // `Never?` in `x?[i] = v`: promoted to `Never`, nothing to report.
            let nn = inf.non_null(recv);
            if matches!(inf.table.get(nn), Type::Never) {
                return inf.core.dynamic_;
            }
            // A missing `[]=`: `UNDEFINED_OPERATOR` (or `…_SUPER_OPERATOR`) from the
            // `[` to the `]` (`_reportUnresolvedIndex`,
            // an611:src/dart/resolver/property_element_resolver.dart:109-133, 365-381).
            let typ = inf.table.format(recv, inf.interner, inf.program);
            let (sp, code) = match target {
                Some(t) => {
                    let sup = matches!(ast(inf, cx).expr(t).kind, ExprKind::Super);
                    let c = if sup {
                        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_SUPER_OPERATOR
                    } else {
                        dartforge_diagnostics::codes::compile_time_error::UNDEFINED_OPERATOR
                    };
                    (index_span(inf, cx, tgt, t), c)
                }
                None => (
                    span,
                    dartforge_diagnostics::codes::compile_time_error::UNDEFINED_OPERATOR,
                ),
            };
            // A compound one without `[]` or `[]=`: the oracle only reports the `[]=`
            // (the read's `[]`, at the same range, is dropped).
            let mut i = 0;
            while i < inf.diagnostics.len() {
                let d = &inf.diagnostics[i];
                if d.span == sp
                    && d.code == Some(code)
                    && d.args.first().is_some_and(|a| &**a == "[]")
                {
                    inf.diagnostics.remove(i);
                    inf.warning_units.remove(i);
                } else {
                    i += 1;
                }
            }
            inf.warning_with_code(code, sp, &["[]=", &typ]);
            inf.core.dynamic_
        }
        _ => {
            infer_free(inf, cx, index);
            inf.core.dynamic_
        }
    }
}

// -------------------------------------------------------------------
// Conditions: true/false flow models
// -------------------------------------------------------------------

/// Infers a condition (context `bool`) and returns `(true(E), false(E))`.
/// Leaves `cx.flow` in the state from before (the caller chooses the
/// branch).
pub(crate) fn the_condition(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> (Flow, Flow) {
    let a = ast(inf, cx);
    let kind = &a.expr(e).kind;
    let span = a.expr(e).span;
    match kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            let r = the_condition(inf, cx, i);
            let t = inf.body_types.units[cx.unit.0 as usize]
                .get_type(i)
                .unwrap_or(inf.core.bool_);
            record_type(inf, cx, e, t);
            r
        }
        ExprKind::Bool(b) => {
            let b = *b;
            let t = inf.core.bool_;
            record_type(inf, cx, e, t);
            let f = cx.flow.clone();
            if b {
                (f.clone(), f.unreachable())
            } else {
                (f.unreachable(), f)
            }
        }
        ExprKind::Unary {
            op: UnaryOp::Not,
            operand,
        } => {
            let o = *operand;
            let (v, f) = the_condition(inf, cx, o);
            verify_bool(inf, cx, o, BoolUse::Negation);
            let t = inf.core.bool_;
            record_type(inf, cx, e, t);
            (f, v)
        }
        ExprKind::Binary { op, left, right }
            if matches!(
                op,
                BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq
            ) =>
        {
            let (op, left, right) = (*op, *left, *right);
            let r = binary_condition(inf, cx, e, op, left, right);
            let t = inf.core.bool_;
            record_type(inf, cx, e, t);
            r
        }
        // `identical(a, b)` of `dart:core` is an equality for flow analysis
        // (`InvocationInferrer._recordIdenticalInfo`, `_isIdentical`):
        // `!identical(x, null)` promotes `x`.
        ExprKind::Call { target, arguments }
            if arguments.args.len() == 2
                && arguments.args.iter().all(|x| x.name.is_none())
                && match &a.expr(*target).kind {
                    ExprKind::Identifier(n) => inf.interner.resolve(n.sym) == "identical",
                    // `core.identical(…)` through an import prefix.
                    ExprKind::Property {
                        target: p,
                        name,
                        null_aware: false,
                    } => {
                        matches!(a.expr(*p).kind, ExprKind::Identifier(_))
                            && inf.interner.resolve(name.sym) == "identical"
                    }
                    _ => false,
                } =>
        {
            let (target, left, right) = (*target, arguments.args[0].value, arguments.args[1].value);
            let b = inf.core.bool_;
            infer(inf, cx, e, b);
            let core_identical =
                super::calls::resolved_function_of(inf, cx, target).is_some_and(|f| {
                    let f = inf.program.function(f);
                    f.class.is_none()
                        && f.extension.is_none()
                        && inf.program.library(f.library).uri == "dart:core"
                });
            if !core_identical {
                let f = cx.flow.clone();
                return (f.clone(), f);
            }
            let types = &inf.body_types.units[cx.unit.0 as usize];
            let u = inf.core.dynamic_;
            let (tl, tr) = (
                types.get_type(left).unwrap_or(u),
                types.get_type(right).unwrap_or(u),
            );
            equality_flows(inf, cx, left, right, tl, tr)
        }
        ExprKind::Is { value, ty, negated } => {
            let (value, ty, negated) = (*value, *ty, *negated);
            let r = type_test(inf, cx, value, ty, negated, span);
            let t = inf.core.bool_;
            record_type(inf, cx, e, t);
            r
        }
        _ => {
            let b = inf.core.bool_;
            infer(inf, cx, e, b);
            let f = cx.flow.clone();
            // Read of a condition variable that was not rewritten (§7.10).
            if let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind
                && let Some(MemberName::Local(id)) = cx.lookup(n.sym)
                && let Some((yes, not_, version)) = cx.conditions.get(&id).cloned()
                && f.version(id) == Some(version)
                && !f.model(id).is_some_and(|m| m.captured)
            {
                let v = inf.reapply(&f, &yes);
                let fa = inf.reapply(&f, &not_);
                return (v, fa);
            }
            (f.clone(), f)
        }
    }
}

/// An expression whose condition information is not trivial (`==`, `!=`,
/// `&&`, `||`, `!`, `is`), for storing in a condition variable.
pub(crate) fn is_condition_form(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    match &ast(inf, cx).expr(e).kind {
        ExprKind::Parenthesized(i) => is_condition_form(inf, cx, *i),
        ExprKind::Unary {
            op: UnaryOp::Not, ..
        }
        | ExprKind::Is { .. } => true,
        ExprKind::Binary { op, .. } => matches!(
            op,
            BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq
        ),
        // `booleanLiteral`: the opposite branch is unreachable (non-trivial
        // information, stored in the condition variable: `bool c = true; c ? a
        // : b` has a dead `b`).
        ExprKind::Bool(_) => true,
        _ => false,
    }
}

/// Condition of `if`/`while`/`?:`/`for`/`when`, with the non-`bool`
/// diagnostic.
pub(crate) fn checked_condition(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
) -> (Flow, Flow) {
    let r = the_condition(inf, cx, e);
    verify_bool(inf, cx, e, BoolUse::Condition);
    r
}

/// Condition of an `assert` (statement or initializer).
pub(crate) fn assert_condition(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
) -> (Flow, Flow) {
    let r = the_condition(inf, cx, e);
    verify_bool(inf, cx, e, BoolUse::Assert);
    r
}

/// Where an expression needs to be a `bool` (the analyzer's
/// `BoolExpressionVerifier` chooses the code from that).
#[derive(Clone, Copy)]
enum BoolUse {
    /// `if`, `while`, `do`, `for`, `?:`, `when`: `NON_BOOL_CONDITION`.
    Condition,
    /// `assert`: `NON_BOOL_EXPRESSION`.
    Assert,
    /// Operand of `&&`/`||`: `NON_BOOL_OPERAND`.
    Operand(&'static str),
    /// Operand of `!`: `NON_BOOL_NEGATION_EXPRESSION`.
    Negation,
}

/// `checkForNonBoolExpression`: a type not assignable to `bool`. With a
/// nullable `bool` itself (`bool?`), the error is the use of a nullable value
/// as a condition, in any of the uses. With `void`, the analyzer reports the
/// use of the `void` result instead (not here); nothing comes out.
fn verify_bool(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, use_: BoolUse) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let t = inf.body_types.units[cx.unit.0 as usize]
        .get_type(e)
        .unwrap_or(inf.core.bool_);
    let b = inf.core.bool_;
    if void_use(inf, cx, e, t) || inf.assignable(t, b) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let is_bool = match (inf.table.get(t), inf.table.get(b)) {
        (Type::Interface { class: c1, .. }, Type::Interface { class: c2, .. }) => c1 == c2,
        _ => false,
    };
    if is_bool {
        let since = inf.diagnostics.len();
        inf.warning_with_code(c::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION, sp, &[]);
        inf.attach_non_promotion(since, cx, Some(e), sp);
        return;
    }
    match use_ {
        BoolUse::Condition => inf.warning(NON_BOOL_CONDITION.template.to_string(), sp),
        BoolUse::Negation => inf.warning(NON_BOOL_NEGATION_EXPRESSION.template.to_string(), sp),
        BoolUse::Assert => inf.warning_with_code(c::NON_BOOL_EXPRESSION, sp, &[]),
        BoolUse::Operand(op) => inf.warning_with_code(c::NON_BOOL_OPERAND, sp, &[op]),
    }
}

/// `UNNECESSARY_NULL_COMPARISON` (`_checkForInvariantNullComparison`,
/// `an611:src/error/best_practices_verifier.dart:1060-1092`): a literal `null`
/// on one side and a strictly non-nullable other side, from the `null` to the
/// operator or from the operator to the `null`; and, in the resolver
/// (`binary_expression_resolver.dart:128-160`), a definitely unassigned local
/// compared with `null` (`ALWAYS_NULL`).
/// `NullableDereferenceVerifier.expression` (`an611:src/error/nullable_dereference_verifier.dart:33-90`):
/// on the whole expression, when the type is not `dynamic`, invalid or
/// non-nullable; with type `Null`, `INVALID_USE_OF_NULL_VALUE`.
pub(crate) fn nullable_dereference(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    e: ExprId,
    t: TypeId,
    code: dartforge_diagnostics::Code,
) {
    if inf.is_dynamic(t) || inf.table.is_invalid(t) || inf.is_non_nullable(t) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let since = inf.diagnostics.len();
    inf.null_warning(t, code, sp, &[]);
    // `_check`: `computeWhyNotPromotedMessages(errorNode, whyNotPromoted(errorNode))`.
    inf.attach_non_promotion(since, cx, Some(e), sp);
}

/// `isDoubleNan` (`an611:src/error/best_practices_verifier.dart:2093-2100`):
/// the `PrefixedIdentifier` `double.nan`, only by its written form.
pub(crate) fn is_double_nan(inf: &BodyInferrer<'_>, cx: &Body, x: ExprId) -> bool {
    let a = ast(inf, cx);
    match &a.expr(x).kind {
        ExprKind::Property {
            target,
            name,
            null_aware: false,
        } => {
            matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if inf.interner.resolve(n.sym) == "double")
                && inf.interner.resolve(name.sym) == "nan"
        }
        _ => false,
    }
}

/// `UNNECESSARY_NAN_COMPARISON` (`_checkForInvariantNanComparison`,
/// `an611:src/error/best_practices_verifier.dart:1031-1058`): `double.nan` on
/// one side of `==`/`!=`, from the `double.nan` to the operator or from the
/// operator to the end.
fn comparison_with_nan(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
) {
    use dartforge_diagnostics::codes::warning as w;
    let code = match op {
        BinaryOp::NotEq => w::UNNECESSARY_NAN_COMPARISON_TRUE,
        BinaryOp::Eq => w::UNNECESSARY_NAN_COMPARISON_FALSE,
        _ => return,
    };
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operator = operator_token(inf, cx, sl.end);
    if is_double_nan(inf, cx, left) {
        inf.warning_with_code(
            code,
            dartforge_diagnostics::Span {
                start: sl.start,
                end: operator.end,
            },
            &[],
        );
    } else if is_double_nan(inf, cx, right) {
        inf.warning_with_code(
            code,
            dartforge_diagnostics::Span {
                start: operator.start,
                end: sr.end,
            },
            &[],
        );
    }
}

/// `dart:core`'s `Null` (`isDartCoreNull`): not `Never?`.
pub(crate) fn is_core_null(inf: &BodyInferrer<'_>, t: TypeId) -> bool {
    matches!(inf.table.get(t), Type::Null)
        && !matches!(
            inf.table.decoration(t),
            Some(crate::table::DisplayDecoration::NeverNullable)
        )
}

/// `NullSafeApiVerifier` (`an611:src/error/null_safe_api_verifier.dart:29-80`):
/// `Future<T>.value(…)` and `Completer<T>.complete(…)` with a non-nullable `T`,
/// at most one argument, and the argument missing (on the whole call) or of
/// type `Null` (at the argument): `NULL_ARGUMENT_TO_NON_NULL_TYPE`.
fn null_free_api(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, result: TypeId) {
    let a = ast(inf, cx);
    let argument_of = |inf: &BodyInferrer<'_>, t: TypeId, cls: Option<ClassId>| -> Option<TypeId> {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } if Some(*class) == cls && args.len() == 1 => {
                Some(args[0])
            }
            _ => None,
        }
    };
    let completer = inf.core.async_library.and_then(|l| {
        let s = inf.interner.lookup("Completer")?;
        match inf.program.library(l).declared.get(&s)?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    });
    let future = inf.core.future_class;
    let (the_name, typ, args): (&str, TypeId, &ast::Arguments) = match &a.expr(e).kind {
        ExprKind::InstanceCreation {
            constructor: Some(n),
            arguments,
            ..
        } if inf.interner.resolve(n.sym) == "value" => {
            let Some(x) = argument_of(inf, result, future) else {
                return;
            };
            ("Future.value", x, arguments)
        }
        ExprKind::Call { target, arguments } => {
            let ExprKind::Property {
                target: receiver,
                name,
                ..
            } = &a.expr(*target).kind
            else {
                return;
            };
            let bt = &inf.body_types.units[cx.unit.0 as usize];
            match bt.get_resolved(*target) {
                Some(Resolved::Constructor(f)) => {
                    if inf.interner.resolve(name.sym) != "value"
                        || inf.program.function(*f).class != future
                    {
                        return;
                    }
                    let Some(x) = argument_of(inf, result, future) else {
                        return;
                    };
                    ("Future.value", x, arguments)
                }
                _ if inf.interner.resolve(name.sym) == "complete" => {
                    let Some(rt) = bt.get_type(*receiver) else {
                        return;
                    };
                    let Some(x) = argument_of(inf, rt, completer) else {
                        return;
                    };
                    ("Completer.complete", x, arguments)
                }
                _ => return,
            }
        }
        _ => return,
    };
    if args.args.len() > 1 || !inf.is_non_nullable(typ) {
        return;
    }
    let argument = args.args.first().map(|x| x.value);
    let argument_type = argument.map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x));
    let null = match argument_type {
        None => true,
        Some(None) => return,
        Some(Some(t)) => matches!(inf.table.get(t), Type::Null),
    };
    if null {
        let mut sp = inf.span_expr(cx.unit, argument.unwrap_or(e));
        // A cascade section (`c..complete()`): the `MethodInvocation` without a
        // target starts at the `..`/`?..` operator.
        if argument.is_none()
            && let ExprKind::Call { target, .. } = &a.expr(e).kind
            && let ExprKind::Property {
                target: receiver, ..
            } = &a.expr(*target).kind
            && matches!(a.expr(*receiver).kind, ExprKind::CascadeTarget)
        {
            let source = inf.program.unit(cx.unit).source.as_bytes();
            let mut i = sp.start;
            while i > 0 && source[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            if i >= 2 && &source[i - 2..i] == b".." {
                i -= 2;
                if i >= 1 && source[i - 1] == b'?' {
                    i -= 1;
                }
                sp.start = i;
            }
        }
        let displayed = inf
            .table
            .format_without_alias(typ, inf.interner, inf.program);
        inf.warning_with_code(
            dartforge_diagnostics::codes::warning::NULL_ARGUMENT_TO_NON_NULL_TYPE,
            sp,
            &[the_name, &displayed],
        );
    }
}

fn comparison_with_null(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    tl: TypeId,
    tr: TypeId,
) {
    use dartforge_diagnostics::codes::warning as w;
    let a = ast(inf, cx);
    let null = |x: ExprId| matches!(a.expr(x).kind, ExprKind::Null);
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operator = operator_token(inf, cx, sl.end);
    // `===`/`!==` (`UNSUPPORTED_OPERATOR`): the node is not an equality
    // comparison for `_checkForUnnecessaryNullComparison`.
    if inf
        .program
        .unit(cx.unit)
        .source
        .get(operator.start..)
        .is_some_and(|s| s.starts_with("===") || s.starts_with("!=="))
    {
        return;
    }
    let different = op == BinaryOp::NotEq;
    let unassigned_local = |inf: &mut BodyInferrer<'_>, x: ExprId| match &ast(inf, cx).expr(x).kind
    {
        ExprKind::Identifier(_) => match inf.body_types.units[cx.unit.0 as usize].get_resolved(x) {
            Some(Resolved::Local(id)) => cx.flow.unassigned(*id),
            _ => false,
        },
        _ => false,
    };
    let always_null = if different {
        w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE
    } else {
        w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE
    };
    if null(right) && unassigned_local(inf, left) {
        inf.warning_with_code(
            always_null,
            dartforge_diagnostics::Span {
                start: sl.start,
                end: operator.end,
            },
            &[],
        );
    } else if null(left) && unassigned_local(inf, right) {
        inf.warning_with_code(
            always_null,
            dartforge_diagnostics::Span {
                start: operator.start,
                end: sr.end,
            },
            &[],
        );
    }
    let never_null = if different {
        w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE
    } else {
        w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE
    };
    if null(left) && strictly_non_nullable(inf, tr) {
        inf.warning_with_code(
            never_null,
            dartforge_diagnostics::Span {
                start: sl.start,
                end: operator.end,
            },
            &[],
        );
    }
    if null(right) && strictly_non_nullable(inf, tl) {
        inf.warning_with_code(
            never_null,
            dartforge_diagnostics::Span {
                start: operator.start,
                end: sr.end,
            },
            &[],
        );
    }
}

fn binary_condition(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
) -> (Flow, Flow) {
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let before = cx.flow.clone();
            let symbol = if op == BinaryOp::And { "&&" } else { "||" };
            let (lv, lf) = the_condition(inf, cx, left);
            verify_bool(inf, cx, left, BoolUse::Operand(symbol));
            cx.flow = if op == BinaryOp::And {
                lv.clone()
            } else {
                lf.clone()
            };
            // The right side's dead code starts at the operator.
            let start = {
                let the_left_end = inf.span_expr(cx.unit, left).end;
                let right_start = inf.span_expr(cx.unit, right).start;
                let snippet = inf
                    .program
                    .unit(cx.unit)
                    .source
                    .get(the_left_end..right_start)
                    .unwrap_or("");
                the_left_end + skip_whitespace_and_comments(snippet)
            };
            let (rv, rf) = flow_operand(inf, cx, right, Some(start), |inf, cx| {
                the_condition(inf, cx, right)
            });
            verify_bool(inf, cx, right, BoolUse::Operand(symbol));
            cx.flow = before;
            if op == BinaryOp::And {
                let false_ = inf.join(&lf, &rf);
                (rv, false_)
            } else {
                let true_ = inf.join(&lv, &rv);
                (true_, rf)
            }
        }
        _ => {
            // `==` / `!=`
            let u = inf.core.unknown;
            let tl = infer(inf, cx, left, u);
            void_use(inf, cx, left, tl);
            // `e == .x`: the shorthand on the right uses the type of `e` (3.10).
            shorthands::record_equality(inf, cx, right, tl);
            dead_in_right_operand(inf, cx, left, right);
            let tr = infer(inf, cx, right, u);
            // The right side is the argument of `operator ==(Object)`.
            void_use(inf, cx, right, tr);
            never_receiver(inf, cx, left, tl);
            comparison_with_null(inf, cx, op, left, right, tl, tr);
            comparison_with_nan(inf, cx, op, left, right);
            if matches!(
                inf.program.unit(cx.unit).ast.expr(left).kind,
                ExprKind::Super
            ) {
                let this = cx.this_type.unwrap_or(inf.core.dynamic_);
                record_type(inf, cx, left, this);
            }
            // `E(x) == y`: the `==` only in the extension (`_resolveUserDefinableElement`
            // with `ExtensionOverride`); missing, `UNDEFINED_EXTENSION_OPERATOR` at
            // the operator, with the name `==` also for `!=`.
            if let Some((x, args)) = cx.overlays.get(&left).cloned()
                && let Some(eq) = inf.sym.equal
                && inf.explicit_extension_member(x, &args, eq, false).is_none()
            {
                let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
                let extension = inf
                    .program
                    .extension(x)
                    .name
                    .map(|n| inf.interner.resolve(n))
                    .unwrap_or("");
                let msg = format!(
                    "{}: '{}' in '{}'",
                    UNDEFINED_EXTENSION_OPERATOR.template, "==", extension
                );
                inf.warning(msg, token);
            }
            // `super == x`: the `==` of the `super` chain, with the parameter made
            // nullable (`_resolveEqual`): `super == 'a'` against
            // `==(covariant num other)` is an `ARGUMENT_TYPE_NOT_ASSIGNABLE` of `num?`.
            if let Some(eq) = inf.sym.equal
                && matches!(
                    inf.program.unit(cx.unit).ast.expr(left).kind,
                    ExprKind::Super
                )
            {
                if let Search::Finding(m) = lookup_super_operator(inf, cx, eq) {
                    let param = match inf.table.get(m.ty) {
                        Type::Function { positional, .. } => positional.first().copied(),
                        _ => None,
                    };
                    if let Some(p) = param
                        && !matches!(inf.table.get(tr), Type::Void)
                    {
                        let p = inf.nullable(p);
                        verify_assignable_expr(
                            inf,
                            cx,
                            right,
                            tr,
                            p,
                            ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                        );
                    }
                    resolver(inf, cx, e, m.resolved);
                }
            } else if let Some(eq) = inf.sym.equal
                && let Search::Finding(m) = inf.lookup_member(cx.lib, tl, eq, false)
            {
                // The right side against the parameter of `operator ==` made nullable
                // (`binary_expression_resolver.dart:117-124`, `promoteParameterToNullable`),
                // looked up on the left type promoted to non-null.
                let tl_nn = inf.non_null(tl);
                let mem = if tl_nn != tl {
                    match inf.lookup_member(cx.lib, tl_nn, eq, false) {
                        Search::Finding(m2) => Some(m2),
                        _ => None,
                    }
                } else {
                    Some(m.clone())
                };
                let param = mem.and_then(|m| match inf.table.get(m.ty) {
                    Type::Function { positional, .. } => positional.first().copied(),
                    _ => None,
                });
                if let Some(p) = param
                    && !matches!(inf.table.get(tr), Type::Void)
                    && !cx.overlays.contains_key(&left)
                    && !matches!(
                        inf.program.unit(cx.unit).ast.expr(left).kind,
                        ExprKind::Super
                    )
                {
                    let p = inf.nullable(p);
                    verify_assignable_expr(
                        inf,
                        cx,
                        right,
                        tr,
                        p,
                        ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                    );
                }
                resolver(inf, cx, e, m.resolved);
            }
            let (the_equal, different) = equality_flows(inf, cx, left, right, tl, tr);
            if op == BinaryOp::Eq {
                (the_equal, different)
            } else {
                (different, the_equal)
            }
        }
    }
}

/// The flows of `left == right` after both operands (`equalityOperation_end`):
/// `(equal, different)`. Comparing a promotable reference with the `null`
/// literal promotes it to non-null in the "different" branch. Also used for
/// `identical(a, b)` (`InvocationInferrer._recordIdenticalInfo`).
fn equality_flows(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    left: ExprId,
    right: ExprId,
    tl: TypeId,
    tr: TypeId,
) -> (Flow, Flow) {
    let after = cx.flow.clone();
    let a = ast(inf, cx);
    let is_null = |x: ExprId| {
        let mut x = x;
        loop {
            match &a.expr(x).kind {
                ExprKind::Parenthesized(i) => x = *i,
                ExprKind::Null => return true,
                _ => return false,
            }
        }
    };
    let tgt = if is_null(right) {
        promotion_target(inf, cx, left)
    } else if is_null(left) {
        promotion_target(inf, cx, right)
    } else {
        None
    };
    let _ = (tl, tr);
    // The target (a field) may have just been created: the current flow has it.
    let after = if tgt.is_some() {
        cx.flow.clone()
    } else {
        after
    };
    let (mut the_equal, mut different) = (after.clone(), after.clone());
    if let Some(id) = tgt {
        let decl = cx.local(id).ty;
        inf.promote_non_null(&mut different, id, decl);
        let _ = &mut the_equal;
    }
    // Sound flow (3.9, `sound-flow-analysis`): comparing an expression of a
    // non-nullable type with `null` has a known result: the "equal" branch
    // is unreachable (`flow-analysis.md`, `equalityOp`:
    // `equivalentToNull(T1)` and `T2` non-nullable, or the reverse). Before
    // 3.9 both branches stay reachable (mixed mode).
    let sound = inf
        .program
        .library(cx.lib)
        .features
        .has(dartforge_frontend::Feature::SoundFlowAnalysis);
    if sound {
        let null = |inf: &BodyInferrer<'_>, t: TypeId| matches!(inf.table.get(t), Type::Null);
        if (null(inf, tr) && inf.is_non_nullable(tl)) || (null(inf, tl) && inf.is_non_nullable(tr))
        {
            the_equal = the_equal.unreachable();
        }
    }
    (the_equal, different)
}

/// `e is T` / `e is! T`: `(true, false)` with promotion.
/// `BestPracticesVerifier`'s `_checkAllTypeChecks`
/// (`an611:src/error/best_practices_verifier.dart:773-823`): `e is dynamic`,
/// `null is Null`, and `e is T` with the static type of `e` a subtype of `T`
/// are always true (`UNNECESSARY_TYPE_CHECK_TRUE`, or `_FALSE` with `is!`);
/// `e is Null` with `e` not a literal is `TYPE_CHECK_IS_NULL`
/// (`_IS_NOT_NULL`).
#[allow(clippy::too_many_arguments)]
fn unnecessary_type_test(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    value: ExprId,
    v: TypeId,
    ty: ast::TypeId,
    t: TypeId,
    the_negated: bool,
    invalid: bool,
    span: dartforge_diagnostics::Span,
) {
    use dartforge_diagnostics::codes::warning as w;
    // An `InvalidType` operand or type: no unnecessary-test warning
    // (`best_practices_verifier.dart:790`, `:801`).
    if invalid || inf.table.is_invalid(v) || inf.table.is_invalid(t) {
        return;
    }
    let written_name = match &ast(inf, cx).ty(ty).kind {
        ast::TypeKind::Named { name, .. } if name.len() == 1 && !ast(inf, cx).ty(ty).nullable => {
            Some(inf.interner.resolve(name[0].sym).to_string())
        }
        _ => None,
    };
    let code = if the_negated {
        w::UNNECESSARY_TYPE_CHECK_FALSE
    } else {
        w::UNNECESSARY_TYPE_CHECK_TRUE
    };
    if inf.is_dynamic(t) {
        // The real `dynamic`: the written one, or the alias that reaches itself
        // (`hasSelfReference` instantiates as `DynamicType`).
        let self_referencing_alias = match &ast(inf, cx).ty(ty).kind {
            ast::TypeKind::Named { name, .. } => {
                let b = match &name[..] {
                    [n] => inf.program.lookup_in_unit(cx.unit, n.sym),
                    [p, n] => inf.program.lookup_prefixed_in_unit(cx.unit, p.sym, n.sym),
                    _ => None,
                };
                matches!(b.and_then(|b| b.getter), Some(Element::Typedef(tid))
                    if crate::self_reference::self_referencing_typedef(inf.program, inf.program.typedef(tid).decl))
            }
            _ => false,
        };
        if written_name.as_deref() == Some("dynamic") || self_referencing_alias {
            inf.warning_with_code(code, span, &[]);
        }
        return;
    }
    // `is Never?` is also `isDartCoreNull` (a written `Never?` becomes `Null`).
    let nullable_never = matches!(&ast(inf, cx).ty(ty).kind, ast::TypeKind::Named { name, .. }
        if ast(inf, cx).ty(ty).nullable && name.len() == 1 && inf.interner.resolve(name[0].sym) == "Never")
        && inf.table.canonical(t) == inf.core.null;
    if (t == inf.core.null && written_name.as_deref() == Some("Null")) || nullable_never {
        if matches!(ast(inf, cx).expr(value).kind, ExprKind::Null) {
            inf.warning_with_code(code, span, &[]);
        } else {
            let c = if the_negated {
                w::TYPE_CHECK_IS_NOT_NULL
            } else {
                w::TYPE_CHECK_IS_NULL
            };
            inf.warning_with_code(c, span, &[]);
        }
        return;
    }
    if inf.sub(v, t) {
        inf.warning_with_code(code, span, &[]);
    }
}

fn type_test(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    value: ExprId,
    ty: ast::TypeId,
    the_negated: bool,
    span: dartforge_diagnostics::Span,
) -> (Flow, Flow) {
    let v = infer_free(inf, cx, value);
    void_use(inf, cx, value, v);
    let warnings_before = inf.diagnostics.len();
    let t = inf.type_in_context(cx, ty, crate::resolve::TypeContext::Is);
    // A type that did not resolve (`InvalidType` in the analyzer): nothing to
    // say.
    let invalid = inf.diagnostics[warnings_before..]
        .iter()
        .any(|d| d.code.is_some());
    unnecessary_type_test(inf, cx, value, v, ty, t, the_negated, invalid, span);
    // `x is Never` does not promote `x` (the "yes" branch keeps the previous
    // type).
    let tgt = promotion_target(inf, cx, value).filter(|_| !matches!(inf.table.get(t), Type::Never));
    let after = cx.flow.clone();
    let (mut yes, mut not_) = (after.clone(), after.clone());
    if let Some(id) = tgt {
        let decl = cx.local(id).ty;
        inf.promote(&mut yes, id, decl, t);
        let factored = factor(inf, v, t);
        // `tryPromoteForTypeCheck` (flow_analysis.dart:2544-2557): the factor
        // `Never` does not promote (it would mark the branch as unreachable, and
        // mixed-mode unsoundness may reach it), nor a factor equal to the
        // previous type; the tested type is recorded anyway.
        let promotes = !matches!(inf.table.get(factored), Type::Never) && factored != v;
        inf.promote_factored(&mut not_, id, promotes.then_some(factored), t);
    }
    // `e is Never` is never true: the "yes" branch is unreachable (without
    // promoting the target).
    if matches!(inf.table.get(t), Type::Never) {
        yes = yes.unreachable();
    }
    if the_negated {
        (not_, yes)
    } else {
        (yes, not_)
    }
}

/// `factor(T, S)` (`flow-analysis.md`).
pub(crate) fn factor(inf: &mut BodyInferrer<'_>, t: TypeId, s: TypeId) -> TypeId {
    if inf.sub(t, s) {
        return inf.core.never;
    }
    let n = inf.core.null;
    match inf.table.get(t).clone() {
        ty if ty.is_declared_nullable() && !matches!(ty, Type::FutureOr { .. }) => {
            let r = inf.non_null(t);
            let f = factor(inf, r, s);
            if inf.sub(n, s) { f } else { inf.nullable(f) }
        }
        Type::FutureOr {
            arg,
            nullable: false,
        } => {
            let fut = inf.future(arg);
            if inf.sub(fut, s) {
                factor(inf, arg, s)
            } else if inf.sub(arg, s) {
                factor(inf, fut, s)
            } else {
                t
            }
        }
        _ => t,
    }
}

/// Declares a local (with a type side table by offset).
pub(crate) fn declare_local(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    local: Local,
    initialized: bool,
) -> LocalId {
    let (offset, typ) = (local.offset, local.ty);
    let id = cx.declare(local);
    if initialized {
        cx.flow.initialize(id);
    }
    inf.body_types.units[cx.unit.0 as usize].set_local_type(offset, typ);
    id
}

impl<'a> BodyInferrer<'a> {
    /// Constant expression (specification, "Constants"), over the resolution
    /// already done: literals, referenced constants, `const` constructors and
    /// collections, operators over constants, `?:`, interpolation, types.
    /// `tgt` (of a call) is the primary constructor of an extension type
    /// declared `const` (`extension type const E._(int v)`: `E._(1)`), which is
    /// not an element and therefore has no `Resolved::Constructor`.
    fn const_primary(&self, cx: &Body, tgt: ExprId) -> bool {
        let a = ast(self, cx);
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let (cls, the_name) = match &a.expr(tgt).kind {
            ExprKind::Identifier(_) => (tgt, self.sym.empty),
            ExprKind::Property { target, name, .. } => (*target, Some(name.sym)),
            _ => return false,
        };
        let Some(Resolved::Element(Element::Class(c))) = bt.get_resolved(cls) else {
            return false;
        };
        let Some(the_name) = the_name else {
            return false;
        };
        if self.constructor_or_primary(*c, the_name) != Some(None) {
            return false;
        }
        let Some(d) = self.program.class(*c).decl else {
            return false;
        };
        matches!(&self.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_)
    }

    pub(crate) fn is_constant(&self, cx: &Body, e: ExprId) -> bool {
        let a = &self.program.unit(cx.unit).ast;
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let var_const = |v: dartforge_elements::model::VariableId| self.program.variable(v).const_;
        let fun_const = |f: dartforge_elements::model::FunctionElementId| {
            let fe = self.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => var_const(v),
                (FunctionKind::Getter | FunctionKind::Setter, _) => false,
                // A tear-off of a top-level or static function is constant.
                _ => fe.static_ || fe.class.is_none(),
            }
        };
        let ref_const = |r: Option<&Resolved>| match r {
            Some(Resolved::Local(id)) => cx.locals.get(id.0 as usize).is_some_and(|l| l.const_),
            Some(Resolved::Element(Element::Variable(v))) => var_const(*v),
            Some(Resolved::Element(Element::Function(f))) => fun_const(*f),
            Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))) => true,
            Some(Resolved::Member {
                member: MemberRef::Variable(v),
                ..
            }) => var_const(*v),
            Some(Resolved::Member {
                member: MemberRef::Function(f),
                ..
            }) => fun_const(*f),
            Some(Resolved::Constructor(_)) => true,
            _ => false,
        };
        match &a.expr(e).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_) => true,
            ExprKind::String(lit) => lit.parts.iter().all(|p| match p {
                ast::StringPart::Interpolation(x) => self.is_constant(cx, *x),
                _ => true,
            }),
            ExprKind::Parenthesized(x) => self.is_constant(cx, *x),
            ExprKind::Identifier(_) => ref_const(bt.get_resolved(e)),
            ExprKind::Property { target, name, .. } => {
                if ref_const(bt.get_resolved(e)) {
                    return true;
                }
                // `s.length` of a constant string.
                self.interner.resolve(name.sym) == "length" && self.is_constant(cx, *target)
            }
            ExprKind::TypeArguments { .. } => true,
            ExprKind::List {
                const_, elements, ..
            }
            | ExprKind::SetOrMap {
                const_, elements, ..
            } => {
                *const_
                    || elements.iter().all(|el| match el {
                        ast::CollectionElement::Expression(x) => self.is_constant(cx, *x),
                        ast::CollectionElement::MapEntry { key, value, .. } => {
                            self.is_constant(cx, *key) && self.is_constant(cx, *value)
                        }
                        _ => false,
                    })
            }
            ExprKind::Record {
                positional, named, ..
            } => {
                positional.iter().all(|x| self.is_constant(cx, *x))
                    && named.iter().all(|(_, x)| self.is_constant(cx, *x))
            }
            ExprKind::InstanceCreation {
                keyword, arguments, ..
            } => {
                matches!(keyword, Some(ast::CreationKeyword::Const))
                    || arguments.args.iter().all(|x| self.is_constant(cx, x.value))
                        && self.const_constructor(bt.get_resolved(e))
            }
            ExprKind::Call { target, arguments } => {
                // A `const` constructor without `new` in a constant context, or `identical`.
                let ok_args = arguments.args.iter().all(|x| self.is_constant(cx, x.value));
                if !ok_args {
                    return false;
                }
                if self.const_constructor(bt.get_resolved(e)) || self.const_primary(cx, *target) {
                    return true;
                }
                matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "identical")
            }
            ExprKind::Unary { op, operand } => {
                matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot)
                    && self.is_constant(cx, *operand)
            }
            ExprKind::Binary { left, right, .. } => {
                self.is_constant(cx, *left) && self.is_constant(cx, *right)
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.is_constant(cx, *condition)
                    && self.is_constant(cx, *then)
                    && self.is_constant(cx, *else_)
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.is_constant(cx, *value),
            _ => false,
        }
    }

    fn const_constructor(&self, r: Option<&Resolved>) -> bool {
        match r {
            Some(Resolved::Constructor(f)) => self.program.function(self.program.public(*f)).const_,
            _ => false,
        }
    }
}
