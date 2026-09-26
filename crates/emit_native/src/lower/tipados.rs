//! O caminho rápido de `[]`, `[]=` e `length` quando o tipo estático do
//! receptor é uma lista tipada numérica (`Int8List` … `Float64List` de
//! `dart:typed_data`) ou uma `List<E>` do núcleo.
//!
//! Essas classes são `final` no SDK: o valor é sempre a lista interna
//! (`_Int32List`) ou uma visão (`_Int32ArrayView`, e a não modificável) do
//! `typed_data_patch.dart`. Então `a[i]`, `a[i] = v` e `a.length` não
//! precisam do despacho pela classe dinâmica: o comprimento e o endereço
//! dos elementos vêm de duas funções do runtime que só leem o heap dele
//! (`dartforge_typed_len`/`dartforge_typed_ptr`, funções puras do handle,
//! declaradas `memory(none) speculatable`, que o LLVM tira dos laços),
//! e o elemento é lido ou gravado direto. Um índice fora dos limites, uma
//! visão não modificável na escrita ou um tipo de elemento inesperado caem
//! no despacho de sempre, com os mesmos erros da VM.
//!
//! `List` não é `final`: uma classe do usuário pode implementá-la. O runtime
//! responde o comprimento só das listas dele (`_List`, `_GrowableList`,
//! `_ImmutableList`; na escrita, só das modificáveis) e 0 para as outras,
//! que ficam com o despacho. Os elementos saem sem caixa na representação
//! do resultado (`int`, `double`); gravar direto só com `E` igual a `int`,
//! `double` ou `bool`, que nenhuma classe estende — com outro `E`, a lista
//! pode ser de um subtipo e o `[]=` do SDK confere o valor (covariância).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

/// Um receptor com caminho rápido de índice.
#[derive(Debug, Clone, Copy)]
pub enum Indexavel {
    Tipada(ListaTipada),
    /// `List<E>`: a representação do elemento que se grava direto (`E`
    /// igual a `int`, `double` ou `bool`), ou `None` (só leitura direta).
    Nucleo { gravacao: Option<Type> },
}

/// Uma lista tipada numérica: o tipo do elemento no runtime (`TIPO_*` de
/// `runtime/src/typed_data.rs`) e o tipo C do elemento.
#[derive(Debug, Clone, Copy)]
pub struct ListaTipada {
    pub tipo: i64,
    pub elemento: TipoC,
}

impl ListaTipada {
    /// A representação Dart do elemento.
    fn repr(self) -> Type {
        if matches!(self.elemento, TipoC::F32 | TipoC::F64) { Type::F64 } else { Type::I64 }
    }

    /// `[]=` pode gravar direto? A `Uint8ClampedList` satura o valor; ela
    /// fica com o `[]=` do SDK.
    fn gravacao_direta(self) -> bool {
        self.tipo != 2
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O caminho rápido de índice do tipo estático `ty` (não anulável), se
    /// houver.
    pub(super) fn indexavel(&self, ty: Option<TypeId>) -> Option<Indexavel> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        let T::Interface { class, args, nullable: false } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = self.ctx.program.classes.get(class.0 as usize)?;
        let biblioteca = self.ctx.program.library(c.library).uri.as_str();
        let nome = self.ctx.symbol_name(c.name);
        if biblioteca == "dart:core" && nome == "List" {
            let gravacao = match args.first().map(|&e| self.ctx.table.get(e)) {
                Some(T::Interface { class: e, nullable: false, .. }) => {
                    let e = self.ctx.program.classes.get(e.0 as usize)?;
                    let nucleo = self.ctx.program.library(e.library).uri == "dart:core";
                    match self.ctx.symbol_name(e.name) {
                        "int" if nucleo => Some(Type::I64),
                        "double" if nucleo => Some(Type::F64),
                        "bool" if nucleo => Some(Type::I1),
                        _ => None,
                    }
                }
                _ => None,
            };
            return Some(Indexavel::Nucleo { gravacao });
        }
        if biblioteca != "dart:typed_data" {
            return None;
        }
        let (tipo, elemento) = match nome {
            "Int8List" => (0, TipoC::I8),
            "Uint8List" => (1, TipoC::U8),
            "Uint8ClampedList" => (2, TipoC::U8),
            "Int16List" => (3, TipoC::I16),
            "Uint16List" => (4, TipoC::U16),
            "Int32List" => (5, TipoC::I32),
            "Uint32List" => (6, TipoC::U32),
            "Int64List" => (7, TipoC::I64),
            "Uint64List" => (8, TipoC::U64),
            "Float32List" => (9, TipoC::F32),
            "Float64List" => (10, TipoC::F64),
            _ => return None,
        };
        Some(Indexavel::Tipada(ListaTipada { tipo, elemento }))
    }

    /// O comprimento para o caminho rápido, ou 0 se ele não serve
    /// (`dartforge_typed_len`/`dartforge_lista_len_rapido`).
    fn comprimento_rapido(&mut self, lista: &Operand, ix: Indexavel, escrita: bool) -> Operand {
        let escrita = (Operand::Constant(Constant::Int(i64::from(escrita))), Type::I64);
        let (name, args) = match ix {
            Indexavel::Tipada(l) => (
                "dartforge_typed_len",
                vec![(lista.clone(), Type::Ref), (Operand::Constant(Constant::Int(l.tipo)), Type::I64), escrita],
            ),
            Indexavel::Nucleo { .. } => ("dartforge_lista_len_rapido", vec![(lista.clone(), Type::Ref), escrita]),
        };
        self.emit(Instruction::CallRuntime { name: name.to_string(), args, ret_ty: Type::I64 }, Type::I64)
    }

    /// `lista.length`.
    pub(super) fn length_indexado(&mut self, lista: Operand, ix: Indexavel) -> Operand {
        let lista = self.coagir(lista, Type::Ref);
        let n = self.comprimento_rapido(&lista, ix, false);
        if let Indexavel::Tipada(_) = ix {
            // O tipo estático garante a lista tipada do tipo: o comprimento
            // é esse.
            return n;
        }
        // 0: vazia, ou não é lista do runtime — o getter responde.
        let vazia = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
        let bloco_rapido = self.new_block();
        let bloco_lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: vazia, then_block: bloco_lento, else_block: bloco_rapido });
        self.set_block(bloco_rapido);
        self.terminate(Terminator::Branch(juncao));
        self.set_block(bloco_lento);
        let s = self.chamar_por_nome(lista, super::sdk_fonte::Tipo::Ler, "length", &[]);
        let s = self.coagir(s, Type::I64);
        let fim_lento = self.current_block;
        if self.is_terminated() {
            self.set_block(juncao);
            return n;
        }
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        self.emit(Instruction::Phi { incoming: vec![(bloco_rapido, n), (fim_lento, s)], ty: Type::I64 }, Type::I64)
    }

    /// Desvia para o caminho rápido quando `indice` está nos limites da
    /// lista apta, senão para o `lento`; junta os dois resultados na
    /// representação `repr`.
    fn desviar_indexado(
        &mut self,
        lista: &Operand,
        indice: &Operand,
        ix: Indexavel,
        escrita: bool,
        repr: Type,
        rapido: &mut dyn FnMut(&mut Self) -> Operand,
        lento: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        let n = self.comprimento_rapido(lista, ix, escrita);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, indice.clone(), n), Type::I1);
        let bloco_rapido = self.new_block();
        let bloco_lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: bloco_rapido, else_block: bloco_lento });

        self.set_block(bloco_rapido);
        let r = rapido(self);
        let r = self.coagir(r, repr);
        let fim_rapido = self.current_block;
        self.terminate(Terminator::Branch(juncao));

        self.set_block(bloco_lento);
        let s = lento(self);
        let s = self.coagir(s, repr);
        let fim_lento = self.current_block;
        let lento_chega = !self.is_terminated();
        if lento_chega {
            self.terminate(Terminator::Branch(juncao));
        }

        self.set_block(juncao);
        if lento_chega {
            self.emit(Instruction::Phi { incoming: vec![(fim_rapido, r), (fim_lento, s)], ty: repr }, repr)
        } else {
            r
        }
    }

    /// O endereço dos elementos de uma lista tipada apta.
    fn enderecos_tipados(&mut self, lista: &Operand) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_ptr".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// `lista[indice]`, no resultado `repr`. `None` quando o índice não é
    /// um `int` sem caixa (fica o despacho).
    pub(super) fn ler_indexado(&mut self, lista: Operand, indice: Operand, ix: Indexavel, repr: Type) -> Option<Operand> {
        if self.operand_type(&indice) != Type::I64 {
            return None;
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2) = (lista.clone(), indice.clone());
        let mut rapido = |s: &mut Self| match ix {
            Indexavel::Tipada(l) => {
                let endereco = s.enderecos_tipados(&lista2);
                s.emit(Instruction::CargaNativa { endereco, indice: indice2.clone(), tipo: l.elemento }, l.repr())
            }
            Indexavel::Nucleo { .. } => {
                let (name, ret) = match repr {
                    Type::I64 => ("dartforge_lista_int", Type::I64),
                    Type::F64 => ("dartforge_lista_double", Type::F64),
                    _ => ("dartforge_lista_ref", Type::Ref),
                };
                s.emit(
                    Instruction::CallRuntime {
                        name: name.to_string(),
                        args: vec![(lista2.clone(), Type::Ref), (indice2.clone(), Type::I64)],
                        ret_ty: ret,
                    },
                    ret,
                )
            }
        };
        let mut lento = |s: &mut Self| {
            s.chamar_por_nome(lista2.clone(), super::sdk_fonte::Tipo::Chamar, "[]", &[(None, indice2.clone())])
        };
        Some(self.desviar_indexado(&lista, &indice, ix, false, repr, &mut rapido, &mut lento))
    }

    /// `lista[indice] = valor`. `false` quando não há caminho rápido (o
    /// chamador faz o despacho).
    pub(super) fn gravar_indexado(&mut self, lista: Operand, indice: Operand, valor: Operand, ix: Indexavel) -> bool {
        let direta = match ix {
            Indexavel::Tipada(l) => l.gravacao_direta(),
            Indexavel::Nucleo { gravacao } => gravacao.is_some(),
        };
        if !direta || self.operand_type(&indice) != Type::I64 {
            return false;
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2, valor2) = (lista.clone(), indice.clone(), valor.clone());
        let mut rapido = |s: &mut Self| {
            match ix {
                Indexavel::Tipada(l) => {
                    let endereco = s.enderecos_tipados(&lista2);
                    let v = s.coagir(valor2.clone(), l.repr());
                    s.emit(
                        Instruction::GravacaoNativa { endereco, indice: indice2.clone(), tipo: l.elemento, valor: v },
                        Type::Void,
                    );
                }
                Indexavel::Nucleo { gravacao: Some(t) } => {
                    let v = s.coagir(valor2.clone(), t);
                    let (name, tv) = match t {
                        Type::F64 => ("dartforge_lista_gravar_double", Type::F64),
                        Type::I1 => ("dartforge_lista_gravar_bool", Type::I8),
                        _ => ("dartforge_lista_gravar_int", Type::I64),
                    };
                    let v = if tv == Type::I8 { s.coagir(v, Type::I8) } else { v };
                    s.emit(
                        Instruction::CallRuntime {
                            name: name.to_string(),
                            args: vec![(lista2.clone(), Type::Ref), (indice2.clone(), Type::I64), (v, tv)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    );
                }
                Indexavel::Nucleo { gravacao: None } => unreachable!("gravação direta conferida acima"),
            }
            Operand::Constant(Constant::Int(0))
        };
        let mut lento = |s: &mut Self| {
            s.chamar_por_nome(
                lista2.clone(),
                super::sdk_fonte::Tipo::Chamar,
                "[]=",
                &[(None, indice2.clone()), (None, valor.clone())],
            );
            Operand::Constant(Constant::Int(0))
        };
        self.desviar_indexado(&lista, &indice, ix, true, Type::I64, &mut rapido, &mut lento);
        true
    }
}
