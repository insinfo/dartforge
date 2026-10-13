//! Calls: top-level, local and static user functions, constructors
//! (`C(…)`, `C.name(…)`) and instance methods through the resolved element.
//! An SDK member without a useful element goes through the dynamic class of the receiver
//! (`FnBuilder::method_by_selector`; the SDK always comes from source).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `f(…)`, `o.m(…)`, `o?.m(…)`, `C(…)`, `C.m(…)`.
    ///
    /// RTI: the type argument tuple of a generic call
    /// (`armed_tuple`) holds only for the call of this expression; the outer one
    /// comes back at the end (a call in the arguments arms its own).
    pub(super) fn lower_call(
        &mut self,
        ast: &ast::Ast,
        expr_id: ExprId,
        expr: &ast::Expr,
        target: &ExprId,
        arguments: &ast::Arguments,
    ) -> Operand {
        let saved = self.armed_tuple.take();
        // The type of the called value, for the typed closure call; saved and
        // restored like the tuple (calls nested in the arguments).
        let saved_type = std::mem::replace(&mut self.callee_type, self.ctx.get_type(self.unit_id, *target));
        let saved_call = self.current_call.replace(expr_id);
        let armed = self.arm_arg_types(*target, expr_id, &arguments.args);
        let r = self.lower_call_internal(ast, expr_id, expr, target, arguments);
        self.arg_types = armed;
        self.armed_tuple = saved;
        self.callee_type = saved_type;
        self.current_call = saved_call;
        r
    }

    fn lower_call_internal(
        &mut self,
        ast: &ast::Ast,
        expr_id: ExprId,
        expr: &ast::Expr,
        target: &ExprId,
        arguments: &ast::Arguments,
    ) -> Operand {
        // `E(…)`, `E.name(…)` of an extension type (`extension_types.rs`).
        if let Some((c, name)) = self.creation_te(ast, *target) {
            return self.build_te(ast, expr_id, c, name, &arguments.args, expr.span);
        }
        // `Native.addressOf<T>(f)`: the VM front end rewrites it into the
        // address of the symbol; the argument is the element, not a value.
        if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = self.ctx.get_resolved(self.unit_id, *target).cloned() {
            let fe = &self.ctx.program.functions[f.0 as usize];
            if self.ctx.symbol_name(fe.name) == "addressOf"
                && fe.class.is_some_and(|c| self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name) == "Native")
                && self.ctx.program.library(fe.library).uri == "dart:ffi"
                && let Some(a) = arguments.args.first()
            {
                return self.native_address(ast, a.value, expr.span);
            }
        }

        // `value(args)` resolved to the `call` of an extension
        // (`calloc<Int32>(4)`, the `AllocatorAlloc.call`): the value is the receiver.
        if let Some(Resolved::ExtensionMember { member, .. }) = self.ctx.get_resolved(self.unit_id, expr_id).cloned()
            && self.ctx.symbol_name(self.ctx.program.functions[member.0 as usize].name) == "call"
        {
            let recv = self.lower_expr(ast, *target);
            let recv = self.coerce(recv, Type::Ref);
            let evaluated = self.evaluate_args(ast, &arguments.args);
            let receiver = self.ctx.get_type_raw(self.unit_id, *target);
            return self.call_extension(recv, member.0 as usize, &evaluated, receiver, Some((expr_id, arguments)), expr.span);
        }

        if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
            let resolved = self.ctx.get_resolved(self.unit_id, *target).cloned();
            // A local or parameter that keeps a function value (local function,
            // closure, tear-off): called through the uniform convention (P1).
            if matches!(
                resolved,
                None | Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. })
            ) && let Some(local) = self.lookup_local(id.sym)
            {
                // Direct local function (`direct_functions.rs`): a static `call`.
                if let Some(d) = self.local_direct(&local) {
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    return self.call_direct_fn(&d, &evaluated);
                }
                let f = self.read_local(&local);
                let evaluated = self.evaluate_args(ast, &arguments.args);
                return self.call_function_value(f, &evaluated);
            }
            // No resolution (closure body): through the lexical scope.
            let resolved = match resolved {
                None => self.resolve_by_name(id.sym),
                r => r,
            };
            // `E(x)`: explicit extension application (override,
            // §13.3). It only appears as the receiver of a member of `E`, which
            // inference already resolved; the value is `x` itself.
            if let Some(Resolved::Element(dartforge_elements::model::Element::Extension(_))) = resolved {
                let Some(a) = arguments.args.first() else {
                    return self.unsupported("aplicação de extensão sem receptor", expr.span);
                };
                return self.lower_expr(ast, a.value);
            }
            // Extension type member with the implicit `this`.
            if let Some(m) = self.resolved_member_te(resolved.as_ref()) {
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                return self.call_member_te(ast, this, m, receiver, expr_id, arguments, expr.span);
            }
            // Top-level function, implicit member (`m()` = `this.m()`) or
            // static: through the resolved element of the target.
            if let Some(Resolved::ExtensionMember { member, .. }) = resolved {
                let evaluated = self.evaluate_args(ast, &arguments.args);
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                return self.call_extension(
                    this,
                    member.0 as usize,
                    &evaluated,
                    receiver,
                    Some((expr_id, arguments)),
                    expr.span,
                );
            }
            // An SDK method called without a receiver (in the body of an extension
            // on an SDK type): through the selector, with `this`.
            if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = resolved
                && !crate::lower::user_function(self.ctx, f.0 as usize)
                && let Some(this) = self.this_param.clone()
            {
                let name = self.ctx.symbol_name(id.sym).to_string();
                return self.method_by_selector(ast, &name, this, arguments);
            }
            match resolved {
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                    if crate::lower::user_function(self.ctx, f.0 as usize)
                        && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
                {
                    let fid = f.0 as usize;
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    if self.ctx.program.functions[fid].kind
                        == dartforge_elements::model::FunctionKind::Getter
                    {
                        let v = self.call_direct(fid, None, Vec::new());
                        return self.call_function_value(v, &evaluated);
                    }
                    let args = self.match_args(fid, &evaluated);
                    self.arm_tuple(fid, expr_id, arguments);
                    return self.call_direct(fid, None, args);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                    if crate::lower::user_function(self.ctx, f.0 as usize) =>
                {
                    // A top-level variable of function type.
                    let vid = self.ctx.program.functions[f.0 as usize]
                        .variable
                        .expect("acessor");
                    let v = self.read_global(vid, expr.span);
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    return self.call_function_value(v, &evaluated);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Variable(vid)))
                    if self.ctx.compiled_library(self.ctx.program.variables[vid.0 as usize].library) =>
                {
                    let v = self.read_global(vid, expr.span);
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    return self.call_function_value(v, &evaluated);
                }
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) if crate::lower::user_function(self.ctx, f.0 as usize)
                    && self.ctx.program.functions[f.0 as usize].variable.is_none()
                    && self.ctx.program.functions[f.0 as usize].kind
                        != dartforge_elements::model::FunctionKind::Getter =>
                {
                    let fid = f.0 as usize;
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    if self.ctx.program.functions[fid].static_ {
                        let args = self.match_args(fid, &evaluated);
                        self.arm_tuple(fid, expr_id, arguments);
                        return self.call_direct(fid, None, args);
                    }
                    let Some(this) = self.this_param.clone() else {
                        return self.unsupported(
                            "método de instância fora de membro de instância",
                            expr.span,
                        );
                    };
                    self.arm_tuple(fid, expr_id, arguments);
                    return self.call_member(this, fid, &evaluated, expr.span);
                }
                Some(Resolved::Element(dartforge_elements::model::Element::Class(c)))
                    if self.ctx.get_resolved(self.unit_id, expr_id).is_none() =>
                {
                    // `C(…)` without resolution (operand of `throw`, body of a
                    // closure): the unnamed constructor of the class.
                    let empty = self.ctx.interner.lookup("");
                    let ctor = empty.and_then(|v| self.constructor_of(c, v));
                    if let Some(f) = ctor {
                        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                        return self.instantiate(ast, f, &arguments.args, expr.span);
                    }
                }
                Some(Resolved::Member { member, class, .. })
                    if self.ctx.compiled_library(self.ctx.program.classes[class.0 as usize].library) =>
                {
                    // A field or getter of function type: reads and calls the value.
                    let v = self.read_implicit_member(member, expr.span);
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    return self.call_function_value(v, &evaluated);
                }
                _ => {}
            }
        }
        // Instantiation without `new`: `C(…)`, `C.name(…)`.
        if let Some(Resolved::Constructor(fid)) =
            self.ctx.get_resolved(self.unit_id, expr_id).cloned()
        {
            self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
            return self.instantiate(ast, fid, &arguments.args, expr.span);
        }

        // Method calls on an object / collection
        if let ExprKind::Property {
            target: inner_target,
            name: method_name,
            null_aware,
        } = &ast.expr(*target).kind
        {
            let m_name = self.ctx.symbol_name(method_name.sym);
            let resolved_target = self.ctx.get_resolved(self.unit_id, *target).cloned();

            // `p.loadLibrary()` of an `import … deferred as p`: the compiled
            // program already has the library (the VM also loads it on the spot);
            // the `Future` completes without a value.
            if m_name == "loadLibrary"
                && arguments.args.is_empty()
                && self.deferred_prefix(ast, *inner_target)
            {
                return self.call_support("_dartforgeCarregarBiblioteca", Vec::new(), expr.span);
            }

            // `prefix.f(…)`: the element imported with a prefix.
            if let Some(el) = self.prefixed_element(ast, *inner_target, method_name.sym) {
                use dartforge_elements::model::Element;
                match el {
                    Element::Function(f)
                        if crate::lower::user_function(self.ctx, f.0 as usize)
                            && self.ctx.program.functions[f.0 as usize].variable.is_none()
                            && self.ctx.program.functions[f.0 as usize].kind
                                != dartforge_elements::model::FunctionKind::Getter =>
                    {
                        let fid = f.0 as usize;
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        let args = self.match_args(fid, &evaluated);
                        return self.call_direct(fid, None, args);
                    }
                    Element::Class(c) if self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library) => {
                        let empty = self.ctx.interner.lookup("");
                        if let Some(f) = empty.and_then(|v| self.constructor_of(c, v)) {
                            self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                            return self.instantiate(ast, f, &arguments.args, expr.span);
                        }
                    }
                    // `p.E(x)`: explicit application of an extension imported
                    // with a prefix (§13.3), like `E(x)` above: the value is `x`.
                    Element::Extension(_) => {
                        let Some(a) = arguments.args.first() else {
                            return self.unsupported("aplicação de extensão sem receptor", expr.span);
                        };
                        return self.lower_expr(ast, a.value);
                    }
                    other => {
                        let v = self.read_element(other, expr.span);
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(v, &evaluated);
                    }
                }
            }

            // `E.m(…)`: a static extension member (a method, or a getter/field
            // of function type that gets called).
            if matches!(
                self.ctx.get_resolved(self.unit_id, *inner_target),
                Some(Resolved::Element(dartforge_elements::model::Element::Extension(_)))
            ) {
                match resolved_target {
                    Some(Resolved::ExtensionMember { member, .. })
                        if crate::lower::user_function(self.ctx, member.0 as usize)
                            && self.ctx.program.functions[member.0 as usize].kind
                                == dartforge_elements::model::FunctionKind::Function =>
                    {
                        let fid = member.0 as usize;
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        let args = self.match_args(fid, &evaluated);
                        self.arm_tuple(fid, expr_id, arguments);
                        return self.call_direct(fid, None, args);
                    }
                    Some(Resolved::ExtensionMember { member, .. }) => {
                        let v = self.read_static_member(MemberRef::Function(member), expr.span);
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(v, &evaluated);
                    }
                    Some(Resolved::Element(el)) => {
                        let v = self.read_element(el, expr.span);
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(v, &evaluated);
                    }
                    _ => return self.unsupported(&format!("membro estático de extensão `{m_name}`"), expr.span),
                }
            }

            // `A<T>.name(…)` of a named mixin application (`class A = B
            // with M;`): the forwarded constructor of the superclass, with the
            // `A<T>` object.
            if let ExprKind::TypeArguments { target: t, type_args } = &ast.expr(*inner_target).kind
                && !matches!(self.ctx.get_resolved(self.unit_id, expr_id), Some(Resolved::Constructor(_)))
                && let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.resolution_or_name(ast, *t)
                && self.ctx.program.classes[c.0 as usize].kind == dartforge_elements::model::ClassKind::MixinApplication
                && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                && let Some(f) = self.constructor_of(c, method_name.sym)
            {
                let concrete = self.concrete_class.take();
                let evaluated = self.evaluate_args(ast, &arguments.args);
                self.concrete_class = concrete;
                let args = self.type_argument_recipes(type_args);
                let object = self.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<{}>", self.ctx.id_rti(c), args.text),
                    variables: args.variables,
                });
                let tuple = self.recipe_rti(&super::rti::Recipe {
                    text: format!("L<{}>", args.text),
                    variables: args.variables,
                });
                return self.instantiate_evaluated_with_rti(f, &evaluated, expr.span, Some(object), Some(tuple));
            }

            // `C.m(…)`: a static user method.
            // `C.m(…)` (and `Alias.m(…)`, with `typedef Alias = C`).
            let target_is_class = matches!(
                self.ctx.get_resolved(self.unit_id, *inner_target),
                Some(Resolved::Element(
                    dartforge_elements::model::Element::Class(_) | dartforge_elements::model::Element::Typedef(_)
                ))
            );
            if target_is_class {
                // `C.name(…)` that is a named constructor (the resolution stayed on the
                // target, not on the call).
                if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.ctx.get_resolved(self.unit_id, *inner_target).cloned()
                    && !matches!(resolved_target, Some(Resolved::Member { .. }))
                    && let Some(f) = self.constructor_of(c, method_name.sym)
                {
                    self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                    return self.instantiate(ast, f, &arguments.args, expr.span);
                }
                // No resolution of the member (inference did not resolve the
                // access): the static of the class by name.
                let resolved_target = resolved_target.or_else(|| {
                    let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                        self.ctx.get_resolved(self.unit_id, *inner_target).cloned()
                    else {
                        return None;
                    };
                    let f = *self.ctx.program.classes[c.0 as usize].static_members.get(&method_name.sym)?;
                    Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false })
                });
                if let Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) = resolved_target
                {
                    let fid = f.0 as usize;
                    if crate::lower::user_function(self.ctx, fid)
                        && self.ctx.program.functions[fid].static_
                    {
                        if self.ctx.program.functions[fid].kind
                            == dartforge_elements::model::FunctionKind::Getter
                        {
                            // `C.g(args)` reads `g` first and calls the returned
                            // function. The arguments do not belong to the getter.
                            let callee = self.call_direct(fid, None, Vec::new());
                            let evaluated = self.evaluate_args(ast, &arguments.args);
                            return self.call_function_value(callee, &evaluated);
                        }
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        let args = self.match_args(fid, &evaluated);
                        self.arm_tuple(fid, expr_id, arguments);
                        return self.call_direct(fid, None, args);
                    }
                }
                // `Error.throwWithStackTrace`, `ArgumentError.checkNotNull`…
                // of the runtime error classes (`runtime_errors.rs`),
                // through the static member of the class with that name.
                if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.ctx.get_resolved(self.unit_id, *inner_target).cloned()
                    && let Some(&f) = self.ctx.program.classes[c.0 as usize].static_members.get(&method_name.sym)
                {
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    if let Some(op) = self.runtime_error_static(f.0 as usize, &evaluated, expr.span) {
                        return op;
                    }
                }
                return self.unsupported(&format!("chamada estática `{m_name}`"), expr.span);
            }
            // `C.name(…)` without resolution (closure body, operand of
            // `throw`): a named constructor or a static method by name.
            if let ExprKind::Identifier(cn) = &ast.expr(*inner_target).kind
                && self.ctx.get_resolved(self.unit_id, *inner_target).is_none()
                && self.lookup_local(cn.sym).is_none()
                && let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.resolve_by_name(cn.sym)
                && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
            {
                let cls = &self.ctx.program.classes[c.0 as usize];
                if let Some(f) = self.constructor_of(c, method_name.sym) {
                    self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                    return self.instantiate(ast, f, &arguments.args, expr.span);
                }
                if let Some(&f) = cls.static_members.get(&method_name.sym) {
                    let fid = f.0 as usize;
                    if self.ctx.program.functions[fid].kind
                        == dartforge_elements::model::FunctionKind::Getter
                    {
                        let callee = self.call_direct(fid, None, Vec::new());
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(callee, &evaluated);
                    }
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    let args = self.match_args(fid, &evaluated);
                    return self.call_direct(fid, None, args);
                }
            }

            // `super.m(…)`: a direct call to the superclass member (P4).
            if matches!(ast.expr(*inner_target).kind, ExprKind::Super) {
                let evaluated = self.evaluate_args(ast, &arguments.args);
                return self.call_super_method(method_name.sym, &evaluated, expr.span);
            }

            let recv_op = self.lower_target(ast, *inner_target);
            if *null_aware {
                self.branch_if_null(&recv_op);
            }

            // Extension member (P4): a direct call with the receiver.
            if let Some(Resolved::ExtensionMember { member, .. }) = resolved_target
                && crate::lower::user_function(self.ctx, member.0 as usize)
            {
                let evaluated = self.evaluate_args(ast, &arguments.args);
                let receiver = self.ctx.get_type_raw(self.unit_id, *inner_target);
                return self.call_extension(
                    recv_op,
                    member.0 as usize,
                    &evaluated,
                    receiver,
                    Some((expr_id, arguments)),
                    expr.span,
                );
            }

            // Extension type member: static, through the non-erased type
            // of the receiver.
            if let Some(m) = self.resolved_member_te(resolved_target.as_ref()) {
                let receiver = self.ctx.get_type_raw(self.unit_id, *inner_target);
                return self.call_member_te(ast, recv_op, m, receiver, expr_id, arguments, expr.span);
            }
            // User class method: through the resolved element, with
            // dispatch on the dynamic class (R7).
            if let Some((_, member)) =
                self.user_member(*target, *inner_target, method_name.sym, false)
            {
                {
                    let field = match member {
                        MemberRef::Variable(v) => Some(v),
                        MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                    };
                    let getter = matches!(member, MemberRef::Function(f)
                        if self.ctx.program.functions[f.0 as usize].kind
                            == dartforge_elements::model::FunctionKind::Getter);
                    if let Some(vid) = field {
                        // A field of function type: reads and calls the value (P1).
                        let v = if crate::lower::is_global(self.ctx, vid) {
                            self.read_global(vid, expr.span)
                        } else {
                            self.read_field_with_late(recv_op, vid, expr.span)
                        };
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(v, &evaluated);
                    }
                    let MemberRef::Function(f) = member else {
                        unreachable!("field handled above")
                    };
                    if getter {
                        let v = self.call_member(recv_op, f.0 as usize, &[], expr.span);
                        let evaluated = self.evaluate_args(ast, &arguments.args);
                        return self.call_function_value(v, &evaluated);
                    }
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    self.arm_tuple(f.0 as usize, expr_id, arguments);
                    return self.call_member(recv_op, f.0 as usize, &evaluated, expr.span);
                }
            }

            // A record field (with a shape) of function type: `r.f(args)`.
            if !self.shapes_with_field(m_name).is_empty() {
                let name = m_name.to_string();
                let r2 = recv_op.clone();
                return self.call_record_field(
                    recv_op,
                    m_name,
                    &mut |s: &mut Self| s.evaluate_args(ast, &arguments.args),
                    &mut |s: &mut Self| {
                        s.call_without_member(ast, expr, target, inner_target, &name, r2.clone(), arguments)
                    },
                );
            }
            return self.call_without_member(ast, expr, target, inner_target, m_name, recv_op, arguments);
        }

        // Any other expression as the target (`f()()`, m['k']!(x),
        // (closure)(x)): its value is called (P1).
        if !matches!(ast.expr(*target).kind, ExprKind::Identifier(_)) {
            let f = self.lower_expr(ast, *target);
            let evaluated = self.evaluate_args(ast, &arguments.args);
            return self.call_function_value(f, &evaluated);
        }
        self.unsupported_call(ast, target, expr)
    }

    /// The expression has a function type (or Function), or is a tear-off.
    fn is_function_value(&self, e: ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return false;
        };
        match self.ctx.table.get(t) {
            dartforge_types::table::Type::Function { .. } => true,
            dartforge_types::table::Type::Interface { class, .. } => {
                self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name) == "Function"
            }
            _ => t == self.ctx.core.dynamic_,
        }
    }

    /// Diagnostic of a call that no path lowered.
    pub(super) fn unsupported_call(
        &mut self,
        ast: &ast::Ast,
        target: &ExprId,
        expr: &ast::Expr,
    ) -> Operand {
        let what = match &ast.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => {
                format!("chamada `{}`", self.ctx.symbol_name(name.sym))
            }
            _ => "chamada de valor de função".to_string(),
        };
        self.unsupported(&what, expr.span)
    }

    /// An SDK method on a receiver that is not a user class (or without a
    /// useful element): the member through the dynamic class of the receiver, by
    /// selector. Written type arguments (`d.m<int>(…)`) go into the tuple of the
    /// call by selector (the generic method, or the `Invocation.typeArguments`
    /// of a `noSuchMethod`).
    pub(super) fn method_by_selector(
        &mut self,
        ast: &ast::Ast,
        m_name: &str,
        recv_op: Operand,
        arguments: &ast::Arguments,
    ) -> Operand {
        let ev = self.evaluate_args(ast, &arguments.args);
        if !arguments.type_args.is_empty() {
            let args = self.type_argument_recipes(&arguments.type_args);
            let tuple = self.recipe_rti(&super::rti::Recipe {
                text: format!("L<{}>", args.text),
                variables: args.variables,
            });
            let lib = self.ctx.program.unit(self.unit_id).library;
            let s = super::sdk_source::selector_string(self.ctx, super::sdk_source::AccessKind::Call, m_name, lib);
            return self.call_by_selector_with_tuple(recv_op, s, &ev, tuple);
        }
        self.call_by_name(recv_op, super::sdk_source::AccessKind::Call, m_name, &ev)
    }

    /// `target.m(args)` when `m` is not a static member of the type of the target: the
    /// member through the dynamic class (a receiver without a useful type), `f.call(…)`, or
    /// the SDK member through the selector.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_without_member(
        &mut self,
        ast: &ast::Ast,
        expr: &ast::Expr,
        target: &ExprId,
        inner_target: &ExprId,
        m_name: &str,
        recv_op: Operand,
        arguments: &ast::Arguments,
    ) -> Operand {
        // `f.call(…)` on a function value.
        if m_name == "call" && self.is_function_value(*inner_target) {
            let evaluated = self.evaluate_args(ast, &arguments.args);
            return self.call_function_value(recv_op, &evaluated);
        }

        let _ = (expr, target);
        self.method_by_selector(ast, m_name, recv_op, arguments)
    }

    /// `prefix.name` with `prefix` from an `import … as prefix`: the element.
    /// `target` is the prefix of an `import … deferred as`.
    fn deferred_prefix(&self, ast: &ast::Ast, target: ExprId) -> bool {
        let ExprKind::Identifier(p) = &ast.expr(target).kind else { return false };
        if !matches!(self.ctx.get_resolved(self.unit_id, target), Some(Resolved::Prefix(_)) | None)
            || self.lookup_local(p.sym).is_some()
        {
            return false;
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        self.ctx.program.library(lib).imports.iter().any(|i| i.deferred && i.prefix == Some(p.sym))
    }

    /// The class whose static members `target.x` reaches: the class
    /// literal `C` (or `p.C`), or an alias `typedef A = C<…>` (specification
    /// §15 "Type Aliases": `A.x` is the static member `x` of `C`; the type
    /// arguments of the alias do not count for static members).
    pub fn static_target_class(&self, target: ExprId) -> Option<dartforge_elements::model::ClassId> {
        use dartforge_elements::model::Element;
        match self.ctx.get_resolved(self.unit_id, target)? {
            Resolved::Element(Element::Class(c)) => Some(*c),
            Resolved::Element(Element::Typedef(td)) => {
                let data = self.ctx.outline.typedefs.get(td.0 as usize)?;
                match self.ctx.table.get(data.target_type) {
                    dartforge_types::table::Type::Interface { class, .. } => Some(*class),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn prefixed_element(
        &self,
        ast: &ast::Ast,
        target: ExprId,
        name: dartforge_intern::SymbolId,
    ) -> Option<dartforge_elements::model::Element> {
        let ExprKind::Identifier(p) = &ast.expr(target).kind else {
            return None;
        };
        match self.ctx.get_resolved(self.unit_id, target) {
            Some(Resolved::Prefix(_)) | None => {}
            _ => return None,
        }
        if self.lookup_local(p.sym).is_some() {
            return None;
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup_prefixed(lib, p.sym, name)?;
        b.getter.or(b.setter)
    }
}
