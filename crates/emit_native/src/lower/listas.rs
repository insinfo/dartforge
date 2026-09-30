//! Os membros externos de `_List`/`_ImmutableList`/`_GrowableList` feitos em linha
//! (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.8 e §4.5): comprimento, capacidade,
//! `_setLength` sem barreira, `_setData` e `_setIndexed` com barreira,
//! `DartForge_lista_get`, pelos ajudantes `@df.lista_*` (`llvm/listas_ir.rs`),
//! como os intrínsecos de grafo da VM (`graph_intrinsifier.cc:523-626`,
//! `kernel_to_il.cc:1191-1235`).
//!
//! O receptor de cada membro é, pela classe que o declara, uma lista do núcleo
//! (nenhuma delas tem subclasse fora do SDK): `_Array.[]`/`List_getLength` recebem
//! `_List` ou `_ImmutableList`; os `GrowableList_*`, a `_GrowableList`. Por isso o
//! código não confere a classe: lê o bloco pelos deslocamentos de
//! `layout::desl` (o comprimento em `h+14` nas três, o armazenamento em `h+22` na
//! expansível).
//!
//! Os ajudantes são chamados por `CallRuntime` com os efeitos de
//! `listas_ir::EFEITOS_DOS_AJUDANTES` (nenhum aloca nem lança): não são pontos de
//! coleta nem pedem conferência de exceção. O que o em linha não cobre (índice
//! fora da faixa, armazenamento compacto, `_setData` que troca a forma) vai ao
//! native de sempre, que lança o erro da VM ou converte a forma.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_runtime::layout::{self, flags};

fn c(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

/// Os bits `FORMA`/`ELEMENTO` (`@df.lista_forma`) do armazenamento geral.
const FORMA_GERAL: i64 = flags::REFS as i64;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O membro externo de lista do núcleo feito em linha, ou `None` (a chamada
    /// de sempre). `membro` é `Classe.nome`; `native`, o nome do native, se tem.
    pub(super) fn lista_em_linha(
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
        match (membro, native, args.len()) {
            (_, Some("List_getLength" | "GrowableList_getLength"), 0) => Some(self.lista_len_em_linha(this)),
            (_, Some("GrowableList_getCapacity"), 0) => Some(self.ajudante_de_lista("df.lista_capacidade", vec![(this, Type::Ref)], Type::I64)),
            (_, Some("GrowableList_setLength"), 1) => {
                let n = self.coagir(args[0].clone(), Type::I64);
                self.ajudante_de_lista("df.lista_gravar_len", vec![(this, Type::Ref), (n, Type::I64)], Type::Void);
                Some(Operand::Constant(Constant::Null))
            }
            (_, Some("GrowableList_setData"), 1) => {
                let dados = self.coagir(args[0].clone(), Type::Ref);
                Some(self.definir_dados_em_linha(this, dados))
            }
            (_, Some(n @ ("List_setIndexed" | "GrowableList_setIndexed")), 2) => {
                let i = self.coagir(args[0].clone(), Type::I64);
                let v = self.coagir(args[1].clone(), Type::Ref);
                Some(self.gravar_indice_em_linha(n, this, i, v))
            }
            ("_Array.[]" | "_List.[]" | "_ImmutableList.[]" | "_GrowableList.[]", None, 1) => {
                let i = self.coagir(args[0].clone(), Type::I64);
                Some(self.ler_indice_em_linha(this, i))
            }
            _ => None,
        }
    }

    /// Um ajudante `@df.lista_*` (sem coleta nem exceção).
    fn ajudante_de_lista(&mut self, nome: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(Instruction::CallRuntime { name: nome.to_string(), args, ret_ty: ret }, ret)
    }

    /// O comprimento (`@df.lista_len`): o mesmo deslocamento nas três classes.
    pub(super) fn lista_len_em_linha(&mut self, l: Operand) -> Operand {
        self.ajudante_de_lista("df.lista_len", vec![(l, Type::Ref)], Type::I64)
    }

    /// O armazenamento (`@df.lista_armazenamento`), como endereço (`I64`): só
    /// para ler e gravar em seguida, sem ponto de coleta no meio (a lista, viva,
    /// o mantém).
    pub(super) fn lista_armazenamento_em_linha(&mut self, l: Operand) -> Operand {
        self.ajudante_de_lista("df.lista_armazenamento", vec![(l, Type::Ref)], Type::I64)
    }

    /// Os bits `FORMA`/`ELEMENTO` do armazenamento `a` (`@df.lista_forma`), em `I64`.
    pub(super) fn lista_forma_em_linha(&mut self, a: Operand) -> Operand {
        let f = self.ajudante_de_lista("df.lista_forma", vec![(a, Type::I64)], Type::I8);
        self.emit(Instruction::ZExt { op: f, from: Type::I8, to: Type::I64 }, Type::I64)
    }

    /// `this._setData(dados)`: a gravação do campo com barreira quando o
    /// armazenamento novo tem a forma do velho (`@df.lista_gravar_dados`
    /// devolve 1); senão o native, que devolve a forma compacta ao novo (o
    /// `_shrink` da VM monta os dados sem tipo).
    fn definir_dados_em_linha(&mut self, this: Operand, dados: Operand) -> Operand {
        let feito = self.ajudante_de_lista(
            "df.lista_gravar_dados",
            vec![(this.clone(), Type::Ref), (dados.clone(), Type::Ref)],
            Type::I64,
        );
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, feito, c(0)), Type::I1);
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fim, else_block: lento });
        self.set_block(lento);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo("GrowableList_setData"),
                args: vec![(this, Type::Ref), (dados, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(fim));
        }
        self.set_block(fim);
        Operand::Constant(Constant::Null)
    }

    /// `this._setIndexed(i, v)`: dentro da faixa e com o armazenamento geral,
    /// a gravação com a barreira de elemento (`@df.lista_gravar_ref`); fora da
    /// faixa (o `RangeError (index)`) ou compacto (o valor sem caixa, ou a
    /// descompactação), o native.
    fn gravar_indice_em_linha(&mut self, native: &str, this: Operand, i: Operand, v: Operand) -> Operand {
        let n = self.lista_len_em_linha(this.clone());
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let b_forma = self.new_block();
        let rapido = self.new_block();
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: b_forma, else_block: lento });

        self.set_block(b_forma);
        let a = self.lista_armazenamento_em_linha(this.clone());
        let f = self.lista_forma_em_linha(a.clone());
        let geral = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(FORMA_GERAL)), Type::I1);
        self.terminate(Terminator::CondBranch { cond: geral, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        self.ajudante_de_lista(
            "df.lista_gravar_ref",
            vec![(a, Type::I64), (i.clone(), Type::I64), (v.clone(), Type::Ref)],
            Type::Void,
        );
        self.terminate(Terminator::Branch(fim));

        self.set_block(lento);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo(native),
                args: vec![(this, Type::Ref), (i, Type::I64), (v, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(fim));
        }
        self.set_block(fim);
        Operand::Constant(Constant::Null)
    }

    /// `this[i]` (`DartForge_lista_get`): dentro da faixa e com o armazenamento
    /// geral, a palavra lida (`@df.lista_ler`, um `Ref`); fora da faixa (o
    /// `RangeError (length)`) ou compacto (a caixa do elemento), o native.
    fn ler_indice_em_linha(&mut self, this: Operand, i: Operand) -> Operand {
        let n = self.lista_len_em_linha(this.clone());
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let b_forma = self.new_block();
        let rapido = self.new_block();
        let lento = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: b_forma, else_block: lento });

        self.set_block(b_forma);
        let a = self.lista_armazenamento_em_linha(this.clone());
        let f = self.lista_forma_em_linha(a.clone());
        let geral = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(FORMA_GERAL)), Type::I1);
        self.terminate(Terminator::CondBranch { cond: geral, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        let r_rapido = self.ajudante_de_lista("df.lista_ler", vec![(a, Type::I64), (i.clone(), Type::I64)], Type::Ref);
        self.terminate(Terminator::Branch(fim));

        self.set_block(lento);
        let r_lento = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::nativos::simbolo("DartForge_lista_get"),
                args: vec![(this, Type::Ref), (i, Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let fim_lento = self.current_block;
        if self.is_terminated() {
            self.set_block(fim);
            return r_rapido;
        }
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
        self.emit(Instruction::Phi { incoming: vec![(rapido, r_rapido), (fim_lento, r_lento)], ty: Type::Ref }, Type::Ref)
    }

    /// O `add` de um escalar numa `_GrowableList` cujo armazenamento é compacto
    /// da forma `forma` (`flags::ELEMENTO_*` com `BRUTO`) e tem lugar: as
    /// palavras gravadas direto (`bits`, na representação `tipo`) e o
    /// comprimento mais um. Devolve `(feito, cheio)`: os blocos onde a gravação
    /// terminou e onde a lista estava cheia (a forma casava). Quem chama já
    /// conferiu a classe (cid 10); o caminho de cada bloco que sobra é dele.
    pub(super) fn acrescentar_escalar_em_linha(
        &mut self,
        lista: Operand,
        valor: Operand,
        tipo: TipoC,
        forma: u8,
        nao_casa: BlockId,
    ) -> (BlockId, BlockId) {
        let len = self.lista_len_em_linha(lista.clone());
        let a = self.lista_armazenamento_em_linha(lista.clone());
        let cap = self.ajudante_de_lista("df.lista_len", vec![(a.clone(), Type::I64)], Type::I64);
        let f = self.lista_forma_em_linha(a.clone());
        let casa = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(i64::from(forma))), Type::I1);
        let b_casa = self.new_block();
        let b_grava = self.new_block();
        let cheio = self.new_block();
        self.terminate(Terminator::CondBranch { cond: casa, then_block: b_casa, else_block: nao_casa });

        self.set_block(b_casa);
        let tem = self.emit(Instruction::ICmp(ICmpOp::Ult, len.clone(), cap), Type::I1);
        self.terminate(Terminator::CondBranch { cond: tem, then_block: b_grava, else_block: cheio });

        self.set_block(b_grava);
        let base = self.emit(Instruction::Add(a, c(layout::desl::ELEMENTOS as i64 - layout::DESLOCAMENTO_DO_HANDLE)), Type::I64);
        self.emit(Instruction::GravacaoNativa { endereco: base, indice: len.clone(), tipo, valor }, Type::Void);
        let novo = self.emit(Instruction::Add(len, c(1)), Type::I64);
        self.ajudante_de_lista("df.lista_gravar_len", vec![(lista, Type::Ref), (novo, Type::I64)], Type::Void);
        let feito = self.current_block;
        (feito, cheio)
    }
}
