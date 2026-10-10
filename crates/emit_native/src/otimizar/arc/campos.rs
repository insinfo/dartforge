//! Prova conservadora de origem local para acesso físico a campos.
//! Alocações HIR completas são as âncoras; cópias/moves/Phis transportam
//! conjuntos de sítios, sem forte atualização por instância. Publicação,
//! uso opaco ou contenção invalidam o sítio inteiro, inclusive antes do uso.
//! Não substitui guardas Dart late/tipo, resumos de heap ou versões/pins.

use super::{PlanoFuncaoDart, arestas::proximo_valor};
use crate::{hir::*, otimizar::operandos::operandos};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default, PartialEq, Eq)]
struct Origem {
    sitios: HashSet<ValueId>,
    desconhecido: bool,
}

fn origem(op: &Operand, origens: &HashMap<ValueId, Origem>) -> Origem {
    match op {
        Operand::Val(v) => origens.get(v).cloned().unwrap_or(Origem {
            desconhecido: true,
            ..Default::default()
        }),
        _ => Origem {
            desconhecido: true,
            ..Default::default()
        },
    }
}

fn origens(f: &Function) -> (HashMap<ValueId, Origem>, HashMap<ValueId, u32>) {
    let mut mapa = HashMap::new();
    let mut classes = HashMap::new();
    for (v, _, _) in &f.params {
        mapa.insert(
            *v,
            Origem {
                desconhecido: true,
                ..Default::default()
            },
        );
    }
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let o = match inst {
            Instruction::AllocObject { class_id, .. } => {
                classes.insert(*v, *class_id);
                Origem {
                    sitios: HashSet::from([*v]),
                    desconhecido: false,
                }
            }
            Instruction::ArcCopy { .. } | Instruction::ArcMove { .. } | Instruction::Phi { .. } => {
                Origem::default()
            }
            _ => Origem {
                desconhecido: true,
                ..Default::default()
            },
        };
        mapa.insert(*v, o);
    }
    loop {
        let mut mudou = false;
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let ops = match inst {
                Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
                Instruction::Phi { incoming, .. } => incoming.iter().map(|(_, o)| o).collect(),
                _ => continue,
            };
            let mut novo = Origem::default();
            for op in ops {
                let o = origem(op, &mapa);
                novo.sitios.extend(o.sitios);
                novo.desconhecido |= o.desconhecido;
            }
            if mapa[v] != novo {
                mapa.insert(*v, novo);
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    (mapa, classes)
}

fn representacao(
    receiver: &Operand,
    index: usize,
    origens: &HashMap<ValueId, Origem>,
    classes: &HashMap<ValueId, u32>,
    layouts: &HashMap<u32, Vec<Type>>,
    escapam: &HashSet<ValueId>,
) -> Result<Type, String> {
    let o = origem(receiver, origens);
    if o.desconhecido || o.sitios.is_empty() || !o.sitios.is_disjoint(escapam) {
        return Err("receiver sem origem local não publicada".into());
    }
    let mut repr = None;
    for sitio in o.sitios {
        let t = layouts
            .get(&classes[&sitio])
            .and_then(|l| l.get(index))
            .ok_or("classe/layout/índice de campo não certificado")?;
        if repr.is_some_and(|anterior| anterior != *t) {
            return Err("representações diferentes na junção de receivers".into());
        }
        repr = Some(*t);
    }
    Ok(repr.unwrap())
}

pub(super) fn preparar(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plano: &mut PlanoFuncaoDart,
) -> Result<(), String> {
    if !f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, i, _)| {
            matches!(
                i,
                Instruction::GetField { .. } | Instruction::SetField { .. }
            )
        })
    {
        return Ok(());
    }
    let (origens, classes) = origens(f);
    let mut escapam = HashSet::new();
    for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let mut expor = |op: &Operand| {
            escapam.extend(origem(op, &origens).sitios);
        };
        match inst {
            Instruction::GetField { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::Phi { .. } => {}
            Instruction::SetField { value, .. } => expor(value),
            Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut expor),
            Instruction::CallRuntime { name, args, .. } => {
                let seguro = matches!(
                    name.as_str(),
                    "dartforge_arc_retain"
                        | "dartforge_arc_release"
                        | "dartforge_arc_objeto_owned_v1"
                        | "dartforge_arc_box_int_owned_v1"
                        | "dartforge_arc_box_double_owned_v1"
                        | "dartforge_exception_pending"
                );
                let campo = match name.as_str() {
                    "dartforge_arc_ler_campo_ref_v1" => Some(true),
                    "dartforge_arc_ler_campo_escalar_v1" => Some(false),
                    _ => None,
                };
                let mut seguro_campo = false;
                if let Some(refs) = campo
                    && let [receiver, indice] = args.as_slice()
                    && let Operand::Constant(Constant::Int(n)) = indice.0
                    && let Ok(index) = usize::try_from(n)
                    && let Ok(repr) = representacao(
                        &receiver.0,
                        index,
                        &origens,
                        &classes,
                        layouts,
                        &HashSet::new(),
                    )
                {
                    seguro_campo = (repr == Type::Ref) == refs;
                }
                if !seguro && !seguro_campo {
                    // Setter de bits nativo não certifica a representação
                    // lógica (bool/byte). Só SetField tipado mantém a prova.
                    args.iter().for_each(|(o, _)| expor(o));
                    // Ausência de Dart no catálogo de ownership não prova
                    // ausência de mutação do layout, por exemplo na recarga.
                    // Só operações enumeradas acima têm contrato fechado aqui.
                    escapam.extend(classes.keys().copied());
                }
            }
            Instruction::CallStatic { .. }
            | Instruction::CallInterface { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallClosureRepasse { .. }
            | Instruction::ChamadaTipada { .. } => escapam.extend(classes.keys().copied()),
            _ => operandos(inst, &mut expor),
        }
    }
    let tipos: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let tipo = |op: &Operand| match op {
        Operand::Val(v) => tipos.get(v).copied(),
        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        _ => None,
    };
    let mut proximo = proximo_valor(f, plano);
    for b in &mut f.blocks {
        let mut instructions = Vec::new();
        for (v, inst, ty) in &b.instructions {
            let (object, index, value) = match inst {
                Instruction::GetField { object, index } => (object, *index, None),
                Instruction::SetField {
                    object,
                    index,
                    value,
                } => (object, *index, Some(value)),
                _ => {
                    instructions.push((*v, inst.clone(), *ty));
                    continue;
                }
            };
            let repr = representacao(object, index, &origens, &classes, layouts, &escapam)
                .map_err(|e| format!("campo ARC em {} v{}: {e}", f.symbol, v.0))?;
            if tipo(object) != Some(Type::Ref)
                || (value.is_none() && *ty != repr)
                || value.is_some_and(|o| tipo(o) != Some(repr) || *ty != Type::Void)
                || plano.tokens.pendencias.contains_key(v)
                || plano.tabelas.invocacoes.contains_key(v)
            {
                return Err(format!(
                    "campo ARC em {} v{}: tipo ou guarda incompatível",
                    f.symbol, v.0
                ));
            }
            let estreito = !matches!(repr, Type::I64 | Type::Ref);
            let auxiliar = if estreito {
                let a = ValueId(
                    u32::try_from(proximo)
                        .map_err(|_| "IDs SSA esgotados ao preparar campos ARC")?,
                );
                proximo += 1;
                if let Some(limites) = plano.escopos.antes.remove(v) {
                    plano.escopos.antes.insert(a, limites);
                }
                Some(a)
            } else {
                None
            };
            let mut args = vec![
                (object.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(index as i64)), Type::I64),
            ];
            if let Some(value) = value {
                let bits = if let Some(a) = auxiliar {
                    let conversao = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: value.clone(),
                            to: Type::I64,
                        }
                    } else {
                        Instruction::ZExt {
                            op: value.clone(),
                            from: repr,
                            to: Type::I64,
                        }
                    };
                    instructions.push((a, conversao, Type::I64));
                    Operand::Val(a)
                } else {
                    value.clone()
                };
                args.push((
                    bits,
                    if repr == Type::Ref {
                        Type::Ref
                    } else {
                        Type::I64
                    },
                ));
                instructions.push((
                    *v,
                    Instruction::CallRuntime {
                        name: if repr == Type::Ref {
                            "dartforge_arc_gravar_campo_ref_v1"
                        } else {
                            "dartforge_arc_gravar_campo_escalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            } else {
                let ret_ty = if repr == Type::Ref {
                    Type::Ref
                } else {
                    Type::I64
                };
                instructions.push((
                    auxiliar.unwrap_or(*v),
                    Instruction::CallRuntime {
                        name: if repr == Type::Ref {
                            "dartforge_arc_ler_campo_ref_v1"
                        } else {
                            "dartforge_arc_ler_campo_escalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty,
                    },
                    ret_ty,
                ));
                if let Some(a) = auxiliar {
                    let conversao = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: Operand::Val(a),
                            to: repr,
                        }
                    } else {
                        Instruction::Trunc {
                            op: Operand::Val(a),
                            from: Type::I64,
                            to: repr,
                        }
                    };
                    instructions.push((*v, conversao, repr));
                }
            }
        }
        b.instructions = instructions;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use crate::otimizar::arc::*;

    fn modulo(
        layout: Vec<Type>,
        fields: Vec<Operand>,
    ) -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: fields.len(),
            vtable: vec![],
            to_string_symbol: None,
        });
        m.layouts_campos_arc.insert(123, layout);
        m.functions.push(Function {
            symbol: "campos".into(),
            name: "campos".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        (
            m,
            HashMap::from([("campos".into(), PlanoFuncaoDart::default())]),
        )
    }

    #[test]
    fn leitura_ref_retida_antes_da_substituicao_da_aresta() {
        let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Val(ValueId(10))]);
        let f = &mut m.functions[0];
        f.return_ty = Type::Ref;
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(i64::MAX)),
                    from: Type::I64,
                },
                Type::Ref,
            ),
        );
        f.blocks[0].instructions.extend([
            (
                ValueId(1),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                    value: Operand::Constant(Constant::Null),
                },
                Type::Void,
            ),
        ]);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        planos.get_mut("campos").unwrap().tokens.retorno = RetornoTokens::Owned;
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (1, 2)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let copy = ops
            .iter()
            .position(|(v, i, _)| *v == ValueId(1) && matches!(i, Instruction::ArcCopy { .. }))
            .unwrap();
        let set = ops.iter().position(|(v, _, _)| *v == ValueId(2)).unwrap();
        assert!(copy < set);
        assert!(!ops.iter().any(|(_, i, _)| matches!(
            i,
            Instruction::GetField { .. } | Instruction::SetField { .. }
        )));
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn campos_estreitos_preservam_bits_e_limite_antes_da_leitura() {
        let (mut m, mut planos) = modulo(
            vec![Type::F64, Type::I1, Type::I8],
            vec![
                Operand::Constant(Constant::Double(-0.0)),
                Operand::Constant(Constant::Bool(true)),
                Operand::Val(ValueId(10)),
            ],
        );
        let f = &mut m.functions[0];
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Trunc {
                    op: Operand::Constant(Constant::Int(255)),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
        );
        for (i, repr) in [Type::F64, Type::I1, Type::I8].into_iter().enumerate() {
            f.blocks[0].instructions.push((
                ValueId(20 + i as u32),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                },
                repr,
            ));
            f.blocks[0].instructions.push((
                ValueId(30 + i as u32),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                    value: Operand::Val(ValueId(20 + i as u32)),
                },
                Type::Void,
            ));
        }
        let p = planos.get_mut("campos").unwrap();
        p.escopos
            .antes
            .insert(ValueId(20), vec![AlteracaoEscopo::Abrir(7)]);
        p.escopos
            .saidas
            .insert(BlockId(0), vec![AlteracaoEscopo::Fechar(7)]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 1)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let pos = ops.iter().position(|(v, _, _)| *v == ValueId(20)).unwrap();
        assert!(matches!(
            ops[pos].1,
            Instruction::Bitcast { to: Type::F64, .. }
        ));
        assert_eq!(
            planos["campos"].escopos.antes[&ops[pos - 1].0],
            vec![AlteracaoEscopo::Abrir(7)]
        );
        assert!(!planos["campos"].escopos.antes.contains_key(&ValueId(20)));
        for v in [21, 22] {
            assert!(matches!(
                ops.iter().find(|(id, _, _)| id.0 == v).unwrap().1,
                Instruction::Trunc {
                    from: Type::I64,
                    ..
                }
            ));
        }
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }

    #[test]
    fn aliases_publicados_e_layouts_incompativeis_rejeitam_atomicamente() {
        for caso in 0..8 {
            let (mut m, mut planos) =
                modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            let f = &mut m.functions[0];
            let mut receiver = Operand::Val(ValueId(0));
            let mut index = 0;
            let mut ty = Type::I64;
            match caso {
                0 => {
                    f.blocks[0].instructions.extend([
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: receiver.clone(),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_print_handle".into(),
                                args: vec![(Operand::Val(ValueId(1)), Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ]);
                }
                1 => {
                    f.params.push((ValueId(7), "C".into(), Type::Ref));
                    receiver = Operand::Val(ValueId(7));
                }
                2 => index = 1,
                3 => ty = Type::Ref,
                4 => {
                    f.blocks[0].instructions.push((
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_gravar_campo_ref_v1".into(),
                            args: vec![
                                (receiver.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
                5 => {
                    m.layouts_campos_arc.insert(123, vec![Type::F64]);
                    ty = Type::F64;
                    planos
                        .get_mut("campos")
                        .unwrap()
                        .classes
                        .insert(ValueId(u32::MAX), Ownership::Trivial);
                }
                6 => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::SetField {
                            object: receiver.clone(),
                            index: 0,
                            value: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ));
                }
                _ => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::CallRuntime {
                            name: "extern_sem_resumo".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
            }
            f.blocks[0].instructions.push((
                ValueId(5),
                Instruction::GetField {
                    object: receiver,
                    index,
                },
                ty,
            ));
            let antes = format!("{m:?}/{planos:?}");
            let erro = preparar_arc_modulo_dart(&mut m, &mut planos).unwrap_err();
            if matches!(caso, 0 | 1 | 4 | 7) {
                assert!(erro.contains("origem local"), "caso {caso}: {erro}");
            }
            assert_eq!(format!("{m:?}/{planos:?}"), antes, "caso {caso}");
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }

    #[test]
    fn phi_com_origem_desconhecida_nao_esconde_escape_da_alocacao() {
        let (mut m, mut planos) =
            modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![
            (ValueId(7), "outro".into(), Type::Ref),
            (ValueId(8), "cond".into(), Type::I1),
        ];
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(8)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for b in [1, 2] {
            f.blocks.push(BasicBlock {
                id: BlockId(b),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            });
        }
        f.blocks.push(BasicBlock {
            id: BlockId(3),
            instructions: vec![
                (
                    ValueId(9),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Val(ValueId(7))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(10),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".into(),
                        args: vec![(Operand::Val(ValueId(9)), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(11),
                    Instruction::GetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                    },
                    Type::I64,
                ),
            ],
            terminator: Terminator::Return(None),
        });
        let antes = format!("{m:?}/{planos:?}");
        assert!(
            preparar_arc_modulo_dart(&mut m, &mut planos)
                .unwrap_err()
                .contains("origem local")
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn phi_e_copia_conservam_todos_os_layouts_candidatos() {
        for caso in 0..3 {
            let (mut m, mut planos) =
                modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            m.classes.push(ClassDef {
                id: 124,
                name: "D".into(),
                field_count: 1,
                vtable: vec![],
                to_string_symbol: None,
            });
            let repr = if caso == 1 { Type::Ref } else { Type::I64 };
            m.layouts_campos_arc.insert(124, vec![repr]);
            let outro = if caso == 1 {
                Operand::Constant(Constant::Null)
            } else {
                Operand::Constant(Constant::Int(86))
            };
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "cond".into(), Type::I1)];
            f.return_ty = Type::I64;
            f.blocks = vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 123,
                            fields: vec![Operand::Constant(Constant::Int(85))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::AllocObject {
                            class_id: 124,
                            fields: vec![outro],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::Phi {
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (
                                        BlockId(2),
                                        if caso == 2 {
                                            Operand::Constant(Constant::Null)
                                        } else {
                                            Operand::Val(ValueId(2))
                                        },
                                    ),
                                ],
                                ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(4),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(3)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(5),
                            Instruction::GetField {
                                object: Operand::Val(ValueId(4)),
                                index: 0,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
            ];
            let antes = format!("{m:?}/{planos:?}");
            if caso != 0 {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 2)
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(
                    ir.contains("call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %v4, i64 0)")
                );
            }
        }
    }

    #[test]
    fn phi_ancorado_em_laco_conserva_origem_sem_presumir_singleton() {
        let (mut m, mut planos) =
            modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![(ValueId(1), "repetir".into(), Type::I1)];
        f.return_ty = Type::I64;
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.extend([
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                        },
                        Type::I64,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(1)),
                    then_block: BlockId(2),
                    else_block: BlockId(3),
                },
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
            },
        ]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 1)
        );
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }
}
