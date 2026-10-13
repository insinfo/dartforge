//! Transporta o CFG excepcional já preparado pelo compilador aos planos ARC.
//! Não inventa ownership, limites semânticos ou contratos de módulos externos.

use super::*;

/// Prepara ownership usando o CFG de exceções já produzido pelo compilador.
///
/// Importa as tabelas, prepara ARC e publica corpos/planos/tabelas juntos.
/// A seleção ARC e o modo de exceções devem preceder esta etapa; tracing
/// devolve (0, 0). Classes, tokens e limites semânticos ainda precisam dos
/// produtores e contratos descritos em [`prepare_arc_dart_module`].
/// Não resolve contratos de callees externos, dispatch ou versões do SDK.
///
/// # Erros
/// Qualquer falha de importação ou preparação. Mesmo se a importação for
/// válida e ownership falhar depois, o módulo e os planos permanecem intactos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
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
pub fn prepare_arc_tabled_module(
    module: &mut Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(usize, usize), String> {
    if !module.arc_memory {
        return Ok((0, 0));
    }
    let mut new_set = plans.clone();
    produce_module_arc_tables(module, &mut new_set)?;
    let result = prepare_arc_dart_module(module, &mut new_set)?;
    *plans = new_set;
    Ok(result)
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
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
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
pub fn produce_module_arc_tables(
    module: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    if !module.arc_memory {
        return Ok(0);
    }
    if !module.excecoes_por_tabelas || module.tables.len() != module.functions.len() {
        return Err("tabelas ARC: módulo sem inventário excepcional completo".into());
    }
    let mut symbols = HashSet::new();
    let empty = FunctionTables::default();
    for (f, table) in module.functions.iter().zip(&module.tables) {
        if !symbols.insert(&f.symbol) {
            return Err(format!("tabelas ARC: símbolo repetido {}", f.symbol));
        }
        super::ssa::verify(f)?;
        let plan = plans
            .get(&f.symbol)
            .ok_or_else(|| format!("tabelas ARC: plano ausente {}", f.symbol))?;
        if plan.tables != empty && plan.tables != *table {
            return Err(format!("tabelas ARC: inventário conflitante {}", f.symbol));
        }
    }
    if let Some(symbol) = plans.keys().filter(|s| !symbols.contains(s)).min() {
        return Err(format!("tabelas ARC: plano obsoleto {symbol}"));
    }
    let mut changed = 0;
    for (f, table) in module.functions.iter().zip(&module.tables) {
        let plan = plans.get_mut(&f.symbol).unwrap();
        if plan.tables != *table {
            plan.tables = table.clone();
            changed += 1;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> (Module, HashMap<String, DartFunctionPlan>) {
        let val = |v| Operand::Val(ValueId(v));
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![
            Function {
                symbol: "caller".into(),
                name: "caller".into(),
                debug_info: None,
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
                debug_info: None,
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
        super::super::super::tables::apply(&mut m);
        let plans = m
            .functions
            .iter()
            .map(|f| {
                let mut plan = DartFunctionPlan::default();
                plan.tokens.ret = TokenReturn::Owned;
                (f.symbol.clone(), plan)
            })
            .collect();
        (m, plans)
    }

    #[test]
    fn real_pass_catch_carries_invokes_and_cleanup_to_emitter() {
        let (mut m, mut plans) = set();
        assert!(!m.tables[0].invocations.is_empty());
        assert_eq!(produce_module_arc_tables(&m, &mut plans).unwrap(), 2);
        assert_eq!(produce_module_arc_tables(&m, &mut plans).unwrap(), 0);
        prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        let caller = &m.functions[0];
        let landing_pad = m.tables[0].invocations[&ValueId(1)];
        assert_eq!(plans["caller"].owners_at_landing_pad[&landing_pad], vec![ValueId(3)]);
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
        assert_eq!(produce_module_arc_tables(&m, &mut plans).unwrap(), 0);
        // Entrada integrada: nenhuma cópia manual de tabela entre as etapas.
        let (mut m, mut plans) = set();
        prepare_arc_tabled_module(&mut m, &mut plans).unwrap();
        let before = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_tabled_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), before);
    }

    #[test]
    fn invalid_ownership_after_import_keeps_module_and_plans() {
        let (mut m, mut plans) = set();
        m.functions[1].blocks[0].instructions.push((
            ValueId(77),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        let before = format!("{m:?}/{plans:?}");
        assert!(prepare_arc_tabled_module(&mut m, &mut plans).is_err());
        assert_eq!(format!("{m:?}/{plans:?}"), before);
    }

    #[test]
    fn invalid_import_does_not_publish_partial_plans() {
        for case in 0..7 {
            let (mut m, mut plans) = set();
            match case {
                0 => {
                    plans.remove("folha");
                }
                1 => {
                    m.tables.pop();
                }
                2 => {
                    plans
                        .get_mut("folha")
                        .unwrap()
                        .tables
                        .landing_pads
                        .insert(BlockId(999));
                }
                3 => m.excecoes_por_tabelas = false,
                4 => {
                    plans.insert("obsoleto".into(), DartFunctionPlan::default());
                }
                5 => m.functions[1].symbol = "caller".into(),
                _ => m.arc_memory = false,
            }
            let before = format!("{m:?}/{plans:?}");
            let result = produce_module_arc_tables(&m, &mut plans);
            if case == 6 {
                assert_eq!(result.unwrap(), 0);
            } else {
                assert!(result.is_err(), "caso {case}");
            }
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }
}
