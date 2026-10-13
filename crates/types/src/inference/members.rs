//! Member lookup: instance members (interface, with substitution of the
//! receiver's type arguments), static members (class literal) and extension
//! members (with inference of the extension's arguments and tie-breaking by
//! specificity).
//!
//! Keys in the element model: getters and methods by name, setters by
//! `nome_=`, operators by their text (`[]`, `[]=`, `unary-`).

use super::BodyInferrer;
use crate::constraints::GenericInferrer;
use crate::resolved::{MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, ExtensionId, FunctionElementId, FunctionKind, LibraryId};
use dartforge_intern::SymbolId;

/// A member found, already with the type instantiated for the receiver.
#[derive(Debug, Clone)]
pub(crate) struct Membro {
    pub resolved: Resolved,
    /// Getter/field: the value's type; setter: the parameter's type; method: the function type.
    pub ty: TypeId,
    /// Method or operator (invocable/tear-off), as opposed to getter/field/setter.
    pub method: bool,
    /// The declared function (for whoever needs the element).
    #[allow(dead_code)]
    pub function: Option<FunctionElementId>,
    pub of_extension: bool,
}

/// Result of a member lookup on a receiver.
#[derive(Debug, Clone)]
pub(crate) enum Search {
    /// A `dynamic` (or invalid) receiver: dynamic dispatch.
    Dinamico,
    /// A `Never` receiver: the access has type `Never`.
    NeverType,
    Finding(Membro),
    Missing,
}

/// Inference as the inheritance's provider: the type of an implicit accessor
/// is the field's, inferred on demand.
impl<'a> crate::heranca::Provider<'a> for BodyInferrer<'a> {
    fn program(&self) -> &'a dartforge_elements::model::Program {
        self.program
    }
    fn interner(&self) -> &'a dartforge_intern::Interner {
        self.interner
    }
    fn core(&self) -> &'a crate::table::CoreTypes {
        self.core
    }
    fn table(&mut self) -> &mut crate::table::TypeTable {
        &mut *self.table
    }
    fn class_data(&self, c: ClassId) -> crate::resolve::ClassTypeData {
        self.outline.classes[c.0 as usize].clone()
    }
    fn member_type(&mut self, f: FunctionElementId) -> TypeId {
        let tv = match self.program.function(f).variable {
            // When looking up what a field of class `c` overrides (the
            // `getOverridden2` of `_inferAccessorOrField`), the members declared in
            // `c` are not typed: the analyzer's interface keeps elements, and the
            // type of the field in progress does not enter (here it would cause a
            // false inference cycle). The interface built this way is provisional
            // and leaves the cache after the lookup.
            Some(v)
                if self.program.function(f).kind == FunctionKind::ImplicitAccessor
                    && self.declared_untyped.is_some()
                    && self.program.variable(v).class == self.declared_untyped =>
            {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            // The field being inferred (`late final a = m();` looks up `m` in the
            // class's own interface): the analyzer's interface keeps the element,
            // and its type is only requested by whoever reads it. Here the interface
            // carries it provisionally (it leaves the cache after the lookup), and
            // whoever actually reads it goes through `variable_type` (the cycle).
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor && matches!(self.vars_state[v.0 as usize], super::VarState::InProgress) => {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            // Another field not yet inferred, in a lookup made during top-level
            // inference (`late final c = m(); late final d = c.x;`: inferring `c`
            // looks up `m`): the analyzer only infers what the initializer reads,
            // and its interface keeps elements. Inferring `d` here would cause a
            // false `TOP_LEVEL_CYCLE`. Only the members with the looked-up name
            // decide the result; the others stay provisional.
            Some(v)
                if self.program.function(f).kind == FunctionKind::ImplicitAccessor
                    && self.lookup_name.is_some_and(|n| n != self.program.variable(v).name)
                    && !self.variable_stack.is_empty()
                    && matches!(self.vars_state[v.0 as usize], super::VarState::Pending)
                    && self.outline.variables.get(v.0 as usize).is_some_and(|d| d.declared_type.is_none()) =>
            {
                self.provisional_inheritance = true;
                self.core.dynamic_
            }
            Some(v) if self.program.function(f).kind == FunctionKind::ImplicitAccessor => self.variable_type(v),
            _ => self.core.dynamic_,
        };
        crate::heranca::member_function_type(self.program, &*self.outline, self.core, &mut *self.table, f, tv)
    }
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        BodyInferrer::sub(self, a, b)
    }
    fn normalize_(&mut self, t: TypeId) -> TypeId {
        crate::ops::normalize(t, &mut *self.table, self.core)
    }
}

impl<'a> BodyInferrer<'a> {
    /// `getMember2` on the interface of `cls`, by the `Name` of the library
    /// where the access is.
    pub(crate) fn inheritance_member(&mut self, cls: ClassId, key: SymbolId, concrete: bool, to_super: bool) -> Option<crate::heranca::Membro> {
        let lib = match self.current_unit {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(cls).library,
        };
        let name = crate::heranca::Nome::novo(self.interner, lib, key);
        // The lookup may infer a field, and its inference may look up members:
        // the manager is moved out during the call (a nested lookup builds the
        // interfaces it needs in its own manager).
        let base = self.interner.resolve(key);
        let base = base.strip_suffix('=').map_or(Some(key), |b| self.interner.lookup(b));
        let saved_name = std::mem::replace(&mut self.lookup_name, base);
        let mut h = std::mem::take(&mut self.inheritance);
        let finding = h.member(self, cls, name, concrete, None, to_super);
        self.inheritance = h;
        self.lookup_name = saved_name;
        self.after_provisional_lookup(finding.as_ref());
        finding
    }

    /// After a lookup that built interfaces with a provisional field being
    /// inferred: outside the overridden-members lookup (which cleans up by
    /// itself), the interfaces leave the cache; and a found member that is the
    /// field being inferred itself asks for its type (the `TOP_LEVEL_CYCLE` of
    /// whoever reads it).
    fn after_provisional_lookup(&mut self, finding: Option<&crate::heranca::Membro>) {
        if !self.provisional_inheritance || self.declared_untyped.is_some() {
            return;
        }
        self.inheritance = crate::heranca::Heranca::default();
        self.provisional_inheritance = false;
        if let Some(m) = finding
            && self.program.function(m.funcao).kind == FunctionKind::ImplicitAccessor
            && let Some(v) = self.program.function(m.funcao).variable
            && matches!(self.vars_state[v.0 as usize], super::VarState::InProgress)
        {
            self.variable_type(v);
        }
    }

    /// `getInherited2` on the interface of `cls`.
    pub(crate) fn inherited_from_inheritance(&mut self, cls: ClassId, key: SymbolId) -> Option<crate::heranca::Membro> {
        let lib = match self.current_unit {
            Some(u) => self.program.unit(u).library,
            None => self.program.class(cls).library,
        };
        let name = crate::heranca::Nome::novo(self.interner, lib, key);
        let base = self.interner.resolve(key);
        let base = base.strip_suffix('=').map_or(Some(key), |b| self.interner.lookup(b));
        let saved_name = std::mem::replace(&mut self.lookup_name, base);
        let mut h = std::mem::take(&mut self.inheritance);
        let finding = h.herdado(self, cls, name);
        self.inheritance = h;
        self.lookup_name = saved_name;
        self.after_provisional_lookup(finding.as_ref());
        finding
    }

    /// The value of an interface member derived from its function type: the
    /// getter's return, the setter's parameter, the method's type.
    fn inheritance_member_type(&self, m: &crate::heranca::Membro) -> (TypeId, bool) {
        match (m.kind, self.table.get(m.ty)) {
            (crate::heranca::Kind::Getter, Type::Function { ret, .. }) => (*ret, false),
            (crate::heranca::Kind::Setter, Type::Function { positional, .. }) => (positional.first().copied().unwrap_or(self.core.dynamic_), false),
            (crate::heranca::Kind::Metodo, _) => (m.ty, true),
            _ => (self.core.dynamic_, false),
        }
    }

    /// Setter key (`nome_=`), if there is any setter with that name.
    pub(crate) fn setter_key(&self, name: SymbolId) -> Option<SymbolId> {
        let s = format!("{}_=", self.interner.resolve(name));
        self.interner.lookup(&s)
    }

    /// Type of a declared member, before the class substitution.
    /// Returns `(ty, é_método)`.
    pub(crate) fn declared_member_type(&mut self, f: FunctionElementId, setter: bool) -> (TypeId, bool) {
        let fe = self.program.function(f);
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let t = match fe.variable {
                    Some(v) => self.variable_type(v),
                    None => self.core.dynamic_,
                };
                (t, false)
            }
            FunctionKind::Getter => (self.outline.functions[f.0 as usize].return_type, false),
            FunctionKind::Setter => {
                let t = self.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(self.core.dynamic_);
                (t, false)
            }
            _ => {
                let _ = setter;
                (self.outline.functions[f.0 as usize].signature, true)
            }
        }
    }

    /// Instance member through the receiver's interface (without extensions).
    pub(crate) fn interface_member(&mut self, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        // `Null` only has `Object`'s members (`null.hashCode`).
        let recv = if matches!(self.table.get(recv), Type::Null) { self.core.object } else { recv };
        let recv = self.non_null(recv);
        let recv = self.complete_args(recv);
        if setter {
            if let Some(m) = self.setter_key(name).and_then(|_| self.interface_member_key(recv, name, true)) {
                return Some(m);
            }
            // `late final x;` without an initializer has an implicit setter (a
            // single assignment); the element model only creates it for non-final
            // fields.
            let m = self.interface_member_key(recv, name, false)?;
            let e_late_final = match m.function.map(|f| self.program.function(f)) {
                Some(fe) if fe.kind == FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                    let ve = self.program.variable(v);
                    ve.late && ve.final_ && self.initializer(v).is_none()
                }),
                _ => false,
            };
            return if e_late_final { Some(m) } else { None };
        }
        self.interface_member_key(recv, name, false)
    }

    fn interface_member_key(&mut self, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        match self.table.get(recv).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                if !setter
                    && let Some(m) = self.representation_member(recv, class, key)
                {
                    return Some(m);
                }
                // The interface of `InheritanceManager3`. A declared member has the type
                // computed now (the field may have finished inference after the
                // interface was built); a synthetic one (`topMerge`) has the
                // interface's, over the class's parameters.
                let m = self.inheritance_member(class, key, false, false)?;
                let f = m.funcao;
                let owner = self.program.function(f).class.unwrap_or(m.class);
                let (t, method) = if m.synthetic {
                    let (t, method) = self.inheritance_member_type(&m);
                    (self.substitute_from_owner(recv, class, class, t), method)
                } else {
                    let (t, method) = self.declared_member_type(f, setter);
                    (self.substitute_from_owner(recv, class, owner, t), method)
                };
                // Instance members (including a field's implicit getter) are referred to
                // by the function, as the emitter expects.
                let member = MemberRef::Function(f);
                Some(Membro {
                    resolved: Resolved::Member { class: owner, member, via_super: false },
                    ty: t,
                    method,
                    function: Some(f),
                    of_extension: false,
                })
            }
            Type::Intersection { bound, .. } => self.interface_member(bound, name, setter),
            Type::TypeParameter { param, .. } => {
                if param == self.core.unknown_param {
                    return None;
                }
                let b = self.table.param(param).bound;
                if b == recv {
                    return None;
                }
                let b = if b == self.core.object_nullable { self.core.object } else { b };
                self.interface_member(b, name, setter)
            }
            Type::Function { .. } => {
                if !setter && Some(name) == self.sym.call {
                    return Some(Membro { resolved: Resolved::Dynamic, ty: recv, method: true, function: None, of_extension: false });
                }
                let f = self.core.function;
                self.interface_member(f, name, setter)
            }
            Type::Record { positional, named, .. } => {
                if !setter {
                    for (n, t) in named.iter() {
                        if *n == name {
                            return Some(Membro { resolved: Resolved::Dynamic, ty: *t, method: false, function: None, of_extension: false });
                        }
                    }
                    let s = self.interner.resolve(name);
                    if let Some(d) = s.strip_prefix('$') {
                        if let Ok(i) = d.parse::<usize>() {
                            if i >= 1 && i <= positional.len() {
                                return Some(Membro { resolved: Resolved::Dynamic, ty: positional[i - 1], method: false, function: None, of_extension: false });
                            }
                        }
                    }
                }
                let r = self.core.record;
                self.interface_member(r, name, setter)
            }
            Type::FutureOr { .. } | Type::Null | Type::Void => {
                let o = self.core.object;
                self.interface_member(o, name, setter)
            }
            _ => None,
        }
    }

    /// Representation field of an extension type
    /// (`extension type Id(int v)`: `i.v`, `v` in the body; R-EXT-04). The
    /// element model does not create the implicit getter.
    fn representation_member(&mut self, recv: TypeId, class: ClassId, name: SymbolId) -> Option<Membro> {
        let cl = self.program.class(class);
        if cl.kind != dartforge_elements::model::ClassKind::ExtensionType {
            return None;
        }
        let rep = cl.representation?;
        if self.program.variable(rep).name != name {
            return None;
        }
        let t = self.outline.variables[rep.0 as usize].declared_type?;
        let ty = self.substitute_from_owner(recv, class, class, t);
        Some(Membro {
            resolved: Resolved::Member { class, member: MemberRef::Variable(rep), via_super: false },
            ty,
            method: false,
            function: None,
            of_extension: false,
        })
    }

    /// Substitutes the owner class's type parameters with what the receiver
    /// instantiates (`List<int>` → a member of `Iterable<E>` with `E = int`).
    pub(crate) fn substitute_from_owner(&mut self, recv: TypeId, recv_class: ClassId, owner: ClassId, t: TypeId) -> TypeId {
        let params = self.outline.classes[owner.0 as usize].type_params.clone();
        if params.is_empty() {
            return t;
        }
        let args: Vec<TypeId> = if owner == recv_class {
            match self.table.get(recv) {
                Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                _ => return t,
            }
        } else {
            match self.outline.hierarchy.supertype_of(recv, owner, self.table, self.core) {
                Some(s) => match self.table.get(s) {
                    Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                    _ => return t,
                },
                None => return t,
            }
        };
        if args.len() != params.len() {
            return t;
        }
        let map = self.map(&params, &args);
        self.subst(t, &map)
    }

    /// Full lookup: interface, then extensions accessible in `lib`.
    pub(crate) fn lookup_member(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Search {
        self.extension_ambiguity = None;
        let b = self.lookup_member_unambiguous(lib, recv, name, setter);
        // Ambiguous extensions: the analyzer does not resolve (invalid type) and
        // only reports the ambiguity.
        if matches!(b, Search::Missing) && self.extension_ambiguity.is_some() {
            return Search::Dinamico;
        }
        b
    }

    fn lookup_member_unambiguous(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Search {
        match self.table.get(recv) {
            Type::Dynamic => return Search::Dinamico,
            Type::Never => return Search::NeverType,
            _ => {}
        }
        if self.the_is_unknown(recv) {
            return Search::Dinamico;
        }
        // A type parameter bounded by `dynamic` (`isDynamicBounded`,
        // `type_property_resolver.dart:81-90`): only `Object`'s members, and the
        // rest without an error (dynamic).
        if matches!(self.table.get(recv), Type::TypeParameter { .. } | Type::Intersection { .. }) {
            let mut b = recv;
            for _ in 0..16 {
                match self.table.get(b) {
                    Type::TypeParameter { param, .. } => {
                        let l = self.table.param(*param).bound;
                        if l == b {
                            break;
                        }
                        b = l;
                    }
                    Type::Intersection { bound, .. } => b = *bound,
                    _ => break,
                }
            }
            if matches!(self.table.get(b), Type::Dynamic) {
                let o = self.core.object;
                if let Some(m) = self.interface_member(o, name, setter) {
                    return Search::Finding(m);
                }
                return Search::Dinamico;
            }
        }
        let nullable = self.the_is_nullable(recv) && !matches!(self.table.get(recv), Type::Null);
        // Nullable receiver: only `Object`'s members through the interface; the
        // rest may come from extensions on the nullable type.
        if nullable {
            let o = self.core.object;
            if let Some(m) = self.interface_member(o, name, setter) {
                return Search::Finding(m);
            }
            if let Some(m) = self.extension_member(lib, recv, name, setter) {
                return Search::Finding(m);
            }
        }
        if let Some(m) = self.interface_member(recv, name, setter) {
            return Search::Finding(m);
        }
        // `_lookupInterfaceType` also looks for the pair (a getter's setter
        // `name=`, a setter's getter; `[]=` for `[]` and vice versa): if the
        // interface has one of them, `_hasGetterOrSetter` holds and extensions
        // are not consulted (`type_property_resolver.dart:181-185, 251-281`).
        let par = if Some(name) == self.sym.index {
            self.sym.set_index.and_then(|s| self.interface_member(recv, s, false))
        } else if Some(name) == self.sym.set_index {
            self.sym.index.and_then(|s| self.interface_member(recv, s, false))
        } else {
            self.interface_member(recv, name, !setter)
        };
        if par.is_some() {
            return Search::Missing;
        }
        if let Some(m) = self.extension_member(lib, recv, name, setter) {
            return Search::Finding(m);
        }
        // `TypePropertyResolver`'s last resort: `Object`'s interface (an
        // extension type without `implements` does not have it in its own).
        if !nullable {
            let o = self.core.object;
            if let Some(m) = self.interface_member(o, name, setter) {
                return Search::Finding(m);
            }
        }
        // `f.call` with `f` of type `Function` (from `dart:core`): no error and
        // dynamic (`TypePropertyResolver`,
        // an611:src/dart/resolver/type_property_resolver.dart:186-191).
        if !nullable
            && Some(name) == self.sym.call
            && matches!(self.table.get(recv), Type::Interface { class, .. } if Some(*class) == self.core.function_class)
        {
            return Search::Dinamico;
        }
        Search::Missing
    }

    /// Accessing `name` on a potentially nullable receiver requires a null
    /// check: the member belongs neither to `Object` nor to an extension that
    /// applies to the nullable type. The analyzer (`TypePropertyResolver`) then
    /// reports `unchecked_use_of_nullable_value`, never `undefined_*`, even when
    /// the member does not exist on the non-nullable type either. `void` is left
    /// out (it is `use_of_void_result`, checked earlier), like `dynamic` and
    /// `Never`.
    pub(crate) fn requires_null_check(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> bool {
        match self.table.get(recv) {
            Type::Dynamic | Type::Never | Type::Void => return false,
            // `type_property_resolver.dart:91-96`: an extension type is nullable
            // only through a written `?` (not by not being a subtype of `Object`).
            Type::ExtensionType { nullable: false, .. } => return false,
            _ => {}
        }
        // `isDynamicBounded`/`isInvalidBounded` (`type_property_resolver.dart:81-90`):
        // a type parameter whose (resolved) bound is `dynamic` or invalid.
        {
            let mut b = recv;
            for _ in 0..16 {
                match self.table.get(b) {
                    Type::TypeParameter { param, .. } => {
                        let l = self.table.param(*param).bound;
                        if l == b {
                            break;
                        }
                        b = l;
                    }
                    Type::Intersection { bound, .. } => b = *bound,
                    _ => break,
                }
            }
            if b != recv && (matches!(self.table.get(b), Type::Dynamic) || self.table.e_invalido(b)) {
                return false;
            }
        }
        if self.the_is_unknown(recv) || self.is_non_nullable(recv) {
            return false;
        }
        let o = self.core.object;
        if self.interface_member(o, name, setter).is_some() {
            return false;
        }
        self.extension_member(lib, recv, name, setter).is_none()
    }

    /// Arguments of extension `e` for the receiver, if it applies.
    pub(crate) fn applicable_extension(&mut self, e: ExtensionId, recv: TypeId) -> Option<Vec<TypeId>> {
        let data = self.outline.extensions[e.0 as usize].clone();
        if data.type_params.is_empty() {
            return if self.sub(recv, data.on) { Some(Vec::new()) } else { None };
        }
        // New parameters: inside the extension itself the receiver mentions its
        // parameters (`this` is `Iterable<T>`), and `Iterable<T> <#
        // Iterable<T>` with `T` in `L` would constrain nothing.
        let new_ = self.new_parameters(&data.type_params);
        let types: Vec<TypeId> = new_.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let m0 = self.map(&data.type_params, &types);
        let on_novo = self.subst(data.on, &m0);
        let mut inf = GenericInferrer::new(&new_);
        let mut env = self.env();
        inf.constrain_argument(recv, on_novo, &mut env);
        let args = inf.choose_final(&mut env);
        drop(env);
        let map = self.map(&data.type_params, &args);
        let on = self.subst(data.on, &map);
        if !self.sub(recv, on) {
            return None;
        }
        // The inferred arguments respect the bounds (`T extends Struct`):
        // otherwise the extension does not apply (§13.2), and another one (that
        // of `Array<Array<T>>`, not that of `Array<T extends AbiSpecificInteger>`)
        // is chosen.
        for (i, &p) in data.type_params.iter().enumerate() {
            let bnd = self.subst(self.table.param(p).bound, &map);
            if !self.sub(args[i], bnd) {
                return None;
            }
        }
        Some(args)
    }

    /// Extension member applicable to the receiver, with tie-breaking by specificity.
    pub(crate) fn extension_member(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        // `findExtension`: an extension is a candidate if it declares the getter
        // **or** the setter of the base name (`extension_member_resolver.dart:70-110`);
        // the chosen one without the requested member does not resolve.
        // Operators only by themselves.
        let identifier = self.interner.resolve(name).chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$');
        let par = if Some(name) == self.sym.index {
            self.sym.set_index
        } else if Some(name) == self.sym.set_index {
            self.sym.index
        } else if !identifier {
            None
        } else if setter {
            Some(name)
        } else {
            self.setter_key(name)
        };
        let exts = self.accessible_extensions(lib);
        let mut candidates: Vec<(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId)> = Vec::new();
        for &e in exts.iter() {
            let members = &self.program.extension(e).instance_members;
            let Some(&f) = members.get(&key).or_else(|| par.and_then(|k| members.get(&k))) else { continue };
            if let Some(args) = self.applicable_extension(e, recv) {
                let data = self.outline.extensions[e.0 as usize].clone();
                let map = self.map(&data.type_params, &args);
                let on = self.subst(data.on, &map);
                candidates.push((e, f, args, on));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        // `_chooseMostSpecific` (`an611:src/dart/resolver/extension_member_resolver.dart:273-312`).
        let mut best_so_far: Option<usize> = None;
        let mut tied: Vec<usize> = Vec::new();
        for i in 0..candidates.len() {
            if !tied.is_empty() {
                let mut is_the_most = true;
                let mut has_more = false;
                for &o in &tied.clone() {
                    if !self.most_specific_extension(&candidates[i], &candidates[o]) {
                        is_the_most = false;
                    }
                    if self.most_specific_extension(&candidates[o], &candidates[i]) {
                        has_more = true;
                    }
                }
                if is_the_most {
                    best_so_far = Some(i);
                    tied.clear();
                } else if !has_more {
                    tied.push(i);
                }
            } else if let Some(b) = best_so_far {
                if self.most_specific_extension(&candidates[b], &candidates[i]) {
                } else if self.most_specific_extension(&candidates[i], &candidates[b]) {
                    best_so_far = Some(i);
                } else {
                    tied.push(b);
                    tied.push(i);
                    best_so_far = None;
                }
            } else {
                best_so_far = Some(i);
            }
        }
        let Some(best) = best_so_far else {
            // Ambiguous: `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` stays pending for
            // whoever has the name (`report_extension_ambiguity`).
            let names: Vec<String> = tied
                .iter()
                .map(|&i| {
                    let x = self.program.extension(candidates[i].0);
                    match x.name {
                        Some(n) => format!("extension '{}'", self.interner.resolve(n)),
                        None => {
                            let on = self.outline.extensions[candidates[i].0 .0 as usize].on;
                            format!("unnamed extension on '{}'", self.table.format(on, self.interner, self.program))
                        }
                    }
                })
                .collect();
            let list = match names.len() {
                0 | 1 => names.join(""),
                2 => format!("{} and {}", names[0], names[1]),
                n => format!("{}, and {}", names[..n - 1].join(", "), names[n - 1]),
            };
            // With exactly two, 3.13.4 writes the extensions out in full
            // (`extension E1 on int`; T2, case c13). Only named ones without type
            // parameters: the display of the others in 3.13.4 was not checked, and
            // without both arguments the text stays the 3.6 one.
            let display = |inf: &Self, i: usize| -> Option<String> {
                let x = inf.program.extension(candidates[i].0);
                let data = &inf.outline.extensions[candidates[i].0 .0 as usize];
                let n = x.name?;
                data.type_params.is_empty().then(|| {
                    format!("extension {} on {}", inf.interner.resolve(n), inf.table.format(data.on, inf.interner, inf.program))
                })
            };
            let two = match tied[..] {
                [a, b] => display(self, a).zip(display(self, b)),
                _ => None,
            };
            self.extension_ambiguity = Some((self.interner.resolve(name).to_string(), list, two));
            return None;
        };
        let (e, f, args, _) = candidates.swap_remove(best);
        if self.program.extension(e).instance_members.get(&key) != Some(&f) {
            return None;
        }
        // `notifyExtensionUsed`: the chosen extension uses its imports.
        self.body_types.extensoes_usadas.insert((lib, e));
        let (t, method) = self.declared_member_type(f, setter);
        let data = self.outline.extensions[e.0 as usize].clone();
        let map = self.map(&data.type_params, &args);
        let t = self.subst(t, &map);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, ty: t, method, function: Some(f), of_extension: true })
    }

    /// `_isMoreSpecific` (`extension_member_resolver.dart:382-418`): outside the
    /// platform, the platform's one wins; otherwise the instantiated extended
    /// type is a subtype of the other and not the reverse, or, on a tie, the
    /// same for the extended types instantiated to bounds.
    fn most_specific_extension(
        &mut self,
        a: &(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId),
        b: &(ExtensionId, FunctionElementId, Vec<TypeId>, TypeId),
    ) -> bool {
        let sdk_a = self.program.library(self.program.extension(a.0).library).is_sdk;
        let sdk_b = self.program.library(self.program.extension(b.0).library).is_sdk;
        if sdk_a != sdk_b {
            return !sdk_a;
        }
        if !self.sub(a.3, b.3) {
            return false;
        }
        if !self.sub(b.3, a.3) {
            return true;
        }
        let la = self.on_at_bounds(a.0);
        let lb = self.on_at_bounds(b.0);
        self.sub(la, lb) && !self.sub(lb, la)
    }

    /// The extended type of `e` instantiated to bounds.
    fn on_at_bounds(&mut self, e: ExtensionId) -> TypeId {
        let data = self.outline.extensions[e.0 as usize].clone();
        let args = self.instantiate_to_bounds(&data.type_params);
        let map = self.map(&data.type_params, &args);
        self.subst(data.on, &map)
    }

    /// Reports, at the name, the extension ambiguity left by the last lookup
    /// (`AMBIGUOUS_EXTENSION_MEMBER_ACCESS`,
    /// `extension_member_resolver.dart:115-128`).
    pub(crate) fn report_extension_ambiguity(&mut self, name: dartforge_diagnostics::Span) {
        if let Some((n, list, two)) = self.extension_ambiguity.take() {
            let code = dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_EXTENSION_MEMBER_ACCESS;
            match two {
                Some((a, b)) => self.warning_with_code(code, name, &[&n, &list, &a, &b]),
                None => self.warning_with_code(code, name, &[&n, &list]),
            }
        }
    }

    /// Instance member of the already instantiated extension `e` (`E(x).m`,
    /// explicit override, R-EXT-02): only that extension is consulted.
    pub(crate) fn explicit_extension_member(&mut self, e: ExtensionId, args: &[TypeId], name: SymbolId, setter: bool) -> Option<Membro> {
        let key = if setter { self.setter_key(name)? } else { name };
        let &f = self.program.extension(e).instance_members.get(&key)?;
        let (t, method) = self.declared_member_type(f, setter);
        let data = self.outline.extensions[e.0 as usize].clone();
        let t = if data.type_params.len() == args.len() {
            let map = self.map(&data.type_params, args);
            self.subst(t, &map)
        } else {
            t
        };
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, ty: t, method, function: Some(f), of_extension: true })
    }

    /// `lookupStaticGetter` (and, without it, `lookupStaticMethod`) or
    /// `lookupStaticSetter` of `cls`
    /// (an362:src/dart/element/element.dart:5378-5403): the first static member
    /// accessible in `lib` along the implementation chain (the class, the mixins
    /// from last to first, the superclass). `(cls que o declares, é método)`.
    pub(crate) fn static_recovery(&mut self, lib: LibraryId, cls: ClassId, name: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let chain = crate::heranca::Heranca::implementation_chain(self, cls);
        let private = self.interner.resolve(name).starts_with('_');
        let accessible: Vec<ClassId> = chain.into_iter().filter(|&k| !private || self.program.class(k).library == lib).collect();
        if setter {
            let key = self.setter_key(name)?;
            return accessible.into_iter().find_map(|k| {
                let &f = self.program.class(k).static_members.get(&key)?;
                matches!(self.program.function(f).kind, FunctionKind::Setter | FunctionKind::ImplicitAccessor).then_some((k, false))
            });
        }
        let getter = accessible.iter().copied().find(|&k| {
            let ke = self.program.class(k);
            ke.enum_constants.iter().any(|&v| self.program.variable(v).name == name)
                || ke.static_members.get(&name).is_some_and(|&f| matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
        });
        if let Some(k) = getter {
            return Some((k, false));
        }
        accessible.into_iter().find(|&k| {
            self.program.class(k).static_members.get(&name).is_some_and(|&f| {
                !matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor)
            })
        }).map(|k| (k, true))
    }

    /// The static recovery of `TypePropertyResolver.resolve` for a receiver that
    /// does not have the member: for a nullable one, `Object`'s comes first;
    /// then that of the class of the receiver resolved to its bound (if it is an
    /// interface); finally that of the final `_lookupInterfaceType(objectType)`
    /// (an362:src/dart/resolver/type_property_resolver.dart:97-245). The
    /// `_getterRecovery ??=` keeps the first.
    pub(crate) fn receiver_static_recovery(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool) -> Option<(ClassId, bool)> {
        let object = self.core.object_class;
        let mut classes = Vec::new();
        if self.the_is_nullable(recv) {
            classes.extend(object);
        }
        classes.extend(self.bound_class(recv));
        classes.extend(object);
        classes.into_iter().find_map(|c| self.static_recovery(lib, c, name, setter))
    }

    /// `TypeSystemImpl.resolveToBound`: a type parameter (without `?`) through
    /// its bound, an intersection through its bound, until a type that is
    /// neither.
    pub(crate) fn resolve_to_bound(&self, t: TypeId) -> TypeId {
        let mut base = t;
        for _ in 0..64 {
            match self.table.get(base) {
                Type::TypeParameter { param, nullable: false } if *param != self.core.unknown_param => {
                    let b = self.table.param(*param).bound;
                    if b == base {
                        break;
                    }
                    base = b;
                }
                Type::Intersection { bound, .. } => base = *bound,
                _ => break,
            }
        }
        base
    }

    /// The class of `resolveToBound(recv)` when it is an interface type (class,
    /// enum, mixin or extension type), without the `?`.
    fn bound_class(&mut self, recv: TypeId) -> Option<ClassId> {
        let mut t = self.non_null(recv);
        for _ in 0..64 {
            match self.table.get(t).clone() {
                Type::Interface { class, .. } => return Some(class),
                Type::ExtensionType { decl, .. } => return Some(decl),
                Type::Intersection { bound, .. } => t = bound,
                Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                    let b = self.table.param(param).bound;
                    if b == t {
                        return None;
                    }
                    t = b;
                }
                _ => return None,
            }
            t = self.non_null(t);
        }
        None
    }

    /// `INSTANCE_ACCESS_TO_STATIC_MEMBER`: a member that does not exist on the
    /// receiver's interface is recovered as a static one
    /// ([`Self::receiver_static_recovery`]); the analyzer resolves the
    /// name to it (the type is its type) and reports the access through the
    /// instance at `span` (`_checkForStaticMember`,
    /// an362:src/dart/resolver/property_element_resolver.dart:316-358;
    /// `_reportInstanceAccessToStaticMember`,
    /// an362:src/dart/resolver/method_invocation_resolver.dart:208-256).
    pub(crate) fn instance_access_to_static(&mut self, lib: LibraryId, recv: TypeId, name: SymbolId, setter: bool, span: dartforge_diagnostics::Span) -> Option<Membro> {
        let (owner, method) = self.receiver_static_recovery(lib, recv, name, setter)?;
        let kind = if setter {
            "setter"
        } else if method {
            "method"
        } else {
            "getter"
        };
        let c = self.program.class(owner);
        let owner_type = match c.kind {
            dartforge_elements::model::ClassKind::Mixin => "mixin",
            dartforge_elements::model::ClassKind::Enum => "enum",
            dartforge_elements::model::ClassKind::ExtensionType => "extension type",
            _ => "class",
        };
        let text = self.interner.resolve(name).to_string();
        let cls = self.interner.resolve(c.name).to_string();
        self.warning_with_code(
            dartforge_diagnostics::codigos::compile_time_error::INSTANCE_ACCESS_TO_STATIC_MEMBER,
            span,
            &[&text, kind, &cls, owner_type],
        );
        self.static_member(owner, name, setter)
    }

    /// Static member of a class (a class literal as receiver): declared statics,
    /// enum constants and constructor tear-offs.
    pub(crate) fn static_member(&mut self, cls: ClassId, name: SymbolId, setter: bool) -> Option<Membro> {
        let c = self.program.class(cls);
        if !setter {
            if let Some(&v) = c.enum_constants.iter().find(|&&v| self.program.variable(v).name == name) {
                // The constant's type (`E<int>` in `a<int>()`), not `E<T>`.
                let t = self.variable_type(v);
                return Some(Membro {
                    resolved: Resolved::Member { class: cls, member: MemberRef::Variable(v), via_super: false },
                    ty: t,
                    method: false,
                    function: None,
                    of_extension: false,
                });
            }
        }
        let key = if setter { self.setter_key(name) } else { Some(name) };
        if let Some(key) = key {
            if let Some(&f) = c.static_members.get(&key) {
                let fe = self.program.function(f);
                let member = match (fe.kind, fe.variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => MemberRef::Variable(v),
                    _ => MemberRef::Function(f),
                };
                let (t, method) = self.declared_member_type(f, setter);
                return Some(Membro {
                    resolved: Resolved::Member { class: cls, member, via_super: false },
                    ty: t,
                    method,
                    function: Some(f),
                    of_extension: false,
                });
            }
        }
        None
    }

    /// Static member of an extension (`Ext.m`).
    pub(crate) fn extension_static_member(&mut self, e: ExtensionId, name: SymbolId, setter: bool) -> Option<Membro> {
        // Static extension fields have no accessors in the model (instance ones
        // do, among the instance members).
        if let Some(&v) = self
            .program
            .extension(e)
            .fields
            .iter()
            .find(|&&v| self.program.variable(v).name == name && self.program.variable(v).static_)
        {
            let t = self.variable_type(v);
            return Some(Membro {
                resolved: Resolved::Element(dartforge_elements::model::Element::Variable(v)),
                ty: t,
                method: false,
                function: None,
                of_extension: true,
            });
        }
        let key = if setter { self.setter_key(name)? } else { name };
        let &f = self.program.extension(e).static_members.get(&key)?;
        let (t, method) = self.declared_member_type(f, setter);
        Some(Membro { resolved: Resolved::ExtensionMember { extension: e, member: f }, ty: t, method, function: Some(f), of_extension: true })
    }

    /// Member declared directly in the body of a class/extension (lexical
    /// scope): instance or static.
    pub(crate) fn lexical_declared_member(&self, cls: Option<ClassId>, ext: Option<ExtensionId>, name: SymbolId, the_setter_key: Option<SymbolId>) -> Option<(FunctionElementId, bool)> {
        if let Some(c) = cls {
            let ce = self.program.class(c);
            for key in [Some(name), the_setter_key].into_iter().flatten() {
                if let Some(&f) = ce.instance_members.get(&key) {
                    return Some((f, false));
                }
                if let Some(&f) = ce.static_members.get(&key) {
                    return Some((f, true));
                }
            }
            if ce.enum_constants.iter().any(|&v| self.program.variable(v).name == name) {
                return None;
            }
        }
        if let Some(e) = ext {
            let ee = self.program.extension(e);
            for key in [Some(name), the_setter_key].into_iter().flatten() {
                if let Some(&f) = ee.instance_members.get(&key) {
                    return Some((f, false));
                }
                if let Some(&f) = ee.static_members.get(&key) {
                    return Some((f, true));
                }
            }
        }
        None
    }
}
