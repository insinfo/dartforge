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

fn modulo(tabelas: bool) -> Result<Module, String> {
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
    if tabelas {
        otimizar::excecoes_por_tabelas(&mut m);
    }
    let mut planos = HashMap::from([("ajudante_nsm".into(), PlanoFuncaoDart::default())]);
    preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if planos["ajudante_nsm"].tokens.retorno != RetornoTokens::Owned {
        return Err("corpo não possui convenção Owned".into());
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
    {
        return Err("preparação não idempotente".into());
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
    let mut m = modulo(tabelas)?;
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
  call void @dartforge_arc_retirar_retorno_owned_v1(i64 %codigo, i64 %versao)
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#,
    );
    match controle.as_deref() {
        None => {}
        Some("sem-registro") => {
            ir = ir.replace("  %versao = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %codigo, i64 7)\n", "  %versao = add i64 0, 0\n");
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
            let m = modulo(tabelas).unwrap();
            let ir = LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @dartforge_arc_box_int_owned_v1"));
            assert!(!m.functions[0]
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. })));
        }
    }
}
