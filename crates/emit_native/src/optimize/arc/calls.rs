//! Summaries of the Dart convention bound to the verification of the callee body.
//! Extern contracts and supplied scopes remain premises.

use super::*;
use std::collections::{HashMap, HashSet};

/// Convention of a verified version of a Dart function.
///
/// Private fields prevent forging a summary without verifying the body.
/// Redo the summary after changing the function or its contracts. Does not certify
/// provenance, slot/keepalive aliases nor the supplied external contracts.
/// Conservative barriers of derived borrows are checked in the body.
/// Direct calls in the body are conservatively considered fallible,
/// as is the stack check required by the explicit context of the
/// body/tables. Frames computed by liveness still require metadata.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "empty".into(), name: "empty".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let summary = verify_dart_function_contract(&f, &HashMap::new(), &TokenPlan::default(),
///     &FunctionTables::default(), &ScopePlan::default())?;
/// # let _ = summary;
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone)]
pub struct DartFunctionContract {
    symbol: String,
    parameters: Vec<Type>,
    ret: Type,
    may_fail: bool,
}

// Only the atomic preparation of the set may use this temporary summary.
// No body/contract is published before verifying every convention.
pub(super) fn provisional_summary(f: &Function, may_fail: bool) -> DartFunctionContract {
    DartFunctionContract {
        symbol: f.symbol.clone(),
        parameters: f.params.iter().map(|(_, _, ty)| *ty).collect(),
        ret: f.return_ty,
        may_fail,
    }
}

/// The summary of a Dart function of another prepared image, from the
/// `CallStatic` call of `f`: the representation of each operand (as the emitter
/// declares it), the return of the call and possible failure.
pub(super) fn external_summary(f: &Function, symbol: &str, args: &[Operand], ret: Type) -> DartFunctionContract {
    let types: HashMap<ValueId, Type> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)))
        .collect();
    let ty = |o: &Operand| match o {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
    };
    DartFunctionContract {
        symbol: symbol.to_string(),
        parameters: args.iter().map(ty).map(|t| if t == Type::Void { Type::I64 } else { t }).collect(),
        ret,
        may_fail: true,
    }
}

/// The external summaries ([`external_summary`]) of the direct calls of
/// `functions` to symbols outside `locals`; calls of the same symbol with
/// different signatures are an error.
pub(super) fn external_summaries(functions: &[Function], locals: &HashSet<String>) -> Result<Vec<DartFunctionContract>, String> {
    let mut by_symbol: HashMap<String, DartFunctionContract> = HashMap::new();
    for f in functions {
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallStatic { symbol, args, ret_ty } = inst else { continue };
            if locals.contains(symbol) {
                continue;
            }
            let r = external_summary(f, symbol, args, *ret_ty);
            match by_symbol.get(symbol) {
                Some(previous) if (&previous.parameters, previous.ret) != (&r.parameters, r.ret) => {
                    return Err(format!("callee externo com assinaturas divergentes: {symbol}"));
                }
                Some(_) => {}
                None => {
                    by_symbol.insert(symbol.clone(), r);
                }
            }
        }
    }
    let mut summaries: Vec<_> = by_symbol.into_values().collect();
    summaries.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    Ok(summaries)
}

// The set reuses this index in every body. Rebuilding it per
// function would make the preparation quadratic in the number of SDK functions.
pub(super) struct DartFunctionIndex<'a> {
    by_symbol: HashMap<&'a String, &'a DartFunctionContract>,
}

impl<'a> DartFunctionIndex<'a> {
    pub(super) fn new(summaries: &'a [DartFunctionContract]) -> Result<Self, String> {
        let mut by_symbol = HashMap::with_capacity(summaries.len());
        for resumo in summaries {
            if by_symbol.insert(&resumo.symbol, resumo).is_some() {
                return Err(format!("resumo Dart repetido: {}", resumo.symbol));
            }
        }
        Ok(Self { by_symbol })
    }
}

/// Verifies the Dart convention of the body and extracts a summary for direct calls.
///
/// The given maps are not changed. Non-Ref parameters need an explicit/produced
/// Trivial classification; they are not scalars by width.
/// The caller must use the summary only with this version of the body and ABI.
///
/// # Errors
/// Failure of the full Dart verifier or a non-Ref parameter that is not Trivial.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "empty".into(), name: "empty".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// verify_dart_function_contract(&f, &HashMap::new(), &TokenPlan::default(),
///     &FunctionTables::default(), &ScopePlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_dart_function_contract(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<DartFunctionContract, String> {
    let mut classes = classes.clone();
    let mut plan = plan.clone();
    produce_and_verify_dart_tokens(f, &mut classes, &mut plan, tables, scopes)?;
    for (v, _, ty) in &f.params {
        if *ty != Type::Ref && classes[v] != Ownership::Trivial {
            return Err(format!("parâmetro não Ref v{} não é Trivial", v.0));
        }
    }
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    for b in &f.blocks {
        if matches!(
            tables.exits.get(&b.id),
            Some(ExceptionExit::Throws | ExceptionExit::Resume)
        ) {
            continue;
        }
        if let Terminator::Return(op) = &b.terminator {
            let compatible = match op {
                None => f.return_ty == Type::Void,
                Some(op) => operand_has_type(op, f.return_ty, &types),
            };
            if !compatible {
                return Err(format!(
                    "representação de retorno Dart incompatível: {}",
                    f.symbol
                ));
            }
        }
    }
    let unknown_set = HashSet::new();
    let may_fail = crate::llvm::LlvmEmitter::requires_explicit_context(f, Some(tables))
        || !tables.exits.is_empty()
        || f.blocks.iter().any(|b| {
            matches!(b.terminator, Terminator::Throw(_))
                || b.instructions.iter().any(|(_, inst, _)| {
                    super::super::effects::instruction_throws(inst, &unknown_set)
                })
        });
    Ok(DartFunctionContract {
        symbol: f.symbol.clone(),
        parameters: f.params.iter().map(|(_, _, ty)| *ty).collect(),
        ret: f.return_ty,
        may_fail,
    })
}

fn operand_has_type(op: &Operand, expected: Type, types: &HashMap<ValueId, Type>) -> bool {
    match op {
        Operand::Val(v) => types.get(v) == Some(&expected),
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            expected == Type::Ref
        }
        Operand::Constant(Constant::Int(_)) => expected == Type::I64,
        Operand::Constant(Constant::Bool(_)) => expected == Type::I1,
        Operand::Constant(Constant::Double(_)) => expected == Type::F64,
        _ => false,
    }
}

/// Produces CallStatic contracts using summaries of verified Dart bodies.
///
/// Ref arguments are borrowed during the call; a Ref result is Owned.
/// Other SSA arguments require a Trivial class, besides the exact ABI type.
/// A non-Ref result is Trivial by the verified convention, not by width.
/// A conservative failure requires an exceptional CFG in the later verifier.
/// Does not cover dynamic dispatch, imported stubs or hidden external retentions.
///
/// # Errors
/// Missing/repeated symbol, incompatible signature or a conflict in the maps.
/// No entry is published on error. Does not verify the caller's flow.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let callee = Function { symbol: "empty".into(), name: "empty".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let summary = verify_dart_function_contract(&callee, &HashMap::new(), &TokenPlan::default(),
///     &FunctionTables::default(), &ScopePlan::default())?;
/// let mut caller = callee.clone();
/// caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "empty".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// produce_dart_calls(&caller, &[summary], &mut HashMap::new(), &mut TokenPlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_dart_calls(
    f: &Function,
    summaries: &[DartFunctionContract],
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let index = DartFunctionIndex::new(summaries)?;
    produce_calls(f, &index, classes, plan, false)
}

// Used only inside the transaction of the set: the provisional results
// feed the classification of the other instructions before checking uses.
pub(super) fn produce_dart_calls_and_instructions(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    produce_calls(f, index, classes, plan, true)
}

/// Classifies parameters, direct Dart calls and instructions with a contract,
/// without resolving Phis nor checking arguments: the basis for deciding which
/// Phi entries need a copy (`phis.rs`). Publishes nothing on error.
pub(super) fn classify_without_phis(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(f, &mut new_classes)?;
    classify_statics(f, index, &mut new_classes, &mut new_plan)?;
    super::contracts::produce_contracts_without_phi(f, &mut new_classes, &mut new_plan)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

/// The result of each direct Dart call through the callee summary (Owned if
/// Ref, otherwise Trivial) and the failure effect; returns the arguments to check.
fn classify_statics<'f, 'a>(
    f: &'f Function,
    index: &DartFunctionIndex<'a>,
    new_classes: &mut HashMap<ValueId, Ownership>,
    new_plan: &mut TokenPlan,
) -> Result<Vec<(&'f Vec<Operand>, &'a DartFunctionContract)>, String> {
    let mut calls = Vec::new();
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = inst
            else {
                continue;
            };
            let resumo = index
                .by_symbol
                .get(symbol)
                .ok_or_else(|| format!("callee Dart sem resumo: {symbol}"))?;
            if args.len() != resumo.parameters.len() || *ret_ty != resumo.ret || ty != ret_ty {
                return Err(format!("assinatura Dart incompatível: {symbol}"));
            }
            let class = if *ret_ty == Type::Ref {
                Ownership::Owned
            } else {
                Ownership::Trivial
            };
            let effect = TokenEffect {
                may_fail: resumo.may_fail,
                ..Default::default()
            };
            if new_classes.get(v).is_some_and(|c| *c != class)
                || new_plan.instructions.get(v).is_some_and(|e| *e != effect)
            {
                return Err(format!("contrato Dart conflitante em v{}", v.0));
            }
            new_classes.insert(*v, class);
            new_plan.instructions.insert(*v, effect);
            calls.push((args, *resumo));
        }
    }
    Ok(calls)
}

fn produce_calls(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    classify_instructions: bool,
) -> Result<(), String> {
    super::ssa::verify(f)?;
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    // The convention of Ref parameters does not depend on the contracts of the calls.
    produce_dart_ref_parameters(f, &mut new_classes)?;
    let calls = classify_statics(f, index, &mut new_classes, &mut new_plan)?;
    // The results have semantic contracts from the callees, regardless of
    // the physical order of the blocks. SSA already checked their dominance at the uses.
    if classify_instructions {
        produce_arc_contracts(f, &mut new_classes, &mut new_plan)?;
    }
    for (args, resumo) in calls {
        for (op, expected) in args.iter().zip(&resumo.parameters) {
            let compatible = operand_has_type(op, *expected, &types)
                && match op {
                    Operand::Val(arg) => {
                        *expected == Type::Ref
                            || new_classes.get(arg) == Some(&Ownership::Trivial)
                    }
                    Operand::Constant(_) => true,
                };
            if !compatible {
                return Err(format!("argumento Dart incompatível: {}", resumo.symbol));
            }
        }
    }
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallible_call_cleans_temporary_without_producing_result_on_error() {
        let (mut callee, callee_plan) = identity();
        // The contract of the external is an explicit premise; the conservative summary
        // marks the callee as fallible because it contains a direct call.
        callee.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "externa".into(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut external_plan = callee_plan;
        external_plan
            .instructions
            .insert(ValueId(1), TokenEffect::default());
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::from([(ValueId(1), Ownership::Owned)]),
            &external_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let mut caller = Function {
            symbol: "caller_falivel".into(),
            name: "caller_falivel".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallStatic {
                                symbol: callee.symbol,
                                args: vec![Operand::Val(ValueId(1))],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
            ],
        };
        let original = caller.clone();
        let mut tables = FunctionTables::default();
        tables.invocations.insert(ValueId(2), BlockId(1));
        tables.landing_pads.insert(BlockId(1));
        let mut plan = TokenPlan {
            ret: TokenReturn::Owned,
            ..Default::default()
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan).unwrap();
        assert_eq!(
            insert_arc_dart_exits(
                &mut caller,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 2)
        );
        for b in &caller.blocks[1..] {
            assert_eq!(b.instructions.len(), 1);
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        assert!(matches!(
            caller.blocks[2].terminator,
            Terminator::Return(Some(Operand::Val(ValueId(2))))
        ));
        produce_and_verify_dart_tokens(
            &caller,
            &mut classes,
            &mut plan,
            &tables,
            &ScopePlan::default(),
        )
        .unwrap();
        // The result does not exist on error, even with a Ref type and an Owned class.
        caller = original;
        caller.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        classes.retain(|v, _| v.0 <= 2);
        let before = format!("{caller:?}");
        let classes_before = classes.clone();
        let effects_before = plan.instructions.clone();
        let error = insert_arc_dart_exits(
            &mut caller,
            &mut classes,
            &mut plan,
            &tables,
            &ScopePlan::default(),
        )
        .unwrap_err();
        assert!(
            error.contains("ARC003")
                && error.contains("resultado v2 usado sem atravessar sua aresta de sucesso"),
            "{error}"
        );
        assert_eq!(format!("{caller:?}"), before);
        assert_eq!(classes, classes_before);
        assert_eq!(plan.instructions, effects_before);
    }

    #[test]
    fn scalar_call_chain_is_independent_of_physical_block_order() {
        let create = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(42)))),
            }],
        };
        let mut identity = create.clone();
        identity.symbol = "identidade_escalar".into();
        identity.params.push((ValueId(0), "x".into(), Type::I64));
        identity.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let plan = TokenPlan::default();
        let summaries = [
            verify_dart_function_contract(
                &create,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap(),
            verify_dart_function_contract(
                &identity,
                &HashMap::from([(ValueId(0), Ownership::Trivial)]),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap(),
        ];
        let mut caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(2)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: identity.symbol.clone(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::CallStatic {
                            symbol: create.symbol,
                            args: vec![],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(1)),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = plan;
        produce_dart_calls(&caller, &summaries, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        produce_and_verify_dart_tokens(
            &caller,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        // The same width without a semantic contract is still insufficient.
        caller
            .params
            .push((ValueId(3), "sem_contrato".into(), Type::I64));
        if let Instruction::CallStatic { args, .. } = &mut caller.blocks[1].instructions[0].1 {
            args[0] = Operand::Val(ValueId(3));
        }
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_dart_calls(&caller, &summaries, &mut classes, &mut plan)
                .unwrap_err()
                .contains("argumento")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn summary_checks_scalar_return_and_fallible_call_requires_exit() {
        let (mut callee, mut plan) = identity();
        callee.params.clear();
        callee.return_ty = Type::I64;
        callee.blocks[0].instructions[0] =
            (ValueId(1), Instruction::Const(Constant::Int(42)), Type::I64);
        plan.ret = TokenReturn::Trivial;
        verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        callee.blocks[0].instructions[0] = (
            ValueId(1),
            Instruction::Const(Constant::Bool(true)),
            Type::I1,
        );
        assert!(
            verify_dart_function_contract(
                &callee,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("representação")
        );
        let (mut callee, plan) = identity();
        callee.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "externa_com_contrato_fornecido".into(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut callee_plan = plan.clone();
        callee_plan
            .instructions
            .insert(ValueId(1), TokenEffect::default());
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::from([(ValueId(1), Ownership::Owned)]),
            &callee_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert!(resumo.may_fail);
        let (mut caller, mut plan) = identity();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol,
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan).unwrap();
        assert!(plan.instructions[&ValueId(1)].may_fail);
        assert!(
            produce_and_verify_dart_tokens(
                &caller,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
    }

    fn identity() -> (Function, TokenPlan) {
        (
            Function {
                symbol: "identidade".into(),
                name: "identidade".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            },
            TokenPlan {
                ret: TokenReturn::Owned,
                ..Default::default()
            },
        )
    }

    #[test]
    fn direct_call_receives_owned_from_verified_callee_without_manual_contract() {
        let (callee, callee_plan) = identity();
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &callee_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert!(!resumo.may_fail);
        let (mut caller, mut plan) = identity();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo.clone()], &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        assert_eq!(
            insert_arc_dart_exits(
                &mut caller,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        // A later unknown call does not publish even the first call.
        caller.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "ausente".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan)
                .unwrap_err()
                .contains("sem resumo")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn borrowed_body_and_same_width_argument_do_not_certify_call() {
        let (mut callee, plan) = identity();
        callee.blocks[0].instructions.clear();
        callee.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        assert!(
            verify_dart_function_contract(
                &callee,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        let (callee, plan) = identity();
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let (mut caller, mut plan) = identity();
        caller.params[0].2 = Type::I64;
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        assert!(
            produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan)
                .unwrap_err()
                .contains("argumento")
        );
        assert_eq!(classes.len(), 1);
        assert!(plan.instructions.is_empty());
    }
}
