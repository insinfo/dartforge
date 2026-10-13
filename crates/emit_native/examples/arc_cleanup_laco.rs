//! AOT proof of cleanup produced on a normal edge inside a loop.
//! The HIR allocates an Owned Mint and uses a Borrow call before the pending guard.
//! LLVM observation hooks replace print_handle with the same signature,
//! without GC, Dart or consumption. Diagnostic addresses are not Ref roots.
//! Each iteration checks the owner before/after the release; the final physical death
//! is checked after leaving the function. Tracing may keep observational roots
//! during the function. The error uses a real preloaded pending state, not a throw from the hook.
//! Does not certify source lowering, Finalizable, suspension or unwinding.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn module() -> Result<Module, String> {
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_cleanup_laco".into());
    let val = |v| Operand::Val(ValueId(v));
    let int = |v| Operand::Constant(Constant::Int(v));
    let call = |name: &str, args, ret_ty| Instruction::CallRuntime {
        name: name.into(),
        args,
        ret_ty,
    };
    let guard = |cmp, error, normal| Terminator::CondBranch {
        cond: val(cmp),
        then_block: BlockId(error),
        else_block: BlockId(normal),
    };
    let flag = |v| {
        (
            ValueId(v),
            call("dartforge_exception_pending", vec![], Type::I8),
            Type::I8,
        )
    };
    let cmp = |v, p| {
        (
            ValueId(v),
            Instruction::ICmp(ICmpOp::Ne, val(p), int(0)),
            Type::I1,
        )
    };
    m.functions.push(Function {
        symbol: "alocar_no_laco".into(),
        name: "alocar_no_laco".into(),
        debug_info: None,
        params: vec![(ValueId(0), "limite".into(), Type::I64)],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Phi {
                            incoming: vec![(BlockId(0), int(0)), (BlockId(4), val(7))],
                            ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        call(
                            "dartforge_arc_box_int_owned_v1",
                            vec![(int(i64::MAX), Type::I64)],
                            Type::Ref,
                        ),
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        call(
                            "dartforge_print_handle",
                            vec![(val(2), Type::Ref)],
                            Type::Void,
                        ),
                        Type::Void,
                    ),
                    flag(4),
                    cmp(5, 4),
                ],
                terminator: guard(5, 5, 2),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![
                    (
                        ValueId(6),
                        call("dartforge_arc_collect", vec![], Type::Void),
                        Type::Void,
                    ),
                    (
                        ValueId(9),
                        call(
                            "dartforge_print_handle",
                            vec![(Operand::Constant(Constant::Null), Type::Ref)],
                            Type::Void,
                        ),
                        Type::Void,
                    ),
                    flag(10),
                    cmp(11, 10),
                ],
                terminator: guard(11, 6, 4),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![
                    (ValueId(7), Instruction::Add(val(1), int(1)), Type::I64),
                    (
                        ValueId(8),
                        Instruction::ICmp(ICmpOp::Slt, val(7), val(0)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(8),
                    then_block: BlockId(1),
                    else_block: BlockId(3),
                },
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(5),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(6),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    });
    let mut plan = DartFunctionPlan::default();
    // The harness limit is a scalar counter, not reference bits.
    plan.classes.insert(ValueId(0), Ownership::Trivial);
    let mut plans = HashMap::from([("alocar_no_laco".into(), plan)]);
    let produced = prepare_arc_dart_module(&mut m, &mut plans)?;
    if produced != (0, 2) {
        return Err(format!("expected cleanup (0, 2), found {produced:?}"));
    }
    let before = format!("{m:?}/{plans:?}");
    if prepare_arc_dart_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}/{plans:?}") != before
    {
        return Err("the loop cleanup was not idempotent".into());
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("provide the output executable")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(other) => return Err(format!("unknown mode: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("unknown level: {other}")),
    };
    let error = match args.next().as_deref() {
        Some("erro") => true,
        Some("normal") | None => false,
        Some(other) => return Err(format!("unknown path: {other}")),
    };
    let control = args.next();
    let mut m = module()?;
    m.arc_memory = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    for (before, after) in [
        ("@dartforge_print_handle(i64 %v2)", "@prova_salvar(i64 %v2)"),
        ("@dartforge_print_handle(i64 0)", "@prova_sem_owner(i64 0)"),
    ] {
        if ir.matches(before).count() != 1 {
            return Err(format!("missing observation hook: {before}"));
        }
        ir = ir.replace(before, after);
    }
    match control.as_deref() {
        None => {}
        Some("sem-drop-normal") if !error => {
            let excerpt = "b7:\n  call void @dartforge_arc_release(i64 %v2)\n";
            if ir.matches(excerpt).count() != 1 {
                return Err("normal cleanup missing in the IR".into());
            }
            ir = ir.replace(excerpt, "b7:\n");
        }
        Some("sem-drop-erro") if error => {
            let excerpt = "b5:\n  call void @dartforge_arc_release(i64 %v2)\n";
            if ir.matches(excerpt).count() != 1 {
                return Err("error cleanup missing in the IR".into());
            }
            ir = ir.replace(excerpt, "b5:\n");
        }
        Some(other) => return Err(format!("incompatible control/path: {other}")),
    }
    let preload = if error {
        "  call void @dartforge_arc_lancar_ref_v1(i64 85)\n"
    } else {
        ""
    };
    ir.push_str(&format!("\ndefine void @prova_cleanup_laco() {{\n{preload}  call void @alocar_no_laco(i64 10)\n  call void @prova_final(i64 {})\n  ret void\n}}\n", if error { 1 } else { 10 }));
    ir.push_str(
        r#"
@prova.endereco = internal global i64 0
@prova.iteracoes = internal global i64 0
define void @prova_salvar(i64 %obj) {
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %salvar, label %falha
salvar:
  store i64 %obj, ptr @prova.endereco
  %i = load i64, ptr @prova.iteracoes
  %j = add i64 %i, 1
  store i64 %j, ptr @prova.iteracoes
  ret void
falha:
  call void @llvm.trap()
  unreachable
}
define void @prova_sem_owner(i64 %ignorado) {
  %obj = load i64, ptr @prova.endereco
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %bit = and i8 %estado, 2
  %solto = icmp eq i8 %bit, 0
  br i1 %solto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  ret void
}
declare void @dartforge_print_handle(i64)
define void @prova_final(i64 %esperado) {
  %i = load i64, ptr @prova.iteracoes
  %contagem = icmp eq i64 %i, %esperado
  br i1 %contagem, label %conferir, label %falha
conferir:
  call void @prova_sem_owner(i64 0)
  %pending = call i8 @dartforge_exception_pending()
  %com_erro = icmp ne i8 %pending, 0
  %quer_erro = icmp eq i64 %esperado, 1
  %ok_pending = icmp eq i1 %com_erro, %quer_erro
  br i1 %ok_pending, label %identidade, label %falha
identidade:
  br i1 %quer_erro, label %capturar, label %limpar
capturar:
  %excecao = call i64 @dartforge_arc_excecao_owned_v1()
  %mesma = icmp eq i64 %excecao, 85
  call void @dartforge_arc_release(i64 %excecao)
  br i1 %mesma, label %limpar, label %falha
limpar:
  call void @dartforge_exception_clear()
  call void @dartforge_arc_collect()
  %obj = load i64, ptr @prova.endereco
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %morto = icmp eq i8 %estado, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if let Some(dir) = exit.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &exit,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loop_cleanup_is_before_collection_and_on_error_exit() {
        let m = module().unwrap();
        let f = &m.functions[0];
        assert!(matches!(
            f.blocks[1].terminator,
            Terminator::CondBranch {
                then_block: BlockId(5),
                else_block: BlockId(7),
                ..
            }
        ));
        for id in [5, 7] {
            let b = f.blocks.iter().find(|b| b.id == BlockId(id)).unwrap();
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(2))
                }
            ));
        }
        LlvmEmitter::new(&m).emit_all();
    }
}
