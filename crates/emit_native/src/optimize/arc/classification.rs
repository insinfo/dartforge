//! Typed ownership inventory supplied by the HIR producers.
//! Coverage and dependencies are checked before liveness;
//! extern contracts, provenance, scope and consumption require other passes.

use super::super::cfg::Cfg;
use super::{Liveness, liveness_with_borrows};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Who sustains a borrow: the caller or a value of this function.
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, optimize::arc::OwnerOrigin};
/// let origin = OwnerOrigin::Value(ValueId(3));
/// assert_ne!(origin, OwnerOrigin::Caller);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerOrigin {
    /// A parameter borrowed from the caller during the invocation.
    Caller,
    /// A managed value or a local owner slot, with explicit provenance.
    Value(ValueId),
}

/// Semantic classification of an HIR definition (§20.1).
///
/// `Owned` describes the obligation of a token, not an already emitted increment.
/// The scope number is an identity; its limits still need to be
/// described and checked by the borrow verifier.
///
/// ```
/// use dartforge_emit_native::optimize::arc::{Ownership, OwnerOrigin};
/// let parameter = Ownership::Borrowed { owner: OwnerOrigin::Caller, scope: 0 };
/// assert_ne!(parameter, Ownership::Owned);
/// assert_ne!(parameter, Ownership::Trivial);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Has no managed ownership obligation.
    Trivial,
    /// Defines an owner that must be consumed, moved or returned.
    Owned,
    /// An alias sustained by an owner during a scope.
    Borrowed {
        /// Origin of the sustainment, which does not create an independent token.
        owner: OwnerOrigin,
        /// Identity of the borrow scope.
        scope: u32,
    },
}

/// Consumes a complete inventory and includes borrowed dependencies in the liveness.
///
/// The classification must come from the contracts and from the semantic provenance,
/// even when the representation is `I64` or a `Ptr` slot. The function
/// checks coverage, the external origin of parameters, the SSA dominance of local
/// owners in reachable blocks and the sustaining chains;
/// it does not prove the classification, the limits of the scopes nor the consumption of tokens.
/// The function must have a valid CFG/SSA. `Phi` definitions are simultaneous at the
/// entry of the block. The existence of the result of an `invoke` only on success
/// still has to be checked by the exceptional token analysis.
/// Include every definition and parameter, `Void` ones too as `Trivial`.
///
/// # Errors
/// Returns a diagnostic with the symbol and the ID if a classification is missing, there is a
/// strange ID, a local result claims a borrow from the caller, or the definition
/// of the owner does not dominate that of the alias. Invalid
/// dependencies keep the `ARC003` diagnostic of [`liveness_with_borrows`].
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let classes = HashMap::from([(ValueId(0), Ownership::Borrowed {
///     owner: OwnerOrigin::Caller, scope: 0 })]);
/// let v = classified_liveness(&f, &classes)?;
/// assert!(v.entry[&BlockId(0)].contains(&ValueId(0)));
/// # Ok::<(), String>(())
/// ```
pub fn classified_liveness(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<Liveness, String> {
    let parameters: HashSet<ValueId> = f.params.iter().map(|(v, _, _)| *v).collect();
    let existing_set: HashSet<ValueId> = parameters
        .iter()
        .copied()
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| *v)),
        )
        .collect();
    let mut ids: Vec<ValueId> = existing_set.iter().copied().collect();
    ids.sort_by_key(|v| v.0);
    for id in ids {
        if !classes.contains_key(&id) {
            let def = f
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .find(|(v, _, _)| *v == id)
                .map_or_else(
                    || {
                        format!(
                            "parameter {:?}",
                            f.params.iter().find(|(v, _, _)| *v == id).map(|p| p.2)
                        )
                    },
                    |(_, i, t)| format!("{i:?}: {t:?}"),
                );
            return Err(format!(
                "ownership in {}: v{} without classification ({def})",
                f.symbol, id.0
            ));
        }
    }
    let mut ids: Vec<ValueId> = classes.keys().copied().collect();
    ids.sort_by_key(|v| v.0);
    // The local definitions of type Ref (a trivial one of these is null, a constant
    // or a Phi of them); parameters do not count.
    let ref_types: HashSet<ValueId> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|(_, _, t)| *t == Type::Ref)
        .map(|(v, _, _)| *v)
        .collect();
    let mut references = HashSet::new();
    let mut borrows = HashMap::new();
    for id in ids {
        if !existing_set.contains(&id) {
            return Err(format!(
                "ownership in {}: v{} does not exist in the function",
                f.symbol, id.0
            ));
        }
        match classes[&id] {
            Ownership::Trivial => {}
            Ownership::Owned => {
                references.insert(id);
            }
            Ownership::Borrowed { owner, .. } => {
                references.insert(id);
                match owner {
                    OwnerOrigin::Caller if !parameters.contains(&id) => {
                        return Err(format!(
                            "ownership in {}: local v{} is not a parameter borrowed from the caller",
                            f.symbol, id.0
                        ));
                    }
                    OwnerOrigin::Caller => {}
                    // Borrowed from a trivial value (null or a permanent
                    // constant, including through a Phi of only nulls): nothing to
                    // keep alive.
                    OwnerOrigin::Value(owner)
                        if classes.get(&owner) == Some(&Ownership::Trivial)
                            && ref_types.contains(&owner) => {}
                    OwnerOrigin::Value(owner) => {
                        borrows.insert(id, owner);
                    }
                }
            }
        }
    }
    check_dominance(f, &borrows)?;
    liveness_with_borrows(f, &references, &borrows).map_err(|e| e.to_string())
}

/// `(block, position, phi)`: parameters stay outside the blocks and dominate
/// their instructions; phis are produced simultaneously at the entry.
fn check_dominance(f: &Function, borrows: &HashMap<ValueId, ValueId>) -> Result<(), String> {
    let cfg = Cfg::new(f);
    let mut definitions: HashMap<ValueId, (Option<usize>, usize, bool)> = f
        .params
        .iter()
        .map(|(v, _, _)| (*v, (None, 0, false)))
        .collect();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            definitions.insert(*v, (Some(bi), i, matches!(inst, Instruction::Phi { .. })));
        }
    }
    let mut aliases: Vec<ValueId> = borrows.keys().copied().collect();
    aliases.sort_by_key(|v| v.0);
    for alias in aliases {
        let owner = borrows[&alias];
        // Missing IDs get the diagnostic of the dependency validation.
        let (Some(&(b_alias, i_alias, phi_alias)), Some(&(b_owner, i_owner, phi_owner))) =
            (definitions.get(&alias), definitions.get(&owner))
        else {
            continue;
        };
        let dominates = match (b_owner, b_alias) {
            (None, _) => true,
            (Some(_), None) => false,
            (_, Some(b)) if !cfg.reachable(b) => true,
            (Some(o), Some(b)) if o == b => {
                if phi_alias {
                    phi_owner
                } else {
                    phi_owner || i_owner < i_alias
                }
            }
            (Some(o), Some(mut b)) => loop {
                if o == b {
                    break true;
                }
                if b == cfg.idom[b] {
                    break false;
                }
                b = cfg.idom[b];
            },
        };
        if !dominates {
            let block = b_alias.map_or_else(
                || "parameter".into(),
                |b| format!("b{} instruction {}", f.blocks[b].id.0, i_alias),
            );
            return Err(format!(
                "ARC003 in {}: {block}: v{alias_id} depends on owner v{owner_id} whose definition does not dominate the alias; path [{alias_id}, {owner_id}]",
                f.symbol,
                alias_id = alias.0,
                owner_id = owner.0
            ));
        }
    }
    Ok(())
}

/// Also checks that owners produced by an `invoke` exist on the path of the alias.
///
/// Requires a valid CFG/SSA and the exceptional plan of the same prepared HIR.
/// Availability requires
/// dominance of the success edge, including when the landing pad and the success join.
/// Direct uses of the result are also checked; Phi entries use
/// the availability on their edge, not that of the destination block.
/// Does not prove consumption of tokens nor results of calls without an `invoke`.
///
/// # Errors
/// Returns an error if the plan does not match the prepared shape, or if an alias
/// can be defined without going through the success of its local owner.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// classified_liveness_with_exceptions(&f, &HashMap::new(), &FunctionTables::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn classified_liveness_with_exceptions(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
) -> Result<Liveness, String> {
    liveness_with_exits(f, classes, tables, &HashMap::new())
}

pub(super) fn liveness_with_exits(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    pendings: &HashMap<ValueId, BlockId>,
) -> Result<Liveness, String> {
    let v = classified_liveness(f, classes)?;
    let cfg = Cfg::new(f);
    let pos: HashMap<BlockId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let defs: HashMap<ValueId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| b.instructions.iter().map(move |(v, _, _)| (*v, i)))
        .collect();
    if pendings.keys().any(|v| tables.invocations.contains_key(v)) {
        return Err(format!(
            "ownership in {}: pending exit duplicated as invoke",
            f.symbol
        ));
    }
    let mut calls: Vec<_> = tables.invocations.iter().chain(pendings).collect();
    calls.sort_by_key(|(v, _)| v.0);
    if calls.is_empty() {
        return Ok(v);
    }
    let uses = ResultUses::new(f, classes, &pos, &calls);
    for (&call, &landing_pad) in calls {
        let Some(&origin) = defs.get(&call) else {
            return Err(format!(
                "ownership in {}: missing invoke v{}",
                f.symbol, call.0
            ));
        };
        let b = &f.blocks[origin];
        if tables.invocations.contains_key(&call)
            && b.instructions
                .iter()
                .any(|(v, inst, _)| *v == call && matches!(inst, Instruction::CallRuntime { .. }))
        {
            return Err(format!(
                "ownership in {}: runtime v{} requires a pending check, not an LLVM invoke",
                f.symbol, call.0
            ));
        }
        let success = if pendings.contains_key(&call) {
            check_pending(b, call, landing_pad).and_then(|s| pos.get(&s).copied())
        } else {
            match &b.terminator {
                Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block,
                    else_block,
                } if *then_block == landing_pad
                    && *else_block != landing_pad
                    && b.instructions.last().is_some_and(|(v, _, _)| *v == call)
                    && tables.landing_pads.contains(&landing_pad) =>
                {
                    pos.get(else_block).copied()
                }
                _ => None,
            }
        }
        .ok_or_else(|| {
            format!(
                "ownership in {}: invoke v{} without a valid success edge",
                f.symbol, call.0
            )
        })?;
        // Call shape and full classification are checked even for unused
        // results. The local index contains every direct use and borrow link;
        // any transitive borrow dependency necessarily includes a direct link.
        let aliases = uses.aliases.get(&call).map_or(&[][..], Vec::as_slice);
        let direct = uses.direct.get(&call).map_or(&[][..], Vec::as_slice);
        if aliases.is_empty() && direct.is_empty() {
            continue;
        }
        let without_success = reachable_without_edge_dense(&cfg, (origin, success));
        for &alias in aliases {
            if let Some(&block) = defs.get(&alias)
                && cfg.reachable(block)
                && without_success[block]
            {
                return Err(format!(
                    "ARC003 in {}: b{}: v{} depends on result v{} unavailable on the exceptional path; path [{}, {}]",
                    f.symbol, f.blocks[block].id.0, alias.0, call.0, alias.0, call.0
                ));
            }
        }
        for site in direct {
            if !cfg.reachable(site.block) {
                continue;
            }
            let invalid = match site.predecessor {
                Some(pred) => {
                    cfg.reachable(pred)
                        && without_success[pred]
                        && (pred, site.block) != (origin, success)
                }
                None => without_success[site.block],
            };
            if invalid {
                return Err(format!(
                    "ARC003 in {}: b{}: result v{} used without crossing its success edge",
                    f.symbol, f.blocks[site.block].id.0, call.0
                ));
            }
        }
    }
    Ok(v)
}

/// Checks the suffix that reads the pending state without creating an LLVM landing pad.
pub(super) fn check_pending(b: &BasicBlock, call: ValueId, error: BlockId) -> Option<BlockId> {
    let n = b.instructions.len();
    if n < 3 {
        return None;
    }
    let (v, call_inst, _) = &b.instructions[n - 3];
    let (p, read, ty_p) = &b.instructions[n - 2];
    let (c, comparison, ty_c) = &b.instructions[n - 1];
    if *v != call
        || !matches!(
            call_inst,
            Instruction::CallRuntime { .. }
                | Instruction::CallStatic { .. }
                | Instruction::CallSelectorForward {
                    arc_owned: true,
                    ..
                }
                | Instruction::CallClosureForward {
                    arc_owned: true,
                    ..
                }
                | Instruction::TypedCall { .. }
                | Instruction::CallClosure { .. }
                | Instruction::CallSelector { .. }
                | Instruction::Unbox { to: Type::I1, .. }
        )
        || *ty_p != Type::I8
        || *ty_c != Type::I1
        || !matches!(read, Instruction::CallRuntime { name, args, ret_ty }
            if name == "dartforge_exception_pending" && args.is_empty() && *ret_ty == Type::I8)
        || !matches!(comparison, Instruction::ICmp(ICmpOp::Ne, Operand::Val(q), Operand::Constant(Constant::Int(0))) if q == p)
    {
        return None;
    }
    match &b.terminator {
        Terminator::CondBranch {
            cond: Operand::Val(q),
            then_block,
            else_block,
        } if q == c && *then_block == error && *else_block != error => Some(*else_block),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct UseSite {
    block: usize,
    predecessor: Option<usize>,
}

#[derive(Default)]
struct ResultUses {
    aliases: HashMap<ValueId, Vec<ValueId>>,
    direct: HashMap<ValueId, Vec<UseSite>>,
}

impl ResultUses {
    fn new(
        f: &Function,
        classes: &HashMap<ValueId, Ownership>,
        positions: &HashMap<BlockId, usize>,
        calls: &[(&ValueId, &BlockId)],
    ) -> Self {
        let calls: HashSet<_> = calls.iter().map(|(call, _)| **call).collect();
        let mut index = Self::default();
        for (&alias, class) in classes {
            if let Ownership::Borrowed {
                owner: OwnerOrigin::Value(owner),
                ..
            } = class
                && calls.contains(owner)
            {
                index.aliases.entry(*owner).or_default().push(alias);
            }
        }
        for aliases in index.aliases.values_mut() {
            aliases.sort_unstable_by_key(|v| v.0);
        }
        // Preserve block order: the first invalid use must produce the same
        // diagnostic, including Phi operands used on predecessor edges.
        for (block, b) in f.blocks.iter().enumerate() {
            let mut record = |operand: &Operand, predecessor| {
                if let Operand::Val(value) = operand
                    && calls.contains(value)
                {
                    index
                        .direct
                        .entry(*value)
                        .or_default()
                        .push(UseSite { block, predecessor });
                }
            };
            for (_, instruction, _) in &b.instructions {
                if let Instruction::Phi { incoming, .. } = instruction {
                    for (pred, operand) in incoming {
                        if let Some(&pred) = positions.get(pred) {
                            record(operand, Some(pred));
                        }
                    }
                } else {
                    super::super::operands::operands(instruction, &mut |operand| {
                        record(operand, None)
                    });
                }
            }
            super::super::operands::terminator_operands(&b.terminator, &mut |operand| {
                record(operand, None)
            });
        }
        index
    }
}

fn reachable_without_edge_dense(cfg: &Cfg, removed: (usize, usize)) -> Vec<bool> {
    let mut seen = vec![false; cfg.successors.len()];
    let mut queue = if seen.is_empty() { vec![] } else { vec![0] };
    while let Some(block) = queue.pop() {
        if !seen[block] {
            seen[block] = true;
            queue.extend(
                cfg.successors[block]
                    .iter()
                    .copied()
                    .filter(|&successor| (block, successor) != removed),
            );
        }
    }
    seen
}

pub(super) fn reachable_without_edge(cfg: &Cfg, removed: (usize, usize)) -> HashSet<usize> {
    reachable_without_edge_dense(cfg, removed)
        .into_iter()
        .enumerate()
        .filter_map(|(block, seen)| seen.then_some(block))
        .collect()
}

#[cfg(test)]
mod availability_tests {
    use super::*;

    #[test]
    fn success_must_dominate_join_even_when_its_block_dominates() {
        let cfg = Cfg {
            successors: vec![vec![1, 2], vec![3], vec![1], vec![]],
            predecessors: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let remaining = reachable_without_edge(&cfg, (0, 1));
        assert!(remaining.contains(&1) && remaining.contains(&3));
        let cfg = Cfg {
            successors: vec![vec![1, 2], vec![3], vec![], vec![]],
            predecessors: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let remaining = reachable_without_edge(&cfg, (0, 1));
        assert!(!remaining.contains(&1) && !remaining.contains(&3));
    }
}

#[cfg(test)]
mod speed_tests;
