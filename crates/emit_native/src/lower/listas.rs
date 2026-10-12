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

#[cfg(test)]
mod testes_arc {
    use super::*;
    use crate::context::Context;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn add_escalar_da_fonte_transporta_ref_em_arc_e_endereco_em_tracing() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entrada = dir.path().join("main.dart");
            std::fs::write(&entrada,
                "void inteiros(List<int> l, int v) { l.add(v); } void reais(List<double> l, double v) { l.add(v); } void booleanos(List<bool> l, bool v) { l.add(v); } void main() { inteiros(<int>[], 1); reais(<double>[], -0.0); booleanos(<bool>[], true); }\n").unwrap();
            let sdk = crate::sdk_modulo::carregar_sdk_nativo(crate::sdk_testes()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, diags_corpos) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&diags_corpos) {
                assert!(!dartforge_types::codes::e_erro_de_compilacao(&d.message), "{d:?}");
            }
            let te = crate::apagamento::calcular(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.memoria_arc = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.erros.is_empty(), "{:?}", m.erros);
                for nome in ["inteiros", "reais", "booleanos"] {
                    let f = m.functions.iter().find(|f| f.name == nome).unwrap();
                    let insts: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).collect();
                    let armazenamento = insts.iter().find(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == if arc {
                            "dartforge_arc_lista_armazenamento_ref_v1"
                        } else { "df.lista_armazenamento" })).unwrap();
                    assert_eq!(armazenamento.2, if arc { Type::Ref } else { Type::I64 });
                    if arc {
                        assert!(!insts.iter().any(|(_, i, _)| matches!(i, Instruction::GravacaoNativa { .. })));
                        let escrita = insts.iter().find(|(_, i, _)| matches!(i,
                            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_gravar_bits_v1")).unwrap();
                        let Instruction::CallRuntime { args, .. } = &escrita.1 else { unreachable!() };
                        assert_eq!(args[0], (Operand::Val(armazenamento.0), Type::Ref));
                        assert_eq!(args[2].1, Type::I64);
                        assert_eq!(args[3], (Operand::Constant(Constant::Int(match nome {
                            "inteiros" => 0x22, "reais" => 0x42, _ => 0x62,
                        })), Type::I64));
                        if nome == "reais" {
                            assert!(insts.iter().any(|(v, i, _)| args[2].0 == Operand::Val(*v)
                                && matches!(i, Instruction::Bitcast { to: Type::I64, .. })));
                        }
                    } else {
                        assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::GravacaoNativa { .. })));
                        assert!(!insts.iter().any(|(_, i, _)| matches!(i,
                            Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_lista_"))));
                    }
                }
            }
        }).unwrap().join().unwrap();
    }
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
        let nome = if self.ctx.memoria_arc {
            match nome {
                "df.lista_armazenamento" => "dartforge_arc_lista_armazenamento_ref_v1",
                "df.lista_capacidade" => "dartforge_arc_lista_capacidade_v1",
                "df.lista_ler" => super::tipados::leitura_ref_de_lista_arc(),
                "df.lista_gravar_ref" => "dartforge_arc_lista_gravar_ref_v1",
                "df.lista_gravar_len" => "dartforge_arc_lista_gravar_len_v1",
                "df.lista_gravar_dados" => "dartforge_arc_lista_gravar_dados_v1",
                _ => nome,
            }
        } else { nome };
        self.emit(Instruction::CallRuntime { name: nome.to_string(), args, ret_ty: ret }, ret)
    }

    /// O comprimento (`@df.lista_len`): o mesmo deslocamento nas três classes.
    pub(super) fn lista_len_em_linha(&mut self, l: Operand) -> Operand {
        self.ajudante_de_lista("df.lista_len", vec![(l, Type::Ref)], Type::I64)
    }

    /// O armazenamento: Ref emprestado em ARC; endereço I64 em tracing,
    /// mantido vivo pela lista e usado sem coleta entre acesso e uso.
    pub(super) fn lista_armazenamento_em_linha(&mut self, l: Operand) -> Operand {
        let ty = if self.ctx.memoria_arc { Type::Ref } else { Type::I64 };
        self.ajudante_de_lista("df.lista_armazenamento", vec![(l, Type::Ref)], ty)
    }

    /// Os bits `FORMA`/`ELEMENTO` do armazenamento `a` (`@df.lista_forma`), em `I64`.
    pub(super) fn lista_forma_em_linha(&mut self, a: Operand) -> Operand {
        if self.ctx.memoria_arc {
            return self.ajudante_de_lista("dartforge_arc_lista_forma_v1", vec![(a, Type::Ref)], Type::I64);
        }
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
            vec![(a.clone(), self.operand_type(&a)), (i.clone(), Type::I64), (v.clone(), Type::Ref)],
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
        let r_rapido = self.ajudante_de_lista("df.lista_ler", vec![(a.clone(), self.operand_type(&a)), (i.clone(), Type::I64)], Type::Ref);
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
        let cap = self.ajudante_de_lista("df.lista_len", vec![(a.clone(), self.operand_type(&a))], Type::I64);
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
        if self.ctx.memoria_arc {
            let bits = if tipo == TipoC::F64 {
                self.emit(Instruction::Bitcast { op: valor, to: Type::I64 }, Type::I64)
            } else { valor };
            self.ajudante_de_lista("dartforge_arc_lista_gravar_bits_v1",
                vec![(a, Type::Ref), (len.clone(), Type::I64), (bits, Type::I64), (c(i64::from(forma)), Type::I64)], Type::Void);
        } else {
            let base = self.emit(Instruction::Add(a, c(layout::desl::ELEMENTOS as i64 - layout::DESLOCAMENTO_DO_HANDLE)), Type::I64);
            self.emit(Instruction::GravacaoNativa { endereco: base, indice: len.clone(), tipo, valor }, Type::Void);
        }
        let novo = self.emit(Instruction::Add(len, c(1)), Type::I64);
        self.ajudante_de_lista("df.lista_gravar_len", vec![(lista, Type::Ref), (novo, Type::I64)], Type::Void);
        let feito = self.current_block;
        (feito, cheio)
    }
}
