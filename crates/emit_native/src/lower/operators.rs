//! Binary operators over operands already in the representation of their type (R5).
//!
//! The choice of the operation comes from the representations: `I64` with `I64` is integer
//! arithmetic; an `F64` promotes the other side (`IntToDouble`, never `bitcast`);
//! equality involving a `Ref` goes to Dart's `==` through the dynamic class (the
//! inline null rule, `source_equality`); `identical` of a `Ref` is the
//! inline `@df.identico` (docs/NATIVO-ESPACO-UNIFICADO.md §2.10). Before,
//! "some side is a `Ref`" meant "concatenate strings", which only held
//! because everything that was not an `int` was a `Ref`.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::BinaryOp;

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn is_numeric_scalar(t: Type) -> bool {
        matches!(t, Type::I64 | Type::F64)
    }

    /// `identical(a, b)`: scalars by value (bits, for `double`); if
    /// some side is a `Ref`, through the inline `@df.identico` (equal handle, or
    /// `_Mint`/`_Double` of the same value — the VM's `Instance::IsIdenticalTo`;
    /// the helper is from P2, `llvm/ir_boxes.rs`).
    pub fn identical(&mut self, a: Operand, b: Operand) -> Operand {
        let (ta, tb) = (self.operand_type(&a), self.operand_type(&b));
        match (ta, tb) {
            (Type::I64, Type::I64) | (Type::I1, Type::I1) => {
                self.emit(Instruction::ICmp(ICmpOp::Eq, a, b), Type::I1)
            }
            (Type::F64, Type::F64) => {
                let x = self.emit(
                    Instruction::Bitcast {
                        op: a,
                        to: Type::I64,
                    },
                    Type::I64,
                );
                let y = self.emit(
                    Instruction::Bitcast {
                        op: b,
                        to: Type::I64,
                    },
                    Type::I64,
                );
                self.emit(Instruction::ICmp(ICmpOp::Eq, x, y), Type::I1)
            }
            _ if ta != Type::Ref && tb != Type::Ref => Operand::Constant(Constant::Bool(false)),
            _ => {
                let a = self.coerce(a, Type::Ref);
                let b = self.coerce(b, Type::Ref);
                self.emit(
                    Instruction::CallRuntime {
                        name: "df.identico".to_string(),
                        args: vec![(a, Type::Ref), (b, Type::Ref)],
                        ret_ty: Type::I1,
                    },
                    Type::I1,
                )
            }
        }
    }

    /// `a == b` with `a` of non-nullable static type `String` (the `==` of
    /// `String` cannot be overridden): the inline `@df.texto_igual_a` — the
    /// identity, the class of `b`, the length and the hash, and the code units in the
    /// runtime only when needed (P1, `llvm/ir_strings.rs`).
    pub fn string_equality(&mut self, a: Operand, b: Operand) -> Operand {
        let a = self.coerce(a, Type::Ref);
        let b = self.coerce(b, Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "df.texto_igual_a".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::I1,
            },
            Type::I1,
        )
    }

    /// A `Ref` operand whose static type is `int?`/`double?` (or `int`,
    /// `double`) goes back to the scalar through `Unbox` — the operator requires the value, and
    /// in a valid Dart program it is not null here (it was promoted).
    pub fn denullify_numeric(
        &mut self,
        e: dartforge_frontend::ast::ExprId,
        op: Operand,
    ) -> Operand {
        if self.operand_type(&op) != Type::Ref {
            return op;
        }
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return op;
        };
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(t) else {
            return op;
        };
        let cls = &self.ctx.program.classes[class.0 as usize];
        if self.ctx.compiled_library(cls.library) {
            return op;
        }
        match self.ctx.symbol_name(cls.name) {
            "int" => self.coerce(op, Type::I64),
            "double" => self.coerce(op, Type::F64),
            _ => op,
        }
    }

    /// `toString()` of a value in any representation.
    pub fn text_of(&mut self, op: Operand) -> Operand {
        let (name, ty) = match self.operand_type(&op) {
            Type::I64 => ("dartforge_to_string_i64", Type::I64),
            Type::F64 => ("dartforge_to_string_f64", Type::F64),
            Type::I1 | Type::I8 => ("dartforge_to_string_bool", Type::I8),
            _ => return self.string_by_selector(op),
        };
        self.emit(
            Instruction::CallRuntime {
                name: name.to_string(),
                args: vec![(op, ty)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `a == b`.
    fn equal(&mut self, a: Operand, b: Operand) -> Operand {
        let (ta, tb) = (self.operand_type(&a), self.operand_type(&b));
        if Self::is_numeric_scalar(ta) && Self::is_numeric_scalar(tb) {
            if ta == Type::I64 && tb == Type::I64 {
                return self.emit(Instruction::ICmp(ICmpOp::Eq, a, b), Type::I1);
            }
            let x = self.coerce(a, Type::F64);
            let y = self.coerce(b, Type::F64);
            return self.emit(Instruction::FCmp(FCmpOp::Eq, x, y), Type::I1);
        }
        if matches!(ta, Type::I1 | Type::I8) && matches!(tb, Type::I1 | Type::I8) {
            let x = self.coerce(a, Type::I1);
            let y = self.coerce(b, Type::I1);
            return self.emit(Instruction::ICmp(ICmpOp::Eq, x, y), Type::I1);
        }
        if ta != Type::Ref && tb != Type::Ref {
            // Scalars of different types (`1 == true`): never equal.
            return Operand::Constant(Constant::Bool(false));
        }
        let a = self.coerce(a, Type::Ref);
        let b = self.coerce(b, Type::Ref);
        // §17.26: with one side null it is `identical`; otherwise, `a.==(b)` through the
        // dynamic class of `a` (the `==` of the SDK source or of the program).
        self.source_equality(a, b)
    }

    /// The `int` operator that the `Smi` path computes inline.
    fn smi_op(op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
        )
    }

    // An interface that does not intersect the closed int family never
    // contains a Smi. Without a type or without the closure proof, keep the guard.
    // Includes the interfaces of the subtypes of int, not only those of int.
    pub(super) fn type_admits_smi(&self, ty: Option<dartforge_types::table::TypeId>) -> bool {
        let Some(ty) = ty else { return true; };
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(ty)
            else { return true; };
        let Some(int) = self.ctx.core.int_class else { return true; };
        if !super::sdk_source::class_closed_by_modifier(self.ctx, int) { return true; }
        self.ctx.subtypes(int).iter().any(|&c| super::members::subclass_of(self.ctx, c, *class))
    }

    /// `a op b` with some `Ref` side (`num`, `dynamic`, `Object`…): if both
    /// are small `int`s — only an `int` can be a `Smi` (R10), so the
    /// operator is the `int` one, whatever the static type —, the computation
    /// inline, with Dart's 64-bit overflow; otherwise the usual
    /// dispatch. It is the `this < other` of the SDK's `int.compareTo(num other)`.
    fn operate_smi(&mut self, op: BinaryOp, lop: Operand, rop: Operand, span: dartforge_diagnostics::Span) -> Operand {
        let one = Operand::Constant(Constant::Int(1));
        let mut conds = Vec::new();
        for x in [&lop, &rop] {
            if self.operand_type(x) == Type::Ref {
                // Under ARC, the tag bit through the helper with the borrowed `Ref`:
                // a Dart word does not become a scalar operand.
                let b = if self.ctx.arc_memory {
                    self.emit(Instruction::CallRuntime { name: "df.bit_smi".into(), args: vec![(x.clone(), Type::Ref)], ret_ty: Type::I64 }, Type::I64)
                } else {
                    self.emit(Instruction::And(x.clone(), one.clone()), Type::I64)
                };
                conds.push(b);
            }
        }
        let bits = match conds.len() {
            1 => conds.pop().expect("um"),
            _ => {
                let (a, b) = (conds[0].clone(), conds[1].clone());
                self.emit(Instruction::And(a, b), Type::I64)
            }
        };
        let smi = self.emit(Instruction::ICmp(ICmpOp::Ne, bits, Operand::Constant(Constant::Int(0))), Type::I1);
        let fast = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: smi, then_block: fast, else_block: slow });

        self.set_block(fast);
        let value = |s: &mut Self, x: &Operand| {
            if s.operand_type(x) == Type::Ref && s.ctx.arc_memory {
                s.emit(Instruction::CallRuntime { name: "df.valor_smi".into(), args: vec![(x.clone(), Type::Ref)], ret_ty: Type::I64 }, Type::I64)
            } else if s.operand_type(x) == Type::Ref {
                s.emit(Instruction::AShr(x.clone(), one.clone()), Type::I64)
            } else {
                x.clone()
            }
        };
        let (a, b) = (value(self, &lop), value(self, &rop));
        let comparison = matches!(op, BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq);
        let r = match op {
            BinaryOp::Add => self.emit(Instruction::Add(a, b), Type::I64),
            BinaryOp::Sub => self.emit(Instruction::Sub(a, b), Type::I64),
            BinaryOp::Mul => self.emit(Instruction::Mul(a, b), Type::I64),
            BinaryOp::BitAnd => self.emit(Instruction::And(a, b), Type::I64),
            BinaryOp::BitOr => self.emit(Instruction::Or(a, b), Type::I64),
            BinaryOp::BitXor => self.emit(Instruction::Xor(a, b), Type::I64),
            BinaryOp::Lt => self.emit(Instruction::ICmp(ICmpOp::Slt, a, b), Type::I1),
            BinaryOp::LtEq => self.emit(Instruction::ICmp(ICmpOp::Sle, a, b), Type::I1),
            BinaryOp::Gt => self.emit(Instruction::ICmp(ICmpOp::Sgt, a, b), Type::I1),
            _ => self.emit(Instruction::ICmp(ICmpOp::Sge, a, b), Type::I1),
        };
        // The result of the dispatch is a `Ref` (the sum of `num`s may be a
        // `double`); the comparison, a `bool`.
        let ty = if comparison { Type::I1 } else { Type::Ref };
        let r = self.coerce(r, ty);
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));

        self.set_block(slow);
        let s = self.operate_dynamic(op, lop, rop, span);
        let s = self.coerce(s, ty);
        let slow_end = self.current_block;
        if self.is_terminated() {
            self.set_block(join);
            return r;
        }
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(fast_end, r), (slow_end, s)], ty }, ty)
    }

    /// Binary operator (not short-circuit). `text`: some side is a
    /// `String` by its static type — `+` concatenates, `*` repeats.
    pub fn operate(
        &mut self,
        op: BinaryOp,
        lop: Operand,
        rop: Operand,
        text: bool,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        self.operate_with_smi(op, lop, rop, text, true, span)
    }

    // The source type may exclude the whole path. Callers without
    // that fact keep using operate, which preserves the dynamic guard.
    pub(super) fn operate_with_smi(
        &mut self, op: BinaryOp, lop: Operand, rop: Operand, text: bool,
        admits_smi: bool, span: dartforge_diagnostics::Span,
    ) -> Operand {
        if text && matches!(op, BinaryOp::Add) {
            let a = self.coerce(lop, Type::Ref);
            let b = self.coerce(rop, Type::Ref);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_concat".to_string(),
                    args: vec![(a, Type::Ref), (b, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        if text && matches!(op, BinaryOp::Mul) {
            // `String.*` from the SDK source (`_StringBase.operator *`).
            let a = self.coerce(lop, Type::Ref);
            let n = self.coerce(rop, Type::Ref);
            let r = self.call_by_name(a, super::sdk_source::AccessKind::Call, "*", &[(None, n)]);
            return self.coerce(r, Type::Ref);
        }
        match op {
            BinaryOp::Eq => return self.equal(lop, rop),
            BinaryOp::NotEq => {
                let e = self.equal(lop, rop);
                let e = self.to_bool(e);
                return self.emit(Instruction::LNot(e), Type::I1);
            }
            _ => {}
        }
        let (ta, tb) = (self.operand_type(&lop), self.operand_type(&rop));
        if !Self::is_numeric_scalar(ta) || !Self::is_numeric_scalar(tb) {
            if admits_smi && Self::smi_op(op) && matches!(ta, Type::I64 | Type::Ref) && matches!(tb, Type::I64 | Type::Ref) {
                return self.operate_smi(op, lop, rop, span);
            }
            return self.operate_dynamic(op, lop, rop, span);
        }
        let in_double = ta == Type::F64 || tb == Type::F64 || matches!(op, BinaryOp::Div);
        if in_double {
            let a = self.coerce(lop, Type::F64);
            let b = self.coerce(rop, Type::F64);
            let cmp = |b_: &mut Self, c: FCmpOp, x: Operand, y: Operand| {
                b_.emit(Instruction::FCmp(c, x, y), Type::I1)
            };
            return match op {
                BinaryOp::Add => self.emit(Instruction::FAdd(a, b), Type::F64),
                BinaryOp::Sub => self.emit(Instruction::FSub(a, b), Type::F64),
                BinaryOp::Mul => self.emit(Instruction::FMul(a, b), Type::F64),
                BinaryOp::Div => self.emit(Instruction::FDiv(a, b), Type::F64),
                BinaryOp::Rem => self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_nativo_DartForge_double_modulo".to_string(),
                        args: vec![(a, Type::F64), (b, Type::F64)],
                        ret_ty: Type::F64,
                    },
                    Type::F64,
                ),
                BinaryOp::TruncDiv => {
                    let q = self.emit(Instruction::FDiv(a, b), Type::F64);
                    self.emit(Instruction::DoubleToInt(q), Type::I64)
                }
                BinaryOp::Lt => cmp(self, FCmpOp::Lt, a, b),
                BinaryOp::LtEq => cmp(self, FCmpOp::Le, a, b),
                BinaryOp::Gt => cmp(self, FCmpOp::Gt, a, b),
                BinaryOp::GtEq => cmp(self, FCmpOp::Ge, a, b),
                _ => self.unsupported("operador de bits ou resto sobre double", span),
            };
        }
        let icmp = |b_: &mut Self, c: ICmpOp, x: Operand, y: Operand| {
            b_.emit(Instruction::ICmp(c, x, y), Type::I1)
        };
        match op {
            BinaryOp::Add => self.emit(Instruction::Add(lop, rop), Type::I64),
            BinaryOp::Sub => self.emit(Instruction::Sub(lop, rop), Type::I64),
            BinaryOp::Mul => self.emit(Instruction::Mul(lop, rop), Type::I64),
            BinaryOp::TruncDiv => self.emit_trunc_div(lop, rop),
            // `a % c` with a constant `c`: inline. Dart's `%` returns a
            // value in `[0, |c|)`: the truncated remainder plus `|c|` when it is
            // negative (`(r >> 63) & |c|`, without a branch). `c = ±1` always gives 0
            // (and avoids the `srem` of `i64::MIN` by `-1`, undefined in LLVM);
            // `c = 0` throws and `c = i64::MIN` has no `|c|`: they stay with the runtime.
            BinaryOp::Rem
                if matches!(rop, Operand::Constant(Constant::Int(c)) if c != 0 && c != i64::MIN) =>
            {
                let Operand::Constant(Constant::Int(c)) = rop else { unreachable!() };
                if c == 1 || c == -1 {
                    return Operand::Constant(Constant::Int(0));
                }
                let r = self.emit(Instruction::SRem(lop, Operand::Constant(Constant::Int(c))), Type::I64);
                let sign = self.emit(Instruction::AShr(r.clone(), Operand::Constant(Constant::Int(63))), Type::I64);
                let adjustment = self.emit(Instruction::And(sign, Operand::Constant(Constant::Int(c.abs()))), Type::I64);
                self.emit(Instruction::Add(r, adjustment), Type::I64)
            }
            BinaryOp::Rem => self.emit_module(lop, rop),
            BinaryOp::Shl => self.emit(Instruction::Shl(lop, rop), Type::I64),
            BinaryOp::Shr => self.emit(Instruction::AShr(lop, rop), Type::I64),
            BinaryOp::UShr => self.emit(Instruction::LShr(lop, rop), Type::I64),
            BinaryOp::BitAnd => self.emit(Instruction::And(lop, rop), Type::I64),
            BinaryOp::BitOr => self.emit(Instruction::Or(lop, rop), Type::I64),
            BinaryOp::BitXor => self.emit(Instruction::Xor(lop, rop), Type::I64),
            BinaryOp::Lt => icmp(self, ICmpOp::Slt, lop, rop),
            BinaryOp::LtEq => icmp(self, ICmpOp::Sle, lop, rop),
            BinaryOp::Gt => icmp(self, ICmpOp::Sgt, lop, rop),
            BinaryOp::GtEq => icmp(self, ICmpOp::Sge, lop, rop),
            BinaryOp::Div | BinaryOp::Eq | BinaryOp::NotEq => unreachable!("tratados acima"),
            BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull => {
                unreachable!("curto-circuito tem lowering próprio")
            }
            // `a ~ b` is a compile error (`not_binary_operator`).
            BinaryOp::NaoBinario => unreachable!("operador não binário"),
        }
    }
}
