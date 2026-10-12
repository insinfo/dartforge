//! Prova AOT das três representações escalares com contexto heap emprestado.
//! O certificado registra o corpo; o chamador promove e libera seu contexto.
use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo(ty: Type, valor: Constant, abi: i64) -> Result<Module, String> {
    let mut m = Module::new(); m.memoria_arc = true; m.entry_symbol = Some("prova".into());
    m.functions = vec![
        Function { symbol: "corpo".into(), name: "corpo".into(), depuracao: None,
            params: vec![(ValueId(0), "contexto".into(), Type::Ref)], return_ty: ty,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(valor.clone()))) }] },
        Function { symbol: "entrada".into(), name: "entrada".into(), depuracao: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
            return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] },
        Function { symbol: "capturar".into(), name: "capturar".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::Box { op: Operand::Constant(Constant::Int(i64::MAX)), from: Type::I64 }, Type::Ref),
                (ValueId(0), Instruction::AllocClosureTipada { code_symbol: "entrada".into(), env: Operand::Val(ValueId(1)), tipado: "corpo".into(), abi, direto: false }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] },
        Function { symbol: "invocar".into(), name: "invocar".into(), depuracao: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref)], return_ty: ty,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_arc_alvo_tipada_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(abi)), Type::I64),
                ], ret_ty: Type::I64 }, Type::I64),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_arc_contexto_closure_v1".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::ChamadaTipada { alvo: Operand::Val(ValueId(1)), args: vec![(Operand::Val(ValueId(2)), Type::Ref)], ret: ty }, ty),
                (ValueId(4), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(5), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(4)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(5)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(valor))) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))) },
            ] },
    ];
    m.retornos_ref_dart.insert("capturar".into()); m.retornos_uniformes_arc.insert("entrada".into());
    m.parametros_vetores_dart.insert("entrada".into(), HashMap::from([
        (ValueId(1), VetorEntradaDart::Argumentos), (ValueId(2), VetorEntradaDart::Descritor),
    ]));
    let mut planos = m.functions.iter().map(|f| (f.symbol.clone(), PlanoFuncaoDart::default())).collect();
    assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos)?, (1, 3));
    assert_eq!(planos["invocar"].classes[&ValueId(3)], Ownership::Trivial);
    let pronto = planos.clone(); assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos)?, (0, 0)); assert_eq!(planos, pronto);
    if certificar_portas_arc_aot(&mut m, &planos)? != 2 { return Err("portas escalares incompletas".into()); }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável")?);
    let arc = match args.next().as_deref() { Some("--memoria=arc") => true, Some("--memoria=tracing") | None => false, _ => return Err("modo inválido".into()) };
    let optimize = match args.next().as_deref() { Some("2") => true, Some("0") | None => false, _ => return Err("nível inválido".into()) };
    let (ty, valor, abi, conferir) = match args.next().as_deref() {
        Some("--int") => (Type::I64, Constant::Int(i64::MIN), 7, "  %ok = icmp eq i64 %valor, -9223372036854775808\n"),
        Some("--double") => (Type::F64, Constant::Double(-0.0), 8, "  %bits = bitcast double %valor to i64\n  %ok = icmp eq i64 %bits, -9223372036854775808\n"),
        Some("--bool") => (Type::I1, Constant::Bool(true), 9, "  %ok = icmp eq i1 %valor, true\n"),
        _ => return Err("informe --int, --double ou --bool".into()),
    };
    let controle = args.next(); if args.next().is_some() { return Err("argumentos excedentes".into()); }
    let mut m = modulo(ty, valor, abi)?; m.memoria_arc = arc; if !arc { m.portas_arc_aot = None; }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    if let Some(c) = controle {
        let alvo = "  call void @dartforge_arc_release(i64 %v2)\n";
        if c != "sem-liberar-contexto" || ir.matches(alvo).count() != 2 { return Err("controle sem alvo único nas duas saídas".into()); }
        ir = ir.replace(alvo, "");
    }
    ir.push_str("declare void @dartforge_print_handle(i64)\ndefine void @prova() {\n");
    if !arc { ir.push_str(&format!("  %pc = ptrtoint ptr @corpo to i64\n  %versao = call i64 @dartforge_arc_registrar_tipada_v1(i64 %pc, i64 {abi})\n")); }
    ir.push_str(&format!("  %closure = call i64 @capturar()\n  %contexto = call i64 @dartforge_arc_contexto_closure_v1(i64 %closure)\n  %valor = call {} @invocar(i64 %closure)\n{conferir}", ty.llvm_ir()));
    ir.push_str(r#"  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %closure)
  call void @dartforge_arc_collect()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %contexto)
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
    if let Some(dir) = saida.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(&ir, &saida, &driver::NativeDriverOptions { optimize, ..Default::default() }).map(|_| ())
}
