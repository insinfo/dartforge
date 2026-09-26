//! O caminho rápido das listas tipadas numéricas (`Int8List` … `Float64List`
//! de `dart:typed_data`) quando o tipo estático do receptor é uma delas.
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

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

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
    /// A lista tipada numérica de `dart:typed_data` do tipo estático `ty`
    /// (não anulável), se for uma.
    pub(super) fn lista_tipada_numerica(&self, ty: Option<TypeId>) -> Option<ListaTipada> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = self.ctx.program.classes.get(class.0 as usize)?;
        if self.ctx.program.library(c.library).uri != "dart:typed_data" {
            return None;
        }
        let (tipo, elemento) = match self.ctx.symbol_name(c.name) {
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
        Some(ListaTipada { tipo, elemento })
    }

    /// `dartforge_typed_len(lista, tipo, escrita)`: o comprimento, ou 0 se
    /// o caminho rápido não serve.
    fn comprimento_tipado(&mut self, lista: &Operand, l: ListaTipada, escrita: bool) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_len".to_string(),
                args: vec![
                    (lista.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(l.tipo)), Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(escrita))), Type::I64),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// `lista.length` de uma lista tipada numérica.
    pub(super) fn length_tipado(&mut self, lista: Operand, l: ListaTipada) -> Operand {
        let lista = self.coagir(lista, Type::Ref);
        self.comprimento_tipado(&lista, l, false)
    }

    /// Desvia para o caminho rápido quando `indice` está nos limites da
    /// lista apta (o `rapido` recebe o endereço dos elementos), senão para
    /// o `lento`; junta os dois resultados na representação `repr`.
    fn desviar_tipado(
        &mut self,
        lista: &Operand,
        indice: &Operand,
        l: ListaTipada,
        escrita: bool,
        repr: Type,
        rapido: &mut dyn FnMut(&mut Self, Operand) -> Operand,
        lento: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        let n = self.comprimento_tipado(lista, l, escrita);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, indice.clone(), n), Type::I1);
        let bloco_rapido = self.new_block();
        let bloco_lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: bloco_rapido, else_block: bloco_lento });

        self.set_block(bloco_rapido);
        let endereco = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_typed_ptr".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let r = rapido(self, endereco);
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

    /// `lista[indice]` de uma lista tipada numérica, no resultado `repr`.
    /// `None` quando o índice não é um `int` sem caixa (fica o despacho).
    pub(super) fn ler_tipado(&mut self, lista: Operand, indice: Operand, l: ListaTipada, repr: Type) -> Option<Operand> {
        if self.operand_type(&indice) != Type::I64 {
            return None;
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2) = (lista.clone(), indice.clone());
        Some(self.desviar_tipado(
            &lista,
            &indice,
            l,
            false,
            repr,
            &mut |s: &mut Self, endereco| {
                s.emit(Instruction::CargaNativa { endereco, indice: indice2.clone(), tipo: l.elemento }, l.repr())
            },
            &mut |s: &mut Self| {
                s.chamar_por_nome(lista2.clone(), super::sdk_fonte::Tipo::Chamar, "[]", &[(None, indice2.clone())])
            },
        ))
    }

    /// `lista[indice] = valor` de uma lista tipada numérica. `false` quando
    /// não há caminho rápido (o chamador faz o despacho).
    pub(super) fn gravar_tipado(&mut self, lista: Operand, indice: Operand, valor: Operand, l: ListaTipada) -> bool {
        if !l.gravacao_direta() || self.operand_type(&indice) != Type::I64 {
            return false;
        }
        let lista = self.coagir(lista, Type::Ref);
        let (lista2, indice2) = (lista.clone(), indice.clone());
        let valor2 = valor.clone();
        self.desviar_tipado(
            &lista,
            &indice,
            l,
            true,
            Type::I64,
            &mut |s: &mut Self, endereco| {
                let v = s.coagir(valor2.clone(), l.repr());
                s.emit(
                    Instruction::GravacaoNativa { endereco, indice: indice2.clone(), tipo: l.elemento, valor: v },
                    Type::Void,
                );
                Operand::Constant(Constant::Int(0))
            },
            &mut |s: &mut Self| {
                s.chamar_por_nome(
                    lista2.clone(),
                    super::sdk_fonte::Tipo::Chamar,
                    "[]=",
                    &[(None, indice2.clone()), (None, valor.clone())],
                );
                Operand::Constant(Constant::Int(0))
            },
        );
        true
    }
}
