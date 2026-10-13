//! Expressions: literals, identifiers, operators, short-circuit,
//! conditional, `?.` chains, properties, indexing, collections, `is`/`as`.
//! Calls are in `calls.rs`; an SDK member without a useful element goes
//! through the dynamic class of the receiver (`FnBuilder::property_by_selector`).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Lowers the three short-circuit operators of Dart: &&, || and ??.
    ///
    /// The semantics is in the language specification: in `a && b`, `b` is only
    /// evaluated when `a` is true, in `a || b` only when `a` is false, and
    /// in `a ?? b` only when `a` is null. It is not an optimization: the right side may
    /// have a side effect (or throw), and evaluating it early changes the program.
    ///
    /// Before this function, && and || fell into the `_` arm of the match of binary
    /// operators and became the constant 0 — that is, every compound condition of the
    /// corpus was false.
    fn lower_short_circuit(
        &mut self,
        ast: &ast::Ast,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
        expr_id: ExprId,
    ) -> Operand {
        let lop = self.lower_expr(ast, left);
        if op == BinaryOp::IfNull {
            return self.lower_if_null(ast, lop, right, expr_id);
        }
        let cond = self.to_bool(lop);
        // The origin block is the one that ends with the branch — `to_bool` may
        // have opened blocks (the `Unbox` of a boxed `bool`).
        let left_block = self.current_block;
        let right_block = self.new_block();
        let end_block = self.new_block();
        if op == BinaryOp::And {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: right_block,
                else_block: end_block,
            });
        } else {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: end_block,
                else_block: right_block,
            });
        }

        self.set_block(right_block);
        let rop = self.lower_expr(ast, right);
        let rop = self.to_bool(rop);
        let right_block_end = self.current_block;
        let right_reaches = !self.is_terminated();
        if right_reaches {
            self.terminate(Terminator::Branch(end_block));
        }

        self.set_block(end_block);
        // The value that comes from the left side when it decides on its own: in `&&`
        // the left only skips the right when false, in `||` when true.
        let from_left = Operand::Constant(Constant::Bool(op != BinaryOp::And));
        if !right_reaches {
            return from_left;
        }
        self.emit(
            Instruction::Phi {
                incoming: vec![(left_block, from_left), (right_block_end, rop)],
                ty: Type::I1,
            },
            Type::I1,
        )
    }

    /// `a ?? b`: `b` is only evaluated when `a` is null. The two entries of the
    /// phi come out in the representation of the result (R4) — the left one in a
    /// block of its own, because the `Unbox` of `a` only holds when it is not null.
    fn lower_if_null(
        &mut self,
        ast: &ast::Ast,
        lop: Operand,
        right: ExprId,
        expr_id: ExprId,
    ) -> Operand {
        if self.operand_type(&lop) != Type::Ref {
            // A scalar is never null: the right side is dead code.
            return lop;
        }
        let ty = self.expression_repr(expr_id).unwrap_or(Type::Ref);
        let is_null = self.emit(
            Instruction::ICmp(ICmpOp::Eq, lop.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let right_block = self.new_block();
        let nonnull_block = self.new_block();
        let end_block = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: is_null,
            then_block: right_block,
            else_block: nonnull_block,
        });
        let mut entries = Vec::new();
        self.set_block(nonnull_block);
        let v = self.coerce(lop, ty);
        entries.push((self.current_block, v));
        self.terminate(Terminator::Branch(end_block));
        self.set_block(right_block);
        let r = self.lower_expr(ast, right);
        if !self.is_terminated() {
            let r = self.coerce(r, ty);
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(end_block));
            }
        }
        self.set_block(end_block);
        if entries.len() == 1 {
            return entries.pop().expect("uma entrada").1;
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty,
            },
            ty,
        )
    }

    /// A link of an access chain: `a.b`, `a[i]`, `a.m()`.
    fn is_link(ast: &ast::Ast, e: ExprId) -> bool {
        match &ast.expr(e).kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } => true,
            // `a?.b!.c`: the `!` is a selector of the chain (§17.23), it does not end it.
            ExprKind::Unary { op: UnaryOp::NullAssert, .. } => true,
            ExprKind::Call { target, .. } => {
                matches!(ast.expr(*target).kind, ExprKind::Property { .. })
            }
            _ => false,
        }
    }

    /// Does the chain that ends in `e` have some `?.`/`?[`?
    pub(crate) fn chain_has_null_aware(ast: &ast::Ast, e: ExprId) -> bool {
        let mut current = e;
        loop {
            match &ast.expr(current).kind {
                ExprKind::Property {
                    target, null_aware, ..
                }
                | ExprKind::Index {
                    target, null_aware, ..
                } => {
                    if *null_aware {
                        return true;
                    }
                    current = *target;
                }
                ExprKind::Call { target, .. }
                    if matches!(ast.expr(*target).kind, ExprKind::Property { .. }) =>
                {
                    current = *target;
                }
                ExprKind::Unary { op: UnaryOp::NullAssert, operand } => {
                    current = *operand;
                }
                _ => return false,
            }
        }
    }

    /// Lowers the receiver of a link keeping the current `?.` chain (N3).
    pub fn lower_target(&mut self, ast: &ast::Ast, alvo: ExprId) -> Operand {
        self.continue_chain = true;
        self.lower_expr(ast, alvo)
    }

    /// `?.`: if the receiver is null, the whole chain is null (N3).
    pub fn branch_if_null(&mut self, recv: &Operand) {
        let Some((exit, _)) = self.null_chain.as_ref() else {
            return;
        };
        let exit = *exit;
        let is_null = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                recv.clone(),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        );
        let follows = self.new_block();
        let origin = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: is_null,
            then_block: exit,
            else_block: follows,
        });
        if let Some((_, entries)) = self.null_chain.as_mut() {
            entries.push((origin, Operand::Constant(Constant::Null)));
        }
        self.set_block(follows);
    }

    pub fn lower_expr(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        self.lower_expr_with(ast, expr_id, false)
    }

    /// The expression of a statement (`e;`): the value is discarded, so it is not
    /// boxed at the end (a SIMD `acc = acc + v` or an `i++` do not allocate the
    /// box nobody reads). The other coercions stay, since they may fail.
    pub fn lower_expr_discarded(&mut self, ast: &ast::Ast, expr_id: ExprId) {
        self.lower_expr_with(ast, expr_id, true);
    }

    /// J05 and the symbolic stack trace (§13.14): the instructions of the expression carry the
    /// position the VM gives it (`expression_position`); on leaving, the outer one comes
    /// back (the coercions of the user belong to the user).
    fn lower_expr_with(&mut self, ast: &ast::Ast, expr_id: ExprId, discarded: bool) -> Operand {
        if self.ctx.debug_info.is_none() {
            return self.lower_expr_with_in_place(ast, expr_id, discarded);
        }
        let previous = self.position;
        if let Some(offset) = expression_position(ast, &self.ctx.program.unit(self.unit_id).source, expr_id) {
            self.mark_position(ast, offset);
        }
        let r = self.lower_expr_with_in_place(ast, expr_id, discarded);
        self.position = previous;
        r
    }

    fn lower_expr_with_in_place(&mut self, ast: &ast::Ast, expr_id: ExprId, discarded: bool) -> Operand {
        if self.receiver_ready.as_ref().is_some_and(|(e, _)| *e == expr_id) {
            self.continue_chain = false;
            return self.receiver_ready.take().expect("verificado acima").1;
        }
        // A tear-off of a generic function in a non-generic function context
        // (`int Function(int) f = id;`): the implicit instantiation that
        // inference chose, like that of `id<int>` — the closure has the
        // instantiated type and carries the type arguments.
        if matches!(ast.expr(expr_id).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
            && let Some(args) = self.ctx.bodies.units[self.unit_id.0 as usize]
                .instanciacao_de_tearoff(expr_id)
                .map(<[_]>::to_vec)
            && let Some(fid) = self.target_generic_function(ast, expr_id)
        {
            let mut r = super::rti::Recipe { text: "L<".to_string(), variables: false };
            for (i, t) in args.iter().enumerate() {
                if i > 0 {
                    r.text.push(',');
                }
                let x = self.type_recipe(*t);
                r.text.push_str(&x.text);
                r.variables |= x.variables;
            }
            r.text.push('>');
            let tuple = self.recipe_rti(&r);
            let typ = self.ctx.get_type(self.unit_id, expr_id);
            return self.instantiated_tearoff(fid, tuple, typ);
        }
        let proceed = std::mem::replace(&mut self.continue_chain, false);
        let saved = if proceed {
            None
        } else {
            self.null_chain.take()
        };
        let root =
            !proceed && Self::is_link(ast, expr_id) && Self::chain_has_null_aware(ast, expr_id);
        if root {
            let exit = self.new_block();
            self.null_chain = Some((exit, Vec::new()));
        }
        let mut v = self.lower_expr_internal(ast, expr_id);
        if root {
            let (exit, mut entries) = self.null_chain.take().expect("cadeia aberta");
            // The `?.` chain is null or the value: always a `Ref` at the join.
            let ty = Type::Ref;
            if !self.is_terminated() {
                if self.operand_type(&v) != Type::Void {
                    v = self.coerce(v, Type::Ref);
                }
                entries.push((self.current_block, v.clone()));
                self.terminate(Terminator::Branch(exit));
            }
            self.set_block(exit);
            v = if entries.is_empty() {
                Operand::Constant(Constant::Null)
            } else {
                self.emit(
                    Instruction::Phi {
                        incoming: entries,
                        ty,
                    },
                    ty,
                )
            };
        }
        if !proceed {
            self.null_chain = saved;
        }
        // R5: the expression comes out in the representation of its static type. A
        // `dynamic` (or absent) type forces nothing: the operand stays in the
        // representation it was produced in, and each boundary coerces by the
        // real type of the operand.
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
            if t != self.ctx.core.dynamic_ && !self.ctx.is_void(t) && !self.is_terminated() {
                let r = self.ctx.to_hir_type(t);
                if !matches!(r, Type::Void)
                    && !matches!(self.operand_type(&v), Type::Void)
                    && !(discarded && r == Type::Ref)
                {
                    v = self.coerce(v, r);
                }
            }
        }
        v
    }

    /// The expression without the final coercion to the static type (an unboxed SIMD local
    /// comes out as a vector): for `simd.rs`.
    pub(super) fn lower_expr_raw(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        self.lower_expr_internal(ast, expr_id)
    }

    fn lower_expr_internal(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let expr = ast.expr(expr_id);
        // Canonical `const` (P3).
        if let Some(op) = self.canonical_constant(ast, expr_id) {
            return op;
        }
        // A SIMD operator, method, getter or constructor with an unboxed form
        // (`simd.rs`): the vector comes out in the box of the static type (end of
        // `lower_expr`), the scalar directly.
        if let Some(op) = self.simd_expression(ast, expr_id) {
            return op;
        }
        // An inline `int`/`double` member (`intrinsics.rs`).
        if let Some(op) = self.intrinsic_expression(ast, expr_id) {
            return op;
        }
        match &expr.kind {
            ExprKind::Int(span) => {
                let raw = &self.source()[span.start as usize..span.end as usize];
                let text = raw.replace('_', "");
                let val: i64 = if let Some(hex) =
                    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
                {
                    u64::from_str_radix(hex, 16).unwrap_or(0) as i64
                } else {
                    // As a `u64`: `9223372036854775808` is only valid under
                    // unary minus (the front end rejects it elsewhere), and the
                    // two's complement gives `-9223372036854775808`.
                    text.parse::<u64>().map_or(0, |v| v as i64)
                };
                self.emit(Instruction::Const(Constant::Int(val)), Type::I64)
            }
            ExprKind::Double(span) => {
                let raw = &self.source()[span.start as usize..span.end as usize];
                let val: f64 = raw.replace('_', "").parse().unwrap_or(0.0);
                self.emit(Instruction::Const(Constant::Double(val)), Type::F64)
            }
            ExprKind::Bool(b) => self.emit(Instruction::Const(Constant::Bool(*b)), Type::I1),
            ExprKind::Null => self.emit(Instruction::Const(Constant::Null), Type::Ref),
            ExprKind::Symbol(names) => {
                let name = self.symbol_literal_name(names);
                let Some(class) = self.ctx.sdk_class("_internal", "Symbol") else {
                    return self.unsupported("literal de símbolo sem dart:_internal", expr.span);
                };
                let Some(empty) = self.ctx.interner.lookup("") else {
                    return self.unsupported("construtor de Symbol ausente", expr.span);
                };
                let Some(&ctor) = self.ctx.program.classes[class.0 as usize].constructors.get(&empty) else {
                    return self.unsupported("construtor de Symbol ausente", expr.span);
                };
                let text = self.emit(Instruction::Const(Constant::String(name)), Type::Ref);
                self.instantiate_evaluated(ctor, &[(None, text)], expr.span)
            }
            ExprKind::String(str_lit) => {
                if let Some(text) = str_lit.constant_value() {
                    self.emit(Instruction::Const(Constant::StringWtf8(text.as_bytes().to_vec())), Type::Ref)
                } else {
                    let mut parts: Vec<Operand> = Vec::with_capacity(str_lit.parts.len());
                    for part in &str_lit.parts {
                        let part_op = match part {
                            ast::StringPart::Text(t) => {
                                self.emit(Instruction::Const(Constant::StringWtf8(t.as_bytes().to_vec())), Type::Ref)
                            }
                            ast::StringPart::Interpolation(sub_expr) => {
                                let raw_op = self.lower_expr(ast, *sub_expr);
                                let raw_ty = self.operand_type(&raw_op);
                                match raw_ty {
                                    // The `int` goes unboxed into `ConcatStrings`: the
                                    // digits go straight into the joined text, without the
                                    // intermediate string (the VM's `_interpolate`
                                    // does not keep it either).
                                    Type::I64 if str_lit.parts.len() > 1 => raw_op,
                                    Type::I64 => self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_to_string_i64".to_string(),
                                            args: vec![(raw_op, Type::I64)],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    ),
                                    Type::F64 => self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_to_string_f64".to_string(),
                                            args: vec![(raw_op, Type::F64)],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    ),
                                    Type::I1 | Type::I8 => {
                                        let b_i8 = if raw_ty == Type::I1 {
                                            self.emit(
                                                Instruction::ZExt {
                                                    op: raw_op,
                                                    from: Type::I1,
                                                    to: Type::I8,
                                                },
                                                Type::I8,
                                            )
                                        } else {
                                            raw_op
                                        };
                                        self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_to_string_bool".to_string(),
                                                args: vec![(b_i8, Type::I8)],
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        )
                                    }
                                    // An unboxed SIMD vector never leaves
                                    // `lower_expr` (it coerces to the static type).
                                    Type::V4F32 | Type::V4I32 | Type::V2F64 => {
                                        let boxed = self.coerce(raw_op, Type::Ref);
                                        self.string_by_selector(boxed)
                                    }
                                    Type::Ref => self.string_by_selector(raw_op),
                                    Type::Void | Type::Ptr => self.emit(
                                        Instruction::Const(Constant::String("null".to_string())),
                                        Type::Ref,
                                    ),
                                }
                            }
                        };
                        parts.push(part_op);
                    }
                    match parts.len() {
                        0 => self.emit(Instruction::Const(Constant::String(String::new())), Type::Ref),
                        1 => parts.pop().expect("uma parte"),
                        _ => self.emit(Instruction::ConcatStrings { parts }, Type::Ref),
                    }
                }
            }
            ExprKind::Identifier(name) => {
                let sym = name.sym;
                let span = expr.span;
                // The resolution decides (R6/N1): a local shadows a field, a
                // field shadows a global. The by-name map only holds for what
                // the resolution says is a local or parameter.
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) | None => {
                        if let Some(op) = self.read_local_by_name(sym) {
                            return op;
                        }
                    }
                    r @ Some(Resolved::Member { .. }) if self.resolved_member_te(r.as_ref()).is_some() => {
                        let m = self.resolved_member_te(r.as_ref()).expect("membro de tipo de extensão");
                        let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                        let receiver = self.implicit_receiver();
                        return self.read_member_te(this, m, receiver, span);
                    }
                    Some(Resolved::Member { member, .. }) => {
                        if let Some(op) = self.read_sdk_implicit_member(member, sym, expr_id, span) {
                            return op;
                        }
                        return self.read_implicit_member(member, span);
                    }
                    Some(Resolved::Element(el)) => {
                        return self.read_element(el, span);
                    }
                    Some(Resolved::ExtensionMember { member, .. }) => {
                        let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                        let receiver = self.implicit_receiver();
                        return self.read_extension(this, member.0 as usize, receiver, span);
                    }
                    _ => {}
                }
                if let Some(op) = self.read_local_by_name(sym) {
                    return op;
                }
                // A local `const` seen from inside the getter of a canonical
                // constant: its initializer, again.
                if let Some((_, init)) = self.local_const_keys.get(&sym).cloned() {
                    return self.lower_in_const_context(ast, init);
                }
                // `$this` in an interpolation arrives as an identifier.
                if self.ctx.symbol_name(sym) == "this"
                    && let Some(t) = self.this_param.clone()
                {
                    return t;
                }
                // `$1` without a receiver, in the body of an extension on a record;
                // also a named field of the record of the `on` (`sqlPrefix` in the
                // extension on `({String? sqlPrefix, bool isNullable})` of
                // drift): the implicit `this` reads the field (inference leaves
                // the identifier as `dynamic`).
                let on_field = self.this_extension.is_some_and(|(_, on)| {
                    matches!(self.ctx.table.get(self.ctx.erase(on)),
                        dartforge_types::table::Type::Record { named, .. } if named.iter().any(|(n, _)| *n == sym))
                });
                if (on_field || self.ctx.symbol_name(sym).starts_with('$'))
                    && let Some(t) = self.this_param.clone()
                    && let Some(op) = {
                        let name_str = self.ctx.symbol_name(sym).to_string();
                        let mut pat = |s: &mut Self| s.throw_nsm(&name_str);
                        self.read_record_field(t, self.ctx.symbol_name(sym), &mut pat)
                    }
                {
                    return op;
                }
                // `x` in the body of a constructor with `this.x`: inference
                // resolves it as a parameter, but the initializing parameter is not
                // in the scope of the body — there `x` is the field.
                if let (Some(this), Some(cid)) = (self.this_param.clone(), self.enclosing_class) {
                    let field = super::members::layout(self.ctx, cid)
                        .into_iter()
                        .find(|v| self.ctx.program.variables[v.0 as usize].name == sym);
                    if let Some(vid) = field {
                        return self.read_field_with_late(this, vid, span);
                    }
                }
                // No resolution (closure body, which inference does not
                // visit yet): the name through the lexical scope — member of the enclosing
                // class, then the library scope.
                if let Some(op) = self.read_unresolved_name(sym, span) {
                    return op;
                }
                // A type variable as an expression (`T`): the `Type` object
                // of the current type argument.
                if let Some(r) = self.type_variable_recipe(sym) {
                    let typ = self.recipe_rti(&r);
                    return self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_rti_objeto_tipo".to_string(),
                            args: vec![(typ, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                }
                let name_str = self.ctx.symbol_name(sym).to_string();
                // `dynamic`, `Never` and `Null` are also expressions that
                // denote `Type` objects. The last two may come through the
                // normal resolution; this path covers the special names without an
                // element in the outline, as the JS emitter already does.
                let recipe = match name_str.as_str() {
                    "dynamic" => Some("D"),
                    "Never" => Some("N"),
                    "Null" => Some("U"),
                    _ => None,
                };
                if let Some(text_str) = recipe {
                    let typ = self.recipe_rti(&super::rti::Recipe {
                        text: text_str.to_string(),
                        variables: false,
                    });
                    return self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_rti_objeto_tipo".to_string(),
                            args: vec![(typ, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                }
                // Nothing in the lexical scope, the imports or the outline has the
                // name: the program is invalid (the VM's `Undefined name`). In
                // the source SDK code, a missing construct.
                if self.ctx.from_source.contains(&self.ctx.program.unit(self.unit_id).library) {
                    return self.unsupported(&format!("identificador `{name_str}`"), span);
                }
                self.language_error(&format!("Undefined name '{name_str}'."), span)
            }
            ExprKind::Parenthesized(sub) => self.lower_expr(ast, *sub),
            ExprKind::Binary { op, left, right } => {
                // Short-circuit comes before the lowering of both sides: && and ||
                // cannot evaluate the right side when the left already decides, and ??
                // only evaluates the right when the left is null. The arithmetic
                // path below evaluates both at once, so these three
                // cannot go through it.
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull) {
                    return self.lower_short_circuit(ast, *op, *left, *right, expr_id);
                }
                // `super op e` (P4).
                if matches!(ast.expr(*left).kind, ExprKind::Super) {
                    let rop = self.lower_expr(ast, *right);
                    let name_str = match op {
                        BinaryOp::Eq | BinaryOp::NotEq => "==",
                        _ => super::dispatch::operator(*op).map_or("?", |(n, _)| n),
                    };
                    let r = self.super_operator(name_str, rop, expr.span);
                    if *op == BinaryOp::NotEq {
                        let b = self.to_bool(r);
                        return self.emit(Instruction::LNot(b), Type::I1);
                    }
                    return r;
                }
                // Extension operator (P4): a direct call to the member.
                if let Some(fid) = self.extension_operator(expr_id) {
                    let lop = self.lower_expr(ast, *left);
                    let rop = self.lower_expr(ast, *right);
                    let receiver = self.ctx.get_type_raw(self.unit_id, *left);
                    let r = self.call_extension(lop, fid, &[(None, rop)], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                // Operator of a program class: a direct call through the member
                // (`==`, with the `null` rule, stays with the general path).
                if let Some((name_str, _)) = super::dispatch::operator(*op)
                    && let Some(fid) = self.program_operator(*left, name_str)
                {
                    let lop = self.lower_expr(ast, *left);
                    let rop = self.lower_expr(ast, *right);
                    let r = self.call_member(lop, fid, &[(None, rop)], expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                let lop = self.lower_expr(ast, *left);
                let rop = self.lower_expr(ast, *right);
                self.mark_if_typed(*left, &lop);
                if let Some((name_str, _)) = super::dispatch::operator(*op)
                    && !matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                {
                    self.implicit_operator_cast(*right, &rop, *left, name_str, 0);
                } else {
                    self.mark_if_typed(*right, &rop);
                }
                let l_ty = self.ctx.get_type(self.unit_id, *left);
                let r_ty = self.ctx.get_type(self.unit_id, *right);
                let unknown = |t: Option<dartforge_types::table::TypeId>| {
                    t.is_none_or(|t| t == self.ctx.core.dynamic_)
                };
                // `String` by the static type; with an unknown type (an uninferred
                // body, an untyped closure parameter) a `Ref` on the
                // left of `+`/`*` is a string, as it was before.
                // With an unknown type, the operator goes to dynamic
                // dispatch (`dispatch.rs`), which knows how to concatenate strings.
                let _ = unknown;
                let text_str = l_ty.is_some_and(|t| self.ctx.is_string(t))
                    || (matches!(op, BinaryOp::Add) && r_ty.is_some_and(|t| self.ctx.is_string(t)));
                // `int?`/`double?` promoted by flow (`if (x != null) x + 1`):
                // inference does not write the promotion into the type of the read yet,
                // so the operand arrives as a `Ref` and goes back to the scalar here.
                // (`==`/`!=` stays out: comparing with null is legitimate.)
                // A non-nullable `String` on the left: the `==` of `String` (which cannot
                // be overridden) inline, `@df.texto_igual_a`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && l_ty.is_some_and(|t| {
                        self.ctx.is_string(t)
                            && matches!(self.ctx.table.get(t), dartforge_types::table::Type::Interface { nullable: false, .. })
                    })
                    && self.operand_type(&lop) == Type::Ref
                {
                    let r = self.string_equality(lop, rop);
                    if matches!(op, BinaryOp::NotEq) {
                        return self.emit(Instruction::LNot(r), Type::I1);
                    }
                    return r;
                }
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && let Some(r) =
                        self.closed_class_equality(l_ty, &lop, &rop, matches!(op, BinaryOp::NotEq))
                {
                    return r;
                }
                let (lop, rop) = if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) || text_str {
                    (lop, rop)
                } else {
                    let l = self.denullify_numeric(*left, lop);
                    let r = self.denullify_numeric(*right, rop);
                    (l, r)
                };
                let admits_smi = self.type_admits_smi(l_ty) && self.type_admits_smi(r_ty);
                self.operate_with_smi(*op, lop, rop, text_str, admits_smi, expr.span)
            }
            ExprKind::Unary { op, operand } => {
                match op {
                    UnaryOp::PrefixInc
                    | UnaryOp::PostfixInc
                    | UnaryOp::PrefixDec
                    | UnaryOp::PostfixDec => {
                        // `x++` = `x += 1` returning the old value; holds for a
                        // local, a field, a global and an index (before only a local, and a
                        // field became the constant 0).
                        let bin = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) {
                            BinaryOp::Add
                        } else {
                            BinaryOp::Sub
                        };
                        let new = self.lower_assignment(
                            ast,
                            ast::AssignOp::Compound(bin),
                            *operand,
                            super::assignment::Rhs::One,
                            expr.span,
                        );
                        let old = self.old_value.take();
                        return match op {
                            UnaryOp::PrefixInc | UnaryOp::PrefixDec => new,
                            _ => old.unwrap_or(new),
                        };
                    }
                    _ => {}
                }
                // Extension `-x`/`~x` (P4): a direct call to the member.
                if matches!(op, UnaryOp::Neg | UnaryOp::BitNot)
                    && let Some(fid) = self.extension_operator(expr_id)
                {
                    let v = self.lower_expr(ast, *operand);
                    let receiver = self.ctx.get_type_raw(self.unit_id, *operand);
                    let r = self.call_extension(v, fid, &[], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                if *op == UnaryOp::NullAssert {
                    let sub_op = self.lower_target(ast, *operand);
                    if self.operand_type(&sub_op) != Type::Ref {
                        // A scalar is not null (R1): nothing to check.
                        return sub_op;
                    }
                    let is_null = self.emit(
                        Instruction::ICmp(
                            ICmpOp::Eq,
                            sub_op.clone(),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    );
                    let null_block = self.new_block();
                    let cont_block = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: is_null,
                        then_block: null_block,
                        else_block: cont_block,
                    });
                    self.set_block(null_block);
                    let err_op = self.new_runtime_error("dartforge_null_check_error_new", Vec::new());
                    self.emit_throw_op(err_op);
                    self.set_block(cont_block);
                    return sub_op;
                }
                let sub_op = self.lower_expr(ast, *operand);
                let sub_op = if matches!(op, UnaryOp::Neg | UnaryOp::BitNot) {
                    self.denullify_numeric(*operand, sub_op)
                } else {
                    sub_op
                };
                match op {
                    UnaryOp::Neg | UnaryOp::BitNot if self.operand_type(&sub_op) == Type::Ref => {
                        self.dynamic_unary(*op, sub_op)
                    }
                    UnaryOp::Neg if self.operand_type(&sub_op) == Type::F64 => {
                        self.emit(Instruction::FNeg(sub_op), Type::F64)
                    }
                    UnaryOp::Neg => self.emit(Instruction::Neg(sub_op), Type::I64),
                    UnaryOp::Not => {
                        let b = self.to_bool(sub_op);
                        self.emit(Instruction::LNot(b), Type::I1)
                    }
                    UnaryOp::BitNot => self.emit(Instruction::Not(sub_op), Type::I64),
                    _ => self.unsupported("incremento/decremento de não-local", expr.span),
                }
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.to_bool(cond_op);
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block,
                    else_block,
                });
                // The two entries of the phi in the representation of the result
                // (R4), coerced in the origin block of each one.
                let ty = self.expression_repr(expr_id);
                self.set_block(then_block);
                let then_op = self.lower_expr(ast, *then);
                let then_op = match ty {
                    Some(t) => self.coerce(then_op, t),
                    None => then_op,
                };
                let then_end = self.current_block;
                let then_reaches = !self.is_terminated();

                // The `then` only closes after the `else`: without a static type, the
                // representation of the result comes from both branches (equal, or
                // `Ref`), and the `then` is coerced in its block.
                self.set_block(else_block);
                let else_op = self.lower_expr(ast, *else_);
                let ty = ty.unwrap_or_else(|| {
                    let (a, b) = (self.operand_type(&then_op), self.operand_type(&else_op));
                    if a == b { a } else { Type::Ref }
                });
                let else_op = self.coerce(else_op, ty);
                let else_end = self.current_block;
                let else_reaches = !self.is_terminated();
                if else_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }
                let (then_op, then_end) = if then_reaches {
                    self.set_block(then_end);
                    let op = self.coerce(then_op, ty);
                    let end = self.current_block;
                    self.terminate(Terminator::Branch(merge_block));
                    (op, end)
                } else {
                    (then_op, then_end)
                };

                self.set_block(merge_block);
                if then_reaches && else_reaches {
                    self.emit(
                        Instruction::Phi {
                            incoming: vec![(then_end, then_op), (else_end, else_op)],
                            ty,
                        },
                        ty,
                    )
                } else if then_reaches {
                    then_op
                } else if else_reaches {
                    else_op
                } else {
                    self.default_return_operand()
                }
            }
            ExprKind::Property {
                target,
                name,
                null_aware,
            } => {
                let prop_name = self.ctx.symbol_name(name.sym);

                let span = expr.span;
                let resolved = self.ctx.get_resolved(self.unit_id, expr_id).cloned();

                // `E.x`: a static field or getter of an extension.
                if matches!(
                    self.ctx.get_resolved(self.unit_id, *target),
                    Some(Resolved::Element(dartforge_elements::model::Element::Extension(_)))
                ) {
                    return match resolved {
                        Some(Resolved::Element(el)) => self.read_element(el, span),
                        Some(Resolved::ExtensionMember { member, .. }) => {
                            self.read_static_member(MemberRef::Function(member), span)
                        }
                        _ => self.unsupported(&format!("membro estático de extensão `{prop_name}`"), span),
                    };
                }
                // `C<T…>.new` / `C<T…>.name`: a tear-off of an instantiated
                // constructor (the type arguments go in the environment).
                if let ExprKind::TypeArguments { target: cls, type_args } = &ast.expr(*target).kind
                    && let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                        self.resolution_or_name(ast, *cls)
                    && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied())
                    else {
                        return self.unsupported(&format!("construtor `{prop_name}`"), span);
                    };
                    let args = self.type_argument_recipes(type_args);
                    let object = self.recipe_rti(&super::rti::Recipe {
                        text: format!("C{}<{}>", self.ctx.id_rti(c), args.text),
                        variables: args.variables,
                    });
                    let tuple = self.recipe_rti(&super::rti::Recipe {
                        text: format!("L<{}>", args.text),
                        variables: args.variables,
                    });
                    let typ = self.ctx.get_type(self.unit_id, expr_id);
                    return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, typ, span);
                }
                // `E.new`/`E.name` of an extension type: a constructor tear-off
                // (the primary one too).
                if let Some(c) = self.named_extension_type(ast, *target) {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    if let Some(k) = key
                        && (super::extension_types::primary_name(self.ctx, c) == Some(k)
                            || self.ctx.program.classes[c.0 as usize].constructors.contains_key(&k))
                        && !matches!(resolved, Some(Resolved::Member { .. }))
                    {
                        return self.constructor_tearoff_te(expr_id, c, k, span);
                    }
                }
                // `Alias.new`/`Alias.name` with `typedef Alias<…> = C<…>`:
                // a tear-off of the constructor of `C` (riverpod's `NotifierFamilyProvider
                // .internal`). With the static type instantiated
                // by inference, the object is born with the arguments; without it,
                // only when the alias forwards the parameters in order (then the
                // tear-off is the generic one of `C`).
                if let Some(Resolved::Element(dartforge_elements::model::Element::Typedef(td))) =
                    self.ctx.get_resolved(self.unit_id, *target).cloned()
                    && let Some(data) = self.ctx.outline.typedefs.get(td.0 as usize)
                    && let dartforge_types::table::Type::Interface { class: c, args, .. } =
                        self.ctx.table.get(data.target_type).clone()
                    && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                    && !matches!(resolved, Some(Resolved::Member { .. }))
                {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    if let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied()) {
                        if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                            && let Some((object, tuple)) = self.tearoff_instantiation(c, typ)
                        {
                            return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, Some(typ), span);
                        }
                        // A generic tear-off through an alias that does not just forward the
                        // parameters (`typedef F<T> = C<int, T>`): the generic
                        // function of the static type, with the object written
                        // in its parameters — or, with the implicit instantiation
                        // that inference chose (`C<int,
                        // String> Function() g = F.name;`), already instantiated.
                        if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                            && let dartforge_types::table::Type::Function { type_params, ret, .. } = self.ctx.table.get(typ).clone()
                            && !type_params.is_empty()
                            && let dartforge_types::table::Type::Interface { class: c2, args: a2, .. } = self.ctx.table.get(ret).clone()
                            && c2 == c
                        {
                            if let Some(inst) = self.ctx.bodies.units[self.unit_id.0 as usize]
                                .instanciacao_de_tearoff(expr_id)
                                .map(<[_]>::to_vec)
                                .filter(|i| i.len() == type_params.len())
                            {
                                let mut variables = false;
                                let real_set: Vec<String> = inst
                                    .iter()
                                    .map(|t| {
                                        let r = self.type_recipe(*t);
                                        variables |= r.variables;
                                        r.text
                                    })
                                    .collect();
                                let strings: Vec<String> =
                                    a2.iter().map(|a| self.type_recipe_with(*a, &type_params, &real_set).text).collect();
                                let object = self.recipe_rti(&super::rti::Recipe {
                                    text: format!("C{}<{}>", self.ctx.id_rti(c), strings.join(",")),
                                    variables,
                                });
                                let tuple = self.recipe_rti(&super::rti::Recipe {
                                    text: format!("L<{}>", strings.join(",")),
                                    variables,
                                });
                                return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, None, span);
                            }
                            return self.generic_constructor_tearoff(f.0 as usize, c, &type_params, &a2, typ, span);
                        }
                        let forwards = args.len() == data.type_params.len()
                            && args.iter().zip(data.type_params.iter()).all(|(a, p)| {
                                matches!(self.ctx.table.get(*a),
                                    dartforge_types::table::Type::TypeParameter { param, nullable: false } if param == p)
                            });
                        if forwards {
                            let t = self.constructor_tearoff(f.0 as usize, span);
                            self.tearoff_generic_signature(&t, expr_id);
                            return t;
                        }
                    }
                }
                // `C.x`: a static member (the target is a class literal, or
                // a `typedef A = C<…>` — specification §15 "Type Aliases":
                // `A.x` denotes the static member `x` of `C`).
                if let Some(c) = self.static_target_class(*target) {
                    if prop_name == "values" && super::enums::is_enum(self.ctx, c) {
                        return self.enum_values(c, span);
                    }
                    // `C.new`/`C.name`: a constructor tear-off (P4).
                    if self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                        && matches!(
                            self.ctx.get_resolved(self.unit_id, *target),
                            Some(Resolved::Element(dartforge_elements::model::Element::Class(_)))
                        )
                    {
                        let key = if prop_name == "new" {
                            self.ctx.interner.lookup("")
                        } else {
                            Some(name.sym)
                        };
                        if let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied())
                            && !matches!(resolved, Some(Resolved::Member { .. }))
                        {
                            // A generic class with the type arguments
                            // inferred from the context (`putIfAbsent(k,
                            // HashSet.new)` is `HashSet<V> Function()`): the
                            // tear-off is the instantiated one, like `C<T…>.new`.
                            if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                                && let Some((object, tuple)) = self.tearoff_instantiation(c, typ)
                            {
                                return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, Some(typ), span);
                            }
                            let t = self.constructor_tearoff(f.0 as usize, span);
                            self.tearoff_generic_signature(&t, expr_id);
                            return t;
                        }
                    }
                    return match resolved {
                        Some(Resolved::Member { member, .. }) => {
                            self.read_static_member(member, span)
                        }
                        _ => self.unsupported(&format!("membro estático `{prop_name}`"), span),
                    };
                }

                // `prefix.x`: the element imported with a prefix.
                if let Some(el) = self.prefixed_element(ast, *target, name.sym) {
                    return self.read_element(el, span);
                }
                // `super.x` (P4).
                if matches!(ast.expr(*target).kind, ExprKind::Super) {
                    return self.read_super(name.sym, span);
                }

                let target_op = self.lower_target(ast, *target);
                if *null_aware {
                    self.branch_if_null(&target_op);
                }

                // Extension type member (`extension_types.rs`): before
                // the paths through the erased type (a `length` of the extension
                // type is not that of the list it represents).
                if let Some(m) = self.resolved_member_te(resolved.as_ref()) {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    let r = self.read_member_te(target_op, m, receiver, span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }

                // `length` of a numeric typed list or `List<E>` (`typed_items.rs`).
                if prop_name == "length"
                    && let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *target))
                {
                    self.access_fixed = self.fixed_list_of(ast, *target);
                    self.access_proven = self.proven_of(ast, *target, None);
                    let n = self.indexed_length(target_op, l);
                    self.access_fixed = None;
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::I64);
                    return self.coerce(n, repr);
                }

                // `index`/`name` of a program enum value (the
                // implicit fields, which the element does not compile).
                if let Some(c) = self.user_class_of(*target)
                    && let Some(op) = self.enum_member(c, prop_name, target_op.clone())
                {
                    return op;
                }

                // A user class member: through the resolved element (R7),
                // never by name.
                if let Some((_, member)) = self.user_member(expr_id, *target, name.sym, false)
                {
                    {
                        let vid = match member {
                            MemberRef::Variable(v) => Some(v),
                            MemberRef::Function(f) => {
                                self.ctx.program.functions[f.0 as usize].variable
                            }
                        };
                        if let Some(vid) = vid {
                            if crate::lower::is_global(self.ctx, vid) {
                                return self.read_global(vid, span);
                            }
                            return self.read_field_with_late(target_op, vid, span);
                        }
                        let MemberRef::Function(f) = member else {
                            unreachable!()
                        };
                        if self.ctx.program.functions[f.0 as usize].kind
                            == dartforge_elements::model::FunctionKind::Getter
                        {
                            return self.call_member(target_op, f.0 as usize, &[], span);
                        }
                        return self.method_tearoff(target_op, f.0 as usize, span);
                    }
                }

                // Extension getter (P4).
                if let Some(Resolved::ExtensionMember { member, .. }) = resolved.clone()
                    && crate::lower::user_function(self.ctx, member.0 as usize)
                {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    return self.read_extension(target_op, member.0 as usize, receiver, span);
                }
                // `r.$1`/`r.name`: a field of a record (a runtime positional
                // one, or one of a shape with a named field).
                let positional_arg = prop_name
                    .strip_prefix('$')
                    .and_then(|d| d.parse::<i64>().ok())
                    .is_some_and(|k| k >= 1);
                if positional_arg || !self.shapes_with_field(prop_name).is_empty() {
                    let name_str = prop_name.to_string();
                    let alvo = *target;
                    let t2 = target_op.clone();
                    let mut rest = |s: &mut Self| s.property_without_member(t2.clone(), alvo, &name_str, expr_id, span);
                    if let Some(op) = self.read_record_field(target_op.clone(), prop_name, &mut rest) {
                        return op;
                    }
                }
                self.property_without_member(target_op, *target, prop_name, expr_id, span)
            }
            // `super[i]` (P4): the `operator []` of the superclass (or of the previous
            // mixin in the linearization).
            ExprKind::Index { target, index, .. } if matches!(ast.expr(*target).kind, ExprKind::Super) => {
                let i = self.lower_expr(ast, *index);
                let Some(name_str) = self.ctx.interner.lookup("[]") else {
                    return self.unsupported("`super[]` sem operador", expr.span);
                };
                let r = self.call_super_method(name_str, &[(None, i)], expr.span);
                let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                self.coerce(r, repr)
            }
            ExprKind::Index {
                target,
                index,
                null_aware,
            } => {
                let target_op = self.lower_target(ast, *target);
                if *null_aware {
                    self.branch_if_null(&target_op);
                }
                let idx_op = self.lower_expr(ast, *index);
                self.implicit_operator_cast(*index, &idx_op, *target, "[]", 0);
                if let Some(fid) = self.extension_operator(expr_id) {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    let r = self.call_extension(target_op, fid, &[(None, idx_op)], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                if let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *target)) {
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    self.access_fixed = self.fixed_list_of(ast, *target);
                    self.access_proven = self.proven_of(ast, *target, Some(*index));
                    let read_done = self.read_indexed(target_op.clone(), idx_op.clone(), l, repr);
                    self.access_proven = None;
                    self.access_fixed = None;
                    if let Some(r) = read_done {
                        return r;
                    }
                }
                // `operator []` of a program class: through the member.
                if let Some(fid) = self.program_operator(*target, "[]") {
                    let r = self.call_member(target_op, fid, &[(None, idx_op)], expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                // SDK `[]` through the dynamic class.
                let r = self.call_by_name_typed(target_op, *target, super::sdk_source::AccessKind::Call, "[]", &[(None, idx_op)]);
                let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                self.coerce(r, repr)
            }
            ExprKind::Record { positional, named, .. } if !named.is_empty() => {
                self.lower_named_record(ast, positional, named, expr.span)
            }
            // The positional record (`_Record`, `REFS`): the fields boxed
            // in the HIR and the `AllocRecord` with only `Ref`s (`records.rs`).
            ExprKind::Record { positional, .. } => self.lower_positional_record(ast, positional),
            ExprKind::Call { target, arguments } => {
                self.lower_call(ast, expr_id, expr, target, arguments)
            }
            // With control elements, or large and constant (the chunked
            // path of `literals.rs`, which does not put a buffer of `n`
            // words and `n` live values in the frame).
            ExprKind::List { elements, .. }
                if !elements.iter().all(|e| matches!(e, ast::CollectionElement::Expression(_)))
                    || self.large_literal_is_constant(ast, elements) =>
            {
                let l = self.lower_collection_literal(ast, super::literals::Collection::List, elements, expr.span);
                self.literal_rti(l, expr_id)
            }
            // Maps and sets are the `_Map`/`_Set` of the SDK source.
            ExprKind::SetOrMap { elements, .. } => {
                let typ = if self.literal_is_set(expr_id, elements) {
                    super::literals::Collection::Set
                } else {
                    super::literals::Collection::Map
                };
                let collection = self.lower_collection_literal(ast, typ, elements, expr.span);
                self.literal_rti(collection, expr_id)
            }
            ExprKind::List { elements, .. } => {
                let mut elem_ops = Vec::new();
                for el in elements.iter() {
                    if let ast::CollectionElement::Expression(e) = el {
                        let op = self.lower_expr(ast, *e);
                        let tag = self.operand_tag(&op);
                        elem_ops.push((op, tag));
                    }
                }
                let l = self.emit(Instruction::AllocList { elements: elem_ops }, Type::Ref);
                // RTI: `<int>[…]` is `List<int>` (the type of the literal).
                if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
                    self.define_rti_if_generic(l.clone(), t);
                }
                l
            }
            ExprKind::InstanceCreation { arguments, constructor, ty, .. } => {
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Constructor(fid)) => {
                        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                        self.instantiate(ast, fid, &arguments.args, expr.span)
                    }
                    // `new A<T>(…)` of a named mixin application
                    // (`class A = B with M;`, the `_SyncStreamController` of
                    // `Stream.fromFutures`): without a declared constructor, that of the
                    // superclass it forwards to (`constructor_of`).
                    _ if let Some(dartforge_types::table::Type::Interface { class: c, .. }) =
                        self.ctx.get_type_raw(self.unit_id, expr_id).map(|t| self.ctx.table.get(t).clone())
                        && self.ctx.program.classes[c.0 as usize].kind == dartforge_elements::model::ClassKind::MixinApplication
                        && let Some(f) = constructor
                            .map(|n| n.sym)
                            .or_else(|| match &ast.ty(*ty).kind {
                                ast::TypeKind::Named { name, .. } if name.len() == 2 => Some(name[1].sym),
                                _ => self.ctx.interner.lookup(""),
                            })
                            .and_then(|n| self.constructor_of(c, n)) =>
                    {
                        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                        self.instantiate(ast, f, &arguments.args, expr.span)
                    }
                    // `new E(…)`/`const E.name(…)` of an extension type: the
                    // primary one is not an element (`extension_types.rs`).
                    _ => match self.ctx.get_type_raw(self.unit_id, expr_id).map(|t| self.ctx.table.get(t).clone()) {
                        Some(dartforge_types::table::Type::ExtensionType { decl, .. }) => {
                            // `new E.name(…)` without a prefix comes from the parser
                            // as the two-part type `E.name` (it does not
                            // distinguish `p.T` from `T.n`): when the first
                            // part names the type itself, the second is the
                            // constructor.
                            let name_str = match constructor {
                                Some(n) => Some(n.sym),
                                None => match &ast.ty(*ty).kind {
                                    ast::TypeKind::Named { name, .. }
                                        if name.len() == 2
                                            && matches!(
                                                self.ctx
                                                    .program
                                                    .lookup(self.ctx.program.unit(self.unit_id).library, name[0].sym)
                                                    .and_then(|b| b.getter),
                                                Some(dartforge_elements::model::Element::Class(c)) if c == decl
                                            ) =>
                                    {
                                        Some(name[1].sym)
                                    }
                                    _ => self.ctx.interner.lookup(""),
                                },
                            };
                            match name_str {
                                Some(n) => self.build_te(ast, expr_id, decl, n, &arguments.args, expr.span),
                                None => self.unsupported("instanciação não resolvida", expr.span),
                            }
                        }
                        _ => self.unsupported("instanciação não resolvida", expr.span),
                    },
                }
            }
            ExprKind::Throw(inner) => self.emit_throw(ast, *inner),
            ExprKind::Rethrow => self.emit_rethrow(),
            ExprKind::Is { value, ty, negated } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                let is_m = self.test_type(ast_ty, val_op);
                let is_m = self.to_bool(is_m);
                if *negated {
                    self.emit(Instruction::LNot(is_m), Type::I1)
                } else {
                    is_m
                }
            }
            ExprKind::As { value, ty } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                self.check_type_or_throw(ast_ty, val_op.clone(), super::rti::CastContext::How);
                val_op
            }
            ExprKind::Assign { op, target, value } => self.lower_assignment(
                ast,
                *op,
                *target,
                super::assignment::Rhs::Expr(*value),
                expr.span,
            ),
            ExprKind::FunctionExpression(fid) => {
                let typ = self.literal_function_type(ast, *fid, Some(expr_id));
                let c = self.lower_closure(ast, *fid, expr.span, typ);
                // RTI: the signature of the closure (`f is R Function(P)`).
                self.define_closure_rti(c.clone(), ast, *fid, Some(expr_id));
                c
            }
            ExprKind::Switch { value, cases } => self.lower_switch_expression(ast, expr_id, *value, cases),
            ExprKind::Cascade {
                target,
                sections,
                null_aware,
            } => self.lower_cascade(ast, *target, sections, *null_aware),
            ExprKind::CascadeTarget => match self.current_cascade_target.clone() {
                Some(t) => t,
                None => self.unsupported("cascata", expr.span),
            },
            ExprKind::PatternAssign { pattern, value } => {
                let v = self.lower_expr(ast, *value);
                self.match_irrefutable(ast, *pattern, v.clone(), super::patterns::Linkage::Assign, *value);
                v
            }
            ExprKind::Await(inner) => self.lower_await(ast, *inner, expr.span),
            ExprKind::This => match self.this_param.clone() {
                Some(t) => t,
                None => self.unsupported("`this` fora de membro de instância", expr.span),
            },
            // `f<T…>`: an instantiated tear-off of a top-level or static generic
            // function (the type arguments go in the environment of the closure).
            ExprKind::TypeArguments { target, type_args }
                if let Some(fid) = self.target_generic_function(ast, *target) =>
            {
                let args = self.type_argument_recipes(type_args);
                let tuple = self.recipe_rti(&super::rti::Recipe {
                    text: format!("L<{}>", args.text),
                    variables: args.variables,
                });
                let typ = self.ctx.get_type(self.unit_id, expr_id);
                self.instantiated_tearoff(fid, tuple, typ)
            }
            // `C<T…>` as an expression: the canonical `Type` object of the instantiated
            // type (the raw literal `C` is in `members.rs`).
            ExprKind::TypeArguments { target, type_args }
                if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.resolution_or_name(ast, *target) =>
            {
                let args = self.type_argument_recipes(type_args);
                let t = self.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<{}>", self.ctx.id_rti(c), args.text),
                    variables: args.variables,
                });
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_objeto_tipo".to_string(),
                        args: vec![(t, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
            other => {
                let what = match other {
                    ExprKind::Super => "`super` como valor",

                    ExprKind::TypeArguments { .. } => "instanciação de tipo genérico",


                    ExprKind::Await(_) => "await",

                    _ => "expressão",
                };
                self.unsupported(what, expr.span)
            }
        }
    }

    /// `target.name` when the name is not a static member of the type of the target:
    /// `index`/`name` of an enum, the member through the dynamic class (a receiver without a
    /// useful type) or the SDK member matched by name.
    pub fn property_without_member(
        &mut self,
        target_op: Operand,
        target: ExprId,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        // `index`/`name` of a program enum value.
        if let Some(c) = self.user_class_of(target)
            && let Some(op) = self.enum_member(c, prop_name, target_op.clone())
        {
            return op;
        }
        self.property_by_selector(target_op, prop_name, expr_id, span)
    }

    /// A property of an SDK value (`length`, `isEmpty`, `first`, `message`…):
    /// the member through the dynamic class of the receiver, by selector (the SDK always
    /// comes from source).
    pub(super) fn property_by_selector(
        &mut self,
        target_op: Operand,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        let _ = (expr_id, span);
        self.call_by_name(target_op, super::sdk_source::AccessKind::Read, prop_name, &[])
    }

    /// An implicit member (`x` = `this.x`) that belongs to the SDK (the extension on an
    /// SDK type, the `name`/`index` of the enum inside it): through the SDK
    /// path with `this` as the receiver.
    pub fn read_sdk_implicit_member(
        &mut self,
        member: dartforge_types::resolved::MemberRef,
        sym: dartforge_intern::SymbolId,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Option<Operand> {
        let lib = match member {
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].library,
            MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].library,
        };
        if self.ctx.compiled_library(lib) {
            return None;
        }
        let this = self.this_param.clone()?;
        let name_str = self.ctx.symbol_name(sym).to_string();
        if let Some(c) = self.enclosing_class
            && let Some(op) = self.enum_member(c, &name_str, this.clone())
        {
            return Some(op);
        }
        if name_str.starts_with('$') {
            let n2 = name_str.clone();
            let mut pat = |s: &mut Self| s.throw_nsm(&n2);
            if let Some(op) = self.read_record_field(this.clone(), &name_str, &mut pat) {
                return Some(op);
            }
        }
        Some(self.property_by_selector(this, &name_str, expr_id, span))
    }
}

/// The offset the VM gives the expression in the stack trace (the `fileOffset` of
/// front_end, `offsetForToken(selector.token)`): the called name in `f()`,
/// `o.m()` and `o.p`; the operator in `a + b`, `a += b` and `e as T`; the `[` in
/// `a[i]`; the `(` in the call of an expression (`f()()`); the start in
/// `throw`, `await`, instance creation, unary and identifier (the
/// top-level getter). `None`: the expression calls nothing by itself, and the
/// outer position stays. `source` is that of the unit of the tree (`mark_position`
/// discards the position of another tree).
fn expression_position(ast: &ast::Ast, source: &str, expr_id: ExprId) -> Option<usize> {
    // The first non-blank byte starting at `end`: the operator after the
    // left operand.
    let after = |end: usize| -> usize {
        source
            .as_bytes()
            .get(end..)
            .and_then(|rest| rest.iter().position(|b| !b.is_ascii_whitespace()))
            .map_or(end, |k| end + k)
    };
    let e = ast.expr(expr_id);
    match &e.kind {
        ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => Some(name.span.start),
            _ => Some(after(ast.expr(*target).span.end)),
        },
        ExprKind::Property { name, .. } => Some(name.span.start),
        ExprKind::Index { target, .. } => Some(after(ast.expr(*target).span.end)),
        ExprKind::Binary { left, .. } | ExprKind::Assign { target: left, .. } | ExprKind::As { value: left, .. } => {
            Some(after(ast.expr(*left).span.end))
        }
        ExprKind::InstanceCreation { .. }
        | ExprKind::Throw(_)
        | ExprKind::Rethrow
        | ExprKind::Await(_)
        | ExprKind::Unary { .. }
        | ExprKind::Identifier(_) => Some(e.span.start),
        _ => None,
    }
}
