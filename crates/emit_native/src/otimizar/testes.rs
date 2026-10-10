use super::*;

#[test]
fn pipeline_tracing_nao_prepara_boxing_owned() {
    for por_tabelas in [false, true] {
        let mut m = Module::new();
        m.functions.push(Function {
            symbol: "caixa_tracing".into(), name: "caixa_tracing".into(),
            depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(0), Instruction::Box { op: Operand::Constant(Constant::Double(-0.0)), from: Type::F64 }, Type::Ref)
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
        });
        preparar_para_emissao(&mut m, false, por_tabelas);
        assert!(!m.memoria_arc);
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::Box { from: Type::F64, .. })));
        for (_, i, _) in m.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
            assert!(!matches!(i, Instruction::ArcCopy { .. } | Instruction::ArcMove { .. } | Instruction::ArcDrop { .. }));
            assert!(!matches!(i, Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_")));
        }
    }
}

fn c(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

#[test]
fn campos_tipados_reinicializam_no_ponto_da_alocacao_e_recusam_bits_mistos() {
    for (ty, zero, valor) in [
        (Type::F64, Constant::Double(0.0), Constant::Double(-0.0)),
        (Type::I1, Constant::Bool(false), Constant::Bool(true)),
        (Type::I8, Constant::Int(0), Constant::Int(255)),
    ] {
        let mut f = Function {
            symbol: "local_tipado".into(), name: "local_tipado".into(),
            depuracao: None, params: vec![], return_ty: ty,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)) },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(0), Instruction::CallRuntime {
                        name: "dartforge_object_new".into(),
                        args: vec![(c(128), Type::I64), (c(1), Type::I64)],
                        ret_ty: Type::Ref,
                    }, Type::Ref),
                    (ValueId(3), if ty == Type::I8 {
                        Instruction::Trunc { op: Operand::Constant(valor), from: Type::I64, to: Type::I8 }
                    } else { Instruction::Const(valor) }, ty),
                    (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, ty),
                    (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                        value: Operand::Val(ValueId(3)) }, Type::Void),
                ], terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(1), else_block: BlockId(2),
                } },
                BasicBlock { id: BlockId(2), instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) },
            ],
        };
        let mut misto = f.clone();
        misto.blocks[1].instructions[3].1 = Instruction::CallRuntime {
            name: "dartforge_object_set".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref), (c(0), Type::I64),
                (c(0), Type::I64), (c(0), Type::I8)], ret_ty: Type::Void,
        };
        let antes = format!("{misto:?}");
        assert!(!escape::substituir_objetos(&mut misto));
        assert_eq!(format!("{misto:?}"), antes);
        assert!(escape::substituir_objetos(&mut f));
        assert!(matches!(f.blocks[0].instructions.as_slice(),
            [(_, Instruction::Alloca(t), Type::Ptr)] if *t == ty));
        assert!(matches!(&f.blocks[1].instructions[0].1,
            Instruction::Store { val: Operand::Constant(z), .. } if *z == zero));
        assert!(matches!(&f.blocks[1].instructions[2].1,
            Instruction::Load { ty: t, .. } if *t == ty));
        assert!(!escape::substituir_objetos(&mut f));
    }
}

/// `p = Ponto(a, b); return p.x + p.y` na forma que o lowering produz: o
/// objeto some, e sobra a soma dos argumentos.
#[test]
fn objeto_temporario_some() {
    let v = ValueId;
    let novo = Function {
        symbol: "Ponto.new".into(),
        name: "Ponto".into(),
        depuracao: None,
        params: vec![(v(0), "this".into(), Type::Ref), (v(1), "x".into(), Type::F64), (v(2), "y".into(), Type::F64)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v(3), Instruction::Bitcast { op: Operand::Val(v(1)), to: Type::I64 }, Type::I64),
                (
                    v(4),
                    Instruction::CallRuntime {
                        name: "dartforge_object_set".into(),
                        args: vec![(Operand::Val(v(0)), Type::Ref), (c(0), Type::I64), (Operand::Val(v(3)), Type::I64), (c(0), Type::I8)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (v(5), Instruction::Bitcast { op: Operand::Val(v(2)), to: Type::I64 }, Type::I64),
                (
                    v(6),
                    Instruction::CallRuntime {
                        name: "dartforge_object_set".into(),
                        args: vec![(Operand::Val(v(0)), Type::Ref), (c(1), Type::I64), (Operand::Val(v(5)), Type::I64), (c(0), Type::I8)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    let get = |r: u32, i: i64| {
        (
            v(r),
            Instruction::CallRuntime {
                name: "dartforge_object_get".into(),
                args: vec![(Operand::Val(v(12)), Type::Ref), (c(i), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    };
    let soma = Function {
        symbol: "soma".into(),
        name: "soma".into(),
        depuracao: None,
        params: vec![(v(0), "a".into(), Type::F64), (v(1), "b".into(), Type::F64)],
        return_ty: Type::F64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (v(2), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (
                        v(3),
                        Instruction::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![(c(921), Type::I64), (c(2), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        v(4),
                        Instruction::CallStatic { symbol: "Ponto.new".into(), args: vec![Operand::Val(v(3)), Operand::Val(v(0)), Operand::Val(v(1))], ret_ty: Type::Void },
                        Type::Void,
                    ),
                    (v(5), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (v(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(v(5)), c(0)), Type::I1),
                ],
                terminator: Terminator::CondBranch { cond: Operand::Val(v(6)), then_block: BlockId(2), else_block: BlockId(1) },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (v(11), Instruction::Store { ptr: Operand::Val(v(2)), val: Operand::Val(v(3)) }, Type::Void),
                    (v(12), Instruction::Load { ptr: Operand::Val(v(2)), ty: Type::Ref }, Type::Ref),
                    get(13, 0),
                    (v(14), Instruction::Bitcast { op: Operand::Val(v(13)), to: Type::F64 }, Type::F64),
                    get(15, 1),
                    (v(16), Instruction::Bitcast { op: Operand::Val(v(15)), to: Type::F64 }, Type::F64),
                    (v(17), Instruction::FAdd(Operand::Val(v(14)), Operand::Val(v(16))), Type::F64),
                ],
                terminator: Terminator::Return(Some(Operand::Val(v(17)))),
            },
            BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Double(0.0)))) },
        ],
    };
    let mut m = Module { functions: vec![novo, soma], ..Default::default() };
    otimizar(&mut m);
    let f = &m.functions[1];
    let texto = format!("{:?}", f.blocks);
    assert!(!texto.contains("object_new") && !texto.contains("object_get") && !texto.contains("Ponto.new"), "{texto}");
    assert!(!texto.contains("exception_pending") && !texto.contains("Alloca"), "{texto}");
    assert_eq!(f.blocks.len(), 1, "{texto}");
    assert!(texto.contains("FAdd"), "{texto}");
}

/// Um objeto que escapa (retornado) continua alocado.
#[test]
fn objeto_que_escapa_fica() {
    let v = ValueId;
    let f = Function {
        symbol: "f".into(),
        name: "f".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                v(0),
                Instruction::CallRuntime { name: "dartforge_object_new".into(), args: vec![(c(921), Type::I64), (c(1), Type::I64)], ret_ty: Type::Ref },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(v(0)))),
        }],
    };
    let mut m = Module { functions: vec![f], ..Default::default() };
    otimizar(&mut m);
    assert!(format!("{:?}", m.functions[0].blocks).contains("object_new"));
}

/// Um local gravado nos dois lados de um `if` vira um `phi`.
#[test]
fn local_vira_phi() {
    let v = ValueId;
    let f = Function {
        symbol: "g".into(),
        name: "g".into(),
        depuracao: None,
        params: vec![(v(0), "c".into(), Type::I1)],
        return_ty: Type::I64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(v(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                terminator: Terminator::CondBranch { cond: Operand::Val(v(0)), then_block: BlockId(1), else_block: BlockId(2) },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(v(2), Instruction::Store { ptr: Operand::Val(v(1)), val: c(1) }, Type::Void)],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(v(3), Instruction::Store { ptr: Operand::Val(v(1)), val: c(2) }, Type::Void)],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![(v(4), Instruction::Load { ptr: Operand::Val(v(1)), ty: Type::I64 }, Type::I64)],
                terminator: Terminator::Return(Some(Operand::Val(v(4)))),
            },
        ],
    };
    let mut m = Module { functions: vec![f], ..Default::default() };
    otimizar(&mut m);
    let texto = format!("{:?}", m.functions[0].blocks);
    assert!(texto.contains("Phi") && !texto.contains("Alloca") && !texto.contains("Load"), "{texto}");
}
