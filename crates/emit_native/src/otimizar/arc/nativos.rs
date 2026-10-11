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
