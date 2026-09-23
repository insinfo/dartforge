//! Variáveis locais: escopo léxico e `alloca` na representação do tipo (R6).
//!
//! Antes, um local era um `alloca i64` criado onde a declaração aparecia (no
//! meio de um laço, o `alloca` crescia a pilha a cada volta) e um mapa único
//! por nome: um bloco que sombreava `current` sobrescrevia o `current` de
//! fora para sempre, e a variável de um `for (var i = 0; …; i++)` nem tinha
//! `alloca` — o `i++` trocava o valor SSA no mapa, e a condição do laço,
//! baixada antes, continuava lendo o `0` da entrada.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Um local visível: em memória (`ptr`) ou um valor imutável já calculado
/// (`valor`, o parâmetro de uma closure baixada em linha).
#[derive(Debug, Clone)]
pub struct Local {
    pub ptr: Option<Operand>,
    pub valor: Option<Operand>,
    pub ty: Type,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn abrir_escopo(&mut self) {
        self.escopos.push(HashMap::new());
    }

    pub fn fechar_escopo(&mut self) {
        if self.escopos.len() > 1 {
            self.escopos.pop();
        }
    }

    /// `alloca` no começo do bloco de entrada: executa uma vez por
    /// ativação e domina todos os usos (o Clang recusava `alloca` criado
    /// num ramo e usado depois da junção).
    fn alloca_na_entrada(&mut self, ty: Type) -> Operand {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, Type::Ptr);
        let b0 = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == BlockId(0))
            .expect("bloco de entrada");
        self.func.blocks[b0]
            .instructions
            .insert(self.n_allocas, (vid, Instruction::Alloca(ty), Type::Ptr));
        self.n_allocas += 1;
        Operand::Val(vid)
    }

    /// Declara um local no escopo corrente, guardado na representação `ty`.
    pub fn declarar_local(&mut self, sym: SymbolId, ty: Type) -> Operand {
        let ty = if matches!(ty, Type::Void | Type::I8 | Type::Ptr) {
            Type::Ref
        } else {
            ty
        };
        let ptr = self.alloca_na_entrada(ty);
        self.escopos.last_mut().expect("escopo da função").insert(
            sym,
            Local {
                ptr: Some(ptr.clone()),
                valor: None,
                ty,
            },
        );
        ptr
    }

    /// Declara e inicializa: o valor é coagido para a representação do local.
    pub fn declarar_local_com_valor(&mut self, sym: SymbolId, ty: Type, valor: Operand) {
        let ptr = self.declarar_local(sym, ty);
        let ty = self.buscar_local(sym).map_or(ty, |l| l.ty);
        let v = self.coagir(valor, ty);
        self.emit(Instruction::Store { ptr, val: v }, Type::Void);
    }

    /// Liga um nome a um valor já calculado (parâmetro de closure em linha).
    pub fn ligar_local(&mut self, sym: SymbolId, valor: Operand) {
        let ty = self.operand_type(&valor);
        self.escopos.last_mut().expect("escopo da função").insert(
            sym,
            Local {
                ptr: None,
                valor: Some(valor),
                ty,
            },
        );
    }

    /// O local visível com esse nome, do escopo mais interno para fora.
    pub fn buscar_local(&self, sym: SymbolId) -> Option<Local> {
        self.escopos.iter().rev().find_map(|e| e.get(&sym).cloned())
    }

    /// Lê um local (ou parâmetro) pelo nome, na representação dele.
    pub fn ler_local_por_nome(&mut self, sym: SymbolId) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        match (local.ptr, local.valor) {
            (Some(ptr), _) => Some(self.emit(Instruction::Load { ptr, ty: local.ty }, local.ty)),
            (None, valor) => valor,
        }
    }

    /// Grava num local visível; devolve o valor já na representação dele.
    pub fn gravar_local(&mut self, sym: SymbolId, valor: Operand) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        let v = self.coagir(valor, local.ty);
        match local.ptr {
            Some(ptr) => {
                self.emit(
                    Instruction::Store {
                        ptr,
                        val: v.clone(),
                    },
                    Type::Void,
                );
            }
            None => {
                // Valor imutável religado no escopo onde ele mora.
                for e in self.escopos.iter_mut().rev() {
                    if let Some(l) = e.get_mut(&sym) {
                        l.valor = Some(v.clone());
                        break;
                    }
                }
            }
        }
        Some(v)
    }

    /// Tira um nome de todos os escopos (os `this.x` no corpo do construtor).
    pub fn remover_local(&mut self, sym: SymbolId) {
        for e in &mut self.escopos {
            e.remove(&sym);
        }
    }

    /// Representação guardada de um local declarado em `offset` (R6): o
    /// tipo declarado, ou o inferido do inicializador. Sem informação, `Ref`
    /// — a caixa é sempre correta, só mais lenta.
    pub fn repr_do_local(&self, offset: usize) -> Type {
        self.ctx
            .tipo_local(self.unit_id, offset)
            .map_or(Type::Ref, |t| self.repr(t))
    }
}
