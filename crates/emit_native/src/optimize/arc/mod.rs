//! Análises e inserção inicial de ownership ARC na HIR (§20 da especificação).
//!
//! A vivacidade recebe o inventário semântico de referências/slots; não
//! classifica externs nem prova consumo de tokens por si só. Produtores,
//! verificadores e inserção em retornos/saídas existem como passes explícitos;
//! ainda precisam de cobertura completa e integração no pipeline padrão.

use super::cfg::Cfg;
use super::operands::{operands, terminator_operands};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// O prefixo do thunk Owned genérico de uma função do runtime que devolve
/// `Ref` (`natives::prepare`, `contracts::generic_owned_contract`); o
/// emissor LLVM define o thunk (`llvm/selectors.rs`).
pub const GENERIC_OWNED_PREFIX: &str = "df.arc.owned.";
/// O thunk de escopo nativo: a função do runtime que pode rodar Dart, com o
/// resultado e o contrato da base, dentro de um escopo nativo (os tokens que
/// portas Dart devolvem morrem no fechamento), definido pelo emissor.
pub const NATIVE_SCOPE_PREFIX: &str = "df.arc.escopo.";
pub(crate) use natives::ERRORS_WITH_ARC_VARIANT;

mod classification;
pub mod analysis;
mod boxes;
mod natives;
mod scopes;
mod tokens;
mod contracts;
mod locals;
mod shared_slots;
mod slot_lives;
mod slot_plans;
pub use slot_plans::{SharedFramesCfgContract, produce_cfg_ref_slot_plans};
pub use slot_lives::verify_cfg_ref_slot_lives;
pub use shared_slots::{audit_shared_ref_slots, lower_shared_ref_slots, ArcRefSlotsDraft};
mod pure_set;
mod derived_set;
mod borrows;
mod edges;
mod phis;
mod layouts;
mod objects;
mod fields;
mod globals;
mod receivers;
mod cfg_classes;
mod calls;
mod functions;
mod modules;
pub use modules::{certify_arc_module_ports, prepare_arc_dart_modules};
mod plans;
pub use plans::produce_module_arc_tables;
pub use plans::prepare_arc_tabled_module;
mod parameters;
mod vectors;
mod selectors;
mod typed_set;
mod ports;
pub use ports::{ArcAotPorts, certify_arc_aot_ports};
mod rti_caches;
pub use parameters::produce_lowering_scalar_parameters;
pub use parameters::produce_lowering_rti_parameters;
mod returns;
pub use returns::produce_lowering_ref_returns;
mod invocations;
mod exits;
mod ssa;
mod frames;
mod foreign_cleanup;
pub use foreign_cleanup::ForeignCleanup;
pub use contracts::{HELPER_CONTRACTS, RuntimeCallContract, contract, runtime_call_contract, produce_runtime_contracts, produce_arc_contracts};
pub use contracts::produce_and_verify_tokens;
pub use contracts::produce_dart_ref_parameters;
pub use contracts::produce_and_verify_dart_tokens;
pub use contracts::insert_dart_return_retention;
pub use contracts::insert_arc_dart_exits;
pub use calls::{DartFunctionContract, verify_dart_function_contract, produce_dart_calls};
pub use functions::{DartFunctionPlan, insert_arc_dart_functions, prepare_arc_dart_functions};
pub use functions::prepare_arc_dart_module;
pub use tokens::{TokenEffect, TokenPlan, TokenReturn, verify_tokens, tokens_at_landing_pad_entry};
pub use scopes::{ScopeChange, ScopePlan, verify_scopes};
pub use classification::{
    OwnerOrigin, Ownership, classified_liveness, classified_liveness_with_exceptions,
};

/// Vivacidade dos valores classificados pelo chamador, só em blocos alcançáveis.
///
/// ```
/// use dartforge_emit_native::optimize::arc::Vivacidade;
/// let vazia = Vivacidade::default();
/// assert!(vazia.entrada.is_empty() && vazia.arestas.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Liveness {
    /// Antes das definições de `Phi` na entrada do bloco.
    pub entry: HashMap<BlockId, HashSet<ValueId>>,
    /// Depois do terminador, união das necessidades das arestas de saída.
    pub exit: HashMap<BlockId, HashSet<ValueId>>,
    /// Antes de cada instrução; entradas de `Phi` pertencem às arestas.
    pub before: HashMap<ValueId, HashSet<ValueId>>,
    /// Depois de cada instrução.
    pub after: HashMap<ValueId, HashSet<ValueId>>,
    /// Necessidade específica de `(predecessor, sucessor)`, incluindo apenas
    /// as entradas de `Phi` desse predecessor.
    pub edges: HashMap<(BlockId, BlockId), HashSet<ValueId>>,
}

struct Inventory<'a> {
    references: &'a HashSet<ValueId>,
    borrows: &'a HashMap<ValueId, ValueId>,
}

fn uses(o: &Operand, references: &Inventory<'_>, live_set: &mut HashSet<ValueId>) {
    if let Operand::Val(v) = o
        && references.references.contains(v)
    {
        let mut current = *v;
        loop {
            live_set.insert(current);
            let Some(&owner) = references.borrows.get(&current) else {
                break;
            };
            current = owner;
        }
    }
}

fn on_edge(
    f: &Function,
    origin: usize,
    dest: usize,
    entry: &[HashSet<ValueId>],
    references: &Inventory<'_>,
) -> HashSet<ValueId> {
    let mut live_set = entry[dest].clone();
    for (v, inst, _) in &f.blocks[dest].instructions {
        if let Instruction::Phi { incoming, .. } = inst {
            live_set.remove(v);
            for (de, op) in incoming {
                if *de == f.blocks[origin].id {
                    uses(op, references, &mut live_set);
                }
            }
        }
    }
    live_set
}

fn at_exit(
    f: &Function,
    cfg: &Cfg,
    b: usize,
    entry: &[HashSet<ValueId>],
    references: &Inventory<'_>,
) -> HashSet<ValueId> {
    let mut live_set = HashSet::new();
    for &s in &cfg.successors[b] {
        live_set.extend(on_edge(f, b, s, entry, references));
    }
    live_set
}

fn retreat(
    v: ValueId,
    inst: &Instruction,
    references: &Inventory<'_>,
    live_set: &mut HashSet<ValueId>,
) {
    live_set.remove(&v);
    if !matches!(inst, Instruction::Phi { .. }) {
        operands(inst, &mut |o| uses(o, references, live_set));
    }
}

/// Calcula vivacidade reversa em ponto fixo, com `Phi` por predecessor.
///
/// A função deve ter CFG/SSA válidos. `references` vem da classificação
/// semântica: não deduzimos ownership de `Type::I64` nem de `Type::Ptr`.
/// Incluir aliases com proveniência gerenciada e slots proprietários, quando
/// classificados. Dependências de borrows e extensões de vida precisam ser
/// materializadas como usos antes desta análise. No modo tabelas, executar
/// depois da preparação excepcional. Conjuntos não têm ordem de emissão;
/// ordenar IDs antes de materializar operações para manter determinismo.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::vivacidade};
/// use std::collections::HashSet;
/// let f = Function {
///     symbol: "identidade".into(), name: "identidade".into(),
///     params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, depuracao: None,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
/// };
/// let vivos = vivacidade(&f, &HashSet::from([ValueId(0)]));
/// assert!(vivos.entrada[&BlockId(0)].contains(&ValueId(0)));
/// assert!(vivos.saida[&BlockId(0)].is_empty());
/// ```
pub fn liveness(f: &Function, references: &HashSet<ValueId>) -> Liveness {
    analyze(
        f,
        &Inventory {
            references,
            borrows: &HashMap::new(),
        },
    )
}

/// Dependência de empréstimo inválida, com origem e caminho (`ARC003`).
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, optimize::arc::ErroEmprestimo};
/// let erro = ErroEmprestimo { funcao: "f".into(), origem: ValueId(2),
///     caminho: vec![ValueId(2), ValueId(2)], motivo: "dependência cíclica" };
/// assert!(erro.to_string().starts_with("ARC003"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowError {
    /// Símbolo da função analisada.
    pub function: String,
    /// Alias que iniciou a verificação desta cadeia.
    pub origin: ValueId,
    /// Cadeia de aliases/owners; um ciclo termina repetindo seu primeiro ID.
    pub path: Vec<ValueId>,
    /// Razão da rejeição do inventário.
    pub reason: &'static str,
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ARC003 em {}: v{}: {}; caminho {:?}",
            self.function, self.origin.0, self.reason, self.path
        )
    }
}

impl std::error::Error for BorrowError {}

/// Inclui a cadeia de sustentação de cada alias borrowed nos seus usos.
///
/// `borrows` associa alias a owner imediato; cadeias incluem também os
/// aliases intermediários. Vale o contrato de CFG/SSA de [`liveness`].
/// Escopos, dominância e escape ainda precisam do verificador de ownership.
/// A análise de `Phi` trata cada entrada na aresta do seu predecessor;
/// transferências de tokens devem usar essas arestas, não pontos entre `Phi`.
///
/// # Erros
/// Retorna `ARC003` se alias/owner não existe na função ou no inventário,
/// ou se a cadeia de sustentação tem ciclo. Verifica em ordem de ID para
/// produzir o mesmo primeiro diagnóstico independentemente da ordem do mapa.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::vivacidade_com_emprestimos};
/// use std::collections::{HashMap, HashSet};
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "owner".into(), Type::Ref),
///                  (ValueId(1), "alias".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
/// let refs = HashSet::from([ValueId(0), ValueId(1)]);
/// let v = vivacidade_com_emprestimos(&f, &refs, &HashMap::from([(ValueId(1), ValueId(0))]))?;
/// assert_eq!(v.entrada[&BlockId(0)], refs);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn liveness_with_borrows(
    f: &Function,
    references: &HashSet<ValueId>,
    borrows: &HashMap<ValueId, ValueId>,
) -> Result<Liveness, BorrowError> {
    let existing_set: HashSet<ValueId> = f
        .params
        .iter()
        .map(|(v, _, _)| *v)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| *v)),
        )
        .collect();
    let mut origins: Vec<ValueId> = borrows.keys().copied().collect();
    origins.sort_by_key(|v| v.0);
    let error = |origin, path, reason| BorrowError {
        function: f.symbol.clone(),
        origin,
        path,
        reason,
    };
    for &alias in &origins {
        let owner = borrows[&alias];
        if [alias, owner]
            .iter()
            .any(|v| !references.contains(v) || !existing_set.contains(v))
        {
            return Err(error(
                alias,
                vec![alias, owner],
                "alias/owner ausente da função ou do inventário",
            ));
        }
    }
    let mut verified_set = HashSet::new();
    for origin in origins {
        let mut path = Vec::new();
        let mut positions = HashMap::new();
        let mut current = origin;
        while !verified_set.contains(&current) {
            if positions.insert(current, path.len()).is_some() {
                path.push(current);
                return Err(error(origin, path, "dependência cíclica"));
            }
            path.push(current);
            let Some(&owner) = borrows.get(&current) else {
                break;
            };
            current = owner;
        }
        verified_set.extend(path);
    }
    Ok(analyze(
        f,
        &Inventory {
            references,
            borrows,
        },
    ))
}

fn analyze(f: &Function, references: &Inventory<'_>) -> Liveness {
    let cfg = Cfg::new(f);
    let mut entry = vec![HashSet::new(); f.blocks.len()];
    let mut queue: Vec<usize> = cfg.rpo.clone();
    let mut in_queue = vec![false; f.blocks.len()];
    for &b in &queue {
        in_queue[b] = true;
    }
    while let Some(b) = queue.pop() {
        in_queue[b] = false;
        let mut live_set = at_exit(f, &cfg, b, &entry, references);
        terminator_operands(&f.blocks[b].terminator, &mut |o| {
            uses(o, references, &mut live_set)
        });
        for (v, inst, _) in f.blocks[b].instructions.iter().rev() {
            retreat(*v, inst, references, &mut live_set);
        }
        if live_set != entry[b] {
            entry[b] = live_set;
            for &p in &cfg.predecessors[b] {
                if cfg.reachable(p) && !in_queue[p] {
                    queue.push(p);
                    in_queue[p] = true;
                }
            }
        }
    }
    let mut result = Liveness::default();
    for &b in &cfg.rpo {
        let block = &f.blocks[b];
        let mut live_set = at_exit(f, &cfg, b, &entry, references);
        result.entry.insert(block.id, entry[b].clone());
        result.exit.insert(block.id, live_set.clone());
        for &s in &cfg.successors[b] {
            result.edges.insert(
                (block.id, f.blocks[s].id),
                on_edge(f, b, s, &entry, references),
            );
        }
        terminator_operands(&block.terminator, &mut |o| uses(o, references, &mut live_set));
        for (v, inst, _) in block.instructions.iter().rev() {
            result.after.insert(*v, live_set.clone());
            retreat(*v, inst, references, &mut live_set);
            result.before.insert(*v, live_set.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests;

mod resumes;
pub(crate) use resumes::{check as check_resumes, resume_instruction};
