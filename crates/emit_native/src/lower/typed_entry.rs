//! Typed call and the entry that only checks what the static caller does not
//! guarantee (N17, docs/NATIVO-PLANO.md §9).
//!
//! **The Dart rule.** In a call whose receiver has a known static type,
//! the analyzer has already guaranteed that each argument is a subtype of the type
//! of the parameter in the interface — except for *covariant* parameters: those
//! declared `covariant` (or that inherit the keyword from an overridden member)
//! and those that mention a type parameter of the class (the covariance of
//! generic classes: `List<Object?> l = <int>[]; l.add('x')` passes
//! analysis and throws `TypeError` at run time). That is why the VM compiles two
//! entries per method: the *checked entry*, for the dynamic forwarders
//! (`dyn:m`), which checks everything, and the *unchecked entry*, for typed
//! calls, which only checks the covariant ones. A `dynamic` argument in a
//! typed call gets from the CFE an implicit cast around it (`as T`, with
//! the `T` of the invocation), evaluated right after the argument itself.
//!
//! **Here.** The call by selector of a member of the source SDK with a
//! typed receiver uses the selector `t` + the usual one (`tc:m`, `ts:x`); the
//! class table binds it to the `$tc`/`$ts` entry of the method when it
//! exists — only when the `$c` checks some parameter that the typed one waives
//! ([`needs_typed_entry`]) — and the runtime (`dartforge_selector`) falls back to the
//! selector without the `t` when the class does not have the typed one (methods with nothing to
//! waive, `noSuchMethod` forwarders, program classes). The
//! typed entry does not check the arity either (`dartforge_args_match`): a
//! valid override accepts the arguments of the interface.
//!
//! The implicit cast of `dynamic` arguments (and the rule for when the call
//! can be typed) is that of [`FnBuilder::evaluate_args`] with the types
//! armed by `FnBuilder::arm_arg_types`: the type of the parameter in the
//! invocation, without free type variables. Only a call whose arguments
//! all have a guaranteed type (`checked_args`) uses the typed selector; the
//! other goes back to the one that checks everything.

use super::fn_builder::FnBuilder;
use super::members::Evaluated;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionRef};
use dartforge_frontend::ast::MemberKind;
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::TypeId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type as T, TypeParamId, TypeParamOwner};
use std::collections::HashMap;

/// The types of the parameters of an invocation, already with the type arguments
/// of the receiver or of the creation substituted: positionals in order and named ones
/// by their external name.
#[derive(Clone, Debug, Default)]
pub struct InvocationTypes {
    positionals: Vec<ParameterType>,
    named: Vec<(SymbolId, ParameterType)>,
}

/// The type of a parameter in the invocation and, when it mentions type
/// parameters of the created class, the substitution that closes it.
#[derive(Clone, Debug)]
struct ParameterType {
    ty: TypeId,
    subst: Option<HashMap<TypeParamId, TypeId>>,
}

impl InvocationTypes {
    fn of_argument(&self, name: Option<SymbolId>, i_pos: usize) -> Option<&ParameterType> {
        match name {
            Some(n) => self.named.iter().find(|(m, _)| *m == n).map(|(_, t)| t),
            None => self.positionals.get(i_pos),
        }
    }
}

/// `t` does not mention a type parameter (neither of the class, nor of the function, nor the
/// bound ones of a generic function type): the recipe is constant and the cast
/// can be done at the call site.
fn closed(ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>) -> bool {
    closed_until(ctx, t, subst, 8)
}

fn closed_until(
    ctx: &Context,
    t: TypeId,
    subst: Option<&HashMap<TypeParamId, TypeId>>,
    prof: u32,
) -> bool {
    let closed = |ctx: &Context, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>| {
        closed_until(ctx, t, subst, prof)
    };
    match ctx.table.get(t) {
        // The substitution may lead to another parameter (that of the supertype to that of the class
        // of the receiver), replaced by the same map.
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            prof > 0
                && subst
                    .and_then(|s| s.get(param))
                    .is_some_and(|&x| closed_until(ctx, x, subst, prof - 1))
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => {
            args.iter().all(|a| closed(ctx, *a, subst))
        }
        T::FutureOr { arg, .. } => closed(ctx, *arg, subst),
        T::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            type_params.is_empty()
                && closed(ctx, *ret, subst)
                && positional
                    .iter()
                    .chain(optional.iter())
                    .all(|p| closed(ctx, *p, subst))
                && named.iter().all(|(_, p, _)| closed(ctx, *p, subst))
        }
        T::Record {
            positional, named, ..
        } => {
            positional.iter().all(|p| closed(ctx, *p, subst))
                && named.iter().all(|(_, p)| closed(ctx, *p, subst))
        }
        T::Dynamic | T::Void | T::Never | T::Null => true,
    }
}

/// The top (`dynamic`, `void`, `Object?`): nothing to check.
fn top(ctx: &Context, t: TypeId) -> bool {
    is_top(ctx, t)
}

/// `t` is a top type (`dynamic`, `Object?`, `void`): no check accepts less.
pub(crate) fn is_top(ctx: &Context, t: TypeId) -> bool {
    t == ctx.core.dynamic_ || t == ctx.core.object_nullable || matches!(ctx.table.get(t), T::Void)
}

/// The parameters of `fid` written `covariant` (by index in the declaration),
/// including the implicit setter of a `covariant` field.
fn declared_covariant(ctx: &Context, fid: usize) -> Vec<(usize, Option<SymbolId>)> {
    let f = &ctx.program.functions[fid];
    let mut exit = Vec::new();
    match f.node {
        FunctionRef::Function { unit, function } => {
            let af = ctx.program.unit(unit).ast.function(function);
            for (i, p) in af.parameters.as_deref().unwrap_or(&[]).iter().enumerate() {
                if p.covariant {
                    let name = (p.kind == ParameterKind::Named)
                        .then(|| p.external_name().map(|n| n.sym))
                        .flatten();
                    exit.push((i, name));
                }
            }
        }
        FunctionRef::None => {
            if let Some(v) = f.variable
                && let dartforge_elements::model::VariableRef::Field { unit, member, .. } =
                    ctx.program.variables[v.0 as usize].node
                && let MemberKind::Field(vl) = &ctx.program.unit(unit).ast.member(member).kind
                && vl.covariant
            {
                exit.push((0, None));
            }
        }
        FunctionRef::Constructor { .. } => {}
    }
    exit
}

/// `t` mentions a type parameter of class `c` in a covariant (or
/// invariant) position: what makes the parameter of type `t` *generic
/// covariant* (the kernel's `isGenericCovariantImpl`). `positive` is the
/// current polarity: the parameter of a function type inverts it.
fn mentions_covariant(ctx: &Context, t: TypeId, c: ClassId, positive: bool) -> bool {
    match ctx.table.get(t) {
        T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
            positive && matches!(ctx.table.param(*param).owner, TypeParamOwner::Class(d) if d == c)
        }
        T::Interface { args, .. } | T::ExtensionType { args, .. } => args
            .iter()
            .any(|a| mentions_covariant(ctx, *a, c, positive)),
        T::FutureOr { arg, .. } => mentions_covariant(ctx, *arg, c, positive),
        T::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            // A bound of the function's own type parameters is an invariant
            // position (`void Function<S extends T>(S)`).
            type_params.iter().any(|&p| {
                let bound = ctx.table.param(p).bound;
                mentions_covariant(ctx, bound, c, true) || mentions_covariant(ctx, bound, c, false)
            }) || mentions_covariant(ctx, *ret, c, positive)
                || positional
                    .iter()
                    .chain(optional.iter())
                    .any(|p| mentions_covariant(ctx, *p, c, !positive))
                || named
                    .iter()
                    .any(|(_, p, _)| mentions_covariant(ctx, *p, c, !positive))
        }
        T::Record {
            positional, named, ..
        } => {
            positional
                .iter()
                .any(|p| mentions_covariant(ctx, *p, c, positive))
                || named
                    .iter()
                    .any(|(_, p)| mentions_covariant(ctx, *p, c, positive))
        }
        T::Dynamic | T::Void | T::Never | T::Null => false,
    }
}

/// The parameter of `g` that corresponds to the `i`-th one of `fid` (same position
/// among the positionals, or same name among the named ones).
fn corresponding(ctx: &Context, fid: usize, i: usize, g: usize) -> Option<usize> {
    let p = ctx.outline.functions.get(fid)?.parameters.get(i)?;
    let gp = &ctx.outline.functions.get(g)?.parameters;
    if p.kind == ParameterKind::Named {
        gp.iter()
            .position(|q| q.kind == ParameterKind::Named && q.external_name == p.external_name)
    } else {
        (gp.get(i)?.kind != ParameterKind::Named).then_some(i)
    }
}

/// The `j`-th parameter of `g` is covariant in `g` itself: written
/// `covariant`, or generic covariant of the class of `g`.
fn covariant_in(ctx: &Context, g: usize, j: usize) -> bool {
    let Some(q) = ctx
        .outline
        .functions
        .get(g)
        .and_then(|o| o.parameters.get(j))
    else {
        return false;
    };
    let name = (q.kind == ParameterKind::Named)
        .then_some(q.external_name)
        .flatten();
    let written = declared_covariant(ctx, g).iter().any(|(k, n)| match name {
        Some(x) => *n == Some(x),
        None => n.is_none() && *k == j,
    });
    written
        || ctx.program.functions[g]
            .class
            .is_some_and(|c| mentions_covariant(ctx, q.ty, c, true))
}

/// Parameter `i` of `fid` is covariant: in the member itself, or because the
/// corresponding one of a same-named member of some supertype of the class is
/// (covariance is inherited through overriding: `ParameterElement.isCovariant`
/// and the `isGenericCovariantImpl` that the CFE propagates).
fn inherited_covariant(ctx: &Context, fid: usize, i: usize) -> bool {
    if covariant_in(ctx, fid, i) {
        return true;
    }
    let f = &ctx.program.functions[fid];
    let Some(c) = f.class else { return false };
    let Some(data) = ctx.outline.hierarchy.get(c) else {
        return false;
    };
    let name = ctx.symbol_name(f.name);
    let key = if f.kind == dartforge_elements::model::FunctionKind::Setter && !name.ends_with("_=")
    {
        ctx.interner.lookup(&format!("{name}_="))
    } else {
        Some(f.name)
    };
    let Some(key) = key else { return false };
    data.supertypes.keys().any(|&s| {
        s != c
            && ctx.program.classes[s.0 as usize]
                .instance_members
                .get(&key)
                .is_some_and(|g| {
                    let g = g.0 as usize;
                    corresponding(ctx, fid, i, g).is_some_and(|j| covariant_in(ctx, g, j))
                })
    })
}

/// The `$c` entry checks parameter `ty` of `fid`
/// ([`FnBuilder::check_entry_arguments`]): a nominal type (class),
/// or a type that mentions a type parameter of the class in a covariant
/// position (`T`, `FutureOr<T>`, `T Function()`), outside the top. The
/// other function types and the type parameters of the function keep the
/// usual conversion.
pub fn checked_at_entry(ctx: &Context, fid: usize, ty: TypeId) -> bool {
    let f = &ctx.program.functions[fid];
    let generic = f
        .class
        .is_some_and(|c| mentions_covariant(ctx, ty, c, true));
    let nominal = matches!(ctx.table.get(ty), T::Interface { .. });
    !top(ctx, ty) && (generic || nominal)
}

/// Parameter `i` of `fid` needs a check even in a typed
/// call: it is covariant (see `inherited_covariant`).
pub fn covariant_parameter(ctx: &Context, fid: usize, i: usize) -> bool {
    inherited_covariant(ctx, fid, i)
}

/// Method `fid` gets the typed entry: its `$c` checks some
/// parameter that the typed call waives.
pub fn needs_typed_entry(ctx: &Context, fid: usize) -> bool {
    let Some(o) = ctx.outline.functions.get(fid) else {
        return false;
    };
    o.parameters
        .iter()
        .enumerate()
        .any(|(i, p)| checked_at_entry(ctx, fid, p.ty) && !covariant_parameter(ctx, fid, i))
}

/// The selector of the typed call corresponding to `selector` (`c:m` → `tc:m`).
pub fn typed_selector(selector: &str) -> String {
    format!("t{selector}")
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Arms the types of the parameters of invocation `expr_id` (target `target`)
    /// for the [`FnBuilder::evaluate_args`] of arguments `args`: the static type
    /// of the target, if it is a function type (that of the member seen through the
    /// receiver, or that of the function), otherwise that of the resolved constructor.
    /// Returns what was armed, for the caller to restore.
    pub(super) fn arm_arg_types(
        &mut self,
        target: ast::ExprId,
        expr_id: ast::ExprId,
        args: &[ast::Argument],
    ) -> Option<(usize, InvocationTypes)> {
        let types = self.target_types(target, expr_id);
        let key = args.as_ptr() as usize;
        std::mem::replace(&mut self.arg_types, types.map(|t| (key, t)))
    }

    /// Arms the types of the parameters of constructor `fid` of a creation whose
    /// static type is `typ` (the type arguments of the class replace its
    /// parameters).
    pub(super) fn arm_constructor_types(
        &mut self,
        fid: usize,
        typ: Option<TypeId>,
        args: &[ast::Argument],
    ) -> Option<(usize, InvocationTypes)> {
        let types = self.function_types(fid, typ);
        let key = args.as_ptr() as usize;
        std::mem::replace(&mut self.arg_types, types.map(|t| (key, t)))
    }

    fn target_types(&self, target: ast::ExprId, expr_id: ast::ExprId) -> Option<InvocationTypes> {
        if let Some(t) = self.ctx.get_type(self.unit_id, target)
            && let T::Function {
                type_params,
                positional,
                optional,
                named,
                ..
            } = self.ctx.table.get(t)
        {
            if !type_params.is_empty() {
                return None;
            }
            let without = |ty: &TypeId| ParameterType {
                ty: *ty,
                subst: None,
            };
            return Some(InvocationTypes {
                positionals: positional
                    .iter()
                    .chain(optional.iter())
                    .map(without)
                    .collect(),
                named: named.iter().map(|(n, ty, _)| (*n, without(ty))).collect(),
            });
        }
        let resolved = self
            .ctx
            .get_resolved(self.unit_id, expr_id)
            .or_else(|| self.ctx.get_resolved(self.unit_id, target))
            .cloned();
        let fid = match resolved {
            Some(Resolved::Constructor(f)) => f.0 as usize,
            Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => {
                f.0 as usize
            }
            Some(Resolved::Member {
                member: MemberRef::Function(f),
                ..
            }) => f.0 as usize,
            _ => return None,
        };
        let typ = self.ctx.get_type(self.unit_id, expr_id);
        self.function_types(fid, typ)
    }

    /// The types of the parameters of `fid`; in a constructor (or factory) of a
    /// generic class, with the type arguments of `created`.
    fn function_types(&self, fid: usize, created: Option<TypeId>) -> Option<InvocationTypes> {
        let o = self.ctx.outline.functions.get(fid)?;
        if !o.type_params.is_empty() {
            return None;
        }
        let f = &self.ctx.program.functions[fid];
        let mut subst = None;
        if let Some(c) = f.class
            && matches!(f.node, FunctionRef::Constructor { .. })
            && let Some(data) = self.ctx.outline.classes.get(c.0 as usize)
            && !data.type_params.is_empty()
            && let Some(t) = created
            && let T::Interface { class, args, .. } = self.ctx.table.get(t)
            && *class == c
            && args.len() == data.type_params.len()
        {
            subst = Some(
                data.type_params
                    .iter()
                    .copied()
                    .zip(args.iter().copied())
                    .collect::<HashMap<_, _>>(),
            );
        }
        let mut t = InvocationTypes::default();
        for p in o.parameters.iter() {
            let x = ParameterType {
                ty: p.ty,
                subst: subst.clone(),
            };
            if p.kind == ParameterKind::Named {
                if let Some(n) = p.external_name {
                    t.named.push((n, x));
                }
            } else {
                t.positionals.push(x);
            }
        }
        Some(t)
    }

    /// After evaluating argument `value` (expression `expr`, name
    /// `name`, `i_pos`-th positional): if its static type is
    /// `dynamic`, the implicit cast to the type of the parameter in the armed
    /// invocation; and the checked mark when the type becomes guaranteed.
    pub(super) fn implicit_argument_cast(
        &mut self,
        types: Option<&InvocationTypes>,
        expr: ast::ExprId,
        name: Option<SymbolId>,
        i_pos: usize,
        value: &Operand,
    ) {
        let Some(static_ty) = self.ctx.get_type(self.unit_id, expr) else {
            return;
        };
        if static_ty != self.ctx.core.dynamic_ {
            self.mark_checked(value);
            return;
        }
        let Some(p) = types.and_then(|t| t.of_argument(name, i_pos)).cloned() else {
            return;
        };
        if top(self.ctx, p.ty) {
            self.mark_checked(value);
        } else if closed(self.ctx, p.ty, p.subst.as_ref()) {
            let r = self.substituted_type_recipe(p.ty, p.subst.as_ref());
            let t = self.recipe_rti(&r);
            self.cast_rti_in(value.clone(), t, super::rti::CastContext::Implicit);
            self.mark_checked(value);
        }
    }

    /// The type of `value` is guaranteed by the static type (see `checked_args`).
    pub(super) fn mark_checked(&mut self, value: &Operand) {
        if let Operand::Val(id) = value {
            self.checked_args.insert(*id);
        }
    }

    /// `value`, the result of `expr`, has its type guaranteed by the static type
    /// when the expression is not `dynamic` (operands of operators and of
    /// indexing, which do not go through `evaluate_args`).
    pub(super) fn mark_if_typed(&mut self, expr: ast::ExprId, value: &Operand) {
        if self
            .ctx
            .get_type(self.unit_id, expr)
            .is_some_and(|t| t != self.ctx.core.dynamic_)
        {
            self.mark_checked(value);
        }
    }

    /// All arguments have a guaranteed type (constants or marked):
    /// the call may use the typed entry.
    pub(super) fn all_checked(&self, evaluated: &[Evaluated]) -> bool {
        // An unboxed scalar (`i64`, `double`, `bool`) already tells the type.
        evaluated.iter().all(|(_, v)| match v {
            Operand::Constant(_) => true,
            Operand::Val(id) => self.checked_args.contains(id) || self.operand_type(v) != Type::Ref,
        })
    }

    /// Before the direct call to the program member `target` (dispatch of
    /// `call_member`): each argument of a covariant parameter of `target`
    /// checked against the type declared on it — evaluated with the receiver
    /// `recv`, whose type arguments hold for those of the class —, with the
    /// VM message (" of 'name'"), before converting the argument to the
    /// representation of the parameter. It is the check the VM does at the entry
    /// of the method; here the call is direct, so it goes at the call
    /// site, per implementation.
    pub(super) fn check_target_covariants(
        &mut self,
        target: usize,
        recv: &Operand,
        evaluated: &[Evaluated],
    ) {
        self.check_target_parameters(target, recv, evaluated, true);
    }

    /// The value of a `dynamic` expression written through the setter (or the
    /// implicit setter of a field) `target` of `recv`: checked against the
    /// declared type of the parameter, as the CFE's implicit downcast.
    pub(super) fn check_setter_value(&mut self, target: usize, recv: &Operand, value: &Operand) {
        self.check_target_parameters(target, recv, &[(None, value.clone())], false);
    }

    /// See [`Self::check_target_covariants`]; with `only_covariant` false,
    /// every parameter whose type is not a top type.
    fn check_target_parameters(
        &mut self,
        target: usize,
        recv: &Operand,
        evaluated: &[Evaluated],
        only_covariant: bool,
    ) {
        if !super::user_function(self.ctx, target) {
            return;
        }
        let Some(o) = self.ctx.outline.functions.get(target) else {
            return;
        };
        if !o.type_params.is_empty() {
            return;
        }
        let params = o.parameters.clone();
        let mut positionals = evaluated
            .iter()
            .filter(|(n, _)| n.is_none())
            .map(|(_, v)| v.clone());
        for (i, p) in params.iter().enumerate() {
            let v = if p.kind == ParameterKind::Named {
                evaluated
                    .iter()
                    .find(|(n, _)| n.is_some() && *n == p.external_name)
                    .map(|(_, v)| v.clone())
            } else {
                positionals.next()
            };
            let Some(v) = v else { continue };
            if top(self.ctx, p.ty)
                || (only_covariant && !covariant_parameter(self.ctx, target, i))
                || self.is_terminated()
            {
                continue;
            }
            // An unboxed scalar in the representation of the parameter already belongs to it.
            let repr = self.repr(p.ty);
            if repr != Type::Ref && self.operand_type(&v) == repr {
                continue;
            }
            let r = self.coerce(recv.clone(), Type::Ref);
            let saved = (
                self.this_param.replace(r),
                std::mem::replace(
                    &mut self.enclosing_class,
                    self.ctx.program.functions[target].class,
                ),
                std::mem::replace(&mut self.class_by_tuple, false),
                self.type_tuple.take(),
            );
            let typ = self.type_rti(p.ty);
            (
                self.this_param,
                self.enclosing_class,
                self.class_by_tuple,
                self.type_tuple,
            ) = saved;
            let name = p
                .name
                .map(|n| self.ctx.symbol_name(n).to_string())
                .unwrap_or_default();
            let v = self.coerce(v, Type::Ref);
            self.cast_rti_in(v, typ, super::rti::CastContext::Parameter(name));
        }
    }

    /// The implicit downcast that the CFE inserts where a `dynamic` value (the
    /// static type of `e`) meets a context of type `target`: an assignment to a
    /// variable, a `return`, an initializer.
    pub(super) fn implicit_downcast(&mut self, e: ast::ExprId, value: &Operand, target: TypeId) {
        if self.is_terminated() || top(self.ctx, target) || !self.dynamic_expression(e) {
            return;
        }
        let rti = self.type_rti(target);
        let v = self.coerce(value.clone(), Type::Ref);
        self.cast_rti_in(v, rti, super::rti::CastContext::Implicit);
    }

    /// The static type to check the value of `e` against when `e` reads a
    /// member (a field, a getter, a method's result or tear-off) whose
    /// declared type uses a type parameter of its class in a contravariant
    /// position, through a receiver other than `this`: the value may not have
    /// the static type (`C<Object> c = C<String>(); c.f` with `void
    /// Function(T) f`), and the CFE checks it.
    pub(super) fn contravariant_member_access(
        &self,
        ast: &ast::Ast,
        e: ast::ExprId,
    ) -> Option<TypeId> {
        let (receiver, access) = match &ast.expr(e).kind {
            ast::ExprKind::Property { target, .. } => (*target, e),
            ast::ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
                ast::ExprKind::Property { target: r, .. } => (*r, *target),
                _ => return None,
            },
            _ => return None,
        };
        if matches!(
            ast.expr(receiver).kind,
            ast::ExprKind::This | ast::ExprKind::Super
        ) {
            return None;
        }
        let member = [access, e].into_iter().find_map(|x| {
            match self.ctx.get_resolved(self.unit_id, x)? {
                Resolved::Member {
                    member,
                    via_super: false,
                    ..
                } => Some(*member),
                _ => None,
            }
        })?;
        let (class, ty) = match member {
            MemberRef::Variable(v) => {
                if access != e {
                    return None;
                }
                let var = &self.ctx.program.variables[v.0 as usize];
                (var.class?, super::members::variable_type(self.ctx, v))
            }
            MemberRef::Function(f) => {
                let func = &self.ctx.program.functions[f.0 as usize];
                if func.static_ {
                    return None;
                }
                let o = self.ctx.outline.functions.get(f.0 as usize)?;
                // A getter's value, a method's result, or the return type
                // in a method's tear-off. The call of a getter's value checks
                // the value (`check_contravariant_value`), not the result.
                if access != e && func.kind != dartforge_elements::model::FunctionKind::Function {
                    return None;
                }
                let ty = match func.kind {
                    dartforge_elements::model::FunctionKind::Getter
                    | dartforge_elements::model::FunctionKind::ImplicitAccessor
                    | dartforge_elements::model::FunctionKind::Function => o.return_type,
                    _ => return None,
                };
                (func.class?, ty)
            }
        };
        if !mentions_covariant(self.ctx, ty, class, false) {
            return None;
        }
        let t = self.ctx.get_type(self.unit_id, e)?;
        (!top(self.ctx, t)).then_some(t)
    }

    /// The type arguments of a call to the generic method `fid` through the
    /// receiver `recv` (the tuple `arm_tuple` prepared), when a bound of its
    /// type parameters uses a type parameter of the class (`foo<E extends
    /// T>()`): checked against the bounds with the receiver's type arguments,
    /// as the VM's method entry does — the static type of the receiver may be
    /// a supertype (`C<Object?>` holding a `C<num>`).
    pub(super) fn check_covariant_bounds(&mut self, fid: usize, recv: &Operand) {
        let Some(class) = self.ctx.program.functions[fid].class else {
            return;
        };
        let Some(o) = self.ctx.outline.functions.get(fid) else {
            return;
        };
        let params: Vec<TypeParamId> = o.type_params.to_vec();
        let covariant = params.iter().any(|&p| {
            let bound = self.ctx.table.param(p).bound;
            mentions_covariant(self.ctx, bound, class, true)
                || mentions_covariant(self.ctx, bound, class, false)
        });
        let Some(tuple) = self.armed_tuple.clone() else {
            return;
        };
        if !covariant || self.is_terminated() {
            return;
        }
        let recv = self.coerce(recv.clone(), Type::Ref);
        let saved = std::mem::replace(&mut self.class_by_tuple, false);
        self.with_member_type_context(fid, Some(recv), Some(tuple), |b| {
            b.check_type_argument_bounds(&params)
        });
        self.class_by_tuple = saved;
    }

    /// The covariance check of [`Self::contravariant_member_access`] on the
    /// value `v` that `e` read.
    pub(super) fn check_contravariant_value(
        &mut self,
        ast: &ast::Ast,
        e: ast::ExprId,
        v: &Operand,
    ) {
        if let Some(t) = self.contravariant_member_access(ast, e)
            && !self.is_terminated()
        {
            let rti = self.type_rti(t);
            let r = self.coerce(v.clone(), Type::Ref);
            self.cast_rti_in(r, rti, super::rti::CastContext::Implicit);
        }
    }

    /// The static type of `e` is `dynamic`.
    pub(super) fn dynamic_expression(&self, e: ast::ExprId) -> bool {
        self.ctx.get_type(self.unit_id, e) == Some(self.ctx.core.dynamic_)
    }

    /// The value of `return e`, against the declared return type (the future
    /// value type of an `async` body).
    pub(super) fn return_downcast(&mut self, e: ast::ExprId, value: &Operand) {
        if let Some(t) = self.return_context {
            self.implicit_downcast(e, value, t);
        }
    }

    /// Member `name` in the interface of class `c` (declared in it or in a
    /// supertype).
    pub(super) fn interface_member(&self, c: ClassId, name: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(name)?;
        if let Some(f) = self.ctx.program.classes[c.0 as usize]
            .instance_members
            .get(&sym)
        {
            return Some(f.0 as usize);
        }
        let data = self.ctx.outline.hierarchy.get(c)?;
        let mut supers: Vec<ClassId> = data
            .supertypes
            .keys()
            .copied()
            .filter(|s| *s != c)
            .collect();
        supers.sort_by_key(|s| s.0);
        supers.into_iter().find_map(|s| {
            self.ctx.program.classes[s.0 as usize]
                .instance_members
                .get(&sym)
                .map(|f| f.0 as usize)
        })
    }

    /// Argument `value` (expression `arg`) of the `i`-th parameter of
    /// operator `name` (`[]`, `[]=`, `+`…) on receiver `receiver`:
    /// if it is `dynamic`, the implicit cast to the type of the parameter seen
    /// through the static type of the receiver (its type arguments replaced
    /// by the parameters of the class that declares the operator), like the CFE; and the
    /// checked mark when the type becomes guaranteed.
    pub(super) fn implicit_operator_cast(
        &mut self,
        arg: ast::ExprId,
        value: &Operand,
        receiver: ast::ExprId,
        name: &str,
        i: usize,
    ) {
        let Some(static_ty) = self.ctx.get_type(self.unit_id, arg) else {
            return;
        };
        if static_ty != self.ctx.core.dynamic_ {
            self.mark_checked(value);
            return;
        }
        let Some(rt) = self.ctx.get_type(self.unit_id, receiver) else {
            return;
        };
        let T::Interface {
            class: rc,
            args: rargs,
            ..
        } = self.ctx.table.get(rt).clone()
        else {
            return;
        };
        let Some(fid) = self.interface_member(rc, name) else {
            return;
        };
        let Some(p) = self
            .ctx
            .outline
            .functions
            .get(fid)
            .and_then(|o| o.parameters.get(i))
            .cloned()
        else {
            return;
        };
        if top(self.ctx, p.ty) {
            self.mark_checked(value);
            return;
        }
        let mut subst: HashMap<TypeParamId, TypeId> = HashMap::new();
        if let Some(data) = self.ctx.outline.classes.get(rc.0 as usize) {
            subst.extend(data.type_params.iter().copied().zip(rargs.iter().copied()));
        }
        if let Some(dc) = self.ctx.program.functions[fid].class
            && dc != rc
            && let Some(data) = self.ctx.outline.classes.get(dc.0 as usize)
            && let Some(&sup) = self
                .ctx
                .outline
                .hierarchy
                .get(rc)
                .and_then(|h| h.supertypes.get(&dc))
            && let T::Interface { args: sargs, .. } = self.ctx.table.get(sup)
        {
            subst.extend(data.type_params.iter().copied().zip(sargs.iter().copied()));
        }
        if !closed(self.ctx, p.ty, Some(&subst)) {
            return;
        }
        let r = self.substituted_type_recipe(p.ty, Some(&subst));
        let t = self.recipe_rti(&r);
        self.cast_rti_in(value.clone(), t, super::rti::CastContext::Implicit);
        self.mark_checked(value);
    }

    /// The single implementation of instance member `fid` of an SDK class
    /// whose member is closed (only its library implements it), with
    /// the same parameters; `None` without it.
    pub(super) fn single_implementation(&self, fid: usize) -> Option<usize> {
        let f = &self.ctx.program.functions[fid];
        let cid = f.class?;
        if f.static_
            || !self
                .ctx
                .program
                .library(self.ctx.program.classes[cid.0 as usize].library)
                .is_sdk
        {
            return None;
        }
        let name = self.ctx.symbol_name(f.name).to_string();
        if !super::sdk_source::closed_member(self.ctx, cid, &name) {
            return None;
        }
        let [super::sdk_source::Implementation::Function(g)] =
            super::sdk_source::implementations(self.ctx, cid, &name)[..]
        else {
            return None;
        };
        let n = |x: usize| {
            self.ctx
                .outline
                .functions
                .get(x)
                .map(|o| o.parameters.len())
        };
        (n(g) == n(fid)).then_some(g)
    }
}
