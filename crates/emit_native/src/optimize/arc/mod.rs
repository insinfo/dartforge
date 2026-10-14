//! Analyses and initial insertion of ARC ownership in the HIR (§20 of the specification).
//!
//! Liveness receives the semantic inventory of references/slots; it does not
//! classify externs nor prove token consumption by itself. Producers,
//! verifiers and insertion into returns/exits exist as explicit passes;
//! they still need full coverage and integration into the default pipeline.

use super::cfg::Cfg;
use super::operands::{operands, terminator_operands};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// The prefix of the generic Owned thunk of a runtime function that returns a
/// `Ref` (`natives::prepare`, `contracts::generic_owned_contract`); the
/// LLVM emitter defines the thunk (`llvm/selectors.rs`).
pub const GENERIC_OWNED_PREFIX: &str = "df.arc.owned.";
/// The native scope thunk: the runtime function that may run Dart, with the
/// result and the contract of the base, inside a native scope (the tokens that
/// Dart ports return die at the close), defined by the emitter.
pub const NATIVE_SCOPE_PREFIX: &str = "df.arc.scope.";
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
pub(super) fn fold_fixed_subtype_tests(module: &mut Module) -> bool {
    fields::fold_fixed_subtype_tests(module)
}
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

/// Liveness of the values classified by the caller, only in reachable blocks.
///
/// ```
/// use dartforge_emit_native::optimize::arc::Liveness;
/// let empty = Liveness::default();
/// assert!(empty.entry.is_empty() && empty.edges.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Liveness {
    /// Before the `Phi` definitions at the entry of the block.
    pub entry: HashMap<BlockId, HashSet<ValueId>>,
    /// After the terminator, the union of the needs of the outgoing edges.
    pub exit: HashMap<BlockId, HashSet<ValueId>>,
    /// Before each instruction; `Phi` entries belong to the edges.
    pub before: HashMap<ValueId, HashSet<ValueId>>,
    /// After each instruction.
    pub after: HashMap<ValueId, HashSet<ValueId>>,
    /// The specific need of `(predecessor, successor)`, including only
    /// the `Phi` entries of that predecessor.
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
            for (of, op) in incoming {
                if *of == f.blocks[origin].id {
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

/// Computes reverse liveness as a fixpoint, with `Phi` per predecessor.
///
/// The function must have a valid CFG/SSA. `references` comes from the semantic
/// classification: we do not deduce ownership from `Type::I64` nor from `Type::Ptr`.
/// Include aliases with managed provenance and owner slots, when
/// classified. Borrow dependencies and lifetime extensions must be
/// materialized as uses before this analysis. In tables mode, run
/// after the exceptional preparation. Sets have no emission order;
/// sort IDs before materializing operations to keep determinism.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::liveness};
/// use std::collections::HashSet;
/// let f = Function {
///     symbol: "identity".into(), name: "identity".into(),
///     params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, debug_info: None,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
/// };
/// let live = liveness(&f, &HashSet::from([ValueId(0)]));
/// assert!(live.entry[&BlockId(0)].contains(&ValueId(0)));
/// assert!(live.exit[&BlockId(0)].is_empty());
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

/// An invalid borrow dependency, with origin and path (`ARC003`).
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, optimize::arc::BorrowError};
/// let error = BorrowError { function: "f".into(), origin: ValueId(2),
///     path: vec![ValueId(2), ValueId(2)], reason: "dependência cíclica" };
/// assert!(error.to_string().starts_with("ARC003"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowError {
    /// Symbol of the analyzed function.
    pub function: String,
    /// Alias that started the verification of this chain.
    pub origin: ValueId,
    /// Chain of aliases/owners; a cycle ends by repeating its first ID.
    pub path: Vec<ValueId>,
    /// Reason for rejecting the inventory.
    pub reason: &'static str,
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ARC003 in {}: v{}: {}; path {:?}",
            self.function, self.origin.0, self.reason, self.path
        )
    }
}

impl std::error::Error for BorrowError {}

/// Includes the sustaining chain of each borrowed alias in its uses.
///
/// `borrows` associates an alias with its immediate owner; chains also include the
/// intermediate aliases. The CFG/SSA contract of [`liveness`] applies.
/// Scopes, dominance and escape still need the ownership verifier.
/// The `Phi` analysis treats each entry on the edge of its predecessor;
/// token transfers must use those edges, not points between `Phi`s.
///
/// # Errors
/// Returns `ARC003` if an alias/owner does not exist in the function or in the inventory,
/// or if the sustaining chain has a cycle. Checks in ID order to
/// produce the same first diagnostic regardless of the map order.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::liveness_with_borrows};
/// use std::collections::{HashMap, HashSet};
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![(ValueId(0), "owner".into(), Type::Ref),
///                  (ValueId(1), "alias".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
/// let refs = HashSet::from([ValueId(0), ValueId(1)]);
/// let v = liveness_with_borrows(&f, &refs, &HashMap::from([(ValueId(1), ValueId(0))]))?;
/// assert_eq!(v.entry[&BlockId(0)], refs);
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
                "alias/owner absent from the function or the inventory",
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
                return Err(error(origin, path, "cyclic dependency"));
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
