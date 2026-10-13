//! Conservative barriers for aliases derived from local owners.
//! Protects Ref runtime results when a barrier requires Owned.
//! Does not prove slot aliases, produce loop cleanup nor certify versions/pins.

use super::super::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use super::{OwnerOrigin, Ownership};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn verify(f: &Function, classes: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    match first_invalid(f, classes)? {
        Some((_, message)) => Err(message),
        None => Ok(()),
    }
}

fn first_invalid(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<Option<(ValueId, String)>, String> {
    // A Phi whose entries are the caller's borrowed parameter itself,
    // another such Phi, null or trivial values is just an alias of the
    // parameter: nothing in this function invalidates it (the caller keeps the owner).
    // Values derived from the parameter (one of its fields) do not enter here.
    let mut caller_aliases: HashSet<ValueId> = f.params.iter()
        .filter(|(v, _, _)| matches!(classes.get(v), Some(Ownership::Borrowed { owner: OwnerOrigin::Caller, .. })))
        .map(|(v, _, _)| *v)
        .collect();
    loop {
        let new_set: Vec<ValueId> = f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, i, _)| {
            let Instruction::Phi { incoming, ty: Type::Ref } = i else { return None };
            (!caller_aliases.contains(v)
                && matches!(classes.get(v), Some(Ownership::Borrowed { .. }))
                && incoming.iter().all(|(_, op)| match op {
                    Operand::Constant(Constant::Null) => true,
                    Operand::Val(x) => caller_aliases.contains(x) || classes.get(x) == Some(&Ownership::Trivial),
                    _ => false,
                })).then_some(*v)
        }).collect();
        if new_set.is_empty() {
            break;
        }
        caller_aliases.extend(new_set);
    }
    let derived_set: HashSet<_> = classes
        .iter()
        .filter_map(|(v, c)| {
            (matches!(
                c,
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(_),
                    ..
                }
            ) && !caller_aliases.contains(v))
            .then_some(*v)
        })
        .collect();
    if derived_set.is_empty() {
        return Ok(None);
    }
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let cfg = Cfg::new(f);
    let pos: HashMap<_, _> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let mut barriers = HashSet::new();
    let mut reentries = HashSet::new();
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let unbox_bool = super::contracts::unbox_bool_call(inst);
        let call = unbox_bool.as_ref().unwrap_or(inst);
        let invalidates = match call {
            Instruction::CallRuntime { name, .. } => {
                match super::contract(name) {
                    Ok(c) => {
                        if c.chama_dart {
                            reentries.insert(*v);
                        }
                        c.invalida_borrows
                    }
                    Err(_) => {
                        reentries.insert(*v);
                        true // A supplied contract does not prove the absence of invalidation.
                    }
                }
            }
            Instruction::Phi { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcLoadStrong { .. }
            | Instruction::Alloca(_)
            | Instruction::Load { .. }
            | Instruction::GetField { .. }
            | Instruction::EnvGet { .. } => false,
            Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => true,
            Instruction::CallStatic { .. }
            | Instruction::CallSelector { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallClosureForward { .. } => {
                reentries.insert(*v);
                true
            }
            Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => true,
            // The factories root elements/parts before allocating and do not reenter Dart.
            Instruction::AllocList { .. } | Instruction::ConcatStrings { .. } => true,
            _ => {
                let invalidates = !super::pure_set::check(*v, inst, ty, &types)?;
                if invalidates {
                    reentries.insert(*v);
                }
                invalidates
            }
        };
        if invalidates {
            barriers.insert(*v);
        }
    }
    let transfer = |bi: usize, mut state: HashSet<ValueId>| {
        for (v, _, _) in &f.blocks[bi].instructions {
            if barriers.contains(v) {
                state.extend(&derived_set);
            }
            // The definition is born after the effect of the call. A Phi is only valid
            // if each operand is valid on its own edge, checked below.
            state.remove(v);
        }
        state
    };
    let mut entries = vec![HashSet::new(); f.blocks.len()];
    let mut exits = entries.clone();
    loop {
        let mut changed = false;
        for &bi in &cfg.rpo {
            let mut state = HashSet::new();
            for &p in &cfg.predecessors[bi] {
                if cfg.idom[p] != usize::MAX {
                    state.extend(&exits[p]);
                }
            }
            let exit = transfer(bi, state.clone());
            changed |= entries[bi] != state || exits[bi] != exit;
            entries[bi] = state;
            exits[bi] = exit;
        }
        if !changed {
            break;
        }
    }
    let check = |op: &Operand, state: &HashSet<ValueId>, point: String| {
        if let Operand::Val(v) = op {
            let mut current = *v;
            let mut seen_set = HashSet::new();
            while seen_set.insert(current) {
                if state.contains(&current) {
                    return Some((
                        current,
                        format!(
                            "empréstimo invalidado em {}: {point}: v{} depende de v{}; copie para Owned antes da barreira",
                            f.symbol, v.0, current.0
                        ),
                    ));
                }
                match classes.get(&current) {
                    Some(Ownership::Borrowed {
                        owner: OwnerOrigin::Value(p),
                        ..
                    }) => current = *p,
                    _ => break,
                }
            }
        }
        None
    };
    for &bi in &cfg.rpo {
        let b = &f.blocks[bi];
        let mut state = entries[bi].clone();
        for (v, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (p, op) in incoming {
                    let pi = pos[p];
                    if cfg.idom[pi] != usize::MAX {
                        if let Some(error) = check(
                            op,
                            &exits[pi],
                            format!("b{} -> b{} Phi v{}", p.0, b.id.0, v.0),
                        ) {
                            return Ok(Some(error));
                        }
                    }
                }
            } else {
                // A call that may reenter Dart cannot depend on
                // a mutable edge, not even during its own execution.
                if reentries.contains(v) {
                    state.extend(&derived_set);
                }
                let mut error = None;
                operands(inst, &mut |op| {
                    if error.is_none() {
                        error = check(op, &state, format!("b{} v{}", b.id.0, v.0));
                    }
                });
                if let Some(error) = error {
                    return Ok(Some(error));
                }
            }
            if barriers.contains(v) {
                state.extend(&derived_set);
            }
            state.remove(v);
        }
        let mut error = None;
        terminator_operands(&b.terminator, &mut |op| {
            if error.is_none() {
                error = check(op, &state, format!("b{} saída", b.id.0));
            }
        });
        if let Some(error) = error {
            return Ok(Some(error));
        }
    }
    Ok(None)
}

// Operates only on the transactional copy of the function set. The checker
// stays read-only; this producer uses the same needs.
pub(super) fn protect(
    f: &mut Function,
    plan: &mut super::DartFunctionPlan,
) -> Result<usize, String> {
    let mut copies = 0;
    while let Some((value, message)) = first_invalid(f, &plan.classes)? {
        // A fresh Phi may be invalidated in its own join block.
        // Protects its borrowed origins; the rebuild below promotes the
        // Phi only when all entries transfer Owned tokens.
        let origins = origins_to_copy(f, &plan.classes, value, &mut HashSet::new())
            .ok_or_else(|| message.clone())?;
        if origins.is_empty() {
            return Err(message);
        }
        // Checks availability in the original body before the first
        // promotion of the batch. The intermediate classes of the Phis are still Borrowed.
        if !plan.tokens.pendings.is_empty() {
            super::classification::liveness_with_exits(
                f,
                &plan.classes,
                &plan.tables,
                &plan.tokens.pendings,
            )?;
        }
        for value in origins {
            let Some((bi, ii)) = f.blocks.iter().enumerate().find_map(|(bi, b)| {
                b.instructions
                    .iter()
                    .position(|(v, _, _)| *v == value)
                    .map(|ii| (bi, ii))
            }) else {
                return Err(message.clone());
            };
            let (_, inst, ty) = &f.blocks[bi].instructions[ii];
            let Instruction::CallRuntime { .. } = inst else {
                return Err(message.clone());
            };
            let contract = super::runtime_call_contract(inst)?;
            if *ty != Type::Ref
                || !matches!(contract.result, Ownership::Borrowed { .. })
                || plan.tables.invocations.contains_key(&value)
            {
                return Err(message.clone());
            }
            if plan.classes.get(&value) != Some(&contract.result) {
                return Err(message.clone());
            }
            let origin = f.blocks[bi].id;
            let continuation = if contract.effect.may_fail {
                let error = *plan
                    .tokens
                    .pendings
                    .get(&value)
                    .ok_or_else(|| message.clone())?;
                let normal = super::classification::check_pending(&f.blocks[bi], value, error)
                    .ok_or_else(|| message.clone())?;
                let largest_block = f
                    .blocks
                    .iter()
                    .map(|b| b.id.0)
                    .chain(plan.tables.exits.keys().map(|b| b.0))
                    .chain(plan.tables.landing_pads.iter().map(|b| b.0))
                    .chain(plan.tables.invocations.values().map(|b| b.0))
                    .chain(plan.tokens.pendings.values().map(|b| b.0))
                    .chain(plan.scopes.exits.keys().map(|b| b.0))
                    .chain(plan.scopes.edges.keys().flat_map(|(a, b)| [a.0, b.0]))
                    .max()
                    .unwrap_or(0);
                Some((
                    normal,
                    BlockId(
                        largest_block
                            .checked_add(1)
                            .ok_or("IDs de blocos esgotados ao proteger empréstimo")?,
                    ),
                ))
            } else {
                if plan.tokens.pendings.contains_key(&value) {
                    return Err(message);
                }
                None
            };
            let larger = f
                .params
                .iter()
                .map(|(v, _, _)| v.0)
                .chain(
                    f.blocks
                        .iter()
                        .flat_map(|b| b.instructions.iter().map(|(v, _, _)| v.0)),
                )
                .chain(plan.classes.keys().map(|v| v.0))
                .chain(plan.tokens.instructions.keys().map(|v| v.0))
                .chain(plan.tokens.pendings.keys().map(|v| v.0))
                .chain(plan.tables.invocations.keys().map(|v| v.0))
                .chain(plan.scopes.before.keys().map(|v| v.0))
                .max()
                .unwrap_or(0);
            let borrowed = ValueId(
                larger
                    .checked_add(1)
                    .ok_or("IDs SSA esgotados ao proteger empréstimo")?,
            );
            // Keeps the original ID as Owned: all uses and aliases come to
            // depend on the independent token, without remapping operators/edges.
            f.blocks[bi].instructions[ii].0 = borrowed;
            let copy_inst = (
                value,
                Instruction::ArcCopy {
                    value: Operand::Val(borrowed),
                },
                Type::Ref,
            );
            if let Some((normal, copy_block)) = continuation {
                let Terminator::CondBranch { else_block, .. } = &mut f.blocks[bi].terminator else {
                    unreachable!("guarda de pendência conferida antes da transformação")
                };
                *else_block = copy_block;
                for b in &mut f.blocks {
                    if b.id == normal {
                        for (_, inst, _) in &mut b.instructions {
                            if let Instruction::Phi { incoming, .. } = inst {
                                for (p, _) in incoming {
                                    if *p == origin {
                                        *p = copy_block;
                                    }
                                }
                            }
                        }
                    }
                }
                f.blocks.push(BasicBlock {
                    id: copy_block,
                    instructions: vec![copy_inst],
                    terminator: Terminator::Branch(normal),
                });
                // The copy sustains the result before the limits of the old edge.
                if let Some(limits) = plan.scopes.edges.remove(&(origin, normal)) {
                    plan.scopes.edges.insert((copy_block, normal), limits);
                }
                let error = plan.tokens.pendings.remove(&value).unwrap();
                plan.tokens.pendings.insert(borrowed, error);
            } else {
                f.blocks[bi].instructions.insert(ii + 1, copy_inst);
            }
            plan.classes.insert(borrowed, contract.result);
            plan.classes.insert(value, Ownership::Owned);
            plan.tokens.instructions.remove(&value);
            plan.tokens.instructions.insert(borrowed, contract.effect);
            if let Some(limits) = plan.scopes.before.remove(&value) {
                plan.scopes.before.insert(borrowed, limits);
            }
            copies += 1;
        }
        // The Phis were already checked in the original body. Rebuilds their
        // classification after the promotion, including the Owned transfer.
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if matches!(inst, Instruction::Phi { .. }) {
                plan.classes.remove(v);
            }
        }
        // A Phi that now receives an owner needs a copy of the other
        // entries (literals, borrows) on the edge.
        let classes = plan.classes.clone();
        copies += super::phis::copy_with_classes(f, plan, &classes)?;
        super::produce_arc_contracts(f, &mut plan.classes, &mut plan.tokens)?;
    }
    Ok(copies)
}

fn origins_to_copy(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    value: ValueId,
    seen_set: &mut HashSet<ValueId>,
) -> Option<Vec<ValueId>> {
    if !seen_set.insert(value) {
        return Some(Vec::new());
    }
    match classes.get(&value) {
        Some(Ownership::Owned | Ownership::Trivial) => return Some(Vec::new()),
        Some(Ownership::Borrowed { .. }) => {}
        _ => return None,
    }
    let (_, inst, _) = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|(v, _, _)| *v == value)?;
    if let Instruction::Phi { incoming, .. } = inst {
        let mut origins = Vec::new();
        for (_, op) in incoming {
            match op {
                Operand::Val(v) => origins.extend(origins_to_copy(f, classes, *v, seen_set)?),
                Operand::Constant(Constant::Null) => {}
                _ => return None,
            }
        }
        Some(origins)
    } else {
        Some(vec![value])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepalive_protects_invalidated_ancestor_instead_of_repeating_alias_copy() {
        let (mut f, mut classes) = function(vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                read(),
                swap(),
                (
                    ValueId(4),
                    Instruction::EnvGet {
                        env: Operand::Val(ValueId(0)),
                        index: 1,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(4)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(6),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(3)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }]);
        // The identity/provenance of this alias is an explicit premise. Its
        // ancestor loses the edge before the alias is materialized.
        classes.insert(ValueId(1), Ownership::Trivial);
        classes.insert(ValueId(3), Ownership::Owned);
        classes.insert(
            ValueId(4),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(2)),
                scope: 0,
            },
        );
        let mut plan = super::super::DartFunctionPlan {
            classes,
            ..Default::default()
        };
        plan
            .tokens
            .instructions
            .insert(ValueId(4), super::super::TokenEffect::default());
        assert_eq!(protect(&mut f, &mut plan).unwrap(), 1);
        assert_eq!(plan.classes[&ValueId(2)], Ownership::Owned);
        assert!(matches!(
            plan.classes[&ValueId(4)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(2)),
                ..
            }
        ));
        verify(&f, &plan.classes).unwrap();
        assert_eq!(protect(&mut f, &mut plan).unwrap(), 0);
    }

    #[test]
    fn abiless_origin_is_rejected_and_record_read_gets_copy() {
        for fallible in [false, true] {
            let (mut f, mut classes) = function(vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    if fallible {
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_nativo_DartForge_record_fieldAt".into(),
                                args: vec![
                                    (Operand::Val(ValueId(0)), Type::Ref),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        )
                    } else {
                        (
                            ValueId(2),
                            Instruction::EnvGet {
                                env: Operand::Val(ValueId(0)),
                                index: 0,
                            },
                            Type::Ref,
                        )
                    },
                    swap(),
                    (
                        ValueId(3),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(2)),
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }]);
            classes.insert(ValueId(3), Ownership::Owned);
            let mut plan = super::super::DartFunctionPlan {
                classes,
                ..Default::default()
            };
            let before = format!("{f:?}/{plan:?}");
            if fallible {
                // The read of the record does not fail: the copy comes right after it.
                assert_eq!(protect(&mut f, &mut plan).unwrap(), 1);
                assert!(matches!(f.blocks[0].instructions[1].1, Instruction::ArcCopy { .. }));
                continue;
            }
            assert!(
                protect(&mut f, &mut plan)
                    .unwrap_err()
                    .contains("empréstimo invalidado")
            );
            assert_eq!(format!("{f:?}/{plan:?}"), before);
        }
    }

    fn read() -> (ValueId, Instruction, Type) {
        (
            ValueId(2),
            Instruction::CallRuntime {
                name: "dartforge_arc_ler_campo_ref_v1".into(),
                args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    fn swap() -> (ValueId, Instruction, Type) {
        (
            ValueId(5),
            Instruction::CallRuntime {
                name: "dartforge_arc_gravar_campo_ref_v1".into(),
                args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Constant(Constant::Null), Type::Ref),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    }

    fn function(blocks: Vec<BasicBlock>) -> (Function, HashMap<ValueId, Ownership>) {
        (
            Function {
                symbol: "fluxo_borrow".into(),
                name: "fluxo_borrow".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "objeto".into(), Type::Ref),
                    (ValueId(1), "cond".into(), Type::I1),
                ],
                return_ty: Type::Void,
                blocks,
            },
            HashMap::from([
                (
                    ValueId(0),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Caller,
                        scope: 0,
                    },
                ),
                (
                    ValueId(2),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Value(ValueId(0)),
                        scope: 0,
                    },
                ),
                (
                    ValueId(4),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Value(ValueId(0)),
                        scope: 0,
                    },
                ),
            ]),
        )
    }

    #[test]
    fn phi_checks_each_edge_and_loop_does_not_revalidate_entry_read() {
        for invalidates in [false, true] {
            let (f, c) = function(vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![read()],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: if invalidates { vec![swap()] } else { vec![] },
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(2))),
                                (BlockId(2), Operand::Val(ValueId(2))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(None),
                },
            ]);
            let result = verify(&f, &c);
            if invalidates {
                assert!(result.unwrap_err().contains("b1 -> b3 Phi v4"));
            } else {
                result.unwrap();
            }
        }
        for reread in [false, true] {
            let mut body = vec![
                (
                    ValueId(3),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Ref,
                ),
                swap(),
            ];
            if reread {
                body.insert(0, read());
            }
            let (f, c) = function(vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: if reread { vec![] } else { vec![read()] },
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: body,
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ]);
            let result = verify(&f, &c);
            if reread {
                result.unwrap();
            } else {
                assert!(result.unwrap_err().contains("empréstimo invalidado"));
            }
        }
    }

    #[test]
    fn opaque_call_requires_owner_independent_of_derived_argument() {
        let (f, c) = function(vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                read(),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "callback".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }]);
        assert!(verify(&f, &c).unwrap_err().contains("b0 v3"));
    }
}
