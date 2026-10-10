//! Prova AOT de getters/setters escolhidos para objeto local não publicado.
//! Usa HIR tipada para ler/gravar escalares e devolver um campo Ref depois
//! de sobrescrevê-lo. O harness observa owners/morte, sem fixture C++.
//! Não certifica lowering Dart completo, late, publicação ou recarga.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter, otimizar::arc::*};
use std::{collections::HashMap, path::PathBuf};

fn modulo() -> Result<Module, String> {
    let val = |v| Operand::Val(ValueId(v));
    let int = |n| Operand::Constant(Constant::Int(n));
    let mut m = Module::new();
    m.memoria_arc = true;
    m.entry_symbol = Some("prova_campos_locais".into());
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
    m.layouts_campos_arc.insert(123, layout);
    let mut fields = vec![int(0); 40];
    fields[1] = Operand::Constant(Constant::Double(2.5));
    fields[2] = Operand::Constant(Constant::Bool(false));
    fields[3] = val(2);
    fields[39] = val(0);
    let mut ops = vec![
        (
            ValueId(0),
            Instruction::Box {
                op: int(i64::MAX),
                from: Type::I64,
            },
            Type::Ref,
        ),
        (
            ValueId(2),
            Instruction::Trunc {
                op: int(0),
                from: Type::I64,
                to: Type::I8,
            },
            Type::I8,
        ),
        (
            ValueId(3),
            Instruction::Trunc {
                op: int(255),
                from: Type::I64,
                to: Type::I8,
            },
            Type::I8,
        ),
        (
            ValueId(1),
            Instruction::AllocObject {
                class_id: 123,
                fields,
            },
            Type::Ref,
        ),
    ];
    ops.push((
        ValueId(4),
        Instruction::GetField {
            object: val(1),
            index: 39,
        },
        Type::Ref,
    ));
    // Leituras abaixo conferem os valores novos, não apenas a inicialização.
    for (v, index, value) in [
        (24, 0, int(85)),
        (25, 1, Operand::Constant(Constant::Double(-0.0))),
        (26, 2, Operand::Constant(Constant::Bool(true))),
        (27, 3, val(3)),
    ] {
        ops.push((
            ValueId(v),
            Instruction::SetField {
                object: val(1),
                index,
                value,
            },
            Type::Void,
        ));
    }
    for (v, index, ty) in [
        (5, 1, Type::F64),
        (6, 2, Type::I1),
        (7, 3, Type::I8),
        (8, 0, Type::I64),
    ] {
        ops.push((
            ValueId(v),
            Instruction::GetField {
                object: val(1),
                index,
            },
            ty,
        ));
    }
    ops.push((
        ValueId(9),
        Instruction::Bitcast {
            op: val(5),
            to: Type::I64,
        },
        Type::I64,
    ));
    for (v, source, from) in [(10, 6, Type::I1), (11, 7, Type::I8)] {
        ops.push((
            ValueId(v),
            Instruction::ZExt {
                op: val(source),
                from,
                to: Type::I64,
            },
            Type::I64,
        ));
    }
    for (v, source, esperado) in [(12, 9, i64::MIN), (14, 10, 1), (16, 11, 255), (18, 8, 85)] {
        ops.push((
            ValueId(v),
            Instruction::ICmp(ICmpOp::Eq, val(source), int(esperado)),
            Type::I1,
        ));
        ops.push((
            ValueId(v + 1),
            Instruction::ZExt {
                op: val(v),
                from: Type::I1,
                to: Type::I64,
            },
            Type::I64,
        ));
    }
    for (v, a, b) in [(20, 13, 15), (21, 17, 19), (22, 20, 21)] {
        ops.push((ValueId(v), Instruction::And(val(a), val(b)), Type::I64));
    }
    ops.push((
        ValueId(23),
        Instruction::ICmp(ICmpOp::Eq, val(22), int(1)),
        Type::I1,
    ));
    let mut sucesso = Vec::new();
    sucesso.push((
        ValueId(28),
        Instruction::SetField {
            object: val(1),
            index: 39,
            value: Operand::Constant(Constant::Null),
        },
        Type::Void,
    ));
    m.functions.push(Function {
        symbol: "ler_objeto_local".into(),
        name: "ler_objeto_local".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: ops,
                terminator: Terminator::CondBranch {
                    cond: val(23),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: sucesso,
                terminator: Terminator::Return(Some(val(4))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    });
    let mut p = PlanoFuncaoDart::default();
    p.tokens.retorno = RetornoTokens::Owned;
    let mut planos = HashMap::from([("ler_objeto_local".into(), p)]);
    let produzido = preparar_arc_modulo_dart(&mut m, &mut planos)?;
    if produzido.0 != 1 {
        return Err(format!("esperado um keepalive, produzido {produzido:?}"));
    }
    let antes = format!("{m:?}/{planos:?}");
    if preparar_arc_modulo_dart(&mut m, &mut planos)? != (0, 0)
        || format!("{m:?}/{planos:?}") != antes
    {
        return Err("preparação não idempotente".into());
    }
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    Ok(m)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável")?);
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
    let alocar = "  %v1 = call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)\n";
    if ir.matches(alocar).count() != 1 {
        return Err("alocação sem alvo único".into());
    }
    ir = ir.replace(
        alocar,
        &format!("{alocar}  store i64 %v1, ptr @prova.objeto\n"),
    );
    if controle.as_deref() == Some("sem-retencao") {
        let linha = ir
            .lines()
            .filter(|l| l.contains("call void @dartforge_arc_retain(i64 %v"))
            .collect::<Vec<_>>();
        if linha.len() != 1 {
            return Err("keepalive sem alvo único".into());
        }
        ir = ir.replace(&format!("{}\n", linha[0]), "");
    } else if controle.as_deref() == Some("sem-drop-objeto") {
        let linha = "  call void @dartforge_arc_release(i64 %v1)\n";
        if !ir.contains(linha) {
            return Err("cleanup do objeto ausente".into());
        }
        ir = ir.replace(linha, "");
    }
    ir.push_str(
        r#"
@prova.objeto = internal global i64 0
declare void @dartforge_print_handle(i64)
define void @prova_campos_locais() {
  %ret = call i64 @ler_objeto_local()
  %estado = call i8 @dartforge_arc_observar_heap_v1(i64 %ret)
  %owned = icmp eq i8 %estado, 3
  br i1 %owned, label %conferir, label %falha
conferir:
  %valor = call i64 @dartforge_unbox_int(i64 %ret)
  %mint = icmp eq i64 %valor, 9223372036854775807
  br i1 %mint, label %coletar, label %falha
coletar:
  call void @dartforge_arc_collect()
  %obj = load i64, ptr @prova.objeto
  %eo = call i8 @dartforge_arc_observar_heap_v1(i64 %obj)
  %er = call i8 @dartforge_arc_observar_heap_v1(i64 %ret)
  %morto = icmp eq i8 %eo, 0
  %vivo = icmp eq i8 %er, 3
  %ok = and i1 %morto, %vivo
  br i1 %ok, label %liberar, label %falha
liberar:
  call void @dartforge_arc_release(i64 %ret)
  call void @dartforge_arc_collect()
  %fim = call i8 @dartforge_arc_observar_heap_v1(i64 %ret)
  %solto = icmp eq i8 %fim, 0
  br i1 %solto, label %passou, label %falha
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
        None | Some("sem-retencao" | "sem-drop-objeto") => {}
        Some("sem-drop-retorno") => {
            ir = ir.replace("  call void @dartforge_arc_release(i64 %ret)\n", "");
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
    fn campo_local_e_keepalive_geram_ir_verificado() {
        LlvmEmitter::new(&modulo().unwrap()).emit_all();
    }
}
