//! Os membros externos de `_List`/`_ImmutableList`/`_GrowableList` feitos em linha
//! (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.8 e §4.5): comprimento, capacidade,
//! `_setLength` sem barreira, `_setData` e `_setIndexed` com barreira,
//! `DartForge_lista_get`, pelos ajudantes `@df.lista_*` (`llvm/ir_lists.rs`),
//! como os intrínsecos de grafo da VM (`graph_intrinsifier.cc:523-626`,
//! `kernel_to_il.cc:1191-1235`).
//!
//! O receptor de cada membro é, pela classe que o declara, uma lista do núcleo
//! (nenhuma delas tem subclasse fora do SDK): `_Array.[]`/`List_getLength` recebem
//! `_List` ou `_ImmutableList`; os `GrowableList_*`, a `_GrowableList`. Por isso o
//! código não confere a classe: lê o bloco pelos deslocamentos de
//! `layout::off` (o comprimento em `h+14` nas três, o armazenamento em `h+22` na
//! expansível).
//!
//! Os ajudantes são chamados por `CallRuntime` com os efeitos de
//! `ir_lists::HELPER_EFFECTS` (nenhum aloca nem lança): não são pontos de
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
mod arc_tests {
    use super::*;
    use crate::context::Context;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn source_scalar_add_carries_ref_in_arc_and_address_in_tracing() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry,
                "void inteiros(List<int> l, int v) { l.add(v); } void reais(List<double> l, double v) { l.add(v); } void booleanos(List<bool> l, bool v) { l.add(v); } void main() { inteiros(<int>[], 1); reais(<double>[], -0.0); booleanos(<bool>[], true); }\n").unwrap();
            let sdk = crate::sdk_modulo::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, diags_corpos) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&diags_corpos) {
                assert!(!dartforge_types::codes::e_erro_de_compilacao(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                for name_str in ["inteiros", "reais", "booleanos"] {
                    let f = m.functions.iter().find(|f| f.name == name_str).unwrap();
                    let insts: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).collect();
                    let storage = insts.iter().find(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == if arc {
                            "dartforge_arc_lista_armazenamento_ref_v1"
                        } else { "df.lista_armazenamento" })).unwrap();
                    assert_eq!(storage.2, if arc { Type::Ref } else { Type::I64 });
                    if arc {
                        assert!(!insts.iter().any(|(_, i, _)| matches!(i, Instruction::NativeWrite { .. })));
                        let write = insts.iter().find(|(_, i, _)| matches!(i,
                            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_gravar_bits_v1")).unwrap();
                        let Instruction::CallRuntime { args, .. } = &write.1 else { unreachable!() };
                        assert_eq!(args[0], (Operand::Val(storage.0), Type::Ref));
                        assert_eq!(args[2].1, Type::I64);
                        assert_eq!(args[3], (Operand::Constant(Constant::Int(match name_str {
                            "inteiros" => 0x22, "reais" => 0x42, _ => 0x62,
                        })), Type::I64));
                        if name_str == "reais" {
                            assert!(insts.iter().any(|(v, i, _)| args[2].0 == Operand::Val(*v)
                                && matches!(i, Instruction::Bitcast { to: Type::I64, .. })));
                        }
                    } else {
                        assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::NativeWrite { .. })));
                        assert!(!insts.iter().any(|(_, i, _)| matches!(i,
                            Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_lista_"))));
                    }
                }
            }
        }).unwrap().join().unwrap();
    }
}

/// Os bits `FORMA`/`ELEMENTO` (`@df.lista_forma`) do armazenamento geral.
const SHAPE_GENERAL: i64 = flags::REFS as i64;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O membro externo de lista do núcleo feito em linha, ou `None` (a chamada
    /// de sempre). `member` é `Classe.name`; `native`, o nome do native, se tem.
    pub(super) fn inline_list(
        &mut self,
        member: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let this = this?.clone();
        if self.operand_type(&this) != Type::Ref {
            return None;
        }
        match (member, native, args.len()) {
            (_, Some("List_getLength" | "GrowableList_getLength"), 0) => Some(self.list_len_inline(this)),
            (_, Some("GrowableList_getCapacity"), 0) => Some(self.list_helper("df.lista_capacidade", vec![(this, Type::Ref)], Type::I64)),
            (_, Some("GrowableList_setLength"), 1) => {
                let n = self.coerce(args[0].clone(), Type::I64);
                self.list_helper("df.lista_gravar_len", vec![(this, Type::Ref), (n, Type::I64)], Type::Void);
                Some(Operand::Constant(Constant::Null))
            }
            (_, Some("GrowableList_setData"), 1) => {
                let data = self.coerce(args[0].clone(), Type::Ref);
                Some(self.define_inline_data(this, data))
            }
            (_, Some(n @ ("List_setIndexed" | "GrowableList_setIndexed")), 2) => {
                let i = self.coerce(args[0].clone(), Type::I64);
                let v = self.coerce(args[1].clone(), Type::Ref);
                Some(self.store_index_inline(n, this, i, v))
            }
            ("_Array.[]" | "_List.[]" | "_ImmutableList.[]" | "_GrowableList.[]", None, 1) => {
                let i = self.coerce(args[0].clone(), Type::I64);
                Some(self.read_index_inline(this, i))
            }
            _ => None,
        }
    }

    /// Um ajudante `@df.lista_*` (sem coleta nem exceção).
    fn list_helper(&mut self, name_str: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        let name_str = if self.ctx.arc_memory {
            match name_str {
                "df.lista_armazenamento" => "dartforge_arc_lista_armazenamento_ref_v1",
                "df.lista_capacidade" => "dartforge_arc_lista_capacidade_v1",
                "df.lista_ler" => super::typed_items::arc_list_ref_read(),
                "df.lista_gravar_ref" => "dartforge_arc_lista_gravar_ref_v1",
                "df.lista_gravar_len" => "dartforge_arc_lista_gravar_len_v1",
                "df.lista_gravar_dados" => "dartforge_arc_lista_gravar_dados_v1",
                _ => name_str,
            }
        } else { name_str };
        self.emit(Instruction::CallRuntime { name: name_str.to_string(), args, ret_ty: ret }, ret)
    }

    /// O comprimento (`@df.lista_len`): o mesmo deslocamento nas três classes.
    pub(super) fn list_len_inline(&mut self, l: Operand) -> Operand {
        self.list_helper("df.lista_len", vec![(l, Type::Ref)], Type::I64)
    }

    /// O armazenamento: Ref emprestado em ARC; endereço I64 em tracing,
    /// mantido vivo pela lista e usado sem coleta entre acesso e uso.
    pub(super) fn list_storage_inline(&mut self, l: Operand) -> Operand {
        let ty = if self.ctx.arc_memory { Type::Ref } else { Type::I64 };
        self.list_helper("df.lista_armazenamento", vec![(l, Type::Ref)], ty)
    }

    /// Os bits `FORMA`/`ELEMENTO` do armazenamento `a` (`@df.lista_forma`), em `I64`.
    pub(super) fn list_shape_inline(&mut self, a: Operand) -> Operand {
        if self.ctx.arc_memory {
            return self.list_helper("dartforge_arc_lista_forma_v1", vec![(a, Type::Ref)], Type::I64);
        }
        let f = self.list_helper("df.lista_forma", vec![(a, Type::I64)], Type::I8);
        self.emit(Instruction::ZExt { op: f, from: Type::I8, to: Type::I64 }, Type::I64)
    }

    /// `this._setData(data)`: a gravação do campo com barreira quando o
    /// armazenamento novo tem a forma do velho (`@df.lista_gravar_dados`
    /// devolve 1); senão o native, que devolve a forma compacta ao novo (o
    /// `_shrink` da VM monta os dados sem tipo).
    fn define_inline_data(&mut self, this: Operand, data: Operand) -> Operand {
        let done = self.list_helper(
            "df.lista_gravar_dados",
            vec![(this.clone(), Type::Ref), (data.clone(), Type::Ref)],
            Type::I64,
        );
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, done, c(0)), Type::I1);
        let slow = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: end, else_block: slow });
        self.set_block(slow);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol("GrowableList_setData"),
                args: vec![(this, Type::Ref), (data, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(end));
        }
        self.set_block(end);
        Operand::Constant(Constant::Null)
    }

    /// `this._setIndexed(i, v)`: dentro da faixa e com o armazenamento geral,
    /// a gravação com a barreira de elemento (`@df.lista_gravar_ref`); fora da
    /// faixa (o `RangeError (index)`) ou compacto (o valor sem caixa, ou a
    /// descompactação), o native.
    fn store_index_inline(&mut self, native: &str, this: Operand, i: Operand, v: Operand) -> Operand {
        let n = self.list_len_inline(this.clone());
        let inside = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let b_shape = self.new_block();
        let fast = self.new_block();
        let slow = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch { cond: inside, then_block: b_shape, else_block: slow });

        self.set_block(b_shape);
        let a = self.list_storage_inline(this.clone());
        let f = self.list_shape_inline(a.clone());
        let general = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(SHAPE_GENERAL)), Type::I1);
        self.terminate(Terminator::CondBranch { cond: general, then_block: fast, else_block: slow });

        self.set_block(fast);
        self.list_helper(
            "df.lista_gravar_ref",
            vec![(a.clone(), self.operand_type(&a)), (i.clone(), Type::I64), (v.clone(), Type::Ref)],
            Type::Void,
        );
        self.terminate(Terminator::Branch(end));

        self.set_block(slow);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(native),
                args: vec![(this, Type::Ref), (i, Type::I64), (v, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(end));
        }
        self.set_block(end);
        Operand::Constant(Constant::Null)
    }

    /// `this[i]` (`DartForge_lista_get`): dentro da faixa e com o armazenamento
    /// geral, a palavra lida (`@df.lista_ler`, um `Ref`); fora da faixa (o
    /// `RangeError (length)`) ou compacto (a caixa do elemento), o native.
    fn read_index_inline(&mut self, this: Operand, i: Operand) -> Operand {
        let n = self.list_len_inline(this.clone());
        let inside = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let b_shape = self.new_block();
        let fast = self.new_block();
        let slow = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch { cond: inside, then_block: b_shape, else_block: slow });

        self.set_block(b_shape);
        let a = self.list_storage_inline(this.clone());
        let f = self.list_shape_inline(a.clone());
        let general = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(SHAPE_GENERAL)), Type::I1);
        self.terminate(Terminator::CondBranch { cond: general, then_block: fast, else_block: slow });

        self.set_block(fast);
        let r_fast = self.list_helper("df.lista_ler", vec![(a.clone(), self.operand_type(&a)), (i.clone(), Type::I64)], Type::Ref);
        self.terminate(Terminator::Branch(end));

        self.set_block(slow);
        let r_slow = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol("DartForge_lista_get"),
                args: vec![(this, Type::Ref), (i, Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let slow_end = self.current_block;
        if self.is_terminated() {
            self.set_block(end);
            return r_fast;
        }
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
        self.emit(Instruction::Phi { incoming: vec![(fast, r_fast), (slow_end, r_slow)], ty: Type::Ref }, Type::Ref)
    }

    /// O `add` de um escalar numa `_GrowableList` cujo armazenamento é compacto
    /// da forma `shape` (`flags::ELEMENTO_*` com `BRUTO`) e tem lugar: as
    /// palavras gravadas direto (`bits`, na representação `ty`) e o
    /// comprimento mais um. Devolve `(done, full)`: os blocos onde a gravação
    /// terminou e onde a lista estava cheia (a forma casava). Quem chama já
    /// conferiu a classe (cid 10); o caminho de cada bloco que sobra é dele.
    pub(super) fn append_scalar_inline(
        &mut self,
        list: Operand,
        value: Operand,
        ty: CType,
        shape: u8,
        does_not_match: BlockId,
    ) -> (BlockId, BlockId) {
        let len = self.list_len_inline(list.clone());
        let a = self.list_storage_inline(list.clone());
        let cap = self.list_helper("df.lista_len", vec![(a.clone(), self.operand_type(&a))], Type::I64);
        let f = self.list_shape_inline(a.clone());
        let matches = self.emit(Instruction::ICmp(ICmpOp::Eq, f, c(i64::from(shape))), Type::I1);
        let b_matches = self.new_block();
        let b_write = self.new_block();
        let full = self.new_block();
        self.terminate(Terminator::CondBranch { cond: matches, then_block: b_matches, else_block: does_not_match });

        self.set_block(b_matches);
        let has = self.emit(Instruction::ICmp(ICmpOp::Ult, len.clone(), cap), Type::I1);
        self.terminate(Terminator::CondBranch { cond: has, then_block: b_write, else_block: full });

        self.set_block(b_write);
        if self.ctx.arc_memory {
            let bits = if ty == CType::F64 {
                self.emit(Instruction::Bitcast { op: value, to: Type::I64 }, Type::I64)
            } else { value };
            self.list_helper("dartforge_arc_lista_gravar_bits_v1",
                vec![(a, Type::Ref), (len.clone(), Type::I64), (bits, Type::I64), (c(i64::from(shape)), Type::I64)], Type::Void);
        } else {
            let base = self.emit(Instruction::Add(a, c(layout::desl::ELEMENTOS as i64 - layout::DESLOCAMENTO_DO_HANDLE)), Type::I64);
            self.emit(Instruction::NativeWrite { address: base, index: len.clone(), ty, value }, Type::Void);
        }
        let new = self.emit(Instruction::Add(len, c(1)), Type::I64);
        self.list_helper("df.lista_gravar_len", vec![(list, Type::Ref), (new, Type::I64)], Type::Void);
        let done = self.current_block;
        (done, full)
    }
}
