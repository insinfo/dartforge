//! Fatos da ABI uniforme: endereço nativo e contagens do descritor imutável.
//! Leituras Ref ganham owner e conferem limites pelo descritor. Não prova
//! extensão de ponteiros externos, palavras RTI nem pins da imagem JIT.

use super::*;

pub(super) const LER_ARGUMENTO: &str = "dartforge_arc_argumento_owned_v1";

/// A palavra da tupla de tipos da chamada uniforme genérica: o slot logo
/// depois dos posicionais e nomeados, `args[desc[0] + desc[1]]`
/// (`tupla_do_slot`), um ID do universo RTI lido como `I64`.
pub(super) fn leitura_da_tupla(
    f: &Function,
    inst: &Instruction,
    ty: Type,
    papeis: &HashMap<ValueId, VetorEntradaDart>,
) -> bool {
    let Instruction::LoadIndexed { base: Operand::Val(base), index: Operand::Val(i) } = inst else {
        return false;
    };
    if ty != Type::I64 || papeis.get(base) != Some(&VetorEntradaDart::Argumentos) {
        return false;
    }
    let def = |x: &ValueId| f.blocks.iter().flat_map(|b| &b.instructions).find(|(v, _, _)| v == x).map(|(_, i, _)| i);
    // `desc[k]` lido do descritor declarado.
    let leitura_do_descritor = |o: &Operand, k: i64| matches!(o, Operand::Val(x)
        if matches!(def(x), Some(Instruction::LoadIndexed { base: Operand::Val(d), index: Operand::Constant(Constant::Int(n)) })
            if *n == k && papeis.get(d) == Some(&VetorEntradaDart::Descritor)));
    matches!(def(i), Some(Instruction::Add(a, b)) if leitura_do_descritor(a, 0) && leitura_do_descritor(b, 1))
}

pub(super) fn leitura_conferida(inst: &Instruction, papeis: &HashMap<ValueId, VetorEntradaDart>) -> bool {
    matches!(inst, Instruction::CallRuntime { name, args, ret_ty: Type::Ref }
        if name == LER_ARGUMENTO && matches!(args.as_slice(),
            [(Operand::Val(a), Type::Ptr), (Operand::Val(d), Type::Ptr), (_, Type::I64)]
                if papeis.get(a) == Some(&VetorEntradaDart::Argumentos)
                    && papeis.get(d) == Some(&VetorEntradaDart::Descritor)))
}

pub(super) fn preparar_leituras(modulo: &Module, funcoes: &mut [Function]) -> Result<(), String> {
    for f in funcoes {
        let Some(papeis) = modulo.parametros_vetores_dart.get(&f.symbol) else { continue; };
        let descritores: Vec<_> = papeis.iter().filter(|(_, p)| **p == VetorEntradaDart::Descritor).map(|(v, _)| *v).collect();
        for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            let Instruction::LoadIndexed { base: Operand::Val(base), index } = inst else { continue; };
            if *ty != Type::Ref || papeis.get(base) != Some(&VetorEntradaDart::Argumentos) { continue; }
            let [descritor] = descritores.as_slice() else {
                return Err(format!("vetores ABI: {} v{} sem descritor único", f.symbol, v.0));
            };
            *inst = Instruction::CallRuntime { name: LER_ARGUMENTO.into(), args: vec![
                (Operand::Val(*base), Type::Ptr), (Operand::Val(*descritor), Type::Ptr), (index.clone(), Type::I64),
            ], ret_ty: Type::Ref };
        }
    }
    Ok(())
}

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
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if matches!(inst, Instruction::CallRuntime { name, .. } if name == LER_ARGUMENTO)
                && (*ty != Type::Ref || modulo.parametros_vetores_dart.get(&f.symbol)
                    .is_none_or(|p| !leitura_conferida(inst, p))) {
                return Err(format!("vetores ABI: {} v{} sem proveniência da leitura", f.symbol, v.0));
            }
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
            if let Instruction::LoadIndexed { base: Operand::Val(base), index: Operand::Val(_) } = inst
                && *ty == Type::I64
                && papeis.get(base) == Some(&VetorEntradaDart::Argumentos)
            {
                if !leitura_da_tupla(f, inst, *ty, papeis) {
                    return Err(format!("vetores ABI: {simbolo} v{} lê palavra escalar fora do slot da tupla", v.0));
                }
                marcar(&mut plano.classes, *v)?;
                let efeito = EfeitoTokens::default();
                if plano.tokens.instrucoes.get(v).is_some_and(|e| *e != efeito) {
                    return Err(format!("vetores ABI: {simbolo} v{} conflita com efeito de leitura", v.0));
                }
                plano.tokens.instrucoes.insert(*v, efeito);
                continue;
            }
            if matches!(inst, Instruction::CallRuntime { name, .. } if name == LER_ARGUMENTO) {
                if *ty != Type::Ref || !leitura_conferida(inst, papeis) {
                    return Err(format!("vetores ABI: {simbolo} v{} sem par argumentos/descritor", v.0));
                }
                continue;
            }
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
    fn tupla_de_tipos_so_no_slot_depois_dos_argumentos() {
        for caso in 0..3 {
            let mut m = Module::new();
            m.memoria_arc = true;
            let val = |n| Operand::Val(ValueId(n));
            let ler = |base, k| Instruction::LoadIndexed { base: val(base), index: Operand::Constant(Constant::Int(k)) };
            let indice = match caso { 1 => val(2), _ => val(4) };
            let base_da_tupla = if caso == 2 { 1 } else { 0 };
            m.functions.push(Function { symbol: "generica".into(), name: "generica".into(), depuracao: None,
                params: vec![(ValueId(0), "args".into(), Type::Ptr), (ValueId(1), "desc".into(), Type::Ptr)],
                return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(2), ler(1, 0), Type::I64),
                    (ValueId(3), ler(1, 1), Type::I64),
                    (ValueId(4), Instruction::Add(val(2), val(3)), Type::I64),
                    (ValueId(5), Instruction::LoadIndexed { base: val(base_da_tupla), index: indice }, Type::I64),
                ], terminator: Terminator::Return(Some(val(5))) }] });
            m.parametros_vetores_dart.insert("generica".into(), HashMap::from([
                (ValueId(0), VetorEntradaDart::Argumentos), (ValueId(1), VetorEntradaDart::Descritor)]));
            let mut planos = HashMap::from([("generica".into(), PlanoFuncaoDart::default())]);
            let r = produzir(&m, &mut planos);
            if caso == 0 {
                r.unwrap();
                assert_eq!(planos["generica"].classes[&ValueId(5)], Ownership::Trivial);
            } else {
                assert!(r.is_err(), "caso {caso}");
            }
        }
    }

    #[test]
    fn argumento_ref_exige_papeis_originais_e_reconfere_chamada_preparada() {
        for caso in 0..5 {
            let mut m = Module::new(); m.memoria_arc = true;
            m.functions.push(Function { symbol: "entrada".into(), name: "entrada".into(), depuracao: None,
                params: vec![(ValueId(0), "args".into(), Type::Ptr), (ValueId(1), "desc".into(), Type::Ptr)],
                return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(2), Instruction::LoadIndexed { base: Operand::Val(ValueId(0)), index: Operand::Constant(Constant::Int(0)) }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) }] });
            m.parametros_vetores_dart.insert("entrada".into(), HashMap::from([
                (ValueId(0), VetorEntradaDart::Argumentos), (ValueId(1), VetorEntradaDart::Descritor)]));
            let mut p = PlanoFuncaoDart::default(); p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("entrada".into(), p)]);
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
            assert!(leitura_conferida(&m.functions[0].blocks[0].instructions[0].1, &m.parametros_vetores_dart["entrada"]));
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            match caso {
                1 => m.parametros_vetores_dart.clear(),
                2 => if let Instruction::CallRuntime { args, .. } = &mut m.functions[0].blocks[0].instructions[0].1 { args.swap(0, 1); },
                3 => if let Instruction::CallRuntime { args, .. } = &mut m.functions[0].blocks[0].instructions[0].1 { args[0].0 = Operand::Constant(Constant::Int(0)); },
                4 => m.functions[0].blocks[0].instructions[0].2 = Type::I64,
                _ => {}
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            assert_eq!(resultado.is_ok(), caso == 0, "caso {caso}: {resultado:?}");
            assert_eq!(antes, format!("{m:?}/{planos:?}"));
        }
    }

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
