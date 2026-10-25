//! Assignment to an identifier, a property and an index.
//!
//! Before, only the identifier wrote; `a.b = v` and `a[i] = v` evaluated the value
//! and discarded it — the next read saw the zeroed field (H4 of the plan,
//! `docs/NATIVO-PLANO.md` §6).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind};
use dartforge_frontend::ast::{self, ExprId};
use dartforge_types::resolved::{MemberRef, Resolved};

/// The right-hand side of an assignment: an expression, or the `1` of `++`/`--`.
#[derive(Clone, Copy)]
pub enum Rhs {
    Expr(ExprId),
    One,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn lower_rhs(&mut self, ast: &ast::Ast, rhs: Rhs, as_type: Type) -> Operand {
        match rhs {
            Rhs::Expr(e) => self.lower_expr(ast, e),
            Rhs::One if as_type == Type::F64 => Operand::Constant(Constant::Double(1.0)),
            Rhs::One => Operand::Constant(Constant::Int(1)),
        }
    }

    /// The value to write: `v`, `cur op v`, or `cur ?? v` (`??=`). The current
    /// value stays in `old_value` for the postfix `x++`.
    fn combine(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        cur: Option<Operand>,
        value: Rhs,
    ) -> Operand {
        self.old_value = cur.clone();
        match (op, cur) {
            (ast::AssignOp::Compound(ast::BinaryOp::IfNull), Some(cur)) => {
                let is_null = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cur.clone(), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_new = self.new_block();
                let b_end = self.new_block();
                let origin = self.current_block;
                self.terminate(Terminator::CondBranch {
                    cond: is_null,
                    then_block: b_new,
                    else_block: b_end,
                });
                self.set_block(b_new);
                let ty = self.operand_type(&cur);
                let v = self.lower_rhs(ast, value, ty);
                let v = self.coerce(v, ty);
                let new_end = self.current_block;
                self.terminate(Terminator::Branch(b_end));
                self.set_block(b_end);
                self.emit(
                    Instruction::Phi {
                        incoming: vec![(origin, cur), (new_end, v)],
                        ty,
                    },
                    ty,
                )
            }
            (ast::AssignOp::Compound(b), Some(cur)) => {
                let ty = self.operand_type(&cur);
                // One side excluding int is enough for the Smi join to be impossible.
                // The destination may be a getter/index without a type known here.
                let admits_smi = match value {
                    Rhs::Expr(e) => self.type_admits_smi(self.ctx.get_type(self.unit_id, e)),
                    Rhs::One => true,
                };
                let rhs = self.lower_rhs(ast, value, ty);
                self.lower_binary_op_helper(b, cur, rhs, admits_smi)
            }
            _ => self.lower_rhs(ast, value, Type::Ref),
        }
    }

    /// Assignment to an explicit top-level getter/setter pair. `f` is the resolved
    /// element (getter or setter); the setter comes from the scope of the library
    /// that declares it, and the current (compound) read from the getter.
    #[allow(clippy::too_many_arguments)]
    fn assign_top_level_accessor(
        &mut self,
        f: dartforge_elements::model::FunctionElementId,
        sym: dartforge_intern::SymbolId,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
        scope: Option<dartforge_elements::model::Binding>,
    ) -> Operand {
        let func = &self.ctx.program.functions[f.0 as usize];
        // The other side of the name in the scope of the assignment (`scope`:
        // the unit's, or the import prefix's): the getter and the setter may
        // come from different imports.
        let binding = scope.or_else(|| self.ctx.program.lookup(func.library, sym));
        let setter = if func.kind == FunctionKind::Setter {
            Some(f)
        } else {
            binding.and_then(|b| b.setter).and_then(|el| match el {
                Element::Function(s) => Some(s),
                _ => None,
            })
        };
        let getter = if func.kind == FunctionKind::Setter {
            binding.and_then(|b| b.getter).and_then(|el| match el {
                Element::Function(g) => Some(g),
                _ => None,
            })
        } else {
            Some(f)
        };
        let Some(setter) = setter.filter(|s| super::user_function(self.ctx, s.0 as usize)) else {
            let n = self.ctx.symbol_name(sym).to_string();
            return self.unsupported(&format!("assignment to `{n}` without a setter"), span);
        };
        let cur = match (matches!(op, ast::AssignOp::Compound(_)), getter) {
            (true, Some(g)) => Some(self.call_direct(g.0 as usize, None, Vec::new())),
            (true, None) => {
                let n = self.ctx.symbol_name(sym).to_string();
                return self.unsupported(&format!("read of `{n}` without a getter"), span);
            }
            _ => None,
        };
        let v = self.combine(ast, op, cur, value);
        let args = self.match_args(setter.0 as usize, &[(None, v.clone())]);
        self.call_direct(setter.0 as usize, None, args);
        v
    }

    /// `r.x op= v` with `x` an instance member of an extension (`member`
    /// is the setter, or the getter when the resolution kept the read): the
    /// setter `x=` and, in the compound case, the getter `x` of the same extension, both
    /// with the receiver evaluated once.
    #[allow(clippy::too_many_arguments)]
    fn assign_extension(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        recv: Option<ExprId>,
        member: dartforge_elements::model::FunctionElementId,
        sym: dartforge_intern::SymbolId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        // The setter and the getter of the same owner: the extension, or the extension
        // type (`extension_types.rs`).
        let name = self.ctx.symbol_name(sym).to_string();
        let m = member.0 as usize;
        let setter_key = self.ctx.interner.lookup(&format!("{name}_="));
        let setter = setter_key.and_then(|k| self.accessor_of_same_owner(m, k));
        let getter = self
            .accessor_of_same_owner(m, sym)
            .filter(|&g| self.ctx.program.functions[g.0 as usize].kind == FunctionKind::Getter);
        let Some(setter) = setter else {
            return self.unsupported(&format!("assignment to `{name}` without a setter"), span);
        };
        let (recv_op, receiver) = match recv {
            Some(r) => (
                self.lower_target(ast, r),
                self.ctx.get_type_raw(self.unit_id, r),
            ),
            None => (
                self.this_param
                    .clone()
                    .unwrap_or(Operand::Constant(Constant::Null)),
                self.implicit_receiver(),
            ),
        };
        let cur = if matches!(op, ast::AssignOp::Compound(_)) {
            let Some(g) = getter else {
                return self.unsupported(&format!("read of `{name}` without a getter"), span);
            };
            Some(self.call_extension(recv_op.clone(), g.0 as usize, &[], receiver, None, span))
        } else {
            None
        };
        let v = self.combine(ast, op, cur, value);
        self.call_extension(
            recv_op,
            setter.0 as usize,
            &[(None, v.clone())],
            receiver,
            None,
            span,
        );
        v
    }

    /// `a?.b op= v` / `a?[i] op= v` (§17.23): `a` is evaluated once; if it is
    /// null, nothing else is evaluated and the value is null; otherwise, the ordinary
    /// assignment over the value of `a`. The `?.` chain of `a` (`x?.y?.b = v`)
    /// branches to the same null. A `?.` before the last link also short-circuits
    /// the whole assignment (`a?.b.c = v`, `a?.b[i] = v`: with a null `a`, not even
    /// `v` is evaluated); there only the receiver chain branches (`last_null`
    /// false: the last link is ordinary).
    #[allow(clippy::too_many_arguments)]
    fn assign_with_null_aware(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        recv: ExprId,
        last_null: bool,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let exit = self.new_block();
        let saved_chain = self.null_chain.replace((exit, Vec::new()));
        let recv_op = self.lower_target(ast, recv);
        if last_null {
            self.branch_if_null(&recv_op);
        }
        self.receiver_ready = Some((recv, recv_op));
        self.handled_null_aware = Some(target);
        let mut v = self.lower_assignment(ast, op, target, value, span);
        self.receiver_ready = None;
        // The old value (`a?.x++`) also goes through the join: null when the
        // chain branched.
        let mut old = self.old_value.take();
        let (exit, mut entries) =
            std::mem::replace(&mut self.null_chain, saved_chain).expect("cadeia aberta");
        let mut old_entries: Vec<(BlockId, Operand)> = entries
            .iter()
            .map(|(b, _)| (*b, Operand::Constant(Constant::Null)))
            .collect();
        if !self.is_terminated() {
            v = self.coerce(v, Type::Ref);
            if let Some(a) = old.take() {
                let a = self.coerce(a, Type::Ref);
                old_entries.push((self.current_block, a));
            }
            entries.push((self.current_block, v));
            self.terminate(Terminator::Branch(exit));
        }
        self.set_block(exit);
        if entries.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        if old_entries.len() == entries.len() {
            self.old_value = Some(self.emit(
                Instruction::Phi {
                    incoming: old_entries,
                    ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `v = e` and `v op= e` on an unboxed SIMD local: the value and the operation
    /// as vectors; an operator without a vector form goes through the box.
    fn assign_simd_local(
        &mut self,
        ast: &ast::Ast,
        sym: dartforge_intern::SymbolId,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let k = self.lookup_local(sym).map(|l| l.ty).expect("local SIMD");
        let v = match (op, value) {
            (ast::AssignOp::Assign, Rhs::Expr(e)) => self.lower_simd(ast, e, k),
            (ast::AssignOp::Compound(b), Rhs::Expr(e)) => {
                let cur = self.read_local_by_name(sym).expect("local SIMD");
                let simd_op = match (b, k) {
                    (ast::BinaryOp::Add, _) => Some(OpSimd::Add),
                    (ast::BinaryOp::Sub, _) => Some(OpSimd::Sub),
                    (ast::BinaryOp::Mul, Type::V4F32 | Type::V2F64) => Some(OpSimd::Mul),
                    (ast::BinaryOp::Div, Type::V4F32 | Type::V2F64) => Some(OpSimd::Div),
                    (ast::BinaryOp::BitAnd, Type::V4I32) => Some(OpSimd::And),
                    (ast::BinaryOp::BitOr, Type::V4I32) => Some(OpSimd::Or),
                    (ast::BinaryOp::BitXor, Type::V4I32) => Some(OpSimd::Xor),
                    _ => None,
                };
                match simd_op {
                    Some(o) => {
                        let rhs = self.lower_simd(ast, e, k);
                        self.emit(
                            Instruction::Simd {
                                op: o,
                                args: vec![cur, rhs],
                            },
                            k,
                        )
                    }
                    None => {
                        let boxed = self.coerce(cur, Type::Ref);
                        let v = self.combine(ast, op, Some(boxed), Rhs::Expr(e));
                        self.coerce(v, k)
                    }
                }
            }
            (op, value) => {
                let cur = matches!(op, ast::AssignOp::Compound(_))
                    .then(|| {
                        self.read_local_by_name(sym)
                            .map(|c| self.coerce(c, Type::Ref))
                    })
                    .flatten();
                let v = self.combine(ast, op, cur, value);
                self.coerce(v, k)
            }
        };
        match self.store_local(sym, v) {
            Some(v) => v,
            None => self.unsupported("assignment to an unknown local", span),
        }
    }

    /// Assignment. Dart evaluation order: receiver, index, current read
    /// (if compound), value, write.
    pub fn lower_assignment(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composite = matches!(op, ast::AssignOp::Compound(_));
        match &ast.expr(target).kind {
            ast::ExprKind::Identifier(name) => {
                let sym = name.sym;
                let resolved = match self.ctx.get_resolved(self.unit_id, target).cloned() {
                    // No resolution (closure body) and no local: through the
                    // lexical scope (class member, library top level).
                    None if self.lookup_local(sym).is_none() => self.resolve_by_name(sym),
                    r => r,
                };
                match resolved {
                    // Extension type setter with the implicit `this`.
                    r @ Some(Resolved::Member { .. })
                        if self.resolved_member_te(r.as_ref()).is_some() =>
                    {
                        match self.resolved_member_te(r.as_ref()) {
                            Some(MemberRef::Function(f))
                                if !self.is_representation(MemberRef::Function(f)) =>
                            {
                                self.assign_extension(ast, op, None, f, sym, value, span)
                            }
                            _ => self.unsupported(
                                "assignment to the representation of an extension type",
                                span,
                            ),
                        }
                    }
                    Some(Resolved::Member { member, .. }) => {
                        self.assign_member(None, member, ast, op, value, span)
                    }
                    Some(Resolved::ExtensionMember { member, .. })
                        if !self.ctx.program.functions[member.0 as usize].static_ =>
                    {
                        self.assign_extension(ast, op, None, member, sym, value, span)
                    }
                    Some(Resolved::Element(Element::Variable(vid))) => {
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        self.store_global(vid, v, span)
                    }
                    Some(Resolved::Element(Element::Function(f)))
                        if self.ctx.program.functions[f.0 as usize].variable.is_some() =>
                    {
                        let vid = self.ctx.program.functions[f.0 as usize]
                            .variable
                            .expect("acessor");
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        self.store_global(vid, v, span)
                    }
                    // Explicit top-level getter/setter (`set exitCode(int)`):
                    // the resolved element may be the getter; the setter is the
                    // other side of the same name in the scope.
                    Some(Resolved::Element(Element::Function(f))) => {
                        let scope = self.ctx.program.lookup_in_unit(self.unit_id, sym);
                        self.assign_top_level_accessor(f, sym, ast, op, value, span, scope)
                    }
                    _ if self.lookup_local(sym).is_some_and(|l| l.ty.is_vector()) => {
                        // Unboxed SIMD local (`simd.rs`): the value comes as a vector.
                        self.assign_simd_local(ast, sym, op, value, span)
                    }
                    _ => {
                        let cur = if composite {
                            self.read_local_by_name(sym)
                        } else {
                            None
                        };
                        if composite && cur.is_none() {
                            return self
                                .unsupported("compound assignment to an unknown local", span);
                        }
                        let v = self.combine(ast, op, cur, value);
                        match self.store_local(sym, v) {
                            Some(v) => v,
                            None => self.unsupported("assignment to an unknown local", span),
                        }
                    }
                }
            }
            ast::ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                if (*null_aware || Self::chain_has_null_aware(ast, *recv))
                    && self.handled_null_aware != Some(target)
                {
                    return self.assign_with_null_aware(
                        ast,
                        op,
                        target,
                        *recv,
                        *null_aware,
                        value,
                        span,
                    );
                }
                self.handled_null_aware = None;
                // `prefix.x = v`: a top-level variable imported with a prefix.
                if let Some(el) = self.prefixed_element(ast, *recv, name.sym) {
                    let vid = match el {
                        Element::Variable(v) => Some(v),
                        Element::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                        _ => None,
                    };
                    if let Some(vid) = vid {
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        return self.store_global(vid, v, span);
                    }
                    // Explicit top-level getter/setter of the library of the
                    // prefix (`intl.defaultLocale = l`).
                    if let Element::Function(f) = el {
                        let scope = match &ast.expr(*recv).kind {
                            ast::ExprKind::Identifier(p) => self
                                .ctx
                                .program
                                .lookup_prefixed_in_unit(self.unit_id, p.sym, name.sym),
                            _ => None,
                        };
                        return self
                            .assign_top_level_accessor(f, name.sym, ast, op, value, span, scope);
                    }
                }
                // `super.x = v` (P4).
                if matches!(ast.expr(*recv).kind, ast::ExprKind::Super) {
                    let cur = if composite {
                        Some(self.read_super(name.sym, span))
                    } else {
                        None
                    };
                    let v = self.combine(ast, op, cur, value);
                    return self.store_super(name.sym, v, span);
                }
                // `E.x = v`: a static extension field (or a static setter).
                if matches!(
                    self.ctx.get_resolved(self.unit_id, *recv),
                    Some(Resolved::Element(Element::Extension(_)))
                ) {
                    match self.ctx.get_resolved(self.unit_id, target).cloned() {
                        Some(Resolved::Element(Element::Variable(vid))) => {
                            let cur = composite.then(|| self.read_global(vid, span));
                            let v = self.combine(ast, op, cur, value);
                            return self.store_global(vid, v, span);
                        }
                        Some(Resolved::ExtensionMember { member, .. })
                            if self.ctx.program.functions[member.0 as usize].kind
                                == FunctionKind::Setter =>
                        {
                            let e = self.ctx.program.functions[member.0 as usize]
                                .extension
                                .expect("extension");
                            let getter = self.ctx.program.extensions[e.0 as usize]
                                .static_members
                                .get(&name.sym)
                                .copied();
                            let cur = match (composite, getter) {
                                (true, Some(g)) => {
                                    Some(self.call_direct(g.0 as usize, None, Vec::new()))
                                }
                                (true, None) => {
                                    return self.unsupported("read without a static getter", span);
                                }
                                _ => None,
                            };
                            let v = self.combine(ast, op, cur, value);
                            self.call_direct(member.0 as usize, None, vec![v.clone()]);
                            return v;
                        }
                        _ => {
                            let n = self.ctx.symbol_name(name.sym).to_string();
                            return self
                                .unsupported(&format!("assignment to extension `{n}`"), span);
                        }
                    }
                }
                // Extension type setter: like the extension one.
                if let Some(m) = self.member_te(target) {
                    return match m {
                        MemberRef::Function(f) if !self.is_representation(m) => {
                            self.assign_extension(ast, op, Some(*recv), f, name.sym, value, span)
                        }
                        _ => self.unsupported(
                            "assignment to the representation of an extension type",
                            span,
                        ),
                    };
                }
                // Extension setter (P4): a direct call with the receiver.
                if let Some(Resolved::ExtensionMember { member, .. }) =
                    self.ctx.get_resolved(self.unit_id, target).cloned()
                    && !self.ctx.program.functions[member.0 as usize].static_
                    && super::user_function(self.ctx, member.0 as usize)
                {
                    return self.assign_extension(
                        ast,
                        op,
                        Some(*recv),
                        member,
                        name.sym,
                        value,
                        span,
                    );
                }
                // `C.x = v` (or `A.x = v` with `typedef A = C<…>`).
                let target_is_class = self.static_target_class(*recv).is_some();
                if target_is_class {
                    let Some(Resolved::Member { class, member, .. }) =
                        self.ctx.get_resolved(self.unit_id, target).cloned()
                    else {
                        let n = self.ctx.symbol_name(name.sym).to_string();
                        return self.unsupported(&format!("assignment to `{n}`"), span);
                    };
                    let member = if matches!(member, MemberRef::Function(f)
                        if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                            && self.ctx.program.functions[f.0 as usize].variable.is_none())
                    {
                        let key = format!("{}_=", self.ctx.symbol_name(name.sym));
                        self.ctx
                            .interner
                            .lookup(&key)
                            .and_then(|s| {
                                self.ctx.program.classes[class.0 as usize]
                                    .static_members
                                    .get(&s)
                                    .copied()
                            })
                            .map(MemberRef::Function)
                            .unwrap_or(member)
                    } else {
                        member
                    };
                    return self.assign_member(None, member, ast, op, value, span);
                }
                let Some((_, member)) = self.user_member(target, *recv, name.sym, true) else {
                    let n = self.ctx.symbol_name(name.sym).to_string();
                    if self.dynamic_receiver(*recv) {
                        // Even a nonexistent setter goes through the selector: the
                        // runtime produces the `NoSuchMethodError`. The RHS is
                        // evaluated after the receiver, as in Dart.
                        let recv_op = self.lower_expr(ast, *recv);
                        let cur = composite.then(|| {
                            self.call_by_name(
                                recv_op.clone(),
                                super::sdk_source::AccessKind::Read,
                                &n,
                                &[],
                            )
                        });
                        let v = self.combine(ast, op, cur, value);
                        self.call_by_name(
                            recv_op,
                            super::sdk_source::AccessKind::Write,
                            &n,
                            &[(None, v.clone())],
                        );
                        return v;
                    }
                    return self.unsupported(&format!("assignment to `{n}`"), span);
                };
                let recv_op = self.lower_expr(ast, *recv);
                self.assign_member(Some(recv_op), member, ast, op, value, span)
            }
            ast::ExprKind::Index {
                target: t,
                index,
                null_aware,
            } => {
                if (*null_aware || Self::chain_has_null_aware(ast, *t))
                    && self.handled_null_aware != Some(target)
                {
                    return self.assign_with_null_aware(
                        ast,
                        op,
                        target,
                        *t,
                        *null_aware,
                        value,
                        span,
                    );
                }
                self.handled_null_aware = None;
                // `super[i] op= v`: the `[]`/`[]=` operators of the superclass.
                if matches!(ast.expr(*t).kind, ast::ExprKind::Super) {
                    let i_op = self.lower_expr(ast, *index);
                    let (Some(get), Some(set)) = (
                        self.ctx.interner.lookup("[]"),
                        self.ctx.interner.lookup("[]="),
                    ) else {
                        return self.unsupported("`super[]=` without an operator", span);
                    };
                    let cur = composite
                        .then(|| self.call_super_method(get, &[(None, i_op.clone())], span));
                    let v = self.combine(ast, op, cur, value);
                    self.call_super_method(set, &[(None, i_op), (None, v.clone())], span);
                    return v;
                }
                let t_op = self.lower_expr(ast, *t);
                let i_op = self.lower_expr(ast, *index);
                self.implicit_operator_cast(*index, &i_op, *t, "[]=", 0);
                // Extension `[]=` (inference resolves the index to the
                // `[]` of the extension; the `[]=` is that of the same extension).
                if let Some(get) = self.extension_operator(target) {
                    let set = self
                        .ctx
                        .interner
                        .lookup("[]=")
                        .and_then(|k| self.accessor_of_same_owner(get, k));
                    let Some(set) = set else {
                        return self.unsupported("missing extension operator []=", span);
                    };
                    let receiver = self.ctx.get_type_raw(self.unit_id, *t);
                    let cur = composite.then(|| {
                        self.call_extension(
                            t_op.clone(),
                            get,
                            &[(None, i_op.clone())],
                            receiver,
                            None,
                            span,
                        )
                    });
                    let v = self.combine(ast, op, cur, value);
                    self.call_extension(
                        t_op,
                        set.0 as usize,
                        &[(None, i_op), (None, v.clone())],
                        receiver,
                        None,
                        span,
                    );
                    return v;
                }
                if let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *t))
                    && self.operand_type(&i_op) == Type::I64
                {
                    // Numeric typed list or `List<E>`: direct read and
                    // write (`typed_items.rs`).
                    let repr = self.expression_repr(target).unwrap_or(Type::Ref);
                    let fixed = self.fixed_list_of(ast, *t);
                    self.access_fixed = fixed.clone();
                    let proven = self.proven_of(ast, *t, Some(*index));
                    self.access_proven = proven.clone();
                    let cur = if composite {
                        self.read_indexed(t_op.clone(), i_op.clone(), l, repr)
                    } else {
                        None
                    };
                    self.access_proven = None;
                    self.access_fixed = None;
                    // SIMD list with a SIMD value of the same type: the value goes
                    // as a vector, and only the slow path (the SDK `[]=`) puts it
                    // in the box — before, `v4[i] = a + b` allocated a box
                    // per write (docs/SIMD-NATIVO.md).
                    let simd = match (l, value) {
                        (super::typed_items::Indexable::Simd { k, .. }, Rhs::Expr(e))
                            if !composite && self.expr_simd(e) == Some(k) =>
                        {
                            Some((e, k))
                        }
                        _ => None,
                    };
                    let v = match simd {
                        Some((e, k)) => self.lower_simd(ast, e, k),
                        None => self.combine(ast, op, cur, value),
                    };
                    if !composite
                        && simd.is_none()
                        && let Rhs::Expr(e) = value
                    {
                        self.implicit_operator_cast(e, &v, *t, "[]=", 1);
                    }
                    self.access_fixed = fixed;
                    self.access_proven = proven;
                    let stored = self.store_indexed(t_op.clone(), i_op.clone(), v.clone(), l);
                    self.access_proven = None;
                    self.access_fixed = None;
                    if stored {
                        return v;
                    }
                    let v = if simd.is_some() {
                        self.coerce(v, Type::Ref)
                    } else {
                        v
                    };
                    self.call_by_name_typed(
                        t_op,
                        *t,
                        super::sdk_source::AccessKind::Call,
                        "[]=",
                        &[(None, i_op), (None, v.clone())],
                    );
                    return v;
                }
                // `[]=` (and the `[]` of the compound case) of a program class: through the
                // members.
                if let Some(set) = self.program_operator(*t, "[]=") {
                    let cur = if composite {
                        let Some(get) = self.program_operator(*t, "[]") else {
                            return self.unsupported("missing operator []", span);
                        };
                        Some(self.call_member(t_op.clone(), get, &[(None, i_op.clone())], span))
                    } else {
                        None
                    };
                    let v = self.combine(ast, op, cur, value);
                    self.call_member(t_op, set, &[(None, i_op), (None, v.clone())], span);
                    return v;
                }
                // SDK `[]`/`[]=` through the dynamic class.
                use super::sdk_source::AccessKind;
                let cur = composite.then(|| {
                    self.call_by_name_typed(
                        t_op.clone(),
                        *t,
                        AccessKind::Call,
                        "[]",
                        &[(None, i_op.clone())],
                    )
                });
                let v = self.combine(ast, op, cur, value);
                if !composite && let Rhs::Expr(e) = value {
                    self.implicit_operator_cast(e, &v, *t, "[]=", 1);
                }
                self.call_by_name_typed(
                    t_op,
                    *t,
                    AccessKind::Call,
                    "[]=",
                    &[(None, i_op), (None, v.clone())],
                );
                v
            }
            _ => self.unsupported("assignment target", span),
        }
    }

    /// Writes into a member: a field (of `recv`, or of `this` when `recv` is
    /// None), a static global, or a setter.
    fn assign_member(
        &mut self,
        recv: Option<Operand>,
        member: MemberRef,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composite = matches!(op, ast::AssignOp::Compound(_));
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                let cur = if composite {
                    Some(self.read_global(vid, span))
                } else {
                    None
                };
                let v = self.combine(ast, op, cur, value);
                return self.store_global(vid, v, span);
            }
            let Some(obj) = recv.or_else(|| self.this_param.clone()) else {
                return self.unsupported("instance field outside an instance member", span);
            };
            let cur = if composite {
                Some(self.read_field_with_late(obj.clone(), vid, span))
            } else {
                None
            };
            let v = self.combine(ast, op, cur, value);
            if !self.store_source_field(obj.clone(), vid, v.clone()) {
                self.store_field(obj, vid, v.clone(), span);
            }
            return v;
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Setter || !super::user_function(self.ctx, fid) {
            return self.unsupported(
                "assignment to a member that is neither a field nor a setter",
                span,
            );
        }
        let static_ty = func.static_;
        let name = self.ctx.symbol_name(func.name).to_string();
        let cid = func.class;
        let obj = if static_ty {
            None
        } else {
            recv.or_else(|| self.this_param.clone())
        };
        let cur = if composite {
            let getter = cid.and_then(|cid| {
                if static_ty {
                    let sym = self.ctx.interner.lookup(&name)?;
                    self.ctx.program.classes[cid.0 as usize]
                        .static_members
                        .get(&sym)
                        .copied()
                } else {
                    // The getter may come from a superinterface (`List.last=`
                    // with the `last` of `Iterable`): the lookup is that of the interface
                    // of the type, not only that of the superclass chain.
                    self.member_in_class(cid, &name)
                        .map(|fid| dartforge_elements::model::FunctionElementId(fid as u32))
                        .or_else(|| interface_getter(self.ctx, cid, &name))
                }
            });
            let Some(getter) = getter else {
                return self.unsupported("missing getter for compound assignment", span);
            };
            let getter_member = MemberRef::Function(getter);
            if static_ty {
                Some(self.read_static_member(getter_member, span))
            } else {
                let Some(receiver) = obj.clone() else {
                    return self.unsupported("setter outside an instance member", span);
                };
                let getter_fid = getter.0 as usize;
                if let Some(vid) = self.ctx.program.functions[getter_fid].variable {
                    Some(self.read_field_with_late(receiver, vid, span))
                } else {
                    Some(self.call_member(receiver, getter_fid, &[], span))
                }
            }
        } else {
            None
        };
        let v = self.combine(ast, op, cur, value);
        if static_ty {
            let args = self.match_args(fid, &[(None, v.clone())]);
            self.call_direct(fid, None, args);
            return v;
        }
        let Some(obj) = obj else {
            return self.unsupported("setter outside an instance member", span);
        };
        self.call_member(obj, fid, &[(None, v.clone())], span);
        v
    }
}

/// The getter `name` of the interface of class `cid`: the class itself, the
/// superclasses, the mixins and the superinterfaces, breadth-first (the first
/// declaration found; in a well-formed interface the others have the same
/// signature).
fn interface_getter(
    ctx: &crate::context::Context,
    cid: dartforge_elements::model::ClassId,
    name: &str,
) -> Option<dartforge_elements::model::FunctionElementId> {
    let sym = ctx.interner.lookup(name)?;
    let mut queue = std::collections::VecDeque::from([cid]);
    let mut seen_set = std::collections::HashSet::new();
    while let Some(c) = queue.pop_front() {
        if !seen_set.insert(c) {
            continue;
        }
        let class = &ctx.program.classes[c.0 as usize];
        if let Some(&f) = class.instance_members.get(&sym)
            && ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
        {
            return Some(f);
        }
        queue.extend(class.supertype_class);
        queue.extend(class.mixin_classes.iter().copied());
        queue.extend(class.interface_classes.iter().copied());
        queue.extend(class.on_classes.iter().copied());
    }
    None
}
