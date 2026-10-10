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
fn modulo(com_quadros: bool, retoma: bool) -> Result<Module, String> {
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
    let mut caller = Function {
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
                        Instruction::Box {
                            op: Operand::Constant(Constant::Int(i64::MAX)),
                            from: Type::I64,
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
    if com_quadros {
        let chamada = |id, nome: &str, args, ty| {
            (
                ValueId(id),
                Instruction::CallRuntime {
                    name: nome.into(),
                    args,
                    ret_ty: ty,
                },
                ty,
            )
        };
        let abrir = |id| {
            chamada(
                id,
                "dartforge_arc_quadro_abrir_v1",
                vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                Type::I64,
            )
        };
        let fechar = |id, quadro| {
            chamada(
                id,
                "dartforge_arc_quadro_fechar_v1",
                vec![(val(quadro), Type::I64)],
                Type::Void,
            )
        };
        let slot = |quadro| SlotForte::Quadro {
            quadro: val(quadro),
            indice: 0,
        };
        let fabrica = caller.blocks[0].instructions.remove(0);
        caller.blocks[0].instructions = vec![
            abrir(6),
            abrir(11),
            fabrica,
            (
                ValueId(7),
                Instruction::ArcStoreStrong {
                    slot: slot(6),
                    value: val(0),
                    modo: ModoStoreForte::Copy,
                },
                Type::Void,
            ),
            (
                ValueId(12),
                Instruction::ArcStoreStrong {
                    slot: slot(11),
                    value: val(0),
                    modo: ModoStoreForte::Move,
                },
                Type::Void,
            ),
            (
                ValueId(8),
                Instruction::ArcLoadStrong { slot: slot(11) },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::CallStatic {
                    symbol: "prova_falha_estrangeira".into(),
                    args: vec![val(8)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        caller.blocks[1].instructions = vec![fechar(13, 11), fechar(14, 6), print(3, val(8))];
        caller.blocks[2].instructions = vec![fechar(15, 11), fechar(16, 6)];
    }
    if retoma {
        caller.blocks[1]
            .instructions
            .retain(|(id, _, _)| *id != ValueId(3));
    }
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
    caller_plano.tabelas.saidas.insert(
        BlockId(1),
        if retoma {
            SaidaPorExcecao::Retoma
        } else {
            SaidaPorExcecao::Guarda
        },
    );
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
        (0, if retoma { 2 } else { 3 })
    );
    assert_eq!(
        planos["prova_owner_estrangeiro"].owners_no_pouso[&BlockId(1)],
        vec![ValueId(if com_quadros { 8 } else { 0 })]
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
    let perfil = std::env::args().nth(3).unwrap_or_else(|| "simples".into());
    let m = modulo(
        perfil == "quadros" || perfil == "retoma",
        perfil == "retoma",
    )?;
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
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "c++".into());
    #[cfg(not(target_os = "macos"))]
    let biblioteca = runtime.lib_path.clone();
    #[cfg(target_os = "macos")]
    let biblioteca = {
        // Compact unwind admite três personalidades por imagem. A fixture
        // combina C++, Dart legado/cleanup e Rust; separa o runtime numa dylib
        // conservando as informações de unwind de ambas as imagens.
        let dylib = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(saida.with_extension("runtime.dylib"));
        let mut ligar_runtime = Command::new(&cxx);
        ligar_runtime
            .args(["-dynamiclib", "-Xlinker", "-force_load", "-Xlinker"])
            .arg(&runtime.lib_path)
            .args(["-Xlinker", "-install_name", "-Xlinker"])
            .arg(&dylib)
            .args(dartforge_emit_native::alvo::bibliotecas_do_sistema())
            .arg("-o")
            .arg(&dylib);
        std::fs::write(
            saida.with_extension("runtime.ligacao.txt"),
            format!("{ligar_runtime:?}\n"),
        )
        .map_err(|e| e.to_string())?;
        let resultado = ligar_runtime.output().map_err(|e| e.to_string())?;
        if !resultado.status.success() {
            return Err(format!(
                "ligação da dylib do runtime falhou: {}",
                String::from_utf8_lossy(&resultado.stderr)
            ));
        }
        dylib
    };
    let mut cmd = Command::new(&cxx);
    cmd.args(["-std=c++17", "-O2"])
        .arg(&cpp)
        .arg(&obj)
        .arg(&biblioteca)
        .arg("-o")
        .arg(&saida);
    cmd.args(dartforge_emit_native::alvo::bibliotecas_do_sistema());
    std::fs::write(saida.with_extension("ligacao.txt"), format!("{cmd:?}\n"))
        .map_err(|e| e.to_string())?;
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
        let m = modulo(false, false).unwrap();
        assert!(m.tabelas[0].cleanup_estrangeiro.is_some());
        #[cfg(unix)]
        {
            let ir = LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("lpad1.estrangeira:"));
            assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
        }
    }
}

#[cfg(test)]
mod testes_quadros {
    use super::*;
    #[test]
    fn prova_estrangeira_com_dois_quadros_locais() {
        let m = modulo(true, false).unwrap();
        assert!(m.tabelas[0].cleanup_estrangeiro.is_some());
        assert!(dartforge_emit_native::lower::verificador::verificar(&m).is_empty());
        #[cfg(unix)]
        {
            let ir = LlvmEmitter::new(&m).emit_all();
            let estrangeiro = ir
                .split("lpad1.estrangeira:")
                .nth(1)
                .unwrap()
                .split("resume ")
                .next()
                .unwrap();
            let interno = estrangeiro
                .find("call void @dartforge_arc_quadro_fechar_v1(i64 %v11)")
                .unwrap();
            let externo = estrangeiro
                .find("call void @dartforge_arc_quadro_fechar_v1(i64 %v6)")
                .unwrap();
            assert!(interno < externo);
        }
    }
}

#[cfg(test)]
mod testes_retoma {
    use super::*;
    #[test]
    fn retoma_fecha_dois_quadros_sem_executar_catch_dart() {
        let m = modulo(true, true).unwrap();
        assert_eq!(
            m.tabelas[0].saidas.get(&BlockId(1)),
            Some(&SaidaPorExcecao::Retoma)
        );
        assert!(m.tabelas[0].cleanup_estrangeiro.is_none());
        assert!(dartforge_emit_native::lower::verificador::verificar(&m).is_empty());
    }
}
