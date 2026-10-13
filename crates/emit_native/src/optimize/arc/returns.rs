//! Nominal Owned conventions of Dart bodies and generated uniform entries.
//! An isolated Ref representation does not certify the ABI of another producer.

use super::*;

/// Produces the Owned convention of the Ref returns recorded by the Dart lowering.
///
/// Facts must match the current symbols and representations. The default
/// Trivial plan is filled in; Owned stays and Borrowed conflicts with the ABI.
/// This does not certify the body: insertion and verification must prove the transfer
/// of the token, including retention of borrowed returns and cleanup of failures.
/// The uniform entry requires its own declaration and a binding to the ABI vectors.
/// That fact does not certify indirect callers, C callbacks or an imported SDK.
/// Tracing does not change plans. Returns the number of conventions added.
///
/// # Errors
/// Duplicated/stale symbol, missing plan, non-Ref return or incompatible
/// convention. No plan is changed, even if other facts are valid.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.arc_memory = true;
/// m.functions.push(Function { symbol: "identity".into(), name: "identity".into(),
///     debug_info: None, params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0),
///     instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
/// m.dart_ref_returns.insert("identity".into());
/// let mut plans = HashMap::from([("identity".into(), DartFunctionPlan::default())]);
/// assert_eq!(produce_lowering_ref_returns(&m, &mut plans)?, 1);
/// assert_eq!(prepare_arc_dart_module(&mut m, &mut plans)?, (1, 0));
/// # Ok::<(), String>(())
/// ```
pub fn produce_lowering_ref_returns(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    if !module.arc_memory {
        return Ok(0);
    }
    let mut functions = HashMap::new();
    for f in &module.functions {
        if functions.insert(&f.symbol, f).is_some() {
            return Err(format!("retorno Dart: símbolo repetido {}", f.symbol));
        }
    }
    // The uniform ABI needs its own declaration: neither Ref nor the vectors
    // alone declare the return mode. Revalidates the binding to the same HIR.
    for sym_name in &module.arc_uniform_returns {
        let f = functions.get(sym_name).ok_or_else(|| format!("retorno uniforme ARC: símbolo obsoleto {sym_name}"))?;
        let roles = module.dart_vector_parameters.get(sym_name);
        if f.return_ty != Type::Ref || f.params.iter().map(|p| p.2).collect::<Vec<_>>() != [Type::Ref, Type::Ptr, Type::Ptr]
            || !roles.is_some_and(|p| p.len() == 2
                && p.get(&f.params[1].0) == Some(&DartEntryVector::Argumentos)
                && p.get(&f.params[2].0) == Some(&DartEntryVector::Descriptor)) {
            return Err(format!("retorno uniforme ARC: declaração/ABI de {sym_name} incompatível"));
        }
    }
    let mut facts: Vec<_> = module.dart_ref_returns.union(&module.arc_uniform_returns).collect();
    facts.sort();
    let mut new_set = Vec::new();
    for sym_name in facts {
        let f = functions
            .get(sym_name)
            .ok_or_else(|| format!("retorno Dart: símbolo obsoleto {sym_name}"))?;
        let plan = plans
            .get(sym_name)
            .ok_or_else(|| format!("retorno Dart: plano ausente {sym_name}"))?;
        if f.return_ty != Type::Ref {
            return Err(format!("retorno Dart: {sym_name} não devolve Ref"));
        }
        match plan.tokens.ret {
            TokenReturn::Trivial => new_set.push(sym_name),
            TokenReturn::Owned => {}
            TokenReturn::Borrowed => {
                return Err(format!("retorno Dart: convenção conflitante {sym_name}"));
            }
        }
    }
    for sym_name in &new_set {
        plans.get_mut(*sym_name).unwrap().tokens.ret = TokenReturn::Owned;
    }
    Ok(new_set.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_owned_return_requires_declaration_and_caller_with_contract() {
        for case in 0..17 {
            let mut m = Module::new();
            m.arc_memory = case != 12;
            let target = Function {
                symbol: "target".into(), name: "target".into(), debug_info: None,
                params: vec![(ValueId(0), "receptor".into(), Type::Ref),
                    (ValueId(1), "vetor".into(), Type::Ptr), (ValueId(2), "assinatura".into(), Type::Ptr)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
            };
            let mut caller = target.clone(); caller.symbol = "caller".into();
            caller.blocks[0].instructions.push((ValueId(3), Instruction::CallStatic {
                symbol: "target".into(), args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1)), Operand::Val(ValueId(2))],
                ret_ty: Type::Ref }, Type::Ref));
            caller.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
            m.functions = vec![target, caller];
            for sym_name in ["target", "caller"] {
                m.dart_vector_parameters.insert(sym_name.into(), HashMap::from([
                    (ValueId(1), DartEntryVector::Argumentos), (ValueId(2), DartEntryVector::Descriptor)]));
                m.arc_uniform_returns.insert(sym_name.into());
            }
            let mut ps = HashMap::from([("target".into(), DartFunctionPlan::default()),
                ("caller".into(), DartFunctionPlan::default())]);
            match case {
                1 => m.functions.reverse(),
                2 => m.functions[1].blocks[0].instructions[0].1 = Instruction::CallClosure {
                    closure: Operand::Val(ValueId(0)), args: vec![], names: vec![],
                    ret_ty: Type::Ref, tuple_types: Operand::Constant(Constant::Int(0)) },
                3 => m.arc_uniform_returns.clear(),
                4 => ps.get_mut("target").unwrap().tokens.ret = TokenReturn::Borrowed,
                5 => m.functions[0].params[1].2 = Type::Ref,
                6 => { m.dart_vector_parameters.remove("target"); },
                7 => { m.dart_vector_parameters.insert("target".into(), HashMap::from([
                    (ValueId(1), DartEntryVector::Descriptor), (ValueId(2), DartEntryVector::Argumentos)])); },
                8 | 12 => { m.arc_uniform_returns.insert("obsoleto".into()); },
                9 => m.functions[0].return_ty = Type::I64,
                10 => m.functions[0].params.push((ValueId(4), "extra".into(), Type::Ptr)),
                11 => m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Int(7)))),
                13 => {
                    m.functions[0].blocks[0].instructions.push((ValueId(4), Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)), from: Type::I64 }, Type::Ref));
                    m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(4))));
                }
                14 => m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null))),
                15 => if let Instruction::CallStatic { symbol, .. } = &mut m.functions[1].blocks[0].instructions[0].1 {
                    *symbol = "externa".into();
                },
                16 => m.functions[0].blocks[0].instructions.push((ValueId(4), Instruction::CallRuntime {
                    name: "externa".into(), args: vec![(Operand::Val(ValueId(2)), Type::Ptr)], ret_ty: Type::Void }, Type::Void)),
                _ => {}
            }
            let before = format!("{m:?}/{ps:?}");
            let result = prepare_arc_dart_module(&mut m, &mut ps);
            if (2..12).contains(&case) || case >= 15 {
                let error = result.expect_err(&format!("caso {case}"));
                if case == 2 { assert!(error.contains("v3 exige guarda pending"), "{error}"); }
                assert_eq!(format!("{m:?}/{ps:?}"), before, "caso {case}");
            } else if case == 12 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{ps:?}"), before);
            } else {
                let total = result.unwrap_or_else(|e| panic!("caso {case}: {e}"));
                assert_eq!(total, (usize::from(case < 13), 0));
                assert_eq!(ps["target"].tokens.ret, TokenReturn::Owned);
                assert_eq!(ps["caller"].classes[&ValueId(3)], Ownership::Owned);
                let caller = m.functions.iter().find(|f| f.symbol == "caller").unwrap();
                assert!(!caller.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
                    Instruction::ArcCopy { value: Operand::Val(ValueId(3)) }
                        | Instruction::ArcDrop { value: Operand::Val(ValueId(3)) })));
                if case == 13 {
                    let target = m.functions.iter().find(|f| f.symbol == "target").unwrap();
                    assert!(target.blocks[0].instructions.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == "dartforge_arc_box_int_owned_v1")));
                }
                let ready = format!("{m:?}");
                let classes = ps["caller"].classes.clone();
                assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{m:?}"), ready);
                assert_eq!(ps["caller"].classes, classes);
            }
        }
    }

    #[test]
    fn return_facts_are_atomic_idempotent_and_require_provenance() {
        for case in 0..8 {
            let mut m = Module::new();
            m.arc_memory = case != 6;
            for sym_name in ["a", "z"] {
                m.functions.push(Function {
                    symbol: sym_name.into(),
                    name: sym_name.into(),
                    debug_info: None,
                    params: vec![(ValueId(0), "x".into(), Type::Ref)],
                    return_ty: Type::Ref,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                    }],
                });
                if case != 7 {
                    m.dart_ref_returns.insert(sym_name.into());
                }
            }
            let mut plans = HashMap::from([
                ("a".into(), DartFunctionPlan::default()),
                ("z".into(), DartFunctionPlan::default()),
            ]);
            match case {
                1 => {
                    m.dart_ref_returns.insert("obsoleto".into());
                }
                2 => {
                    plans.remove("z");
                }
                3 => m.functions[1].return_ty = Type::I64,
                4 => plans.get_mut("z").unwrap().tokens.ret = TokenReturn::Borrowed,
                5 => m.functions[1].symbol = "a".into(),
                _ => {}
            }
            let before = format!("{m:?}/{plans:?}");
            let result = produce_lowering_ref_returns(&m, &mut plans);
            if (1..6).contains(&case) {
                assert!(result.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else if case >= 6 {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(result.unwrap(), 2);
                assert_eq!(
                    produce_lowering_ref_returns(&m, &mut plans).unwrap(),
                    0
                );
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (2, 0)
                );
                assert!(m.functions.iter().all(|f| {
                    f.blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| {
                            matches!(
                                i,
                                Instruction::ArcCopy {
                                    value: Operand::Val(ValueId(0))
                                }
                            )
                        })
                }));
            }
        }
    }
}
