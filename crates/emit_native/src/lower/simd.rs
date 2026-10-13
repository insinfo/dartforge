//! Unboxed `Float32x4`, `Int32x4` and `Float64x2` (`docs/SIMD-NATIVO.md`).
//!
//! The SIMD locals of a synchronous function that are not captured keep the
//! vector (`Type::V4F32`…), and the SIMD expressions the compiler recognizes
//! (operators, methods, getters and constructors of the SDK, `a[i]` of a SIMD
//! list) become vector instructions (`Instruction::Simd`, emitted in
//! `llvm/simd.rs`). The box (`Ref`) stays the value at every
//! boundary — parameters, returns, fields, collections, `dynamic`: the
//! general `lower_expr` coerces to the static type, which is `Ref`, and only
//! `FnBuilder::lower_simd` hands over the vector.
//!
//! What is not recognized (a `shuffle` mask that is not constant, a
//! member without a vector form) follows the usual path, through the box and the
//! runtime's `simd.rs`, with the same semantics.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_types::resolved::Resolved;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

/// How an argument enters the operation.
#[derive(Clone, Copy)]
enum Arg {
    /// A vector of the given type (through [`FnBuilder::lower_simd`]).
    Vector(Type),
    /// A scalar in the given representation (`double`, `int`, `bool`).
    Scalar(Type),
}

/// A SIMD expression with an unboxed form: the operation, the receiver (if any),
/// the arguments and the result type.
struct Recipe {
    op: OpSimd,
    receiver: Option<(ExprId, Type)>,
    args: Vec<(ExprId, Arg)>,
    result: Type,
}

/// The lane of `x`/`y`/`z`/`w`.
fn lane(c: char) -> Option<u8> {
    match c {
        'x' => Some(0),
        'y' => Some(1),
        'z' => Some(2),
        'w' => Some(3),
        _ => None,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The unboxed vector of static type `ty`, if it is a non-nullable `Float32x4`,
    /// `Int32x4` or `Float64x2` and the function is synchronous (an
    /// asynchronous frame keeps 64 bits per position).
    pub(super) fn simd_type(&self, ty: Option<TypeId>) -> Option<Type> {
        if self.async_state.is_some() {
            return None;
        }
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = &self.ctx.program.classes[class.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:typed_data" {
            return None;
        }
        match self.ctx.symbol_name(c.name) {
            "Float32x4" | "_Float32x4" => Some(Type::V4F32),
            "Int32x4" | "_Int32x4" => Some(Type::V4I32),
            "Float64x2" | "_Float64x2" => Some(Type::V2F64),
            _ => None,
        }
    }

    /// The unboxed vector of the static type of expression `e`.
    pub(super) fn expr_simd(&self, e: ExprId) -> Option<Type> {
        self.simd_type(self.ctx.get_type(self.unit_id, e))
    }

    /// The constant mask of a `shuffle`: a `0..=255` literal, or one of the
    /// `xyzw` constants of the SIMD classes (the name gives the lanes).
    fn constant_mask(&self, ast: &ast::Ast, e: ExprId) -> Option<u8> {
        match &ast.expr(e).kind {
            ExprKind::Parenthesized(x) => self.constant_mask(ast, *x),
            ExprKind::Int(span) => {
                let text = self.source()[span.start..span.end].replace('_', "");
                let v = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).ok()?,
                    None => text.parse::<i64>().ok()?,
                };
                u8::try_from(v).ok()
            }
            ExprKind::Property { target, name, .. } => {
                let ExprKind::Identifier(c) = &ast.expr(*target).kind else { return None };
                if !matches!(self.ctx.symbol_name(c.sym), "Float32x4" | "Int32x4") {
                    return None;
                }
                let name_str = self.ctx.symbol_name(name.sym);
                let lanes: Vec<u8> = name_str.chars().map(lane).collect::<Option<_>>()?;
                if lanes.len() != 4 {
                    return None;
                }
                Some(lanes[0] | (lanes[1] << 2) | (lanes[2] << 4) | (lanes[3] << 6))
            }
            _ => None,
        }
    }

    /// The unboxed form of expression `e`, if the compiler recognizes it.
    fn simd_recipe(&self, ast: &ast::Ast, e: ExprId) -> Option<Recipe> {
        use Type::{V2F64, V4F32, V4I32};
        if let Some(r) = self.recipe_api_314(ast, e) {
            return Some(r);
        }
        let expr = ast.expr(e);
        match &expr.kind {
            ExprKind::Binary { op, left, right } => {
                let k = self.expr_simd(*left)?;
                let op = match (op, k) {
                    (BinaryOp::Add, _) => OpSimd::Add,
                    (BinaryOp::Sub, _) => OpSimd::Sub,
                    (BinaryOp::Mul, V4F32 | V2F64) => OpSimd::Mul,
                    (BinaryOp::Div, V4F32 | V2F64) => OpSimd::Div,
                    (BinaryOp::BitAnd, V4I32) => OpSimd::And,
                    (BinaryOp::BitOr, V4I32) => OpSimd::Or,
                    (BinaryOp::BitXor, V4I32) => OpSimd::Xor,
                    _ => return None,
                };
                (self.expr_simd(*right)? == k).then_some(())?;
                Some(Recipe { op, receiver: Some((*left, k)), args: vec![(*right, Arg::Vector(k))], result: k })
            }
            ExprKind::Unary { op: UnaryOp::Neg, operand } => {
                let k = self.expr_simd(*operand)?;
                (k != V4I32).then_some(())?;
                Some(Recipe { op: OpSimd::Neg, receiver: Some((*operand, k)), args: Vec::new(), result: k })
            }
            ExprKind::Property { target, name, null_aware: false } => {
                let k = self.expr_simd(*target)?;
                let name_str = self.ctx.symbol_name(name.sym);
                let scalar = if k == V4I32 { Type::I64 } else { Type::F64 };
                let (op, result) = match name_str {
                    "signMask" => (OpSimd::SignMask, Type::I64),
                    "x" | "y" | "z" | "w" => {
                        let i = lane(name_str.chars().next()?)?;
                        (k != V2F64 || i < 2).then_some(())?;
                        (OpSimd::Hint(i), scalar)
                    }
                    "flagX" | "flagY" | "flagZ" | "flagW" if k == V4I32 => {
                        (OpSimd::Flag(lane(name_str.chars().nth(4)?.to_ascii_lowercase())?), Type::I1)
                    }
                    _ => return None,
                };
                Some(Recipe { op, receiver: Some((*target, k)), args: Vec::new(), result })
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() || arguments.args.iter().any(|a| a.name.is_some()) {
                    return None;
                }
                let args: Vec<ExprId> = arguments.args.iter().map(|a| a.value).collect();
                if let Some(Resolved::Constructor(fid)) = self.ctx.get_resolved(self.unit_id, e) {
                    return self.constructor_recipe(fid.0 as usize, args);
                }
                let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
                    return None;
                };
                let k = self.expr_simd(*recv)?;
                let name_str = self.ctx.symbol_name(name.sym);
                let receiver = Some((*recv, k));
                let one = |op: OpSimd, a: Arg, result: Type| {
                    (args.len() == 1).then(|| Recipe { op, receiver, args: vec![(args[0], a)], result })
                };
                let none = |op: OpSimd| args.is_empty().then(|| Recipe { op, receiver, args: Vec::new(), result: k });
                match (name_str, k) {
                    ("scale", V4F32 | V2F64) => one(OpSimd::Scale, Arg::Scalar(Type::F64), k),
                    ("abs", V4F32 | V2F64) => none(OpSimd::Abs),
                    ("sqrt", V4F32 | V2F64) => none(OpSimd::Sqrt),
                    ("reciprocal", V4F32) => none(OpSimd::Recip),
                    ("reciprocalSqrt", V4F32) => none(OpSimd::RecipSqrt),
                    ("min", V4F32 | V2F64) => one(OpSimd::Min, Arg::Vector(k), k),
                    ("max", V4F32 | V2F64) => one(OpSimd::Max, Arg::Vector(k), k),
                    ("clamp", V4F32 | V2F64) if args.len() == 2 => Some(Recipe {
                        op: OpSimd::Clamp,
                        receiver,
                        args: vec![(args[0], Arg::Vector(k)), (args[1], Arg::Vector(k))],
                        result: k,
                    }),
                    ("equal", V4F32) => one(OpSimd::Cmp(FCmpOp::Eq), Arg::Vector(k), V4I32),
                    ("notEqual", V4F32) => one(OpSimd::Cmp(FCmpOp::Ne), Arg::Vector(k), V4I32),
                    ("lessThan", V4F32) => one(OpSimd::Cmp(FCmpOp::Lt), Arg::Vector(k), V4I32),
                    ("lessThanOrEqual", V4F32) => one(OpSimd::Cmp(FCmpOp::Le), Arg::Vector(k), V4I32),
                    ("greaterThan", V4F32) => one(OpSimd::Cmp(FCmpOp::Gt), Arg::Vector(k), V4I32),
                    ("greaterThanOrEqual", V4F32) => one(OpSimd::Cmp(FCmpOp::Ge), Arg::Vector(k), V4I32),
                    ("shuffle", V4F32 | V4I32) if args.len() == 1 => {
                        let m = self.constant_mask(ast, args[0])?;
                        Some(Recipe { op: OpSimd::Shuffle(m), receiver, args: Vec::new(), result: k })
                    }
                    ("shuffleMix", V4F32 | V4I32) if args.len() == 2 => {
                        let m = self.constant_mask(ast, args[1])?;
                        Some(Recipe { op: OpSimd::ShuffleMix(m), receiver, args: vec![(args[0], Arg::Vector(k))], result: k })
                    }
                    ("withX" | "withY" | "withZ" | "withW", _) => {
                        let i = lane(name_str.chars().nth(4)?.to_ascii_lowercase())?;
                        (k != V2F64 || i < 2).then_some(())?;
                        let scalar = if k == V4I32 { Type::I64 } else { Type::F64 };
                        one(OpSimd::WithHint(i), Arg::Scalar(scalar), k)
                    }
                    ("withFlagX" | "withFlagY" | "withFlagZ" | "withFlagW", V4I32) => {
                        let i = lane(name_str.chars().nth(8)?.to_ascii_lowercase())?;
                        one(OpSimd::WithFlag(i), Arg::Scalar(Type::I1), k)
                    }
                    ("select", V4I32) if args.len() == 2 => Some(Recipe {
                        op: OpSimd::Select,
                        receiver,
                        args: vec![(args[0], Arg::Vector(V4F32)), (args[1], Arg::Vector(V4F32))],
                        result: V4F32,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `e` calls a member of an extension on `Int32x4` marked with
    /// `@pragma('dartforge:simd-api', '3.14')` — the `Int32x4` API of Dart
    /// 3.14 written in Dart 3.6 (`docs/SIMD-NATIVO.md` §6). The body of the
    /// members is that of the VM's `typed_data_patch.dart` (it runs on VM 3.6.2, which is
    /// the oracle); here the call becomes the vector instruction.
    fn api_314(&self, e: ExprId) -> bool {
        let Some(Resolved::ExtensionMember { extension, .. }) = self.ctx.get_resolved(self.unit_id, e) else {
            return false;
        };
        let decl = self.ctx.program.extension(*extension).decl;
        let unit = self.ctx.program.unit(decl.unit);
        let Some(d) = unit.ast.decls.get(decl.decl.0 as usize) else { return false };
        d.metadata.iter().any(|an| {
            if an.name.len() != 1 || self.ctx.symbol_name(an.name[0].sym) != "pragma" {
                return false;
            }
            let Some(args) = &an.arguments else { return false };
            let strings: Vec<Option<String>> = args
                .args
                .iter()
                .map(|a| match &unit.ast.exprs[a.value.0 as usize].kind {
                    ExprKind::String(lit) => lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned()),
                    _ => None,
                })
                .collect();
            matches!(strings.as_slice(), [Some(a), Some(b)] if a == "dartforge:simd-api" && b == "3.14")
        })
    }

    /// `e` has non-nullable static type `int`.
    fn is_int(&self, e: ExprId) -> bool {
        matches!(
            self.ctx.get_type(self.unit_id, e).map(|t| self.ctx.table.get(t)),
            Some(T::Interface { class, nullable: false, .. }) if Some(*class) == self.ctx.core.int_class
        )
    }

    /// The unboxed form of a member of the Dart 3.14 `Int32x4` API
    /// ([`Self::api_314`]): operators `~`, unary `-`, `*`, `<<`, `>>`;
    /// `andNot`, `abs`, the signed comparisons, `min`/`max`; `anyTrue` and
    /// `allTrue`.
    fn recipe_api_314(&self, ast: &ast::Ast, e: ExprId) -> Option<Recipe> {
        use Type::V4I32;
        let int4 = |x: ExprId| self.expr_simd(x) == Some(V4I32);
        match &ast.expr(e).kind {
            ExprKind::Binary { op, left, right } if self.api_314(e) && int4(*left) => {
                let receiver = Some((*left, V4I32));
                match op {
                    BinaryOp::Mul if int4(*right) => {
                        Some(Recipe { op: OpSimd::Mul, receiver, args: vec![(*right, Arg::Vector(V4I32))], result: V4I32 })
                    }
                    BinaryOp::Shl | BinaryOp::Shr if self.is_int(*right) => Some(Recipe {
                        op: OpSimd::Shifts(*op == BinaryOp::Shr),
                        receiver,
                        args: vec![(*right, Arg::Scalar(Type::I64))],
                        result: V4I32,
                    }),
                    _ => None,
                }
            }
            ExprKind::Unary { op: op @ (UnaryOp::Neg | UnaryOp::BitNot), operand } if self.api_314(e) && int4(*operand) => {
                let op = if *op == UnaryOp::Neg { OpSimd::Neg } else { OpSimd::Not };
                Some(Recipe { op, receiver: Some((*operand, V4I32)), args: Vec::new(), result: V4I32 })
            }
            ExprKind::Property { target, name, null_aware: false } if self.api_314(e) && int4(*target) => {
                let op = match self.ctx.symbol_name(name.sym) {
                    "anyTrue" => OpSimd::Any,
                    "allTrue" => OpSimd::All,
                    _ => return None,
                };
                Some(Recipe { op, receiver: Some((*target, V4I32)), args: Vec::new(), result: Type::I1 })
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() || arguments.args.iter().any(|a| a.name.is_some()) {
                    return None;
                }
                let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
                    return None;
                };
                if !(self.api_314(e) || self.api_314(*target)) || !int4(*recv) {
                    return None;
                }
                let args: Vec<ExprId> = arguments.args.iter().map(|a| a.value).collect();
                let receiver = Some((*recv, V4I32));
                let op = match (self.ctx.symbol_name(name.sym), args.as_slice()) {
                    ("abs", []) => {
                        return Some(Recipe { op: OpSimd::Abs, receiver, args: Vec::new(), result: V4I32 });
                    }
                    (_, [a]) if !int4(*a) => return None,
                    ("andNot", [_]) => OpSimd::AndNot,
                    ("equal", [_]) => OpSimd::CmpInt(ICmpOp::Eq),
                    ("notEqual", [_]) => OpSimd::CmpInt(ICmpOp::Ne),
                    ("lessThan", [_]) => OpSimd::CmpInt(ICmpOp::Slt),
                    ("lessThanOrEqual", [_]) => OpSimd::CmpInt(ICmpOp::Sle),
                    ("greaterThan", [_]) => OpSimd::CmpInt(ICmpOp::Sgt),
                    ("greaterThanOrEqual", [_]) => OpSimd::CmpInt(ICmpOp::Sge),
                    ("min", [_]) => OpSimd::Min,
                    ("max", [_]) => OpSimd::Max,
                    _ => return None,
                };
                Some(Recipe { op, receiver, args: vec![(args[0], Arg::Vector(V4I32))], result: V4I32 })
            }
            _ => None,
        }
    }

    /// The unboxed form of a constructor of the SIMD classes.
    fn constructor_recipe(&self, fid: usize, args: Vec<ExprId>) -> Option<Recipe> {
        use Type::{V2F64, V4F32, V4I32};
        let f = &self.ctx.program.functions[fid];
        let c = &self.ctx.program.classes[f.class?.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:typed_data" || self.async_state.is_some() {
            return None;
        }
        let k = match self.ctx.symbol_name(c.name) {
            "Float32x4" => V4F32,
            "Int32x4" => V4I32,
            "Float64x2" => V2F64,
            _ => return None,
        };
        let name_str = self.ctx.symbol_name(f.name);
        let lanes = if k == V2F64 { 2 } else { 4 };
        let recipe = |op: OpSimd, a: Vec<Arg>| {
            (a.len() == args.len()).then(|| Recipe {
                op,
                receiver: None,
                args: args.iter().copied().zip(a).collect(),
                result: k,
            })
        };
        match (name_str, k) {
            ("", V4I32) => recipe(OpSimd::Builds, vec![Arg::Scalar(Type::I64); 4]),
            ("", _) => recipe(OpSimd::Builds, vec![Arg::Scalar(Type::F64); lanes]),
            ("bool", V4I32) => recipe(OpSimd::BuildsFlags, vec![Arg::Scalar(Type::I1); 4]),
            ("splat", V4F32 | V2F64) => recipe(OpSimd::Splat, vec![Arg::Scalar(Type::F64)]),
            ("zero", V4F32 | V2F64) => recipe(OpSimd::Zero, Vec::new()),
            ("fromInt32x4Bits", V4F32) => recipe(OpSimd::Bits, vec![Arg::Vector(V4I32)]),
            ("fromFloat32x4Bits", V4I32) => recipe(OpSimd::Bits, vec![Arg::Vector(V4F32)]),
            ("fromFloat64x2", V4F32) => recipe(OpSimd::Converts, vec![Arg::Vector(V2F64)]),
            ("fromFloat32x4", V2F64) => recipe(OpSimd::Converts, vec![Arg::Vector(V4F32)]),
            _ => None,
        }
    }

    /// Emits the recipe: the receiver and the arguments (unboxed vectors through
    /// `lower_simd` itself, scalars in the representation of the operation), in
    /// Dart evaluation order.
    fn emit_recipe(&mut self, ast: &ast::Ast, r: Recipe) -> Operand {
        let mut ops = Vec::with_capacity(r.args.len() + 1);
        if let Some((recv, k)) = r.receiver {
            ops.push(self.lower_simd(ast, recv, k));
        }
        for (e, a) in r.args {
            let op = match a {
                Arg::Vector(k) => self.lower_simd(ast, e, k),
                Arg::Scalar(t) => {
                    let v = self.lower_expr(ast, e);
                    self.coerce(v, t)
                }
            };
            ops.push(op);
        }
        self.emit(Instruction::Simd { op: r.op, args: ops }, r.result)
    }

    /// A recognized SIMD expression, in the general `lower_expr`: the direct scalar
    /// result, or the vector (which `lower_expr` puts in the box of the static
    /// type). `None`: the expression follows the usual path.
    pub(super) fn simd_expression(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        // The member of an extension type over a SIMD type is its own.
        let member = match &ast.expr(e).kind {
            ExprKind::Call { target, .. } => *target,
            _ => e,
        };
        if self.member_te(member).is_some() {
            return None;
        }
        let r = self.simd_recipe(ast, e)?;
        Some(self.emit_recipe(ast, r))
    }

    /// Expression `e`, of SIMD static type `k`, as an unboxed vector:
    /// through the recognized form, through the local that keeps the vector, through access to
    /// a SIMD list, or by unboxing the value from the general path.
    pub(super) fn lower_simd(&mut self, ast: &ast::Ast, e: ExprId, k: Type) -> Operand {
        match &ast.expr(e).kind {
            ExprKind::Parenthesized(x) => return self.lower_simd(ast, *x, k),
            ExprKind::Identifier(_) => {
                // The unboxed local comes out directly (`lower_expr` would box it).
                let v = self.lower_expr_raw(ast, e);
                return self.coerce(v, k);
            }
            ExprKind::Index { target, index, null_aware: false } => {
                if let Some(ix) = self.indexable(self.ctx.get_type(self.unit_id, *target))
                    && let super::typed_items::Indexable::Simd { .. } = ix
                {
                    let list = self.lower_expr(ast, *target);
                    let i = self.lower_expr(ast, *index);
                    // Coerced once: the slow path reuses the same value.
                    let i = if self.operand_type(&i) == Type::Ref { self.coerce(i, Type::I64) } else { i };
                    self.access_proven = self.proven_of(ast, *target, Some(*index));
                    let read_done = self.read_indexed(list, i, ix, k);
                    self.access_proven = None;
                    if let Some(v) = read_done {
                        return v;
                    }
                }
            }
            _ => {}
        }
        if let Some(v) = self.lane_conversion(ast, e, k) {
            return v;
        }
        if let Some(r) = self.simd_recipe(ast, e).filter(|r| r.result == k) {
            return self.emit_recipe(ast, r);
        }
        let v = self.lower_expr(ast, e);
        self.coerce(v, k)
    }

    /// The lane-by-lane round trip between `Int32x4` and `Float32x4` that the Dart 3.6
    /// API forces (there is no numeric conversion between the two), as a
    /// vector conversion:
    ///
    /// * `Float32x4(v.x.toDouble(), v.y.toDouble(), v.z.toDouble(),
    ///   v.w.toDouble())`, with `v` an `Int32x4` local: `sitofp` — the same
    ///   value per lane (the 32-bit `int` is exact in `double`, and the
    ///   `double` becomes a `float` with a single rounding, like `sitofp`);
    /// * `Int32x4(r.x.toInt(), …)` and `Int32x4(r.x.toInt() >> c, …)` (the same
    ///   literal `c` in `0..=31`), with `r` a `Float32x4` local: if all
    ///   lanes are in `[-2^31, 2^31)`, `fptosi` (`toInt()` truncates, and then
    ///   neither throws nor saturates) and `ashr` by `c` (the `>> c` of the 64-bit `int`
    ///   of a 32-bit value, truncated to 32 bits by the constructor, is the
    ///   32-bit `ashr`); otherwise (NaN, infinity, out of range), the usual
    ///   lane-by-lane path, which throws the `UnsupportedError` of `toInt()`
    ///   or saturates and truncates like the VM.
    ///
    /// The locals are read once instead of four times (reading a local has no
    /// effect).
    fn lane_conversion(&mut self, ast: &ast::Ast, e: ExprId, k: Type) -> Option<Operand> {
        use Type::{V4F32, V4I32};
        if k != V4F32 && k != V4I32 {
            return None;
        }
        let ExprKind::Call { arguments, .. } = &ast.expr(e).kind else { return None };
        if !arguments.type_args.is_empty() || arguments.args.len() != 4 || arguments.args.iter().any(|a| a.name.is_some()) {
            return None;
        }
        let Some(Resolved::Constructor(fid)) = self.ctx.get_resolved(self.unit_id, e) else { return None };
        let f = &self.ctx.program.functions[fid.0 as usize];
        let c = &self.ctx.program.classes[f.class?.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:typed_data" || !self.ctx.symbol_name(f.name).is_empty() {
            return None;
        }
        let expected = match self.ctx.symbol_name(c.name) {
            "Float32x4" => V4F32,
            "Int32x4" => V4I32,
            _ => return None,
        };
        if expected != k {
            return None;
        }
        let without_parentheses = |mut x: ExprId| {
            while let ExprKind::Parenthesized(y) = &ast.expr(x).kind {
                x = *y;
            }
            x
        };
        // `origin.<lane i>.<method>()`: the symbol of local `origin`.
        let converted_lane = |x: ExprId, i: u8, method: &str, origin: Type| -> Option<dartforge_intern::SymbolId> {
            let ExprKind::Call { target, arguments } = &ast.expr(without_parentheses(x)).kind else { return None };
            if !arguments.args.is_empty() || !arguments.type_args.is_empty() {
                return None;
            }
            let ExprKind::Property { target: p, name, null_aware: false } = &ast.expr(*target).kind else { return None };
            if self.ctx.symbol_name(name.sym) != method {
                return None;
            }
            let ExprKind::Property { target: v, name: l, null_aware: false } = &ast.expr(*p).kind else { return None };
            if lane(self.ctx.symbol_name(l.sym).chars().next()?)? != i || self.ctx.symbol_name(l.sym).len() != 1 {
                return None;
            }
            let ExprKind::Identifier(n) = &ast.expr(*v).kind else { return None };
            (self.expr_simd(*v) == Some(origin) && self.lookup_local(n.sym).is_some()).then_some(n.sym)
        };
        let args: Vec<ExprId> = arguments.args.iter().map(|a| a.value).collect();
        if k == V4F32 {
            let mut origin = None;
            for (i, &a) in args.iter().enumerate() {
                let s = converted_lane(a, i as u8, "toDouble", V4I32)?;
                if origin.is_some_and(|o| o != s) {
                    return None;
                }
                origin = Some(s);
            }
            let v = without_parentheses(args[0]);
            let ExprKind::Call { target, .. } = &ast.expr(v).kind else { return None };
            let ExprKind::Property { target: p, .. } = &ast.expr(*target).kind else { return None };
            let ExprKind::Property { target: local, .. } = &ast.expr(*p).kind else { return None };
            let x = self.lower_simd(ast, *local, V4I32);
            return Some(self.emit(Instruction::Simd { op: OpSimd::IntToFloat, args: vec![x] }, V4F32));
        }
        // `Int32x4(r.x.toInt() [>> c], …)`.
        let source = self.source();
        let literal = |x: ExprId| match &ast.expr(without_parentheses(x)).kind {
            ExprKind::Int(span) => source.get(span.start as usize..span.end as usize)?.replace('_', "").parse::<i64>().ok(),
            _ => None,
        };
        let mut origin = None;
        let mut offset: Option<Option<i64>> = None;
        let mut local = None;
        for (i, &a) in args.iter().enumerate() {
            let (conv, c) = match &ast.expr(without_parentheses(a)).kind {
                ExprKind::Binary { op: BinaryOp::Shr, left, right } => (*left, Some(literal(*right).filter(|c| (0..=31).contains(c))?)),
                _ => (a, None),
            };
            let s = converted_lane(conv, i as u8, "toInt", V4F32)?;
            if origin.is_some_and(|o| o != s) || offset.is_some_and(|d| d != c) {
                return None;
            }
            origin = Some(s);
            offset = Some(c);
            if local.is_none() {
                let ExprKind::Call { target, .. } = &ast.expr(without_parentheses(conv)).kind else { return None };
                let ExprKind::Property { target: p, .. } = &ast.expr(*target).kind else { return None };
                let ExprKind::Property { target: l, .. } = &ast.expr(*p).kind else { return None };
                local = Some(*l);
            }
        }
        let r = self.simd_recipe(ast, e)?;
        let f = self.lower_simd(ast, local?, V4F32);
        let ok = self.emit(Instruction::Simd { op: OpSimd::InInt32Range, args: vec![f.clone()] }, Type::I1);
        let fast = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fast, else_block: slow });
        self.set_block(fast);
        let mut t = self.emit(Instruction::Simd { op: OpSimd::FloatToInt, args: vec![f] }, V4I32);
        if let Some(Some(c)) = offset {
            t = self.emit(Instruction::Simd { op: OpSimd::Shifts(true), args: vec![t, Operand::Constant(Constant::Int(c))] }, V4I32);
        }
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(slow);
        let s = self.emit_recipe(ast, r);
        let slow_end = self.current_block;
        let slow_arrives = !self.is_terminated();
        if slow_arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(if slow_arrives {
            self.emit(Instruction::Phi { incoming: vec![(fast_end, t), (slow_end, s)], ty: V4I32 }, V4I32)
        } else {
            t
        })
    }
}
