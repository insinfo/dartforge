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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_parametros_escalares_do_lowering(
    modulo: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<usize, String> {
    if !modulo.memoria_arc {
        return Ok(0);
    }
    let mut funcoes = HashMap::new();
    for f in &modulo.functions {
        if funcoes.insert(&f.symbol, f).is_some() {
            return Err(format!("fatos escalares: símbolo repetido {}", f.symbol));
        }
    }
    let mut fatos: Vec<_> = modulo.parametros_escalares_dart.iter().collect();
    fatos.sort_by_key(|(s, _)| *s);
    let mut novos = Vec::new();
    for (simbolo, ids) in fatos {
        let f = funcoes
            .get(simbolo)
            .ok_or_else(|| format!("fatos escalares: símbolo obsoleto {simbolo}"))?;
        let plano = planos
            .get(simbolo)
            .ok_or_else(|| format!("fatos escalares: plano ausente {simbolo}"))?;
        let tipos: HashMap<_, _> = f.params.iter().map(|(v, _, ty)| (*v, *ty)).collect();
        if tipos.len() != f.params.len() {
            return Err(format!("fatos escalares: parâmetros repetidos {simbolo}"));
        }
        let mut ids: Vec<_> = ids.iter().copied().collect();
        ids.sort_by_key(|v| v.0);
        for v in ids {
            if !matches!(
                tipos.get(&v),
                Some(Type::I64 | Type::F64 | Type::I1 | Type::I8)
            ) {
                return Err(format!(
                    "fato escalar {simbolo} v{} não corresponde a parâmetro escalar",
                    v.0
                ));
            }
            match plano.classes.get(&v) {
                None => novos.push((simbolo, v)),
                Some(Ownership::Trivial) => {}
                _ => {
                    return Err(format!(
                        "fato escalar {simbolo} v{} conflita com ownership",
                        v.0
                    ));
                }
            }
        }
    }
    let total = novos.len();
    for (simbolo, v) in novos {
        planos
            .get_mut(simbolo)
            .unwrap()
            .classes
            .insert(v, Ownership::Trivial);
    }
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn fatos_invalidos_nao_publicam_classificacao_parcial_e_tracing_nao_os_aplica() {
        for variante in 0..5 {
            let mut m = Module::new();
            m.memoria_arc = true;
            for simbolo in ["a", "z"] {
                m.functions.push(Function {
                    symbol: simbolo.into(),
                    name: simbolo.into(),
                    depuracao: None,
                    params: vec![(ValueId(0), "x".into(), Type::I64)],
                    return_ty: Type::Void,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Return(None),
                    }],
                });
                m.parametros_escalares_dart
                    .insert(simbolo.into(), HashSet::from([ValueId(0)]));
            }
            let mut ps = HashMap::from([
                ("a".into(), PlanoFuncaoDart::default()),
                ("z".into(), PlanoFuncaoDart::default()),
            ]);
            match variante {
                0 => {
                    m.parametros_escalares_dart
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
                _ => {
                    m.functions.pop();
                }
            }
            let antes = format!("{ps:?}");
            assert!(produzir_parametros_escalares_do_lowering(&m, &mut ps).is_err());
            assert_eq!(format!("{ps:?}"), antes);
            m.memoria_arc = false;
            assert_eq!(
                produzir_parametros_escalares_do_lowering(&m, &mut ps).unwrap(),
                0
            );
            assert_eq!(format!("{ps:?}"), antes);
        }
    }
}
