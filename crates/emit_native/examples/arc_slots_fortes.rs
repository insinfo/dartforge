//! Prova AOT dirigida das operações fortes, antes do produtor automático.
//! Usa um literal permanente, Smi e null: não certifica coleta de um objeto
//! mortal, escopos/borrows ou o restante dos contratos do pipeline ARC.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let arc = args.next().as_deref() != Some("tracing");
    let mut m = Module::new();
    m.memoria_arc = arc;
    m.entry_symbol = Some("prova_slots".into());
    m.globais.push((0, Type::Ref, "dfg.prova".into()));
    let quadro = SlotForte::Quadro {
        quadro: Operand::Val(ValueId(0)),
        indice: 0,
    };
    let global = SlotForte::Global {
        simbolo: "dfg.prova".into(),
    };
    let valor = |v| Operand::Val(ValueId(v));
    let chamada = |nome: &str, args, ret_ty| Instruction::CallRuntime {
        name: nome.into(),
        args,
        ret_ty,
    };
    m.functions.push(Function {
        symbol: "prova_slots".into(),
        name: "prova_slots".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    chamada(
                        "dartforge_arc_quadro_abrir_v1",
                        vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                        Type::I64,
                    ),
                    Type::I64,
                ),
                (
                    ValueId(1),
                    Instruction::Const(Constant::String("slots ARC".into())),
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ArcCopy { value: valor(1) },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcStoreStrong {
                        slot: quadro.clone(),
                        value: valor(2),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::ArcLoadStrong { slot: quadro },
                    Type::Ref,
                ),
                (
                    ValueId(5),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: valor(4),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(6),
                    chamada(
                        "dartforge_arc_quadro_fechar_v1",
                        vec![(valor(0), Type::I64)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(7),
                    chamada("dartforge_arc_collect", vec![], Type::Void),
                    Type::Void,
                ),
                (
                    ValueId(8),
                    Instruction::ArcLoadStrong {
                        slot: global.clone(),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(9),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(8), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(10),
                    Instruction::ArcDrop { value: valor(8) },
                    Type::Void,
                ),
                (
                    ValueId(11),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(42)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(12),
                    Instruction::ArcCopy { value: valor(11) },
                    Type::Ref,
                ),
                (
                    ValueId(13),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: valor(12),
                        modo: ModoStoreForte::Move,
                    },
                    Type::Void,
                ),
                (
                    ValueId(14),
                    Instruction::ArcLoadStrong {
                        slot: global.clone(),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(15),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(14), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(16),
                    Instruction::ArcDrop { value: valor(14) },
                    Type::Void,
                ),
                (
                    ValueId(17),
                    Instruction::ArcStoreStrong {
                        slot: global.clone(),
                        value: Operand::Constant(Constant::Null),
                        modo: ModoStoreForte::Copy,
                    },
                    Type::Void,
                ),
                (
                    ValueId(18),
                    Instruction::ArcLoadStrong { slot: global },
                    Type::Ref,
                ),
                (
                    ValueId(19),
                    chamada(
                        "dartforge_print_handle",
                        vec![(valor(18), Type::Ref)],
                        Type::Void,
                    ),
                    Type::Void,
                ),
                (
                    ValueId(20),
                    Instruction::ArcDrop { value: valor(18) },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    dartforge_emit_native::otimizar::otimizar(&mut m);
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // Auxiliar de observação do harness, exportado pelo runtime mas fora
    // do catálogo normal usado pelo lowering do SDK.
    ir.push_str("\ndeclare void @dartforge_print_handle(i64)\n");
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    driver::compile_and_link(
        &ir,
        &saida,
        &driver::NativeDriverOptions {
            optimize: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
