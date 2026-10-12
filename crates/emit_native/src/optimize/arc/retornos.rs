//! Convenções Owned nominais de corpos Dart e entradas uniformes geradas.
//! Uma representação Ref isolada não certifica a ABI de outro produtor.

use super::*;

/// Produz a convenção Owned dos retornos Ref registrados pelo lowering Dart.
///
/// Fatos devem corresponder aos símbolos e representações atuais. O plano
/// padrão Trivial é preenchido; Owned permanece e Borrowed conflita com a ABI.
/// Isso não certifica o corpo: inserção e verificação devem provar a transferência
/// do token, incluindo retenção de retornos emprestados e cleanup de falhas.
/// A entrada uniforme exige declaração própria e vínculo aos vetores da ABI.
/// Esse fato não certifica chamadores indiretos, callbacks C ou SDK importado.
/// Tracing não altera planos. Retorna o número de convenções acrescentadas.
///
/// # Erros
/// Símbolo duplicado/obsoleto, plano ausente, retorno não Ref ou convenção
/// incompatível. Nenhum plano é alterado, mesmo se outros fatos forem válidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
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
    // A ABI uniforme precisa de declaração própria: nem Ref nem os vetores
    // sozinhos declaram o modo de retorno. Revalida o vínculo à mesma HIR.
    for simbolo in &modulo.retornos_uniformes_arc {
        let f = funcoes.get(simbolo).ok_or_else(|| format!("retorno uniforme ARC: símbolo obsoleto {simbolo}"))?;
        let papeis = modulo.parametros_vetores_dart.get(simbolo);
        if f.return_ty != Type::Ref || f.params.iter().map(|p| p.2).collect::<Vec<_>>() != [Type::Ref, Type::Ptr, Type::Ptr]
            || !papeis.is_some_and(|p| p.len() == 2
                && p.get(&f.params[1].0) == Some(&VetorEntradaDart::Argumentos)
                && p.get(&f.params[2].0) == Some(&VetorEntradaDart::Descritor)) {
            return Err(format!("retorno uniforme ARC: declaração/ABI de {simbolo} incompatível"));
        }
    }
    let mut fatos: Vec<_> = modulo.retornos_ref_dart.union(&modulo.retornos_uniformes_arc).collect();
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
    fn retorno_uniforme_owned_exige_declaracao_e_chamador_com_contrato() {
        for caso in 0..17 {
            let mut m = Module::new();
            m.memoria_arc = caso != 12;
            let target = Function {
                symbol: "target".into(), name: "target".into(), depuracao: None,
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
            for simbolo in ["target", "caller"] {
                m.parametros_vetores_dart.insert(simbolo.into(), HashMap::from([
                    (ValueId(1), VetorEntradaDart::Argumentos), (ValueId(2), VetorEntradaDart::Descritor)]));
                m.retornos_uniformes_arc.insert(simbolo.into());
            }
            let mut ps = HashMap::from([("target".into(), PlanoFuncaoDart::default()),
                ("caller".into(), PlanoFuncaoDart::default())]);
            match caso {
                1 => m.functions.reverse(),
                2 => m.functions[1].blocks[0].instructions[0].1 = Instruction::CallClosure {
                    closure: Operand::Val(ValueId(0)), args: vec![], nomes: vec![],
                    ret_ty: Type::Ref, tupla_tipos: Operand::Constant(Constant::Int(0)) },
                3 => m.retornos_uniformes_arc.clear(),
                4 => ps.get_mut("target").unwrap().tokens.retorno = RetornoTokens::Borrowed,
                5 => m.functions[0].params[1].2 = Type::Ref,
                6 => { m.parametros_vetores_dart.remove("target"); },
                7 => { m.parametros_vetores_dart.insert("target".into(), HashMap::from([
                    (ValueId(1), VetorEntradaDart::Descritor), (ValueId(2), VetorEntradaDart::Argumentos)])); },
                8 | 12 => { m.retornos_uniformes_arc.insert("obsoleto".into()); },
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
            let antes = format!("{m:?}/{ps:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut ps);
            if (2..12).contains(&caso) || caso >= 15 {
                let erro = resultado.expect_err(&format!("caso {caso}"));
                if caso == 2 { assert!(erro.contains("v3 exige guarda pending"), "{erro}"); }
                assert_eq!(format!("{m:?}/{ps:?}"), antes, "caso {caso}");
            } else if caso == 12 {
                assert_eq!(resultado.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{ps:?}"), antes);
            } else {
                let total = resultado.unwrap_or_else(|e| panic!("caso {caso}: {e}"));
                assert_eq!(total, (usize::from(caso < 13), 0));
                assert_eq!(ps["target"].tokens.retorno, RetornoTokens::Owned);
                assert_eq!(ps["caller"].classes[&ValueId(3)], Ownership::Owned);
                let caller = m.functions.iter().find(|f| f.symbol == "caller").unwrap();
                assert!(!caller.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
                    Instruction::ArcCopy { value: Operand::Val(ValueId(3)) }
                        | Instruction::ArcDrop { value: Operand::Val(ValueId(3)) })));
                if caso == 13 {
                    let target = m.functions.iter().find(|f| f.symbol == "target").unwrap();
                    assert!(target.blocks[0].instructions.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == "dartforge_arc_box_int_owned_v1")));
                }
                let pronto = format!("{m:?}");
                let classes = ps["caller"].classes.clone();
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{m:?}"), pronto);
                assert_eq!(ps["caller"].classes, classes);
            }
        }
    }

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
