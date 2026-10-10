//! Liga origens/layouts do lowering à extração local, sem selecionar políticas.
//! Accessos de receivers opacos ou com layouts distintos continuam desconhecidos.

use super::{hir::*, modelo::*, points_to::*};
use crate::hir::*;
use std::collections::HashMap;

/// Analisa um corpo ARC usando origens e layouts já registrados no módulo.
/// Tracing devolve None sem executar a análise. Origem/layout ausente conserva
/// desconhecimento; nunca cria sítio a partir do ID SSA atual. Campos de um
/// receiver só recebem esquema quando todos os nós conhecidos têm a mesma
/// classe/layout e o índice existe. Cópias/Phis preservam essa união; leitura
/// opaca, parâmetro e call sem resumo não provam a classe do receiver.
/// Chaves de layout são locais à versão do módulo, sem validade de recarga.
/// CallStatic usa aliases normais dos corpos locais até ponto fixo por SCC.
/// Folha com cobertura de campos e ABI exata preserva a precisão dos campos;
/// publicação pode ser limitada ao resultado pela cobertura própria; demais
/// escapes/retenções continuam opacos. Guarda corpos transitivos e tabelas de
/// emissão consultadas, inclusive conferência implícita de pilha.
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
    let instrucoes: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let constantes = super::hir::constantes_inteiras(f);
    let mut fatos = FatosHir::default();
    let mut classes = HashMap::new();
    if let Some(sitios) = m.sitios_arc.get(simbolo) {
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
    // Primeiro resolve origens/cópias/Phis sem supor layouts de receivers.
    // Falta de contrato mantém topo; só os conjuntos fechados abaixo refinam.
    let preliminar = analisar_com_resumos(f, &fatos, limite, &resumos)?;
    for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let (objeto, indice) = match i {
            Instruction::GetField { object, index }
            | Instruction::SetField { object, index, .. } => (object, *index),
            _ => continue,
        };
        let Operand::Val(objeto) = objeto else {
            continue;
        };
        let Some(nos) = preliminar
            .valor(*objeto)
            .and_then(|p| p.nos())
            .filter(|ns| !ns.is_empty())
        else {
            continue;
        };
        let mut classe = None;
        let mut fechado = true;
        for no in nos {
            let Some(&atual) = classes.get(no) else {
                fechado = false;
                break;
            };
            if classe.is_some_and(|c| c != atual) {
                fechado = false;
                break;
            }
            classe = Some(atual);
        }
        if !fechado {
            continue;
        }
        let classe = classe.unwrap();
        if indice >= m.layouts_campos_arc[&classe].len() {
            continue;
        }
        fatos.acessos.insert(
            *v,
            CampoArc {
                layout: format!("classe:{classe}"),
                posicao: indice as u32,
            },
        );
    }
    let mut analise = analisar_com_resumos(f, &fatos, limite, &resumos)?;
    analise.premissas_modulo = Some(assinatura_premissas(m, simbolo));
    // Guarda conservadora de todo o alcance consultado, inclusive corpos
    // transitivos. Não é cache persistente nem dependência de geração JIT.
    let posicoes: HashMap<_, _> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    for (s, (corpo, _)) in &resumos {
        analise
            .dependencias_corpos
            .insert(s.clone(), assinatura_corpo(corpo));
        let indice = posicoes[s.as_str()];
        analise
            .dependencias_tabelas
            .insert(s.clone(), assinatura_tabela(m, indice));
    }
    Ok(Some(analise))
}

// Guarda local dos fatos lidos; ordenação nominal evita depender da ordem
// de inserção/capacidade dos HashMaps. Layouts não usados também invalidam,
// conservadoramente, até termos dependências finas/versionadas de esquema.
pub(super) fn assinatura_premissas(m: &Module, simbolo: &str) -> blake3::Hash {
    let layouts: std::collections::BTreeMap<_, _> = m.layouts_campos_arc.iter().collect();
    let sitios = m.sitios_arc.get(simbolo).map(|origens| {
        origens
            .iter()
            .map(|(v, sitio)| (v.0, sitio))
            .collect::<std::collections::BTreeMap<_, _>>()
    });
    blake3::hash(format!("{:?}", (m.memoria_arc, simbolo, layouts, sitios)).as_bytes())
}

#[cfg(test)]
mod testes {
    use super::*;

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
