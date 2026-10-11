//! Prova AOT de retorno Owned de um corpo preparado através de noSuchMethod.
//! O consumidor nativo recebe um empréstimo ligado ao escopo explícito, sem
//! retain adicional. Não habilita a preparação de todo o SDK/pipeline nem
//! certifica o registro/cleanup automático de imagens e recargas. Tracing
//! executa o corpo com tokens explícitos para conferir compatibilidade.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar, otimizar::arc::*};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

fn modulo(tabelas: bool, certificar: bool) -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_porta_owned".into());
    m.functions.push(Function {
        symbol: "ajudante_nsm".into(),
        name: "ajudante_nsm".into(),
        depuracao: None,
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
    m.parametros_escalares_dart
        .insert("ajudante_nsm".into(), HashSet::from([ValueId(1)]));
    m.retornos_ref_dart.insert("ajudante_nsm".into());
    m.ajudantes
        .push(("_dartforgeNoSuchMethod".into(), "ajudante_nsm".into()));
    m.functions.push(Function {
        symbol: "entrada_uniforme".into(),
        name: "entrada_uniforme".into(),
        depuracao: None,
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
    m.retornos_uniformes_arc.insert("entrada_uniforme".into());
    m.parametros_vetores_dart.insert(
        "entrada_uniforme".into(),
        HashMap::from([
            (ValueId(1), VetorEntradaDart::Argumentos),
            (ValueId(2), VetorEntradaDart::Descritor),
        ]),
    );
    let mut falivel = m.functions[1].clone();
    falivel.symbol = "uniforme_falivel".into();
    falivel.name = "uniforme_falivel".into();
    falivel.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
    m.functions.push(falivel);
    m.retornos_uniformes_arc.insert("uniforme_falivel".into());
    m.parametros_vetores_dart.insert(
        "uniforme_falivel".into(),
        m.parametros_vetores_dart["entrada_uniforme"].clone(),
    );
    if tabelas {
        otimizar::excecoes_por_tabelas(&mut m);
    }
    let mut planos = HashMap::from([
        ("ajudante_nsm".into(), PlanoFuncaoDart::default()),
        ("entrada_uniforme".into(), PlanoFuncaoDart::default()),
        ("uniforme_falivel".into(), PlanoFuncaoDart::default()),
    ]);
    if certificar {
        let mut repasse = m.functions[1].clone();
        repasse.symbol = "repasse_seletor".into();
        repasse.name = "repasse_seletor".into();
        repasse.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::CallSeletorRepasse {
                            arc_owned: false,
                            seletor: "c:ausente".into(),
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
        m.functions.push(repasse);
        m.retornos_uniformes_arc.insert("repasse_seletor".into());
        m.parametros_vetores_dart.insert(
            "repasse_seletor".into(),
            m.parametros_vetores_dart["entrada_uniforme"].clone(),
        );
        let mut plano = PlanoFuncaoDart::default();
        if tabelas {
            plano
                .tabelas
                .saidas
                .insert(BlockId(1), SaidaPorExcecao::Lanca);
            m.tabelas.push(plano.tabelas.clone());
        }
        planos.insert("repasse_seletor".into(), plano.clone());
        let mut closure = m.functions.last().unwrap().clone();
        closure.symbol = "repasse_closure".into();
        closure.name = "repasse_closure".into();
        closure.blocks[0].instructions[0].1 = Instruction::CallClosureRepasse {
            arc_owned: false,
            closure: Operand::Val(ValueId(0)),
            args: Operand::Val(ValueId(1)),
            desc: Operand::Val(ValueId(2)),
        };
        m.functions.push(closure);
        m.retornos_uniformes_arc.insert("repasse_closure".into());
        m.parametros_vetores_dart.insert(
            "repasse_closure".into(),
            m.parametros_vetores_dart["entrada_uniforme"].clone(),
        );
        if tabelas {
            m.tabelas.push(plano.tabelas.clone());
        }
        planos.insert("repasse_closure".into(), plano);
    }
    preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if planos
        .values()
        .any(|p| p.tokens.retorno != RetornoTokens::Owned)
    {
        return Err("corpo não possui convenção Owned".into());
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
    {
        return Err("preparação não idempotente".into());
    }
    if certificar && certificar_portas_arc_aot(&mut m, &planos)? != 5 {
        return Err("inventário de portas certificado incompleto".into());
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
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
    let tabelas = match args.next().as_deref() {
        Some("checagem") => false,
        Some("tabelas") => true,
        _ => return Err("informe checagem ou tabelas".into()),
    };
    let controle = args.next();
    if args.next().is_some() {
        return Err("argumentos excedentes".into());
    }
    let mut m = modulo(tabelas, arc)?;
    m.memoria_arc = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // O callback registrado é exatamente o corpo verificado acima. O harness
    // observa endereços mortos como diagnósticos, sem os tratar como raízes.
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
        // Exercita a busca real por seletor emitida pelo compilador: o fallback
        // transfere seu único owner através do adaptador e do corpo Dart.
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
"#);
    }
    if arc {
        // O emissor registra os corpos certificados na entrada da imagem.
        // Não repetir os registros no harness: isso esconderia um registro
        // ausente na partida. A imagem fixa conserva as versões até o exit.
        for linha in [
            "  %versao = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %codigo, i64 7)\n",
            "  %versao_uniforme = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_uniforme)\n",
            "  %versao_falivel = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %codigo_falivel)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_falivel, i64 %versao_falivel)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo_uniforme, i64 %versao_uniforme)\n",
            "  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo, i64 %versao)\n",
        ] {
            ir = ir.replace(linha, "");
        }
    }
    match controle.as_deref() {
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
        Some(outro) => return Err(format!("controle desconhecido: {outro}")),
    }
    if let Some(p) = saida.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )
    .map(|_| ())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn corpo_do_callback_tem_fabrica_owned_e_nao_retem_o_retorno_duas_vezes() {
        for tabelas in [false, true] {
            let m = modulo(tabelas, true).unwrap();
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
