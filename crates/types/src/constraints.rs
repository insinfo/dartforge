//! Solucionador de restrições de tipos para inferência de argumentos de tipo genéricos.
//!
//! Implementa as regras de inferência de chamadas genéricas de `inference.md`:
//! - Coleta de limites inferiores (`L <: X`) a partir de argumentos posicionais e nomeados.
//! - Coleta de limites superiores (`X <: U`) a partir do tipo de contexto de retorno e contravariância.
//! - Solução de restrições com LUB / GLB e fallback para o bound declarado da variável de tipo.

use crate::hierarchy::ClassHierarchy;
use crate::ops::{glb, lub};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use std::collections::{HashMap, HashSet};

/// Limites acumulados para um parâmetro de tipo genérico `X`.
#[derive(Debug, Clone, Default)]
pub struct TypeParamBounds {
    pub lower: Vec<TypeId>,
    pub upper: Vec<TypeId>,
}

/// Solucionador de restrições de tipos genéricos em chamadas de métodos, funções e construtores.
pub struct ConstraintSolver<'a> {
    pub table: &'a mut TypeTable,
    pub hierarchy: &'a ClassHierarchy,
    pub core: &'a CoreTypes,
}

impl<'a> ConstraintSolver<'a> {
    pub fn new(
        table: &'a mut TypeTable,
        hierarchy: &'a ClassHierarchy,
        core: &'a CoreTypes,
    ) -> Self {
        Self {
            table,
            hierarchy,
            core,
        }
    }

    /// Infere os argumentos de tipo para um conjunto de parâmetros formais `type_params`.
    pub fn infer_type_arguments(
        &mut self,
        type_params: &[TypeParamId],
        param_types: &[TypeId],
        arg_types: &[TypeId],
        return_type: TypeId,
        context_type: Option<TypeId>,
    ) -> Vec<TypeId> {
        let targets: HashSet<TypeParamId> = type_params.iter().copied().collect();
        let mut bounds_map: HashMap<TypeParamId, TypeParamBounds> = HashMap::new();
        for &tp in type_params {
            bounds_map.insert(tp, TypeParamBounds::default());
        }

        // 1. Restrições do tipo de contexto sobre o retorno: R <: Context
        if let Some(ctx) = context_type {
            self.collect_constraints(return_type, ctx, &targets, &mut bounds_map, true);
        }

        // 2. Restrições dos argumentos sobre os parâmetros formais: Arg_i <: Param_i
        for (&arg_ty, &param_ty) in arg_types.iter().zip(param_types.iter()) {
            self.collect_constraints(arg_ty, param_ty, &targets, &mut bounds_map, true);
        }

        // 3. Resolve cada parâmetro de tipo
        let mut solution = Vec::with_capacity(type_params.len());
        for &tp in type_params {
            let bounds = bounds_map.get(&tp).cloned().unwrap_or_default();
            let resolved_ty = self.solve_parameter_bounds(tp, &bounds);
            solution.push(resolved_ty);
        }

        solution
    }

    /// Resolve um único parâmetro de tipo a partir de seus limites acumulados.
    fn solve_parameter_bounds(&mut self, tp: TypeParamId, bounds: &TypeParamBounds) -> TypeId {
        let declared_bound = self.table.param(tp).bound;

        // Se há limites inferiores (ex.: argumentos concretos passados para X)
        if !bounds.lower.is_empty() {
            let mut current_lub = bounds.lower[0];
            for &l in &bounds.lower[1..] {
                let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
                current_lub = lub(current_lub, l, &mut env);
            }

            // Verifica se satisfaz o bound declarado
            let satisfies_declared = {
                let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
                is_subtype(current_lub, declared_bound, &mut env)
            };

            if satisfies_declared {
                return current_lub;
            }
        }

        // Se há limites superiores (ex.: contexto de retorno esperando X <: U)
        if !bounds.upper.is_empty() {
            let mut current_glb = bounds.upper[0];
            for &u in &bounds.upper[1..] {
                let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
                current_glb = glb(current_glb, u, &mut env);
            }
            let satisfies_declared = {
                let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
                is_subtype(current_glb, declared_bound, &mut env)
            };
            if satisfies_declared {
                return current_glb;
            }
        }

        // Fallback: limite declarado se não for Object?, senão dynamic
        if declared_bound != self.core.object_nullable && declared_bound != self.core.object {
            declared_bound
        } else {
            self.core.dynamic_
        }
    }

    /// Coleta recursivamente restrições de subtipagem: `t1 <: t2`.
    fn collect_constraints(
        &mut self,
        t1: TypeId,
        t2: TypeId,
        targets: &HashSet<TypeParamId>,
        bounds: &mut HashMap<TypeParamId, TypeParamBounds>,
        direction_sub: bool,
    ) {
        // Se t2 é um dos parâmetros alvo: t1 <: X => X tem limite inferior t1
        if let Type::TypeParameter { param, .. } = *self.table.get(t2) {
            if targets.contains(&param) {
                if direction_sub {
                    bounds.entry(param).or_default().lower.push(t1);
                } else {
                    bounds.entry(param).or_default().upper.push(t1);
                }
                return;
            }
        }

        // Se t1 é um dos parâmetros alvo: X <: t2 => X tem limite superior t2
        if let Type::TypeParameter { param, .. } = *self.table.get(t1) {
            if targets.contains(&param) {
                if direction_sub {
                    bounds.entry(param).or_default().upper.push(t2);
                } else {
                    bounds.entry(param).or_default().lower.push(t2);
                }
                return;
            }
        }

        let type1 = self.table.get(t1).clone();
        let type2 = self.table.get(t2).clone();

        match (type1, type2) {
            // Interfaces com mesma classe: decompõe argumentos
            (
                Type::Interface { class: c1, args: a1, .. },
                Type::Interface { class: c2, args: a2, .. },
            ) => {
                if c1 == c2 && a1.len() == a2.len() {
                    for (&arg1, &arg2) in a1.iter().zip(a2.iter()) {
                        self.collect_constraints(arg1, arg2, targets, bounds, direction_sub);
                    }
                } else if let Some(super_t1) = self.hierarchy.supertype_of(t1, c2, self.table, self.core) {
                    if let Type::Interface { args: super_args, .. } = self.table.get(super_t1).clone() {
                        if super_args.len() == a2.len() {
                            for (&sa, &arg2) in super_args.iter().zip(a2.iter()) {
                                self.collect_constraints(sa, arg2, targets, bounds, direction_sub);
                            }
                        }
                    }
                }
            }
            // Tipos funcionais: contravariância nos parâmetros e covariância no retorno
            (
                Type::Function {
                    ret: r1,
                    positional: p1,
                    ..
                },
                Type::Function {
                    ret: r2,
                    positional: p2,
                    ..
                },
            ) => {
                // Retorno é covariante
                self.collect_constraints(r1, r2, targets, bounds, direction_sub);
                // Parâmetros são contravariantes (inverte direção)
                for (&param1, &param2) in p1.iter().zip(p2.iter()) {
                    self.collect_constraints(param2, param1, targets, bounds, !direction_sub);
                }
            }
            // FutureOr<T>
            (Type::FutureOr { arg: a1, .. }, Type::FutureOr { arg: a2, .. }) => {
                self.collect_constraints(a1, a2, targets, bounds, direction_sub);
            }
            // Records posicionais
            (
                Type::Record { positional: p1, .. },
                Type::Record { positional: p2, .. },
            ) => {
                if p1.len() == p2.len() {
                    for (&arg1, &arg2) in p1.iter().zip(p2.iter()) {
                        self.collect_constraints(arg1, arg2, targets, bounds, direction_sub);
                    }
                }
            }
            _ => {}
        }
    }
}
