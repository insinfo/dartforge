//! Variáveis locais: escopo léxico e `alloca` na representação do tipo (R6),
//! e as variáveis capturadas por closures (P1).
//!
//! Antes, um local era um `alloca i64` criado onde a declaração aparecia (no
//! meio de um laço, o `alloca` crescia a pilha a cada volta) e um mapa único
//! por nome: um bloco que sombreava `current` sobrescrevia o `current` de
//! fora para sempre, e a variável de um `for (var i = 0; …; i++)` nem tinha
//! `alloca` — o `i++` trocava o valor SSA no mapa, e a condição do laço,
//! baixada antes, continuava lendo o `0` da entrada.
//!
//! Uma variável capturada por uma closure e **atribuída** em algum ponto
//! (`captura.rs` decide, antes de baixar a função) mora numa `Cell` do heap: o
//! `alloca` guarda o handle da célula, e quem lê ou grava passa por ela — a
//! closure vê a gravação feita depois da captura, e a gravação dela é vista
//! de fora. Capturada e nunca atribuída, ela é copiada para o ambiente da
//! closure na criação. Dentro da closure, a variável de fora é uma posição do
//! ambiente (`Ambiente`), com o handle da célula ou a cópia.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Onde um local visível mora.
#[derive(Debug, Clone)]
pub enum Modo {
    /// `alloca` na representação do local.
    Memoria(Operand),
    /// Valor imutável já calculado (o parâmetro de uma closure baixada em
    /// linha pelo código congelado de `sdk_por_nome.rs`).
    Valor(Operand),
    /// `alloca` `Ref` com o handle da célula que guarda o valor.
    Celula(Operand),
    /// Posição `indice` do ambiente da closure corrente (`env`); com
    /// `celula`, o que está lá é o handle da célula.
    Ambiente {
        env: Operand,
        indice: usize,
        celula: bool,
    },
}

/// Um local visível.
#[derive(Debug, Clone)]
pub struct Local {
    pub modo: Modo,
    pub ty: Type,
    /// Offset do nome na declaração (a chave de `captura.rs`); `None` para
    /// os ligados por valor.
    pub offset: Option<usize>,
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
    pub fn alloca_na_entrada(&mut self, ty: Type) -> Operand {
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

    fn repr_de_local(ty: Type) -> Type {
        if matches!(ty, Type::Void | Type::I8 | Type::Ptr) {
            Type::Ref
        } else {
            ty
        }
    }

    fn inserir_local(&mut self, sym: SymbolId, local: Local) {
        self.escopos
            .last_mut()
            .expect("escopo da função")
            .insert(sym, local);
    }

    /// Declara um local (sem offset: nunca é célula) no escopo corrente,
    /// guardado na representação `ty`.
    pub fn declarar_local(&mut self, sym: SymbolId, ty: Type) -> Operand {
        let ty = Self::repr_de_local(ty);
        let ptr = self.alloca_na_entrada(ty);
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Memoria(ptr.clone()),
                ty,
                offset: None,
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

    /// Declara a variável do usuário cujo nome começa em `offset` e a
    /// inicializa com `valor`. Se `captura.rs` a marcou (capturada e
    /// atribuída), ela mora numa célula nova — cada execução da declaração
    /// cria uma variável nova (a do corpo de um laço, uma por volta).
    pub fn declarar_variavel(&mut self, sym: SymbolId, offset: usize, ty: Type, valor: Operand) {
        let ty = Self::repr_de_local(ty);
        if !self.celulas.contains(&offset) {
            let ptr = self.alloca_na_entrada(ty);
            let v = self.coagir(valor, ty);
            self.emit(
                Instruction::Store {
                    ptr: ptr.clone(),
                    val: v,
                },
                Type::Void,
            );
            self.inserir_local(
                sym,
                Local {
                    modo: Modo::Memoria(ptr),
                    ty,
                    offset: Some(offset),
                },
            );
            return;
        }
        let ptr = self.alloca_na_entrada(Type::Ref);
        let v = self.coagir(valor, ty);
        let celula = self.emit(Instruction::AllocCell { value: v }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: celula,
            },
            Type::Void,
        );
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Celula(ptr),
                ty,
                offset: Some(offset),
            },
        );
    }

    /// Liga um nome a um posição do ambiente da closure corrente.
    pub fn ligar_ambiente(&mut self, sym: SymbolId, env: Operand, indice: usize, celula: bool, ty: Type) {
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Ambiente {
                    env,
                    indice,
                    celula,
                },
                ty,
                offset: None,
            },
        );
    }

    /// Liga um nome a um valor já calculado (parâmetro de closure em linha).
    pub fn ligar_local(&mut self, sym: SymbolId, valor: Operand) {
        let ty = self.operand_type(&valor);
        self.inserir_local(
            sym,
            Local {
                modo: Modo::Valor(valor),
                ty,
                offset: None,
            },
        );
    }

    /// O local visível com esse nome, do escopo mais interno para fora.
    pub fn buscar_local(&self, sym: SymbolId) -> Option<Local> {
        self.escopos.iter().rev().find_map(|e| e.get(&sym).cloned())
    }

    /// O handle da célula de um local que mora numa (declarado aqui ou
    /// vindo do ambiente); `None` se o local não é célula.
    pub fn celula_do_local(&mut self, local: &Local) -> Option<Operand> {
        match &local.modo {
            Modo::Celula(ptr) => Some(self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: Type::Ref,
                },
                Type::Ref,
            )),
            Modo::Ambiente {
                env,
                indice,
                celula: true,
            } => Some(self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *indice,
                },
                Type::Ref,
            )),
            _ => None,
        }
    }

    /// Lê um local já encontrado, na representação dele.
    pub fn ler_local(&mut self, local: &Local) -> Operand {
        if let Some(c) = self.celula_do_local(local) {
            return self.emit(Instruction::CellGet { cell: c }, local.ty);
        }
        match &local.modo {
            Modo::Memoria(ptr) => self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: local.ty,
                },
                local.ty,
            ),
            Modo::Valor(v) => v.clone(),
            Modo::Ambiente { env, indice, .. } => self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *indice,
                },
                local.ty,
            ),
            Modo::Celula(_) => unreachable!("célula tratada acima"),
        }
    }

    /// Lê um local (ou parâmetro) pelo nome, na representação dele.
    pub fn ler_local_por_nome(&mut self, sym: SymbolId) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        Some(self.ler_local(&local))
    }

    /// Grava num local visível; devolve o valor já na representação dele.
    /// Um local copiado para o ambiente nunca é gravado (`captura.rs` põe
    /// numa célula toda variável capturada que é atribuída).
    pub fn gravar_local(&mut self, sym: SymbolId, valor: Operand) -> Option<Operand> {
        let local = self.buscar_local(sym)?;
        let v = self.coagir(valor, local.ty);
        if let Some(c) = self.celula_do_local(&local) {
            self.emit(
                Instruction::CellSet {
                    cell: c,
                    value: v.clone(),
                },
                Type::Void,
            );
            return Some(v);
        }
        match local.modo {
            Modo::Memoria(ptr) => {
                self.emit(
                    Instruction::Store {
                        ptr,
                        val: v.clone(),
                    },
                    Type::Void,
                );
            }
            Modo::Valor(_) => {
                // Valor imutável religado no escopo onde ele mora.
                for e in self.escopos.iter_mut().rev() {
                    if let Some(l) = e.get_mut(&sym) {
                        l.modo = Modo::Valor(v.clone());
                        break;
                    }
                }
            }
            Modo::Ambiente { .. } | Modo::Celula(_) => return None,
        }
        Some(v)
    }

    /// Uma volta nova de um `for` clássico (a especificação, "for loop":
    /// cada iteração tem a sua variável, iniciada com o valor da anterior).
    /// Só importa para a variável que mora numa célula — uma closure da
    /// volta anterior continua com a célula antiga.
    pub fn renovar_celula(&mut self, sym: SymbolId) {
        let Some(local) = self.buscar_local(sym) else {
            return;
        };
        let Modo::Celula(ptr) = &local.modo else {
            return;
        };
        let antiga = self.emit(
            Instruction::Load {
                ptr: ptr.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        let valor = self.emit(Instruction::CellGet { cell: antiga }, local.ty);
        let nova = self.emit(Instruction::AllocCell { value: valor }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: nova,
            },
            Type::Void,
        );
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
