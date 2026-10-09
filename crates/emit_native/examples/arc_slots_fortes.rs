//! Prova AOT dirigida das operações fortes com planos produzidos/verificados.
//! Variantes usam literal permanente ou Mint mortal, além de Smi e null.
//! Não certifica morte após o último owner nem o restante do pipeline ARC.

use dartforge_emit_native::otimizar::arc::*;
use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = args.next().as_deref() != Some("tracing");
    let variante = args.next();
    let sem_cleanup = matches!(
        variante.as_deref(),
        Some("sem-cleanup" | "retorno-sem-cleanup" | "retorno-mortal-sem-cleanup")
    );
    let prova_retorno = matches!(
        variante.as_deref(),
        Some("retorno" | "retorno-sem-cleanup" | "retorno-mortal" | "retorno-mortal-sem-cleanup")
    );
    let prova_mortal = matches!(
        variante.as_deref(),
        Some("retorno-mortal" | "retorno-mortal-sem-cleanup")
    );
    let mut m = Module::new();
    m.memoria_arc = arc;
    m.entry_symbol = Some("prova_slots".into());
    m.globais.push((0, Type::Ref, "dfg.prova".into()));
    let quadro = SlotForte::Quadro {
        quadro: Operand::Val(ValueId(0)),
        indice: 0,
    };
    let global = SlotForte::Global {
        simbolo: "dfg.prova".into(),
    };
    let valor = |v| Operand::Val(ValueId(v));
    let chamada = |nome: &str, args, ret_ty| Instruction::CallRuntime {
        name: nome.into(),
        args,
        ret_ty,
    };
    m.functions.push(Function {
        symbol: "prova_slots".into(),
        name: "prova_slots".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    chamada(
                        "dartforge_arc_quadro_abrir_v1",
                        vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                        Type::I64,
                    ),
                    Type::I64,
                ),
                (
                    ValueId(1),
                    Instruction::Const(Constant::String("slots ARC".into())),
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ArcCopy { value: valor(1) },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcStoreStrong {
                        slot: quadro.clone(),
                        value: valor(2),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::ArcLoadStrong { slot: quadro },
                    Type::Ref,
                ),
                (
                    ValueId(5),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: valor(4),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(6),
                    chamada(
                        "dartforge_arc_quadro_fechar_v1",
                        vec![(valor(0), Type::I64)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(7),
                    chamada("dartforge_arc_collect", vec![], Type::Void),
                    Type::Void,
                ),
                (
                    ValueId(8),
                    Instruction::ArcLoadStrong {
                        slot: global.clone(),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(9),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(8), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(10),
                    Instruction::ArcDrop { value: valor(8) },
                    Type::Void,
                ),
                (
                    ValueId(11),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(42)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(12),
                    Instruction::ArcCopy { value: valor(11) },
                    Type::Ref,
                ),
                (
                    ValueId(13),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: valor(12),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(14),
                    Instruction::ArcLoadStrong {
                        slot: global.clone(),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(15),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(14), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(16),
                    Instruction::ArcDrop { value: valor(14) },
                    Type::Void,
                ),
                (
                    ValueId(17),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: Operand::Constant(Constant::Null),
                        modo: ModoStoreForte::Copy,
                    },
                    Type::Void,
                ),
                (
                    ValueId(18),
                    Instruction::ArcLoadStrong { slot: global },
                    Type::Ref,
                ),
                (
                    ValueId(19),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(18), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(20),
                    Instruction::ArcDrop { value: valor(18) },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    dartforge_emit_native::otimizar::otimizar(&mut m);
    if prova_retorno {
        let mut identidade = Function {
            symbol: "prova_retorno".into(),
            name: "prova_retorno".into(),
            depuracao: None,
            params: vec![(ValueId(0), "valor".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(valor(0))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plano = PlanoTokens {
            retorno: RetornoTokens::Owned,
            ..Default::default()
        };
        let copias = inserir_retencao_retornos_dart(
            &mut identidade,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )?;
        if copias != 1 {
            return Err("prova de retorno exige uma retenção inserida".into());
        }
        // A função é acrescentada depois da otimização geral, para conferir
        // a chamada/retorno real sem inlining esconder essa fronteira.
        m.functions.push(identidade);
        for (v, inst, _) in &mut m.functions[0].blocks[0].instructions {
            if *v == ValueId(2) {
                *inst = Instruction::CallStatic {
                    symbol: "prova_retorno".into(),
                    args: vec![valor(1)],
                    ret_ty: Type::Ref,
                };
            }
        }
    }
    if prova_mortal {
        let corpo = &mut m.functions[0].blocks[0].instructions;
        for (v, inst, _) in corpo.iter_mut() {
            if *v == ValueId(1) {
                *inst = chamada(
                    "dartforge_arc_box_int_owned_v1",
                    vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                    Type::Ref,
                );
            }
        }
        let depois_do_store = corpo
            .iter()
            .position(|(v, _, _)| *v == ValueId(3))
            .ok_or("store ausente na prova mortal")?
            + 1;
        // O slot conserva o token devolvido pelo callee; solta a ocorrência
        // inicial da fábrica antes de fechar o quadro e coletar.
        corpo.insert(
            depois_do_store,
            (
                ValueId(100),
                Instruction::ArcDrop { value: valor(1) },
                Type::Void,
            ),
        );
    }
    let f = &mut m.functions[0];
    let originais = std::mem::take(&mut f.blocks[0].instructions);
    f.blocks.clear();
    let mut corrente = BlockId(0);
    let mut corpo = Vec::new();
    let mut proximo = originais.iter().map(|(v, _, _)| v.0).max().unwrap_or(0) + 1;
    for instrucao in originais {
        let impresso = match &instrucao.1 {
            Instruction::CallRuntime { name, args, .. } if name == "dartforge_print_handle" => {
                Some(args[0].0.clone())
            }
            _ => None,
        };
        corpo.push(instrucao);
        if let Some(impresso) = impresso {
            let p = ValueId(proximo);
            let c = ValueId(proximo + 1);
            proximo += 2;
            corpo.push((
                p,
                chamada("dartforge_exception_pending", vec![], Type::I8),
                Type::I8,
            ));
            corpo.push((
                c,
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(p),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ));
            let erro = BlockId(corrente.0 + 1);
            let sucesso = BlockId(corrente.0 + 2);
            f.blocks.push(BasicBlock {
                id: corrente,
                instructions: std::mem::take(&mut corpo),
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(c),
                    then_block: erro,
                    else_block: sucesso,
                },
            });
            let mut cleanup = Vec::new();
            if !sem_cleanup {
                cleanup.push((
                    ValueId(proximo),
                    Instruction::ArcDrop { value: impresso },
                    Type::Void,
                ));
                proximo += 1;
            }
            cleanup.push((
                ValueId(proximo),
                Instruction::ArcStoreStrong {
                    slot: SlotForte::Global {
                        simbolo: "dfg.prova".into(),
                    },
                    value: Operand::Constant(Constant::Null),
                    modo: ModoStoreForte::Copy,
                },
                Type::Void,
            ));
            proximo += 1;
            f.blocks.push(BasicBlock {
                id: erro,
                instructions: cleanup,
                terminator: Terminator::Return(None),
            });
            corrente = sucesso;
        }
    }
    f.blocks.push(BasicBlock {
        id: corrente,
        instructions: corpo,
        terminator: Terminator::Return(None),
    });
    // A caixa de 42 é Smi imediato; o literal permanente é classificado
    // pelo produtor de constantes, sem contrato manual desta prova.
    let mut classes = HashMap::from([(ValueId(11), Ownership::Trivial)]);
    let mut plano = PlanoTokens::default();
    if prova_retorno {
        // Contrato do callee preparado/verificado acima: receptor emprestado,
        // resultado Owned normal. Não presume contratos de outras chamadas.
        classes.insert(ValueId(2), Ownership::Owned);
        plano.instrucoes.insert(ValueId(2), EfeitoTokens::default());
    }
    plano
        .instrucoes
        .insert(ValueId(11), EfeitoTokens::default());
    produzir_e_verificar_tokens(
        f,
        &mut classes,
        &mut plano,
        &TabelasDaFuncao::default(),
        &PlanoEscopos::default(),
    )?;
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // Auxiliar de observação do harness, exportado pelo runtime mas fora
    // do catálogo normal usado pelo lowering do SDK.
    ir.push_str("\ndeclare void @dartforge_print_handle(i64)\n");
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
