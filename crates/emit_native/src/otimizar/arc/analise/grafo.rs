//! Grafo local de sítios/campos e SCCs de objetos (§29.1).
//! Ausência de ciclo conhecido não é certificado durante toda a vida do objeto.

use super::{hir::AnaliseHir, modelo::NoAbstrato};
use crate::hir::{Function, Module, Type};
use std::collections::{BTreeMap, BTreeSet};

/// Fotografia imutável das arestas locais inferidas, incluindo fronteiras opacas.
/// SCC representa possibilidade abstrata, não grupo de instâncias concreto.
/// Não seleciona política nem certifica NaoParticipa/AciclicoProvado: mutações
/// futuras, entradas externas e observadores exigem a cobertura global da §29.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "f", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct GrafoHeapArc {
    arestas: BTreeMap<NoAbstrato, BTreeSet<NoAbstrato>>,
    componentes: Vec<(Vec<NoAbstrato>, bool)>,
    opaco: bool,
    analise: AnaliseHir,
}

impl GrafoHeapArc {
    /// Confere dependências locais e a projeção do points-to sem repetir o solver
    /// nem Tarjan. Exige todos os nós/arestas inferidos, preserva desconhecimento
    /// e confere a partição SCC e suas marcas de ciclo independentemente.
    /// Sobreaproximações são aceitas, mas não podem apagar fronteiras opacas.
    /// Não prova cobertura global, vida, SDK, observadores ou política.
    ///
    /// # Erros
    /// Módulo/corpo/tabelas obsoletos, nó/aresta omitido, destino ausente,
    /// opacidade apagada ou partição/marca de ciclo inválida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// construir_local_no_modulo(&m, "f", 8)?.unwrap().conferir_no_modulo(&m, "f")?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn conferir_no_modulo(&self, m: &Module, simbolo: &str) -> Result<(), String> {
        let f = super::escape::conferir_analise_no_modulo(m, simbolo, &self.analise)?;
        conferir_projecao(f, self)
    }

    /// Consulta arestas conhecidas sem apagar as fronteiras opacas.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(construir_local_no_modulo(&m, "f", 8)?.unwrap().arestas().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn arestas(&self) -> &BTreeMap<NoAbstrato, BTreeSet<NoAbstrato>> {
        &self.arestas
    }

    /// Consulta se faltam arestas/esquemas ou efeitos da fotografia local.
    /// Falso não prova cobertura durante toda a vida de nenhuma instância.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(!construir_local_no_modulo(&m, "f", 8)?.unwrap().opaco());
    /// # Ok::<(), String>(())
    /// ```
    pub fn opaco(&self) -> bool {
        self.opaco
    }

    /// Enumera SCCs e a presença de ciclo abstrato nas arestas conhecidas.
    /// Componente unitário é cíclico somente quando contém autoaresta.
    /// Um falso não exclui ciclos por fronteiras opacas/mutações futuras.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert_eq!(construir_local_no_modulo(&m, "f", 8)?.unwrap().componentes().count(), 0);
    /// # Ok::<(), String>(())
    /// ```
    pub fn componentes(&self) -> impl Iterator<Item = (&[NoAbstrato], bool)> {
        self.componentes
            .iter()
            .map(|(nos, ciclo)| (nos.as_slice(), *ciclo))
    }
}

/// Analisa o corpo e extrai SCCs de arestas de campos, sem usar SCCs de chamadas.
/// Só no módulo ARC; tracing devolve None. O resultado é local à versão atual,
/// não certificado de política, grupo concreto, vida ou cobertura do programa.
///
/// # Erros
/// Corpo/SSA ou premissas de sítio/layout/resumos inválidos na análise do módulo.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "ausente", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
pub fn construir_local_no_modulo(
    m: &Module,
    simbolo: &str,
    limite: usize,
) -> Result<Option<GrafoHeapArc>, String> {
    let Some(a) = super::modulo::analisar_no_modulo(m, simbolo, limite)? else {
        return Ok(None);
    };
    let f = m
        .functions
        .iter()
        .find(|f| f.symbol == simbolo)
        .expect("corpo analisado");
    let g = construir(f, a);
    g.conferir_no_modulo(m, simbolo)?;
    Ok(Some(g))
}

fn construir(f: &Function, a: AnaliseHir) -> GrafoHeapArc {
    let mut arestas: BTreeMap<_, BTreeSet<_>> = a
        .esquemas
        .keys()
        .cloned()
        .map(|no| (no, BTreeSet::new()))
        .collect();
    let mut opaco = !a.desconhecidos().is_empty() || a.emissao_implicita_opaca;
    // Valores que podem ser handles/endereços sem origem fechada conservam
    // uma fronteira, mesmo quando nenhuma aresta desse valor foi materializada.
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.valor(*v).is_none_or(|p| p.nos().is_none())
        {
            opaco = true;
        }
    }
    for (no, campos) in &a.esquemas {
        for campo in campos {
            let Some(destinos) = a.pontos().campo(no, campo).and_then(|p| p.nos()) else {
                opaco = true;
                continue;
            };
            for destino in destinos {
                arestas
                    .entry(no.clone())
                    .or_default()
                    .insert(destino.clone());
                arestas.entry(destino.clone()).or_default();
                if !a.esquemas.contains_key(destino) {
                    opaco = true;
                }
            }
        }
    }
    let nos: Vec<_> = arestas.keys().cloned().collect();
    let ids: BTreeMap<_, _> = nos.iter().enumerate().map(|(i, no)| (no, i)).collect();
    let adjacentes: Vec<Vec<_>> = nos
        .iter()
        .map(|no| arestas[no].iter().map(|destino| ids[destino]).collect())
        .collect();
    let componentes = crate::otimizar::scc::componentes(&adjacentes)
        .into_iter()
        .map(|c| {
            let ciclo = c.len() > 1 || adjacentes[c[0]].contains(&c[0]);
            let mut membros: Vec<_> = c.into_iter().map(|v| nos[v].clone()).collect();
            membros.sort();
            (membros, ciclo)
        })
        .collect();
    GrafoHeapArc {
        arestas,
        componentes,
        opaco,
        analise: a,
    }
}

// Confere obrigações por inclusão, não por igualdade com uma nova execução
// do produtor. A análise é premissa local imutável, não certificado global.
fn conferir_projecao(f: &Function, g: &GrafoHeapArc) -> Result<(), String> {
    let a = &g.analise;
    let exigir_opaco = || {
        if g.opaco {
            Ok(())
        } else {
            Err("grafo ARC: fronteira opaca apagada".to_string())
        }
    };
    if !a.desconhecidos().is_empty() || a.emissao_implicita_opaca {
        exigir_opaco()?;
    }
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.valor(*v).is_none_or(|p| p.nos().is_none())
        {
            exigir_opaco()?;
        }
    }
    for (no, campos) in &a.esquemas {
        let saidas = g
            .arestas
            .get(no)
            .ok_or("grafo ARC: nó de esquema omitido")?;
        for campo in campos {
            match a.pontos().campo(no, campo).and_then(|p| p.nos()) {
                None => exigir_opaco()?,
                Some(destinos) => {
                    for destino in destinos {
                        if !saidas.contains(destino) {
                            return Err("grafo ARC: aresta inferida omitida".into());
                        }
                    }
                }
            }
        }
    }
    // Inclui nós extras de sobreaproximações: um destino sem esquema não
    // pode ser tratado como folha fechada só por não ter arestas explícitas.
    for (no, saidas) in &g.arestas {
        if !a.esquemas.contains_key(no) {
            exigir_opaco()?;
        }
        if saidas
            .iter()
            .any(|destino| !g.arestas.contains_key(destino))
        {
            return Err("grafo ARC: destino fora do grafo".into());
        }
    }
    let ids: BTreeMap<_, _> = g
        .arestas
        .keys()
        .enumerate()
        .map(|(i, no)| (no, i))
        .collect();
    let adjacentes: Vec<Vec<_>> = g
        .arestas
        .values()
        .map(|saidas| saidas.iter().map(|no| ids[no]).collect())
        .collect();
    let mut cs = Vec::new();
    for (membros, ciclo) in &g.componentes {
        let c: Vec<_> = membros
            .iter()
            .map(|no| ids.get(no).copied().ok_or("grafo ARC: membro SCC ausente"))
            .collect::<Result<_, _>>()?;
        if c.is_empty() {
            return Err("grafo ARC: SCC vazia".into());
        }
        if *ciclo != (c.len() > 1 || adjacentes[c[0]].contains(&c[0])) {
            return Err("grafo ARC: marca de ciclo inválida".into());
        }
        cs.push(c);
    }
    crate::otimizar::scc::conferir(&adjacentes, &cs)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hir::*;

    fn modulo(mascara: usize) -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        let mut instructions: Vec<_> = (0..3)
            .map(|v| {
                (
                    ValueId(v),
                    Instruction::AllocObject {
                        class_id: 1,
                        fields: vec![Operand::Constant(Constant::Null)],
                    },
                    Type::Ref,
                )
            })
            .collect();
        for v in 0..3 {
            for w in 0..3 {
                if mascara & (1 << (3 * v + w)) != 0 {
                    instructions.push((
                        ValueId(instructions.len() as u32),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(v)),
                            index: 0,
                            value: Operand::Val(ValueId(w)),
                        },
                        Type::Void,
                    ));
                }
            }
        }
        m.functions.push(Function {
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
        });
        super::super::origens::registrar(&mut m, 0, true);
        m
    }

    #[test]
    fn scc_de_campos_confere_com_floyd_nos_512_grafos_hir() {
        for mascara in 0..512 {
            let m = modulo(mascara);
            let mut g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
            // Campos/alocação em linha exigem contexto: a conferência de
            // pilha do prólogo permanece opaca, sem apagar as arestas.
            assert!(g.opaco());
            assert_eq!(g.arestas().len(), 3);
            let nos: Vec<_> = (0..3)
                .map(|v| NoAbstrato::Alocacao {
                    sitio: m.sitios_arc["f"][&ValueId(v)].clone(),
                    contexto: "".into(),
                })
                .collect();
            let mut alcance = [[false; 3]; 3];
            for (v, linha) in alcance.iter_mut().enumerate() {
                for (w, destino) in linha.iter_mut().enumerate() {
                    *destino = mascara & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.arestas()[&nos[v]].contains(&nos[w]), *destino);
                }
            }
            // Adulterações são conferidas contra as obrigações inferidas,
            // não contra uma nova execução do produtor/Tarjan.
            g.conferir_no_modulo(&m, "f").unwrap();
            for no in &nos {
                let destinos: Vec<_> = g.arestas[no].iter().cloned().collect();
                for destino in destinos {
                    g.arestas.get_mut(no).unwrap().remove(&destino);
                    assert!(g.conferir_no_modulo(&m, "f").is_err());
                    g.arestas.get_mut(no).unwrap().insert(destino);
                }
                let saidas = g.arestas.remove(no).unwrap();
                assert!(g.conferir_no_modulo(&m, "f").is_err());
                g.arestas.insert(no.clone(), saidas);
            }
            g.opaco = false;
            assert!(g.conferir_no_modulo(&m, "f").is_err());
            g.opaco = true;
            for id in 0..g.componentes.len() {
                g.componentes[id].1 = !g.componentes[id].1;
                assert!(g.conferir_no_modulo(&m, "f").is_err());
                g.componentes[id].1 = !g.componentes[id].1;
            }
            let ultimo = g.componentes.pop().unwrap();
            assert!(g.conferir_no_modulo(&m, "f").is_err());
            g.componentes.push(ultimo);
            g.conferir_no_modulo(&m, "f").unwrap();
            // Fecho sem identidade: diagonal só verdadeira quando há ciclo.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        alcance[v][w] |= alcance[v][k] && alcance[k][w];
                    }
                }
            }
            let mut donos = [usize::MAX; 3];
            for (id, (membros, ciclo)) in g.componentes().enumerate() {
                for no in membros {
                    let v = nos.iter().position(|n| n == no).unwrap();
                    assert_eq!(donos[v], usize::MAX);
                    donos[v] = id;
                    assert_eq!(ciclo, alcance[v][v], "grafo {mascara}, nó {v}");
                }
            }
            for v in 0..3 {
                for w in 0..3 {
                    assert_eq!(
                        donos[v] == donos[w],
                        v == w || (alcance[v][w] && alcance[w][v])
                    );
                }
            }
        }
    }

    #[test]
    fn conferir_grafo_aceita_sobregrafo_e_rejeita_sccs_adulteradas() {
        let m = modulo(0);
        let mut g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        let nos: Vec<_> = g.arestas.keys().cloned().collect();
        g.arestas.get_mut(&nos[0]).unwrap().insert(nos[1].clone());
        g.componentes = vec![
            (vec![nos[1].clone()], false),
            (vec![nos[0].clone()], false),
            (vec![nos[2].clone()], false),
        ];
        g.conferir_no_modulo(&m, "f").unwrap();
        // União de nós sem alcance de volta não é componente forte.
        g.componentes = vec![
            (vec![nos[0].clone(), nos[1].clone()], true),
            (vec![nos[2].clone()], false),
        ];
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        // Acrescentar aresta de volta torna a união forte, sem executar Tarjan.
        g.arestas.get_mut(&nos[1]).unwrap().insert(nos[0].clone());
        g.conferir_no_modulo(&m, "f").unwrap();
        // Dividir o ciclo em duas SCCs é inválido mesmo com cobertura única.
        g.componentes = vec![
            (vec![nos[1].clone()], false),
            (vec![nos[0].clone()], false),
            (vec![nos[2].clone()], false),
        ];
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        g.componentes = vec![
            (vec![nos[0].clone(), nos[1].clone()], true),
            (vec![nos[2].clone()], false),
        ];
        g.componentes.push((vec![nos[2].clone()], false));
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        g.componentes.pop();
        g.componentes.push((vec![], false));
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        g.componentes.pop();
        let externo = NoAbstrato::Global("externo".into());
        g.arestas.get_mut(&nos[0]).unwrap().insert(externo.clone());
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        g.arestas.insert(externo.clone(), BTreeSet::new());
        // Nó extra sem esquema exige fronteira opaca e partição completa.
        g.componentes.insert(0, (vec![externo], false));
        g.conferir_no_modulo(&m, "f").unwrap();
        g.opaco = false;
        assert!(g.conferir_no_modulo(&m, "f").is_err());
    }

    #[test]
    fn conferir_grafo_recusa_mudancas_de_corpo_callee_layout_sitio_e_modo() {
        let mut m = modulo(0);
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
        m.functions[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "id".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        g.conferir_no_modulo(&m, "f").unwrap();
        // Reordenação nominal mantém dependências; símbolo é a identidade.
        m.functions.swap(0, 1);
        g.conferir_no_modulo(&m, "f").unwrap();
        m.functions.swap(0, 1);
        m.memoria_arc = false;
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.memoria_arc = true;
        m.excecoes_por_tabelas = true;
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.excecoes_por_tabelas = false;
        m.layouts_campos_arc.insert(1, vec![Type::I64]);
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        let sitio = m
            .sitios_arc
            .get_mut("f")
            .unwrap()
            .remove(&ValueId(0))
            .unwrap();
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.sitios_arc.get_mut("f").unwrap().insert(ValueId(0), sitio);
        let retorno = m.functions[1].blocks[0].terminator.clone();
        m.functions[1].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.functions[1].blocks[0].terminator = retorno;
        let retorno = m.functions[0].blocks[0].terminator.clone();
        m.functions[0].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.functions[0].blocks[0].terminator = retorno;
        let callee = m.functions.pop().unwrap();
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.functions.push(callee.clone());
        m.functions.push(callee);
        assert!(g.conferir_no_modulo(&m, "f").is_err());
        m.functions.pop();
        g.conferir_no_modulo(&m, "f").unwrap();
        assert!(g.conferir_no_modulo(&m, "id").is_err());
    }

    #[test]
    fn fronteiras_opacas_e_budget_nao_se_tornam_grafo_fechado() {
        let mut m = modulo(511);
        assert!(
            construir_local_no_modulo(&m, "f", 1)
                .unwrap()
                .unwrap()
                .opaco()
        );
        assert!(
            construir_local_no_modulo(&m, "f", 0)
                .unwrap()
                .unwrap()
                .opaco()
        );
        m.layouts_campos_arc.clear();
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.arestas().is_empty());
        m.memoria_arc = false;
        assert!(construir_local_no_modulo(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn instancias_resumidas_no_mesmo_sitio_nao_provam_singleton_ou_ausencia_de_ciclo() {
        let mut m = modulo(1 << 1); // Instância 0 aponta para instância 1.
        let origem = m.sitios_arc["f"][&ValueId(0)].clone();
        m.sitios_arc
            .get_mut("f")
            .unwrap()
            .insert(ValueId(1), origem);
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert_eq!(g.arestas().len(), 2);
        assert!(g.componentes().any(|(_, ciclo)| ciclo));
        assert!(g.opaco()); // O prólogo com contexto ainda não possui resumo.
    }

    #[test]
    fn parametro_opaco_nao_vira_grafo_vazio_fechado() {
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
        let g = construir_local_no_modulo(&m, "id", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.arestas().is_empty());
    }

    #[test]
    fn recursao_de_chamadas_nao_fabrica_autoaresta_de_objeto() {
        let mut m = modulo(0);
        m.functions[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "f".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.componentes().all(|(_, ciclo)| !ciclo));
        assert_eq!(g.arestas().len(), 3);
    }
}
