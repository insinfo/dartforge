//! Check of the borrow limits in the prepared CFG, including edges.
//! The lowering is expected to supply the plan; we do not infer scopes by liveness.

use super::super::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use super::{OwnerOrigin, Ownership, classified_liveness};
use crate::hir::*;
use std::collections::{HashMap, VecDeque};

/// Lexical opening or closing; zero designates the whole invocation.
///
/// ```
/// use dartforge_emit_native::optimize::arc::ScopeChange;
/// let open = ScopeChange::Open(1);
/// assert_ne!(open, ScopeChange::Close(1));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeChange {
    /// Enters a scope that is not active yet.
    Open(u32),
    /// Leaves exactly the innermost scope.
    Close(u32),
}

/// Explicit limits in the same HIR as the ownership inventory.
///
/// Changes happen before the instruction, before the terminator and on the edge,
/// in that order. Edges allow representing cleanup without assigning it to all
/// successors. Scope zero is always active and cannot be closed.
///
/// ```
/// use dartforge_emit_native::optimize::arc::ScopePlan;
/// let plan = ScopePlan::default();
/// assert!(plan.edges.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopePlan {
    /// Changes before a definition identified by the ValueId.
    pub before: HashMap<ValueId, Vec<ScopeChange>>,
    /// Changes before the terminator of the block.
    pub exits: HashMap<BlockId, Vec<ScopeChange>>,
    /// Changes after the terminator and before the Phis of the successor.
    pub edges: HashMap<(BlockId, BlockId), Vec<ScopeChange>>,
}

fn change(stack: &mut Vec<u32>, events: &[ScopeChange]) -> Result<(), String> {
    for event in events {
        match *event {
            ScopeChange::Open(e) if !stack.contains(&e) => stack.push(e),
            ScopeChange::Close(e) if e != 0 && stack.last() == Some(&e) => {
                stack.pop();
            }
            _ => return Err(format!("invalid limit {event:?}; active {stack:?}")),
        }
    }
    Ok(())
}

fn check(
    mut v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    stack: &[u32],
) -> Result<(), String> {
    // The inventory was already checked: the chain has an origin and no cycles.
    while let Some(Ownership::Borrowed { scope, owner }) = classes.get(&v) {
        if !stack.contains(scope) {
            return Err(format!(
                "v{} borrowed from {owner:?} outside scope {scope}; active {stack:?}",
                v.0
            ));
        }
        match owner {
            OwnerOrigin::Value(next_item) => v = *next_item,
            OwnerOrigin::Caller => break,
        }
    }
    Ok(())
}

/// Rebuilds the path of the discovery tree only on error, without keeping
/// a copy of every prefix in each block.
fn path(f: &Function, parents: &[Option<usize>], mut b: usize) -> Vec<u32> {
    let mut path = vec![f.blocks[b].id.0];
    while let Some(p) = parents[b] {
        b = p;
        path.push(f.blocks[b].id.0);
    }
    path.reverse();
    path
}

fn error_in_flow(
    f: &Function,
    parents: &[Option<usize>],
    b: usize,
    edge: Option<usize>,
    point: String,
    reason: String,
) -> String {
    let mut path = path(f, parents, b);
    if let Some(s) = edge {
        path.push(f.blocks[s].id.0);
    }
    format!(
        "ARC003 in {}: {point}: {reason}; path {path:?}",
        f.symbol
    )
}

/// Checks borrow limits and uses with equal stacks at joins.
///
/// Requires a valid CFG/SSA and an explicit plan of the same function. The zero scopes of the
/// parameters last for the invocation. A Phi reads its entry after the events of its
/// edge. Each use also checks the transitive sustaining chain.
/// This analysis does not prove consumption of owners, the type contract,
/// transfer on return nor exceptional availability of an invoke.
///
/// # Errors
/// Returns ARC003 with the function and the point if there is a stale limit, a close
/// out of order, different stacks at a join, a scope open at the exit or a
/// borrowed definition/use outside its scope. Ignores unreachable flow,
/// but rejects plan keys absent from the HIR.
/// Flow errors include a path from the entry; joins include
/// the two paths that present distinct stacks.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), params: vec![],
///     return_ty: Type::Void, debug_info: None, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// verify_scopes(&f, &HashMap::new(), &ScopePlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_scopes(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &ScopePlan,
) -> Result<(), String> {
    analyze(f, Some(classes), plan).map(|_| ())
}

/// Rebuilds lexical stacks at call sites without inventing ownership
/// classes. The verification of borrowed uses stays in the full pass.
pub(super) fn stacks_at_calls(
    f: &Function,
    plan: &ScopePlan,
) -> Result<HashMap<ValueId, Vec<u32>>, String> {
    analyze(f, None, plan)
}

fn analyze(
    f: &Function,
    classes: Option<&HashMap<ValueId, Ownership>>,
    plan: &ScopePlan,
) -> Result<HashMap<ValueId, Vec<u32>>, String> {
    if let Some(classes) = classes {
        classified_liveness(f, classes)?;
    }
    let check_value = |v: ValueId, stack: &[u32]| {
        classes.map_or(Ok(()), |classes| check(v, classes, stack))
    };
    let mut stacks = HashMap::new();
    let cfg = Cfg::new(f);
    let pos: HashMap<_, _> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter().map(|(v, _, _)| (*v, b.id)))
        .collect();
    let error = |point: String, reason: String| format!("ARC003 in {}: {point}: {reason}", f.symbol);
    let mut ids: Vec<_> = plan.before.keys().copied().collect();
    ids.sort_by_key(|v| v.0);
    for id in ids {
        if !defs.contains_key(&id) {
            return Err(error(format!("v{}", id.0), "limit without an instruction".into()));
        }
    }
    let mut ids: Vec<_> = plan.exits.keys().copied().collect();
    ids.sort_by_key(|b| b.0);
    for id in ids {
        if !pos.contains_key(&id) {
            return Err(error(format!("b{}", id.0), "limit without a block".into()));
        }
    }
    let mut edges: Vec<_> = plan.edges.keys().copied().collect();
    edges.sort_by_key(|(a, b)| (a.0, b.0));
    for (of, to) in edges {
        if !pos
            .get(&of)
            .zip(pos.get(&to))
            .is_some_and(|(&a, &b)| cfg.successors[a].contains(&b))
        {
            return Err(error(
                format!("b{} -> b{}", of.0, to.0),
                "limit without an edge".into(),
            ));
        }
    }
    for (v, _, _) in &f.params {
        check_value(*v, &[0]).map_err(|m| error("parameter".into(), m))?;
    }
    if f.blocks.is_empty() {
        return Ok(stacks);
    }
    let mut entries: Vec<Option<Vec<u32>>> = vec![None; f.blocks.len()];
    let mut parents = vec![None; f.blocks.len()];
    entries[0] = Some(vec![0]);
    let mut queue = VecDeque::from([0]);
    while let Some(i) = queue.pop_front() {
        let b = &f.blocks[i];
        let mut stack = entries[i].clone().unwrap();
        let point = |v: ValueId| format!("b{} v{}", b.id.0, v.0);
        for (v, inst, _) in &b.instructions {
            change(&mut stack, plan.before.get(v).map_or(&[], Vec::as_slice))
                .map_err(|m| error_in_flow(f, &parents, i, None, point(*v), m))?;
            if classes.is_none() && matches!(inst, Instruction::CallStatic { .. }) {
                stacks.insert(*v, stack.clone());
            }
            check_value(*v, &stack)
                .map_err(|m| error_in_flow(f, &parents, i, None, point(*v), m))?;
            if !matches!(inst, Instruction::Phi { .. }) {
                let mut invalid = None;
                operands(inst, &mut |o| {
                    if invalid.is_none()
                        && let Operand::Val(v) = o
                    {
                        invalid = check_value(*v, &stack).err();
                    }
                });
                if let Some(m) = invalid {
                    return Err(error_in_flow(f, &parents, i, None, point(*v), m));
                }
            }
        }
        change(
            &mut stack,
            plan.exits.get(&b.id).map_or(&[], Vec::as_slice),
        )
        .map_err(|m| error_in_flow(f, &parents, i, None, format!("b{} exit", b.id.0), m))?;
        let mut invalid = None;
        terminator_operands(&b.terminator, &mut |o| {
            if invalid.is_none()
                && let Operand::Val(v) = o
            {
                invalid = check_value(*v, &stack).err();
            }
        });
        if let Some(m) = invalid {
            return Err(error_in_flow(
                f,
                &parents,
                i,
                None,
                format!("b{} terminator", b.id.0),
                m,
            ));
        }
        if cfg.successors[i].is_empty() && stack != [0] {
            return Err(error_in_flow(
                f,
                &parents,
                i,
                None,
                format!("b{} exit", b.id.0),
                format!("unclosed scopes {stack:?}"),
            ));
        }
        for &s in &cfg.successors[i] {
            let dest = &f.blocks[s];
            let point = format!("b{} -> b{}", b.id.0, dest.id.0);
            let mut next_one = stack.clone();
            change(
                &mut next_one,
                plan
                    .edges
                    .get(&(b.id, dest.id))
                    .map_or(&[], Vec::as_slice),
            )
            .map_err(|m| error_in_flow(f, &parents, i, Some(s), point.clone(), m))?;
            for (_, inst, _) in &dest.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (of, o) in incoming {
                        if *of == b.id
                            && let Operand::Val(v) = o
                        {
                            check_value(*v, &next_one).map_err(|m| {
                                error_in_flow(f, &parents, i, Some(s), point.clone(), m)
                            })?;
                        }
                    }
                }
            }
            match &entries[s] {
                Some(previous) if *previous != next_one => {
                    return Err(error_in_flow(
                        f,
                        &parents,
                        i,
                        Some(s),
                        point,
                        format!(
                            "join with different scopes {previous:?} and {next_one:?}; previous path {:?}",
                            path(f, &parents, s)
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    parents[s] = Some(i);
                    entries[s] = Some(next_one);
                    queue.push_back(s);
                }
            }
        }
    }
    Ok(stacks)
}

#[cfg(test)]
mod tests {
    use super::super::OwnerOrigin;
    use super::*;

    fn example() -> (Function, HashMap<ValueId, Ownership>, ScopePlan) {
        let f = Function {
            symbol: "escopos".into(),
            name: "escopos".into(),
            debug_info: None,
            params: vec![(ValueId(0), "dono".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(0)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let classes = HashMap::from([
            (
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            ),
            (
                ValueId(1),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(0)),
                    scope: 1,
                },
            ),
            (ValueId(2), Ownership::Owned),
        ]);
        let plan = ScopePlan {
            before: HashMap::from([(ValueId(1), vec![ScopeChange::Open(1)])]),
            exits: HashMap::from([(BlockId(0), vec![ScopeChange::Close(1)])]),
            ..Default::default()
        };
        (f, classes, plan)
    }

    #[test]
    fn borrow_cannot_be_defined_or_used_outside_limit() {
        let (f, classes, mut plan) = example();
        verify_scopes(&f, &classes, &plan).unwrap();
        plan.exits.clear();
        plan
            .before
            .insert(ValueId(2), vec![ScopeChange::Close(1)]);
        assert!(
            verify_scopes(&f, &classes, &plan)
                .unwrap_err()
                .contains("v1 borrowed")
        );
        plan.before.clear();
        assert!(
            verify_scopes(&f, &classes, &plan)
                .unwrap_err()
                .contains("b0 v1")
        );
    }

    #[test]
    fn transitive_borrow_does_not_extend_owner_limit() {
        let (mut f, mut classes, plan) = example();
        f.return_ty = Type::Ref;
        classes.insert(
            ValueId(2),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(1)),
                scope: 0,
            },
        );
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let error = verify_scopes(&f, &classes, &plan).unwrap_err();
        assert!(
            error.contains("b0 terminator")
                && error.contains("v1 borrowed")
                && error.contains("path [0]"),
            "{error}"
        );
    }

    #[test]
    fn edge_cleanup_must_agree_at_join_and_backedge() {
        let (mut f, classes, mut plan) = example();
        plan.exits.clear();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for i in 1..=3 {
            f.blocks.push(BasicBlock {
                id: BlockId(i),
                instructions: vec![],
                terminator: if i == 3 {
                    Terminator::Return(None)
                } else {
                    Terminator::Branch(BlockId(3))
                },
            });
        }
        plan
            .edges
            .insert((BlockId(0), BlockId(1)), vec![ScopeChange::Close(1)]);
        plan
            .edges
            .insert((BlockId(0), BlockId(2)), vec![ScopeChange::Close(1)]);
        verify_scopes(&f, &classes, &plan).unwrap();
        plan.edges.remove(&(BlockId(0), BlockId(2)));
        let error = verify_scopes(&f, &classes, &plan).unwrap_err();
        assert!(
            error.contains("join")
                && error.contains("previous path [0, 1, 3]")
                && error.contains("path [0, 2, 3]"),
            "{error}"
        );
        // The back edge must close the previous activation before opening the next one.
        f.blocks.truncate(1);
        f.blocks[0].terminator = Terminator::Branch(BlockId(0));
        plan.edges.clear();
        assert!(
            verify_scopes(&f, &classes, &plan)
                .unwrap_err()
                .contains("join")
        );
        plan
            .edges
            .insert((BlockId(0), BlockId(0)), vec![ScopeChange::Close(1)]);
        verify_scopes(&f, &classes, &plan).unwrap();
    }

    #[test]
    fn phi_uses_borrow_after_edge_cleanup_and_stale_plan_fails() {
        let (mut f, mut classes, mut plan) = example();
        plan.exits.clear();
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![(
                ValueId(3),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(None),
        });
        classes.insert(ValueId(3), Ownership::Owned);
        plan
            .exits
            .insert(BlockId(1), vec![ScopeChange::Close(1)]);
        verify_scopes(&f, &classes, &plan).unwrap();
        plan
            .edges
            .insert((BlockId(0), BlockId(1)), vec![ScopeChange::Close(1)]);
        assert!(
            verify_scopes(&f, &classes, &plan)
                .unwrap_err()
                .contains("b0 -> b1: v1")
        );
        plan.before.insert(ValueId(99), vec![]);
        assert!(
            verify_scopes(&f, &classes, &plan)
                .unwrap_err()
                .contains("limit without an instruction")
        );
    }
}
