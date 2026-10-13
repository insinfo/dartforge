//! Transporta fatos semânticos do lowering aos planos da mesma versão da HIR.
use super::*;
use std::collections::HashMap;

/// Classifica parâmetros escalares explicitamente registrados pelo lowering.
///
/// Revalida símbolos, IDs e representações antes de publicar classes Trivial.
/// A origem semântica dos fatos continua sendo responsabilidade do lowering;
/// i64 sem fato não é classificado. Tracing devolve zero sem alterar planos.
/// Não prepara tabelas, retornos, instruções, parâmetros ocultos ou borrows.
/// Retorna o número de novas classificações; repetir fatos válidos devolve zero.
///
/// # Erros
/// Símbolo/ID obsoleto, parâmetros repetidos, plano ausente, representação
/// incompatível ou classe conflitante. Nenhuma classe é publicada em erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new();
/// m.memoria_arc = true;
/// m.functions.push(Function { symbol: "int_dart".into(), name: "int_dart".into(),
///     depuracao: None, params: vec![(ValueId(0), "numero".into(), Type::I64)],
///     return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///     terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
/// m.parametros_escalares_dart.insert("int_dart".into(), HashSet::from([ValueId(0)]));
/// let mut planos = HashMap::from([("int_dart".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(produzir_parametros_escalares_do_lowering(&m, &mut planos)?, 1);
/// assert_eq!(inserir_arc_funcoes_dart(&mut m.functions, &mut planos)?, (0, 0));
/// # Ok::<(), String>(())
/// ```
pub fn produce_lowering_scalar_parameters(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    produce(module, plans, false)
}

/// Classifica como Trivial os IDs RTI registrados pelo produtor nativo.
///
/// Exige parâmetro I64 e fato RTI explícito, sem marca de escalar Dart no
/// mesmo ID. Não certifica vida do universo, pins ou recarga de módulos.
/// Tracing não altera planos; repetir fatos válidos devolve zero.
///
/// # Erros
/// Fato obsoleto, representação diferente de I64, origens conflitantes,
/// plano ausente ou ownership incompatível. Publicação é atômica.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new();
/// m.memoria_arc = true;
/// m.functions.push(Function { symbol: "generica".into(), name: "generica".into(),
///     depuracao: None, params: vec![(ValueId(0), "$tipos".into(), Type::I64)],
///     return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///     terminator: Terminator::Return(None) }] });
/// m.parametros_rti_dart.insert("generica".into(), HashSet::from([ValueId(0)]));
/// let mut planos = HashMap::from([("generica".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(produzir_parametros_rti_do_lowering(&m, &mut planos)?, 1);
/// assert_eq!(inserir_arc_funcoes_dart(&mut m.functions, &mut planos)?, (0, 0));
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
                "fatos de parâmetros: símbolo repetido {}",
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
            .ok_or_else(|| format!("fatos de parâmetros: símbolo obsoleto {symbol}"))?;
        let plan = plans
            .get(symbol)
            .ok_or_else(|| format!("fatos de parâmetros: plano ausente {symbol}"))?;
        let types: HashMap<_, _> = f.params.iter().map(|(v, _, ty)| (*v, *ty)).collect();
        if types.len() != f.params.len() {
            return Err(format!(
                "fatos de parâmetros: parâmetros repetidos {symbol}"
            ));
        }
        let mut ids: Vec<_> = ids.iter().copied().collect();
        ids.sort_by_key(|v| v.0);
        for v in ids {
            if others.get(symbol).is_some_and(|ids| ids.contains(&v)) {
                return Err(format!(
                    "fato {symbol} v{} tem origens escalar/RTI conflitantes",
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
                    "fato de parâmetro {symbol} v{} não corresponde à representação declarada",
                    v.0
                ));
            }
            match plan.classes.get(&v) {
                None => new_set.push((symbol, v)),
                Some(Ownership::Trivial) => {}
                _ => {
                    return Err(format!(
                        "fato de parâmetro {symbol} v{} conflita com ownership",
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
