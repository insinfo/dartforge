//! `TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` for written types: the analyzer 7.x
//! `TypeArgumentsVerifier._checkForTypeArgumentNotMatchingBounds`
//! (`src/error/type_arguments_verifier.dart`).
//!
//! For each written `C<A1, …, An>` (class or type alias), each argument
//! must be a subtype of the parameter's bound with the arguments
//! substituted (*regular-bounded*). If it is not, and the place accepts a
//! *super-bounded* type (not `extends`/`with`/`implements`/`on`, instance
//! creation, a `typedef` body or an extension type), the inverted type
//! (`invertido`: top replaced by `Never` in non-contravariant positions,
//! bottom by `Object?` in contravariant ones) is checked the same way, and
//! the diagnostic shows the inverted arguments and bounds.
//!
//! The type of each `NamedType` is the resolved one (`NamedType.type`): the
//! sketch's `written_types` or the bodies' `annotation_types`, with raw
//! types instantiated to bounds. The named types that the analyzer creates
//! from expressions (type literal `C<T>`, creation `C<T>()` without `new`,
//! constructor reference `C<T>.new`) come in through the expression tree. A
//! node that no resolution recorded falls back to resolution in the
//! library scope, which skips type parameter names.

use crate::resolve::OutlineTypes;
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{Diagnostic, Span, codes::compile_time_error as c};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, ParameterKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Variance of a position (the analyzer's `Variance`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variance_ {
    None_,
    Co,
    Contra,
    In,
}

impl Variance_ {
    /// `Variance.combine`: the sign of a position inside another.
    fn combine(self, other: Variance_) -> Variance_ {
        use Variance_::*;
        match (self, other) {
            (None_, _) | (_, None_) => None_,
            (In, _) | (_, In) => In,
            (a, b) if a == b => Co,
            _ => Contra,
        }
    }

    /// `Variance.meet`: two occurrences of the same parameter.
    fn join(self, other: Variance_) -> Variance_ {
        use Variance_::*;
        match (self, other) {
            (None_, x) | (x, None_) => x,
            (a, b) if a == b => a,
            _ => In,
        }
    }
}

/// The element of a named type and its arguments.
struct Parts {
    name: String,
    params: Vec<TypeParamId>,
    types: Vec<TypeId>,
    of_extension: bool,
    /// The alias target (the variances of the parameters in the inversion).
    alias_target: Option<TypeId>,
}

struct Verifier<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    unit: UnitId,
    /// Names of type parameters declared somewhere in the unit.
    parameter_names: HashSet<SymbolId>,
    /// The resolved types of the unit's bodies.
    body: Option<&'a crate::resolved::UnitBodyTypes>,
}

/// The `type_argument_not_matching_bounds` of the written types in a unit.
pub fn arguments_out_of_bounds(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: Option<&crate::resolved::UnitBodyTypes>,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verifier {
        program,
        interner,
        table,
        core,
        outline,
        unit,
        parameter_names: HashSet::new(),
        body,
    };
    v.parameter_names = parameter_names(a, &program.unit(unit).unit);
    let without_super = without_super_bound(a, &program.unit(unit).unit);
    // A creation without written arguments is checked with the inferred
    // ones (`inferred_arguments_out_of_bounds`).
    let raw_creations: HashSet<ast::TypeId> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::InstanceCreation { ty, .. } if matches!(&a.ty(*ty).kind, TypeKind::Named { args, .. } if args.is_empty()) => Some(*ty),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (i, t) in a.types.iter().enumerate() {
        if let TypeKind::Named { name, args } = &t.kind {
            let id = ast::TypeId(i as u32);
            if t.span.start == t.span.end || raw_creations.contains(&id) {
                continue;
            }
            let super_allowed = !without_super.contains(&id);
            match v.registered(id) {
                Some(r) => v.check_type(r, t.span, args, super_allowed, &mut out),
                None if !args.is_empty() => v.check(t.span, name, args, super_allowed, &mut out),
                None => {}
            }
        }
    }
    if body.is_some() {
        v.expression_named_types(&mut out);
    }
    out
}

/// `ReplaceTopBottomVisitor.process` over an already resolved type: top
/// becomes `Never` outside contravariant positions; whatever is a subtype of
/// `Never` becomes `Object?` in contravariant ones. An alias carries each
/// argument with the variance of its parameter in the target combined with
/// the outer one; classes (and `FutureOr`, extension types) carry the
/// variance forward; functions invert in the parameters (type parameters
/// stay).
fn swap_top_and_bottom(v: &mut Verifier<'_>, t: TypeId, va: Variance_) -> TypeId {
    if va == Variance_::Contra {
        let never = v.core.never;
        if v.sub(t, never) {
            return v.core.object_nullable;
        }
    } else if v.the_is_top(t) {
        return v.core.never;
    }
    let is_nullable = v.table.get(t).is_declared_nullable();
    if let Some(crate::table::DisplayDecoration::Alias { typedef, args }) =
        v.table.decoration(t).cloned()
    {
        let d = &v.outline.typedefs[typedef.0 as usize];
        let (params, tgt) = (d.type_params.to_vec(), d.target_type);
        if params.len() == args.len() {
            let mut new_ = Vec::with_capacity(args.len());
            for (&x, &p) in args.iter().zip(params.iter()) {
                let vp = variance_in(v.table, tgt, p, Variance_::Co);
                new_.push(swap_top_and_bottom(v, x, vp.combine(va)));
            }
            let r = v.subst(tgt, &params, &new_);
            let r = if is_nullable {
                crate::ops::nullable(r, v.table)
            } else {
                r
            };
            return v.table.decorate(
                r,
                crate::table::DisplayDecoration::Alias {
                    typedef,
                    args: new_.into_boxed_slice(),
                },
            );
        }
    }
    match v.table.get(t).clone() {
        Type::Interface {
            class,
            args,
            nullable,
        } if !args.is_empty() => {
            let new_: Vec<TypeId> = args
                .iter()
                .map(|&x| swap_top_and_bottom(v, x, va))
                .collect();
            v.table.intern(Type::Interface {
                class,
                args: new_.into_boxed_slice(),
                nullable,
            })
        }
        Type::ExtensionType {
            decl,
            args,
            nullable,
        } if !args.is_empty() => {
            let new_: Vec<TypeId> = args
                .iter()
                .map(|&x| swap_top_and_bottom(v, x, va))
                .collect();
            v.table.intern(Type::ExtensionType {
                decl,
                args: new_.into_boxed_slice(),
                nullable,
            })
        }
        Type::FutureOr { arg, nullable } => {
            let arg = swap_top_and_bottom(v, arg, va);
            v.table.intern(Type::FutureOr { arg, nullable })
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } => {
            let ret = swap_top_and_bottom(v, ret, va);
            let against = va.combine(Variance_::Contra);
            let positional: Vec<TypeId> = positional
                .iter()
                .map(|&x| swap_top_and_bottom(v, x, against))
                .collect();
            let optional: Vec<TypeId> = optional
                .iter()
                .map(|&x| swap_top_and_bottom(v, x, against))
                .collect();
            let named: Vec<(SymbolId, TypeId, bool)> = named
                .iter()
                .map(|&(n, x, r)| (n, swap_top_and_bottom(v, x, against), r))
                .collect();
            v.table.intern(Type::Function {
                type_params,
                ret,
                positional: positional.into_boxed_slice(),
                optional: optional.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable,
            })
        }
        _ => t,
    }
}

/// `enum_instantiated_to_bounds_is_not_well_bounded`
/// (`ErrorVerifier._checkForEnumInstantiatedToBoundsIsNotWellBounded`,
/// `analyzer/lib/src/generated/error_verifier.dart:3239-3256`): the type
/// argument of the `values` field (`List<E<instantiated to bounds>>`) goes
/// through `isWellBounded(…, allowSuperBounded: true)`, which only checks the
/// inverted one (`_isSuperBounded`: the regular result is discarded). At the
/// enum name.
pub fn enum_instantiated_to_bounds(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: dartforge_elements::model::LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for (i, ce) in program.classes.iter().enumerate() {
        if ce.library != lib || ce.kind != dartforge_elements::model::ClassKind::Enum {
            continue;
        }
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let decl = a.decl(d.decl);
        let DeclKind::Enum(x) = &decl.kind else {
            continue;
        };
        if decl.augment {
            continue;
        }
        let params = outline.classes[i].type_params.to_vec();
        if params.is_empty() {
            continue;
        }
        let args = crate::ops::instantiate_to_bounds(&params, &[], table, core);
        let mut v = Verifier {
            program,
            interner,
            table: &mut *table,
            core,
            outline,
            unit: d.unit,
            parameter_names: HashSet::new(),
            body: None,
        };
        let inverted: Vec<TypeId> = args
            .iter()
            .map(|&t| swap_top_and_bottom(&mut v, t, Variance_::Co))
            .collect();
        let mut well_bounded = true;
        for (k, &p) in params.iter().enumerate() {
            let data = v.table.param(p).clone();
            if !data.explicit {
                continue;
            }
            let bnd = v.subst(data.bound, &params, &inverted);
            if !v.sub(inverted[k], bnd) {
                well_bounded = false;
                break;
            }
        }
        if !well_bounded {
            output.push((
                d.unit,
                Diagnostic::with_code(
                    c::ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED,
                    x.name.span,
                    Vec::<String>::new(),
                ),
            ));
        }
    }
    output
}

/// The `type_argument_not_matching_bounds` of the **inferred** type arguments
/// of an instance creation without written arguments (`C(x)`,
/// `new C.the_name(x)`): the analyzer checks the constructor's `NamedType` with
/// the already inferred type (`checkNamedType`,
/// `an611:src/error/type_arguments_verifier.dart`), at the class name.
/// Never super-bounded (instance creation).
pub fn inferred_arguments_out_of_bounds(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &crate::resolved::UnitBodyTypes,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verifier {
        program,
        interner,
        table,
        core,
        outline,
        unit,
        parameter_names: HashSet::new(),
        body: Some(body),
    };
    let mut out = Vec::new();
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        let (span, args) = match &e.kind {
            ExprKind::InstanceCreation { ty, arguments, .. } => {
                let TypeKind::Named {
                    args: written_ones, ..
                } = &a.ty(*ty).kind
                else {
                    continue;
                };
                if !written_ones.is_empty() {
                    continue;
                }
                (a.ty(*ty).span, arguments)
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() {
                    continue;
                }
                let sp = match &a.expr(*target).kind {
                    ExprKind::Identifier(_) => a.expr(*target).span,
                    // `C.the_name(x)`: the class name; `p.C(x)`: the whole name.
                    ExprKind::Property { target: t, .. } => match &a.expr(*t).kind {
                        ExprKind::Identifier(_) => a.expr(*t).span,
                        _ => continue,
                    },
                    _ => continue,
                };
                (sp, arguments)
            }
            _ => continue,
        };
        let Some(crate::resolved::Resolved::Constructor(f)) = body.get_resolved(id) else {
            continue;
        };
        let Some(c) = program.function(*f).class else {
            continue;
        };
        let params = outline.classes[c.0 as usize].type_params.to_vec();
        if params.is_empty() {
            continue;
        }
        let Some(types) = body.instantiation(args.span.start).map(|x| x.to_vec()) else {
            continue;
        };
        if types.len() != params.len() {
            continue;
        }
        for (i, &p) in params.iter().enumerate() {
            let bnd = v.table.param(p).bound;
            let bnd = v.subst(bnd, &params, &types);
            if !v.sub(types[i], bnd) {
                let ta = v.format(types[i]);
                let the_name = interner.resolve(v.table.param(p).name).to_string();
                let l = v.format(bnd);
                out.push(Diagnostic::with_code(
                    c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS,
                    span,
                    [ta.as_str(), the_name.as_str(), l.as_str()],
                ));
            }
        }
    }
    out
}

impl Verifier<'_> {
    fn ast(&self) -> &ast::Ast {
        &self.program.unit(self.unit).ast
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    fn the_is_top(&mut self, t: TypeId) -> bool {
        let env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        crate::bounds::is_top(t, &env)
    }

    fn subst(&mut self, t: TypeId, params: &[TypeParamId], args: &[TypeId]) -> TypeId {
        let map: HashMap<TypeParamId, TypeId> =
            params.iter().copied().zip(args.iter().copied()).collect();
        crate::ops::substitute(t, &map, self.table)
    }

    fn format(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// The type the resolution recorded for the node (`NamedType.type`).
    fn registered(&self, t: ast::TypeId) -> Option<TypeId> {
        self.outline
            .written_types
            .get(&(self.unit, t))
            .or_else(|| self.body.and_then(|c| c.annotation_types.get(&t)))
            .copied()
    }

    /// The type of a written argument: the recorded one, or the one resolved
    /// here.
    fn node_type(&mut self, t: ast::TypeId) -> Option<TypeId> {
        match self.registered(t) {
            Some(r) => Some(r),
            None => self.resolver(t),
        }
    }

    /// `_checkForTypeArgumentNotMatchingBounds` over the resolved type `t` of a
    /// `NamedType` with range `typ`, with the written arguments `written_ones`
    /// (empty: raw type).
    fn check_type(
        &mut self,
        t: TypeId,
        typ: Span,
        written_ones: &[ast::TypeId],
        super_allowed: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        if self.table.is_invalid(t) {
            return;
        }
        let Some(parts) = self.parts(t) else { return };
        self.check_parts(t, parts, typ, written_ones, super_allowed, out);
    }

    /// The same, with the element and the arguments already known (the type
    /// built here, which without the display decoration does not keep the
    /// alias).
    fn check_parts(
        &mut self,
        t: TypeId,
        parts: Parts,
        typ: Span,
        written_ones: &[ast::TypeId],
        super_allowed: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        let Parts {
            name: the_name,
            params,
            types,
            of_extension,
            alias_target,
        } = parts;
        if params.is_empty() || params.len() != types.len() {
            return;
        }
        let mut problems = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            let data = self.table.param(p).clone();
            if !data.explicit {
                continue;
            }
            let bnd = self.subst(data.bound, &params, &types);
            if !self.sub(types[i], bnd) {
                problems.push((i, types[i], bnd));
            }
        }
        if problems.is_empty() {
            return;
        }
        let list = |v: &mut Self, xs: &[TypeId]| {
            xs.iter()
                .map(|&x| v.format(x))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut context: Vec<String> = Vec::new();
        if written_ones.is_empty() {
            context.push(format!(
                "The raw type was instantiated as '{the_name}<{}>', and is not regular-bounded.",
                list(self, &types)
            ));
        }
        let error_node = |i: usize| written_ones.get(i).copied();
        if !super_allowed || of_extension {
            for (i, arg, bnd) in problems {
                self.report_at(error_node(i), typ, (params[i], arg, bnd), &context, out);
            }
            return;
        }
        // `replaceTopAndBottom(type)`: a whole type that is top becomes `Never`
        // (neither alias nor interface); otherwise each argument by the
        // variance of the parameter in the alias target (in a class, covariant).
        if self.the_is_top(t) {
            return;
        }
        let mut inv = Vec::with_capacity(types.len());
        for (&x, &p) in types.iter().zip(params.iter()) {
            let va = match alias_target {
                Some(tgt) => variance_in(self.table, tgt, p, Variance_::Co).combine(Variance_::Co),
                None => Variance_::Co,
            };
            inv.push(swap_top_and_bottom(self, x, va));
        }
        context.push(format!("The inverted type '{the_name}<{}>' is also not regular-bounded, so the type is not well-bounded.", list(self, &inv)));
        for (i, &p) in params.iter().enumerate() {
            let data = self.table.param(p).clone();
            if !data.explicit {
                continue;
            }
            let bnd = self.subst(data.bound, &params, &inv);
            if !self.sub(inv[i], bnd) {
                self.report_at(error_node(i), typ, (p, inv[i], bnd), &context, out);
            }
        }
    }

    /// The element name, the parameters and the type arguments of `t`
    /// (`type.alias` before `InterfaceType`), and whether it is an extension
    /// type.
    fn parts(&self, t: TypeId) -> Option<Parts> {
        if let Some(crate::table::DisplayDecoration::Alias { typedef, args }) =
            self.table.decoration(t)
        {
            return Some(self.alias_parts(*typedef, args.to_vec(), t));
        }
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some(self.class_parts(*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some(self.class_parts(*decl, args.to_vec())),
            // `FutureOr<T>`: `T` has no bound.
            _ => None,
        }
    }

    fn alias_parts(
        &self,
        typedef: dartforge_elements::model::TypedefId,
        types: Vec<TypeId>,
        t: TypeId,
    ) -> Parts {
        let d = &self.outline.typedefs[typedef.0 as usize];
        Parts {
            name: self
                .interner
                .resolve(self.program.typedef(typedef).name)
                .to_string(),
            params: d.type_params.to_vec(),
            types,
            of_extension: matches!(self.table.get(t), Type::ExtensionType { .. }),
            alias_target: Some(d.target_type),
        }
    }

    fn class_parts(&self, c: dartforge_elements::model::ClassId, types: Vec<TypeId>) -> Parts {
        Parts {
            name: self
                .interner
                .resolve(self.program.class(c).name)
                .to_string(),
            params: self.outline.classes[c.0 as usize].type_params.to_vec(),
            types,
            of_extension: self.program.class(c).kind
                == dartforge_elements::model::ClassKind::ExtensionType,
            alias_target: None,
        }
    }

    fn report_at(
        &mut self,
        no: Option<ast::TypeId>,
        typ: Span,
        (p, arg, bnd): (TypeParamId, TypeId, TypeId),
        context: &[String],
        out: &mut Vec<Diagnostic>,
    ) {
        let span = no.map(|x| self.ast().ty(x).span).unwrap_or(typ);
        let a = self.format(arg);
        let the_name = self.interner.resolve(self.table.param(p).name).to_string();
        let l = self.format(bnd);
        let mut d = Diagnostic::with_code(
            c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS,
            span,
            [a.as_str(), the_name.as_str(), l.as_str()],
        );
        for m in context {
            d.context.push(dartforge_diagnostics::ContextMessage {
                file: None,
                span: typ,
                message: m.as_str().into(),
            });
        }
        out.push(d);
    }

    /// The `NamedType`s the analyzer creates from expressions: the type literal
    /// `C<T>` (accepts *super-bounded*), the constructor reference `C<T>.the_name`
    /// and the creation `C<T>()` without `new` (`ConstructorName`: does not
    /// accept).
    fn expression_named_types(&mut self, out: &mut Vec<Diagnostic>) {
        let Some(body) = self.body else { return };
        let a = &self.program.unit(self.unit).ast;
        // The `C<T>` that are the target of `.the_name`.
        let mut of_constructor: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Property { target, .. } = &e.kind
                && matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. })
            {
                of_constructor.insert(*target);
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let (tgt, written_ones, super_allowed, span) = match &e.kind {
                ExprKind::TypeArguments { target, type_args } => {
                    let constructor = of_constructor.contains(&id);
                    // The literal has type `Type`; the function reference does not.
                    if !constructor && body.get_type(id) != Some(self.core.type_) {
                        continue;
                    }
                    (*target, type_args.to_vec(), !constructor, e.span)
                }
                ExprKind::Call { target, arguments } if !arguments.type_args.is_empty() => {
                    if !matches!(
                        body.get_resolved(id),
                        Some(crate::resolved::Resolved::Constructor(_))
                    ) {
                        continue;
                    }
                    let end = a.ty(*arguments.type_args.last().unwrap()).span.end;
                    let source = self.program.unit(self.unit).source.as_str();
                    let end = source
                        .get(end..)
                        .and_then(|r| r.find('>'))
                        .map_or(end, |k| end + k + 1);
                    (
                        *target,
                        arguments.type_args.to_vec(),
                        false,
                        Span {
                            start: a.expr(*target).span.start,
                            end,
                        },
                    )
                }
                _ => continue,
            };
            let el = match &a.expr(tgt).kind {
                ExprKind::Identifier(n) => self
                    .program
                    .lookup_in_unit(self.unit, n.sym)
                    .filter(|b| !b.ambiguous)
                    .and_then(|b| b.getter),
                ExprKind::Property {
                    target: p, name, ..
                } => match &a.expr(*p).kind {
                    ExprKind::Identifier(p)
                        if self
                            .program
                            .prefixes_in_unit(self.unit)
                            .contains_key(&p.sym) =>
                    {
                        self.program
                            .lookup_prefixed_in_unit(self.unit, p.sym, name.sym)
                            .filter(|b| !b.ambiguous)
                            .and_then(|b| b.getter)
                    }
                    _ => None,
                },
                _ => None,
            };
            // The name resolved in the body must be the same element.
            if let Some(crate::resolved::Resolved::Element(r)) = body.get_resolved(tgt)
                && Some(*r) != el
            {
                continue;
            }
            let Some(el) = el else { continue };
            let Some((params, alias_target)) = self.parameters(el) else {
                continue;
            };
            if params.len() != written_ones.len() || params.is_empty() {
                continue;
            }
            let Some(types) = written_ones
                .iter()
                .map(|&x| self.node_type(x))
                .collect::<Option<Vec<TypeId>>>()
            else {
                continue;
            };
            let Some((t, parts)) = self.assemble(el, alias_target, &params, types) else {
                continue;
            };
            self.check_parts(t, parts, span, &written_ones, super_allowed, out);
        }
    }

    /// The element that the written name designates in the unit's scope.
    fn element(&self, name: &[ast::Name]) -> Option<Element> {
        if name.iter().any(|n| self.parameter_names.contains(&n.sym)) {
            return None;
        }
        let b = match name {
            [n] => self.program.lookup_in_unit(self.unit, n.sym),
            [p, n] => self
                .program
                .lookup_prefixed_in_unit(self.unit, p.sym, n.sym),
            _ => None,
        }?;
        if b.ambiguous {
            return None;
        }
        b.getter
    }

    /// Type parameters (and the target, if an alias) of the generic element.
    fn parameters(&self, el: Element) -> Option<(Vec<TypeParamId>, Option<TypeId>)> {
        match el {
            Element::Class(c) => Some((
                self.outline.classes[c.0 as usize].type_params.to_vec(),
                None,
            )),
            Element::Typedef(t) => {
                // The alias that reaches itself instantiates as `dynamic`: no bounds
                // check (`self_reference`).
                if crate::self_reference::self_referencing_typedef(
                    self.program,
                    self.program.typedef(t).decl,
                ) {
                    return None;
                }
                let d = &self.outline.typedefs[t.0 as usize];
                Some((d.type_params.to_vec(), Some(d.target_type)))
            }
            _ => None,
        }
    }

    /// A `NamedType` with written arguments that no resolution recorded: the
    /// element through the unit's scope (`Type.constructor` by the first name)
    /// and the arguments recorded or resolved here.
    fn check(
        &mut self,
        typ: Span,
        name: &[ast::Name],
        args: &[ast::TypeId],
        super_allowed: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        let el = match self.element(name) {
            Some(el) => Some(el),
            None if name.len() == 2 => self.element(&name[..1]),
            None => None,
        };
        let Some(el) = el else { return };
        let Some((params, tgt)) = self.parameters(el) else {
            return;
        };
        if params.len() != args.len() || params.is_empty() {
            return;
        }
        let Some(types) = args
            .iter()
            .map(|&x| self.node_type(x))
            .collect::<Option<Vec<TypeId>>>()
        else {
            return;
        };
        let Some((t, parts)) = self.assemble(el, tgt, &params, types) else {
            return;
        };
        self.check_parts(t, parts, typ, args, super_allowed, out);
    }

    /// The type `el<types>` and its parts.
    fn assemble(
        &mut self,
        el: Element,
        tgt: Option<TypeId>,
        params: &[TypeParamId],
        types: Vec<TypeId>,
    ) -> Option<(TypeId, Parts)> {
        match (el, tgt) {
            (Element::Typedef(td), Some(tgt)) => {
                let r = self.subst(tgt, params, &types);
                let t = self.table.decorate(
                    r,
                    crate::table::DisplayDecoration::Alias {
                        typedef: td,
                        args: types.clone().into_boxed_slice(),
                    },
                );
                let parts = self.alias_parts(td, types, t);
                Some((t, parts))
            }
            (Element::Class(c), _) => {
                let t = if self.program.class(c).kind
                    == dartforge_elements::model::ClassKind::ExtensionType
                {
                    self.table.intern(Type::ExtensionType {
                        decl: c,
                        args: types.clone().into_boxed_slice(),
                        nullable: false,
                    })
                } else {
                    self.table.intern(Type::Interface {
                        class: c,
                        args: types.clone().into_boxed_slice(),
                        nullable: false,
                    })
                };
                Some((t, self.class_parts(c, types)))
            }
            _ => None,
        }
    }

    /// The written type, resolved in the library scope; `None` if it depends on
    /// something that cannot be resolved here with certainty.
    fn resolver(&mut self, t: ast::TypeId) -> Option<TypeId> {
        let no = self.ast().ty(t);
        let is_nullable = no.nullable;
        let r = match &no.kind {
            TypeKind::Void => self.core.void_,
            TypeKind::Named { name, args } => {
                let (name, args) = (name.clone(), args.clone());
                if let [n] = &name[..] {
                    match self.interner.resolve(n.sym) {
                        "dynamic" if args.is_empty() => return Some(self.core.dynamic_),
                        "Never" if args.is_empty() => {
                            return Some(if is_nullable {
                                self.table.decorate(
                                    self.core.null,
                                    crate::table::DisplayDecoration::NeverNullable,
                                )
                            } else {
                                self.core.never
                            });
                        }
                        _ => {}
                    }
                }
                let el = self.element(&name)?;
                let types = args
                    .iter()
                    .map(|&x| self.resolver(x))
                    .collect::<Option<Vec<TypeId>>>()?;
                match el {
                    Element::Class(cid) => {
                        let cl = self.program.class(cid);
                        let the_name = self.interner.resolve(cl.name);
                        if Some(cid) == self.core.null_class {
                            self.core.null
                        } else if the_name == "FutureOr" && self.program.library(cl.library).is_sdk
                        {
                            let [arg] = types[..] else { return None };
                            self.table.intern(Type::FutureOr {
                                arg,
                                nullable: false,
                            })
                        } else {
                            let params = self.outline.classes[cid.0 as usize].type_params.clone();
                            if params.len() != types.len() {
                                // Raw and generic (instantiated to bounds): skipped.
                                return None;
                            }
                            if cl.kind == dartforge_elements::model::ClassKind::ExtensionType {
                                self.table.intern(Type::ExtensionType {
                                    decl: cid,
                                    args: types.into_boxed_slice(),
                                    nullable: false,
                                })
                            } else {
                                self.table.intern(Type::Interface {
                                    class: cid,
                                    args: types.into_boxed_slice(),
                                    nullable: false,
                                })
                            }
                        }
                    }
                    Element::Typedef(tid) => {
                        let d = &self.outline.typedefs[tid.0 as usize];
                        let (params, tgt) = (d.type_params.to_vec(), d.target_type);
                        if params.len() != types.len() {
                            return None;
                        }
                        let r = self.subst(tgt, &params, &types);
                        self.table.decorate(
                            r,
                            crate::table::DisplayDecoration::Alias {
                                typedef: tid,
                                args: types.into_boxed_slice(),
                            },
                        )
                    }
                    _ => return None,
                }
            }
            TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                if !type_params.is_empty() {
                    return None;
                }
                let (ret, parameters) = (*return_type, parameters.clone_params());
                let ret = match ret {
                    Some(r) => self.resolver(r)?,
                    None => self.core.dynamic_,
                };
                let mut pos = Vec::new();
                let mut opt = Vec::new();
                let mut named = Vec::new();
                for (kind, ty, the_name, req) in parameters {
                    let t = match ty {
                        Some(x) => self.resolver(x)?,
                        None => self.core.dynamic_,
                    };
                    match kind {
                        ParameterKind::Required => pos.push(t),
                        ParameterKind::Optional => opt.push(t),
                        ParameterKind::Named => named.push((the_name?, t, req)),
                    }
                }
                self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            TypeKind::Record { positional, named } => {
                let (positional, named) = (positional.clone(), named.clone());
                let pos = positional
                    .iter()
                    .map(|&x| self.resolver(x))
                    .collect::<Option<Vec<TypeId>>>()?;
                let mut nm = Vec::new();
                for (n, x) in named.iter() {
                    nm.push((n.sym, self.resolver(*x)?));
                }
                self.table.intern(Type::Record {
                    positional: pos.into_boxed_slice(),
                    named: nm.into_boxed_slice(),
                    nullable: false,
                })
            }
        };
        Some(if is_nullable {
            crate::ops::nullable(r, self.table)
        } else {
            r
        })
    }
}

/// The parameters of a written list, only what resolution uses.
trait SimpleParameters {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)>;
}

impl SimpleParameters for Box<[ast::Parameter]> {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)> {
        self.iter()
            .map(|p| (p.kind, p.ty, p.name.map(|n| n.sym), p.required))
            .collect()
    }
}

/// Variance of parameter `p` in type `t` (the outer position is `va`).
fn variance_in(table: &TypeTable, t: TypeId, p: TypeParamId, va: Variance_) -> Variance_ {
    match table.get(t) {
        Type::TypeParameter { param, .. } if *param == p => va,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().fold(Variance_::None_, |acc, &a| {
                acc.join(variance_in(table, a, p, va))
            })
        }
        Type::FutureOr { arg, .. } => variance_in(table, *arg, p, va),
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            let against = va.combine(Variance_::Contra);
            let mut v = variance_in(table, *ret, p, va);
            for &x in positional.iter().chain(optional.iter()) {
                v = v.join(variance_in(table, x, p, against));
            }
            for (_, x, _) in named.iter() {
                v = v.join(variance_in(table, *x, p, against));
            }
            // In the bounds of the function's type parameters: invariant.
            for &tp in type_params.iter() {
                if variance_in(table, table.param(tp).bound, p, va) != Variance_::None_ {
                    v = v.join(Variance_::In);
                }
            }
            v
        }
        Type::Record {
            positional, named, ..
        } => {
            let mut v = Variance_::None_;
            for &x in positional.iter() {
                v = v.join(variance_in(table, x, p, va));
            }
            for (_, x) in named.iter() {
                v = v.join(variance_in(table, *x, p, va));
            }
            v
        }
        Type::Intersection { bound, .. } => variance_in(table, *bound, p, va),
        _ => Variance_::None_,
    }
}

/// Every type parameter name declared in the unit.
fn parameter_names(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<SymbolId> {
    let mut s = HashSet::new();
    let mut tps = |l: &[ast::TypeParameter]| {
        for tp in l {
            s.insert(tp.name.sym);
        }
    };
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => tps(&x.type_params),
            DeclKind::Mixin(x) => tps(&x.type_params),
            DeclKind::Enum(x) => tps(&x.type_params),
            DeclKind::Extension(x) => tps(&x.type_params),
            DeclKind::ExtensionType(x) => tps(&x.type_params),
            DeclKind::Typedef(x) => tps(&x.type_params),
            DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for f in &a.functions {
        tps(&f.type_params);
    }
    for t in &a.types {
        if let TypeKind::Function {
            type_params,
            parameters,
            ..
        } = &t.kind
        {
            tps(type_params);
            for p in parameters.iter() {
                tps(&p.function_type_params);
            }
        }
    }
    for f in &a.functions {
        for p in f.parameters.iter().flatten() {
            tps(&p.function_type_params);
        }
    }
    s
}

/// Written types where the analyzer does not accept *super-bounded*:
/// `extends`, `with`, `implements`, a mixin's `on`, the body of
/// `typedef X = T` and the type of an instance creation.
fn without_super_bound(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<ast::TypeId> {
    let mut s = HashSet::new();
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => {
                s.extend(x.extends);
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Mixin(x) => {
                s.extend(x.on.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Enum(x) => {
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::ExtensionType(x) => s.extend(x.implements.iter().copied()),
            DeclKind::Typedef(x) => {
                if let ast::TypedefKind::Alias(t) = &x.kind {
                    s.insert(*t);
                }
            }
            DeclKind::Extension(_) | DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for e in &a.exprs {
        if let ExprKind::InstanceCreation { ty, .. } = &e.kind {
            s.insert(*ty);
        }
    }
    for m in &a.members {
        if let ast::MemberKind::Constructor(k) = &m.kind
            && let Some(r) = &k.redirect
        {
            s.insert(r.ty);
        }
    }
    s
}
