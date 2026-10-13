//! Unix AOT proof: ARC cleanup with a C++ exception, identity and death of the Mint.
//! Injects a C++ throw at the print_handle point after emitting the audited HIR.
//! The hook keeps the Borrow signature, consumes no tokens nor creates a Dart pending state.
//! The symbol swap is explicit in the IR; this is a fault-injection proof.

#[cfg(unix)]
use dartforge_emit_native::llvm::LlvmEmitter;
#[cfg(any(unix, test))]
use dartforge_emit_native::{hir::*, optimize::arc::*};
#[cfg(any(unix, test))]
use std::collections::HashMap;

#[cfg(any(unix, test))]
fn module(with_frames: bool, resume: bool) -> Result<Module, String> {
    let val = |v| Operand::Val(ValueId(v));
    let print = |v, argument| {
        (
            ValueId(v),
            Instruction::CallRuntime {
                name: "dartforge_print_handle".into(),
                args: vec![(argument, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    };
    let mut caller = Function {
        symbol: "prova_owner_estrangeiro".into(),
        name: "prova_owner_estrangeiro".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::Box {
                            op: Operand::Constant(Constant::Int(i64::MAX)),
                            from: Type::I64,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "prova_falha_estrangeira".into(),
                            args: vec![val(0)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
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
                instructions: vec![print(3, val(0))],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    };
    if with_frames {
        let call = |id, name: &str, args, ty| {
            (
                ValueId(id),
                Instruction::CallRuntime {
                    name: name.into(),
                    args,
                    ret_ty: ty,
                },
                ty,
            )
        };
        let open = |id| {
            call(
                id,
                "dartforge_arc_frame_open_v1",
                vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                Type::I64,
            )
        };
        let close = |id, frame| {
            call(
                id,
                "dartforge_arc_frame_close_v1",
                vec![(val(frame), Type::I64)],
                Type::Void,
            )
        };
        let slot = |frame| StrongSlot::Frame {
            frame: val(frame),
            index: 0,
        };
        let factory = caller.blocks[0].instructions.remove(0);
        caller.blocks[0].instructions = vec![
            open(6),
            open(11),
            factory,
            (
                ValueId(7),
                Instruction::ArcStoreStrong {
                    slot: slot(6),
                    value: val(0),
                    mode: StrongStoreMode::Copy,
                },
                Type::Void,
            ),
            (
                ValueId(12),
                Instruction::ArcStoreStrong {
                    slot: slot(11),
                    value: val(0),
                    mode: StrongStoreMode::Move,
                },
                Type::Void,
            ),
            (
                ValueId(8),
                Instruction::ArcLoadStrong { slot: slot(11) },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::CallStatic {
                    symbol: "prova_falha_estrangeira".into(),
                    args: vec![val(8)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        caller.blocks[1].instructions = vec![close(13, 11), close(14, 6), print(3, val(8))];
        caller.blocks[2].instructions = vec![close(15, 11), close(16, 6)];
    }
    if resume {
        caller.blocks[1]
            .instructions
            .retain(|(id, _, _)| *id != ValueId(3));
    }
    let leaf = Function {
        symbol: "prova_falha_estrangeira".into(),
        name: "prova_falha_estrangeira".into(),
        debug_info: None,
        params: vec![(ValueId(0), "valor".into(), Type::Ref)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![print(1, val(0))],
            terminator: Terminator::Return(None),
        }],
    };
    let mut caller_plan = DartFunctionPlan::default();
    caller_plan
        .tables
        .invocations
        .insert(ValueId(2), BlockId(1));
    caller_plan.tables.landing_pads.insert(BlockId(1));
    caller_plan.tables.exits.insert(
        BlockId(1),
        if resume {
            ExceptionExit::Resume
        } else {
            ExceptionExit::Guard
        },
    );
    let mut leaf_plan = DartFunctionPlan::default();
    leaf_plan
        .tables
        .exits
        .insert(BlockId(0), ExceptionExit::Guard);
    let mut plans = HashMap::from([
        (caller.symbol.clone(), caller_plan),
        (leaf.symbol.clone(), leaf_plan),
    ]);
    let mut m = Module::new();
    m.functions = vec![caller, leaf];
    m.table_exceptions = true;
    m.entry_symbol = Some("prova_owner_estrangeiro".into());
    assert_eq!(
        insert_arc_dart_functions(&mut m.functions, &mut plans)?,
        (0, if resume { 2 } else { 3 })
    );
    assert_eq!(
        plans["prova_owner_estrangeiro"].owners_at_landing_pad[&BlockId(1)],
        vec![ValueId(if with_frames { 8 } else { 0 })]
    );
    m.tables = m
        .functions
        .iter()
        .map(|f| plans[&f.symbol].tables.clone())
        .collect();
    Ok(m)
}

#[cfg(unix)]
fn main() -> Result<(), String> {
    use dartforge_emit_native::{
        cache::RuntimeCache,
        generator::{Generation, Generator},
    };
    use std::{path::PathBuf, process::Command};
    let exit = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("provide the output executable")?,
    );
    if let Some(dir) = exit.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let profile = std::env::args().nth(3).unwrap_or_else(|| "simples".into());
    let m = module(
        profile == "quadros" || profile == "retoma",
        profile == "retoma",
    )?;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    ir.push_str("\ndeclare void @dartforge_print_handle(i64)\n");
    ir = ir.replace("@dartforge_print_handle(", "@prova_print_estrangeira(");
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    let obj = exit.with_extension("o");
    Generator::choose(&PathBuf::from("clang")).generate(
        &ir,
        Generation::of_program(std::env::args().nth(2).as_deref() != Some("0"), false),
        &obj,
    )?;
    let cpp = exit.with_extension("cpp");
    std::fs::write(
        &cpp,
        include_str!("../tests/fixtures/arc_unwind_estrangeiro.cpp"),
    )
    .map_err(|e| e.to_string())?;
    let runtime = RuntimeCache::for_production()?;
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "c++".into());
    #[cfg(not(target_os = "macos"))]
    let library = runtime.lib_path.clone();
    #[cfg(target_os = "macos")]
    let library = {
        // Compact unwind admits three personalities per image. The fixture
        // combines C++, legacy Dart/cleanup and Rust; it splits the runtime into a dylib
        // keeping the unwind information of both images.
        let dylib = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(exit.with_extension("runtime.dylib"));
        let mut link_runtime = Command::new(&cxx);
        link_runtime
            .args(["-dynamiclib", "-Xlinker", "-force_load", "-Xlinker"])
            .arg(&runtime.lib_path)
            .args(["-Xlinker", "-install_name", "-Xlinker"])
            .arg(&dylib)
            .args(dartforge_emit_native::target::system_libraries())
            .arg("-o")
            .arg(&dylib);
        std::fs::write(
            exit.with_extension("runtime.ligacao.txt"),
            format!("{link_runtime:?}\n"),
        )
        .map_err(|e| e.to_string())?;
        let result = link_runtime.output().map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(format!(
                "linking the runtime dylib failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        dylib
    };
    let mut cmd = Command::new(&cxx);
    cmd.args(["-std=c++17", "-O2"])
        .arg(&cpp)
        .arg(&obj)
        .arg(&library)
        .arg("-o")
        .arg(&exit);
    cmd.args(dartforge_emit_native::target::system_libraries());
    std::fs::write(exit.with_extension("ligacao.txt"), format!("{cmd:?}\n"))
        .map_err(|e| e.to_string())?;
    let result = cmd.output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "C++ linking failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn main() -> Result<(), String> {
    Err("the proof requires Itanium Unix unwinding".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_proof_has_certified_inventory() {
        let m = module(false, false).unwrap();
        assert!(m.tables[0].foreign_cleanup.is_some());
        #[cfg(unix)]
        {
            let ir = LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("lpad1.estrangeira:"));
            assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
        }
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    #[test]
    fn foreign_proof_with_two_local_frames() {
        let m = module(true, false).unwrap();
        assert!(m.tables[0].foreign_cleanup.is_some());
        assert!(dartforge_emit_native::lower::verifier::verify(&m).is_empty());
        #[cfg(unix)]
        {
            let ir = LlvmEmitter::new(&m).emit_all();
            let foreign = ir
                .split("lpad1.estrangeira:")
                .nth(1)
                .unwrap()
                .split("resume ")
                .next()
                .unwrap();
            let internal = foreign
                .find("call void @dartforge_arc_frame_close_v1(i64 %v11)")
                .unwrap();
            let external = foreign
                .find("call void @dartforge_arc_frame_close_v1(i64 %v6)")
                .unwrap();
            assert!(internal < external);
        }
    }
}

#[cfg(test)]
mod resume_tests {
    use super::*;
    #[test]
    fn resume_closes_two_frames_without_running_dart_catch() {
        let m = module(true, true).unwrap();
        assert_eq!(
            m.tables[0].exits.get(&BlockId(1)),
            Some(&ExceptionExit::Resume)
        );
        assert!(m.tables[0].foreign_cleanup.is_none());
        assert!(dartforge_emit_native::lower::verifier::verify(&m).is_empty());
    }
}
