//! Emission of unboxed SIMD operations (`Instruction::Simd`,
//! `docs/SIMD-NATIVO.md`): LLVM vectors (`<4 x float>`, `<4 x i32>`,
//! `<2 x double>`) with the VM's lane-by-lane semantics — the same as
//! `runtime/src/simd.rs`, which still serves the boxed values. No
//! `fast-math`: `min`/`max` are `fcmp` + `select` (with NaN, the second
//! operand, the VM's `Utils::Minimum`/`Maximum`), the square root is `llvm.sqrt` (the
//! correct IEEE one), and a `double` becomes a `float` by `fptrunc` (the `as f32`).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// The declarations of the intrinsics the emission uses.
pub const DECLARATIONS: &str = "declare <4 x float> @llvm.fabs.v4f32(<4 x float>)\n\
    declare <2 x double> @llvm.fabs.v2f64(<2 x double>)\n\
    declare <4 x float> @llvm.sqrt.v4f32(<4 x float>)\n\
    declare <2 x double> @llvm.sqrt.v2f64(<2 x double>)\n\
    declare <4 x i32> @llvm.abs.v4i32(<4 x i32>, i1)\n";

/// Lanes, lane type and intrinsic suffix of a vector.
fn shape(k: Type) -> (u32, &'static str, &'static str) {
    match k {
        Type::V4F32 => (4, "float", "v4f32"),
        Type::V4I32 => (4, "i32", "v4i32"),
        Type::V2F64 => (2, "double", "v2f64"),
        _ => unreachable!("expected SIMD type: {k:?}"),
    }
}

impl LlvmEmitter<'_> {
    /// `%v{v}` = the operation `op` over `args`, with result `ty`.
    pub(super) fn emit_simd(&mut self, v: u32, op: OpSimd, args: &[Operand], ty: Type) {
        // The vector type: that of the first vector operand, or the result.
        let k = args.iter().map(|a| self.type_of(a)).find(|t| t.is_vector()).unwrap_or(ty);
        let (n, el, suf) = if k.is_vector() { shape(k) } else { shape(ty) };
        let kt = k.llvm_ir();
        let floating_point = k != Type::V4I32;
        let a = |s: &mut Self, i: usize| {
            let t = s.type_of(&args[i]);
            s.coerce(&args[i], t)
        };
        let mut w = String::new();
        let r = format!("%v{v}");
        match op {
            OpSimd::Add | OpSimd::Sub | OpSimd::Mul | OpSimd::Div | OpSimd::And | OpSimd::Or | OpSimd::Xor => {
                let (x, y) = (a(self, 0), a(self, 1));
                let instr = match (op, floating_point) {
                    (OpSimd::Add, true) => "fadd",
                    (OpSimd::Sub, true) => "fsub",
                    (OpSimd::Mul, true) => "fmul",
                    (OpSimd::Div, true) => "fdiv",
                    (OpSimd::Add, false) => "add",
                    // `Int32x4 * Int32x4` (3.14): the low 32 bits of the product.
                    (OpSimd::Mul, false) => "mul",
                    (OpSimd::Sub, false) => "sub",
                    (OpSimd::And, false) => "and",
                    (OpSimd::Or, false) => "or",
                    (OpSimd::Xor, false) => "xor",
                    _ => unreachable!("SIMD operation {op:?} on {k:?}"),
                };
                writeln!(w, "  {r} = {instr} {kt} {x}, {y}").unwrap();
            }
            // `Int32x4.min`/`max` (3.14): signed comparison.
            OpSimd::Min | OpSimd::Max if !floating_point => {
                let (x, y) = (a(self, 0), a(self, 1));
                let p = if op == OpSimd::Min { "slt" } else { "sgt" };
                writeln!(w, "  %sc{v} = icmp {p} {kt} {x}, {y}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sc{v}, {kt} {x}, {kt} {y}").unwrap();
            }
            OpSimd::Min | OpSimd::Max => {
                let (x, y) = (a(self, 0), a(self, 1));
                let p = if op == OpSimd::Min { "olt" } else { "ogt" };
                writeln!(w, "  %sc{v} = fcmp {p} {kt} {x}, {y}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sc{v}, {kt} {x}, {kt} {y}").unwrap();
            }
            OpSimd::Clamp if cfg!(target_arch = "aarch64") => {
                // The VM's `vminf`/`vmaxf` on arm64 (`simd128.cc`, and the
                // runtime's `clamp_pista`): outside equality, with NaN the
                // first operand stays; on equality (only ±0 matters) `min`
                // is the OR of the bits (`-0`) and `max` is the AND (`+0`).
                let (x, lo, hi) = (a(self, 0), a(self, 1), a(self, 2));
                let it = format!("<{n} x i{}>", if k == Type::V2F64 { 64 } else { 32 });
                writeln!(w, "  %sa{v} = fcmp ogt {kt} {x}, {hi}").unwrap();
                writeln!(w, "  %sd{v} = select <{n} x i1> %sa{v}, {kt} {hi}, {kt} {x}").unwrap();
                writeln!(w, "  %sx{v} = bitcast {kt} {x} to {it}").unwrap();
                writeln!(w, "  %sh{v} = bitcast {kt} {hi} to {it}").unwrap();
                writeln!(w, "  %so{v} = or {it} %sx{v}, %sh{v}").unwrap();
                writeln!(w, "  %sof{v} = bitcast {it} %so{v} to {kt}").unwrap();
                writeln!(w, "  %se{v} = fcmp oeq {kt} {x}, {hi}").unwrap();
                writeln!(w, "  %sm{v} = select <{n} x i1> %se{v}, {kt} %sof{v}, {kt} %sd{v}").unwrap();
                writeln!(w, "  %sb{v} = fcmp olt {kt} %sm{v}, {lo}").unwrap();
                writeln!(w, "  %sg{v} = select <{n} x i1> %sb{v}, {kt} {lo}, {kt} %sm{v}").unwrap();
                writeln!(w, "  %smi{v} = bitcast {kt} %sm{v} to {it}").unwrap();
                writeln!(w, "  %sl{v} = bitcast {kt} {lo} to {it}").unwrap();
                writeln!(w, "  %sy{v} = and {it} %smi{v}, %sl{v}").unwrap();
                writeln!(w, "  %syf{v} = bitcast {it} %sy{v} to {kt}").unwrap();
                writeln!(w, "  %sf{v} = fcmp oeq {kt} %sm{v}, {lo}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sf{v}, {kt} %syf{v}, {kt} %sg{v}").unwrap();
            }
            OpSimd::Clamp => {
                let (x, lo, hi) = (a(self, 0), a(self, 1), a(self, 2));
                writeln!(w, "  %sa{v} = fcmp olt {kt} {x}, {hi}").unwrap();
                writeln!(w, "  %sm{v} = select <{n} x i1> %sa{v}, {kt} {x}, {kt} {hi}").unwrap();
                writeln!(w, "  %sb{v} = fcmp ogt {kt} %sm{v}, {lo}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sb{v}, {kt} %sm{v}, {kt} {lo}").unwrap();
            }
            // `-Int32x4` and `Int32x4.abs()` (3.14): wrapping, and the `abs` of
            // -2^31 is -2^31 (`llvm.abs` with `is_int_min_poison` false).
            OpSimd::Neg if !floating_point => {
                let x = a(self, 0);
                writeln!(w, "  {r} = sub {kt} zeroinitializer, {x}").unwrap();
            }
            OpSimd::Abs if !floating_point => {
                let x = a(self, 0);
                writeln!(w, "  {r} = call {kt} @llvm.abs.v4i32({kt} {x}, i1 false)").unwrap();
            }
            OpSimd::CmpInt(c) => {
                let (x, y) = (a(self, 0), a(self, 1));
                let p = match c {
                    ICmpOp::Eq => "eq",
                    ICmpOp::Ne => "ne",
                    ICmpOp::Slt => "slt",
                    ICmpOp::Sle => "sle",
                    ICmpOp::Sgt => "sgt",
                    ICmpOp::Sge => "sge",
                    ICmpOp::Ult => "ult",
                };
                writeln!(w, "  %sc{v} = icmp {p} {kt} {x}, {y}").unwrap();
                writeln!(w, "  {r} = sext <{n} x i1> %sc{v} to <{n} x i32>").unwrap();
            }
            OpSimd::AndNot | OpSimd::Not => {
                let x = a(self, 0);
                let some = self.constant_splat(n, el, "-1");
                if op == OpSimd::Not {
                    writeln!(w, "  {r} = xor {kt} {x}, {some}").unwrap();
                } else {
                    let y = a(self, 1);
                    writeln!(w, "  %sn{v} = xor {kt} {y}, {some}").unwrap();
                    writeln!(w, "  {r} = and {kt} {x}, %sn{v}").unwrap();
                }
            }
            OpSimd::Shifts(right) => {
                // The amount is `s & 31` (also for a negative one), like the VM.
                let x = a(self, 0);
                let s = self.coerce(&args[1], Type::I64);
                writeln!(w, "  %sm{v} = and i64 {s}, 31").unwrap();
                writeln!(w, "  %st{v} = trunc i64 %sm{v} to i32").unwrap();
                self.spread(&mut w, v, "d", &format!("%st{v}"), k);
                let instr = if right { "ashr" } else { "shl" };
                writeln!(w, "  {r} = {instr} {kt} {x}, %ssd{v}").unwrap();
            }
            OpSimd::Any | OpSimd::All => {
                let x = a(self, 0);
                writeln!(w, "  %sz{v} = icmp ne {kt} {x}, zeroinitializer").unwrap();
                writeln!(w, "  %sb{v} = bitcast <{n} x i1> %sz{v} to i{n}").unwrap();
                let (p, target) = if op == OpSimd::Any { ("ne", "0") } else { ("eq", "-1") };
                writeln!(w, "  {r} = icmp {p} i{n} %sb{v}, {target}").unwrap();
            }
            OpSimd::Neg => {
                let x = a(self, 0);
                writeln!(w, "  {r} = fneg {kt} {x}").unwrap();
            }
            OpSimd::Abs | OpSimd::Sqrt => {
                let x = a(self, 0);
                let f = if op == OpSimd::Abs { "fabs" } else { "sqrt" };
                writeln!(w, "  {r} = call {kt} @llvm.{f}.{suf}({kt} {x})").unwrap();
            }
            OpSimd::Recip | OpSimd::RecipSqrt => {
                let x = a(self, 0);
                let one = self.constant_splat(n, el, "1.0");
                let dest = if op == OpSimd::Recip { r.clone() } else { format!("%sr{v}") };
                writeln!(w, "  {dest} = fdiv {kt} {one}, {x}").unwrap();
                if op == OpSimd::RecipSqrt {
                    writeln!(w, "  {r} = call {kt} @llvm.sqrt.{suf}({kt} %sr{v})").unwrap();
                }
            }
            OpSimd::Scale => {
                let x = a(self, 0);
                let s = self.coerce(&args[1], Type::F64);
                let esc = self.lane_scalar(&mut w, v, "e", &s, k);
                self.spread(&mut w, v, "e", &esc, k);
                writeln!(w, "  {r} = fmul {kt} {x}, %sse{v}").unwrap();
            }
            OpSimd::Cmp(c) => {
                let (x, y) = (a(self, 0), a(self, 1));
                let p = match c {
                    FCmpOp::Eq => "oeq",
                    FCmpOp::Ne => "une",
                    FCmpOp::Lt => "olt",
                    FCmpOp::Le => "ole",
                    FCmpOp::Gt => "ogt",
                    FCmpOp::Ge => "oge",
                };
                writeln!(w, "  %sc{v} = fcmp {p} {kt} {x}, {y}").unwrap();
                writeln!(w, "  {r} = sext <{n} x i1> %sc{v} to <{n} x i32>").unwrap();
            }
            OpSimd::Hint(i) => {
                let x = a(self, 0);
                match k {
                    Type::V4F32 => {
                        writeln!(w, "  %sp{v} = extractelement {kt} {x}, i32 {i}").unwrap();
                        writeln!(w, "  {r} = fpext float %sp{v} to double").unwrap();
                    }
                    Type::V4I32 => {
                        writeln!(w, "  %sp{v} = extractelement {kt} {x}, i32 {i}").unwrap();
                        writeln!(w, "  {r} = sext i32 %sp{v} to i64").unwrap();
                    }
                    _ => writeln!(w, "  {r} = extractelement {kt} {x}, i32 {i}").unwrap(),
                }
            }
            OpSimd::WithHint(i) => {
                let x = a(self, 0);
                let esc = match k {
                    Type::V4I32 => {
                        let s = self.coerce(&args[1], Type::I64);
                        writeln!(w, "  %sq{v} = trunc i64 {s} to i32").unwrap();
                        format!("%sq{v}")
                    }
                    _ => {
                        let s = self.coerce(&args[1], Type::F64);
                        self.lane_scalar(&mut w, v, "q", &s, k)
                    }
                };
                writeln!(w, "  {r} = insertelement {kt} {x}, {el} {esc}, i32 {i}").unwrap();
            }
            OpSimd::Flag(i) => {
                let x = a(self, 0);
                writeln!(w, "  %sp{v} = extractelement {kt} {x}, i32 {i}").unwrap();
                writeln!(w, "  {r} = icmp ne i32 %sp{v}, 0").unwrap();
            }
            OpSimd::WithFlag(i) => {
                let x = a(self, 0);
                let b = self.coerce(&args[1], Type::I1);
                writeln!(w, "  %sq{v} = select i1 {b}, i32 -1, i32 0").unwrap();
                writeln!(w, "  {r} = insertelement {kt} {x}, i32 %sq{v}, i32 {i}").unwrap();
            }
            OpSimd::Builds | OpSimd::BuildsFlags => {
                let mut current = "poison".to_string();
                for (i, arg) in args.iter().enumerate() {
                    let esc = if op == OpSimd::BuildsFlags {
                        let b = self.coerce(arg, Type::I1);
                        writeln!(w, "  %sq{v}_{i} = select i1 {b}, i32 -1, i32 0").unwrap();
                        format!("%sq{v}_{i}")
                    } else if k == Type::V4I32 {
                        let s = self.coerce(arg, Type::I64);
                        writeln!(w, "  %sq{v}_{i} = trunc i64 {s} to i32").unwrap();
                        format!("%sq{v}_{i}")
                    } else {
                        let s = self.coerce(arg, Type::F64);
                        self.lane_scalar(&mut w, v, &format!("q{i}_"), &s, k)
                    };
                    let next_item = if i + 1 == args.len() { r.clone() } else { format!("%sv{v}_{i}") };
                    writeln!(w, "  {next_item} = insertelement {kt} {current}, {el} {esc}, i32 {i}").unwrap();
                    current = next_item;
                }
            }
            OpSimd::Splat => {
                let s = self.coerce(&args[0], Type::F64);
                let esc = self.lane_scalar(&mut w, v, "e", &s, k);
                writeln!(w, "  %sie{v} = insertelement {kt} poison, {el} {esc}, i32 0").unwrap();
                writeln!(w, "  {r} = shufflevector {kt} %sie{v}, {kt} poison, <{n} x i32> zeroinitializer").unwrap();
            }
            OpSimd::Zero => {
                writeln!(w, "  {r} = select i1 true, {kt} zeroinitializer, {kt} zeroinitializer").unwrap();
            }
            OpSimd::SignMask => {
                let x = a(self, 0);
                let bits = if k == Type::V2F64 { format!("<{n} x i64>") } else { format!("<{n} x i32>") };
                writeln!(w, "  %sb{v} = bitcast {kt} {x} to {bits}").unwrap();
                writeln!(w, "  %sc{v} = icmp slt {bits} %sb{v}, zeroinitializer").unwrap();
                writeln!(w, "  %sm{v} = bitcast <{n} x i1> %sc{v} to i{n}").unwrap();
                writeln!(w, "  {r} = zext i{n} %sm{v} to i64").unwrap();
            }
            OpSimd::Shuffle(m) | OpSimd::ShuffleMix(m) => {
                let x = a(self, 0);
                let y = if op == OpSimd::ShuffleMix(m) { a(self, 1) } else { x.clone() };
                let m = u32::from(m);
                let high = if matches!(op, OpSimd::ShuffleMix(_)) { 4 } else { 0 };
                let ind = [m & 3, (m >> 2) & 3, high + ((m >> 4) & 3), high + ((m >> 6) & 3)];
                writeln!(
                    w,
                    "  {r} = shufflevector {kt} {x}, {kt} {y}, <4 x i32> <i32 {}, i32 {}, i32 {}, i32 {}>",
                    ind[0], ind[1], ind[2], ind[3]
                )
                .unwrap();
            }
            OpSimd::Select => {
                let (m, t, f) = (a(self, 0), a(self, 1), a(self, 2));
                writeln!(w, "  %st{v} = bitcast <4 x float> {t} to <4 x i32>").unwrap();
                writeln!(w, "  %sf{v} = bitcast <4 x float> {f} to <4 x i32>").unwrap();
                writeln!(w, "  %sa{v} = and <4 x i32> {m}, %st{v}").unwrap();
                writeln!(w, "  %sn{v} = xor <4 x i32> {m}, <i32 -1, i32 -1, i32 -1, i32 -1>").unwrap();
                writeln!(w, "  %sb{v} = and <4 x i32> %sn{v}, %sf{v}").unwrap();
                writeln!(w, "  %so{v} = or <4 x i32> %sa{v}, %sb{v}").unwrap();
                writeln!(w, "  {r} = bitcast <4 x i32> %so{v} to <4 x float>").unwrap();
            }
            OpSimd::Bits => {
                let x = a(self, 0);
                writeln!(w, "  {r} = bitcast {kt} {x} to {}", ty.llvm_ir()).unwrap();
            }
            OpSimd::Converts => {
                let x = a(self, 0);
                if k == Type::V2F64 {
                    // `Float32x4.fromFloat64x2`: [x, y, 0, 0].
                    for i in 0..2 {
                        writeln!(w, "  %sd{v}_{i} = extractelement <2 x double> {x}, i32 {i}").unwrap();
                        writeln!(w, "  %sf{v}_{i} = fptrunc double %sd{v}_{i} to float").unwrap();
                    }
                    writeln!(w, "  %sv{v}_0 = insertelement <4 x float> zeroinitializer, float %sf{v}_0, i32 0").unwrap();
                    writeln!(w, "  {r} = insertelement <4 x float> %sv{v}_0, float %sf{v}_1, i32 1").unwrap();
                } else {
                    // `Float64x2.fromFloat32x4`: lanes 0 and 1.
                    for i in 0..2 {
                        writeln!(w, "  %sf{v}_{i} = extractelement <4 x float> {x}, i32 {i}").unwrap();
                        writeln!(w, "  %sd{v}_{i} = fpext float %sf{v}_{i} to double").unwrap();
                    }
                    writeln!(w, "  %sv{v}_0 = insertelement <2 x double> poison, double %sd{v}_0, i32 0").unwrap();
                    writeln!(w, "  {r} = insertelement <2 x double> %sv{v}_0, double %sd{v}_1, i32 1").unwrap();
                }
            }
            OpSimd::IntToFloat => {
                let x = a(self, 0);
                writeln!(w, "  {r} = sitofp <4 x i32> {x} to <4 x float>").unwrap();
            }
            OpSimd::FloatToInt => {
                let x = a(self, 0);
                writeln!(w, "  {r} = fptosi <4 x float> {x} to <4 x i32>").unwrap();
            }
            OpSimd::InInt32Range => {
                // -2^31 and 2^31, exact in `float` (LLVM's hexadecimal
                // notation is the `double` one); ordered comparisons: NaN stays out.
                let x = a(self, 0);
                let lo = self.constant_splat(4, "float", "0xC1E0000000000000");
                let hi = self.constant_splat(4, "float", "0x41E0000000000000");
                writeln!(w, "  %sl{v} = fcmp oge <4 x float> {x}, {lo}").unwrap();
                writeln!(w, "  %sh{v} = fcmp olt <4 x float> {x}, {hi}").unwrap();
                writeln!(w, "  %sa{v} = and <4 x i1> %sl{v}, %sh{v}").unwrap();
                writeln!(w, "  %sm{v} = bitcast <4 x i1> %sa{v} to i4").unwrap();
                writeln!(w, "  {r} = icmp eq i4 %sm{v}, -1").unwrap();
            }
            OpSimd::Loads => {
                let e = self.coerce(&args[0], Type::I64);
                let i = self.coerce(&args[1], Type::I64);
                let kt = ty.llvm_ir();
                writeln!(w, "  %sp{v} = inttoptr i64 {e} to ptr").unwrap();
                writeln!(w, "  %sg{v} = getelementptr {kt}, ptr %sp{v}, i64 {i}").unwrap();
                writeln!(w, "  {r} = load {kt}, ptr %sg{v}, align 1").unwrap();
            }
            OpSimd::Writes => {
                let e = self.coerce(&args[0], Type::I64);
                let i = self.coerce(&args[1], Type::I64);
                let t = self.type_of(&args[2]);
                let x = self.coerce(&args[2], t);
                let kt = t.llvm_ir();
                writeln!(w, "  %sp{v} = inttoptr i64 {e} to ptr").unwrap();
                writeln!(w, "  %sg{v} = getelementptr {kt}, ptr %sp{v}, i64 {i}").unwrap();
                writeln!(w, "  store {kt} {x}, ptr %sg{v}, align 1").unwrap();
            }
        }
        self.out.push_str(&w);
    }

    /// A `double` already coerced as a lane of vector `k` (`fptrunc` in
    /// `Float32x4`); returns the name.
    fn lane_scalar(&self, w: &mut String, v: u32, suffix: &str, s: &str, k: Type) -> String {
        if k == Type::V4F32 {
            writeln!(w, "  %sx{suffix}{v} = fptrunc double {s} to float").unwrap();
            format!("%sx{suffix}{v}")
        } else {
            s.to_string()
        }
    }

    /// `%ss{suffix}{v}` = vector `k` with `esc` in every lane.
    fn spread(&self, w: &mut String, v: u32, suffix: &str, esc: &str, k: Type) {
        let (n, el, _) = shape(k);
        let kt = k.llvm_ir();
        writeln!(w, "  %si{suffix}{v} = insertelement {kt} poison, {el} {esc}, i32 0").unwrap();
        writeln!(w, "  %ss{suffix}{v} = shufflevector {kt} %si{suffix}{v}, {kt} poison, <{n} x i32> zeroinitializer").unwrap();
    }

    /// A vector constant with `value` in every lane.
    fn constant_splat(&self, n: u32, el: &str, value: &str) -> String {
        let lanes: Vec<String> = (0..n).map(|_| format!("{el} {value}")).collect();
        format!("<{}>", lanes.join(", "))
    }
}
