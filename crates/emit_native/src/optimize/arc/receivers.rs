//! Classes físicas por fluxo SSA e argumentos de chamadas diretas fechadas.
use super::*;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Default)]
struct Classes {
    cids: BTreeSet<u32>,
    topo: bool,
}

pub(super) fn produzir(m: &Module) -> HashMap<String, HashMap<ValueId, u32>> {
    let mut ids = HashMap::new();
    let mut estados = Vec::<Classes>::new();
    for f in &m.functions {
        for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, t)| (v, t)),
        ) {
            if *t == Type::Ref {
                ids.insert((f.symbol.clone(), *v), estados.len());
                estados.push(Classes::default());
            }
        }
    }
    let por_nome: HashMap<_, _> = m.functions.iter().map(|f| (&f.symbol, f)).collect();
    let mut entradas: HashSet<String> = m
        .retornos_uniformes_arc
        .iter()
        .cloned()
        .chain(m.ajudantes.iter().map(|(_, s)| s.clone()))
        .chain(m.classes.iter().flat_map(|c| {
            c.vtable
                .iter()
                .map(|(_, s)| s.clone())
                .chain(c.to_string_symbol.iter().cloned())
        }))
        .chain(m.entry_symbol.iter().cloned())
        .chain(m.chamar_dart.iter().cloned())
        .chain(m.ffi_callbacks.iter().map(|c| c.corpo.clone()))
        .chain(m.ffi_trampolins.iter().map(|(_, s)| s.clone()))
        .chain(m.raizes_da_fonte.iter().cloned())
        .chain(m.funcoes_de_tabela.values().cloned())
        .chain(m.registro.iter().cloned())
        .chain(m.registros_do_sdk.iter().cloned())
        .chain(m.iniciar_rti.iter().cloned())
        .chain(m.tearoffs_do_rastro.iter().map(|(s, _)| s.clone()))
        .chain(
            m.tabelas_de_metodos
                .iter()
                .flat_map(|(_, _, t)| t.iter().map(|(_, s)| s.clone())),
        )
        .collect();
    let mut chamadas = HashSet::new();
    let mut arestas = vec![Vec::new(); estados.len()];
    let mut alimentar = |simbolo: &str, op: &Operand, destino: usize| {
        if let Operand::Val(v) = op
            && let Some(&origem) = ids.get(&(simbolo.to_string(), *v))
        {
            arestas[origem].push(destino);
        } else {
            estados[destino].topo = true;
        }
    };
    // Endereço materializado é uma entrada aberta, mesmo que existam callers
    // locais conhecidos. Nunca inferir todos os argumentos só desses callers.
    for f in &m.functions {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            match i {
                Instruction::Const(Constant::Funcao(s))
                | Instruction::TearOff { code_symbol: s }
                | Instruction::AllocClosure { code_symbol: s, .. } => {
                    entradas.insert(s.clone());
                }
                Instruction::AllocClosureTipada {
                    code_symbol,
                    tipado,
                    ..
                } => {
                    entradas.insert(code_symbol.clone());
                    entradas.insert(tipado.clone());
                }
                Instruction::TabelaDeFuncoes(simbolos) => entradas.extend(simbolos.iter().cloned()),
                _ => {}
            }
            super::super::operandos::operandos(i, &mut |o| {
                if let Operand::Constant(Constant::Funcao(s)) = o {
                    entradas.insert(s.clone());
                }
            });
            if let Instruction::CallStatic { symbol, args, .. } = i
                && let Some(callee) = por_nome.get(symbol)
            {
                chamadas.insert(symbol.clone());
                if args.len() == callee.params.len() {
                    for (op, (v, _, t)) in args.iter().zip(&callee.params) {
                        if *t == Type::Ref {
                            alimentar(&f.symbol, op, ids[&(symbol.clone(), *v)]);
                        }
                    }
                } else {
                    entradas.insert(symbol.clone());
                }
            }
        }
    }
    for f in &m.functions {
        for b in &f.blocks {
            super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Funcao(s)) = o {
                    entradas.insert(s.clone());
                }
            });
        }
    }
    for f in &m.functions {
        for (v, _, t) in &f.params {
            if *t == Type::Ref && (entradas.contains(&f.symbol) || !chamadas.contains(&f.symbol)) {
                estados[ids[&(f.symbol.clone(), *v)]].topo = true;
            }
        }
        for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *t != Type::Ref {
                continue;
            }
            let destino = ids[&(f.symbol.clone(), *v)];
            let classe = match i {
                Instruction::AllocObject { class_id, fields } => m
                    .layouts_campos_arc
                    .get(class_id)
                    .filter(|l| l.len() == fields.len())
                    .map(|_| *class_id),
                Instruction::CallRuntime { name, args, ret_ty }
                    if *ret_ty == Type::Ref
                        && matches!(
                            name.as_str(),
                            "dartforge_object_new" | "dartforge_arc_objeto_owned_v1"
                        ) =>
                {
                    match args.as_slice() {
                        [
                            (Operand::Constant(Constant::Int(cid)), Type::I64),
                            (Operand::Constant(Constant::Int(n)), Type::I64),
                        ] => u32::try_from(*cid)
                            .ok()
                            .filter(|cid| i32::try_from(*cid).is_ok())
                            .filter(|cid| {
                                usize::try_from(*n).ok().is_some_and(|n| {
                                    u16::try_from(n).is_ok()
                                        && m.layouts_campos_arc
                                            .get(cid)
                                            .is_some_and(|l| l.len() == n)
                                })
                            }),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(cid) = classe {
                estados[destino].cids.insert(cid);
                continue;
            }
            let ops: Vec<_> = match i {
                Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
                Instruction::Phi { incoming, .. } => incoming.iter().map(|(_, o)| o).collect(),
                _ => {
                    estados[destino].topo = true;
                    continue;
                }
            };
            for op in ops {
                if let Operand::Val(v) = op
                    && let Some(&origem) = ids.get(&(f.symbol.clone(), *v))
                {
                    arestas[origem].push(destino);
                } else {
                    estados[destino].topo = true;
                }
            }
        }
    }
    let mut fila: VecDeque<_> = (0..estados.len()).collect();
    let mut agendado = vec![true; estados.len()];
    while let Some(origem) = fila.pop_front() {
        agendado[origem] = false;
        let atual = estados[origem].clone();
        for &destino in &arestas[origem] {
            let antes = (estados[destino].topo, estados[destino].cids.len());
            estados[destino].topo |= atual.topo;
            estados[destino].cids.extend(&atual.cids);
            if antes != (estados[destino].topo, estados[destino].cids.len()) && !agendado[destino] {
                agendado[destino] = true;
                fila.push_back(destino);
            }
        }
    }
    let mut resultado = HashMap::<String, HashMap<ValueId, u32>>::new();
    for f in &m.functions {
        for (v, _, t) in &f.params {
            if *t != Type::Ref {
                continue;
            }
            let c = &estados[ids[&(f.symbol.clone(), *v)]];
            if !c.topo && c.cids.len() == 1 {
                resultado
                    .entry(f.symbol.clone())
                    .or_default()
                    .insert(*v, *c.cids.first().unwrap());
            }
        }
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modulo() -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 1,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.layouts_campos_arc.insert(123, vec![Type::Ref]);
        m.functions = vec![
            Function {
                symbol: "caller".into(),
                name: "caller".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::AllocObject {
                                class_id: 123,
                                fields: vec![Operand::Constant(Constant::Null)],
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: "ctor".into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            },
            Function {
                symbol: "ctor".into(),
                name: "ctor".into(),
                depuracao: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::SetField {
                                object: Operand::Val(ValueId(0)),
                                index: 0,
                                value: Operand::Constant(Constant::Null),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallStatic {
                                symbol: "base".into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            },
            Function {
                symbol: "base".into(),
                name: "base".into(),
                depuracao: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            },
        ];
        let planos = m
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        (m, planos)
    }

    #[test]
    fn caller_ancora_this_e_inicializacao_anterior_ao_super_sem_inventar_frescor() {
        let (mut m, mut planos) = modulo();
        let fatos = produzir(&m);
        assert_eq!(fatos["ctor"][&ValueId(0)], 123);
        assert_eq!(fatos["base"][&ValueId(0)], 123);
        preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
        assert!(m.functions[1].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_ref_v1")));
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
    }

    #[test]
    fn fronteiras_abertas_exigem_guarda_e_provas_estaticas_invalidas_recusam() {
        for caso in 0..7 {
            let (mut m, mut planos) = modulo();
            match caso {
                0 => m.ajudantes.push(("nativo".into(), "ctor".into())),
                1 => {
                    if let Instruction::CallStatic { args, .. } =
                        &mut m.functions[0].blocks[0].instructions[1].1
                    {
                        args[0] = Operand::Constant(Constant::Null);
                    }
                }
                2 => m.functions[1].blocks[0].instructions.swap(0, 1),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(9),
                    Instruction::Const(Constant::Funcao("ctor".into())),
                    Type::I64,
                )),
                4 => {
                    let mut caller = m.functions[0].clone();
                    caller.symbol = "outro".into();
                    caller.params.push((ValueId(8), "opaco".into(), Type::Ref));
                    if let Instruction::CallStatic { args, .. } =
                        &mut caller.blocks[0].instructions[1].1
                    {
                        args[0] = Operand::Val(ValueId(8));
                    }
                    m.functions.push(caller);
                    planos.insert("outro".into(), PlanoFuncaoDart::default());
                }
                5 => m.functions[1].blocks[0].terminator = Terminator::Branch(BlockId(0)),
                _ => {
                    let mut corpo = m.functions[1].blocks[0].clone();
                    corpo.id = BlockId(1);
                    m.functions[1].blocks[0].instructions.clear();
                    m.functions[1].blocks[0].terminator = Terminator::Branch(BlockId(1));
                    m.functions[1].blocks.push(corpo);
                }
            }
            if matches!(caso, 0 | 1 | 3 | 4) {
                assert!(
                    !produzir(&m).contains_key("ctor"),
                    "caso {caso}: entrada aberta ganhou classe"
                );
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if matches!(caso, 0 | 1 | 2 | 4 | 5 | 6) {
                // Entradas abertas não ganham classe por haver callers
                // conhecidos. Entradas expostas também exigem guarda física;
                // null, CID ou tamanho inválido serão recusados pelo runtime.
                resultado.unwrap();
                assert!(m.functions[1].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                    matches!(i, Instruction::CallRuntime { name, .. }
                        if name == "dartforge_arc_gravar_campo_ref_conferido_v1")));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                assert!(resultado.is_err(), "caso {caso}: {resultado:?}");
                assert_eq!(antes, format!("{m:?}/{planos:?}"));
            }
        }
    }
}
