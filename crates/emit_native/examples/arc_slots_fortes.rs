//! Directed AOT proof of the strong operations with produced/verified plans.
//! Variants use a permanent literal or a mortal Mint, besides Smi and null.
//! Does not certify death after the last owner nor the rest of the ARC pipeline.

use dartforge_emit_native::optimize::arc::*;
use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = args.next().as_deref() != Some("tracing");
    let variant = args.next();
    let forced_error = match variant.as_deref() {
        Some("retorno-mortal-erro-1") => Some(ValueId(9)),
        Some("retorno-mortal-erro-2") => Some(ValueId(15)),
        Some("retorno-mortal-erro-3") => Some(ValueId(19)),
        _ => None,
    };
    let auto_cleanup =
        variant.as_deref() == Some("retorno-mortal-auto-cleanup") || forced_error.is_some();
    let without_cleanup = matches!(
        variant.as_deref(),
        Some(
            "sem-cleanup"
                | "retorno-sem-cleanup"
                | "retorno-mortal-sem-cleanup"
                | "retorno-mortal-auto-cleanup"
        )
    ) || forced_error.is_some();
    let return_proof = matches!(
        variant.as_deref(),
        Some(
            "retorno"
                | "retorno-sem-cleanup"
                | "retorno-mortal"
                | "retorno-mortal-sem-cleanup"
                | "retorno-mortal-auto-cleanup"
        )
    ) || forced_error.is_some();
    let mortal_proof = matches!(
        variant.as_deref(),
        Some("retorno-mortal" | "retorno-mortal-sem-cleanup" | "retorno-mortal-auto-cleanup")
    ) || forced_error.is_some();
    let mut m = Module::new();
    m.arc_memory = arc;
    m.entry_symbol = Some("prova_slots".into());
    m.globals.push((0, Type::Ref, "dfg.prova".into()));
    let frame = StrongSlot::Quadro {
        frame: Operand::Val(ValueId(0)),
        index: 0,
    };
    let global = StrongSlot::Global {
        symbol: "dfg.prova".into(),
    };
    let value = |v| Operand::Val(ValueId(v));
    let call = |name_str: &str, args, ret_ty| Instruction::CallRuntime {
        name: name_str.into(),
        args,
        ret_ty,
    };
    m.functions.push(Function {
        symbol: "prova_slots".into(),
        name: "prova_slots".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    call(
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
                    Instruction::ArcCopy { value: value(1) },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcStoreStrong {
                        slot: frame.clone(),
                        value: value(2),
                        mode: StrongStoreMode::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::ArcLoadStrong { slot: frame },
                    Type::Ref,
                ),
                (
                    ValueId(5),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: value(4),
                        mode: StrongStoreMode::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(6),
                    call(
                        "dartforge_arc_quadro_fechar_v1",
                        vec![(value(0), Type::I64)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(7),
                    call("dartforge_arc_collect", vec![], Type::Void),
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
                    call(
                        "dartforge_print_handle",
                        vec![(value(8), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(10),
                    Instruction::ArcDrop { value: value(8) },
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
                    Instruction::ArcCopy { value: value(11) },
                    Type::Ref,
                ),
                (
                    ValueId(13),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: value(12),
                        mode: StrongStoreMode::Move,
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
                    call(
                        "dartforge_print_handle",
                        vec![(value(14), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(16),
                    Instruction::ArcDrop { value: value(14) },
                    Type::Void,
                ),
                (
                    ValueId(17),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: Operand::Constant(Constant::Null),
                        mode: StrongStoreMode::Copy,
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
                    call(
                        "dartforge_print_handle",
                        vec![(value(18), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(20),
                    Instruction::ArcDrop { value: value(18) },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    dartforge_emit_native::optimize::optimize(&mut m);
    let mut dart_summaries = Vec::new();
    if return_proof {
        let mut identity = Function {
            symbol: "prova_retorno".into(),
            name: "prova_retorno".into(),
            debug_info: None,
            params: vec![(ValueId(0), "valor".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(value(0))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: TokenReturn::Owned,
            ..Default::default()
        };
        if !auto_cleanup {
            let copies = insert_dart_return_retention(
                &mut identity,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )?;
            if copies != 1 {
                return Err("prova de retorno exige uma retenção inserida".into());
            }
            dart_summaries.push(verify_dart_function_contract(
                &identity,
                &classes,
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )?);
        }
        // The function is added after the general optimization, to check
        // the real call/return without inlining hiding that boundary.
        m.functions.push(identity);
        for (v, inst, _) in &mut m.functions[0].blocks[0].instructions {
            if *v == ValueId(2) {
                *inst = Instruction::CallStatic {
                    symbol: "prova_retorno".into(),
                    args: vec![value(1)],
                    ret_ty: Type::Ref,
                };
            }
        }
    }
    if mortal_proof {
        let body = &mut m.functions[0].blocks[0].instructions;
        for (v, inst, _) in body.iter_mut() {
            if *v == ValueId(1) {
                *inst = call(
                    "dartforge_arc_box_int_owned_v1",
                    vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                    Type::Ref,
                );
            }
        }
        let after_store = body
            .iter()
            .position(|(v, _, _)| *v == ValueId(3))
            .ok_or("store ausente na prova mortal")?
            + 1;
        // The slot keeps the token returned by the callee; releases the initial
        // occurrence of the factory before closing the frame and collecting.
        body.insert(
            after_store,
            (
                ValueId(100),
                Instruction::ArcDrop { value: value(1) },
                Type::Void,
            ),
        );
    }
    let f = &mut m.functions[0];
    let originals = std::mem::take(&mut f.blocks[0].instructions);
    f.blocks.clear();
    let mut current = BlockId(0);
    let mut body = Vec::new();
    let mut next_item = originals.iter().map(|(v, _, _)| v.0).max().unwrap_or(0) + 1;
    for mut instruction in originals {
        if forced_error == Some(instruction.0) {
            if let Instruction::CallRuntime { name, .. } = &mut instruction.1 {
                // Replaces the observation with an audited Ref throw. The next
                // test selects the same prepared cleanup block.
                *name = "dartforge_arc_lancar_ref_v1".into();
            }
        }
        let printed = match &instruction.1 {
            Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_print_handle" || name == "dartforge_arc_lancar_ref_v1" =>
            {
                Some(args[0].0.clone())
            }
            _ => None,
        };
        body.push(instruction);
        if let Some(printed) = printed {
            let p = ValueId(next_item);
            let c = ValueId(next_item + 1);
            next_item += 2;
            body.push((
                p,
                call("dartforge_exception_pending", vec![], Type::I8),
                Type::I8,
            ));
            body.push((
                c,
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(p),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ));
            let error = BlockId(current.0 + 1);
            let success = BlockId(current.0 + 2);
            f.blocks.push(BasicBlock {
                id: current,
                instructions: std::mem::take(&mut body),
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(c),
                    then_block: error,
                    else_block: success,
                },
            });
            let mut cleanup = Vec::new();
            if !without_cleanup {
                cleanup.push((
                    ValueId(next_item),
                    Instruction::ArcDrop { value: printed },
                    Type::Void,
                ));
                next_item += 1;
            }
            cleanup.push((
                ValueId(next_item),
                Instruction::ArcStoreStrong {
                    slot: StrongSlot::Global {
                        symbol: "dfg.prova".into(),
                    },
                    value: Operand::Constant(Constant::Null),
                    mode: StrongStoreMode::Copy,
                },
                Type::Void,
            ));
            next_item += 1;
            if forced_error.is_some() {
                cleanup.push((
                    ValueId(next_item),
                    call("dartforge_exception_clear", vec![], Type::Void),
                    Type::Void,
                ));
                next_item += 1;
            }
            f.blocks.push(BasicBlock {
                id: error,
                instructions: cleanup,
                terminator: Terminator::Return(None),
            });
            current = success;
        }
    }
    f.blocks.push(BasicBlock {
        id: current,
        instructions: body,
        terminator: Terminator::Return(None),
    });
    // The box of 42 is an immediate Smi; the permanent literal is classified
    // by the constants producer, without a manual contract in this proof.
    let mut classes = HashMap::from([(ValueId(11), Ownership::Trivial)]);
    let mut plan = TokenPlan::default();
    // The Dart call gets its contract from the summary of the body verified above.
    if !auto_cleanup {
        produce_dart_calls(f, &dart_summaries, &mut classes, &mut plan)?;
    }
    plan
        .instructions
        .insert(ValueId(11), TokenEffect::default());
    if auto_cleanup {
        // Prepares caller and callee together: a retention at the return of the callee
        // and three drops at the error exits of the caller, without partial publication.
        let symbol = f.symbol.clone();
        let mut callee = DartFunctionPlan::default();
        callee.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([
            (
                symbol,
                DartFunctionPlan {
                    classes,
                    tokens: plan,
                    ..Default::default()
                },
            ),
            ("prova_retorno".into(), callee),
        ]);
        let inserted = insert_arc_dart_functions(&mut m.functions, &mut plans)?;
        if inserted != (1, 3) {
            return Err(format!(
                "preparação esperada (1, 3), encontrada {inserted:?}"
            ));
        }
    } else {
        produce_and_verify_tokens(
            f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )?;
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // Observation helper of the harness, exported by the runtime but outside
    // the normal catalog used by the SDK lowering.
    ir.push_str("\ndeclare void @dartforge_print_handle(i64)\n");
    if let Some(dir) = exit.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &exit,
        &driver::NativeDriverOptions {
            optimize: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
