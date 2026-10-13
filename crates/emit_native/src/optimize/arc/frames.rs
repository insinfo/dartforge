//! Lifetime of the local owner frames opened by the audited ARC ABI.
//! Requires direct SSA IDs and checks constant bounds when the capacity is
//! known. The ordinary entry rejects imported ones. The audit of the closed set
//! may supply borrowed pairs, without including them in the cleanup stack.
//! Aliases and dynamic indices without that pair stay out of the forwarding.

use super::super::cfg::Cfg;
use crate::hir::*;
use std::collections::{HashMap, VecDeque};

pub(super) fn verify(f: &Function) -> Result<(), String> {
    analyze(f).map(|_| ())
}

pub(super) fn verify_with_plan(f: &Function, p: &super::TokenPlan) -> Result<(), String> {
    if let Some(c) = &p.shared_frames {
        c.check(f)?;
        analyze_shared(f, c.pairs(), &c.targets()).map(|_| ())
    } else { verify(f) }
}

fn analyze(f: &Function) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    analyze_shared(f, &HashMap::new(), &HashMap::new())
}

// The imported pairs must come from the audit of the closed set. The returned
// stack holds only local frames: a borrow never becomes a cleanup.
pub(super) fn analyze_shared(
    f: &Function,
    borrowed_set: &HashMap<ValueId, ValueId>,
    targets: &HashMap<String, Vec<(usize, usize)>>,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let cfg = Cfg::new(f);
    let capacities: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, inst, _)| match inst {
            Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_arc_frame_open_v1" =>
            {
                match args.first() {
                    Some((Operand::Constant(Constant::Int(n)), _)) => Some((*v, *n)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    if f.blocks.is_empty() {
        return Err("ARC frames: function without an entry".into());
    }
    let mut entries: Vec<Option<Vec<ValueId>>> = vec![None; f.blocks.len()];
    entries[0] = Some(Vec::new());
    let mut queue = VecDeque::from([0]);
    while let Some(bi) = queue.pop_front() {
        let b = &f.blocks[bi];
        let error = |m: String| format!("ARC frames in {} b{}: {m}", f.symbol, b.id.0);
        let mut active_set = entries[bi].clone().unwrap();
        for (v, inst, _) in &b.instructions {
            let use_value = |op: &Operand, active_set: &[ValueId]| -> Result<ValueId, String> {
                let Operand::Val(q) = op else {
                    return Err(error("frame requires a local SSA ID".into()));
                };
                if !active_set.contains(q) && !borrowed_set.contains_key(q) {
                    return Err(error(format!("frame v{} is not open", q.0)));
                }
                Ok(*q)
            };
            let index = |q: ValueId, i: i64| -> Result<(), String> {
                if borrowed_set.contains_key(&q) {
                    return Err(error(
                        "borrowed frame requires its index parameter".into(),
                    ));
                }
                if i < 0 || capacities.get(&q).is_some_and(|n| i >= *n) {
                    return Err(error(format!("index {i} outside frame v{}", q.0)));
                }
                Ok(())
            };
            let par = |q: ValueId, op: &Operand| -> Result<(), String> {
                if let Some(i) = borrowed_set.get(&q) {
                    if op != &Operand::Val(*i) {
                        return Err(error(format!(
                            "index does not belong to the borrowed frame v{}",
                            q.0
                        )));
                    }
                } else if let Operand::Constant(Constant::Int(i)) = op {
                    if !capacities.contains_key(&q) {
                        return Err(error(
                            "forwarding requires a constant capacity of the local frame".into(),
                        ));
                    }
                    index(q, *i)?;
                } else {
                    return Err(error(
                        "forwarding requires a constant local index or a borrowed pair".into(),
                    ));
                }
                Ok(())
            };
            match inst {
                Instruction::ArcLoadStrong {
                    slot: StrongSlot::Frame { frame, index: i },
                }
                | Instruction::ArcStoreStrong {
                    slot: StrongSlot::Frame { frame, index: i },
                    ..
                } => {
                    let q = use_value(frame, &active_set)?;
                    index(q, i64::from(*i))?;
                }
                Instruction::CallRuntime { name, args, .. }
                    if name.starts_with("dartforge_arc_frame_") =>
                {
                    if name == "dartforge_arc_frame_open_v1" {
                        if capacities.get(v).is_some_and(|n| *n < 0) {
                            return Err(error("negative frame capacity".into()));
                        }
                        if active_set.contains(v) {
                            return Err(error(format!("frame v{} opened twice", v.0)));
                        }
                        active_set.push(*v);
                    } else {
                        let first_one = args
                            .first()
                            .ok_or_else(|| error("frame without an argument".into()))?;
                        let q = use_value(&first_one.0, &active_set)?;
                        if borrowed_set.contains_key(&q) {
                            if !matches!(
                                name.as_str(),
                                "dartforge_arc_frame_load_v1"
                                    | "dartforge_arc_frame_copy_v1"
                            ) {
                                return Err(error(
                                    "function cannot close or transfer a borrowed frame".into(),
                                ));
                            }
                            let i = args
                                .get(1)
                                .ok_or_else(|| error("borrowed frame without an index".into()))?;
                            par(q, &i.0)?;
                        }
                        if name != "dartforge_arc_frame_close_v1" {
                            if let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) {
                                index(q, *i)?;
                            }
                        }
                        if name == "dartforge_arc_frame_close_v1" {
                            if active_set.last() != Some(&q) {
                                return Err(error(format!("closing out of LIFO order of v{}", q.0)));
                            }
                            active_set.pop();
                        } else if name == "dartforge_arc_frame_move_v1" {
                            let dest = args
                                .get(2)
                                .ok_or_else(|| error("move without a destination frame".into()))?;
                            let q_dest = use_value(&dest.0, &active_set)?;
                            if borrowed_set.contains_key(&q_dest) {
                                return Err(error(
                                    "move cannot transfer to a borrowed frame".into(),
                                ));
                            }
                            if let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(3) {
                                index(q_dest, *i)?;
                            }
                        }
                    }
                }
                Instruction::CallStatic { symbol, args, .. } if targets.contains_key(symbol) => {
                    for &(qi, ii) in &targets[symbol] {
                        let q = args
                            .get(qi)
                            .ok_or_else(|| error("forwarding without a frame".into()))?;
                        let i = args
                            .get(ii)
                            .ok_or_else(|| error("forwarding without an index".into()))?;
                        let q = use_value(q, &active_set)?;
                        par(q, i)?;
                    }
                }
                _ => {}
            }
        }
        if cfg.successors[bi].is_empty() && !active_set.is_empty() {
            return Err(error(format!("frames still open at the exit: {active_set:?}")));
        }
        for &s in &cfg.successors[bi] {
            match &entries[s] {
                Some(previous) if *previous != active_set => {
                    return Err(error(format!(
                        "different frame stacks at the join b{}",
                        f.blocks[s].id.0
                    )));
                }
                Some(_) => {}
                None => {
                    entries[s] = Some(active_set.clone());
                    queue.push_back(s);
                }
            }
        }
    }
    Ok(f.blocks
        .iter()
        .zip(entries)
        .filter_map(|(b, q)| q.map(|q| (b.id, q)))
        .collect())
}

pub(super) fn at_landing_pad_entry(
    f: &Function,
    t: &FunctionTables,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let entries = if let Some(c) = &t.shared_frames {
        c.check(f)?;
        analyze_shared(f, c.pairs(), &c.targets())?
    } else { analyze(f)? };
    let mut result = HashMap::new();
    let mut landing_pads: Vec<_> = t.landing_pads.iter().copied().collect();
    landing_pads.sort_by_key(|b| b.0);
    for b in landing_pads {
        let frames = entries
            .get(&b)
            .ok_or_else(|| format!("ARC frames: landing pad b{} without an entry", b.0))?;
        result.insert(b, frames.clone());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(
        v: u32,
        name: &str,
        args: Vec<(Operand, Type)>,
        ty: Type,
    ) -> (ValueId, Instruction, Type) {
        (
            ValueId(v),
            Instruction::CallRuntime {
                name: name.into(),
                args,
                ret_ty: ty,
            },
            ty,
        )
    }
    fn example() -> Function {
        Function {
            symbol: "quadros".into(),
            name: "quadros".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    call(
                        0,
                        "dartforge_arc_frame_open_v1",
                        vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                        Type::I64,
                    ),
                    call(
                        1,
                        "dartforge_arc_frame_close_v1",
                        vec![(Operand::Val(ValueId(0)), Type::I64)],
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        }
    }
    #[test]
    fn close_and_use_require_active_local_frame() {
        let mut f = example();
        verify(&f).unwrap();
        f.blocks[0].instructions.push((
            ValueId(2),
            Instruction::ArcLoadStrong {
                slot: StrongSlot::Frame {
                    frame: Operand::Val(ValueId(0)),
                    index: 0,
                },
            },
            Type::Ref,
        ));
        assert!(verify(&f).unwrap_err().contains("is not open"));
        f.blocks[0].instructions.pop();
        f.blocks[0].instructions.pop();
        assert!(verify(&f).unwrap_err().contains("still open"));
        f = example();
        let close = f.blocks[0].instructions[1].clone();
        f.blocks[0].instructions.push(close);
        assert!(verify(&f).unwrap_err().contains("is not open"));
    }
    #[test]
    fn divergent_cfg_stack_does_not_publish_plans() {
        let mut f = example();
        let closer = f.blocks[0].instructions.pop().unwrap();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        f.blocks.extend([
            BasicBlock {
                id: BlockId(1),
                instructions: vec![closer],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ]);
        let mut classes = std::collections::HashMap::new();
        let mut plan = super::super::TokenPlan::default();
        let error = super::super::produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &super::super::ScopePlan::default(),
        )
        .unwrap_err();
        assert!(error.contains("different frame stacks"), "{error}");
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn constant_slots_require_index_within_capacity() {
        let mut f = example();
        f.blocks[0].instructions.insert(
            1,
            (
                ValueId(2),
                Instruction::ArcLoadStrong {
                    slot: StrongSlot::Frame {
                        frame: Operand::Val(ValueId(0)),
                        index: 0,
                    },
                },
                Type::Ref,
            ),
        );
        verify(&f).unwrap();
        f.blocks[0].instructions[1].1 = Instruction::ArcLoadStrong {
            slot: StrongSlot::Frame {
                frame: Operand::Val(ValueId(0)),
                index: 1,
            },
        };
        assert!(verify(&f).unwrap_err().contains("index 1 outside"));
        f.blocks[0].instructions[1] = call(
            2,
            "dartforge_arc_frame_load_v1",
            vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Constant(Constant::Int(-1)), Type::I64),
            ],
            Type::Ref,
        );
        assert!(verify(&f).unwrap_err().contains("index -1 outside"));
        f = example();
        f.blocks[0].instructions[0] = call(
            0,
            "dartforge_arc_frame_open_v1",
            vec![(Operand::Constant(Constant::Int(-1)), Type::I64)],
            Type::I64,
        );
        assert!(verify(&f).unwrap_err().contains("negative frame capacity"));
    }

    #[test]
    fn closing_obeys_lifo() {
        let mut f = example();
        f.blocks[0].instructions.insert(
            1,
            call(
                2,
                "dartforge_arc_frame_open_v1",
                vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                Type::I64,
            ),
        );
        assert!(verify(&f).unwrap_err().contains("out of LIFO order"));
    }
}
