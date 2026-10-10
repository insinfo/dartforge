//! Convenção Owned registrada nominalmente pelo lowering de corpos Dart.
//! Uma representação Ref isolada não certifica a ABI de outro produtor.

use super::*;

/// Produz a convenção Owned dos retornos Ref registrados pelo lowering Dart.
///
/// Fatos devem corresponder aos símbolos e representações atuais. O plano
/// padrão Trivial é preenchido; Owned permanece e Borrowed conflita com a ABI.
/// Isso não certifica o corpo: inserção e verificação devem provar a transferência
/// do token, incluindo retenção de retornos emprestados e cleanup de falhas.
/// Tracing não altera planos. Retorna o número de convenções acrescentadas.
///
/// # Erros
/// Símbolo duplicado/obsoleto, plano ausente, retorno não Ref ou convenção
/// incompatível. Nenhum plano é alterado, mesmo se outros fatos forem válidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.memoria_arc = true;
/// m.functions.push(Function { symbol: "identidade".into(), name: "identidade".into(),
///     depuracao: None, params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0),
///     instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
/// m.retornos_ref_dart.insert("identidade".into());
/// let mut planos = HashMap::from([("identidade".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(produzir_retornos_ref_do_lowering(&m, &mut planos)?, 1);
/// assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos)?, (1, 0));
/// # Ok::<(), String>(())
/// ```
pub fn produzir_retornos_ref_do_lowering(
    modulo: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<usize, String> {
    if !modulo.memoria_arc {
        return Ok(0);
    }
    let mut funcoes = HashMap::new();
    for f in &modulo.functions {
        if funcoes.insert(&f.symbol, f).is_some() {
            return Err(format!("retorno Dart: símbolo repetido {}", f.symbol));
        }
    }
    let mut fatos: Vec<_> = modulo.retornos_ref_dart.iter().collect();
    fatos.sort();
    let mut novos = Vec::new();
    for simbolo in fatos {
        let f = funcoes
            .get(simbolo)
            .ok_or_else(|| format!("retorno Dart: símbolo obsoleto {simbolo}"))?;
        let plano = planos
            .get(simbolo)
            .ok_or_else(|| format!("retorno Dart: plano ausente {simbolo}"))?;
        if f.return_ty != Type::Ref {
            return Err(format!("retorno Dart: {simbolo} não devolve Ref"));
        }
        match plano.tokens.retorno {
            RetornoTokens::Trivial => novos.push(simbolo),
            RetornoTokens::Owned => {}
            RetornoTokens::Borrowed => {
                return Err(format!("retorno Dart: convenção conflitante {simbolo}"));
            }
        }
    }
    for simbolo in &novos {
        planos.get_mut(*simbolo).unwrap().tokens.retorno = RetornoTokens::Owned;
    }
    Ok(novos.len())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fatos_de_retorno_sao_atomicos_idempotentes_e_exigem_proveniencia() {
        for caso in 0..8 {
            let mut m = Module::new();
            m.memoria_arc = caso != 6;
            for simbolo in ["a", "z"] {
                m.functions.push(Function {
                    symbol: simbolo.into(),
                    name: simbolo.into(),
                    depuracao: None,
                    params: vec![(ValueId(0), "x".into(), Type::Ref)],
                    return_ty: Type::Ref,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                    }],
                });
                if caso != 7 {
                    m.retornos_ref_dart.insert(simbolo.into());
                }
            }
            let mut planos = HashMap::from([
                ("a".into(), PlanoFuncaoDart::default()),
                ("z".into(), PlanoFuncaoDart::default()),
            ]);
            match caso {
                1 => {
                    m.retornos_ref_dart.insert("obsoleto".into());
                }
                2 => {
                    planos.remove("z");
                }
                3 => m.functions[1].return_ty = Type::I64,
                4 => planos.get_mut("z").unwrap().tokens.retorno = RetornoTokens::Borrowed,
                5 => m.functions[1].symbol = "a".into(),
                _ => {}
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = produzir_retornos_ref_do_lowering(&m, &mut planos);
            if (1..6).contains(&caso) {
                assert!(resultado.is_err(), "caso {caso}");
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else if caso >= 6 {
                assert_eq!(resultado.unwrap(), 0);
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(resultado.unwrap(), 2);
                assert_eq!(
                    produzir_retornos_ref_do_lowering(&m, &mut planos).unwrap(),
                    0
                );
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
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
