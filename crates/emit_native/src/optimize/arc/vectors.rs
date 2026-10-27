//! Facts of the uniform ABI: native address and counts of the immutable descriptor.
//! Ref reads get an owner and check bounds through the descriptor. Does not prove
//! the extent of external pointers, RTI words nor pins of the JIT image.

use super::*;

pub(super) const READ_ARGUMENT: &str = "dartforge_arc_argument_owned_v1";

/// The word of the type tuple of the generic uniform call: the slot right
/// after the positional and named arguments, `args[desc[0] + desc[1]]`
/// (`slot_tuple`), an ID of the RTI universe read as `I64`.
pub(super) fn tuple_read(
    f: &Function,
    inst: &Instruction,
    ty: Type,
    roles: &HashMap<ValueId, DartEntryVector>,
) -> bool {
    let Instruction::LoadIndexed {
        base: Operand::Val(base),
        index: Operand::Val(i),
    } = inst
    else {
        return false;
    };
    if ty != Type::I64 || roles.get(base) != Some(&DartEntryVector::Arguments) {
        return false;
    }
    let def = |x: &ValueId| {
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .find(|(v, _, _)| v == x)
            .map(|(_, i, _)| i)
    };
    // `desc[k]` read from the declared descriptor.
    let descriptor_read = |o: &Operand, k: i64| {
        matches!(o, Operand::Val(x)
        if matches!(def(x), Some(Instruction::LoadIndexed { base: Operand::Val(d), index: Operand::Constant(Constant::Int(n)) })
            if *n == k && roles.get(d) == Some(&DartEntryVector::Descriptor)))
    };
    matches!(def(i), Some(Instruction::Add(a, b)) if descriptor_read(a, 0) && descriptor_read(b, 1))
}

/// The receiver's `noSuchMethod` for a uniform call whose arguments do not
/// match (`dartforge_nsm_mismatch(recv, name, code, args, desc)`, emitted by
/// `FnBuilder::unpack` and the type-argument check of a generic entry): the
/// runtime only reads the declared vector and descriptor, borrowing the `Ref`
/// arguments for the duration of the call, like a forwarding adapter. The
/// name may carry the scope or Owned-thunk prefix of a later preparation.
pub(super) fn mismatch_forward(
    inst: &Instruction,
    roles: &HashMap<ValueId, DartEntryVector>,
) -> bool {
    let Instruction::CallRuntime { name, args, .. } = inst else {
        return false;
    };
    let base = name
        .strip_prefix(super::NATIVE_SCOPE_PREFIX)
        .or_else(|| name.strip_prefix(super::GENERIC_OWNED_PREFIX))
        .unwrap_or(name);
    let role = |o: &Operand| match o {
        Operand::Val(v) => roles.get(v).copied(),
        _ => None,
    };
    base == "dartforge_nsm_mismatch"
        && matches!(args.as_slice(),
            [(recv, Type::Ref), (text, Type::Ref), (code, Type::I64), (a, Type::Ptr), (d, Type::Ptr)]
                if role(recv).is_none() && role(text).is_none() && role(code).is_none()
                    && role(a) == Some(DartEntryVector::Arguments)
                    && role(d) == Some(DartEntryVector::Descriptor))
}

pub(super) fn checked_read(inst: &Instruction, roles: &HashMap<ValueId, DartEntryVector>) -> bool {
    matches!(inst, Instruction::CallRuntime { name, args, ret_ty: Type::Ref }
        if name == READ_ARGUMENT && matches!(args.as_slice(),
            [(Operand::Val(a), Type::Ptr), (Operand::Val(d), Type::Ptr), (_, Type::I64)]
                if roles.get(a) == Some(&DartEntryVector::Arguments)
                    && roles.get(d) == Some(&DartEntryVector::Descriptor)))
}

pub(super) fn prepare_reads(module: &Module, functions: &mut [Function]) -> Result<(), String> {
    for f in functions {
        let Some(roles) = module.dart_vector_parameters.get(&f.symbol) else {
            continue;
        };
        let descriptors: Vec<_> = roles
            .iter()
            .filter(|(_, p)| **p == DartEntryVector::Descriptor)
            .map(|(v, _)| *v)
            .collect();
        for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            let Instruction::LoadIndexed {
                base: Operand::Val(base),
                index,
            } = inst
            else {
                continue;
            };
            if *ty != Type::Ref || roles.get(base) != Some(&DartEntryVector::Arguments) {
                continue;
            }
            let [descriptor] = descriptors.as_slice() else {
                return Err(format!(
                    "ABI vectors: {} v{} without a single descriptor",
                    f.symbol, v.0
                ));
            };
            *inst = Instruction::CallRuntime {
                name: READ_ARGUMENT.into(),
                args: vec![
                    (Operand::Val(*base), Type::Ptr),
                    (Operand::Val(*descriptor), Type::Ptr),
                    (index.clone(), Type::I64),
                ],
                ret_ty: Type::Ref,
            };
        }
    }
    Ok(())
}

pub(super) fn produce(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(), String> {
    if !module.arc_memory {
        return Ok(());
    }
    let mut functions = HashMap::new();
    for f in &module.functions {
        if functions.insert(&f.symbol, f).is_some() {
            return Err(format!("ABI vectors: repeated symbol {}", f.symbol));
        }
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if matches!(inst, Instruction::CallRuntime { name, .. } if name == READ_ARGUMENT)
                && (*ty != Type::Ref
                    || module
                        .dart_vector_parameters
                        .get(&f.symbol)
                        .is_none_or(|p| !checked_read(inst, p)))
            {
                return Err(format!(
                    "ABI vectors: {} v{} without provenance of the read",
                    f.symbol, v.0
                ));
            }
        }
    }
    let mut new_set = plans.clone();
    let mut facts: Vec<_> = module.dart_vector_parameters.iter().collect();
    facts.sort_by_key(|(s, _)| *s);
    for (sym_name, roles) in facts {
        let f = functions
            .get(sym_name)
            .ok_or_else(|| format!("ABI vectors: stale symbol {sym_name}"))?;
        let plan = new_set
            .get_mut(sym_name)
            .ok_or_else(|| format!("ABI vectors: missing plan {sym_name}"))?;
        let types: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t)).collect();
        if types.len() != f.params.len() {
            return Err(format!("ABI vectors: repeated parameters {sym_name}"));
        }
        let mark = |classes: &mut HashMap<ValueId, Ownership>, v: ValueId| -> Result<(), String> {
            if classes.get(&v).is_some_and(|c| *c != Ownership::Trivial) {
                return Err(format!(
                    "ABI vectors: {sym_name} v{} conflicts with ownership",
                    v.0
                ));
            }
            classes.insert(v, Ownership::Trivial);
            Ok(())
        };
        let mut ids: Vec<_> = roles.keys().copied().collect();
        ids.sort_by_key(|v| v.0);
        for v in ids {
            if types.get(&v) != Some(&Type::Ptr)
                || module
                    .dart_scalar_parameters
                    .get(sym_name)
                    .is_some_and(|s| s.contains(&v))
                || module
                    .dart_rti_parameters
                    .get(sym_name)
                    .is_some_and(|s| s.contains(&v))
            {
                return Err(format!(
                    "ABI vectors: {sym_name} v{} without a Ptr parameter of exclusive origin",
                    v.0
                ));
            }
            mark(&mut plan.classes, v)?;
        }
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::LoadIndexed {
                base: Operand::Val(base),
                index: Operand::Val(_),
            } = inst
                && *ty == Type::I64
                && roles.get(base) == Some(&DartEntryVector::Arguments)
            {
                if !tuple_read(f, inst, *ty, roles) {
                    return Err(format!(
                        "ABI vectors: {sym_name} v{} reads a scalar word outside the tuple slot",
                        v.0
                    ));
                }
                mark(&mut plan.classes, *v)?;
                let effect = TokenEffect::default();
                if plan
                    .tokens
                    .instructions
                    .get(v)
                    .is_some_and(|e| *e != effect)
                {
                    return Err(format!(
                        "ABI vectors: {sym_name} v{} conflicts with the read effect",
                        v.0
                    ));
                }
                plan.tokens.instructions.insert(*v, effect);
                continue;
            }
            if matches!(inst, Instruction::CallRuntime { name, .. } if name == READ_ARGUMENT) {
                if *ty != Type::Ref || !checked_read(inst, roles) {
                    return Err(format!(
                        "ABI vectors: {sym_name} v{} without an arguments/descriptor pair",
                        v.0
                    ));
                }
                continue;
            }
            let descriptor = |o: &Operand| {
                matches!(o, Operand::Val(p)
                if roles.get(p) == Some(&DartEntryVector::Descriptor))
            };
            let mut uses_descriptor = false;
            super::super::operands::operands(inst, &mut |o| uses_descriptor |= descriptor(o));
            if !uses_descriptor {
                continue;
            }
            match inst {
                // Forwarding only to the declared descriptor of a local
                // body. All those bodies are validated in this transaction;
                // even cycles may only read/forward, never publish.
                Instruction::CallStatic { symbol, args, .. }
                    if functions.get(symbol).is_some_and(|callee| {
                        args.len() == callee.params.len()
                            && args.iter().enumerate().all(|(i, o)| {
                                !descriptor(o)
                                    || (callee.params[i].2 == Type::Ptr
                                        && module
                                            .dart_vector_parameters
                                            .get(symbol)
                                            .and_then(|p| p.get(&callee.params[i].0))
                                            == Some(&DartEntryVector::Descriptor))
                            })
                    }) => {}
                Instruction::LoadIndexed { base, index }
                    if descriptor(base)
                        && matches!(index, Operand::Constant(Constant::Int(0 | 1)))
                        && *ty == Type::I64 =>
                {
                    mark(&mut plan.classes, *v)?;
                    let effect = TokenEffect::default();
                    if plan
                        .tokens
                        .instructions
                        .get(v)
                        .is_some_and(|e| *e != effect)
                    {
                        return Err(format!(
                            "ABI vectors: {sym_name} v{} conflicts with the read effect",
                            v.0
                        ));
                    }
                    plan.tokens.instructions.insert(*v, effect);
                }
                // The adapter rejects destinations without a readonly uniform contract.
                Instruction::CallSelectorForward {
                    recv, args, desc, ..
                }
                | Instruction::CallClosureForward {
                    closure: recv,
                    args,
                    desc,
                    ..
                } if descriptor(desc) && !descriptor(recv) && !descriptor(args) => {}
                // The receiver's `noSuchMethod` on an arity mismatch.
                Instruction::CallRuntime { .. } if mismatch_forward(inst, roles) => {}
                // Read-only native queries of the ABI; their call/representation
                // contracts are still checked by the catalog.
                Instruction::CallRuntime { name, args, .. }
                    if matches!(
                        name.as_str(),
                        "dartforge_args_match" | "dartforge_arg_index"
                    ) && args
                        .first()
                        .is_some_and(|(o, t)| descriptor(o) && *t == Type::Ptr)
                        && args.iter().skip(1).all(|(o, _)| !descriptor(o)) => {}
                _ => {
                    return Err(format!(
                        "ABI vectors: {sym_name} v{} uses the descriptor outside the covered reads: {inst:?}",
                        v.0
                    ));
                }
            }
        }
        let mut uses_in_terminator = false;
        for b in &f.blocks {
            super::super::operands::terminator_operands(&b.terminator, &mut |o| {
                uses_in_terminator |= matches!(o, Operand::Val(p) if roles.get(p) == Some(&DartEntryVector::Descriptor));
            });
        }
        if uses_in_terminator {
            return Err(format!(
                "ABI vectors: {sym_name} publishes the descriptor in the terminator"
            ));
        }
    }
    *plans = new_set;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_tuple_only_in_slot_after_arguments() {
        for case in 0..3 {
            let mut m = Module::new();
            m.arc_memory = true;
            let val = |n| Operand::Val(ValueId(n));
            let read = |base, k| Instruction::LoadIndexed {
                base: val(base),
                index: Operand::Constant(Constant::Int(k)),
            };
            let index = match case {
                1 => val(2),
                _ => val(4),
            };
            let tuple_base = if case == 2 { 1 } else { 0 };
            m.functions.push(Function {
                symbol: "generica".into(),
                name: "generica".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "args".into(), Type::Ptr),
                    (ValueId(1), "desc".into(), Type::Ptr),
                ],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(2), read(1, 0), Type::I64),
                        (ValueId(3), read(1, 1), Type::I64),
                        (ValueId(4), Instruction::Add(val(2), val(3)), Type::I64),
                        (
                            ValueId(5),
                            Instruction::LoadIndexed {
                                base: val(tuple_base),
                                index,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(5))),
                }],
            });
            m.dart_vector_parameters.insert(
                "generica".into(),
                HashMap::from([
                    (ValueId(0), DartEntryVector::Arguments),
                    (ValueId(1), DartEntryVector::Descriptor),
                ]),
            );
            let mut plans = HashMap::from([("generica".into(), DartFunctionPlan::default())]);
            let r = produce(&m, &mut plans);
            if case == 0 {
                r.unwrap();
                assert_eq!(plans["generica"].classes[&ValueId(5)], Ownership::Trivial);
            } else {
                assert!(r.is_err(), "caso {case}");
            }
        }
    }

    #[test]
    fn ref_argument_requires_original_roles_and_rechecks_prepared_call() {
        for case in 0..5 {
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(Function {
                symbol: "entrada".into(),
                name: "entrada".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "args".into(), Type::Ptr),
                    (ValueId(1), "desc".into(), Type::Ptr),
                ],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::LoadIndexed {
                            base: Operand::Val(ValueId(0)),
                            index: Operand::Constant(Constant::Int(0)),
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                }],
            });
            m.dart_vector_parameters.insert(
                "entrada".into(),
                HashMap::from([
                    (ValueId(0), DartEntryVector::Arguments),
                    (ValueId(1), DartEntryVector::Descriptor),
                ]),
            );
            let mut p = DartFunctionPlan::default();
            p.tokens.ret = TokenReturn::Owned;
            let mut plans = HashMap::from([("entrada".into(), p)]);
            prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            assert!(checked_read(
                &m.functions[0].blocks[0].instructions[0].1,
                &m.dart_vector_parameters["entrada"]
            ));
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            match case {
                1 => m.dart_vector_parameters.clear(),
                2 => {
                    if let Instruction::CallRuntime { args, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        args.swap(0, 1);
                    }
                }
                3 => {
                    if let Instruction::CallRuntime { args, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        args[0].0 = Operand::Constant(Constant::Int(0));
                    }
                }
                4 => m.functions[0].blocks[0].instructions[0].2 = Type::I64,
                _ => {}
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            assert_eq!(result.is_ok(), case == 0, "caso {case}: {result:?}");
            assert_eq!(before, format!("{m:?}/{plans:?}"));
        }
    }

    #[test]
    fn descriptor_produces_counts_without_certifying_argument_words() {
        for case in 0..18 {
            let mut m = Module::new();
            m.arc_memory = case != 13;
            m.functions.push(Function {
                symbol: "entrada".into(),
                name: "entrada".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "sem_nome_semantico".into(), Type::Ptr),
                    (ValueId(1), "outro".into(), Type::Ptr),
                ],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::LoadIndexed {
                                base: Operand::Val(ValueId(0)),
                                index: Operand::Constant(Constant::Int(0)),
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::Add(
                                Operand::Val(ValueId(2)),
                                Operand::Constant(Constant::Int(1)),
                            ),
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
                }],
            });
            m.dart_vector_parameters.insert(
                "entrada".into(),
                HashMap::from([
                    (ValueId(0), DartEntryVector::Descriptor),
                    (ValueId(1), DartEntryVector::Arguments),
                ]),
            );
            let mut ps = HashMap::from([("entrada".into(), DartFunctionPlan::default())]);
            match case {
                1 => {
                    if let Instruction::LoadIndexed { index, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *index = Operand::Constant(Constant::Int(1));
                    }
                }
                2 => {
                    if let Instruction::LoadIndexed { base, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *base = Operand::Val(ValueId(1));
                    }
                }
                3 => m.functions[0].blocks[0].instructions[0].2 = Type::Ref,
                4 => {
                    if let Instruction::LoadIndexed { index, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *index = Operand::Constant(Constant::Int(2));
                    }
                }
                5 => {
                    m.dart_vector_parameters
                        .get_mut("entrada")
                        .unwrap()
                        .insert(ValueId(99), DartEntryVector::Descriptor);
                }
                6 => m.functions[0].params[0].2 = Type::I64,
                7 => {
                    ps.get_mut("entrada")
                        .unwrap()
                        .classes
                        .insert(ValueId(2), Ownership::Owned);
                }
                8 => m.functions[0].blocks[0].instructions.push((
                    ValueId(4),
                    Instruction::CallRuntime {
                        name: "externa".into(),
                        args: vec![(Operand::Val(ValueId(0)), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                9 => {
                    m.functions[0].blocks[0].terminator =
                        Terminator::Return(Some(Operand::Val(ValueId(0))))
                }
                10 => {
                    m.dart_scalar_parameters
                        .insert("entrada".into(), HashSet::from([ValueId(0)]));
                }
                11 => {
                    m.dart_vector_parameters
                        .insert("obsoleto".into(), HashMap::new());
                }
                12 => {
                    ps.get_mut("entrada").unwrap().classes.insert(
                        ValueId(0),
                        Ownership::Borrowed {
                            owner: OwnerOrigin::Caller,
                            scope: 0,
                        },
                    );
                }
                13 => {
                    m.dart_vector_parameters
                        .insert("obsoleto".into(), HashMap::new());
                }
                14 => m.dart_vector_parameters.clear(),
                15 => {
                    ps.get_mut("entrada").unwrap().tokens.instructions.insert(
                        ValueId(2),
                        TokenEffect {
                            may_fail: true,
                            ..Default::default()
                        },
                    );
                }
                16 => {
                    let p = m.functions[0].params[0].clone();
                    m.functions[0].params.push(p);
                }
                17 => {
                    let mut another = m.functions[0].clone();
                    another.symbol = "outra".into();
                    m.functions.insert(0, another);
                    let facts = m.dart_vector_parameters["entrada"].clone();
                    m.dart_vector_parameters.insert("outra".into(), facts);
                    ps.insert("outra".into(), DartFunctionPlan::default());
                }
                _ => {}
            }
            let before = format!("{m:?}/{ps:?}");
            let result = prepare_arc_dart_module(&mut m, &mut ps);
            if case == 13 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{ps:?}"), before);
            } else if (2..=16).contains(&case) {
                let error = result.expect_err(&format!("caso {case}"));
                if case == 2 {
                    assert!(error.contains("in entrada (origin v2)"), "{error}");
                }
                assert_eq!(format!("{m:?}/{ps:?}"), before, "caso {case}");
            } else {
                result.unwrap_or_else(|e| panic!("caso {case}: {e}"));
                for id in 0..4 {
                    assert_eq!(ps["entrada"].classes[&ValueId(id)], Ownership::Trivial);
                }
                let ready = format!("{m:?}");
                let classes = ps["entrada"].classes.clone();
                assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{m:?}"), ready);
                assert_eq!(ps["entrada"].classes, classes);
            }
        }
    }

    #[test]
    fn arity_mismatch_forwards_only_the_declared_vector_and_descriptor() {
        let roles: HashMap<_, _> = [
            (ValueId(1), DartEntryVector::Arguments),
            (ValueId(2), DartEntryVector::Descriptor),
        ]
        .into_iter()
        .collect();
        let call = |name: &str, args: u32, desc: u32, recv: u32| Instruction::CallRuntime {
            name: name.into(),
            args: vec![
                (Operand::Val(ValueId(recv)), Type::Ref),
                (Operand::Val(ValueId(3)), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64),
                (Operand::Val(ValueId(args)), Type::Ptr),
                (Operand::Val(ValueId(desc)), Type::Ptr),
            ],
            ret_ty: Type::Ref,
        };
        assert!(mismatch_forward(
            &call("dartforge_nsm_mismatch", 1, 2, 0),
            &roles
        ));
        let scoped = format!(
            "{}dartforge_nsm_mismatch",
            super::super::NATIVE_SCOPE_PREFIX
        );
        assert!(mismatch_forward(&call(&scoped, 1, 2, 0), &roles));
        // Swapped roles, a vector as the receiver, or another runtime function.
        assert!(!mismatch_forward(
            &call("dartforge_nsm_mismatch", 2, 1, 0),
            &roles
        ));
        assert!(!mismatch_forward(
            &call("dartforge_nsm_mismatch", 1, 2, 1),
            &roles
        ));
        assert!(!mismatch_forward(
            &call("dartforge_forward_nsm", 1, 2, 0),
            &roles
        ));
    }
}
