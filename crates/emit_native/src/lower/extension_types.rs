//! Erased extension types (Dart 3.3).
//!
//! At run time a value of an extension type **is** the value of the
//! representation (`erasure.rs` erases the types; no extension type has a
//! class in the heap). The members are resolved statically, like those of an
//! extension, and so the lowering cannot look them up in the (erased) type of the
//! receiver: inference says, at each access, that the member belongs to an extension
//! type (`Resolved::Member { class }`), and it is inference that decides the route.
//!
//! * instance member (method, getter, setter, operator) → the function
//!   `df.<library>.<E>.<m>(this, …)` with `this` in the representation, like the
//!   member of an extension (`extensions.rs`); the type arguments of `E`
//!   go in the tuple, before those of the member, taken from the static (not
//!   erased) type of the receiver;
//! * the representation (`e.v`) → the receiver itself;
//! * primary constructor → the value of the argument;
//! * another constructor (generative or factory) → a function that returns the
//!   representation, called like a factory (the creation tuple at the end);
//! * statics → like those of a class.

use super::closures::{DefaultValue, EntryParam};
use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionRef};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::TypeId;

/// The extension type that declares `fid`, when it is a written instance
/// member (method, getter, setter, operator; not the accessor of the
/// representation nor a constructor).
pub fn instance_owner(ctx: &Context, fid: usize) -> Option<ClassId> {
    let f = &ctx.program.functions[fid];
    let c = f.class?;
    (ctx.is_extension_type(c) && !f.static_ && f.variable.is_none() && matches!(f.node, FunctionRef::Function { .. }))
        .then_some(c)
}

/// The name of the primary constructor of `c` (the empty symbol for the unnamed one).
pub fn primary_name(ctx: &Context, c: ClassId) -> Option<SymbolId> {
    let decl = ctx.program.class(c).decl?;
    match &ctx.program.unit(decl.unit).ast.decl(decl.decl).kind {
        DeclKind::ExtensionType(ed) => match ed.constructor {
            Some(n) => Some(n.sym),
            None => ctx.interner.lookup(""),
        },
        _ => None,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The receiver pattern of an instance member called with the
    /// receiver as the first parameter: the number of type parameters of the
    /// owner and the type the receiver's type is matched against (the `on` of an extension,
    /// `E<T…>` of an extension type).
    pub fn receiver_pattern(&self, fid: usize) -> Option<(usize, TypeId)> {
        let f = &self.ctx.program.functions[fid];
        if let Some(e) = f.extension.filter(|_| !f.static_) {
            let x = self.ctx.outline.extensions.get(e.0 as usize)?;
            return Some((x.type_params.len(), x.on));
        }
        let c = instance_owner(self.ctx, fid)?;
        let n = self.ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
        Some((n, *self.ctx.te.this.get(&c)?))
    }

    /// The representation of `this` in an instance member of an extension or of an
    /// extension type.
    pub fn static_this_repr(&self, fid: usize) -> Option<Type> {
        self.receiver_pattern(fid).map(|(_, t)| self.repr(t))
    }

    /// The static type of the implicit receiver (`this`) in a member of an extension
    /// or of an extension type.
    pub fn implicit_receiver(&self) -> Option<TypeId> {
        self.this_extension.map(|(_, on)| on).or(self.this_ext_type.map(|(_, t)| t))
    }

    /// The extension type instance member that inference resolved
    /// for `e` (read, call, write or operator).
    pub fn member_te(&self, e: ExprId) -> Option<MemberRef> {
        let r = self.ctx.get_resolved(self.unit_id, e).cloned();
        self.resolved_member_te(r.as_ref())
    }

    /// [`Self::member_te`] of an already read resolution.
    pub fn resolved_member_te(&self, r: Option<&Resolved>) -> Option<MemberRef> {
        let Some(Resolved::Member { class, member, via_super: false }) = r else { return None };
        if !self.ctx.is_extension_type(*class) {
            return None;
        }
        let static_ty = match member {
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].static_,
            MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].static_,
        };
        (!static_ty).then_some(*member)
    }

    /// The member is the representation (the field, or its getter).
    pub fn is_representation(&self, m: MemberRef) -> bool {
        let v = match m {
            MemberRef::Variable(v) => v,
            MemberRef::Function(f) => match self.ctx.program.functions[f.0 as usize].variable {
                Some(v) => v,
                None => return false,
            },
        };
        let var = &self.ctx.program.variables[v.0 as usize];
        var.class.is_some_and(|c| self.ctx.program.classes[c.0 as usize].representation == Some(v))
    }

    /// The function of the extension type member resolved for `e` (the
    /// representation is left out: it is not called).
    pub fn function_te(&self, e: ExprId) -> Option<usize> {
        match self.member_te(e)? {
            MemberRef::Function(f) if !self.is_representation(MemberRef::Function(f)) => Some(f.0 as usize),
            _ => None,
        }
    }

    /// `recv.m`: the representation is the receiver itself; the getter is called; the
    /// method becomes a tear-off.
    pub fn read_member_te(&mut self, recv: Operand, m: MemberRef, receiver: Option<TypeId>, span: Span) -> Operand {
        if self.is_representation(m) {
            return recv;
        }
        match m {
            MemberRef::Function(f) => self.read_extension(recv, f.0 as usize, receiver, span),
            MemberRef::Variable(_) => self.unsupported("campo de instância de tipo de extensão", span),
        }
    }

    /// The extension type that the expression names (`E`, `p.E`), if it is one.
    pub fn named_extension_type(&self, ast: &ast::Ast, e: ExprId) -> Option<ClassId> {
        let c = match &ast.expr(e).kind {
            ExprKind::Identifier(n) => match self.ctx.get_resolved(self.unit_id, e).cloned() {
                Some(Resolved::Element(Element::Class(c))) => c,
                None if self.lookup_local(n.sym).is_none() => match self.resolve_by_name(n.sym)? {
                    Resolved::Element(Element::Class(c)) => c,
                    _ => return None,
                },
                _ => return None,
            },
            ExprKind::Property { target, name, null_aware: false } => match self.prefixed_element(ast, *target, name.sym)? {
                Element::Class(c) => c,
                _ => return None,
            },
            _ => return None,
        };
        self.ctx.is_extension_type(c).then_some(c)
    }

    /// The extension type creation that the call `target(…)` performs: `E(…)`,
    /// `E.name(…)` (also with a prefix); the type and the name of the constructor.
    pub fn creation_te(&self, ast: &ast::Ast, alvo: ExprId) -> Option<(ClassId, SymbolId)> {
        if let Some(c) = self.named_extension_type(ast, alvo) {
            return Some((c, self.ctx.interner.lookup("")?));
        }
        let ExprKind::Property { target, name, null_aware: false } = &ast.expr(alvo).kind else { return None };
        let c = self.named_extension_type(ast, *target)?;
        let is_constructor = primary_name(self.ctx, c) == Some(name.sym)
            || self.ctx.program.classes[c.0 as usize].constructors.contains_key(&name.sym);
        is_constructor.then_some((c, name.sym))
    }

    /// `E.name(args)` (`expr` is the creation, whose static type gives the
    /// type arguments): the primary one is the value of the argument; the others, the
    /// function of the constructor.
    pub fn build_te(
        &mut self,
        ast: &ast::Ast,
        expr: ExprId,
        c: ClassId,
        name_str: SymbolId,
        args: &[ast::Argument],
        span: Span,
    ) -> Operand {
        if primary_name(self.ctx, c) == Some(name_str) {
            let Some(a) = args.first() else {
                return self.unsupported("construtor primário sem argumento", span);
            };
            let v = self.lower_expr(ast, a.value);
            return match self.ctx.get_type(self.unit_id, expr) {
                Some(t) => {
                    let r = self.repr(t);
                    self.coerce(v, r)
                }
                None => v,
            };
        }
        let Some(&f) = self.ctx.program.classes[c.0 as usize].constructors.get(&name_str) else {
            return self.unsupported("construtor de tipo de extensão não encontrado", span);
        };
        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr);
        self.instantiate(ast, f, args, span)
    }

    /// Tear-off of an extension type constructor (`E.new`, `E.name`): the
    /// primary one returns the argument; the others are those of an ordinary constructor.
    pub fn constructor_tearoff_te(&mut self, expr: ExprId, c: ClassId, name_str: SymbolId, span: Span) -> Operand {
        if primary_name(self.ctx, c) != Some(name_str) {
            let Some(&f) = self.ctx.program.classes[c.0 as usize].constructors.get(&name_str) else {
                return self.unsupported("construtor de tipo de extensão não encontrado", span);
            };
            return self.constructor_tearoff(f.0 as usize, span);
        }
        let class = &self.ctx.program.classes[c.0 as usize];
        let symbol_ent = format!(
            "df.{}.{}.$primario$tear",
            crate::context::escape_str(&self.ctx.library_name(class.library)),
            crate::context::escape_str(self.ctx.symbol_name(class.name))
        );
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let rep_name = class
                .representation
                .map(|v| self.ctx.symbol_name(self.ctx.program.variables[v.0 as usize].name).to_string());
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), "new".to_string(), Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let infos = [EntryParam { name: rep_name, kind: ParameterKind::Required, required: true, pattern: DefaultValue::Absent }];
            if let Some(vals) = e.unpack(&infos, args, desc) {
                let v = e.coerce(vals[0].clone(), Type::Ref);
                e.terminate(Terminator::Return(Some(v)));
            }
            self.absorb(e);
        }
        let t = self.emit(Instruction::TearOff { code_symbol: symbol_ent }, Type::Ref);
        // The signature is the static type of the tear-off (`E Function(R)`),
        // erased.
        if let Some(sig) = self.ctx.get_type_raw(self.unit_id, expr) {
            let r = self.type_recipe(sig);
            let ty = self.recipe_rti(&r);
            self.define_rti(t.clone(), ty);
        }
        t
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The body of a generative constructor of extension type `c`: the
    /// initializers give the representation (`this.v`, `v = e` or `this(…)`),
    /// which is `this` in the body and the returned value. The parameters are already
    /// declared; in a generic class, the tuple comes at the end, as in a factory.
    pub fn lower_constructor_te(&mut self, ast: &ast::Ast, c: ClassId, ctor: &ast::Constructor, span: Span) {
        self.enclosing_class = Some(c);
        self.member_class = Some(c);
        if self.generic_class(c) {
            let t = self.add_param_rti("$tipos".to_string());
            self.type_tuple = Some(Operand::Val(t));
            self.class_by_tuple = true;
        }
        let this_t = self.ctx.te.this.get(&c).copied();
        let rep = self.ctx.program.classes[c.0 as usize].representation;
        let rep_name = rep.map(|v| self.ctx.program.variables[v.0 as usize].name);
        let mut val_ref: Option<Operand> = None;
        for p in ctor.parameters.iter() {
            if p.this_
                && let Some(n) = p.name
                && Some(n.sym) == rep_name
            {
                val_ref = self.read_local_by_name(n.sym);
            }
        }
        for init in ctor.initializers.iter() {
            match init {
                ast::Initializer::Field { name, value, .. } if Some(name.sym) == rep_name => {
                    val_ref = Some(self.lower_expr(ast, *value));
                }
                ast::Initializer::Field { span, .. } => {
                    self.unsupported("campo de instância de tipo de extensão", *span);
                }
                ast::Initializer::Assert { condition, message, .. } => self.lower_assert(ast, *condition, *message),
                ast::Initializer::Redirect { constructor, arguments, span } => {
                    let name_str = match constructor {
                        Some(n) => Some(n.sym),
                        None => self.ctx.interner.lookup(""),
                    };
                    val_ref = Some(match name_str {
                        Some(n) if primary_name(self.ctx, c) == Some(n) => match arguments.args.first() {
                            Some(a) => self.lower_expr(ast, a.value),
                            None => self.unsupported("construtor primário sem argumento", *span),
                        },
                        Some(n) => match self.ctx.program.classes[c.0 as usize].constructors.get(&n).copied() {
                            Some(f) => {
                                let evaluated = self.evaluate_args(ast, &arguments.args);
                                self.creation_type = this_t;
                                self.instantiate_evaluated(f, &evaluated, *span)
                            }
                            None => self.unsupported("construtor redirecionado não encontrado", *span),
                        },
                        None => self.unsupported("construtor redirecionado não encontrado", *span),
                    });
                }
                ast::Initializer::Super { span, .. } => {
                    self.unsupported("`super` em construtor de tipo de extensão", *span);
                }
            }
            if self.is_terminated() {
                return;
            }
        }
        let ret = self.func.return_ty;
        let v = match val_ref {
            Some(v) => self.coerce(v, ret),
            None => self.unsupported("construtor de tipo de extensão sem a representação", span),
        };
        self.this_param = Some(v.clone());
        self.finalizable_this = self.ctx.classify_finalizable_class(c);
        self.this_ext_type = this_t.map(|t| (c, t));
        self.constructor_return = Some(v);
        match &ctor.body {
            ast::FunctionBody::Block(s) => self.lower_stmt(ast, *s),
            ast::FunctionBody::Expression(e) => {
                self.lower_expr(ast, *e);
            }
            _ => {}
        }
        if !self.is_terminated() {
            self.route_return(None);
        }
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.m(args)` (or `m(args)` with `this`) for the extension type member
    /// `m`: the method is called directly; the representation and the getter
    /// are read and the value called.
    #[allow(clippy::too_many_arguments)]
    pub fn call_member_te(
        &mut self,
        ast: &ast::Ast,
        recv: Operand,
        m: MemberRef,
        receiver: Option<TypeId>,
        expr: ExprId,
        arguments: &ast::Arguments,
        span: Span,
    ) -> Operand {
        let getter = match m {
            MemberRef::Function(f) => {
                self.ctx.program.functions[f.0 as usize].kind == dartforge_elements::model::FunctionKind::Getter
            }
            MemberRef::Variable(_) => true,
        };
        if getter || self.is_representation(m) {
            let v = self.read_member_te(recv, m, receiver, span);
            let evaluated = self.evaluate_args(ast, &arguments.args);
            return self.call_function_value(v, &evaluated);
        }
        let MemberRef::Function(f) = m else { unreachable!("campo tratado acima") };
        let evaluated = self.evaluate_args(ast, &arguments.args);
        self.call_extension(recv, f.0 as usize, &evaluated, receiver, Some((expr, arguments)), span)
    }

    /// Member `sym` seen without a receiver in the body of a member of the current
    /// extension type: that of the type itself, otherwise that of an extension type
    /// it implements (breadth-first); `None` for the rest (the members of the
    /// representation it implements keep the ordinary resolution).
    pub fn member_te_by_name(&self, sym: SymbolId) -> Option<Resolved> {
        let (c0, _) = self.this_ext_type?;
        self.member_te_in_class(c0, sym)
    }

    /// Member `sym` of extension type `c0` (or of an extension type
    /// it implements), as inference would resolve it.
    pub fn member_te_in_class(&self, c0: ClassId, sym: SymbolId) -> Option<Resolved> {
        let mut queue = std::collections::VecDeque::from([c0]);
        let mut seen_set = std::collections::HashSet::new();
        while let Some(c) = queue.pop_front() {
            if !seen_set.insert(c) {
                continue;
            }
            let class = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class.instance_members.get(&sym).or_else(|| class.static_members.get(&sym)) {
                return Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false });
            }
            if let Some(&v) = class.fields.iter().find(|&&v| self.ctx.program.variables[v.0 as usize].name == sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            queue.extend(class.interface_classes.iter().copied().filter(|&i| self.ctx.is_extension_type(i)));
        }
        None
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The instance member `sym` of the owner of `fid` (the same extension; the
    /// same extension type or an extension type it implements):
    /// the getter/setter pair and the `[]=` of the `[]` in a compound write.
    pub fn accessor_of_same_owner(&self, fid: usize, sym: SymbolId) -> Option<dartforge_elements::model::FunctionElementId> {
        let f = &self.ctx.program.functions[fid];
        if let Some(e) = f.extension {
            return self.ctx.program.extensions[e.0 as usize].instance_members.get(&sym).copied();
        }
        let c0 = f.class.filter(|&c| self.ctx.is_extension_type(c))?;
        let mut queue = std::collections::VecDeque::from([c0]);
        let mut seen_set = std::collections::HashSet::new();
        while let Some(c) = queue.pop_front() {
            if !seen_set.insert(c) {
                continue;
            }
            let class = &self.ctx.program.classes[c.0 as usize];
            if let Some(&g) = class.instance_members.get(&sym) {
                return Some(g);
            }
            queue.extend(class.interface_classes.iter().copied().filter(|&i| self.ctx.is_extension_type(i)));
        }
        None
    }
}
