//! Prova AOT de cleanup produzido numa aresta normal dentro de um laço.
//! HIR aloca Mint Owned e usa uma chamada Borrow antes da guarda pending.
//! Hooks LLVM de observação substituem print_handle com a mesma assinatura,
//! sem GC, Dart ou consumo. Endereços de diagnóstico não são raízes Ref.
//! Cada iteração confere owner antes/depois do release; morte física final
//! é conferida após sair da função. Tracing pode preservar raízes observacionais
//! durante a função. O erro usa pendência real pré-carregada, não throw do hook.
//! Não certifica lowering da fonte, Finalizable, suspensão ou unwind.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo() -> Result<Module, String> {
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_cleanup_laco".into());
    let val = |v| Operand::Val(ValueId(v));
    let int = |v| Operand::Constant(Constant::Int(v));
    let call = |name: &str, args, ret_ty| Instruction::CallRuntime {
        name: name.into(),
        args,
        ret_ty,
    };
    let guarda = |cmp, erro, normal| Terminator::CondBranch {
        cond: val(cmp),
        then_block: BlockId(erro),
        else_block: BlockId(normal),
    };
    let flag = |v| {
        (
            ValueId(v),
            call("dartforge_exception_pending", vec![], Type::I8),
            Type::I8,
        )
    };
    let cmp = |v, p| {
        (
            ValueId(v),
            Instruction::ICmp(ICmpOp::Ne, val(p), int(0)),
            Type::I1,
        )
    };
    m.functions.push(Function {
        symbol: "alocar_no_laco".into(),
        name: "alocar_no_laco".into(),
        depuracao: None,
        params: vec![(ValueId(0), "limite".into(), Type::I64)],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Phi {
                            incoming: vec![(BlockId(0), int(0)), (BlockId(4), val(7))],
                            ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        call(
                            "dartforge_arc_box_int_owned_v1",
                            vec![(int(i64::MAX), Type::I64)],
                            Type::Ref,
                        ),
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        call(
                            "dartforge_print_handle",
                            vec![(val(2), Type::Ref)],
                            Type::Void,
                        ),
                        Type::Void,
                    ),
                    flag(4),
                    cmp(5, 4),
                ],
                terminator: guarda(5, 5, 2),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![
                    (
                        ValueId(6),
                        call("dartforge_arc_collect", vec![], Type::Void),
                        Type::Void,
                    ),
                    (
                        ValueId(9),
                        call(
                            "dartforge_print_handle",
                            vec![(Operand::Constant(Constant::Null), Type::Ref)],
                            Type::Void,
                        ),
                        Type::Void,
                    ),
                    flag(10),
                    cmp(11, 10),
                ],
                terminator: guarda(11, 6, 4),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![
                    (ValueId(7), Instruction::Add(val(1), int(1)), Type::I64),
                    (
                        ValueId(8),
                        Instruction::ICmp(ICmpOp::Slt, val(7), val(0)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(8),
                    then_block: BlockId(1),
                    else_block: BlockId(3),
                },
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(5),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(6),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    });
    let mut plano = PlanoFuncaoDart::default();
    // O limite do harness é um contador escalar, não bits de referência.
    plano.classes.insert(ValueId(0), Ownership::Trivial);
    let mut planos = HashMap::from([("alocar_no_laco".into(), plano)]);
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if produzido != (0, 2) {
        return Err(format!("cleanup esperado (0, 2), encontrado {produzido:?}"));
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
    {
        return Err("cleanup do laço não foi idempotente".into());
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
    let erro = match args.next().as_deref() {
        Some("erro") => true,
        Some("normal") | None => false,
        Some(outro) => return Err(format!("caminho desconhecido: {outro}")),
    };
    let controle = args.next();
    let mut m = modulo()?;
    m.memoria_arc = arc;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    for (antes, depois) in [
        ("@dartforge_print_handle(i64 %v2)", "@prova_salvar(i64 %v2)"),
        ("@dartforge_print_handle(i64 0)", "@prova_sem_owner(i64 0)"),
    ] {
        if ir.matches(antes).count() != 1 {
            return Err(format!("hook de observação ausente: {antes}"));
        }
        ir = ir.replace(antes, depois);
    }
    match controle.as_deref() {
        None => {}
        Some("sem-drop-normal") if !erro => {
            let trecho = "b7:\n  call void @dartforge_arc_release(i64 %v2)\n";
            if ir.matches(trecho).count() != 1 {
                return Err("cleanup normal ausente no IR".into());
            }
            ir = ir.replace(trecho, "b7:\n");
        }
        Some("sem-drop-erro") if erro => {
            let trecho = "b5:\n  call void @dartforge_arc_release(i64 %v2)\n";
            if ir.matches(trecho).count() != 1 {
                return Err("cleanup de erro ausente no IR".into());
            }
            ir = ir.replace(trecho, "b5:\n");
        }
        Some(outro) => return Err(format!("controle/caminho incompatível: {outro}")),
    }
    let pre_carga = if erro {
        "  call void @dartforge_arc_lancar_ref_v1(i64 85)\n"
    } else {
        ""
    };
    ir.push_str(&format!("\ndefine void @prova_cleanup_laco() {{\n{pre_carga}  call void @alocar_no_laco(i64 10)\n  call void @prova_final(i64 {})\n  ret void\n}}\n", if erro { 1 } else { 10 }));
    ir.push_str(
        r#"
@prova.endereco = internal global i64 0
@prova.iteracoes = internal global i64 0
define void @prova_salvar(i64 %obj) {
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %salvar, label %falha
salvar:
  store i64 %obj, ptr @prova.endereco
  %i = load i64, ptr @prova.iteracoes
  %j = add i64 %i, 1
  store i64 %j, ptr @prova.iteracoes
  ret void
falha:
  call void @llvm.trap()
  unreachable
}
define void @prova_sem_owner(i64 %ignorado) {
  %obj = load i64, ptr @prova.endereco
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %bit = and i8 %estado, 2
  %solto = icmp eq i8 %bit, 0
  br i1 %solto, label %passou, label %falha
falha:
  call void @llvm.trap()
  unreachable
passou:
  ret void
}
declare void @dartforge_print_handle(i64)
define void @prova_final(i64 %esperado) {
  %i = load i64, ptr @prova.iteracoes
  %contagem = icmp eq i64 %i, %esperado
  br i1 %contagem, label %conferir, label %falha
conferir:
  call void @prova_sem_owner(i64 0)
  %pending = call i8 @dartforge_exception_pending()
  %com_erro = icmp ne i8 %pending, 0
  %quer_erro = icmp eq i64 %esperado, 1
  %ok_pending = icmp eq i1 %com_erro, %quer_erro
  br i1 %ok_pending, label %identidade, label %falha
identidade:
  br i1 %quer_erro, label %capturar, label %limpar
capturar:
  %excecao = call i64 @dartforge_arc_excecao_owned_v1()
  %mesma = icmp eq i64 %excecao, 85
  call void @dartforge_arc_release(i64 %excecao)
  br i1 %mesma, label %limpar, label %falha
limpar:
  call void @dartforge_exception_clear()
  call void @dartforge_arc_collect()
  %obj = load i64, ptr @prova.endereco
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %morto = icmp eq i8 %estado, 0
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
    fn cleanup_do_laco_esta_antes_da_coleta_e_na_saida_de_erro() {
        let m = modulo().unwrap();
        let f = &m.functions[0];
        assert!(matches!(
            f.blocks[1].terminator,
            Terminator::CondBranch {
                then_block: BlockId(5),
                else_block: BlockId(7),
                ..
            }
        ));
        for id in [5, 7] {
            let b = f.blocks.iter().find(|b| b.id == BlockId(id)).unwrap();
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(2))
                }
            ));
        }
        LlvmEmitter::new(&m).emit_all();
    }
}
