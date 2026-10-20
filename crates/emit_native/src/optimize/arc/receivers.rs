//! Physical classes per SSA flow and arguments of closed direct calls.
use super::*;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Default)]
struct Classes {
    cids: BTreeSet<u32>,
    top: bool,
}

pub(super) fn produce(m: &Module) -> HashMap<String, HashMap<ValueId, u32>> {
    let mut ids = HashMap::new();
    let mut states = Vec::<Classes>::new();
    for f in &m.functions {
        for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, t)| (v, t)),
        ) {
            if *t == Type::Ref {
                ids.insert((f.symbol.clone(), *v), states.len());
                states.push(Classes::default());
            }
        }
    }
    let by_name: HashMap<_, _> = m.functions.iter().map(|f| (&f.symbol, f)).collect();
    let mut entries: HashSet<String> = m
        .arc_uniform_returns
        .iter()
        .cloned()
        .chain(m.helpers.iter().map(|(_, s)| s.clone()))
        .chain(m.classes.iter().flat_map(|c| {
            c.vtable
                .iter()
                .map(|(_, s)| s.clone())
                .chain(c.to_string_symbol.iter().cloned())
        }))
        .chain(m.entry_symbol.iter().cloned())
        .chain(m.call_dart.iter().cloned())
        .chain(m.ffi_callbacks.iter().map(|c| c.body.clone()))
        .chain(m.ffi_trampolines.iter().map(|(_, s)| s.clone()))
        .chain(m.source_roots.iter().cloned())
        .chain(m.table_functions.values().cloned())
        .chain(m.record.iter().cloned())
        .chain(m.sdk_records.iter().cloned())
        .chain(m.start_rti.iter().cloned())
        .chain(m.stack_trace_tearoffs.iter().map(|(s, _)| s.clone()))
        .chain(
            m.method_tables
                .iter()
                .flat_map(|(_, _, t)| t.iter().map(|(_, s)| s.clone())),
        )
        .collect();
    let mut calls = HashSet::new();
    let mut edges = vec![Vec::new(); states.len()];
    let mut feed = |sym_name: &str, op: &Operand, dest: usize| {
        if let Operand::Val(v) = op
            && let Some(&origin) = ids.get(&(sym_name.to_string(), *v))
        {
            edges[origin].push(dest);
        } else {
            states[dest].top = true;
        }
    };
    // A materialized address is an open entry, even when known local callers
    // exist. Never infer all the arguments from those callers alone.
    for f in &m.functions {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            match i {
                Instruction::Const(Constant::Function(s))
                | Instruction::TearOff { code_symbol: s }
                | Instruction::AllocClosure { code_symbol: s, .. } => {
                    entries.insert(s.clone());
                }
                Instruction::AllocClosureTyped {
                    code_symbol, typed, ..
                } => {
                    entries.insert(code_symbol.clone());
                    entries.insert(typed.clone());
                }
                Instruction::FunctionTable(symbols) => entries.extend(symbols.iter().cloned()),
                _ => {}
            }
            super::super::operands::operands(i, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    entries.insert(s.clone());
                }
            });
            if let Instruction::CallStatic { symbol, args, .. } = i
                && let Some(callee) = by_name.get(symbol)
            {
                calls.insert(symbol.clone());
                if args.len() == callee.params.len() {
                    for (op, (v, _, t)) in args.iter().zip(&callee.params) {
                        if *t == Type::Ref {
                            feed(&f.symbol, op, ids[&(symbol.clone(), *v)]);
                        }
                    }
                } else {
                    entries.insert(symbol.clone());
                }
            }
        }
    }
    for f in &m.functions {
        for b in &f.blocks {
            super::super::operands::terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    entries.insert(s.clone());
                }
            });
        }
    }
    for f in &m.functions {
        for (v, _, t) in &f.params {
            if *t == Type::Ref && (entries.contains(&f.symbol) || !calls.contains(&f.symbol)) {
                states[ids[&(f.symbol.clone(), *v)]].top = true;
            }
        }
        for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *t != Type::Ref {
                continue;
            }
            let dest = ids[&(f.symbol.clone(), *v)];
            let class = match i {
                Instruction::AllocObject { class_id, fields } => m
                    .arc_field_layouts
                    .get(class_id)
                    .filter(|l| l.len() == fields.len())
                    .map(|_| *class_id),
                Instruction::CallRuntime { name, args, ret_ty }
                    if *ret_ty == Type::Ref
                        && matches!(
                            name.as_str(),
                            "dartforge_object_new" | "dartforge_arc_object_owned_v1"
                        ) =>
                {
                    match args.as_slice() {
                        [
                            (Operand::Constant(Constant::Int(cid)), Type::I64),
                            (Operand::Constant(Constant::Int(n)), Type::I64),
                        ] => u32::try_from(*cid)
                            .ok()
                            .filter(|cid| i32::try_from(*cid).is_ok())
                            .filter(|cid| {
                                usize::try_from(*n).ok().is_some_and(|n| {
                                    u16::try_from(n).is_ok()
                                        && m.arc_field_layouts
                                            .get(cid)
                                            .is_some_and(|l| l.len() == n)
                                })
                            }),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(cid) = class {
                states[dest].cids.insert(cid);
                continue;
            }
            let ops: Vec<_> = match i {
                Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
                Instruction::Phi { incoming, .. } => incoming.iter().map(|(_, o)| o).collect(),
                _ => {
                    states[dest].top = true;
                    continue;
                }
            };
            for op in ops {
                if let Operand::Val(v) = op
                    && let Some(&origin) = ids.get(&(f.symbol.clone(), *v))
                {
                    edges[origin].push(dest);
                } else {
                    states[dest].top = true;
                }
            }
        }
    }
    let mut queue: VecDeque<_> = (0..states.len()).collect();
    let mut scheduled = vec![true; states.len()];
    while let Some(origin) = queue.pop_front() {
        scheduled[origin] = false;
        let current = states[origin].clone();
        for &dest in &edges[origin] {
            let before = (states[dest].top, states[dest].cids.len());
            states[dest].top |= current.top;
            states[dest].cids.extend(&current.cids);
            if before != (states[dest].top, states[dest].cids.len()) && !scheduled[dest] {
                scheduled[dest] = true;
                queue.push_back(dest);
            }
        }
    }
    let mut result = HashMap::<String, HashMap<ValueId, u32>>::new();
    for f in &m.functions {
        for (v, _, t) in &f.params {
            if *t != Type::Ref {
                continue;
            }
            let c = &states[ids[&(f.symbol.clone(), *v)]];
            if !c.top && c.cids.len() == 1 {
                result
                    .entry(f.symbol.clone())
                    .or_default()
                    .insert(*v, *c.cids.first().unwrap());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new();
        m.arc_memory = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 1,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.arc_field_layouts.insert(123, vec![Type::Ref]);
        m.functions = vec![
            Function {
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
                            Instruction::AllocObject {
                                class_id: 123,
                                fields: vec![Operand::Constant(Constant::Null)],
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: "ctor".into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            },
            Function {
                symbol: "ctor".into(),
                name: "ctor".into(),
                debug_info: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::SetField {
                                object: Operand::Val(ValueId(0)),
                                index: 0,
                                value: Operand::Constant(Constant::Null),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallStatic {
                                symbol: "base".into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            },
            Function {
                symbol: "base".into(),
                name: "base".into(),
                debug_info: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            },
        ];
        let plans = m
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        (m, plans)
    }

    #[test]
    fn caller_anchors_this_and_pre_super_initialization_without_inventing_freshness() {
        let (mut m, mut plans) = module();
        let facts = produce(&m);
        assert_eq!(facts["ctor"][&ValueId(0)], 123);
        assert_eq!(facts["base"][&ValueId(0)], 123);
        prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        assert!(m.functions[1].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_write_field_ref_v1")));
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
    }

    #[test]
    fn open_boundaries_require_guard_and_invalid_static_proofs_reject() {
        for case in 0..7 {
            let (mut m, mut plans) = module();
            match case {
                0 => m.helpers.push(("nativo".into(), "ctor".into())),
                1 => {
                    if let Instruction::CallStatic { args, .. } =
                        &mut m.functions[0].blocks[0].instructions[1].1
                    {
                        args[0] = Operand::Constant(Constant::Null);
                    }
                }
                2 => m.functions[1].blocks[0].instructions.swap(0, 1),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(9),
                    Instruction::Const(Constant::Function("ctor".into())),
                    Type::I64,
                )),
                4 => {
                    let mut caller = m.functions[0].clone();
                    caller.symbol = "outro".into();
                    caller.params.push((ValueId(8), "opaco".into(), Type::Ref));
                    if let Instruction::CallStatic { args, .. } =
                        &mut caller.blocks[0].instructions[1].1
                    {
                        args[0] = Operand::Val(ValueId(8));
                    }
                    m.functions.push(caller);
                    plans.insert("outro".into(), DartFunctionPlan::default());
                }
                5 => m.functions[1].blocks[0].terminator = Terminator::Branch(BlockId(0)),
                _ => {
                    let mut body = m.functions[1].blocks[0].clone();
                    body.id = BlockId(1);
                    m.functions[1].blocks[0].instructions.clear();
                    m.functions[1].blocks[0].terminator = Terminator::Branch(BlockId(1));
                    m.functions[1].blocks.push(body);
                }
            }
            if matches!(case, 0 | 1 | 3 | 4) {
                assert!(
                    !produce(&m).contains_key("ctor"),
                    "case {case}: open entry gained a class"
                );
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if matches!(case, 0 | 1 | 2 | 4 | 5 | 6) {
                // Open entries do not get a class because there are known
                // callers. Exposed entries also require a physical guard;
                // null, a CID or an invalid size will be rejected by the runtime.
                result.unwrap();
                assert!(
                    m.functions[1]
                        .blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(
                            |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
                        if name == "dartforge_arc_write_field_ref_checked_v1")
                        )
                );
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                assert!(result.is_err(), "caso {case}: {result:?}");
                assert_eq!(before, format!("{m:?}/{plans:?}"));
            }
        }
    }
}
