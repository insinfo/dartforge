//! Members of `int` and `double` inline, when the static type of the
//! receiver is exactly `int` or `double` (non-nullable) — the `compareTo`
//! of `int` with an `int` argument included —, and the unboxed `add` of
//! `List<int>`/`List<double>`/`List<bool>`.
//!
//! The two classes cannot be extended nor implemented outside the
//! core: the value is always the number, and the member is the SDK one. Without this, an
//! `i.toDouble()` or `x.isEven` boxed the number, went through dispatch
//! by selector and unboxed the result. Each form here computes the
//! same value as the VM's `int_patch.dart`/`double_patch.dart`; what
//! may throw (`toInt()` of NaN or infinity) leaves through the SDK member on the
//! cold path.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_types::table::Type as T;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrimitiveKind {
    Int,
    Double,
}

fn c(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn primitive(&self, e: ExprId) -> Option<PrimitiveKind> {
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, e)?) else {
            return None;
        };
        if Some(*class) == self.ctx.core.int_class {
            Some(PrimitiveKind::Int)
        } else if Some(*class) == self.ctx.core.double_class {
            Some(PrimitiveKind::Double)
        } else {
            None
        }
    }

    /// `ClassID.cidX` (`dart:_internal`): in the VM, `static final` fields that the
    /// runtime fills with the class id of `_List`, `_OneByteString`…; the
    /// SDK compares `ClassID.getID(x)` with them to take shortcuts (the
    /// single-character `split`, the `_List` loops). Here the cid of those
    /// classes is fixed (`layout::cid`, docs/NATIVO-ESPACO-UNIFICADO.md §2.4), the
    /// same in the runtime, in the emitter and in every generation: the contract constant.
    /// `numPredefinedCids` is the first free cid (the runtime classes stay
    /// below, like the VM's predefined ones in the constants hash,
    /// `compact_hash.dart`). The VM's external classes (`cidExternal*`) do not
    /// exist here: the field stays as it is (0, which no object has).
    fn runtime_class_id(&self, ast: &ast::Ast, e: ExprId) -> Option<i64> {
        use dartforge_runtime::layout::{cid, FIRST_FREE_CID};
        use dartforge_runtime::typed_lists::ty;
        let ExprKind::Property { name, null_aware: false, .. } = &ast.expr(e).kind else { return None };
        let Some(dartforge_types::resolved::Resolved::Member { class, .. }) = self.ctx.get_resolved(self.unit_id, e) else {
            return None;
        };
        if Some(*class) != self.ctx.sdk_class("_internal", "ClassID") {
            return None;
        }
        let id = match self.ctx.symbol_name(name.sym) {
            "cidArray" => cid::LIST,
            "cidGrowableObjectArray" => cid::GROWABLE_LIST,
            "cidImmutableArray" => cid::IMMUTABLE_LIST,
            "cidOneByteString" => cid::ONE_BYTE_STRING,
            "cidTwoByteString" => cid::TWO_BYTE_STRING,
            "cidUint8ArrayView" => cid::view(ty::UINT8, false),
            "cidUint8Array" => cid::typed(ty::UINT8),
            "cidInt8ArrayView" => cid::view(ty::INT8, false),
            "cidInt8Array" => cid::typed(ty::INT8),
            "cidUint8ClampedArray" => cid::typed(ty::UINT8_CLAMPED),
            "numPredefinedCids" => return Some(FIRST_FREE_CID),
            _ => return None,
        };
        Some(i64::from(id))
    }

    /// The `int`/`double` member inline, or `None` (the usual path).
    pub(super) fn intrinsic_expression(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if let Some(id) = self.runtime_class_id(ast, e) {
            return Some(self.emit(Instruction::Const(Constant::Int(id)), Type::I64));
        }
        // A member of an extension type: the receiver has the erased type of a
        // primitive or of a list, but the member is that of the extension type.
        let member = match &ast.expr(e).kind {
            ExprKind::Call { target, .. } => *target,
            _ => e,
        };
        if self.member_te(member).is_some() {
            return None;
        }
        if let Some(r) = self.list_add_scalar(ast, e) {
            return Some(r);
        }
        if let Some(r) = self.fill_typed(ast, e) {
            return Some(r);
        }
        if let Some(r) = self.compare_to_int(ast, e) {
            return Some(r);
        }
        if let Some(r) = self.num_to_int(ast, e) {
            return Some(r);
        }
        let (recv, name_str, call) = match &ast.expr(e).kind {
            ExprKind::Property { target, name, null_aware: false } => (*target, name.sym, false),
            ExprKind::Call { target, arguments } if arguments.args.is_empty() && arguments.type_args.is_empty() => {
                match &ast.expr(*target).kind {
                    ExprKind::Property { target, name, null_aware: false } => (*target, name.sym, true),
                    _ => return None,
                }
            }
            _ => return None,
        };
        let p = self.primitive(recv)?;
        let name_str = self.ctx.symbol_name(name_str);
        let shape = match (p, name_str, call) {
            (PrimitiveKind::Int, "isEven" | "isOdd" | "isNegative", false) => name_str,
            (PrimitiveKind::Int, "toDouble" | "abs" | "toInt" | "floor" | "ceil" | "round" | "truncate", true) => name_str,
            (PrimitiveKind::Double, "isNaN" | "isInfinite" | "isFinite" | "isNegative", false) => name_str,
            (PrimitiveKind::Double, "toDouble" | "abs" | "toInt" | "truncate", true) => name_str,
            _ => return None,
        };
        let v = self.lower_expr(ast, recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        Some(match p {
            PrimitiveKind::Int => self.int_intrinsic(v, shape),
            PrimitiveKind::Double => self.double_intrinsic(v, shape),
        })
    }

    /// `a.compareTo(b)` with `a` and `b` exactly `int` (non-nullable): the
    /// VM's `_IntegerImplementation.compareTo` with an integer `other` is `-1`
    /// if `a < b`, `1` if `a > b`, otherwise `0`; here, `(a > b) - (a < b)`
    /// inline. Without this, both numbers went boxed (a new `_Mint`
    /// for each outside the `Smi` range) to dispatch by selector and to the uniform
    /// entry, which checked the argument through the RTI, and the `this < other` of the
    /// body (`num other`) went to dynamic dispatch for every `_Mint` — the
    /// comparator `(a, b) => a.compareTo(b)` of `sort` paid all of that on
    /// every comparison. `double` (and `num`) keep the SDK member:
    /// NaN and `-0.0` have their own rules.
    fn compare_to_int(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else { return None };
        if arguments.args.len() != 1 || arguments.args[0].name.is_some() || !arguments.type_args.is_empty() {
            return None;
        }
        let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
            return None;
        };
        if self.ctx.symbol_name(name.sym) != "compareTo" {
            return None;
        }
        let arg = arguments.args[0].value;
        if self.primitive(*recv) != Some(PrimitiveKind::Int) || self.primitive(arg) != Some(PrimitiveKind::Int) {
            return None;
        }
        let a = self.lower_expr(ast, *recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let a = self.coerce(a, Type::I64);
        let b = self.lower_expr(ast, arg);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let b = self.coerce(b, Type::I64);
        let larger = self.emit(Instruction::ICmp(ICmpOp::Sgt, a.clone(), b.clone()), Type::I1);
        let smaller = self.emit(Instruction::ICmp(ICmpOp::Slt, a, b), Type::I1);
        let larger = self.emit(Instruction::ZExt { op: larger, from: Type::I1, to: Type::I64 }, Type::I64);
        let smaller = self.emit(Instruction::ZExt { op: smaller, from: Type::I1, to: Type::I64 }, Type::I64);
        Some(self.emit(Instruction::Sub(larger, smaller), Type::I64))
    }

    /// `n.toInt()` (and `truncate()`) with `n` statically `num`
    /// (non-nullable): `num` is sealed, so the value is an `int` — a `Smi` or
    /// a `_Mint`, the result itself — or a `_Double`, which takes the guarded
    /// `fptosi` of [`Self::double_intrinsic`] (NaN and the values outside
    /// `[-2^63, 2^63)` keep going through the SDK member, which throws the
    /// `UnsupportedError`). Before, a `num` receiver went through the dispatch
    /// by class to `_Double.toInt`, a call into the runtime, on every
    /// conversion of the dart_ui color kernel.
    fn num_to_int(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        use dartforge_runtime::layout::cid;
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else { return None };
        if !arguments.args.is_empty() || !arguments.type_args.is_empty() {
            return None;
        }
        let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
            return None;
        };
        let shape = self.ctx.symbol_name(name.sym);
        if shape != "toInt" && shape != "truncate" {
            return None;
        }
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, *recv)?) else {
            return None;
        };
        if Some(*class) != self.ctx.core.num_class || self.ctx.arc_memory {
            return None;
        }
        let v = self.lower_expr(ast, *recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let v = self.coerce(v, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(v.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let is_double = self.emit(Instruction::ICmp(ICmpOp::Eq, cls, c(i64::from(cid::DOUBLE))), Type::I1);
        let b_double = self.new_block();
        let b_int = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: is_double, then_block: b_double, else_block: b_int });
        self.set_block(b_double);
        let d = self.double_intrinsic(v.clone(), "toInt");
        let mut incoming = Vec::new();
        if !self.is_terminated() {
            incoming.push((self.current_block, d));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(b_int);
        let i = self.coerce(v, Type::I64);
        incoming.push((self.current_block, i));
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        Some(self.emit(Instruction::Phi { incoming, ty: Type::I64 }, Type::I64))
    }

    /// `list.add(v)` with a list of static type `List<int>`,
    /// `List<double>` or `List<bool>` and `v` exactly of that type (unboxed),
    /// like the VM's inline `add`: in a `_GrowableList` (cid 10) with
    /// compact storage of the value's shape (docs/NATIVO-ESPACO-UNIFICADO.md
    /// §2.16: the compact shape only exists when the reified `E` is the scalar)
    /// with room, the word written directly and the length plus one; when full,
    /// `dartforge_list_append` (grows to `(capacity * 2) | 3`);
    /// any other (a user class, fixed size, unmodifiable,
    /// general storage) calls the usual `add`. Without this, each `add`
    /// went through dispatch, checked `E` twice through the RTI (at the uniform
    /// entry and in the `[]=` inside `add`) and wrote the boxed value.
    fn list_add_scalar(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        use dartforge_runtime::layout::{cid, flags};
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else { return None };
        if arguments.args.len() != 1 || arguments.args[0].name.is_some() || !arguments.type_args.is_empty() {
            return None;
        }
        let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
            return None;
        };
        if self.ctx.symbol_name(name.sym) != "add" {
            return None;
        }
        let Some(super::typed_items::Indexable::Core { write_op: Some(t) }) =
            self.indexable(self.ctx.get_type(self.unit_id, *recv))
        else {
            return None;
        };
        // The argument must be of the element type, non-nullable: a
        // `dynamic` or `num` would need the checked implicit conversion.
        let arg = arguments.args[0].value;
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, arg)?) else {
            return None;
        };
        let (expected, shape) = match t {
            Type::I64 => (self.ctx.core.int_class, flags::RAW | flags::ELEMENT_INT),
            Type::F64 => (self.ctx.core.double_class, flags::RAW | flags::ELEMENT_DOUBLE),
            _ => (self.ctx.core.bool_class, flags::RAW | flags::ELEMENT_BOOL),
        };
        if Some(*class) != expected {
            return None;
        }
        let list = self.lower_expr(ast, *recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let list = self.coerce(list, Type::Ref);
        let v = self.lower_expr(ast, arg);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let v = self.coerce(v, t);
        // The word of the shape: the `i64`, the `double` (the bits, through the `F64`
        // write) or 0/1.
        let (word, ty) = match t {
            Type::I64 => (v.clone(), CType::I64),
            Type::F64 => (v.clone(), CType::F64),
            _ => (self.emit(Instruction::ZExt { op: v.clone(), from: Type::I1, to: Type::I64 }, Type::I64), CType::I64),
        };
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(list.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let growable = self.emit(Instruction::ICmp(ICmpOp::Eq, cls, c(i64::from(cid::GROWABLE_LIST))), Type::I1);
        let b_exp = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: growable, then_block: b_exp, else_block: slow });

        self.set_block(b_exp);
        let (_done, full) = self.append_scalar_inline(list.clone(), word, ty, shape, slow);
        self.terminate(Terminator::Branch(join));

        // Full: the growth (and the write) in the runtime, with the value in a
        // box (the `Smi`, without allocation, in the common `int` case).
        self.set_block(full);
        let boxed = self.coerce(v.clone(), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_list_append".to_string(),
                args: vec![(list.clone(), Type::Ref), (boxed, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Branch(join));

        self.set_block(slow);
        self.call_by_name(list, super::sdk_source::AccessKind::Call, "add", &[(None, v)]);
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(Operand::Constant(Constant::Null))
    }

    fn int_intrinsic(&mut self, v: Operand, shape: &str) -> Operand {
        let x = self.coerce(v, Type::I64);
        match shape {
            "isEven" | "isOdd" => {
                let bit = self.emit(Instruction::And(x, c(1)), Type::I64);
                let op = if shape == "isEven" { ICmpOp::Eq } else { ICmpOp::Ne };
                self.emit(Instruction::ICmp(op, bit, c(0)), Type::I1)
            }
            "isNegative" => self.emit(Instruction::ICmp(ICmpOp::Slt, x, c(0)), Type::I1),
            "toDouble" => self.emit(Instruction::IntToDouble(x), Type::F64),
            // `(x ^ s) - s` with `s = x >> 63`: the `abs` of `int` with the
            // VM's 64-bit overflow (`abs(-2^63) == -2^63`).
            "abs" => {
                let s = self.emit(Instruction::AShr(x.clone(), c(63)), Type::I64);
                let t = self.emit(Instruction::Xor(x, s.clone()), Type::I64);
                self.emit(Instruction::Sub(t, s), Type::I64)
            }
            // `toInt`, `floor`, `ceil`, `round`, `truncate` of `int`: itself.
            _ => x,
        }
    }

    fn double_intrinsic(&mut self, v: Operand, shape: &str) -> Operand {
        let boxed = v.clone();
        let x = self.coerce(v, Type::F64);
        let bits = |s: &mut Self, x: Operand| s.emit(Instruction::Bitcast { op: x, to: Type::I64 }, Type::I64);
        match shape {
            "isNaN" => {
                let equal = self.emit(Instruction::FCmp(FCmpOp::Eq, x.clone(), x), Type::I1);
                self.emit(Instruction::LNot(equal), Type::I1)
            }
            // Exponent all ones and zero mantissa.
            "isInfinite" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Eq, m, c(0x7FF0_0000_0000_0000)), Type::I1)
            }
            // Exponent not all ones (neither infinity nor NaN).
            "isFinite" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Slt, m, c(0x7FF0_0000_0000_0000)), Type::I1)
            }
            // The sign bit, except for NaN (`-0.0.isNegative` is true;
            // `double.nan.isNegative`, false).
            "isNegative" => {
                let b = bits(self, x.clone());
                let sign = self.emit(Instruction::ICmp(ICmpOp::Slt, b, c(0)), Type::I1);
                let not_nan = self.emit(Instruction::FCmp(FCmpOp::Eq, x.clone(), x), Type::I1);
                let s = self.emit(Instruction::ZExt { op: sign, from: Type::I1, to: Type::I64 }, Type::I64);
                let n = self.emit(Instruction::ZExt { op: not_nan, from: Type::I1, to: Type::I64 }, Type::I64);
                let e = self.emit(Instruction::And(s, n), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Ne, e, c(0)), Type::I1)
            }
            "toDouble" => x,
            "abs" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::Bitcast { op: m, to: Type::F64 }, Type::F64)
            }
            // `toInt`/`truncate`: in [-2^63, 2^63) the `fptosi`; outside (and NaN),
            // the SDK member, which throws the `UnsupportedError`.
            _ => {
                let lo = self.emit(Instruction::FCmp(FCmpOp::Ge, x.clone(), Operand::Constant(Constant::Double(-(2f64.powi(63))))), Type::I1);
                let hi = self.emit(Instruction::FCmp(FCmpOp::Lt, x.clone(), Operand::Constant(Constant::Double(2f64.powi(63)))), Type::I1);
                let lo = self.emit(Instruction::ZExt { op: lo, from: Type::I1, to: Type::I64 }, Type::I64);
                let hi = self.emit(Instruction::ZExt { op: hi, from: Type::I1, to: Type::I64 }, Type::I64);
                let ok = self.emit(Instruction::And(lo, hi), Type::I64);
                let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, ok, c(0)), Type::I1);
                let fast = self.new_block();
                let slow = self.new_block();
                let join = self.new_block();
                self.terminate(Terminator::CondBranch { cond: ok, then_block: fast, else_block: slow });
                self.set_block(fast);
                let r = self.emit(Instruction::DoubleToInt(x.clone()), Type::I64);
                let fast_end = self.current_block;
                self.terminate(Terminator::Branch(join));
                self.set_block(slow);
                let boxed = self.coerce(boxed, Type::Ref);
                let s = self.call_by_name(boxed, super::sdk_source::AccessKind::Call, shape, &[]);
                let s = self.coerce(s, Type::I64);
                let slow_end = self.current_block;
                if self.is_terminated() {
                    self.set_block(join);
                    return r;
                }
                self.terminate(Terminator::Branch(join));
                self.set_block(join);
                self.emit(Instruction::Phi { incoming: vec![(fast_end, r), (slow_end, s)], ty: Type::I64 }, Type::I64)
            }
        }
    }
}
