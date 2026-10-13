//! Membros de `int` e `double` em linha, quando o tipo estático do
//! receptor é exatamente `int` ou `double` (não anulável) — o `compareTo`
//! de `int` com argumento `int` inclusive —, e o `add` de
//! `List<int>`/`List<double>`/`List<bool>` sem caixa.
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

    /// `ClassID.cidX` (`dart:_internal`): na VM, campos `static final` que o
    /// runtime preenche com o id de classe de `_List`, `_OneByteString`…; o
    /// SDK compara `ClassID.getID(x)` com eles para tomar os atalhos (o
    /// `split` de um caractere, os laços de `_List`). Aqui o cid dessas
    /// classes é fixo (`layout::cid`, docs/NATIVO-ESPACO-UNIFICADO.md §2.4), o
    /// mesmo no runtime, no emissor e em toda geração: a constante do contrato.
    /// `numPredefinedCids` é o primeiro cid livre (as classes do runtime ficam
    /// abaixo, como as predefinidas da VM no hash das constantes,
    /// `compact_hash.dart`). As classes externas da VM (`cidExternal*`) não
    /// existem aqui: o campo fica como está (0, que nenhum objeto tem).
    fn runtime_class_id(&self, ast: &ast::Ast, e: ExprId) -> Option<i64> {
        use dartforge_runtime::layout::{cid, PRIMEIRO_CID_LIVRE};
        use dartforge_runtime::tipadas::tipo;
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
            "cidUint8ArrayView" => cid::visao(tipo::UINT8, false),
            "cidUint8Array" => cid::tipada(tipo::UINT8),
            "cidInt8ArrayView" => cid::visao(tipo::INT8, false),
            "cidInt8Array" => cid::tipada(tipo::INT8),
            "cidUint8ClampedArray" => cid::tipada(tipo::UINT8_CLAMPED),
            "numPredefinedCids" => return Some(PRIMEIRO_CID_LIVRE),
            _ => return None,
        };
        Some(i64::from(id))
    }

    /// O membro de `int`/`double` em linha, ou `None` (o caminho de sempre).
    pub(super) fn intrinsic_expression(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if let Some(id) = self.runtime_class_id(ast, e) {
            return Some(self.emit(Instruction::Const(Constant::Int(id)), Type::I64));
        }
        // Membro de tipo de extensão: o receptor tem o tipo apagado de um
        // primitivo ou de uma lista, mas o membro é o do tipo de extensão.
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

    /// `a.compareTo(b)` com `a` e `b` exatamente `int` (não anuláveis): o
    /// `_IntegerImplementation.compareTo` da VM com `other` inteiro é `-1`
    /// se `a < b`, `1` se `a > b`, senão `0`; aqui, `(a > b) - (a < b)` em
    /// linha. Sem isto, os dois números iam encaixotados (um `_Mint` novo
    /// para cada um fora do `Smi`) ao despacho por seletor e à entrada
    /// uniforme, que conferia o argumento pela RTI, e o `this < other` do
    /// corpo (`num other`) ia ao despacho dinâmico para todo `_Mint` — o
    /// comparador `(a, b) => a.compareTo(b)` de `sort` pagava tudo isso a
    /// cada comparação. O `double` (e o `num`) fica com o membro do SDK:
    /// NaN e `-0.0` têm regras próprias.
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

    /// `list.add(v)` com a lista de tipo estático `List<int>`,
    /// `List<double>` ou `List<bool>` e `v` exatamente desse tipo (sem caixa),
    /// como o `add` da VM em linha: numa `_GrowableList` (cid 10) de
    /// armazenamento compacto da forma do valor (docs/NATIVO-ESPACO-UNIFICADO.md
    /// §2.16: a forma compacta só existe quando o `E` reificado é o escalar)
    /// com lugar, a palavra gravada direto e o comprimento mais um; cheia,
    /// `dartforge_lista_acrescentar` (cresce para `(capacity * 2) | 3`);
    /// qualquer outra (uma classe do usuário, tamanho fixo, não modificável,
    /// armazenamento geral) chama o `add` de sempre. Sem isto, cada `add`
    /// passava pelo despacho, conferia o `E` duas vezes pela RTI (na entrada
    /// uniforme e no `[]=` dentro do `add`) e gravava o valor encaixotado.
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
        // O argumento tem de ser do tipo do elemento, não anulável: um
        // `dynamic` ou `num` precisaria da conversão implícita conferida.
        let arg = arguments.args[0].value;
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, arg)?) else {
            return None;
        };
        let (expected, shape) = match t {
            Type::I64 => (self.ctx.core.int_class, flags::BRUTO | flags::ELEMENTO_INT),
            Type::F64 => (self.ctx.core.double_class, flags::BRUTO | flags::ELEMENTO_DOUBLE),
            _ => (self.ctx.core.bool_class, flags::BRUTO | flags::ELEMENTO_BOOL),
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
        // A palavra da forma: o `i64`, o `double` (os bits, pela gravação de
        // `F64`) ou 0/1.
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

        // Cheia: o crescimento (e a gravação) no runtime, com o valor numa
        // caixa (o `Smi`, sem alocação, no caso comum do `int`).
        self.set_block(full);
        let boxed = self.coerce(v.clone(), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_acrescentar".to_string(),
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

    fn double_intrinsic(&mut self, v: Operand, shape: &str) -> Operand {
        let boxed = v.clone();
        let x = self.coerce(v, Type::F64);
        let bits = |s: &mut Self, x: Operand| s.emit(Instruction::Bitcast { op: x, to: Type::I64 }, Type::I64);
        match shape {
            "isNaN" => {
                let equal = self.emit(Instruction::FCmp(FCmpOp::Eq, x.clone(), x), Type::I1);
                self.emit(Instruction::LNot(equal), Type::I1)
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
            // `toInt`/`truncate`: em [-2^63, 2^63) o `fptosi`; fora (e NaN),
            // o membro do SDK, que lança o `UnsupportedError`.
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
