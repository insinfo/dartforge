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
/// CallStatic usa aliases normais dos corpos locais até ponto fixo por SCC;
/// efeitos continuam opacos. Todo corpo transitivo consultado tem sua versão
/// registrada para conferência conservadora no consumo.
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
    // Guarda conservadora de todo o alcance consultado, inclusive corpos
    // transitivos. Não é cache persistente nem dependência de geração JIT.
    for (s, (corpo, _)) in &resumos {
        analise
            .dependencias_corpos
            .insert(s.clone(), assinatura_corpo(corpo));
    }
    Ok(Some(analise))
}

#[cfg(test)]
mod testes {
    use super::*;

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
        assert_eq!(a.desconhecidos, vec![ValueId(1)]);
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
        assert!(a.desconhecidos.is_empty());
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
