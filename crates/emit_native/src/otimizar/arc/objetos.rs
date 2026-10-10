//! Materializa alocação Owned e inicialização de campos com layout explícito.
//! Não deduz layout de receivers, guardas late/tipo ou versões do heap.
//! Só roda na transação privada do módulo ARC. O emissor conserva o registro
//! de métodos da fábrica; construtores e inicializadores continuam separados.

use super::{PlanoFuncaoDart, arestas::proximo_valor};
use crate::hir::*;
use std::collections::HashMap;

pub(super) fn preparar(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plano: &PlanoFuncaoDart,
) -> Result<(), String> {
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
    let mut proximo = proximo_valor(f, plano);
    let mut id = || -> Result<ValueId, String> {
        let v = u32::try_from(proximo)
            .map_err(|_| format!("IDs SSA esgotados ao inicializar objetos em {}", f.symbol))?;
        proximo += 1;
        Ok(ValueId(v))
    };
    let tipo = |op: &Operand| match op {
        Operand::Val(v) => tipos.get(v).copied(),
        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        // Materialização de literais/endereço nativo precisa estar explícita
        // antes da alocação; não esconder safepoints em argumentos dos setters.
        _ => None,
    };
    for b in &mut f.blocks {
        let mut instructions = Vec::new();
        for (v, inst, ty) in &b.instructions {
            if let Instruction::CallRuntime { name, args, ret_ty } = inst
                && name == "dartforge_object_new"
            {
                let erro = || {
                    format!(
                        "alocação zerada ARC em {} v{}: classe/quantidade/layout incompatível",
                        f.symbol, v.0
                    )
                };
                let [
                    (Operand::Constant(Constant::Int(classe)), Type::I64),
                    (Operand::Constant(Constant::Int(campos)), Type::I64),
                ] = args.as_slice()
                else {
                    return Err(erro());
                };
                let classe = u32::try_from(*classe).map_err(|_| erro())?;
                let campos = u16::try_from(*campos).map_err(|_| erro())?;
                if *ty != Type::Ref
                    || *ret_ty != Type::Ref
                    || i32::try_from(classe).is_err()
                    || layouts
                        .get(&classe)
                        .is_none_or(|l| l.len() != usize::from(campos))
                {
                    return Err(erro());
                }
                // Instância ainda em construção: apenas a alocação ganha um
                // token. Guardas, RTI, inicializadores e construtor continuam
                // nas operações seguintes. Não certifica seus campos zerados.
                instructions.push((
                    *v,
                    Instruction::CallRuntime {
                        name: "dartforge_arc_objeto_owned_v1".into(),
                        args: args.clone(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
                continue;
            }
            let Instruction::AllocObject { class_id, fields } = inst else {
                instructions.push((*v, inst.clone(), *ty));
                continue;
            };
            let layout = layouts.get(class_id).ok_or_else(|| {
                format!(
                    "alocação ARC em {} v{}: classe {class_id} sem layout",
                    f.symbol, v.0
                )
            })?;
            if *ty != Type::Ref
                || i32::try_from(*class_id).is_err()
                || u16::try_from(fields.len()).is_err()
                || layout.len() != fields.len()
            {
                return Err(format!(
                    "alocação ARC em {} v{}: representação ou quantidade incompatível",
                    f.symbol, v.0
                ));
            }
            for (i, (op, esperado)) in fields.iter().zip(layout).enumerate() {
                if tipo(op) != Some(*esperado) {
                    return Err(format!(
                        "alocação ARC em {} v{}: campo {i} incompatível com {esperado:?}",
                        f.symbol, v.0
                    ));
                }
            }
            instructions.push((
                *v,
                Instruction::CallRuntime {
                    name: "dartforge_arc_objeto_owned_v1".into(),
                    args: vec![
                        (
                            Operand::Constant(Constant::Int(i64::from(*class_id))),
                            Type::I64,
                        ),
                        (
                            Operand::Constant(Constant::Int(fields.len() as i64)),
                            Type::I64,
                        ),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
            for (i, (op, repr)) in fields.iter().zip(layout).enumerate() {
                let (valor, argumento, nome) = if *repr == Type::Ref {
                    (op.clone(), Type::Ref, "dartforge_arc_gravar_campo_ref_v1")
                } else {
                    let bits = if *repr == Type::I64 {
                        op.clone()
                    } else {
                        let bits = id()?;
                        let conversao = if *repr == Type::F64 {
                            Instruction::Bitcast {
                                op: op.clone(),
                                to: Type::I64,
                            }
                        } else {
                            Instruction::ZExt {
                                op: op.clone(),
                                from: *repr,
                                to: Type::I64,
                            }
                        };
                        instructions.push((bits, conversao, Type::I64));
                        Operand::Val(bits)
                    };
                    (bits, Type::I64, "dartforge_arc_gravar_campo_escalar_v1")
                };
                instructions.push((
                    id()?,
                    Instruction::CallRuntime {
                        name: nome.into(),
                        args: vec![
                            (Operand::Val(*v), Type::Ref),
                            (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                            (valor, argumento),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
        }
        b.instructions = instructions;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use crate::otimizar::arc::*;

    #[test]
    fn ponteiro_de_tabela_sem_proveniencia_nao_ganha_contrato_runtime() {
        let (mut m, _) = modulo();
        m.functions[0].params.clear();
        m.functions[0].blocks[0].instructions = vec![(
            ValueId(2),
            Instruction::CallRuntime {
                name: "dartforge_arc_objeto_owned_t_v1".into(),
                args: vec![
                    (Operand::Constant(Constant::Int(123)), Type::I64),
                    (Operand::Constant(Constant::Int(5)), Type::I64),
                    (
                        Operand::Constant(Constant::Funcao("arbitraria".into())),
                        Type::Ptr,
                    ),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        let mut classes = HashMap::new();
        let mut tokens = PlanoTokens::default();
        let antes = format!("{classes:?}{tokens:?}");
        let erro =
            produzir_contratos_runtime(&m.functions[0], &mut classes, &mut tokens).unwrap_err();
        assert!(
            erro.contains("tabela estática exige proveniência"),
            "{erro}"
        );
        assert_eq!(format!("{classes:?}{tokens:?}"), antes);
    }

    #[test]
    fn instancia_zerada_preserva_construtor_e_recusa_metadados_invalidos_atomicamente() {
        for caso in 0..7 {
            let (mut m, mut planos) = modulo();
            m.functions[0].params.clear();
            m.functions[0].blocks[0].instructions = vec![
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_object_new".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(123)), Type::I64),
                            (Operand::Constant(Constant::Int(5)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "construtor".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ];
            m.functions.push(Function {
                symbol: "construtor".into(),
                name: "construtor".into(),
                depuracao: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            });
            planos.insert("construtor".into(), PlanoFuncaoDart::default());
            let Instruction::CallRuntime { args, ret_ty, .. } =
                &mut m.functions[0].blocks[0].instructions[0].1
            else {
                unreachable!()
            };
            match caso {
                1 => args[1].0 = Operand::Constant(Constant::Int(4)),
                2 => args[0].0 = Operand::Constant(Constant::Int(999)),
                3 => args[1].1 = Type::Ref,
                4 => *ret_ty = Type::I64,
                5 => args[1].0 = Operand::Constant(Constant::Int(-1)),
                6 => {
                    m.memoria_arc = false;
                }
                _ => {}
            }
            let antes = format!("{m:?}{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if (1..6).contains(&caso) {
                assert!(resultado.is_err(), "caso {caso}");
                assert_eq!(format!("{m:?}{planos:?}"), antes);
            } else if caso == 6 {
                assert_eq!(resultado.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}{planos:?}"), antes);
            } else {
                resultado.unwrap();
                assert_eq!(planos["objeto"].classes[&ValueId(2)], Ownership::Owned);
                let ops: Vec<_> = m.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .collect();
                assert!(ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_objeto_owned_v1")));
                assert!(ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallStatic { symbol, args, .. }
                    if symbol == "construtor" && args == &vec![Operand::Val(ValueId(2))])));
                assert!(!ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_gravar_campo"))));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            }
        }
    }

    fn modulo() -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 5,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.layouts_campos_arc.insert(
            123,
            vec![Type::Ref, Type::I64, Type::F64, Type::I1, Type::I8],
        );
        m.functions.push(Function {
            symbol: "objeto".into(),
            name: "objeto".into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "filho".into(), Type::Ref),
                (ValueId(1), "byte".into(), Type::I8),
            ],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields: vec![
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(i64::MAX)),
                            Operand::Constant(Constant::Double(-0.0)),
                            Operand::Constant(Constant::Bool(true)),
                            Operand::Val(ValueId(1)),
                        ],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        });
        let mut p = PlanoFuncaoDart::default();
        p.tokens.retorno = RetornoTokens::Owned;
        (m, HashMap::from([("objeto".into(), p)]))
    }

    #[test]
    fn alocacao_inicializa_ref_e_bits_sem_converter_double_numericamente() {
        let (mut m, mut planos) = modulo();
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_objeto_owned_v1"));
        assert_eq!(planos["objeto"].classes[&ValueId(2)], Ownership::Owned);
        assert_eq!(
            ops.iter()
                .filter(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_ref_v1"))
                .count(),
            1
        );
        assert_eq!(ops.iter().filter(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_escalar_v1")).count(), 4);
        assert!(ops.iter().any(|(_, i, _)| matches!(i, Instruction::Bitcast {
            op: Operand::Constant(Constant::Double(n)), to: Type::I64 } if n.to_bits() == (-0.0f64).to_bits())));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("bitcast double"));
        assert!(!ir.contains("call i64 @dartforge_object_new("));
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn alocacao_invalida_ou_falha_posterior_nao_publica_transformacao() {
        for caso in 0..6 {
            let (mut m, mut planos) = modulo();
            match caso {
                0 => m.layouts_campos_arc.clear(),
                1 => {
                    let Instruction::AllocObject { fields, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    fields[0] = Operand::Constant(Constant::Int(85));
                }
                2 => {
                    let Instruction::AllocObject { fields, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    fields.pop();
                }
                3 => {
                    planos
                        .get_mut("objeto")
                        .unwrap()
                        .classes
                        .insert(ValueId(u32::MAX), Ownership::Trivial);
                }
                4 => {
                    m.functions[0].blocks[0].instructions.push((
                        ValueId(99),
                        Instruction::CallRuntime {
                            name: "extern_sem_contrato".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
                _ => {
                    let mut segunda = m.functions[0].clone();
                    segunda.symbol = "segunda".into();
                    segunda.params[0].2 = Type::I64;
                    m.functions.push(segunda);
                    planos.insert("segunda".into(), PlanoFuncaoDart::default());
                }
            }
            let antes = format!("{m:?}/{planos:?}");
            assert!(
                preparar_arc_modulo_dart(&mut m, &mut planos).is_err(),
                "caso {caso}"
            );
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
    fn filho_owned_e_objeto_tem_cleanup_depois_da_inicializacao() {
        for quantidade in [0, 1, 40] {
            let (mut m, mut planos) = modulo();
            m.classes[0].field_count = quantidade;
            let mut layout = vec![Type::I64; quantidade];
            let mut fields = vec![Operand::Constant(Constant::Int(85)); quantidade];
            if quantidade != 0 {
                layout[quantidade - 1] = Type::Ref;
                fields[quantidade - 1] = Operand::Val(ValueId(0));
            }
            m.layouts_campos_arc.insert(123, layout);
            let f = &mut m.functions[0];
            f.params.clear();
            f.return_ty = Type::Void;
            f.blocks[0].instructions = vec![
                (
                    ValueId(0),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                ),
            ];
            f.blocks[0].terminator = Terminator::Return(None);
            planos.get_mut("objeto").unwrap().tokens.retorno = RetornoTokens::Trivial;
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 2)
            );
            let ops = &m.functions[0].blocks[0].instructions;
            assert_eq!(
                ops.iter()
                    .filter(|(_, i, _)| matches!(i, Instruction::ArcDrop { .. }))
                    .count(),
                2
            );
            assert!(
                ops[..ops.len() - 2]
                    .iter()
                    .all(|(_, i, _)| !matches!(i, Instruction::ArcDrop { .. }))
            );
            if quantidade != 0 {
                assert!(
                    matches!(&ops[ops.len() - 3].1, Instruction::CallRuntime { name, args, .. }
                    if name == "dartforge_arc_gravar_campo_ref_v1"
                    && args[1].0 == Operand::Constant(Constant::Int((quantidade - 1) as i64))
                    && args[2] == (Operand::Val(ValueId(0)), Type::Ref))
                );
            }
            crate::llvm::LlvmEmitter::new(&m).emit_all();
        }
    }

    #[test]
    fn campo_emprestado_ganha_owner_antes_do_safepoint_da_alocacao() {
        let (mut m, mut planos) = modulo();
        m.classes[0].field_count = 1;
        m.layouts_campos_arc.insert(123, vec![Type::Ref]);
        let f = &mut m.functions[0];
        f.params.truncate(1);
        f.blocks[0].instructions = vec![
            (
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::AllocObject {
                    class_id: 123,
                    fields: vec![Operand::Val(ValueId(1))],
                },
                Type::Ref,
            ),
        ];
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (1, 1)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        assert_eq!(ops[1].0, ValueId(1));
        assert!(matches!(ops[1].1, Instruction::ArcCopy { .. }));
        assert!(matches!(&ops[2].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_objeto_owned_v1"));
        assert!(matches!(
            ops.last().unwrap().1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(1))
            }
        ));
        assert_eq!(planos["objeto"].classes[&ValueId(1)], Ownership::Owned);
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }
}
