//! AOT proof of the preparation of the zeroed factory used by the instance lowering.
//! Checks the Owned token, method registration, call by selector and final death.
//! The construction is an explicit HIR fixture; it does not certify general constructors.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn module() -> Result<Module, String> {
    let mut m = Module::new();
    m.arc_memory = true;
    m.sdk_mode = true;
    m.sdk_library = true;
    m.entry_symbol = Some("prova_instancia_zerada".into());
    // Emits the thunk through the real tables protocol. The registration of this fixture
    // is not called at startup: the factory must perform the lazy registration.
    m.record = Some("df.reg.C".into());
    m.classes.push(ClassDef {
        id: 32001,
        name: "C".into(),
        field_count: 2,
        vtable: vec![],
        to_string_symbol: None,
    });
    m.arc_field_layouts
        .insert(32001, vec![Type::I64, Type::Ref]);
    m.table_functions.insert(32001, "df.mt.C".into());
    m.method_tables.push((
        32001,
        "df.mt.C".into(),
        vec![("c:valor".into(), "metodo".into())],
    ));
    m.functions.push(Function {
        symbol: "criar".into(),
        name: "criar".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::CallRuntime {
                    name: "dartforge_object_new".into(),
                    args: vec![
                        (Operand::Constant(Constant::Int(32001)), Type::I64),
                        (Operand::Constant(Constant::Int(2)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    });
    m.functions.push(Function {
        symbol: "metodo".into(),
        name: "metodo".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "desc".into(), Type::Ptr),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(3),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(1)),
                    from: Type::I64,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        }],
    });
    // The receiver of the constructor is the physical instance of the factory above. The
    // first setter must be audited before delegating to the base body.
    m.functions.push(Function {
        symbol: "receita".into(), name: "receita".into(), debug_info: None, params: vec![], return_ty: Type::I64,
        blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(0), Instruction::Const(Constant::String("C32001".into())), Type::Ref),
            (ValueId(1), Instruction::CallRuntime { name: "dartforge_rti_receita".into(),
                args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
        ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }],
    });
    m.functions[0].blocks[0].instructions.extend([
        (ValueId(3), Instruction::CallStatic { symbol: "receita".into(), args: vec![], ret_ty: Type::I64 }, Type::I64),
        (ValueId(4), Instruction::CallRuntime { name: "dartforge_rti_avaliar".into(), args: vec![
            (Operand::Val(ValueId(3)), Type::I64), (Operand::Constant(Constant::Null), Type::Ref),
            (Operand::Constant(Constant::Int(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64),
        ], ret_ty: Type::I64 }, Type::I64),
        (ValueId(5), Instruction::CallRuntime { name: "dartforge_rti_definir".into(), args: vec![
            (Operand::Val(ValueId(0)), Type::Ref), (Operand::Val(ValueId(4)), Type::I64),
        ], ret_ty: Type::Void }, Type::Void),
        (ValueId(6), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
            value: Operand::Constant(Constant::Int(42)) }, Type::Void),
    ]);
    m.functions[0].blocks[0].instructions.push((
        ValueId(2),
        Instruction::Box {
            op: Operand::Constant(Constant::Int(i64::MAX)),
            from: Type::I64,
        },
        Type::Ref,
    ));
    m.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallStatic {
            symbol: "construir".into(),
            args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(2))],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    m.functions[0].blocks[0].instructions.push((ValueId(8), Instruction::SetField {
        object: Operand::Val(ValueId(0)), index: 1, value: Operand::Val(ValueId(2)),
    }, Type::Void));
    // The owner crosses the call and the normal successor; both fields must
    // check the current layout, instead of reusing freshness from the previous block.
    let start = m.functions[0].blocks[0].instructions.iter().position(|(v, _, _)| *v == ValueId(6)).unwrap();
    let later = m.functions[0].blocks[0].instructions.split_off(start);
    m.functions[0].blocks[0].instructions.push((ValueId(7), Instruction::CallStatic {
        symbol: "base".into(), args: vec![Operand::Val(ValueId(0))], ret_ty: Type::Void,
    }, Type::Void));
    m.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(1));
    m.functions[0].blocks.push(BasicBlock { id: BlockId(1), instructions: later,
        terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) });
    m.functions.push(Function {
        symbol: "construir".into(),
        name: "construir".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "filho".into(), Type::Ref),
        ],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                        value: Operand::Constant(Constant::Int(42)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(0)),
                        index: 1,
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::CallStatic {
                        symbol: "base".into(),
                        args: vec![Operand::Val(ValueId(0))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    m.functions.push(Function {
        symbol: "base".into(),
        name: "base".into(),
        debug_info: None,
        params: vec![(ValueId(0), "this".into(), Type::Ref)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(None),
        }],
    });
    let mut ret = DartFunctionPlan::default();
    ret.tokens.ret = TokenReturn::Owned;
    let mut method = ret.clone();
    // Native addresses of the uniform ABI, without handles or owner slots.
    method.classes.extend([
        (ValueId(1), Ownership::Trivial),
        (ValueId(2), Ownership::Trivial),
    ]);
    let mut plans = HashMap::from([
        ("criar".into(), ret),
        ("metodo".into(), method),
        ("construir".into(), DartFunctionPlan::default()),
        ("base".into(), DartFunctionPlan::default()),
        ("receita".into(), DartFunctionPlan::default()),
    ]);
    prepare_arc_dart_module(&mut m, &mut plans)?;
    let before = format!("{m:?}{plans:?}");
    if prepare_arc_dart_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}{plans:?}") != before
    {
        return Err("non-idempotent preparation".into());
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("provide the executable")?);
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
    let mut m = module()?;
    // Tracing uses explicit tokens only in this comparison harness.
    m.arc_memory = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    let factory = "call i64 @dartforge_arc_objeto_owned_t_v1(i64 32001, i64 2, ptr @df.mt.C)";
    if ir.matches(factory).count() != 1 {
        return Err("factory with a missing/ambiguous table".into());
    }
    ir.push_str(r#"
@cache.prova = private global [2 x i64] zeroinitializer
@nome.prova = private constant [7 x i8] c"c:valor"
declare void @dartforge_print_handle(i64)
define void @prova_instancia_zerada() {
  %obj = call i64 @criar()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %campo, label %falha
campo:
  %campo_atual = call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %obj, i64 0)
  %construido = icmp eq i64 %campo_atual, 42
  br i1 %construido, label %filho, label %falha
filho:
  %hfilho = call i64 @dartforge_arc_ler_campo_ref_v1(i64 %obj, i64 1)
  call void @dartforge_arc_collect()
  %estado_filho = call i8 @dartforge_arc_observar_heap_v1(i64 %hfilho)
  %valor_filho = call i64 @dartforge_unbox_int(i64 %hfilho)
  %filho_sem_owner_codigo = icmp eq i8 %estado_filho, 1
  %filho_valor_ok = icmp eq i64 %valor_filho, 9223372036854775807
  %filho_ok = and i1 %filho_sem_owner_codigo, %filho_valor_ok
  br i1 %filho_ok, label %seletor, label %falha
seletor:
  %ptr = call ptr @dartforge_seletor(ptr @cache.prova, i64 %obj, i64 HASH_PROVA, ptr @nome.prova, i64 7)
  %tabela = icmp eq ptr %ptr, @metodo
  br i1 %tabela, label %chamar, label %falha
chamar:
  %valor = call i64 %ptr(i64 %obj, ptr null, ptr null)
  %um = icmp eq i64 %valor, 3
  br i1 %um, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %fim = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %morto = icmp eq i8 %fim, 0
  %fim_filho = call i8 @dartforge_arc_observar_heap_v1(i64 %hfilho)
  %filho_morto = icmp eq i8 %fim_filho, 0
  %ambos_mortos = and i1 %morto, %filho_morto
  br i1 %ambos_mortos, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#);
    let hash = b"c:valor".iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    }) as i64;
    ir = ir.replace("HASH_PROVA", &hash.to_string());
    // Emits as a library so that the startup registration does not anticipate the
    // factory. The harness entry links only the requested policy.
    ir.push_str("\ndefine void @dartforge_entry() {\n");
    if arc {
        ir.push_str("  call void @dartforge_memoria_arc_v1()\n");
    }
    ir.push_str("  call void @prova_instancia_zerada()\n  ret void\n}\n");
    ir.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n  ret i64 0\n}\n");
    if let Some(control) = args.next() {
        let (before, after) = match control.as_str() {
            "sem-owner" => (
                factory,
                "call i64 @dartforge_object_new_t(i64 32001, i64 2, ptr @df.mt.C)",
            ),
            "sem-tabela" => (
                factory,
                "call i64 @dartforge_arc_objeto_owned_v1(i64 32001, i64 2)",
            ),
            "sem-drop" => ("  call void @dartforge_arc_release(i64 %obj)\n", ""),
            "layout-obsoleto" => (
                "constant [2 x i64] [i64 32001, i64 2]",
                "constant [2 x i64] [i64 32001, i64 3]",
            ),
            _ => return Err(format!("unknown control: {control}")),
        };
        if ir.matches(before).count() != 1 {
            return Err("control without a single target".into());
        }
        ir = ir.replace(before, after);
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
    fn zeroed_instance_generates_verified_ir_with_owned_table() {
        let ir = LlvmEmitter::new(&module().unwrap()).emit_all();
        assert!(ir.contains("call i64 @dartforge_arc_objeto_owned_t_v1"));
        assert!(ir.contains("define ptr @df.mt.C()"));
        assert!(ir.contains("call void @dartforge_arc_gravar_campo_escalar_conferido_v1"));
        assert!(ir.contains("call void @dartforge_arc_gravar_campo_ref_conferido_v1"));
    }
}
