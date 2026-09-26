//! A emissão das operações SIMD sem caixa (`Instruction::Simd`,
//! `docs/SIMD-NATIVO.md`): vetores do LLVM (`<4 x float>`, `<4 x i32>`,
//! `<2 x double>`) com a semântica pista a pista da VM — a mesma de
//! `runtime/src/simd.rs`, que continua servindo os valores em caixa. Sem
//! `fast-math`: `min`/`max` são `fcmp` + `select` (com NaN, o segundo
//! operando, o `Utils::Minimum`/`Maximum` da VM), a raiz é `llvm.sqrt` (a
//! correta do IEEE), e o `double` vira `float` por `fptrunc` (o `as f32`).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// As declarações das intrínsecas que a emissão usa.
pub const DECLARACOES: &str = "declare <4 x float> @llvm.fabs.v4f32(<4 x float>)\n\
    declare <2 x double> @llvm.fabs.v2f64(<2 x double>)\n\
    declare <4 x float> @llvm.sqrt.v4f32(<4 x float>)\n\
    declare <2 x double> @llvm.sqrt.v2f64(<2 x double>)\n";

/// Pistas, tipo da pista e sufixo das intrínsecas de um vetor.
fn forma(k: Type) -> (u32, &'static str, &'static str) {
    match k {
        Type::V4F32 => (4, "float", "v4f32"),
        Type::V4I32 => (4, "i32", "v4i32"),
        Type::V2F64 => (2, "double", "v2f64"),
        _ => unreachable!("tipo SIMD esperado: {k:?}"),
    }
}

impl LlvmEmitter<'_> {
    /// `%v{v}` = a operação `op` sobre `args`, com resultado `ty`.
    pub(super) fn emitir_simd(&mut self, v: u32, op: OpSimd, args: &[Operand], ty: Type) {
        // O tipo do vetor: o do primeiro operando vetorial, ou o resultado.
        let k = args.iter().map(|a| self.tipo_de(a)).find(|t| t.e_vetor()).unwrap_or(ty);
        let (n, el, suf) = if k.e_vetor() { forma(k) } else { forma(ty) };
        let kt = k.llvm_ir();
        let ponto_flutuante = k != Type::V4I32;
        let a = |s: &mut Self, i: usize| {
            let t = s.tipo_de(&args[i]);
            s.coagir(&args[i], t)
        };
        let mut w = String::new();
        let r = format!("%v{v}");
        match op {
            OpSimd::Add | OpSimd::Sub | OpSimd::Mul | OpSimd::Div | OpSimd::And | OpSimd::Or | OpSimd::Xor => {
                let (x, y) = (a(self, 0), a(self, 1));
                let instr = match (op, ponto_flutuante) {
                    (OpSimd::Add, true) => "fadd",
                    (OpSimd::Sub, true) => "fsub",
                    (OpSimd::Mul, true) => "fmul",
                    (OpSimd::Div, true) => "fdiv",
                    (OpSimd::Add, false) => "add",
                    (OpSimd::Sub, false) => "sub",
                    (OpSimd::And, false) => "and",
                    (OpSimd::Or, false) => "or",
                    (OpSimd::Xor, false) => "xor",
                    _ => unreachable!("operação SIMD {op:?} sobre {k:?}"),
                };
                writeln!(w, "  {r} = {instr} {kt} {x}, {y}").unwrap();
            }
            OpSimd::Min | OpSimd::Max => {
                let (x, y) = (a(self, 0), a(self, 1));
                let p = if op == OpSimd::Min { "olt" } else { "ogt" };
                writeln!(w, "  %sc{v} = fcmp {p} {kt} {x}, {y}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sc{v}, {kt} {x}, {kt} {y}").unwrap();
            }
            OpSimd::Clamp => {
                let (x, lo, hi) = (a(self, 0), a(self, 1), a(self, 2));
                writeln!(w, "  %sa{v} = fcmp olt {kt} {x}, {hi}").unwrap();
                writeln!(w, "  %sm{v} = select <{n} x i1> %sa{v}, {kt} {x}, {kt} {hi}").unwrap();
                writeln!(w, "  %sb{v} = fcmp ogt {kt} %sm{v}, {lo}").unwrap();
                writeln!(w, "  {r} = select <{n} x i1> %sb{v}, {kt} %sm{v}, {kt} {lo}").unwrap();
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
                let um = self.splat_constante(n, el, "1.0");
                let destino = if op == OpSimd::Recip { r.clone() } else { format!("%sr{v}") };
                writeln!(w, "  {destino} = fdiv {kt} {um}, {x}").unwrap();
                if op == OpSimd::RecipSqrt {
                    writeln!(w, "  {r} = call {kt} @llvm.sqrt.{suf}({kt} %sr{v})").unwrap();
                }
            }
            OpSimd::Escala => {
                let x = a(self, 0);
                let s = self.coagir(&args[1], Type::F64);
                let esc = self.escalar_da_pista(&mut w, v, "e", &s, k);
                self.espalhar(&mut w, v, "e", &esc, k);
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
            OpSimd::Pista(i) => {
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
            OpSimd::ComPista(i) => {
                let x = a(self, 0);
                let esc = match k {
                    Type::V4I32 => {
                        let s = self.coagir(&args[1], Type::I64);
                        writeln!(w, "  %sq{v} = trunc i64 {s} to i32").unwrap();
                        format!("%sq{v}")
                    }
                    _ => {
                        let s = self.coagir(&args[1], Type::F64);
                        self.escalar_da_pista(&mut w, v, "q", &s, k)
                    }
                };
                writeln!(w, "  {r} = insertelement {kt} {x}, {el} {esc}, i32 {i}").unwrap();
            }
            OpSimd::Flag(i) => {
                let x = a(self, 0);
                writeln!(w, "  %sp{v} = extractelement {kt} {x}, i32 {i}").unwrap();
                writeln!(w, "  {r} = icmp ne i32 %sp{v}, 0").unwrap();
            }
            OpSimd::ComFlag(i) => {
                let x = a(self, 0);
                let b = self.coagir(&args[1], Type::I1);
                writeln!(w, "  %sq{v} = select i1 {b}, i32 -1, i32 0").unwrap();
                writeln!(w, "  {r} = insertelement {kt} {x}, i32 %sq{v}, i32 {i}").unwrap();
            }
            OpSimd::Monta | OpSimd::MontaFlags => {
                let mut atual = "poison".to_string();
                for (i, arg) in args.iter().enumerate() {
                    let esc = if op == OpSimd::MontaFlags {
                        let b = self.coagir(arg, Type::I1);
                        writeln!(w, "  %sq{v}_{i} = select i1 {b}, i32 -1, i32 0").unwrap();
                        format!("%sq{v}_{i}")
                    } else if k == Type::V4I32 {
                        let s = self.coagir(arg, Type::I64);
                        writeln!(w, "  %sq{v}_{i} = trunc i64 {s} to i32").unwrap();
                        format!("%sq{v}_{i}")
                    } else {
                        let s = self.coagir(arg, Type::F64);
                        self.escalar_da_pista(&mut w, v, &format!("q{i}_"), &s, k)
                    };
                    let proximo = if i + 1 == args.len() { r.clone() } else { format!("%sv{v}_{i}") };
                    writeln!(w, "  {proximo} = insertelement {kt} {atual}, {el} {esc}, i32 {i}").unwrap();
                    atual = proximo;
                }
            }
            OpSimd::Splat => {
                let s = self.coagir(&args[0], Type::F64);
                let esc = self.escalar_da_pista(&mut w, v, "e", &s, k);
                writeln!(w, "  %sie{v} = insertelement {kt} poison, {el} {esc}, i32 0").unwrap();
                writeln!(w, "  {r} = shufflevector {kt} %sie{v}, {kt} poison, <{n} x i32> zeroinitializer").unwrap();
            }
            OpSimd::Zero => {
                writeln!(w, "  {r} = select i1 true, {kt} zeroinitializer, {kt} zeroinitializer").unwrap();
            }
            OpSimd::SinalMask => {
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
                let alto = if matches!(op, OpSimd::ShuffleMix(_)) { 4 } else { 0 };
                let ind = [m & 3, (m >> 2) & 3, alto + ((m >> 4) & 3), alto + ((m >> 6) & 3)];
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
            OpSimd::Converte => {
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
                    // `Float64x2.fromFloat32x4`: as pistas 0 e 1.
                    for i in 0..2 {
                        writeln!(w, "  %sf{v}_{i} = extractelement <4 x float> {x}, i32 {i}").unwrap();
                        writeln!(w, "  %sd{v}_{i} = fpext float %sf{v}_{i} to double").unwrap();
                    }
                    writeln!(w, "  %sv{v}_0 = insertelement <2 x double> poison, double %sd{v}_0, i32 0").unwrap();
                    writeln!(w, "  {r} = insertelement <2 x double> %sv{v}_0, double %sd{v}_1, i32 1").unwrap();
                }
            }
            OpSimd::Carrega => {
                let e = self.coagir(&args[0], Type::I64);
                let i = self.coagir(&args[1], Type::I64);
                let kt = ty.llvm_ir();
                writeln!(w, "  %sp{v} = inttoptr i64 {e} to ptr").unwrap();
                writeln!(w, "  %sg{v} = getelementptr {kt}, ptr %sp{v}, i64 {i}").unwrap();
                writeln!(w, "  {r} = load {kt}, ptr %sg{v}, align 1").unwrap();
            }
            OpSimd::Grava => {
                let e = self.coagir(&args[0], Type::I64);
                let i = self.coagir(&args[1], Type::I64);
                let t = self.tipo_de(&args[2]);
                let x = self.coagir(&args[2], t);
                let kt = t.llvm_ir();
                writeln!(w, "  %sp{v} = inttoptr i64 {e} to ptr").unwrap();
                writeln!(w, "  %sg{v} = getelementptr {kt}, ptr %sp{v}, i64 {i}").unwrap();
                writeln!(w, "  store {kt} {x}, ptr %sg{v}, align 1").unwrap();
            }
        }
        self.out.push_str(&w);
    }

    /// Um `double` já coagido como pista do vetor `k` (`fptrunc` no
    /// `Float32x4`); devolve o nome.
    fn escalar_da_pista(&self, w: &mut String, v: u32, sufixo: &str, s: &str, k: Type) -> String {
        if k == Type::V4F32 {
            writeln!(w, "  %sx{sufixo}{v} = fptrunc double {s} to float").unwrap();
            format!("%sx{sufixo}{v}")
        } else {
            s.to_string()
        }
    }

    /// `%ss{sufixo}{v}` = o vetor `k` com `esc` em todas as pistas.
    fn espalhar(&self, w: &mut String, v: u32, sufixo: &str, esc: &str, k: Type) {
        let (n, el, _) = forma(k);
        let kt = k.llvm_ir();
        writeln!(w, "  %si{sufixo}{v} = insertelement {kt} poison, {el} {esc}, i32 0").unwrap();
        writeln!(w, "  %ss{sufixo}{v} = shufflevector {kt} %si{sufixo}{v}, {kt} poison, <{n} x i32> zeroinitializer").unwrap();
    }

    /// Uma constante vetorial com `valor` em todas as pistas.
    fn splat_constante(&self, n: u32, el: &str, valor: &str) -> String {
        let pistas: Vec<String> = (0..n).map(|_| format!("{el} {valor}")).collect();
        format!("<{}>", pistas.join(", "))
    }
}
