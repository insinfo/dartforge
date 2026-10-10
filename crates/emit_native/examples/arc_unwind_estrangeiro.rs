//! Prova AOT Unix: cleanup ARC com exceção C++, identidade e morte do Mint.
//! Injeta um lançamento C++ no ponto print_handle após emitir a HIR auditada.
//! O hook conserva a assinatura Borrow, não consome tokens nem cria pendência Dart.
//! A troca de símbolo fica explícita no IR; esta é uma prova de fault injection.

#[cfg(unix)]
use dartforge_emit_native::llvm::LlvmEmitter;
#[cfg(any(unix, test))]
use dartforge_emit_native::{hir::*, otimizar::arc::*};
#[cfg(any(unix, test))]
use std::collections::HashMap;

#[cfg(any(unix, test))]
fn modulo() -> Result<Module, String> {
    let val = |v| Operand::Val(ValueId(v));
    let print = |v, argumento| {
        (
            ValueId(v),
            Instruction::CallRuntime {
                name: "dartforge_print_handle".into(),
                args: vec![(argumento, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    };
    let caller = Function {
        symbol: "prova_owner_estrangeiro".into(),
        name: "prova_owner_estrangeiro".into(),
        depuracao: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_box_int_owned_v1".into(),
                            args: vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "prova_falha_estrangeira".into(),
                            args: vec![val(0)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![print(3, val(0))],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    };
    let folha = Function {
        symbol: "prova_falha_estrangeira".into(),
        name: "prova_falha_estrangeira".into(),
        depuracao: None,
        params: vec![(ValueId(0), "valor".into(), Type::Ref)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![print(1, val(0))],
            terminator: Terminator::Return(None),
        }],
    };
    let mut caller_plano = PlanoFuncaoDart::default();
    caller_plano
        .tabelas
        .invocacoes
        .insert(ValueId(2), BlockId(1));
    caller_plano.tabelas.pousos.insert(BlockId(1));
    caller_plano
        .tabelas
        .saidas
        .insert(BlockId(1), SaidaPorExcecao::Guarda);
    let mut folha_plano = PlanoFuncaoDart::default();
    folha_plano
        .tabelas
        .saidas
        .insert(BlockId(0), SaidaPorExcecao::Guarda);
    let mut planos = HashMap::from([
        (caller.symbol.clone(), caller_plano),
        (folha.symbol.clone(), folha_plano),
    ]);
    let mut m = Module::new();
    m.functions = vec![caller, folha];
    m.excecoes_por_tabelas = true;
    m.entry_symbol = Some("prova_owner_estrangeiro".into());
    assert_eq!(
        inserir_arc_funcoes_dart(&mut m.functions, &mut planos)?,
        (0, 3)
    );
    assert_eq!(
        planos["prova_owner_estrangeiro"].owners_no_pouso[&BlockId(1)],
        vec![ValueId(0)]
    );
    m.tabelas = m
        .functions
        .iter()
        .map(|f| planos[&f.symbol].tabelas.clone())
        .collect();
    Ok(m)
}

#[cfg(unix)]
fn main() -> Result<(), String> {
    use dartforge_emit_native::{
        cache::RuntimeCache,
        gerador::{Geracao, Gerador},
    };
    use std::{path::PathBuf, process::Command};
    let saida = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("informe o executável de saída")?,
    );
    if let Some(dir) = saida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let m = modulo()?;
    let mut ir = LlvmEmitter::new(&m).emit_all();
    ir.push_str("\ndeclare void @dartforge_print_handle(i64)\n");
    ir = ir.replace("@dartforge_print_handle(", "@prova_print_estrangeira(");
    std::fs::write(saida.with_extension("ll"), &ir).map_err(|e| e.to_string())?;
    let obj = saida.with_extension("o");
    Gerador::escolher(&PathBuf::from("clang")).gerar(
        &ir,
        Geracao::do_programa(std::env::args().nth(2).as_deref() != Some("0"), false),
        &obj,
    )?;
    let cpp = saida.with_extension("cpp");
    std::fs::write(
        &cpp,
        include_str!("../tests/fixtures/arc_unwind_estrangeiro.cpp"),
    )
    .map_err(|e| e.to_string())?;
    let runtime = RuntimeCache::para_producao()?;
    let mut cmd = Command::new(std::env::var_os("CXX").unwrap_or_else(|| "c++".into()));
    cmd.args(["-std=c++17", "-O2"])
        .arg(&cpp)
        .arg(&obj)
        .arg(&runtime.lib_path)
        .arg("-o")
        .arg(&saida);
    #[cfg(target_os = "linux")]
    cmd.args(["-ldl", "-lpthread", "-lm"]);
    let resultado = cmd.output().map_err(|e| e.to_string())?;
    if !resultado.status.success() {
        return Err(format!(
            "ligação C++ falhou: {}",
            String::from_utf8_lossy(&resultado.stderr)
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn main() -> Result<(), String> {
    Err("prova exige unwind Itanium Unix".into())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn prova_estrangeira_tem_inventario_certificado() {
        let m = modulo().unwrap();
        assert!(m.tabelas[0].cleanup_estrangeiro.is_some());
        #[cfg(unix)]
        {
            let ir = LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("lpad1.estrangeira:"));
            assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
        }
    }
}
