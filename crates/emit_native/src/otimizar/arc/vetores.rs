//! Fatos da ABI uniforme: endereço nativo e contagens do descritor imutável.
//! Não classifica palavras do vetor de argumentos, bounds ou pins da imagem.

use super::*;

pub(super) fn produzir(
    modulo: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(), String> {
    if !modulo.memoria_arc {
        return Ok(());
    }
    let mut funcoes = HashMap::new();
    for f in &modulo.functions {
        if funcoes.insert(&f.symbol, f).is_some() {
            return Err(format!("vetores ABI: símbolo repetido {}", f.symbol));
        }
    }
    let mut novos = planos.clone();
    let mut fatos: Vec<_> = modulo.parametros_vetores_dart.iter().collect();
    fatos.sort_by_key(|(s, _)| *s);
    for (simbolo, papeis) in fatos {
        let f = funcoes
            .get(simbolo)
            .ok_or_else(|| format!("vetores ABI: símbolo obsoleto {simbolo}"))?;
        let plano = novos
            .get_mut(simbolo)
            .ok_or_else(|| format!("vetores ABI: plano ausente {simbolo}"))?;
        let tipos: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t)).collect();
        if tipos.len() != f.params.len() {
            return Err(format!("vetores ABI: parâmetros repetidos {simbolo}"));
        }
        let marcar =
            |classes: &mut HashMap<ValueId, Ownership>, v: ValueId| -> Result<(), String> {
                if classes.get(&v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "vetores ABI: {simbolo} v{} conflita com ownership",
                        v.0
                    ));
                }
                classes.insert(v, Ownership::Trivial);
                Ok(())
            };
        let mut ids: Vec<_> = papeis.keys().copied().collect();
        ids.sort_by_key(|v| v.0);
        for v in ids {
            if tipos.get(&v) != Some(&Type::Ptr)
                || modulo
                    .parametros_escalares_dart
                    .get(simbolo)
                    .is_some_and(|s| s.contains(&v))
                || modulo
                    .parametros_rti_dart
                    .get(simbolo)
                    .is_some_and(|s| s.contains(&v))
            {
                return Err(format!(
                    "vetores ABI: {simbolo} v{} sem parâmetro Ptr de origem exclusiva",
                    v.0
                ));
            }
            marcar(&mut plano.classes, v)?;
        }
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let descritor = |o: &Operand| {
                matches!(o, Operand::Val(p)
                if papeis.get(p) == Some(&VetorEntradaDart::Descritor))
            };
            let mut usa_descritor = false;
            super::super::operandos::operandos(inst, &mut |o| usa_descritor |= descritor(o));
            if !usa_descritor {
                continue;
            }
            match inst {
                // Encaminhamento somente ao descritor declarado de um corpo
                // local. Todos esses corpos são validados nesta transação;
                // inclusive ciclos só podem ler/encaminhar, nunca publicar.
                Instruction::CallStatic { symbol, args, .. }
                    if funcoes.get(symbol).is_some_and(|callee| {
                        args.len() == callee.params.len()
                            && args.iter().enumerate().all(|(i, o)| {
                                !descritor(o)
                                    || (callee.params[i].2 == Type::Ptr
                                        && modulo
                                            .parametros_vetores_dart
                                            .get(symbol)
                                            .and_then(|p| p.get(&callee.params[i].0))
                                            == Some(&VetorEntradaDart::Descritor))
                            })
                    }) => {}
                Instruction::LoadIndexed { base, index }
                    if descritor(base)
                        && matches!(index, Operand::Constant(Constant::Int(0 | 1)))
                        && *ty == Type::I64 =>
                {
                    marcar(&mut plano.classes, *v)?;
                    let efeito = EfeitoTokens::default();
                    if plano.tokens.instrucoes.get(v).is_some_and(|e| *e != efeito) {
                        return Err(format!(
                            "vetores ABI: {simbolo} v{} conflita com efeito de leitura",
                            v.0
                        ));
                    }
                    plano.tokens.instrucoes.insert(*v, efeito);
                }
                // O adaptador recusa destinos sem contrato uniforme readonly.
                Instruction::CallSeletorRepasse { recv, args, desc, .. }
                | Instruction::CallClosureRepasse { closure: recv, args, desc, .. }
                    if descritor(desc) && !descritor(recv) && !descritor(args) => {}
                // Consultas nativas somente de leitura da ABI; seus contratos
                // de chamada/representações ainda são conferidos pelo catálogo.
                Instruction::CallRuntime { name, args, .. }
                    if matches!(
                        name.as_str(),
                        "dartforge_args_casam" | "dartforge_arg_indice"
                    ) && args
                        .first()
                        .is_some_and(|(o, t)| descritor(o) && *t == Type::Ptr)
                        && args.iter().skip(1).all(|(o, _)| !descritor(o)) => {}
                _ => {
                    return Err(format!(
                        "vetores ABI: {simbolo} v{} usa descritor fora das leituras cobertas",
                        v.0
                    ));
                }
            }
        }
        let mut usa_no_terminador = false;
        for b in &f.blocks {
            super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                usa_no_terminador |= matches!(o, Operand::Val(p) if papeis.get(p) == Some(&VetorEntradaDart::Descritor));
            });
        }
        if usa_no_terminador {
            return Err(format!(
                "vetores ABI: {simbolo} publica descritor no terminador"
            ));
        }
    }
    *planos = novos;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn descritor_produz_contagens_sem_certificar_palavras_dos_argumentos() {
        for caso in 0..18 {
            let mut m = Module::new();
            m.memoria_arc = caso != 13;
            m.functions.push(Function {
                symbol: "entrada".into(),
                name: "entrada".into(),
                depuracao: None,
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
            m.parametros_vetores_dart.insert(
                "entrada".into(),
                HashMap::from([
                    (ValueId(0), VetorEntradaDart::Descritor),
                    (ValueId(1), VetorEntradaDart::Argumentos),
                ]),
            );
            let mut ps = HashMap::from([("entrada".into(), PlanoFuncaoDart::default())]);
            match caso {
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
                    m.parametros_vetores_dart
                        .get_mut("entrada")
                        .unwrap()
                        .insert(ValueId(99), VetorEntradaDart::Descritor);
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
                    m.parametros_escalares_dart
                        .insert("entrada".into(), HashSet::from([ValueId(0)]));
                }
                11 => {
                    m.parametros_vetores_dart
                        .insert("obsoleto".into(), HashMap::new());
                }
                12 => {
                    ps.get_mut("entrada").unwrap().classes.insert(
                        ValueId(0),
                        Ownership::Borrowed {
                            owner: OrigemOwner::Chamador,
                            escopo: 0,
                        },
                    );
                }
                13 => {
                    m.parametros_vetores_dart
                        .insert("obsoleto".into(), HashMap::new());
                }
                14 => m.parametros_vetores_dart.clear(),
                15 => {
                    ps.get_mut("entrada").unwrap().tokens.instrucoes.insert(
                        ValueId(2),
                        EfeitoTokens {
                            pode_falhar: true,
                            ..Default::default()
                        },
                    );
                }
                16 => {
                    let p = m.functions[0].params[0].clone();
                    m.functions[0].params.push(p);
                }
                17 => {
                    let mut outra = m.functions[0].clone();
                    outra.symbol = "outra".into();
                    m.functions.insert(0, outra);
                    let fatos = m.parametros_vetores_dart["entrada"].clone();
                    m.parametros_vetores_dart.insert("outra".into(), fatos);
                    ps.insert("outra".into(), PlanoFuncaoDart::default());
                }
                _ => {}
            }
            let antes = format!("{m:?}/{ps:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut ps);
            if caso == 13 {
                assert_eq!(resultado.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{ps:?}"), antes);
            } else if (2..=16).contains(&caso) {
                let erro = resultado.expect_err(&format!("caso {caso}"));
                if caso == 2 {
                    assert!(erro.contains("em entrada (origem v2)"), "{erro}");
                }
                assert_eq!(format!("{m:?}/{ps:?}"), antes, "caso {caso}");
            } else {
                resultado.unwrap_or_else(|e| panic!("caso {caso}: {e}"));
                for id in 0..4 {
                    assert_eq!(ps["entrada"].classes[&ValueId(id)], Ownership::Trivial);
                }
                let pronto = format!("{m:?}");
                let classes = ps["entrada"].classes.clone();
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{m:?}"), pronto);
                assert_eq!(ps["entrada"].classes, classes);
            }
        }
    }
}
