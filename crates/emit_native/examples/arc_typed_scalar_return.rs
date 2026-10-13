//! AOT proof of the three scalar representations with a borrowed heap context.
//! The certificate registers the body; the caller promotes and releases its context.
use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn module(ty: Type, value: Constant, abi: i64) -> Result<Module, String> {
    let mut m = Module::new(); m.arc_memory = true; m.entry_symbol = Some("prova".into());
    m.functions = vec![
        Function { symbol: "corpo".into(), name: "corpo".into(), debug_info: None,
            params: vec![(ValueId(0), "contexto".into(), Type::Ref)], return_ty: ty,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(value.clone()))) }] },
        Function { symbol: "entrada".into(), name: "entrada".into(), debug_info: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
            return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] },
        Function { symbol: "capturar".into(), name: "capturar".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::Box { op: Operand::Constant(Constant::Int(i64::MAX)), from: Type::I64 }, Type::Ref),
                (ValueId(0), Instruction::AllocClosureTyped { code_symbol: "entrada".into(), env: Operand::Val(ValueId(1)), typed: "corpo".into(), abi, direct: false }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] },
        Function { symbol: "invocar".into(), name: "invocar".into(), debug_info: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref)], return_ty: ty,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_arc_typed_target_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(abi)), Type::I64),
                ], ret_ty: Type::I64 }, Type::I64),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_arc_closure_context_v1".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::TypedCall { target: Operand::Val(ValueId(1)), args: vec![(Operand::Val(ValueId(2)), Type::Ref)], ret: ty }, ty),
                (ValueId(4), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(5), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(4)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(5)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(value))) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))) },
            ] },
    ];
    m.dart_ref_returns.insert("capturar".into()); m.arc_uniform_returns.insert("entrada".into());
    m.dart_vector_parameters.insert("entrada".into(), HashMap::from([
        (ValueId(1), DartEntryVector::Arguments), (ValueId(2), DartEntryVector::Descriptor),
    ]));
    let mut plans = m.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect();
    assert_eq!(prepare_arc_dart_module(&mut m, &mut plans)?, (1, 3));
    assert_eq!(plans["invocar"].classes[&ValueId(3)], Ownership::Trivial);
    let ready = plans.clone(); assert_eq!(prepare_arc_dart_module(&mut m, &mut plans)?, (0, 0)); assert_eq!(plans, ready);
    if certify_arc_aot_ports(&mut m, &plans)? != 2 { return Err("portas escalares incompletas".into()); }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("provide the executable")?);
    let arc = match args.next().as_deref() { Some("--memory=arc") => true, Some("--memory=tracing") | None => false, _ => return Err("invalid mode".into()) };
    let optimize = match args.next().as_deref() { Some("2") => true, Some("0") | None => false, _ => return Err("invalid level".into()) };
    let (ty, value, abi, check) = match args.next().as_deref() {
        Some("--int") => (Type::I64, Constant::Int(i64::MIN), 7, "  %ok = icmp eq i64 %valor, -9223372036854775808\n"),
        Some("--double") => (Type::F64, Constant::Double(-0.0), 8, "  %bits = bitcast double %valor to i64\n  %ok = icmp eq i64 %bits, -9223372036854775808\n"),
        Some("--bool") => (Type::I1, Constant::Bool(true), 9, "  %ok = icmp eq i1 %valor, true\n"),
        _ => return Err("provide --int, --double or --bool".into()),
    };
    let control = args.next(); if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = module(ty, value, abi)?; m.arc_memory = arc; if !arc { m.arc_aot_ports = None; }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    if let Some(c) = control {
        let target = "  call void @dartforge_arc_release(i64 %v2)\n";
        if c != "no-context-release" || ir.matches(target).count() != 2 { return Err("control without a single target on both exits".into()); }
        ir = ir.replace(target, "");
    }
    ir.push_str("declare void @dartforge_print_handle(i64)\ndefine void @prova() {\n");
    if !arc { ir.push_str(&format!("  %pc = ptrtoint ptr @corpo to i64\n  %versao = call i64 @dartforge_arc_register_typed_v1(i64 %pc, i64 {abi})\n")); }
    ir.push_str(&format!("  %closure = call i64 @capturar()\n  %contexto = call i64 @dartforge_arc_closure_context_v1(i64 %closure)\n  %valor = call {} @invocar(i64 %closure)\n{check}", ty.llvm_ir()));
    ir.push_str(r#"  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %closure)
  call void @dartforge_arc_collect()
  %estado = call i8 @dartforge_arc_observe_heap_v1(i64 %contexto)
  %morto = icmp eq i8 %estado, 0
  br i1 %morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#);
    if let Some(dir) = exit.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(&ir, &exit, &driver::NativeDriverOptions { optimize, ..Default::default() }).map(|_| ())
}
