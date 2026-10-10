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
#[derive(Debug, Clone, Default)]
pub struct PlanoFuncaoDart {
    /// Classificações semânticas fornecidas; os produtores completam cobertura.
    pub classes: HashMap<ValueId, Ownership>,
    /// Consumos e convenção de retorno da mesma versão do CFG.
    pub tokens: PlanoTokens,
    /// Invokes/pousos excepcionais preparados para a função.
    pub tabelas: TabelasDaFuncao,
    /// Limites lexicais dos empréstimos, sem inferência por último uso.
    pub escopos: PlanoEscopos,
    /// Inventário validado após inserção; inclui somente owners da aresta de erro.
    /// Recalculado atomicamente; ainda não gera cleanup de unwind estrangeiro.
    pub owners_no_pouso: HashMap<BlockId, Vec<ValueId>>,
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
/// Separa saídas Guarda em sucesso/erro e transporta os limites de saída.
/// Demais CFG/escopos precisam estar preparados. Não divide arestas gerais,
/// resolve finally/cancelamento/suspensão nem materializa tabelas.
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
    inserir(funcoes, planos, false)
}

/// Prepara invokes ausentes e insere ARC num conjunto fechado de funções Dart.
///
/// Chamadas diretas falíveis sem pouso fornecido ganham uma continuação
/// normal e um pouso que libera owners. Em Unix, propaga por Retoma usando
/// o objeto Itanium original; em Windows, usa Lanca até a integração SEH.
/// Um sítio já preparado conserva seu tratador. Phis e limites lexicais
/// seguem o terminador original; a saída excepcional fecha os escopos ativos.
/// Conferência de pendência logo após chamada exige sítio preparado, pois
/// seu desvio pode representar catch/finally que não pode ser omitido.
/// Não cria catch/finally nem fecha quadros proprietários abertos: esses
/// protocolos precisam de cleanup explícito. Também não cobre dispatch
/// indireto, cancelamento, suspensão ou classificação semântica do lowering.
/// Conferência de pilha por quadros de raízes calculados por vivacidade
/// exige a marca `tabelas.confere_pilha` enquanto não integrar esse inventário.
/// Os demais contratos são os de [`inserir_arc_funcoes_dart`].
/// Retorna (retenções de retorno, liberações nas saídas).
///
/// # Erros
/// CFG/SSA ou limites inválidos, IDs esgotados ou qualquer erro de produção
/// e verificação ARC. Nenhum corpo ou plano é publicado em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let folha = Function { symbol: "folha".into(), name: "folha".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// let mut caller = folha.clone();
/// caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "folha".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// let mut p = PlanoFuncaoDart::default();
/// p.tabelas.confere_pilha = true;
/// let mut planos = HashMap::from([("folha".into(), p),
///     ("caller".into(), PlanoFuncaoDart::default())]);
/// preparar_arc_funcoes_dart(&mut [caller, folha], &mut planos)?;
/// assert_eq!(planos["caller"].tabelas.invocacoes.len(), 1);
/// # Ok::<(), String>(())
/// ```
pub fn preparar_arc_funcoes_dart(
    funcoes: &mut [Function],
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(usize, usize), String> {
    inserir(funcoes, planos, true)
}

fn inserir(
    funcoes: &mut [Function],
    planos: &mut HashMap<String, PlanoFuncaoDart>,
    preparar_chamadas: bool,
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
    let mut nao_lancam = super::super::efeitos::nao_lancam(&modulo);
    // Contexto explícito implica conferência de pilha no LLVM. Inclui
    // leituras/limpeza de pendência e pousos, além das marcas nas tabelas.
    // Propaga esta falha do prólogo aos chamadores em O(V+E).
    let falha_no_prologo_ou_saida = |f: &Function| {
        let t = &planos[&f.symbol].tabelas;
        crate::llvm::LlvmEmitter::exige_contexto_explicito(f, Some(t)) || !t.saidas.is_empty()
    };
    if modulo.functions.iter().any(falha_no_prologo_ou_saida) {
        let mut chamadores: HashMap<&str, Vec<&str>> = HashMap::new();
        for f in &modulo.functions {
            for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                if let Instruction::CallStatic { symbol, .. } = inst {
                    chamadores.entry(symbol).or_default().push(&f.symbol);
                }
            }
        }
        let mut fila: Vec<_> = modulo
            .functions
            .iter()
            .filter(|f| falha_no_prologo_ou_saida(f))
            .map(|f| f.symbol.as_str())
            .collect();
        while let Some(simbolo) = fila.pop() {
            if nao_lancam.remove(simbolo) {
                if let Some(anteriores) = chamadores.get(simbolo) {
                    fila.extend(anteriores.iter().copied());
                }
            }
        }
    }
    let mut novos_planos = planos.clone();
    if preparar_chamadas {
        for f in &mut modulo.functions {
            super::invocacoes::preparar(f, novos_planos.get_mut(&f.symbol).unwrap(), &nao_lancam)?;
        }
    }
    let resumos: Vec<_> = modulo
        .functions
        .iter()
        .map(|f| super::chamadas::resumo_provisorio(f, !nao_lancam.contains(&f.symbol)))
        .collect();
    let indice = super::chamadas::IndiceFuncoesDart::novo(&resumos)?;
    let mut total = (0, 0);
    for f in &mut modulo.functions {
        let novo = novos_planos.get_mut(&f.symbol).unwrap();
        super::saidas::separar_guardas(f, novo)?;
        super::tokens::normalizar_saidas_lanca(f, &novo.tabelas)?;
        super::chamadas::produzir_chamadas_e_instrucoes_dart(
            f,
            &indice,
            &mut novo.classes,
            &mut novo.tokens,
        )?;
        let (copias, drops) = inserir_arc_saidas_dart(
            f,
            &mut novo.classes,
            &mut novo.tokens,
            &novo.tabelas,
            &novo.escopos,
        )?;
        verificar_contrato_funcao_dart(
            f,
            &novo.classes,
            &novo.tokens,
            &novo.tabelas,
            &novo.escopos,
        )?;
        novo.owners_no_pouso =
            tokens_na_entrada_dos_pousos(f, &novo.classes, &novo.tabelas, &novo.tokens)?;
        total.0 += copias;
        total.1 += drops;
    }
    // A convenção foi comprovada conjuntamente; os resumos provisórios não
    // escapam desta transação. Tabelas/limites incluem as saídas separadas.
    funcoes.clone_from_slice(&modulo.functions);
    *planos = novos_planos;
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contexto_implicito_no_corpo_propaga_unwind_do_prologo() {
        for (name, ret_ty) in [
            ("dartforge_exception_pending", Type::I8),
            ("dartforge_exception_clear", Type::Void),
        ] {
            let (mut funcoes, mut planos) = conjunto();
            funcoes[2].blocks[0].instructions.push((
                ValueId(2),
                Instruction::CallRuntime {
                    name: name.into(),
                    args: vec![],
                    ret_ty,
                },
                ret_ty,
            ));
            let antes = format!("{funcoes:?}");
            let planos_antes = format!("{planos:?}");
            let erro = inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap_err();
            assert!(
                erro.contains("caller") && erro.contains("saídas excepcionais"),
                "{erro}"
            );
            assert_eq!(format!("{funcoes:?}"), antes);
            assert_eq!(format!("{planos:?}"), planos_antes);
            // A folha isolada prepara seu retorno; o resumo deve continuar
            // falível mesmo sem a marca confere_pilha ou saída Lanca.
            funcoes.drain(..2);
            planos.retain(|s, _| s == "folha");
            assert_eq!(
                inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
                (1, 0)
            );
            let p = &planos["folha"];
            let resumo = verificar_contrato_funcao_dart(
                &funcoes[0],
                &p.classes,
                &p.tokens,
                &p.tabelas,
                &p.escopos,
            )
            .unwrap();
            let (caller, _) = conjunto();
            let caller = caller[1].clone();
            let mut classes = HashMap::new();
            let mut tokens = PlanoTokens::default();
            produzir_chamadas_dart(&caller, &[resumo], &mut classes, &mut tokens).unwrap();
            assert!(tokens.instrucoes[&ValueId(1)].pode_falhar);
            // O emissor também materializa a conferência na folha.
            let mut modulo = Module::new();
            modulo.functions = funcoes;
            modulo.excecoes_por_tabelas = true;
            modulo.tabelas.push(p.tabelas.clone());
            let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
            let corpo = ir
                .split("define i64 @folha(")
                .nth(1)
                .unwrap()
                .split("\n}")
                .next()
                .unwrap();
            let estouro = corpo
                .split("pilha.estouro:")
                .nth(1)
                .unwrap()
                .split("pilha.ok:")
                .next()
                .unwrap();
            assert!(estouro.contains("call void @df.lancar()"));
        }
    }

    #[test]
    fn conferencia_de_pilha_propaga_falha_e_exige_aresta_no_chamador() {
        let (mut funcoes, mut planos) = conjunto();
        planos.get_mut("folha").unwrap().tabelas.confere_pilha = true;
        let antes = format!("{funcoes:?}");
        let planos_antes = format!("{planos:?}");
        let erro = inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap_err();
        assert!(
            erro.contains("caller") && erro.contains("saídas excepcionais"),
            "{erro}"
        );
        assert_eq!(format!("{funcoes:?}"), antes);
        assert_eq!(format!("{planos:?}"), planos_antes);
        // Com a saída preparada, o resultado só é transferido no sucesso.
        let (mut funcoes, mut planos) = conjunto();
        funcoes.remove(1);
        planos.remove("intermediaria");
        let caller = &mut funcoes[0];
        if let Instruction::CallStatic { symbol, .. } = &mut caller.blocks[0].instructions[0].1 {
            *symbol = "folha".into();
        }
        caller.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        caller.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
        });
        caller.blocks.push(BasicBlock {
            id: BlockId(2),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        });
        let tabelas = &mut planos.get_mut("caller").unwrap().tabelas;
        tabelas.invocacoes.insert(ValueId(1), BlockId(1));
        tabelas.pousos.insert(BlockId(1));
        planos.get_mut("folha").unwrap().tabelas.confere_pilha = true;
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 0)
        );
        assert!(planos["caller"].tokens.instrucoes[&ValueId(1)].pode_falhar);
        let folha = &planos["folha"];
        let resumo = verificar_contrato_funcao_dart(
            &funcoes[1],
            &folha.classes,
            &folha.tokens,
            &folha.tabelas,
            &folha.escopos,
        )
        .unwrap();
        let mut classes = HashMap::new();
        let mut tokens = PlanoTokens::default();
        produzir_chamadas_dart(&funcoes[0], &[resumo], &mut classes, &mut tokens).unwrap();
        assert!(tokens.instrucoes[&ValueId(1)].pode_falhar);
    }

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
        planos
            .get_mut("caller")
            .unwrap()
            .owners_no_pouso
            .insert(BlockId(99), vec![ValueId(99)]);
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (0, 0)
        );
        assert!(planos["caller"].owners_no_pouso.is_empty());
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
