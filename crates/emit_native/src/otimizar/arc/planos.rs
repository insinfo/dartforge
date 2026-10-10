//! Transporta o CFG excepcional já preparado pelo compilador aos planos ARC.
//! Não inventa ownership, limites semânticos ou contratos de módulos externos.

use super::*;

/// Prepara ownership usando o CFG de exceções já produzido pelo compilador.
///
/// Importa as tabelas, prepara ARC e publica corpos/planos/tabelas juntos.
/// A seleção ARC e o modo de exceções devem preceder esta etapa; tracing
/// devolve (0, 0). Classes, tokens e limites semânticos ainda precisam dos
/// produtores e contratos descritos em [`preparar_arc_modulo_dart`].
/// Não resolve contratos de callees externos, dispatch ou versões do SDK.
///
/// # Erros
/// Qualquer falha de importação ou preparação. Mesmo se a importação for
/// válida e ownership falhar depois, o módulo e os planos permanecem intactos.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.memoria_arc = true; m.excecoes_por_tabelas = true;
/// m.functions.push(Function { symbol: "folha".into(), name: "folha".into(),
///     depuracao: None, params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] });
/// m.tabelas.push(TabelasDaFuncao { confere_pilha: true, ..Default::default() });
/// let mut planos = HashMap::from([("folha".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(preparar_arc_modulo_tabelado(&mut m, &mut planos)?, (0, 0));
/// assert!(planos["folha"].tabelas.confere_pilha);
/// # Ok::<(), String>(())
/// ```
pub fn preparar_arc_modulo_tabelado(
    modulo: &mut Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(usize, usize), String> {
    if !modulo.memoria_arc {
        return Ok((0, 0));
    }
    let mut novos = planos.clone();
    produzir_tabelas_arc_do_modulo(modulo, &mut novos)?;
    let resultado = preparar_arc_modulo_dart(modulo, &mut novos)?;
    *planos = novos;
    Ok(resultado)
}

/// Importa as tabelas de exceções do módulo para os planos ARC da mesma HIR.
///
/// Roda depois do passe de exceções e antes da preparação de ownership. Planos
/// conservam classes, tokens e limites; suas tabelas devem estar vazias ou ser
/// iguais às do módulo. Importar não certifica esses contratos: a preparação
/// ARC deve verificar todo o conjunto antes da emissão. Tracing não altera
/// planos. Retorna a quantidade de tabelas alteradas; repetir devolve zero.
///
/// # Erros
/// Modo de tabelas desligado, inventário incompleto, símbolo repetido, plano
/// ausente/obsoleto, SSA inválida ou tabela conflitante. Nenhum plano é alterado.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.memoria_arc = true; m.excecoes_por_tabelas = true;
/// m.functions.push(Function { symbol: "folha".into(), name: "folha".into(),
///     depuracao: None, params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] });
/// m.tabelas.push(TabelasDaFuncao { confere_pilha: true, ..Default::default() });
/// let mut planos = HashMap::from([("folha".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(produzir_tabelas_arc_do_modulo(&m, &mut planos)?, 1);
/// assert!(planos["folha"].tabelas.confere_pilha);
/// # Ok::<(), String>(())
/// ```
pub fn produzir_tabelas_arc_do_modulo(
    modulo: &Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<usize, String> {
    if !modulo.memoria_arc {
        return Ok(0);
    }
    if !modulo.excecoes_por_tabelas || modulo.tabelas.len() != modulo.functions.len() {
        return Err("tabelas ARC: módulo sem inventário excepcional completo".into());
    }
    let mut simbolos = HashSet::new();
    let vazio = TabelasDaFuncao::default();
    for (f, tabela) in modulo.functions.iter().zip(&modulo.tabelas) {
        if !simbolos.insert(&f.symbol) {
            return Err(format!("tabelas ARC: símbolo repetido {}", f.symbol));
        }
        super::ssa::verificar(f)?;
        let plano = planos
            .get(&f.symbol)
            .ok_or_else(|| format!("tabelas ARC: plano ausente {}", f.symbol))?;
        if plano.tabelas != vazio && plano.tabelas != *tabela {
            return Err(format!("tabelas ARC: inventário conflitante {}", f.symbol));
        }
    }
    if let Some(simbolo) = planos.keys().filter(|s| !simbolos.contains(s)).min() {
        return Err(format!("tabelas ARC: plano obsoleto {simbolo}"));
    }
    let mut alteradas = 0;
    for (f, tabela) in modulo.functions.iter().zip(&modulo.tabelas) {
        let plano = planos.get_mut(&f.symbol).unwrap();
        if plano.tabelas != *tabela {
            plano.tabelas = tabela.clone();
            alteradas += 1;
        }
    }
    Ok(alteradas)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn conjunto() -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let val = |v| Operand::Val(ValueId(v));
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![
            Function {
                symbol: "caller".into(),
                name: "caller".into(),
                depuracao: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(3),
                                Instruction::ArcCopy { value: val(0) },
                                Type::Ref,
                            ),
                            (
                                ValueId(1),
                                Instruction::CallStatic {
                                    symbol: "folha".into(),
                                    args: vec![val(3)],
                                    ret_ty: Type::Ref,
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
                                    val(4),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: val(5),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![(
                            ValueId(6),
                            Instruction::CallRuntime {
                                name: "dartforge_exception_clear".into(),
                                args: vec![],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        )],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(val(1))),
                    },
                ],
            },
            Function {
                symbol: "folha".into(),
                name: "folha".into(),
                depuracao: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_print_handle".into(),
                            args: vec![(val(0), Type::Ref)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(Some(val(0))),
                }],
            },
        ];
        // Mesmo passe usado pelo fluxo de emissão da fonte, sem inventar pousos.
        super::super::super::tabelas::aplicar(&mut m);
        let planos = m
            .functions
            .iter()
            .map(|f| {
                let mut plano = PlanoFuncaoDart::default();
                plano.tokens.retorno = RetornoTokens::Owned;
                (f.symbol.clone(), plano)
            })
            .collect();
        (m, planos)
    }

    #[test]
    fn catch_do_passe_real_transporta_invokes_e_cleanup_ao_emissor() {
        let (mut m, mut planos) = conjunto();
        assert!(!m.tabelas[0].invocacoes.is_empty());
        assert_eq!(produzir_tabelas_arc_do_modulo(&m, &mut planos).unwrap(), 2);
        assert_eq!(produzir_tabelas_arc_do_modulo(&m, &mut planos).unwrap(), 0);
        preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
        let caller = &m.functions[0];
        let pouso = m.tabelas[0].invocacoes[&ValueId(1)];
        assert_eq!(planos["caller"].owners_no_pouso[&pouso], vec![ValueId(3)]);
        assert!(
            caller
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .any(|(_, i, _)| matches!(
                    i,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(3))
                    }
                ))
        );
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("invoke i64 @folha("));
        assert!(ir.contains("call void @dartforge_exception_clear()"));
        assert_eq!(produzir_tabelas_arc_do_modulo(&m, &mut planos).unwrap(), 0);
        // Entrada integrada: nenhuma cópia manual de tabela entre as etapas.
        let (mut m, mut planos) = conjunto();
        preparar_arc_modulo_tabelado(&mut m, &mut planos).unwrap();
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_tabelado(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn ownership_invalido_depois_da_importacao_conserva_modulo_e_planos() {
        let (mut m, mut planos) = conjunto();
        m.functions[1].blocks[0].instructions.push((
            ValueId(77),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        let antes = format!("{m:?}/{planos:?}");
        assert!(preparar_arc_modulo_tabelado(&mut m, &mut planos).is_err());
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn importacao_invalida_nao_publica_planos_parciais() {
        for caso in 0..7 {
            let (mut m, mut planos) = conjunto();
            match caso {
                0 => {
                    planos.remove("folha");
                }
                1 => {
                    m.tabelas.pop();
                }
                2 => {
                    planos
                        .get_mut("folha")
                        .unwrap()
                        .tabelas
                        .pousos
                        .insert(BlockId(999));
                }
                3 => m.excecoes_por_tabelas = false,
                4 => {
                    planos.insert("obsoleto".into(), PlanoFuncaoDart::default());
                }
                5 => m.functions[1].symbol = "caller".into(),
                _ => m.memoria_arc = false,
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = produzir_tabelas_arc_do_modulo(&m, &mut planos);
            if caso == 6 {
                assert_eq!(resultado.unwrap(), 0);
            } else {
                assert!(resultado.is_err(), "caso {caso}");
            }
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }
}
