//! Prova AOT de keepalive produzido antes de substituir uma aresta de campo.
//! A HIR verificada lê BorrowArg e devolve Owned após retirar a aresta forte.
//! O harness LLVM observa os endereços sem raízes, depois que a função saiu.
//! Em tracing, o mesmo corpo com tokens explícitos testa compatibilidade do
//! protocolo runtime; não ativa preparação ARC no pipeline padrão tracing.
//! Não certifica lowering de fonte, guardas pending, recarga ou suspensão.
//! Controles negativos `sem-retencao` e `sem-liberar` alteram somente o IR
//! final do harness, depois da conferência; a execução deve falhar.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo() -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_keepalive".into());
    let val = |n| Operand::Val(ValueId(n));
    m.functions.push(Function {
        symbol: "ler_e_substituir".into(),
        name: "ler_e_substituir".into(),
        depuracao: None,
        params: vec![(ValueId(0), "receiver".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_ler_campo_ref_v1".into(),
                        args: vec![
                            (val(0), Type::Ref),
                            (Operand::Constant(Constant::Int(39)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_gravar_campo_ref_v1".into(),
                        args: vec![
                            (val(0), Type::Ref),
                            (Operand::Constant(Constant::Int(39)), Type::I64),
                            (Operand::Constant(Constant::Null), Type::Ref),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(val(1))),
        }],
    });
    let mut plano = PlanoFuncaoDart::default();
    plano.tokens.retorno = RetornoTokens::Owned;
    let mut planos = HashMap::from([("ler_e_substituir".into(), plano)]);
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if produzido != (1, 0) || planos["ler_e_substituir"].classes[&ValueId(1)] != Ownership::Owned {
        return Err(format!(
            "keepalive esperado (1, 0), encontrado {produzido:?}"
        ));
    }
    let preparado = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != preparado
    {
        return Err("preparação de keepalive não foi idempotente".into());
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(outro) => return Err(format!("modo desconhecido: {outro}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(outro) => return Err(format!("nível desconhecido: {outro}")),
    };
    let controle = args.next();
    let mut m = modulo()?;
    m.memoria_arc = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // Nenhum Ref observado passa pela HIR do harness: um endereço já morto
    // é apenas diagnóstico nativo. Não há alocação entre morte e observação,
    // evitando confundir reutilização de endereço com sobrevivência.
    ir.push_str(
        r#"
declare void @dartforge_print_handle(i64)
define void @prova_keepalive() {
  %obj = call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)
  %filho = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  call void @dartforge_arc_gravar_campo_ref_v1(i64 %obj, i64 39, i64 %filho)
  call void @dartforge_arc_release(i64 %filho)
  %aresta = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %ok_aresta = icmp eq i8 %aresta, 1
  br i1 %ok_aresta, label %ler, label %falha
ler:
  %retorno = call i64 @ler_e_substituir(i64 %obj)
  %mesmo = icmp eq i64 %retorno, %filho
  br i1 %mesmo, label %coletar, label %falha
coletar:
  call void @dartforge_arc_release(i64 %obj)
  call void @dartforge_arc_collect()
  %obj_morto = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %filho_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %ok_obj = icmp eq i8 %obj_morto, 0
  %ok_filho = icmp eq i8 %filho_vivo, 3
  %ok = and i1 %ok_obj, %ok_filho
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %retorno)
  call void @dartforge_arc_collect()
  %final = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %morto = icmp eq i8 %final, 0
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
    match controle.as_deref() {
        None => {}
        Some("sem-retencao") => {
            let emprestado = m.functions[0].blocks[0].instructions[0].0.0;
            let chamada = format!("  call void @dartforge_arc_retain(i64 %v{emprestado})\n");
            if ir.matches(&chamada).count() != 1 {
                return Err("retenção produzida não foi localizada unicamente no IR".into());
            }
            ir = ir.replace(&chamada, "");
        }
        Some("sem-liberar") => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %retorno)\n", "");
        }
        Some(outro) => return Err(format!("controle desconhecido: {outro}")),
    }
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize,
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn retencao_produzida_antecede_substituicao_da_aresta() {
        let m = modulo().unwrap();
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(
            ops[1],
            (ValueId(1), Instruction::ArcCopy { .. }, Type::Ref)
        ));
        let ir = LlvmEmitter::new(&m).emit_all();
        assert!(
            ir.find("call void @dartforge_arc_retain").unwrap()
                < ir.find("call void @dartforge_arc_gravar_campo_ref_v1")
                    .unwrap()
        );
    }
}
