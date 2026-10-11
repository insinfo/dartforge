//! Seleciona entradas auditadas para operações nativas ARC.
//! Os contratos conferem SSA, pendência e callbacks na preparação completa.

use crate::hir::*;

pub(super) fn preparar(f: &mut Function) -> Result<(), String> {
    for (v, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        let Instruction::CallRuntime { name, args, ret_ty } = inst else {
            continue;
        };
        if name == "df.lista_len" {
            if *ty != Type::I64
                || *ret_ty != Type::I64
                || !matches!(args.as_slice(), [(_, Type::Ref)])
            {
                return Err(format!(
                    "comprimento ARC em {} v{}: ABI Ref/I64 incompatível",
                    f.symbol, v.0
                ));
            }
            *name = "dartforge_arc_lista_len_v1".into();
            continue;
        }
        if name != "dartforge_nativo_Socket_RecvFrom" {
            continue;
        }
        if *ty != Type::Ref || *ret_ty != Type::Ref || !matches!(args.as_slice(), [(_, Type::Ref)])
        {
            return Err(format!(
                "recepção ARC em {} v{}: ABI Ref incompatível",
                f.symbol, v.0
            ));
        }
        *name = "dartforge_arc_socket_recvfrom_owned_v1".into();
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use crate::otimizar::arc::*;
    use std::collections::HashMap;

    #[test]
    fn literal_produz_owner_por_tipos_reais_e_recusa_abi_incompativel() {
        for caso in 0..8 {
            let mut m = Module::new();
            m.memoria_arc = caso != 7;
            let t = match caso { 2 => Type::Ptr, 3 => Type::V4I32, 4 => Type::I8, _ => Type::Ref };
            let mut elements = if caso == 0 { vec![] } else { vec![(Operand::Val(ValueId(0)), 255),
                (Operand::Constant(Constant::Int(i64::MAX)), 255),
                (Operand::Constant(Constant::Double(-0.0)), 255),
                (Operand::Constant(Constant::Bool(true)), 255)] };
            if caso == 5 { elements.push((Operand::Constant(Constant::Funcao("alvo".into())), 1)); }
            m.functions.push(Function { symbol: "literal".into(), name: "literal".into(), depuracao: None,
                params: vec![(ValueId(0), "elemento".into(), t)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1),
                    Instruction::AllocList { elements }, if caso == 6 { Type::I64 } else { Type::Ref })],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("literal".into(), p)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso < 2 {
                r.unwrap();
                assert_eq!(planos["literal"].classes[&ValueId(1)], Ownership::Owned);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_lista_nova_owned_v1(ptr %list_buf_1"));
                if caso == 1 { assert!(ir.contains("store i8 3") && ir.contains("store i8 4")); }
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                if caso == 7 {
                    assert_eq!(r.unwrap(), (0, 0));
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(ir.contains("call i64 @dartforge_lista_nova(ptr %list_buf_1"));
                } else { assert!(r.is_err(), "caso {caso}"); }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn phi_null_aguarda_join_owned_e_recusa_premissa_trivial_incompativel() {
        for caso in 0..3 {
            let val = |n| Operand::Val(ValueId(n));
            let phi = |entradas| Instruction::Phi {
                incoming: entradas,
                ty: Type::Ref,
            };
            let caixa = |n| {
                if caso == 1 {
                    Instruction::Const(Constant::Null)
                } else {
                    Instruction::CallRuntime {
                        name: "dartforge_arc_box_int_owned_v1".into(),
                        args: vec![(Operand::Constant(Constant::Int(n)), Type::I64)],
                        ret_ty: Type::Ref,
                    }
                }
            };
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(Function {
                symbol: "phi_misto".into(),
                name: "phi_misto".into(),
                depuracao: None,
                params: vec![
                    (ValueId(100), "externo".into(), Type::I1),
                    (ValueId(101), "interno".into(), Type::I1),
                ],
                return_ty: Type::I64,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: val(100),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: val(101),
                            then_block: BlockId(4),
                            else_block: BlockId(5),
                        },
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                    // Ordem física: o Phi externo é visitado antes do join interno.
                    BasicBlock {
                        id: BlockId(3),
                        instructions: vec![
                            (
                                ValueId(3),
                                phi(vec![
                                    (BlockId(2), Operand::Constant(Constant::Null)),
                                    (BlockId(6), val(2)),
                                ]),
                                Type::Ref,
                            ),
                            (
                                ValueId(4),
                                Instruction::CallRuntime {
                                    name: "dartforge_value_class".into(),
                                    args: vec![(val(3), Type::Ref)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            ),
                        ],
                        terminator: Terminator::Return(Some(val(4))),
                    },
                    BasicBlock {
                        id: BlockId(4),
                        instructions: vec![(ValueId(0), caixa(i64::MAX), Type::Ref)],
                        terminator: Terminator::Branch(BlockId(6)),
                    },
                    BasicBlock {
                        id: BlockId(5),
                        instructions: vec![(ValueId(1), caixa(i64::MIN), Type::Ref)],
                        terminator: Terminator::Branch(BlockId(6)),
                    },
                    BasicBlock {
                        id: BlockId(6),
                        instructions: vec![(
                            ValueId(2),
                            phi(vec![(BlockId(4), val(0)), (BlockId(5), val(1))]),
                            Type::Ref,
                        )],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                ],
            });
            let mut plano = PlanoFuncaoDart::default();
            if caso == 2 {
                plano.classes.insert(ValueId(3), Ownership::Trivial);
            }
            let mut planos = HashMap::from([("phi_misto".into(), plano)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 2 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
                continue;
            }
            r.unwrap();
            let esperada = if caso == 1 {
                Ownership::Trivial
            } else {
                Ownership::Owned
            };
            for v in [ValueId(2), ValueId(3)] {
                assert_eq!(planos["phi_misto"].classes[&v], esperada);
            }
            if caso == 0 {
                assert!(
                    m.functions[0]
                        .blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| matches!(
                            i,
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(3))
                            }
                        ))
                );
            }
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
        }
    }

    #[test]
    fn acrescentar_preserva_borrow_do_receiver_e_protege_storage_anterior_ao_gc() {
        for caso in 0..6 {
            let mut m = Module::new();
            m.memoria_arc = caso != 5;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "crescer".into(),
                name: "crescer".into(),
                depuracao: None,
                params: vec![
                    (
                        ValueId(0),
                        "lista".into(),
                        if caso == 2 { Type::I64 } else { Type::Ref },
                    ),
                    (
                        ValueId(1),
                        "valor".into(),
                        if caso == 3 { Type::I64 } else { Type::Ref },
                    ),
                ],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_armazenamento_ref_v1".into(),
                                args: vec![(val(0), Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_lista_acrescentar".into(),
                                args: vec![(val(0), Type::Ref), (val(1), Type::Ref)],
                                ret_ty: if caso == 4 { Type::I64 } else { Type::Ref },
                            },
                            Type::Ref,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(if caso == 1 { 2 } else { 3 }))),
                }],
            });
            let contrato = contrato_chamada_runtime(&m.functions[0].blocks[0].instructions[1].1);
            if caso != 4 {
                assert_eq!(
                    contrato.unwrap().resultado,
                    Ownership::Borrowed {
                        owner: OrigemOwner::Valor(ValueId(0)),
                        escopo: 0,
                    }
                );
            } else {
                assert!(contrato.is_err());
            }
            let mut plano = PlanoFuncaoDart::default();
            plano.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("crescer".into(), plano)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso <= 1 {
                assert_eq!(r.unwrap(), (1, 0));
                let insts = &m.functions[0].blocks[0].instructions;
                let copia = insts
                    .iter()
                    .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                    .unwrap();
                let chamada = insts
                    .iter()
                    .position(|(_, i, _)| {
                        matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_lista_acrescentar")
                    })
                    .unwrap();
                assert_eq!(copia < chamada, caso == 1);
                let Terminator::Return(Some(Operand::Val(retorno))) =
                    m.functions[0].blocks[0].terminator
                else {
                    panic!("retorno SSA esperado");
                };
                assert_eq!(planos["crescer"].classes[&retorno], Ownership::Owned);
                if caso == 0 {
                    assert_ne!(retorno, ValueId(3));
                    assert!(matches!(
                        planos["crescer"].classes[&ValueId(3)],
                        Ownership::Borrowed { .. }
                    ));
                }
                let preparado = format!("{m:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), preparado);
            } else {
                if caso == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn armazenamento_ref_recebe_owner_antes_de_mutacao_e_recusa_bits_nativos() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.memoria_arc = true;
            let val = |n| Operand::Val(ValueId(n));
            m.functions.push(Function {
                symbol: "armazenamento".into(),
                name: "armazenamento".into(),
                depuracao: None,
                params: vec![(ValueId(0), "lista".into(), ty)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_armazenamento_ref_v1".into(),
                                args: vec![(val(0), Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_forma_v1".into(),
                                args: vec![(val(1), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_lista_gravar_len_v1".into(),
                                args: vec![
                                    (val(0), Type::Ref),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(Some(val(1))),
                }],
            });
            let mut planos = HashMap::from([("armazenamento".into(), PlanoFuncaoDart::default())]);
            planos.get_mut("armazenamento").unwrap().tokens.retorno = RetornoTokens::Owned;
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if ty == Type::I64 {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
                continue;
            }
            assert_eq!(r.unwrap().0, 1);
            let insts = &m.functions[0].blocks[0].instructions;
            let copia = insts
                .iter()
                .position(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                .unwrap();
            let mutacao = insts.iter().position(|(_, i, _)| matches!(i,
                Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_gravar_len_v1")).unwrap();
            assert!(copia < mutacao);
            assert_eq!(
                planos["armazenamento"].classes[&insts[copia].0],
                Ownership::Owned
            );
            let preparado = format!("{m:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}"), preparado);
        }
    }
    #[test]
    fn comprimento_empresta_ref_e_recusa_abi_invalida_sem_alterar_tracing() {
        for caso in 0..6 {
            let mut m = Module::new();
            m.memoria_arc = caso != 5;
            m.functions.push(Function {
                symbol: "comprimento".into(),
                name: "comprimento".into(),
                depuracao: None,
                params: vec![(
                    ValueId(0),
                    "lista".into(),
                    if caso == 1 { Type::I64 } else { Type::Ref },
                )],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "df.lista_len".into(),
                            args: if caso == 2 {
                                vec![]
                            } else {
                                vec![(
                                    Operand::Val(ValueId(0)),
                                    if caso == 3 { Type::I64 } else { Type::Ref },
                                )]
                            },
                            ret_ty: if caso == 4 { Type::Ref } else { Type::I64 },
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut planos = HashMap::from([("comprimento".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(
                    planos["comprimento"].classes[&ValueId(1)],
                    Ownership::Trivial
                );
                assert!(planos["comprimento"].tokens.pendencias.is_empty());
                assert!(
                    matches!(&m.functions[0].blocks[0].instructions[0].1,                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lista_len_v1")
                );
                let preparado = format!("{m:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}"), preparado);
            } else {
                if caso == 5 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err());
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn consulta_de_classe_empresta_ref_sem_criar_token_ou_aceitar_bits_nativos() {
        for ty in [Type::Ref, Type::I64] {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(Function {
                symbol: "classe".into(),
                name: "classe".into(),
                depuracao: None,
                params: vec![(ValueId(0), "valor".into(), ty)],
                return_ty: Type::I64,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_value_class".into(),
                            args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut planos = HashMap::from([("classe".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if ty == Type::Ref {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(planos["classe"].classes[&ValueId(1)], Ownership::Trivial);
                assert!(planos["classe"].tokens.pendencias.is_empty());
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            } else {
                assert!(r.is_err());
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }

    #[test]
    fn recepcao_owned_preserva_pending_sem_promover_abi_invalida_ou_tracing() {
        for caso in 0..5 {
            let mut m = Module::new();
            m.memoria_arc = caso != 4;
            m.functions.push(Function {
                symbol: "receber".into(),
                name: "receber".into(),
                depuracao: None,
                params: vec![(
                    ValueId(0),
                    "socket".into(),
                    if caso == 1 { Type::I64 } else { Type::Ref },
                )],
                return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: "dartforge_nativo_Socket_RecvFrom".into(),
                                    args: if caso == 2 {
                                        vec![]
                                    } else {
                                        vec![(Operand::Val(ValueId(0)), Type::Ref)]
                                    },
                                    ret_ty: if caso == 3 { Type::I64 } else { Type::Ref },
                                },
                                Type::Ref,
                            ),
                            (
                                ValueId(2),
                                Instruction::CallRuntime {
                                    name: "dartforge_exception_pending".into(),
                                    args: vec![],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            ),
                            (
                                ValueId(3),
                                Instruction::ICmp(
                                    ICmpOp::Ne,
                                    Operand::Val(ValueId(2)),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            });
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            let mut planos = HashMap::from([("receber".into(), p)]);
            let antes = format!("{m:?}");
            let ps = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 0 {
                r.unwrap();
                assert_eq!(planos["receber"].classes[&ValueId(1)], Ownership::Owned);
                assert!(
                    planos["receber"]
                        .tokens
                        .pendencias
                        .contains_key(&ValueId(1))
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_socket_recvfrom_owned_v1"));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            } else {
                if caso == 4 {
                    assert_eq!(r.unwrap(), (0, 0));
                } else {
                    assert!(r.is_err(), "caso {caso}");
                }
                assert_eq!(format!("{m:?}"), antes);
                assert_eq!(planos, ps);
            }
        }
    }
}
