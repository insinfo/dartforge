//! AOT proof of a strong global with a mortal target observed through a weak reference.
//! The producing function leaves before the collections, removing its observational roots.
//! Explicit HIR and LLVM observation helpers do not certify the automatic producer.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter};
use std::path::PathBuf;

fn call(name: &str, args: Vec<(Operand, Type)>) -> Instruction {
    Instruction::CallRuntime {
        name: name.into(),
        args,
        ret_ty: Type::Void,
    }
}

fn function(name: &str, ops: Vec<(Instruction, Type)>) -> Function {
    Function {
        symbol: name.into(),
        name: name.into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: ops
                .into_iter()
                .enumerate()
                .map(|(i, (op, ty))| (ValueId(i as u32), op, ty))
                .collect(),
            terminator: Terminator::Return(None),
        }],
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let mut m = Module::new();
    m.arc_memory = args.next().as_deref() != Some("tracing");
    let sabotage = args.next();
    m.entry_symbol = Some("prova_mortal".into());
    m.globals.push((0, Type::Ref, "dfg.mortal".into()));
    let global = StrongSlot::Global {
        symbol: "dfg.mortal".into(),
    };
    let integer = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
    m.functions.push(function(
        "produzir_mortal",
        vec![
            (
                Instruction::CallRuntime {
                    name: "dartforge_alocar".into(),
                    args: vec![integer(0), integer(1), integer(2)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            (
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(0)),
                },
                Type::Ref,
            ),
            (
                Instruction::ArcStoreStrong {
                    slot: global.clone(),
                    value: Operand::Val(ValueId(1)),
                    mode: StrongStoreMode::Move,
                },
                Type::Void,
            ),
            (
                call(
                    "prova_criar_fraca",
                    vec![(Operand::Val(ValueId(0)), Type::Ref)],
                ),
                Type::Void,
            ),
        ],
    ));
    m.functions.push(function(
        "prova_mortal",
        vec![
            // CallRuntime preserves the boundary in the HIR optimizer. Even if LLVM
            // inlines the body, the close of the emitted frame precedes the collection.
            (call("produzir_mortal", vec![]), Type::Void),
            (call("dartforge_arc_collect", vec![]), Type::Void),
            (
                call("prova_conferir_fraca", vec![integer(1)]),
                Type::Void,
            ),
            (
                Instruction::ArcStoreStrong {
                    slot: global,
                    value: Operand::Constant(Constant::Null),
                    mode: StrongStoreMode::Copy,
                },
                Type::Void,
            ),
            (call("dartforge_arc_collect", vec![]), Type::Void),
            (
                call("prova_conferir_fraca", vec![integer(0)]),
                Type::Void,
            ),
            (call("prova_fechar_fraca", vec![]), Type::Void),
        ],
    ));
    match sabotage.as_deref() {
        None => {}
        // Negative controls: one dies early, the other stays alive.
        Some("sem-owner") => {
            m.functions[0].blocks[0].instructions[2].1 = Instruction::ArcDrop {
                value: Operand::Val(ValueId(1)),
            };
        }
        Some("sem-liberar") => {
            m.functions[1].blocks[0].instructions[3].1 = call("dartforge_arc_collect", vec![]);
        }
        Some(other) => return Err(format!("sabotagem desconhecida: {other}")),
    }
    dartforge_emit_native::optimize::optimize(&mut m);
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // The observation stays outside the HIR: the weak target gets no strong root
    // in the caller. Only the weak container receives an independent token.
    ir.push_str(
        r#"
@prova.fraca = internal global i64 0
declare void @dartforge_nativo_WeakReference_setTarget(i64, i64)
declare i64 @dartforge_nativo_WeakReference_getTarget(i64)
declare void @dartforge_print_handle(i64)
define void @prova_criar_fraca(i64 %alvo) {
  %w = call i64 @dartforge_alocar(i64 0, i64 1, i64 2)
  call void @dartforge_arc_retain(i64 %w)
  store i64 %w, ptr @prova.fraca
  call void @dartforge_nativo_WeakReference_setTarget(i64 %w, i64 %alvo)
  ret void
}
define void @prova_conferir_fraca(i64 %esperado) {
  %w = load i64, ptr @prova.fraca
  %alvo = call i64 @dartforge_nativo_WeakReference_getTarget(i64 %w)
  %vivo = icmp ne i64 %alvo, 0
  %quer = icmp ne i64 %esperado, 0
  %ok = icmp eq i1 %vivo, %quer
  br i1 %ok, label %passou, label %falhou
falhou:
  call void @llvm.trap()
  unreachable
passou:
  %saida = select i1 %vivo, i64 3, i64 0
  call void @dartforge_print_handle(i64 %saida)
  ret void
}
define void @prova_fechar_fraca() {
  %w = load i64, ptr @prova.fraca
  store i64 0, ptr @prova.fraca
  call void @dartforge_arc_release(i64 %w)
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
            optimize: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
