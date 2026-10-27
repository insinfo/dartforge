//! The impact of an element: what its body uses.
//!
//! It is the analogue of dart2js's `ImpactBuilder` (`ir/impact.dart`,
//! `kernel/kernel_impact.dart`), over our AST and the `BodyTypes`. The walk is
//! **exhaustive**: every `match` over `ExprKind`, `StmtKind`, `PatternKind`
//! and `CollectionElement` lists the variants without a `_` arm, so that a new
//! AST variant breaks compilation here instead of being silently forgotten — a
//! forgotten name would be pruned code that someone still calls.
//!
//! Three sources, combined (never one instead of another):
//!
//! 1. every written member **name** (`x.name`, cascade section, object
//!    pattern field — and a bare `name` only on real dispatch on `this`, never
//!    a local, parameter, top-level or static) becomes a selector, whatever
//!    the resolution — that is what makes the world independent of
//!    inference —, with the **receiver cone** when its static type is known
//!    (`x: T` keeps the member only in the subtype classes of `T`; a bare
//!    `name` uses the enclosing class; a dynamic/unknown receiver stays
//!    unrestricted);
//! 2. the `Resolved` of the `BodyTypes`, for static uses;
//! 3. name resolution in scope (`Program::lookup`), for when `Resolved` is
//!    missing or diverges from what the emitter would resolve.
//!
//! Types: the written annotations and the **static type of every traversed
//! expression** (covers the inferred type arguments that the emitter writes in
//! the rti recipes).

use crate::Engine;
use crate::params::{Callee, Value};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef,
    LibraryId, UnitId, VariableId,
};
use dartforge_frontend::ast::{
    self, AssignOp, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody,
    Initializer, PatternId, PatternKind, StmtId, StmtKind, StringPart, TypeKind, UnaryOp,
};
use dartforge_types::resolved::{MemberRef, Resolved};

/// Where the body being traversed is.
#[derive(Clone, Copy)]
pub(crate) struct Context {
    pub unit: UnitId,
    pub library: LibraryId,
    pub class: Option<ClassId>,
    pub extension: Option<ExtensionId>,
    /// No `this` (static function/method, top-level, static field): a bare
    /// `name` is never dispatch on an instance, so it restricts no cone.
    pub no_this: bool,
}

/// What a target of `.` denotes statically.
enum StaticTarget {
    Class(ClassId),
    Extension(ExtensionId),
    Prefix(dartforge_intern::SymbolId),
}

pub(crate) fn of_function(m: &mut Engine<'_>, f: FunctionElementId) {
    let e = m.e;
    let p = e.program;
    let func = p.function(f);
    if let Some(ft) = e.outline.functions.get(f.0 as usize) {
        let s = ft.signature;
        m.visit_type(s);
    }
    match m.function_node(f) {
        FunctionRef::Function { unit, function } => {
            let ctx = Context {
                unit,
                library: func.library,
                class: func.class,
                extension: func.extension,
                no_this: func.static_,
            };
            walk_function(m, &ctx, function);
        }
        FunctionRef::Constructor { unit, member } => {
            let ctx = Context {
                unit,
                library: func.library,
                class: func.class,
                extension: None,
                no_this: false,
            };
            let ast = &p.unit(unit).ast;
            let ast::MemberKind::Constructor(c) = &ast.member(member).kind else {
                return;
            };
            let Some(k) = func.class else { return };
            walk_parameters(m, &ctx, &c.parameters);
            let mut explicit = false;
            for ini in c.initializers.iter() {
                match ini {
                    Initializer::Field { value, .. } => expr(m, &ctx, *value),
                    Initializer::Super {
                        constructor,
                        arguments,
                        ..
                    } => {
                        explicit = true;
                        let ctor_name = constructor
                            .map(|n| m.e.interner.resolve(n.sym))
                            .unwrap_or("");
                        if let Some(sc) = m.super_constructor(k, ctor_name) {
                            // Super parameters forward arguments the call
                            // does not list: the target stays unproved.
                            if forwards_super(c) {
                                m.use_constructor_from_initializer(sc);
                            } else {
                                initializer_call(m, &ctx, sc, Some(arguments));
                            }
                        }
                        walk_arguments(m, &ctx, arguments);
                    }
                    Initializer::Redirect {
                        constructor,
                        arguments,
                        ..
                    } => {
                        explicit = true;
                        let ctor_name = constructor
                            .map(|n| m.e.interner.resolve(n.sym))
                            .unwrap_or("");
                        if let Some(rc) = m.constructor(k, ctor_name) {
                            initializer_call(m, &ctx, rc, Some(arguments));
                        }
                        walk_arguments(m, &ctx, arguments);
                    }
                    Initializer::Assert { .. } if no_asserts(m) => {}
                    Initializer::Assert {
                        condition, message, ..
                    } => {
                        expr(m, &ctx, *condition);
                        if let Some(x) = message {
                            expr(m, &ctx, *x);
                        }
                    }
                }
            }
            if let Some(r) = &c.redirect {
                // `factory C() = D.n;` — direct calls already go to the target
                // (`module.rs::emit_factory`), but the body forwards to the
                // tearoff, and the target is instantiated.
                ast_type(m, &ctx, r.ty);
                if let Some((dc, ctor_name)) = class_and_constructor(m, &ctx, r.ty, r.constructor) {
                    m.create(dc, ctor_name, false);
                }
            } else if !c.factory && !explicit {
                // Implicit super: the superclass's unnamed constructor.
                if let Some(sc) = m.super_constructor(k, "") {
                    if forwards_super(c) {
                        m.use_constructor_from_initializer(sc);
                    } else {
                        initializer_call(m, &ctx, sc, None);
                    }
                }
            }
            walk_body(m, &ctx, &c.body);
        }
        FunctionRef::None => {
            // The synthetic `values` of an enum lists every constant.
            if func.static_
                && func.kind == FunctionKind::Getter
                && let Some(k) = func.class.filter(|k| {
                    p.class(*k).kind == ClassKind::Enum && e.interner.resolve(func.name) == "values"
                })
            {
                for &vid in &p.class(k).enum_constants {
                    m.use_variable(vid);
                }
            }
            if func.kind == FunctionKind::SyntheticConstructor
                && let Some(k) = func.class
            {
                // A synthetic mixin application forwards to the
                // superclass constructor of the same name; a regular
                // class, to the unnamed one.
                let ctor_name = if p.class(k).decl.is_none() {
                    m.e.interner.resolve(func.name)
                } else {
                    ""
                };
                if let Some(sc) = m.super_constructor(k, ctor_name) {
                    m.use_constructor_from_initializer(sc);
                }
            }
        }
    }
}

/// The number of arguments of the JavaScript call that starts `rest` (the
/// template text right after a member name): `Some(n)` for `(a, b)`, `None`
/// when `rest` is not a call or the list does not close in this text.
fn template_arity(rest: &str) -> Option<usize> {
    let rest = rest.trim_start();
    let inner = rest.strip_prefix('(')?;
    let mut depth = 0usize;
    let mut commas = 0;
    let mut empty = true;
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                empty = false;
            }
            '(' | '[' | '{' => {
                depth += 1;
                empty = false;
            }
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ')' => return Some(if empty { 0 } else { commas + 1 }),
            ',' if depth == 0 => commas += 1,
            c if !c.is_whitespace() => empty = false,
            _ => {}
        }
    }
    None
}

/// Whether the member read `id` (not a call) may be a method tear-off: its
/// resolution is unknown, dynamic or an instance method. A read resolved to
/// a getter or a field is not — a getter is never overridden by a method.
fn may_tear_off(m: &Engine<'_>, ctx: &Context, id: ExprId) -> bool {
    let resolved =
        m.e.bodies
            .units
            .get(ctx.unit.0 as usize)
            .and_then(|u| u.get_resolved(id));
    match resolved {
        Some(Resolved::Member {
            member: MemberRef::Variable(_),
            ..
        }) => false,
        Some(Resolved::Member {
            member: MemberRef::Function(f),
            ..
        }) => matches!(
            m.e.program.function(*f).kind,
            FunctionKind::Function | FunctionKind::Operator
        ),
        _ => true,
    }
}

/// Whether a constructor has super parameters (`super.x`).
fn forwards_super(c: &ast::Constructor) -> bool {
    c.parameters.iter().any(|p| p.super_)
}

/// `super(..)`, `this.n(..)` or the implicit `super()` (`arguments` is
/// `None`): a call of `f` with these arguments.
fn initializer_call(
    m: &mut Engine<'_>,
    ctx: &Context,
    f: FunctionElementId,
    arguments: Option<&ast::Arguments>,
) {
    let callee = Callee::Direct(f);
    match arguments {
        Some(a) => record_arguments(m, ctx, &callee, a),
        None => m.record_call(&callee, &[], &[]),
    }
    set_permit(m, Some(f));
    m.use_constructor_from_initializer(f);
    set_permit(m, None);
}

fn set_permit(m: &mut Engine<'_>, f: Option<FunctionElementId>) {
    if let Some(pf) = m.params.as_mut() {
        pf.permit = f;
    }
}

/// Records the arguments of a call site of `callee`.
fn record_arguments(m: &mut Engine<'_>, ctx: &Context, callee: &Callee, a: &ast::Arguments) {
    if m.params.is_none() || matches!(callee, Callee::Untracked) {
        return;
    }
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for arg in a.args.iter() {
        let v = m.argument_value(ctx.unit, arg.value);
        match arg.name {
            Some(n) => named.push((n.sym, v)),
            None => positional.push(v),
        }
    }
    m.record_call(callee, &positional, &named);
}

/// The callee of a `Call` target without its type arguments.
fn unwrapped_callee(ast: &ast::Ast, mut t: ExprId) -> ExprId {
    while let ExprKind::TypeArguments { target, .. } = &ast.expr(t).kind {
        t = *target;
    }
    t
}

/// Who the call or creation `id` calls, for the argument tables
/// (`params.rs`). A direct callee found here is only *permitted*: if the
/// engine marks another function, that one is poisoned, so a disagreement
/// costs precision, never soundness.
fn callee_of(m: &Engine<'_>, ctx: &Context, id: ExprId) -> Callee {
    let p = m.e.program;
    let ast = &p.unit(ctx.unit).ast;
    let constructor = |c: ClassId, n: &str| {
        m.constructor(c, n)
            .map_or(Callee::Untracked, Callee::Direct)
    };
    let target = match &ast.expr(id).kind {
        ExprKind::InstanceCreation {
            ty, constructor: k, ..
        } => {
            return match class_and_constructor(m, ctx, *ty, *k) {
                Some((c, n)) => constructor(c, n),
                None => Callee::Untracked,
            };
        }
        ExprKind::Call { target, .. } => unwrapped_callee(ast, *target),
        _ => return Callee::Untracked,
    };
    let resolved =
        m.e.bodies
            .units
            .get(ctx.unit.0 as usize)
            .and_then(|u| u.get_resolved(target));
    match resolved {
        Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Constructor(f)) => {
            return Callee::Direct(*f);
        }
        Some(Resolved::Member {
            member: MemberRef::Function(f),
            ..
        }) if p.function(*f).static_ => return Callee::Direct(*f),
        _ => {}
    }
    match &ast.expr(target).kind {
        ExprKind::Identifier(n) => {
            if let Some(StaticTarget::Class(c)) = static_target(m, ctx, target) {
                return constructor(c, "");
            }
            match resolved {
                None | Some(Resolved::Member { .. }) | Some(Resolved::Dynamic) => {
                    Callee::Name(n.sym)
                }
                Some(_) => Callee::Untracked,
            }
        }
        ExprKind::Property {
            target: receiver,
            name,
            ..
        } => match static_target(m, ctx, *receiver) {
            Some(StaticTarget::Class(c)) => match p.class(c).static_members.get(&name.sym) {
                Some(f) => Callee::Direct(*f),
                None => constructor(c, m.e.interner.resolve(name.sym)),
            },
            Some(StaticTarget::Prefix(pre)) => {
                match p
                    .lookup_prefixed(ctx.library, pre, name.sym)
                    .and_then(|b| b.getter)
                {
                    Some(Element::Function(f)) => Callee::Direct(f),
                    Some(Element::Class(c)) => constructor(c, ""),
                    _ => Callee::Untracked,
                }
            }
            Some(StaticTarget::Extension(_)) => Callee::Untracked,
            None => Callee::Name(name.sym),
        },
        _ => Callee::Untracked,
    }
}

pub(crate) fn of_variable(m: &mut Engine<'_>, v: VariableId) {
    let e = m.e;
    let var = e.program.variable(v);
    if let dartforge_elements::model::VariableRef::EnumConstant { .. } = var.node {
        enum_constant(m, v);
        return;
    }
    if let Some(vt) = e.outline.variables.get(v.0 as usize)
        && let Some(t) = vt.declared_type.or(vt.inferred)
    {
        m.visit_type(t);
    }
    if let Some((unit, Some(init))) = m.variable_node(v) {
        let ctx = Context {
            unit,
            library: var.library,
            class: var.class,
            extension: var.extension,
            no_this: var.static_ || var.class.is_none(),
        };
        expr(m, &ctx, init);
    }
}

/// Initializers of the instance fields (and the constants of an enum) of a
/// class that has just been instantiated — `fieldInit`,
/// `kernel_impact.dart:969`.
pub(crate) fn of_fields(m: &mut Engine<'_>, c: ClassId) {
    let e = m.e;
    let p = e.program;
    let class = p.class(c);
    for &vid in &class.fields {
        let var = p.variable(vid);
        if var.static_ {
            continue;
        }
        if let Some(vt) = e.outline.variables.get(vid.0 as usize)
            && let Some(t) = vt.declared_type.or(vt.inferred)
        {
            m.visit_type(t);
        }
        if let Some((unit, Some(init))) = m.variable_node(vid) {
            let ctx = Context {
                unit,
                library: var.library,
                class: Some(c),
                extension: None,
                no_this: false,
            };
            expr(m, &ctx, init);
        }
    }
    // The constants of an enum live one by one: each one read (or all of
    // them, through `values`) is created by [`enum_constant`], and the
    // emitter leaves out the others (`docs/JS-PRODUCAO-TAMANHO.md` §12).
}

/// An enum constant that has become live: its constructor and arguments.
fn enum_constant(m: &mut Engine<'_>, v: VariableId) {
    let p = m.e.program;
    let var = p.variable(v);
    let dartforge_elements::model::VariableRef::EnumConstant { unit, decl, index } = var.node
    else {
        return;
    };
    let Some(c) = var.class else { return };
    let ast = &p.unit(unit).ast;
    let ast::DeclKind::Enum(ed) = &ast.decl(decl).kind else {
        return;
    };
    let Some(k) = ed.constants.get(index) else {
        return;
    };
    let ctx = Context {
        unit,
        library: p.class(c).library,
        class: Some(c),
        extension: None,
        no_this: false,
    };
    let ctor_name = k
        .constructor
        .map(|n| m.e.interner.resolve(n.sym))
        .unwrap_or("");
    m.create(c, ctor_name, false);
    for t in k.type_args.iter() {
        ast_type(m, &ctx, *t);
    }
    if let Some(a) = &k.arguments {
        walk_arguments(m, &ctx, a);
    }
}

/// Records the `JS_CLASS_REF(C)` arguments of a `JS(type, template, …)`
/// call whose placeholder the template only compares: right after
/// `instanceof`, or next to `===`/`!==`. Such a use tests the class and
/// never constructs or extends it, so `C` does not need to be instantiated
/// (`_isJsObject`'s `# instanceof #` with `RecordImpl` kept the whole record
/// runtime alive).
fn compared_class_refs(m: &mut Engine<'_>, ctx: &Context, arguments: &ast::Arguments) {
    let ast = &m.e.program.unit(ctx.unit).ast;
    let Some(t) = arguments.args.get(1) else {
        return;
    };
    let ExprKind::String(lit) = &ast.expr(t.value).kind else {
        return;
    };
    let Some(template) = lit
        .constant_value()
        .map(|v| v.to_string_lossy().to_string())
    else {
        return;
    };
    let holes: Vec<usize> = template.match_indices('#').map(|(p, _)| p).collect();
    for (k, a) in arguments.args.iter().skip(2).enumerate() {
        let ExprKind::Call { target, .. } = &ast.expr(a.value).kind else {
            continue;
        };
        let is_ref = match &ast.expr(*target).kind {
            ExprKind::Identifier(n) => m.e.interner.resolve(n.sym) == "JS_CLASS_REF",
            _ => false,
        };
        let Some(&p) = holes.get(k) else { continue };
        // A template with `#` used any other way (numbered holes) is not read.
        if !is_ref || holes.len() != arguments.args.len() - 2 {
            continue;
        }
        let before = template[..p].trim_end();
        let after = template[p + 1..].trim_start();
        let compared = before.ends_with("instanceof")
            || before.ends_with("===")
            || before.ends_with("!==")
            || after.starts_with("===")
            || after.starts_with("!==");
        if compared {
            m.compared_class_refs.insert((ctx.unit, a.value));
        }
    }
}

/// The enum constant argument that the emitter folds in an intrinsic of the
/// SDK (`dartforge_emit_js::own_sdk`, `Intrinsic::GetName` and
/// `Intrinsic::Builtin`): its index, when the member is one the emitter
/// knows. Reading it would instantiate `JsGetName`/`JsBuiltin` with all their
/// constants for nothing.
fn folded_enum_argument(
    m: &Engine<'_>,
    ctx: &Context,
    target: ExprId,
    arguments: &ast::Arguments,
) -> Option<usize> {
    if !m.include_sdk || !m.e.program.library(ctx.library).is_sdk {
        return None;
    }
    let ast = &m.e.program.unit(ctx.unit).ast;
    let ExprKind::Identifier(n) = &ast.expr(target).kind else {
        return None;
    };
    let (index, known): (usize, &[&str]) = match m.e.interner.resolve(n.sym) {
        "JS_GET_NAME" => (
            0,
            &[
                "OPERATOR_IS_PREFIX",
                "SIGNATURE_NAME",
                "RTI_NAME",
                "FUTURE_CLASS_TYPE_NAME",
                "LIST_CLASS_TYPE_NAME",
                "RTI_FIELD_AS",
                "RTI_FIELD_IS",
            ],
        ),
        "JS_BUILTIN" => (1, &["dartClosureConstructor", "dartObjectConstructor"]),
        _ => return None,
    };
    let arg = arguments.args.get(index)?;
    let member = match &ast.expr(arg.value).kind {
        ExprKind::Property { name, .. } => name.sym,
        ExprKind::Identifier(i) => i.sym,
        _ => return None,
    };
    known
        .contains(&m.e.interner.resolve(member))
        .then_some(index)
}

// ---------------------------------------------------------------- functions

fn walk_function(m: &mut Engine<'_>, ctx: &Context, id: ast::FunctionId) {
    let e = m.e;
    let f = e.program.unit(ctx.unit).ast.function(id);
    for tp in f.type_params.iter() {
        if let Some(b) = tp.bound {
            ast_type(m, ctx, b);
        }
    }
    if let Some(r) = f.return_type {
        ast_type(m, ctx, r);
    }
    if let Some(ps) = &f.parameters {
        walk_parameters(m, ctx, ps);
    }
    walk_body(m, ctx, &f.body);
}

fn walk_body(m: &mut Engine<'_>, ctx: &Context, b: &FunctionBody) {
    match b {
        FunctionBody::Block(s) => stmt(m, ctx, *s),
        FunctionBody::Expression(x) => expr(m, ctx, *x),
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    }
}

fn walk_parameters(m: &mut Engine<'_>, ctx: &Context, ps: &[ast::Parameter]) {
    for p in ps {
        if let Some(t) = p.ty {
            ast_type(m, ctx, t);
        }
        if let Some(d) = p.default_value {
            expr(m, ctx, d);
        }
        for tp in p.function_type_params.iter() {
            if let Some(b) = tp.bound {
                ast_type(m, ctx, b);
            }
        }
        if let Some(fp) = &p.function_parameters {
            walk_parameters(m, ctx, fp);
        }
    }
}

fn walk_arguments(m: &mut Engine<'_>, ctx: &Context, a: &ast::Arguments) {
    for t in a.type_args.iter() {
        ast_type(m, ctx, *t);
    }
    for arg in a.args.iter() {
        expr(m, ctx, arg.value);
    }
}

// ---------------------------------------------------------------- written types

/// Class named by a type annotation (`C`, `p.C`, a typedef of `C`).
fn class_of_ast_type(m: &Engine<'_>, ctx: &Context, t: ast::TypeId) -> Option<ClassId> {
    let e = m.e;
    let p = e.program;
    let TypeKind::Named { name, .. } = &p.unit(ctx.unit).ast.ty(t).kind else {
        return None;
    };
    let b = match name.len() {
        1 => p.lookup(ctx.library, name[0].sym)?,
        2 => p.lookup_prefixed(ctx.library, name[0].sym, name[1].sym)?,
        _ => return None,
    };
    match b.getter? {
        Element::Class(c) => Some(c),
        Element::Typedef(td) => {
            let tt = e.outline.typedefs.get(td.0 as usize)?.target_type;
            match e.table.get(tt) {
                dartforge_types::table::Type::Interface { class, .. } => Some(*class),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `C`, `p.C` or `C.name` (the parser does not tell an import prefix from a
/// named constructor in `new C.name()` / `factory X() = C.name`): the class
/// and the constructor name.
fn class_and_constructor<'n>(
    m: &Engine<'n>,
    ctx: &Context,
    t: ast::TypeId,
    constructor: Option<ast::Name>,
) -> Option<(ClassId, &'n str)> {
    let e = m.e;
    let p = e.program;
    let ctor_name = constructor.map(|n| e.interner.resolve(n.sym)).unwrap_or("");
    if let TypeKind::Named { name, .. } = &p.unit(ctx.unit).ast.ty(t).kind
        && name.len() == 2
        && let Some(Element::Class(c)) = p.lookup(ctx.library, name[0].sym).and_then(|b| b.getter)
    {
        return Some((c, e.interner.resolve(name[1].sym)));
    }
    class_of_ast_type(m, ctx, t).map(|c| (c, ctor_name))
}

fn ast_type(m: &mut Engine<'_>, ctx: &Context, t: ast::TypeId) {
    let e = m.e;
    let p = e.program;
    let ta = p.unit(ctx.unit).ast.ty(t);
    match &ta.kind {
        TypeKind::Named { name, args } => {
            let b = match name.len() {
                1 => p.lookup(ctx.library, name[0].sym),
                2 => p.lookup_prefixed(ctx.library, name[0].sym, name[1].sym),
                _ => None,
            };
            if let Some(el) = b.and_then(|b| b.getter)
                && matches!(el, Element::Class(_) | Element::Typedef(_))
            {
                m.use_element(el);
            }
            for a in args.iter() {
                ast_type(m, ctx, *a);
            }
        }
        TypeKind::Void => {}
        TypeKind::Function {
            return_type,
            type_params,
            parameters,
        } => {
            if let Some(r) = return_type {
                ast_type(m, ctx, *r);
            }
            for tp in type_params.iter() {
                if let Some(b) = tp.bound {
                    ast_type(m, ctx, b);
                }
            }
            walk_parameters(m, ctx, parameters);
        }
        TypeKind::Record { positional, named } => {
            for x in positional.iter() {
                ast_type(m, ctx, *x);
            }
            for (_, x) in named.iter() {
                ast_type(m, ctx, *x);
            }
        }
    }
}

// ---------------------------------------------------------------- statements

fn walk_variables(m: &mut Engine<'_>, ctx: &Context, l: &ast::VariableList) {
    if let Some(t) = l.ty {
        ast_type(m, ctx, t);
    }
    for v in l.variables.iter() {
        if let Some(i) = v.initializer {
            expr(m, ctx, i);
        }
    }
}

fn for_in_target(m: &mut Engine<'_>, ctx: &Context, t: &ForInTarget) {
    match t {
        ForInTarget::Declared { ty, .. } => {
            if let Some(t) = ty {
                ast_type(m, ctx, *t);
            }
        }
        ForInTarget::Pattern { pattern, .. } => walk_pattern(m, ctx, *pattern),
        ForInTarget::Expression(x) => {
            // `for (target in ...)` writes to the target (and reads it, if compound).
            write_to_target(m, ctx, *x);
            expr(m, ctx, *x);
        }
    }
    // `for-in` calls the iteration protocol (`kernel_impact.dart:906-918`),
    // with calls the AST does not have.
    for s in ["iterator", "moveNext", "current"] {
        m.poison_name_str(s);
        m.new_selector(s);
    }
}

/// The class of the `receiver`'s static type, when known and nominal. `None`
/// is a dynamic or unknown receiver: the selector stays unrestricted (nothing
/// is ever pruned for lack of information).
fn receiver_class(m: &Engine<'_>, ctx: &Context, receiver: ExprId) -> Option<ClassId> {
    let u = m.e.bodies.units.get(ctx.unit.0 as usize)?;
    m.class_of_type(u.get_type(receiver)?)
}

/// The receiver cone of a member use (`x.name`, bare `name`):
///
/// * `x.name` with a static target (`C.name`, `p.name`) is not dispatch on an
///   instance — unrestricted, as before;
/// * `x.name` with a known `x: T` restricts to the cone of `T`;
/// * a bare `name` is an implicit `this.name`: the cone is the enclosing class
///   (top-level and static methods have no `this`, so `ctx.class` is `None`
///   and the selector stays unrestricted).
fn use_receiver(m: &Engine<'_>, ctx: &Context, id: ExprId) -> Option<ClassId> {
    let ast = &m.e.program.unit(ctx.unit).ast;
    match &ast.expr(id).kind {
        ExprKind::Property { target, .. } => {
            if static_target(m, ctx, *target).is_some() {
                return None;
            }
            // `super.s()` runs the superclass's implementation, but the static
            // type of the receiver `super` is the current class (`tipo_this`):
            // restricting by the type could prune the superclass at run time.
            // The cone is the class where the member resolved (static
            // dispatch).
            if matches!(&ast.expr(*target).kind, ExprKind::Super) {
                return m
                    .e
                    .bodies
                    .units
                    .get(ctx.unit.0 as usize)
                    .and_then(|u| u.get_resolved(id))
                    .and_then(|r| match r {
                        Resolved::Member { class, .. } => Some(*class),
                        _ => None,
                    });
            }
            receiver_class(m, ctx, *target)
        }
        ExprKind::Identifier(_) => {
            if ctx.no_this {
                None
            } else {
                ctx.class
            }
        }
        _ => None,
    }
}

/// What a bare `name` records as an instance selector: not every identifier
/// is dispatch on `this` — locals, parameters, types, top-level and statics
/// are not, and recording them (even restricted) would poison
/// `has_restriction` and block the verifier's fix-up in the classes outside the
/// cone. The resolution decides; without a resolution or with `dynamic`, the
/// earlier conservative choice (restricted to the enclosing class, or
/// unrestricted when dynamic).
enum NameTarget {
    Skip,
    Register(Option<ClassId>),
}

fn is_instance_member(m: &Engine<'_>, member: &MemberRef) -> bool {
    match member {
        MemberRef::Function(f) => {
            let f = m.e.program.function(*f);
            !f.static_ && (f.class.is_some() || f.extension.is_some())
        }
        MemberRef::Variable(v) => {
            let v = m.e.program.variable(*v);
            !v.static_ && (v.class.is_some() || v.extension.is_some())
        }
    }
}

fn name_target(m: &Engine<'_>, ctx: &Context, id: ExprId) -> NameTarget {
    let res =
        m.e.bodies
            .units
            .get(ctx.unit.0 as usize)
            .and_then(|u| u.get_resolved(id));
    match res {
        None => NameTarget::Register(if ctx.no_this { None } else { ctx.class }),
        Some(Resolved::Dynamic) => NameTarget::Register(None),
        Some(
            Resolved::Local(_)
            | Resolved::Parameter { .. }
            | Resolved::TypeParameter(_)
            | Resolved::Prefix(_)
            | Resolved::Constructor(_)
            | Resolved::Element(_)
            | Resolved::ExtensionMember { .. },
        ) => NameTarget::Skip,
        Some(Resolved::Member { member, .. }) => {
            if is_instance_member(m, member) {
                NameTarget::Register(if ctx.no_this { None } else { ctx.class })
            } else {
                NameTarget::Skip
            }
        }
    }
}

/// A write use outside a simple `Assign` (`+=`, `++`, a `for-in` target):
/// records the write kind (`name_=`, the `instance_members` key) without
/// suppressing the read, which the normal walk records next. The cone is the
/// target's, by the same rule as for the use.
fn write_to_target(m: &mut Engine<'_>, ctx: &Context, lhs: ExprId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unit).ast;
    // An extension accessor (`c.x += v` with `x` of an extension): the target
    // resolves to one of the pair, and both run — the getter and the setter
    // of that name, which are functions, not selectors.
    if let Some(Resolved::ExtensionMember { extension, member }) = e
        .bodies
        .units
        .get(ctx.unit.0 as usize)
        .and_then(|u| u.get_resolved(lhs))
    {
        let base = |f: FunctionElementId| {
            let name = e.interner.resolve(e.program.function(f).name);
            name.trim_end_matches('=').trim_end_matches('_').to_string()
        };
        let name = base(*member);
        let pair: Vec<FunctionElementId> = e
            .program
            .extension(*extension)
            .instance_members
            .values()
            .chain(e.program.extension(*extension).static_members.values())
            .copied()
            .filter(|&f| base(f) == name)
            .collect();
        for f in pair {
            m.use_function(f);
        }
    }
    match &ast.expr(lhs).kind {
        ExprKind::Property { name, .. } => {
            let text = e.interner.resolve(name.sym);
            let receiver = use_receiver(m, ctx, lhs);
            m.new_selector_with_receiver(&format!("{text}_="), receiver);
        }
        ExprKind::Identifier(n) => {
            let sym = n.sym;
            let text = e.interner.resolve(sym).to_string();
            match name_target(m, ctx, lhs) {
                NameTarget::Register(r) => m.new_selector_with_receiver(&format!("{text}_="), r),
                NameTarget::Skip => {}
            }
        }
        // `a[i] += v`, `a[i]++`: the read (`[]`) comes from the expression
        // itself.
        ExprKind::Index { target, .. } if m.operators_by_selector() => {
            let receiver = receiver_class(m, ctx, *target);
            m.new_selector_with_receiver("[]=", receiver);
        }
        _ => {}
    }
}

/// The Dart name (the `instance_members` key) of a user-definable binary
/// operator.
fn binary_operator_name(op: ast::BinaryOp) -> Option<&'static str> {
    use ast::BinaryOp as B;
    Some(match op {
        B::Add => "+",
        B::Sub => "-",
        B::Mul => "*",
        B::Div => "/",
        B::TruncDiv => "~/",
        B::Rem => "%",
        B::Shl => "<<",
        B::Shr => ">>",
        B::UShr => ">>>",
        B::BitAnd => "&",
        B::BitOr => "|",
        B::BitXor => "^",
        B::Lt => "<",
        B::Gt => ">",
        B::LtEq => "<=",
        B::GtEq => ">=",
        // `==` is an `Object` member (universal); the rest are not operators
        // of a class.
        B::Eq | B::NotEq | B::And | B::Or | B::IfNull | B::NotBinary => return None,
    })
}

/// Whether `c` is `int`, `double` or `num` of `dart:core`.
fn num_like(m: &Engine<'_>, c: Option<ClassId>) -> bool {
    let p = m.e.program;
    c.is_some_and(|c| {
        Some(p.class(c).library) == p.core
            && matches!(
                m.e.interner.resolve(p.class(c).name),
                "int" | "double" | "num"
            )
    })
}

/// The selector of a binary operator applied to `left` (the receiver), with
/// [`Engine::operators_by_selector`]. The operators the emitter writes as
/// JavaScript operators on numbers (`emit_binop_values`: `+ - * / & | ^ < >
/// <= >=` with a number on the left and a number or `dynamic` on the right)
/// call no member and register nothing; `right` is `None` for `x++`/`x--`
/// (the `1` is an `int`). A use the analysis misses is still caught by the
/// text verifier.
fn operator_selector(
    m: &mut Engine<'_>,
    ctx: &Context,
    op: ast::BinaryOp,
    left: ExprId,
    right: Option<ExprId>,
) {
    if !m.operators_by_selector() {
        return;
    }
    let Some(name) = binary_operator_name(op) else {
        return;
    };
    use ast::BinaryOp as B;
    let receiver = receiver_class(m, ctx, left);
    let inlined = matches!(
        op,
        B::Add
            | B::Sub
            | B::Mul
            | B::Div
            | B::BitAnd
            | B::BitOr
            | B::BitXor
            | B::Lt
            | B::Gt
            | B::LtEq
            | B::GtEq
    );
    if inlined && num_like(m, receiver) {
        let right_ok = match right {
            None => true,
            Some(r) => {
                let ty =
                    m.e.bodies
                        .units
                        .get(ctx.unit.0 as usize)
                        .and_then(|u| u.get_type(r));
                ty.is_some_and(|t| {
                    matches!(m.e.table.get(t), dartforge_types::table::Type::Dynamic)
                }) || num_like(m, receiver_class(m, ctx, r))
            }
        };
        if right_ok {
            return;
        }
    }
    m.new_selector_with_receiver(name, receiver);
}

fn for_init(m: &mut Engine<'_>, ctx: &Context, i: &ForInit) {
    match i {
        ForInit::Variables(l) => walk_variables(m, ctx, l),
        ForInit::Expression(x) => expr(m, ctx, *x),
        ForInit::Pattern { pattern, value, .. } => {
            walk_pattern(m, ctx, *pattern);
            expr(m, ctx, *value);
        }
    }
}

fn stmt(m: &mut Engine<'_>, ctx: &Context, id: StmtId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unit).ast;
    match &ast.stmt(id).kind {
        StmtKind::Block(ss) => {
            for s in ss.iter() {
                stmt(m, ctx, *s);
                // What follows a statement that always leaves is dead, and
                // the emitter does not write it (`if (p == null) return;`
                // with `p` always null).
                if m.constants
                    .as_ref()
                    .is_some_and(|c| c.always_exits(ctx.unit, *s))
                {
                    break;
                }
            }
        }
        StmtKind::Variables(l) => walk_variables(m, ctx, l),
        StmtKind::PatternVariables { pattern, value, .. } => {
            walk_pattern(m, ctx, *pattern);
            expr(m, ctx, *value);
        }
        StmtKind::Function(f) => walk_function(m, ctx, *f),
        StmtKind::Expression(x) => expr(m, ctx, *x),
        StmtKind::If {
            condition,
            case_pattern: None,
            guard: None,
            then,
            else_,
        } if constant_condition(m, ctx, *condition).is_some() => {
            // Dead branch by a constant condition: the emitter does not write it either.
            if constant_condition(m, ctx, *condition) == Some(true) {
                stmt(m, ctx, *then);
            } else if let Some(x) = else_ {
                stmt(m, ctx, *x);
            }
        }
        StmtKind::If {
            condition,
            case_pattern,
            guard,
            then,
            else_,
        } => {
            expr(m, ctx, *condition);
            if let Some(pp) = case_pattern {
                walk_pattern(m, ctx, *pp);
            }
            if let Some(g) = guard {
                expr(m, ctx, *g);
            }
            stmt(m, ctx, *then);
            if let Some(x) = else_ {
                stmt(m, ctx, *x);
            }
        }
        StmtKind::For {
            init,
            condition,
            updates,
            body,
            ..
        } => {
            if let Some(i) = init {
                for_init(m, ctx, i);
            }
            if let Some(c) = condition {
                expr(m, ctx, *c);
            }
            for u in updates.iter() {
                expr(m, ctx, *u);
            }
            stmt(m, ctx, *body);
        }
        StmtKind::ForIn {
            target,
            iterable,
            body,
            await_,
        } => {
            // `await for`: the emitter walks through a `StreamIterator`
            // (`moveNext`/`current`/`cancel`), calls that the AST does not have.
            if *await_ && m.include_sdk {
                for n in ["moveNext", "current", "cancel"] {
                    m.new_external_selector(n);
                }
            }
            for_in_target(m, ctx, target);
            expr(m, ctx, *iterable);
            stmt(m, ctx, *body);
        }
        StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
            expr(m, ctx, *condition);
            stmt(m, ctx, *body);
        }
        StmtKind::Switch { value, cases } => {
            expr(m, ctx, *value);
            for c in cases.iter() {
                if let Some(pp) = c.pattern {
                    walk_pattern(m, ctx, pp);
                }
                if let Some(g) = c.guard {
                    expr(m, ctx, g);
                }
                for s in c.body.iter() {
                    stmt(m, ctx, *s);
                }
            }
        }
        StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
        StmtKind::Return(x) => {
            if let Some(x) = x {
                expr(m, ctx, *x);
            }
        }
        StmtKind::Yield { value, .. } => expr(m, ctx, *value),
        StmtKind::Try {
            body,
            catches,
            finally_,
        } => {
            stmt(m, ctx, *body);
            for c in catches.iter() {
                if let Some(t) = c.on_type {
                    ast_type(m, ctx, t);
                }
                stmt(m, ctx, c.body);
            }
            if let Some(f) = finally_ {
                stmt(m, ctx, *f);
            }
        }
        StmtKind::Labeled { body, .. } => stmt(m, ctx, *body),
        StmtKind::Assert { .. } if no_asserts(m) => {}
        StmtKind::Assert { condition, message } => {
            expr(m, ctx, *condition);
            if let Some(x) = message {
                expr(m, ctx, *x);
            }
        }
    }
}

// ---------------------------------------------------------------- patterns

/// Names of the variables declared in a pattern (`:x` in an object pattern is
/// the getter `x`).
fn variable_names(ast: &ast::Ast, p: PatternId, out: &mut Vec<dartforge_intern::SymbolId>) {
    match &ast.pattern(p).kind {
        PatternKind::Variable { name, .. } => out.push(name.sym),
        PatternKind::NullCheck(x)
        | PatternKind::NullAssert(x)
        | PatternKind::Parenthesized(x)
        | PatternKind::Cast { pattern: x, .. } => variable_names(ast, *x, out),
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            variable_names(ast, *a, out);
            variable_names(ast, *b, out);
        }
        PatternKind::Wildcard { .. }
        | PatternKind::Constant(_)
        | PatternKind::Relational { .. }
        | PatternKind::List { .. }
        | PatternKind::Map { .. }
        | PatternKind::Record { .. }
        | PatternKind::Object { .. } => {}
    }
}

fn walk_pattern(m: &mut Engine<'_>, ctx: &Context, id: PatternId) {
    let e = m.e;
    let ast = &e.program.unit(ctx.unit).ast;
    match &ast.pattern(id).kind {
        PatternKind::Wildcard { ty } => {
            if let Some(t) = ty {
                ast_type(m, ctx, *t);
            }
        }
        PatternKind::Variable {
            ty,
            name,
            var_,
            final_,
        } => {
            if let Some(t) = ty {
                ast_type(m, ctx, *t);
            }
            // `case NAME:` — a bare identifier in a `case` is a constant (the
            // parser hands it over as a variable pattern and resolution decides).
            if ty.is_none() && !*var_ && !*final_ {
                m.use_name(ctx, name.sym);
            }
        }
        PatternKind::Constant(x) => {
            expr(m, ctx, *x);
            m.new_selector("==");
        }
        PatternKind::Relational { op, value } => {
            // The matched value's type is not at hand: unrestricted.
            if m.operators_by_selector()
                && let Some(n) = binary_operator_name(*op)
            {
                m.new_selector(n);
            }
            expr(m, ctx, *value)
        }
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            walk_pattern(m, ctx, *a);
            walk_pattern(m, ctx, *b);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => {
            walk_pattern(m, ctx, *x)
        }
        PatternKind::Cast { pattern, ty } => {
            ast_type(m, ctx, *ty);
            walk_pattern(m, ctx, *pattern);
        }
        PatternKind::List {
            type_args,
            elements,
        } => {
            if m.operators_by_selector() {
                m.new_selector("[]");
            }
            for t in type_args.iter() {
                ast_type(m, ctx, *t);
            }
            for el in elements.iter() {
                match el {
                    ast::ListPatternElement::Pattern(x) => walk_pattern(m, ctx, *x),
                    ast::ListPatternElement::Rest(x) => {
                        if let Some(x) = x {
                            walk_pattern(m, ctx, *x);
                        }
                    }
                }
            }
        }
        PatternKind::Map {
            type_args, entries, ..
        } => {
            if m.operators_by_selector() {
                m.new_selector("[]");
            }
            for t in type_args.iter() {
                ast_type(m, ctx, *t);
            }
            for en in entries.iter() {
                expr(m, ctx, en.key);
                walk_pattern(m, ctx, en.value);
            }
        }
        PatternKind::Record { fields } => {
            for f in fields.iter() {
                walk_pattern(m, ctx, f.pattern);
            }
        }
        PatternKind::Object { ty, fields } => {
            ast_type(m, ctx, *ty);
            for f in fields.iter() {
                // A field of an object pattern reads the getter (or tears a
                // method off).
                match f.name {
                    Some(n) => {
                        let s = e.interner.resolve(n.sym);
                        m.poison_name(n.sym);
                        m.new_selector(s);
                    }
                    None => {
                        let mut ns = Vec::new();
                        variable_names(ast, f.pattern, &mut ns);
                        for n in ns {
                            let s = e.interner.resolve(n);
                            m.poison_name(n);
                            m.new_selector(s);
                        }
                    }
                }
                walk_pattern(m, ctx, f.pattern);
            }
        }
    }
}

// ---------------------------------------------------------------- expressions

fn walk_element(m: &mut Engine<'_>, ctx: &Context, el: &CollectionElement) {
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
            expr(m, ctx, *x)
        }
        CollectionElement::MapEntry { key, value, .. } => {
            expr(m, ctx, *key);
            expr(m, ctx, *value);
        }
        CollectionElement::Spread { value, .. } => {
            expr(m, ctx, *value);
            for s in ["iterator", "moveNext", "current"] {
                m.poison_name_str(s);
                m.new_selector(s);
            }
        }
        CollectionElement::If {
            condition,
            case_pattern,
            guard,
            then,
            else_,
        } => {
            expr(m, ctx, *condition);
            if let Some(pp) = case_pattern {
                walk_pattern(m, ctx, *pp);
            }
            if let Some(g) = guard {
                expr(m, ctx, *g);
            }
            walk_element(m, ctx, then);
            if let Some(x) = else_ {
                walk_element(m, ctx, x);
            }
        }
        CollectionElement::For {
            init,
            condition,
            updates,
            body,
            ..
        } => {
            if let Some(i) = init {
                for_init(m, ctx, i);
            }
            if let Some(c) = condition {
                expr(m, ctx, *c);
            }
            for u in updates.iter() {
                expr(m, ctx, *u);
            }
            walk_element(m, ctx, body);
        }
        CollectionElement::ForIn {
            target,
            iterable,
            body,
            await_,
        } => {
            // `await for` in a literal: the same `StreamIterator` calls as
            // the statement.
            if *await_ && m.include_sdk {
                for n in ["moveNext", "current", "cancel"] {
                    m.new_external_selector(n);
                }
            }
            for_in_target(m, ctx, target);
            expr(m, ctx, *iterable);
            walk_element(m, ctx, body);
        }
    }
}

fn apply_resolved(m: &mut Engine<'_>, r: &Resolved, receiver: Option<ClassId>) {
    match r {
        Resolved::Element(el) => m.use_element(*el),
        Resolved::Member { member, .. } => match member {
            MemberRef::Function(f) => m.use_function_with_receiver(*f, receiver),
            MemberRef::Variable(v) => m.use_variable_with_receiver(*v, receiver),
        },
        Resolved::ExtensionMember { member, .. } => m.use_function(*member),
        Resolved::Constructor(f) => m.use_constructor(*f, false),
        Resolved::Local(_)
        | Resolved::Parameter { .. }
        | Resolved::TypeParameter(_)
        | Resolved::Prefix(_)
        | Resolved::Dynamic => {}
    }
}

/// What `prefix` denotes when it is to the left of `.`: a class (statics and
/// named constructors), an extension (statics) or an import prefix.
fn static_target(m: &Engine<'_>, ctx: &Context, prefix: ExprId) -> Option<StaticTarget> {
    let e = m.e;
    let p = e.program;
    let ast = &p.unit(ctx.unit).ast;
    let body_unit = e.bodies.units.get(ctx.unit.0 as usize);
    match &ast.expr(prefix).kind {
        ExprKind::Identifier(n) => {
            match body_unit.and_then(|u| u.get_resolved(prefix)) {
                Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => return None,
                Some(Resolved::Element(Element::Class(c))) => return Some(StaticTarget::Class(*c)),
                Some(Resolved::Element(Element::Extension(x))) => {
                    return Some(StaticTarget::Extension(*x));
                }
                Some(Resolved::Prefix(_)) => return Some(StaticTarget::Prefix(n.sym)),
                _ => {}
            }
            match p.lookup(ctx.library, n.sym).and_then(|b| b.getter)? {
                Element::Class(c) => Some(StaticTarget::Class(c)),
                Element::Extension(x) => Some(StaticTarget::Extension(x)),
                Element::Prefix(..) => Some(StaticTarget::Prefix(n.sym)),
                Element::Typedef(td) => {
                    let tt = e.outline.typedefs.get(td.0 as usize)?.target_type;
                    match e.table.get(tt) {
                        dartforge_types::table::Type::Interface { class, .. } => {
                            Some(StaticTarget::Class(*class))
                        }
                        _ => None,
                    }
                }
                Element::Function(_) | Element::Variable(_) => None,
            }
        }
        // `C<int>.new`, `p.C<T>.name`
        ExprKind::TypeArguments { target, .. } => static_target(m, ctx, *target),
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(pre) = &ast.expr(*target).kind else {
                return None;
            };
            match p
                .lookup_prefixed(ctx.library, pre.sym, name.sym)
                .and_then(|b| b.getter)?
            {
                Element::Class(c) => Some(StaticTarget::Class(c)),
                Element::Extension(x) => Some(StaticTarget::Extension(x)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `target.name` with a static target.
fn static_member(
    m: &mut Engine<'_>,
    ctx: &Context,
    target: StaticTarget,
    name: dartforge_intern::SymbolId,
    tearoff: bool,
) {
    let e = m.e;
    let p = e.program;
    let text = e.interner.resolve(name);
    let setter = e.interner.lookup(&format!("{text}_="));
    match target {
        StaticTarget::Class(c) => {
            m.mark_type(c);
            let class = p.class(c);
            let mut fs: Vec<FunctionElementId> = Vec::new();
            fs.extend(class.static_members.get(&name).copied());
            if let Some(s) = setter {
                fs.extend(class.static_members.get(&s).copied());
            }
            for f in fs {
                m.use_function(f);
            }
            // An enum constant (`.azul` as a dot shorthand) lives one by one.
            let constants: Vec<VariableId> = class
                .enum_constants
                .iter()
                .copied()
                .filter(|v| p.variable(*v).name == name)
                .collect();
            for v in constants {
                m.use_variable(v);
            }
            // A call (`C.name(..)`) or a tearoff (`C.name` without a call):
            // only the tearoff cites the static `_#name#tearOff`.
            m.create(c, text, tearoff);
        }
        StaticTarget::Extension(x) => m.use_extension_static(x, name),
        StaticTarget::Prefix(pre) => {
            if let Some(b) = p.lookup_prefixed(ctx.library, pre, name) {
                for el in [b.getter, b.setter].into_iter().flatten() {
                    m.use_element(el);
                }
            }
        }
    }
}

/// `assert`s disabled in the production profile.
fn no_asserts(m: &Engine<'_>) -> bool {
    m.constants.as_ref().is_some_and(|c| c.without_asserts())
}

/// Is the condition `x` constant (`constants.rs`)? Only for the forms that
/// the emitter also folds (identifier, property, `!`, `&&`, `||`,
/// parentheses); a literal does not count (there is nothing to save).
pub(crate) fn constant_condition(m: &Engine<'_>, ctx: &Context, x: ExprId) -> Option<bool> {
    let c = m.constants.as_ref()?;
    match &m.e.program.unit(ctx.unit).ast.expr(x).kind {
        ExprKind::Identifier(_)
        | ExprKind::Property { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Parenthesized(_) => c.bool_of(ctx.unit, x),
        _ => None,
    }
}

fn expr(m: &mut Engine<'_>, ctx: &Context, id: ExprId) {
    if constant_condition(m, ctx, id).is_some() {
        return;
    }
    let e = m.e;
    let p = e.program;
    let ast = &p.unit(ctx.unit).ast;
    // The receiver cone, when the use is dispatch on an instance with a known
    // static type: it applies to the `Resolved` (which does not carry the
    // receiver type) and to the written name (which is independent of the
    // resolution).
    let receiver = use_receiver(m, ctx, id);
    // A call or a creation: its arguments go to the callee's table, and the
    // callee may be used (`Resolved`, the target) without being poisoned.
    if m.params.is_some()
        && let ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } =
            &ast.expr(id).kind
    {
        let callee = callee_of(m, ctx, id);
        record_arguments(m, ctx, &callee, arguments);
        set_permit(
            m,
            match callee {
                Callee::Direct(f) => Some(f),
                _ => None,
            },
        );
    }
    if let Some(u) = e.bodies.units.get(ctx.unit.0 as usize) {
        if let Some(t) = u.get_type(id) {
            m.visit_type(t);
        }
        if let Some(r) = u.get_resolved(id) {
            let r = r.clone();
            apply_resolved(m, &r, receiver);
        }
    }
    match &ast.expr(id).kind {
        ExprKind::Int(_)
        | ExprKind::Double(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::Symbol(_)
        | ExprKind::This
        | ExprKind::Super
        | ExprKind::CascadeTarget
        | ExprKind::Rethrow => {}
        ExprKind::String(lit) => {
            for part in lit.parts.iter() {
                match part {
                    StringPart::Text(_) => {}
                    StringPart::Interpolation(x) => expr(m, ctx, *x),
                }
            }
        }
        ExprKind::Identifier(n) => {
            // A bare `name` may be an implicit `this.name`. In the target of
            // a simple `Assign` it is a write (`name_=`, the setter's key in
            // `instance_members`); otherwise, a read. It only becomes an
            // instance selector on real dispatch (see `name_target`); the rest
            // (local, top-level, static) lives through the `Resolved` and
            // through `use_name`, below.
            let write = if m.write_target == Some(id) {
                m.write_target = None;
                true
            } else {
                false
            };
            let s = e.interner.resolve(n.sym);
            let key = if write {
                format!("{s}_=")
            } else {
                s.to_string()
            };
            match name_target(m, ctx, id) {
                NameTarget::Register(r) => {
                    m.new_selector_with_receiver(&key, r);
                    if !write && m.invoked != Some(id) && may_tear_off(m, ctx, id) {
                        m.poison_name(n.sym);
                    }
                }
                NameTarget::Skip => {}
            }
            m.use_name(ctx, n.sym);
        }
        ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => expr(m, ctx, *x),
        ExprKind::List {
            type_args,
            elements,
            ..
        }
        | ExprKind::SetOrMap {
            type_args,
            elements,
            ..
        } => {
            for t in type_args.iter() {
                ast_type(m, ctx, *t);
            }
            for el in elements.iter() {
                walk_element(m, ctx, el);
            }
            // With the SDK in the world: a literal with `if`/`for`/`...` is
            // built by the emitter with `set.add(v)` and `map[$_set](k, v)` over
            // the implementation (`LinkedHashSet`/`LinkedMap`), with no AST node.
            if m.include_sdk
                && matches!(&ast.expr(id).kind, ExprKind::SetOrMap { .. })
                && elements.iter().any(|x| {
                    !matches!(
                        x,
                        CollectionElement::Expression(_) | CollectionElement::MapEntry { .. }
                    )
                })
            {
                m.new_external_selector("add");
                m.new_external_selector("[]=");
            }
        }
        ExprKind::Record {
            positional, named, ..
        } => {
            for x in positional.iter() {
                expr(m, ctx, *x);
            }
            for (_, x) in named.iter() {
                expr(m, ctx, *x);
            }
        }
        ExprKind::InstanceCreation {
            ty,
            constructor,
            arguments,
            ..
        } => {
            ast_type(m, ctx, *ty);
            if let Some((c, ctor_name)) = class_and_constructor(m, ctx, *ty, *constructor) {
                m.create(c, ctor_name, false);
            }
            set_permit(m, None);
            walk_arguments(m, ctx, arguments);
        }
        ExprKind::FunctionExpression(f) => walk_function(m, ctx, *f),
        ExprKind::Property { target, name, .. } => {
            // In the target of a simple `Assign` it is a write (`name_=`, the
            // setter's key in `instance_members`); otherwise, a read or a
            // call. The receiver is still a read, and the `Resolved` (which
            // already tells the getter from the setter) still applies. The
            // cone is the receiver's (`receiver`, above). A static access
            // (`C.name`, `prefix.name`) is no dispatch: it records no
            // selector (`static_member`, below, keeps the target live).
            let is_static = m.static_access_selectors() && static_target(m, ctx, *target).is_some();
            if is_static {
                if m.write_target == Some(id) {
                    m.write_target = None;
                }
            } else if m.write_target == Some(id) {
                m.write_target = None;
                let s = e.interner.resolve(name.sym);
                m.new_selector_with_receiver(&format!("{s}_="), receiver);
            } else {
                let s = e.interner.resolve(name.sym);
                m.new_selector_with_receiver(s, receiver);
                // A read that is not the callee may tear the method off.
                if m.invoked != Some(id) && may_tear_off(m, ctx, id) {
                    m.poison_name(name.sym);
                }
            }
            let is_call = m.call_target == Some(id);
            m.call_target = None;
            if let Some(a) = static_target(m, ctx, *target) {
                static_member(m, ctx, a, name.sym, !is_call);
            }
            expr(m, ctx, *target);
        }
        ExprKind::Index { target, index, .. } => {
            if m.operators_by_selector() {
                let receiver = receiver_class(m, ctx, *target);
                if m.write_target == Some(id) {
                    m.write_target = None;
                    m.new_selector_with_receiver("[]=", receiver);
                } else {
                    m.new_selector_with_receiver("[]", receiver);
                }
            }
            expr(m, ctx, *target);
            expr(m, ctx, *index);
        }
        // Dot shorthand (3.10): `.name` is `D.name`, with `D` recorded by
        // `types` in `Resolved` (docs/VERSOES-LINGUAGEM.md §4.3). Where
        // `types` does not reach (`switch` patterns), every declaration that
        // has a static, an enum constant or a constructor with that name
        // counts: more live, never less.
        ExprKind::DotShorthand { name, .. } => {
            let resolved = e
                .bodies
                .units
                .get(ctx.unit.0 as usize)
                .and_then(|u| u.get_resolved(id))
                .and_then(|r| match r {
                    Resolved::Element(Element::Class(c)) => Some(*c),
                    _ => None,
                });
            let classes: Vec<ClassId> = match resolved {
                Some(c) => vec![c],
                None => {
                    let text = e.interner.resolve(name.sym);
                    let ctor_key = if text == "new" {
                        e.interner.lookup("")
                    } else {
                        Some(name.sym)
                    };
                    (0..p.classes.len() as u32)
                        .map(ClassId)
                        .filter(|&c| {
                            let k = p.class(c);
                            k.static_members.contains_key(&name.sym)
                                || k.enum_constants
                                    .iter()
                                    .any(|v| p.variable(*v).name == name.sym)
                                || ctor_key.is_some_and(|s| k.constructors.contains_key(&s))
                        })
                        .collect()
                }
            };
            for c in classes {
                static_member(m, ctx, StaticTarget::Class(c), name.sym, true);
            }
        }
        ExprKind::Call { target, arguments } => {
            // `C(..)` / `p.C(..)` without `new`: creation through the unnamed constructor.
            if let Some(StaticTarget::Class(c)) = static_target(m, ctx, *target) {
                m.create(c, "", false);
            }
            // SDK: `JS_CLASS_REF(C)` hands the class to a JS *template*, which
            // extends or constructs it (`class _Record extends #`, `new #(…)`):
            // the class counts as instantiated.
            if m.e.program.library(ctx.library).is_sdk {
                let ast = &m.e.program.unit(ctx.unit).ast;
                let callee = match &ast.expr(*target).kind {
                    ExprKind::Identifier(n) => Some(n.sym),
                    ExprKind::Property { name, .. } => Some(name.sym),
                    _ => None,
                };
                // `extensionSymbol('toString')`: the runtime calls that
                // `Object` member on an unknown receiver.
                if m.include_sdk
                    && callee.is_some_and(|s| m.e.interner.resolve(s) == "extensionSymbol")
                    && let Some(a) = arguments.args.first()
                    && let ExprKind::String(lit) = &ast.expr(a.value).kind
                    && let Some(v) = lit.constant_value()
                {
                    m.runtime_object_selector(&v.to_string_lossy());
                }
                // `JS('', '#.moveNext()', it)`: the member name written in the
                // *template* is called on an untyped receiver — an
                // unrestricted selector, of both kinds.
                if m.include_sdk && callee.is_some_and(|s| m.e.interner.resolve(s) == "JS") {
                    compared_class_refs(m, ctx, arguments);
                    if let Some(a) = arguments.args.get(1)
                        && let ExprKind::String(lit) = &ast.expr(a.value).kind
                    {
                        let mut names: Vec<(String, Option<usize>)> = Vec::new();
                        for part in lit.parts.iter() {
                            if let StringPart::Text(t) = part {
                                let t = t.to_string_lossy();
                                // Every word of the *template* is a JS name (property, object
                                // key, global): property renaming does not touch it.
                                m.record_js_words(&t);
                                let b = t.as_bytes();
                                for (i, _) in t.match_indices('.') {
                                    let start = i + 1;
                                    let mut f = start;
                                    while f < b.len()
                                        && (b[f].is_ascii_alphanumeric()
                                            || b[f] == b'_'
                                            || b[f] == b'$')
                                    {
                                        f += 1;
                                    }
                                    if f > start && !b[start].is_ascii_digit() {
                                        names.push((
                                            t[start..f].to_string(),
                                            template_arity(&t[f..]),
                                        ));
                                    }
                                }
                            }
                        }
                        for (n, arity) in names {
                            if crate::is_js_function_method(&n) {
                                continue;
                            }
                            // `#.name(a, b)` calls the member with that many
                            // positional arguments of unknown value; any other
                            // use may tear it off.
                            match (arity, m.e.interner.lookup(&n)) {
                                (Some(k), Some(sym)) => {
                                    m.record_call(&Callee::Name(sym), &vec![Value::Top; k], &[]);
                                }
                                _ => m.poison_name_str(&n),
                            }
                            m.new_template_selector(&n);
                        }
                    }
                }
            }
            // `x.isA<T>()` from `dart:js_interop` is lowered by the emitter to
            // `JSAnyUtilityExtension|instanceOfString(x, "T")`.
            if m.include_sdk {
                let target_ast = &m.e.program.unit(ctx.unit).ast;
                let real_target = match &target_ast.expr(*target).kind {
                    ExprKind::TypeArguments { target: t, .. } => *t,
                    _ => *target,
                };
                if let ExprKind::Property { name, .. } = &target_ast.expr(real_target).kind
                    && m.e.interner.resolve(name.sym) == "isA"
                {
                    m.new_external_selector("instanceOfString");
                }
            }
            if m.e.program.library(ctx.library).is_sdk {
                let ast = &m.e.program.unit(ctx.unit).ast;
                let callee = match &ast.expr(*target).kind {
                    ExprKind::Identifier(n) => Some(n.sym),
                    ExprKind::Property { name, .. } => Some(name.sym),
                    _ => None,
                };
                if callee.is_some_and(|s| m.e.interner.resolve(s) == "JS_CLASS_REF") {
                    let compared = m.compared_class_refs.contains(&(ctx.unit, id));
                    if let Some(a) = arguments.args.first() {
                        if compared {
                            if let Some(StaticTarget::Class(c)) = static_target(m, ctx, a.value) {
                                m.mark_type(c);
                            }
                        } else if let Some(StaticTarget::Class(c)) = static_target(m, ctx, a.value)
                        {
                            m.instantiate(c);
                            // The constructor is called by the *template*
                            // (`Object.getPrototypeOf(#).new.call(…)`).
                            let ctors: Vec<FunctionElementId> =
                                m.e.program
                                    .class(c)
                                    .constructors
                                    .values()
                                    .copied()
                                    .collect();
                            for k in ctors {
                                m.use_constructor(k, false);
                            }
                        }
                    }
                }
            }
            // `JS_GET_NAME(JsGetName.RTI_NAME)`: the emitter folds the enum
            // constant into the name itself, so neither it nor the helper's
            // type (`String Function(JsGetName)`) is read.
            let folded = folded_enum_argument(m, ctx, *target, arguments);
            if folded.is_none() {
                m.call_target = Some(*target);
                let saved = m.invoked.replace(unwrapped_callee(ast, *target));
                expr(m, ctx, *target);
                m.invoked = saved;
                m.call_target = None;
            }
            set_permit(m, None);
            for t in arguments.type_args.iter() {
                ast_type(m, ctx, *t);
            }
            for (i, arg) in arguments.args.iter().enumerate() {
                if folded != Some(i) {
                    expr(m, ctx, arg.value);
                }
            }
        }
        ExprKind::TypeArguments { target, type_args } => {
            expr(m, ctx, *target);
            for t in type_args.iter() {
                ast_type(m, ctx, *t);
            }
        }
        ExprKind::Unary { op, operand } => {
            // `++`/`--` reads and writes (`x++` calls the getter and the setter).
            if matches!(
                op,
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec
            ) {
                write_to_target(m, ctx, *operand);
                let bop = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) {
                    ast::BinaryOp::Add
                } else {
                    ast::BinaryOp::Sub
                };
                operator_selector(m, ctx, bop, *operand, None);
            }
            if m.operators_by_selector() && matches!(op, UnaryOp::Neg | UnaryOp::BitNot) {
                let receiver = receiver_class(m, ctx, *operand);
                // The emitter writes `-x` and `~x >>> 0` for numbers.
                if !num_like(m, receiver) {
                    m.new_selector_with_receiver(
                        if *op == UnaryOp::Neg { "unary-" } else { "~" },
                        receiver,
                    );
                }
            }
            expr(m, ctx, *operand);
        }
        ExprKind::Binary { op, left, right } => {
            operator_selector(m, ctx, *op, *left, Some(*right));
            expr(m, ctx, *left);
            expr(m, ctx, *right);
        }
        ExprKind::Conditional {
            condition,
            then,
            else_,
        } => match constant_condition(m, ctx, *condition) {
            Some(true) => expr(m, ctx, *then),
            Some(false) => expr(m, ctx, *else_),
            None => {
                expr(m, ctx, *condition);
                expr(m, ctx, *then);
                expr(m, ctx, *else_);
            }
        },
        ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
            expr(m, ctx, *value);
            ast_type(m, ctx, *ty);
        }
        ExprKind::Assign { op, target, value } => {
            match op {
                // A pure write: only the outermost member of the target is
                // `name=`; the rest of the target (receiver, index) is still a
                // read.
                AssignOp::Assign => {
                    // `(x.w) = v`: the real target is inside the parentheses.
                    let mut lhs = *target;
                    while let ExprKind::Parenthesized(x) =
                        &e.program.unit(ctx.unit).ast.expr(lhs).kind
                    {
                        lhs = *x;
                    }
                    m.write_target = Some(lhs);
                    expr(m, ctx, *target);
                    m.write_target = None;
                    expr(m, ctx, *value);
                }
                // Compound (`+=`, `??=`): reads and writes.
                AssignOp::Compound(bop) => {
                    write_to_target(m, ctx, *target);
                    operator_selector(m, ctx, *bop, *target, Some(*value));
                    expr(m, ctx, *target);
                    expr(m, ctx, *value);
                }
            }
        }
        ExprKind::PatternAssign { pattern, value } => {
            walk_pattern(m, ctx, *pattern);
            expr(m, ctx, *value);
        }
        ExprKind::Cascade {
            target, sections, ..
        } => {
            expr(m, ctx, *target);
            for s in sections.iter() {
                expr(m, ctx, *s);
            }
        }
        ExprKind::Switch { value, cases } => {
            expr(m, ctx, *value);
            for c in cases.iter() {
                walk_pattern(m, ctx, c.pattern);
                if let Some(g) = c.guard {
                    expr(m, ctx, g);
                }
                expr(m, ctx, c.body);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::template_arity;

    #[test]
    fn template_call_arity() {
        assert_eq!(template_arity("(#)"), Some(1));
        assert_eq!(template_arity(" ()"), Some(0));
        assert_eq!(template_arity("(#, f(#, #), [#, #])"), Some(3));
        assert_eq!(template_arity("(#, ')', #)"), Some(3));
        assert_eq!(template_arity(" = #"), None);
        assert_eq!(template_arity("(#, "), None);
    }
}
