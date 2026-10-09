//! Preparação atômica de um conjunto fechado de funções com ABI Dart.
//! Metadados e CFG excepcional devem corresponder aos corpos após otimização.

use super::*;
use std::collections::{HashMap, HashSet};

/// Metadados de uma função antes da inserção ARC nas saídas Dart.
///
/// Contratos não cobertos pelos produtores e limites lexicais são premissas
/// explícitas. O retorno Ref exige `tokens.retorno = RetornoTokens::Owned`.
/// Tabelas descrevem o CFG preparado, antes de materializar a emissão LLVM.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::{PlanoFuncaoDart, RetornoTokens};
/// let mut plano = PlanoFuncaoDart::default();
/// plano.tokens.retorno = RetornoTokens::Owned;
/// assert!(plano.classes.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct PlanoFuncaoDart {
    /// Classificações semânticas fornecidas; os produtores completam cobertura.
    pub classes: HashMap<ValueId, Ownership>,
    /// Consumos e convenção de retorno da mesma versão do CFG.
    pub tokens: PlanoTokens,
    /// Invokes/pousos excepcionais preparados para a função.
    pub tabelas: TabelasDaFuncao,
    /// Limites lexicais dos empréstimos, sem inferência por último uso.
    pub escopos: PlanoEscopos,
}

/// Insere e verifica retornos/cleanup de um conjunto fechado de funções Dart.
///
/// Cria contratos provisórios das chamadas internas e confere todos os corpos
/// antes de publicar qualquer função ou mapa. Usa o resumo de exceções do
/// pipeline, incluindo ciclos de chamadas e conferência de pilha. A ordem
/// das funções não define a disponibilidade dos contratos.
///
/// Exige um plano por símbolo e somente callees do conjunto. Argumentos
/// passam pela classificação ARC após classificar resultados das chamadas:
/// constantes/aritmética/Phis/runtime cobertos não exigem mapas manuais.
/// Parâmetros não Ref e operações não cobertas exigem contratos prévios.
/// Não prepara CFG/escopos, divide
/// arestas, resolve finally/cancelamento/suspensão nem materializa tabelas.
/// Descritores de slots e proveniência continuam premissas do lowering.
///
/// # Erros
/// Símbolo repetido, plano ausente/obsoleto ou falha de produção/inserção/
/// verificação de qualquer corpo. Todos os corpos e mapas ficam intactos.
/// Retorna os totais de (retenções de retorno, liberações nas saídas).
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut funcoes = vec![Function { symbol: "identidade".into(), name: "identidade".into(),
///     depuracao: None, params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] }];
/// let mut plano = PlanoFuncaoDart::default();
/// plano.tokens.retorno = RetornoTokens::Owned;
/// let mut planos = HashMap::from([("identidade".into(), plano)]);
/// assert_eq!(inserir_arc_funcoes_dart(&mut funcoes, &mut planos)?, (1, 0));
/// # Ok::<(), String>(())
/// ```
pub fn inserir_arc_funcoes_dart(
    funcoes: &mut [Function],
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(usize, usize), String> {
    let mut simbolos = HashSet::new();
    for f in funcoes.iter() {
        if !simbolos.insert(f.symbol.clone()) {
            return Err(format!("função Dart repetida: {}", f.symbol));
        }
        if !planos.contains_key(&f.symbol) {
            return Err(format!("função Dart sem plano: {}", f.symbol));
        }
    }
    if let Some(simbolo) = planos.keys().filter(|s| !simbolos.contains(*s)).min() {
        return Err(format!("plano Dart obsoleto: {simbolo}"));
    }
    let mut modulo = Module::new();
    modulo.functions = funcoes.to_vec();
    let nao_lancam = super::super::efeitos::nao_lancam(&modulo);
    let resumos: Vec<_> = modulo
        .functions
        .iter()
        .map(|f| super::chamadas::resumo_provisorio(f, !nao_lancam.contains(&f.symbol)))
        .collect();
    let mut novos_planos: HashMap<_, _> = planos
        .iter()
        .map(|(s, p)| (s.clone(), (p.classes.clone(), p.tokens.clone())))
        .collect();
    let mut total = (0, 0);
    for f in &mut modulo.functions {
        let original = &planos[&f.symbol];
        let (classes, tokens) = novos_planos.get_mut(&f.symbol).unwrap();
        super::chamadas::produzir_chamadas_e_instrucoes_dart(f, &resumos, classes, tokens)?;
        let (copias, drops) =
            inserir_arc_saidas_dart(f, classes, tokens, &original.tabelas, &original.escopos)?;
        verificar_contrato_funcao_dart(f, classes, tokens, &original.tabelas, &original.escopos)?;
        total.0 += copias;
        total.1 += drops;
    }
    // A convenção foi comprovada conjuntamente; os resumos provisórios não
    // escapam desta transação. Tabelas e limites mantêm seus IDs originais.
    funcoes.clone_from_slice(&modulo.functions);
    for (s, (classes, tokens)) in novos_planos {
        let destino = planos.get_mut(&s).unwrap();
        destino.classes = classes;
        destino.tokens = tokens;
    }
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn argumentos_escalares_com_phi_e_aritmetica_sao_produzidos_no_conjunto() {
        let criar = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(41)))),
            }],
        };
        let mut identidade = criar.clone();
        identidade.symbol = "identidade_escalar".into();
        identidade.params.push((ValueId(0), "x".into(), Type::I64));
        identidade.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(true)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::Phi {
                                ty: Type::I64,
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (BlockId(2), Operand::Val(ValueId(2))),
                                ],
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(4),
                            Instruction::Add(
                                Operand::Val(ValueId(3)),
                                Operand::Constant(Constant::Int(1)),
                            ),
                            Type::I64,
                        ),
                        (
                            ValueId(5),
                            Instruction::CallStatic {
                                symbol: identidade.symbol.clone(),
                                args: vec![Operand::Val(ValueId(4))],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Const(Constant::Int(41)),
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: criar.symbol.clone(),
                            args: vec![],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
            ],
        };
        let mut funcoes = vec![caller, identidade, criar];
        let mut planos: HashMap<_, _> = funcoes
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        // O parâmetro escalar do callee tem contrato semântico explícito.
        // Nenhuma classe/efeito é fornecido para o caller.
        planos
            .get_mut("identidade_escalar")
            .unwrap()
            .classes
            .insert(ValueId(0), Ownership::Trivial);
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (0, 0)
        );
        for v in 1..=5 {
            assert_eq!(planos["caller"].classes[&ValueId(v)], Ownership::Trivial);
        }
        let (mut ruins, mut planos_ruins) = (funcoes, planos);
        ruins[0].blocks[2].instructions[0] =
            (ValueId(2), Instruction::Const(Constant::Null), Type::Ref);
        let antes = format!("{ruins:?}");
        let planos_antes = format!("{planos_ruins:?}");
        assert!(
            inserir_arc_funcoes_dart(&mut ruins, &mut planos_ruins)
                .unwrap_err()
                .contains("Phi")
        );
        assert_eq!(format!("{ruins:?}"), antes);
        assert_eq!(format!("{planos_ruins:?}"), planos_antes);
    }

    fn conjunto() -> (Vec<Function>, HashMap<String, PlanoFuncaoDart>) {
        let folha = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let chamada = |simbolo: &str, destino: &str| {
            let mut f = folha.clone();
            f.symbol = simbolo.into();
            f.blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallStatic {
                    symbol: destino.into(),
                    args: vec![Operand::Val(ValueId(0))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            f
        };
        let funcoes = vec![
            chamada("caller", "intermediaria"),
            chamada("intermediaria", "folha"),
            folha,
        ];
        let planos = funcoes
            .iter()
            .map(|f| {
                let mut p = PlanoFuncaoDart::default();
                p.tokens.retorno = RetornoTokens::Owned;
                (f.symbol.clone(), p)
            })
            .collect();
        (funcoes, planos)
    }

    #[test]
    fn cadeia_dart_prepara_callees_posteriores_e_e_idempotente() {
        let (mut funcoes, mut planos) = conjunto();
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 0)
        );
        for f in &funcoes {
            let p = &planos[&f.symbol];
            assert_eq!(p.classes[&ValueId(1)], Ownership::Owned);
            produzir_e_verificar_tokens_dart(
                f,
                &mut p.classes.clone(),
                &mut p.tokens.clone(),
                &p.tabelas,
                &p.escopos,
            )
            .unwrap();
        }
        assert!(matches!(
            funcoes[2].blocks[0].instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        let antes = format!("{funcoes:?}");
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{funcoes:?}"), antes);
    }

    #[test]
    fn corpo_invalido_posterior_ou_recursao_sem_saida_nao_publicam_conjunto() {
        let (mut funcoes, mut planos) = conjunto();
        funcoes[2].blocks[0].instructions.push((
            ValueId(1),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        let antes = format!("{funcoes:?}");
        let planos_antes = format!("{planos:?}");
        assert!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos)
                .unwrap_err()
                .contains("Owned")
        );
        assert_eq!(format!("{funcoes:?}"), antes);
        assert_eq!(format!("{planos:?}"), planos_antes);
        let (mut funcoes, mut planos) = conjunto();
        if let Instruction::CallStatic { symbol, .. } = &mut funcoes[1].blocks[0].instructions[0].1
        {
            *symbol = "caller".into();
        }
        let antes = format!("{funcoes:?}");
        let planos_antes = format!("{planos:?}");
        assert!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos)
                .unwrap_err()
                .contains("saídas excepcionais")
        );
        assert_eq!(format!("{funcoes:?}"), antes);
        assert_eq!(format!("{planos:?}"), planos_antes);
    }
}
