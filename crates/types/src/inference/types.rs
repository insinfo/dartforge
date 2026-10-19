//! Type operations used by inference: subtyping, UP/DOWN, schema closures,
//! `flatten`, building core types and resolving annotations written inside
//! bodies.

use super::body::Body;
use super::BodyInferrer;
use crate::bounds;
use crate::ops::{non_nullable, nullable, substitute};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{Type, TypeId, TypeParamId, TypeParamOwner, Variance};
use dartforge_elements::model::{ClassId, ClassKind, Element, UnitId};
use dartforge_frontend::ast;
use std::collections::HashMap;

/// Required, optional and named parameter types of a written signature.
type ParameterTypes = (Vec<TypeId>, Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId, bool)>);

impl<'a> BodyInferrer<'a> {
    pub(crate) fn env(&mut self) -> SubtypeEnv<'_> {
        SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core)
    }

    pub(crate) fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        if a == b {
            return true;
        }
        let mut env = self.env();
        is_subtype(a, b, &mut env)
    }

    pub(crate) fn up(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::up(a, b, &mut env)
    }

    pub(crate) fn down(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::down(a, b, &mut env)
    }

    pub(crate) fn nullable(&mut self, t: TypeId) -> TypeId {
        nullable(t, self.table)
    }

    pub(crate) fn non_null(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t) {
            Type::FutureOr { arg, nullable: true } => {
                let a = *arg;
                self.table.intern(Type::FutureOr { arg: a, nullable: false })
            }
            _ => non_nullable(t, self.table),
        }
    }

    /// `Null <: t`.
    pub(crate) fn the_is_nullable(&mut self, t: TypeId) -> bool {
        let n = self.core.null;
        self.sub(n, t)
    }

    /// `t <: Object`.
    pub(crate) fn is_non_nullable(&mut self, t: TypeId) -> bool {
        let o = self.core.object;
        self.sub(t, o)
    }

    pub(crate) fn is_dynamic(&self, t: TypeId) -> bool {
        matches!(self.table.get(t), Type::Dynamic)
    }

    /// **BOTTOM**(`t`).
    pub(crate) fn is_bottom(&mut self, t: TypeId) -> bool {
        let env = self.env();
        bounds::is_bottom(t, &env)
    }

    pub(crate) fn the_is_unknown(&self, t: TypeId) -> bool {
        self.core.is_unknown(self.table, t)
    }


    /// Greatest closure of a schema with respect to `_`.
    pub(crate) fn greatest_closure_of(&mut self, t: TypeId) -> TypeId {
        if self.the_is_unknown(t) {
            return self.core.object_nullable;
        }
        let mut env = self.env();
        bounds::schema_greatest(t, &mut env)
    }


    pub(crate) fn iface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(c) => self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    pub(crate) fn future(&mut self, t: TypeId) -> TypeId {
        let c = self.core.future_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn future_or(&mut self, t: TypeId) -> TypeId {
        self.table.intern(Type::FutureOr { arg: t, nullable: false })
    }

    pub(crate) fn iterable(&mut self, t: TypeId) -> TypeId {
        let c = self.core.iterable_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn flow_of(&mut self, t: TypeId) -> TypeId {
        let c = self.core.stream_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn list(&mut self, t: TypeId) -> TypeId {
        let c = self.core.list_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn set(&mut self, t: TypeId) -> TypeId {
        let c = self.core.set_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn map_type(&mut self, k: TypeId, v: TypeId) -> TypeId {
        let c = self.core.map_class;
        self.iface(c, vec![k, v])
    }

    /// A raw interface type (`List` without arguments, as the outline sometimes
    /// stores it) gets the arguments of the instantiation to bounds.
    pub(crate) fn complete_args(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Interface { class, args, nullable } if args.is_empty() => {
                let params = self.outline.classes[class.0 as usize].type_params.clone();
                if params.is_empty() {
                    return t;
                }
                let args = self.instantiate_to_bounds(&params);
                self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable })
            }
            _ => t,
        }
    }

    /// Arguments of `class` in `t` seen as an instance of it (`List<int>` as
    /// `Iterable` gives `[int]`), ignoring the nullability of `t`.
    pub(crate) fn as_instance_of(&mut self, t: TypeId, class: Option<ClassId>) -> Option<Vec<TypeId>> {
        let class = class?;
        let t = self.non_null(t);
        let t = self.complete_args(t);
        match self.table.get(t).clone() {
            Type::Interface { class: c, args, .. } if c == class => return Some(args.to_vec()),
            Type::Interface { .. } | Type::ExtensionType { .. } => {}
            Type::Intersection { bound, .. } => return self.as_instance_of(bound, Some(class)),
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t {
                    return None;
                }
                return self.as_instance_of(b, Some(class));
            }
            _ => return None,
        }
        let s = self.outline.hierarchy.supertype_of(t, class, self.table, self.core)?;
        match self.table.get(s) {
            Type::Interface { args, .. } => Some(args.to_vec()),
            _ => None,
        }
    }

    /// `flatten(T)` (specification, "Function Expressions"; the CFE's
    /// `flatten`).
    pub(crate) fn flatten(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Dynamic | Type::Void => t,
            Type::Intersection { bound, .. } => match self.as_instance_of(bound, self.core.future_class) {
                Some(a) => a[0],
                None => t,
            },
            Type::FutureOr { arg, nullable: n } => {
                if n {
                    self.nullable(arg)
                } else {
                    arg
                }
            }
            ty if ty.is_declared_nullable() => {
                let s = self.non_null(t);
                let f = self.flatten(s);
                self.nullable(f)
            }
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t || self.table.param(param).bound == self.core.object_nullable {
                    return t;
                }
                // `X extends Future<S>`: flatten is S.
                match self.as_instance_of(b, self.core.future_class) {
                    Some(a) => a[0],
                    None => t,
                }
            }
            _ => match self.as_instance_of(t, self.core.future_class) {
                Some(a) => a[0],
                None => t,
            },
        }
    }

    /// **futureValueTypeSchema**(`S`) (`inference.md`).
    pub(crate) fn future_value_type_schema(&mut self, s: TypeId) -> TypeId {
        if self.the_is_unknown(s) {
            return s;
        }
        match self.table.get(s).clone() {
            Type::Void | Type::Dynamic => s,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Assignable: subtype, `dynamic` (implicit cast) or coercion through
    /// `call`.
    pub(crate) fn assignable(&mut self, of: TypeId, for_: TypeId) -> bool {
        if self.is_dynamic(of) || self.sub(of, for_) {
            return true;
        }
        // `getImplicitCallMethod`: only a non-nullable interface type tears off
        // its `call` (`C?` is not assignable to a function type).
        if matches!(self.table.get(of), Type::Interface { nullable: false, .. })
            && let Some(call) = self.sym.call
                && let Some(m) = self.interface_member(of, call, false)
                    && m.method {
                        // The `ImplicitCallReference` and, the `call` being generic, the
                        // instantiation from the context (`insertGenericFunctionInstantiation`,
                        // INFERENCIA §8.4).
                        let t = self.instantiate_function_from_context(m.ty, for_);
                        return self.sub(t, for_);
                    }
        false
    }

    /// The implicit instantiation of a generic function type `f` by a context
    /// whose `flatten`, without `?`, is a non-generic function type
    /// (`insertGenericFunctionInstantiation`, `resolver.dart:1171-1224`);
    /// otherwise `f` itself.
    pub(crate) fn instantiate_function_from_context(&mut self, f: TypeId, ctx: TypeId) -> TypeId {
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.table.get(f).clone() else { return f };
        if type_params.is_empty() || self.the_is_unknown(ctx) {
            return f;
        }
        let k = self.flatten(ctx);
        let k = self.non_null(k);
        let Type::Function { type_params: tp_ctx, .. } = self.table.get(k).clone() else { return f };
        if !tp_ctx.is_empty() {
            return f;
        }
        let without = self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable });
        let mut gi = crate::constraints::GenericInferrer::new(&type_params);
        let mut env = self.env();
        gi.constrain_return(without, k, &mut env);
        let args = gi.choose_final(&mut env);
        crate::constraints::instantiate_function(f, &args, &mut env)
    }

    /// Coercion through `call` (*implicit call tearoff*): an object of an
    /// interface type with a `call` method, in a function type context, takes
    /// the type of its `call`. It is with that type that the argument
    /// constrains the type parameters of a generic call (`[1].map(somador)`
    /// infers `T` from the return of `Somador.call`). `None` when the coercion
    /// does not apply.
    pub(crate) fn implicit_call_type(&mut self, of: TypeId, context: TypeId) -> Option<TypeId> {
        if !matches!(self.table.get(context), Type::Function { .. }) || !matches!(self.table.get(of), Type::Interface { nullable: false, .. }) {
            return None;
        }
        let call = self.sym.call?;
        let m = self.interface_member(of, call, false)?;
        m.method.then_some(m.ty)
    }

    /// The class's `this` type (arguments = its own parameters).
    pub(crate) fn class_this_type(&mut self, c: ClassId) -> TypeId {
        let params = self.outline.classes[c.0 as usize].type_params.clone();
        let args: Vec<TypeId> = params
            .iter()
            .map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false }))
            .collect();
        if self.program.class(c).kind == ClassKind::ExtensionType {
            self.table.intern(Type::ExtensionType { decl: c, args: args.into_boxed_slice(), nullable: false })
        } else {
            self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false })
        }
    }

    /// Instantiation to bounds (`instantiate to bounds`) of the parameters of a
    /// class used raw (`List` → `List<dynamic>`).
    pub(crate) fn instantiate_to_bounds(&mut self, params: &[TypeParamId]) -> Vec<TypeId> {
        crate::ops::instantiate_to_bounds(params, &[], self.table, self.core)
    }

    /// Fresh copies (reused per list) of type parameters, with the bounds
    /// rewritten in them.
    pub(crate) fn new_parameters(&mut self, original: &[TypeParamId]) -> Vec<TypeParamId> {
        if original.is_empty() {
            return Vec::new();
        }
        if let Some(n) = self.constructor_params.get(&original[0].0) {
            return n.clone();
        }
        let new_: Vec<TypeParamId> = original
            .iter()
            .map(|&p| {
                let d = self.table.param(p).clone();
                self.table.alloc_type_param(d.name, TypeParamOwner::GenericFunctionType, d.bound, d.variance)
            })
            .collect();
        let types: Vec<TypeId> = new_.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let map = self.map(original, &types);
        for (&p, &o) in new_.iter().zip(original) {
            let b = self.table.param(p).bound;
            let b = self.subst(b, &map);
            self.table.set_type_param_bound(p, b);
            self.table.param_mut(p).explicit = self.table.param(o).explicit;
        }
        self.constructor_params.insert(original[0].0, new_.clone());
        new_
    }

    /// Substitution map `params → args`.
    pub(crate) fn map(&self, params: &[TypeParamId], args: &[TypeId]) -> HashMap<TypeParamId, TypeId> {
        params.iter().copied().zip(args.iter().copied()).collect()
    }

    pub(crate) fn subst(&mut self, t: TypeId, map: &HashMap<TypeParamId, TypeId>) -> TypeId {
        substitute(t, map, self.table)
    }

    /// Type of an annotation written in a body (type parameters in scope come
    /// from `cx`; the members of `cx`'s class or extension hide the top-level
    /// names, like the analyzer's `InstanceScope`).
    pub(crate) fn type_from_annotation(&mut self, cx: &Body, t: ast::TypeId) -> TypeId {
        let unit = cx.unit;
        let lib = cx.lib;
        let scope = cx.visible_type_parameters();
        let before = std::mem::replace(&mut self.type_container, (cx.class, cx.extension));
        let static_before = std::mem::replace(&mut self.in_static_member, cx.static_member);
        let locals = cx.visible_locals(self.interner);
        let locals_before = std::mem::replace(&mut self.locals_as_type, locals);
        let r = self.resolve_annotation(unit, lib, t, &scope);
        self.locals_as_type = locals_before;
        self.type_container = before;
        self.in_static_member = static_before;
        // The analyzer's `TypeAnnotation.type`, for the rules that read it.
        self.body_types.units[unit.0 as usize].annotation_types.insert(t, r);
        r
    }

    /// Like [`BodyInferrer::type_from_annotation`], for an annotation that appears
    /// in the given `context` (`is`, `as`, `catch`, type argument): that is
    /// what chooses the code for a name that is not a type.
    pub(crate) fn type_in_context(&mut self, cx: &Body, t: ast::TypeId, context: crate::resolve::TypeContext) -> TypeId {
        self.type_context = context;
        self.type_from_annotation(cx, t)
    }

    /// A written type argument (`f<T>()`, `<T>[]`, `C<T>()`).
    pub(crate) fn type_argument_type(&mut self, cx: &Body, t: ast::TypeId) -> TypeId {
        self.type_in_context(cx, t, crate::resolve::TypeContext::TypeArgument)
    }

    /// Resolves an annotation and stores its type and that of each nested type
    /// (type arguments, function and record parts) in `annotation_types` (the
    /// `NamedType.type` of each analyzer node).
    pub(crate) fn resolve_annotation(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        t: ast::TypeId,
        scope: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> TypeId {
        let r = self.resolve_annotation_without_recording(unit, lib, t, scope);
        self.body_types.units[unit.0 as usize].annotation_types.insert(t, r);
        r
    }

    fn resolve_annotation_without_recording(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        t: ast::TypeId,
        scope: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> TypeId {
        let node = self.program.unit(unit).ast.ty(t);
        let is_nullable = node.nullable;
        let span = node.span;
        let context = std::mem::take(&mut self.type_context);
        let r = match &node.kind {
            ast::TypeKind::Void => return self.core.void_,
            ast::TypeKind::Named { name, args } => {
                // Range of the name errors: from the prefix to the end of the name
                // (`_getErrorRange`); the argument count goes on the whole type.
                let range = dartforge_diagnostics::Span { start: name[0].span.start, end: name[name.len() - 1].span.end };
                let text = self.interner.resolve(name[name.len() - 1].sym).to_string();
                // `visitImportPrefixReference`: a prefix that names a local of the block
                // not yet declared (`_checkForReferenceBeforeDeclaration`).
                if name.len() == 2
                    && let Some(&(decl, true)) = self.locals_as_type.get(&name[0].sym)
                {
                    let name_text = self.interner.resolve(name[0].sym).to_string();
                    let msg = format!("{}: '{}'", crate::codes::REFERENCED_BEFORE_DECLARATION.template, name_text);
                    let context_msg = format!("The declaration of '{name_text}' is here.");
                    self.diagnostics.push(dartforge_diagnostics::Diagnostic::new(msg, name[0].span).with_context(decl, context_msg));
                    self.warning_units.push(self.current_unit);
                }
                if name.len() == 2 && name[0].span.start != name[0].span.end {
                    let p = name[0].sym;
                    let (cls, extension) = self.type_container;
                    let local = scope.contains_key(&p) || self.locals_as_type.contains_key(&p);
                    if crate::resolve::shadowed_prefix(self.program, self.interner, unit, p, local, cls, extension) {
                        let the_name = self.interner.resolve(p).to_string();
                        self.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, name[0].span, &[&the_name]);
                        return self.table.invalid(self.core.dynamic_);
                    }
                }
                let binding = if name.len() == 2 {
                    self.program.lookup_prefixed_in_unit(unit, name[0].sym, name[1].sym)
                } else {
                    let sym = name[0].sym;
                    if let Some(&pid) = scope.get(&sym) {
                        if !args.is_empty() {
                            self.report_type_arguments(&text, 0, args.len(), span);
                        }
                        if self.in_static_member && crate::resolve::class_param(self.table, pid) {
                            self.warning_with_code(
                                dartforge_diagnostics::codes::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC,
                                range,
                                &[],
                            );
                        }
                        let tp = self.table.intern(Type::TypeParameter { param: pid, nullable: false });
                        return if is_nullable { self.nullable(tp) } else { tp };
                    }
                    // A local (variable or local function) with that name in the body's
                    // scope: the type's element is it.
                    // `as`, `is` and type arguments have their own codes first
                    // (`named_type_resolver.dart:544-596`).
                    if let Some(&(decl, _)) = self.locals_as_type.get(&sym)
                        && matches!(context, crate::resolve::TypeContext::As | crate::resolve::TypeContext::Is | crate::resolve::TypeContext::TypeArgument)
                    {
                        let _ = decl;
                        self.report_type_name(context, true, &text, range);
                        return self.table.invalid(self.core.dynamic_);
                    }
                    if let Some(&(decl, _)) = self.locals_as_type.get(&sym) {
                        let name_text = self.interner.resolve(sym).to_string();
                        let msg = format!("{}: '{}'", crate::codes::REFERENCED_BEFORE_DECLARATION.template, name_text);
                        let context = format!("The declaration of '{name_text}' is here.");
                        self.diagnostics.push(dartforge_diagnostics::Diagnostic::new(msg, range).with_context(decl, context));
                        self.warning_units.push(self.current_unit);
                        return self.table.invalid(self.core.dynamic_);
                    }
                    if matches!(self.interner.resolve(sym), "dynamic" | "Never")
                        && self.program.lookup_in_unit(unit, sym).and_then(|b| b.getter).is_none()
                        && !args.is_empty()
                    {
                        self.report_type_arguments(&text, 0, args.len(), span);
                    }
                    match self.interner.resolve(sym) {
                        "dynamic" if self.program.lookup_in_unit(unit, sym).is_none() => return self.core.dynamic_,
                        "void" => return self.core.void_,
                        "Never" if self.program.lookup_in_unit(unit, sym).and_then(|b| b.getter).is_none() => {
                            return if is_nullable { self.table.decorate(self.core.null, crate::table::DisplayDecoration::NeverNullable) } else { self.core.never };
                        }
                        _ => {}
                    }
                    // The instance scope hides the top-level one.
                    let (cls, extension) = self.type_container;
                    if range.start != range.end {
                        match crate::resolve::name_in_container(self.program, self.interner, cls, extension, sym) {
                            Some(crate::resolve::InContainer::Getter) => {
                                self.report_type_name(context, true, &text, range);
                                return self.table.invalid(self.core.dynamic_);
                            }
                            // Setter only: `ResolutionVisitor`'s `LocalScope` keeps it under `x=`;
                            // the lookup goes on outward.
                            Some(crate::resolve::InContainer::OnlySetter) | None => {}
                        }
                    }
                    self.program.lookup_in_unit(unit, sym)
                };
                let args: Vec<ast::TypeId> = args.to_vec();
                let resolved: Vec<TypeId> = args
                    .iter()
                    .map(|&a| {
                        self.type_context = crate::resolve::TypeContext::TypeArgument;
                        self.resolve_annotation(unit, lib, a, scope)
                    })
                    .collect();
                // How many parameters the named type declares (for the number of
                // written arguments).
                let n_params = match binding.and_then(|b| b.getter) {
                    Some(Element::Class(cid)) => Some(self.outline.classes[cid.0 as usize].type_params.len()),
                    Some(Element::Typedef(tid)) => Some(self.outline.typedefs[tid.0 as usize].type_params.len()),
                    _ => None,
                };
                if let Some(n) = n_params
                    && !args.is_empty() && args.len() != n {
                        self.report_type_arguments(&text, n, args.len(), span);
                    }
                match binding.and_then(|b| b.getter) {
                    // `dart:core`'s `Null` is a class, but the type is the table's `Null`
                    // (as in the outline): otherwise `flatten(Future<Null>)` was not `Null`
                    // and `return Future<Null>…` in `Future<void> f() async` became an error.
                    Some(Element::Class(cid)) if Some(cid) == self.core.null_class => self.core.null,
                    Some(Element::Class(cid)) => {
                        // Wrong number of type arguments: all invalid
                        // (`named_type_resolver.dart:148`).
                        let resolved = match n_params {
                            Some(n) if !args.is_empty() && args.len() != n => vec![self.table.invalid(self.core.dynamic_); n],
                            _ => resolved,
                        };
                        self.class_type_with_args(cid, resolved)
                    }
                    // An alias that reaches itself instantiates as `dynamic`
                    // (`hasSelfReference`, `self_reference`).
                    Some(Element::Typedef(tid)) if crate::self_reference::self_referencing_typedef(self.program, self.program.typedef(tid).decl) => {
                        return self.core.dynamic_;
                    }
                    Some(Element::Typedef(tid)) => {
                        let data = self.outline.typedefs[tid.0 as usize].clone();
                        let args = if resolved.len() == data.type_params.len() {
                            resolved
                        } else if !args.is_empty() {
                            vec![self.table.invalid(self.core.dynamic_); data.type_params.len()]
                        } else {
                            self.instantiate_to_bounds(&data.type_params)
                        };
                        let map = self.map(&data.type_params, &args);
                        let r = self.subst(data.target_type, &map);
                        self.table.decorate(r, crate::table::DisplayDecoration::Alias { typedef: tid, args: args.into_boxed_slice() })
                    }
                    _ => {
                        let the_name = self.interner.resolve(name[name.len() - 1].sym);
                        match the_name {
                            "dynamic" if name.len() == 1 => return self.core.dynamic_,
                            "Never" if name.len() == 1 => {
                                return if is_nullable { self.table.decorate(self.core.null, crate::table::DisplayDecoration::NeverNullable) } else { self.core.never };
                            }
                            "Null" if name.len() == 1 => return self.core.null,
                            _ => {
                                // A name that does not resolve to a type (`NamedTypeResolver`); a
                                // synthetic recovery name (without width) and an ambiguous reference
                                // (reported separately) are left out.
                                let ambiguous = binding.is_some_and(|b| b.ambiguous);
                                if range.start != range.end && !ambiguous {
                                    let found = binding.is_some_and(|b| b.getter.is_some());
                                    self.report_type_name(context, found, &text, range);
                                }
                                // The recovery type: `InvalidType`.
                                self.table.invalid(self.core.dynamic_)
                            }
                        }
                    }
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                let mut local = scope.clone();
                let mut tps = Vec::new();
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    self.table.without_default_type.insert(pid);
                    local.insert(tp.name.sym, pid);
                    tps.push(pid);
                }
                for (tp, &pid) in type_params.iter().zip(tps.iter()) {
                    if let Some(b) = tp.bound {
                        let bt = self.resolve_annotation(unit, lib, b, &local);
                        self.table.set_type_param_bound(pid, bt);
                    }
                }
                let ret = match return_type {
                    Some(r) => self.resolve_annotation(unit, lib, *r, &local),
                    None => self.core.dynamic_,
                };
                let (pos, opt, named) = self.parameter_types(unit, lib, parameters, &local);
                self.table.intern(Type::Function {
                    type_params: tps.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let positional: Vec<ast::TypeId> = positional.to_vec();
                let named: Vec<(dartforge_intern::SymbolId, ast::TypeId)> = named.iter().map(|(n, t)| (n.sym, *t)).collect();
                let pos: Vec<TypeId> = positional.iter().map(|&t| self.resolve_annotation(unit, lib, t, scope)).collect();
                let nm: Vec<_> = named.iter().map(|(n, t)| (*n, self.resolve_annotation(unit, lib, *t, scope))).collect();
                self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            }
        };
        if is_nullable {
            self.nullable(r)
        } else {
            r
        }
    }

    /// A type name that does not resolve to a type, in the context's code.
    fn report_type_name(&mut self, context: crate::resolve::TypeContext, found: bool, the_name: &str, range: dartforge_diagnostics::Span) {
        // The synthetic name is not reported (`reportNullOrNonTypeElement`).
        if the_name.is_empty() {
            return;
        }
        let d = crate::resolve::type_name_diagnostic(context, found, the_name, range);
        let args: Vec<&str> = d.args.iter().map(|a| &**a).collect();
        let code = d.code.expect("has a code");
        self.warning_with_code(code, range, &args);
    }

    /// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` on the whole type.
    pub(crate) fn report_type_arguments(&mut self, the_name: &str, params_list: usize, arguments: usize, span: dartforge_diagnostics::Span) {
        let (p, a) = (params_list.to_string(), arguments.to_string());
        self.warning_with_code(
            dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS,
            span,
            &[the_name, &p, &a],
        );
    }

    /// `extensionTypeErasure` (the representation type in place of each
    /// extension type).
    pub(crate) fn erase_extension(&mut self, t: TypeId) -> TypeId {
        let outline = &*self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut crate::table::TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, self.table, &rep)
    }

    /// `C<args>` (with instantiation to bounds if arguments are missing).
    pub(crate) fn class_type_with_args(&mut self, cid: ClassId, args: Vec<TypeId>) -> TypeId {
        let params = self.outline.classes[cid.0 as usize].type_params.clone();
        let args = if args.len() == params.len() { args } else { self.instantiate_to_bounds(&params) };
        let class = self.program.class(cid);
        if class.kind == ClassKind::ExtensionType {
            return self.table.intern(Type::ExtensionType { decl: cid, args: args.into_boxed_slice(), nullable: false });
        }
        if Some(class.library) == self.core.async_library && self.interner.resolve(class.name) == "FutureOr" {
            let a = args.first().copied().unwrap_or(self.core.dynamic_);
            return self.table.intern(Type::FutureOr { arg: a, nullable: false });
        }
        self.table.intern(Type::Interface { class: cid, args: args.into_boxed_slice(), nullable: false })
    }

    /// Types (positional, optional, named) of a written parameter list.
    pub(crate) fn parameter_types(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        parameters: &[ast::Parameter],
        scope: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> ParameterTypes {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in parameters.iter() {
            let t = self.written_parameter_type(unit, lib, p, scope).unwrap_or(self.core.dynamic_);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
        }
        (pos, opt, named)
    }

    /// Written type of a parameter (including the old form `int f(int x)`).
    pub(crate) fn written_parameter_type(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        p: &ast::Parameter,
        scope: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> Option<TypeId> {
        if let Some(fps) = &p.function_parameters {
            let mut local = scope.clone();
            let mut tps = Vec::new();
            for tp in p.function_type_params.iter() {
                let pid = self.table.alloc_type_param(
                    tp.name.sym,
                    TypeParamOwner::GenericFunctionType,
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                self.table.without_default_type.insert(pid);
                local.insert(tp.name.sym, pid);
                tps.push(pid);
            }
            let ret = match p.ty {
                Some(r) => self.resolve_annotation(unit, lib, r, &local),
                None => self.core.dynamic_,
            };
            let (pos, opt, named) = self.parameter_types(unit, lib, fps, &local);
            let f = self.table.intern(Type::Function {
                type_params: tps.into_boxed_slice(),
                ret,
                positional: pos.into_boxed_slice(),
                optional: opt.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable: p.function_nullable,
            });
            return Some(f);
        }
        p.ty.map(|t| self.resolve_annotation(unit, lib, t, scope))
    }
}
