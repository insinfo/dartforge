//! AOT proof of the AllocObject preparation with a full layout of 40 fields.
//! Checks scalar bits, the Ref edge and death after releasing the Owned return.
//! Tracing uses explicit tokens only in the comparison harness.
//! Does not certify Dart source lowering, getter guards or reload.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn module(frame: bool, typed: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_alocacao_campos".into());
    m.classes.push(ClassDef {
        id: 123,
        name: "C".into(),
        field_count: 40,
        vtable: vec![],
        to_string_symbol: None,
    });
    let mut layout = vec![Type::I64; 40];
    layout[1] = Type::F64;
    layout[2] = Type::I1;
    layout[3] = Type::I8;
    layout[39] = Type::Ref;
    m.arc_field_layouts.insert(123, layout);
    let mut fields = vec![Operand::Constant(Constant::Int(85)); 40];
    fields[1] = Operand::Constant(Constant::Double(-0.0));
    fields[2] = Operand::Constant(Constant::Bool(true));
    fields[3] = Operand::Val(ValueId(3));
    fields[39] = Operand::Val(ValueId(0));
    if typed {
        fields[0] = Operand::Val(ValueId(12));
        fields[1] = Operand::Val(ValueId(11));
        fields[2] = Operand::Val(ValueId(13));
        m.dart_scalar_parameters.insert("criar_objeto".into(), std::collections::HashSet::from([ValueId(12)]));
    }
    if frame {
        m.classes.clear(); m.arc_field_layouts.clear();
        fields[0] = Operand::Constant(Constant::Int(0));
        fields[6] = fields[1].clone(); fields[7] = fields[2].clone(); fields[8] = fields[3].clone();
        fields[1..5].fill(Operand::Constant(Constant::Null));
        fields[5] = Operand::Constant(Constant::Int(0));
    }
    m.functions.push(Function {
        symbol: "criar_objeto".into(),
        name: "criar_objeto".into(),
        debug_info: None,
        params: if typed { vec![(ValueId(10), "contexto".into(), Type::Ref),
            (ValueId(11), "double".into(), Type::F64), (ValueId(12), "int".into(), Type::I64),
            (ValueId(13), "bool".into(), Type::I1)] } else { vec![] },
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
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
                    ValueId(3),
                    Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)),
                        from: Type::I64,
                        to: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: if frame { dartforge_emit_native::lower::async_sm::ASYNC_FRAME_ID } else { 123 },
                        fields,
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    if typed {
        let f = &mut m.functions[0];
        f.blocks[0].id = BlockId(1);
        f.blocks.insert(0, BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
            cond: Operand::Val(ValueId(13)), then_block: BlockId(1), else_block: BlockId(2),
        } });
        f.blocks.extend([
            BasicBlock { id: BlockId(2), instructions: vec![
                (ValueId(20), Instruction::CallClosure { closure: Operand::Constant(Constant::Null), args: vec![], names: vec![],
                    tuple_types: Operand::Constant(Constant::Int(0)), ret_ty: Type::Ref }, Type::Ref),
                (ValueId(21), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(22), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(21)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(22)), then_block: BlockId(3), else_block: BlockId(4) } },
            BasicBlock { id: BlockId(3), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(20)))) },
        ]);
    }
    let mut plan = DartFunctionPlan::default();
    plan.tokens.ret = TokenReturn::Owned;
    if frame {
        m.functions.push(Function { symbol: "ler_quadro".into(), name: "ler_quadro".into(), debug_info: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 39 }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
    }
    let mut plans = HashMap::from([("criar_objeto".into(), plan)]);
    if typed {
        m.functions.push(Function { symbol: "entrada_objeto".into(), name: "entrada_objeto".into(), debug_info: None,
            params: vec![(ValueId(0), "recv".into(), Type::Ref),
                (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
            return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] });
        m.arc_uniform_returns.insert("entrada_objeto".into());
        m.dart_vector_parameters.insert("entrada_objeto".into(), HashMap::from([
            (ValueId(1), DartEntryVector::Arguments), (ValueId(2), DartEntryVector::Descriptor),
        ]));
        m.functions.push(Function { symbol: "criar_closure".into(), name: "criar_closure".into(), debug_info: None,
            params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::Box { op: Operand::Constant(Constant::Int(i64::MAX - 1)), from: Type::I64 }, Type::Ref),
                (ValueId(0), Instruction::AllocClosureTyped {
                    code_symbol: "entrada_objeto".into(), env: Operand::Val(ValueId(1)),
                    typed: "criar_objeto".into(), abi: 1071, direct: false,
                }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] });
        let mut p = DartFunctionPlan::default(); p.tokens.ret = TokenReturn::Owned;
        plans.insert("criar_closure".into(), p);
        plans.insert("entrada_objeto".into(), DartFunctionPlan::default());
        m.functions.push(Function { symbol: "invocar_closure".into(), name: "invocar_closure".into(), debug_info: None,
            params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(7), "normal".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_arc_typed_target_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(1071)), Type::I64),
                ], ret_ty: Type::I64 }, Type::I64),
                (ValueId(2), Instruction::CallRuntime { name: "dartforge_arc_closure_context_v1".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                ], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(3), Instruction::TypedCall { target: Operand::Val(ValueId(1)), args: vec![
                    (Operand::Val(ValueId(2)), Type::Ref), (Operand::Constant(Constant::Double(-0.0)), Type::F64),
                    (Operand::Constant(Constant::Int(85)), Type::I64), (Operand::Val(ValueId(7)), Type::I1),
                ], ret: Type::Ref }, Type::Ref),
                (ValueId(4), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(5), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(4)), Operand::Constant(Constant::Int(0))), Type::I1),
            ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(5)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))) },
            ] });
        let mut p = DartFunctionPlan::default(); p.tokens.ret = TokenReturn::Owned;
        plans.insert("invocar_closure".into(), p);
    }
    if frame {
        let mut p = DartFunctionPlan::default(); p.tokens.ret = TokenReturn::Owned;
        plans.insert("ler_quadro".into(), p);
    }
    let produced = prepare_arc_dart_module(&mut m, &mut plans)?;
    let expected = if frame { (1, 1) } else if typed { (1, 4) } else { (0, 1) };
    if produced != expected {
        return Err(format!("expected {expected:?}, found {produced:?}"));
    }
    let before = format!("{m:?}");
    let plans_before = plans.clone();
    let repeated = prepare_arc_dart_module(&mut m, &mut plans)?;
    if repeated != (0, 0)
        || format!("{m:?}") != before || plans != plans_before
    {
        return Err(format!("non-idempotent preparation: {repeated:?}"));
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    if typed && certify_arc_aot_ports(&mut m, &plans)? != 2 {
        return Err("typed image without the two verified ports".into());
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("provide the executable")?);
    let arc = match args.next().as_deref() {
        Some("--memory=arc") => true,
        Some("--memory=tracing") | None => false,
        Some(other) => return Err(format!("unknown mode: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("unknown level: {other}")),
    };
    let mut control = args.next();
    let frame = control.as_deref() == Some("--frame");
    let typed_error = control.as_deref() == Some("--typed-error");
    let typed = control.as_deref() == Some("--typed") || typed_error;
    if frame || typed { control = args.next(); }
    if args.next().is_some() { return Err("extra arguments".into()); }
    let mut m = module(frame, typed)?;
    m.arc_memory = arc;
    if !arc { m.arc_aot_ports = None; }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    let cid = if frame { dartforge_emit_native::lower::async_sm::ASYNC_FRAME_ID } else { 123 };
    let owned = format!("call i64 @dartforge_arc_object_owned_v1(i64 {cid}, i64 40)");
    let without_owner = format!("call i64 @dartforge_object_new(i64 {cid}, i64 40)");
    let retain_read = m.functions.iter().find(|f| f.symbol == "ler_quadro")
        .and_then(|f| f.blocks.iter().flat_map(|b| &b.instructions).find_map(|(_, i, _)|
            if let Instruction::ArcCopy { value: Operand::Val(v) } = i {
                Some(format!("  call void @dartforge_arc_retain(i64 %v{})\n", v.0))
            } else { None })).unwrap_or_default();
    if let Some(control) = control {
        let (before, after) = match control.as_str() {
            "no-child-drop" => ("  call void @dartforge_arc_release(i64 %v0)\n", ""),
            "no-read-retain" if frame => (retain_read.as_str(), ""),
            "no-context-release" if typed => ("  call void @dartforge_arc_release(i64 %v2)\n", ""),
            "no-object-owner" => (
                owned.as_str(),
                without_owner.as_str(),
            ),
            _ => return Err(format!("unknown control: {control}")),
        };
        let quantity = if control == "no-context-release" { 2 } else { 1 };
        if ir.matches(before).count() != quantity {
            return Err(format!("control without a single target: {control}"));
        }
        ir = ir.replace(before, after);
    }
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_alocacao_campos() {
  %obj = call i64 @criar_objeto()
  %estado = call i8 @dartforge_arc_observe_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %campos, label %falha
campos:
  %a = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 0)
  %b = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 1)
  %c = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 2)
  %d = call i64 @dartforge_arc_read_field_scalar_v1(i64 %obj, i64 3)
  %aok = icmp eq i64 %a, 85
  %bok = icmp eq i64 %b, -9223372036854775808
  %cok = icmp eq i64 %c, 1
  %dok = icmp eq i64 %d, 255
  %ab = and i1 %aok, %bok
  %cd = and i1 %cok, %dok
  %bits = and i1 %ab, %cd
  br i1 %bits, label %filho, label %falha
filho:
  %ref = call i64 @dartforge_arc_read_field_ref_v1(i64 %obj, i64 39)
  %valor = call i64 @dartforge_unbox_int(i64 %ref)
  %mint = icmp eq i64 %valor, 9223372036854775807
  %ef = call i8 @dartforge_arc_observe_heap_v1(i64 %ref)
  %sem_owner = icmp eq i8 %ef, 1
  %ok = and i1 %mint, %sem_owner
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %eo = call i8 @dartforge_arc_observe_heap_v1(i64 %obj)
  %em = call i8 @dartforge_arc_observe_heap_v1(i64 %ref)
  %morto = icmp eq i8 %eo, 0
  %filho_morto = icmp eq i8 %em, 0
  %fim = and i1 %morto, %filho_morto
  br i1 %fim, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if frame {
        // The header reserves four Refs and the RTI tuple; the scalars of this
        // fixture are in the tail. The heap observes the real map of the instance.
        for (of, to) in [(1, 6), (2, 7), (3, 8)] {
            ir = ir.replace(&format!("i64 %obj, i64 {of})"), &format!("i64 %obj, i64 {to})"));
        }
        ir = ir.replace("%aok = icmp eq i64 %a, 85", "%aok = icmp eq i64 %a, 0");
        ir = ir.replace("%ref = call i64 @dartforge_arc_read_field_ref_v1(i64 %obj, i64 39)",
            "%ref = call i64 @ler_quadro(i64 %obj)");
        ir = ir.replace("%sem_owner = icmp eq i8 %ef, 1", "%sem_owner = icmp eq i8 %ef, 3");
        ir = ir.replace("  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()",
            "  call void @dartforge_arc_release(i64 %obj)\n  call void @dartforge_arc_collect()\n  %sobreviveu = call i8 @dartforge_arc_observe_heap_v1(i64 %ref)\n  %ainda_owned = icmp eq i8 %sobreviveu, 3\n  br i1 %ainda_owned, label %soltar_leitura, label %falha\nsoltar_leitura:\n  call void @dartforge_arc_release(i64 %ref)\n  call void @dartforge_arc_collect()");
    }
    if typed {
        // ARC uses the automatic registration of the certificate before the Dart entry.
        // Tracing registers manually only to compare the same native ABI;
        // that registration neither prepares nor certifies the tracing pipeline.
        let tracing_registration = if arc { "" } else {
            "  %pc = ptrtoint ptr @criar_objeto to i64\n  %versao = call i64 @dartforge_arc_register_typed_v1(i64 %pc, i64 1071)\n"
        };
        ir = ir.replace("  %obj = call i64 @criar_objeto()",
            &format!("{tracing_registration}  %closure = call i64 @criar_closure()\n  %contexto = call i64 @dartforge_arc_closure_context_v1(i64 %closure)\n  %obj = call i64 @invocar_closure(i64 %closure, i1 true)\n  call void @dartforge_arc_release(i64 %closure)\n  call void @dartforge_arc_collect()\n  %ec = call i8 @dartforge_arc_observe_heap_v1(i64 %contexto)\n  %contexto_morto = icmp eq i8 %ec, 0\n  br i1 %contexto_morto, label %conferir_objeto, label %falha\nconferir_objeto:"));
        if typed_error {
            let start = ir.find("define void @prova_alocacao_campos() {").ok_or("missing harness")?;
            ir.truncate(start);
            ir.push_str(&format!("define void @prova_alocacao_campos() {{\n{tracing_registration}"));
            ir.push_str(r#"
  %closure = call i64 @criar_closure()
  %contexto = call i64 @dartforge_arc_closure_context_v1(i64 %closure)
  %obj = call i64 @invocar_closure(i64 %closure, i1 false)
  %pending = call i8 @dartforge_exception_pending()
  %erro = icmp ne i8 %pending, 0
  %sem_resultado = icmp eq i64 %obj, 0
  %ok = and i1 %erro, %sem_resultado
  br i1 %ok, label %limpar, label %falha
limpar:
  call void @dartforge_exception_clear()
  call void @dartforge_arc_release(i64 %closure)
  call void @dartforge_arc_collect()
  %ec = call i8 @dartforge_arc_observe_heap_v1(i64 %contexto)
  %contexto_morto = icmp eq i8 %ec, 0
  br i1 %contexto_morto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#);
        }
    }
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
    fn allocation_and_field_preparation_generates_verified_ir() {
        for (frame, typed) in [(false, false), (true, false), (false, true)] {
            LlvmEmitter::new(&module(frame, typed).unwrap()).emit_all();
        }
    }
}
