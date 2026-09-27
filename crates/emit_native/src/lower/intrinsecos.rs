//! Membros de `int` e `double` em linha, quando o tipo estático do
//! receptor é exatamente `int` ou `double` (não anulável), e o `add` de
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

    /// `ClassID.cidX` (`dart:_internal`): na VM, campos `static final` que o
    /// runtime preenche com o id de classe de `_List`, `_OneByteString`…; o
    /// SDK compara `ClassID.getID(x)` com eles para tomar os atalhos (o
    /// `split` de um caractere, os laços de `_List`). Aqui o id de cada
    /// classe é o que `dartforge_value_class` devolve (a tabela de
    /// `sdk_modulo::cids_do_runtime`), conhecido ao compilar; sem esta
    /// troca, os campos ficavam 0 e o SDK nunca tomava os atalhos. Um campo
    /// sem classe correspondente no runtime fica como está.
    fn id_de_classe_do_runtime(&self, ast: &ast::Ast, e: ExprId) -> Option<i64> {
        let ExprKind::Property { name, null_aware: false, .. } = &ast.expr(e).kind else { return None };
        let Some(dartforge_types::resolved::Resolved::Member { class, .. }) = self.ctx.get_resolved(self.unit_id, e) else {
            return None;
        };
        if Some(*class) != self.ctx.classe_do_sdk("_internal", "ClassID") {
            return None;
        }
        let (lib, classe) = match self.ctx.symbol_name(name.sym) {
            "cidArray" => ("core", "_List"),
            "cidGrowableObjectArray" => ("core", "_GrowableList"),
            "cidImmutableArray" => ("core", "_ImmutableList"),
            "cidOneByteString" => ("core", "_OneByteString"),
            "cidTwoByteString" => ("core", "_TwoByteString"),
            "cidUint8ArrayView" => ("typed_data", "_Uint8ArrayView"),
            "cidUint8Array" => ("typed_data", "_Uint8List"),
            "cidInt8Array" => ("typed_data", "_Int8List"),
            "cidUint8ClampedArray" => ("typed_data", "_Uint8ClampedList"),
            _ => return None,
        };
        let c = self.ctx.classe_do_sdk(lib, classe)?;
        self.ctx.id_de_classe(c).map(i64::from)
    }

    /// O membro de `int`/`double` em linha, ou `None` (o caminho de sempre).
    pub(super) fn expressao_intrinseca(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        if let Some(id) = self.id_de_classe_do_runtime(ast, e) {
            return Some(self.emit(Instruction::Const(Constant::Int(id)), Type::I64));
        }
        if let Some(r) = self.lista_add_escalar(ast, e) {
            return Some(r);
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

    /// `lista.add(v)` com a lista de tipo estático `List<int>`,
    /// `List<double>` ou `List<bool>` e `v` exatamente desse tipo (sem caixa):
    /// `dartforge_lista_add_escalar` acrescenta o escalar numa lista
    /// crescível do runtime; qualquer outra (uma classe do usuário, tamanho
    /// fixo, não modificável, `E` que não aceita o valor) chama o `add` de
    /// sempre. Sem isto, cada `add` passava pelo despacho, conferia o `E`
    /// duas vezes pela RTI (na entrada uniforme e no `[]=` dentro do `add`)
    /// e gravava o valor encaixotado.
    fn lista_add_escalar(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
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
        let Some(super::tipados::Indexavel::Nucleo { gravacao: Some(t) }) =
            self.indexavel(self.ctx.get_type(self.unit_id, *recv))
        else {
            return None;
        };
        // O argumento tem de ser do tipo do elemento, não anulável: um
        // `dynamic` ou `num` precisaria da conversão implícita conferida.
        let arg = arguments.args[0].value;
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(self.ctx.get_type(self.unit_id, arg)?) else {
            return None;
        };
        let (esperada, codigo) = match t {
            Type::I64 => (self.ctx.core.int_class, 1),
            Type::F64 => (self.ctx.core.double_class, 2),
            _ => (self.ctx.core.bool_class, 3),
        };
        if Some(*class) != esperada {
            return None;
        }
        let lista = self.lower_expr(ast, *recv);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let lista = self.coagir(lista, Type::Ref);
        let v = self.lower_expr(ast, arg);
        if self.is_terminated() {
            return Some(Operand::Constant(Constant::Null));
        }
        let v = self.coagir(v, t);
        let bits = match t {
            Type::I64 => v.clone(),
            Type::F64 => self.emit(Instruction::Bitcast { op: v.clone(), to: Type::I64 }, Type::I64),
            _ => self.emit(Instruction::ZExt { op: v.clone(), from: Type::I1, to: Type::I64 }, Type::I64),
        };
        let feito = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_lista_add_escalar".to_string(),
                args: vec![(lista.clone(), Type::Ref), (bits, Type::I64), (c(codigo), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, feito, c(0)), Type::I1);
        let lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: juncao, else_block: lento });
        self.set_block(lento);
        self.chamar_por_nome(lista, super::sdk_fonte::Tipo::Chamar, "add", &[(None, v)]);
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        Some(Operand::Constant(Constant::Null))
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
