//! Proveniência dos caches nativos de receitas RTI; não cobre globais Dart.

use super::*;

pub(super) fn produzir(
    modulo: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(), String> {
    if !modulo.memoria_arc {
        return Ok(());
    }
    let mut globais = HashMap::new();
    for cache in &modulo.caches_rti_arc {
        let declaracoes: Vec<_> = modulo.globais.iter().filter(|g| &g.2 == cache).collect();
        if declaracoes.len() != 1 || declaracoes[0].0 != 0 || declaracoes[0].1 != Type::I64 {
            return Err(format!("cache RTI sem declaração nativa única: {cache}"));
        }
        let flag = format!("{cache}$ok");
        if modulo.globais.iter().any(|g| g.2 == flag)
            || globais.insert(cache.clone(), Type::I64).is_some()
            || globais.insert(flag, Type::I8).is_some()
        {
            return Err(format!("cache RTI com símbolo sobreposto: {cache}"));
        }
    }
    let mut novos = planos.clone();
    for f in &modulo.functions {
        let definicoes: HashMap<_, _> = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, i, t)| (*v, (i, *t)))
            .collect();
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            // A HIR não oferece endereço de global. Recusa também a tentativa
            // de usar seu símbolo como endereço de função/publicação opaca.
            let mut endereco = false;
            super::super::operandos::operandos(inst, &mut |o| {
                endereco |=
                    matches!(o, Operand::Constant(Constant::Funcao(s)) if globais.contains_key(s));
            });
            if matches!(inst, Instruction::Const(Constant::Funcao(s)) if globais.contains_key(s))
                || endereco
            {
                return Err(format!("cache RTI publicado como endereço em {}", f.symbol));
            }
            let (simbolo, esperado) = match inst {
                Instruction::LoadGlobal {
                    simbolo,
                    ty: origem,
                } if globais.contains_key(simbolo) => {
                    if globais[simbolo] != *origem || *ty != *origem {
                        return Err(format!("cache RTI com leitura incompatível: {simbolo}"));
                    }
                    (simbolo, *origem)
                }
                Instruction::StoreGlobal {
                    simbolo,
                    val,
                    ty: destino,
                    raiz,
                } if globais.contains_key(simbolo) => {
                    let nativo = match globais[simbolo] {
                        Type::I8 => matches!(val, Operand::Constant(Constant::Int(0 | 1))),
                        Type::I64 => {
                            matches!(val, Operand::Val(id) if definicoes.get(id).is_some_and(|(i, t)|
                            *t == Type::I64 && matches!(i, Instruction::CallRuntime { name, args, ret_ty }
                            if name == "dartforge_rti_receita" && *ret_ty == Type::I64
                            && args.len() == 1 && args[0].1 == Type::Ref)))
                        }
                        _ => false,
                    };
                    if *destino != globais[simbolo]
                        || *ty != Type::Void
                        || raiz.is_some()
                        || !nativo
                    {
                        return Err(format!(
                            "cache RTI com escrita sem origem nativa: {simbolo}"
                        ));
                    }
                    (simbolo, Type::Void)
                }
                _ => continue,
            };
            let plano = novos
                .get_mut(&f.symbol)
                .ok_or_else(|| format!("cache RTI sem plano: {}", f.symbol))?;
            if plano
                .classes
                .get(v)
                .is_some_and(|c| *c != Ownership::Trivial)
                || plano
                    .tokens
                    .instrucoes
                    .get(v)
                    .is_some_and(|e| *e != EfeitoTokens::default())
            {
                return Err(format!(
                    "cache RTI com contrato conflitante: {simbolo} ({esperado:?})"
                ));
            }
            plano.classes.insert(*v, Ownership::Trivial);
            plano.tokens.instrucoes.insert(*v, EfeitoTokens::default());
        }
        let mut endereco = false;
        for b in &f.blocks {
            super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                endereco |=
                    matches!(o, Operand::Constant(Constant::Funcao(s)) if globais.contains_key(s));
            });
        }
        if endereco {
            return Err(format!("cache RTI publicado no terminador em {}", f.symbol));
        }
    }
    *planos = novos;
    Ok(())
}

// Corpos atuais e caches já auditados, sem pressupor prefixo de símbolo.
// A conclusão não cobre recarga nem funções com contexto/saída Dart implícita.
pub(super) fn preservam_campos(m: &Module, planos: &HashMap<String, PlanoFuncaoDart>) -> HashSet<String> {
    let mut globais = HashMap::new();
    for nome in &m.caches_rti_arc {
        globais.insert(nome.clone(), Type::I64);
        globais.insert(format!("{nome}$ok"), Type::I8);
    }
    let mut pendentes = HashMap::new();
    let mut pais: HashMap<String, Vec<String>> = HashMap::new();
    let mut fila = std::collections::VecDeque::new();
    for f in &m.functions {
        if !f.params.is_empty() || f.return_ty != Type::I64
            || crate::llvm::LlvmEmitter::exige_contexto_explicito(f, planos.get(&f.symbol).map(|p| &p.tabelas)) {
            continue;
        }
        let tipos: HashMap<_, _> = f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)).collect();
        let numerico = |o: &Operand| match o {
            Operand::Val(v) => matches!(tipos.get(v), Some(Type::I64 | Type::I8 | Type::I1)),
            Operand::Constant(Constant::Int(_) | Constant::Bool(_)) => true,
            _ => false,
        };
        let mut dependencias = HashSet::new();
        let fechado = f.blocks.iter().all(|b| {
            let saida = match &b.terminator {
                Terminator::Return(Some(o)) => numerico(o),
                Terminator::Branch(_) | Terminator::Unreachable => true,
                Terminator::CondBranch { cond, .. } => numerico(cond),
                _ => false,
            };
            saida && b.instructions.iter().all(|(_, inst, ty)| match inst {
                Instruction::Const(Constant::Int(_)) => *ty == Type::I64,
                Instruction::Const(Constant::Bool(_)) => *ty == Type::I1,
                Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => *ty == Type::Ref,
                Instruction::ICmp(_, a, b) => *ty == Type::I1 && numerico(a) && numerico(b),
                Instruction::Phi { incoming, .. } => matches!(ty, Type::I64 | Type::I8 | Type::I1)
                    && incoming.iter().all(|(_, o)| numerico(o)),
                Instruction::LoadGlobal { simbolo, ty: lido } => globais.get(simbolo) == Some(lido) && ty == lido,
                Instruction::StoreGlobal { simbolo, ty: gravado, raiz, val } => globais.get(simbolo) == Some(gravado)
                    && *ty == Type::Void && raiz.is_none() && numerico(val),
                Instruction::CallRuntime { name, args, ret_ty } => name == "dartforge_rti_receita"
                    && *ty == Type::I64 && *ret_ty == Type::I64 && args.len() == 1 && args[0].1 == Type::Ref,
                Instruction::CallStatic { symbol, args, ret_ty } => {
                    dependencias.insert(symbol.clone());
                    args.is_empty() && *ty == Type::I64 && *ret_ty == Type::I64
                }
                _ => false,
            })
        });
        if !fechado { continue; }
        if dependencias.is_empty() { fila.push_back(f.symbol.clone()); }
        pendentes.insert(f.symbol.clone(), dependencias.len());
        for dep in dependencias { pais.entry(dep).or_default().push(f.symbol.clone()); }
    }
    let mut seguros = HashSet::new();
    while let Some(s) = fila.pop_front() {
        if !seguros.insert(s.clone()) { continue; }
        if let Some(chamadores) = pais.get(&s) {
            for caller in chamadores {
                let n = pendentes.get_mut(caller).unwrap();
                *n -= 1;
                if *n == 0 { fila.push_back(caller.clone()); }
            }
        }
    }
    seguros
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modulo() -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        // O nome não participa da prova: somente o fato nominal e os escritores.
        m.globais.push((0, Type::I64, "cache_sem_prefixo".into()));
        m.caches_rti_arc.insert("cache_sem_prefixo".into());
        m.functions.push(Function {
            symbol: "getter".into(),
            name: "getter".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::Const(Constant::String("C201".into())),
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_rti_receita".into(),
                            args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::StoreGlobal {
                            simbolo: "cache_sem_prefixo".into(),
                            val: Operand::Val(ValueId(1)),
                            ty: Type::I64,
                            raiz: None,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(3),
                        Instruction::StoreGlobal {
                            simbolo: "cache_sem_prefixo$ok".into(),
                            val: Operand::Constant(Constant::Int(1)),
                            ty: Type::I8,
                            raiz: None,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(4),
                        Instruction::LoadGlobal {
                            simbolo: "cache_sem_prefixo".into(),
                            ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(5),
                        Instruction::LoadGlobal {
                            simbolo: "cache_sem_prefixo$ok".into(),
                            ty: Type::I8,
                        },
                        Type::I8,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            }],
        });
        m
    }

    #[test]
    fn receitas_preservam_campos_pelo_corpo_e_dependencias_sem_confiar_no_nome() {
        let base = modulo();
        for caso in 0..7 {
            let mut m = modulo();
            m.functions.push(Function { symbol: "ponte".into(), name: "ponte".into(), depuracao: None,
                params: vec![], return_ty: Type::I64, blocks: vec![BasicBlock { id: BlockId(0),
                    instructions: vec![(ValueId(0), Instruction::CallStatic { symbol: "getter".into(), args: vec![], ret_ty: Type::I64 }, Type::I64)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
            let mut planos = HashMap::from([("getter".into(), PlanoFuncaoDart::default()), ("ponte".into(), PlanoFuncaoDart::default())]);
            match caso {
                1 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::CallRuntime {
                    name: "dartforge_print_handle".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Void,
                }, Type::Void)),
                2 => m.functions[0].params.push((ValueId(9), "receiver".into(), Type::Ref)),
                3 => planos.get_mut("getter").unwrap().tabelas.confere_pilha = true,
                4 => m.caches_rti_arc.clear(),
                5 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::CallStatic {
                    symbol: "ponte".into(), args: vec![], ret_ty: Type::I64,
                }, Type::I64)),
                6 => m.functions[0].blocks[0].instructions.push((ValueId(9), Instruction::StoreGlobal {
                    simbolo: "objeto_global".into(), val: Operand::Val(ValueId(0)), ty: Type::Ref, raiz: Some(1),
                }, Type::Void)),
                _ => {}
            }
            let seguros = super::preservam_campos(&m, &planos);
            assert_eq!(seguros.contains("getter"), caso == 0, "caso {caso}");
            assert_eq!(seguros.contains("ponte"), caso == 0, "caso {caso}");
        }
        assert!(super::preservam_campos(&base, &HashMap::new()).contains("getter"));
    }

    #[test]
    fn cache_exige_declaracao_e_todos_os_escritores_nativos() {
        for caso in 0..19 {
            let mut m = modulo();
            let mut planos = HashMap::from([("getter".into(), PlanoFuncaoDart::default())]);
            match caso {
                1 => m.globais[0].1 = Type::Ref,
                2 => m.globais[0].0 = 9,
                3 => m.globais.clear(),
                4 => m.globais.push(m.globais[0].clone()),
                5 => m.globais.push((0, Type::I8, "cache_sem_prefixo$ok".into())),
                6 => {
                    m.caches_rti_arc.insert("obsoleto".into());
                }
                7 => {
                    if let Instruction::StoreGlobal { val, .. } =
                        &mut m.functions[0].blocks[0].instructions[2].1
                    {
                        *val = Operand::Val(ValueId(0));
                    }
                }
                8 => {
                    if let Instruction::StoreGlobal { raiz, .. } =
                        &mut m.functions[0].blocks[0].instructions[2].1
                    {
                        *raiz = Some(9);
                    }
                }
                9 => {
                    if let Instruction::StoreGlobal { val, .. } =
                        &mut m.functions[0].blocks[0].instructions[3].1
                    {
                        *val = Operand::Constant(Constant::Int(2));
                    }
                }
                10 => m.functions[0].blocks[0].instructions[4].2 = Type::Ref,
                11 => {
                    planos
                        .get_mut("getter")
                        .unwrap()
                        .classes
                        .insert(ValueId(5), Ownership::Owned);
                }
                12 => {
                    planos.get_mut("getter").unwrap().tokens.instrucoes.insert(
                        ValueId(2),
                        EfeitoTokens {
                            pode_falhar: true,
                            ..Default::default()
                        },
                    );
                }
                13 => {
                    m.functions[0].blocks[0].instructions.push((
                        ValueId(6),
                        Instruction::Const(Constant::Funcao("cache_sem_prefixo".into())),
                        Type::I64,
                    ));
                }
                14 => m.caches_rti_arc.clear(),
                15 => {
                    let mut estranho = m.functions[0].clone();
                    estranho.symbol = "estranho".into();
                    estranho.blocks[0].instructions[2].1 = Instruction::StoreGlobal {
                        simbolo: "cache_sem_prefixo".into(),
                        val: Operand::Constant(Constant::Int(42)),
                        ty: Type::I64,
                        raiz: None,
                    };
                    m.functions.insert(0, estranho);
                    planos.insert("estranho".into(), PlanoFuncaoDart::default());
                }
                16 => m.memoria_arc = false,
                17 => {
                    let mut outro = m.functions[0].clone();
                    outro.symbol = "outro".into();
                    m.functions.insert(0, outro);
                    planos.insert("outro".into(), PlanoFuncaoDart::default());
                }
                18 => {
                    if let Instruction::CallRuntime { name, .. } =
                        &mut m.functions[0].blocks[0].instructions[1].1
                    {
                        *name = "opaco".into();
                    }
                }
                _ => {}
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if matches!(caso, 0 | 16 | 17) {
                resultado.unwrap_or_else(|e| panic!("caso {caso}: {e}"));
                if caso == 16 {
                    assert_eq!(format!("{m:?}/{planos:?}"), antes);
                } else {
                    for v in [2, 3, 4, 5] {
                        assert_eq!(planos["getter"].classes[&ValueId(v)], Ownership::Trivial);
                        assert_eq!(
                            planos["getter"].tokens.instrucoes[&ValueId(v)],
                            EfeitoTokens::default()
                        );
                    }
                    let depois = format!("{m:?}/{planos:?}");
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
                    assert_eq!(format!("{m:?}/{planos:?}"), depois);
                }
            } else {
                assert!(resultado.is_err(), "caso {caso}: {resultado:?}");
                assert_eq!(format!("{m:?}/{planos:?}"), antes, "caso {caso}");
            }
        }
    }
}
