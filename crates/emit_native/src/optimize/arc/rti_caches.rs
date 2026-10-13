//! Provenance of the native caches of RTI recipes; does not cover Dart globals.

use super::*;

pub(super) fn produce(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(), String> {
    if !module.arc_memory {
        return Ok(());
    }
    let mut globals = HashMap::new();
    for cache in &module.arc_rti_caches {
        let declarations: Vec<_> = module.globals.iter().filter(|g| &g.2 == cache).collect();
        if declarations.len() != 1 || declarations[0].0 != 0 || declarations[0].1 != Type::I64 {
            return Err(format!("RTI cache without a single native declaration: {cache}"));
        }
        let flag = format!("{cache}$ok");
        if module.globals.iter().any(|g| g.2 == flag)
            || globals.insert(cache.clone(), Type::I64).is_some()
            || globals.insert(flag, Type::I8).is_some()
        {
            return Err(format!("RTI cache with an overlapping symbol: {cache}"));
        }
    }
    let mut new_set = plans.clone();
    for f in &module.functions {
        let definitions: HashMap<_, _> = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, i, t)| (*v, (i, *t)))
            .collect();
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            // The HIR offers no address of a global. Also rejects the attempt
            // to use its symbol as a function address/opaque publication.
            let mut address = false;
            super::super::operands::operands(inst, &mut |o| {
                address |=
                    matches!(o, Operand::Constant(Constant::Function(s)) if globals.contains_key(s));
            });
            if matches!(inst, Instruction::Const(Constant::Function(s)) if globals.contains_key(s))
                || address
            {
                return Err(format!("RTI cache published as an address in {}", f.symbol));
            }
            let (sym_name, expected) = match inst {
                Instruction::LoadGlobal {
                    symbol: sym_name,
                    ty: origin,
                } if globals.contains_key(sym_name) => {
                    if globals[sym_name] != *origin || *ty != *origin {
                        return Err(format!("RTI cache with an incompatible read: {sym_name}"));
                    }
                    (sym_name, *origin)
                }
                Instruction::StoreGlobal {
                    symbol: sym_name,
                    val,
                    ty: dest,
                    root,
                } if globals.contains_key(sym_name) => {
                    let native = match globals[sym_name] {
                        Type::I8 => matches!(val, Operand::Constant(Constant::Int(0 | 1))),
                        Type::I64 => {
                            matches!(val, Operand::Val(id) if definitions.get(id).is_some_and(|(i, t)|
                            *t == Type::I64 && matches!(i, Instruction::CallRuntime { name, args, ret_ty }
                            if name == "dartforge_rti_receita" && *ret_ty == Type::I64
                            && args.len() == 1 && args[0].1 == Type::Ref)))
                        }
                        _ => false,
                    };
                    if *dest != globals[sym_name]
                        || *ty != Type::Void
                        || root.is_some()
                        || !native
                    {
                        return Err(format!(
                            "RTI cache with a write without a native origin: {sym_name}"
                        ));
                    }
                    (sym_name, Type::Void)
                }
                _ => continue,
            };
            let plan = new_set
                .get_mut(&f.symbol)
                .ok_or_else(|| format!("RTI cache without a plan: {}", f.symbol))?;
            if plan
                .classes
                .get(v)
                .is_some_and(|c| *c != Ownership::Trivial)
                || plan
                    .tokens
                    .instructions
                    .get(v)
                    .is_some_and(|e| *e != TokenEffect::default())
            {
                return Err(format!(
                    "RTI cache with a conflicting contract: {sym_name} ({expected:?})"
                ));
            }
            plan.classes.insert(*v, Ownership::Trivial);
            plan.tokens.instructions.insert(*v, TokenEffect::default());
        }
        let mut address = false;
        for b in &f.blocks {
            super::super::operands::terminator_operands(&b.terminator, &mut |o| {
                address |=
                    matches!(o, Operand::Constant(Constant::Function(s)) if globals.contains_key(s));
            });
        }
        if address {
            return Err(format!("RTI cache published in the terminator in {}", f.symbol));
        }
    }
    *plans = new_set;
    Ok(())
}

// Current bodies and already audited caches, without assuming a symbol prefix.
// The conclusion does not cover reload nor functions with implicit Dart context/exit.
pub(super) fn preserve_fields(m: &Module, plans: &HashMap<String, DartFunctionPlan>) -> HashSet<String> {
    let mut globals = HashMap::new();
    for name_str in &m.arc_rti_caches {
        globals.insert(name_str.clone(), Type::I64);
        globals.insert(format!("{name_str}$ok"), Type::I8);
    }
    let mut pending_set = HashMap::new();
    let mut pais: HashMap<String, Vec<String>> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for f in &m.functions {
        if !f.params.is_empty() || f.return_ty != Type::I64
            || crate::llvm::LlvmEmitter::requires_explicit_context(f, plans.get(&f.symbol).map(|p| &p.tables)) {
            continue;
        }
        let types: HashMap<_, _> = f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)).collect();
        let numeric = |o: &Operand| match o {
            Operand::Val(v) => matches!(types.get(v), Some(Type::I64 | Type::I8 | Type::I1)),
            Operand::Constant(Constant::Int(_) | Constant::Bool(_)) => true,
            _ => false,
        };
        let mut dependencies = HashSet::new();
        let closed = f.blocks.iter().all(|b| {
            let exit = match &b.terminator {
                Terminator::Return(Some(o)) => numeric(o),
                Terminator::Branch(_) | Terminator::Unreachable => true,
                Terminator::CondBranch { cond, .. } => numeric(cond),
                _ => false,
            };
            exit && b.instructions.iter().all(|(_, inst, ty)| match inst {
                Instruction::Const(Constant::Int(_)) => *ty == Type::I64,
                Instruction::Const(Constant::Bool(_)) => *ty == Type::I1,
                Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => *ty == Type::Ref,
                Instruction::ICmp(_, a, b) => *ty == Type::I1 && numeric(a) && numeric(b),
                Instruction::Phi { incoming, .. } => matches!(ty, Type::I64 | Type::I8 | Type::I1)
                    && incoming.iter().all(|(_, o)| numeric(o)),
                Instruction::LoadGlobal { symbol: sym_name, ty: read_done } => globals.get(sym_name) == Some(read_done) && ty == read_done,
                Instruction::StoreGlobal { symbol: sym_name, ty: stored, root, val } => globals.get(sym_name) == Some(stored)
                    && *ty == Type::Void && root.is_none() && numeric(val),
                Instruction::CallRuntime { name, args, ret_ty } => name == "dartforge_rti_receita"
                    && *ty == Type::I64 && *ret_ty == Type::I64 && args.len() == 1 && args[0].1 == Type::Ref,
                Instruction::CallStatic { symbol, args, ret_ty } => {
                    dependencies.insert(symbol.clone());
                    args.is_empty() && *ty == Type::I64 && *ret_ty == Type::I64
                }
                _ => false,
            })
        });
        if !closed { continue; }
        if dependencies.is_empty() { queue.push_back(f.symbol.clone()); }
        pending_set.insert(f.symbol.clone(), dependencies.len());
        for dep in dependencies { pais.entry(dep).or_default().push(f.symbol.clone()); }
    }
    let mut safe_set = HashSet::new();
    while let Some(s) = queue.pop_front() {
        if !safe_set.insert(s.clone()) { continue; }
        if let Some(callers) = pais.get(&s) {
            for caller in callers {
                let n = pending_set.get_mut(caller).unwrap();
                *n -= 1;
                if *n == 0 { queue.push_back(caller.clone()); }
            }
        }
    }
    safe_set
}

// Getters whose result is provably an ID of the RTI universe: a closed
// body of `preserve_fields` and every return coming from an audited RTI cache,
// from `dartforge_rti_receita` or from another proven getter (greatest fixpoint).
// A constant or arithmetic is not enough: preserving fields does not prove the
// origin of the ID. Every I64 cache write in the module must come from the recipe;
// without that, no getter is accepted. Does not cover JIT reload.
pub(super) fn getters_rti(m: &Module, plans: &HashMap<String, DartFunctionPlan>) -> HashSet<String> {
    let caches: HashSet<_> = m.arc_rti_caches.iter().collect();
    let native_writes = m.functions.iter().all(|f| {
        let definitions: HashMap<_, _> = f.blocks.iter().flat_map(|b| &b.instructions)
            .map(|(v, i, t)| (*v, (i, *t))).collect();
        f.blocks.iter().flat_map(|b| &b.instructions).all(|(_, inst, _)| match inst {
            Instruction::StoreGlobal { symbol: sym_name, val, ty: Type::I64, .. } if caches.contains(sym_name) =>
                matches!(val, Operand::Val(id) if definitions.get(id).is_some_and(|(i, t)| *t == Type::I64
                    && matches!(i, Instruction::CallRuntime { name, args, ret_ty: Type::I64 }
                        if name == "dartforge_rti_receita" && args.len() == 1 && args[0].1 == Type::Ref))),
            _ => true,
        })
    });
    if !native_writes {
        return HashSet::new();
    }
    let functions: HashMap<_, _> = m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    let mut accepted = preserve_fields(m, plans);
    loop {
        let rejected: Vec<String> = accepted.iter().filter(|s| {
            let Some(f) = functions.get(s.as_str()) else { return true };
            let definitions: HashMap<_, _> = f.blocks.iter().flat_map(|b| &b.instructions)
                .map(|(v, i, t)| (*v, (i, *t))).collect();
            !f.blocks.iter().all(|b| match &b.terminator {
                Terminator::Return(Some(Operand::Val(v))) =>
                    rti_origin(*v, &definitions, &caches, &accepted, &mut HashSet::new()),
                Terminator::Return(_) => false,
                _ => true,
            })
        }).cloned().collect();
        if rejected.is_empty() {
            return accepted;
        }
        for s in rejected { accepted.remove(&s); }
    }
}

// Origin of an I64 returned by an RTI getter; a Phi requires all its entries.
fn rti_origin(
    v: ValueId,
    definitions: &HashMap<ValueId, (&Instruction, Type)>,
    caches: &HashSet<&String>,
    accepted: &HashSet<String>,
    visited: &mut HashSet<ValueId>,
) -> bool {
    if !visited.insert(v) {
        return true;
    }
    match definitions.get(&v) {
        Some((Instruction::LoadGlobal { symbol: sym_name, ty: Type::I64 }, Type::I64)) => caches.contains(sym_name),
        Some((Instruction::CallRuntime { name, args, ret_ty: Type::I64 }, Type::I64)) =>
            name == "dartforge_rti_receita" && args.len() == 1 && args[0].1 == Type::Ref,
        Some((Instruction::CallStatic { symbol, args, ret_ty: Type::I64 }, Type::I64)) =>
            args.is_empty() && accepted.contains(symbol),
        Some((Instruction::Phi { incoming, .. }, Type::I64)) => incoming.iter().all(|(_, o)|
            matches!(o, Operand::Val(id) if rti_origin(*id, definitions, caches, accepted, visited))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> Module {
        let mut m = Module::new();
        m.arc_memory = true;
        // The name takes no part in the proof: only the nominal fact and the writers.
        m.globals.push((0, Type::I64, "cache_sem_prefixo".into()));
        m.arc_rti_caches.insert("cache_sem_prefixo".into());
        m.functions.push(Function {
            symbol: "getter".into(),
            name: "getter".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::Const(Constant::String("C201".into())),
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_rti_receita".into(),
                            args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::StoreGlobal {
                            symbol: "cache_sem_prefixo".into(),
                            val: Operand::Val(ValueId(1)),
                            ty: Type::I64,
                            root: None,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(3),
                        Instruction::StoreGlobal {
                            symbol: "cache_sem_prefixo$ok".into(),
                            val: Operand::Constant(Constant::Int(1)),
                            ty: Type::I8,
                            root: None,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(4),
                        Instruction::LoadGlobal {
                            symbol: "cache_sem_prefixo".into(),
                            ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(5),
                        Instruction::LoadGlobal {
                            symbol: "cache_sem_prefixo$ok".into(),
                            ty: Type::I8,
                        },
                        Type::I8,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            }],
        });
        m
    }

    #[test]
    fn recipes_preserve_fields_through_body_and_dependencies_without_trusting_name() {
        let base = module();
        for case in 0..7 {
            let mut m = module();
            m.functions.push(Function { symbol: "ponte".into(), name: "ponte".into(), debug_info: None,
                params: vec![], return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0),
                    instructions: vec![(ValueId(0), Instruction::CallStatic { symbol: "getter".into(), args: vec![], ret_ty: Type::I64 }, Type::I64)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
            let mut plans = HashMap::from([("getter".into(), DartFunctionPlan::default()), ("ponte".into(), DartFunctionPlan::default())]);
            match case {
                1 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::CallRuntime {
                    name: "dartforge_print_handle".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Void,
                }, Type::Void)),
                2 => m.functions[0].params.push((ValueId(9), "receiver".into(), Type::Ref)),
                3 => plans.get_mut("getter").unwrap().tables.checks_stack = true,
                4 => m.arc_rti_caches.clear(),
                5 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::CallStatic {
                    symbol: "ponte".into(), args: vec![], ret_ty: Type::I64,
                }, Type::I64)),
                6 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::StoreGlobal {
                    symbol: "objeto_global".into(), val: Operand::Val(ValueId(0)), ty: Type::Ref, root: Some(1),
                }, Type::Void)),
                _ => {}
            }
            let safe_set = super::preserve_fields(&m, &plans);
            assert_eq!(safe_set.contains("getter"), case == 0, "caso {case}");
            assert_eq!(safe_set.contains("ponte"), case == 0, "caso {case}");
        }
        assert!(super::preserve_fields(&base, &HashMap::new()).contains("getter"));
    }

    #[test]
    fn rti_getters_require_cache_or_recipe_return_and_native_writers() {
        for case in 0..8 {
            let mut m = module();
            m.functions.push(Function { symbol: "ponte".into(), name: "ponte".into(), debug_info: None,
                params: vec![], return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0),
                    instructions: vec![(ValueId(0), Instruction::CallStatic { symbol: "getter".into(), args: vec![], ret_ty: Type::I64 }, Type::I64)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
            let plans = HashMap::from([("getter".into(), DartFunctionPlan::default()), ("ponte".into(), DartFunctionPlan::default())]);
            let getter = &mut m.functions[0];
            let fixed = |v: u32| (ValueId(v), Instruction::Const(Constant::Int(42)), Type::I64);
            let phi = |a: Operand| (ValueId(8), Instruction::Phi { incoming: vec![(BlockId(0), Operand::Val(ValueId(4))), (BlockId(0), a)], ty: Type::I64 }, Type::I64);
            match case {
                1 => {
                    getter.blocks[0].instructions.push(fixed(9));
                    getter.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(9))));
                }
                2 => getter.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Int(42)))),
                3 => {
                    getter.blocks[0].instructions.push(fixed(9));
                    getter.blocks[0].instructions.push(phi(Operand::Val(ValueId(9))));
                    getter.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(8))));
                }
                4 => {
                    getter.blocks[0].instructions.push(phi(Operand::Val(ValueId(1))));
                    getter.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(8))));
                }
                5 => m.functions.push(Function { symbol: "estranho".into(), name: "estranho".into(), debug_info: None,
                    params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0),
                        instructions: vec![(ValueId(0), Instruction::StoreGlobal { symbol: "cache_sem_prefixo".into(),
                            val: Operand::Constant(Constant::Int(42)), ty: Type::I64, root: None }, Type::Void)],
                        terminator: Terminator::Return(None) }] }),
                6 => m.arc_rti_caches.clear(),
                7 => if let Instruction::CallStatic { args, .. } = &mut m.functions[1].blocks[0].instructions[0].1 {
                    args.push(Operand::Constant(Constant::Int(0)));
                },
                _ => {}
            }
            let accepted = super::getters_rti(&m, &plans);
            assert_eq!(accepted.contains("getter"), matches!(case, 0 | 4 | 7), "caso {case}");
            assert_eq!(accepted.contains("ponte"), matches!(case, 0 | 4), "caso {case}");
        }
    }

    #[test]
    fn cache_requires_declaration_and_all_native_writers() {
        for case in 0..19 {
            let mut m = module();
            let mut plans = HashMap::from([("getter".into(), DartFunctionPlan::default())]);
            match case {
                1 => m.globals[0].1 = Type::Ref,
                2 => m.globals[0].0 = 9,
                3 => m.globals.clear(),
                4 => m.globals.push(m.globals[0].clone()),
                5 => m.globals.push((0, Type::I8, "cache_sem_prefixo$ok".into())),
                6 => {
                    m.arc_rti_caches.insert("obsoleto".into());
                }
                7 => {
                    if let Instruction::StoreGlobal { val, .. } =
                        &mut m.functions[0].blocks[0].instructions[2].1
                    {
                        *val = Operand::Val(ValueId(0));
                    }
                }
                8 => {
                    if let Instruction::StoreGlobal { root, .. } =
                        &mut m.functions[0].blocks[0].instructions[2].1
                    {
                        *root = Some(9);
                    }
                }
                9 => {
                    if let Instruction::StoreGlobal { val, .. } =
                        &mut m.functions[0].blocks[0].instructions[3].1
                    {
                        *val = Operand::Constant(Constant::Int(2));
                    }
                }
                10 => m.functions[0].blocks[0].instructions[4].2 = Type::Ref,
                11 => {
                    plans
                        .get_mut("getter")
                        .unwrap()
                        .classes
                        .insert(ValueId(5), Ownership::Owned);
                }
                12 => {
                    plans.get_mut("getter").unwrap().tokens.instructions.insert(
                        ValueId(2),
                        TokenEffect {
                            may_fail: true,
                            ..Default::default()
                        },
                    );
                }
                13 => {
                    m.functions[0].blocks[0].instructions.push((
                        ValueId(6),
                        Instruction::Const(Constant::Function("cache_sem_prefixo".into())),
                        Type::I64,
                    ));
                }
                14 => m.arc_rti_caches.clear(),
                15 => {
                    let mut strange = m.functions[0].clone();
                    strange.symbol = "estranho".into();
                    strange.blocks[0].instructions[2].1 = Instruction::StoreGlobal {
                        symbol: "cache_sem_prefixo".into(),
                        val: Operand::Constant(Constant::Int(42)),
                        ty: Type::I64,
                        root: None,
                    };
                    m.functions.insert(0, strange);
                    plans.insert("estranho".into(), DartFunctionPlan::default());
                }
                16 => m.arc_memory = false,
                17 => {
                    let mut other = m.functions[0].clone();
                    other.symbol = "outro".into();
                    m.functions.insert(0, other);
                    plans.insert("outro".into(), DartFunctionPlan::default());
                }
                18 => {
                    if let Instruction::CallRuntime { name, .. } =
                        &mut m.functions[0].blocks[0].instructions[1].1
                    {
                        *name = "opaco".into();
                    }
                }
                _ => {}
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if matches!(case, 0 | 16 | 17) {
                result.unwrap_or_else(|e| panic!("caso {case}: {e}"));
                if case == 16 {
                    assert_eq!(format!("{m:?}/{plans:?}"), before);
                } else {
                    for v in [2, 3, 4, 5] {
                        assert_eq!(plans["getter"].classes[&ValueId(v)], Ownership::Trivial);
                        assert_eq!(
                            plans["getter"].tokens.instructions[&ValueId(v)],
                            TokenEffect::default()
                        );
                    }
                    let after = format!("{m:?}/{plans:?}");
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap();
                    assert_eq!(format!("{m:?}/{plans:?}"), after);
                }
            } else {
                assert!(result.is_err(), "caso {case}: {result:?}");
                assert_eq!(format!("{m:?}/{plans:?}"), before, "caso {case}");
            }
        }
    }
}
