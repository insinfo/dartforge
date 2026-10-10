//! Publicações locais e seu alcance transitivo (§28.2 e §30.1).
//! Não calcula regiões/observadores ou efeitos interprocedurais de chamadas.

use super::{hir::*, modelo::*};
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Destino conservador de publicação; operação opaca pode reter ou reentrar.
/// Isso não significa que toda publicação possível ocorra numa execução.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::escape::CausaEscape;
/// assert_ne!(CausaEscape::Retorno, CausaEscape::Excecao);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CausaEscape {
    /// Valor devolvido ao chamador e seus descendentes possíveis.
    Retorno,
    /// Valor lançado, incluindo os objetos alcançáveis do payload.
    Excecao,
    /// Publicação numa raiz global nominal.
    Global(String),
    /// ID local de uma operação sem resumo de heap/retencão válido.
    OperacaoOpaca(u32),
}

/// Alcance possível publicado, agrupado por causa. Topo impede prova negativa.
/// Ausência de causa aqui não prova confinamento: observadores/externs/SDK,
/// callbacks, vidas e saídas excepcionais ainda exigem cobertura completa.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::escape::ResultadoEscape;
/// assert!(ResultadoEscape::default().publicacoes.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct ResultadoEscape {
    /// Conjuntos transitivos, ou topo quando qualquer fronteira é desconhecida.
    pub publicacoes: BTreeMap<CausaEscape, ConjuntoPontos>,
}

/// Calcula publicações sobre a solução da mesma versão local da função.
/// Percorre todos os campos do esquema de cada nó e conserva ciclos/aliases;
/// esquema/campo ausente, campo opaco e budget excedido propagam topo.
/// Operação opaca recebe alcance desconhecido, inclusive sem argumentos,
/// pois pode acessar globais/callbacks não resumidos. Não infere noescape
/// do contrato Borrowed da ABI. Não transforma HIR nem seleciona política.
///
/// # Erros
/// O corpo mudou depois da solução points-to, ou há dependências de callees
/// que precisam ser conferidas por [`calcular_no_modulo`]. Recalcular quando necessário.
/// O hash é uma guarda local, não dependência versionada SDK/JIT.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::{hir::*, escape::*}};
/// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let a = analisar(&f, &FatosHir::default(), 8)?;
/// assert!(calcular(&f, &a)?.publicacoes[&CausaEscape::Retorno].nos().is_none());
/// # Ok::<(), String>(())
/// ```
pub fn calcular(f: &Function, a: &AnaliseHir) -> Result<ResultadoEscape, String> {
    if !a.dependencias_corpos.is_empty() {
        return Err("escape ARC: validar dependências pelo módulo".into());
    }
    calcular_validado(f, a)
}

/// Confere no módulo as versões dos callees usados para aliases, antes do escape.
/// Não valida gerações JIT, layouts/pins ou resumos SDK ainda não implementados.
///
/// # Erros
/// Corpo/callee ausente, duplicado ou alterado após a análise.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::{hir::*, escape::*}};
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// let a = analisar(&f, &FatosHir::default(), 8)?;
/// let mut m = Module::new(); m.functions.push(f);
/// assert!(calcular_no_modulo(&m, "f", &a)?.publicacoes.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn calcular_no_modulo(
    m: &Module,
    simbolo: &str,
    a: &AnaliseHir,
) -> Result<ResultadoEscape, String> {
    let mut indice = std::collections::HashMap::new();
    for f in &m.functions {
        if indice.insert(&f.symbol, f).is_some() {
            return Err("escape ARC: corpo duplicado".into());
        }
    }
    for (s, assinatura) in &a.dependencias_corpos {
        let f = indice.get(s).ok_or("escape ARC: callee ausente")?;
        if assinatura_corpo(f) != *assinatura {
            return Err(format!("escape ARC: callee mudou {s}"));
        }
    }
    let f = indice
        .get(&simbolo.to_string())
        .ok_or("escape ARC: corpo ausente")?;
    calcular_validado(f, a)
}

fn calcular_validado(f: &Function, a: &AnaliseHir) -> Result<ResultadoEscape, String> {
    if a.corpo != assinatura_corpo(f) {
        return Err("escape ARC: corpo mudou após points-to".into());
    }
    let mut r = ResultadoEscape::default();
    let operando = |o: &Operand| match o {
        Operand::Val(v) => a
            .valor(*v)
            .cloned()
            .unwrap_or_else(|| ConjuntoPontos::desconhecido(a.limite)),
        Operand::Constant(Constant::Null) => ConjuntoPontos::vazio(a.limite),
        _ => ConjuntoPontos::desconhecido(a.limite),
    };
    let mut publicar = |causa, raizes: ConjuntoPontos| {
        let destinos = alcance(a, &raizes);
        r.publicacoes
            .entry(causa)
            .or_insert_with(|| ConjuntoPontos::vazio(a.limite))
            .unir(&destinos);
    };
    for b in &f.blocks {
        match &b.terminator {
            Terminator::Return(Some(o)) => publicar(CausaEscape::Retorno, operando(o)),
            Terminator::Throw(o) => publicar(CausaEscape::Excecao, operando(o)),
            _ => {}
        }
        for (_, i, _) in &b.instructions {
            if let Instruction::StoreGlobal { simbolo, val, .. } = i {
                publicar(CausaEscape::Global(simbolo.clone()), operando(val));
            }
        }
    }
    for id in &a.desconhecidos {
        publicar(
            CausaEscape::OperacaoOpaca(id.0),
            ConjuntoPontos::desconhecido(a.limite),
        );
    }
    Ok(r)
}

fn alcance(a: &AnaliseHir, raizes: &ConjuntoPontos) -> ConjuntoPontos {
    let mut resultado = raizes.clone();
    let Some(nos) = raizes.nos() else {
        return resultado;
    };
    let mut fila: VecDeque<_> = nos.iter().cloned().collect();
    let mut vistos = BTreeSet::new();
    while let Some(no) = fila.pop_front() {
        if !vistos.insert(no.clone()) {
            continue;
        }
        let Some(campos) = a.esquemas.get(&no) else {
            return ConjuntoPontos::desconhecido(a.limite);
        };
        for campo in campos {
            let Some(destinos) = a.pontos.campo(&no, campo) else {
                return ConjuntoPontos::desconhecido(a.limite);
            };
            let Some(nos) = destinos.nos() else {
                return ConjuntoPontos::desconhecido(a.limite);
            };
            for filho in nos {
                fila.push_back(filho.clone());
            }
            resultado.unir(destinos);
            if resultado.nos().is_none() {
                return resultado;
            }
        }
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::super::{hir::OrigemObjeto, points_to::CampoArc};
    use super::*;
    use std::collections::HashMap;

    fn grafo(mascara: usize) -> (Function, FatosHir) {
        let campo = CampoArc {
            layout: "No".into(),
            posicao: 0,
        };
        let mut fatos = FatosHir::default();
        let mut instructions = Vec::new();
        for v in 0..3 {
            instructions.push((
                ValueId(v),
                Instruction::AllocObject {
                    class_id: 1,
                    fields: vec![Operand::Constant(Constant::Null)],
                },
                Type::Ref,
            ));
            fatos.objetos.insert(
                ValueId(v),
                OrigemObjeto {
                    no: NoAbstrato::Alocacao {
                        sitio: SitioArc {
                            funcao: "f".into(),
                            origem: v as u64,
                            especializacao: "".into(),
                        },
                        contexto: "".into(),
                    },
                    campos: vec![campo.clone()],
                },
            );
        }
        for v in 0..3 {
            for w in 0..3 {
                if mascara & (1 << (3 * v + w)) != 0 {
                    let id = ValueId(instructions.len() as u32);
                    instructions.push((
                        id,
                        Instruction::SetField {
                            object: Operand::Val(ValueId(v)),
                            index: 0,
                            value: Operand::Val(ValueId(w)),
                        },
                        Type::Void,
                    ));
                    fatos.acessos.insert(id, campo.clone());
                }
            }
        }
        (
            Function {
                symbol: "f".into(),
                name: "f".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions,
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                }],
            },
            fatos,
        )
    }

    #[test]
    fn retorno_publica_descendentes_em_todos_os_grafos_de_tres_nos() {
        for mascara in 0..512 {
            let (f, fatos) = grafo(mascara);
            let a = analisar(&f, &fatos, 3).unwrap();
            let r = calcular(&f, &a).unwrap();
            // Oráculo por expansão de alcance, sem consultar campos do solver.
            let mut vivos = BTreeSet::from([0]);
            loop {
                let antes = vivos.clone();
                for v in antes.iter().copied() {
                    for w in 0..3 {
                        if mascara & (1 << (3 * v + w)) != 0 {
                            vivos.insert(w);
                        }
                    }
                }
                if vivos == antes {
                    break;
                }
            }
            let esperado: BTreeSet<_> = vivos
                .iter()
                .map(|v| fatos.objetos[&ValueId(*v as u32)].no.clone())
                .collect();
            assert_eq!(
                r.publicacoes[&CausaEscape::Retorno].nos().unwrap(),
                &esperado
            );
        }
    }

    #[test]
    fn excecao_publica_payload_e_analise_obsoleta_e_recusada() {
        let (mut f, fatos) = grafo(1 << 1);
        f.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
        let a = analisar(&f, &fatos, 3).unwrap();
        let r = calcular(&f, &a).unwrap();
        assert_eq!(r.publicacoes[&CausaEscape::Excecao].nos().unwrap().len(), 2);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(calcular(&f, &a).is_err());
    }

    #[test]
    fn opacidade_e_budget_nunca_provam_confinamento() {
        let (mut f, fatos) = grafo(1 << 1);
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::CallRuntime {
                name: "opaca".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let a = analisar(&f, &fatos, 3).unwrap();
        let r = calcular(&f, &a).unwrap();
        assert!(r.publicacoes[&CausaEscape::Retorno].nos().is_none());
        assert!(
            r.publicacoes[&CausaEscape::OperacaoOpaca(9)]
                .nos()
                .is_none()
        );
        f.blocks[0].instructions.pop();
        let a = analisar(&f, &fatos, 1).unwrap();
        assert!(
            calcular(&f, &a).unwrap().publicacoes[&CausaEscape::Retorno]
                .nos()
                .is_none()
        );
        let a = analisar(
            &f,
            &FatosHir {
                objetos: HashMap::new(),
                acessos: HashMap::new(),
            },
            3,
        )
        .unwrap();
        assert!(
            calcular(&f, &a).unwrap().publicacoes[&CausaEscape::Retorno]
                .nos()
                .is_none()
        );
    }

    #[test]
    fn publicacao_global_e_registrada_mesmo_sem_retorno_do_objeto() {
        let (mut f, fatos) = grafo(1 << 1);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::StoreGlobal {
                simbolo: "g".into(),
                val: Operand::Val(ValueId(0)),
                ty: Type::Ref,
                raiz: Some(0),
            },
            Type::Void,
        ));
        let a = analisar(&f, &fatos, 3).unwrap();
        let r = calcular(&f, &a).unwrap();
        // Store global ainda é opaco ao domínio de efeitos de heap: não
        // fabricar precisão transitiva antes de possuir esse contrato.
        assert!(
            r.publicacoes[&CausaEscape::Global("g".into())]
                .nos()
                .is_none()
        );
        assert!(
            r.publicacoes[&CausaEscape::Retorno]
                .nos()
                .unwrap()
                .is_empty()
        );
    }
}
