//! Os natives de string feitos em linha (P1, docs/NATIVO-ESPACO-UNIFICADO.md
//! §3.5, §3.8 e §4.3): as leituras e gravações que a VM faz como intrínseco.
//!
//! Toda `String` é um bloco `_OneByteString`/`_TwoByteString` do espaço de
//! objetos: o comprimento, as unidades e o hash estão em deslocamentos fixos do
//! handle, e o código gerado os lê pelos ajudantes `@df.texto_*`
//! (`llvm/textos_ir.rs`, `alwaysinline`), sem chamada ao runtime:
//!
//! * `length` (`String_getLength`) → `@df.texto_len`;
//! * `codeUnitAt` → `@df.texto_len` e `@df.texto_unidade` com o teste de
//!   limites; fora da faixa, o native (`DartForge_string_codeUnitAt`), que lança
//!   o `RangeError` da VM;
//! * `allocateOneByteString`/`allocateTwoByteString` → `@df.texto_alocar`;
//! * `writeIntoOneByteString`/`writeIntoTwoByteString` → `@df.texto_gravar`;
//! * `hashCode` (`String_getHashCode`) → `@df.texto_hash`;
//! * `==` (`DartForge_string_igual_a`, `DartForge_string_iguais`) →
//!   `@df.texto_igual_a`/`@df.texto_igual`.
//!
//! O bloco não se move e as unidades de uma string publicada não mudam, então
//! não há cache por ponto de leitura: o LLVM junta e tira dos laços as cargas
//! repetidas.

use super::fn_builder::FnBuilder;
use crate::hir::*;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O membro externo de string feito em linha, ou `None` (a chamada de
    /// sempre). `membro` é `Classe.nome` (ou só o nome, numa função de topo) e
    /// `native` o nome do native, se houver.
    pub(super) fn texto_em_linha(
        &mut self,
        membro: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        match (membro, native, this) {
            ("_StringBase.codeUnitAt", None, Some(s)) if args.len() == 1 && self.e_ref(s) => {
                let i = self.coagir(args[0].clone(), Type::I64);
                Some(self.unidade_em_linha(s.clone(), i))
            }
            (_, Some("String_getLength"), Some(s)) if args.is_empty() && self.e_ref(s) => {
                Some(self.ajudante("df.texto_len", vec![(s.clone(), Type::Ref)], Type::I64))
            }
            (_, Some("String_getHashCode"), Some(s)) if args.is_empty() && self.e_ref(s) => {
                Some(self.ajudante("df.texto_hash", vec![(s.clone(), Type::Ref)], Type::I64))
            }
            (_, Some(n @ ("Internal_allocateOneByteString" | "Internal_allocateTwoByteString")), None) if args.len() == 1 => {
                let len = self.coagir(args[0].clone(), Type::I64);
                let dois = Operand::Constant(Constant::Bool(n == "Internal_allocateTwoByteString"));
                Some(self.ajudante("df.texto_alocar", vec![(len, Type::I64), (dois, Type::I1)], Type::Ref))
            }
            (_, Some("Internal_writeIntoOneByteString" | "Internal_writeIntoTwoByteString"), None) if args.len() == 3 => {
                let s = self.coagir(args[0].clone(), Type::Ref);
                let i = self.coagir(args[1].clone(), Type::I64);
                let u = self.coagir(args[2].clone(), Type::I64);
                self.ajudante("df.texto_gravar", vec![(s, Type::Ref), (i, Type::I64), (u, Type::I64)], Type::Void);
                Some(Operand::Constant(Constant::Null))
            }
            (_, Some(n @ ("DartForge_string_igual_a" | "DartForge_string_iguais")), Some(s)) if args.len() == 1 && self.e_ref(s) => {
                let outro = self.coagir(args[0].clone(), Type::Ref);
                let nome = if n == "DartForge_string_iguais" { "df.texto_igual" } else { "df.texto_igual_a" };
                Some(self.ajudante(nome, vec![(s.clone(), Type::Ref), (outro, Type::Ref)], Type::I1))
            }
            _ => None,
        }
    }

    /// O operando é um `Ref` (o receptor de um native de string sempre é; outra
    /// representação cai na chamada de sempre).
    fn e_ref(&self, op: &Operand) -> bool {
        self.operand_type(op) == Type::Ref
    }

    /// A chamada a um ajudante `@df.texto_*`: não lança (os efeitos estão em
    /// `llvm/textos_ir.rs`, `EFEITOS_DOS_AJUDANTES`), então sem conferência de
    /// exceção.
    fn ajudante(&mut self, nome: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(Instruction::CallRuntime { name: nome.to_string(), args, ret_ty: ret }, ret)
    }

    /// `s.codeUnitAt(i)`: a unidade lida em linha quando `0 <= i < length`; fora
    /// da faixa, o native, que lança.
    fn unidade_em_linha(&mut self, s: Operand, i: Operand) -> Operand {
        let n = self.ajudante("df.texto_len", vec![(s.clone(), Type::Ref)], Type::I64);
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let rapido = self.new_block();
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        let u = self.ajudante("df.texto_unidade", vec![(s.clone(), Type::Ref), (i.clone(), Type::I64)], Type::I64);
        let fim_rapido = self.current_block;
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
        self.emit(Instruction::Phi { incoming: vec![(fim_rapido, u), (fim_lento, r)], ty: Type::I64 }, Type::I64)
    }
}
