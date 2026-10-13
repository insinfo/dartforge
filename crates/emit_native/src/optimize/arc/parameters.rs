//! Carries semantic facts from the lowering to the plans of the same HIR version.
use super::*;
use std::collections::HashMap;

/// Classifies scalar parameters explicitly recorded by the lowering.
///
/// Revalidates symbols, IDs and representations before publishing Trivial classes.
/// The semantic origin of the facts remains the lowering's responsibility;
/// an i64 without a fact is not classified. Tracing returns zero without changing plans.
/// Does not prepare tables, returns, instructions, hidden parameters or borrows.
/// Returns the number of new classifications; repeating valid facts returns zero.
///
/// # Errors
/// Stale symbol/ID, repeated parameters, missing plan, incompatible
/// representation or conflicting class. No class is published on error.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new();
/// m.arc_memory = true;
/// m.functions.push(Function { symbol: "int_dart".into(), name: "int_dart".into(),
///     debug_info: None, params: vec![(ValueId(0), "number".into(), Type::I64)],
///     return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///     terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
/// m.dart_scalar_parameters.insert("int_dart".into(), HashSet::from([ValueId(0)]));
/// let mut plans = HashMap::from([("int_dart".into(), DartFunctionPlan::default())]);
/// assert_eq!(produce_lowering_scalar_parameters(&m, &mut plans)?, 1);
/// assert_eq!(insert_arc_dart_functions(&mut m.functions, &mut plans)?, (0, 0));
/// # Ok::<(), String>(())
/// ```
pub fn produce_lowering_scalar_parameters(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    produce(module, plans, false)
}

/// Classifies as Trivial the RTI IDs recorded by the native producer.
///
/// Requires an I64 parameter and an explicit RTI fact, without a Dart scalar mark on the
/// same ID. Does not certify the lifetime of the universe, pins or module reload.
/// Tracing does not change plans; repeating valid facts returns zero.
///
/// # Errors
/// Stale fact, representation other than I64, conflicting origins,
/// missing plan or incompatible ownership. Publication is atomic.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new();
/// m.arc_memory = true;
/// m.functions.push(Function { symbol: "generic".into(), name: "generic".into(),
///     debug_info: None, params: vec![(ValueId(0), "$types".into(), Type::I64)],
///     return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///     terminator: Terminator::Return(None) }] });
/// m.dart_rti_parameters.insert("generic".into(), HashSet::from([ValueId(0)]));
/// let mut plans = HashMap::from([("generic".into(), DartFunctionPlan::default())]);
/// assert_eq!(produce_lowering_rti_parameters(&m, &mut plans)?, 1);
/// assert_eq!(insert_arc_dart_functions(&mut m.functions, &mut plans)?, (0, 0));
/// # Ok::<(), String>(())
/// ```
pub fn produce_lowering_rti_parameters(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    produce(module, plans, true)
}

fn produce(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
    rti: bool,
) -> Result<usize, String> {
    if !module.arc_memory {
        return Ok(0);
    }
    let mut functions = HashMap::new();
    for f in &module.functions {
        if functions.insert(&f.symbol, f).is_some() {
            return Err(format!(
                "parameter facts: repeated symbol {}",
                f.symbol
            ));
        }
    }
    let (origins, others) = if rti {
        (
            &module.dart_rti_parameters,
            &module.dart_scalar_parameters,
        )
    } else {
        (
            &module.dart_scalar_parameters,
            &module.dart_rti_parameters,
        )
    };
    let mut facts: Vec<_> = origins.iter().collect();
    facts.sort_by_key(|(s, _)| *s);
    let mut new_set = Vec::new();
    for (symbol, ids) in facts {
        let f = functions
            .get(symbol)
            .ok_or_else(|| format!("parameter facts: stale symbol {symbol}"))?;
        let plan = plans
            .get(symbol)
            .ok_or_else(|| format!("parameter facts: missing plan {symbol}"))?;
        let types: HashMap<_, _> = f.params.iter().map(|(v, _, ty)| (*v, *ty)).collect();
        if types.len() != f.params.len() {
            return Err(format!(
                "parameter facts: repeated parameters {symbol}"
            ));
        }
        let mut ids: Vec<_> = ids.iter().copied().collect();
        ids.sort_by_key(|v| v.0);
        for v in ids {
            if others.get(symbol).is_some_and(|ids| ids.contains(&v)) {
                return Err(format!(
                    "fact {symbol} v{} has conflicting scalar/RTI origins",
                    v.0
                ));
            }
            if if rti {
                types.get(&v) != Some(&Type::I64)
            } else {
                !matches!(
                    types.get(&v),
                    Some(Type::I64 | Type::F64 | Type::I1 | Type::I8)
                )
            } {
                return Err(format!(
                    "parameter fact {symbol} v{} does not match the declared representation",
                    v.0
                ));
            }
            match plan.classes.get(&v) {
                None => new_set.push((symbol, v)),
                Some(Ownership::Trivial) => {}
                _ => {
                    return Err(format!(
                        "parameter fact {symbol} v{} conflicts with ownership",
                        v.0
                    ));
                }
            }
        }
    }
    let total = new_set.len();
    for (symbol, v) in new_set {
        plans
            .get_mut(symbol)
            .unwrap()
            .classes
            .insert(v, Ownership::Trivial);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_facts_do_not_publish_partial_classification_and_tracing_does_not_apply_them() {
        for rti in [false, true] {
            for variant in 0..8 {
                let mut m = Module::new();
                m.arc_memory = true;
                for symbol in ["a", "z"] {
                    m.functions.push(Function {
                        symbol: symbol.into(),
                        name: symbol.into(),
                        debug_info: None,
                        params: vec![(ValueId(0), "x".into(), Type::I64)],
                        return_ty: Type::Void,
                        blocks: vec![BasicBlock {
                            id: BlockId(0),
                            instructions: vec![],
                            terminator: Terminator::Return(None),
                        }],
                    });
                    if rti {
                        &mut m.dart_rti_parameters
                    } else {
                        &mut m.dart_scalar_parameters
                    }
                    .insert(symbol.into(), HashSet::from([ValueId(0)]));
                }
                let mut ps = HashMap::from([
                    ("a".into(), DartFunctionPlan::default()),
                    ("z".into(), DartFunctionPlan::default()),
                ]);
                match variant {
                    0 => {
                        if rti {
                            &mut m.dart_rti_parameters
                        } else {
                            &mut m.dart_scalar_parameters
                        }
                        .get_mut("z")
                        .unwrap()
                        .insert(ValueId(9));
                    }
                    1 => {
                        m.functions[1].params[0].2 = Type::Ref;
                    }
                    2 => {
                        ps.get_mut("z")
                            .unwrap()
                            .classes
                            .insert(ValueId(0), Ownership::Owned);
                    }
                    3 => {
                        ps.remove("z");
                    }
                    4 => {
                        m.functions.pop();
                    }
                    5 => {
                        if rti {
                            &mut m.dart_scalar_parameters
                        } else {
                            &mut m.dart_rti_parameters
                        }
                        .insert("z".into(), HashSet::from([ValueId(0)]));
                    }
                    6 => {
                        let p = m.functions[1].params[0].clone();
                        m.functions[1].params.push(p);
                    }
                    _ => {
                        m.functions.push(m.functions[1].clone());
                    }
                }
                let before = format!("{ps:?}");
                assert!(produce(&m, &mut ps, rti).is_err());
                assert_eq!(format!("{ps:?}"), before);
                m.arc_memory = false;
                assert_eq!(produce(&m, &mut ps, rti).unwrap(), 0);
                assert_eq!(format!("{ps:?}"), before);
            }
        }
    }
}
