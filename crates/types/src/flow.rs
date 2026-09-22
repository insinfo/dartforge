//! Análise de fluxo, atribuição definitiva e promoção de tipos (Flow Analysis).
//!
//! Implementa as regras normativas de `flow-analysis.md`:
//! - Promoção de variáveis locais em `is`, `is!`, `!= null`, `== null`.
//! - Bifurcações booleanas (`SplitFlowState`) em `&&`, `||`, `!`.
//! - Interrupção de fluxo em `return`, `throw`, `rethrow` e chamadas com retorno `Never`.
//! - Junção de estados em pontos de convergência (`join` com LUB e interseção de `assigned`).
//! - Despromoção em reatribuição e detecção de atribuição definitiva antes do uso.

use crate::hierarchy::ClassHierarchy;
use crate::ops::{lub, non_nullable};
use crate::resolved::LocalId;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, TypeId, TypeTable};
use std::collections::HashMap;

/// Estado de uma variável local no fluxo de execução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarFlowState {
    /// Tipo promovido atualmente (ou `None` se for o tipo estático declarado).
    pub promoted: Option<TypeId>,
    /// Se a variável foi definitivamente atribuída até este ponto do fluxo.
    pub assigned: bool,
}

/// Estado geral de fluxo de controle em um determinado ponto de um corpo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowState {
    /// Se o ponto atual é alcançável (código após `return`/`throw` é inalcançável).
    pub reachable: bool,
    /// Mapa de variáveis locais para seus estados no fluxo.
    pub vars: HashMap<LocalId, VarFlowState>,
}

/// Par de estados de fluxo para expressões condicionais booleanas.
#[derive(Debug, Clone)]
pub struct SplitFlowState {
    /// Estado assumido se a condição for verdadeira.
    pub true_state: FlowState,
    /// Estado assumido se a condição for falsa.
    pub false_state: FlowState,
}

impl FlowState {
    /// Cria um novo estado de fluxo inicialmente alcançável.
    pub fn new_reachable() -> Self {
        Self {
            reachable: true,
            vars: HashMap::new(),
        }
    }

    /// Cria um novo estado inalcançável (após `return`, `throw`, etc.).
    pub fn new_unreachable() -> Self {
        Self {
            reachable: false,
            vars: HashMap::new(),
        }
    }

    /// Declara uma nova variável local no fluxo.
    pub fn declare(&mut self, id: LocalId, assigned: bool) {
        self.vars.insert(
            id,
            VarFlowState {
                promoted: None,
                assigned,
            },
        );
    }

    /// Obtém o tipo estático efetivo da variável local (promovido ou declarado).
    pub fn get_effective_type(&self, id: LocalId, declared_type: TypeId) -> TypeId {
        self.vars
            .get(&id)
            .and_then(|v| v.promoted)
            .unwrap_or(declared_type)
    }

    /// Verifica se a variável local foi definitivamente atribuída.
    pub fn is_assigned(&self, id: LocalId) -> bool {
        self.vars.get(&id).is_some_and(|v| v.assigned)
    }

    /// Registra uma atribuição à variável local.
    pub fn assign(
        &mut self,
        id: LocalId,
        expr_ty: TypeId,
        declared_type: TypeId,
        table: &mut TypeTable,
        hierarchy: &ClassHierarchy,
        core: &CoreTypes,
    ) {
        let is_sub = {
            let mut env = SubtypeEnv::new(table, hierarchy, core);
            is_subtype(expr_ty, declared_type, &mut env)
        };

        if let Some(state) = self.vars.get_mut(&id) {
            state.assigned = true;
            if is_sub && expr_ty != declared_type {
                state.promoted = Some(expr_ty);
            } else {
                state.promoted = None;
            }
        }
    }

    /// Promove o tipo de uma variável para `target_type` se for um subtipo do tipo atual.
    pub fn promote(
        &mut self,
        id: LocalId,
        target_type: TypeId,
        declared_type: TypeId,
        table: &mut TypeTable,
        hierarchy: &ClassHierarchy,
        core: &CoreTypes,
    ) {
        let current_ty = self.get_effective_type(id, declared_type);
        let can_promote = {
            let mut env = SubtypeEnv::new(table, hierarchy, core);
            is_subtype(target_type, current_ty, &mut env)
        };

        if can_promote {
            if let Some(state) = self.vars.get_mut(&id) {
                state.promoted = Some(target_type);
            }
        }
    }

    /// Promove uma variável para sua versão não-anulável (`T` se for `T?`).
    pub fn promote_non_null(&mut self, id: LocalId, declared_type: TypeId, table: &mut TypeTable) {
        let current_ty = self.get_effective_type(id, declared_type);
        let non_null_ty = non_nullable(current_ty, table);
        if non_null_ty != current_ty {
            if let Some(state) = self.vars.get_mut(&id) {
                state.promoted = Some(non_null_ty);
            }
        }
    }

    /// Marca o ponto atual como inalcançável.
    pub fn terminate(&mut self) {
        self.reachable = false;
    }

    /// Retorna verdadeiro se o fluxo de controle foi interrompido (código inalcançável).
    pub fn is_terminated(&self) -> bool {
        !self.reachable
    }

    /// Unifica dois estados de fluxo concorrentes em um ponto de junção (ex.: fim de `if/else`).
    pub fn join(
        s1: &FlowState,
        s2: &FlowState,
        declared_types: &HashMap<LocalId, TypeId>,
        table: &mut TypeTable,
        hierarchy: &ClassHierarchy,
        core: &CoreTypes,
    ) -> FlowState {
        // Se um dos ramos é inalcançável, o ponto de junção assume 100% do ramo alcançável
        if !s1.reachable {
            return s2.clone();
        }
        if !s2.reachable {
            return s1.clone();
        }

        let mut joined_vars = HashMap::new();
        for (&id, v1) in &s1.vars {
            if let Some(v2) = s2.vars.get(&id) {
                let assigned = v1.assigned && v2.assigned;
                let decl_ty = declared_types.get(&id).copied().unwrap_or(core.dynamic_);

                let promoted = match (v1.promoted, v2.promoted) {
                    (Some(p1), Some(p2)) if p1 == p2 => Some(p1),
                    (Some(p1), Some(p2)) => {
                        let mut env = SubtypeEnv::new(table, hierarchy, core);
                        let lub_ty = lub(p1, p2, &mut env);
                        if lub_ty != decl_ty {
                            Some(lub_ty)
                        } else {
                            None
                        }
                    }
                    (Some(p1), None) => {
                        let mut env = SubtypeEnv::new(table, hierarchy, core);
                        let lub_ty = lub(p1, decl_ty, &mut env);
                        if lub_ty != decl_ty {
                            Some(lub_ty)
                        } else {
                            None
                        }
                    }
                    (None, Some(p2)) => {
                        let mut env = SubtypeEnv::new(table, hierarchy, core);
                        let lub_ty = lub(decl_ty, p2, &mut env);
                        if lub_ty != decl_ty {
                            Some(lub_ty)
                        } else {
                            None
                        }
                    }
                    (None, None) => None,
                };

                joined_vars.insert(id, VarFlowState { promoted, assigned });
            }
        }

        FlowState {
            reachable: true,
            vars: joined_vars,
        }
    }
}

impl SplitFlowState {
    /// Cria um par de estados onde ambos os ramos partem do mesmo estado inicial.
    pub fn from_state(state: &FlowState) -> Self {
        Self {
            true_state: state.clone(),
            false_state: state.clone(),
        }
    }

    /// Aplica o operador lógico `NOT` (`!e`), invertendo os ramos verdadeiro e falso.
    pub fn not(self) -> Self {
        Self {
            true_state: self.false_state,
            false_state: self.true_state,
        }
    }

    /// Combina com uma segunda condição sob conjunção (`left && right`).
    pub fn and<F>(
        self,
        right_fn: F,
        declared_types: &HashMap<LocalId, TypeId>,
        table: &mut TypeTable,
        hierarchy: &ClassHierarchy,
        core: &CoreTypes,
    ) -> Self
    where
        F: FnOnce(FlowState) -> SplitFlowState,
    {
        // O lado direito só é avaliado se o lado esquerdo for verdadeiro
        let right_split = right_fn(self.true_state);
        // O resultado geral é falso se o esquerdo for falso OU se o direito for falso
        let joined_false = FlowState::join(
            &self.false_state,
            &right_split.false_state,
            declared_types,
            table,
            hierarchy,
            core,
        );

        Self {
            true_state: right_split.true_state,
            false_state: joined_false,
        }
    }

    /// Combina com uma segunda condição sob disjunção (`left || right`).
    pub fn or<F>(
        self,
        right_fn: F,
        declared_types: &HashMap<LocalId, TypeId>,
        table: &mut TypeTable,
        hierarchy: &ClassHierarchy,
        core: &CoreTypes,
    ) -> Self
    where
        F: FnOnce(FlowState) -> SplitFlowState,
    {
        // O lado direito só é avaliado se o lado esquerdo for falso
        let right_split = right_fn(self.false_state);
        // O resultado geral é verdadeiro se o esquerdo for verdadeiro OU se o direito for verdadeiro
        let joined_true = FlowState::join(
            &self.true_state,
            &right_split.true_state,
            declared_types,
            table,
            hierarchy,
            core,
        );

        Self {
            true_state: joined_true,
            false_state: right_split.false_state,
        }
    }
}
