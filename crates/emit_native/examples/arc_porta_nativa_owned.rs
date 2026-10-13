//! AOT proof of the Owned return of a body prepared through noSuchMethod.
//! The native consumer receives a borrow bound to the explicit scope, without
//! an additional retain. Does not enable the preparation of the whole SDK/pipeline nor
//! certify the automatic registration/cleanup of images and reloads. Tracing
//! runs the body with explicit tokens to check compatibility.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize, optimize::arc::*};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

fn module(tables: bool, certify: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_porta_owned".into());
    m.functions.push(Function {
        symbol: "ajudante_nsm".into(),
        name: "ajudante_nsm".into(),
        debug_info: None,
        params: (0..7)
            .map(|n| {
                (
                    ValueId(n),
                    format!("p{n}"),
                    if n == 1 { Type::I64 } else { Type::Ref },
                )
            })
            .collect(),
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(7),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(i64::MAX)),
                    from: Type::I64,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(7)))),
        }],
    });
    m.dart_scalar_parameters
        .insert("ajudante_nsm".into(), HashSet::from([ValueId(1)]));
    m.dart_ref_returns.insert("ajudante_nsm".into());
    m.helpers
        .push(("_dartforgeNoSuchMethod".into(), "ajudante_nsm".into()));
    m.functions.push(Function {
        symbol: "entrada_uniforme".into(),
        name: "entrada_uniforme".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "recv".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "desc".into(), Type::Ptr),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "ajudante_nsm".into(),
                    args: (0..7)
                        .map(|n| match n {
                            0 => Operand::Val(ValueId(0)),
                            1 => Operand::Constant(Constant::Int(0)),
                            _ => Operand::Constant(Constant::Null),
                        })
                        .collect(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        }],
    });
    m.arc_uniform_returns.insert("entrada_uniforme".into());
    m.dart_vector_parameters.insert(
        "entrada_uniforme".into(),
        HashMap::from([
            (ValueId(1), DartEntryVector::Argumentos),
            (ValueId(2), DartEntryVector::Descriptor),
        ]),
    );
    let mut fallible = m.functions[1].clone();
    fallible.symbol = "uniforme_falivel".into();
    fallible.name = "uniforme_falivel".into();
    fallible.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
    m.functions.push(fallible);
    m.arc_uniform_returns.insert("uniforme_falivel".into());
    m.dart_vector_parameters.insert(
        "uniforme_falivel".into(),
        m.dart_vector_parameters["entrada_uniforme"].clone(),
    );
    if tables {
        optimize::excecoes_por_tabelas(&mut m);
    }
    let mut plans = HashMap::from([
        ("ajudante_nsm".into(), DartFunctionPlan::default()),
        ("entrada_uniforme".into(), DartFunctionPlan::default()),
        ("uniforme_falivel".into(), DartFunctionPlan::default()),
    ]);
    if certify {
        let mut forward = m.functions[1].clone();
        forward.symbol = "repasse_seletor".into();
        forward.name = "repasse_seletor".into();
        forward.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::CallSelectorForward {
                            arc_owned: false,
                            selector: "c:ausente".into(),
                            recv: Operand::Val(ValueId(0)),
                            args: Operand::Val(ValueId(1)),
                            desc: Operand::Val(ValueId(2)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(5),
                        Instruction::ICmp(
                            ICmpOp::Ne,
                            Operand::Val(ValueId(4)),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(5)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            },
        ];
        m.functions.push(forward);
        m.arc_uniform_returns.insert("repasse_seletor".into());
        m.dart_vector_parameters.insert(
            "repasse_seletor".into(),
            m.dart_vector_parameters["entrada_uniforme"].clone(),
        );
        let mut plan = DartFunctionPlan::default();
        if tables {
            plan
                .tables
                .exits
                .insert(BlockId(1), ExceptionExit::Throws);
            m.tables.push(plan.tables.clone());
        }
        plans.insert("repasse_seletor".into(), plan.clone());
        let mut closure = m.functions.last().unwrap().clone();
        closure.symbol = "repasse_closure".into();
        closure.name = "repasse_closure".into();
        closure.blocks[0].instructions[0].1 = Instruction::CallClosureForward {
            arc_owned: false,
            closure: Operand::Val(ValueId(0)),
            args: Operand::Val(ValueId(1)),
            desc: Operand::Val(ValueId(2)),
        };
        m.functions.push(closure);
        m.arc_uniform_returns.insert("repasse_closure".into());
        m.dart_vector_parameters.insert(
            "repasse_closure".into(),
            m.dart_vector_parameters["entrada_uniforme"].clone(),
        );
        if tables {
            m.tables.push(plan.tables.clone());
        }
        plans.insert("repasse_closure".into(), plan.clone());
        let mut local = m.functions.last().unwrap().clone();
        local.symbol = "seletor_local".into();
        local.name = "seletor_local".into();
        local.blocks[0].instructions[0].1 = Instruction::CallSelector {
            selector: "c:ausente".into(), recv: Operand::Val(ValueId(0)),
            args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Int(0)),
        };
        m.functions.push(local);
        m.arc_uniform_returns.insert("seletor_local".into());
        m.dart_vector_parameters.insert("seletor_local".into(), m.dart_vector_parameters["entrada_uniforme"].clone());
        if tables { m.tables.push(plan.tables.clone()); }
        plans.insert("seletor_local".into(), plan);
    }
    prepare_arc_dart_module(&mut m, &mut plans)?;
    if plans
        .values()
        .any(|p| p.tokens.ret != TokenReturn::Owned)
    {
        return Err("corpo não possui convenção Owned".into());
    }
    let before = format!("{m:?}/{plans:?}");
    if prepare_arc_dart_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}/{plans:?}") != before
    {
        return Err("preparação não idempotente".into());
    }
    if certify && certify_arc_aot_ports(&mut m, &plans)? != 6 {
        return Err("inventário de portas certificado incompleto".into());
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") => false,
        _ => return Err("informe --memoria=arc ou --memoria=tracing".into()),
    };
    let optimize = match args.next().as_deref() {
        Some("0") => false,
        Some("2") => true,
        _ => return Err("informe 0 ou 2".into()),
    };
    let tables = match args.next().as_deref() {
        Some("checagem") => false,
        Some("tabelas") => true,
        _ => return Err("informe checagem ou tabelas".into()),
    };
    let control = args.next();
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let mut m = module(tables, arc)?;
    m.arc_memory = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // The registered callback is exactly the body verified above. The harness
    // observes dead addresses as diagnostics, without treating them as roots.
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
@nome_helper = private constant [22 x i8] c"_dartforgeNoSuchMethod"
@nome_seletor = private constant [11 x i8] c"c:inexiste!"
@desc_nsm = private constant [2 x i64] [i64 0, i64 0]
@args_nsm = private constant [1 x i64] zeroinitializer
@cache_nsm = private global [2 x i64] zeroinitializer

define void @prova_porta_owned() {
  call void @dartforge_registrar_ajudante(ptr @nome_helper, i64 22, ptr @ajudante_nsm)
  %codigo = ptrtoint ptr @ajudante_nsm to i64
  %versao = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %codigo, i64 7)
  %escopo = call i64 @dartforge_arc_escopo_nativo_abrir_v1()
  %recv = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775806)
  %f = call ptr @dartforge_seletor(ptr @cache_nsm, i64 %recv, i64 12345, ptr @nome_seletor, i64 11)
  %r = call i64 %f(i64 %recv, ptr @args_nsm, ptr @desc_nsm)
  %pendente = call i8 @dartforge_exception_pending()
  %normal = icmp eq i8 %pendente, 0
  br i1 %normal, label %verificar, label %falha
verificar:
  call void @dartforge_arc_collect()
  %vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %r)
  %valor = call i64 @dartforge_unbox_int(i64 %r)
  %ok_owner = icmp eq i8 %vivo, 3
  %ok_valor = icmp eq i64 %valor, 9223372036854775807
  %ok = and i1 %ok_owner, %ok_valor
  br i1 %ok, label %fechar, label %falha
fechar:
  call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %escopo)
  call void @dartforge_arc_release(i64 %recv)
  call void @dartforge_arc_collect()
  %final = call i8 @dartforge_arc_observar_heap_v1(i64 %r)
  %morto = icmp eq i8 %final, 0
  br i1 %morto, label %sucesso, label %falha
falha:
  call void @llvm.trap()
  unreachable
sucesso:
  %codigo_uniforme = ptrtoint ptr @entrada_uniforme to i64
  %versao_uniforme = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_uniforme)
  %args_ptr = ptrtoint ptr @args_nsm to i64
  %desc_ptr = ptrtoint ptr @desc_nsm to i64
  %ru = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_uniforme, i64 0, i64 %args_ptr, i64 %desc_ptr)
  call void @dartforge_arc_collect()
  %uniforme_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %ru)
  %uniforme_valor = call i64 @dartforge_unbox_int(i64 %ru)
  %uniforme_ok_owner = icmp eq i8 %uniforme_vivo, 3
  %uniforme_ok_valor = icmp eq i64 %uniforme_valor, 9223372036854775807
  %uniforme_ok = and i1 %uniforme_ok_owner, %uniforme_ok_valor
  br i1 %uniforme_ok, label %liberar_uniforme, label %falha
liberar_uniforme:
  call void @dartforge_arc_release(i64 %ru)
  call void @dartforge_arc_collect()
  %uniforme_final = call i8 @dartforge_arc_observar_heap_v1(i64 %ru)
  %uniforme_morto = icmp eq i8 %uniforme_final, 0
  br i1 %uniforme_morto, label %sucesso_uniforme, label %falha
sucesso_uniforme:
  %codigo_fallback = ptrtoint ptr %f to i64
  %rf = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_fallback, i64 0, i64 %args_ptr, i64 %desc_ptr)
  call void @dartforge_arc_collect()
  %fallback_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %rf)
  %fallback_valor = call i64 @dartforge_unbox_int(i64 %rf)
  %fallback_ok_owner = icmp eq i8 %fallback_vivo, 3
  %fallback_ok_valor = icmp eq i64 %fallback_valor, 9223372036854775807
  %fallback_ok = and i1 %fallback_ok_owner, %fallback_ok_valor
  br i1 %fallback_ok, label %liberar_fallback, label %falha
liberar_fallback:
  call void @dartforge_arc_release(i64 %rf)
  call void @dartforge_arc_collect()
  %fallback_final = call i8 @dartforge_arc_observar_heap_v1(i64 %rf)
  %fallback_morto = icmp eq i8 %fallback_final, 0
  br i1 %fallback_morto, label %sucesso_fallback, label %falha
sucesso_fallback:
  %codigo_falivel = ptrtoint ptr @uniforme_falivel to i64
  %versao_falivel = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_falivel)
  %xp = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775806)
  %rp = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_falivel, i64 %xp, i64 %args_ptr, i64 %desc_ptr)
  %pending_falivel = call i8 @dartforge_exception_pending()
  %payload = call i64 @dartforge_arc_excecao_owned_v1()
  call void @dartforge_arc_release(i64 %xp)
  call void @dartforge_arc_collect()
  %payload_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %payload)
  %payload_ok_owner = icmp eq i8 %payload_vivo, 3
  %payload_identico = icmp eq i64 %payload, %xp
  %payload_ok = and i1 %payload_ok_owner, %payload_identico
  %falivel_null = icmp eq i64 %rp, 0
  %falivel_pending = icmp ne i8 %pending_falivel, 0
  %pending_ok = and i1 %falivel_null, %falivel_pending
  %falivel_ok = and i1 %pending_ok, %payload_ok
  br i1 %falivel_ok, label %sucesso_pending, label %falha
sucesso_pending:
  call void @dartforge_exception_clear()
  call void @dartforge_arc_release(i64 %payload)
  call void @dartforge_arc_collect()
  %payload_final = call i8 @dartforge_arc_observar_heap_v1(i64 %payload)
  %payload_morto = icmp eq i8 %payload_final, 0
  br i1 %payload_morto, label %sucesso_excecao, label %falha
sucesso_excecao:
  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_falivel, i64 %versao_falivel)
  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_uniforme, i64 %versao_uniforme)
  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo, i64 %versao)
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    if arc {
        // Exercises the real lookup by selector emitted by the compiler: the fallback
        // transfers its single owner through the adapter and the Dart body.
        ir = ir.replace("sucesso_excecao:\n", r#"sucesso_excecao:
  %codigo_repasse = ptrtoint ptr @repasse_seletor to i64
  %rs = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_repasse, i64 0, i64 %args_ptr, i64 %desc_ptr)
  call void @dartforge_arc_collect()
  %repasse_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %rs)
  %repasse_valor = call i64 @dartforge_unbox_int(i64 %rs)
  %repasse_owner_ok = icmp eq i8 %repasse_vivo, 3
  %repasse_valor_ok = icmp eq i64 %repasse_valor, 9223372036854775807
  %repasse_ok = and i1 %repasse_owner_ok, %repasse_valor_ok
  br i1 %repasse_ok, label %liberar_repasse, label %falha
liberar_repasse:
  call void @dartforge_arc_release(i64 %rs)
  call void @dartforge_arc_collect()
  %repasse_final = call i8 @dartforge_arc_observar_heap_v1(i64 %rs)
  %repasse_morto = icmp eq i8 %repasse_final, 0
  br i1 %repasse_morto, label %repasse_concluido, label %falha
repasse_concluido:
  %closure = call i64 @dartforge_arc_closure_owned_v1(i64 %codigo_uniforme, i64 0, i64 0, i64 0)
  %codigo_closure = ptrtoint ptr @repasse_closure to i64
  %rc = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_closure, i64 %closure, i64 %args_ptr, i64 %desc_ptr)
  call void @dartforge_arc_release(i64 %closure)
  call void @dartforge_arc_collect()
  %closure_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %rc)
  %closure_valor = call i64 @dartforge_unbox_int(i64 %rc)
  %closure_owner_ok = icmp eq i8 %closure_vivo, 3
  %closure_valor_ok = icmp eq i64 %closure_valor, 9223372036854775807
  %closure_ok = and i1 %closure_owner_ok, %closure_valor_ok
  br i1 %closure_ok, label %liberar_closure, label %falha
liberar_closure:
  call void @dartforge_arc_release(i64 %rc)
  call void @dartforge_arc_collect()
  %closure_final = call i8 @dartforge_arc_observar_heap_v1(i64 %rc)
  %closure_morto = icmp eq i8 %closure_final, 0
  br i1 %closure_morto, label %closure_concluida, label %falha
closure_concluida:
  %codigo_local = ptrtoint ptr @seletor_local to i64
  %rl = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %codigo_local, i64 0, i64 %args_ptr, i64 %desc_ptr)
  call void @dartforge_arc_collect()
  %local_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %rl)
  %local_valor = call i64 @dartforge_unbox_int(i64 %rl)
  %local_owner_ok = icmp eq i8 %local_vivo, 3
  %local_valor_ok = icmp eq i64 %local_valor, 9223372036854775807
  %local_ok = and i1 %local_owner_ok, %local_valor_ok
  br i1 %local_ok, label %liberar_local, label %falha
liberar_local:
  call void @dartforge_arc_release(i64 %rl)
  call void @dartforge_arc_collect()
  %local_final = call i8 @dartforge_arc_observar_heap_v1(i64 %rl)
  %local_morto = icmp eq i8 %local_final, 0
  br i1 %local_morto, label %local_concluido, label %falha
local_concluido:
"#);
    }
    if arc {
        // The emitter registers the certified bodies at the entry of the image.
        // Do not repeat the registrations in the harness: that would hide a registration
        // missing at startup. The fixed image keeps the versions until exit.
        for line in [
            "  %versao = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %codigo, i64 7)\n",
            "  %versao_uniforme = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_uniforme)\n",
            "  %versao_falivel = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_falivel)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_falivel, i64 %versao_falivel)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_uniforme, i64 %versao_uniforme)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo, i64 %versao)\n",
        ] {
            ir = ir.replace(line, "");
        }
    }
    match control.as_deref() {
        None => {}
        Some("sem-registro") => {
            if arc {
                ir = ir.replace(
                    "  %cv0 = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %c0, i64 7)\n",
                    "",
                );
            } else {
                ir = ir.replace("  %versao = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %codigo, i64 7)\n", "  %versao = add i64 0, 0\n");
            }
        }
        Some("sem-fechamento") => {
            ir = ir.replace(
                "  call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %escopo)\n",
                "",
            );
        }
        Some(other) => return Err(format!("controle desconhecido: {other}")),
    }
    if let Some(p) = exit.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(exit.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &exit,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_body_has_owned_factory_and_does_not_retain_return_twice() {
        for tables in [false, true] {
            let m = module(tables, true).unwrap();
            let ir = LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @dartforge_arc_box_int_owned_v1"));
            assert!(ir.contains("call void @df.arc.registrar_portas()"));
            assert!(
                !m.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
        }
    }
}
