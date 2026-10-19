//! Statements: blocks, declarations, `if`, loops, labeled `break`/`continue`,
//! `return`, `try`/`catch`/`finally` and `assert`.

use super::fn_builder::{FinallyScope, FnBuilder, FixedList};
use dartforge_intern::SymbolId;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StmtId, StmtKind};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `assert(cond, msg)`: throws a catchable `AssertionError` when the
    /// asserts are on (`Context::enable_asserts`: the `dart run
    /// --enable-asserts` oracle, the JIT, development). With them off
    /// (`DartFlags::PRODUCTION`) nothing is emitted — neither the condition nor
    /// the message is evaluated, as in `dart compile exe`. The statement, the
    /// constructor initializer and the extension type initializer all come
    /// through here.
    pub fn lower_assert(&mut self, ast: &ast::Ast, condition: ExprId, message: Option<ExprId>) {
        if !self.ctx.enable_asserts {
            return;
        }
        let c = self.lower_expr(ast, condition);
        let c = self.to_bool(c);
        let fail_b = self.new_block();
        let cont_b = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: c,
            then_block: cont_b,
            else_block: fail_b,
        });
        self.set_block(fail_b);
        let msg = match message {
            Some(m) => self.lower_expr(ast, m),
            None => Operand::Constant(Constant::Null),
        };
        // Under ARC the message goes as a `Ref` (`new_runtime_error`).
        let (bits, is_ref) = if self.ctx.arc_memory { (self.coerce(msg, Type::Ref), true) } else { self.to_bits(msg) };
        let err = self.new_runtime_error(
            "dartforge_assertion_error_new",
            vec![(bits, Type::I64), (Operand::Constant(Constant::Int(i64::from(is_ref))), Type::I8)],
        );
        self.emit_throw_op(err);
        self.set_block(cont_b);
    }

    pub fn lower_stmt(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
        // J05: the instructions of the statement carry its position; on leaving, the
        // position of the outer statement comes back (the update and the condition of a loop
        // belong to it, not to the last statement of the body).
        let previous = self.position;
        if !matches!(ast.stmt(stmt_id).kind, StmtKind::Block(_)) {
            self.mark_position(ast, ast.stmt(stmt_id).span.start);
        }
        self.lower_stmt_in_place(ast, stmt_id);
        self.position = previous;
    }

    fn lower_stmt_in_place(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
        let stmt = ast.stmt(stmt_id);
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.open_scope();
                for &s in stmts.iter() {
                    self.lower_stmt(ast, s);
                }
                self.close_scope();
            }
            StmtKind::Expression(expr_id) => {
                self.lower_expr_discarded(ast, *expr_id);
                // Inside a `try`: an extern that does not check the pending
                // exception (those of the SDK matched by name, frozen) cannot
                // leave the `catch` for later — it checks at the end of the statement.
                if !self.is_terminated() && (!self.exception_targets.is_empty() || !self.finally_scopes.is_empty()) {
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                }
            }
            StmtKind::Variables(var_list) => {
                let var_ty_opt = var_list.ty;
                for var in &var_list.variables {
                    let sym = var.name.sym;
                    // R6: the local keeps the representation of the declared type
                    // (or the one inferred from the initializer), not that of the value.
                    let mut ty = self.local_repr_of(var.name.span.start);
                    if var_list.late && ty.is_vector() {
                        // A `late` keeps the state next to the value: the box.
                        ty = Type::Ref;
                    }
                    let late_local = var_list.late
                        && (var.initializer.is_none()
                            || !self.cells.contains(&var.name.span.start));
                    if var_list.const_
                        && let Some(init_id) = var.initializer
                        && let Some(k) = self.constant_key(ast, init_id, true)
                    {
                        self.local_const_keys.insert(sym, (k, init_id));
                    }
                    let init_op = if late_local {
                        Self::zero_value(ty)
                    } else if ty.is_vector() {
                        // Unboxed SIMD: the initializer as a vector, or zero
                        // (a read before the assignment does not compile).
                        match var.initializer {
                            Some(init_id) => self.lower_simd(ast, init_id, ty),
                            None => self.emit(Instruction::Simd { op: OpSimd::Zero, args: Vec::new() }, ty),
                        }
                    } else if let Some(init_id) = var.initializer {
                        let op = if var_list.const_ {
                            self.lower_in_const_context(ast, init_id)
                        } else {
                            self.lower_expr(ast, init_id)
                        };
                        // A check at the declaration only when the static type does not
                        // guarantee it: a `dynamic` initializer for a declared type.
                        // Before it always ran, and `Foo? x = null` called
                        // `dartforge_value_class(0)` (H6).
                        let dynamic_init = self.ctx.get_type(self.unit_id, init_id)
                            == Some(self.ctx.core.dynamic_);
                        if let Some(tid) = var_ty_opt.filter(|_| dynamic_init) {
                            self.check_type_or_throw(ast.ty(tid), op.clone(), super::rti::CastContext::Implicit);
                        }
                        op
                    } else {
                        Self::zero_value(ty)
                    };
                    self.declare_variable(sym, var.name.span.start, ty, init_op);
                    if late_local {
                        self.configure_late_local(sym, var_list.final_, var.initializer);
                    }
                }
            }
            StmtKind::If {
                condition,
                case_pattern: Some(p),
                guard,
                then,
                else_,
            } => {
                self.lower_if_case(ast, *condition, *p, *guard, *then, *else_);
            }
            StmtKind::If {
                condition,
                then,
                else_,
                ..
            } => {
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.to_bool(cond_op);
                let then_b = self.new_block();
                let else_b = self.new_block();
                let merge_b = self.new_block();

                let false_target = if else_.is_some() { else_b } else { merge_b };
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: then_b,
                    else_block: false_target,
                });

                self.set_block(then_b);
                self.lower_stmt(ast, *then);
                self.terminate(Terminator::Branch(merge_b));

                if let Some(e) = else_ {
                    self.set_block(else_b);
                    self.lower_stmt(ast, *e);
                    self.terminate(Terminator::Branch(merge_b));
                }

                self.set_block(merge_b);
            }
            StmtKind::While { condition, body } => {
                let loop_header = self.new_block();
                let loop_body = self.new_block();
                let exit_block = self.new_block();

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_header);
                }

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.to_bool(cond_op);
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_header);

                self.set_block(loop_body);
                self.emit_safepoint();
                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_header));

                self.break_targets.pop();
                self.continue_targets.pop();

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(exit_block);
            }
            StmtKind::DoWhile { body, condition } => {
                let loop_body = self.new_block();
                let loop_cond = self.new_block();
                let exit_block = self.new_block();

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_cond);
                }

                self.terminate(Terminator::Branch(loop_body));
                self.set_block(loop_body);
                self.emit_safepoint();

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_cond);

                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_cond));

                self.break_targets.pop();
                self.continue_targets.pop();

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(loop_cond);
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.to_bool(cond_op);
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.set_block(exit_block);
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                // A counted loop ends by itself: no safepoint in the
                // iterations (what stays in an infinite loop is in a callee, which
                // has its own), and LLVM remains free to vectorize it.
                let counted = self.counted_loop(ast, init.as_ref(), *condition, updates, *body);
                // The `for` variable lives in an `alloca` (R6): before it was only
                // an SSA value in the by-name map, and the condition, lowered before
                // the `i++`, read the initial value forever.
                self.open_scope();
                if let Some(init) = init {
                    match init {
                        ast::ForInit::Variables(var_list) => {
                            for var in &var_list.variables {
                                let sym = var.name.sym;
                                let ty = self.local_repr_of(var.name.span.start);
                                let late_local = var_list.late
                                    && (var.initializer.is_none()
                                        || !self.cells.contains(&var.name.span.start));
                                let init_op = if late_local {
                                    Self::zero_value(ty)
                                } else if let Some(init_id) = var.initializer {
                                    self.lower_expr(ast, init_id)
                                } else {
                                    Self::zero_value(ty)
                                };
                                self.declare_variable(sym, var.name.span.start, ty, init_op);
                                if late_local {
                                    self.configure_late_local(sym, var_list.final_, var.initializer);
                                }
                            }
                        }
                        ast::ForInit::Expression(e) => {
                            self.lower_expr_discarded(ast, *e);
                        }
                        // Like a declaration by pattern, in the loop scope.
                        ast::ForInit::Pattern { pattern, value, .. } => {
                            let v = self.lower_expr(ast, *value);
                            self.match_irrefutable(ast, *pattern, v, super::patterns::Linkage::Declare, *value);
                        }
                    }
                }

                let label_list = std::mem::take(&mut self.pending_labels);
                match counted {
                    // N13: the list of the bound belongs to the runtime (the class of the block says so,
                    // `@df.core_len`) — the iterations use the length and the data
                    // read once, and the bounds checks of `v[i]`
                    // are proven by the condition; otherwise (empty, or a
                    // program class that implements `List`), the usual iterations,
                    // which read `length` on every iteration. N14: the fast
                    // iterations also require the shape of the elements that
                    // the static type asks for (compact for `List<int>`,
                    // `List<double>`, `List<bool>`; general for the others), and
                    // read it at emission; a list of another shape goes to the usual
                    // iterations, which check the shape at each access.
                    Some(Counted::CoreList { target, local }) => {
                        let shape = match self.indexable(self.ctx.get_type(self.unit_id, target)) {
                            Some(super::typed_items::Indexable::Core { write_op: Some(t) }) => super::typed_items::shape_code(t),
                            _ => super::typed_items::SHAPE_GENERAL,
                        };
                        let list = self.lower_expr(ast, target);
                        let list = self.coerce(list, Type::Ref);
                        // The length (0 for whatever is not a runtime list) and,
                        // once the class is checked, its storage and shape
                        // (unified space, §2.16): the body calls nothing, and
                        // neither the length nor the storage change during the iterations.
                        let n = self.core_length(&list);
                        let from_runtime = self.emit(Instruction::ICmp(ICmpOp::Ne, n.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                        let check_shape = self.new_block();
                        let fast = self.new_block();
                        let general = self.new_block();
                        let join = self.new_block();
                        self.terminate(Terminator::CondBranch { cond: from_runtime, then_block: check_shape, else_block: general });
                        self.set_block(check_shape);
                        let (data, f) = self.list_storage(&list);
                        let from_shape = self.emit(Instruction::ICmp(ICmpOp::Eq, f, Operand::Constant(Constant::Int(shape))), Type::I1);
                        self.terminate(Terminator::CondBranch { cond: from_shape, then_block: fast, else_block: general });
                        self.set_block(fast);
                        let writable_length =
                            (shape != super::typed_items::SHAPE_GENERAL).then(|| self.core_writable_length(&list, shape));
                        let key = match self.lookup_local(local).map(|l| l.mode) {
                            Some(super::locals::Mode::Memory(p)) => p,
                            _ => unreachable!("the list loop requires a local in memory"),
                        };
                        self.fixed_lists.push(FixedList { key, length: n, data, shape, writable_length });
                        self.for_iterations(ast, init.as_ref(), *condition, updates, *body, true, &label_list);
                        self.fixed_lists.pop();
                        self.terminate(Terminator::Branch(join));
                        self.set_block(general);
                        self.for_iterations(ast, init.as_ref(), *condition, updates, *body, false, &label_list);
                        self.terminate(Terminator::Branch(join));
                        self.set_block(join);
                    }
                    Some(Counted::TypedSet(t)) => self.versioned_typed_round_trips(ast, init.as_ref(), *condition, updates, *body, &t, &label_list),
                    _ => {
                        self.for_iterations(ast, init.as_ref(), *condition, updates, *body, counted.is_some(), &label_list);
                    }
                }
                self.close_scope();
            }
            StmtKind::Return(expr_opt) => {
                let op = expr_opt.map(|e| self.lower_expr(ast, e));
                self.route_return(op);
            }
            StmtKind::Break(lbl) => {
                let sym = lbl.as_ref().map(|n| n.sym);
                self.route_break_to(sym);
            }
            StmtKind::Continue(lbl) => {
                let sym = lbl.as_ref().map(|n| n.sym);
                self.route_continue_to(sym);
            }
            StmtKind::Assert { condition, message } => {
                self.lower_assert(ast, *condition, *message);
            }
            StmtKind::ForIn {
                await_: true,
                target,
                iterable,
                body,
            } => {
                self.lower_await_for(ast, target, *iterable, *body, stmt.span);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                // The `Iterator` protocol (§17.7.3).
                self.lower_source_for_in(ast, target, *iterable, *body, stmt.span);
            }
            StmtKind::Function(fid) => {
                if self.direct_local_function(*fid) {
                    self.declare_direct_function(ast, *fid, stmt.span);
                } else {
                    self.declare_local_function(ast, *fid, stmt.span);
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.lower_try_stmt(ast, *body, catches, *finally_);
            }
            StmtKind::Labeled { labels, body } => {
                let loop_fn = matches!(
                    ast.stmt(*body).kind,
                    StmtKind::While { .. }
                        | StmtKind::DoWhile { .. }
                        | StmtKind::For { .. }
                        | StmtKind::ForIn { .. }
                        | StmtKind::Switch { .. }
                        | StmtKind::Labeled { .. }
                );
                if loop_fn {
                    for l in labels.iter() {
                        self.pending_labels.push(l.sym);
                    }
                    self.lower_stmt(ast, *body);
                } else {
                    // `label: { … break label; … }`: the `break` leaves the
                    // labeled statement (§18.13 "Labels").
                    let exit = self.new_block();
                    for l in labels.iter() {
                        self.labeled_break_targets.insert(l.sym, exit);
                    }
                    self.lower_stmt(ast, *body);
                    self.terminate(Terminator::Branch(exit));
                    for l in labels.iter() {
                        self.labeled_break_targets.remove(&l.sym);
                    }
                    self.set_block(exit);
                }
            }
            StmtKind::Empty => {}
            StmtKind::PatternVariables { pattern, value, .. } => {
                let v = self.lower_expr(ast, *value);
                self.match_irrefutable(ast, *pattern, v, super::patterns::Linkage::Declare, *value);
            }
            StmtKind::Switch { value, cases } => {
                self.lower_switch_statement(ast, *value, cases);
            }
            StmtKind::Yield { star, value } => {
                self.lower_yield(ast, *star, *value, stmt.span);
            }
        }
    }

    fn lower_try_stmt(
        &mut self,
        ast: &ast::Ast,
        body: StmtId,
        catches: &[ast::CatchClause],
        finally_: Option<StmtId>,
    ) {
        let mut fin = finally_.map(|f| move |b: &mut Self| b.lower_stmt(ast, f));
        self.lower_try_with(
            ast,
            &mut |b: &mut Self| b.lower_stmt(ast, body),
            catches,
            fin.as_mut().map(|f| f as &mut dyn FnMut(&mut Self)),
        );
    }

    /// The `try` with the body and the `finally` given by the caller: that of the statement
    /// (`lower_try_stmt`) and the ones synthesized by the lowering (the `finally` that
    /// cancels the subscription of an `await for`).
    pub(super) fn lower_try_with(
        &mut self,
        ast: &ast::Ast,
        body: &mut dyn FnMut(&mut Self),
        catches: &[ast::CatchClause],
        mut finally_: Option<&mut dyn FnMut(&mut Self)>,
    ) {
        let try_body_block = self.new_block();
        let merge_block = self.new_block();

        let has_catches = !catches.is_empty();
        let catch_dispatch_block = if has_catches {
            Some(self.new_block())
        } else {
            None
        };

        let fin_info = if finally_.is_some() {
            let entry = self.new_block();
            let resume = self.new_block();
            let reason_phi = ValueId(self.next_value);
            self.next_value += 1;
            self.value_types.insert(reason_phi, Type::I64);

            let ret_val_phi = ValueId(self.next_value);
            self.next_value += 1;
            let ret_ty = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
            self.value_types.insert(ret_val_phi, ret_ty);

            Some((entry, resume, reason_phi, ret_val_phi))
        } else {
            None
        };

        if let Some((entry, resume, reason_phi, ret_val_phi)) = fin_info {
            self.finally_scopes.push(FinallyScope {
                entry_block: entry,
                resume_block: resume,
                reason_phi,
                ret_val_phi,
                incoming: Vec::new(),
                prof_break: self.break_targets.len(),
                prof_continue: self.continue_targets.len(),
                break_labels: self.labeled_break_targets.keys().copied().collect(),
                continue_labels: self.labeled_continue_targets.keys().copied().collect(),
                jumps: Vec::new(),
            });
        }

        // Landing pad of the exceptions that go to the `finally`: a single block, which
        // records the "exception" entry (2) of the phis of the `finally`. The exception
        // branches (`emit_call_with_check` with an exception target) do not
        // record a phi entry; going straight to the `finally` left the phi without
        // the entry of each of them (Clang rejects the module). It is also the
        // target of the exceptions inside the `catch`es: they go through the `finally`
        // before going up.
        let landing_pad = fin_info.map(|(entry, _, _, _)| {
            let p = self.new_block();
            let prev = self.current_block;
            self.set_block(p);
            let default_ret = self.default_return_operand();
            self.finally_scopes
                .last_mut()
                .expect("finally scope")
                .incoming
                .push((p, 2, default_ret));
            self.terminate(Terminator::Branch(entry));
            self.set_block(prev);
            p
        });

        let try_exc_target = if let Some(cb) = catch_dispatch_block {
            cb
        } else if let Some(p) = landing_pad {
            p
        } else if let Some(&parent_target) = self.exception_targets.last() {
            parent_target
        } else {
            let u = self.new_block();
            let prev = self.current_block;
            self.set_block(u);
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
            self.set_block(prev);
            u
        };

        self.exception_targets.push(try_exc_target);
        self.terminate(Terminator::Branch(try_body_block));

        self.set_block(try_body_block);
        body(self);

        if !self.is_terminated() {
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope
                    .incoming
                    .push((self.current_block, 0, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else {
                self.terminate(Terminator::Branch(merge_block));
            }
        }

        self.exception_targets.pop();

        if let Some(dispatch_block) = catch_dispatch_block {
            self.set_block(dispatch_block);
            // The exception as a reference (R5): `throw 42` arrives boxed, and
            // the `on T` tests and the `catch` variable are about a `Ref`.
            let ex_bits = self.pending_exception();

            // Isolate unwinding (`Isolate.exit`) is not catchable:
            // it goes straight to the "no clause matched" path.
            let capturable = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_catchable".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let capturable = self.emit(
                Instruction::ICmp(ICmpOp::Ne, capturable, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let first_test = self.new_block();
            let without_capture = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: capturable,
                then_block: first_test,
                else_block: without_capture,
            });
            let mut current_test_block = first_test;

            for clause in catches.iter() {
                self.set_block(current_test_block);
                let body_b = self.new_block();
                let next_test_b = self.new_block();

                if let Some(on_tid) = clause.on_type {
                    let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(on_tid);
                    let is_match = self.test_type(ast_ty, ex_bits.clone());
                    self.terminate(Terminator::CondBranch {
                        cond: is_match,
                        then_block: body_b,
                        else_block: next_test_b,
                    });
                } else {
                    self.terminate(Terminator::Branch(body_b));
                }

                self.set_block(body_b);
                // The explicit stack trace belongs to the pending exception. Read it
                // before `clear`, which releases the slot and the runtime root.
                let st_val = clause.stack_trace.as_ref().map(|_| self.pending_stack_trace());
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_clear".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );

                self.open_scope();
                if let Some(ex_name) = &clause.exception {
                    self.declare_variable(ex_name.sym, ex_name.span.start, Type::Ref, ex_bits.clone());
                }
                if let (Some(st_name), Some(st_val)) = (&clause.stack_trace, st_val) {
                    self.declare_variable(st_name.sym, st_name.span.start, Type::Ref, st_val);
                }

                self.active_catch_stack.push((ex_bits.clone(), 3));
                if let Some(p) = landing_pad {
                    self.exception_targets.push(p);
                }
                self.lower_stmt(ast, clause.body);
                if landing_pad.is_some() {
                    self.exception_targets.pop();
                }
                self.close_scope();
                self.active_catch_stack.pop();

                if !self.is_terminated() {
                    if let Some((entry, _, _, _)) = fin_info {
                        let default_ret = self.default_return_operand();
                        let last_scope = self.finally_scopes.last_mut().unwrap();
                        last_scope
                            .incoming
                            .push((self.current_block, 0, default_ret));
                        self.terminate(Terminator::Branch(entry));
                    } else {
                        self.terminate(Terminator::Branch(merge_block));
                    }
                }

                current_test_block = next_test_b;
            }

            self.set_block(without_capture);
            self.terminate(Terminator::Branch(current_test_block));
            self.set_block(current_test_block);
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope
                    .incoming
                    .push((self.current_block, 2, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else if let Some(&parent_target) = self.exception_targets.last() {
                self.terminate(Terminator::Branch(parent_target));
            } else {
                self.terminate(Terminator::Return(self.default_return_operand_opt()));
            }
        }

        if let Some(fin_body) = finally_.as_mut() {
            let (entry, _resume, reason_phi, ret_val_phi) = fin_info.unwrap();
            let scope = self.finally_scopes.pop().unwrap();

            self.set_block(entry);

            let header_idx = self.func.blocks.iter().position(|b| b.id == entry).unwrap();
            let mut reason_incoming = Vec::new();
            let mut ret_val_incoming = Vec::new();
            for (b, reason, ret_op) in &scope.incoming {
                reason_incoming.push((*b, Operand::Constant(Constant::Int(*reason))));
                ret_val_incoming.push((*b, ret_op.clone()));
            }

            self.func.blocks[header_idx].instructions.push((
                reason_phi,
                Instruction::Phi {
                    incoming: reason_incoming,
                    ty: Type::I64,
                },
                Type::I64,
            ));
            let ret_ty = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
            self.func.blocks[header_idx].instructions.push((
                ret_val_phi,
                Instruction::Phi {
                    incoming: ret_val_incoming,
                    ty: ret_ty,
                },
                ret_ty,
            ));

            // The exception that entered the `finally` (reason 2) is kept aside and leaves
            // the pending state while the body of the `finally` runs: otherwise its
            // first call sees the pending exception and branches, and the
            // `finally` does not run (it is the VM semantics: the body of the `finally`
            // runs normally and the exception comes back at the end, unless it
            // leaves through `return`/`break`/`continue`/`throw`, which discard it).
            let saved = self.pending_exception();
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_clear".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::Void,
                },
                Type::Void,
            );

            // Isolate unwinding (`kill`, `Isolate.exit`: the VM's `UnwindError`)
            // does not run the body of the `finally`, like the VM: the pending state
            // goes straight to the outer handler. Before, that depended on the
            // first checked call of the body seeing the pending state — and the effect
            // of a call that happens before the check escaped.
            let capturable = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_catchable".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let unwinding = self.emit(
                Instruction::ICmp(ICmpOp::Eq, capturable, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_body = self.new_block();
            let b_unwind = self.new_block();
            self.terminate(Terminator::CondBranch { cond: unwinding, then_block: b_unwind, else_block: b_body });
            self.set_block(b_unwind);
            if let Some(&parent_target) = self.exception_targets.last() {
                self.terminate(Terminator::Branch(parent_target));
            } else if !self.finally_scopes.is_empty() {
                let default_ret = self.default_return_operand();
                let parent_fin = self.finally_scopes.last_mut().unwrap();
                parent_fin.incoming.push((b_unwind, 2, default_ret));
                let p_entry = parent_fin.entry_block;
                self.terminate(Terminator::Branch(p_entry));
            } else {
                self.terminate(Terminator::Return(self.default_return_operand_opt()));
            }
            self.set_block(b_body);

            fin_body(self);

            if !self.is_terminated() {
                let b_norm = merge_block;
                let b_ret = self.new_block();
                let b_exc = self.new_block();
                let b_jumps: Vec<BlockId> = scope.jumps.iter().map(|_| self.new_block()).collect();
                let mut cases = vec![(0, b_norm), (1, b_ret), (2, b_exc)];
                for (k, b) in b_jumps.iter().enumerate() {
                    cases.push((5 + k as i64, *b));
                }
                self.terminate(Terminator::Switch {
                    val: Operand::Val(reason_phi),
                    default: b_norm,
                    cases,
                });

                self.set_block(b_ret);
                let ret_op = if self.func.return_ty == Type::Void {
                    None
                } else {
                    Some(Operand::Val(ret_val_phi))
                };
                self.route_return(ret_op);

                self.set_block(b_exc);
                self.emit_throw_value(saved, None);
                // Under ARC the throw ends at the guard: the branch leaves from it.
                let b_exc = self.current_block;
                // The exception goes up to the innermost handler: the current exception
                // target (an outer `catch`, or the landing pad of the
                // outer `finally`, which records the phi entry) comes before
                // an outer `finally` — that one is only the innermost when
                // there is no target.
                if let Some(&parent_target) = self.exception_targets.last() {
                    self.terminate(Terminator::Branch(parent_target));
                } else if !self.finally_scopes.is_empty() {
                    let default_ret = self.default_return_operand();
                    let parent_fin = self.finally_scopes.last_mut().unwrap();
                    parent_fin.incoming.push((b_exc, 2, default_ret));
                    let p_entry = parent_fin.entry_block;
                    self.terminate(Terminator::Branch(p_entry));
                } else {
                    self.terminate(Terminator::Return(self.default_return_operand_opt()));
                }

                // The jumps that crossed this `finally` continue.
                for (k, b) in b_jumps.into_iter().enumerate() {
                    self.set_block(b);
                    let (is_continue, label) = scope.jumps[k];
                    self.jump(is_continue, label);
                }
            }
        }

        self.set_block(merge_block);
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The body of a loop over the local list `list` (N13) calls nothing:
    /// only locals, literals, `int`/`double`/`bool` arithmetic and comparison,
    /// `if`/blocks/`break`/`continue`, and the index or `length` of the list itself
    /// or of `dart:typed_data` lists (final SDK classes). No
    /// path of it runs program code, so the length and the
    /// data of the list do not change during the iterations (the heap does not move objects).
    fn body_without_calls(&self, ast: &ast::Ast, body: StmtId, list: SymbolId, counter: SymbolId) -> bool {
        let mut declared_set: std::collections::HashSet<SymbolId> = [counter].into_iter().collect();
        self.statement_without_calls(ast, body, list, &mut declared_set)
    }

    fn statement_without_calls(
        &self,
        ast: &ast::Ast,
        s: StmtId,
        list: SymbolId,
        declared_set: &mut std::collections::HashSet<SymbolId>,
    ) -> bool {
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => ss.iter().all(|&x| self.statement_without_calls(ast, x, list, declared_set)),
            StmtKind::Expression(e) => self.expression_without_calls(ast, *e, list, declared_set),
            StmtKind::Variables(l) => {
                if l.late {
                    return false;
                }
                l.variables.iter().all(|v| {
                    let ok = v.initializer.is_none_or(|e| self.expression_without_calls(ast, e, list, declared_set))
                        && self.scalar(self.ctx.local_type(self.unit_id, v.name.span.start));
                    declared_set.insert(v.name.sym);
                    ok
                })
            }
            StmtKind::If { condition, then, else_, .. } => {
                self.expression_without_calls(ast, *condition, list, declared_set)
                    && self.statement_without_calls(ast, *then, list, declared_set)
                    && else_.is_none_or(|x| self.statement_without_calls(ast, x, list, declared_set))
            }
            StmtKind::Break(None) | StmtKind::Continue(None) => true,
            _ => false,
        }
    }

    /// `int`, `double` or `bool` (non-nullable): their operators belong to the
    /// compiler, they are not called.
    fn scalar(&self, t: Option<dartforge_types::TypeId>) -> bool {
        let c = &self.ctx.core;
        t.is_some_and(|t| t == c.int || t == c.double || t == c.bool_)
    }

    fn expression_without_calls(
        &self,
        ast: &ast::Ast,
        e: ExprId,
        list: SymbolId,
        declared_set: &std::collections::HashSet<SymbolId>,
    ) -> bool {
        use super::typed_items::Indexable;
        let ty = |x: ExprId| self.ctx.get_type(self.unit_id, x);
        let local = |n: &ast::Name| declared_set.contains(&n.sym) || self.lookup_local(n.sym).is_some();
        // The list itself, or one from `dart:typed_data`.
        let list_ok = |x: ExprId| match &ast.expr(x).kind {
            ExprKind::Identifier(n) if n.sym == list => true,
            ExprKind::Identifier(n) if local(n) => {
                matches!(self.indexable(ty(x)), Some(Indexable::Typed(_) | Indexable::Simd { .. }))
            }
            _ => false,
        };
        let sub = |x: ExprId| self.expression_without_calls(ast, x, list, declared_set);
        match &ast.expr(e).kind {
            ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) => true,
            ExprKind::Identifier(n) => local(n) && (self.scalar(ty(e)) || n.sym == list),
            ExprKind::Parenthesized(x) => sub(*x),
            ExprKind::Binary { op, left, right } => {
                !matches!(op, ast::BinaryOp::IfNull)
                    && self.scalar(ty(*left))
                    && self.scalar(ty(*right))
                    && sub(*left)
                    && sub(*right)
            }
            ExprKind::Unary { operand, .. } => self.scalar(ty(*operand)) && sub(*operand),
            ExprKind::Conditional { condition, then, else_ } => sub(*condition) && sub(*then) && sub(*else_),
            ExprKind::Index { target, index, null_aware: false } => {
                list_ok(*target) && self.scalar(ty(*index)) && sub(*index)
            }
            ExprKind::Property { target, name, null_aware: false } => {
                self.ctx.symbol_name(name.sym) == "length" && list_ok(*target)
            }
            ExprKind::Assign { target, value, .. } => {
                let target_ok = match &ast.expr(*target).kind {
                    // The type of the target of an assignment stays on the
                    // assignment itself (that of the result: the local's, in `s += x`).
                    ExprKind::Identifier(n) => local(n) && self.scalar(ty(e)),
                    ExprKind::Index { target: t, index, null_aware: false } => {
                        list_ok(*t) && self.scalar(ty(*index)) && sub(*index)
                    }
                    _ => false,
                };
                target_ok && self.scalar(ty(*value)) && sub(*value)
            }
            _ => false,
        }
    }

    /// The iterations of a `for` (the condition, the body and the updates), after
    /// the initialization; ends at the exit block. `counted`: the loop ends
    /// by itself and the iterations carry no safepoint (J01).
    #[allow(clippy::too_many_arguments)]
    fn for_iterations(
        &mut self,
        ast: &ast::Ast,
        init: Option<&ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
        counted: bool,
        label_list: &[SymbolId],
    ) {
        let loop_header = self.new_block();
        let loop_body = self.new_block();
        let loop_update = self.new_block();
        let exit_block = self.new_block();

        for &lbl in label_list {
            self.labeled_break_targets.insert(lbl, exit_block);
            self.labeled_continue_targets.insert(lbl, loop_update);
        }

        self.terminate(Terminator::Branch(loop_header));
        self.set_block(loop_header);

        if let Some(cond_id) = &condition {
            let cond_op = self.lower_expr(ast, *cond_id);
            let cond_op = self.to_bool(cond_op);
            self.terminate(Terminator::CondBranch {
                cond: cond_op,
                then_block: loop_body,
                else_block: exit_block,
            });
        } else {
            self.terminate(Terminator::Branch(loop_body));
        }

        self.break_targets.push(exit_block);
        self.continue_targets.push(loop_update);

        self.set_block(loop_body);
        if !counted {
            self.emit_safepoint();
        }
        self.lower_stmt(ast, body);
        self.terminate(Terminator::Branch(loop_update));

        self.set_block(loop_update);
        // A new variable per iteration (the `for` specification):
        // it only changes something for one that lives in a cell (P1).
        if let Some(ast::ForInit::Variables(var_list)) = init {
            for var in var_list.variables.iter() {
                self.renew_cell(var.name.sym);
            }
        }
        for &u in updates.iter() {
            self.lower_expr_discarded(ast, u);
        }
        self.terminate(Terminator::Branch(loop_header));

        self.break_targets.pop();
        self.continue_targets.pop();

        for &lbl in label_list {
            self.labeled_break_targets.remove(&lbl);
            self.labeled_continue_targets.remove(&lbl);
        }

        self.set_block(exit_block);
    }

    /// `for (int i = a; i < L; i++)` and its variations (`<=`, `>`, `>=`;
    /// `i += c`, `i -= c` with a positive literal `c` in the direction of the condition),
    /// with `i` not written elsewhere in the body nor in the condition, and the bound `L`
    /// invariant: a literal, a cell-less local that the loop does not write, or
    /// `v.length` of such a local `v` whose list has a fixed length (the
    /// `dart:typed_data` lists). `i` also cannot overflow when
    /// going past the last value: with an arbitrary `L`, only `<`/`>` with step 1.
    /// Such a loop ends: the back safepoint (J01) is dropped,
    /// as HotSpot drops that of counted loops.
    fn counted_loop(
        &self,
        ast: &ast::Ast,
        init: Option<&ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
    ) -> Option<Counted> {
        let Some(ast::ForInit::Variables(list)) = init else { return None };
        let [var] = &list.variables[..] else { return None };
        if list.late || var.initializer.is_none() {
            return None;
        }
        let i = var.name.sym;
        let int = self.ctx.core.int;
        if self.ctx.local_type(self.unit_id, var.name.span.start) != Some(int) {
            return None;
        }
        let cond = condition?;
        let ExprKind::Binary { op, left, right } = &ast.expr(cond).kind else { return None };
        let e_i = |e: ExprId| matches!(&ast.expr(e).kind, ExprKind::Identifier(n) if n.sym == i);
        // `i < L` or `L > i` (and the others): the direction in which `i` moves and whether the
        // comparison is strict.
        let (limit, increasing, strict) = match op {
            ast::BinaryOp::Lt if e_i(*left) => (*right, true, true),
            ast::BinaryOp::LtEq if e_i(*left) => (*right, true, false),
            ast::BinaryOp::Gt if e_i(*left) => (*right, false, true),
            ast::BinaryOp::GtEq if e_i(*left) => (*right, false, false),
            ast::BinaryOp::Gt if e_i(*right) => (*left, true, true),
            ast::BinaryOp::GtEq if e_i(*right) => (*left, true, false),
            ast::BinaryOp::Lt if e_i(*right) => (*left, false, true),
            ast::BinaryOp::LtEq if e_i(*right) => (*left, false, false),
            _ => return None,
        };
        let [u] = updates else { return None };
        let source = self.source();
        let literal = |e: ExprId| match &ast.expr(e).kind {
            ExprKind::Int(span) => source.get(span.start..span.end).and_then(|t| {
                let t = t.replace('_', "");
                match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).ok(),
                    None => t.parse::<i64>().ok(),
                }
            }),
            _ => None,
        };
        // The step, positive, in the direction of the comparison.
        let step = match &ast.expr(*u).kind {
            ExprKind::Unary { op: ast::UnaryOp::PostfixInc | ast::UnaryOp::PrefixInc, operand } if increasing && e_i(*operand) => 1,
            ExprKind::Unary { op: ast::UnaryOp::PostfixDec | ast::UnaryOp::PrefixDec, operand } if !increasing && e_i(*operand) => 1,
            ExprKind::Assign { op: ast::AssignOp::Compound(ast::BinaryOp::Add), target, value } if increasing && e_i(*target) => {
                literal(*value).unwrap_or(0)
            }
            ExprKind::Assign { op: ast::AssignOp::Compound(ast::BinaryOp::Sub), target, value } if !increasing && e_i(*target) => {
                literal(*value).unwrap_or(0)
            }
            _ => 0,
        };
        if step <= 0 {
            return None;
        }
        let stored_set = super::capture::stored_from_outside(self.ctx, self.unit_id, ast, body, &[cond]);
        if stored_set.contains(&i) {
            return None;
        }
        // A local that neither the body, nor the condition, nor any closure
        // writes: without a cell (`capture.rs`), the value does not change in the loop.
        let fixed_local = |e: ExprId| match &ast.expr(e).kind {
            ExprKind::Identifier(n) => {
                !stored_set.contains(&n.sym)
                    && self.lookup_local(n.sym).is_some_and(|l| {
                        matches!(l.mode, super::locals::Mode::Memory(_) | super::locals::Mode::Value(_) | super::locals::Mode::Environment { cell: false, .. })
                    })
            }
            _ => false,
        };
        // The last value `i` can have without leaving the loop is `L - 1` (strict)
        // or `L`; the loop ends if adding the step to it does not overflow — the
        // native `int` wraps around, and then the loop would not end.
        let without_overflow = |l: i64| {
            let last_one = if strict { if increasing { l.checked_sub(1) } else { l.checked_add(1) } } else { Some(l) };
            last_one.is_some_and(|u| if increasing { u.checked_add(step).is_some() } else { u.checked_sub(step).is_some() })
        };
        let simple = match &ast.expr(limit).kind {
            ExprKind::Int(_) => literal(limit).is_some_and(without_overflow),
            // An arbitrary value: only a strict step of 1 never overflows.
            ExprKind::Identifier(_) => {
                strict && step == 1 && fixed_local(limit) && self.ctx.get_type(self.unit_id, limit) == Some(int)
            }
            // The length of a typed list stays in `0..=2^60` (it fits in
            // addressable memory); the step does not make it overflow.
            ExprKind::Property { target, name, null_aware: false } if self.ctx.symbol_name(name.sym) == "length" => {
                let indexable = self.indexable(self.ctx.get_type(self.unit_id, *target));
                // N13: `List<E>` only ends by itself if the length does not change
                // during the iterations — no callee in the body — and the list belongs to the
                // runtime, which only execution tells (the iterations are duplicated).
                // Under ARC, the preparation holds the list at each access through the
                // storage read before the iterations (`optimize/arc/derived_set.rs`).
                if increasing
                    && strict
                    && step <= 1 << 60
                    && fixed_local(*target)
                    && matches!(indexable, Some(super::typed_items::Indexable::Core { .. }))
                    && let ExprKind::Identifier(n) = &ast.expr(*target).kind
                    && self.lookup_local(n.sym).is_some_and(|l| matches!(l.mode, super::locals::Mode::Memory(_)))
                    && self.body_without_calls(ast, body, n.sym, i)
                {
                    return Some(Counted::CoreList { target: *target, local: n.sym });
                }
                increasing
                    && step <= 1 << 60
                    && fixed_local(*target)
                    && matches!(indexable, Some(super::typed_items::Indexable::Typed(_) | super::typed_items::Indexable::Simd { .. }))
            }
            _ => false,
        };
        if !simple {
            return None;
        }
        if increasing
            && strict
            && step <= 1 << 60
            && let Some(t) = self.typed_loop(ast, i, limit, body, &stored_set)
        {
            return Some(Counted::TypedSet(t));
        }
        Some(Counted::Simple)
    }

    /// Loop versioning for typed lists: in an increasing, strict counted
    /// loop (`for (var i = a; i < L; i += p)`, `L` fixed during the
    /// iterations), the accesses `x[i]` to `dart:typed_data` lists kept in
    /// locals the loop does not write. The length and the address of the elements
    /// of a typed list do not change while it lives (not even through a call in the
    /// body), so it is enough to prove before the iterations that `0 <= a` and
    /// `L <= x.length` for each `x`: every `i` of the iterations stays in
    /// `a..L ⊆ 0..x.length`. The fast iterations read and write without bounds
    /// tests (without the slow path with a box and dispatch, which prevented LLVM
    /// from vectorizing); if the proof fails, the usual iterations. The body is
    /// lowered twice: only shapes that the lowering repeats without an emission
    /// effect (`duplicable`).
    fn typed_loop(
        &self,
        ast: &ast::Ast,
        i: SymbolId,
        limit: ExprId,
        body: StmtId,
        stored_set: &std::collections::HashSet<SymbolId>,
    ) -> Option<TypedRoundTrips> {
        use super::typed_items::Indexable;
        if self.async_state.is_some() || versioning_disabled() {
            return None;
        }
        // The bound is read once before the iterations, for the proof: not a `late`
        // local (the read may initialize it).
        if let ExprKind::Identifier(n) = &ast.expr(limit).kind
            && self.lookup_local(n.sym)?.late.is_some()
        {
            return None;
        }
        // The counter has not been declared yet (the analysis comes before the
        // initialization); its `alloca` is checked at emission.
        if !self.duplicable(ast, body, 0) {
            return None;
        }
        let mut accesses = Vec::new();
        self.indexed_accesses_stmt(ast, body, i, &mut accesses);
        let mut lists: Vec<LoopList> = Vec::new();
        for (target, write) in accesses {
            let ExprKind::Identifier(n) = &ast.expr(target).kind else { continue };
            if stored_set.contains(&n.sym) {
                continue;
            }
            let Some(local) = self.lookup_local(n.sym) else { continue };
            if local.late.is_some() || !matches!(local.mode, super::locals::Mode::Memory(_)) {
                continue;
            }
            // The type of the access is the declared one of the local (not a promotion,
            // which may only hold inside the body): the list already has that type
            // before the iterations.
            let ty = self.ctx.get_type(self.unit_id, target);
            let Some(off) = local.offset else { continue };
            if ty.is_none() || self.ctx.local_type(self.unit_id, off) != ty {
                continue;
            }
            if !matches!(self.indexable(ty), Some(Indexable::Typed(_) | Indexable::Simd { .. })) {
                continue;
            }
            match lists.iter_mut().find(|l| l.local == n.sym) {
                Some(l) => l.write |= write,
                None => lists.push(LoopList { target, local: n.sym, write }),
            }
        }
        (!lists.is_empty()).then_some(TypedRoundTrips { counter: i, limit, lists })
    }

    /// The accesses `x[i]` (`x` an identifier, `i` the counter) of statement
    /// `s`, with `true` when the access writes (assignment target, `++`/`--`).
    fn indexed_accesses_stmt(&self, ast: &ast::Ast, s: StmtId, i: SymbolId, out: &mut Vec<(ExprId, bool)>) {
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => ss.iter().for_each(|&x| self.indexed_accesses_stmt(ast, x, i, out)),
            StmtKind::Expression(e) => self.indexed_accesses_expr(ast, *e, i, false, out),
            StmtKind::Variables(l) => {
                for v in l.variables.iter() {
                    if let Some(e) = v.initializer {
                        self.indexed_accesses_expr(ast, e, i, false, out);
                    }
                }
            }
            StmtKind::If { condition, then, else_, .. } => {
                self.indexed_accesses_expr(ast, *condition, i, false, out);
                self.indexed_accesses_stmt(ast, *then, i, out);
                if let Some(x) = else_ {
                    self.indexed_accesses_stmt(ast, *x, i, out);
                }
            }
            StmtKind::Return(Some(e)) => self.indexed_accesses_expr(ast, *e, i, false, out),
            _ => {}
        }
    }

    fn indexed_accesses_expr(&self, ast: &ast::Ast, e: ExprId, i: SymbolId, writes: bool, out: &mut Vec<(ExprId, bool)>) {
        let sub = |x: ExprId, g: bool, out: &mut Vec<(ExprId, bool)>| self.indexed_accesses_expr(ast, x, i, g, out);
        match &ast.expr(e).kind {
            ExprKind::Index { target, index, null_aware: false } => {
                if matches!(&ast.expr(*target).kind, ExprKind::Identifier(_))
                    && matches!(&ast.expr(*index).kind, ExprKind::Identifier(n) if n.sym == i)
                {
                    out.push((*target, writes));
                }
                sub(*target, false, out);
                sub(*index, false, out);
            }
            ExprKind::Assign { target, value, .. } => {
                sub(*target, true, out);
                sub(*value, false, out);
            }
            ExprKind::Unary { op, operand } => {
                use ast::UnaryOp::*;
                let g = matches!(op, PrefixInc | PrefixDec | PostfixInc | PostfixDec);
                sub(*operand, g, out);
            }
            ExprKind::Parenthesized(x) => sub(*x, writes, out),
            ExprKind::Property { target, .. } => sub(*target, false, out),
            ExprKind::Call { target, arguments } => {
                sub(*target, false, out);
                arguments.args.iter().for_each(|a| sub(a.value, false, out));
            }
            ExprKind::InstanceCreation { arguments, .. } => arguments.args.iter().for_each(|a| sub(a.value, false, out)),
            ExprKind::Binary { left, right, .. } => {
                sub(*left, false, out);
                sub(*right, false, out);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                sub(*condition, false, out);
                sub(*then, false, out);
                sub(*else_, false, out);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } | ExprKind::Throw(value) => sub(*value, false, out),
            _ => {}
        }
    }

    /// Can statement `s` be lowered twice (the fast iterations and the usual
    /// ones of a versioned loop) without an emission effect beyond the repeated
    /// code: without closures and local functions (each lowering would emit their
    /// body), labels, `try`, `switch`, patterns, collection literals,
    /// `const`, `await`/`yield`, and without another loop inside (the copies would
    /// multiply; the inner loop is versioned by itself)?
    fn duplicable(&self, ast: &ast::Ast, s: StmtId, prof: u32) -> bool {
        if prof > 64 {
            return false;
        }
        let e = |x: ExprId| self.duplicable_expr(ast, x, prof + 1);
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => ss.iter().all(|&x| self.duplicable(ast, x, prof + 1)),
            StmtKind::Expression(x) => e(*x),
            StmtKind::Variables(l) => !l.late && !l.const_ && l.variables.iter().all(|v| v.initializer.is_none_or(e)),
            StmtKind::If { condition, case_pattern: None, guard: None, then, else_ } => {
                e(*condition) && self.duplicable(ast, *then, prof + 1) && else_.is_none_or(|x| self.duplicable(ast, x, prof + 1))
            }
            StmtKind::Break(None) | StmtKind::Continue(None) | StmtKind::Empty => true,
            StmtKind::Return(x) => x.is_none_or(e),
            _ => false,
        }
    }

    fn duplicable_expr(&self, ast: &ast::Ast, x: ExprId, prof: u32) -> bool {
        if prof > 64 {
            return false;
        }
        let e = |y: ExprId| self.duplicable_expr(ast, y, prof + 1);
        let args = |a: &ast::Arguments| a.type_args.is_empty() && a.args.iter().all(|a| e(a.value));
        match &ast.expr(x).kind {
            ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Identifier(_) | ExprKind::This => true,
            ExprKind::Parenthesized(y) | ExprKind::Throw(y) => e(*y),
            ExprKind::Property { target, .. } => e(*target),
            ExprKind::Index { target, index, .. } => e(*target) && e(*index),
            ExprKind::Call { target, arguments } => e(*target) && args(arguments),
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                !matches!(keyword, Some(ast::CreationKeyword::Const)) && args(arguments)
            }
            ExprKind::Unary { operand, .. } => e(*operand),
            ExprKind::Binary { left, right, .. } => e(*left) && e(*right),
            ExprKind::Conditional { condition, then, else_ } => e(*condition) && e(*then) && e(*else_),
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => e(*value),
            ExprKind::Assign { target, value, .. } => e(*target) && e(*value),
            _ => false,
        }
    }

    /// The iterations of a `for` with typed lists (`Counted::TypedSet`),
    /// versioned: before them, the length and the data of each list and the
    /// proof `0 <= i && L <= x.length` for all of them; proven, the fast
    /// iterations, in which `x[i]` and `x.length` use what was read; otherwise, the
    /// usual ones.
    #[allow(clippy::too_many_arguments)]
    fn versioned_typed_round_trips(
        &mut self,
        ast: &ast::Ast,
        init: Option<&ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
        t: &TypedRoundTrips,
        label_list: &[SymbolId],
    ) {
        use super::fn_builder::ProvenTyped;
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let Some(super::locals::Mode::Memory(counter)) = self.lookup_local(t.counter).map(|l| l.mode) else {
            // In a cell (captured): the usual iterations.
            self.for_iterations(ast, init, condition, updates, body, true, label_list);
            return;
        };
        let fast = self.new_block();
        let general = self.new_block();
        let join = self.new_block();
        let i0 = self.read_local_by_name(t.counter).expect("contador declarado");
        let i0 = self.coerce(i0, Type::I64);
        let b = self.lower_expr(ast, t.limit);
        let b = self.coerce(b, Type::I64);
        let mut proven_set = Vec::with_capacity(t.lists.len());
        let ok = self.emit(Instruction::ICmp(ICmpOp::Sge, i0, int(0)), Type::I1);
        let mut next = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: next, else_block: general });
        for l in &t.lists {
            self.set_block(next);
            let ix = self.indexable(self.ctx.get_type(self.unit_id, l.target)).expect("checked typed list");
            let Some(super::locals::Mode::Memory(key)) = self.lookup_local(l.local).map(|x| x.mode) else {
                unreachable!("list of the versioned loop in an `alloca`")
            };
            let list = self.read_local_by_name(l.local).expect("declared list");
            let (data, n) = self.proven_data(&list, ix, l.write).expect("typed or SIMD list");
            let fits = self.emit(Instruction::ICmp(ICmpOp::Sle, b.clone(), n.clone()), Type::I1);
            next = self.new_block();
            self.terminate(Terminator::CondBranch { cond: fits, then_block: next, else_block: general });
            proven_set.push(ProvenTyped { key, counter: counter.clone(), data, length: n, write: l.write });
        }
        self.set_block(next);
        self.terminate(Terminator::Branch(fast));

        self.set_block(fast);
        let before = self.proven_typed.len();
        self.proven_typed.extend(proven_set);
        self.for_iterations(ast, init, condition, updates, body, true, label_list);
        self.proven_typed.truncate(before);
        self.terminate(Terminator::Branch(join));

        self.set_block(general);
        self.for_iterations(ast, init, condition, updates, body, true, label_list);
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
    }
}

/// A `for` that ends by itself (J01): the bound is fixed, or (N13) it is the
/// `length` of a core list that the body cannot change — then the iterations
/// hold for the runtime list, and a program class that implements
/// `List` goes through the usual iterations; or it has accesses `x[i]` to typed
/// lists, and the iterations are versioned by the bounds proof
/// (`typed_loop`).
#[derive(Debug, Clone)]
pub(super) enum Counted {
    Simple,
    CoreList { target: ExprId, local: SymbolId },
    TypedSet(TypedRoundTrips),
}

/// A loop versioned by typed lists (`typed_loop`).
#[derive(Debug, Clone)]
pub(super) struct TypedRoundTrips {
    /// The counter `i`.
    counter: SymbolId,
    /// The `L` of `i < L`.
    limit: ExprId,
    lists: Vec<LoopList>,
}

/// A typed list with an `x[i]` access in a versioned loop.
#[derive(Debug, Clone)]
struct LoopList {
    /// An access to it (the identifier): the static type.
    target: ExprId,
    local: SymbolId,
    /// Algum acesso grava.
    write: bool,
}

/// `DARTFORGE_NO_TYPED_VERSIONING=1` at compile time turns off the
/// versioning of typed-list loops (`typed_loop`), to measure
/// before and after (it goes into the key of the source SDK, like the other
/// `DARTFORGE_NO_…`).
fn versioning_disabled() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| std::env::var_os("DARTFORGE_NO_TYPED_VERSIONING").is_some_and(|v| !v.is_empty() && v != "0"))
}
