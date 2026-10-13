//! Generation of local landing pads to propagate failures of direct Dart calls.
//! Runs on the transaction copies, preserving already prepared sites.

use super::*;

pub(super) fn prepare(
    f: &mut Function,
    plan: &mut DartFunctionPlan,
    do_not_throw: &HashSet<String>,
    by_tables: bool,
) -> Result<(), String> {
    // The check lowering has already chosen catch/finally and the continuation.
    // Recognizing the exact shape keeps that CFG as pending, without inventing
    // an LLVM invoke nor replacing the handler with exception propagation.
    super::ssa::verify(f)?;
    let mut pendings = HashMap::new();
    for b in &f.blocks {
        let Terminator::CondBranch { then_block, .. } = b.terminator else { continue };
        let Some((v, Instruction::CallStatic { symbol, .. }, _)) = b.instructions
            .len().checked_sub(3).map(|i| &b.instructions[i]) else { continue };
        if do_not_throw.contains(symbol) || plan.tables.invocations.contains_key(v) {
            continue;
        }
        if super::classification::check_pending(b, *v, then_block).is_some() {
            if plan.tokens.pendings.get(v).is_some_and(|previous| *previous != then_block) {
                return Err(format!("{}: v{}: saída pending conflita com CFG", f.symbol, v.0));
            }
            pendings.insert(*v, then_block);
        }
    }
    plan.tokens.pendings.extend(pendings);
    let absent = |v: &ValueId, inst: &Instruction| {
        matches!(inst,
        Instruction::CallStatic { symbol, .. }
        if !do_not_throw.contains(symbol) && !plan.tables.invocations.contains_key(v)
            && !plan.tokens.pendings.contains_key(v))
    };
    if !f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(v, i, _)| absent(v, i))
    {
        return Ok(());
    }
    // An existing check may select catch/finally. We do not replace it
    // with propagation without the sites supplied by the lowering.
    if f.blocks.iter().any(|b| {
        b.instructions.windows(2).any(|par| {
            absent(&par[0].0, &par[0].1)
                && matches!(&par[1].1, Instruction::CallRuntime { name, .. }
                if name == "dartforge_exception_pending")
        })
    }) {
        return Err(format!(
            "{}: conferência de pendência Dart exige sítio preparado",
            f.symbol
        ));
    }
    let stacks = super::scopes::stacks_at_calls(f, &plan.scopes)?;
    let mut next_item = f
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
    let mut reserve = || -> Result<BlockId, String> {
        let id =
            u32::try_from(next_item).map_err(|_| "IDs de bloco esgotados ao preparar invoke Dart")?;
        next_item += 1;
        Ok(BlockId(id))
    };
    let mut next_val = super::edges::next_val(f, plan);
    let mut finals = HashMap::new();
    let originals = f.blocks.len();
    for index in 0..originals {
        let origin = f.blocks[index].id;
        let instructions = std::mem::take(&mut f.blocks[index].instructions);
        let terminator = f.blocks[index].terminator.clone();
        let mut current = index;
        for (v, inst, ty) in instructions {
            let split = matches!(&inst, Instruction::CallStatic { symbol, .. }
                if !do_not_throw.contains(symbol) && !plan.tables.invocations.contains_key(&v)
                    && !plan.tokens.pendings.contains_key(&v));
            f.blocks[current].instructions.push((v, inst, ty));
            if !split {
                continue;
            }
            let error = reserve()?;
            let normal = reserve()?;
            if by_tables {
                f.blocks[current].terminator = Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: error,
                    else_block: normal,
                };
                plan.tables.invocations.insert(v, error);
                plan.tables.landing_pads.insert(error);
                plan.tables.exits.insert(
                    error,
                    if cfg!(unix) {
                        ExceptionExit::Resume
                    } else {
                        ExceptionExit::Throws
                    },
                );
            } else {
                // Exceptions by check: the guard the lowering would emit
                // (an optimization removed it assuming the callee throws nothing);
                // the exit returns the default value with the pending exception.
                let p = ValueId(u32::try_from(next_val).map_err(|_| "IDs SSA esgotados ao preparar guarda Dart")?);
                let c = ValueId(p.0.checked_add(1).ok_or("IDs SSA esgotados ao preparar guarda Dart")?);
                next_val = u64::from(c.0) + 1;
                f.blocks[current].instructions.extend([
                    (p, Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (c, Instruction::ICmp(ICmpOp::Ne, Operand::Val(p), Operand::Constant(Constant::Int(0))), Type::I1),
                ]);
                f.blocks[current].terminator = Terminator::CondBranch {
                    cond: Operand::Val(c),
                    then_block: error,
                    else_block: normal,
                };
                plan.tokens.pendings.insert(v, error);
            }
            // The landing pad releases the tokens before leaving the active scopes.
            // Unreachable calls have no executable stack to close.
            let close: Vec<_> = stacks
                .get(&v)
                .into_iter()
                .flatten()
                .rev()
                .filter(|e| **e != 0)
                .map(|e| ScopeChange::Close(*e))
                .collect();
            if !close.is_empty() {
                plan.scopes.exits.insert(error, close);
            }
            f.blocks.push(BasicBlock {
                id: error,
                instructions: vec![],
                terminator: Terminator::Return(if f.return_ty == Type::Void {
                    None
                } else {
                    Some(super::super::operands::default_constant(f.return_ty))
                }),
            });
            current = f.blocks.len();
            f.blocks.push(BasicBlock {
                id: normal,
                instructions: vec![],
                terminator: terminator.clone(),
            });
        }
        f.blocks[current].terminator = terminator;
        if current != index {
            finals.insert(origin, f.blocks[current].id);
        }
    }
    // Only the last segment keeps the original terminator. We update
    // Phis and events once per function, without searching per call/successor.
    for b in &mut f.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (de, _) in incoming {
                    if let Some(new) = finals.get(de) {
                        *de = *new;
                    }
                }
            }
        }
    }
    plan.scopes.edges = std::mem::take(&mut plan.scopes.edges)
        .into_iter()
        .map(|((de, to), events)| ((*finals.get(&de).unwrap_or(&de), to), events))
        .collect();
    for (origin, final_) in finals {
        if let Some(events) = plan.scopes.exits.remove(&origin) {
            plan.scopes.exits.insert(final_, events);
        }
        if let Some(exit) = plan.tables.exits.remove(&origin) {
            plan.tables.exits.insert(final_, exit);
        }
    }
    super::ssa::verify(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> (Vec<Function>, HashMap<String, DartFunctionPlan>) {
        let call = |v| {
            (
                ValueId(v),
                Instruction::CallStatic {
                    symbol: "folha".into(),
                    args: vec![Operand::Val(ValueId(v - 1))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    ),
                    call(2),
                    call(3),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        };
        let leaf = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            debug_info: None,
            params: caller.params.clone(),
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        let mut q = p.clone();
        q.tables.checks_stack = true;
        (
            vec![caller, leaf],
            HashMap::from([("caller".into(), p), ("folha".into(), q)]),
        )
    }

    #[test]
    fn consecutive_calls_have_exact_cleanup_and_close_scopes() {
        let (mut functions, mut plans) = set();
        let p = plans.get_mut("caller").unwrap();
        p.scopes
            .before
            .insert(ValueId(2), vec![ScopeChange::Open(7)]);
        p.scopes
            .exits
            .insert(BlockId(0), vec![ScopeChange::Close(7)]);
        assert_eq!(
            prepare_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 4)
        );
        let p = &plans["caller"];
        // The owner v1 ends in the normal continuation of the first call;
        // it does not need to stay until the landing pad of the second one.
        for (v, expected) in [(2, vec![1]), (3, vec![2])] {
            let error = p.tables.invocations[&ValueId(v)];
            assert!(p.tables.landing_pads.contains(&error));
            assert_eq!(
                p.tables.exits[&error],
                if cfg!(unix) {
                    ExceptionExit::Resume
                } else {
                    ExceptionExit::Throws
                }
            );
            assert_eq!(p.scopes.exits[&error], vec![ScopeChange::Close(7)]);
            let b = functions[0].blocks.iter().find(|b| b.id == error).unwrap();
            let drops: Vec<_> = b
                .instructions
                .iter()
                .filter_map(|(_, i, _)| match i {
                    Instruction::ArcDrop {
                        value: Operand::Val(v),
                    } => Some(v.0),
                    _ => None,
                })
                .collect();
            assert_eq!(drops, expected);
        }
        assert!(!p.scopes.exits.contains_key(&BlockId(0)));
        let before = format!("{functions:?}");
        assert_eq!(
            prepare_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{functions:?}"), before);
    }

    #[test]
    fn phi_and_edge_limit_follow_normal_continuation() {
        let (mut functions, mut plans) = set();
        let f = &mut functions[0];
        f.blocks[0].instructions.pop();
        f.blocks[0].terminator = Terminator::Branch(BlockId(9));
        f.blocks.push(BasicBlock {
            id: BlockId(9),
            instructions: vec![(
                ValueId(3),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(2)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        });
        let p = plans.get_mut("caller").unwrap();
        p.scopes
            .before
            .insert(ValueId(2), vec![ScopeChange::Open(7)]);
        p.scopes
            .edges
            .insert((BlockId(0), BlockId(9)), vec![ScopeChange::Close(7)]);
        assert_eq!(
            prepare_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 2)
        );
        let Instruction::Phi { incoming, .. } = &functions[0].blocks[1].instructions[0].1 else {
            panic!("Phi removido");
        };
        let de = incoming[0].0;
        assert_ne!(de, BlockId(0));
        assert_eq!(
            plans["caller"].scopes.edges[&(de, BlockId(9))],
            vec![ScopeChange::Close(7)]
        );
        assert!(
            !plans["caller"]
                .scopes
                .edges
                .contains_key(&(BlockId(0), BlockId(9)))
        );
    }

    #[test]
    fn existing_handler_is_preserved_when_preparing_later_call() {
        let (mut functions, mut plans) = set();
        let f = &mut functions[0];
        let second = f.blocks[0].instructions.pop().unwrap();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(7),
            else_block: BlockId(4),
        };
        f.blocks.push(BasicBlock {
            id: BlockId(4),
            instructions: vec![second],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        });
        f.blocks.push(BasicBlock {
            id: BlockId(7),
            instructions: vec![(
                ValueId(4),
                Instruction::CallRuntime {
                    name: "dartforge_exception_clear".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )],
            terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
        });
        let p = plans.get_mut("caller").unwrap();
        p.tables.invocations.insert(ValueId(2), BlockId(7));
        p.tables.landing_pads.insert(BlockId(7));
        assert_eq!(
            prepare_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 4)
        );
        let p = &plans["caller"];
        assert_eq!(p.tables.invocations[&ValueId(2)], BlockId(7));
        assert!(!p.tables.exits.contains_key(&BlockId(7)));
        assert_eq!(
            p.tables.exits[&p.tables.invocations[&ValueId(3)]],
            if cfg!(unix) {
                ExceptionExit::Resume
            } else {
                ExceptionExit::Throws
            }
        );
    }

    #[test]
    fn contract_or_id_failure_preserves_whole_transaction() {
        for exhausted in [false, true] {
            let (mut functions, mut plans) = set();
            if exhausted {
                functions[0].blocks.push(BasicBlock {
                    id: BlockId(u32::MAX - 2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                });
            } else if let Instruction::CallStatic { symbol, .. } =
                &mut functions[0].blocks[0].instructions[2].1
            {
                *symbol = "ausente".into();
            }
            let before = format!("{functions:?}");
            let plans_before = format!("{plans:?}");
            let error = prepare_arc_dart_functions(&mut functions, &mut plans).unwrap_err();
            assert!(
                error.contains(if exhausted {
                    "IDs de bloco esgotados"
                } else {
                    "ausente"
                }),
                "{error}"
            );
            assert_eq!(format!("{functions:?}"), before);
            assert_eq!(format!("{plans:?}"), plans_before);
        }
    }

    #[test]
    fn dart_pending_preserves_catch_and_result_exists_only_on_success() {
        for case in 0..7 {
            let (mut functions, mut plans) = set();
            let caller = &mut functions[0];
            caller.blocks[0].instructions.pop();
            caller.blocks[0].instructions.extend([
                (ValueId(3), Instruction::CallRuntime {
                    name: "dartforge_exception_pending".into(),
                    args: vec![], ret_ty: if case == 2 { Type::I64 } else { Type::I8 },
                }, if case == 2 { Type::I64 } else { Type::I8 }),
                (ValueId(4), Instruction::ICmp(
                    if case == 1 { ICmpOp::Eq } else { ICmpOp::Ne },
                    Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0)),
                ), Type::I1),
            ]);
            caller.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Val(ValueId(4)), then_block: BlockId(2), else_block: BlockId(1),
            };
            caller.blocks.extend([
                BasicBlock { id: BlockId(1), instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) },
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(5), Instruction::CallRuntime {
                        name: "dartforge_exception_clear".into(), args: vec![], ret_ty: Type::Void,
                    }, Type::Void),
                ], terminator: Terminator::Return(Some(if case == 3 {
                    Operand::Val(ValueId(2))
                } else { Operand::Constant(Constant::Null) })) },
            ]);
            if case == 4 {
                plans.get_mut("caller").unwrap().tokens.pendings.insert(ValueId(2), BlockId(1));
            }
            let mut m = Module::new();
            m.arc_memory = case != 5;
            m.functions = functions;
            if case == 6 { m.functions.reverse(); }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if (1..5).contains(&case) {
                assert!(result.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            if case == 5 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            result.unwrap();
            let p = &plans["caller"];
            assert_eq!(p.tokens.pendings[&ValueId(2)], BlockId(2));
            assert!(p.tables.invocations.is_empty());
            assert!(p.tables.landing_pads.is_empty());
            assert!(!m.excecoes_por_tabelas);
            assert!(m.tables.is_empty());
            let caller = m.functions.iter().find(|f| f.symbol == "caller").unwrap();
            assert!(matches!(&caller.blocks[0].terminator,
                Terminator::CondBranch { cond: Operand::Val(ValueId(4)), then_block: BlockId(2), .. }));
            let catch = caller.blocks.iter().find(|b| b.id == BlockId(2)).unwrap();
            assert!(catch.instructions.iter().any(|(_, i, _)| matches!(i,
                Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear")));
            assert!(catch.instructions.iter().any(|(_, i, _)| matches!(i,
                Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
            assert!(!catch.instructions.iter().any(|(_, i, _)| matches!(i,
                Instruction::ArcDrop { value: Operand::Val(ValueId(2)) })));
            assert!(matches!(catch.terminator,
                Terminator::Return(Some(Operand::Constant(Constant::Null)))));
            let verified = format!("{m:?}/{plans:?}");
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}/{plans:?}"), verified);
        }
    }

    #[test]
    fn existing_check_is_not_replaced_by_propagation() {
        let (mut functions, mut plans) = set();
        functions[0].blocks[0].instructions.pop();
        functions[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".into(),
                args: vec![],
                ret_ty: Type::I8,
            },
            Type::I8,
        ));
        functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let before = format!("{functions:?}");
        let plans_before = format!("{plans:?}");
        assert!(
            prepare_arc_dart_functions(&mut functions, &mut plans)
                .unwrap_err()
                .contains("conferência de pendência Dart exige sítio preparado")
        );
        assert_eq!(format!("{functions:?}"), before);
        assert_eq!(format!("{plans:?}"), plans_before);
    }
    #[test]
    fn resume_clears_owners_without_transferring_result_and_rejects_changing_exception() {
        let (mut functions, mut plans) = set();
        prepare_arc_dart_functions(&mut functions, &mut plans).unwrap();
        for mode in plans
            .get_mut("caller")
            .unwrap()
            .tables
            .exits
            .values_mut()
        {
            *mode = ExceptionExit::Resume;
        }
        let before = format!("{functions:?}");
        assert_eq!(
            super::super::insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{functions:?}"), before);
        let p = &plans["caller"];
        for b in p.tables.exits.keys() {
            assert!(matches!(
                functions[0]
                    .blocks
                    .iter()
                    .find(|bl| bl.id == *b)
                    .unwrap()
                    .terminator,
                Terminator::Return(Some(Operand::Constant(Constant::Null)))
            ));
        }
        let id = p.tables.invocations[&ValueId(2)];
        functions[0]
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .unwrap()
            .instructions
            .push((
                ValueId(1000),
                Instruction::CallRuntime {
                    name: "dartforge_exception_clear".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ));
        let before = format!("{functions:?}");
        let plans_before = format!("{plans:?}");
        assert!(
            super::super::insert_arc_dart_functions(&mut functions, &mut plans)
                .unwrap_err()
                .contains("só admite drops")
        );
        assert_eq!(format!("{functions:?}"), before);
        assert_eq!(format!("{plans:?}"), plans_before);
    }
}
