//! Explicita owners em globais Ref após conferir declaração e todas as escritas.
//! O endereço permanece privado ao armazenamento; o conteúdo não é token do código.

use super::*;

pub(super) fn preparar(m: &mut Module) -> Result<(), String> {
    if !m.memoria_arc {
        return Ok(());
    }
    let mut declaracoes = HashMap::new();
    for (_, ty, s) in &m.globais {
        if declaracoes.insert(s.as_str(), *ty).is_some() {
            return Err(format!("globais ARC: declaração repetida {s}"));
        }
    }
    let refs: HashSet<_> = declaracoes
        .iter()
        .filter_map(|(s, ty)| (*ty == Type::Ref).then_some(*s))
        .collect();
    for f in &m.functions {
        let tipos: HashMap<_, _> = f
            .params
            .iter()
            .map(|(v, _, t)| (*v, *t))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, t)| (*v, *t)),
            )
            .collect();
        let referencia = |o: &Operand| match o {
            Operand::Val(v) => tipos.get(v) == Some(&Type::Ref),
            Operand::Constant(Constant::Null) => true,
            _ => false,
        };
        let mut afetada = false;
        let mut endereco_publicado = false;
        let mut constante = |o: &Operand| {
            if let Operand::Constant(Constant::Funcao(s)) = o {
                endereco_publicado |= refs.contains(s.as_str());
            }
        };
        for b in &f.blocks {
            for (v, inst, ty) in &b.instructions {
                let ref_global = match inst {
                    Instruction::LoadGlobal {
                        simbolo,
                        ty: Type::Ref,
                    }
                    | Instruction::StoreGlobal {
                        simbolo,
                        ty: Type::Ref,
                        ..
                    }
                    | Instruction::ArcLoadStrong {
                        slot: SlotForte::Global { simbolo },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: SlotForte::Global { simbolo },
                        ..
                    } => Some(simbolo),
                    _ => None,
                };
                if ref_global.is_some_and(|s| !refs.contains(s.as_str())) {
                    return Err(format!(
                        "globais ARC em {} v{}: Ref sem declaração",
                        f.symbol, v.0
                    ));
                }
                if let Instruction::Const(Constant::Funcao(s)) = inst {
                    constante(&Operand::Constant(Constant::Funcao(s.clone())));
                }
                super::super::operandos::operandos(inst, &mut constante);
                let valido = match inst {
                    Instruction::LoadGlobal { simbolo, ty: lido }
                        if refs.contains(simbolo.as_str()) =>
                    {
                        afetada = true;
                        *lido == Type::Ref && *ty == Type::Ref
                    }
                    Instruction::StoreGlobal {
                        simbolo,
                        ty: gravado,
                        raiz,
                        val,
                    } if refs.contains(simbolo.as_str()) => {
                        afetada = true;
                        *gravado == Type::Ref
                            && *ty == Type::Void
                            && raiz.is_some()
                            && referencia(val)
                    }
                    Instruction::ArcLoadStrong {
                        slot: SlotForte::Global { simbolo },
                    } if refs.contains(simbolo.as_str()) => {
                        afetada = true;
                        *ty == Type::Ref
                    }
                    Instruction::ArcStoreStrong {
                        slot: SlotForte::Global { simbolo },
                        value,
                        ..
                    } if refs.contains(simbolo.as_str()) => {
                        afetada = true;
                        *ty == Type::Void && referencia(value)
                    }
                    _ => true,
                };
                if !valido {
                    return Err(format!(
                        "globais ARC em {} v{}: acesso Ref incompatível",
                        f.symbol, v.0
                    ));
                }
            }
            super::super::operandos::operandos_do_terminador(&b.terminator, &mut constante);
        }
        if endereco_publicado {
            return Err(format!(
                "globais ARC em {}: endereço Ref publicado",
                f.symbol
            ));
        }
        if afetada {
            super::ssa::verificar(f)?;
        }
    }
    let mut mudou = false;
    for f in &mut m.functions {
        for (_, inst, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            *inst = match inst {
                Instruction::LoadGlobal { simbolo, .. } if refs.contains(simbolo.as_str()) => {
                    Instruction::ArcLoadStrong {
                        slot: SlotForte::Global {
                            simbolo: simbolo.clone(),
                        },
                    }
                }
                Instruction::StoreGlobal { simbolo, val, .. }
                    if refs.contains(simbolo.as_str()) =>
                {
                    Instruction::ArcStoreStrong {
                        slot: SlotForte::Global {
                            simbolo: simbolo.clone(),
                        },
                        value: val.clone(),
                        modo: ModoStoreForte::Copy,
                    }
                }
                _ => continue,
            };
            mudou = true;
        }
    }
    if mudou {
        m.portas_arc_aot = None;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn origem() -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.globais.push((0, Type::Ref, "g".into()));
        m.functions.push(Function {
            symbol: "f".into(),
            name: "f".into(),
            params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
            return_ty: Type::Void,
            depuracao: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::StoreGlobal {
                            simbolo: "g".into(),
                            val: Operand::Val(ValueId(0)),
                            ty: Type::Ref,
                            raiz: Some(0),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(2),
                        Instruction::LoadGlobal {
                            simbolo: "g".into(),
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::StoreGlobal {
                            simbolo: "g".into(),
                            val: Operand::Constant(Constant::Null),
                            ty: Type::Ref,
                            raiz: Some(0),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(4),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        m
    }

    #[test]
    fn modulo_produz_owner_da_leitura_antes_de_limpar_global_e_ler_campo() {
        let mut m = origem();
        let mut ps = HashMap::from([("f".into(), PlanoFuncaoDart::default())]);
        preparar_arc_modulo_dart(&mut m, &mut ps).unwrap();
        assert_eq!(ps["f"].classes[&ValueId(2)], Ownership::Owned);
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_ler_campo_ref_v1")));
        verificar_contrato_funcao_dart(
            &m.functions[0],
            &ps["f"].classes,
            &ps["f"].tokens,
            &ps["f"].tabelas,
            &ps["f"].escopos,
        )
        .unwrap();
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        let retencao = ir.find("call void @dartforge_arc_retain(i64 %v2)").unwrap();
        let limpeza = ir[retencao..].find("store i64 0, ptr %ga3").unwrap();
        assert!(limpeza > 0);
    }

    #[test]
    fn escritas_invalidas_e_enderecos_expostos_recusam_sem_publicacao() {
        for caso in 0..6 {
            let mut m = origem();
            match caso {
                0 => {
                    if let Instruction::StoreGlobal { raiz, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *raiz = None;
                    }
                }
                1 => m.functions[0].params[0].2 = Type::I64,
                2 => m.globais.push((1, Type::Ref, "g".into())),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(5),
                    Instruction::Const(Constant::Funcao("g".into())),
                    Type::I64,
                )),
                4 => m.globais[0].1 = Type::I64,
                _ => {
                    if let Instruction::StoreGlobal { ty, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *ty = Type::I64;
                    }
                }
            }
            let antes = format!("{m:?}");
            assert!(preparar(&mut m).is_err(), "caso {caso}");
            assert_eq!(format!("{m:?}"), antes, "caso {caso}");
        }
    }

    #[test]
    fn tracing_preserva_operacoes_e_globais_escalares_nao_viram_slots_ref() {
        let mut m = origem();
        m.memoria_arc = false;
        let antes = format!("{m:?}");
        preparar(&mut m).unwrap();
        assert_eq!(format!("{m:?}"), antes);
        let mut m = Module::new();
        m.memoria_arc = true;
        m.globais.push((0, Type::I64, "contador".into()));
        let antes = format!("{m:?}");
        preparar(&mut m).unwrap();
        assert_eq!(format!("{m:?}"), antes);
    }

    #[test]
    fn areas_de_modulos_distintos_restauram_nomes_dos_slots_fortes() {
        let a = origem();
        let mut b = origem();
        b.functions[0].symbol = "segunda".into();
        let mut ms = [a, b];
        let mut ps = [
            HashMap::from([("f".into(), PlanoFuncaoDart::default())]),
            HashMap::from([("segunda".into(), PlanoFuncaoDart::default())]),
        ];
        preparar_arc_modulos_dart(&mut ms, &mut ps).unwrap();
        for (m, ps) in ms.iter().zip(&ps) {
            for f in &m.functions {
                for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                    if let Instruction::ArcLoadStrong {
                        slot: SlotForte::Global { simbolo },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: SlotForte::Global { simbolo },
                        ..
                    } = i
                    {
                        assert_eq!(simbolo, "g");
                    }
                }
                let p = &ps[&f.symbol];
                verificar_contrato_funcao_dart(f, &p.classes, &p.tokens, &p.tabelas, &p.escopos)
                    .unwrap();
            }
            crate::llvm::LlvmEmitter::new(m).emit_all();
        }
    }
}
