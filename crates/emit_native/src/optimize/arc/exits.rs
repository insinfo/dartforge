//! Exposes the success and the unwinding of Guard returns before inserting tokens.
//! This pass changes only temporary copies in the transaction of the Dart set.

use super::*;

// Publishes the exception before releasing local owners. The Ref Throw goes through the
// audited ABI and through two exits with cleanup; Throws only unwinds after the drops.
// The pending protocol also works when the emitter does not use tables.
pub(super) fn prepare_ref_throws(
    f: &mut Function,
    plan: &mut DartFunctionPlan,
) -> Result<(), String> {
    super::ssa::verify(f)?;
    let types: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)))
        .collect();
    let throws: Vec<_> = f.blocks.iter().enumerate().filter_map(|(i, b)| {
        if let Terminator::Throw(o) = &b.terminator { Some((i, b.id, o.clone())) } else { None }
    }).collect();
    if throws.is_empty() { return Ok(()); }
    let mut value = types.keys().map(|v| v.0)
        .chain(plan.classes.keys().map(|v| v.0))
        .chain(plan.tokens.instructions.keys().map(|v| v.0))
        .chain(plan.tokens.pendings.keys().map(|v| v.0))
        .chain(plan.tables.invocations.keys().map(|v| v.0))
        .chain(plan.scopes.before.keys().map(|v| v.0))
        .max().map_or(0, |v| u64::from(v) + 1);
    let mut block = f.blocks.iter().map(|b| b.id.0)
        .chain(plan.tables.exits.keys().map(|b| b.0))
        .chain(plan.tables.landing_pads.iter().map(|b| b.0))
        .chain(plan.tables.invocations.values().map(|b| b.0))
        .chain(plan.tokens.pendings.values().map(|b| b.0))
        .chain(plan.scopes.exits.keys().map(|b| b.0))
        .chain(plan.scopes.edges.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max().map_or(0, |b| u64::from(b) + 1);
    let mut value_id = || -> Result<ValueId, String> {
        let id = u32::try_from(value).map_err(|_| "SSA IDs exhausted while preparing Throw")?;
        value += 1; Ok(ValueId(id))
    };
    let mut block_id = || -> Result<BlockId, String> {
        let id = u32::try_from(block).map_err(|_| "block IDs exhausted while preparing Throw")?;
        block += 1; Ok(BlockId(id))
    };
    for (i, origin, payload) in throws {
        if !matches!(&payload, Operand::Constant(Constant::Null))
            && !matches!(&payload, Operand::Val(v) if types.get(v) == Some(&Type::Ref)) {
            return Err(format!("{} b{}: Throw requires a prepared Ref payload", f.symbol, origin.0));
        }
        if plan.tables.exits.contains_key(&origin) {
            return Err(format!("{} b{}: Throw conflicts with the supplied exit", f.symbol, origin.0));
        }
        let call = value_id()?;
        let pending = value_id()?;
        let cond = value_id()?;
        let error = block_id()?;
        let normal = block_id()?;
        f.blocks[i].instructions.extend([
            (call, Instruction::CallRuntime { name: "dartforge_arc_throw_ref_v1".into(),
                args: vec![(payload, Type::Ref)], ret_ty: Type::Void }, Type::Void),
            (pending, Instruction::CallRuntime { name: "dartforge_exception_pending".into(),
                args: vec![], ret_ty: Type::I8 }, Type::I8),
            (cond, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pending),
                Operand::Constant(Constant::Int(0))), Type::I1),
        ]);
        f.blocks[i].terminator = Terminator::CondBranch {
            cond: Operand::Val(cond), then_block: error, else_block: normal,
        };
        let placeholder = if f.return_ty == Type::Void { None } else {
            Some(super::super::operands::default_constant(f.return_ty))
        };
        for id in [error, normal] {
            f.blocks.push(BasicBlock { id, instructions: vec![],
                terminator: Terminator::Return(placeholder.clone()) });
        }
        plan.tables.exits.insert(error, ExceptionExit::Throws);
        if let Some(events) = plan.scopes.exits.remove(&origin) {
            plan.scopes.exits.insert(error, events.clone());
            plan.scopes.exits.insert(normal, events);
        }
    }
    Ok(())
}

/// Gives each pending guard its own copy of a shared exit block
/// (without a Phi, ending in a return or `Unreachable`, not being a
/// landing pad, an invoke target or an endpoint of a scope edge; the exit class,
/// its scope events and the pending exit of the call go along): the
/// live owners differ from path to path, and
/// the cleanup of each exit must see its own. The definitions of the copy get
/// new IDs; nothing outside the block uses them. Returns how many copies it created.
pub(super) fn split_shared_exits(f: &mut Function, plan: &mut DartFunctionPlan) -> Result<usize, String> {
    let guard = |b: &BasicBlock| -> Option<BlockId> {
        let Terminator::CondBranch { cond: Operand::Val(c), then_block, .. } = &b.terminator else { return None };
        let n = b.instructions.len();
        let [(p, Instruction::CallRuntime { name, .. }, _), (ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pv), Operand::Constant(Constant::Int(0))), _)] =
            b.instructions.get(n.checked_sub(2)?..)? else { return None };
        (name == "dartforge_exception_pending" && pv == p && ic == c).then_some(*then_block)
    };
    let mut predecessors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for b in &f.blocks {
        for s in super::super::cfg::successors(&b.terminator) {
            predecessors.entry(s).or_default().push(b.id);
        }
    }
    let reserved = |id: &BlockId| plan.tables.landing_pads.contains(id)
        || plan.tables.invocations.values().any(|b| b == id)
        || plan.scopes.edges.keys().any(|(a, b)| a == id || b == id);
    // Also the empty trampoline up to a handler (`Branch`): the normal edge of the
    // copy up to it receives the cleanup of its path.
    let trampoline = |b: &BasicBlock| b.instructions.is_empty()
        && matches!(b.terminator, Terminator::Branch(d) if !reserved(&d));
    let targets: Vec<BlockId> = f.blocks.iter().filter(|b| {
        (matches!(b.terminator, Terminator::Return(_) | Terminator::Unreachable) || trampoline(b))
            && !b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::Phi { .. }))
            && predecessors.get(&b.id).is_some_and(|ps| ps.len() > 1)
            && !reserved(&b.id)
    }).map(|b| b.id).collect();
    // A handler (a block with instructions, target of guards and of other
    // paths): each guard gets an empty trampoline up to it, split below.
    let guards_of = |alvo: BlockId, f: &Function| -> Vec<BlockId> {
        predecessors.get(&alvo).into_iter().flatten().copied()
            .filter(|p| f.blocks.iter().find(|b| b.id == *p).and_then(guard) == Some(alvo))
            .collect()
    };
    let handlers: Vec<BlockId> = f.blocks.iter().filter(|b| {
        !targets.contains(&b.id)
            && !reserved(&b.id)
            && predecessors.get(&b.id).is_some_and(|ps| ps.len() > 1)
    }).map(|b| b.id).filter(|t| !guards_of(*t, f).is_empty()).collect();
    if targets.is_empty() && handlers.is_empty() {
        return Ok(0);
    }
    let mut value = super::edges::next_val(f, plan);
    let mut block = f.blocks.iter().map(|b| b.id.0)
        .chain(plan.tables.exits.keys().map(|b| b.0))
        .chain(plan.tables.landing_pads.iter().map(|b| b.0))
        .chain(plan.tables.invocations.values().map(|b| b.0))
        .chain(plan.tokens.pendings.values().map(|b| b.0))
        .chain(plan.scopes.exits.keys().map(|b| b.0))
        .chain(plan.scopes.edges.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max().map_or(0, |b| u64::from(b) + 1);
    let mut copies = 0;
    for alvo in targets {
        let ps = predecessors[&alvo].clone();
        // The first guard keeps the original; the others get copies.
        let mut guards: Vec<BlockId> = ps.iter().copied().filter(|p| {
            f.blocks.iter().find(|b| b.id == *p).and_then(guard) == Some(alvo)
        }).collect();
        if guards.len() == ps.len() {
            guards.remove(0);
        }
        let original = f.blocks.iter().find(|b| b.id == alvo).expect("alvo existe").clone();
        for p in guards {
            let id = BlockId(u32::try_from(block).map_err(|_| "block IDs exhausted while splitting exits")?);
            block += 1;
            let mut new_set = HashMap::new();
            let mut instructions = Vec::with_capacity(original.instructions.len());
            for (v, inst, ty) in &original.instructions {
                let n = ValueId(u32::try_from(value).map_err(|_| "SSA IDs exhausted while splitting exits")?);
                value += 1;
                new_set.insert(*v, n);
                let mut inst = inst.clone();
                super::super::operands::operands_mut(&mut inst, &mut |o| {
                    if let Operand::Val(x) = o && let Some(y) = new_set.get(x) { *x = *y; }
                });
                instructions.push((n, inst, *ty));
            }
            let mut terminator = original.terminator.clone();
            if let Terminator::Return(Some(Operand::Val(x))) = &mut terminator && let Some(y) = new_set.get(x) {
                *x = *y;
            }
            // The Phis of the trampoline destination receive the same entry through the copy.
            if let Terminator::Branch(dest) = terminator {
                for (_, inst, _) in f.blocks.iter_mut().filter(|b| b.id == dest).flat_map(|b| &mut b.instructions) {
                    if let Instruction::Phi { incoming, .. } = inst
                        && let Some(op) = incoming.iter().find(|(de, _)| *de == alvo).map(|(_, op)| op.clone())
                    {
                        incoming.push((id, op));
                    }
                }
            }
            f.blocks.push(BasicBlock { id, instructions: instructions, terminator: terminator });
            if let Some(class) = plan.tables.exits.get(&alvo).cloned() {
                plan.tables.exits.insert(id, class);
            }
            if let Some(events) = plan.scopes.exits.get(&alvo).cloned() {
                plan.scopes.exits.insert(id, events);
            }
            let b = f.blocks.iter_mut().find(|b| b.id == p).expect("guarda existe");
            if let Terminator::CondBranch { then_block, .. } = &mut b.terminator {
                *then_block = id;
            }
            // The recorded pending exit of the guard's call follows the copy.
            if let Some((call, _, _)) = b.instructions.len().checked_sub(3).map(|k| &b.instructions[k])
                && plan.tokens.pendings.get(call) == Some(&alvo)
            {
                plan.tokens.pendings.insert(*call, id);
            }
            copies += 1;
        }
    }
    for handler in handlers {
        for p in guards_of(handler, f) {
            let id = BlockId(u32::try_from(block).map_err(|_| "block IDs exhausted while splitting exits")?);
            block += 1;
            for (_, inst, _) in f.blocks.iter_mut().filter(|b| b.id == handler).flat_map(|b| &mut b.instructions) {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (de, _) in incoming.iter_mut().filter(|(de, _)| *de == p) {
                        *de = id;
                    }
                }
            }
            f.blocks.push(BasicBlock { id, instructions: Vec::new(), terminator: Terminator::Branch(handler) });
            let b = f.blocks.iter_mut().find(|b| b.id == p).expect("guarda existe");
            if let Terminator::CondBranch { then_block, .. } = &mut b.terminator {
                *then_block = id;
            }
            if let Some((call, _, _)) = b.instructions.len().checked_sub(3).map(|k| &b.instructions[k])
                && plan.tokens.pendings.get(call) == Some(&handler)
            {
                plan.tokens.pendings.insert(*call, id);
            }
            copies += 1;
        }
    }
    Ok(copies)
}

pub(super) fn split_guards(f: &mut Function, plan: &mut DartFunctionPlan) -> Result<(), String> {
    super::ssa::verify(f)?;
    let guards: Vec<_> = f
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| plan.tables.exits.get(&b.id) == Some(&ExceptionExit::Guard))
        .map(|(index, b)| (index, b.id))
        .collect();
    if guards.is_empty() {
        return Ok(());
    }
    let mut value = f
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, _)| v.0),
        )
        .chain(plan.classes.keys().map(|v| v.0))
        .chain(plan.tokens.instructions.keys().map(|v| v.0))
        .chain(plan.tokens.pendings.keys().map(|v| v.0))
        .chain(plan.tables.invocations.keys().map(|v| v.0))
        .chain(plan.scopes.before.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1);
    // Stale metadata must not coincide with freshly inserted blocks.
    let mut block = f
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
        .map_or(0, |b| u64::from(b) + 1);
    let mut value_id = || -> Result<ValueId, String> {
        let id = u32::try_from(value).map_err(|_| "SSA IDs exhausted while splitting Guard")?;
        value += 1;
        Ok(ValueId(id))
    };
    let mut block_id = || -> Result<BlockId, String> {
        let id = u32::try_from(block).map_err(|_| "block IDs exhausted while splitting Guard")?;
        block += 1;
        Ok(BlockId(id))
    };
    // We only append blocks: the original indices stay valid and
    // avoid a linear search per exit in functions with many returns.
    for (index, origin) in guards {
        let b = &mut f.blocks[index];
        let Terminator::Return(op) = &b.terminator else {
            return Err(format!("Guard exit b{} is not a Return", origin.0));
        };
        let ret = op.clone();
        let pending = value_id()?;
        let cond = value_id()?;
        let error = block_id()?;
        let normal = block_id()?;
        b.instructions.extend([
            (
                pending,
                Instruction::CallRuntime {
                    name: "dartforge_exception_pending".into(),
                    args: vec![],
                    ret_ty: Type::I8,
                },
                Type::I8,
            ),
            (
                cond,
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(pending),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ),
        ]);
        b.terminator = Terminator::CondBranch {
            cond: Operand::Val(cond),
            then_block: error,
            else_block: normal,
        };
        f.blocks.extend([
            BasicBlock {
                id: error,
                instructions: vec![],
                terminator: Terminator::Return(if f.return_ty == Type::Void {
                    None
                } else {
                    Some(super::super::operands::default_constant(f.return_ty))
                }),
            },
            BasicBlock {
                id: normal,
                instructions: vec![],
                terminator: Terminator::Return(ret),
            },
        ]);
        plan.tables.exits.remove(&origin);
        plan.tables.exits.insert(error, ExceptionExit::Throws);
        // The retention in the normal return must happen before closing the
        // scope of the borrow. Each exit closes the same original lexical stack.
        if let Some(events) = plan.scopes.exits.remove(&origin) {
            plan.scopes.exits.insert(error, events.clone());
            plan.scopes.exits.insert(normal, events);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn throw_ref_publishes_payload_and_releases_locals_without_transferring_return() {
        for case in 0..4 {
            let mut m = Module::new(); m.arc_memory = true;
            m.functions.push(Function { symbol: "lancar".into(), name: "lancar".into(),
                debug_info: None, params: vec![(ValueId(0), "payload".into(),
                    if case == 3 { Type::I64 } else { Type::Ref })], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1),
                    Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
                    terminator: Terminator::Throw(match case {
                        1 => Operand::Val(ValueId(1)),
                        2 => Operand::Constant(Constant::Null),
                        _ => Operand::Val(ValueId(0)),
                    }) }] });
            m.dart_ref_returns.insert("lancar".into());
            if case == 3 {
                m.dart_scalar_parameters.insert("lancar".into(), HashSet::from([ValueId(0)]));
                m.functions[0].blocks[0].instructions[0].1 = Instruction::ArcCopy {
                    value: Operand::Constant(Constant::Null),
                };
            }
            let mut plans = HashMap::from([("lancar".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 3 {
                let error = result.unwrap_err();
                assert!(error.contains("Throw requires a prepared Ref payload"), "{error}");
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            result.unwrap();
            let f = &m.functions[0];
            assert!(f.blocks.iter().all(|b| !matches!(b.terminator, Terminator::Throw(_))));
            let publications = f.blocks.iter().flat_map(|b| &b.instructions)
                .filter(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
                    if name == "dartforge_arc_throw_ref_v1")).count();
            assert_eq!(publications, 1);
            // A dead local may be released before the Throw. The Owned
            // payload, however, may only be released after publishing the exception.
            assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                matches!(i, Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
            if case == 1 {
                let b = &f.blocks[0];
                let publication = b.instructions.iter().position(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_throw_ref_v1")).unwrap();
                assert!(!b.instructions[..publication].iter().any(|(_, i, _)| matches!(i,
                    Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
            }
            for b in f.blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return(_))) {
                assert!(matches!(b.terminator, Terminator::Return(Some(Operand::Constant(Constant::Null)))));
            }
            let prepared = format!("{m:?}/{plans:?}");
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}/{plans:?}"), prepared);
        }
    }

    fn leaf(owned: bool) -> (Vec<Function>, HashMap<String, DartFunctionPlan>) {
        let f = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: if owned {
                    vec![(
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )]
                } else {
                    vec![]
                },
                terminator: Terminator::Return(Some(Operand::Val(ValueId(if owned {
                    1
                } else {
                    0
                })))),
            }],
        };
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        p.tables.exits.insert(BlockId(0), ExceptionExit::Guard);
        (vec![f], HashMap::from([("folha".into(), p)]))
    }

    #[test]
    fn guard_transfers_owned_only_on_success_and_retains_borrow_only_on_normal() {
        for owned in [false, true] {
            let (mut functions, mut plans) = leaf(owned);
            let expected = if owned { (0, 1) } else { (1, 0) };
            assert_eq!(
                insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
                expected
            );
            let f = &functions[0];
            let p = &plans["folha"];
            let (error, normal) = match f.blocks[0].terminator {
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => (then_block, else_block),
                _ => panic!("Guard not split"),
            };
            assert_eq!(p.tables.exits[&error], ExceptionExit::Throws);
            assert!(!p.tables.exits.contains_key(&normal));
            let b_error = f.blocks.iter().find(|b| b.id == error).unwrap();
            assert!(
                !b_error
                    .instructions
                    .iter()
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
            if owned {
                assert!(matches!(
                    b_error.instructions[0].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1))
                    }
                ));
            } else {
                let b_normal = f.blocks.iter().find(|b| b.id == normal).unwrap();
                assert!(matches!(
                    b_normal.instructions[0].1,
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0))
                    }
                ));
            }
            let before = format!("{functions:?}");
            assert_eq!(
                insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{functions:?}"), before);
        }
    }

    #[test]
    fn guard_carries_scope_close_to_both_exits() {
        let (mut functions, mut plans) = leaf(false);
        let f = &mut functions[0];
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![(
                ValueId(1),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        });
        let p = plans.get_mut("folha").unwrap();
        p.tables.exits.clear();
        p.tables.exits.insert(BlockId(1), ExceptionExit::Guard);
        p.classes.insert(
            ValueId(1),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(0)),
                scope: 7,
            },
        );
        p.scopes
            .before
            .insert(ValueId(1), vec![ScopeChange::Open(7)]);
        p.scopes
            .exits
            .insert(BlockId(1), vec![ScopeChange::Close(7)]);
        assert_eq!(
            insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 0)
        );
        let p = &plans["folha"];
        assert!(!p.scopes.exits.contains_key(&BlockId(1)));
        if let Terminator::CondBranch {
            then_block,
            else_block,
            ..
        } = functions[0].blocks[1].terminator
        {
            for b in [then_block, else_block] {
                assert_eq!(p.scopes.exits[&b], vec![ScopeChange::Close(7)]);
            }
        } else {
            panic!("Guard not split");
        }
    }

    #[test]
    fn multiple_guards_preserve_transfer_and_reject_later_failure() {
        for failure in 0..3 {
            let (mut functions, mut plans) = leaf(true);
            let f = &mut functions[0];
            f.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(true)),
                then_block: BlockId(9),
                else_block: BlockId(4),
            };
            // IDs out of physical order must not change the chosen block.
            for id in [9, 4] {
                f.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                });
            }
            let p = plans.get_mut("folha").unwrap();
            p.tables.exits.clear();
            for id in [9, 4] {
                p.tables
                    .exits
                    .insert(BlockId(id), ExceptionExit::Guard);
            }
            if failure == 1 {
                // The first exit will already have been split in the temporary copy.
                p.scopes
                    .exits
                    .insert(BlockId(4), vec![ScopeChange::Close(7)]);
            } else if failure == 2 {
                // The first Guard uses the two remaining IDs; the second
                // must fail without publishing even the initial transformation.
                functions[0].blocks[0].instructions.push((
                    ValueId(u32::MAX - 2),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            }
            let before = format!("{functions:?}");
            let plans_before = format!("{plans:?}");
            let result = insert_arc_dart_functions(&mut functions, &mut plans);
            if failure != 0 {
                assert!(result.unwrap_err().contains(if failure == 1 {
                    "invalid limit"
                } else {
                    "SSA IDs exhausted"
                }));
                assert_eq!(format!("{functions:?}"), before);
                assert_eq!(format!("{plans:?}"), plans_before);
                continue;
            }
            assert_eq!(result.unwrap(), (0, 2));
            assert_eq!(functions[0].blocks.len(), 7);
            let p = &plans["folha"];
            assert_eq!(p.tables.exits.len(), 2);
            for original in &functions[0].blocks[1..3] {
                let Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } = original.terminator
                else {
                    panic!("Guard not split");
                };
                assert_eq!(p.tables.exits[&then_block], ExceptionExit::Throws);
                let error = functions[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == then_block)
                    .unwrap();
                let normal = functions[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == else_block)
                    .unwrap();
                assert!(matches!(
                    error.instructions[0].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1))
                    }
                ));
                assert!(normal.instructions.is_empty());
                assert!(matches!(
                    normal.terminator,
                    Terminator::Return(Some(Operand::Val(ValueId(1))))
                ));
            }
            assert_eq!(
                insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
                (0, 0)
            );
        }
    }

    #[test]
    fn guard_does_not_publish_cfg_tables_or_scopes_after_failure() {
        for exhausted in [false, true] {
            let (mut functions, mut plans) = leaf(false);
            if exhausted {
                functions[0].blocks[0].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            } else {
                plans
                    .get_mut("folha")
                    .unwrap()
                    .scopes
                    .exits
                    .insert(BlockId(0), vec![ScopeChange::Close(7)]);
            }
            let before = format!("{functions:?}");
            let plans_before = format!("{plans:?}");
            let error = insert_arc_dart_functions(&mut functions, &mut plans).unwrap_err();
            assert!(
                error.contains(if exhausted {
                    "SSA IDs exhausted"
                } else {
                    "invalid limit"
                }),
                "{error}"
            );
            assert_eq!(format!("{functions:?}"), before);
            assert_eq!(format!("{plans:?}"), plans_before);
        }
    }
}

#[cfg(test)]
mod shared_exit_tests {
    use super::*;

    #[test]
    fn each_guard_gets_its_own_exit_and_rest_stays() {
        let val = |n| Operand::Val(ValueId(n));
        let guard = |p: u32, c: u32, error: u32, follows: u32, call: Instruction| BasicBlock {
            id: BlockId(p),
            instructions: vec![
                (ValueId(c), call, Type::Ref),
                (ValueId(c + 1), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(c + 2), Instruction::ICmp(ICmpOp::Ne, val(c + 1), Operand::Constant(Constant::Int(0))), Type::I1),
            ],
            terminator: Terminator::CondBranch { cond: val(c + 2), then_block: BlockId(error), else_block: BlockId(follows) },
        };
        let call = || Instruction::CallStatic { symbol: "g".into(), args: vec![], ret_ty: Type::Ref };
        let mut f = Function { symbol: "f".into(), name: "f".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![
                guard(0, 1, 9, 1, call()),
                guard(1, 4, 9, 2, call()),
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(val(4))) },
                BasicBlock { id: BlockId(9), instructions: vec![
                    (ValueId(20), Instruction::StoreGlobal { symbol: "ok".into(), val: Operand::Constant(Constant::Int(0)), ty: Type::I8, root: None }, Type::Void),
                ], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            ] };
        let mut plan = DartFunctionPlan::default();
        plan.tables.exits.insert(BlockId(9), ExceptionExit::Throws);
        assert_eq!(split_shared_exits(&mut f, &mut plan).unwrap(), 1);
        let Terminator::CondBranch { then_block: a, .. } = f.blocks[0].terminator else { unreachable!() };
        let Terminator::CondBranch { then_block: b, .. } = f.blocks[1].terminator else { unreachable!() };
        assert_eq!(a, BlockId(9));
        assert_ne!(b, BlockId(9));
        assert_eq!(plan.tables.exits.get(&b), Some(&ExceptionExit::Throws));
        let copy_inst = f.blocks.iter().find(|x| x.id == b).unwrap();
        assert_ne!(copy_inst.instructions[0].0, ValueId(20));
        assert!(matches!(&copy_inst.instructions[0].1, Instruction::StoreGlobal { symbol, .. } if symbol == "ok"));
        super::super::ssa::verify(&f).unwrap();
        // A landing-pad exit stays as it is.
        let mut g = f.clone();
        if let Terminator::CondBranch { then_block, .. } = &mut g.blocks[1].terminator { *then_block = BlockId(9); }
        g.blocks.retain(|x| x.id != b);
        let mut plan = DartFunctionPlan::default();
        plan.tables.landing_pads.insert(BlockId(9));
        assert_eq!(split_shared_exits(&mut g, &mut plan).unwrap(), 0);
    }

    #[test]
    fn shared_handler_gets_one_trampoline_per_guard() {
        let val = |n| Operand::Val(ValueId(n));
        let guard = |p: u32, c: u32, follows: u32| BasicBlock {
            id: BlockId(p),
            instructions: vec![
                (ValueId(c), Instruction::CallStatic { symbol: "g".into(), args: vec![], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(c + 1), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(c + 2), Instruction::ICmp(ICmpOp::Ne, val(c + 1), Operand::Constant(Constant::Int(0))), Type::I1),
            ],
            terminator: Terminator::CondBranch { cond: val(c + 2), then_block: BlockId(9), else_block: BlockId(follows) },
        };
        let mut f = Function { symbol: "f".into(), name: "f".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![
                guard(0, 1, 1),
                guard(1, 4, 2),
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(val(4))) },
                // The handler: a Phi over the guards and the `clear` of the pending state.
                BasicBlock { id: BlockId(9), instructions: vec![
                    (ValueId(20), Instruction::Phi { incoming: vec![
                        (BlockId(0), Operand::Constant(Constant::Int(1))),
                        (BlockId(1), Operand::Constant(Constant::Int(2)))], ty: Type::I64 }, Type::I64),
                    (ValueId(21), Instruction::CallRuntime { name: "dartforge_exception_clear".into(), args: vec![], ret_ty: Type::Void }, Type::Void),
                ], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            ] };
        let mut plan = DartFunctionPlan::default();
        plan.tokens.pendings.insert(ValueId(1), BlockId(9));
        plan.tokens.pendings.insert(ValueId(4), BlockId(9));
        assert_eq!(split_shared_exits(&mut f, &mut plan).unwrap(), 2);
        for (g, call, constant) in [(0, 1, 1), (1, 4, 2)] {
            let Terminator::CondBranch { then_block: t, .. } = f.blocks[g].terminator else { unreachable!() };
            let trampoline = f.blocks.iter().find(|x| x.id == t).unwrap();
            assert!(trampoline.instructions.is_empty());
            assert!(matches!(trampoline.terminator, Terminator::Branch(BlockId(9))));
            assert_eq!(plan.tokens.pendings[&ValueId(call)], t);
            let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
            assert!(incoming.contains(&(t, Operand::Constant(Constant::Int(constant)))));
        }
        super::super::ssa::verify(&f).unwrap();
        // Idempotent: the handler now has only trampolines as predecessors.
        assert_eq!(split_shared_exits(&mut f, &mut plan).unwrap(), 0);
    }
}
