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
    /// Recalculado atomicamente e ligado ao certificado de cleanup estrangeiro.
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
/// Boxing escalar explícito vira fábrica Owned para int/double; bool usa caixas
/// estáticas Trivial. SIMD e outras representações exigem produtores próprios.
/// Unbox int/double vira chamada Borrow falível auditada; exige saída de erro.
/// Unbox bool conserva o helper LLVM que converte u8/I1, com o mesmo contrato
/// Borrow falível e sua checagem de pendência, sem mudar IDs do CFG.
/// Parâmetros F64/I1/I8 são escalares Trivial. I64/Ptr e operações não
/// cobertas exigem contratos prévios; largura não prova semântica gerenciada.
/// Separa saídas Guarda em sucesso/erro e transporta os limites de saída.
/// Demais CFG/escopos precisam estar preparados. Não divide arestas gerais,
/// resolve finally/cancelamento/suspensão nem materializa tabelas.
/// Descritores de slots e proveniência continuam premissas do lowering.
/// Pousos catch ganham inventário certificado para cleanup estrangeiro Unix;
/// quadros locais são fechados em LIFO nesse braço, segundo a pilha validada.
/// Aliases e quadros importados permanecem fora desse protocolo.
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

/// Prepara atomicamente os fatos do lowering e o conjunto fechado de um módulo ARC.
///
/// Tracing devolve (0, 0) sem tocar corpos ou planos. Em ARC, produz parâmetros
/// escalares/RTI nas cópias dos planos e prepara/verifica todas as funções antes
/// de publicar. Os planos devem descrever o CFG final após as otimizações.
/// Confere também os layouts de campos declarados fornecidos pelo lowering:
/// classe única, quantidade coerente e representações válidas. Isso não prova
/// o tipo/forma física dos receivers nem a versão/pins de recarga.
/// Retorno Ref exige convenção Owned e operações não cobertas exigem contratos
/// explícitos. Não materializa tabelas nem chama este passe no pipeline padrão.
/// Demais limites são os de [`preparar_arc_funcoes_dart`].
///
/// # Erros
/// Fatos inválidos ou qualquer erro de preparação do conjunto. Nenhum corpo
/// ou plano é publicado, incluindo fatos de parâmetros previamente válidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new(); m.memoria_arc = true;
/// m.functions.push(Function { symbol: "caixa".into(), name: "caixa".into(),
///     depuracao: None, params: vec![(ValueId(0), "numero".into(), Type::I64)],
///     return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0),
///     instructions: vec![(ValueId(1), Instruction::Box { op: Operand::Val(ValueId(0)),
///     from: Type::I64 }, Type::Ref)], terminator: Terminator::Return(None) }] });
/// m.parametros_escalares_dart.insert("caixa".into(), HashSet::from([ValueId(0)]));
/// let mut planos = HashMap::from([("caixa".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos)?, (0, 1));
/// # Ok::<(), String>(())
/// ```
pub fn preparar_arc_modulo_dart(
    modulo: &mut Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(usize, usize), String> {
    if !modulo.memoria_arc {
        return Ok((0, 0));
    }
    super::layouts::verificar(modulo)?;
    let mut novos = planos.clone();
    produzir_parametros_escalares_do_lowering(modulo, &mut novos)?;
    produzir_parametros_rti_do_lowering(modulo, &mut novos)?;
    // O conjunto já mantém os corpos privados até verificar todos eles.
    // Após seu sucesso, a atribuição dos planos não introduz nova falha.
    let total = preparar_arc_funcoes_dart(&mut modulo.functions, &mut novos)?;
    *planos = novos;
    Ok(total)
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
    for f in &mut modulo.functions {
        super::caixas::preparar(f)?;
    }
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
    for f in &modulo.functions {
        let classes = &mut novos_planos.get_mut(&f.symbol).unwrap().classes;
        for (v, _, ty) in &f.params {
            // Essas representações não transportam handles nem slots fortes.
            // I64 e Ptr continuam dependentes da proveniência do lowering.
            if matches!(ty, Type::F64 | Type::I1 | Type::I8) {
                if classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "parâmetro escalar {} v{} exige Trivial",
                        f.symbol, v.0
                    ));
                }
                classes.insert(*v, Ownership::Trivial);
            }
        }
    }
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
        novo.tabelas.cleanup_estrangeiro = CleanupEstrangeiro::novo(f, novo)?;
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
    fn campo_emprestado_exige_copia_antes_de_substituir_aresta() {
        for copia_antes in [false, true] {
            let leitura = (
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(39)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let troca = (
                ValueId(2),
                Instruction::CallRuntime {
                    name: "dartforge_arc_gravar_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(39)), Type::I64),
                        (Operand::Constant(Constant::Null), Type::Ref),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            let copia = (
                ValueId(3),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(1)),
                },
                Type::Ref,
            );
            let mut instrucoes = vec![leitura];
            if copia_antes {
                instrucoes.extend([copia, troca]);
            } else {
                instrucoes.extend([troca, copia]);
            }
            instrucoes.push((
                ValueId(4),
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(3)),
                },
                Type::Void,
            ));
            let f = Function {
                symbol: "borrow_campo".into(),
                name: "borrow_campo".into(),
                depuracao: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: instrucoes,
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OrigemOwner::Chamador,
                    escopo: 0,
                },
            )]);
            let mut plano = PlanoTokens::default();
            produzir_contratos_arc(&f, &mut classes, &mut plano).unwrap();
            let resultado = verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano);
            if copia_antes {
                resultado.unwrap();
            } else {
                assert!(resultado.unwrap_err().contains("empréstimo invalidado"));
            }
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(f);
            let mut planos = HashMap::from([("borrow_campo".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}/{planos:?}");
            if copia_antes {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
            } else {
                assert!(
                    preparar_arc_modulo_dart(&mut m, &mut planos)
                        .unwrap_err()
                        .contains("empréstimo invalidado")
                );
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            }
        }
    }

    #[test]
    fn leitura_ref_do_campo_empresta_receiver_e_copia_retorno_owned() {
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = true;
            let ret_ty = if caso == 1 { Type::I64 } else { Type::Ref };
            m.functions.push(Function {
                symbol: "leitura_ref".into(),
                name: "leitura_ref".into(),
                depuracao: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: if caso == 2 {
                                "dartforge_object_get"
                            } else {
                                "dartforge_arc_ler_campo_ref_v1"
                            }
                            .into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(39)), Type::I64),
                            ],
                            ret_ty,
                        },
                        ret_ty,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut p = PlanoFuncaoDart::default();
            p.tokens.retorno = RetornoTokens::Owned;
            if caso == 3 {
                p.classes.insert(ValueId(1), Ownership::Owned);
            }
            let mut planos = HashMap::from([("leitura_ref".into(), p)]);
            let antes = format!("{m:?}/{planos:?}");
            if caso != 0 {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (1, 0)
                );
                assert_eq!(
                    planos["leitura_ref"].classes[&ValueId(1)],
                    Ownership::Borrowed {
                        owner: OrigemOwner::Valor(ValueId(0)),
                        escopo: 0
                    }
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_ler_campo_ref_v1(i64 %v0, i64 39)"));
                assert!(ir.contains("call void @dartforge_arc_retain(i64 %v1)"));
                let preparado = format!("{m:?}/{planos:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{planos:?}"), preparado);
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }

    #[test]
    fn gravacao_de_campos_preserva_owners_e_recusa_assinatura_falseada() {
        let inteiro = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
        let referencia = |n| (Operand::Val(ValueId(n)), Type::Ref);
        let chamada = |id, nome: &str, args, ret_ty| {
            (
                ValueId(id),
                Instruction::CallRuntime {
                    name: nome.into(),
                    args,
                    ret_ty,
                },
                ret_ty,
            )
        };
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(Function {
                symbol: "campos".into(),
                name: "campos".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        chamada(
                            0,
                            "dartforge_arc_objeto_owned_v1",
                            vec![inteiro(123), inteiro(40)],
                            Type::Ref,
                        ),
                        chamada(
                            1,
                            "dartforge_arc_box_int_owned_v1",
                            vec![inteiro(i64::MAX)],
                            Type::Ref,
                        ),
                        chamada(
                            2,
                            if caso == 3 {
                                "dartforge_object_set"
                            } else {
                                "dartforge_arc_gravar_campo_ref_v1"
                            },
                            vec![
                                referencia(0),
                                inteiro(37),
                                if caso == 1 {
                                    (Operand::Val(ValueId(1)), Type::I64)
                                } else {
                                    referencia(1)
                                },
                            ],
                            Type::Void,
                        ),
                        chamada(
                            3,
                            "dartforge_arc_gravar_campo_escalar_v1",
                            vec![
                                referencia(0),
                                inteiro(39),
                                if caso == 2 {
                                    referencia(1)
                                } else {
                                    inteiro(i64::MAX)
                                },
                            ],
                            Type::Void,
                        ),
                        chamada(
                            4,
                            "dartforge_arc_ler_campo_escalar_v1",
                            vec![referencia(0), inteiro(39)],
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut planos = HashMap::from([("campos".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}/{planos:?}");
            if caso != 0 {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 2)
                );
                let p = &planos["campos"];
                for id in [0, 1] {
                    assert_eq!(p.classes[&ValueId(id)], Ownership::Owned);
                }
                for id in [2, 3, 4] {
                    assert_eq!(p.classes[&ValueId(id)], Ownership::Trivial);
                }
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains(
                    "call void @dartforge_arc_gravar_campo_ref_v1(i64 %v0, i64 37, i64 %v1)"
                ));
                assert!(ir.contains("call void @dartforge_arc_gravar_campo_escalar_v1(i64 %v0, i64 39, i64 9223372036854775807)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v1)"));
                let preparado = format!("{m:?}/{planos:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{planos:?}"), preparado);
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }

    #[test]
    fn fabrica_de_instancia_publica_owned_e_cleanup_sem_classificar_alocador_legado() {
        for caso in 0..4 {
            let mut m = Module::new();
            m.memoria_arc = true;
            let ret_ty = if caso == 1 { Type::I64 } else { Type::Ref };
            let nome = if caso == 3 {
                "dartforge_object_new"
            } else {
                "dartforge_arc_objeto_owned_v1"
            };
            m.functions.push(Function {
                symbol: "instancia".into(),
                name: "instancia".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::CallRuntime {
                                name: nome.into(),
                                args: vec![
                                    (Operand::Constant(Constant::Int(123)), Type::I64),
                                    (
                                        Operand::Constant(Constant::Int(40)),
                                        if caso == 2 { Type::Ref } else { Type::I64 },
                                    ),
                                ],
                                ret_ty,
                            },
                            ret_ty,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_ler_campo_escalar_v1".into(),
                                args: vec![
                                    (Operand::Val(ValueId(0)), Type::Ref),
                                    (Operand::Constant(Constant::Int(39)), Type::I64),
                                ],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(2),
                            Instruction::Bitcast {
                                op: Operand::Val(ValueId(1)),
                                to: Type::F64,
                            },
                            Type::F64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut planos = HashMap::from([("instancia".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}/{planos:?}");
            if caso != 0 {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 1)
                );
                let p = &planos["instancia"];
                assert_eq!(p.classes[&ValueId(0)], Ownership::Owned);
                assert_eq!(p.classes[&ValueId(1)], Ownership::Trivial);
                assert_eq!(p.classes[&ValueId(2)], Ownership::Trivial);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(
                    ir.contains("declare i64 @dartforge_arc_objeto_owned_v1(i64, i64) nounwind")
                );
                assert!(ir.contains("call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
                let preparado = format!("{m:?}/{planos:?}");
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{planos:?}"), preparado);
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }

    #[test]
    fn modulo_publica_fatos_e_cleanup_juntos_e_tracing_nao_prepara() {
        for falha in 0..3 {
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), Type::I64);
            fs[0].params = vec![
                (ValueId(3), "numero".into(), Type::I64),
                (ValueId(4), "$tipos".into(), Type::I64),
            ];
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions = fs;
            m.parametros_escalares_dart
                .insert("boxing".into(), HashSet::from([ValueId(3)]));
            m.parametros_rti_dart
                .insert("boxing".into(), HashSet::from([ValueId(4)]));
            if falha == 1 {
                m.parametros_rti_dart
                    .get_mut("boxing")
                    .unwrap()
                    .insert(ValueId(99));
            }
            if falha == 2 {
                m.functions[0].blocks[0].instructions.push((
                    ValueId(9),
                    Instruction::CallRuntime {
                        name: "sem_contrato".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
            let antes = format!("{m:?}/{ps:?}");
            if falha == 0 {
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 1));
                assert_eq!(ps["boxing"].classes[&ValueId(3)], Ownership::Trivial);
                assert_eq!(ps["boxing"].classes[&ValueId(4)], Ownership::Trivial);
                assert!(matches!(
                    m.functions[0].blocks[0].instructions.last().unwrap().1,
                    Instruction::ArcDrop { .. }
                ));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
            } else {
                assert!(preparar_arc_modulo_dart(&mut m, &mut ps).is_err());
                assert_eq!(format!("{m:?}/{ps:?}"), antes);
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{ps:?}");
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}/{ps:?}"), antes);
        }
    }

    #[test]
    fn unboxing_escalar_preserva_borrow_e_exige_type_error_preparado() {
        for ty in [Type::I64, Type::F64, Type::I1] {
            let f = Function {
                symbol: "unboxing".into(),
                name: "unboxing".into(),
                depuracao: None,
                params: vec![(ValueId(0), "valor".into(), Type::Ref)],
                return_ty: ty,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::Unbox {
                                    op: Operand::Val(ValueId(0)),
                                    to: ty,
                                },
                                ty,
                            ),
                            (
                                ValueId(2),
                                Instruction::CallRuntime {
                                    name: "dartforge_exception_pending".into(),
                                    args: vec![],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            ),
                            (
                                ValueId(3),
                                Instruction::ICmp(
                                    ICmpOp::Ne,
                                    Operand::Val(ValueId(2)),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(None),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut p = PlanoFuncaoDart::default();
            p.tabelas.saidas.insert(BlockId(1), SaidaPorExcecao::Lanca);
            let mut fs = vec![f.clone()];
            let mut ps = HashMap::from([("unboxing".into(), p)]);
            assert_eq!(inserir_arc_funcoes_dart(&mut fs, &mut ps).unwrap(), (0, 0));
            assert_eq!(ps["unboxing"].classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(ps["unboxing"].tokens.pendencias[&ValueId(1)], BlockId(1));
            if ty == Type::I1 {
                assert!(matches!(
                    fs[0].blocks[0].instructions[0].1,
                    Instruction::Unbox { to: Type::I1, .. }
                ));
                let p = &ps["unboxing"];
                let mut adulterado = p.tokens.clone();
                adulterado
                    .instrucoes
                    .get_mut(&ValueId(1))
                    .unwrap()
                    .pode_falhar = false;
                adulterado.pendencias.remove(&ValueId(1));
                assert!(
                    verificar_tokens(&fs[0], &p.classes, &p.tabelas, &adulterado)
                        .unwrap_err()
                        .contains("ownership.tsv")
                );
                let mut m = Module::new();
                m.functions = fs.clone();
                m.excecoes_por_tabelas = true;
                m.tabelas.push(p.tabelas.clone());
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("%v1 = call i1 @df.desencaixa_bool(i64 %v0)"));
                assert!(ir.contains("%u = call i8 @dartforge_unbox_bool(i64 %r)"));
                assert!(ir.contains("%x = icmp ne i8 %u, 0"));
            } else {
                assert!(
                    matches!(&fs[0].blocks[0].instructions[0].1, Instruction::CallRuntime { name, .. } if name == if ty == Type::I64 { "dartforge_unbox_int" } else { "dartforge_unbox_double" })
                );
            }
            let mut owned = f.clone();
            owned.params.clear();
            if let Instruction::Unbox { op, .. } = &mut owned.blocks[0].instructions[0].1 {
                *op = Operand::Val(ValueId(4));
            }
            owned.blocks[0].instructions.insert(
                0,
                (
                    ValueId(4),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
            );
            let mut plano_owned = PlanoFuncaoDart::default();
            plano_owned
                .tabelas
                .saidas
                .insert(BlockId(1), SaidaPorExcecao::Lanca);
            let mut owned = vec![owned];
            let mut planos_owned = HashMap::from([("unboxing".into(), plano_owned)]);
            assert_eq!(
                inserir_arc_funcoes_dart(&mut owned, &mut planos_owned).unwrap(),
                (0, 2)
            );
            for b in &owned[0].blocks[1..] {
                assert_eq!(
                    b.instructions
                        .iter()
                        .filter(|(_, i, _)| matches!(
                            i,
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(4))
                            }
                        ))
                        .count(),
                    1
                );
            }
            let mut fs = vec![f];
            fs[0].blocks.truncate(1);
            fs[0].blocks[0].instructions.truncate(1);
            fs[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            let mut ps = HashMap::from([("unboxing".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{fs:?}/{ps:?}");
            assert!(inserir_arc_funcoes_dart(&mut fs, &mut ps).is_err());
            assert_eq!(format!("{fs:?}/{ps:?}"), antes);
        }
    }

    #[test]
    fn parametros_double_e_bool_preparam_boxing_sem_contrato_manual() {
        for ty in [Type::F64, Type::I1, Type::I8] {
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), ty);
            fs[0].params.push((ValueId(3), "valor".into(), ty));
            assert_eq!(
                inserir_arc_funcoes_dart(&mut fs, &mut ps).unwrap(),
                (0, usize::from(ty == Type::F64))
            );
            assert_eq!(ps["boxing"].classes[&ValueId(3)], Ownership::Trivial);
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), ty);
            fs[0].params.push((ValueId(3), "valor".into(), ty));
            ps.get_mut("boxing")
                .unwrap()
                .classes
                .insert(ValueId(3), Ownership::Owned);
            let antes = format!("{fs:?}/{ps:?}");
            assert!(
                inserir_arc_funcoes_dart(&mut fs, &mut ps)
                    .unwrap_err()
                    .contains("exige Trivial")
            );
            assert_eq!(format!("{fs:?}/{ps:?}"), antes);
        }
        // A mesma preparação não autoriza presumir o significado de I64.
        let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), Type::I64);
        fs[0].params.push((ValueId(3), "valor".into(), Type::I64));
        let antes = format!("{fs:?}/{ps:?}");
        assert!(inserir_arc_funcoes_dart(&mut fs, &mut ps).is_err());
        assert_eq!(format!("{fs:?}/{ps:?}"), antes);
        ps.get_mut("boxing")
            .unwrap()
            .classes
            .insert(ValueId(3), Ownership::Trivial);
        assert_eq!(inserir_arc_funcoes_dart(&mut fs, &mut ps).unwrap(), (0, 1));
    }

    fn boxing(op: Operand, from: Type) -> (Vec<Function>, HashMap<String, PlanoFuncaoDart>) {
        let f = Function {
            symbol: "boxing".into(),
            name: "boxing".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(ValueId(0), Instruction::Box { op, from }, Type::Ref)],
                terminator: Terminator::Return(None),
            }],
        };
        (
            vec![f],
            HashMap::from([("boxing".into(), PlanoFuncaoDart::default())]),
        )
    }

    #[test]
    fn boxing_owned_libera_mortais_e_preserva_bool_estatico() {
        for (constante, from, fabrica) in [
            (
                Constant::Int(i64::MAX),
                Type::I64,
                Some("dartforge_arc_box_int_owned_v1"),
            ),
            (
                Constant::Double(-0.0),
                Type::F64,
                Some("dartforge_arc_box_double_owned_v1"),
            ),
            (
                Constant::Double(f64::from_bits(0x7ff80000deadbeef)),
                Type::F64,
                Some("dartforge_arc_box_double_owned_v1"),
            ),
            (Constant::Bool(false), Type::I1, None),
        ] {
            let (mut fs, mut ps) = boxing(Operand::Constant(constante), from);
            assert_eq!(
                inserir_arc_funcoes_dart(&mut fs, &mut ps).unwrap(),
                (0, usize::from(fabrica.is_some()))
            );
            assert_eq!(
                ps["boxing"].classes[&ValueId(0)],
                if fabrica.is_some() {
                    Ownership::Owned
                } else {
                    Ownership::Trivial
                }
            );
            if let Some(nome) = fabrica {
                assert!(
                    matches!(&fs[0].blocks[0].instructions[0].1, Instruction::CallRuntime { name, .. } if name == nome)
                );
                assert!(matches!(
                    &fs[0].blocks[0].instructions[1].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(0))
                    }
                ));
            } else {
                assert!(matches!(
                    fs[0].blocks[0].instructions[0].1,
                    Instruction::Box { .. }
                ));
            }
        }
    }

    #[test]
    fn boxing_owned_transfere_retorno_sem_reter_e_rejeita_formas_invalidas_atomicamente() {
        let (mut fs, mut ps) = boxing(Operand::Constant(Constant::Double(3.25)), Type::F64);
        fs[0].return_ty = Type::Ref;
        fs[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        ps.get_mut("boxing").unwrap().tokens.retorno = RetornoTokens::Owned;
        assert_eq!(inserir_arc_funcoes_dart(&mut fs, &mut ps).unwrap(), (0, 0));
        assert_eq!(fs[0].blocks[0].instructions.len(), 1);
        for (op, from) in [
            (Operand::Constant(Constant::Int(1)), Type::F64),
            (Operand::Constant(Constant::Double(1.0)), Type::I64),
            (Operand::Constant(Constant::Null), Type::I1),
            (Operand::Constant(Constant::Int(256)), Type::I8),
            (Operand::Constant(Constant::Null), Type::Ref),
        ] {
            let (mut fs, mut ps) = boxing(op, from);
            let antes = format!("{fs:?}/{ps:?}");
            assert!(inserir_arc_funcoes_dart(&mut fs, &mut ps).is_err());
            assert_eq!(format!("{fs:?}/{ps:?}"), antes);
        }
    }

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
