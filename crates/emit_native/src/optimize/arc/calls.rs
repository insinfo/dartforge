//! Resumos de convenção Dart vinculados à verificação do corpo do callee.
//! Contratos de externs e escopos fornecidos continuam sendo premissas.

use super::*;
use std::collections::{HashMap, HashSet};

/// Convenção de uma versão verificada de uma função Dart.
///
/// Campos privados impedem fabricar um resumo sem verificar o corpo.
/// Refaça o resumo após mudar a função ou seus contratos. Não certifica
/// proveniência, aliases de slots/keepalive nem os contratos externos fornecidos.
/// Barreiras conservadoras de borrows derivados são conferidas no corpo.
/// Chamadas diretas no corpo são conservadoramente consideradas falíveis,
/// assim como a conferência de pilha exigida pelo contexto explícito do
/// corpo/tabelas. Quadros calculados por vivacidade ainda exigem metadados.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let resumo = verificar_contrato_funcao_dart(&f, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # let _ = resumo;
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone)]
pub struct DartFunctionContract {
    symbol: String,
    parameters: Vec<Type>,
    ret: Type,
    may_fail: bool,
}

// Somente a preparação atômica do conjunto pode usar este resumo temporário.
// Nenhum corpo/contrato é publicado antes de verificar todas as convenções.
pub(super) fn provisional_summary(f: &Function, may_fail: bool) -> DartFunctionContract {
    DartFunctionContract {
        symbol: f.symbol.clone(),
        parameters: f.params.iter().map(|(_, _, ty)| *ty).collect(),
        ret: f.return_ty,
        may_fail,
    }
}

/// O resumo de uma função Dart de outra imagem preparada, pela chamada
/// `CallStatic` de `f`: a representação de cada operando (como o emissor a
/// declara), o retorno da chamada e falha possível.
pub(super) fn external_summary(f: &Function, symbol: &str, args: &[Operand], ret: Type) -> DartFunctionContract {
    let types: HashMap<ValueId, Type> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)))
        .collect();
    let ty = |o: &Operand| match o {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
    };
    DartFunctionContract {
        symbol: symbol.to_string(),
        parameters: args.iter().map(ty).map(|t| if t == Type::Void { Type::I64 } else { t }).collect(),
        ret,
        may_fail: true,
    }
}

/// Os resumos externos ([`external_summary`]) das chamadas diretas de
/// `functions` a símbolos fora de `locals`; chamadas do mesmo símbolo com
/// assinaturas diferentes são erro.
pub(super) fn external_summaries(functions: &[Function], locals: &HashSet<String>) -> Result<Vec<DartFunctionContract>, String> {
    let mut by_symbol: HashMap<String, DartFunctionContract> = HashMap::new();
    for f in functions {
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallStatic { symbol, args, ret_ty } = inst else { continue };
            if locals.contains(symbol) {
                continue;
            }
            let r = external_summary(f, symbol, args, *ret_ty);
            match by_symbol.get(symbol) {
                Some(previous) if (&previous.parameters, previous.ret) != (&r.parameters, r.ret) => {
                    return Err(format!("callee externo com assinaturas divergentes: {symbol}"));
                }
                Some(_) => {}
                None => {
                    by_symbol.insert(symbol.clone(), r);
                }
            }
        }
    }
    let mut summaries: Vec<_> = by_symbol.into_values().collect();
    summaries.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    Ok(summaries)
}

// O conjunto reutiliza este índice em todos os corpos. Reconstruí-lo por
// função tornaria a preparação quadrática no número de funções do SDK.
pub(super) struct DartFunctionIndex<'a> {
    by_symbol: HashMap<&'a String, &'a DartFunctionContract>,
}

impl<'a> DartFunctionIndex<'a> {
    pub(super) fn new(summaries: &'a [DartFunctionContract]) -> Result<Self, String> {
        let mut by_symbol = HashMap::with_capacity(summaries.len());
        for resumo in summaries {
            if by_symbol.insert(&resumo.symbol, resumo).is_some() {
                return Err(format!("resumo Dart repetido: {}", resumo.symbol));
            }
        }
        Ok(Self { by_symbol })
    }
}

/// Verifica a convenção Dart do corpo e extrai um resumo para chamadas diretas.
///
/// Os mapas fornecidos não são alterados. Parâmetros não Ref precisam de
/// classificação Trivial explícita/produzida; não são escalares pela largura.
/// O chamador deve usar o resumo apenas com esta versão do corpo e ABI.
///
/// # Erros
/// Falha do verificador Dart completo ou parâmetro não Ref não Trivial.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// verificar_contrato_funcao_dart(&f, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_dart_function_contract(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<DartFunctionContract, String> {
    let mut classes = classes.clone();
    let mut plan = plan.clone();
    produce_and_verify_dart_tokens(f, &mut classes, &mut plan, tables, scopes)?;
    for (v, _, ty) in &f.params {
        if *ty != Type::Ref && classes[v] != Ownership::Trivial {
            return Err(format!("parâmetro não Ref v{} não é Trivial", v.0));
        }
    }
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    for b in &f.blocks {
        if matches!(
            tables.exits.get(&b.id),
            Some(ExceptionExit::Throws | ExceptionExit::Resume)
        ) {
            continue;
        }
        if let Terminator::Return(op) = &b.terminator {
            let compatible = match op {
                None => f.return_ty == Type::Void,
                Some(op) => operand_has_type(op, f.return_ty, &types),
            };
            if !compatible {
                return Err(format!(
                    "representação de retorno Dart incompatível: {}",
                    f.symbol
                ));
            }
        }
    }
    let unknown_set = HashSet::new();
    let may_fail = crate::llvm::LlvmEmitter::requires_explicit_context(f, Some(tables))
        || !tables.exits.is_empty()
        || f.blocks.iter().any(|b| {
            matches!(b.terminator, Terminator::Throw(_))
                || b.instructions.iter().any(|(_, inst, _)| {
                    super::super::effects::instruction_throws(inst, &unknown_set)
                })
        });
    Ok(DartFunctionContract {
        symbol: f.symbol.clone(),
        parameters: f.params.iter().map(|(_, _, ty)| *ty).collect(),
        ret: f.return_ty,
        may_fail,
    })
}

fn operand_has_type(op: &Operand, expected: Type, types: &HashMap<ValueId, Type>) -> bool {
    match op {
        Operand::Val(v) => types.get(v) == Some(&expected),
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            expected == Type::Ref
        }
        Operand::Constant(Constant::Int(_)) => expected == Type::I64,
        Operand::Constant(Constant::Bool(_)) => expected == Type::I1,
        Operand::Constant(Constant::Double(_)) => expected == Type::F64,
        _ => false,
    }
}

/// Produz contratos de CallStatic usando resumos de corpos Dart verificados.
///
/// Argumentos Ref são emprestados durante a chamada; resultado Ref é Owned.
/// Demais argumentos SSA exigem classe Trivial, além do tipo exato da ABI.
/// Resultado não Ref é Trivial pela convenção verificada, não pela largura.
/// Falha conservadora exige CFG excepcional no verificador posterior.
/// Não cobre dispatch dinâmico, stubs importados ou retenções ocultas externas.
///
/// # Erros
/// Símbolo ausente/repetido, assinatura incompatível ou conflito nos mapas.
/// Nenhuma entrada é publicada em caso de erro. Não verifica o fluxo do caller.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let callee = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let resumo = verificar_contrato_funcao_dart(&callee, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// let mut caller = callee.clone();
/// caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "vazia".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// produzir_chamadas_dart(&caller, &[resumo], &mut HashMap::new(), &mut PlanoTokens::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_dart_calls(
    f: &Function,
    summaries: &[DartFunctionContract],
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let index = DartFunctionIndex::new(summaries)?;
    produce_calls(f, &index, classes, plan, false)
}

// Usado somente dentro da transação do conjunto: os resultados provisórios
// alimentam a classificação das demais instruções antes de conferir usos.
pub(super) fn produce_dart_calls_and_instructions(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    produce_calls(f, index, classes, plan, true)
}

/// Classifica parâmetros, chamadas Dart diretas e instruções com contrato,
/// sem resolver Phis nem conferir argumentos: a base para decidir que
/// entradas de Phi precisam de cópia (`phis.rs`). Não publica nada em erro.
pub(super) fn classify_without_phis(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(f, &mut new_classes)?;
    classify_statics(f, index, &mut new_classes, &mut new_plan)?;
    super::contracts::produce_contracts_without_phi(f, &mut new_classes, &mut new_plan)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

/// O resultado de cada chamada Dart direta pelo resumo do callee (Owned se
/// Ref, senão Trivial) e o efeito de falha; devolve os argumentos a conferir.
fn classify_statics<'f, 'a>(
    f: &'f Function,
    index: &DartFunctionIndex<'a>,
    new_classes: &mut HashMap<ValueId, Ownership>,
    new_plan: &mut TokenPlan,
) -> Result<Vec<(&'f Vec<Operand>, &'a DartFunctionContract)>, String> {
    let mut calls = Vec::new();
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = inst
            else {
                continue;
            };
            let resumo = index
                .by_symbol
                .get(symbol)
                .ok_or_else(|| format!("callee Dart sem resumo: {symbol}"))?;
            if args.len() != resumo.parameters.len() || *ret_ty != resumo.ret || ty != ret_ty {
                return Err(format!("assinatura Dart incompatível: {symbol}"));
            }
            let class = if *ret_ty == Type::Ref {
                Ownership::Owned
            } else {
                Ownership::Trivial
            };
            let effect = TokenEffect {
                may_fail: resumo.may_fail,
                ..Default::default()
            };
            if new_classes.get(v).is_some_and(|c| *c != class)
                || new_plan.instructions.get(v).is_some_and(|e| *e != effect)
            {
                return Err(format!("contrato Dart conflitante em v{}", v.0));
            }
            new_classes.insert(*v, class);
            new_plan.instructions.insert(*v, effect);
            calls.push((args, *resumo));
        }
    }
    Ok(calls)
}

fn produce_calls(
    f: &Function,
    index: &DartFunctionIndex<'_>,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    classify_instructions: bool,
) -> Result<(), String> {
    super::ssa::verify(f)?;
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    // A convenção dos parâmetros Ref não depende dos contratos das chamadas.
    produce_dart_ref_parameters(f, &mut new_classes)?;
    let calls = classify_statics(f, index, &mut new_classes, &mut new_plan)?;
    // Os resultados têm contratos semânticos dos callees, independentemente
    // da ordem física dos blocos. SSA já conferiu sua dominância nos usos.
    if classify_instructions {
        produce_arc_contracts(f, &mut new_classes, &mut new_plan)?;
    }
    for (args, resumo) in calls {
        for (op, expected) in args.iter().zip(&resumo.parameters) {
            let compatible = operand_has_type(op, *expected, &types)
                && match op {
                    Operand::Val(arg) => {
                        *expected == Type::Ref
                            || new_classes.get(arg) == Some(&Ownership::Trivial)
                    }
                    Operand::Constant(_) => true,
                };
            if !compatible {
                return Err(format!("argumento Dart incompatível: {}", resumo.symbol));
            }
        }
    }
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallible_call_cleans_temporary_without_producing_result_on_error() {
        let (mut callee, callee_plan) = identity();
        // O contrato da externa é premissa explícita; o resumo conservador
        // marca o callee como falível por conter uma chamada direta.
        callee.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "externa".into(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut external_plan = callee_plan;
        external_plan
            .instructions
            .insert(ValueId(1), TokenEffect::default());
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::from([(ValueId(1), Ownership::Owned)]),
            &external_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let mut caller = Function {
            symbol: "caller_falivel".into(),
            name: "caller_falivel".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallStatic {
                                symbol: callee.symbol,
                                args: vec![Operand::Val(ValueId(1))],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
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
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
            ],
        };
        let original = caller.clone();
        let mut tables = FunctionTables::default();
        tables.invocations.insert(ValueId(2), BlockId(1));
        tables.landing_pads.insert(BlockId(1));
        let mut plan = TokenPlan {
            ret: TokenReturn::Owned,
            ..Default::default()
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan).unwrap();
        assert_eq!(
            insert_arc_dart_exits(
                &mut caller,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 2)
        );
        for b in &caller.blocks[1..] {
            assert_eq!(b.instructions.len(), 1);
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        assert!(matches!(
            caller.blocks[2].terminator,
            Terminator::Return(Some(Operand::Val(ValueId(2))))
        ));
        produce_and_verify_dart_tokens(
            &caller,
            &mut classes,
            &mut plan,
            &tables,
            &ScopePlan::default(),
        )
        .unwrap();
        // O resultado não existe no erro, mesmo tendo tipo Ref e classe Owned.
        caller = original;
        caller.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        classes.retain(|v, _| v.0 <= 2);
        let before = format!("{caller:?}");
        let classes_before = classes.clone();
        let effects_before = plan.instructions.clone();
        let error = insert_arc_dart_exits(
            &mut caller,
            &mut classes,
            &mut plan,
            &tables,
            &ScopePlan::default(),
        )
        .unwrap_err();
        assert!(
            error.contains("ARC003")
                && error.contains("resultado v2 usado sem atravessar sua aresta de sucesso"),
            "{error}"
        );
        assert_eq!(format!("{caller:?}"), before);
        assert_eq!(classes, classes_before);
        assert_eq!(plan.instructions, effects_before);
    }

    #[test]
    fn scalar_call_chain_is_independent_of_physical_block_order() {
        let create = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(42)))),
            }],
        };
        let mut identity = create.clone();
        identity.symbol = "identidade_escalar".into();
        identity.params.push((ValueId(0), "x".into(), Type::I64));
        identity.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let plan = TokenPlan::default();
        let summaries = [
            verify_dart_function_contract(
                &create,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap(),
            verify_dart_function_contract(
                &identity,
                &HashMap::from([(ValueId(0), Ownership::Trivial)]),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap(),
        ];
        let mut caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(2)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: identity.symbol.clone(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::CallStatic {
                            symbol: create.symbol,
                            args: vec![],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(1)),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = plan;
        produce_dart_calls(&caller, &summaries, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        produce_and_verify_dart_tokens(
            &caller,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        // Mesma largura sem contrato semântico continua sendo insuficiente.
        caller
            .params
            .push((ValueId(3), "sem_contrato".into(), Type::I64));
        if let Instruction::CallStatic { args, .. } = &mut caller.blocks[1].instructions[0].1 {
            args[0] = Operand::Val(ValueId(3));
        }
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_dart_calls(&caller, &summaries, &mut classes, &mut plan)
                .unwrap_err()
                .contains("argumento")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn summary_checks_scalar_return_and_fallible_call_requires_exit() {
        let (mut callee, mut plan) = identity();
        callee.params.clear();
        callee.return_ty = Type::I64;
        callee.blocks[0].instructions[0] =
            (ValueId(1), Instruction::Const(Constant::Int(42)), Type::I64);
        plan.ret = TokenReturn::Trivial;
        verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        callee.blocks[0].instructions[0] = (
            ValueId(1),
            Instruction::Const(Constant::Bool(true)),
            Type::I1,
        );
        assert!(
            verify_dart_function_contract(
                &callee,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("representação")
        );
        let (mut callee, plan) = identity();
        callee.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "externa_com_contrato_fornecido".into(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut callee_plan = plan.clone();
        callee_plan
            .instructions
            .insert(ValueId(1), TokenEffect::default());
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::from([(ValueId(1), Ownership::Owned)]),
            &callee_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert!(resumo.may_fail);
        let (mut caller, mut plan) = identity();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol,
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan).unwrap();
        assert!(plan.instructions[&ValueId(1)].may_fail);
        assert!(
            produce_and_verify_dart_tokens(
                &caller,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
    }

    fn identity() -> (Function, TokenPlan) {
        (
            Function {
                symbol: "identidade".into(),
                name: "identidade".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            },
            TokenPlan {
                ret: TokenReturn::Owned,
                ..Default::default()
            },
        )
    }

    #[test]
    fn direct_call_receives_owned_from_verified_callee_without_manual_contract() {
        let (callee, callee_plan) = identity();
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &callee_plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert!(!resumo.may_fail);
        let (mut caller, mut plan) = identity();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produce_dart_calls(&caller, &[resumo.clone()], &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        assert_eq!(
            insert_arc_dart_exits(
                &mut caller,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        // Uma chamada desconhecida posterior não publica nem a primeira chamada.
        caller.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "ausente".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan)
                .unwrap_err()
                .contains("sem resumo")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn borrowed_body_and_same_width_argument_do_not_certify_call() {
        let (mut callee, plan) = identity();
        callee.blocks[0].instructions.clear();
        callee.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        assert!(
            verify_dart_function_contract(
                &callee,
                &HashMap::new(),
                &plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        let (callee, plan) = identity();
        let resumo = verify_dart_function_contract(
            &callee,
            &HashMap::new(),
            &plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let (mut caller, mut plan) = identity();
        caller.params[0].2 = Type::I64;
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        assert!(
            produce_dart_calls(&caller, &[resumo], &mut classes, &mut plan)
                .unwrap_err()
                .contains("argumento")
        );
        assert_eq!(classes.len(), 1);
        assert!(plan.instructions.is_empty());
    }
}
