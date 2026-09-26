//! Membros de `int` e `double` em linha, quando o tipo estático do
//! receptor é exatamente `int` ou `double` (não anulável).
//!
//! As duas classes não podem ser estendidas nem implementadas fora do
//! núcleo: o valor é sempre o número, e o membro é o do SDK. Sem isto, um
//! `i.toDouble()` ou `x.isEven` encaixotava o número, passava pelo despacho
//! por seletor e desencaixotava o resultado. Cada forma aqui calcula o
//! mesmo valor que o `int_patch.dart`/`double_patch.dart` da VM; o que
//! pode lançar (`toInt()` de NaN ou infinito) sai pelo membro do SDK no
//! caminho frio.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_types::table::Type as T;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Primitivo {
    Int,
    Double,
}

fn c(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn primitivo(&self, e: ExprId) -> Option<Primitivo> {
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, e)?) else {
            return None;
        };
        if Some(*class) == self.ctx.core.int_class {
            Some(Primitivo::Int)
        } else if Some(*class) == self.ctx.core.double_class {
            Some(Primitivo::Double)
        } else {
            None
        }
    }

    /// O membro de `int`/`double` em linha, ou `None` (o caminho de sempre).
    pub(super) fn expressao_intrinseca(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        let (recv, nome, chamada) = match &ast.expr(e).kind {
            ExprKind::Property { target, name, null_aware: false } => (*target, name.sym, false),
            ExprKind::Call { target, arguments } if arguments.args.is_empty() && arguments.type_args.is_empty() => {
                match &ast.expr(*target).kind {
                    ExprKind::Property { target, name, null_aware: false } => (*target, name.sym, true),
                    _ => return None,
                }
            }
            _ => return None,
        };
        let p = self.primitivo(recv)?;
        let nome = self.ctx.symbol_name(nome);
        let forma = match (p, nome, chamada) {
            (Primitivo::Int, "isEven" | "isOdd" | "isNegative", false) => nome,
            (Primitivo::Int, "toDouble" | "abs" | "toInt" | "floor" | "ceil" | "round" | "truncate", true) => nome,
            (Primitivo::Double, "isNaN" | "isInfinite" | "isFinite" | "isNegative", false) => nome,
            (Primitivo::Double, "toDouble" | "abs" | "toInt" | "truncate", true) => nome,
            _ => return None,
        };
        let v = self.lower_expr(ast, recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        Some(match p {
            Primitivo::Int => self.intrinseco_int(v, forma),
            Primitivo::Double => self.intrinseco_double(v, forma),
        })
    }

    fn intrinseco_int(&mut self, v: Operand, forma: &str) -> Operand {
        let x = self.coagir(v, Type::I64);
        match forma {
            "isEven" | "isOdd" => {
                let bit = self.emit(Instruction::And(x, c(1)), Type::I64);
                let op = if forma == "isEven" { ICmpOp::Eq } else { ICmpOp::Ne };
                self.emit(Instruction::ICmp(op, bit, c(0)), Type::I1)
            }
            "isNegative" => self.emit(Instruction::ICmp(ICmpOp::Slt, x, c(0)), Type::I1),
            "toDouble" => self.emit(Instruction::IntToDouble(x), Type::F64),
            // `(x ^ s) - s` com `s = x >> 63`: o `abs` de `int` com o
            // estouro de 64 bits da VM (`abs(-2^63) == -2^63`).
            "abs" => {
                let s = self.emit(Instruction::AShr(x.clone(), c(63)), Type::I64);
                let t = self.emit(Instruction::Xor(x, s.clone()), Type::I64);
                self.emit(Instruction::Sub(t, s), Type::I64)
            }
            // `toInt`, `floor`, `ceil`, `round`, `truncate` de `int`: ele mesmo.
            _ => x,
        }
    }

    fn intrinseco_double(&mut self, v: Operand, forma: &str) -> Operand {
        let caixa = v.clone();
        let x = self.coagir(v, Type::F64);
        let bits = |s: &mut Self, x: Operand| s.emit(Instruction::Bitcast { op: x, to: Type::I64 }, Type::I64);
        match forma {
            "isNaN" => {
                let igual = self.emit(Instruction::FCmp(FCmpOp::Eq, x.clone(), x), Type::I1);
                self.emit(Instruction::LNot(igual), Type::I1)
            }
            // Expoente todo em 1 e mantissa zero.
            "isInfinite" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Eq, m, c(0x7FF0_0000_0000_0000)), Type::I1)
            }
            // Expoente não todo em 1 (nem infinito nem NaN).
            "isFinite" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Slt, m, c(0x7FF0_0000_0000_0000)), Type::I1)
            }
            // O bit de sinal, fora o NaN (`-0.0.isNegative` é verdade;
            // `double.nan.isNegative`, falso).
            "isNegative" => {
                let b = bits(self, x.clone());
                let sinal = self.emit(Instruction::ICmp(ICmpOp::Slt, b, c(0)), Type::I1);
                let nao_nan = self.emit(Instruction::FCmp(FCmpOp::Eq, x.clone(), x), Type::I1);
                let s = self.emit(Instruction::ZExt { op: sinal, from: Type::I1, to: Type::I64 }, Type::I64);
                let n = self.emit(Instruction::ZExt { op: nao_nan, from: Type::I1, to: Type::I64 }, Type::I64);
                let e = self.emit(Instruction::And(s, n), Type::I64);
                self.emit(Instruction::ICmp(ICmpOp::Ne, e, c(0)), Type::I1)
            }
            "toDouble" => x,
            "abs" => {
                let b = bits(self, x);
                let m = self.emit(Instruction::And(b, c(i64::MAX)), Type::I64);
                self.emit(Instruction::Bitcast { op: m, to: Type::F64 }, Type::F64)
            }
            // `toInt`/`truncate`: em [-2^63, 2^63) o `fptosi`; fora (e NaN),
            // o membro do SDK, que lança o `UnsupportedError`.
            _ => {
                let lo = self.emit(Instruction::FCmp(FCmpOp::Ge, x.clone(), Operand::Constant(Constant::Double(-(2f64.powi(63))))), Type::I1);
                let hi = self.emit(Instruction::FCmp(FCmpOp::Lt, x.clone(), Operand::Constant(Constant::Double(2f64.powi(63)))), Type::I1);
                let lo = self.emit(Instruction::ZExt { op: lo, from: Type::I1, to: Type::I64 }, Type::I64);
                let hi = self.emit(Instruction::ZExt { op: hi, from: Type::I1, to: Type::I64 }, Type::I64);
                let ok = self.emit(Instruction::And(lo, hi), Type::I64);
                let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, ok, c(0)), Type::I1);
                let rapido = self.new_block();
                let lento = self.new_block();
                let juncao = self.new_block();
                self.terminate(Terminator::CondBranch { cond: ok, then_block: rapido, else_block: lento });
                self.set_block(rapido);
                let r = self.emit(Instruction::DoubleToInt(x.clone()), Type::I64);
                let fim_rapido = self.current_block;
                self.terminate(Terminator::Branch(juncao));
                self.set_block(lento);
                let caixa = self.coagir(caixa, Type::Ref);
                let s = self.chamar_por_nome(caixa, super::sdk_fonte::Tipo::Chamar, forma, &[]);
                let s = self.coagir(s, Type::I64);
                let fim_lento = self.current_block;
                if self.is_terminated() {
                    self.set_block(juncao);
                    return r;
                }
                self.terminate(Terminator::Branch(juncao));
                self.set_block(juncao);
                self.emit(Instruction::Phi { incoming: vec![(fim_rapido, r), (fim_lento, s)], ty: Type::I64 }, Type::I64)
            }
        }
    }
}
