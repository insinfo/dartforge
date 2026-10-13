//! Prova AOT de keepalive e cleanup ao redor de uma guarda pending.
//! Usa o getter auditado de record e uma pendência real pré-carregada no
//! caminho de erro; o getter não lança por si mesmo e o catálogo o declara
//! sem pendência (efeitos `0 0 0`, conferidos), então a guarda é um desvio
//! comum sobre a flag: a cópia Owned do resultado vem logo após o getter e
//! o caminho de erro solta o receiver e a cópia. Não confundir esta prova
//! do desvio com uma exceção originada dentro do getter.
//! O harness LLVM prepara o record, observa endereços sem raízes e confere
//! identidade da exceção, sobrevivência Owned e morte após a última liberação.
//! Tracing executa o mesmo corpo com tokens explícitos, sem mudar seu fluxo
//! padrão. Não prova lowering de fonte, suspensão, recarga ou unwinding.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, optimize::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn module() -> Result<Module, String> {
    let mut m = Module::new();
    m.arc_memory = true;
    m.entry_symbol = Some("prova_pending".into());
    let val = |n| Operand::Val(ValueId(n));
    m.functions.push(Function {
        symbol: "ler_record".into(),
        name: "ler_record".into(),
        debug_info: None,
        params: vec![(ValueId(0), "record".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy { value: val(0) },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallRuntime {
                            name: "dartforge_nativo_DartForge_record_fieldAt".into(),
                            args: vec![
                                (val(1), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(4),
                        Instruction::ICmp(ICmpOp::Ne, val(3), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(4),
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
                instructions: vec![(
                    ValueId(5),
                    Instruction::ArcDrop { value: val(1) },
                    Type::Void,
                )],
                terminator: Terminator::Return(Some(val(2))),
            },
        ],
    });
    let mut plan = DartFunctionPlan::default();
    plan.tokens.ret = TokenReturn::Owned;
    let mut plans = HashMap::from([("ler_record".into(), plan)]);
    let produced = prepare_arc_dart_module(&mut m, &mut plans)?;
    if produced != (1, 2) || plans["ler_record"].classes[&ValueId(2)] != Ownership::Owned {
        return Err(format!(
            "keepalive/cleanup esperado (1, 2), encontrado {produced:?}"
        ));
    }
    let prepared = format!("{m:?}/{plans:?}");
    if prepare_arc_dart_module(&mut m, &mut plans)? != (0, 0)
        || format!("{m:?}/{plans:?}") != prepared
    {
        return Err("preparação pending não foi idempotente".into());
    }
    let errors = dartforge_emit_native::lower::verifier::verify(&m);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let exit = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = match args.next().as_deref() {
        Some("--memoria=arc") => true,
        Some("--memoria=tracing") | None => false,
        Some(other) => return Err(format!("modo desconhecido: {other}")),
    };
    let optimize = match args.next().as_deref() {
        Some("2") => true,
        Some("0") | None => false,
        Some(other) => return Err(format!("nível desconhecido: {other}")),
    };
    let error = match args.next().as_deref() {
        Some("erro") => true,
        Some("normal") | None => false,
        Some(other) => return Err(format!("caminho desconhecido: {other}")),
    };
    let control = args.next();
    let mut m = module()?;
    m.arc_memory = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // O record legado pertence somente ao harness: retain imediatamente
    // após sua construção, sem alocar antes da publicação do owner. A prova
    // não atribui um contrato Owned auditado ao alocador legado no lowering.
    ir.push_str(
        &r#"
declare void @dartforge_print_handle(i64)
define void @prova_pending() {
  %campos = alloca i64
  %filho = call i64 @dartforge_arc_box_int_owned_v1(i64 9223372036854775807)
  store i64 %filho, ptr %campos
  %record = call i64 @dartforge_record_novo(ptr %campos, i64 1)
  call void @dartforge_arc_retain(i64 %record)
  call void @dartforge_arc_release(i64 %filho)
  br i1 __ERRO__, label %preparar_erro, label %ler
preparar_erro:
  call void @dartforge_arc_lancar_ref_v1(i64 %filho)
  br label %ler
ler:
  %retorno = call i64 @ler_record(i64 %record)
  %pending = call i8 @dartforge_exception_pending()
  br i1 __ERRO__, label %erro, label %normal
normal:
  %identico = icmp eq i64 %retorno, %filho
  %sem_erro = icmp eq i8 %pending, 0
  %ok_normal = and i1 %identico, %sem_erro
  br i1 %ok_normal, label %coletar_normal, label %falha
coletar_normal:
  call void @dartforge_arc_release(i64 %record)
  call void @dartforge_arc_collect()
  %record_morto = call i8 @dartforge_arc_observar_heap_v1(i64 %record)
  %filho_vivo = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %ok_record = icmp eq i8 %record_morto, 0
  %ok_filho = icmp eq i8 %filho_vivo, 3
  %ok = and i1 %ok_record, %ok_filho
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %retorno)
  call void @dartforge_arc_collect()
  %final = call i8 @dartforge_arc_observar_heap_v1(i64 %retorno)
  %morto = icmp eq i8 %final, 0
  br i1 %morto, label %passou, label %falha
erro:
  %ret_null = icmp eq i64 %retorno, 0
  %com_erro = icmp ne i8 %pending, 0
  %sem_owner = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %ok_borrow = icmp eq i8 %sem_owner, 1
  %ok_erro1 = and i1 %ret_null, %com_erro
  %ok_erro = and i1 %ok_erro1, %ok_borrow
  br i1 %ok_erro, label %conferir_excecao, label %falha
conferir_excecao:
  %capturada = call i64 @dartforge_arc_excecao_owned_v1()
  %mesma_excecao = icmp eq i64 %capturada, %filho
  br i1 %mesma_excecao, label %limpar_erro, label %falha
limpar_erro:
  call void @dartforge_arc_release(i64 %capturada)
  call void @dartforge_exception_clear()
  call void @dartforge_arc_release(i64 %record)
  call void @dartforge_arc_collect()
  %erro_record = call i8 @dartforge_arc_observar_heap_v1(i64 %record)
  %erro_filho = call i8 @dartforge_arc_observar_heap_v1(i64 %filho)
  %erro_pending = call i8 @dartforge_exception_pending()
  %record_zero = icmp eq i8 %erro_record, 0
  %filho_zero = icmp eq i8 %erro_filho, 0
  %pending_zero = icmp eq i8 %erro_pending, 0
  %mortos = and i1 %record_zero, %filho_zero
  %limpo = and i1 %mortos, %pending_zero
  br i1 %limpo, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  call void @dartforge_print_handle(i64 3)
  ret void
}
"#
        .replace("__ERRO__", if error { "true" } else { "false" }),
    );
    let call = "  call void @dartforge_arc_retain(i64 %v6)\n";
    if ir.matches(call).count() != 1 {
        return Err("cópia na aresta normal ausente no IR".into());
    }
    match control.as_deref() {
        None => {}
        Some("sem-retencao") if !error => {
            ir = ir.replace(call, "");
        }
        Some("sem-liberar") if !error => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %retorno)\n", "");
        }
        Some("copia-no-erro") if error => {
            // O emissor lê a flag do Contexto em linha neste modo.
            let guard = "  %v3 = load i8, ptr %ctx, align 8\n";
            if ir.matches(guard).count() != 1 {
                return Err("guarda pending ausente no IR".into());
            }
            ir = ir.replace(guard, &format!("{call}{guard}"));
        }
        Some(other) => return Err(format!("controle/caminho incompatível: {other}")),
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
    fn copy_follows_getter_and_error_releases_receiver_and_copy() {
        let m = module().unwrap();
        let f = &m.functions[0];
        let entry = &f.blocks[0].instructions;
        let getter = entry.iter().position(|(v, _, _)| *v == ValueId(6)).unwrap();
        assert!(matches!(
            entry[getter + 1],
            (ValueId(2), Instruction::ArcCopy { value: Operand::Val(ValueId(6)) }, Type::Ref)
        ));
        let Terminator::CondBranch { then_block, .. } = f.blocks[0].terminator else {
            panic!("guarda pending ausente");
        };
        let error = f.blocks.iter().find(|b| b.id == then_block).unwrap();
        let released: Vec<_> = error.instructions.iter().filter_map(|(_, i, _)| match i {
            Instruction::ArcDrop { value: Operand::Val(v) } => Some(*v),
            _ => None,
        }).collect();
        assert_eq!(released, [ValueId(1), ValueId(2)]);
        assert!(error.instructions.iter().all(|(_, i, _)| !matches!(i, Instruction::ArcCopy { .. })));
    }
}
