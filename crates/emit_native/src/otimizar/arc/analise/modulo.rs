//! Liga origens/layouts do lowering à extração local, sem selecionar políticas.
//! Acessos selecionam o campo nominal pelo esquema de cada receiver possível.

use super::{hir::*, modelo::*, points_to::*};
use crate::hir::*;
use std::collections::HashMap;

/// Analisa um corpo ARC usando origens e layouts já registrados no módulo.
/// Tracing devolve None sem executar a análise. Origem/layout ausente conserva
/// desconhecimento; nunca cria sítio a partir do ID SSA atual. Campos de um
/// receiver selecionam o campo do próprio layout, incluindo uniões de classes
/// e leituras encadeadas, no mesmo ponto fixo dos aliases. Esquema/índice
/// ausente e receiver opaco ou vazio conservam uma fronteira desconhecida.
/// Cópias/Phis preservam a união; parâmetro e call sem resumo não provam layout.
/// Chaves de layout são locais à versão do módulo, sem validade de recarga.
/// CallStatic usa aliases normais dos corpos locais até ponto fixo por SCC.
/// Folha com cobertura de campos e ABI exata preserva a precisão dos campos;
/// publicação pode ser limitada ao resultado pela cobertura própria; demais
/// escapes/retenções continuam opacos. Guarda corpos transitivos e tabelas de
/// emissão consultadas, inclusive conferência implícita de pilha.
/// Contexto explícito e caminhos de tabelas sem resumo conservam uma fronteira
/// de emissão opaca, separada dos IDs de instruções. Quadros por vivacidade
/// também conservam essa fronteira antes de reduções por mapas. Instrumentação
/// e demais opções do emissor ainda exigem cobertura própria.
///
/// # Erros
/// Símbolo ausente/duplicado, fato de sítio obsoleto, origem com classes
/// conflitantes, inicializador incompatível ou falha da extração/SSA.
/// Módulo não é alterado; resultado não é certificado de política.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::modulo::analisar_no_modulo};
/// let m = Module::new();
/// assert!(analisar_no_modulo(&m, "ausente", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
pub fn analisar_no_modulo(
    m: &Module,
    simbolo: &str,
    limite: usize,
) -> Result<Option<AnaliseHir>, String> {
    if !m.memoria_arc {
        return Ok(None);
    }
    let mut corpos = m.functions.iter().filter(|f| f.symbol == simbolo);
    let f = corpos
        .next()
        .ok_or_else(|| format!("points-to: corpo ausente {simbolo}"))?;
    if corpos.next().is_some() {
        return Err(format!("points-to: corpo duplicado {simbolo}"));
    }
    super::chamadas::conferir_globais(m, f)?;
    let chamadas: std::collections::HashSet<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, i, _)| {
            if let Instruction::CallStatic { symbol, .. } = i {
                Some(symbol.as_str())
            } else {
                None
            }
        })
        .collect();
    let resumos = super::chamadas::resolver(m, &chamadas, limite)?;
    let fatos = fatos_no_modulo(m, f)?;
    // A seleção acompanha aliases e campos na mesma solução. Não refinar
    // topo de uma primeira passagem nem pressupor um layout único do Phi.
    let mut analise = analisar_com_resumos(f, &fatos, limite, &resumos, true)?;
    analise.premissas_modulo = Some(assinatura_premissas(m, simbolo));
    // Guarda conservadora de todo o alcance consultado, inclusive corpos
    // transitivos. Não é cache persistente nem dependência de geração JIT.
    let posicoes: HashMap<_, _> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    // O prólogo/cleanup do próprio chamador também depende destas tabelas,
    // inclusive quando o corpo não possui nenhuma chamada direta local.
    analise
        .dependencias_tabelas
        .insert(simbolo.to_string(), estado_tabela(m, posicoes[simbolo]));
    let tabela = if m.excecoes_por_tabelas {
        m.tabelas.get(posicoes[simbolo])
    } else {
        None
    };
    // Contexto implica CheckStackOverflow no prólogo, mesmo se toda a HIR
    // explícita tiver cobertura. Pousos/saídas/cleanup também inserem caminhos
    // sem contrato de heap aqui. Não inventar um ValueId para esses efeitos.
    analise.emissao_implicita_opaca = (m.excecoes_por_tabelas && tabela.is_none())
        || crate::llvm::LlvmEmitter::exige_contexto_explicito(f, tabela)
        || crate::llvm::LlvmEmitter::pode_exigir_contexto_por_raizes(f)
        || tabela.is_some_and(|t| {
            !t.invocacoes.is_empty()
                || !t.pousos.is_empty()
                || !t.saidas.is_empty()
                || t.cleanup_estrangeiro.is_some()
        });
    for (s, (corpo, _)) in &resumos {
        analise
            .dependencias_corpos
            .insert(s.clone(), assinatura_corpo(corpo));
        let indice = posicoes[s.as_str()];
        analise
            .dependencias_tabelas
            .insert(s.clone(), estado_tabela(m, indice));
    }
    Ok(Some(analise))
}

// Uma única validação das origens/layouts para o corpo local e para os
// objetos mencionados pelos resumos de callees. Não fabrica origem por SSA.
pub(super) fn fatos_no_modulo(m: &Module, f: &Function) -> Result<FatosHir, String> {
    let instrucoes: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let constantes = super::hir::constantes_inteiras(f);
    let mut fatos = FatosHir::default();
    let mut classes = HashMap::new();
    if let Some(sitios) = m.sitios_arc.get(&f.symbol) {
        for (v, sitio) in sitios {
            let i = instrucoes
                .get(v)
                .ok_or_else(|| format!("points-to: sítio obsoleto v{}", v.0))?;
            if !super::origens::alocacao(i) {
                return Err(format!("points-to: sítio não aloca v{}", v.0));
            }
            let (classe, quantidade) = match i {
                Instruction::AllocObject { class_id, fields } => (*class_id, fields.len()),
                _ => match super::hir::fabrica_zerada(i, &constantes) {
                    Some(c) => c,
                    None => continue,
                },
            };
            let Some(layout) = m.layouts_campos_arc.get(&classe) else {
                continue;
            };
            if layout.len() != quantidade {
                return Err(format!("points-to: layout incompatível v{}", v.0));
            }
            let no = NoAbstrato::Alocacao {
                sitio: sitio.clone(),
                contexto: String::new(),
            };
            if classes
                .insert(no.clone(), classe)
                .is_some_and(|anterior| anterior != classe)
            {
                return Err("points-to: origem com classes conflitantes".into());
            }
            let campos = (0..quantidade)
                .map(|posicao| CampoArc {
                    layout: format!("classe:{classe}"),
                    posicao: posicao as u32,
                })
                .collect();
            fatos.objetos.insert(*v, OrigemObjeto { no, campos });
        }
    }
    Ok(fatos)
}

// Guarda local dos fatos lidos; ordenação nominal evita depender da ordem
// de inserção/capacidade dos HashMaps. Layouts não usados também invalidam,
// conservadoramente, até termos dependências finas/versionadas de esquema.
pub(super) fn assinatura_premissas(m: &Module, simbolo: &str) -> blake3::Hash {
    let layouts: std::collections::BTreeMap<_, _> = m.layouts_campos_arc.iter().collect();
    // A identidade de um resultado alocado pode vir de outro corpo. Guarda
    // nominal do módulo antes de consumir essas origens entre chamadas;
    // mudar só a proveniência também precisa invalidar a solução anterior.
    let sitios: std::collections::BTreeMap<_, _> = m
        .sitios_arc
        .iter()
        .map(|(funcao, origens)| {
            let origens: std::collections::BTreeMap<_, _> =
                origens.iter().map(|(v, sitio)| (v.0, sitio)).collect();
            (funcao, origens)
        })
        .collect();
    // Preservar duplicatas: uma declaração repetida deixa de ser ABI válida.
    let mut globais: Vec<_> = m
        .globais
        .iter()
        .map(|(slot, ty, nome)| (nome, slot, format!("{ty:?}")))
        .collect();
    globais.sort();
    blake3::hash(format!("{:?}", (m.memoria_arc, simbolo, layouts, sitios, globais)).as_bytes())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn origem_de_callee_invalida_escape_sem_depender_da_ordem_dos_mapas() {
        use super::super::escape::calcular_no_modulo;
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![]);
        m.functions = vec![
            Function {
                symbol: "caller".into(),
                name: "caller".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::CallStatic {
                            symbol: "criar".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                }],
            },
            Function {
                symbol: "criar".into(),
                name: "criar".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(37),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(37)))),
                }],
            },
        ];
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(
            a.valor(ValueId(0)).unwrap().nos(),
            Some(&std::collections::BTreeSet::from([NoAbstrato::Alocacao {
                sitio: m.sitios_arc["criar"][&ValueId(37)].clone(),
                contexto: String::new(),
            }]))
        );
        calcular_no_modulo(&m, "caller", &a).unwrap();
        // Reconstruir tabelas com outra ordem/capacidade não muda premissas.
        let mut origens: Vec<_> = m.sitios_arc.drain().collect();
        origens.reverse();
        m.sitios_arc = origens.into_iter().collect();
        calcular_no_modulo(&m, "caller", &a).unwrap();
        let corpo = assinatura_corpo(&m.functions[1]);
        m.sitios_arc
            .get_mut("criar")
            .unwrap()
            .get_mut(&ValueId(37))
            .unwrap()
            .origem += 1;
        assert_eq!(assinatura_corpo(&m.functions[1]), corpo);
        assert!(calcular_no_modulo(&m, "caller", &a).is_err());
        let nova = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        calcular_no_modulo(&m, "caller", &nova).unwrap();
        m.sitios_arc.remove("criar");
        assert!(calcular_no_modulo(&m, "caller", &nova).is_err());
    }

    #[test]
    fn fabrica_zerada_publica_origem_alocada_em_global_e_excecao_por_wrapper() {
        use super::super::{escape::*, grafo::construir_local_no_modulo};
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![]);
        m.globais.push((0, Type::Ref, "g".into()));
        let wrapper = |nome: &str, alvo: &str| Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallStatic {
                        symbol: alvo.into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        m.functions = vec![
            wrapper("caller", "wrapper"),
            wrapper("wrapper", "criar"),
            Function {
                symbol: "criar".into(),
                name: "criar".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(37),
                            Instruction::CallRuntime {
                                name: "dartforge_object_new".into(),
                                args: vec![
                                    (Operand::Constant(Constant::Int(1)), Type::I64),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(38),
                            Instruction::StoreGlobal {
                                simbolo: "g".into(),
                                val: Operand::Val(ValueId(37)),
                                ty: Type::Ref,
                                raiz: Some(0),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Throw(Operand::Val(ValueId(37))),
                }],
            },
        ];
        super::super::origens::registrar(&mut m, 0, true);
        let esperado = std::collections::BTreeSet::from([NoAbstrato::Alocacao {
            sitio: m.sitios_arc["criar"][&ValueId(37)].clone(),
            contexto: String::new(),
        }]);
        for reversa in [false, true] {
            if reversa {
                m.functions.reverse();
            }
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            let e = calcular_no_modulo(&m, "caller", &a).unwrap();
            assert_eq!(
                e.publicacoes[&CausaEscape::Global("g".into())].nos(),
                Some(&esperado)
            );
            assert_eq!(e.publicacoes[&CausaEscape::Excecao].nos(), Some(&esperado));
            assert!(
                e.publicacoes[&CausaEscape::OperacaoOpaca(0)]
                    .nos()
                    .is_none()
            );
            let g = construir_local_no_modulo(&m, "caller", 8).unwrap().unwrap();
            g.conferir_no_modulo(&m, "caller").unwrap();
            assert_eq!(
                g.arestas()
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
                esperado
            );
            assert!(g.opaco());
        }
        let criar = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "criar")
            .unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut criar.blocks[0].instructions[0].1 {
            args[0].1 = Type::Ref;
        }
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        let e = calcular_no_modulo(&m, "caller", &a).unwrap();
        assert!(e.publicacoes[&CausaEscape::Excecao].nos().is_none());
    }

    #[test]
    fn retorno_alocado_compõe_parametro_null_recursao_e_campos_importados_opacos() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.layouts_campos_arc.insert(2, vec![]);
        let criar = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "x".into(), Type::Ref),
                (ValueId(1), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(37),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Val(ValueId(0))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(38),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(37))),
                                (BlockId(2), Operand::Val(ValueId(0))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(38)))),
                },
            ],
        };
        let chamada = |x| Instruction::CallStatic {
            symbol: "criar".into(),
            args: vec![x, Operand::Constant(Constant::Bool(true))],
            ret_ty: Type::Ref,
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 2,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (ValueId(1), chamada(Operand::Val(ValueId(0))), Type::Ref),
                    (
                        ValueId(2),
                        chamada(Operand::Constant(Constant::Null)),
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        m.functions = vec![caller, criar];
        super::super::origens::registrar(&mut m, 0, true);
        let no = |funcao: &str, id| NoAbstrato::Alocacao {
            sitio: m.sitios_arc[funcao][&ValueId(id)].clone(),
            contexto: String::new(),
        };
        let alocado = no("criar", 37);
        let parametro = no("caller", 0);
        for reversa in [false, true] {
            if reversa {
                m.functions.reverse();
            }
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(
                a.valor(ValueId(1)).unwrap().nos(),
                Some(&std::collections::BTreeSet::from([
                    parametro.clone(),
                    alocado.clone()
                ]))
            );
            assert_eq!(
                a.valor(ValueId(2)).unwrap().nos(),
                Some(&std::collections::BTreeSet::from([alocado.clone()]))
            );
            assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
            assert!(
                calcular_no_modulo(&m, "caller", &a).unwrap().publicacoes
                    [&CausaEscape::OperacaoOpaca(1)]
                    .nos()
                    .is_none()
            );
            let a = analisar_no_modulo(&m, "caller", 1).unwrap().unwrap();
            assert!(a.valor(ValueId(1)).unwrap().nos().is_none());
        }
        let criar = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "criar")
            .unwrap();
        criar.blocks[2].instructions.push((
            ValueId(39),
            Instruction::CallStatic {
                symbol: "criar".into(),
                args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut criar.blocks[3].instructions[0].1 {
            incoming[1].1 = Operand::Val(ValueId(39));
        }
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(
            a.valor(ValueId(1)).unwrap().nos(),
            Some(&std::collections::BTreeSet::from([alocado]))
        );
        m.sitios_arc.remove("criar");
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(1)).unwrap().nos().is_none());
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn excecao_por_chamada_preserva_payload_sem_fechar_fronteira_e_valida_corpos() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![]);
        let explode = Function {
            symbol: "explode".into(),
            name: "explode".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Throw(Operand::Val(ValueId(0))),
            }],
        };
        let ponte = Function {
            symbol: "ponte".into(),
            name: "ponte".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::CallStatic {
                        symbol: "explode".into(),
                        args: vec![Operand::Val(ValueId(0))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "ponte".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        };
        m.functions = vec![caller, ponte, explode];
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        let e = calcular_no_modulo(&m, "caller", &a).unwrap();
        assert_eq!(
            e.publicacoes[&CausaEscape::Excecao].nos(),
            a.valor(ValueId(0)).unwrap().nos()
        );
        assert!(
            e.publicacoes[&CausaEscape::OperacaoOpaca(1)]
                .nos()
                .is_none()
        );
        m.functions[2].blocks[0].terminator =
            Terminator::Throw(Operand::Constant(Constant::String("erro".into())));
        assert!(calcular_no_modulo(&m, "caller", &a).is_err());
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        let e = calcular_no_modulo(&m, "caller", &a).unwrap();
        assert!(e.publicacoes[&CausaEscape::Excecao].nos().is_none());
        let a = analisar_no_modulo(&m, "caller", 0).unwrap().unwrap();
        let e = calcular_no_modulo(&m, "caller", &a).unwrap();
        assert!(
            e.publicacoes[&CausaEscape::OperacaoOpaca(1)]
                .nos()
                .is_none()
        );
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn publicacao_por_chamada_conserva_identidade_e_opacidade_e_confere_globais() {
        use super::super::escape::*;
        for modo in [ModoStoreForte::Copy, ModoStoreForte::Move] {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.globais = vec![(0, Type::Ref, "g".into()), (1, Type::Ref, "h".into())];
            m.layouts_campos_arc.insert(1, vec![]);
            let publica = Function {
                symbol: "pub".into(),
                name: "pub".into(),
                depuracao: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::ArcStoreStrong {
                            slot: SlotForte::Global {
                                simbolo: "g".into(),
                            },
                            value: Operand::Val(ValueId(0)),
                            modo,
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                }],
            };
            let caller = Function {
                symbol: "caller".into(),
                name: "caller".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::AllocObject {
                                class_id: 1,
                                fields: vec![],
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: "pub".into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            m.functions = vec![caller, publica];
            super::super::origens::registrar(&mut m, 0, true);
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            let e = calcular_no_modulo(&m, "caller", &a).unwrap();
            assert_eq!(
                e.publicacoes[&CausaEscape::Global("g".into())].nos(),
                a.valor(ValueId(0)).unwrap().nos()
            );
            assert!(
                e.publicacoes[&CausaEscape::OperacaoOpaca(1)]
                    .nos()
                    .is_none()
            );
            // Ordem nominal das declarações não muda a premissa; duplicação muda.
            m.globais.reverse();
            assert!(calcular_no_modulo(&m, "caller", &a).is_ok());
            m.globais.push((0, Type::Ref, "g".into()));
            assert!(calcular_no_modulo(&m, "caller", &a).is_err());
            assert!(analisar_no_modulo(&m, "caller", 0).is_err());
            m.globais.pop();
            m.globais.iter_mut().find(|(_, _, s)| s == "g").unwrap().1 = Type::I64;
            assert!(calcular_no_modulo(&m, "caller", &a).is_err());
            m.globais.iter_mut().find(|(_, _, s)| s == "g").unwrap().1 = Type::Ref;
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[1].1
            {
                args[0] = Operand::Constant(Constant::Null);
            }
            let n = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            let e = calcular_no_modulo(&m, "caller", &n).unwrap();
            assert!(
                e.publicacoes[&CausaEscape::Global("g".into())]
                    .nos()
                    .unwrap()
                    .is_empty()
            );
            m.memoria_arc = false;
            assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
        }
    }

    #[test]
    fn publicacao_direta_em_slot_forte_exige_declaracao_e_conserva_fronteira() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(Function {
            symbol: "pub".into(),
            name: "pub".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::ArcStoreStrong {
                        slot: SlotForte::Global {
                            simbolo: "g".into(),
                        },
                        value: Operand::Constant(Constant::Null),
                        modo: ModoStoreForte::Copy,
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        assert!(analisar_no_modulo(&m, "pub", 0).is_err());
        m.globais.push((0, Type::Ref, "g".into()));
        let a = analisar_no_modulo(&m, "pub", 8).unwrap().unwrap();
        let e = calcular_no_modulo(&m, "pub", &a).unwrap();
        assert!(
            e.publicacoes[&CausaEscape::Global("g".into())]
                .nos()
                .unwrap()
                .is_empty()
        );
        assert!(
            e.publicacoes[&CausaEscape::OperacaoOpaca(0)]
                .nos()
                .is_none()
        );
        m.globais[0].1 = Type::I64;
        assert!(analisar_no_modulo(&m, "pub", 0).is_err());
        assert!(calcular_no_modulo(&m, "pub", &a).is_err());
    }

    #[test]
    fn retorno_por_campo_conserva_opacidade_guarda_do_corpo_e_abi_exata() {
        let mut m = Module::new();
        m.memoria_arc = true;
        let id = Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallStatic {
                        symbol: "id".into(),
                        args: vec![Operand::Constant(Constant::Null)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        m.functions = vec![caller, id];
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(0)).unwrap().nos().unwrap().is_empty());
        if let Instruction::CallStatic { args, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[0] = Operand::Constant(Constant::Int(0));
        }
        let b = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(b.valor(ValueId(0)).unwrap().nos().is_none());
        if let Instruction::CallStatic { args, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[0] = Operand::Constant(Constant::Null);
        }
        m.functions[1].blocks[0].instructions.push((
            ValueId(1),
            Instruction::GetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
            },
            Type::Ref,
        ));
        m.functions[1].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &a).is_err());
        let c = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(c.valor(ValueId(0)).unwrap().nos().is_none());
        assert_eq!(c.desconhecidos(), &[ValueId(0)]);
        let e = super::super::escape::calcular_no_modulo(&m, "caller", &c).unwrap();
        assert!(
            e.publicacoes[&super::super::escape::CausaEscape::OperacaoOpaca(0)]
                .nos()
                .is_none()
        );
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn leituras_encadeadas_e_escrita_por_alias_resolvem_no_mesmo_ponto_fixo() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.layouts_campos_arc.insert(2, vec![Type::Ref]);
        m.layouts_campos_arc.insert(3, vec![]);
        m.functions.push(Function {
            symbol: "cadeia".into(),
            name: "cadeia".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 3,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 2,
                            fields: vec![Operand::Constant(Constant::Null)],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Val(ValueId(1))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(5),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
            }],
        });
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "cadeia", 8).unwrap().unwrap();
        assert_eq!(a.valor(ValueId(3)), a.valor(ValueId(1)));
        assert_eq!(a.valor(ValueId(5)), a.valor(ValueId(0)));
        assert!(a.desconhecidos().is_empty());
        let g = super::super::grafo::construir_local_no_modulo(&m, "cadeia", 8)
            .unwrap()
            .unwrap();
        // O prólogo ainda é opaco; esta etapa só resolve as arestas conhecidas.
        assert!(g.opaco());
        // Índice fora do esquema não pode produzir resultado vazio preciso.
        if let Instruction::GetField { index, .. } = &mut m.functions[0].blocks[0].instructions[5].1
        {
            *index = 1;
        }
        let a = analisar_no_modulo(&m, "cadeia", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(5)).unwrap().nos().is_none());
        assert!(a.desconhecidos().contains(&ValueId(5)));
    }

    #[test]
    fn chamada_coberta_ainda_exige_contexto_pelas_raizes_vivas() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        let mut f = m.functions[0].clone();
        f.symbol = "f".into();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallStatic {
                symbol: "id".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        assert!(!crate::llvm::LlvmEmitter::exige_contexto_explicito(
            &f, None
        ));
        assert!(crate::llvm::LlvmEmitter::pode_exigir_contexto_por_raizes(
            &f
        ));
        assert!(!crate::llvm::LlvmEmitter::pode_exigir_contexto_por_raizes(
            &m.functions[0]
        ));
        m.functions.push(f);
        let a = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(a.desconhecidos().is_empty()); // A folha id é coberta.
        assert!(
            calcular_no_modulo(&m, "f", &a).unwrap().publicacoes[&CausaEscape::EmissaoImplicita]
                .nos()
                .is_none()
        );
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("pilha.estouro:"));
        assert!(ir.contains("%gcq = alloca"));
    }

    #[test]
    fn prologo_implicito_publica_topo_sem_inventar_instrucao_hir() {
        use super::super::{escape::*, grafo::construir_local_no_modulo};
        let mut m = Module::new();
        m.memoria_arc = true;
        m.excecoes_por_tabelas = true;
        m.functions.push(Function {
            symbol: "f".into(),
            name: "f".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(None),
            }],
        });
        for confere in [false, true] {
            m.tabelas = vec![TabelasDaFuncao {
                confere_pilha: confere,
                ..Default::default()
            }];
            let a = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
            assert!(a.desconhecidos().is_empty()); // Não há instrução explícita.
            let e = calcular_no_modulo(&m, "f", &a).unwrap();
            if confere {
                assert!(
                    e.publicacoes[&CausaEscape::EmissaoImplicita]
                        .nos()
                        .is_none()
                );
            } else {
                assert!(e.publicacoes.is_empty());
            }
            assert_eq!(
                construir_local_no_modulo(&m, "f", 8)
                    .unwrap()
                    .unwrap()
                    .opaco(),
                confere
            );
            // Confere o caminho realmente inserido na emissão, além da metadata.
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert_eq!(ir.contains("pilha.estouro:"), confere);
        }
        m.tabelas.clear(); // Sem tabela, não há cobertura da emissão disponível.
        let a = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(
            calcular_no_modulo(&m, "f", &a).unwrap().publicacoes[&CausaEscape::EmissaoImplicita]
                .nos()
                .is_none()
        );
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn tabela_do_proprio_corpo_invalida_consumo_sem_chamadas_e_acompanha_simbolo() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.excecoes_por_tabelas = true;
        for s in ["f", "g"] {
            m.functions.push(Function {
                symbol: s.into(),
                name: s.into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            });
        }
        m.tabelas = vec![TabelasDaFuncao::default(); 2];
        let a = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_ok());
        m.tabelas[0].confere_pilha = true;
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_err());
        m.tabelas[0].confere_pilha = false;
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_ok());
        m.functions.reverse();
        m.tabelas.reverse();
        m.tabelas[0].confere_pilha = true; // Mudança só em g, fora das dependências.
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_ok());
        m.tabelas[1].confere_pilha = true;
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_err());
        m.excecoes_por_tabelas = false;
        assert!(super::super::escape::calcular_no_modulo(&m, "f", &a).is_err());
    }

    #[test]
    fn folha_preserva_campos_mas_conversoes_e_caminhos_opacos_invalidam() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.layouts_campos_arc.insert(2, vec![]);
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 2,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Val(ValueId(0))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "id".into(),
                            args: vec![Operand::Val(ValueId(1))],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        });
        m.functions.push(Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.valor(ValueId(0)), a.valor(ValueId(3)));
        assert_eq!(a.valor(ValueId(3)).unwrap().nos().unwrap().len(), 1);
        let e = super::super::escape::calcular_no_modulo(&m, "caller", &a).unwrap();
        assert!(
            !e.publicacoes
                .contains_key(&super::super::escape::CausaEscape::OperacaoOpaca(2))
        );
        assert_eq!(
            e.publicacoes[&super::super::escape::CausaEscape::Retorno],
            *a.valor(ValueId(0)).unwrap()
        );
        // Ignorar o resultado da identidade não publica o argumento.
        m.functions[0].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let descartado = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        let e = super::super::escape::calcular_no_modulo(&m, "caller", &descartado).unwrap();
        assert!(
            e.publicacoes[&super::super::escape::CausaEscape::Retorno]
                .nos()
                .unwrap()
                .is_empty()
        );
        assert!(
            !e.publicacoes
                .contains_key(&super::super::escape::CausaEscape::OperacaoOpaca(2))
        );
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
        // Metadados de emissão podem introduzir chamada implícita, mesmo
        // sem mudança do corpo HIR; devem invalidar o consumo anterior.
        m.excecoes_por_tabelas = true;
        m.tabelas = vec![TabelasDaFuncao::default(); 2];
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &a).is_err());
        let com_tabelas = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(com_tabelas.valor(ValueId(0)), com_tabelas.valor(ValueId(3)));
        m.tabelas[1].confere_pilha = true;
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &com_tabelas).is_err());
        let com_guarda = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(com_guarda.desconhecidos().contains(&ValueId(2)));
        assert!(
            analisar_no_modulo(&m, "caller", 8)
                .unwrap()
                .unwrap()
                .valor(ValueId(3))
                .unwrap()
                .nos()
                .is_none()
        );
        m.excecoes_por_tabelas = false;
        m.tabelas.clear();
        let folha = m.functions[1].clone();
        // Alias normal não apaga escrita explícita.
        m.functions[1].blocks[0].instructions.push((
            ValueId(1),
            Instruction::SetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &a).is_err());
        assert!(
            analisar_no_modulo(&m, "caller", 8)
                .unwrap()
                .unwrap()
                .valor(ValueId(3))
                .unwrap()
                .nos()
                .is_none()
        );
        m.functions[1] = folha.clone();
        m.globais.push((0, Type::Ref, "g".into()));
        m.functions[1].blocks[0].instructions.push((
            ValueId(1),
            Instruction::StoreGlobal {
                simbolo: "g".into(),
                val: Operand::Val(ValueId(0)),
                ty: Type::Ref,
                raiz: Some(0),
            },
            Type::Void,
        ));
        let global = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(global.desconhecidos().contains(&ValueId(2)));
        let e = super::super::escape::calcular_no_modulo(&m, "caller", &global).unwrap();
        assert!(
            e.publicacoes[&super::super::escape::CausaEscape::OperacaoOpaca(2)]
                .nos()
                .is_none()
        );
        m.functions[1] = folha;
        // Preservação de campos independe de existir retorno Ref/alias.
        m.functions[1].return_ty = Type::Void;
        m.functions[1].blocks[0].terminator = Terminator::Return(None);
        m.functions[0].blocks[0].instructions[2].2 = Type::Void;
        if let Instruction::CallStatic { ret_ty, .. } =
            &mut m.functions[0].blocks[0].instructions[2].1
        {
            *ret_ty = Type::Void;
        }
        if let Instruction::GetField { object, .. } =
            &mut m.functions[0].blocks[0].instructions[3].1
        {
            *object = Operand::Val(ValueId(1));
        }
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.valor(ValueId(0)), a.valor(ValueId(3)));
        for constante in [Constant::Int(42), Constant::String("texto".into())] {
            // Converter/materializar um argumento pode coletar antes da folha.
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[2].1
            {
                args[0] = Operand::Constant(constante);
            }
            assert!(
                analisar_no_modulo(&m, "caller", 8)
                    .unwrap()
                    .unwrap()
                    .valor(ValueId(3))
                    .unwrap()
                    .nos()
                    .is_none()
            );
        }
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn chamada_direta_preserva_alias_e_confere_dependencia_sem_provar_noescape() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Constant(Constant::Null)],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "ponte".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(1)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        });
        m.functions.push(Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        let mut ponte = m.functions[1].clone();
        ponte.symbol = "ponte".into();
        ponte.name = "ponte".into();
        ponte.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "id".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        ponte.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        m.functions.push(ponte);
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.valor(ValueId(0)), a.valor(ValueId(1)));
        assert_eq!(a.valor(ValueId(1)).unwrap().nos().unwrap().len(), 1);
        // Devolver o argumento não garante que o campo continue null:
        // o callee acima escreve uma autoaresta antes do retorno normal.
        assert!(a.valor(ValueId(2)).unwrap().nos().is_none());
        assert_eq!(a.desconhecidos(), &[ValueId(1)]);
        assert!(super::super::escape::calcular(&m.functions[0], &a).is_err());
        let r = super::super::escape::calcular_no_modulo(&m, "caller", &a).unwrap();
        assert!(
            r.publicacoes[&super::super::escape::CausaEscape::OperacaoOpaca(1)]
                .nos()
                .is_none()
        );
        m.functions[1].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &a).is_err());
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(1)).unwrap().nos().unwrap().is_empty());
        m.functions.pop();
        assert!(super::super::escape::calcular_no_modulo(&m, "caller", &a).is_err());
        let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(1)).unwrap().nos().is_none());
    }

    #[test]
    fn fabrica_zerada_usa_origem_layout_e_stores_sem_plano_manual() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.functions.push(Function {
            symbol: "criar".into(),
            name: "criar".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![
                                (Operand::Constant(Constant::Int(1)), Type::I64),
                                (Operand::Constant(Constant::Int(1)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(2),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        });
        super::super::origens::registrar(&mut m, 0, true);
        let a = analisar_no_modulo(&m, "criar", 8).unwrap().unwrap();
        assert_eq!(a.valor(ValueId(0)), a.valor(ValueId(2)));
        assert_eq!(a.valor(ValueId(0)).unwrap().nos().unwrap().len(), 1);
        assert!(a.desconhecidos().is_empty());
        assert!(super::super::escape::calcular(&m.functions[0], &a).is_err());
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &a).is_ok());
        m.layouts_campos_arc.insert(1, vec![Type::I64]);
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &a).is_err());
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        let origem = m.sitios_arc["criar"][&ValueId(0)].clone();
        m.sitios_arc
            .get_mut("criar")
            .unwrap()
            .get_mut(&ValueId(0))
            .unwrap()
            .origem += 1;
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &a).is_err());
        m.sitios_arc
            .get_mut("criar")
            .unwrap()
            .insert(ValueId(0), origem);
        m.memoria_arc = false;
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &a).is_err());
        m.memoria_arc = true;
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &a).is_ok());
        // Reconstruir HashMaps em outra ordem não muda as premissas nominais.
        m.layouts_campos_arc.insert(2, vec![Type::F64]);
        m.layouts_campos_arc.insert(3, vec![Type::I1]);
        let ordenada = analisar_no_modulo(&m, "criar", 8).unwrap().unwrap();
        let mut layouts: Vec<_> = m.layouts_campos_arc.drain().collect();
        layouts.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
        m.layouts_campos_arc.extend(layouts);
        assert!(super::super::escape::calcular_no_modulo(&m, "criar", &ordenada).is_ok());
        // ABI/contagem alteradas invalidam o reconhecimento, sem reinterpretar bits.
        if let Instruction::CallRuntime { args, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[1].0 = Operand::Constant(Constant::Int(2));
        }
        assert!(analisar_no_modulo(&m, "criar", 8).is_err());
        if let Instruction::CallRuntime { args, ret_ty, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[1].0 = Operand::Constant(Constant::Int(1));
            *ret_ty = Type::I64;
        }
        let a = analisar_no_modulo(&m, "criar", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(0)).unwrap().nos().is_none());
        if let Instruction::CallRuntime { ret_ty, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            *ret_ty = Type::Ref;
        }
        m.layouts_campos_arc.clear();
        let a = analisar_no_modulo(&m, "criar", 8).unwrap().unwrap();
        assert!(a.valor(ValueId(2)).unwrap().nos().is_none());
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "criar", 8).unwrap().is_none());
    }
}
