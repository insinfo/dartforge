//! Prova AOT de um global forte com alvo mortal observado por referência fraca.
//! A função produtora sai antes das coletas, removendo suas raízes observacionais.
//! HIR explícita e auxiliares LLVM de observação não certificam o produtor automático.

use dartforge_emit_native::{driver, hir::*, llvm::LlvmEmitter};
use std::path::PathBuf;

fn chamada(nome: &str, args: Vec<(Operand, Type)>) -> Instruction {
    Instruction::CallRuntime {
        name: nome.into(),
        args,
        ret_ty: Type::Void,
    }
}

fn funcao(nome: &str, ops: Vec<(Instruction, Type)>) -> Function {
    Function {
        symbol: nome.into(),
        name: nome.into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: ops
                .into_iter()
                .enumerate()
                .map(|(i, (op, ty))| (ValueId(i as u32), op, ty))
                .collect(),
            terminator: Terminator::Return(None),
        }],
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let saida = PathBuf::from(args.next().ok_or("informe o executável de saída")?);
    let mut m = Module::new();
    m.memoria_arc = args.next().as_deref() != Some("tracing");
    let sabotagem = args.next();
    m.entry_symbol = Some("prova_mortal".into());
    m.globais.push((0, Type::Ref, "dfg.mortal".into()));
    let global = SlotForte::Global {
        simbolo: "dfg.mortal".into(),
    };
    let inteiro = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
    m.functions.push(funcao(
        "produzir_mortal",
        vec![
            (
                Instruction::CallRuntime {
                    name: "dartforge_alocar".into(),
                    args: vec![inteiro(0), inteiro(1), inteiro(2)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            (
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(0)),
                },
                Type::Ref,
            ),
            (
                Instruction::ArcStoreStrong {
                    slot: global.clone(),
                    value: Operand::Val(ValueId(1)),
                    modo: ModoStoreForte::Move,
                },
                Type::Void,
            ),
            (
                chamada(
                    "prova_criar_fraca",
                    vec![(Operand::Val(ValueId(0)), Type::Ref)],
                ),
                Type::Void,
            ),
        ],
    ));
    m.functions.push(funcao(
        "prova_mortal",
        vec![
            // CallRuntime preserva a fronteira no otimizador HIR. Mesmo se LLVM
            // incorporar o corpo, o fecho do quadro emitido antecede a coleta.
            (chamada("produzir_mortal", vec![]), Type::Void),
            (chamada("dartforge_arc_collect", vec![]), Type::Void),
            (
                chamada("prova_conferir_fraca", vec![inteiro(1)]),
                Type::Void,
            ),
            (
                Instruction::ArcStoreStrong {
                    slot: global,
                    value: Operand::Constant(Constant::Null),
                    modo: ModoStoreForte::Copy,
                },
                Type::Void,
            ),
            (chamada("dartforge_arc_collect", vec![]), Type::Void),
            (
                chamada("prova_conferir_fraca", vec![inteiro(0)]),
                Type::Void,
            ),
            (chamada("prova_fechar_fraca", vec![]), Type::Void),
        ],
    ));
    match sabotagem.as_deref() {
        None => {}
        // Controles negativos: um morre cedo, o outro continua vivo.
        Some("sem-owner") => {
            m.functions[0].blocks[0].instructions[2].1 = Instruction::ArcDrop {
                value: Operand::Val(ValueId(1)),
            };
        }
        Some("sem-liberar") => {
            m.functions[1].blocks[0].instructions[3].1 = chamada("dartforge_arc_collect", vec![]);
        }
        Some(outro) => return Err(format!("sabotagem desconhecida: {outro}")),
    }
    dartforge_emit_native::otimizar::otimizar(&mut m);
    let erros = dartforge_emit_native::lower::verificador::verificar(&m);
    if !erros.is_empty() {
        return Err(erros.join("\n"));
    }
    let mut ir = LlvmEmitter::new(&m).emit_all();
    // A observação permanece fora da HIR: o alvo weak não ganha uma raiz
    // forte no chamador. Só o contêiner fraco recebe um token independente.
    ir.push_str(
        r#"
@prova.fraca = internal global i64 0
declare void @dartforge_nativo_WeakReference_setTarget(i64, i64)
declare i64 @dartforge_nativo_WeakReference_getTarget(i64)
declare void @dartforge_print_handle(i64)
define void @prova_criar_fraca(i64 %alvo) {
  %w = call i64 @dartforge_alocar(i64 0, i64 1, i64 2)
  call void @dartforge_arc_retain(i64 %w)
  store i64 %w, ptr @prova.fraca
  call void @dartforge_nativo_WeakReference_setTarget(i64 %w, i64 %alvo)
  ret void
}
define void @prova_conferir_fraca(i64 %esperado) {
  %w = load i64, ptr @prova.fraca
  %alvo = call i64 @dartforge_nativo_WeakReference_getTarget(i64 %w)
  %vivo = icmp ne i64 %alvo, 0
  %quer = icmp ne i64 %esperado, 0
  %ok = icmp eq i1 %vivo, %quer
  br i1 %ok, label %passou, label %falhou
falhou:
  call void @llvm.trap()
  unreachable
passou:
  %saida = select i1 %vivo, i64 3, i64 0
  call void @dartforge_print_handle(i64 %saida)
  ret void
}
define void @prova_fechar_fraca() {
  %w = load i64, ptr @prova.fraca
  store i64 0, ptr @prova.fraca
  call void @dartforge_arc_release(i64 %w)
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
            optimize: true,
            ..Default::default()
        },
    )?;
    Ok(())
}
