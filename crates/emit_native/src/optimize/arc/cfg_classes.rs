//! Prunes impossible class branches using every caller of the closed group.
//! Preserves the IDs of live queries and the reachability of exceptional edges.

use super::*;

pub(super) fn prepare(
    m: &mut Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(), String> {
    let receivers = super::receivers::produce(m);
    let mut changed = false;
    for f in &mut m.functions {
        let Some(classes) = receivers.get(&f.symbol) else {
            continue;
        };
        let p = plans
            .get_mut(&f.symbol)
            .ok_or_else(|| format!("ARC class: missing plan for {}", f.symbol))?;
        let mut queries = HashMap::new();
        for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallRuntime {
                name,
                args,
                ret_ty: Type::I64,
            } = i
                && name == "dartforge_value_class"
                && *ty == Type::I64
                && let [(Operand::Val(obj), Type::Ref)] = args.as_slice()
                && let Some(cid) = classes.get(obj)
                && !p.tables.invocations.contains_key(v)
                && !p.tokens.pendings.contains_key(v)
            {
                queries.insert(*v, i64::from(*cid));
            }
        }
        if queries.is_empty() {
            continue;
        }
        changed = true;
        super::ssa::verify(f)?;
        let edges_before: HashSet<_> = f
            .blocks
            .iter()
            .flat_map(|b| {
                super::super::cfg::successors(&b.terminator)
                    .into_iter()
                    .chain(
                        b.instructions
                            .iter()
                            .filter_map(|(v, _, _)| p.tables.invocations.get(v).copied()),
                    )
                    .map(move |dest| (b.id, dest))
            })
            .collect();
        if let Some(c) = &p.tables.foreign_cleanup {
            c.check(f, &p.tables)?;
        }
        for b in &mut f.blocks {
            for (v, i, _) in &mut b.instructions {
                if let Some(cid) = queries.get(v) {
                    *i = Instruction::Const(Constant::Int(*cid));
                }
            }
            if let Terminator::Switch {
                val: Operand::Val(v),
                default,
                cases,
            } = &b.terminator
                && let Some(cid) = queries.get(v)
            {
                let dest = cases
                    .iter()
                    .find(|(c, _)| c == cid)
                    .map_or(*default, |(_, b)| *b);
                b.terminator = Terminator::Branch(dest);
            }
        }
        let blocks: HashMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
        let mut live_set = HashSet::new();
        let mut queue = vec![f.blocks[0].id];
        let mut edges = HashSet::new();
        while let Some(id) = queue.pop() {
            if !live_set.insert(id) {
                continue;
            }
            let b = blocks.get(&id).ok_or("ARC class: destination outside the CFG")?;
            let dests = super::super::cfg::successors(&b.terminator)
                .into_iter()
                .chain(
                    b.instructions
                        .iter()
                        .filter_map(|(v, _, _)| p.tables.invocations.get(v).copied()),
                );
            for dest in dests {
                edges.insert((id, dest));
                queue.push(dest);
            }
        }
        let dead_set: HashSet<_> = f
            .blocks
            .iter()
            .filter(|b| !live_set.contains(&b.id))
            .map(|b| b.id)
            .collect();
        let dead_ids: HashSet<_> = f
            .blocks
            .iter()
            .filter(|b| dead_set.contains(&b.id))
            .flat_map(|b| &b.instructions)
            .map(|(v, _, _)| *v)
            .collect();
        f.blocks.retain(|b| !dead_set.contains(&b.id));
        for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            if let Instruction::Phi { incoming, .. } = i {
                incoming.retain(|(b, _)| !dead_set.contains(b));
            }
        }
        p.classes.retain(|v, _| !dead_ids.contains(v));
        p.tokens.instructions.retain(|v, _| !dead_ids.contains(v));
        p.tokens.pendings.retain(|v, _| !dead_ids.contains(v));
        p.tables.invocations.retain(|v, _| !dead_ids.contains(v));
        p.tables.landing_pads.retain(|b| !dead_set.contains(b));
        p.tables.exits.retain(|b, _| !dead_set.contains(b));
        p.owners_at_landing_pad.retain(|b, _| !dead_set.contains(b));
        p.scopes.before.retain(|v, _| !dead_ids.contains(v));
        p.scopes.exits.retain(|b, _| !dead_set.contains(b));
        p.scopes
            .edges
            .retain(|e, _| !edges_before.contains(e) || edges.contains(e));
        // The old certificate belongs to the previous body. The full
        // transaction recomputes owners/frames and seals the cleanup after inserting ARC.
        p.tables.foreign_cleanup = None;
        if let Some(sites) = m.arc_sites.get_mut(&f.symbol) {
            sites.retain(|v, _| !dead_ids.contains(v));
        }
        super::ssa::verify(f)?;
    }
    if changed {
        m.arc_aot_ports = None;
    }
    if m.table_exceptions {
        m.tables = m
            .functions
            .iter()
            .map(|f| {
                plans
                    .get(&f.symbol)
                    .map(|p| p.tables.clone())
                    .ok_or_else(|| format!("ARC class: missing plan for {}", f.symbol))
            })
            .collect::<Result<_, _>>()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(123, vec![Type::I64, Type::Ref]);
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 2,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![
                                (Operand::Constant(Constant::Int(123)), Type::I64),
                                (Operand::Constant(Constant::Int(2)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "construtor".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        m.functions.push(Function {
            symbol: "construtor".into(),
            name: "construtor".into(),
            debug_info: None,
            params: vec![(ValueId(0), "this".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_value_class".into(),
                                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Switch {
                        val: Operand::Val(ValueId(2)),
                        default: BlockId(2),
                        cases: vec![(123, BlockId(1)), (124, BlockId(2))],
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(3),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(4),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 1,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                },
            ],
        });
        let ps = m
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        (m, ps)
    }

    #[test]
    fn class_of_all_callers_removes_impossible_write_before_guards() {
        let (mut m, mut ps) = origin();
        ps.get_mut("construtor")
            .unwrap()
            .classes
            .insert(ValueId(4), Ownership::Trivial);
        prepare(&mut m, &mut ps).unwrap();
        assert!(!m.functions[1].blocks.iter().any(|b| b.id == BlockId(2)));
        assert!(!ps["construtor"].classes.contains_key(&ValueId(4)));
        prepare_arc_dart_module(&mut m, &mut ps).unwrap();
        for f in &m.functions {
            let p = &ps[&f.symbol];
            verify_dart_function_contract(f, &p.classes, &p.tokens, &p.tables, &p.scopes)
                .unwrap();
        }
    }

    #[test]
    fn open_parameter_or_divergent_class_receiver_does_not_fold_switch() {
        for case in 0..4 {
            let (mut m, mut ps) = origin();
            if case == 1 {
                m.helpers.push(("externa".into(), "construtor".into()));
            } else if case == 2 {
                m.source_roots.push("construtor".into());
            } else if case == 3 {
                m.table_functions.insert(1, "construtor".into());
            } else {
                m.functions[0]
                    .params
                    .push((ValueId(7), "outra_origem".into(), Type::Ref));
                m.functions[0].blocks[0].instructions.push((
                    ValueId(8),
                    Instruction::CallStatic {
                        symbol: "construtor".into(),
                        args: vec![Operand::Val(ValueId(7))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
            let before = format!("{m:?}");
            prepare(&mut m, &mut ps).unwrap();
            assert_eq!(format!("{m:?}"), before);
        }
    }

    #[test]
    fn exceptional_only_landing_pad_stays_reachable_and_query_scope_stays() {
        let (mut m, mut ps) = origin();
        let p = ps.get_mut("construtor").unwrap();
        p.tables.invocations.insert(ValueId(9), BlockId(9));
        p.tables.landing_pads.insert(BlockId(9));
        p.tables.exits.insert(BlockId(9), ExceptionExit::Throws);
        p.owners_at_landing_pad.insert(BlockId(9), vec![]);
        p.scopes
            .before
            .insert(ValueId(2), vec![ScopeChange::Open(1)]);
        p.scopes
            .edges
            .insert((BlockId(0), BlockId(2)), vec![ScopeChange::Close(1)]);
        m.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(9),
                Instruction::CallStatic {
                    symbol: "caller".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        );
        m.functions[1].blocks.push(BasicBlock {
            id: BlockId(9),
            instructions: vec![],
            terminator: Terminator::Return(None),
        });
        prepare(&mut m, &mut ps).unwrap();
        assert!(m.functions[1].blocks.iter().any(|b| b.id == BlockId(9)));
        assert!(ps["construtor"].scopes.before.contains_key(&ValueId(2)));
        assert!(
            !ps["construtor"]
                .scopes
                .edges
                .contains_key(&(BlockId(0), BlockId(2)))
        );
    }
}
