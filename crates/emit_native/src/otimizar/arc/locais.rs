//! Contratos de locais escalares privados da ativação, sem aliases ou escape.
//! Campos, locais Ref e endereços importados exigem outros produtores.

use super::{EfeitoTokens, Ownership, PlanoTokens};
use crate::hir::*;
use crate::otimizar::{
    cfg::Cfg,
    operandos::{operandos, operandos_do_terminador},
};
use std::collections::{HashMap, HashSet};

pub(super) fn produzir(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, Type>, String> {
    let (locais, produzidas) = analisar(f, classes, plano)?;
    for v in produzidas {
        classes.insert(v, Ownership::Trivial);
        plano.instrucoes.insert(v, EfeitoTokens::default());
    }
    Ok(locais)
}

/// Reconstrói o contrato no corpo atual sem completar metadados ausentes.
pub(super) fn verificar(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(), String> {
    let (locais, produzidas) = analisar(f, classes, plano)?;
    for v in produzidas {
        if classes.get(&v) != Some(&Ownership::Trivial)
            || plano.instrucoes.get(&v) != Some(&EfeitoTokens::default())
        {
            return Err(format!("v{}: local escalar sem contrato completo", v.0));
        }
    }
    verificar_valores(f, &locais, classes)
}

fn analisar(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(HashMap<ValueId, Type>, HashSet<ValueId>), String> {
    let mut locais = HashMap::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (v, inst, ty) in &b.instructions {
            if let Instruction::Alloca(t @ (Type::I1 | Type::I8 | Type::I64 | Type::F64)) = inst {
                if bi != 0 || *ty != Type::Ptr {
                    return Err(format!(
                        "v{}: local escalar exige alloca Ptr na entrada",
                        v.0
                    ));
                }
                locais.insert(*v, *t);
            }
        }
    }
    if locais.is_empty() {
        return Ok((locais, HashSet::new()));
    }
    super::ssa::verificar(f)?;
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
    let local = |op: &Operand| match op {
        Operand::Val(v) if locais.contains_key(v) => Some(*v),
        _ => None,
    };
    let mut produzidas = HashSet::new();
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let mut permitido = None;
            let mut esperado = None;
            match inst {
                Instruction::Alloca(_) if locais.contains_key(v) => esperado = Some(Type::Ptr),
                Instruction::Load { ptr, ty: lido } => {
                    if let Some(p) = local(ptr) {
                        permitido = Some(p);
                        if *lido != locais[&p] {
                            return Err(format!("v{}: leitura diverge do tipo do local", v.0));
                        }
                        esperado = Some(*lido);
                    }
                }
                Instruction::Store { ptr, val } => {
                    if let Some(p) = local(ptr) {
                        permitido = Some(p);
                        let tipo = match val {
                            Operand::Val(v) => tipos.get(v).copied(),
                            // Estado de late usa bytes literais (0/1/2). O
                            // tipo vem do slot, com faixa conferida antes da
                            // coerção textual; não se aplica a valores SSA.
                            Operand::Constant(Constant::Int(n))
                                if locais[&p] == Type::I8 && (0..=255).contains(n) =>
                            {
                                Some(Type::I8)
                            }
                            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                            _ => None,
                        };
                        if tipo != Some(locais[&p]) {
                            return Err(format!("v{}: gravação diverge do tipo do local", v.0));
                        }
                        esperado = Some(Type::Void);
                    }
                }
                _ => {}
            }
            // Apenas o operando ptr de Load/Store pode usar o endereço. Uma
            // segunda ocorrência (p.ex. Store como valor) também é escape.
            let mut usos = 0;
            let mut inesperado = false;
            operandos(inst, &mut |op| {
                if let Some(p) = local(op) {
                    usos += 1;
                    inesperado |= permitido != Some(p);
                }
            });
            if inesperado || usos != usize::from(permitido.is_some()) {
                return Err(format!(
                    "v{}: endereço de local escalar escapa ou tem alias",
                    v.0
                ));
            }
            if let Some(esperado) = esperado {
                if *ty != esperado
                    || classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plano
                        .instrucoes
                        .get(v)
                        .is_some_and(|e| *e != EfeitoTokens::default())
                {
                    return Err(format!("v{}: contrato incompatível com local escalar", v.0));
                }
                produzidas.insert(*v);
            }
        }
        let mut escapa = false;
        operandos_do_terminador(&b.terminator, &mut |op| {
            escapa |= local(op).is_some();
        });
        if escapa {
            return Err(format!(
                "b{}: endereço de local escalar escapa no terminador",
                b.id.0
            ));
        }
    }
    let cfg = Cfg::novo(f);
    let universo: HashSet<_> = locais.keys().copied().collect();
    // Análise must: a interseção preserva apenas inicializações presentes
    // em todos os predecessores. O topo nos laços decresce até o ponto fixo.
    let mut entradas = vec![universo.clone(); f.blocks.len()];
    let mut saidas = entradas.clone();
    let transferir = |bi: usize, mut estado: HashSet<ValueId>| {
        for (v, inst, _) in &f.blocks[bi].instructions {
            match inst {
                Instruction::Alloca(_) if locais.contains_key(v) => {
                    estado.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        estado.insert(p);
                    }
                }
                _ => {}
            }
        }
        estado
    };
    loop {
        let mut mudou = false;
        for &bi in &cfg.rpo {
            let mut entrada = if bi == 0 {
                HashSet::new()
            } else {
                universo.clone()
            };
            if bi != 0 {
                for &p in &cfg.predecessores[bi] {
                    if cfg.alcancavel(p) {
                        entrada.retain(|v| saidas[p].contains(v));
                    }
                }
            }
            let saida = transferir(bi, entrada.clone());
            mudou |= entradas[bi] != entrada || saidas[bi] != saida;
            entradas[bi] = entrada;
            saidas[bi] = saida;
        }
        if !mudou {
            break;
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut estado = if cfg.alcancavel(bi) {
            entradas[bi].clone()
        } else {
            HashSet::new()
        };
        for (v, inst, _) in &b.instructions {
            match inst {
                Instruction::Alloca(_) if locais.contains_key(v) => {
                    estado.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        estado.insert(p);
                    }
                }
                Instruction::Load { ptr, .. } => {
                    if let Some(p) = local(ptr)
                        && !estado.contains(&p)
                    {
                        return Err(format!(
                            "v{}: local escalar não inicializado em todos os caminhos",
                            v.0
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Ok((locais, produzidas))
}

pub(super) fn verificar_valores(
    f: &Function,
    locais: &HashMap<ValueId, Type>,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Store {
            ptr: Operand::Val(p),
            val: Operand::Val(origem),
        } = inst
            && locais.contains_key(p)
            && classes.get(origem) != Some(&Ownership::Trivial)
        {
            return Err(format!(
                "v{}: gravação de local escalar exige origem Trivial",
                v.0
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::otimizar::arc::{
        OrigemOwner, PlanoEscopos, produzir_contratos_arc, produzir_e_verificar_tokens,
    };

    fn diamante() -> Function {
        Function {
            symbol: "local".into(),
            name: "local".into(),
            depuracao: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(4),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(5),
                            Instruction::IntToDouble(Operand::Val(ValueId(4))),
                            Type::F64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(3),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(9)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(7)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
            ],
        }
    }

    #[test]
    fn local_escalar_exige_inicializacao_em_todas_as_arestas() {
        let mut f = diamante();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        f.blocks[2].instructions.clear();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let antes = classes.clone();
        let mut plano = PlanoTokens::default();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("não inicializado")
        );
        assert_eq!(classes, antes);
        assert!(plano.instrucoes.is_empty());
    }

    #[test]
    fn local_no_laco_exige_gravacao_antes_da_primeira_volta() {
        let mut f = Function {
            symbol: "laco".into(),
            name: "laco".into(),
            depuracao: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(1)),
                                val: Operand::Constant(Constant::Int(9)),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("não inicializado")
        );
        assert_eq!(classes.len(), 1);
        assert!(plano.instrucoes.is_empty());
        f.blocks[0].instructions.push((
            ValueId(4),
            Instruction::Store {
                ptr: Operand::Val(ValueId(1)),
                val: Operand::Constant(Constant::Int(7)),
            },
            Type::Void,
        ));
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
    }

    #[test]
    fn locais_tipados_alimentam_produtores_sem_inferir_por_largura() {
        for (ty, valor) in [
            (Type::I64, Instruction::Const(Constant::Int(7))),
            (Type::F64, Instruction::Const(Constant::Double(-0.0))),
            (Type::I1, Instruction::Const(Constant::Bool(true))),
            (
                Type::I8,
                Instruction::ZExt {
                    op: Operand::Constant(Constant::Bool(true)),
                    from: Type::I1,
                    to: Type::I8,
                },
            ),
        ] {
            let f = Function {
                symbol: "tipo_local".into(),
                name: "tipo_local".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(ty), Type::Ptr),
                        (ValueId(1), valor, ty),
                        (
                            ValueId(2),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Val(ValueId(1)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty,
                            },
                            ty,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plano = PlanoTokens::default();
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(classes.len(), 4);
            assert!(classes.values().all(|c| *c == Ownership::Trivial));
            let mut modulo = Module::default();
            modulo.memoria_arc = true;
            modulo.functions.push(f);
            let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
            assert!(ir.contains(&format!("load {}, ptr %v0", ty.llvm_ir())));
        }
    }

    #[test]
    fn modulo_liga_leitura_escalar_a_boxing_owned_sem_mudar_tracing() {
        use crate::otimizar::arc::{PlanoFuncaoDart, preparar_arc_modulo_dart};
        for arc in [false, true] {
            let mut f = diamante();
            f.blocks[1].instructions.push((
                ValueId(6),
                Instruction::Box {
                    op: Operand::Val(ValueId(4)),
                    from: Type::I64,
                },
                Type::Ref,
            ));
            let mut modulo = Module::default();
            modulo.memoria_arc = arc;
            modulo.functions.push(f);
            let mut planos = HashMap::from([("local".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{modulo:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut modulo, &mut planos).unwrap(),
                if arc { (0, 1) } else { (0, 0) }
            );
            if arc {
                assert_eq!(planos["local"].classes[&ValueId(4)], Ownership::Trivial);
                assert_eq!(planos["local"].classes[&ValueId(6)], Ownership::Owned);
                assert!(matches!(
                    modulo.functions[0].blocks[1].instructions.last().unwrap().1,
                    Instruction::ArcDrop { .. }
                ));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut modulo, &mut planos).unwrap(),
                    (0, 0)
                );
                modulo.functions[0].blocks[2].instructions.clear();
                let antes = format!("{modulo:?}/{planos:?}");
                assert!(preparar_arc_modulo_dart(&mut modulo, &mut planos).is_err());
                assert_eq!(format!("{modulo:?}/{planos:?}"), antes);
            } else {
                assert_eq!(format!("{modulo:?}/{planos:?}"), antes);
            }
        }
    }

    #[test]
    fn verificador_recusa_plano_local_obsoleto_apos_mudar_o_corpo() {
        use crate::otimizar::arc::verificar_tokens;
        let base = diamante();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plano = PlanoTokens::default();
        produzir_contratos_arc(&base, &mut classes, &mut plano).unwrap();
        verificar_tokens(&base, &classes, &TabelasDaFuncao::default(), &plano).unwrap();
        for caso in 0..5 {
            let mut f = base.clone();
            let mut classes = classes.clone();
            let mut plano = plano.clone();
            match caso {
                0 => {
                    f.blocks[2].instructions.clear();
                    classes.remove(&ValueId(3));
                    plano.instrucoes.remove(&ValueId(3));
                }
                1 => {
                    f.blocks[1].instructions.push((
                        ValueId(6),
                        Instruction::CallRuntime {
                            name: "extern_sem_ownership".into(),
                            args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                    classes.insert(ValueId(6), Ownership::Trivial);
                    plano.instrucoes.insert(ValueId(6), EfeitoTokens::default());
                }
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::F64,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => {
                    plano.instrucoes.get_mut(&ValueId(4)).unwrap().pode_falhar = true;
                    plano.pendencias.insert(ValueId(4), BlockId(2));
                }
                _ => unreachable!(),
            }
            let antes = format!("{classes:?}/{plano:?}");
            assert!(
                verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano).is_err(),
                "caso {caso}"
            );
            assert_eq!(format!("{classes:?}/{plano:?}"), antes);
        }
    }

    #[test]
    fn byte_literal_tem_faixa_explicita_no_slot() {
        for n in [0, 1, 2, 255, -1, 256] {
            let f = Function {
                symbol: "byte".into(),
                name: "byte".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(Type::I8), Type::Ptr),
                        (
                            ValueId(1),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Constant(Constant::Int(n)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty: Type::I8,
                            },
                            Type::I8,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plano = PlanoTokens::default();
            let resultado = produzir_contratos_arc(&f, &mut classes, &mut plano);
            assert_eq!(resultado.is_ok(), (0..=255).contains(&n));
            if resultado.is_err() {
                assert!(classes.is_empty());
                assert!(plano.instrucoes.is_empty());
            }
        }
    }

    #[test]
    fn local_escalar_recusa_escape_e_gravacao_gerenciada_atomicamente() {
        let base = diamante();
        for caso in 0..6 {
            let mut f = base.clone();
            match caso {
                0 => f.blocks[1].instructions.push((
                    ValueId(6),
                    Instruction::CallRuntime {
                        name: "sem_contrato".into(),
                        args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                1 => f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1)))),
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::Ref,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => f.blocks[0].instructions[0].2 = Type::I64,
                5 => {
                    let a = f.blocks[0].instructions.remove(0);
                    f.blocks[3].instructions.insert(0, a);
                }
                _ => unreachable!(),
            }
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano).is_err(),
                "caso {caso}"
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
        let mut f = base;
        f.params.push((ValueId(6), "valor".into(), Type::I64));
        f.blocks[2].instructions[0].1 = Instruction::Store {
            ptr: Operand::Val(ValueId(1)),
            val: Operand::Val(ValueId(6)),
        };
        for classe in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            }),
        ] {
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            if let Some(c) = classe {
                classes.insert(ValueId(6), c);
            }
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano)
                    .unwrap_err()
                    .contains("origem Trivial")
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
    }
}
