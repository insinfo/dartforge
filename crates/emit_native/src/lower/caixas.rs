//! Caixas, closures e records no lowering (P2, docs/NATIVO-ESPACO-UNIFICADO.md
//! §3.8 e §4.4): os membros externos feitos em linha e a leitura dos campos de
//! uma `_Closure` (`INSTANCIA`: código, contexto, corpo tipado, ABI — §2.5).
//!
//! Os campos da closure são lidos por `dartforge_object_get` com índice
//! constante, que o emissor expande em linha (a palavra `b+16+8i`; null e
//! `Smi` leem zeros): sem chamada ao runtime.

use super::fn_builder::FnBuilder;
use crate::hir::*;

/// O campo do contexto (ou do valor do ambiente direto) de uma `_Closure`;
/// o campo 0 é o código (a entrada uniforme).
pub(super) const CLOSURE_CONTEXTO: i64 = 1;
/// O campo do corpo tipado.
pub(super) const CLOSURE_TIPADO: i64 = 2;
/// O campo da ABI do corpo tipado.
pub(super) const CLOSURE_ABI: i64 = 3;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// A palavra `campo` do corpo do objeto `obj`, na representação `ty` (a
    /// leitura em linha de `dartforge_object_get`). Só para quem já sabe a
    /// classe do objeto (ou que ele tem o campo).
    pub(super) fn palavra_do_objeto(&mut self, obj: Operand, campo: i64, ty: Type) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(campo)), Type::I64)],
                ret_ty: ty,
            },
            ty,
        )
    }

    /// O contexto da closure `clo` (o `env` que o corpo recebe): o `_Contexto`
    /// das capturas, o valor do ambiente direto ou null.
    pub(super) fn contexto_da_closure(&mut self, clo: Operand) -> Operand {
        self.palavra_do_objeto(clo, CLOSURE_CONTEXTO, Type::Ref)
    }

    /// O membro externo de caixa/closure/record feito em linha, ou `None` (a
    /// chamada de sempre). `membro` é `Classe.nome`; `native`, o nome do native,
    /// se tem.
    pub(super) fn caixa_em_linha(
        &mut self,
        membro: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let _ = membro;
        match (native?, this, args) {
            // `identical(a, b)` (§2.10): `@df.identico`, sem chamada.
            ("Identical_comparison", None, [a, b]) => {
                let a = self.coagir(a.clone(), Type::Ref);
                let b = self.coagir(b.clone(), Type::Ref);
                Some(self.emit(
                    Instruction::CallRuntime {
                        name: "df.identico".to_string(),
                        args: vec![(a, Type::Ref), (b, Type::Ref)],
                        ret_ty: Type::I1,
                    },
                    Type::I1,
                ))
            }
            // `_Smi.hashCode`/`_Mint.hashCode`: o `HashIntegerOp` da VM
            // (`il_x64.cc`): o produto de 96 bits `v · 0x2d51` (v sem sinal),
            // as três palavras de 32 bits dele combinadas por xor, 30 bits.
            ("DartForge_int_hashCode", Some(t), []) => {
                let v = self.coagir(t.clone(), Type::I64);
                let k = |c: i64| Operand::Constant(Constant::Int(c));
                let lo32 = self.emit(Instruction::And(v.clone(), k(0xffff_ffff)), Type::I64);
                let hi32 = self.emit(Instruction::LShr(v, k(32)), Type::I64);
                let p_lo = self.emit(Instruction::Mul(lo32, k(0x2d51)), Type::I64);
                let p_hi = self.emit(Instruction::Mul(hi32, k(0x2d51)), Type::I64);
                let vai = self.emit(Instruction::LShr(p_lo.clone(), k(32)), Type::I64);
                let p_hi = self.emit(Instruction::Add(p_hi, vai), Type::I64);
                // c0 ^ c1 ^ c2: p_lo (baixo), p_hi (baixo) e p_hi >> 32.
                let c2 = self.emit(Instruction::LShr(p_hi.clone(), k(32)), Type::I64);
                let x = self.emit(Instruction::Xor(p_lo, p_hi), Type::I64);
                let x = self.emit(Instruction::Xor(x, c2), Type::I64);
                Some(self.emit(Instruction::And(x, k(0x3fff_ffff)), Type::I64))
            }
            // `_Double._bitsDe(d)`: os bits do `double`.
            ("DartForge_double_bits", None, [d]) | ("DartForge_double_bits", Some(d), []) => {
                let d = self.coagir(d.clone(), Type::F64);
                Some(self.emit(Instruction::Bitcast { op: d, to: Type::I64 }, Type::I64))
            }
            _ => None,
        }
    }
}
