//! Inferência de argumentos de tipo: geração de restrições de subtipo e
//! solução, conforme `inference.md` ("Subtype constraint generation",
//! "Constraint solving") e a implementação oficial (`GenericInferrer` do
//! analyzer, `chooseTypes` de `_fe_analyzer_shared`).
//!
//! Uso típico (invocação genérica):
//!
//! 1. [`GenericInferrer::constrain_return`] com o tipo de retorno e o contexto
//!    (inferência "para baixo");
//! 2. [`GenericInferrer::choose_preliminary`] dá os esquemas parciais, com os
//!    quais se calculam os contextos dos argumentos;
//! 3. [`GenericInferrer::constrain_argument`] para cada argumento já inferido;
//! 4. [`GenericInferrer::choose_final`] dá os tipos finais (sem `_`).

use crate::bounds::{down, has_unknown, least_closure, schema_greatest, schema_least, up};
use crate::ops::{non_nullable, substitute};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{Type, TypeId, TypeParamId};
use std::collections::HashMap;

/// Restrição `lower <: X <: upper` (qualquer lado pode ser `_`).
#[derive(Debug, Clone, Copy)]
struct Restricao {
    param: usize,
    lower: TypeId,
    upper: TypeId,
    /// Índice em `GenericInferrer::origens` (`TypeConstraintOrigin`).
    origem: Option<usize>,
}

/// A origem de uma restrição, para a mensagem de `COULD_NOT_INFER`
/// (`fe76:type_inference/type_constraint.dart:173-330`).
#[derive(Debug, Clone)]
pub enum Origem {
    /// `TypeConstraintFromArgument`: `Parameter 'p' declared as '…' but
    /// argument is '…'.`; `prefixo` substitui `Parameter 'p'` (`List element`,
    /// `Map key`, `Map value`).
    Argumento { parametro: String, declarado: TypeId, argumento: TypeId, prefixo: Option<String> },
    /// `TypeConstraintFromReturnType`.
    Retorno { declarado: TypeId, contexto: TypeId },
    /// `TypeConstraintFromFunctionContext`.
    Funcao { declarado: TypeId, contexto: TypeId },
}

/// Inferidor de argumentos de tipo para um conjunto de parâmetros `L`.
pub struct GenericInferrer {
    pub params: Vec<TypeParamId>,
    restricoes: Vec<Restricao>,
    /// Escolhas já fixadas (tipos conhecidos) de rodadas anteriores.
    fixados: Vec<Option<TypeId>>,
    origens: Vec<Origem>,
    origem_atual: Option<usize>,
    /// `genericMetadataIsEnabled` (2.14): sem ele, o candidato que é tipo de
    /// função genérico é `COULD_NOT_INFER` (`generic_inferrer.dart:302-322`).
    pub metadados_genericos: bool,
}

impl GenericInferrer {
    pub fn new(params: &[TypeParamId]) -> Self {
        Self { params: params.to_vec(), restricoes: Vec::new(), fixados: vec![None; params.len()], origens: Vec::new(), origem_atual: None, metadados_genericos: true }
    }

    /// A origem das restrições geradas a seguir.
    pub fn com_origem(&mut self, o: Origem) {
        self.origens.push(o);
        self.origem_atual = Some(self.origens.len() - 1);
    }

    /// `arg <: param` (os parâmetros de `L` estão em `param`).
    pub fn constrain_argument(&mut self, arg: TypeId, param: TypeId, env: &mut SubtypeEnv) -> bool {
        let mark = self.restricoes.len();
        let ok = self.try_match(arg, param, false, env);
        if !ok {
            self.restricoes.truncate(mark);
        } else {
            self.descartar_fixados(mark);
        }
        ok
    }

    /// `declared <: context` (os parâmetros de `L` estão em `declared`).
    pub fn constrain_return(&mut self, declared: TypeId, context: TypeId, env: &mut SubtypeEnv) -> bool {
        let mark = self.restricoes.len();
        let ok = self.try_match(declared, context, true, env);
        if !ok {
            self.restricoes.truncate(mark);
        } else {
            self.descartar_fixados(mark);
        }
        ok
    }

    fn index(&self, t: TypeId, env: &SubtypeEnv) -> Option<usize> {
        match env.table.get(t) {
            Type::TypeParameter { param, nullable: false } => self.params.iter().position(|p| p == param),
            _ => None,
        }
    }

    fn is_unknown(t: TypeId, env: &SubtypeEnv) -> bool {
        env.core.is_unknown(env.table, t)
    }

    /// `p <# q [L]`: `left` indica que os parâmetros de `L` estão em `p`
    /// (o lado direito é o esquema), senão estão em `q`.
    fn try_match(&mut self, p: TypeId, q: TypeId, left: bool, env: &mut SubtypeEnv) -> bool {
        if Self::is_unknown(p, env) || Self::is_unknown(q, env) {
            return true;
        }
        if left {
            if let Some(i) = self.index(p, env) {
                self.restricoes.push(Restricao { param: i, lower: env.core.unknown, upper: q, origem: self.origem_atual });
                return true;
            }
        } else if let Some(i) = self.index(q, env) {
            self.restricoes.push(Restricao { param: i, lower: p, upper: env.core.unknown, origem: self.origem_atual });
            return true;
        }
        if p == q {
            return true;
        }
        let tp = env.table.get(p).clone();
        let tq = env.table.get(q).clone();
        // Q é FutureOr<Q0>.
        if let Type::FutureOr { arg: q0, nullable: false } = tq {
            if let Type::FutureOr { arg: p0, nullable: false } = tp {
                if self.sub(p0, q0, left, env) {
                    return true;
                }
            }
            if let Some(fq0) = future_of(q0, env) {
                let mark = self.restricoes.len();
                if self.try_match(p, fq0, left, env) && self.restricoes.len() > mark {
                    return true;
                }
                self.restricoes.truncate(mark);
                if self.sub(p, q0, left, env) {
                    return true;
                }
                return self.sub(p, fq0, left, env);
            }
            return self.sub(p, q0, left, env);
        }
        // Q é Q0?.
        if tq.is_declared_nullable() {
            let q0 = non_nullable(q, env.table);
            if tp.is_declared_nullable() {
                let p0 = non_nullable(p, env.table);
                if self.sub(p0, q0, left, env) {
                    return true;
                }
            }
            if matches!(tp, Type::Dynamic | Type::Void) {
                let o = env.core.object;
                if self.sub(o, q0, left, env) {
                    return true;
                }
            }
            let mark = self.restricoes.len();
            if self.try_match(p, q0, left, env) && self.restricoes.len() > mark {
                return true;
            }
            self.restricoes.truncate(mark);
            let n = env.core.null;
            if self.sub(p, n, left, env) {
                return true;
            }
            return self.sub(p, q0, left, env);
        }
        // P é FutureOr<P0>.
        if let Type::FutureOr { arg: p0, nullable: false } = tp {
            let Some(fp0) = future_of(p0, env) else { return false };
            let mark = self.restricoes.len();
            if self.try_match(fp0, q, left, env) && self.try_match(p0, q, left, env) {
                return true;
            }
            self.restricoes.truncate(mark);
            return false;
        }
        // P é P0?.
        if tp.is_declared_nullable() {
            let p0 = non_nullable(p, env.table);
            let mark = self.restricoes.len();
            let n = env.core.null;
            if self.try_match(p0, q, left, env) && self.try_match(n, q, left, env) {
                return true;
            }
            self.restricoes.truncate(mark);
            return false;
        }
        // Q topo.
        if matches!(tq, Type::Dynamic | Type::Void) || q == env.core.object_nullable {
            return true;
        }
        if matches!(tp, Type::Never) {
            return true;
        }
        // Q é Object: só se P não é anulável.
        if q == env.core.object {
            return estruturalmente_nao_anulavel(p, env);
        }
        // P é Null: só se Q é anulável.
        if matches!(tp, Type::Null) {
            return matches!(tq, Type::Null) || tq.is_declared_nullable();
        }
        // P é X & B: pelo limite promovido.
        if let Type::Intersection { bound, .. } = tp {
            return self.try_match(bound, q, left, env);
        }
        // P é variável de tipo fora de L: pelo limite.
        if let Type::TypeParameter { param, .. } = tp {
            if param == env.core.unknown_param {
                return true;
            }
            if let Type::TypeParameter { param: pq, .. } = tq {
                if pq == param {
                    return true;
                }
            }
            let b = env.table.param(param).bound;
            if b == p {
                return false;
            }
            return self.try_match(b, q, left, env);
        }
        // Interfaces (e tipos de extensão).
        match (&tp, &tq) {
            (
                Type::Interface { class: c0, args: a0, .. } | Type::ExtensionType { decl: c0, args: a0, .. },
                Type::Interface { class: c1, args: a1, .. } | Type::ExtensionType { decl: c1, args: a1, .. },
            ) => {
                if c0 == c1 && a0.len() == a1.len() {
                    // `_interfaceTypeArguments` (`type_analyzer_operations.dart:1230-1261`):
                    // pela variância declarada de cada parâmetro (contravariante
                    // troca os lados e o esquema; invariante, os dois).
                    let params: Vec<crate::table::TypeParamId> = env.hierarchy.get(*c0).map(|d| d.type_params.to_vec()).unwrap_or_default();
                    let mark = self.restricoes.len();
                    for (i, (x, y)) in a0.iter().zip(a1.iter()).enumerate() {
                        let variancia = params.get(i).map(|&p| env.table.param(p).variance).unwrap_or_default();
                        let co = !matches!(variancia, crate::table::Variance::Contravariant);
                        let contra = matches!(variancia, crate::table::Variance::Contravariant | crate::table::Variance::Invariant);
                        if co && !self.try_match(*x, *y, left, env) {
                            self.restricoes.truncate(mark);
                            return false;
                        }
                        if contra && !self.try_match(*y, *x, !left, env) {
                            self.restricoes.truncate(mark);
                            return false;
                        }
                    }
                    return true;
                }
                if let Some(sup) = env.hierarchy.supertype_of(p, *c1, env.table, env.core) {
                    if sup != p {
                        return self.try_match(sup, q, left, env);
                    }
                }
                return false;
            }
            _ => {}
        }
        // Função <: Function.
        if matches!(tp, Type::Function { .. }) {
            if let Type::Interface { class, .. } = tq {
                if Some(class) == env.core.function_class {
                    return true;
                }
            }
        }
        if let (
            Type::Function { type_params: tp0, ret: r0, positional: p0, optional: o0, named: n0, .. },
            Type::Function { type_params: tp1, ret: r1, positional: p1, optional: o1, named: n1, .. },
        ) = (&tp, &tq)
        {
            if tp0.len() != tp1.len() {
                return false;
            }
            // Genéricos: renomeia os parâmetros de Q para os de P.
            let mapa: HashMap<TypeParamId, TypeId> = tp1
                .iter()
                .zip(tp0.iter())
                .map(|(b, a)| (*b, env.table.intern(Type::TypeParameter { param: *a, nullable: false })))
                .collect();
            let s = |t: TypeId, env: &mut SubtypeEnv| substitute(t, &mapa, env.table);
            let r1 = s(*r1, env);
            let p1: Vec<TypeId> = p1.iter().map(|t| s(*t, env)).collect();
            let o1: Vec<TypeId> = o1.iter().map(|t| s(*t, env)).collect();
            let n1: Vec<_> = n1.iter().map(|(n, t, r)| (*n, s(*t, env), *r)).collect();
            let mark = self.restricoes.len();
            let ok = (|| {
                if !self.try_match(*r0, r1, left, env) {
                    return false;
                }
                if n0.is_empty() && n1.is_empty() {
                    let (n, k) = (p0.len(), p1.len());
                    let (m, r) = (n + o0.len(), k + o1.len());
                    if !(n <= k && r <= m) {
                        return false;
                    }
                    let todos0: Vec<TypeId> = p0.iter().chain(o0.iter()).copied().collect();
                    let todos1: Vec<TypeId> = p1.iter().chain(o1.iter()).copied().collect();
                    for i in 0..r {
                        if !self.try_match(todos1[i], todos0[i], !left, env) {
                            return false;
                        }
                    }
                    return true;
                }
                if !o0.is_empty() || !o1.is_empty() || p0.len() != p1.len() {
                    return false;
                }
                for (a, b) in p0.iter().zip(p1.iter()) {
                    if !self.try_match(*b, *a, !left, env) {
                        return false;
                    }
                }
                for (nm, t1, _r1) in n1.iter() {
                    match n0.iter().find(|(m, _, _)| m == nm) {
                        Some((_, t0, _)) => {
                            if !self.try_match(*t1, *t0, !left, env) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                for (nm, _, r0) in n0.iter() {
                    if *r0 && !n1.iter().any(|(m, _, r)| m == nm && *r) {
                        return false;
                    }
                }
                true
            })();
            if !ok {
                self.restricoes.truncate(mark);
            }
            return ok;
        }
        // Records.
        if matches!(tp, Type::Record { .. }) {
            if let Type::Interface { class, .. } = tq {
                if Some(class) == env.core.record_class {
                    return true;
                }
            }
        }
        if let (Type::Record { positional: p0, named: n0, .. }, Type::Record { positional: p1, named: n1, .. }) = (&tp, &tq) {
            if p0.len() != p1.len() || n0.len() != n1.len() {
                return false;
            }
            let mark = self.restricoes.len();
            for (a, b) in p0.iter().zip(p1.iter()) {
                if !self.try_match(*a, *b, left, env) {
                    self.restricoes.truncate(mark);
                    return false;
                }
            }
            for ((na, a), (nb, b)) in n0.iter().zip(n1.iter()) {
                if na != nb || !self.try_match(*a, *b, left, env) {
                    self.restricoes.truncate(mark);
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// Tentativa com desfazimento em caso de falha.
    fn sub(&mut self, p: TypeId, q: TypeId, left: bool, env: &mut SubtypeEnv) -> bool {
        let mark = self.restricoes.len();
        if self.try_match(p, q, left, env) {
            true
        } else {
            self.restricoes.truncate(mark);
            false
        }
    }

    /// Restrição fundida de um parâmetro: `UP` dos inferiores, `DOWN` dos superiores.
    fn fundir(&self, i: usize, env: &mut SubtypeEnv) -> (TypeId, TypeId) {
        let mut lower = env.core.unknown;
        let mut upper = env.core.unknown;
        for r in self.restricoes.iter().filter(|r| r.param == i) {
            lower = up(lower, r.lower, env);
            upper = down(upper, r.upper, env);
        }
        (lower, upper)
    }

    fn escolher(lower: TypeId, upper: TypeId, grounded: bool, env: &mut SubtypeEnv) -> TypeId {
        if !has_unknown(lower, env) {
            return lower;
        }
        if !has_unknown(upper, env) {
            return upper;
        }
        if !env.core.is_unknown(env.table, lower) {
            return if grounded { schema_least(lower, env) } else { lower };
        }
        if !env.core.is_unknown(env.table, upper) {
            return if grounded { schema_greatest(upper, env) } else { upper };
        }
        lower
    }

    /// Limite declarado do parâmetro `i`, se escrito (não o `Object?` implícito).
    fn limite(&self, i: usize, atuais: &[TypeId], env: &mut SubtypeEnv) -> Option<TypeId> {
        let b = env.table.param(self.params[i]).bound;
        if b == env.core.object_nullable || matches!(env.table.get(b), Type::Dynamic) {
            return None;
        }
        let mapa: HashMap<TypeParamId, TypeId> = self.params.iter().copied().zip(atuais.iter().copied()).collect();
        Some(substitute(b, &mapa, env.table))
    }

    /// Inferência usando bounds (3.7, `inference-using-bounds`): para cada
    /// parâmetro cuja restrição inferior fundida `L` (sem `_`) não satisfaz o
    /// limite declarado `B`, acrescenta as restrições de `L <: B` com os
    /// parâmetros de `B` como incógnitas. Em `f<X extends A<X>>(C())`, com
    /// `C <: B <: A<B>`, `C <# A<X>` dá `B <: X`, e a solução passa a ser
    /// `UP(C, B) = B` (antes da 3.7, `X = C` e o limite era violado).
    pub fn restringir_pelos_limites(&mut self, env: &mut SubtypeEnv) {
        for i in 0..self.params.len() {
            if self.fixados[i].is_some() {
                continue;
            }
            let b = env.table.param(self.params[i]).bound;
            if b == env.core.object_nullable || matches!(env.table.get(b), Type::Dynamic) {
                continue;
            }
            let (lower, _) = self.fundir(i, env);
            if has_unknown(lower, env) {
                continue;
            }
            let atuais: Vec<TypeId> = (0..self.params.len()).map(|j| self.fundir(j, env).0).collect();
            let limite = self.limite(i, &atuais, env);
            if limite.is_some_and(|l| !has_unknown(l, env) && is_subtype(lower, l, env)) {
                continue;
            }
            let mark = self.restricoes.len();
            if !self.try_match(lower, b, false, env) {
                self.restricoes.truncate(mark);
            } else {
                self.descartar_fixados(mark);
            }
        }
    }

    /// `_tryMatchSubtypeOf` (3.6.2 `generic_inferrer.dart:880-906`): das
    /// restrições do casamento bem-sucedido, as de um parâmetro já fixado
    /// pela inferência preliminar (`_typesInferredSoFar`) não entram
    /// (`Future<Future<String>> v = Future.value(Future<String>.value(''))`:
    /// o `String <: T` do argumento é descartado, `T` já é `Future<String>`).
    fn descartar_fixados(&mut self, mark: usize) {
        let fixados = &self.fixados;
        let mut i = mark;
        while i < self.restricoes.len() {
            if fixados[self.restricoes[i].param].is_some() {
                self.restricoes.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Inferência parcial (para baixo ou horizontal): esquemas que podem conter `_`.
    pub fn choose_preliminary(&mut self, env: &mut SubtypeEnv) -> Vec<TypeId> {
        let n = self.params.len();
        let mut tipos: Vec<TypeId> = (0..n).map(|i| self.fixados[i].unwrap_or(env.core.unknown)).collect();
        for i in 0..n {
            if let Some(f) = self.fixados[i] {
                tipos[i] = f;
                continue;
            }
            let (lower, upper) = self.fundir(i, env);
            let t = Self::escolher(lower, upper, false, env);
            if has_unknown(t, env) {
                tipos[i] = t;
                continue;
            }
            let t = match self.limite(i, &tipos, env) {
                Some(b) => {
                    let upper2 = down(upper, b, env);
                    Self::escolher(lower, upper2, false, env)
                }
                None => t,
            };
            tipos[i] = t;
        }
        for i in 0..n {
            if !has_unknown(tipos[i], env) {
                self.fixados[i] = Some(tipos[i]);
            }
        }
        tipos
    }

    /// Inferência final: tipos sem `_`; o que ficou sem restrição vai ao
    /// limite (instanciação para os limites: `dynamic` se não há limite).
    pub fn choose_final(&mut self, env: &mut SubtypeEnv) -> Vec<TypeId> {
        let n = self.params.len();
        let mut tipos: Vec<TypeId> = (0..n).map(|i| self.fixados[i].unwrap_or(env.core.unknown)).collect();
        let mut conhecidos = vec![false; n];
        for i in 0..n {
            if let Some(f) = self.fixados[i] {
                tipos[i] = f;
                conhecidos[i] = true;
                continue;
            }
            let (lower, mut upper) = self.fundir(i, env);
            if let Some(b) = self.limite(i, &tipos, env) {
                upper = down(upper, b, env);
            }
            let t = Self::escolher(lower, upper, true, env);
            tipos[i] = t;
            conhecidos[i] = !has_unknown(t, env);
        }
        // Instanciar para os limites os que não foram inferidos (na ordem
        // das dependências entre os limites; limite escrito `Object?` fica).
        let fixos: Vec<Option<TypeId>> = (0..n).map(|i| conhecidos[i].then_some(tipos[i])).collect();
        let instanciados = crate::ops::instanciar_para_limites(&self.params, &fixos, env.table, env.core);
        for i in 0..n {
            if conhecidos[i] {
                continue;
            }
            tipos[i] = instanciados[i];
            // Um limite F (`T extends Comparable<T>`) substituído pode ainda
            // mencionar parâmetros: fecha para cima.
            let params = self.params.clone();
            tipos[i] = crate::bounds::greatest_closure(tipos[i], &params, env);
        }
        tipos
    }

    /// `tryChooseFinalTypes(failAtError: false)` (`an611:src/dart/element/
    /// generic_inferrer.dart:250-386`): os `COULD_NOT_INFER` da inferência,
    /// `(nome do parâmetro de tipo, sufixo da mensagem)`. Refaz a escolha
    /// final como o analyzer (`_chooseTypes`, com a cláusula `extends`
    /// substituída pelos já escolhidos) e confere cada restrição e o limite
    /// com o tipo escolhido; sem erro, confere os tipos finais contra os
    /// limites (`_checkArgumentsNotMatchingBounds`, `:388-420`).
    pub fn falhas(
        &self,
        finais: &[TypeId],
        env: &mut SubtypeEnv,
        interner: &dartforge_intern::Interner,
        program: &dartforge_elements::model::Program,
    ) -> Vec<(String, String)> {
        let n = self.params.len();
        let fmt = |env: &SubtypeEnv, t: TypeId| env.table.format_sem_alias(t, interner, program);
        let mut inferidos: Vec<TypeId> = vec![env.core.unknown; n];
        let mut saida = Vec::new();
        for i in 0..n {
            let (lower, mut upper) = self.fundir(i, env);
            let limite = self.limite_escrito(i, &inferidos, env);
            if let Some(b) = limite {
                upper = down(upper, b, env);
            }
            let t = match self.fixados[i] {
                Some(f) => f,
                None => Self::escolher(lower, upper, true, env),
            };
            inferidos[i] = t;
        }
        // `tryChooseFinalTypes` (`generic_inferrer.dart:250-300`): cada
        // parâmetro é conferido contra as próprias restrições e o `extends`
        // com a escolha do `_chooseTypes` final (`rec`); só sem erro os tipos
        // finais (instanciados para os limites) passam pela conferência dos
        // limites (`_checkArgumentsNotMatchingBounds`).
        let rec = inferidos;
        for i in 0..n {
            let nome = interner.resolve(env.table.param(self.params[i]).name).to_string();
            if let Some(msg) = self.mensagem_de_falha(i, &rec, env, interner, program) {
                saida.push((nome.clone(), msg));
            }
            // Candidato tipo de função genérico sem `generic-metadata`
            // (`generic_inferrer.dart:302-322`).
            if !self.metadados_genericos
                && let Type::Function { type_params, .. } = env.table.get(rec[i]).clone()
                && !type_params.is_empty()
            {
                let formais: Vec<String> = type_params
                    .iter()
                    .map(|&p| {
                        let d = env.table.param(p).clone();
                        let n = interner.resolve(d.name).to_string();
                        if d.explicito { format!("{n} extends {}", fmt(env, d.bound)) } else { n }
                    })
                    .collect();
                saida.push((
                    nome,
                    format!(
                        " Inferred candidate type {} has type parameters [{}], but a function with type parameters cannot be used as a type argument.",
                        fmt(env, rec[i]),
                        formais.join(", ")
                    ),
                ));
            }
        }
        if saida.is_empty() {
            for i in 0..n.min(finais.len()) {
                let d = env.table.param(self.params[i]).clone();
                if !d.explicito {
                    continue;
                }
                let mapa: HashMap<TypeParamId, TypeId> = self.params.iter().copied().zip(finais.iter().copied()).collect();
                let b = substitute(d.bound, &mapa, env.table);
                let dinamico = matches!(env.table.get(finais[i]), Type::Dynamic)
                    && self.restricoes.iter().any(|r| r.param == i && matches!(env.table.get(r.lower), Type::Dynamic));
                if !dinamico && !is_subtype(finais[i], b, env) {
                    let nome = interner.resolve(d.name).to_string();
                    let args: Vec<String> = finais.iter().map(|&x| fmt(env, x)).collect();
                    saida.push((
                        nome,
                        format!(
                            "\n'{}' doesn't conform to the bound '{}', instantiated from '{}' using type arguments [{}].",
                            fmt(env, finais[i]),
                            fmt(env, b),
                            fmt(env, d.bound),
                            args.join(", ")
                        ),
                    ));
                }
            }
        }
        saida
    }

    /// O sufixo de `COULD_NOT_INFER` do parâmetro `i` com os tipos
    /// `vetor` (`_formatError`, `an611:src/dart/element/generic_inferrer.dart:587-630`),
    /// ou `None` se o escolhido satisfaz as restrições e o limite.
    fn mensagem_de_falha(
        &self,
        i: usize,
        vetor: &[TypeId],
        env: &mut SubtypeEnv,
        interner: &dartforge_intern::Interner,
        program: &dartforge_elements::model::Program,
    ) -> Option<String> {
        let fmt = |env: &SubtypeEnv, t: TypeId| env.table.format_sem_alias(t, interner, program);
        let t = vetor[i];
        let proprias: Vec<Restricao> = self.restricoes.iter().filter(|r| r.param == i).copied().collect();
        let satisfaz = |r: &Restricao, env: &mut SubtypeEnv| {
            let ok_l = env.core.is_unknown(env.table, r.lower) || {
                let l = schema_least(r.lower, env);
                is_subtype(l, t, env)
            };
            let ok_u = env.core.is_unknown(env.table, r.upper) || {
                let u = schema_greatest(r.upper, env);
                is_subtype(t, u, env)
            };
            ok_l && ok_u
        };
        let mut sucesso = proprias.iter().all(|r| satisfaz(r, env));
        let mut extends: Option<(String, String, bool)> = None;
        if sucesso {
            if let Some(b) = self.limite_escrito(i, vetor, env) {
                let bruto = env.table.param(self.params[i]).bound;
                // `isSatisfiedBy` (`type_constraint.dart:128-135`): o tipo
                // contra o fecho maior do esquema (`A<_>` vale `A<Object?>`).
                let fecho = schema_greatest(b, env);
                let ok = is_subtype(t, fecho, env);
                extends = Some((fmt(env, bruto), fmt(env, b), ok));
                sucesso = ok;
            }
        }
        if sucesso {
            return None;
        }
        let nome = interner.resolve(env.table.param(self.params[i]).name).to_string();
        // Por origem, na ordem de chegada: as não satisfeitas e as satisfeitas.
        let mut por_origem: Vec<(Option<usize>, bool)> = Vec::new();
        for r in &proprias {
            let ok = satisfaz(r, env);
            match por_origem.iter_mut().find(|(o, _)| *o == r.origem) {
                Some(x) => x.1 &= ok,
                None => por_origem.push((r.origem, ok)),
            }
        }
        let mut linhas_ns: Vec<Vec<String>> = Vec::new();
        let mut linhas_s: Vec<Vec<String>> = Vec::new();
        for (o, ok) in &por_origem {
            let partes = match o.and_then(|o| self.origens.get(o)) {
                Some(Origem::Argumento { parametro, declarado, argumento, prefixo }) => vec![
                    prefixo.clone().unwrap_or_else(|| format!("Parameter '{parametro}'")),
                    format!("declared as     '{}'", fmt(env, *declarado)),
                    format!("but argument is '{}'.", fmt(env, *argumento)),
                ],
                Some(Origem::Retorno { declarado, contexto }) => vec![
                    "Return type".to_string(),
                    format!("declared as '{}'", fmt(env, *declarado)),
                    format!("used where  '{}' is required.", fmt(env, *contexto)),
                ],
                Some(Origem::Funcao { declarado, contexto }) => vec![
                    "Function type".to_string(),
                    format!("declared as '{}'", fmt(env, *declarado)),
                    format!("used where  '{}' is required.", fmt(env, *contexto)),
                ],
                None => continue,
            };
            if *ok {
                linhas_s.push(partes);
            } else {
                linhas_ns.push(partes);
            }
        }
        if let Some((bruto, produzido, ok)) = extends {
            let partes = vec![format!("Type parameter '{nome}'"), format!("is declared to extend '{bruto}' producing '{produzido}'.")];
            if ok {
                linhas_s.push(partes);
            } else {
                linhas_ns.push(partes);
            }
        }
        let ts = fmt(env, t);
        let nao = formatar_restricoes(&linhas_ns);
        let mut sim = formatar_restricoes(&linhas_s);
        if !sim.is_empty() {
            sim = format!("
The type '{ts}' was inferred from:
{sim}");
        }
        let intro = format!("Tried to infer '{ts}' for '{nome}' which doesn't work:");
        Some(format!("

{intro}
{nao}{sim}

Consider passing explicit type argument(s) to the generic.

"))
    }

    /// O limite escrito do parâmetro `i`, com os parâmetros substituídos
    /// pelos `atuais` (a cláusula `extends`).
    fn limite_escrito(&self, i: usize, atuais: &[TypeId], env: &mut SubtypeEnv) -> Option<TypeId> {
        if !env.table.param(self.params[i]).explicito {
            return None;
        }
        let b = env.table.param(self.params[i]).bound;
        let mapa: HashMap<TypeParamId, TypeId> = self.params.iter().copied().zip(atuais.iter().copied()).collect();
        Some(substitute(b, &mapa, env.table))
    }

    /// Se os tipos finais satisfazem as restrições coletadas.
    pub fn satisfeito(&self, tipos: &[TypeId], env: &mut SubtypeEnv) -> bool {
        for r in &self.restricoes {
            let t = tipos[r.param];
            if !env.core.is_unknown(env.table, r.lower) {
                let l = schema_least(r.lower, env);
                if !is_subtype(l, t, env) {
                    return false;
                }
            }
            if !env.core.is_unknown(env.table, r.upper) {
                let u = schema_greatest(r.upper, env);
                if !is_subtype(t, u, env) {
                    return false;
                }
            }
        }
        true
    }
}

/// `_formatConstraints` (`an611:src/dart/element/generic_inferrer.dart:914-947`):
/// uma linha por origem (sem repetir), o prefixo alinhado pelo maior.
fn formatar_restricoes(partes: &[Vec<String>]) -> String {
    let maior = partes.iter().map(|p| p[0].chars().count()).max().unwrap_or(0);
    let mut linhas: Vec<String> = Vec::new();
    for p in partes {
        let pad = " ".repeat(maior - p[0].chars().count());
        let meio = " ".repeat(maior);
        let fim = if p.len() > 2 { format!("\n  {meio} {}", p[2]) } else { String::new() };
        let l = format!("  {}{pad} {}{fim}", p[0], p[1]);
        if !linhas.contains(&l) {
            linhas.push(l);
        }
    }
    linhas.join("\n")
}

fn future_of(t: TypeId, env: &mut SubtypeEnv) -> Option<TypeId> {
    let f = env.core.future_class?;
    Some(env.table.intern(Type::Interface { class: f, args: Box::new([t]), nullable: false }))
}

/// Não-anulabilidade estrutural (sem consultar subtipagem com variáveis de `L`).
fn estruturalmente_nao_anulavel(t: TypeId, env: &mut SubtypeEnv) -> bool {
    match env.table.get(t).clone() {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Never => true,
        Type::FutureOr { arg, nullable } => !nullable && estruturalmente_nao_anulavel(arg, env),
        Type::TypeParameter { param, nullable } => {
            if nullable {
                return false;
            }
            if param == env.core.unknown_param {
                return true;
            }
            let b = env.table.param(param).bound;
            b != t && estruturalmente_nao_anulavel(b, env)
        }
        other => !other.is_declared_nullable(),
    }
}

/// Instancia um tipo genérico de função (`<X..>(..) -> R`) com os argumentos
/// dados, devolvendo a função sem parâmetros de tipo.
pub fn instanciar_funcao(f: TypeId, args: &[TypeId], env: &mut SubtypeEnv) -> TypeId {
    let Type::Function { type_params, ret, positional, optional, named, nullable } = env.table.get(f).clone() else {
        return f;
    };
    let mapa: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(args.iter().copied()).collect();
    let ret = substitute(ret, &mapa, env.table);
    let positional: Vec<TypeId> = positional.iter().map(|t| substitute(*t, &mapa, env.table)).collect();
    let optional: Vec<TypeId> = optional.iter().map(|t| substitute(*t, &mapa, env.table)).collect();
    let named: Vec<_> = named.iter().map(|(n, t, r)| (*n, substitute(*t, &mapa, env.table), *r)).collect();
    env.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: positional.into_boxed_slice(),
        optional: optional.into_boxed_slice(),
        named: named.into_boxed_slice(),
        nullable,
    })
}

/// Substitui os parâmetros `params` por `_` (esquema de contexto de argumento).
pub fn esquema_com_desconhecidos(t: TypeId, params: &[TypeParamId], tipos: &[TypeId], env: &mut SubtypeEnv) -> TypeId {
    let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(tipos.iter().copied()).collect();
    substitute(t, &mapa, env.table)
}

/// Fecho menor em relação a parâmetros (reexportado para os chamadores).
pub fn fecho_menor(t: TypeId, params: &[TypeParamId], env: &mut SubtypeEnv) -> TypeId {
    least_closure(t, params, env)
}
