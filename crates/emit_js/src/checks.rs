//! Which run-time checks a member's body emits in the production profile:
//! the check policies of `dart2js` 3.6.2
//! (`pkg/compiler/lib/src/js_backend/annotations.dart`, `CheckPolicy`).
//!
//! The optimization level gives the defaults (`options.dart`,
//! `deriveOptions`): `-O3` trusts implicit downcasts, conditions and
//! parameters (`--omit-implicit-checks`), `-O4` also trusts index bounds
//! (`--trust-primitives`); explicit `as` casts are checked at every level.
//! A member overrides them with its `@pragma('dart2js:…')` annotations, at
//! every level, with `dart2js`'s precedence:
//!
//! * parameters: `types:trust`, `types:check`, `parameter:trust`,
//!   `parameter:check`;
//! * implicit downcasts and conditions: `types:trust`, `types:check`,
//!   `downcast:trust`, `downcast:check`;
//! * explicit casts: `as:trust`, `as:check`;
//! * index bounds: `index-bounds:trust`, `index-bounds:check`.
//!
//! As in `dart2js`, the policy is the member's: a closure inside it has the
//! level's defaults (`dart2js` compiles a closure as a member of its own,
//! without the enclosing member's annotations). The annotation may be the
//! `pragma` constructor itself or a constant that evaluates to one
//! (`const noBoundsChecks = isJavaScript ? pragma('dart2js:…') : …`, with
//! `identical(1, 1.0)` true on the web).
//!
//! The development profile (no filter) follows DDC, which ignores these
//! pragmas: everything is checked.

use crate::ctx::Ctx;
use dartforge_elements::model::{
    Element, FunctionElementId, FunctionRef, UnitId, VariableId, VariableRef,
};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;
use std::rc::Rc;

/// What a body checks (`true`: the check is emitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checks {
    /// Explicit `e as T` (and cast patterns).
    pub explicit_casts: bool,
    /// Implicit downcasts (from `dynamic`, the CFE's covariance checks on
    /// reads, the `noSuchMethod` forwarders' results) and boolean conditions.
    pub downcasts: bool,
    /// Parameter checks: covariant parameters and fields, and the arguments
    /// of dynamic calls.
    pub parameters: bool,
    /// Index range checks of the indexable primitives (`JSArray`,
    /// `JSString`, the native typed arrays).
    pub index_bounds: bool,
}

impl Checks {
    /// Everything checked: the development profile and `-O1`/`-O2`.
    pub const ALL: Checks = Checks {
        explicit_casts: true,
        downcasts: true,
        parameters: true,
        index_bounds: true,
    };

    /// The defaults of the level the filter carries.
    pub fn level_default(ctx: &Ctx) -> Checks {
        match ctx.filter {
            Some(f) if ctx.production() => {
                let implicit = !f.omit_implicit_checks();
                Checks {
                    explicit_casts: true,
                    downcasts: implicit,
                    parameters: implicit,
                    index_bounds: !f.trust_primitives(),
                }
            }
            _ => Checks::ALL,
        }
    }

    /// `self` overridden by the `dart2js` pragmas in `names`.
    pub fn with_pragmas(self, names: &[Pragma]) -> Checks {
        let has = |p: Pragma| names.contains(&p);
        let pick = |trust: Pragma, check: Pragma, default: bool| {
            if has(trust) {
                false
            } else if has(check) {
                true
            } else {
                default
            }
        };
        let typed = |trust: Pragma, check: Pragma, default: bool| {
            if has(Pragma::TypesTrust) {
                false
            } else if has(Pragma::TypesCheck) {
                true
            } else {
                pick(trust, check, default)
            }
        };
        Checks {
            explicit_casts: pick(Pragma::AsTrust, Pragma::AsCheck, self.explicit_casts),
            downcasts: typed(Pragma::DowncastTrust, Pragma::DowncastCheck, self.downcasts),
            parameters: typed(
                Pragma::ParameterTrust,
                Pragma::ParameterCheck,
                self.parameters,
            ),
            index_bounds: pick(
                Pragma::IndexBoundsTrust,
                Pragma::IndexBoundsCheck,
                self.index_bounds,
            ),
        }
    }

    /// The checks of the body of function `fid` (a top-level function, a
    /// method, an accessor or a constructor).
    pub fn of_function(ctx: &Ctx, fid: FunctionElementId) -> Checks {
        let base = Checks::level_default(ctx);
        if !ctx.production() {
            return base;
        }
        let f = ctx.program.function(fid);
        let mut names = match f.node {
            FunctionRef::Function { unit, function } => unit_pragmas(ctx, unit)
                .functions
                .get(&function)
                .cloned()
                .unwrap_or_default(),
            FunctionRef::Constructor { unit, member } => unit_pragmas(ctx, unit)
                .members
                .get(&member)
                .cloned()
                .unwrap_or_default(),
            FunctionRef::None => Vec::new(),
        };
        // An implicit accessor is the field's.
        if let Some(v) = f.variable {
            names.extend(variable_pragmas(ctx, v));
        }
        // The public declaration of an SDK patch carries pragmas too.
        if let Some(p) = f.public_declaration
            && p != fid
            && let FunctionRef::Function { unit, function } = ctx.program.function(p).node
            && let Some(more) = unit_pragmas(ctx, unit).functions.get(&function)
        {
            names.extend(more.iter().copied());
        }
        base.with_pragmas(&names)
    }

    /// The checks of field (or top-level variable) `v`: its initializer and
    /// its setter.
    pub fn of_variable(ctx: &Ctx, v: VariableId) -> Checks {
        let base = Checks::level_default(ctx);
        if !ctx.production() {
            return base;
        }
        base.with_pragmas(&variable_pragmas(ctx, v))
    }
}

/// A `dart2js` check pragma (`pragmas.md`, "Unsafe pragmas for general use").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pragma {
    AsTrust,
    AsCheck,
    TypesTrust,
    TypesCheck,
    ParameterTrust,
    ParameterCheck,
    DowncastTrust,
    DowncastCheck,
    IndexBoundsTrust,
    IndexBoundsCheck,
}

impl Pragma {
    /// The pragma named `name` (`dart2js:as:trust`…).
    pub fn parse(name: &str) -> Option<Pragma> {
        Some(match name.strip_prefix("dart2js:")? {
            "as:trust" => Pragma::AsTrust,
            "as:check" => Pragma::AsCheck,
            "types:trust" => Pragma::TypesTrust,
            "types:check" => Pragma::TypesCheck,
            "parameter:trust" => Pragma::ParameterTrust,
            "parameter:check" => Pragma::ParameterCheck,
            "downcast:trust" => Pragma::DowncastTrust,
            "downcast:check" => Pragma::DowncastCheck,
            "index-bounds:trust" => Pragma::IndexBoundsTrust,
            "index-bounds:check" => Pragma::IndexBoundsCheck,
            _ => return None,
        })
    }
}

/// The check pragmas of the declarations of one unit.
#[derive(Default)]
pub struct UnitPragmas {
    /// Top-level functions and methods, by their function node.
    functions: HashMap<ast::FunctionId, Vec<Pragma>>,
    /// Constructors and fields, by member.
    members: HashMap<ast::MemberId, Vec<Pragma>>,
    /// Top-level variable lists, by declaration.
    decls: HashMap<ast::DeclId, Vec<Pragma>>,
}

/// [`UnitPragmas`] of `unit`, computed once per emission context.
fn unit_pragmas(ctx: &Ctx, unit: UnitId) -> Rc<UnitPragmas> {
    if let Some(p) = ctx.check_pragmas.borrow().get(&unit) {
        return p.clone();
    }
    let a = &ctx.program.unit(unit).ast;
    let mut out = UnitPragmas::default();
    for (i, d) in a.decls.iter().enumerate() {
        if d.metadata.is_empty() {
            continue;
        }
        let names = pragmas(ctx, unit, &d.metadata);
        if names.is_empty() {
            continue;
        }
        match &d.kind {
            ast::DeclKind::Function(f) => {
                out.functions.insert(*f, names);
            }
            ast::DeclKind::Variables(_) => {
                out.decls.insert(ast::DeclId(i as u32), names);
            }
            _ => {}
        }
    }
    for (i, m) in a.members.iter().enumerate() {
        if m.metadata.is_empty() {
            continue;
        }
        let names = pragmas(ctx, unit, &m.metadata);
        if names.is_empty() {
            continue;
        }
        match &m.kind {
            ast::MemberKind::Method(f) => {
                out.functions.insert(*f, names);
            }
            ast::MemberKind::Constructor(_) | ast::MemberKind::Field(_) => {
                out.members.insert(ast::MemberId(i as u32), names);
            }
        }
    }
    let out = Rc::new(out);
    ctx.check_pragmas.borrow_mut().insert(unit, out.clone());
    out
}

fn variable_pragmas(ctx: &Ctx, v: VariableId) -> Vec<Pragma> {
    match ctx.program.variable(v).node {
        VariableRef::Field { unit, member, .. } => unit_pragmas(ctx, unit)
            .members
            .get(&member)
            .cloned()
            .unwrap_or_default(),
        VariableRef::TopLevel { unit, decl, .. } => unit_pragmas(ctx, unit)
            .decls
            .get(&decl)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The check pragmas among the annotations `metadata` of unit `unit`.
fn pragmas(ctx: &Ctx, unit: UnitId, metadata: &[ast::Annotation]) -> Vec<Pragma> {
    let mut out = Vec::new();
    for a in metadata {
        let name = match &a.arguments {
            Some(args) if is_pragma_class(ctx, unit, &a.name) => pragma_argument(ctx, unit, args),
            Some(_) => None,
            // `@noBoundsChecks`: a constant.
            None => match a.name.as_slice() {
                [n] => ctx
                    .program
                    .lookup_in_unit(unit, n.sym)
                    .and_then(|b| b.getter)
                    .and_then(|el| constant_pragma(ctx, el, 0)),
                [prefix, n] => ctx
                    .program
                    .lookup_prefixed_in_unit(unit, prefix.sym, n.sym)
                    .and_then(|b| b.getter)
                    .and_then(|el| constant_pragma(ctx, el, 0)),
                _ => None,
            },
        };
        if let Some(p) = name.as_deref().and_then(Pragma::parse) {
            out.push(p);
        }
    }
    out
}

/// `name` (`pragma` or `core.pragma`) is `dart:core`'s `pragma` class.
fn is_pragma_class(ctx: &Ctx, unit: UnitId, name: &[ast::Name]) -> bool {
    let binding = match name {
        [n] => ctx.program.lookup_in_unit(unit, n.sym),
        [prefix, n] => ctx.program.lookup_prefixed_in_unit(unit, prefix.sym, n.sym),
        _ => None,
    };
    match binding.and_then(|b| b.getter) {
        Some(Element::Class(c)) => {
            let class = ctx.program.class(c);
            ctx.interner.resolve(class.name) == "pragma"
                && ctx.program.library(class.library).uri == "dart:core"
        }
        _ => false,
    }
}

/// The first positional argument of `pragma(…)`, a string literal.
fn pragma_argument(ctx: &Ctx, unit: UnitId, args: &ast::Arguments) -> Option<String> {
    let first = args.args.iter().find(|a| a.name.is_none())?;
    match &ctx.program.unit(unit).ast.expr(first.value).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        _ => None,
    }
}

/// The pragma name a constant variable evaluates to.
fn constant_pragma(ctx: &Ctx, el: Element, depth: u32) -> Option<String> {
    let (unit, x) = constant_initializer(ctx, el)?;
    pragma_value(ctx, unit, x, depth + 1)
}

/// The initializer of a `const` variable.
fn constant_initializer(ctx: &Ctx, el: Element) -> Option<(UnitId, ExprId)> {
    let v = match el {
        Element::Variable(v) => v,
        Element::Function(f) => ctx.program.function(f).variable?,
        _ => return None,
    };
    let var = ctx.program.variable(v);
    if !var.const_ {
        return None;
    }
    match var.node {
        VariableRef::TopLevel { unit, decl, index } => {
            match &ctx.program.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(l) => Some((unit, l.variables.get(index)?.initializer?)),
                _ => None,
            }
        }
        VariableRef::Field {
            unit,
            member,
            index,
        } => match &ctx.program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => Some((unit, l.variables.get(index)?.initializer?)),
            _ => None,
        },
        _ => None,
    }
}

/// The pragma name of a constant expression: `pragma('x')`, a constant
/// that holds one, or a conditional between them.
fn pragma_value(ctx: &Ctx, unit: UnitId, x: ExprId, depth: u32) -> Option<String> {
    if depth > 16 {
        return None;
    }
    let a = &ctx.program.unit(unit).ast;
    match &a.expr(x).kind {
        ExprKind::Parenthesized(i) => pragma_value(ctx, unit, *i, depth + 1),
        ExprKind::InstanceCreation {
            ty,
            constructor: None,
            arguments,
            ..
        } => match &a.ty(*ty).kind {
            ast::TypeKind::Named { name, .. } if is_pragma_class(ctx, unit, name) => {
                pragma_argument(ctx, unit, arguments)
            }
            _ => None,
        },
        ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
            ExprKind::Identifier(n) if is_pragma_class(ctx, unit, std::slice::from_ref(n)) => {
                pragma_argument(ctx, unit, arguments)
            }
            _ => None,
        },
        ExprKind::Identifier(n) => {
            let el = ctx.program.lookup_in_unit(unit, n.sym)?.getter?;
            constant_pragma(ctx, el, depth)
        }
        ExprKind::Conditional {
            condition,
            then,
            else_,
        } => {
            if bool_value(ctx, unit, *condition, depth + 1)? {
                pragma_value(ctx, unit, *then, depth + 1)
            } else {
                pragma_value(ctx, unit, *else_, depth + 1)
            }
        }
        _ => None,
    }
}

/// The value on the web of the constant conditions that choose a platform's
/// pragma: literals, `!`, constants, `identical(a, b)` of number literals
/// (`identical(1, 1.0)` is true in JavaScript) and
/// `bool.fromEnvironment(name)` (only the `dart.library.*` the platform
/// supports are declared).
fn bool_value(ctx: &Ctx, unit: UnitId, x: ExprId, depth: u32) -> Option<bool> {
    if depth > 16 {
        return None;
    }
    let u = ctx.program.unit(unit);
    let a = &u.ast;
    match &a.expr(x).kind {
        ExprKind::Bool(b) => Some(*b),
        ExprKind::Parenthesized(i) => bool_value(ctx, unit, *i, depth + 1),
        ExprKind::Unary {
            op: ast::UnaryOp::Not,
            operand,
        } => bool_value(ctx, unit, *operand, depth + 1).map(|b| !b),
        ExprKind::Identifier(n) => {
            let el = ctx.program.lookup_in_unit(unit, n.sym)?.getter?;
            let (u2, init) = constant_initializer(ctx, el)?;
            bool_value(ctx, u2, init, depth + 1)
        }
        ExprKind::Call { target, arguments } => {
            let number = |e: ExprId| -> Option<f64> {
                match &a.expr(e).kind {
                    ExprKind::Int(s) | ExprKind::Double(s) => {
                        u.source[s.start..s.end].replace('_', "").parse().ok()
                    }
                    _ => None,
                }
            };
            match &a.expr(*target).kind {
                ExprKind::Identifier(n) if ctx.interner.resolve(n.sym) == "identical" => {
                    match arguments.args.as_ref() {
                        [l, r] => Some(number(l.value)? == number(r.value)?),
                        _ => None,
                    }
                }
                ExprKind::Property { target, name, .. }
                    if ctx.interner.resolve(name.sym) == "fromEnvironment"
                        && matches!(&a.expr(*target).kind,
                            ExprKind::Identifier(b) if ctx.interner.resolve(b.sym) == "bool") =>
                {
                    let first = arguments.args.iter().find(|a| a.name.is_none())?;
                    let ExprKind::String(lit) = &a.expr(first.value).kind else {
                        return None;
                    };
                    let name = lit.constant_value()?.to_string_lossy();
                    Some(name.strip_prefix("dart.library.").is_some_and(|l| {
                        crate::SUPPORTED_DART_LIBRARIES
                            .get()
                            .is_some_and(|s| s.contains(l))
                    }))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether `fid` is a static function whose body only returns its single
/// required positional parameter through a cast its policy trusts
/// (`@pragma('dart2js:as:trust') T unsafeCast<T>(Object? o) => o as T;`,
/// ngdart's `unsafeCast`; or `=> o` with trusted downcasts): `dart2js`
/// inlines it (`dart2js:tryInline`) into its argument, and the production
/// profile writes its calls the same way (`call.rs`).
pub fn trusted_identity(ctx: &Ctx, fid: FunctionElementId) -> bool {
    if !ctx.production() {
        return false;
    }
    let f = ctx.program.function(fid);
    if (f.class.is_some() && !f.static_) || f.external || f.extension.is_some() {
        return false;
    }
    let FunctionRef::Function { unit, function } = f.node else {
        return false;
    };
    let a = &ctx.program.unit(unit).ast;
    let func = a.function(function);
    let [param] = func.parameters.as_deref().unwrap_or(&[]) else {
        return false;
    };
    let (Some(name), ast::ParameterKind::Required) = (param.name, param.kind) else {
        return false;
    };
    if func.modifier != ast::AsyncModifier::None || param.this_ || param.super_ {
        return false;
    }
    let ast::FunctionBody::Expression(mut x) = func.body else {
        return false;
    };
    while let ExprKind::Parenthesized(i) = &a.expr(x).kind {
        x = *i;
    }
    let checks = Checks::of_function(ctx, fid);
    let (value, trusted) = match &a.expr(x).kind {
        ExprKind::As { value, .. } => (*value, !checks.explicit_casts),
        ExprKind::Identifier(_) => (x, !checks.downcasts),
        _ => return false,
    };
    trusted && matches!(&a.expr(value).kind, ExprKind::Identifier(n) if n.sym == name.sym)
}
