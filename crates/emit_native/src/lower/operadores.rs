//! Operadores binários sobre operandos já na representação do seu tipo (R5).
//!
//! A escolha da operação vem das representações: `I64` com `I64` é aritmética
//! inteira; um `F64` promove o outro lado (`IntToDouble`, nunca `bitcast`);
//! igualdade envolvendo `Ref` vai ao `==` do Dart pela classe dinâmica (a
//! regra do null em linha, `igualdade_fonte`); `identical` de `Ref` é o
//! `@df.identico` em linha (docs/NATIVO-ESPACO-UNIFICADO.md §2.10). Antes,
//! "algum lado é `Ref`" queria dizer "concatena strings", o que valia só
//! porque tudo que não era `int` era `Ref`.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::BinaryOp;

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn e_escalar_numerico(t: Type) -> bool {
        matches!(t, Type::I64 | Type::F64)
    }

    /// `identical(a, b)`: escalares por valor (bits, para `double`); se
    /// algum lado é `Ref`, pelo `@df.identico` em linha (handle igual, ou
    /// `_Mint`/`_Double` de mesmo valor — `Instance::IsIdenticalTo` da VM;
    /// o ajudante é da P2, `llvm/caixas_ir.rs`).
    pub fn identicos(&mut self, a: Operand, b: Operand) -> Operand {
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
                let a = self.coagir(a, Type::Ref);
                let b = self.coagir(b, Type::Ref);
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

    /// `a == b` com `a` de tipo estático `String` não anulável (o `==` da
    /// `String` não é sobrescrevível): o `@df.texto_igual_a` em linha — a
    /// identidade, a classe de `b`, o comprimento e o hash, e as unidades no
    /// runtime só quando preciso (P1, `llvm/textos_ir.rs`).
    pub fn igualdade_de_texto(&mut self, a: Operand, b: Operand) -> Operand {
        let a = self.coagir(a, Type::Ref);
        let b = self.coagir(b, Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "df.texto_igual_a".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::I1,
            },
            Type::I1,
        )
    }

    /// Operando `Ref` cujo tipo estático é `int?`/`double?` (ou `int`,
    /// `double`) volta ao escalar por `Unbox` — o operador exige o valor, e
    /// num programa Dart válido ele não é null aqui (foi promovido).
    pub fn desnulificar_numerico(
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
        let classe = &self.ctx.program.classes[class.0 as usize];
        if self.ctx.biblioteca_compilada(classe.library) {
            return op;
        }
        match self.ctx.symbol_name(classe.name) {
            "int" => self.coagir(op, Type::I64),
            "double" => self.coagir(op, Type::F64),
            _ => op,
        }
    }

    /// `toString()` de um valor em qualquer representação.
    pub fn texto_de(&mut self, op: Operand) -> Operand {
        let (nome, ty) = match self.operand_type(&op) {
            Type::I64 => ("dartforge_to_string_i64", Type::I64),
            Type::F64 => ("dartforge_to_string_f64", Type::F64),
            Type::I1 | Type::I8 => ("dartforge_to_string_bool", Type::I8),
            _ => return self.texto_por_seletor(op),
        };
        self.emit(
            Instruction::CallRuntime {
                name: nome.to_string(),
                args: vec![(op, ty)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `a == b`.
    fn iguais(&mut self, a: Operand, b: Operand) -> Operand {
        let (ta, tb) = (self.operand_type(&a), self.operand_type(&b));
        if Self::e_escalar_numerico(ta) && Self::e_escalar_numerico(tb) {
            if ta == Type::I64 && tb == Type::I64 {
                return self.emit(Instruction::ICmp(ICmpOp::Eq, a, b), Type::I1);
            }
            let x = self.coagir(a, Type::F64);
            let y = self.coagir(b, Type::F64);
            return self.emit(Instruction::FCmp(FCmpOp::Eq, x, y), Type::I1);
        }
        if matches!(ta, Type::I1 | Type::I8) && matches!(tb, Type::I1 | Type::I8) {
            let x = self.coagir(a, Type::I1);
            let y = self.coagir(b, Type::I1);
            return self.emit(Instruction::ICmp(ICmpOp::Eq, x, y), Type::I1);
        }
        if ta != Type::Ref && tb != Type::Ref {
            // Escalares de tipos diferentes (`1 == true`): nunca iguais.
            return Operand::Constant(Constant::Bool(false));
        }
        let a = self.coagir(a, Type::Ref);
        let b = self.coagir(b, Type::Ref);
        // §17.26: com um lado null vale `identical`; senão, `a.==(b)` pela
        // classe dinâmica de `a` (o `==` da fonte do SDK ou do programa).
        self.igualdade_fonte(a, b)
    }

    /// O operador de `int` que o caminho do `Smi` calcula em linha.
    fn op_de_smi(op: BinaryOp) -> bool {
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

    /// `a op b` com algum lado `Ref` (`num`, `dynamic`, `Object`…): se os
    /// dois são `int` pequenos — só um `int` pode ser `Smi` (R10), então o
    /// operador é o de `int`, qualquer que seja o tipo estático —, a conta
    /// em linha, com o estouro de 64 bits do Dart; senão o despacho de
    /// sempre. É o `this < other` do `int.compareTo(num other)` do SDK.
    fn operar_smi(&mut self, op: BinaryOp, lop: Operand, rop: Operand, span: dartforge_diagnostics::Span) -> Operand {
        let um = Operand::Constant(Constant::Int(1));
        let mut conds = Vec::new();
        for x in [&lop, &rop] {
            if self.operand_type(x) == Type::Ref {
                let b = self.emit(Instruction::And(x.clone(), um.clone()), Type::I64);
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
        let rapido = self.new_block();
        let lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: smi, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        let valor = |s: &mut Self, x: &Operand| {
            if s.operand_type(x) == Type::Ref {
                s.emit(Instruction::AShr(x.clone(), um.clone()), Type::I64)
            } else {
                x.clone()
            }
        };
        let (a, b) = (valor(self, &lop), valor(self, &rop));
        let comparacao = matches!(op, BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq);
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
        // O resultado do despacho é `Ref` (a soma de `num` pode ser
        // `double`); a comparação, `bool`.
        let ty = if comparacao { Type::I1 } else { Type::Ref };
        let r = self.coagir(r, ty);
        let fim_rapido = self.current_block;
        self.terminate(Terminator::Branch(juncao));

        self.set_block(lento);
        let s = self.operar_dinamico(op, lop, rop, span);
        let s = self.coagir(s, ty);
        let fim_lento = self.current_block;
        if self.is_terminated() {
            self.set_block(juncao);
            return r;
        }
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        self.emit(Instruction::Phi { incoming: vec![(fim_rapido, r), (fim_lento, s)], ty }, ty)
    }

    /// Operador binário (não curto-circuito). `texto`: algum lado é
    /// `String` pelo tipo estático — `+` concatena, `*` repete.
    pub fn operar(
        &mut self,
        op: BinaryOp,
        lop: Operand,
        rop: Operand,
        texto: bool,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        if texto && matches!(op, BinaryOp::Add) {
            let a = self.coagir(lop, Type::Ref);
            let b = self.coagir(rop, Type::Ref);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_concat".to_string(),
                    args: vec![(a, Type::Ref), (b, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        if texto && matches!(op, BinaryOp::Mul) {
            // `String.*` da fonte do SDK (`_StringBase.operator *`).
            let a = self.coagir(lop, Type::Ref);
            let n = self.coagir(rop, Type::Ref);
            let r = self.chamar_por_nome(a, super::sdk_fonte::Tipo::Chamar, "*", &[(None, n)]);
            return self.coagir(r, Type::Ref);
        }
        match op {
            BinaryOp::Eq => return self.iguais(lop, rop),
            BinaryOp::NotEq => {
                let e = self.iguais(lop, rop);
                let e = self.para_bool(e);
                return self.emit(Instruction::LNot(e), Type::I1);
            }
            _ => {}
        }
        let (ta, tb) = (self.operand_type(&lop), self.operand_type(&rop));
        if !Self::e_escalar_numerico(ta) || !Self::e_escalar_numerico(tb) {
            if Self::op_de_smi(op) && matches!(ta, Type::I64 | Type::Ref) && matches!(tb, Type::I64 | Type::Ref) {
                return self.operar_smi(op, lop, rop, span);
            }
            return self.operar_dinamico(op, lop, rop, span);
        }
        let em_double = ta == Type::F64 || tb == Type::F64 || matches!(op, BinaryOp::Div);
        if em_double {
            let a = self.coagir(lop, Type::F64);
            let b = self.coagir(rop, Type::F64);
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
                _ => self.nao_suportado("operador de bits ou resto sobre double", span),
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
            // `a % c` com `c` constante: em linha. O `%` do Dart devolve um
            // valor em `[0, |c|)`: o resto truncado mais `|c|` quando ele é
            // negativo (`(r >> 63) & |c|`, sem desvio). `c = ±1` dá sempre 0
            // (e evita o `srem` de `i64::MIN` por `-1`, indefinido no LLVM);
            // `c = 0` lança e `c = i64::MIN` não tem `|c|`: ficam com o runtime.
            BinaryOp::Rem
                if matches!(rop, Operand::Constant(Constant::Int(c)) if c != 0 && c != i64::MIN) =>
            {
                let Operand::Constant(Constant::Int(c)) = rop else { unreachable!() };
                if c == 1 || c == -1 {
                    return Operand::Constant(Constant::Int(0));
                }
                let r = self.emit(Instruction::SRem(lop, Operand::Constant(Constant::Int(c))), Type::I64);
                let sinal = self.emit(Instruction::AShr(r.clone(), Operand::Constant(Constant::Int(63))), Type::I64);
                let ajuste = self.emit(Instruction::And(sinal, Operand::Constant(Constant::Int(c.abs()))), Type::I64);
                self.emit(Instruction::Add(r, ajuste), Type::I64)
            }
            BinaryOp::Rem => {
                // O native não confere o divisor (o Dart confere antes).
                self.exigir_divisor(&rop);
                self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_nativo_Integer_moduloFromInteger".to_string(),
                    args: vec![(rop, Type::I64), (lop, Type::I64)],
                    ret_ty: Type::I64,
                },
                Type::I64,
                )
            }
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
        }
    }
}
