//! `String.codeUnitAt` e `String.length` em linha (item 2 de
//! docs/NATIVO-PLANO.md §9.9; a seção JSON do mesmo documento).
//!
//! Na VM os dois são intrínsecos: uma carga do comprimento no cabeçalho do
//! objeto e uma carga da unidade. Aqui eram uma chamada ao runtime cada
//! (`DartForge_string_codeUnitAt`, `String_getLength`: TLS, empréstimo do
//! heap, consulta ao slot), por unidade lida — o laço do `_JsonStringParser`
//! (`parseString`, `parse`) e o `writeStringContent` do `_JsonStringifier`
//! faziam duas por caractere.
//!
//! As unidades de um `Texto` (`heap.rs`) moram num `Vec` que nunca muda de
//! tamanho depois de criado (a string é imutável; o `writeInto*String` da
//! construção grava no lugar), então o endereço delas é fixo enquanto a
//! string vive. O runtime dá esse endereço e o comprimento
//! (`dartforge_texto_dados`, com o bit 63 ligado no `_TwoByteString`, e
//! `dartforge_texto_len`, puras do handle); o código lê a unidade em linha.
//! Um valor que não é string do runtime tem endereço 0 e comprimento 0, e o
//! índice fora da faixa também vai à chamada de antes, que lança o
//! `RangeError` da VM.
//!
//! A string relida de um campo a cada acesso (o `chunk` do parser) passa
//! por um cache em cada ponto de leitura, como o das listas tipadas (`tipados.rs`,
//! `cabecalho_tipado`): o último handle (num local `Ref`, enraizado: a
//! string não morre e o handle não é reusado enquanto está no cache), o
//! endereço e o comprimento. A de parâmetro ou local fica com as chamadas
//! puras, que o LLVM tira dos laços.

use super::fn_builder::FnBuilder;
use crate::hir::*;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O membro externo de `_StringBase` feito em linha, ou `None` (a
    /// chamada de sempre): `codeUnitAt` (`membro`) e o getter `length`
    /// (`native` `String_getLength`).
    pub(super) fn texto_em_linha(
        &mut self,
        membro: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let this = this?.clone();
        if self.operand_type(&this) != Type::Ref {
            return None;
        }
        match (membro, native) {
            ("_StringBase.codeUnitAt", None) if args.len() == 1 => {
                let i = self.coagir(args[0].clone(), Type::I64);
                Some(self.unidade_em_linha(this, i))
            }
            (_, Some("String_getLength")) if args.is_empty() => Some(self.comprimento_em_linha(this)),
            _ => None,
        }
    }

    /// `s.length`: o comprimento do runtime; o que não é string do runtime
    /// (endereço 0) volta ao native.
    fn comprimento_em_linha(&mut self, s: Operand) -> Operand {
        let (dados, n) = self.dados_do_texto(&s);
        let e_texto = self.emit(Instruction::ICmp(ICmpOp::Ne, dados, Operand::Constant(Constant::Int(0))), Type::I1);
        let rapido = self.current_block;
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: e_texto, then_block: fim, else_block: lento });
        self.set_block(lento);
        let r = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo("String_getLength"),
                args: vec![(s, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let fim_lento = self.current_block;
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
        self.emit(Instruction::Phi { incoming: vec![(rapido, n), (fim_lento, r)], ty: Type::I64 }, Type::I64)
    }

    /// `s.codeUnitAt(i)`: a unidade lida em linha quando `0 <= i < n`; fora
    /// da faixa (ou `s` sem unidades do runtime), o native, que lança.
    fn unidade_em_linha(&mut self, s: Operand, i: Operand) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let (dados, n) = self.dados_do_texto(&s);
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let rapido = self.new_block();
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        let dois = self.emit(Instruction::ICmp(ICmpOp::Slt, dados.clone(), int(0)), Type::I1);
        let p = self.emit(Instruction::And(dados, int(i64::MAX)), Type::I64);
        let b1 = self.new_block();
        let b2 = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dois, then_block: b2, else_block: b1 });
        self.set_block(b1);
        let u1 = self.emit(Instruction::CargaNativa { endereco: p.clone(), indice: i.clone(), tipo: TipoC::U8 }, Type::I64);
        self.terminate(Terminator::Branch(fim));
        self.set_block(b2);
        let u2 = self.emit(Instruction::CargaNativa { endereco: p, indice: i.clone(), tipo: TipoC::U16 }, Type::I64);
        self.terminate(Terminator::Branch(fim));

        self.set_block(lento);
        let r = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo("DartForge_string_codeUnitAt"),
                args: vec![(s, Type::Ref), (i, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let fim_lento = self.current_block;
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
        self.emit(Instruction::Phi { incoming: vec![(b1, u1), (b2, u2), (fim_lento, r)], ty: Type::I64 }, Type::I64)
    }

    /// O endereço das unidades (bit 63: duas por unidade) e o comprimento
    /// de `s`; (0, 0) quando `s` não é string do runtime.
    fn dados_do_texto(&mut self, s: &Operand) -> (Operand, Operand) {
        let chamada = |b: &mut Self, nome: &str| {
            b.emit(
                Instruction::CallRuntime { name: nome.to_string(), args: vec![(s.clone(), Type::Ref)], ret_ty: Type::I64 },
                Type::I64,
            )
        };
        let chave = match s {
            Operand::Val(v) if self.async_estado.is_none() && self.texto_relido(s) => *v,
            _ => {
                let d = chamada(self, "dartforge_texto_dados");
                let n = chamada(self, "dartforge_texto_len");
                return (d, n);
            }
        };
        // Um cache por valor lido (o mesmo campo lido em dois lugares são
        // dois caches): um só por função alternava entre as strings de
        // dois campos e falhava a cada acesso.
        let (cache_h, cache_d, cache_n) = match self.caches_de_texto.get(&chave) {
            Some(c) => c.clone(),
            None => {
                let h = self.alloca_na_entrada(Type::Ref);
                let d = self.alloca_na_entrada(Type::I64);
                let n = self.alloca_na_entrada(Type::I64);
                // Vazio: o handle null nunca acerta (a string não é null).
                self.gravar_no_inicio(&h, Operand::Constant(Constant::Null));
                self.gravar_no_inicio(&d, Operand::Constant(Constant::Int(0)));
                self.gravar_no_inicio(&n, Operand::Constant(Constant::Int(0)));
                self.caches_de_texto.insert(chave, (h.clone(), d.clone(), n.clone()));
                (h, d, n)
            }
        };
        let h = self.emit(Instruction::Load { ptr: cache_h.clone(), ty: Type::Ref }, Type::Ref);
        let igual = self.emit(Instruction::ICmp(ICmpOp::Eq, s.clone(), h), Type::I1);
        let acerto = self.new_block();
        let falha = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: igual, then_block: acerto, else_block: falha });
        self.set_block(acerto);
        let d_acerto = self.emit(Instruction::Load { ptr: cache_d.clone(), ty: Type::I64 }, Type::I64);
        let n_acerto = self.emit(Instruction::Load { ptr: cache_n.clone(), ty: Type::I64 }, Type::I64);
        let fim_acerto = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(falha);
        let d_falha = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_texto_na_falha".to_string(),
                args: vec![(s.clone(), Type::Ref), (cache_n.clone(), Type::Ptr)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let n_falha = self.emit(Instruction::Load { ptr: cache_n, ty: Type::I64 }, Type::I64);
        self.emit(Instruction::Store { ptr: cache_h, val: s.clone() }, Type::Void);
        self.emit(Instruction::Store { ptr: cache_d, val: d_falha.clone() }, Type::Void);
        let fim_falha = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        let d = self.emit(
            Instruction::Phi { incoming: vec![(fim_acerto, d_acerto), (fim_falha, d_falha)], ty: Type::I64 },
            Type::I64,
        );
        let n = self.emit(
            Instruction::Phi { incoming: vec![(fim_acerto, n_acerto), (fim_falha, n_falha)], ty: Type::I64 },
            Type::I64,
        );
        (d, n)
    }

    /// `s` é relido a cada acesso (campo, global, ou a junção da via
    /// rápida de um campo `late`): o critério de `tipados.rs`
    /// (`lido_de_campo`), para o cache.
    fn texto_relido(&self, s: &Operand) -> bool {
        let leitura = |v: &ValueId| {
            self.func.blocks.iter().flat_map(|b| b.instructions.iter()).any(|(vid, inst, _)| {
                *vid == *v
                    && match inst {
                        Instruction::GetField { .. } | Instruction::LoadGlobal { .. } => true,
                        Instruction::CallRuntime { name, .. } => name == "dartforge_object_get",
                        _ => false,
                    }
            })
        };
        let Operand::Val(v) = s else { return false };
        if leitura(v) || self.lidos_de_global.contains(v) {
            return true;
        }
        self.func.blocks.iter().flat_map(|b| b.instructions.iter()).any(|(vid, inst, _)| {
            *vid == *v
                && matches!(inst, Instruction::Phi { incoming, .. }
                    if incoming.iter().any(|(_, o)| matches!(o, Operand::Val(w) if leitura(w))))
        })
    }

    /// `*ptr = val` no bloco de entrada, logo depois dos `alloca`.
    fn gravar_no_inicio(&mut self, ptr: &Operand, val: Operand) {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, Type::Void);
        let b0 = self.func.blocks.iter().position(|b| b.id == BlockId(0)).expect("bloco de entrada");
        let pos = self.n_allocas;
        self.func.blocks[b0].instructions.insert(pos, (vid, Instruction::Store { ptr: ptr.clone(), val }, Type::Void));
    }
}
