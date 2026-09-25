//! Operadores binários sobre operandos já na representação do seu tipo (R5).
//!
//! A escolha da operação vem das representações: `I64` com `I64` é aritmética
//! inteira; um `F64` promove o outro lado (`IntToDouble`, nunca `bitcast`);
//! igualdade envolvendo `Ref` vai ao runtime (`==` do Dart, com caixas por
//! valor — R9). Antes, "algum lado é `Ref`" queria dizer "concatena strings",
//! o que valia só porque tudo que não era `int` era `Ref`.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::BinaryOp;

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn e_escalar_numerico(t: Type) -> bool {
        matches!(t, Type::I64 | Type::F64)
    }

    /// `identical(a, b)`: escalares por valor (bits, para `double`); se
    /// algum lado é `Ref`, pelo runtime (handle igual, ou caixas de mesmo
    /// valor — `Instance::IsIdenticalTo` da VM).
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
                let r = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_identical".to_string(),
                        args: vec![(a, Type::Ref), (b, Type::Ref)],
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                );
                self.emit(
                    Instruction::Trunc {
                        op: r,
                        from: Type::I8,
                        to: Type::I1,
                    },
                    Type::I1,
                )
            }
        }
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
            _ if self.ctx.sdk_da_fonte => return self.texto_por_seletor(op),
            _ => {
                let op = self.coagir(op, Type::Ref);
                return self.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: "dartforge_dispatch_toString".to_string(),
                        args: vec![op],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
            }
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
        if self.ctx.sdk_da_fonte {
            return self.igualdade_fonte(a, b);
        }
        // `operator ==` de uma classe do programa (§17.26: com um lado null
        // vale `identical`; senão, `a.==(b)` pela classe dinâmica de `a`).
        let alvos: Vec<(i64, super::despacho::Alvo)> = self
            .alvos_por_nome("==")
            .into_iter()
            .filter(|(_, x)| matches!(x, super::despacho::Alvo::Funcao(_)))
            .collect();
        if !alvos.is_empty() {
            let b_nulo = self.emit(
                Instruction::ICmp(ICmpOp::Eq, b.clone(), Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_din = self.new_block();
            let b_id = self.new_block();
            let juncao = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: b_nulo,
                then_block: b_id,
                else_block: b_din,
            });
            self.set_block(b_din);
            let (a2, b2, b3) = (a.clone(), b.clone(), b.clone());
            let r = self.despachar(
                a.clone(),
                &alvos,
                super::despacho::Uso::Chamar,
                &mut |_s: &mut Self| vec![(None, b3.clone())],
                &mut |s: &mut Self| {
                    let r = s.igualdade_do_runtime(a2.clone(), b2.clone());
                    s.coagir(r, Type::Ref)
                },
                dartforge_diagnostics::Span { start: 0, end: 0 },
            );
            let r1 = self.coagir(r, Type::I1);
            let fim1 = self.current_block;
            self.terminate(Terminator::Branch(juncao));
            self.set_block(b_id);
            let r2 = self.igualdade_do_runtime(a, b);
            let fim2 = self.current_block;
            self.terminate(Terminator::Branch(juncao));
            self.set_block(juncao);
            return self.emit(
                Instruction::Phi {
                    incoming: vec![(fim1, r1), (fim2, r2)],
                    ty: Type::I1,
                },
                Type::I1,
            );
        }
        self.igualdade_do_runtime(a, b)
    }

    /// `==` do runtime (identidade, caixas por valor, strings por conteúdo),
    /// passando antes pela igualdade estrutural dos records com forma
    /// quando o programa tem algum (`registros.rs`).
    fn igualdade_do_runtime(&mut self, a: Operand, b: Operand) -> Operand {
        if !self.ctx.formas_de_record.is_empty() {
            return self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: super::registros::SIMBOLO_IGUAL.to_string(),
                    args: vec![a, b],
                    ret_ty: Type::I1,
                },
                Type::I1,
            );
        }
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_equal".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        self.emit(
            Instruction::Trunc {
                op: r,
                from: Type::I8,
                to: Type::I1,
            },
            Type::I1,
        )
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
            let a = self.coagir(lop, Type::Ref);
            let n = self.coagir(rop, Type::I64);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_repeat".to_string(),
                    args: vec![(a, Type::Ref), (n, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
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
            BinaryOp::Rem => self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_nativo_Integer_moduloFromInteger".to_string(),
                    args: vec![(rop, Type::I64), (lop, Type::I64)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            ),
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
