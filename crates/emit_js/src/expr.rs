//! Expression emission.

use crate::body::{AsyncKind, FnEmitter, js_member_name, static_member_name};
use crate::ctx::{Member, MemberKind};
use crate::js::{
    self, Js, P_ADD, P_AND, P_ASSIGN, P_BITAND, P_BITOR, P_BITXOR, P_COMMA, P_COND, P_EQ, P_MUL,
    P_OR, P_PRIMARY, P_REL, P_SHIFT, P_UNARY, P_YIELD,
};
use crate::ty::Ty;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId};
use dartforge_frontend::ast::{
    self, AssignOp, BinaryOp, CollectionElement, ExprId, ExprKind, UnaryOp,
};
use dartforge_types::resolved::{MemberRef, Resolved};
use std::collections::HashMap;

type ScoredExtensionMatch = (
    dartforge_elements::model::ExtensionId,
    dartforge_elements::model::FunctionElementId,
    HashMap<u32, Ty>,
    u32,
);
type AssignmentWriter<'m, 'a> = Box<dyn Fn(&mut FnEmitter<'m, 'a>, &Js) -> Js>;

/// Null-shorting guard: `temp` receives `init`; if it is null, the chain evaluates to `null`.
#[derive(Clone, Debug)]
pub struct Guard {
    pub temp: String,
    pub init: String,
}

/// Resolved target of a simple identifier.
pub enum IdentTarget {
    Local(String, Ty),
    /// Instance member through the implicit `this`.
    ThisMember(Member),
    /// Static member of the enclosing class (or of a superclass).
    Static(ClassId, MemberKind),
    Element(Element),
    Prefix(dartforge_intern::SymbolId),
    TypeParam(Ty),
    /// Member of the implicit `$this` receiver of an extension.
    ExtThisMember(Member),
    /// Extension member applicable to `this`.
    ThisExt,
    /// Instance member of the extension itself (called with `$this`).
    ExtMember(
        dartforge_elements::model::ExtensionId,
        dartforge_elements::model::FunctionElementId,
    ),
    ExtStatic(
        dartforge_elements::model::ExtensionId,
        dartforge_elements::model::FunctionElementId,
    ),
    /// Static field of the extension itself.
    ExtField(dartforge_elements::model::ExtensionId),
    Unknown,
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// Resolves a simple identifier.
    pub fn resolve_ident(&self, sym: dartforge_intern::SymbolId) -> IdentTarget {
        if let Some(l) = self.lookup_local(sym) {
            return IdentTarget::Local(l.js.clone(), l.ty.clone());
        }
        let n = self.name(sym);
        if let Some(t) = self.type_param_by_name(n) {
            return IdentTarget::TypeParam(t);
        }
        if let Some(t) = self.extension_this.clone() {
            // Members of the extension itself, then of the `on` type.
            if let Some(ext) = self.current_extension {
                let e = self.ctx.program.extension(ext);
                let setter = self.ctx.sym(&format!("{n}_="));
                let member = |m: &HashMap<dartforge_intern::SymbolId, _>| {
                    m.get(&sym)
                        .or_else(|| setter.and_then(|k| m.get(&k)))
                        .copied()
                };
                if let Some(fid) = member(&e.instance_members) {
                    return IdentTarget::ExtMember(ext, fid);
                }
                if let Some(fid) = member(&e.static_members) {
                    return IdentTarget::ExtStatic(ext, fid);
                }
                if e.fields
                    .iter()
                    .any(|v| self.ctx.program.variable(*v).name == sym)
                {
                    return IdentTarget::ExtField(ext);
                }
            }
            if let Some(m) = self.ctx.lookup_member(&t, n, false) {
                return IdentTarget::ExtThisMember(m);
            }
            if let Ty::Record { pos, named, .. } = &t {
                let is_field = n
                    .strip_prefix('$')
                    .and_then(|x| x.parse::<usize>().ok())
                    .is_some_and(|i| i >= 1 && i <= pos.len())
                    || named.iter().any(|(k, _)| k == n);
                if is_field {
                    return IdentTarget::ExtThisMember(Member {
                        class: self.ctx.object.unwrap_or(ClassId(0)),
                        kind: MemberKind::Field(dartforge_elements::model::VariableId(0)),
                        subst: HashMap::new(),
                    });
                }
            }
            if self.find_extension_member(&t, n, false).is_some() {
                return IdentTarget::ExtThisMember(Member {
                    class: self.ctx.object.unwrap_or(ClassId(0)),
                    kind: MemberKind::Field(dartforge_elements::model::VariableId(0)),
                    subst: HashMap::new(),
                });
            }
        }
        // Statics of the extension type whose member is being emitted (instance
        // ones arrive through the common resolution, `crate::extension_type`).
        if let Some(c) = self.extension_type {
            if let Some(mk) = self
                .ctx
                .declared_static(c, n, false)
                .or_else(|| self.ctx.declared_static(c, n, true))
            {
                return IdentTarget::Static(c, mk);
            }
        }
        if let Some(c) = self.class {
            if !self.is_static {
                if let Some(m) = self.ctx.lookup_member(&self.ctx.this_ty(c), n, false) {
                    return IdentTarget::ThisMember(m);
                }
            }
            // Statics of the class and its superclasses.
            let mut cur = Some(c);
            while let Some(k) = cur {
                if let Some(mk) = self.ctx.declared_static(k, n, false) {
                    return IdentTarget::Static(k, mk);
                }
                cur = self.ctx.superclass_of(k);
            }
            let class = self.ctx.program.class(c);
            if class.kind == ClassKind::Enum {
                if let Some(&vid) = class
                    .enum_constants
                    .iter()
                    .find(|v| self.ctx.program.variable(**v).name == sym)
                {
                    return IdentTarget::Static(c, MemberKind::Field(vid));
                }
            }
            if !self.is_static && self.ctx.program.lookup_in_unit(self.unit, sym).is_none() {
                if self
                    .find_extension_member(&self.ctx.this_ty(c), n, false)
                    .is_some()
                {
                    return IdentTarget::ThisExt;
                }
            }
        }
        if let Some(b) = self.ctx.program.lookup_in_unit(self.unit, sym) {
            if let Some(Element::Prefix(_, p)) = b.getter {
                return IdentTarget::Prefix(p);
            }
            if let Some(e) = b.getter {
                return IdentTarget::Element(e);
            }
            if let Some(e) = b.setter {
                return IdentTarget::Element(e);
            }
        }
        if self
            .ctx
            .program
            .prefixes_in_unit(self.unit)
            .contains_key(&sym)
        {
            return IdentTarget::Prefix(sym);
        }
        IdentTarget::Unknown
    }

    /// Target of the identifier `e` (named `sym`), read, called, written or
    /// used as a receiver, from the resolution of the common inference
    /// (`BodyTypes`, the same one the native backend consumes), which follows
    /// Dart's lexical scope: top-level element, local and parameter, type
    /// parameter, prefix, class member through the implicit `this` (instance
    /// or static, inherited or not, enum constant), member of the extension
    /// itself and of another extension applicable to `this`, static field of
    /// an extension. The emitter's own lookup ([`Self::resolve_ident`]) only
    /// decides what the common resolution did not record (a synthetic node,
    /// an extension's `this`, which the parser yields as an identifier) or
    /// what the emitter cannot access through the common identity (an
    /// extension receiver without a class, a dynamic member).
    /// Step 2 of the single source of inference (docs/INFERENCIA-JS-ALINHAMENTO.md).
    pub fn ident_target(&self, sym: dartforge_intern::SymbolId, e: ExprId) -> IdentTarget {
        let common = self
            .common_resolution(e)
            .and_then(|r| self.target_from_resolution(sym, r));
        self.count_target_origin(e, common.is_some());
        match common {
            Some(target) => target,
            None => self.resolve_ident(sym),
        }
    }

    /// Instance member of the access `e` with an explicit receiver (`a.b`,
    /// `a.m(…)`, `a.b = v`), whose receiver has type `recv_ty`: the
    /// declaration the common resolution recorded for the node (the most
    /// specific one through the receiver's interface, `Resolved::Member`),
    /// with the emitter's member kind and the owner's substitution through the
    /// receiver's supertype. Without a translatable common record (a
    /// synthetic node, a `dynamic` receiver, a member of an extension or of an
    /// extension type, `super`), the emitter's own lookup
    /// ([`crate::ctx::Ctx::lookup_member`]) decides.
    pub fn access_member(
        &self,
        e: Option<ExprId>,
        recv_ty: &Ty,
        name: &str,
        setter: bool,
    ) -> Option<Member> {
        let common = e.and_then(|e| self.member_from_resolution(e, recv_ty, name, setter));
        let own = || self.lookup_member_here(recv_ty, name, setter);
        if let Some(e) = e {
            self.check_member(e, name, common.as_ref(), own().as_ref());
        }
        common.or_else(own)
    }

    /// [`Ctx::lookup_member`] seen from the current library: a private name
    /// is only that library's (`c._f` does not find another library's `_f`
    /// declared in a subclass).
    pub fn lookup_member_here(&self, recv_ty: &Ty, name: &str, setter: bool) -> Option<Member> {
        let found = self.ctx.lookup_member(recv_ty, name, setter);
        if !name.starts_with('_') {
            return found;
        }
        if found
            .as_ref()
            .is_none_or(|m| self.ctx.lib_of_class(m.class) == self.lib)
        {
            return found;
        }
        let start = match recv_ty.non_null() {
            Ty::Iface { class, .. } => class,
            Ty::Param { .. } => self.ctx.resolve_param_bound(recv_ty).class()?,
            _ => return found,
        };
        let key = self.ctx.sym(&if setter {
            format!("{name}_=")
        } else {
            name.to_string()
        })?;
        let plain = self.ctx.sym(name);
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(k) = queue.pop_front() {
            if !seen.insert(k) {
                continue;
            }
            let class = self.ctx.program.class(k);
            if class.library == self.lib
                && (class.instance_members.contains_key(&key)
                    || plain.is_some_and(|p| class.instance_members.contains_key(&p)))
            {
                let sup = self.ctx.as_super(&recv_ty.non_null(), k)?;
                return self.ctx.lookup_member(&sup, name, setter);
            }
            queue.extend(
                self.ctx.class_supers[k.0 as usize]
                    .iter()
                    .filter_map(|s| s.class()),
            );
        }
        found
    }

    fn member_from_resolution(
        &self,
        e: ExprId,
        recv_ty: &Ty,
        name: &str,
        setter: bool,
    ) -> Option<Member> {
        let Resolved::Member {
            class: owner,
            member,
            via_super: false,
        } = self.common_resolution(e)?
        else {
            return None;
        };
        let p = self.ctx.program;
        let is_static = match *member {
            MemberRef::Variable(v) => p.variable(v).static_,
            MemberRef::Function(f) => p.function(f).static_,
        };
        if is_static {
            return None;
        }
        let kind = self.ctx.declared_member(*owner, name, setter)?;
        // The emitter's member kind must be the same declaration: the target
        // node of a compound write keeps a single resolution (the setter's),
        // and reading it does not use that.
        let same = match (kind, *member) {
            (
                MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f),
                MemberRef::Function(g),
            ) => f == g,
            (MemberKind::Field(v), MemberRef::Function(g)) => p.function(g).variable == Some(v),
            (MemberKind::Field(v), MemberRef::Variable(w)) => v == w,
            _ => false,
        };
        if !same {
            return None;
        }
        let params = &self.ctx.class_params[owner.0 as usize];
        let mut subst = HashMap::new();
        if !params.is_empty() {
            let sup = self.ctx.as_super(&recv_ty.non_null(), *owner)?;
            for (q, a) in params.iter().zip(sup.args().iter()) {
                subst.insert(q.id, a.clone());
            }
            for q in params.iter().skip(sup.args().len()) {
                subst.insert(q.id, Ty::Dynamic);
            }
        }
        Some(Member {
            class: *owner,
            kind,
            subst,
        })
    }

    /// Translates the common resolution `r` of the identifier `sym` into the
    /// emitter's target; `None` when there is no translation (the emitter's
    /// own lookup decides).
    fn target_from_resolution(
        &self,
        sym: dartforge_intern::SymbolId,
        r: &Resolved,
    ) -> Option<IdentTarget> {
        let p = self.ctx.program;
        let n = self.name(sym);
        match r {
            // The identity is the declaration the common resolution found; the
            // JS name and the declared type come from the emitter's scope,
            // which declared the local with the same lexical scope.
            Resolved::Local(_) | Resolved::Parameter { .. } => {
                let l = self.lookup_local(sym)?;
                Some(IdentTarget::Local(l.js.clone(), l.ty.clone()))
            }
            Resolved::TypeParameter(tp) => {
                let t = self.common_type_param_ty(*tp)?;
                Some(IdentTarget::TypeParam(t))
            }
            Resolved::Prefix(_) => Some(IdentTarget::Prefix(sym)),
            Resolved::Element(el) => match *el {
                Element::Prefix(..) => None,
                // Static field of the extension itself, read in its body: the
                // emitter accesses it through the extension.
                Element::Variable(v) if p.variable(v).extension.is_some() => {
                    let x = p.variable(v).extension?;
                    (self.current_extension == Some(x)).then_some(IdentTarget::ExtField(x))
                }
                Element::Variable(v) if p.variable(v).class.is_some() => None,
                Element::Function(f)
                    if p.function(f).class.is_some() || p.function(f).extension.is_some() =>
                {
                    None
                }
                el => Some(IdentTarget::Element(el)),
            },
            Resolved::Member {
                class: owner,
                member,
                via_super: false,
            } => {
                let (is_static, enum_constant) = match *member {
                    MemberRef::Variable(v) => {
                        let var = p.variable(v);
                        let of_enum = var
                            .class
                            .is_some_and(|c| p.class(c).enum_constants.contains(&v));
                        (var.static_ || of_enum, of_enum.then_some(v))
                    }
                    MemberRef::Function(f) => {
                        let fe = p.function(f);
                        let is_static =
                            fe.static_ || fe.variable.is_some_and(|v| p.variable(v).static_);
                        (is_static, None)
                    }
                };
                if is_static {
                    if let Some(v) = enum_constant {
                        return Some(IdentTarget::Static(*owner, MemberKind::Field(v)));
                    }
                    let mk = self
                        .ctx
                        .declared_static(*owner, n, false)
                        .or_else(|| self.ctx.declared_static(*owner, n, true))?;
                    return Some(IdentTarget::Static(*owner, mk));
                }
                if let Some(t) = &self.extension_this {
                    // Implicit `this` of an extension: the receiver is `$this`,
                    // and the access goes through the `on` type (the class that
                    // declares the member in the emitter's lookup may be the
                    // implementation, not the interface the common resolution
                    // records).
                    let m = self
                        .ctx
                        .lookup_member(t, n, false)
                        .or_else(|| self.ctx.lookup_member(t, n, true))?;
                    return Some(IdentTarget::ExtThisMember(m));
                }
                if self.is_static {
                    return None;
                }
                let c = self.class?;
                let kind = self
                    .ctx
                    .declared_member(*owner, n, false)
                    .or_else(|| self.ctx.declared_member(*owner, n, true))?;
                let sup = self.ctx.as_super(&self.ctx.this_ty(c), *owner)?;
                let subst = self.ctx.class_params[owner.0 as usize]
                    .iter()
                    .map(|q| q.id)
                    .zip(sup.args().iter().cloned())
                    .collect();
                Some(IdentTarget::ThisMember(Member {
                    class: *owner,
                    kind,
                    subst,
                }))
            }
            Resolved::ExtensionMember { extension, member } => {
                // The extension's own member (a setter is keyed `name_=`).
                if self.current_extension == Some(*extension) {
                    let fe = p.function(*member);
                    let is_static =
                        fe.static_ || fe.variable.is_some_and(|v| p.variable(v).static_);
                    return Some(if is_static {
                        IdentTarget::ExtStatic(*extension, *member)
                    } else {
                        IdentTarget::ExtMember(*extension, *member)
                    });
                }
                // Another extension applicable to the `$this` of an
                // extension: the emitter dispatches on `$this` (getter or
                // setter) and finds the same extension.
                if self.extension_this.is_some() {
                    return Some(IdentTarget::ExtThisMember(Member {
                        class: self.ctx.object.unwrap_or(ClassId(0)),
                        kind: MemberKind::Field(dartforge_elements::model::VariableId(0)),
                        subst: HashMap::new(),
                    }));
                }
                // Another extension applicable to the class's `this`: the
                // emitter dispatches on the `this` receiver and finds the same
                // extension.
                (self.class.is_some() && !self.is_static).then_some(IdentTarget::ThisExt)
            }
            Resolved::Member {
                via_super: true, ..
            }
            | Resolved::Dynamic
            | Resolved::Constructor(_) => None,
        }
    }

    /// `Ty` of a `TypeTable` type parameter in scope in this emission (of the
    /// class, of an enclosing function, or of a local generic function,
    /// translated by [`FnEmitter::shared_params`]).
    fn common_type_param_ty(&self, tp: dartforge_types::table::TypeParamId) -> Option<Ty> {
        if let Some(t) = self.shared_params.get(&tp.0) {
            return Some(t.clone());
        }
        let in_fn = self.fn_type_params.iter().any(|(id, _)| *id == tp.0);
        let in_class = self.class.is_some_and(|c| {
            self.ctx.class_params[c.0 as usize]
                .iter()
                .any(|q| q.id == tp.0)
        });
        if !(in_fn || in_class) {
            return None;
        }
        let name = self.name(self.ctx.table.param(tp).name).to_string();
        Some(Ty::Param {
            id: tp.0,
            name,
            nullable: false,
        })
    }

    /// The target the common inference recorded for `e`, if it recorded one.
    pub fn common_resolution(&self, e: ExprId) -> Option<&'a Resolved> {
        if e.0 == u32::MAX {
            return None;
        }
        self.ctx
            .bodies
            .units
            .get(self.unit.0 as usize)?
            .get_resolved(e)
    }

    /// Static type the common inference recorded for `e`, as a `Ty`, if it can
    /// be reified here (see [`Self::common_ty_in_scope`]).
    /// The `writeType` the common inference recorded for the assignment `e`.
    pub fn common_write_type(&self, e: ExprId) -> Option<Ty> {
        let id = *self
            .ctx
            .bodies
            .units
            .get(self.unit.0 as usize)?
            .write_types
            .get(&e)?;
        self.common_ty_in_scope(id)
    }

    pub fn common_type(&self, e: ExprId) -> Option<Ty> {
        if e.0 == u32::MAX {
            return None;
        }
        let id = self
            .ctx
            .bodies
            .units
            .get(self.unit.0 as usize)?
            .get_type(e)?;
        self.common_ty_in_scope(id)
    }

    /// Type arguments the common inference chose for the generic call whose
    /// argument list is `arguments`, if all of them can be reified here.
    /// `n` is the number of type parameters the emitter expects; another
    /// arity is not used.
    pub fn common_instantiation(&self, arguments: &ast::Arguments, n: usize) -> Option<Vec<Ty>> {
        let inst = (|| {
            let ids = self
                .ctx
                .bodies
                .units
                .get(self.unit.0 as usize)?
                .instantiation(arguments.span.start)?;
            let tys: Option<Vec<Ty>> = ids.iter().map(|&t| self.common_ty_in_scope(t)).collect();
            tys.filter(|t| t.len() == n)
        })();
        self.count_instantiation_origin(arguments.span.start, inst.is_some());
        inst
    }

    /// Converts a type from the common inference into a `Ty` when every free
    /// type parameter in it is in scope in this emission (of the enclosing
    /// class or of an enclosing function), that is, when the rti recipe can
    /// find it in the environment. Parameters of local generic functions (the
    /// emitter gives them its own ids, [`crate::ctx::Ctx::fresh_param`]) are
    /// translated by the [`FnEmitter::shared_params`] map; anything left out of
    /// scope (the unknown `_`, a parameter of another function) makes the
    /// conversion fail, and the caller keeps its own deduction.
    pub fn common_ty_in_scope(&self, id: dartforge_types::table::TypeId) -> Option<Ty> {
        // The context's entry point: registered bounds, erased extension
        // types and full arity, like the outline types.
        let t = self.ctx.ty_of(id).subst_prop(&self.shared_params);
        let mut free = Vec::new();
        t.collect_params(&mut free);
        let of_class = |id: u32| {
            self.class.is_some_and(|c| {
                self.ctx.class_params[c.0 as usize]
                    .iter()
                    .any(|p| p.id == id)
            })
        };
        let in_scope = free
            .iter()
            .all(|&id| self.fn_type_params.iter().any(|(p, _)| *p == id) || of_class(id));
        in_scope.then_some(t)
    }

    /// The type of `e` that emission follows: the one deduced by the emitter,
    /// replaced by the common inference's when that is a **refinement** of it
    /// (a proper subtype that is not `dynamic`): `15.clamp(0, 10)` is `int`
    /// and not `num`, `await null` is `Null`, `soma5(10)` with `call` is `int`,
    /// a promoted read has the promoted type. It never widens the emitter's
    /// type nor replaces it with an unrelated one; a node the common inference
    /// did not visit has `dynamic` and stays as it is. The target of a
    /// compound assignment and of `++`/`--` has no common type of its own
    /// ([`crate::ctx::Ctx::is_write_target`]).
    fn refine_by_common(&self, e: ExprId, ty: Ty) -> Ty {
        if e.0 == u32::MAX || self.ctx.is_write_target(self.unit, e) {
            return ty;
        }
        match self.common_type(e) {
            // The common type of a generic function value in a non-generic
            // context is already its instantiation, which `coerce_to` makes.
            Some(Ty::Fn { type_params, .. })
                if type_params.is_empty()
                    && matches!(&ty, Ty::Fn { type_params, .. } if !type_params.is_empty()) =>
            {
                ty
            }
            Some(c) if !c.is_dynamic() && c != ty && self.ctx.is_subtype(&c, &ty) => c,
            _ => ty,
        }
    }

    /// Constant boolean value of `x` proven by the world (`Liveness::const_bool`),
    /// only in the forms the world also folds (the same as in
    /// `dartforge_world::impact::constant_condition`).
    pub fn constant(&self, x: ExprId) -> Option<bool> {
        let f = self.ctx.filter?;
        match &self.expr(x).kind {
            ExprKind::Identifier(_)
            | ExprKind::Property { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Parenthesized(_) => f.const_bool(self.unit, x),
            _ => None,
        }
    }

    /// Emits an expression; `expected` is the context type (downward inference).
    pub fn emit_expr(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        if let Some(b) = self.constant(e) {
            return (
                Js::prim(if b { "true" } else { "false" }.to_string()),
                self.ctx.t_bool(),
            );
        }
        let expr = self.expr(e);
        let registration = self.register_shorthand_context(e, expected);
        let (js, ty) = match &expr.kind {
            ExprKind::Property { .. }
            | ExprKind::Index { .. }
            | ExprKind::Call { .. }
            | ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            } => {
                let (js, ty, guards) = self.emit_selector(e, expected);
                (self.wrap_guards(js, guards), ty)
            }
            _ => self.emit_expr_inner(e, expected),
        };
        let js = match self.deferred_check(e) {
            Some(check) => Js::new(
                format!("({check}, {})", js.at(crate::js::P_ASSIGN)),
                P_PRIMARY,
            ),
            None => js,
        };
        let ty = self.refine_by_common(e, ty);
        if self.ctx.type_check.is_some() {
            self.check_type(e, &ty);
        }
        if let Some((root, previous)) = registration {
            match previous {
                Some(a) => {
                    self.shorthand_contexts.insert(root, a);
                }
                None => {
                    self.shorthand_contexts.remove(&root);
                }
            }
        }
        match expected {
            Some(exp) => {
                let whole = !self.uncoerced.remove(&e);
                self.coerce_with(js, ty, exp, whole)
            }
            None => (js, ty),
        }
    }

    /// Dot shorthand (3.10): if `e` is a node of a selector chain (`.x`,
    /// `(…)`, `[…]`, `<T>`, `!`) whose root is `.id`, the chain's context
    /// (`expected`) becomes the root's *shorthand context*, unless an outer
    /// node (or `==`, or a pattern) already registered a known context.
    /// Returns what to restore at the end of the emission of `e`.
    fn register_shorthand_context(
        &mut self,
        e: ExprId,
        expected: Option<&Ty>,
    ) -> Option<(ExprId, Option<Option<Ty>>)> {
        if !self.shorthands {
            return None;
        }
        if !matches!(
            self.expr(e).kind,
            ExprKind::Property { .. }
                | ExprKind::Index { .. }
                | ExprKind::Call { .. }
                | ExprKind::TypeArguments { .. }
                | ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    ..
                }
        ) {
            return None;
        }
        let root = self.ast().shorthand_root(e)?;
        let previous = self.shorthand_contexts.get(&root).cloned();
        if matches!(previous, Some(Some(_))) || (previous.is_some() && expected.is_none()) {
            return None;
        }
        self.shorthand_contexts.insert(root, expected.cloned());
        Some((root, previous))
    }

    /// Registers `ctx` as the shorthand context at the root of `e` (if there
    /// is one) while `f` emits; `==` and patterns provide the context this way
    /// (spec 3.10, "Special case for `==`").
    pub fn with_shorthand_context<R>(
        &mut self,
        e: ExprId,
        ctx: &Ty,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let root = if self.shorthands {
            self.ast().shorthand_root(e)
        } else {
            None
        };
        let Some(root) = root else { return f(self) };
        let previous = self.shorthand_contexts.insert(root, Some(ctx.clone()));
        let r = f(self);
        match previous {
            Some(a) => {
                self.shorthand_contexts.insert(root, a);
            }
            None => {
                self.shorthand_contexts.remove(&root);
            }
        }
        r
    }

    /// The declaration that a shorthand's context denotes (spec 3.10): `C`,
    /// `C<…>` and `C?` denote `C`; `FutureOr<S>` denotes what `S` denotes;
    /// function types, records, type parameters and `dynamic` denote
    /// nothing.
    pub fn context_declaration(&self, t: &Ty) -> Option<ClassId> {
        match t {
            Ty::Iface { class, args, .. } if Some(*class) == self.ctx.future_or => {
                args.first().and_then(|a| self.context_declaration(a))
            }
            Ty::Iface { class, .. } => Some(*class),
            Ty::FutureOr { arg, .. } => self.context_declaration(arg),
            _ => None,
        }
    }

    /// `D` of a dot shorthand: from the context registered for it, otherwise
    /// from the `expected` type of its own position. Without a declaration,
    /// records the language error (outside speculative emission).
    pub(crate) fn shorthand_class(&mut self, e: ExprId, expected: Option<&Ty>) -> Option<ClassId> {
        let ctx = match self.shorthand_contexts.get(&e) {
            Some(Some(t)) => Some(t.clone()),
            _ => expected.cloned(),
        };
        let d = ctx.as_ref().and_then(|t| self.context_declaration(t));
        if d.is_none() {
            let ExprKind::DotShorthand { name, .. } = &self.expr(e).kind else {
                return None;
            };
            let msg = format!(
                "no context type to resolve the dot shorthand '.{}'",
                self.name(name.sym)
            );
            self.language_error(self.expr(e).span, msg);
        }
        d
    }

    /// Records a language error during emission (ignored when the emission
    /// only serves to find out a type).
    pub fn language_error(&self, span: dartforge_diagnostics::Span, msg: String) {
        if self.speculating > 0 {
            return;
        }
        let file = self
            .ctx
            .program
            .unit(self.unit)
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.ctx
            .errors
            .borrow_mut()
            .push(dartforge_diagnostics::Diagnostic::new(
                format!(
                    "{}{file}:{}: {msg}",
                    dartforge_types::codes::LANGUAGE_ERROR,
                    span.start
                ),
                span,
            ));
    }

    /// `.id` / `.new` without a call: `D.id` (getter, field, enum constant,
    /// method or constructor tear-off).
    fn emit_dot_shorthand(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        let ExprKind::DotShorthand { name, .. } = &self.expr(e).kind else {
            unreachable!()
        };
        let name = self.name(name.sym).to_string();
        let span = self.expr(e).span;
        let Some(d) = self.shorthand_class(e, expected) else {
            return (Js::prim("null"), Ty::Dynamic);
        };
        match self.static_member_get(d, &name) {
            Some(r) => r,
            None => {
                let msg = format!(
                    "'{}' has no static member or constructor '{name}' for the dot shorthand",
                    self.ctx.class_name(d)
                );
                self.language_error(span, msg);
                (Js::prim("null"), Ty::Dynamic)
            }
        }
    }

    /// Implicit coercions: instantiation of a generic tearoff and `.call` of a callable class.
    pub fn coerce_to(&mut self, js: Js, ty: Ty, expected: &Ty) -> (Js, Ty) {
        self.coerce_with(js, ty, expected, true)
    }

    /// [`Self::coerce_to`]; without `whole`, only the generic instantiation:
    /// the downcast and the `call` tear-off apply to the enclosing expression.
    fn coerce_with(&mut self, js: Js, ty: Ty, expected: &Ty, whole: bool) -> (Js, Ty) {
        if !whole && !matches!(ty, Ty::Fn { .. }) {
            return (js, ty);
        }
        if ty.is_dynamic() && !self.in_const {
            let needs_cast = match expected {
                Ty::Iface {
                    class, nullable, ..
                } => !(Some(*class) == self.ctx.object && *nullable),
                Ty::Fn { .. } | Ty::Record { .. } | Ty::FutureOr { .. } | Ty::Param { .. } => true,
                _ => false,
            };
            if needs_cast
                && js.code != "null"
                && !self.ctx.filter.is_some_and(|f| f.omit_implicit_checks())
            {
                return (self.as_cast(&js, expected), expected.clone());
            }
            return (js, ty);
        }
        // `Function`/`Function?` context: a callable class is torn off too.
        if matches!(expected, Ty::Iface { class, .. } if Some(*class) == self.ctx.function_)
            && let Ty::Iface {
                class,
                nullable: false,
                ..
            } = &ty
            && Some(*class) != self.ctx.function_
            && let Some(m) = self.ctx.lookup_member(&ty, "call", false)
            && matches!(m.kind, MemberKind::Method(_))
        {
            let mty = self.ctx.member_ty(&m);
            let key = self.member_key_string(&ty, "call");
            return (Js::prim(format!("dart.bind({}, {key})", js.code)), mty);
        }
        let Ty::Fn {
            type_params: etps, ..
        } = expected
        else {
            return (js, ty);
        };
        if !etps.is_empty() {
            return (js, ty);
        }
        match &ty {
            Ty::Fn { type_params, .. } if !type_params.is_empty() => {
                // Instantiate with the arguments inferred from the expected type.
                let free: Vec<u32> = type_params.iter().map(|p| p.id).collect();
                let inner = match &ty {
                    Ty::Fn {
                        ret,
                        pos,
                        opt,
                        named,
                        nullable,
                        ..
                    } => Ty::Fn {
                        type_params: vec![],
                        ret: ret.clone(),
                        pos: pos.clone(),
                        opt: opt.clone(),
                        named: named.clone(),
                        nullable: *nullable,
                    },
                    t => t.clone(),
                };
                let mut subst = HashMap::new();
                self.match_type(&inner, expected, &free, &mut subst);
                let mut rtis = Vec::new();
                let mut constant = true;
                for p in type_params {
                    let t = subst
                        .get(&p.id)
                        .cloned()
                        .unwrap_or_else(|| self.default_type_arg(&p.bound));
                    constant &= !t.mentions_params();
                    subst.insert(p.id, t.clone());
                    rtis.push(self.rti(&t));
                }
                let code = format!("dart.gbind({}, {})", js.code, rtis.join(", "));
                (
                    Js::prim(self.constant_instantiation(&js, code, constant)),
                    inner.subst(&subst),
                )
            }
            Ty::Iface {
                class,
                nullable: false,
                ..
            } if Some(*class) != self.ctx.function_ => {
                // Callable class: tearoff of `call`.
                match self.ctx.lookup_member(&ty, "call", false) {
                    Some(m) if matches!(m.kind, MemberKind::Method(_)) => {
                        let mty = self.ctx.member_ty(&m);
                        let key = self.member_key_string(&ty, "call");
                        (Js::prim(format!("dart.bind({}, {key})", js.code)), mty)
                    }
                    _ => (js, ty),
                }
            }
            _ => (js, ty),
        }
    }

    /// Member name as a JS expression (string, `dartx` symbol or private symbol) for `dart.bind`.
    pub fn member_key_string(&self, recv_ty: &Ty, name: &str) -> String {
        if name.starts_with('_') {
            let lib = self
                .lookup_member_here(recv_ty, name, false)
                .map(|m| self.ctx.lib_of_class(m.class))
                .unwrap_or(self.lib);
            self.private_sym(lib, name)
        } else if self.ctx.is_ext_member(recv_ty, name, false) {
            self.dartx(&js_member_name(name))
        } else {
            js::string_literal(&js_member_name(name))
        }
    }

    /// `-x`, `~x` through an applicable (or the explicit) extension:
    /// `Ext|unary-`/`Ext|~`.
    fn unary_extension_call(&mut self, v: &Js, vty: &Ty, op: &str) -> Option<(Js, Ty)> {
        let found = self.find_extension_member(vty, op, false);
        self.forced_ext = None;
        let (ext, fid, subst) = found?;
        let e = self.ctx.program.extension(ext);
        let lib_var = self.lib_var(e.library);
        let ext_name = self.extension_js_name(ext);
        let ret = match self.ctx.fn_ty(fid).subst_prop(&subst) {
            Ty::Fn { ret, .. } => (*ret).clone(),
            _ => Ty::Dynamic,
        };
        let mut args = self.ext_type_args(ext, &subst);
        args.push(v.code.clone());
        Some((
            Js::prim(format!(
                "{lib_var}[{}]({})",
                js::string_literal(&format!("{ext_name}|{op}")),
                args.join(", ")
            )),
            ret,
        ))
    }

    /// The instantiation `code` (a `dart.gbind`) of `f`: when `f` is a static
    /// or top-level function and the type arguments are `constant`, a
    /// constant (`identical` to every other one), as the CFE makes it.
    fn constant_instantiation(&mut self, f: &Js, code: String, constant: bool) -> String {
        if !constant || !f.code.starts_with("dart.gFn(L$") {
            return code;
        }
        self.m.note_const_record(self.lib);
        let lv = self.lib_var(self.lib);
        format!("{lv}.$C({}, () => {code})", js::string_literal(&code))
    }

    pub fn wrap_guards(&mut self, js: Js, guards: Vec<Guard>) -> Js {
        if guards.is_empty() {
            return js;
        }
        let mut code = js.code;
        for g in guards.iter().rev() {
            code = format!(
                "{} = {}, {} == null ? null : {}",
                g.temp, g.init, g.temp, code
            );
        }
        Js::new(code, P_COMMA).paren()
    }

    fn emit_expr_inner(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        let expr = self.expr(e);
        match &expr.kind {
            ExprKind::Int(span) => {
                let text = self.text(*span).replace('_', "");
                let v = parse_int(&text);
                let is_double_ctx = expected.is_some_and(|t| self.ctx.is_double(t));
                if is_double_ctx {
                    (Js::prim(format!("{v}.0")), self.ctx.t_double())
                } else {
                    (Js::prim(v), self.ctx.t_int())
                }
            }
            ExprKind::Double(span) => {
                let text = self.text(*span).replace('_', "");
                let v: f64 = text.parse().unwrap_or(0.0);
                let s = if v.fract() == 0.0 && v.abs() < 1e21 {
                    format!("{v:.1}")
                } else {
                    let t = text.trim_start_matches('0');
                    if t.starts_with('.') {
                        format!("0{t}")
                    } else {
                        t.to_string()
                    }
                };
                (Js::prim(s), self.ctx.t_double())
            }
            ExprKind::Bool(b) => (
                Js::prim(if *b { "true" } else { "false" }),
                self.ctx.t_bool(),
            ),
            ExprKind::Null => (Js::prim("null"), Ty::Null),
            ExprKind::String(lit) => self.emit_string_lit(lit),
            ExprKind::Symbol(names) => {
                let text: Vec<&str> = names.iter().map(|n| self.name(n.sym)).collect();
                self.m.use_sdk("_internal");
                (
                    Js::prim(format!(
                        "dart.const(new _internal.Symbol.new({}))",
                        js::string_literal(&text.join("."))
                    )),
                    self.ctx.symbol_.map(Ty::iface).unwrap_or(Ty::Dynamic),
                )
            }
            ExprKind::Identifier(name) => self.emit_identifier(name.sym, e),
            ExprKind::DotShorthand { .. } => self.emit_dot_shorthand(e, expected),
            ExprKind::This => self.emit_this(),
            ExprKind::Super => {
                let ty = self.super_ty();
                (Js::prim("super"), ty)
            }
            ExprKind::Parenthesized(inner) => {
                if self.uncoerced.contains(&e) {
                    self.uncoerced.insert(*inner);
                }
                let (js, ty) = self.emit_expr(*inner, expected);
                (js.paren(), ty)
            }
            ExprKind::List {
                const_,
                type_args,
                elements,
            } => {
                let common = if type_args.is_empty() {
                    self.common_type(e)
                } else {
                    None
                };
                self.emit_list_literal(*const_, type_args, elements, expected, common.as_ref(), e)
            }
            ExprKind::SetOrMap {
                const_,
                type_args,
                elements,
            } => {
                let common = if type_args.is_empty() {
                    self.common_type(e)
                } else {
                    None
                };
                self.emit_set_or_map(*const_, type_args, elements, expected, common.as_ref())
            }
            ExprKind::Record {
                positional,
                named,
                const_,
            } => {
                let saved = self.in_const;
                self.in_const |= *const_;
                let record = self.emit_record(e, positional, named, expected);
                self.in_const = saved;
                record
            }
            ExprKind::InstanceCreation {
                keyword,
                ty,
                constructor,
                arguments,
            } => self.emit_instance_creation(
                *keyword,
                *ty,
                constructor.as_ref(),
                arguments,
                expected,
            ),
            ExprKind::FunctionExpression(fid) => {
                let saved_const = self.in_const;
                self.in_const = false;
                // The runtime type of a generator without a written return
                // type (the CFE's rule, which gives the `runtimeType`) takes
                // precedence over the static one, which follows the analyzer
                // (`tipos_de_execucao_de_funcoes`).
                let common = self
                    .ctx
                    .bodies
                    .units
                    .get(self.unit.0 as usize)
                    .and_then(|u| u.function_runtime_type(*fid).or_else(|| u.get_type(e)));
                let r = self.emit_function_expr(*fid, expected, common);
                self.in_const = saved_const;
                r
            }
            ExprKind::TypeArguments { target, type_args } => {
                let tys: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(*t)).collect();
                if let ExprKind::Identifier(id) = &self.expr(*target).kind {
                    if let IdentTarget::Element(Element::Class(c)) =
                        self.ident_target(id.sym, *target)
                    {
                        let t = Ty::Iface {
                            class: c,
                            args: tys,
                            nullable: false,
                        };
                        let rti = self.rti(&t);
                        if self.in_foreign_js {
                            return (Js::prim(rti), self.ctx.t_type());
                        }
                        return (
                            Js::prim(format!("dart_rti.createRuntimeType({rti})")),
                            self.ctx.t_type(),
                        );
                    }
                    // `G<int>` of `typedef G<T> = List<T>`: the aliased type.
                    if let IdentTarget::Element(Element::Typedef(td)) =
                        self.ident_target(id.sym, *target)
                    {
                        let data = &self.ctx.outline.typedefs[td.0 as usize];
                        let map: HashMap<u32, Ty> = data
                            .type_params
                            .iter()
                            .map(|p| p.0)
                            .zip(tys.iter().cloned())
                            .collect();
                        let t = self.ctx.ty_of(data.target_type).subst_prop(&map);
                        let rti = self.rti(&t);
                        if self.in_foreign_js {
                            return (Js::prim(rti), self.ctx.t_type());
                        }
                        return (
                            Js::prim(format!("dart_rti.createRuntimeType({rti})")),
                            self.ctx.t_type(),
                        );
                    }
                }
                let (fjs, fty) = self.emit_expr(*target, None);
                let rtis: Vec<String> = tys.iter().map(|t| self.rti(t)).collect();
                let ret_ty = match &fty {
                    Ty::Fn { type_params, .. } => {
                        let mut map = HashMap::new();
                        for (p, a) in type_params.iter().zip(tys.iter()) {
                            map.insert(p.id, a.clone());
                        }
                        match fty.subst(&map) {
                            Ty::Fn {
                                ret,
                                pos,
                                opt,
                                named,
                                nullable,
                                ..
                            } => Ty::Fn {
                                type_params: vec![],
                                ret,
                                pos,
                                opt,
                                named,
                                nullable,
                            },
                            t => t,
                        }
                    }
                    _ => Ty::Dynamic,
                };
                let code = format!("dart.gbind({}, {})", fjs.code, rtis.join(", "));
                let constant = tys.iter().all(|t| !t.mentions_params());
                (
                    Js::prim(self.constant_instantiation(&fjs, code, constant)),
                    ret_ty,
                )
            }
            ExprKind::Unary {
                op: op @ (UnaryOp::Neg | UnaryOp::BitNot),
                operand,
            } if self.ext_type_member(e).is_some() => {
                let _ = op;
                let Some((owner, MemberRef::Function(f))) = self.ext_type_member(e) else {
                    unreachable!()
                };
                let (v, _) = self.emit_expr(*operand, None);
                self.ext_type_operator(&v, Some(*operand), owner, f, &[])
            }
            ExprKind::Unary { op, operand } => self.emit_unary(*op, *operand, e),
            ExprKind::Binary { op, left, right }
                if !matches!(
                    op,
                    BinaryOp::And
                        | BinaryOp::Or
                        | BinaryOp::IfNull
                        | BinaryOp::Eq
                        | BinaryOp::NotEq
                ) && self.ext_type_member(e).is_some() =>
            {
                // Operator declared in an extension type: the helper function.
                let Some((owner, MemberRef::Function(f))) = self.ext_type_member(e) else {
                    unreachable!()
                };
                let (l, _) = self.emit_expr(*left, None);
                let ctx_r = self.ext_type_param(Some(*left), owner, f, 0);
                let (r, _) = self.emit_expr(*right, ctx_r.as_ref());
                self.ext_type_operator(&l, Some(*left), owner, f, &[r])
            }
            ExprKind::Binary { op, left, right } => {
                self.emit_binary(*op, *left, *right, expected, e)
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } if self.constant(*condition).is_some() => {
                let branch = if self.constant(*condition) == Some(true) {
                    *then
                } else {
                    *else_
                };
                self.emit_expr(branch, expected)
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.pending_promotions.clear();
                self.negated_promotions.clear();
                let (c, _) = self.emit_cond(*condition);
                let promos = std::mem::take(&mut self.pending_promotions);
                let neg = std::mem::take(&mut self.negated_promotions);
                self.push_scope();
                for (sym, t) in &promos {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                self.uncoerced.insert(*then);
                let (a, at) = self.emit_expr(*then, expected);
                self.pop_scope();
                self.push_scope();
                for (sym, t) in &neg {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                self.uncoerced.insert(*else_);
                let (b, bt) = self.emit_expr(*else_, expected);
                self.pop_scope();
                let ty = self.ctx.lub(&at, &bt);
                (
                    Js::new(format!("{c} ? {} : {}", a.at(P_COND), b.at(P_COND)), P_COND),
                    ty,
                )
            }
            ExprKind::Is { value, ty, negated } => {
                let (v, vty) = self.emit_expr(*value, None);
                let t = self.resolve_type(*ty);
                let test = self.is_test(&v, &t);
                // Local variable promotion.
                if let ExprKind::Identifier(n) = &self.expr(*value).kind {
                    if self.lookup_local(n.sym).is_some()
                        && (self.ctx.is_subtype(&t, &vty) || vty.is_dynamic())
                    {
                        if *negated {
                            self.negated_promotions.push((n.sym, t.clone()));
                        } else {
                            self.pending_promotions.push((n.sym, t.clone()));
                        }
                    }
                }
                if *negated {
                    (
                        Js::new(format!("!{}", test.at(P_UNARY)), P_UNARY),
                        self.ctx.t_bool(),
                    )
                } else {
                    (test, self.ctx.t_bool())
                }
            }
            ExprKind::As { value, ty } => {
                let t = self.resolve_type(*ty);
                let (v, vty) = self.emit_expr(*value, Some(&t));
                if self.ctx.is_subtype(&vty, &t) && !vty.is_dynamic() {
                    return (v, t);
                }
                (self.as_cast(&v, &t), t)
            }
            ExprKind::Assign { op, target, value } => self.emit_assign(*op, *target, *value, e),
            ExprKind::PatternAssign { pattern, value } => {
                let (vjs, vty) = self.emit_expr(*value, None);
                let t = self.temp();
                let mut binds = Vec::new();
                self.pattern_assign = true;
                let cond = self.pattern_cond(*pattern, &t, &vty, &mut binds, true);
                self.pattern_assign = false;
                let code = if cond == "true" {
                    format!("{t} = {}, {t}", vjs.code)
                } else {
                    self.m.use_sdk("core");
                    format!(
                        "{t} = {}, ({cond}) || dart.throw(new core.StateError.new(\"Pattern matching error\")), {t}",
                        vjs.code
                    )
                };
                (Js::new(code, P_COMMA).paren(), vty)
            }
            ExprKind::Cascade {
                target,
                sections,
                null_aware,
            } => {
                self.uncoerced.insert(*target);
                let (tjs, tty) = self.emit_expr(*target, expected);
                let t = self.temp();
                self.cascade.push((t.clone(), tty.clone()));
                let mut parts = Vec::new();
                for s in sections.iter() {
                    let (sjs, _) = self.emit_expr(*s, None);
                    parts.push(sjs.into_at(P_ASSIGN));
                }
                self.cascade.pop();
                let body = if parts.is_empty() {
                    t.clone()
                } else {
                    format!("{}, {t}", parts.join(", "))
                };
                let code = if *null_aware {
                    format!("{t} = {}, {t} == null ? null : ({body})", tjs.code)
                } else {
                    format!("{t} = {}, {body}", tjs.code)
                };
                (Js::new(code, P_COMMA).paren(), tty)
            }
            ExprKind::CascadeTarget => {
                let (t, ty) = self
                    .cascade
                    .last()
                    .cloned()
                    .unwrap_or(("null".into(), Ty::Dynamic));
                (Js::prim(t), ty)
            }
            ExprKind::Await(inner) => {
                let (v, vty) = self.emit_expr(*inner, None);
                let ty = match &vty {
                    Ty::FutureOr { arg, .. } => (**arg).clone(),
                    Ty::Iface { .. } => {
                        match self.ctx.future_.and_then(|f| self.ctx.as_super(&vty, f)) {
                            Some(ft) => ft.args().first().cloned().unwrap_or(Ty::Dynamic),
                            None => vty.clone(),
                        }
                    }
                    // `flatten(X)` is that of its bound when the bound is a
                    // future type (`X extends FutureOr<A>` flattens to `A`).
                    Ty::Param { .. } => match self.ctx.resolve_param_bound(&vty) {
                        Ty::FutureOr { arg, .. } => (*arg).clone(),
                        bound => {
                            match self.ctx.future_.and_then(|f| self.ctx.as_super(&bound, f)) {
                                Some(ft) => ft.args().first().cloned().unwrap_or(Ty::Dynamic),
                                None => vty.clone(),
                            }
                        }
                    },
                    _ => Ty::Dynamic,
                };
                // A value whose static type is not a future is awaited only if
                // it is a `Future<flatten(S)>` (the CFE's runtime check type):
                // `await o` with `Object o = Future<Object?>…` keeps the future.
                let needs_check = match &vty {
                    Ty::FutureOr { .. } => true,
                    Ty::Param { .. } => self
                        .ctx
                        .future_
                        .and_then(|f| self.ctx.as_super(&vty, f))
                        .is_none(),
                    Ty::Iface { class, .. } => {
                        self.ctx
                            .future_
                            .and_then(|f| self.ctx.as_super(&vty, f))
                            .is_none()
                            && ![
                                self.ctx.int_,
                                self.ctx.double_,
                                self.ctx.num_,
                                self.ctx.bool_,
                                self.ctx.string_,
                            ]
                            .contains(&Some(*class))
                    }
                    _ => false,
                };
                let v = match self.ctx.future_ {
                    Some(future) if needs_check => {
                        self.m.use_sdk("async");
                        let check = Ty::iface_args(future, vec![ty.clone()]);
                        Js::prim(format!(
                            "async.awaitWithTypeCheck({}, {})",
                            self.rti(&check),
                            v.code
                        ))
                    }
                    _ => v,
                };
                (
                    Js::new(format!("yield {}", v.at(P_YIELD)), P_YIELD).paren(),
                    ty,
                )
            }
            ExprKind::Throw(inner) => {
                let (v, _) = self.emit_expr(*inner, None);
                (Js::prim(format!("dart.throw({})", v.code)), Ty::Never)
            }
            ExprKind::Rethrow => {
                let v = self
                    .rethrow_var
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "null".into());
                (Js::prim(format!("dart.rethrow({v})")), Ty::Never)
            }
            ExprKind::Switch { value, cases } => self.emit_switch_expr(*value, cases, expected),
            ExprKind::Property { .. } | ExprKind::Index { .. } | ExprKind::Call { .. } => {
                let (js, ty, guards) = self.emit_selector(e, expected);
                (self.wrap_guards(js, guards), ty)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Literals
    // -----------------------------------------------------------------------

    fn emit_string_lit(&mut self, lit: &ast::StringLit) -> (Js, Ty) {
        let mut parts: Vec<String> = Vec::new();
        let mut pending: Vec<u16> = Vec::new();
        for part in lit.parts.iter() {
            match part {
                ast::StringPart::Text(t) => pending.extend(t.code_units()),
                ast::StringPart::Interpolation(e) => {
                    if !pending.is_empty() {
                        parts.push(js::string_literal_units(pending.drain(..)));
                    }
                    let (v, vty) = self.emit_expr(*e, None);
                    if self.ctx.is_string(&vty) {
                        parts.push(v.into_at(P_ADD));
                    } else if self.ctx.is_num_like(&vty) && !vty.is_class(self.ctx.num_)
                        || self.ctx.is_bool(&vty)
                    {
                        parts.push(format!("dart.strSafe({})", v.code));
                    } else {
                        parts.push(format!("dart.str({})", v.code));
                    }
                }
            }
        }
        if !pending.is_empty() || parts.is_empty() {
            parts.push(js::string_literal_units(pending.drain(..)));
        }
        if parts.len() == 1 {
            return (Js::new(parts.remove(0), P_PRIMARY), self.ctx.t_string());
        }
        // Make sure the concatenation starts with a string.
        if !parts[0].starts_with('"') {
            parts.insert(0, "\"\"".into());
        }
        (Js::new(parts.join(" + "), P_ADD), self.ctx.t_string())
    }

    /// Expected element type for a collection literal.
    fn expected_arg(
        &self,
        expected: Option<&Ty>,
        class: Option<ClassId>,
        idx: usize,
    ) -> Option<Ty> {
        let e = expected?;
        let c = class?;
        let sup = self.ctx.as_super(&e.non_null(), c)?;
        sup.args().get(idx).cloned()
    }

    /// List literal. `common` is the type the common inference gave the
    /// literal without written type arguments: the element type comes from it
    /// (with the context and the upper bound of the specification); without
    /// it, from the context and the elements by the emitter's own deduction.
    fn emit_list_literal(
        &mut self,
        const_: bool,
        type_args: &[ast::TypeId],
        elements: &[CollectionElement],
        expected: Option<&Ty>,
        common: Option<&Ty>,
        node: ExprId,
    ) -> (Js, Ty) {
        let const_ = const_ || self.in_const;
        let saved_const = self.in_const;
        if const_ {
            self.in_const = true;
        }
        let common = common
            .filter(|t| t.is_class(self.ctx.list_))
            .and_then(|t| t.args().first().cloned());
        let r = self.emit_list_literal_inner(const_, type_args, elements, expected, common, node);
        self.in_const = saved_const;
        r
    }

    fn emit_list_literal_inner(
        &mut self,
        const_: bool,
        type_args: &[ast::TypeId],
        elements: &[CollectionElement],
        expected: Option<&Ty>,
        common: Option<Ty>,
        node: ExprId,
    ) -> (Js, Ty) {
        let elem_ty = if let Some(t) = type_args.first() {
            self.resolve_type(*t)
        } else if let Some(t) = common {
            t
        } else if let Some(t) = self
            .expected_arg(expected, self.ctx.list_, 0)
            .or_else(|| self.expected_arg(expected, self.ctx.iterable_, 0))
        {
            t
        } else {
            self.infer_elements_ty(elements)
        };
        let list_ty = self.ctx.t_list(elem_ty.clone());
        let simple = elements
            .iter()
            .all(|e| matches!(e, CollectionElement::Expression(_)));
        if simple {
            let items: Vec<String> = elements
                .iter()
                .map(|e| match e {
                    CollectionElement::Expression(x) => {
                        self.emit_expr(*x, Some(&elem_ty)).0.into_at(P_ASSIGN)
                    }
                    _ => unreachable!(),
                })
                .collect();
            if const_ {
                let rti = self.rti(&elem_ty);
                return (
                    Js::prim(format!("dart.constList({rti}, [{}])", items.join(", "))),
                    list_ty,
                );
            }
            if elem_ty.is_dynamic() {
                return (Js::prim(format!("[{}]", items.join(", "))), list_ty);
            }
            let jsarr = self
                .ctx
                .js_array
                .map(|c| Ty::iface_args(c, vec![elem_ty.clone()]))
                .unwrap_or(list_ty.clone());
            let rti = self.rti(&jsarr);
            self.m.use_sdk("_interceptors");
            if let Some(fact) = self
                .fresh_list_append
                .get(&node)
                .cloned()
                .filter(|f| f.initializer == node && f.element == elem_ty && items.is_empty())
            {
                let helper = crate::fresh_list_append::helper_name(self.ctx);
                let owner = self.temp();
                let factory = self.temp();
                return (
                    Js::prim(format!(
                        "({owner} = _interceptors.JSArray, {factory} = {owner}.of, {} = {helper}.create({factory}, {owner}, {rti}, []), {}.list)",
                        fact.token, fact.token
                    )),
                    list_ty,
                );
            }
            return (
                Js::prim(format!(
                    "_interceptors.JSArray.of({rti}, [{}])",
                    items.join(", ")
                )),
                list_ty,
            );
        }
        // General form: an IIFE that builds the list.
        let t = self.temp();
        let saved_w = std::mem::take(&mut self.w);
        let jsarr = self
            .ctx
            .js_array
            .map(|c| Ty::iface_args(c, vec![elem_ty.clone()]))
            .unwrap_or(list_ty.clone());
        let rti = self.rti(&jsarr);
        self.m.use_sdk("_interceptors");
        if const_ {
            crate::emit_line!(self.w, "{t} = [];");
        } else {
            crate::emit_line!(self.w, "{t} = _interceptors.JSArray.of({rti}, []);");
        }
        let t2 = t.clone();
        for el in elements {
            self.emit_collection_element(el, &elem_ty, None, &|s: &mut Self, v: String| {
                crate::emit_line!(s.w, "{t2}.push({v});");
            });
        }
        if const_ {
            let er = self.rti(&elem_ty);
            crate::emit_line!(self.w, "return dart.constList({er}, {t});");
        } else {
            crate::emit_line!(self.w, "return {t};");
        }
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), list_ty)
    }

    fn infer_elements_ty(&mut self, elements: &[CollectionElement]) -> Ty {
        // Without emitting: types the elements in a throwaway copy of the writer.
        let mut tys = Vec::new();
        let saved = std::mem::take(&mut self.w);
        let saved_temps = self.temps.len();
        let saved_counter = self.temp_counter;
        for el in elements {
            match el {
                CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => {
                    let t = self.type_of(*e);
                    tys.push(if matches!(el, CollectionElement::NullAwareExpression(_)) {
                        t.non_null()
                    } else {
                        t
                    });
                }
                CollectionElement::Spread { value, .. } => {
                    let t = self.type_of(*value);
                    if let Some(it) = self
                        .ctx
                        .iterable_
                        .and_then(|i| self.ctx.as_super(&t.non_null(), i))
                    {
                        tys.push(it.args().first().cloned().unwrap_or(Ty::Dynamic));
                    } else if !matches!(t, Ty::Null) {
                        tys.push(Ty::Dynamic);
                    }
                }
                CollectionElement::If { then, else_, .. } => {
                    tys.push(self.infer_elements_ty(std::slice::from_ref(then)));
                    if let Some(e) = else_ {
                        tys.push(self.infer_elements_ty(std::slice::from_ref(e)));
                    }
                }
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    self.push_scope();
                    if let CollectionElement::ForIn {
                        target: ast::ForInTarget::Declared { name, ty, .. },
                        iterable,
                        ..
                    } = el
                    {
                        let it = self.type_of(*iterable);
                        let et = ty.map(|t| self.resolve_type(t)).unwrap_or_else(|| {
                            self.ctx
                                .iterable_
                                .and_then(|i| self.ctx.as_super(&it, i))
                                .and_then(|t| t.args().first().cloned())
                                .unwrap_or(Ty::Dynamic)
                        });
                        self.declare(name.sym, et);
                    }
                    if let CollectionElement::For {
                        init: Some(ast::ForInit::Variables(list)),
                        ..
                    } = el
                    {
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let t = match (declared.clone(), v.initializer) {
                                (Some(d), _) => d,
                                (None, Some(i)) => self.type_of(i),
                                _ => Ty::Dynamic,
                            };
                            self.declare(v.name.sym, t);
                        }
                    }
                    tys.push(self.infer_elements_ty(std::slice::from_ref(body)));
                    self.pop_scope();
                }
                CollectionElement::MapEntry { .. } => tys.push(Ty::Dynamic),
            }
        }
        self.w = saved;
        self.temps.truncate(saved_temps);
        self.temp_counter = saved_counter;
        let mut it = tys.into_iter();
        match it.next() {
            None => Ty::Dynamic,
            Some(first) => it.fold(first, |a, b| self.ctx.lub(&a, &b)),
        }
    }

    /// Static type of an expression without emitting it (discards the text).
    pub fn type_of(&mut self, e: ExprId) -> Ty {
        let saved = std::mem::take(&mut self.w);
        let saved_temps = self.temps.len();
        let saved_counter = self.temp_counter;
        let saved_ret = self.returns.len();
        self.speculating += 1;
        let (_, t) = self.emit_expr(e, None);
        self.speculating -= 1;
        self.w = saved;
        self.temps.truncate(saved_temps);
        self.temp_counter = saved_counter;
        self.returns.truncate(saved_ret);
        t
    }

    /// IIFE (or a generator `yield*` inside generators).
    pub fn iife(&self, body: &str) -> Js {
        match self.async_kind {
            AsyncKind::None => Js::prim(format!("(() => {{\n{body}}})()")),
            _ => Js::new(
                format!("yield* (function*() {{\n{body}}}).call(this)"),
                P_YIELD,
            )
            .paren(),
        }
    }

    fn emit_collection_element(
        &mut self,
        el: &CollectionElement,
        elem_ty: &Ty,
        value_ty: Option<&Ty>,
        add: &dyn Fn(&mut Self, String),
    ) {
        match el {
            CollectionElement::Expression(e) => {
                let (v, _) = self.emit_expr(*e, Some(elem_ty));
                add(self, v.code);
            }
            CollectionElement::NullAwareExpression(e) => {
                // Nullable context for the operand (spec 3.8): the `null` is
                // discarded here, it does not reach the literal.
                let (v, _) = self.emit_expr(*e, Some(&elem_ty.with_nullable(true)));
                let t = self.temp();
                crate::emit_line!(self.w, "{t} = {};", v.code);
                crate::open_block!(self.w, "if ({t} != null) {{");
                add(self, t);
                self.w.close("}");
            }
            CollectionElement::MapEntry {
                key,
                value,
                null_aware_key: false,
                null_aware_value: false,
            } => {
                let (k, _) = self.emit_expr(*key, Some(elem_ty));
                let (v, _) = self.emit_expr(*value, value_ty);
                add(self, format!("{}\u{0}{}", k.code, v.code));
            }
            CollectionElement::MapEntry {
                key,
                value,
                null_aware_key,
                null_aware_value,
            } => {
                // Null-aware entry (3.8): the key before the value; a null
                // null-aware key discards the entry WITHOUT evaluating the value.
                let kctx = if *null_aware_key {
                    elem_ty.with_nullable(true)
                } else {
                    elem_ty.clone()
                };
                let (k, _) = self.emit_expr(*key, Some(&kctx));
                let tk = self.temp();
                crate::emit_line!(self.w, "{tk} = {};", k.code);
                if *null_aware_key {
                    crate::open_block!(self.w, "if ({tk} != null) {{");
                }
                let vctx = value_ty.map(|t| {
                    if *null_aware_value {
                        t.with_nullable(true)
                    } else {
                        t.clone()
                    }
                });
                let (v, _) = self.emit_expr(*value, vctx.as_ref());
                let tv = self.temp();
                crate::emit_line!(self.w, "{tv} = {};", v.code);
                if *null_aware_value {
                    crate::open_block!(self.w, "if ({tv} != null) {{");
                }
                add(self, format!("{tk}\u{0}{tv}"));
                if *null_aware_value {
                    self.w.close("}");
                }
                if *null_aware_key {
                    self.w.close("}");
                }
            }
            CollectionElement::Spread { value, null_aware } => {
                let (v, vty) = self.emit_expr(*value, None);
                let t = self.temp();
                crate::emit_line!(self.w, "{t} = {};", v.code);
                if *null_aware {
                    crate::open_block!(self.w, "if ({t} != null) {{");
                }
                // A `dynamic` spread is cast to `Iterable`/`Map` and each
                // element to the literal's type, as the CFE does.
                let checked =
                    vty.is_dynamic() && !self.ctx.filter.is_some_and(|f| f.omit_implicit_checks());
                let vty = if checked {
                    let class = if value_ty.is_some() {
                        self.ctx.map_
                    } else {
                        self.ctx.iterable_
                    };
                    match class {
                        Some(class) => {
                            let n = if value_ty.is_some() { 2 } else { 1 };
                            let target = Ty::iface_args(class, vec![Ty::Dynamic; n]);
                            let cast = self.as_cast(&Js::prim(t.clone()), &target);
                            crate::emit_line!(self.w, "{t} = {};", cast.code);
                            target
                        }
                        None => vty,
                    }
                } else {
                    vty
                };
                let element = |s: &Self, code: String, ty: &Ty| {
                    if checked {
                        s.as_cast(&Js::prim(code), ty).code
                    } else {
                        code
                    }
                };
                if let Some(vt) = value_ty {
                    // Map: iterate over the entries.
                    let entries = self.member_access(&vty.non_null(), "entries", false);
                    let x = self.temp();
                    crate::open_block!(self.w, "for ({x} of {t}{entries}) {{");
                    let key = element(self, format!("{x}.key"), elem_ty);
                    let value = element(self, format!("{x}.value"), vt);
                    add(self, format!("{key}\u{0}{value}"));
                    self.w.close("}");
                } else {
                    let x = self.temp();
                    crate::open_block!(self.w, "for ({x} of {t}) {{");
                    let item = element(self, x, elem_ty);
                    add(self, item);
                    self.w.close("}");
                }
                if *null_aware {
                    self.w.close("}");
                }
            }
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                if let Some(pat) = case_pattern {
                    let (vjs, vty) = self.emit_expr(*condition, None);
                    let t = self.temp();
                    crate::emit_line!(self.w, "{t} = {};", vjs.code);
                    self.push_scope();
                    let mut binds = Vec::new();
                    let cond = self.pattern_cond(*pat, &t, &vty, &mut binds, false);
                    for (_, _, jsn) in &binds {
                        crate::emit_line!(self.w, "let {jsn} = null;");
                    }
                    let mut full = cond;
                    if let Some(g) = guard {
                        let (gjs, _) = self.emit_expr(*g, Some(&self.ctx.t_bool()));
                        full = format!("({full}) && {}", gjs.at(P_AND));
                    }
                    crate::open_block!(self.w, "if ({full}) {{");
                    self.emit_collection_element(then, elem_ty, value_ty, add);
                    self.w.close("}");
                    if let Some(e) = else_ {
                        self.w.open("else {");
                        self.emit_collection_element(e, elem_ty, value_ty, add);
                        self.w.close("}");
                    }
                    self.pop_scope();
                    return;
                }
                let (c, _) = self.emit_cond(*condition);
                crate::open_block!(self.w, "if ({c}) {{");
                self.emit_collection_element(then, elem_ty, value_ty, add);
                self.w.close("}");
                if let Some(e) = else_ {
                    self.w.open("else {");
                    self.emit_collection_element(e, elem_ty, value_ty, add);
                    self.w.close("}");
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.push_scope();
                let init_js = match init {
                    Some(ast::ForInit::Variables(list)) => {
                        let mut parts = Vec::new();
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let (ijs, ity) = match v.initializer {
                                Some(e) => {
                                    let (j, t) = self.emit_expr(e, declared.as_ref());
                                    (Some(j), t)
                                }
                                None => (None, Ty::Null),
                            };
                            let ty = declared.clone().unwrap_or(ity);
                            let jsn = self.declare(v.name.sym, ty);
                            match ijs {
                                Some(j) => parts.push(format!("{jsn} = {}", j.at(P_ASSIGN + 1))),
                                None => parts.push(jsn),
                            }
                        }
                        format!("let {}", parts.join(", "))
                    }
                    Some(ast::ForInit::Expression(e)) => self.emit_expr(*e, None).0.code,
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        self.emit_pattern_variables(*pattern, *value);
                        String::new()
                    }
                    None => String::new(),
                };
                let cond_js = match condition {
                    Some(c) => self.emit_cond(*c).0,
                    None => String::new(),
                };
                let upd: Vec<String> = updates
                    .iter()
                    .map(|u| self.emit_expr(*u, None).0.code)
                    .collect();
                self.w.open(&format!(
                    "for ({init_js}; {cond_js}; {}) {{",
                    upd.join(", ")
                ));
                self.emit_collection_element(body, elem_ty, value_ty, add);
                self.w.close("}");
                self.pop_scope();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                await_,
            } => {
                let (ijs, ity) = self.emit_expr(*iterable, None);
                let (ijs, ity) = self.checked_iterable(ijs, ity, *await_);
                let et = if *await_ {
                    self.ctx.stream_
                } else {
                    self.ctx.iterable_
                }
                .and_then(|i| self.ctx.as_super(&ity, i))
                .and_then(|t| t.args().first().cloned())
                .unwrap_or(Ty::Dynamic);
                self.push_scope();
                if *await_ {
                    // `await for` in an `async` body, like the statement: a
                    // `StreamIterator` advanced with `yield` (`await`).
                    self.m.use_sdk("async");
                    let it = self.temp();
                    let elem_rti = self.rti(&et);
                    let recipe =
                        self.ctx
                            .recipe_of("dart:async", "StreamIterator", "async|StreamIterator");
                    crate::emit_line!(
                        self.w,
                        "{it} = async.StreamIterator.new({elem_rti}[_eval](\"{recipe}<0>\"), {});",
                        ijs.code
                    );
                    self.w.open("try {");
                    crate::open_block!(self.w, "while ((yield {it}.moveNext())) {{");
                    let current = format!("{it}.current");
                    match target {
                        ast::ForInTarget::Declared { name, ty, .. } => {
                            let t = ty.map(|t| self.resolve_type(t)).unwrap_or(et.clone());
                            let value = self.checked_loop_value(&current, &et, &t);
                            let jsn = self.declare(name.sym, t);
                            crate::emit_line!(self.w, "let {jsn} = {value};");
                        }
                        ast::ForInTarget::Pattern { pattern, .. } => {
                            let t = self.temp();
                            crate::emit_line!(self.w, "{t} = {current};");
                            self.emit_pattern_bind_stmt(*pattern, &t, &et);
                        }
                        ast::ForInTarget::Expression(e) => {
                            let (ljs, _) = self.emit_assign_to(*e, &Js::prim(current), &et);
                            crate::emit_line!(self.w, "{};", ljs.code);
                        }
                    }
                    self.emit_collection_element(body, elem_ty, value_ty, add);
                    self.w.close("}");
                    self.w.close("}");
                    self.w.open("finally {");
                    crate::emit_line!(self.w, "yield {it}.cancel();");
                    self.w.close("}");
                    self.pop_scope();
                    return;
                }
                match target {
                    ast::ForInTarget::Declared { name, ty, .. } => {
                        let t = ty.map(|t| self.resolve_type(t)).unwrap_or(et.clone());
                        let jsn = self.declare(name.sym, t.clone());
                        crate::open_block!(self.w, "for (let {jsn} of {}) {{", ijs.code);
                        let value = self.checked_loop_value(&jsn, &et, &t);
                        if value != jsn {
                            crate::emit_line!(self.w, "{jsn} = {value};");
                        }
                    }
                    ast::ForInTarget::Pattern { pattern, .. } => {
                        let t = self.temp();
                        crate::open_block!(self.w, "for ({t} of {}) {{", ijs.code);
                        self.emit_pattern_bind_stmt(*pattern, &t, &et);
                    }
                    ast::ForInTarget::Expression(e) => {
                        let t = self.temp();
                        crate::open_block!(self.w, "for ({t} of {}) {{", ijs.code);
                        let (ljs, _) = self.emit_assign_to(*e, &Js::prim(t), &et);
                        crate::emit_line!(self.w, "{};", ljs.code);
                    }
                }
                self.emit_collection_element(body, elem_ty, value_ty, add);
                self.w.close("}");
                self.pop_scope();
            }
        }
    }

    /// Set or map literal; `common` as in [`Self::emit_list_literal`] (it
    /// also decides between set and map, for `{}` with a context).
    fn emit_set_or_map(
        &mut self,
        const_: bool,
        type_args: &[ast::TypeId],
        elements: &[CollectionElement],
        expected: Option<&Ty>,
        common: Option<&Ty>,
    ) -> (Js, Ty) {
        let const_ = const_ || self.in_const;
        let saved_const = self.in_const;
        if const_ {
            self.in_const = true;
        }
        let common = common
            .filter(|t| t.is_class(self.ctx.map_) || t.is_class(self.ctx.set_))
            .cloned();
        let r = self.emit_set_or_map_inner(const_, type_args, elements, expected, common);
        self.in_const = saved_const;
        r
    }

    fn emit_set_or_map_inner(
        &mut self,
        const_: bool,
        type_args: &[ast::TypeId],
        elements: &[CollectionElement],
        expected: Option<&Ty>,
        common: Option<Ty>,
    ) -> (Js, Ty) {
        let is_map = if type_args.len() == 2 {
            true
        } else if type_args.len() == 1 {
            false
        } else if let Some(c) = &common {
            c.is_class(self.ctx.map_)
        } else if elements.is_empty() {
            !expected.is_some_and(|t| {
                t.non_null().class() == self.ctx.set_
                    || self.ctx.set_.is_some_and(|s| {
                        self.ctx.as_super(&t.non_null(), s).is_some()
                            && t.non_null().class() != self.ctx.map_
                    })
            })
        } else {
            let mut any_map = false;
            let mut any_set = false;
            for e in elements {
                self.classify_element(e, &mut any_map, &mut any_set);
            }
            any_map && !any_set
        };
        if is_map {
            let (kt, vt) = if type_args.len() == 2 {
                (
                    self.resolve_type(type_args[0]),
                    self.resolve_type(type_args[1]),
                )
            } else if let Some([k, v]) = common.as_ref().map(|c| c.args()) {
                (k.clone(), v.clone())
            } else if let (Some(k), Some(v)) = (
                self.expected_arg(expected, self.ctx.map_, 0),
                self.expected_arg(expected, self.ctx.map_, 1),
            ) {
                (k, v)
            } else {
                self.infer_map_tys(elements)
            };
            let map_ty = self.ctx.t_map(kt.clone(), vt.clone());
            let simple = elements.iter().all(|e| {
                matches!(
                    e,
                    CollectionElement::MapEntry {
                        null_aware_key: false,
                        null_aware_value: false,
                        ..
                    }
                )
            });
            if simple {
                let mut items = Vec::new();
                for e in elements {
                    if let CollectionElement::MapEntry { key, value, .. } = e {
                        items.push(self.emit_expr(*key, Some(&kt)).0.into_at(P_ASSIGN));
                        items.push(self.emit_expr(*value, Some(&vt)).0.into_at(P_ASSIGN));
                    }
                }
                if const_ {
                    let kr = self.rti(&kt);
                    let vr = self.rti(&vt);
                    return (
                        Js::prim(format!("dart.constMap({kr}, {vr}, [{}])", items.join(", "))),
                        map_ty,
                    );
                }
                let (cls, rti) = self.map_impl(&kt, &vt);
                if items.is_empty() {
                    return (Js::prim(format!("new {cls}.new({rti})")), map_ty);
                }
                return (
                    Js::prim(format!("new {cls}.from({rti}, [{}])", items.join(", "))),
                    map_ty,
                );
            }
            let (cls, rti) = self.map_impl(&kt, &vt);
            let t = self.temp();
            let saved_w = std::mem::take(&mut self.w);
            if const_ {
                crate::emit_line!(self.w, "{t} = [];");
            } else {
                crate::emit_line!(self.w, "{t} = new {cls}.new({rti});");
            }
            let set = self.member_access(&map_ty, "[]=", false);
            let t2 = t.clone();
            for el in elements {
                self.emit_collection_element(el, &kt, Some(&vt), &|s: &mut Self, v: String| {
                    let (k, val) = v.split_once('\u{0}').unwrap_or((&v, "null"));
                    if const_ {
                        crate::emit_line!(s.w, "{t2}.push({k}, {val});");
                    } else {
                        crate::emit_line!(s.w, "{t2}{set}({k}, {val});");
                    }
                });
            }
            if const_ {
                let kr = self.rti(&kt);
                let vr = self.rti(&vt);
                crate::emit_line!(self.w, "return dart.constMap({kr}, {vr}, {t});");
            } else {
                crate::emit_line!(self.w, "return {t};");
            }
            let body = std::mem::replace(&mut self.w, saved_w).out;
            return (self.iife(&body), map_ty);
        }
        // Set
        let et = if let Some(t) = type_args.first() {
            self.resolve_type(*t)
        } else if let Some([t]) = common.as_ref().map(|c| c.args()) {
            t.clone()
        } else if let Some(t) = self
            .expected_arg(expected, self.ctx.set_, 0)
            .or_else(|| self.expected_arg(expected, self.ctx.iterable_, 0))
        {
            t
        } else {
            self.infer_elements_ty(elements)
        };
        let set_ty = self.ctx.t_set(et.clone());
        self.m.use_sdk("collection");
        let lhs = self
            .ctx
            .linked_hash_set
            .map(|c| Ty::iface_args(c, vec![et.clone()]))
            .unwrap_or(set_ty.clone());
        let rti = self.rti(&lhs);
        let simple = elements
            .iter()
            .all(|e| matches!(e, CollectionElement::Expression(_)));
        if simple {
            let items: Vec<String> = elements
                .iter()
                .map(|e| match e {
                    CollectionElement::Expression(x) => {
                        self.emit_expr(*x, Some(&et)).0.into_at(P_ASSIGN)
                    }
                    _ => unreachable!(),
                })
                .collect();
            if const_ {
                let er = self.rti(&et);
                return (
                    Js::prim(format!("dart.constSet({er}, [{}])", items.join(", "))),
                    set_ty,
                );
            }
            if items.is_empty() {
                return (
                    Js::prim(format!("collection.LinkedHashSet.new({rti})")),
                    set_ty,
                );
            }
            return (
                Js::prim(format!(
                    "collection.LinkedHashSet.from({rti}, [{}])",
                    items.join(", ")
                )),
                set_ty,
            );
        }
        let t = self.temp();
        let saved_w = std::mem::take(&mut self.w);
        if const_ {
            crate::emit_line!(self.w, "{t} = [];");
        } else {
            crate::emit_line!(self.w, "{t} = collection.LinkedHashSet.new({rti});");
        }
        let t2 = t.clone();
        for el in elements {
            self.emit_collection_element(el, &et, None, &|s: &mut Self, v: String| {
                if const_ {
                    crate::emit_line!(s.w, "{t2}.push({v});");
                } else {
                    crate::emit_line!(s.w, "{t2}.add({v});");
                }
            });
        }
        if const_ {
            let er = self.rti(&et);
            crate::emit_line!(self.w, "return dart.constSet({er}, {t});");
        } else {
            crate::emit_line!(self.w, "return {t};");
        }
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), set_ty)
    }

    /// Marks whether the element indicates a map (an entry) or a set (an expression).
    fn classify_element(&mut self, e: &CollectionElement, any_map: &mut bool, any_set: &mut bool) {
        match e {
            CollectionElement::MapEntry { .. } => *any_map = true,
            CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => {
                *any_set = true
            }
            CollectionElement::Spread { value, .. } => {
                let t = self.type_of(*value);
                if self
                    .ctx
                    .map_
                    .is_some_and(|m| self.ctx.as_super(&t.non_null(), m).is_some())
                {
                    *any_map = true;
                } else if !t.is_dynamic() {
                    *any_set = true;
                }
            }
            CollectionElement::If { then, else_, .. } => {
                self.classify_element(then, any_map, any_set);
                if let Some(e) = else_ {
                    self.classify_element(e, any_map, any_set);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                self.classify_element(body, any_map, any_set);
            }
        }
    }

    fn infer_map_tys(&mut self, elements: &[CollectionElement]) -> (Ty, Ty) {
        let mut ks = Vec::new();
        let mut vs = Vec::new();
        for el in elements {
            match el {
                CollectionElement::MapEntry {
                    key,
                    value,
                    null_aware_key,
                    null_aware_value,
                } => {
                    // A null-aware part enters without the `null` (spec 3.8, NonNull).
                    let k = self.type_of(*key);
                    let v = self.type_of(*value);
                    ks.push(if *null_aware_key { k.non_null() } else { k });
                    vs.push(if *null_aware_value { v.non_null() } else { v });
                }
                CollectionElement::Spread { value, .. } => {
                    let t = self.type_of(*value);
                    if let Some(mt) = self
                        .ctx
                        .map_
                        .and_then(|m| self.ctx.as_super(&t.non_null(), m))
                    {
                        ks.push(mt.args().first().cloned().unwrap_or(Ty::Dynamic));
                        vs.push(mt.args().get(1).cloned().unwrap_or(Ty::Dynamic));
                    }
                }
                CollectionElement::If { then, else_, .. } => {
                    let (k, v) = self.infer_map_tys(std::slice::from_ref(then));
                    if !k.is_dynamic() {
                        ks.push(k);
                        vs.push(v);
                    }
                    if let Some(e) = else_ {
                        let (k, v) = self.infer_map_tys(std::slice::from_ref(e));
                        if !k.is_dynamic() {
                            ks.push(k);
                            vs.push(v);
                        }
                    }
                }
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    self.push_scope();
                    if let CollectionElement::ForIn {
                        target: ast::ForInTarget::Declared { name, ty, .. },
                        iterable,
                        ..
                    } = el
                    {
                        let it = self.type_of(*iterable);
                        let et = ty.map(|t| self.resolve_type(t)).unwrap_or_else(|| {
                            self.ctx
                                .iterable_
                                .and_then(|i| self.ctx.as_super(&it, i))
                                .and_then(|t| t.args().first().cloned())
                                .unwrap_or(Ty::Dynamic)
                        });
                        self.declare(name.sym, et);
                    }
                    if let CollectionElement::For {
                        init: Some(ast::ForInit::Variables(list)),
                        ..
                    } = el
                    {
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let t = match (declared.clone(), v.initializer) {
                                (Some(d), _) => d,
                                (None, Some(i)) => self.type_of(i),
                                _ => Ty::Dynamic,
                            };
                            self.declare(v.name.sym, t);
                        }
                    }
                    let (k, v) = self.infer_map_tys(std::slice::from_ref(body));
                    self.pop_scope();
                    if !k.is_dynamic() {
                        ks.push(k);
                        vs.push(v);
                    }
                }
                _ => {}
            }
        }
        let fold = |s: &Self, v: Vec<Ty>| -> Ty {
            let mut it = v.into_iter();
            match it.next() {
                None => Ty::Dynamic,
                Some(f) => it.fold(f, |a, b| s.ctx.lub(&a, &b)),
            }
        };
        (fold(self, ks), fold(self, vs))
    }

    /// Map implementation class (`IdentityMap` for primitive keys) and its rti.
    pub(crate) fn map_impl(&mut self, kt: &Ty, vt: &Ty) -> (String, String) {
        self.m.use_sdk("_js_helper");
        let identity = self.ctx.is_js_primitive(kt);
        let name = if identity { "IdentityMap" } else { "LinkedMap" };
        let kr = self.recipe_plain(kt);
        let vr = self.recipe_plain(vt);
        let _ = (kr, vr);
        let cls = format!("_js_helper.{name}");
        // Rti: `_js_helper|IdentityMap<K,V>`.
        let lib = self
            .ctx
            .program
            .libraries
            .iter()
            .position(|l| l.uri == "dart:_js_helper");
        let rti = match lib.and_then(|i| {
            let sym = self.ctx.sym(name)?;
            match self.ctx.program.libraries[i].declared.get(&sym)?.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        }) {
            Some(c) => self.rti(&Ty::iface_args(c, vec![kt.clone(), vt.clone()])),
            None => self.rti(&self.ctx.t_map(kt.clone(), vt.clone())),
        };
        (cls, rti)
    }

    pub fn recipe_plain(&self, t: &Ty) -> String {
        let _ = t;
        String::new()
    }

    // A textual key cannot represent a lexical value: separate functions can
    // have the same emitted binding name with different constant values.
    // Records do not require persistent identity, so leave other constants
    // uncached while retaining their constant context for nested collections.
    fn literal_record_cache_value(&self, value: ExprId) -> bool {
        match &self.expr(value).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_) => true,
            ExprKind::String(literal) => literal
                .parts
                .iter()
                .all(|part| matches!(part, ast::StringPart::Text(_))),
            ExprKind::Parenthesized(inner) => self.literal_record_cache_value(*inner),
            ExprKind::Record {
                positional, named, ..
            } => {
                positional
                    .iter()
                    .all(|value| self.literal_record_cache_value(*value))
                    && named
                        .iter()
                        .all(|(_, value)| self.literal_record_cache_value(*value))
            }
            _ => false,
        }
    }

    fn emit_record(
        &mut self,
        record: ExprId,
        positional: &[ExprId],
        named: &[(ast::Name, ExprId)],
        expected: Option<&Ty>,
    ) -> (Js, Ty) {
        // Field contexts: those of the record context or, when it does not
        // have the literal's shape (a pattern's schema), the field types the
        // common inference gave the literal, so that implicit `call`
        // tear-offs and downcasts happen per field like in the CFE.
        let same_shape = |t: &Ty| {
            matches!(t, Ty::Record { pos, named: n, .. }
                if pos.len() == positional.len() && n.len() == named.len())
        };
        let context = expected
            .filter(|t| same_shape(t))
            .cloned()
            .or_else(|| self.common_type(record).filter(|t| same_shape(t)));
        let (ctx_pos, ctx_named) = match context {
            Some(Ty::Record { pos, named, .. }) => (pos, named),
            _ => (Vec::new(), Vec::new()),
        };
        let mut pos_js = Vec::new();
        let mut pos_ty = Vec::new();
        for (i, e) in positional.iter().enumerate() {
            let (j, t) = self.emit_expr(*e, ctx_pos.get(i));
            pos_js.push(j.into_at(P_ASSIGN));
            pos_ty.push(t);
        }
        let mut named_items: Vec<(String, String, Ty)> = Vec::new();
        for (n, e) in named {
            let name = self.name(n.sym).to_string();
            let field = ctx_named.iter().find(|(m, _)| *m == name).map(|(_, t)| t);
            let (j, t) = self.emit_expr(*e, field);
            named_items.push((name, j.into_at(P_ASSIGN), t));
        }
        // Evaluation order preserved through temps when named fields are out of order.
        let mut sorted = named_items.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let in_order = named_items
            .iter()
            .map(|x| &x.0)
            .eq(sorted.iter().map(|x| &x.0));
        // Named fields before some positional one in the source? Then the evaluation order requires temps.
        let mut interleaved = false;
        let mut seen_named = false;
        for (n, e) in named
            .iter()
            .map(|(n, e)| (Some(n), *e))
            .chain(std::iter::empty())
        {
            let _ = (n, e);
        }
        {
            let mut named_spans: Vec<usize> = named
                .iter()
                .map(|(_, e)| self.expr(*e).span.start)
                .collect();
            let pos_spans: Vec<usize> = positional
                .iter()
                .map(|e| self.expr(*e).span.start)
                .collect();
            named_spans.sort();
            if let (Some(&first_named), Some(&last_pos)) = (named_spans.first(), pos_spans.last()) {
                if first_named < last_pos {
                    interleaved = true;
                }
            }
            let _ = &mut seen_named;
        }
        let mut prefix = String::new();
        let sorted_vals: Vec<String> = if (in_order && !interleaved) || (named_items.is_empty()) {
            sorted.iter().map(|x| x.1.clone()).collect()
        } else {
            // Evaluate everything in source order into temps.
            let mut temps: HashMap<String, String> = HashMap::new();
            let mut order: Vec<(usize, Option<String>, String)> = Vec::new();
            for (i, e) in positional.iter().enumerate() {
                order.push((self.expr(*e).span.start, None, pos_js[i].clone()));
            }
            for ((n, e), (_, v, _)) in named.iter().zip(named_items.iter()) {
                order.push((
                    self.expr(*e).span.start,
                    Some(self.name(n.sym).to_string()),
                    v.clone(),
                ));
            }
            order.sort_by_key(|x| x.0);
            let mut new_pos: Vec<String> = Vec::new();
            for (_, name, v) in order {
                let t = self.temp();
                prefix.push_str(&format!("{t} = {v}, "));
                match name {
                    Some(n) => {
                        temps.insert(n, t);
                    }
                    None => new_pos.push(t),
                }
            }
            pos_js = new_pos;
            sorted.iter().map(|x| temps[&x.0].clone()).collect()
        };
        let names: Vec<String> = sorted.iter().map(|x| x.0.clone()).collect();
        let shape = format!("{};{}", pos_js.len() + names.len(), names.join(","));
        let named_js = if names.is_empty() {
            "void 0".to_string()
        } else {
            format!(
                "[{}]",
                names
                    .iter()
                    .map(|n| js::string_literal(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let mut values = pos_js.clone();
        values.extend(sorted_vals);
        let mut code = format!(
            "dart.recordLiteral({}, {}, {named_js}, [{}])",
            js::string_literal(&shape),
            pos_js.len(),
            values.join(", ")
        );
        if self.in_const
            && prefix.is_empty()
            && positional
                .iter()
                .all(|value| self.literal_record_cache_value(*value))
            && named
                .iter()
                .all(|(_, value)| self.literal_record_cache_value(*value))
        {
            self.m.note_const_record(self.lib);
            let lv = self.lib_var(self.lib);
            code = format!("{lv}.$C({}, () => {code})", js::string_literal(&code));
        }
        let ty = Ty::Record {
            pos: pos_ty,
            named: sorted.into_iter().map(|(n, _, t)| (n, t)).collect(),
            nullable: false,
        };
        if prefix.is_empty() {
            (Js::prim(code), ty)
        } else {
            (Js::new(format!("{prefix}{code}"), P_COMMA).paren(), ty)
        }
    }

    // -----------------------------------------------------------------------
    // Identifiers and member access
    // -----------------------------------------------------------------------

    /// Value of an identifier (without an expression node).
    pub fn emit_identifier_value(&mut self, sym: dartforge_intern::SymbolId) -> (Js, Ty) {
        self.emit_identifier(sym, ExprId(u32::MAX))
    }

    /// `this` (the extension's receiver, `$this`, inside one).
    fn emit_this(&mut self) -> (Js, Ty) {
        if let Some(t) = &self.extension_this {
            return (Js::prim("$this"), t.clone());
        }
        let ty = self
            .class
            .map(|c| self.ctx.this_ty(c))
            .unwrap_or(Ty::Dynamic);
        (Js::prim("this"), ty)
    }

    fn emit_identifier(&mut self, sym: dartforge_intern::SymbolId, e: ExprId) -> (Js, Ty) {
        let n = self.name(sym).to_string();
        // `$this` in a simple interpolation arrives as an identifier.
        if n == "this" {
            return self.emit_this();
        }
        if self.ctx.type_check.is_some() {
            self.check_target(e, &self.resolve_ident(sym));
        }
        // Instance member of an extension type through the implicit `this`.
        if let Some((owner, member)) = self.ext_type_member(e) {
            return self.ext_type_read(&Js::prim("$this"), None, owner, member);
        }
        match self.ident_target(sym, e) {
            IdentTarget::Local(js, ty) => {
                // The type of the read is the common inference's: the declared
                // one with the promotions and demotions of the specification's
                // flow analysis (`int? v = 3; v` is `int`; patterns, `catch`
                // and `as` promote; an assignment in a loop undoes it). The
                // emitter's scope only keeps the declared type. Only for a node
                // the inference visited (the resolution is a local) and that is
                // not the target of a compound write (whose read type stays on
                // the assignment node), so as not to swap a type for `dynamic`.
                let ty = match self.common_resolution(e) {
                    Some(Resolved::Local(_) | Resolved::Parameter { .. })
                        if !self.ctx.is_write_target(self.unit, e) =>
                    {
                        match self.common_type(e) {
                            // A generic function in a non-generic context:
                            // the common type is the instantiation that
                            // `coerce_to` still has to make.
                            Some(Ty::Fn { type_params, .. })
                                if type_params.is_empty()
                                    && matches!(&ty, Ty::Fn { type_params, .. } if !type_params.is_empty()) =>
                            {
                                ty
                            }
                            common => common.unwrap_or(ty),
                        }
                    }
                    _ => ty,
                };
                if let Some(l) = self.lookup_local(sym).cloned() {
                    if let Some(init) = &l.lazy_init
                        && l.late_final
                    {
                        // A `late final` local assigned while its initializer
                        // runs is an error (`LateError.localADI`).
                        self.m.use_sdk("_internal");
                        let t = self.temp();
                        let name = js::string_literal(self.ctx.late_name(&n));
                        return (
                            Js::new(
                                format!(
                                    "{js} === void 0 ? ({t} = {init}, {js} === void 0 ? {js} = {t} : dart.throw(new _internal.LateError.localADI({name}))) : {js}"
                                ),
                                P_COND,
                            )
                            .paren(),
                            ty,
                        );
                    }
                    if let Some(init) = &l.lazy_init {
                        return (
                            Js::new(format!("{js} === void 0 ? {js} = {init} : {js}",), P_COND)
                                .paren(),
                            ty,
                        );
                    }
                    if l.late_check {
                        self.m.use_sdk("_internal");
                        return (Js::new(format!("{js} === void 0 ? dart.throw(new _internal.LateError.localNI({})) : {js}", js::string_literal(self.ctx.late_name(&n))), P_COND).paren(), ty);
                    }
                }
                (Js::prim(js), ty)
            }
            IdentTarget::ThisMember(m) => self.emit_member_get(
                &Js::prim("this"),
                &self
                    .class
                    .map(|c| self.ctx.this_ty(c))
                    .unwrap_or(Ty::Dynamic),
                &n,
                Some(m),
            ),
            IdentTarget::Static(c, mk) => self.emit_static_get(c, &n, mk),
            IdentTarget::Element(el) => self.emit_element_get(el, &n),
            IdentTarget::TypeParam(t) if self.in_foreign_js => {
                (Js::prim(self.rti(&t)), self.ctx.t_type())
            }
            IdentTarget::TypeParam(t) => (
                Js::prim(format!("dart_rti.createRuntimeType({})", self.rti(&t))),
                self.ctx.t_type(),
            ),
            IdentTarget::ExtThisMember(_) => {
                let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                self.emit_member_get(&Js::prim("$this"), &t, &n, None)
            }
            IdentTarget::ThisExt => {
                let t = self
                    .class
                    .map(|c| self.ctx.this_ty(c))
                    .unwrap_or(Ty::Dynamic);
                self.emit_member_get(&Js::prim("this"), &t, &n, None)
            }
            IdentTarget::ExtField(ext) => {
                let e = self.ctx.program.extension(ext);
                let ext_name = self.extension_js_name(ext);
                let lib_var = self.lib_var(e.library);
                let ty = e
                    .fields
                    .iter()
                    .find(|v| self.ctx.program.variable(**v).name == sym)
                    .map(|v| self.ctx.var_ty(*v))
                    .unwrap_or(Ty::Dynamic);
                (
                    Js::prim(format!(
                        "{lib_var}[{}]",
                        js::string_literal(&format!("{ext_name}|{n}"))
                    )),
                    ty,
                )
            }
            IdentTarget::ExtStatic(ext, fid) => {
                let e = self.ctx.program.extension(ext);
                let ext_name = self.extension_js_name(ext);
                let lib_var = self.lib_var(e.library);
                let f = self.ctx.program.function(fid);
                if f.kind == FunctionKind::Getter {
                    let js = format!(
                        "{lib_var}[{}]",
                        js::string_literal(&format!("{ext_name}|get#{n}"))
                    );
                    return (
                        Js::prim(format!("{js}()")),
                        self.ctx
                            .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
                    );
                }
                let js = format!(
                    "{lib_var}[{}]",
                    js::string_literal(&format!("{ext_name}|{n}"))
                );
                let ty = self.ctx.fn_ty(fid);
                (self.tearoff_static(&js, &ty), ty)
            }
            IdentTarget::ExtMember(ext, _) => {
                let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                self.forced_ext = Some(ext);
                match self.try_extension_get(&Js::prim("$this"), &t, &n) {
                    Some(r) => r,
                    None => (Js::prim("null"), Ty::Dynamic),
                }
            }
            IdentTarget::Prefix(_) => (Js::prim("null"), Ty::Dynamic),
            IdentTarget::Unknown => {
                // SDK mode: in an extension on an extension type (the
                // `JSArrayToList on JSArray<T>` *patch*), the representation
                // field read without `this` is the value itself: the extension
                // type is erased to its representation.
                if self.ctx.sdk.is_some() {
                    if let Some(Ty::Iface { class, .. }) = self.extension_this.clone() {
                        if let Some(rep) = self.ctx.program.class(class).representation {
                            if self.ctx.program.variable(rep).name == sym {
                                return (Js::prim("$this"), self.ctx.var_ty(rep));
                            }
                        }
                    }
                }
                // Special type literals.
                let special = match n.as_str() {
                    "dynamic" => Some(Ty::Dynamic),
                    "Never" => Some(Ty::Never),
                    _ => None,
                };
                if let Some(t) = special {
                    return (
                        Js::prim(format!("dart_rti.createRuntimeType({})", self.rti(&t))),
                        self.ctx.t_type(),
                    );
                }
                // Nothing in the lexical scope, the imports or the outline has
                // the name: the program is invalid (the VM's `Undefined name`).
                let library = self.ctx.program.unit(self.unit).library;
                if e != ExprId(u32::MAX) && !self.ctx.libs[library.0 as usize].is_sdk {
                    self.language_error(self.expr(e).span, format!("Undefined name '{n}'."));
                }
                (Js::prim(js::ident(&n)), Ty::Dynamic)
            }
        }
    }

    pub fn emit_element_get(&mut self, el: Element, n: &str) -> (Js, Ty) {
        match el {
            Element::Function(fid) => {
                // SDK mode: `staticInteropGlobalContext` is `dart.global`.
                if crate::own_sdk::intrinsic_of(self.ctx, fid)
                    == Some(crate::own_sdk::Intrinsic::GlobalContext)
                {
                    return (Js::prim("dart.global"), Ty::Dynamic);
                }
                let (js, ty) = self.element_ref(el).expect("function");
                // Tearoff of a top-level function.
                let f = self.ctx.program.function(fid);
                if self.ctx.is_js_member(fid) {
                    return match f.kind {
                        FunctionKind::Getter => (
                            self.js_null_check(Js::prim(js), &MemberKind::Getter(fid)),
                            self.ctx
                                .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
                        ),
                        FunctionKind::Setter => (Js::prim(js), Ty::Dynamic),
                        _ => (
                            Js::prim(format!(
                                "dart.tearoffInterop({js}, {})",
                                self.ctx.js_null_checkable(&MemberKind::Method(fid))
                            )),
                            ty,
                        ),
                    };
                }
                match f.kind {
                    FunctionKind::Getter => (
                        Js::prim(js),
                        self.ctx
                            .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
                    ),
                    FunctionKind::Setter => (Js::prim(js), Ty::Dynamic),
                    // SDK mode: inside `JS()` and for runtime functions, the
                    // *tearoff* is the raw function (`compiler.dart:7331-7338, 7366`).
                    _ if self.in_foreign_js
                        || crate::own_sdk::function_without_reify(self.ctx, fid) =>
                    {
                        (Js::prim(js), ty)
                    }
                    _ => (self.tearoff_static(&js, &ty), ty),
                }
            }
            Element::Variable(vid) => {
                // SDK mode: a primitive constant folded at the use site.
                if let Some(js) = crate::own_sdk::inline_constant(self, vid) {
                    return (js, self.ctx.var_ty(vid));
                }
                let (js, ty) = self.element_ref(el).expect("variable");
                if self.ctx.is_js_var(vid) {
                    return (
                        self.js_null_check(Js::prim(js), &MemberKind::Field(vid)),
                        ty,
                    );
                }
                (Js::prim(js), ty)
            }
            Element::Class(c) => {
                // A generic class without arguments is instantiated to its
                // bounds (`C` is `C<num>` for `class C<T extends num>`).
                let params: Vec<(u32, Ty)> = self.ctx.class_params[c.0 as usize]
                    .iter()
                    .map(|p| (p.id, (*p.bound).clone()))
                    .collect();
                let ty = Ty::Iface {
                    class: c,
                    args: self.ctx.bounds_instantiation(&params),
                    nullable: false,
                };
                let rti = self.rti(&ty);
                // SDK mode: a type literal inside `JS()` is the raw rti.
                if self.in_foreign_js {
                    return (Js::prim(rti), self.ctx.t_type());
                }
                (
                    Js::prim(format!("dart_rti.createRuntimeType({rti})")),
                    self.ctx.t_type(),
                )
            }
            // A type alias used as a value: the `Type` of the aliased type,
            // its parameters instantiated to bounds.
            Element::Typedef(td) => {
                let data = &self.ctx.outline.typedefs[td.0 as usize];
                let params: Vec<(u32, Ty)> = data
                    .type_params
                    .iter()
                    .map(|p| (p.0, self.ctx.ty_of(self.ctx.table.param(*p).bound)))
                    .collect();
                let args = self.ctx.bounds_instantiation(&params);
                let map: HashMap<u32, Ty> = params.iter().map(|(id, _)| *id).zip(args).collect();
                let ty = self.ctx.ty_of(data.target_type).subst_prop(&map);
                let rti = self.rti(&ty);
                if self.in_foreign_js {
                    return (Js::prim(rti), self.ctx.t_type());
                }
                (
                    Js::prim(format!("dart_rti.createRuntimeType({rti})")),
                    self.ctx.t_type(),
                )
            }
            _ => (Js::prim(js::ident(n)), Ty::Dynamic),
        }
    }

    /// Tearoff of a static/top-level function: `dart.fn(f, rti)` or `dart.gFn`.
    pub fn tearoff_static(&mut self, js: &str, ty: &Ty) -> Js {
        match ty {
            Ty::Fn { type_params, .. } if !type_params.is_empty() => {
                let rti = self.rti(ty);
                let defaults: Vec<String> = type_params
                    .iter()
                    .map(|p| self.rti(&self.default_type_arg(&p.bound)))
                    .collect();
                Js::prim(format!(
                    "dart.gFn({js}, {rti}, dart.constList(dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true), [{}]))",
                    defaults.join(", ")
                ))
            }
            Ty::Fn { .. } => {
                let rti = self.rti(ty);
                Js::prim(format!("dart.fn({js}, {rti})"))
            }
            _ => Js::prim(js.to_string()),
        }
    }

    pub fn default_type_arg(&self, bound: &Ty) -> Ty {
        if bound.mentions_params() {
            Ty::Dynamic
        } else if let Ty::Iface {
            class,
            nullable: true,
            ..
        } = bound
        {
            if Some(*class) == self.ctx.object {
                return Ty::Dynamic;
            }
            bound.clone()
        } else {
            bound.clone()
        }
    }

    pub fn emit_static_get(&mut self, c: ClassId, n: &str, mk: MemberKind) -> (Js, Ty) {
        if self.ctx.is_js_class(c) && self.ctx.is_js_member_kind(&mk) {
            let js = self
                .ctx
                .js_static_ref(Some(c), self.ctx.lib_of_class(c), &mk, n);
            return match mk {
                MemberKind::Method(fid) => (
                    Js::prim(format!(
                        "dart.tearoffInterop({js}, {})",
                        self.ctx.js_null_checkable(&mk)
                    )),
                    self.ctx.fn_ty(fid),
                ),
                MemberKind::Getter(fid) => (
                    self.js_null_check(Js::prim(js), &mk),
                    self.ctx
                        .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
                ),
                MemberKind::Setter(_) => (Js::prim(js), Ty::Dynamic),
                MemberKind::Field(vid) => {
                    (self.js_null_check(Js::prim(js), &mk), self.ctx.var_ty(vid))
                }
            };
        }
        let cls = self.class_ref(c);
        let js = format!("{cls}{}", js::prop_access(&static_member_name(n)));
        match mk {
            // SDK mode: inside `JS()` and in the runtime, the *tearoff* is raw.
            MemberKind::Method(fid)
                if self.in_foreign_js || crate::own_sdk::function_without_reify(self.ctx, fid) =>
            {
                (Js::prim(js), self.ctx.fn_ty(fid))
            }
            MemberKind::Method(fid) => {
                let ty = self.ctx.fn_ty(fid);
                (self.tearoff_static(&js, &ty), ty)
            }
            MemberKind::Getter(fid) => (
                Js::prim(js),
                self.ctx
                    .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
            ),
            MemberKind::Setter(_) => (Js::prim(js), Ty::Dynamic),
            MemberKind::Field(vid) => match crate::own_sdk::inline_constant(self, vid) {
                Some(v) => (v, self.ctx.var_ty(vid)),
                None => (Js::prim(js), self.ctx.var_ty(vid)),
            },
        }
    }

    /// `recv.name` (getter, field, or method tearoff).
    pub fn emit_member_get(
        &mut self,
        recv: &Js,
        recv_ty: &Ty,
        name: &str,
        member: Option<Member>,
    ) -> (Js, Ty) {
        // SDK mode: `this._jsString` in an extension type is the value itself
        // (the type is erased to its representation).
        if self.ctx.sdk.is_some() {
            if let Ty::Iface { class, .. } = recv_ty.non_null() {
                if let Some(rep) = self.ctx.program.class(class).representation {
                    if self.ctx.name(self.ctx.program.variable(rep).name) == name {
                        return (recv.clone(), self.ctx.var_ty(rep));
                    }
                }
            }
        }
        if self.forced_ext.is_some() {
            if let Some(r) = self.try_extension_get(recv, recv_ty, name) {
                return r;
            }
            self.forced_ext = None;
        }
        let member = member.or_else(|| self.ctx.lookup_member(recv_ty, name, false));
        // Object members with helpers.
        let recv_nn = recv_ty.non_null();
        // A nullable receiver may be `null`: the helpers handle it.
        let helper = !self.is_user_class_recv(&recv_nn) || recv_ty.is_nullable();
        match name {
            "hashCode" if helper => {
                return (
                    Js::prim(format!("dart.hashCode({})", recv.code)),
                    self.ctx.t_int(),
                );
            }
            "runtimeType" if helper => {
                return (
                    Js::prim(format!("dart.runtimeType({})", recv.code)),
                    self.ctx.t_type(),
                );
            }
            "toString" | "noSuchMethod" if helper => {
                let ty = member
                    .as_ref()
                    .map(|m| self.ctx.member_ty(m))
                    .unwrap_or(Ty::Dynamic);
                return (Js::prim(format!("dart.{}Tearoff({})", name, recv.code)), ty);
            }
            _ => {}
        }
        if recv_ty.is_dynamic() && member.is_none() {
            return (
                Js::prim(format!(
                    "dart.dload({}, {})",
                    recv.code,
                    self.dynamic_member_key(name)
                )),
                Ty::Dynamic,
            );
        }
        let Some(m) = member else {
            // A record's own field wins over an extension's getter.
            let record_field = match &recv_nn {
                Ty::Record { pos, named, .. } => {
                    name.strip_prefix('$')
                        .and_then(|s| s.parse::<usize>().ok())
                        .is_some_and(|i| i >= 1 && i <= pos.len())
                        || named.iter().any(|(n, _)| n == name)
                }
                _ => false,
            };
            // Extension?
            if !record_field && let Some(r) = self.try_extension_get(recv, recv_ty, name) {
                return r;
            }
            if let Ty::Fn { .. } = recv_nn {
                if name == "call" {
                    return (recv.clone(), recv_ty.clone());
                }
            }
            if let Ty::Record { pos, named, .. } = &recv_nn {
                if let Some(idx) = name.strip_prefix('$').and_then(|s| s.parse::<usize>().ok()) {
                    if idx >= 1 && idx <= pos.len() {
                        return (
                            Js::prim(format!("{}.${idx}", recv.at(P_PRIMARY))),
                            pos[idx - 1].clone(),
                        );
                    }
                }
                if let Some((_, t)) = named.iter().find(|(n, _)| n == name) {
                    return (
                        Js::prim(format!("{}{}", recv.at(P_PRIMARY), js::prop_access(name))),
                        t.clone(),
                    );
                }
            }
            return (
                Js::prim(format!(
                    "dart.dload({}, {})",
                    recv.code,
                    self.dynamic_member_key(name)
                )),
                Ty::Dynamic,
            );
        };
        let ty = self.ctx.member_ty(&m);
        let access = self.member_access(recv_ty, name, false);
        let js = format!("{}{access}", recv.at(P_PRIMARY));
        if self.ctx.is_js_member_kind(&m.kind) {
            // Interop: direct property; `tearoffInterop` for methods.
            return match m.kind {
                MemberKind::Method(_) => (
                    Js::prim(format!(
                        "dart.tearoffInterop({js}, {})",
                        self.ctx.js_null_checkable(&m.kind)
                    )),
                    ty,
                ),
                _ => (self.js_null_check(Js::prim(js), &m.kind), ty),
            };
        }
        match m.kind {
            MemberKind::Method(_) => {
                // Instance method tearoff.
                let key = if name.starts_with('_') {
                    self.private_sym(self.ctx.lib_of_class(m.class), name)
                } else if self.ctx.is_ext_member(recv_ty, name, false) {
                    self.dartx(&js_member_name(name))
                } else {
                    js::string_literal(&js_member_name(name))
                };
                (Js::prim(format!("dart.bind({}, {key})", recv.code)), ty)
            }
            _ => (Js::prim(js), ty),
        }
    }

    fn is_user_class_recv(&self, t: &Ty) -> bool {
        match t {
            Ty::Iface {
                class,
                nullable: false,
                ..
            } => {
                let lib = self.ctx.lib_of_class(*class);
                !self.ctx.libs[lib.0 as usize].is_sdk
            }
            _ => false,
        }
    }

    /// Applicable extension getter.
    pub fn try_extension_get(&mut self, recv: &Js, recv_ty: &Ty, name: &str) -> Option<(Js, Ty)> {
        let found = self.find_extension_member(recv_ty, name, false);
        self.forced_ext = None;
        let (ext, fid, subst) = found?;
        let f = self.ctx.program.function(fid);
        let e = self.ctx.program.extension(ext);
        // `Function.toJS`/`toJSCaptureThis` from `dart:js_interop`: the CFE's
        // interop transformer replaces them with `dart:js_util`'s
        // `_functionToJS{N}`, N = positional parameters of the static type
        // (`_js_interop_checks/.../js_util_optimizer.dart:1003-1041`; in the DDC
        // `toJSCaptureThis` always through `_functionToJSCaptureThisN`).
        if self.ctx.program.library(e.library).uri == "dart:js_interop"
            && e.name
                .is_some_and(|n| self.ctx.name(n) == "FunctionToJSExportedDartFunction")
            && matches!(name, "toJS" | "toJSCaptureThis")
        {
            if let Ty::Fn { pos, opt, .. } = recv_ty {
                let n = pos.len() + opt.len();
                let ret = match self.ctx.fn_ty(fid) {
                    Ty::Fn { ret, .. } => (*ret).clone(),
                    _ => Ty::Dynamic,
                };
                self.m.use_sdk("js_util");
                let r = &recv.code;
                let call = match name {
                    "toJS" if n <= 5 => format!("js_util._functionToJS{n}({r})"),
                    "toJS" => format!("js_util._functionToJSN({r}, {n})"),
                    _ => format!("js_util._functionToJSCaptureThisN({r}, {n})"),
                };
                return Some((Js::prim(call), ret));
            }
        }
        if let Some(js_name) = self.ctx.interop_extension_member(fid) {
            let ty = self.ctx.fn_ty(fid).subst_prop(&subst);
            let read = format!("{}{}", recv.at(P_PRIMARY), js::prop_access(&js_name));
            return match f.kind {
                FunctionKind::Getter => {
                    let ret = match &ty {
                        Ty::Fn { ret, .. } => (**ret).clone(),
                        _ => Ty::Dynamic,
                    };
                    Some((
                        self.js_null_check(Js::prim(read), &MemberKind::Getter(fid)),
                        ret,
                    ))
                }
                FunctionKind::Function => {
                    let checkable = self.ctx.js_null_checkable(&MemberKind::Method(fid));
                    Some((
                        Js::prim(format!("dart.tearoffInterop({read}, {checkable})")),
                        ty,
                    ))
                }
                _ => None,
            };
        }
        let ext_name = self.extension_js_name(ext);
        let lib_var = self.lib_var(e.library);
        let ty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let targs: Vec<String> = self.ctx.outline.extensions[ext.0 as usize]
            .type_params
            .iter()
            .map(|p| self.rti(subst.get(&p.0).unwrap_or(&Ty::Dynamic)))
            .collect();
        let mut args = targs;
        args.push(recv.code.clone());
        match f.kind {
            FunctionKind::Getter => {
                let ret = match &ty {
                    Ty::Fn { ret, .. } => (**ret).clone(),
                    _ => Ty::Dynamic,
                };
                Some((
                    Js::prim(format!(
                        "{lib_var}[{}]({})",
                        js::string_literal(&format!("{ext_name}|get#{name}")),
                        args.join(", ")
                    )),
                    ret,
                ))
            }
            FunctionKind::Function | FunctionKind::Operator => {
                // Extension method tearoff: a closure.
                let (pos_n, ..) = match &ty {
                    Ty::Fn { pos, .. } => (pos.len(), ()),
                    _ => (0, ()),
                };
                let params: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
                let mut call_args = args.clone();
                call_args.extend(params.iter().cloned());
                let rti = self.rti(&ty);
                Some((
                    Js::prim(format!(
                        "dart.fn(({}) => {lib_var}[{}]({}), {rti})",
                        params.join(", "),
                        js::string_literal(&format!("{ext_name}|{name}")),
                        call_args.join(", ")
                    )),
                    ty,
                ))
            }
            _ => None,
        }
    }

    /// Rtis of the extension's type parameters (in order), from the substitution.
    pub fn ext_type_args(
        &mut self,
        ext: dartforge_elements::model::ExtensionId,
        subst: &HashMap<u32, Ty>,
    ) -> Vec<String> {
        self.ctx.outline.extensions[ext.0 as usize]
            .type_params
            .iter()
            .map(|p| self.rti(subst.get(&p.0).unwrap_or(&Ty::Dynamic)))
            .collect()
    }

    pub fn extension_js_name(&self, ext: dartforge_elements::model::ExtensionId) -> String {
        let e = self.ctx.program.extension(ext);
        match e.name {
            Some(n) => self.name(n).to_string(),
            None => {
                // Index among the library's unnamed extensions.
                let mut idx = 0;
                for (i, x) in self.ctx.program.extensions.iter().enumerate() {
                    if x.library == e.library && x.name.is_none() {
                        if i == ext.0 as usize {
                            break;
                        }
                        idx += 1;
                    }
                }
                format!("_extension#{idx}")
            }
        }
    }

    /// `recv.name = v` through the setter `fid` of extension `ext`: the
    /// `Ext|set#name` function, or the direct write of an interop `external`
    /// setter (`Ctx::interop_extension_member`).
    pub fn extension_set(
        &mut self,
        ext: dartforge_elements::model::ExtensionId,
        fid: dartforge_elements::model::FunctionElementId,
        subst: &HashMap<u32, Ty>,
        recv: &Js,
        v: &Js,
        vty: &Ty,
    ) -> Js {
        if let Some(js_name) = self.ctx.interop_extension_member(fid) {
            let v = self.assert_interop(v.clone(), vty);
            return Js::new(
                format!(
                    "{}{} = {}",
                    recv.at(P_PRIMARY),
                    js::prop_access(&js_name),
                    v.at(P_ASSIGN)
                ),
                P_ASSIGN,
            );
        }
        let e = self.ctx.program.extension(ext);
        let lib_var = self.lib_var(e.library);
        let ext_name = self.extension_js_name(ext);
        let name = self
            .name(self.ctx.program.function(fid).name)
            .trim_end_matches("_=")
            .to_string();
        let mut args = self.ext_type_args(ext, subst);
        args.push(recv.code.clone());
        let result = self.temp();
        args.push(format!("{result} = {}", v.at(P_ASSIGN)));
        // A Dart assignment returns its RHS, even when the setter returns void.
        Js::new(
            format!(
                "({lib_var}[{}]({}), {result})",
                js::string_literal(&format!("{ext_name}|set#{name}")),
                args.join(", ")
            ),
            P_PRIMARY,
        )
    }

    /// Looks up an extension member applicable to the receiver type.
    pub fn find_extension_member(
        &self,
        recv_ty: &Ty,
        name: &str,
        setter: bool,
    ) -> Option<(
        dartforge_elements::model::ExtensionId,
        dartforge_elements::model::FunctionElementId,
        HashMap<u32, Ty>,
    )> {
        let key = if setter {
            format!("{name}_=")
        } else {
            name.to_string()
        };
        let sym = self.ctx.sym(&key)?;
        if let Some(forced) = self.forced_ext {
            let e = self.ctx.program.extension(forced);
            let fid = *e.instance_members.get(&sym)?;
            let data = &self.ctx.outline.extensions[forced.0 as usize];
            let on = self.ctx.ty_of(data.on);
            let mut subst = HashMap::new();
            let params: Vec<u32> = data.type_params.iter().map(|p| p.0).collect();
            self.match_type(&on, recv_ty, &params, &mut subst);
            return Some((forced, fid, subst));
        }
        // A type variable bounded by `dynamic` is a dynamic receiver: no
        // extension applies.
        if matches!(recv_ty, Ty::Param { .. }) && self.ctx.resolve_param_bound(recv_ty).is_dynamic()
        {
            return None;
        }
        let recv_nn = recv_ty;
        let mut best: Option<ScoredExtensionMatch> = None;
        for ext in self.ctx.visible_extensions(self.lib) {
            let e = self.ctx.program.extension(ext);
            let Some(&fid) = e.instance_members.get(&sym) else {
                continue;
            };
            let data = &self.ctx.outline.extensions[ext.0 as usize];
            let on = self.ctx.ty_of(data.on);
            let mut subst = HashMap::new();
            // Bind the extension's parameters by matching against the receiver.
            let params: Vec<u32> = data.type_params.iter().map(|p| p.0).collect();
            let ok = self.match_type(&on, recv_nn, &params, &mut subst);
            if !ok {
                continue;
            }
            let on_s = on.subst(&subst);
            if !self.ctx.is_subtype(recv_nn, &on_s) && !recv_nn.is_dynamic() {
                continue;
            }
            // The inferred arguments must satisfy the bounds
            // (`extension<T extends num> on List<T>` does not apply to a
            // `List<Object>`).
            let within_bounds = data.type_params.iter().all(|&pid| {
                let p = self.ctx.ty_param_of(pid);
                // (A generic function type is not compared: the emitter's
                // subtyping does not rename its type parameters.)
                subst.get(&p.id).is_none_or(|a| {
                    a.is_dynamic()
                        || matches!(a, Ty::Fn { type_params, .. } if !type_params.is_empty())
                        || self.ctx.is_subtype(a, &p.bound.subst(&subst))
                })
            });
            if !within_bounds {
                continue;
            }
            // Specificity: the more specific `on` wins; on a tie → the current library.
            let score = if e.library == self.lib { 1 } else { 0 };
            let better = match &best {
                None => true,
                Some((bext, _, bsubst, bscore)) => {
                    let bon = self
                        .ctx
                        .ty_of(self.ctx.outline.extensions[bext.0 as usize].on)
                        .subst(bsubst);
                    let a_more =
                        self.ctx.is_subtype(&on_s, &bon) && !self.ctx.is_subtype(&bon, &on_s);
                    let b_more =
                        self.ctx.is_subtype(&bon, &on_s) && !self.ctx.is_subtype(&on_s, &bon);
                    a_more || (!b_more && score > *bscore)
                }
            };
            if better {
                best = Some((ext, fid, subst, score));
            }
        }
        best.map(|(a, b, c, _)| (a, b, c))
    }

    /// Matches `pattern` (with free parameters `params`) against `actual`, filling `subst`.
    pub fn match_type(
        &self,
        pattern: &Ty,
        actual: &Ty,
        params: &[u32],
        subst: &mut HashMap<u32, Ty>,
    ) -> bool {
        match pattern {
            Ty::Param { id, .. } if params.contains(id) => {
                if matches!(actual, Ty::Dynamic | Ty::Never) {
                    return true;
                }
                if let Some(prev) = subst.get(id) {
                    let l = self.ctx.lub(prev, actual);
                    subst.insert(*id, l);
                } else {
                    subst.insert(*id, actual.non_null_if(pattern.is_nullable()));
                }
                true
            }
            Ty::Iface { class, args, .. } => {
                let actual_r = self.ctx.resolve_param_bound(actual);
                // Every function type is a subtype of `Function` (the
                // `dart:js_interop` extension `on Function` applies to `f.toJS`).
                if Some(*class) == self.ctx.function_
                    && matches!(actual_r.non_null(), Ty::Fn { .. })
                {
                    return true;
                }
                let Some(sup) = self.ctx.as_super(&actual_r.non_null(), *class) else {
                    return matches!(actual, Ty::Dynamic | Ty::Never | Ty::Null);
                };
                for (p, a) in args.iter().zip(sup.args().iter()) {
                    self.match_type(p, a, params, subst);
                }
                true
            }
            Ty::Fn {
                ret,
                pos,
                opt,
                named,
                ..
            } => {
                if let Ty::Fn {
                    ret: r2,
                    pos: p2,
                    opt: o2,
                    named: n2,
                    ..
                } = actual
                {
                    self.match_type(ret, r2, params, subst);
                    for (a, b) in pos.iter().chain(opt.iter()).zip(p2.iter().chain(o2.iter())) {
                        self.match_type(a, b, params, subst);
                    }
                    for (n, t, _) in named {
                        if let Some((_, t2, _)) = n2.iter().find(|(m, _, _)| m == n) {
                            self.match_type(t, t2, params, subst);
                        }
                    }
                }
                true
            }
            Ty::FutureOr { arg, .. } => {
                if let Ty::FutureOr { arg: a2, .. } = actual {
                    return self.match_type(arg, a2, params, subst);
                }
                if let Some(ft) = self
                    .ctx
                    .future_
                    .and_then(|f| self.ctx.as_super(&actual.non_null(), f))
                {
                    return self.match_type(
                        arg,
                        ft.args().first().unwrap_or(&Ty::Dynamic),
                        params,
                        subst,
                    );
                }
                self.match_type(arg, actual, params, subst)
            }
            Ty::Record { pos, named, .. } => {
                if let Ty::Record {
                    pos: p2, named: n2, ..
                } = actual
                {
                    for (a, b) in pos.iter().zip(p2.iter()) {
                        self.match_type(a, b, params, subst);
                    }
                    for (n, t) in named {
                        if let Some((_, t2)) = n2.iter().find(|(m, _)| m == n) {
                            self.match_type(t, t2, params, subst);
                        }
                    }
                }
                true
            }
            _ => true,
        }
    }

    // -----------------------------------------------------------------------
    // Selectors (with null-shorting)
    // -----------------------------------------------------------------------

    /// Emits a selector (`a.b`, `a[i]`, `f(x)`), returning the text without
    /// the `?.` guards, the type, and the accumulated guards.
    /// A link of a null-shorting chain: a property, index, call, or `!`.
    pub fn is_selector(&self, e: ExprId) -> bool {
        matches!(
            self.expr(e).kind,
            ExprKind::Property { .. }
                | ExprKind::Index { .. }
                | ExprKind::Call { .. }
                | ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    ..
                }
        )
    }

    pub fn emit_selector(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty, Vec<Guard>) {
        let expr = self.expr(e);
        if let Some(ty) = self.typed_loop_raw.get(&e).cloned() {
            match &expr.kind {
                ExprKind::Property { target, .. } => {
                    let (recv, _) = self.emit_expr(*target, None);
                    return (Js::prim(format!("{}.length", recv.code)), ty, vec![]);
                }
                ExprKind::Index { target, index, .. } => {
                    let (recv, _) = self.emit_expr(*target, None);
                    let (idx, _) = self.emit_expr(*index, None);
                    return (Js::prim(format!("{}[{}]", recv.code, idx.code)), ty, vec![]);
                }
                _ => unreachable!("typed loop proof contains only selectors"),
            }
        }
        // `a?.b!`: the null check belongs to the null-shorting chain
        // (`a == null ? null : nullCheck(a.b)`).
        if let ExprKind::Unary {
            op: UnaryOp::NullAssert,
            operand,
        } = &expr.kind
        {
            let operand = *operand;
            let (v, vty, guards) = if self.is_selector(operand) {
                self.emit_selector(operand, None)
            } else {
                let (v, vty) = self.emit_expr(operand, None);
                (v, vty, vec![])
            };
            if !vty.is_nullable() && !vty.is_dynamic() {
                return (v, vty, guards);
            }
            return (
                Js::prim(format!("dart.nullCheck({})", v.code)),
                vty.non_null(),
                guards,
            );
        }
        match &expr.kind {
            ExprKind::Property {
                target,
                name,
                null_aware,
            } => {
                let n = self.name(name.sym).to_string();
                if let Some((owner, member)) = self.ext_type_member(e) {
                    let (tjs, _, mut guards) = self.emit_target(*target);
                    let recv = if *null_aware {
                        let t = self.temp();
                        guards.push(Guard {
                            temp: t.clone(),
                            init: tjs.code,
                        });
                        Js::prim(t)
                    } else {
                        tjs
                    };
                    let (js, ty) = self.ext_type_read(&recv, Some(*target), owner, member);
                    let ty = if *null_aware {
                        ty.with_nullable(true)
                    } else {
                        ty
                    };
                    return (js, ty, guards);
                }
                // Static cases: prefix, class, enum.
                if let Some(r) = self.try_static_property(*target, &n) {
                    return (r.0, r.1, vec![]);
                }
                if let ExprKind::Super = self.expr(*target).kind {
                    let sup_ty = self.super_ty_for(&n, false);
                    let m = self.ctx.lookup_member(&sup_ty, &n, false);
                    let ty = m
                        .as_ref()
                        .map(|m| self.ctx.member_ty(m))
                        .unwrap_or(Ty::Dynamic);
                    let access = self.member_access(&sup_ty, &n, false);
                    if self.super_field_storage(&sup_ty, &n, false) {
                        return (Js::prim(format!("this{access}")), ty, vec![]);
                    }
                    // `super.m` of a method: the tear-off bound to `this` with
                    // the superclass's function.
                    if matches!(m.as_ref().map(|m| &m.kind), Some(MemberKind::Method(_))) {
                        return (
                            Js::prim(format!(
                                "dart.bind(this, {}, {})",
                                access_key(&access),
                                self.super_read(&access)
                            )),
                            ty,
                            vec![],
                        );
                    }
                    return (Js::prim(self.super_read(&access)), ty, vec![]);
                }
                let (tjs, tty, mut guards) = self.emit_target(*target);
                let (recv, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: tjs.code,
                    });
                    (Js::prim(t), tty.non_null())
                } else {
                    (tjs, tty)
                };
                // A type variable promoted to an intersection (`S & int`,
                // which the emitter does not model): the extension the common
                // resolution found.
                if self.forced_ext.is_none()
                    && matches!(recv_ty, Ty::Param { .. })
                    && let Some(Resolved::ExtensionMember { extension, .. }) =
                        self.common_resolution(e)
                {
                    self.forced_ext = Some(*extension);
                }
                let member = self.access_member(Some(e), &recv_ty, &n, false);
                let (js, ty) = self.emit_member_get(&recv, &recv_ty, &n, member);
                let js = self.contravariant_read_check(js, *target, &recv_ty, &n, &ty, true);
                let ty = if *null_aware {
                    ty.with_nullable(true)
                } else {
                    ty
                };
                (js, ty, guards)
            }
            ExprKind::Index {
                target,
                index,
                null_aware,
            } => {
                let (tjs, tty, mut guards) = self.emit_target(*target);
                let (recv, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: tjs.code,
                    });
                    (Js::prim(t), tty.non_null())
                } else {
                    (tjs, tty)
                };
                if let Some((owner, MemberRef::Function(f))) = self.ext_type_member(e) {
                    let ctx_i = self.ext_type_param(Some(*target), owner, f, 0);
                    let (ijs, _) = self.emit_expr(*index, ctx_i.as_ref());
                    let (js, ty) = self.ext_type_operator(&recv, Some(*target), owner, f, &[ijs]);
                    let ty = if *null_aware {
                        ty.with_nullable(true)
                    } else {
                        ty
                    };
                    return (js, ty, guards);
                }
                let (ijs, _) = self.emit_expr(*index, None);
                let (js, ty) = self.emit_index_get(&recv, &recv_ty, &ijs, Some(e));
                let ty = if *null_aware {
                    ty.with_nullable(true)
                } else {
                    ty
                };
                (js, ty, guards)
            }
            ExprKind::Call { target, arguments } => self.emit_call(*target, arguments, expected),
            _ => {
                let (js, ty) = self.emit_expr(e, expected);
                (js, ty, vec![])
            }
        }
    }

    /// Type of the superclass (Object when there is no `extends`).
    pub fn super_ty(&self) -> Ty {
        self.class
            .and_then(|c| {
                self.ctx
                    .direct_supers(&self.ctx.this_ty(c))
                    .first()
                    .cloned()
            })
            .filter(|t| matches!(t, Ty::Iface { .. }))
            .or_else(|| self.ctx.object.map(Ty::iface))
            .unwrap_or(Ty::Dynamic)
    }

    /// The type of `super` for the member `name`: the superclass, or — when
    /// the member comes from a mixin applied to the class (`extends B with M`)
    /// — the last mixin that declares it. Without this, `super.m()` of a mixin
    /// member fell into `dart.dsend(super, …)`, which is not even valid JS.
    pub fn super_ty_for(&self, name: &str, setter: bool) -> Ty {
        let base = self.super_ty();
        if self.ctx.lookup_member(&base, name, setter).is_some() {
            return base;
        }
        let Some(c) = self.class else { return base };
        let mixins = self.ctx.mixins_of(c);
        let supers = self.ctx.direct_supers(&self.ctx.this_ty(c));
        for t in supers.iter().rev() {
            if t.class().is_some_and(|k| mixins.contains(&k))
                && self.ctx.lookup_member(t, name, setter).is_some()
            {
                return t.clone();
            }
        }
        base
    }

    /// `super` cannot be written here (an `async` body, a constructor):
    /// members go through the superclass's prototype, with `this`.
    pub fn super_by_prototype(&self) -> bool {
        self.async_kind != AsyncKind::None || self.explicit_super
    }

    /// The member `super.name` resolves to is a field stored directly on the
    /// instance, without accessors on the prototype: a private field, or a
    /// public one nobody overrides in the production profile
    /// (`own_sdk::non_virtual_field`). Then `super.name` is `this.name`.
    pub fn super_field_storage(&self, sup_ty: &Ty, name: &str, setter: bool) -> bool {
        let Some(m) = self.ctx.lookup_member(sup_ty, name, setter) else {
            return false;
        };
        let MemberKind::Field(vid) = m.kind else {
            return false;
        };
        let var = self.ctx.program.variable(vid);
        if var.late {
            return false;
        }
        if name.starts_with('_') {
            return !crate::module::private_field_is_virtual(self.ctx, m.class, name);
        }
        crate::own_sdk::non_virtual_field(self.ctx, m.class, var.name)
    }

    /// The DDC's covariance check of a member read through a receiver other
    /// than `this` (a field, a getter, a method's result or tear-off) whose
    /// declared type uses a type parameter of its class in a contravariant
    /// position: the value may not have the static type (`C<Object> c =
    /// C<String>(); c.f` with `void Function(T) f`), so it is cast to it.
    pub fn contravariant_read_check(
        &self,
        js: Js,
        recv_expr: ExprId,
        recv_ty: &Ty,
        name: &str,
        result: &Ty,
        tear_off: bool,
    ) -> Js {
        if matches!(self.expr(recv_expr).kind, ExprKind::This | ExprKind::Super)
            || self.ctx.filter.is_some_and(|f| f.omit_implicit_checks())
            || crate::own_sdk::is_top(self.ctx, result)
        {
            return js;
        }
        let Some(m) = self.ctx.lookup_member(recv_ty, name, false) else {
            return js;
        };
        if self.ctx.libs[self.ctx.lib_of_class(m.class).0 as usize].is_sdk {
            return js;
        }
        let declared = match m.kind {
            // The tear-off of a method has the method's whole type.
            MemberKind::Method(fid) if tear_off => self.ctx.fn_ty(fid),
            MemberKind::Method(fid) | MemberKind::Getter(fid) => match self.ctx.fn_ty(fid) {
                Ty::Fn { ret, .. } => *ret,
                t => t,
            },
            MemberKind::Field(vid) => self.ctx.var_ty(vid),
            MemberKind::Setter(_) => return js,
        };
        let ids: std::collections::HashSet<u32> = self.ctx.class_params[m.class.0 as usize]
            .iter()
            .map(|p| p.id)
            .collect();
        if !crate::own_sdk::occurs_covariantly(&declared, &ids, false) {
            return js;
        }
        Js::prim(format!("{}[_as]({})", self.rti(result), js.code))
    }

    /// `super{access}` read with `this` as the receiver.
    pub fn super_read(&self, access: &str) -> String {
        if self.super_by_prototype() {
            format!(
                "dart.global.Reflect.get({}, {}, this)",
                self.super_ref(),
                access_key(access)
            )
        } else {
            format!("super{access}")
        }
    }

    pub fn super_ref(&self) -> String {
        if !self.super_by_prototype() {
            "super".to_string()
        } else {
            match self.class {
                Some(c) => format!("Object.getPrototypeOf({}.prototype)", self.class_ref(c)),
                None => "super".to_string(),
            }
        }
    }

    /// Target of a selector: if it is a selector too, propagates the guards.
    pub fn emit_target(&mut self, target: ExprId) -> (Js, Ty, Vec<Guard>) {
        let t = self.expr(target);
        // `Ext(x).member`: explicit extension application.
        if let ExprKind::Call {
            target: ct,
            arguments,
        } = &t.kind
        {
            // `Ext(x)` or `prefix.Ext(x)` (the ngdart template writes
            // `import18.FunctionToJSExportedDartFunction(h).toJS`).
            let ext = match &self.expr(*ct).kind {
                ExprKind::Identifier(id) => match self.ident_target(id.sym, *ct) {
                    IdentTarget::Element(Element::Extension(ext)) => Some(ext),
                    _ => None,
                },
                ExprKind::Property {
                    target: pt,
                    name,
                    null_aware: false,
                } => match &self.expr(*pt).kind {
                    ExprKind::Identifier(pid) => match self.ident_target(pid.sym, *pt) {
                        IdentTarget::Prefix(p) => self
                            .ctx
                            .program
                            .lookup_prefixed_in_unit(self.unit, p, name.sym)
                            .and_then(|b| match b.getter {
                                Some(Element::Extension(ext)) => Some(ext),
                                _ => None,
                            }),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            };
            if let (Some(ext), Some(a)) = (ext, arguments.args.first()) {
                let (js, ty) = self.emit_expr(a.value, None);
                self.forced_ext = Some(ext);
                return (js, ty, vec![]);
            }
        }
        match &t.kind {
            ExprKind::Property { .. }
            | ExprKind::Index { .. }
            | ExprKind::Call { .. }
            | ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            } => {
                // As in `emit_expr`: a promoted field read has the promoted type.
                let (js, ty, guards) = self.emit_selector(target, None);
                (js, self.refine_by_common(target, ty), guards)
            }
            _ => {
                let (js, ty) = self.emit_expr(target, None);
                (js, ty, vec![])
            }
        }
    }

    /// `prefix.x`, `Class.static`, `Enum.value`, `prefix.Class.static`.
    pub fn try_static_property(&mut self, target: ExprId, name: &str) -> Option<(Js, Ty)> {
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(id) => match self.ident_target(id.sym, target) {
                IdentTarget::Prefix(p) => {
                    if name == "loadLibrary" {
                        // The tear-off `p.loadLibrary`.
                        if !self.deferred_import(p) {
                            return None;
                        }
                        let (call, _) = self.load_library(p);
                        let ty = Ty::Fn {
                            type_params: vec![],
                            ret: Box::new(self.ctx.t_future(Ty::Void)),
                            pos: vec![],
                            opt: vec![],
                            named: vec![],
                            nullable: false,
                        };
                        let rti = self.rti(&ty);
                        return Some((
                            Js::prim(format!("dart.fn(() => {}, {rti})", call.code)),
                            ty,
                        ));
                    }
                    let sym = self.ctx.sym(name)?;
                    let b = self
                        .ctx
                        .program
                        .lookup_prefixed_in_unit(self.unit, p, sym)?;
                    let el = b.getter.or(b.setter)?;
                    Some(self.emit_element_get(el, name))
                }
                IdentTarget::Element(Element::Class(c)) => self.static_member_get(c, name),
                // `Alias.x`: a static of the class the alias denotes.
                IdentTarget::Element(Element::Typedef(td)) => {
                    let (c, _, _) = self.alias_creation(td)?;
                    self.static_member_get(c, name)
                }
                IdentTarget::Element(Element::Extension(ext)) => {
                    let e = self.ctx.program.extension(ext);
                    let sym = self.ctx.sym(name)?;
                    let ext_name = self.extension_js_name(ext);
                    let lib_var = self.lib_var(e.library);
                    if let Some(&vid) = e
                        .fields
                        .iter()
                        .find(|v| self.ctx.program.variable(**v).name == sym)
                    {
                        return Some((
                            Js::prim(format!(
                                "{lib_var}[{}]",
                                js::string_literal(&format!("{ext_name}|{name}"))
                            )),
                            self.ctx.var_ty(vid),
                        ));
                    }
                    if let Some(&fid) = e.static_members.get(&sym) {
                        let f = self.ctx.program.function(fid);
                        let key = if f.kind == FunctionKind::Getter {
                            format!("{ext_name}|get#{name}")
                        } else {
                            format!("{ext_name}|{name}")
                        };
                        let js = format!("{lib_var}[{}]", js::string_literal(&key));
                        if f.kind == FunctionKind::Getter {
                            return Some((
                                Js::prim(format!("{js}()")),
                                self.ctx
                                    .ty_of(self.ctx.outline.functions[fid.0 as usize].return_type),
                            ));
                        }
                        let ty = self.ctx.fn_ty(fid);
                        return Some((self.tearoff_static(&js, &ty), ty));
                    }
                    None
                }
                IdentTarget::TypeParam(_) => None,
                _ => None,
            },
            ExprKind::Property {
                target: t2,
                name: n2,
                ..
            } => {
                // prefix.Class.member
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Prefix(p) = self.ident_target(id.sym, *t2) {
                        let sym = self.ctx.sym(self.name(n2.sym))?;
                        let b = self
                            .ctx
                            .program
                            .lookup_prefixed_in_unit(self.unit, p, sym)?;
                        if let Some(Element::Class(c)) = b.getter {
                            return self.static_member_get(c, name);
                        }
                    }
                }
                None
            }
            ExprKind::TypeArguments {
                target: t2,
                type_args,
            } => {
                // `C<T>.new` / `C<T>.named`: tearoff of an instantiated constructor.
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Element(Element::Class(c)) = self.ident_target(id.sym, *t2)
                    {
                        let targs: Vec<Ty> =
                            type_args.iter().map(|t| self.resolve_type(*t)).collect();
                        return self.ctor_tearoff(c, targs, name);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Tearoff of the constructor `C<args>.name` as a closure.
    pub fn ctor_tearoff(&mut self, c: ClassId, targs: Vec<Ty>, name: &str) -> Option<(Js, Ty)> {
        if crate::extension_type::is_erased_extension_type(self.ctx, c) {
            return self.ext_type_ctor_tearoff(c, targs, name);
        }
        let class = self.ctx.program.class(c);
        let key = if name == "new" {
            self.ctx.empty_sym
        } else {
            self.ctx.sym(name)
        };
        let fid = *class.constructors.get(&key?)?;
        let params = &self.ctx.class_params[c.0 as usize];
        let mut subst = HashMap::new();
        for (p, a) in params.iter().zip(targs.iter()) {
            subst.insert(p.id, a.clone());
        }
        let ty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let inst = Ty::Iface {
            class: c,
            args: targs.clone(),
            nullable: false,
        };
        let ty = match ty {
            Ty::Fn {
                pos,
                opt,
                named,
                nullable,
                ..
            } => Ty::Fn {
                type_params: vec![],
                ret: Box::new(inst.clone()),
                pos,
                opt,
                named,
                nullable,
            },
            t => t,
        };
        let (pos_n, ..) = match &ty {
            Ty::Fn { pos, .. } => (pos.len(), ()),
            _ => (0, ()),
        };
        let params_js: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
        let f = self.ctx.program.function(fid);
        let cname = if name == "new" {
            "new".to_string()
        } else {
            static_member_name(name)
        };
        // The static tearoff exists when the class has no type parameters of
        // its own (it passes the class's `rti` to the constructor itself).
        if self.ctx.class_params[c.0 as usize].is_empty() {
            let rti = self.rti(&ty);
            return Some((
                Js::prim(format!(
                    "dart.fn({}[{}], {rti})",
                    self.class_ref(c),
                    js::string_literal(&format!("_#{cname}#tearOff"))
                )),
                ty,
            ));
        }
        let mut args = params_js.clone();
        if self.ctx.requires_rti(c) {
            args.insert(0, self.rti(&inst));
        }
        let call = if f.factory {
            format!("{}.{cname}({})", self.class_ref(c), args.join(", "))
        } else {
            format!("new {}.{cname}({})", self.class_ref(c), args.join(", "))
        };
        let rti = self.rti(&ty);
        Some((
            Js::prim(format!(
                "dart.fn(({}) => {call}, {rti})",
                params_js.join(", ")
            )),
            ty,
        ))
    }

    pub fn static_member_get(&mut self, c: ClassId, name: &str) -> Option<(Js, Ty)> {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if let Some(mk) = self.ctx.declared_static(k, name, false) {
                return Some(self.emit_static_get(k, name, mk));
            }
            cur = self.ctx.superclass_of(k);
        }
        // Enum constants.
        {
            let class = self.ctx.program.class(c);
            if class.kind == ClassKind::Enum {
                if let Some(sym) = self.ctx.sym(name) {
                    if class
                        .enum_constants
                        .iter()
                        .any(|v| self.ctx.program.variable(*v).name == sym)
                    {
                        let cls = self.class_ref(c);
                        return Some((
                            Js::prim(format!(
                                "{cls}{}",
                                js::prop_access(&static_member_name(name))
                            )),
                            Ty::iface(c),
                        ));
                    }
                }
            }
        }
        // Constructor as a tearoff: `C.new`/`C.named` (in an extension type,
        // the primary one too, which is not an element).
        if crate::extension_type::is_erased_extension_type(self.ctx, c) {
            if !crate::extension_type::has_constructor(
                self.ctx,
                c,
                if name == "new" { "" } else { name },
            ) {
                return None;
            }
            let targs: Vec<Ty> = self.ctx.class_params[c.0 as usize]
                .iter()
                .map(|_| Ty::Dynamic)
                .collect();
            return self.ext_type_ctor_tearoff(c, targs, name);
        }
        let class = self.ctx.program.class(c);
        let key = if name == "new" {
            self.ctx.empty_sym
        } else {
            self.ctx.sym(name)
        };
        if let Some(sym) = key {
            if class.constructors.contains_key(&sym) {
                let targs: Vec<Ty> = self.ctx.class_params[c.0 as usize]
                    .iter()
                    .map(|_| Ty::Dynamic)
                    .collect();
                return self.ctor_tearoff(c, targs, name);
            }
            if let Some(&fid) = class.constructors.get(&sym) {
                let ty = self.ctx.fn_ty(fid);
                let (pos_n, ..) = match &ty {
                    Ty::Fn { pos, .. } => (pos.len(), ()),
                    _ => (0, ()),
                };
                let params: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
                let f = self.ctx.program.function(fid);
                let cname = if name == "new" {
                    "new".to_string()
                } else {
                    static_member_name(name)
                };
                let call = if f.factory {
                    format!("{}.{cname}({})", self.class_ref(c), params.join(", "))
                } else {
                    format!("new {}.{cname}({})", self.class_ref(c), params.join(", "))
                };
                let rti = self.rti(&ty);
                return Some((
                    Js::prim(format!("dart.fn(({}) => {call}, {rti})", params.join(", "))),
                    ty,
                ));
            }
        }
        None
    }

    /// `recv[i]`; `node` is the index expression in the source, when there is
    /// one (the operator is the one the common resolution recorded for it,
    /// [`Self::access_member`]).
    pub fn emit_index_get(
        &mut self,
        recv: &Js,
        recv_ty: &Ty,
        idx: &Js,
        node: Option<ExprId>,
    ) -> (Js, Ty) {
        if recv_ty.is_dynamic() {
            return (
                Js::prim(format!("dart.dsend({}, '_get', [{}])", recv.code, idx.code)),
                Ty::Dynamic,
            );
        }
        // `Ext(x)[i]`: the explicit extension's operator.
        let class_member = if self.forced_ext.is_some() {
            None
        } else {
            self.access_member(node, recv_ty, "[]", false)
        };
        match class_member {
            Some(m) => {
                let ty = match self.ctx.member_ty(&m) {
                    Ty::Fn { ret, .. } => (*ret).clone(),
                    _ => Ty::Dynamic,
                };
                let access = self.member_access(recv_ty, "[]", false);
                (
                    Js::prim(format!("{}{access}({})", recv.at(P_PRIMARY), idx.code)),
                    ty,
                )
            }
            None => {
                let found = self.find_extension_member(recv_ty, "[]", false);
                self.forced_ext = None;
                if let Some((ext, fid, subst)) = found {
                    let e = self.ctx.program.extension(ext);
                    let lib_var = self.lib_var(e.library);
                    let ext_name = self.extension_js_name(ext);
                    let ty = match self.ctx.fn_ty(fid).subst_prop(&subst) {
                        Ty::Fn { ret, .. } => (*ret).clone(),
                        _ => Ty::Dynamic,
                    };
                    let mut args = self.ext_type_args(ext, &subst);
                    args.push(recv.code.clone());
                    args.push(idx.code.clone());
                    return (
                        Js::prim(format!(
                            "{lib_var}[{}]({})",
                            js::string_literal(&format!("{ext_name}|[]")),
                            args.join(", ")
                        )),
                        ty,
                    );
                }
                (
                    Js::prim(format!("dart.dsend({}, '_get', [{}])", recv.code, idx.code)),
                    Ty::Dynamic,
                )
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operators
    // -----------------------------------------------------------------------

    /// An expression whose value is discarded (an expression statement, a
    /// `for` update): `x++` and `x--` are emitted as `++x` and `--x`, without
    /// the temporary that keeps the old value.
    pub fn emit_discarded(&mut self, e: ExprId) -> Js {
        // The type checker (the analysis mode) sees the expression as written.
        if let (
            None,
            ExprKind::Unary {
                op: op @ (UnaryOp::PostfixInc | UnaryOp::PostfixDec),
                operand,
            },
        ) = (&self.ctx.type_check, &self.expr(e).kind)
        {
            let (op, operand) = (*op, *operand);
            let prefix = if op == UnaryOp::PostfixInc {
                UnaryOp::PrefixInc
            } else {
                UnaryOp::PrefixDec
            };
            return self.emit_unary(prefix, operand, e).0;
        }
        self.emit_expr(e, None).0
    }

    fn emit_unary(&mut self, op: UnaryOp, operand: ExprId, e: ExprId) -> (Js, Ty) {
        match op {
            UnaryOp::Not => {
                let (c, _) = self.emit_cond(operand);
                let p = std::mem::take(&mut self.pending_promotions);
                let n = std::mem::take(&mut self.negated_promotions);
                self.pending_promotions = n;
                self.negated_promotions = p;
                (
                    Js::new(format!("!{}", Js::new(c, P_PRIMARY).at(P_UNARY)), P_UNARY),
                    self.ctx.t_bool(),
                )
            }
            UnaryOp::Neg => {
                let (v, vty) = self.emit_expr(operand, None);
                if self.ctx.is_num_like_nullable(&vty) {
                    let v = self.not_null(&v, &vty);
                    return (
                        Js::new(format!("-{}", v.at(P_UNARY)), P_UNARY),
                        vty.non_null(),
                    );
                }
                if vty.is_dynamic() {
                    return (
                        Js::prim(format!("dart.dsend({}, '_negate', [])", v.code)),
                        Ty::Dynamic,
                    );
                }
                let class_member = if self.forced_ext.is_some() {
                    None
                } else {
                    self.access_member(Some(e), &vty, "unary-", false)
                };
                match class_member {
                    Some(m) => {
                        let ret = match self.ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(&vty, "unary-", false);
                        (Js::prim(format!("{}{access}()", v.at(P_PRIMARY))), ret)
                    }
                    None => self
                        .unary_extension_call(&v, &vty, "unary-")
                        .unwrap_or_else(|| {
                            (
                                Js::prim(format!("dart.dsend({}, '_negate', [])", v.code)),
                                Ty::Dynamic,
                            )
                        }),
                }
            }
            UnaryOp::BitNot => {
                let (v, vty) = self.emit_expr(operand, None);
                if self.ctx.is_num_like_nullable(&vty) {
                    let v = self.not_null(&v, &vty);
                    return (
                        Js::new(format!("~{} >>> 0", v.at(P_UNARY)), P_SHIFT),
                        self.ctx.t_int(),
                    );
                }
                if vty.is_dynamic() {
                    return (
                        Js::prim(format!("dart.dsend({}, '~', [])", v.code)),
                        Ty::Dynamic,
                    );
                }
                let class_member = if self.forced_ext.is_some() {
                    None
                } else {
                    self.access_member(Some(e), &vty, "~", false)
                };
                match class_member {
                    Some(m) => {
                        let ret = match self.ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(&vty, "~", false);
                        (Js::prim(format!("{}{access}()", v.at(P_PRIMARY))), ret)
                    }
                    None => self.unary_extension_call(&v, &vty, "~").unwrap_or_else(|| {
                        (
                            Js::prim(format!("dart.dsend({}, '~', [])", v.code)),
                            Ty::Dynamic,
                        )
                    }),
                }
            }
            UnaryOp::NullAssert => {
                let (v, vty) = self.emit_expr(operand, None);
                if !vty.is_nullable() && !vty.is_dynamic() {
                    return (v, vty);
                }
                (
                    Js::prim(format!("dart.nullCheck({})", v.code)),
                    vty.non_null(),
                )
            }
            UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                let is_inc = matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc);
                let prefix = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
                let bop = if is_inc { BinaryOp::Add } else { BinaryOp::Sub };
                self.emit_compound(operand, bop, None, Some(1), prefix, e)
            }
        }
    }

    pub fn not_null(&self, v: &Js, ty: &Ty) -> Js {
        if ty.is_nullable() && !ty.is_dynamic() {
            Js::prim(format!("dart.notNull({})", v.code))
        } else {
            v.clone()
        }
    }

    /// Binary operator applied to already emitted values, with types.
    /// `node` is the operator expression in the source, when there is one: the
    /// member is the one the common resolution recorded for it
    /// ([`Self::access_member`]).
    pub fn emit_binop_values(
        &mut self,
        op: BinaryOp,
        l: Js,
        lt: &Ty,
        r: Js,
        rt: &Ty,
        node: Option<ExprId>,
    ) -> (Js, Ty) {
        let ctx = self.ctx;
        if node.is_some()
            && node == self.sdk_number_comparison
            && op == BinaryOp::Gt
            && !self.is_closure_body
        {
            // JS remainder can produce BigInt under foreign borrowing. Preserve
            // its original symbol dispatch, including errors and custom behavior.
            let access = self.member_access(lt, ">", false);
            return (
                Js::new(
                    format!(
                        "typeof {} === 'number' ? {} > {} : {}{}({})",
                        l.code,
                        l.at(P_REL),
                        r.at(P_REL + 1),
                        l.at(P_PRIMARY),
                        access,
                        r.code
                    ),
                    P_COND,
                ),
                ctx.t_bool(),
            );
        }
        if op == BinaryOp::Rem
            && node.is_some_and(|id| self.counter_modulo.contains(&id))
            && ctx.is_int(lt)
            && ctx.is_int(rt)
        {
            return (
                Js::new(format!("{} % {}", l.at(P_MUL), r.at(P_MUL + 1)), P_MUL),
                ctx.t_int(),
            );
        }
        let num_l = ctx.is_num_like_nullable(lt);
        let num_r = ctx.is_num_like_nullable(rt) || matches!(rt, Ty::Dynamic);
        if num_l
            && (num_r || ctx.is_num_like_nullable(rt))
            && !matches!(
                op,
                BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull
            )
        {
            let ln = self.not_null(&l, lt);
            let rn = self.not_null(&r, rt);
            let is_int = ctx.is_int(&lt.non_null()) && ctx.is_int(&rt.non_null());
            let is_double = ctx.is_double(&lt.non_null()) || ctx.is_double(&rt.non_null());
            let arith_ty = if is_int {
                ctx.t_int()
            } else if is_double {
                ctx.t_double()
            } else {
                ctx.t_num()
            };
            let dyn_r = matches!(rt, Ty::Dynamic);
            let rn = if dyn_r && !ctx.filter.is_some_and(|f| f.omit_implicit_checks()) {
                // Numeric operators have a checked argument even when their
                // receiver is statically known. A null check alone permits JS
                // coercion of strings, booleans and arbitrary user objects.
                let argument_ty = if matches!(
                    op,
                    BinaryOp::Shl
                        | BinaryOp::Shr
                        | BinaryOp::UShr
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor
                ) {
                    ctx.t_int()
                } else {
                    ctx.t_num()
                };
                self.as_cast(&r, &argument_ty)
            } else if dyn_r {
                Js::prim(format!("dart.notNull({})", rn.code))
            } else {
                rn
            };
            return match op {
                BinaryOp::Add => (
                    Js::new(format!("{} + {}", ln.at(P_ADD), rn.at(P_ADD + 1)), P_ADD),
                    arith_ty,
                ),
                BinaryOp::Sub => (
                    Js::new(format!("{} - {}", ln.at(P_ADD), rn.at(P_ADD + 1)), P_ADD),
                    arith_ty,
                ),
                BinaryOp::Mul => (
                    Js::new(format!("{} * {}", ln.at(P_MUL), rn.at(P_MUL + 1)), P_MUL),
                    arith_ty,
                ),
                BinaryOp::Div => (
                    Js::new(format!("{} / {}", ln.at(P_MUL), rn.at(P_MUL + 1)), P_MUL),
                    ctx.t_double(),
                ),
                BinaryOp::TruncDiv => (
                    Js::prim(format!(
                        "{}[{}]({})",
                        ln.at(P_PRIMARY),
                        self.dartx("~/"),
                        rn.code
                    )),
                    ctx.t_int(),
                ),
                BinaryOp::Rem => (
                    Js::prim(format!(
                        "{}[{}]({})",
                        ln.at(P_PRIMARY),
                        self.dartx("%"),
                        rn.code
                    )),
                    arith_ty,
                ),
                BinaryOp::Shl => (
                    Js::prim(format!(
                        "{}[{}]({})",
                        ln.at(P_PRIMARY),
                        self.dartx("<<"),
                        rn.code
                    )),
                    ctx.t_int(),
                ),
                BinaryOp::Shr => (
                    Js::prim(format!(
                        "{}[{}]({})",
                        ln.at(P_PRIMARY),
                        self.dartx(">>"),
                        rn.code
                    )),
                    ctx.t_int(),
                ),
                BinaryOp::UShr => (
                    Js::prim(format!(
                        "{}[{}]({})",
                        ln.at(P_PRIMARY),
                        self.dartx(">>>"),
                        rn.code
                    )),
                    ctx.t_int(),
                ),
                BinaryOp::BitAnd => (
                    Js::new(
                        format!("({} & {}) >>> 0", ln.at(P_BITAND), rn.at(P_BITAND + 1)),
                        P_SHIFT,
                    ),
                    ctx.t_int(),
                ),
                BinaryOp::BitOr => (
                    Js::new(
                        format!("({} | {}) >>> 0", ln.at(P_BITOR), rn.at(P_BITOR + 1)),
                        P_SHIFT,
                    ),
                    ctx.t_int(),
                ),
                BinaryOp::BitXor => (
                    Js::new(
                        format!("({} ^ {}) >>> 0", ln.at(P_BITXOR), rn.at(P_BITXOR + 1)),
                        P_SHIFT,
                    ),
                    ctx.t_int(),
                ),
                BinaryOp::Lt => (
                    Js::new(format!("{} < {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL),
                    ctx.t_bool(),
                ),
                BinaryOp::Gt => (
                    Js::new(format!("{} > {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL),
                    ctx.t_bool(),
                ),
                BinaryOp::LtEq => (
                    Js::new(format!("{} <= {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL),
                    ctx.t_bool(),
                ),
                BinaryOp::GtEq => (
                    Js::new(format!("{} >= {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL),
                    ctx.t_bool(),
                ),
                _ => unreachable!(),
            };
        }
        if ctx.is_string(&lt.non_null()) && op == BinaryOp::Add && !lt.is_nullable() {
            let rn = if ctx.is_string(rt) {
                r
            } else {
                Js::prim(format!("dart.nullCheck({})", r.code))
            };
            return (
                Js::new(format!("{} + {}", l.at(P_ADD), rn.at(P_ADD + 1)), P_ADD),
                ctx.t_string(),
            );
        }
        if ctx.is_string(&lt.non_null()) && op == BinaryOp::Mul && !lt.is_nullable() {
            return (
                Js::prim(format!(
                    "{}[{}]({})",
                    l.at(P_PRIMARY),
                    self.dartx("*"),
                    r.code
                )),
                ctx.t_string(),
            );
        }
        match op {
            BinaryOp::Eq | BinaryOp::NotEq => {
                let neg = op == BinaryOp::NotEq;
                let js = self.emit_equals(&l, lt, &r, rt);
                if neg {
                    (
                        Js::new(format!("!{}", js.at(P_UNARY)), P_UNARY),
                        ctx.t_bool(),
                    )
                } else {
                    (js, ctx.t_bool())
                }
            }
            _ => {
                let name = binop_name(op);
                if lt.is_dynamic() {
                    return (
                        Js::prim(format!(
                            "dart.dsend({}, {}, [{}])",
                            l.code,
                            js::string_literal(&crate::body::js_member_name(name)),
                            r.code
                        )),
                        Ty::Dynamic,
                    );
                }
                match self.access_member(node, lt, name, false) {
                    Some(m) => {
                        let ret = match ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(lt, name, false);
                        (
                            Js::prim(format!("{}{access}({})", l.at(P_PRIMARY), r.code)),
                            ret,
                        )
                    }
                    None => {
                        if let Some((ext, fid, subst)) = self.find_extension_member(lt, name, false)
                        {
                            let e = ctx.program.extension(ext);
                            let lib_var = self.lib_var(e.library);
                            let ext_name = self.extension_js_name(ext);
                            let ret = match ctx.fn_ty(fid).subst_prop(&subst) {
                                Ty::Fn { ret, .. } => (*ret).clone(),
                                _ => Ty::Dynamic,
                            };
                            let mut args = self.ext_type_args(ext, &subst);
                            args.push(l.code.clone());
                            args.push(r.code.clone());
                            return (
                                Js::prim(format!(
                                    "{lib_var}[{}]({})",
                                    js::string_literal(&format!("{ext_name}|{name}")),
                                    args.join(", ")
                                )),
                                ret,
                            );
                        }
                        (
                            Js::prim(format!(
                                "dart.dsend({}, {}, [{}])",
                                l.code,
                                js::string_literal(&crate::body::js_member_name(name)),
                                r.code
                            )),
                            Ty::Dynamic,
                        )
                    }
                }
            }
        }
    }

    /// Records the `x != null` / `x == null` promotion for the variable `sym`.
    pub fn note_null_promotion(&mut self, sym: dartforge_intern::SymbolId, ty: &Ty, negated: bool) {
        if ty.is_nullable() && !ty.is_dynamic() {
            let nn = ty.non_null();
            if negated {
                self.pending_promotions.push((sym, nn));
            } else {
                self.negated_promotions.push((sym, nn));
            }
        }
    }

    pub fn emit_equals(&mut self, l: &Js, lt: &Ty, r: &Js, rt: &Ty) -> Js {
        let ctx = self.ctx;
        if matches!(lt, Ty::Null) || matches!(rt, Ty::Null) || l.code == "null" || r.code == "null"
        {
            return Js::new(format!("{} == {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
        }
        let prim = ctx.is_js_primitive(&lt.non_null())
            && (ctx.is_js_primitive(&rt.non_null()) || rt.is_dynamic());
        let is_enum = lt.class().is_some_and(|c| ctx.is_enum_class(c));
        if prim || is_enum {
            if !lt.is_nullable() || !rt.is_nullable() {
                return Js::new(format!("{} === {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
            }
            return Js::new(format!("{} == {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
        }
        Js::prim(format!("dart.equals({}, {})", l.code, r.code))
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
        expected: Option<&Ty>,
        e: ExprId,
    ) -> (Js, Ty) {
        match op {
            BinaryOp::And => {
                let (l, _) = self.emit_cond(left);
                let promos = std::mem::take(&mut self.pending_promotions);
                self.push_scope();
                for (sym, t) in &promos {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                let (r, _) = self.emit_cond(right);
                self.pop_scope();
                // `a && b` true: both promote. False: neither side is known
                // to be false, so nothing is promoted.
                let right_promos = std::mem::take(&mut self.pending_promotions);
                self.pending_promotions = promos;
                self.pending_promotions.extend(right_promos);
                self.negated_promotions.clear();
                return (
                    Js::new(
                        format!(
                            "{} && {}",
                            Js::new(l, P_PRIMARY).at(P_AND),
                            Js::new(r, P_PRIMARY).at(P_AND + 1)
                        ),
                        P_AND,
                    ),
                    self.ctx.t_bool(),
                );
            }
            BinaryOp::Or => {
                let (l, _) = self.emit_cond(left);
                self.pending_promotions.clear();
                let (r, _) = self.emit_cond(right);
                self.pending_promotions.clear();
                return (
                    Js::new(
                        format!(
                            "{} || {}",
                            Js::new(l, P_PRIMARY).at(P_OR),
                            Js::new(r, P_PRIMARY).at(P_OR + 1)
                        ),
                        P_OR,
                    ),
                    self.ctx.t_bool(),
                );
            }
            BinaryOp::IfNull => {
                // The context of the left operand is the nullable version of
                // the context (`K?`): a null `dynamic` must not go through the
                // `K` test before the `??`; the result's context applies to the
                // whole expression.
                let context = expected.map(|t| t.with_nullable(true));
                self.uncoerced.insert(left);
                let (l, lt) = self.emit_expr(left, context.as_ref());
                self.uncoerced.insert(right);
                let (r, rt) = self.emit_expr(right, expected.or(Some(&lt.non_null())));
                let ty = self.ctx.lub(&lt.non_null(), &rt);
                // A constant (or a plain name) is read twice instead of
                // through a temporary: default values have no body to declare
                // one in.
                if self.in_const || crate::expr::is_simple(&l.code) {
                    return (
                        Js::new(
                            format!(
                                "{} == null ? {} : {}",
                                l.at(P_EQ),
                                r.at(P_COND),
                                l.at(P_COND)
                            ),
                            P_COND,
                        )
                        .paren(),
                        ty,
                    );
                }
                let t = self.temp();
                return (
                    Js::new(
                        format!("{t} = {}, {t} == null ? {} : {t}", l.code, r.at(P_COND)),
                        P_COMMA,
                    )
                    .paren(),
                    ty,
                );
            }
            _ => {}
        }
        // `super == x` / `super + x`: direct call of the superclass's operator.
        if let ExprKind::Super = self.expr(left).kind {
            let sup_ty = self.super_ty_for(
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                    "=="
                } else {
                    binop_name(op)
                },
                false,
            );
            let name = binop_name(op);
            let is_eq = matches!(op, BinaryOp::Eq | BinaryOp::NotEq);
            let mname = if is_eq { "==" } else { name };
            let (r, _) = self.emit_expr(right, None);
            let access = self.member_access(&sup_ty, mname, false);
            let call = if self.super_by_prototype() {
                format!("{}{access}.call(this, {})", self.super_ref(), r.code)
            } else {
                format!("super{access}({})", r.code)
            };
            let ret = if is_eq {
                self.ctx.t_bool()
            } else {
                self.ctx
                    .lookup_member(&sup_ty, mname, false)
                    .map(|m| match self.ctx.member_ty(&m) {
                        Ty::Fn { ret, .. } => (*ret).clone(),
                        _ => Ty::Dynamic,
                    })
                    .unwrap_or(Ty::Dynamic)
            };
            if op == BinaryOp::NotEq {
                return (Js::new(format!("!{call}"), P_UNARY), ret);
            }
            return (Js::prim(call), ret);
        }
        let (l, lt) = self.emit_expr(left, None);
        if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            if let (ExprKind::Identifier(n), ExprKind::Null) =
                (&self.expr(left).kind, &self.expr(right).kind)
            {
                if self.lookup_local(n.sym).is_some() {
                    self.note_null_promotion(n.sym, &lt, op == BinaryOp::NotEq);
                }
            }
        }
        let r_expected: Option<Ty> =
            if self.ctx.is_num_like_nullable(&lt) || matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                None
            } else {
                self.access_member(Some(e), &lt.non_null(), binop_name(op), false)
                    .and_then(|m| match self.ctx.member_ty(&m) {
                        Ty::Fn { pos, .. } => pos.first().cloned(),
                        _ => None,
                    })
            };
        // `e == .x` (3.10): the shorthand on the right uses the static type of the left.
        let (r, rt) = if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            let left_ty = lt.clone();
            self.with_shorthand_context(right, &left_ty, |s| {
                s.emit_expr(right, r_expected.as_ref())
            })
        } else {
            self.emit_expr(right, r_expected.as_ref())
        };
        self.emit_binop_values(op, l, &lt, r, &rt, Some(e))
    }

    // -----------------------------------------------------------------------
    // Assignments
    // -----------------------------------------------------------------------

    fn emit_assign(&mut self, op: AssignOp, target: ExprId, value: ExprId, e: ExprId) -> (Js, Ty) {
        match op {
            AssignOp::Assign => {
                // A local's context is its declared type (`writeType`), not
                // the promoted one the emitter's scope may hold.
                let declared = matches!(
                    self.common_resolution(target),
                    Some(Resolved::Local(_) | Resolved::Parameter { .. })
                )
                .then(|| self.common_write_type(e))
                .flatten();
                let tty = declared.or_else(|| self.target_ty(target));
                let (v, vty) = self.emit_expr(value, tty.as_ref());
                // Update the type of an unannotated local with a known value type (promotion by assignment).
                let (js, _) = self.emit_assign_to(target, &v, &vty);
                (js, vty)
            }
            AssignOp::Compound(BinaryOp::IfNull) => {
                self.emit_compound(target, BinaryOp::IfNull, Some(value), None, true, e)
            }
            AssignOp::Compound(bop) => self.emit_compound(target, bop, Some(value), None, true, e),
        }
    }

    /// Static type of the assignment target (without emitting).
    pub fn target_ty(&mut self, target: ExprId) -> Option<Ty> {
        if let Some((owner, MemberRef::Function(f))) = self.ext_type_member(target) {
            let fe = self.ctx.program.function(f);
            let name = self.name(fe.name).trim_end_matches("_=").to_string();
            return match &self.expr(target).kind {
                ExprKind::Index { target: recv, .. } => {
                    self.ext_type_param(Some(*recv), owner, f, 1)
                }
                kind => {
                    let recv = match kind {
                        ExprKind::Property { target: recv, .. } => Some(*recv),
                        _ => None,
                    };
                    let setter = if fe.kind == FunctionKind::Setter {
                        f
                    } else {
                        self.ext_type_setter(owner, &name)?
                    };
                    self.ext_type_param(recv, owner, setter, 0)
                }
            };
        }
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(n) => match self.ident_target(n.sym, target) {
                IdentTarget::Local(_, ty) => Some(ty),
                IdentTarget::ThisMember(m) => {
                    let name = self.name(n.sym).to_string();
                    let m2 = self
                        .ctx
                        .lookup_member(&self.ctx.this_ty(m.class), &name, true)
                        .unwrap_or(m);
                    Some(self.ctx.member_ty(&m2))
                }
                IdentTarget::Static(_, mk) => match mk {
                    MemberKind::Field(v) => Some(self.ctx.var_ty(v)),
                    MemberKind::Setter(f) => self.ctx.outline.functions[f.0 as usize]
                        .parameters
                        .first()
                        .map(|p| self.ctx.ty_of(p.ty)),
                    _ => None,
                },
                IdentTarget::Element(Element::Variable(v)) => Some(self.ctx.var_ty(v)),
                IdentTarget::Element(Element::Function(f)) => self.setter_parameter_ty(f),
                _ => None,
            },
            ExprKind::Property {
                target: recv, name, ..
            } => {
                let n = self.name(name.sym).to_string();
                // `C.x = v`, `prefix.x = v`: a static or top-level setter or variable.
                if let ExprKind::Identifier(id) = &self.expr(*recv).kind {
                    match self.ident_target(id.sym, *recv) {
                        IdentTarget::Element(Element::Class(c)) => {
                            return match self.ctx.declared_static(c, &n, true) {
                                Some(MemberKind::Field(v)) => Some(self.ctx.var_ty(v)),
                                Some(MemberKind::Setter(f)) => self.setter_parameter_ty(f),
                                _ => None,
                            };
                        }
                        IdentTarget::Prefix(p) => {
                            let b = self
                                .ctx
                                .program
                                .lookup_prefixed_in_unit(self.unit, p, name.sym)?;
                            return match b.setter.or(b.getter)? {
                                Element::Variable(v) => Some(self.ctx.var_ty(v)),
                                Element::Function(f) => self.setter_parameter_ty(f),
                                _ => None,
                            };
                        }
                        _ => {}
                    }
                }
                let rty = self.type_of(*recv);
                if let Some(m) = self.access_member(Some(target), &rty.non_null(), &n, true) {
                    return Some(self.ctx.member_ty(&m));
                }
                // A setter of an applicable extension.
                self.extension_parameter_ty(&rty, &n, true, 0)
            }
            ExprKind::Index { target: recv, .. } => {
                let rty = self.type_of(*recv);
                match self.ctx.lookup_member(&rty.non_null(), "[]=", false) {
                    Some(m) => match self.ctx.member_ty(&m) {
                        Ty::Fn { pos, .. } => pos.get(1).cloned(),
                        _ => None,
                    },
                    None => self.extension_parameter_ty(&rty, "[]=", false, 1),
                }
            }
            _ => None,
        }
    }

    /// The iterable of a `for`-in over a `dynamic` value, cast to
    /// `Iterable<dynamic>` (`Stream<dynamic>` for `await for`) as the CFE does.
    pub fn checked_iterable(&self, js: Js, ty: Ty, stream: bool) -> (Js, Ty) {
        let class = if stream {
            self.ctx.stream_
        } else {
            self.ctx.iterable_
        };
        match class {
            Some(class)
                if ty.is_dynamic()
                    && !self.ctx.filter.is_some_and(|f| f.omit_implicit_checks()) =>
            {
                let target = Ty::iface_args(class, vec![Ty::Dynamic]);
                (self.as_cast(&js, &target), target)
            }
            _ => (js, ty),
        }
    }

    /// The loop variable's value of a `for`-in: `value` cast to the declared
    /// type when the elements are `dynamic`.
    pub fn checked_loop_value(&self, value: &str, element: &Ty, declared: &Ty) -> String {
        if element.is_dynamic()
            && !declared.is_dynamic()
            && !self.ctx.filter.is_some_and(|f| f.omit_implicit_checks())
        {
            self.as_cast(&Js::prim(value.to_string()), declared).code
        } else {
            value.to_string()
        }
    }

    /// The parameter type of the setter `f`.
    fn setter_parameter_ty(&self, f: dartforge_elements::model::FunctionElementId) -> Option<Ty> {
        if self.ctx.program.function(f).kind != FunctionKind::Setter {
            return None;
        }
        let p = self.ctx.outline.functions[f.0 as usize]
            .parameters
            .first()?;
        Some(self.ctx.ty_of(p.ty))
    }

    /// The type of positional parameter `i` of the member `name` of the
    /// extension applicable to `recv_ty`, instantiated for it.
    fn extension_parameter_ty(
        &self,
        recv_ty: &Ty,
        name: &str,
        setter: bool,
        i: usize,
    ) -> Option<Ty> {
        let (_, fid, subst) = self.find_extension_member(recv_ty, name, setter)?;
        let p = self.ctx.outline.functions[fid.0 as usize]
            .parameters
            .get(i)?;
        Some(self.ctx.ty_of(p.ty).subst(&subst))
    }

    /// Assigns `v` to the target. Returns the expression (whose value is `v`).
    pub fn emit_assign_to(&mut self, target: ExprId, v: &Js, vty: &Ty) -> (Js, Ty) {
        if let Some(r) = self.assign_to_extension_type(target, v, vty) {
            return r;
        }
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(n) => {
                let name = self.name(n.sym).to_string();
                match self.ident_target(n.sym, target) {
                    IdentTarget::Local(js, lty) => {
                        if let Some(l) = self.lookup_local(n.sym).cloned() {
                            if l.late_final {
                                self.m.use_sdk("_internal");
                                return (Js::new(format!("{js} === void 0 ? {js} = {} : dart.throw(new _internal.LateError.localAI({}))", v.at(P_ASSIGN), js::string_literal(self.ctx.late_name(&name))), P_COND).paren(), lty);
                            }
                        }
                        // Promotion by assignment: a `dynamic`/nullable local takes the value's type when compatible.
                        if lty.is_nullable()
                            && !lty.is_dynamic()
                            && self.ctx.is_subtype(vty, &lty)
                            && !vty.is_dynamic()
                            && !matches!(vty, Ty::Null)
                        {
                            self.set_local_ty(n.sym, vty.clone());
                        } else if matches!(vty, Ty::Null) && lty.is_nullable() {
                            // keep it
                        }
                        (Js::new(format!("{js} = {}", v.at(P_ASSIGN)), P_ASSIGN), lty)
                    }
                    IdentTarget::ThisMember(m) => {
                        let this_ty = self.ctx.this_ty(m.class);
                        let m2 = self.ctx.lookup_member(&this_ty, &name, true).unwrap_or(m);
                        let access = self.member_access(&this_ty, &name, true);
                        let v = if self.ctx.is_js_member_kind(&m2.kind) {
                            self.assert_interop(v.clone(), vty)
                        } else {
                            v.clone()
                        };
                        (
                            Js::new(format!("this{access} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                            self.ctx.member_ty(&m2),
                        )
                    }
                    IdentTarget::ThisExt => {
                        let t = self
                            .class
                            .map(|c| self.ctx.this_ty(c))
                            .unwrap_or(Ty::Dynamic);
                        if let Some((ext, fid, subst)) = self.find_extension_member(&t, &name, true)
                        {
                            return (
                                self.extension_set(ext, fid, &subst, &Js::prim("this"), v, vty),
                                vty.clone(),
                            );
                        }
                        (Js::prim("null"), Ty::Dynamic)
                    }
                    IdentTarget::ExtField(ext) => {
                        let e = self.ctx.program.extension(ext);
                        let lib_var = self.lib_var(e.library);
                        let ext_name = self.extension_js_name(ext);
                        (
                            Js::new(
                                format!(
                                    "{lib_var}[{}] = {}",
                                    js::string_literal(&format!("{ext_name}|{name}")),
                                    v.at(P_ASSIGN)
                                ),
                                P_ASSIGN,
                            ),
                            vty.clone(),
                        )
                    }
                    tgt @ (IdentTarget::ExtThisMember(_)
                    | IdentTarget::ExtMember(..)
                    | IdentTarget::ExtStatic(..)) => {
                        let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                        // A static setter of the extension itself.
                        if let IdentTarget::ExtStatic(ext, _) = tgt {
                            let e = self.ctx.program.extension(ext);
                            if let Some(fid) = self
                                .ctx
                                .sym(&format!("{name}_="))
                                .and_then(|k| e.static_members.get(&k).copied())
                            {
                                let lib_var = self.lib_var(e.library);
                                let ext_name = self.extension_js_name(ext);
                                let result = self.temp();
                                let pty = self.ctx.outline.functions[fid.0 as usize]
                                    .parameters
                                    .first()
                                    .map(|p| self.ctx.ty_of(p.ty))
                                    .unwrap_or(Ty::Dynamic);
                                return (
                                    Js::prim(format!(
                                        "({lib_var}[{}]({result} = {}), {result})",
                                        js::string_literal(&format!("{ext_name}|set#{name}")),
                                        v.at(P_ASSIGN)
                                    )),
                                    pty,
                                );
                            }
                        }
                        // The extension's own setter is lexical.
                        if let IdentTarget::ExtMember(ext, _) = tgt {
                            self.forced_ext = Some(ext);
                            let found = self.find_extension_member(&t, &name, true);
                            self.forced_ext = None;
                            if let Some((ext, fid, subst)) = found {
                                let recv = Js::prim("$this");
                                return (
                                    self.extension_set(ext, fid, &subst, &recv, v, vty),
                                    vty.clone(),
                                );
                            }
                        }
                        // A setter of the `on` type wins over another
                        // extension's.
                        let own = matches!(tgt, IdentTarget::ExtThisMember(_))
                            && self.ctx.lookup_member(&t, &name, true).is_some();
                        if !own
                            && let Some((ext, fid, subst)) =
                                self.find_extension_member(&t, &name, true)
                        {
                            return (
                                self.extension_set(ext, fid, &subst, &Js::prim("$this"), v, vty),
                                vty.clone(),
                            );
                        }
                        let access = self.member_access(&t, &name, true);
                        (
                            Js::new(format!("$this{access} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                            Ty::Dynamic,
                        )
                    }
                    IdentTarget::Static(c, mk) => {
                        if self.ctx.is_js_class(c) {
                            let smk = self.ctx.declared_static(c, &name, true).unwrap_or(mk);
                            if self.ctx.is_js_member_kind(&smk) {
                                let target_js = self.ctx.js_static_ref(
                                    Some(c),
                                    self.ctx.lib_of_class(c),
                                    &smk,
                                    &name,
                                );
                                let v = self.assert_interop(v.clone(), vty);
                                return (
                                    Js::new(format!("{target_js} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                                    Ty::Dynamic,
                                );
                            }
                        }
                        let cls = self.class_ref(c);
                        (
                            Js::new(
                                format!(
                                    "{cls}{} = {}",
                                    js::prop_access(&static_member_name(&name)),
                                    v.at(P_ASSIGN)
                                ),
                                P_ASSIGN,
                            ),
                            Ty::Dynamic,
                        )
                    }
                    IdentTarget::Element(el) => {
                        let mut v = v.clone();
                        let js = match el {
                            Element::Variable(vid) if self.ctx.is_js_var(vid) => {
                                let var = self.ctx.program.variable(vid);
                                v = self.assert_interop(v, vty);
                                self.ctx.js_static_ref(
                                    None,
                                    var.library,
                                    &MemberKind::Field(vid),
                                    &name,
                                )
                            }
                            Element::Variable(vid) => {
                                let var = self.ctx.program.variable(vid);
                                format!("{}{}", self.lib_var(var.library), js::prop_access(&name))
                            }
                            Element::Function(fid) if self.ctx.is_js_member(fid) => {
                                let f = self.ctx.program.function(fid);
                                v = self.assert_interop(v, vty);
                                self.ctx.js_static_ref(
                                    None,
                                    f.library,
                                    &MemberKind::Setter(fid),
                                    &name,
                                )
                            }
                            Element::Function(fid) => {
                                let f = self.ctx.program.function(fid);
                                format!("{}{}", self.lib_var(f.library), js::prop_access(&name))
                            }
                            _ => js::ident(&name),
                        };
                        (
                            Js::new(format!("{js} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                            Ty::Dynamic,
                        )
                    }
                    _ => (
                        Js::new(
                            format!("{} = {}", js::ident(&name), v.at(P_ASSIGN)),
                            P_ASSIGN,
                        ),
                        Ty::Dynamic,
                    ),
                }
            }
            ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                let n = self.name(name.sym).to_string();
                // `Ext.x = v`: a static field or setter of the extension.
                if let ExprKind::Identifier(id) = &self.expr(*recv).kind
                    && let IdentTarget::Element(Element::Extension(ext)) =
                        self.ident_target(id.sym, *recv)
                {
                    let e = self.ctx.program.extension(ext);
                    let lib_var = self.lib_var(e.library);
                    let ext_name = self.extension_js_name(ext);
                    if let Some(&vid) = e
                        .fields
                        .iter()
                        .find(|v| self.ctx.program.variable(**v).name == name.sym)
                    {
                        return (
                            Js::new(
                                format!(
                                    "{lib_var}[{}] = {}",
                                    js::string_literal(&format!("{ext_name}|{n}")),
                                    v.at(P_ASSIGN)
                                ),
                                P_ASSIGN,
                            ),
                            self.ctx.var_ty(vid),
                        );
                    }
                    let result = self.temp();
                    return (
                        Js::prim(format!(
                            "({lib_var}[{}]({result} = {}), {result})",
                            js::string_literal(&format!("{ext_name}|set#{n}")),
                            v.at(P_ASSIGN)
                        )),
                        vty.clone(),
                    );
                }
                // Static through a class/prefix.
                if let Some((target_js, interop)) = self.try_static_lvalue(*recv, &n) {
                    let v = if interop {
                        self.assert_interop(v.clone(), vty)
                    } else {
                        v.clone()
                    };
                    return (
                        Js::new(format!("{target_js} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                        Ty::Dynamic,
                    );
                }
                if let ExprKind::Super = self.expr(*recv).kind {
                    let sup_ty = self.super_ty_for(&n, true);
                    let access = self.member_access(&sup_ty, &n, true);
                    if self.super_field_storage(&sup_ty, &n, true) {
                        return (
                            Js::new(format!("this{access} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                            Ty::Dynamic,
                        );
                    }
                    if self.super_by_prototype() {
                        // The setter of the superclass with `this` as the
                        // receiver; the expression's value is the value.
                        let t = self.temp();
                        return (
                            Js::new(
                                format!(
                                    "({t} = {}, dart.global.Reflect.set({}, {}, {t}, this), {t})",
                                    v.at(P_ASSIGN),
                                    self.super_ref(),
                                    access_key(&access)
                                ),
                                P_ASSIGN,
                            ),
                            Ty::Dynamic,
                        );
                    }
                    return (
                        Js::new(format!("super{access} = {}", v.at(P_ASSIGN)), P_ASSIGN),
                        Ty::Dynamic,
                    );
                }
                let (rjs, rty, mut guards) = self.emit_target(*recv);
                let (recv_js, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: rjs.code,
                    });
                    (Js::prim(t), rty.non_null())
                } else {
                    (rjs, rty)
                };
                // `Ext(x).b = v`: the explicit extension's setter.
                if self.forced_ext.is_some() {
                    let found = self.find_extension_member(&recv_ty, &n, true);
                    self.forced_ext = None;
                    if let Some((ext, fid, subst)) = found {
                        let js = self.extension_set(ext, fid, &subst, &recv_js, v, vty);
                        return (self.wrap_guards(js, guards), vty.clone());
                    }
                }
                let setter = self.access_member(Some(target), &recv_ty, &n, true);
                let js = if recv_ty.is_dynamic() || setter.is_none() {
                    if let Some((ext, fid, subst)) = self.find_extension_member(&recv_ty, &n, true)
                    {
                        self.extension_set(ext, fid, &subst, &recv_js, v, vty)
                    } else if let Ty::Record { .. } = recv_ty {
                        Js::new(
                            format!(
                                "{}{} = {}",
                                recv_js.at(P_PRIMARY),
                                js::prop_access(&n),
                                v.at(P_ASSIGN)
                            ),
                            P_ASSIGN,
                        )
                    } else {
                        Js::prim(format!(
                            "dart.dput({}, {}, {})",
                            recv_js.code,
                            self.dynamic_member_key(&n),
                            v.code
                        ))
                    }
                } else {
                    let access = self.member_access(&recv_ty, &n, true);
                    let interop = setter.is_some_and(|m| self.ctx.is_js_member_kind(&m.kind));
                    let v = if interop {
                        self.assert_interop(v.clone(), vty)
                    } else {
                        v.clone()
                    };
                    Js::new(
                        format!("{}{access} = {}", recv_js.at(P_PRIMARY), v.at(P_ASSIGN)),
                        P_ASSIGN,
                    )
                };
                (self.wrap_guards(js, guards), vty.clone())
            }
            ExprKind::Index {
                target: recv,
                index,
                null_aware,
            } => {
                let (rjs, rty, mut guards) = self.emit_target(*recv);
                let (recv_js, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: rjs.code,
                    });
                    (Js::prim(t), rty.non_null())
                } else {
                    (rjs, rty)
                };
                let forced = self.forced_ext.take();
                let (ijs, _) = self.emit_expr(*index, None);
                self.forced_ext = forced;
                let js = self.emit_index_set(&recv_js, &recv_ty, &ijs, v);
                (self.wrap_guards(js, guards), vty.clone())
            }
            ExprKind::Parenthesized(inner) => self.emit_assign_to(*inner, v, vty),
            ExprKind::CascadeTarget => (Js::prim("null"), Ty::Dynamic),
            _ => {
                let (js, _) = self.emit_expr(target, None);
                (
                    Js::new(format!("{} = {}", js.code, v.at(P_ASSIGN)), P_ASSIGN),
                    Ty::Dynamic,
                )
            }
        }
    }

    /// Write to an instance member of an extension type (`x.s = v`, `s = v`
    /// in the body, `x[i] = v`), when the common resolution of the target
    /// points to one.
    fn assign_to_extension_type(&mut self, target: ExprId, v: &Js, vty: &Ty) -> Option<(Js, Ty)> {
        let (owner, MemberRef::Function(f)) = self.ext_type_member(target)? else {
            return None;
        };
        let fe = self.ctx.program.function(f);
        let name = self.name(fe.name).trim_end_matches("_=").to_string();
        match &self.expr(target).kind {
            ExprKind::Identifier(_) => {
                let setter = if fe.kind == FunctionKind::Setter {
                    f
                } else {
                    self.ext_type_setter(owner, &name)?
                };
                Some(self.ext_type_write(&Js::prim("$this"), None, owner, setter, v, vty))
            }
            ExprKind::Property {
                target: recv,
                null_aware,
                ..
            } => {
                let setter = if fe.kind == FunctionKind::Setter {
                    f
                } else {
                    self.ext_type_setter(owner, &name)?
                };
                let (rjs, _, mut guards) = self.emit_target(*recv);
                let recv_js = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: rjs.code,
                    });
                    Js::prim(t)
                } else {
                    rjs
                };
                let (js, ty) = self.ext_type_write(&recv_js, Some(*recv), owner, setter, v, vty);
                Some((self.wrap_guards(js, guards), ty))
            }
            ExprKind::Index {
                target: recv,
                index,
                null_aware,
            } if name == "[]=" => {
                let (rjs, _, mut guards) = self.emit_target(*recv);
                let recv_js = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard {
                        temp: t.clone(),
                        init: rjs.code,
                    });
                    Js::prim(t)
                } else {
                    rjs
                };
                let ctx_i = self.ext_type_param(Some(*recv), owner, f, 0);
                let (ijs, _) = self.emit_expr(*index, ctx_i.as_ref());
                let t = self.temp();
                let assign = Js::new(format!("{t} = {}", v.at(P_ASSIGN)), P_ASSIGN);
                let (call, _) =
                    self.ext_type_operator(&recv_js, Some(*recv), owner, f, &[ijs, assign]);
                let js = Js::new(format!("({}, {t})", call.code), P_PRIMARY);
                Some((self.wrap_guards(js, guards), vty.clone()))
            }
            _ => None,
        }
    }

    fn try_static_lvalue(&mut self, recv: ExprId, name: &str) -> Option<(String, bool)> {
        let t = self.expr(recv);
        match &t.kind {
            ExprKind::Identifier(id) => match self.ident_target(id.sym, recv) {
                IdentTarget::Prefix(p) => {
                    let sym = self.ctx.sym(name)?;
                    let b = self
                        .ctx
                        .program
                        .lookup_prefixed_in_unit(self.unit, p, sym)?;
                    match b.setter.or(b.getter)? {
                        Element::Variable(vid) if self.ctx.is_js_var(vid) => {
                            let var = self.ctx.program.variable(vid);
                            Some((
                                self.ctx.js_static_ref(
                                    None,
                                    var.library,
                                    &MemberKind::Field(vid),
                                    name,
                                ),
                                true,
                            ))
                        }
                        Element::Variable(vid) => {
                            let var = self.ctx.program.variable(vid);
                            Some((
                                format!("{}{}", self.lib_var(var.library), js::prop_access(name)),
                                false,
                            ))
                        }
                        Element::Function(fid) if self.ctx.is_js_member(fid) => {
                            let f = self.ctx.program.function(fid);
                            Some((
                                self.ctx.js_static_ref(
                                    None,
                                    f.library,
                                    &MemberKind::Setter(fid),
                                    name,
                                ),
                                true,
                            ))
                        }
                        Element::Function(fid) => {
                            let f = self.ctx.program.function(fid);
                            Some((
                                format!("{}{}", self.lib_var(f.library), js::prop_access(name)),
                                false,
                            ))
                        }
                        _ => None,
                    }
                }
                IdentTarget::Element(Element::Class(c)) => self.class_static_lvalue(c, name),
                // `Alias.x = v`: the static of the class the alias denotes.
                IdentTarget::Element(Element::Typedef(td)) => {
                    let (c, _, _) = self.alias_creation(td)?;
                    self.class_static_lvalue(c, name)
                }
                _ => None,
            },
            // `p.C.x` with `p` an import prefix: the static member of `C`.
            ExprKind::Property {
                target: inner,
                name: class_name,
                ..
            } => {
                let ExprKind::Identifier(id) = &self.expr(*inner).kind else {
                    return None;
                };
                let IdentTarget::Prefix(p) = self.ident_target(id.sym, *inner) else {
                    return None;
                };
                let b = self
                    .ctx
                    .program
                    .lookup_prefixed_in_unit(self.unit, p, class_name.sym)?;
                match b.getter? {
                    Element::Class(c) => self.class_static_lvalue(c, name),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The static field or setter `name` of the class `c` (or of a superclass),
    /// as an lvalue.
    fn class_static_lvalue(&mut self, c: ClassId, name: &str) -> Option<(String, bool)> {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if let Some(mk) = self
                .ctx
                .declared_static(k, name, true)
                .or_else(|| self.ctx.declared_static(k, name, false))
            {
                if self.ctx.is_js_class(k) && self.ctx.is_js_member_kind(&mk) {
                    return Some((
                        self.ctx
                            .js_static_ref(Some(k), self.ctx.lib_of_class(k), &mk, name),
                        true,
                    ));
                }
                return Some((
                    format!(
                        "{}{}",
                        self.class_ref(k),
                        js::prop_access(&static_member_name(name))
                    ),
                    false,
                ));
            }
            cur = self.ctx.superclass_of(k);
        }
        None
    }

    /// `recv[i] = v`; the expression evaluates to `v`.
    pub fn emit_index_set(&mut self, recv: &Js, recv_ty: &Ty, idx: &Js, v: &Js) -> Js {
        // An identifier is cheap to read, but its binding can change while the
        // index or RHS runs. Snapshot both operands before evaluating the RHS.
        let mut parts = Vec::new();
        let recv_js = if matches!(recv.code.as_str(), "this" | "super") {
            recv.code.clone()
        } else {
            let t = self.temp();
            parts.push(format!("{t} = {}", recv.code));
            t
        };
        let index = self.temp();
        parts.push(format!("{index} = {}", idx.code));
        let value = self.temp();
        parts.push(format!("{value} = {}", v.code));
        let call = if recv_ty.is_dynamic() {
            format!("dart.dsend({recv_js}, '_set', [{index}, {value}])")
        } else if self.forced_ext.is_none()
            && self.ctx.lookup_member(recv_ty, "[]=", false).is_some()
        {
            let access = self.member_access(recv_ty, "[]=", false);
            format!("{recv_js}{access}({index}, {value})")
        } else if let Some((ext, _, subst)) = {
            let found = self.find_extension_member(recv_ty, "[]=", false);
            self.forced_ext = None;
            found
        } {
            let e = self.ctx.program.extension(ext);
            let lib_var = self.lib_var(e.library);
            let ext_name = self.extension_js_name(ext);
            let mut args = self.ext_type_args(ext, &subst);
            args.extend([recv_js, index, value.clone()]);
            format!(
                "{lib_var}[{}]({})",
                js::string_literal(&format!("{ext_name}|[]=")),
                args.join(", ")
            )
        } else {
            format!("dart.dsend({recv_js}, '_set', [{index}, {value}])")
        };
        parts.push(call);
        // Assignment expressions yield the RHS, regardless of setter return.
        parts.push(value);
        Js::new(parts.join(", "), P_COMMA).paren()
    }

    /// `x op= v`, `++x`, `x++`.
    /// `node` is the compound assignment or the `++`/`--`: the common
    /// resolution of the operator is recorded on it.
    fn emit_compound(
        &mut self,
        target: ExprId,
        bop: BinaryOp,
        value: Option<ExprId>,
        literal: Option<i64>,
        prefix: bool,
        node: ExprId,
    ) -> (Js, Ty) {
        // `a?.b op= v`, `a?.b[i]++`: the null-aware receivers short-circuit
        // the whole read-modify-write.
        let saved = std::mem::take(&mut self.compound_guards);
        let (js, ty) = self.emit_compound_inner(target, bop, value, literal, prefix, node);
        let guards = std::mem::replace(&mut self.compound_guards, saved);
        if guards.is_empty() {
            return (js, ty);
        }
        (self.wrap_guards(js, guards), ty.with_nullable(true))
    }

    fn emit_compound_inner(
        &mut self,
        target: ExprId,
        bop: BinaryOp,
        value: Option<ExprId>,
        literal: Option<i64>,
        prefix: bool,
        node: ExprId,
    ) -> (Js, Ty) {
        // Read the target once, with temps for receivers/indices.
        let t = self.expr(target);
        let (read, read_ty, write): (Js, Ty, AssignmentWriter<'m, 'a>) = match &t.kind {
            ExprKind::Identifier(_) | ExprKind::Parenthesized(_) => {
                let (r, rt) = self.emit_expr(target, None);
                let tgt = target;
                (
                    r,
                    rt,
                    Box::new(move |s: &mut Self, v: &Js| s.emit_assign_to(tgt, v, &Ty::Dynamic).0),
                )
            }
            ExprKind::Property {
                target: recv, name, ..
            } if self.ext_type_member(target).is_some() => {
                // Extension type getter and setter: the receiver is evaluated once.
                let Some((owner, member)) = self.ext_type_member(target) else {
                    unreachable!()
                };
                let n = self.name(name.sym).to_string();
                let recv_e = *recv;
                let (rjs, _, _guards) = self.emit_target(recv_e);
                let recv_js = if matches!(rjs.code.as_str(), "this" | "super") {
                    rjs.code.clone()
                } else {
                    let tmp = self.temp();
                    self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                    tmp
                };
                let (r, rt) =
                    self.ext_type_read(&Js::prim(recv_js.clone()), Some(recv_e), owner, member);
                let setter = self.ext_type_setter(owner, &n);
                (
                    r,
                    rt,
                    Box::new(move |s: &mut Self, v: &Js| match setter {
                        Some(f) => {
                            s.ext_type_write(
                                &Js::prim(recv_js.clone()),
                                Some(recv_e),
                                owner,
                                f,
                                v,
                                &Ty::Dynamic,
                            )
                            .0
                        }
                        None => v.clone(),
                    }),
                )
            }
            ExprKind::Index {
                target: recv,
                index,
                ..
            } if self.ext_type_member(target).is_some() => {
                // `x[i] op= v` with an extension type's `[]`/`[]=`.
                let Some((owner, _)) = self.ext_type_member(target) else {
                    unreachable!()
                };
                let recv_e = *recv;
                let (rjs, _, _guards) = self.emit_target(recv_e);
                let recv_js = if matches!(rjs.code.as_str(), "this" | "super") {
                    rjs.code.clone()
                } else {
                    let tmp = self.temp();
                    self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                    tmp
                };
                let read_op = self.ext_type_operator_fn(owner, "[]");
                let write_op = self.ext_type_operator_fn(owner, "[]=");
                let ctx_i = read_op.and_then(|f| self.ext_type_param(Some(recv_e), owner, f, 0));
                let (ijs, _) = self.emit_expr(*index, ctx_i.as_ref());
                let idx_js = self.temp();
                self.pending_prefix.push(format!("{idx_js} = {}", ijs.code));
                let (r, rt) = match read_op {
                    Some(f) => self.ext_type_operator(
                        &Js::prim(recv_js.clone()),
                        Some(recv_e),
                        owner,
                        f,
                        &[Js::prim(idx_js.clone())],
                    ),
                    None => (Js::prim("null"), Ty::Dynamic),
                };
                (
                    r,
                    rt,
                    Box::new(move |s: &mut Self, v: &Js| match write_op {
                        Some(f) => {
                            let t = s.temp();
                            let assign = Js::new(format!("{t} = {}", v.at(P_ASSIGN)), P_ASSIGN);
                            let (call, _) = s.ext_type_operator(
                                &Js::prim(recv_js.clone()),
                                Some(recv_e),
                                owner,
                                f,
                                &[Js::prim(idx_js.clone()), assign],
                            );
                            Js::new(format!("({}, {t})", call.code), P_PRIMARY)
                        }
                        None => v.clone(),
                    }),
                )
            }
            ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                let n = self.name(name.sym).to_string();
                if let Some((sjs, _)) = self.try_static_lvalue(*recv, &n) {
                    let rt = self.type_of(target);
                    let sjs2 = sjs.clone();
                    (
                        Js::prim(sjs),
                        rt,
                        Box::new(move |_s: &mut Self, v: &Js| {
                            Js::new(format!("{sjs2} = {}", v.at(P_ASSIGN)), P_ASSIGN)
                        }),
                    )
                } else if let ExprKind::Super = self.expr(*recv).kind {
                    let sup_ty = self.super_ty_for(&n, false);
                    let m = self.ctx.lookup_member(&sup_ty, &n, false);
                    let rt = m.map(|m| self.ctx.member_ty(&m)).unwrap_or(Ty::Dynamic);
                    let get = self.member_access(&sup_ty, &n, false);
                    let set = self.member_access(&sup_ty, &n, true);
                    let storage = self.super_field_storage(&sup_ty, &n, false);
                    let by_prototype = self.super_by_prototype();
                    let sr = self.super_ref();
                    let read = if storage {
                        format!("this{get}")
                    } else {
                        self.super_read(&get)
                    };
                    (
                        Js::prim(read),
                        rt,
                        Box::new(move |s: &mut Self, v: &Js| {
                            if storage {
                                return Js::new(
                                    format!("this{set} = {}", v.at(P_ASSIGN)),
                                    P_ASSIGN,
                                );
                            }
                            if by_prototype {
                                let t = s.temp();
                                return Js::new(
                                    format!(
                                        "({t} = {}, dart.global.Reflect.set({sr}, {}, {t}, this), {t})",
                                        v.at(P_ASSIGN),
                                        access_key(&set)
                                    ),
                                    P_ASSIGN,
                                );
                            }
                            Js::new(format!("super{set} = {}", v.at(P_ASSIGN)), P_ASSIGN)
                        }),
                    )
                } else {
                    let (rjs, rty, guards) = self.emit_target(*recv);
                    self.compound_guards.extend(guards);
                    let rty = if *null_aware { rty.non_null() } else { rty };
                    let recv_js = if matches!(rjs.code.as_str(), "this" | "super") {
                        rjs.code.clone()
                    } else if *null_aware {
                        let tmp = self.temp();
                        self.compound_guards.push(Guard {
                            temp: tmp.clone(),
                            init: rjs.code,
                        });
                        tmp
                    } else {
                        let tmp = self.temp();
                        // The temp's initialization prefix goes with the read.
                        self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                        tmp
                    };
                    // `Ext(x).b op= v`: the read and the write go through the
                    // explicit extension.
                    let forced = self.forced_ext;
                    // The common resolution of the target of `a.b op= v` is the
                    // setter's (one per node): the read finds the getter through
                    // the emitter's own lookup, on the same receiver type.
                    let (r, rt) = self.emit_member_get(&Js::prim(recv_js.clone()), &rty, &n, None);
                    let recv_ty2 = rty.clone();
                    let n2 = n.clone();
                    (
                        r,
                        rt,
                        Box::new(move |s: &mut Self, v: &Js| {
                            if forced.is_some() {
                                s.forced_ext = forced;
                                let found = s.find_extension_member(&recv_ty2, &n2, true);
                                s.forced_ext = None;
                                if let Some((ext, fid, subst)) = found {
                                    return s.extension_set(
                                        ext,
                                        fid,
                                        &subst,
                                        &Js::prim(recv_js.clone()),
                                        v,
                                        &Ty::Dynamic,
                                    );
                                }
                            }
                            if recv_ty2.is_dynamic()
                                || s.access_member(Some(target), &recv_ty2, &n2, true)
                                    .is_none()
                            {
                                // An extension's setter (the read already went
                                // through its getter, `emit_member_get`), as in a
                                // simple assignment.
                                if let Some((ext, fid, subst)) =
                                    s.find_extension_member(&recv_ty2, &n2, true)
                                {
                                    return s.extension_set(
                                        ext,
                                        fid,
                                        &subst,
                                        &Js::prim(recv_js.clone()),
                                        v,
                                        &Ty::Dynamic,
                                    );
                                }
                                Js::prim(format!(
                                    "dart.dput({recv_js}, {}, {})",
                                    s.dynamic_member_key(&n2),
                                    v.code
                                ))
                            } else {
                                let access = s.member_access(&recv_ty2, &n2, true);
                                Js::new(format!("{recv_js}{access} = {}", v.at(P_ASSIGN)), P_ASSIGN)
                            }
                        }),
                    )
                }
            }
            ExprKind::Index {
                target: recv,
                index,
                null_aware,
            } => {
                let (rjs, rty, guards) = self.emit_target(*recv);
                self.compound_guards.extend(guards);
                let rty = if *null_aware { rty.non_null() } else { rty };
                let recv_js = if matches!(rjs.code.as_str(), "this" | "super") {
                    rjs.code.clone()
                } else if *null_aware {
                    let tmp = self.temp();
                    self.compound_guards.push(Guard {
                        temp: tmp.clone(),
                        init: rjs.code,
                    });
                    tmp
                } else {
                    let tmp = self.temp();
                    self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                    tmp
                };
                let forced = self.forced_ext.take();
                let (ijs, _) = self.emit_expr(*index, None);
                let idx_js = self.temp();
                self.pending_prefix.push(format!("{idx_js} = {}", ijs.code));
                self.forced_ext = forced;
                let (r, rt) = self.emit_index_get(
                    &Js::prim(recv_js.clone()),
                    &rty,
                    &Js::prim(idx_js.clone()),
                    Some(target),
                );
                let rty2 = rty.clone();
                (
                    r,
                    rt,
                    Box::new(move |s: &mut Self, v: &Js| {
                        s.forced_ext = forced;
                        s.emit_index_set(
                            &Js::prim(recv_js.clone()),
                            &rty2,
                            &Js::prim(idx_js.clone()),
                            v,
                        )
                    }),
                )
            }
            _ => {
                let (r, rt) = self.emit_expr(target, None);
                (r, rt, Box::new(|_s: &mut Self, v: &Js| v.clone()))
            }
        };
        let prefix_parts: Vec<String> = std::mem::take(&mut self.pending_prefix);
        if bop == BinaryOp::IfNull {
            let (v, vty) = match value {
                Some(e) => self.emit_expr(e, Some(&read_ty.non_null())),
                None => (Js::prim("null"), Ty::Null),
            };
            let t = self.temp();
            let assign = write(self, &v);
            let ty = self.ctx.lub(&read_ty.non_null(), &vty);
            let mut parts = prefix_parts;
            parts.push(format!("{t} = {}", read.code));
            parts.push(format!("{t} == null ? {} : {t}", assign.at(P_COND)));
            return (Js::new(parts.join(", "), P_COMMA).paren(), ty);
        }
        let (v, vty) = match (value, literal) {
            (Some(e), _) => self.emit_expr(e, None),
            (None, Some(n)) => (Js::prim(n.to_string()), self.ctx.t_int()),
            _ => (Js::prim("1"), self.ctx.t_int()),
        };
        if prefix {
            let (res, rty) = self.emit_binop_values(bop, read, &read_ty, v, &vty, Some(node));
            let assign = write(self, &res);
            let code = if prefix_parts.is_empty() {
                assign.code
            } else {
                format!("{}, {}", prefix_parts.join(", "), assign.code)
            };
            let ty = if literal.is_some() { read_ty } else { rty };
            (Js::new(code, P_COMMA).paren(), ty)
        } else {
            // Postfix increment: the old value.
            let old = self.temp();
            let (res, _) =
                self.emit_binop_values(bop, Js::prim(old.clone()), &read_ty, v, &vty, Some(node));
            let assign = write(self, &res);
            let mut parts = prefix_parts;
            parts.push(format!("{old} = {}", read.code));
            parts.push(assign.code);
            parts.push(old);
            (Js::new(parts.join(", "), P_COMMA).paren(), read_ty)
        }
    }

    // -----------------------------------------------------------------------
    // Switch expression
    // -----------------------------------------------------------------------

    fn emit_switch_expr(
        &mut self,
        value: ExprId,
        cases: &[ast::SwitchExprCase],
        expected: Option<&Ty>,
    ) -> (Js, Ty) {
        let (vjs, vty) = self.emit_expr(value, None);
        let saved_w = std::mem::take(&mut self.w);
        let t = self.temp();
        crate::emit_line!(self.w, "{t} = {};", vjs.code);
        let mut result_ty: Option<Ty> = None;
        for c in cases {
            self.w.open("{");
            self.push_scope();
            let mut binds = Vec::new();
            let cond = self.pattern_cond(c.pattern, &t, &vty, &mut binds, false);
            for (_, _, jsn) in &binds {
                crate::emit_line!(self.w, "let {jsn} = null;");
            }
            let mut full = cond;
            if let Some(g) = c.guard {
                let (gjs, _) = self.emit_expr(g, Some(&self.ctx.t_bool()));
                full = format!("({full}) && {}", gjs.at(P_AND));
            }
            let (bjs, bty) = self.emit_expr(c.body, expected);
            result_ty = Some(match result_ty {
                None => bty,
                Some(r) => self.ctx.lub(&r, &bty),
            });
            crate::emit_line!(self.w, "if ({full}) return {};", bjs.code);
            self.pop_scope();
            self.w.close("}");
        }
        self.m.use_sdk("core");
        self.w
            .line("dart.throw(new core.StateError.new(\"Pattern matching error\"));");
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), result_ty.unwrap_or(Ty::Dynamic))
    }

    // -----------------------------------------------------------------------
    // Instantiation
    // -----------------------------------------------------------------------

    fn emit_instance_creation(
        &mut self,
        keyword: Option<ast::CreationKeyword>,
        ty: ast::TypeId,
        constructor: Option<&ast::Name>,
        arguments: &ast::Arguments,
        expected: Option<&Ty>,
    ) -> (Js, Ty) {
        let mut t = self.resolve_type(ty);
        let is_const =
            keyword == Some(ast::CreationKeyword::Const) || (keyword.is_none() && self.in_const);
        let mut ctor_name = constructor
            .map(|n| self.name(n.sym).to_string())
            .unwrap_or_default();
        // `const C.name(...)`: the parser may read `C.name` as a prefixed type.
        if !matches!(t, Ty::Iface { .. }) && constructor.is_none() {
            if let ast::TypeKind::Named { name: parts, .. } = &self.ast().ty(ty).kind {
                if parts.len() == 2 {
                    if let Some(Element::Class(c)) = self
                        .ctx
                        .program
                        .lookup_in_unit(self.unit, parts[0].sym)
                        .and_then(|b| b.getter)
                    {
                        t = self.ctx.this_ty_default(c);
                        ctor_name = self.name(parts[1].sym).to_string();
                    }
                }
            }
        }
        let Ty::Iface { class, args, .. } = &t else {
            return (Js::prim("null"), Ty::Dynamic);
        };
        let explicit_args = !self.explicit_type_args(ty).is_empty();
        let class_args = if explicit_args { args.clone() } else { vec![] };
        let saved = self.in_const;
        if is_const {
            self.in_const = true;
        }
        let r = self.emit_constructor_call(
            *class,
            class_args,
            explicit_args,
            &ctor_name,
            arguments,
            expected,
            is_const,
        );
        self.in_const = saved;
        r
    }

    fn explicit_type_args(&self, t: ast::TypeId) -> Vec<ast::TypeId> {
        match &self.ast().ty(t).kind {
            ast::TypeKind::Named { args, .. } => args.to_vec(),
            _ => vec![],
        }
    }

    /// The type of the constructor `name` that the named mixin application
    /// `class` forwards to its superclass (`class C<T> = B<T> with M;`), in
    /// terms of the parameters of `class`, so that the arguments are checked
    /// against the superclass constructor's parameters like in the CFE.
    fn forwarded_constructor_ty(
        &self,
        class: ClassId,
        name: dartforge_intern::SymbolId,
    ) -> Option<Ty> {
        let params = &self.ctx.class_params[class.0 as usize];
        let mut current = Ty::Iface {
            class,
            args: params
                .iter()
                .map(|p| Ty::Param {
                    id: p.id,
                    name: p.name.clone(),
                    nullable: false,
                })
                .collect(),
            nullable: false,
        };
        let own = current.clone();
        for _ in 0..64 {
            let Ty::Iface { class: c, args, .. } = &current else {
                return None;
            };
            let cls = self.ctx.program.class(*c);
            if let Some(&f) = cls.constructors.get(&name) {
                let Ty::Fn {
                    ret,
                    pos,
                    opt,
                    named,
                    nullable,
                    ..
                } = self.ctx.fn_ty(f)
                else {
                    return None;
                };
                let map: HashMap<u32, Ty> = self.ctx.class_params[c.0 as usize]
                    .iter()
                    .zip(args.iter())
                    .map(|(p, a)| (p.id, a.clone()))
                    .collect();
                let inner = Ty::Fn {
                    type_params: vec![],
                    ret,
                    pos,
                    opt,
                    named,
                    nullable,
                }
                .subst(&map);
                let Ty::Fn {
                    pos,
                    opt,
                    named,
                    nullable,
                    ..
                } = inner
                else {
                    return None;
                };
                return Some(Ty::Fn {
                    type_params: params.clone(),
                    ret: Box::new(own),
                    pos,
                    opt,
                    named,
                    nullable,
                });
            }
            if cls.mixin_classes.is_empty() || !cls.constructors.is_empty() {
                return None;
            }
            current = self.ctx.as_super(&current, cls.supertype_class?)?;
        }
        None
    }

    /// Constructor call `C<args>.name(...)`; infers type arguments when they are missing.
    pub fn emit_constructor_call(
        &mut self,
        class: ClassId,
        class_args: Vec<Ty>,
        explicit_args: bool,
        ctor_name: &str,
        arguments: &ast::Arguments,
        expected: Option<&Ty>,
        is_const: bool,
    ) -> (Js, Ty) {
        // `bool/int/String.fromEnvironment` and `bool.hasEnvironment` (`const
        // factory external` constructors): environment constants, evaluated at
        // compile time — that is what the CFE does and what `dart_sdk.js`
        // requires, since the constructors throw `UnsupportedError` at runtime.
        if ctor_name == "fromEnvironment" {
            if let Some(r) = self.environment_constant(class, arguments) {
                return r;
            }
        }
        if ctor_name == "hasEnvironment" && Some(class) == self.ctx.bool_ {
            // Outside a constant it is unsupported on the web, as in the SDK.
            if !is_const && !self.in_const {
                self.m.use_sdk("core");
                return (
                    Js::prim(
                        "dart.throw(new core.UnsupportedError.new(\"bool.hasEnvironment can only be used as a const constructor\"))",
                    ),
                    self.ctx.t_bool(),
                );
            }
            // Without `-D` on the command line, no variable is declared.
            return (Js::prim("false"), self.ctx.t_bool());
        }
        if crate::extension_type::is_erased_extension_type(self.ctx, class) {
            let _ = is_const;
            return self.ext_type_construct(
                class,
                class_args,
                explicit_args,
                ctor_name,
                arguments,
                expected,
            );
        }
        let cls = self.ctx.program.class(class);
        let key = if ctor_name.is_empty() {
            self.ctx.empty_sym
        } else {
            self.ctx.sym(ctor_name)
        };
        let fid = key.and_then(|k| cls.constructors.get(&k).copied());
        let params = &self.ctx.class_params[class.0 as usize];
        // Function type of the constructor with the class's parameters as generics.
        let ctor_fn = match fid
            .map(|f| self.ctx.fn_ty(f))
            .or_else(|| key.and_then(|k| self.forwarded_constructor_ty(class, k)))
        {
            Some(t) => t,
            None => Ty::Fn {
                type_params: vec![],
                ret: Box::new(Ty::Dynamic),
                pos: vec![],
                opt: vec![],
                named: vec![],
                nullable: false,
            },
        };
        let (ctor_tps, ctor_fn) = match ctor_fn {
            Ty::Fn {
                type_params,
                ret,
                pos,
                opt,
                named,
                nullable,
            } => (
                type_params,
                Ty::Fn {
                    type_params: vec![],
                    ret,
                    pos,
                    opt,
                    named,
                    nullable,
                },
            ),
            t => (vec![], t),
        };
        let _ = ctor_tps;
        let mut subst: HashMap<u32, Ty> = HashMap::new();
        let mut targs: Vec<Ty> = Vec::new();
        // Without written arguments, the instantiation the common inference
        // chose for the class (the constructor is inferred as a generic
        // function over the class's parameters).
        let common = if explicit_args || params.is_empty() {
            None
        } else {
            self.common_instantiation(arguments, params.len())
        };
        if let Some(inst) = common {
            for (p, a) in params.iter().zip(inst) {
                subst.insert(p.id, a.clone());
                targs.push(a);
            }
        } else if explicit_args || params.is_empty() {
            for (p, a) in params.iter().zip(class_args.iter()) {
                subst.insert(p.id, a.clone());
                targs.push(a.clone());
            }
        } else {
            // Inference: expected context and arguments.
            let free: Vec<u32> = params.iter().map(|p| p.id).collect();
            if let Some(e) = expected {
                if let Some(ec) = e.non_null().class() {
                    // `C<T..>` seen as the expected supertype, matched against the expected type.
                    let this_ty = self.ctx.this_ty(class);
                    if let Some(sup) = self.ctx.as_super(&this_ty, ec) {
                        let mut tmp = HashMap::new();
                        self.match_type(&sup, &e.non_null(), &free, &mut tmp);
                        for (k, v) in tmp {
                            if !v.is_dynamic() && !v.mentions_params() {
                                subst.insert(k, v);
                            }
                        }
                    }
                }
            }
            let saved_interop = self.interop_args;
            self.interop_args = fid.is_some_and(|f| self.ctx.is_js_member(f));
            let (arg_js, _arg_tys) =
                self.emit_args_infer(&ctor_fn, arguments, &free, &mut subst, expected);
            self.interop_args = saved_interop;
            self.constrain_by_bounds(params, &free, &mut subst);
            for p in params {
                let t = subst
                    .get(&p.id)
                    .cloned()
                    .unwrap_or_else(|| self.default_type_arg(&p.bound));
                targs.push(t.clone());
                subst.insert(p.id, t);
            }
            return self.finish_ctor_call(class, targs, fid, ctor_name, arg_js, is_const);
        }
        let saved_interop = self.interop_args;
        self.interop_args = fid.is_some_and(|f| self.ctx.is_js_member(f));
        let (arg_js, _) = self.emit_args_infer(
            &ctor_fn.subst(&subst),
            arguments,
            &[],
            &mut HashMap::new(),
            expected,
        );
        self.interop_args = saved_interop;
        self.finish_ctor_call(class, targs, fid, ctor_name, arg_js, is_const)
    }

    /// Value of `const bool/int/String.fromEnvironment(name, defaultValue: v)`.
    /// Without `-D` on the command line, the value is always the `defaultValue`
    /// (`false`, `0` and `""` when omitted, as the specification requires).
    pub fn environment_constant(
        &mut self,
        class: ClassId,
        arguments: &ast::Arguments,
    ) -> Option<(Js, Ty)> {
        let (default_js, ty) = if Some(class) == self.ctx.bool_ {
            ("false".to_string(), self.ctx.t_bool())
        } else if Some(class) == self.ctx.int_ {
            ("0".to_string(), self.ctx.t_int())
        } else if Some(class) == self.ctx.string_ {
            ("\"\"".to_string(), self.ctx.t_string())
        } else {
            return None;
        };
        // `dart.library.x` is declared (`"true"`) for every library the
        // platform supports.
        if (Some(class) == self.ctx.bool_ || Some(class) == self.ctx.string_)
            && let Some(first) = arguments.args.iter().find(|a| a.name.is_none())
            && let ExprKind::String(lit) = &self.expr(first.value).kind
            && let Some(text) = lit.constant_value()
            && let Some(library) = text.as_str().and_then(|t| t.strip_prefix("dart.library."))
            && crate::SUPPORTED_DART_LIBRARIES
                .get()
                .is_some_and(|s| s.contains(library))
        {
            let value = if Some(class) == self.ctx.bool_ {
                "true"
            } else {
                "\"true\""
            };
            return Some((Js::prim(value.to_string()), ty));
        }
        let default_arg = arguments
            .args
            .iter()
            .find(|a| {
                a.name
                    .as_ref()
                    .is_some_and(|n| self.name(n.sym) == "defaultValue")
            })
            .map(|a| a.value);
        let Some(arg) = default_arg else {
            return Some((Js::prim(default_js), ty));
        };
        let (js, jty) = self.emit_expr(arg, Some(&ty));
        // Only fold when the default is already a literal; any other form falls
        // into the normal path (and the program is not valid Dart anyway).
        let literal = matches!(js.code.as_str(), "true" | "false")
            || js.code.starts_with('"')
            || js
                .code
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-');
        if literal && !js.code.is_empty() {
            return Some((js, jty));
        }
        None
    }

    fn finish_ctor_call(
        &mut self,
        class: ClassId,
        targs: Vec<Ty>,
        fid: Option<dartforge_elements::model::FunctionElementId>,
        ctor_name: &str,
        mut arg_js: Vec<String>,
        is_const: bool,
    ) -> (Js, Ty) {
        let ty = Ty::Iface {
            class,
            args: targs.clone(),
            nullable: false,
        };
        let cls = self.ctx.program.class(class);
        let cls_ref = self.class_ref(class);
        let jsname = if ctor_name.is_empty() {
            "new".to_string()
        } else {
            static_member_name(ctor_name)
        };
        let is_factory = fid.is_some_and(|f| self.ctx.program.function(f).factory);
        let generic = self.ctx.requires_rti(class);
        // JS interop (`visitConstructorInvocation`/`_emitJSInteropNew`): an
        // `@anonymous` type becomes an object literal; an `external` (or
        // synthetic) constructor is `new dart.global.Name(args)`, without rti;
        // a non-`external` factory stays in the emitted class.
        if let Some(jc) = self.ctx.js_classes.get(&class) {
            let external = fid.is_none_or(|f| {
                let fun = self.ctx.program.function(f);
                fun.external || fun.kind == FunctionKind::SyntheticConstructor
            });
            if external {
                // Object literal: `@anonymous` (package:js) and an `external`
                // extension type constructor with only named parameters
                // (`dart:js_interop`, compiler.dart:6948) — `{a: 1, b: 2}`.
                let only_named = fid.is_some_and(|f| {
                    let ps = &self.ctx.outline.functions[f.0 as usize].parameters;
                    !ps.is_empty() && ps.iter().all(|p| p.kind == ast::ParameterKind::Named)
                });
                if jc.anonymous || only_named {
                    let obj = arg_js
                        .last()
                        .filter(|a| a.starts_with('{'))
                        .cloned()
                        .unwrap_or_else(|| "{}".to_string());
                    return (Js::prim(obj), ty);
                }
                return (
                    Js::prim(format!(
                        "new {}({})",
                        self.ctx.js_class_global(class),
                        arg_js.join(", ")
                    )),
                    ty,
                );
            }
        }
        // Factory redirection `= Other`: resolve the target.
        if is_factory {
            if let Some(target) = self.factory_redirect_target_type(fid.unwrap()) {
                let (tclass, tname, is_target_factory, target_ty) = target;
                let tref = self.class_ref(tclass);
                let tgeneric = self.ctx.requires_rti(tclass);
                if tgeneric {
                    // The target type with this call's class arguments.
                    let map: HashMap<u32, Ty> = self.ctx.class_params[class.0 as usize]
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (p.id, targs.get(i).cloned().unwrap_or(Ty::Dynamic)))
                        .collect();
                    let tt = match target_ty.subst(&map) {
                        t @ Ty::Iface { .. } => t,
                        _ => ty.clone(),
                    };
                    arg_js.insert(0, self.rti(&tt));
                }
                let tjs = if tname.is_empty() {
                    "new".to_string()
                } else {
                    static_member_name(&tname)
                };
                let call = if is_target_factory {
                    format!("{tref}.{tjs}({})", arg_js.join(", "))
                } else {
                    format!("new {tref}.{tjs}({})", arg_js.join(", "))
                };
                let call = if is_const {
                    format!("dart.const({call})")
                } else {
                    call
                };
                self.m.note_class(class);
                return (Js::prim(call), ty);
            }
        }
        if generic {
            arg_js.insert(0, self.rti(&ty));
        }
        self.m.note_class(class);
        let _ = cls;
        // Non-`external` generative constructor of an interop extension type:
        // the static function of the class object (`emit_js_interop_class`),
        // which returns the representation — there is no JS class for `new`.
        let ext_interop = cls.kind == dartforge_elements::model::ClassKind::ExtensionType
            && self.ctx.js_classes.contains_key(&class);
        let call = if is_factory || ext_interop {
            format!("{cls_ref}.{jsname}({})", arg_js.join(", "))
        } else {
            format!("new {cls_ref}.{jsname}({})", arg_js.join(", "))
        };
        let call = if is_const {
            format!("dart.const({call})")
        } else {
            call
        };
        (Js::prim(call), ty)
    }

    /// Target of `factory C.x(...) = D.y;`.
    pub fn factory_redirect_target(
        &self,
        fid: dartforge_elements::model::FunctionElementId,
    ) -> Option<(ClassId, String, bool)> {
        self.factory_redirect_target_type(fid)
            .map(|(c, n, f, _)| (c, n, f))
    }

    /// [`Self::factory_redirect_target`] with the effective target's type, in
    /// terms of the redirecting constructor class's type parameters (`C() =
    /// D<int>` and `D<T>() = E<double, T>` give `E<double, int>`).
    pub fn factory_redirect_target_type(
        &self,
        fid: dartforge_elements::model::FunctionElementId,
    ) -> Option<(ClassId, String, bool, Ty)> {
        let fid = self.ctx.program.function(fid).patched_by.unwrap_or(fid);
        let f = self.ctx.program.function(fid);
        let dartforge_elements::model::FunctionRef::Constructor { unit, member } = f.node else {
            return None;
        };
        let m = self.ctx.program.unit(unit).ast.member(member);
        let ast::MemberKind::Constructor(ctor) = &m.kind else {
            return None;
        };
        let redirect = ctor.redirect.as_ref()?;
        // Resolve the target type in the context of the constructor's unit.
        let sub = FnEmitter::new(self.ctx, self.m, unit, f.class, true);
        let mut t = sub.resolve_type(redirect.ty);
        let mut name = redirect
            .constructor
            .map(|n| self.name(n.sym).to_string())
            .unwrap_or_default();
        // `= Class.name` arrives as a type with two parts (`prefix.Type`): if
        // the first part is a class, the second is the constructor name.
        if !matches!(t, Ty::Iface { .. }) && redirect.constructor.is_none() {
            if let ast::TypeKind::Named { name: parts, .. } = &sub.ast().ty(redirect.ty).kind {
                if parts.len() == 2 {
                    if let Some(b) = self.ctx.program.lookup_in_unit(sub.unit, parts[0].sym) {
                        if let Some(Element::Class(c)) = b.getter {
                            t = Ty::iface(c);
                            name = self.name(parts[1].sym).to_string();
                        }
                    }
                }
            }
        }
        let Ty::Iface {
            class, ref args, ..
        } = t
        else {
            return None;
        };
        let key = if name.is_empty() {
            self.ctx.empty_sym
        } else {
            self.ctx.sym(&name)
        };
        let tfid = key.and_then(|k| self.ctx.program.class(class).constructors.get(&k).copied());
        let is_factory = tfid.is_some_and(|x| self.ctx.program.function(x).factory);
        // If the target redirects too, follow it, composing the arguments.
        if let Some(tf) = tfid {
            if is_factory {
                if let Some((c2, n2, f2, t2)) = self.factory_redirect_target_type(tf) {
                    let map: HashMap<u32, Ty> = self.ctx.class_params[class.0 as usize]
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (p.id, args.get(i).cloned().unwrap_or(Ty::Dynamic)))
                        .collect();
                    return Some((c2, n2, f2, t2.subst(&map)));
                }
            }
        }
        Some((class, name, is_factory, t))
    }
}

/// Dart name of the binary operator.
pub fn binop_name(op: BinaryOp) -> &'static str {
    match op {
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
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IfNull => "??",
        // `a ~ b`: a compile-time error (`NOT_BINARY_OPERATOR`); it never reaches emission.
        BinaryOp::NotBinary => "~",
    }
}

fn parse_int(text: &str) -> String {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        match u64::from_str_radix(hex, 16) {
            Ok(v) => v.to_string(),
            // Beyond 64 bits (only valid as a `double`): the decimal digits
            // of the nearest double.
            Err(_) => {
                let v = u128::from_str_radix(hex, 16).map_or_else(
                    |_| {
                        hex.chars()
                            .filter_map(|c| c.to_digit(16))
                            .fold(0f64, |acc, d| acc * 16.0 + f64::from(d))
                    },
                    |v| v as f64,
                );
                format!("{v:.0}")
            }
        }
    } else {
        let t = text.trim_start_matches('0');
        if t.is_empty() {
            "0".to_string()
        } else {
            t.to_string()
        }
    }
}

pub fn is_simple(code: &str) -> bool {
    code == "this"
        || code
            .chars()
            .all(|c| c.is_alphanumeric() || c == '$' || c == '_')
}

impl Ty {
    pub fn non_null_if(&self, cond: bool) -> Ty {
        if cond { self.non_null() } else { self.clone() }
    }
}

#[allow(dead_code)]
fn _unused(_: ClassKind, _: LibraryId) {}

/// The property key of a member access (`.name` or `[key]`), as an
/// expression: `"name"` or `key`.
fn access_key(access: &str) -> String {
    match access.strip_prefix('.') {
        Some(name) => format!("\"{name}\""),
        None => access
            .strip_prefix('[')
            .and_then(|a| a.strip_suffix(']'))
            .unwrap_or(access)
            .to_string(),
    }
}
