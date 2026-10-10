//! Prova AOT de retorno Guarda, cleanup e unwind entre duas funções Dart.
//! O argumento final `automatico` acrescenta um propagador cujo invoke,
//! pouso, fechamento léxico e cleanup são gerados pela preparação ARC.
//! O argumento adicional `misto` dá ao caller um catch preparado e outro
//! invoke automático, exercitando o seletor catch no perfil de cleanup Unix.
//! Não observa morte final nem certifica o restante do pipeline ARC.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = args.next().as_deref() != Some("tracing");
    let lancar = args.next().as_deref() == Some("erro");
    let automatico = args.next().as_deref() == Some("automatico");
    let misto = args.next().as_deref() == Some("misto");
    if misto && !automatico {
        return Err("misto exige preparação automática".into());
    }
    let val = |v| Operand::Val(ValueId(v));
    let runtime = |name: &str, args, ret_ty| Instruction::CallRuntime {
        name: name.into(),
        args,
        ret_ty,
    };
    let mut modulo = Module::new();
    modulo.memoria_arc = arc;
    modulo.excecoes_por_tabelas = true;
    modulo.entry_symbol = Some("prova_guardas".into());
    modulo.functions.push(Function {
        symbol: "prova_guardas".into(),
        name: "prova_guardas".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        runtime(
                            "dartforge_arc_box_int_owned_v1",
                            vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                            Type::Ref,
                        ),
                        Type::Ref,
                    ),
                    (
                        ValueId(22),
                        runtime("dartforge_arc_rastro_owned_v1", vec![], Type::Ref),
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: if automatico {
                                "propagador"
                            } else {
                                "retorno_guardado"
                            }
                            .into(),
                            args: vec![val(0), val(22)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(2),
                    runtime("dartforge_exception_clear", vec![], Type::Void),
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::ArcDrop { value: val(1) },
                        Type::Void,
                    ),
                    (
                        ValueId(8),
                        runtime("dartforge_arc_collect", vec![], Type::Void),
                        Type::Void,
                    ),
                    (
                        ValueId(4),
                        runtime(
                            "dartforge_print_handle",
                            vec![(val(0), Type::Ref)],
                            Type::Void,
                        ),
                        Type::Void,
                    ),
                    (
                        ValueId(5),
                        runtime("dartforge_exception_pending", vec![], Type::I8),
                        Type::I8,
                    ),
                    (
                        ValueId(6),
                        Instruction::ICmp(ICmpOp::Ne, val(5), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(6),
                    then_block: BlockId(3),
                    else_block: BlockId(4),
                },
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![(
                    ValueId(7),
                    runtime("dartforge_exception_clear", vec![], Type::Void),
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    });
    modulo.functions.push(Function {
        symbol: "retorno_guardado".into(),
        name: "retorno_guardado".into(),
        depuracao: None,
        params: vec![
            (ValueId(0), "x".into(), Type::Ref),
            (ValueId(2), "rastro".into(), Type::Ref),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: if lancar {
                vec![(
                    ValueId(1),
                    runtime(
                        "dartforge_arc_lancar_com_rastro_ref_v1",
                        vec![(val(0), Type::Ref), (val(2), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                )]
            } else {
                vec![]
            },
            terminator: Terminator::Return(Some(val(0))),
        }],
    });
    let mut caller = PlanoFuncaoDart::default();
    caller.tabelas.invocacoes.insert(ValueId(1), BlockId(1));
    caller.tabelas.pousos.insert(BlockId(1));
    let mut callee = PlanoFuncaoDart::default();
    callee.tokens.retorno = RetornoTokens::Owned;
    callee
        .tabelas
        .saidas
        .insert(BlockId(0), SaidaPorExcecao::Guarda);
    let mut planos = HashMap::from([
        ("prova_guardas".into(), caller),
        ("retorno_guardado".into(), callee),
    ]);
    if automatico {
        modulo.functions.push(Function {
            symbol: "propagador".into(),
            name: "propagador".into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "x".into(), Type::Ref),
                (ValueId(3), "rastro".into(), Type::Ref),
            ],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy { value: val(0) },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "retorno_guardado".into(),
                            args: vec![val(1), val(3)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(val(2))),
            }],
        });
        let mut propagador = PlanoFuncaoDart::default();
        propagador.tokens.retorno = RetornoTokens::Owned;
        propagador
            .escopos
            .antes
            .insert(ValueId(2), vec![AlteracaoEscopo::Abrir(7)]);
        propagador
            .escopos
            .saidas
            .insert(BlockId(0), vec![AlteracaoEscopo::Fechar(7)]);
        planos.insert("propagador".into(), propagador);
    }
    if misto {
        // O catch inicial permanece explícito. O segundo invoke, sem catch,
        // faz a mesma função receber também Retoma no protótipo Unix.
        modulo.functions[0]
            .blocks
            .iter_mut()
            .find(|b| b.id == BlockId(4))
            .unwrap()
            .instructions
            .push((
                ValueId(9),
                Instruction::CallStatic {
                    symbol: "propagador".into(),
                    args: vec![val(0), val(22)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
    }
    // Verifica identidade antes do clear; depois libera o owner original,
    // coleta e lê a cópia Owned, sem depender da raiz de pendência.
    let catch = modulo.functions[0]
        .blocks
        .iter_mut()
        .find(|b| b.id == BlockId(1))
        .unwrap();
    catch.instructions = vec![
        (
            ValueId(10),
            runtime("dartforge_arc_excecao_owned_v1", vec![], Type::Ref),
            Type::Ref,
        ),
        (
            ValueId(11),
            Instruction::ICmp(ICmpOp::Ne, val(10), val(0)),
            Type::I1,
        ),
        (
            ValueId(23),
            runtime("dartforge_arc_rastro_owned_v1", vec![], Type::Ref),
            Type::Ref,
        ),
        (
            ValueId(24),
            Instruction::ICmp(ICmpOp::Ne, val(23), val(22)),
            Type::I1,
        ),
        (
            ValueId(2),
            runtime("dartforge_exception_clear", vec![], Type::Void),
            Type::Void,
        ),
        (
            ValueId(16),
            Instruction::ArcDrop { value: val(0) },
            Type::Void,
        ),
        (
            ValueId(27),
            Instruction::ArcDrop { value: val(22) },
            Type::Void,
        ),
        (
            ValueId(17),
            runtime("dartforge_arc_collect", vec![], Type::Void),
            Type::Void,
        ),
    ];
    // Retain após a coleta confere que a captura do rastro continua viva.
    catch.instructions.push((
        ValueId(26),
        Instruction::ArcCopy { value: val(23) },
        Type::Ref,
    ));
    catch.terminator = Terminator::CondBranch {
        cond: val(11),
        then_block: BlockId(6),
        else_block: BlockId(12),
    };
    modulo.functions[0].blocks.extend([
        BasicBlock {
            id: BlockId(12),
            instructions: vec![],
            terminator: Terminator::CondBranch {
                cond: val(24),
                then_block: BlockId(6),
                else_block: BlockId(7),
            },
        },
        BasicBlock {
            id: BlockId(6),
            instructions: vec![
                // Troca de identidade produz null, distinto do MAX esperado.
                (
                    ValueId(12),
                    runtime(
                        "dartforge_print_handle",
                        vec![(Operand::Constant(Constant::Null), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(13),
                    runtime("dartforge_exception_pending", vec![], Type::I8),
                    Type::I8,
                ),
                (
                    ValueId(14),
                    Instruction::ICmp(ICmpOp::Ne, val(13), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                ),
            ],
            terminator: Terminator::CondBranch {
                cond: val(14),
                then_block: BlockId(8),
                else_block: BlockId(9),
            },
        },
        BasicBlock {
            id: BlockId(7),
            instructions: vec![
                (
                    ValueId(18),
                    runtime(
                        "dartforge_print_handle",
                        vec![(val(10), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(19),
                    runtime("dartforge_exception_pending", vec![], Type::I8),
                    Type::I8,
                ),
                (
                    ValueId(20),
                    Instruction::ICmp(ICmpOp::Ne, val(19), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                ),
            ],
            terminator: Terminator::CondBranch {
                cond: val(20),
                then_block: BlockId(10),
                else_block: BlockId(11),
            },
        },
        BasicBlock {
            id: BlockId(8),
            instructions: vec![(
                ValueId(15),
                runtime("dartforge_exception_clear", vec![], Type::Void),
                Type::Void,
            )],
            terminator: Terminator::Return(None),
        },
        BasicBlock {
            id: BlockId(9),
            instructions: vec![],
            terminator: Terminator::Return(None),
        },
        BasicBlock {
            id: BlockId(10),
            instructions: vec![(
                ValueId(21),
                runtime("dartforge_exception_clear", vec![], Type::Void),
                Type::Void,
            )],
            terminator: Terminator::Return(None),
        },
        BasicBlock {
            id: BlockId(11),
            instructions: vec![],
            terminator: Terminator::Return(None),
        },
    ]);
    let inseridos = if automatico {
        preparar_arc_funcoes_dart(&mut modulo.functions, &mut planos)?
    } else {
        inserir_arc_funcoes_dart(&mut modulo.functions, &mut planos)?
    };
    let esperado = if misto {
        (1, 21)
    } else if automatico {
        (1, 18)
    } else {
        (1, 16)
    };
    if inseridos != esperado {
        return Err(format!(
            "inserção esperada {esperado:?}, encontrada {inseridos:?}"
        ));
    }
    modulo.tabelas = modulo
        .functions
        .iter()
        .map(|f| planos[&f.symbol].tabelas.clone())
        .collect();
    let erros = dartforge_emit_native::lower::verificador::verificar(&modulo);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&modulo).emit_all();
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
