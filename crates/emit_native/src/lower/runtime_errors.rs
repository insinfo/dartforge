//! The error classes the runtime represents (ids 1000–1012:
//! `Exception`, `StateError`, `ArgumentError`, `RangeError`…) created by the
//! code of the source libraries (P6): `new StateError(m)`,
//! `ArgumentError.value(v, n)`, `Error.throwWithStackTrace(e, s)`.
//!
//! While these classes do not come from source (P5d), an object of theirs is the
//! runtime's — it is what `catch (e) on ArgumentError` tests and what the runtime
//! prints —, so the construction is the same extern the runtime already uses for
//! the errors it throws (`dartforge_argument_error_value`…). It is not a new case
//! of matching by name (which left with the unified space): it is the runtime's
//! representation of the same classes, and it goes away with the 1000–1012 range in P5d.

use super::fn_builder::FnBuilder;
use super::members::Evaluated;
use crate::hir::*;
use dartforge_diagnostics::Span;

use crate::optimize::arc::ERRORS_WITH_ARC_VARIANT;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The arguments matched with the parameters of the outline of `fid`, by
    /// parameter name (absent = null).
    fn args_by_name(&self, fid: usize, evaluated: &[Evaluated]) -> Vec<(String, Operand)> {
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return Vec::new();
        };
        let mut positionals = evaluated.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| v.clone());
        data
            .parameters
            .iter()
            .map(|p| {
                let name = p.name.map(|s| self.ctx.symbol_name(s).to_string()).unwrap_or_default();
                let v = if p.kind == dartforge_frontend::ast::ParameterKind::Named {
                    evaluated.iter().find(|(n, _)| *n == p.name).map(|(_, v)| v.clone())
                } else {
                    positionals.next()
                };
                (name, v.unwrap_or(Operand::Constant(Constant::Null)))
            })
            .collect()
    }

    fn arg(args: &[(String, Operand)], i: usize) -> Operand {
        args.get(i).map_or(Operand::Constant(Constant::Null), |(_, v)| v.clone())
    }

    fn runtime_ref(&mut self, name: &str, args: Vec<(Operand, Type)>) -> Operand {
        self.new_runtime_error(name, args)
    }

    /// A new error from a runtime constructor (`dartforge_*_error_new`…).
    ///
    /// Under ARC, the pending state is checked and a (bits, is-reference) pair at the
    /// start of the arguments carries the value as `Ref`; the ARC preparation replaces it
    /// with the `dartforge_arc_<name>_owned_v1` variant (`excecoes.rs`). Without the
    /// preparation (the legacy CLI regime), the usual extern stays.
    pub fn new_runtime_error(&mut self, name: &str, mut args: Vec<(Operand, Type)>) -> Operand {
        let base = name.strip_prefix("dartforge_").unwrap_or(name);
        if !self.ctx.arc_memory || !ERRORS_WITH_ARC_VARIANT.contains(&base) {
            return self.emit(Instruction::CallRuntime { name: name.into(), args, ret_ty: Type::Ref }, Type::Ref);
        }
        // Under ARC the value of the (bits, is-reference) pair goes as `Ref` (the
        // mark stays 1) and the pending state is checked: the preparation replaces it with the
        // Owned variant (`optimize::arc::natives`), which runs Dart.
        if matches!(base, "exception_new" | "assertion_error_new" | "argument_error_value") {
            let value = self.coerce(args[0].0.clone(), Type::Ref);
            args[0].0 = value;
            args[1].0 = Operand::Constant(Constant::Int(1));
        }
        self.emit_call_with_check(Instruction::CallRuntime { name: name.into(), args, ret_ty: Type::Ref }, Type::Ref)
    }

    /// `C(...)`/`C.name(...)` of a runtime error class; `None` if it is not one.
    pub fn runtime_error_constructor(&mut self, fid: usize, evaluated: &[Evaluated]) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        let class = f.class?;
        if self.ctx.program.library(self.ctx.program.classes[class.0 as usize].library).uri != "dart:core" {
            return None;
        }
        let class_name = self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name).to_string();
        let ctor = self.ctx.symbol_name(f.name).to_string();
        let a = self.args_by_name(fid, evaluated);
        let r = |s: &mut Self, i: usize| {
            let v = Self::arg(&a, i);
            s.coerce(v, Type::Ref)
        };
        Some(match (class_name.as_str(), ctor.as_str()) {
            ("Exception", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_exception_new", vec![(m, Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8)])
            }
            ("StateError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_state_error_new", vec![(m, Type::Ref)])
            }
            ("UnsupportedError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_unsupported_error_new", vec![(m, Type::Ref)])
            }
            ("UnimplementedError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_unimplemented_error_new", vec![(m, Type::Ref)])
            }
            ("ArgumentError", "") => {
                let (m, n) = (r(self, 0), r(self, 1));
                self.runtime_ref("dartforge_argument_error_new", vec![(m, Type::Ref), (n, Type::Ref)])
            }
            ("ArgumentError", "value") => {
                let (v, n, m) = (r(self, 0), r(self, 1), r(self, 2));
                self.runtime_ref(
                    "dartforge_argument_error_value",
                    vec![(v, Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8), (n, Type::Ref), (m, Type::Ref)],
                )
            }
            ("ArgumentError", "notNull") => {
                let n = r(self, 0);
                self.runtime_ref("dartforge_argument_error_not_null", vec![(n, Type::Ref)])
            }
            ("TypeError", "") => self.runtime_ref("dartforge_type_error_new", Vec::new()),
            _ => return None,
        })
    }

    /// Static methods of the runtime error classes that the source
    /// code calls: `Error.throwWithStackTrace`, `ArgumentError.checkNotNull`,
    /// `RangeError.checkNotNegative`. `None` if it is not one of them.
    pub fn runtime_error_static(&mut self, fid: usize, evaluated: &[Evaluated], span: Span) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        let class = f.class?;
        if self.ctx.program.library(self.ctx.program.classes[class.0 as usize].library).uri != "dart:core" {
            return None;
        }
        let class_name = self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name).to_string();
        let method = self.ctx.symbol_name(f.name).to_string();
        let a = self.args_by_name(fid, evaluated);
        match (class_name.as_str(), method.as_str()) {
            ("Error", "throwWithStackTrace") => {
                let e = Self::arg(&a, 0);
                let e = self.coerce(e, Type::Ref);
                let s = Self::arg(&a, 1);
                let s = self.coerce(s, Type::Ref);
                self.emit_throw_value(e, Some(s));
                self.branch_to_exception_handler();
                Some(Operand::Constant(Constant::Null))
            }
            ("ArgumentError", "checkNotNull") => {
                let v = Self::arg(&a, 0);
                let v = self.coerce(v, Type::Ref);
                let n = Self::arg(&a, 1);
                let n = self.coerce(n, Type::Ref);
                let null = self.emit(Instruction::ICmp(ICmpOp::Eq, v.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                let b_error = self.new_block();
                let b_ok = self.new_block();
                self.terminate(Terminator::CondBranch { cond: null, then_block: b_error, else_block: b_ok });
                self.set_block(b_error);
                let e = self.runtime_ref("dartforge_argument_error_not_null", vec![(n, Type::Ref)]);
                self.emit_throw_op(e);
                self.set_block(b_ok);
                Some(v)
            }
            ("RangeError", "checkNotNegative") => {
                let v = Self::arg(&a, 0);
                let v = self.coerce(v, Type::I64);
                let n = Self::arg(&a, 1);
                let n = self.coerce(n, Type::Ref);
                let m = Self::arg(&a, 2);
                let m = self.coerce(m, Type::Ref);
                let neg = self.emit(Instruction::ICmp(ICmpOp::Slt, v.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                let b_error = self.new_block();
                let b_ok = self.new_block();
                self.terminate(Terminator::CondBranch { cond: neg, then_block: b_error, else_block: b_ok });
                self.set_block(b_error);
                let e = self.runtime_ref(
                    "dartforge_range_error_range",
                    vec![
                        (v.clone(), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                        (Operand::Constant(Constant::Int(i64::MAX)), Type::I64),
                        (n, Type::Ref),
                        (m, Type::Ref),
                    ],
                );
                self.emit_throw_op(e);
                self.set_block(b_ok);
                Some(v)
            }
            _ => {
                let _ = span;
                None
            }
        }
    }

    /// After an extern that left a pending exception: the branch to the
    /// current handler (the `finally`, or the exit of the function).
    /// Throws `value` (with the `StackTrace` `stack_trace`, if given) and leaves the current
    /// block ready for the branch to the handler.
    ///
    /// Under tracing, `dartforge_exception_throw` with the tag of the
    /// representation (1 `int`, 2 `bool`, 3 `Ref`, 4 `double`), or
    /// `dartforge_throw_with_stack_trace`. Under ARC, the value goes as a borrowed
    /// `Ref` (a scalar is boxed first) to
    /// `dartforge_arc_lancar_ref_v1`/`…_com_rastro_ref_v1`, which publish the
    /// exception's own root, followed by the pending guard that the
    /// ARC preparation recognizes; the path without a pending exception is unreachable.
    pub fn emit_throw_value(&mut self, value: Operand, stack_trace: Option<Operand>) {
        if self.ctx.arc_memory {
            let value = self.coerce(value, Type::Ref);
            let (name, mut args) = match stack_trace {
                Some(s) => {
                    let s = self.coerce(s, Type::Ref);
                    ("dartforge_arc_lancar_com_rastro_ref_v1", vec![(s, Type::Ref)])
                }
                None => ("dartforge_arc_lancar_ref_v1", Vec::new()),
            };
            args.insert(0, (value, Type::Ref));
            self.emit(Instruction::CallRuntime { name: name.into(), args, ret_ty: Type::Void }, Type::Void);
            let pending = self.emit(
                Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: Vec::new(), ret_ty: Type::I8 },
                Type::I8,
            );
            let threw = self.emit(Instruction::ICmp(ICmpOp::Ne, pending, Operand::Constant(Constant::Int(0))), Type::I1);
            let exits = self.new_block();
            let dead = self.new_block();
            self.terminate(Terminator::CondBranch { cond: threw, then_block: exits, else_block: dead });
            self.set_block(dead);
            self.terminate(Terminator::Unreachable);
            self.set_block(exits);
            return;
        }
        let (tag, bits) = match self.operand_type(&value) {
            Type::I64 => (1, value),
            Type::I1 => (2, self.emit(Instruction::ZExt { op: value, from: Type::I1, to: Type::I8 }, Type::I8)),
            Type::I8 => (2, value),
            Type::F64 => (4, value),
            _ => (3, value),
        };
        let tag = Operand::Constant(Constant::Int(tag));
        let (name, args) = match stack_trace {
            Some(s) => {
                let s = self.coerce(s, Type::Ref);
                ("dartforge_throw_with_stack_trace", vec![(bits, Type::I64), (tag, Type::I8), (s, Type::Ref)])
            }
            None => ("dartforge_exception_throw", vec![(bits, Type::I64), (tag, Type::I8)]),
        };
        self.emit(Instruction::CallRuntime { name: name.into(), args, ret_ty: Type::Void }, Type::Void);
    }

    /// The pending exception as a `Ref` (the `catch` variable, the one kept by the
    /// `finally`).
    pub fn pending_exception(&mut self) -> Operand {
        self.emit(Instruction::CallRuntime { name: "dartforge_exception_peek_ref".into(), args: Vec::new(), ret_ty: Type::Ref }, Type::Ref)
    }

    /// The `StackTrace` of the pending exception. The ARC preparation gives it (and the
    /// exception) an owner of its own, independent of the `clear` that follows.
    pub fn pending_stack_trace(&mut self) -> Operand {
        self.emit(Instruction::CallRuntime { name: "dartforge_stack_trace_get".into(), args: Vec::new(), ret_ty: Type::Ref }, Type::Ref)
    }

    pub fn branch_to_exception_handler(&mut self) {
        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }
        let dead = self.new_block();
        self.set_block(dead);
    }
}
