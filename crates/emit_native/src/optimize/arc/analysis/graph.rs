//! Grafo local de sítios/campos e SCCs de objetos (§29.1).
//! Ausência de ciclo conhecido não é certificado durante toda a vida do objeto.

use super::{hir::HirAnalysis, model::AbstractNode};
use crate::hir::{Function, Module, Type};
use std::collections::{BTreeMap, BTreeSet};

/// Fotografia imutável das arestas locais inferidas, incluindo fronteiras opacas.
/// SCC representa possibilidade abstrata, não grupo de instâncias concreto.
/// Não seleciona política nem certifica NaoParticipa/AciclicoProvado: mutações
/// futuras, entradas externas e observadores exigem a cobertura global da §29.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "f", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ArcHeapGraph {
    edges: BTreeMap<AbstractNode, BTreeSet<AbstractNode>>,
    components: Vec<(Vec<AbstractNode>, bool)>,
    opaque: bool,
    analysis: HirAnalysis,
}

impl ArcHeapGraph {
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
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// construir_local_no_modulo(&m, "f", 8)?.unwrap().conferir_no_modulo(&m, "f")?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn check_in_module(&self, m: &Module, symbol: &str) -> Result<(), String> {
        let f = super::escape::check_analysis_in_module(m, symbol, &self.analysis)?;
        check_projection(f, self)
    }

    /// Consulta arestas conhecidas sem apagar as fronteiras opacas.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(construir_local_no_modulo(&m, "f", 8)?.unwrap().arestas().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn edges(&self) -> &BTreeMap<AbstractNode, BTreeSet<AbstractNode>> {
        &self.edges
    }

    /// Consulta se faltam arestas/esquemas ou efeitos da fotografia local.
    /// Falso não prova cobertura durante toda a vida de nenhuma instância.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(!construir_local_no_modulo(&m, "f", 8)?.unwrap().opaco());
    /// # Ok::<(), String>(())
    /// ```
    pub fn opaque(&self) -> bool {
        self.opaque
    }

    /// Enumera SCCs e a presença de ciclo abstrato nas arestas conhecidas.
    /// Componente unitário é cíclico somente quando contém autoaresta.
    /// Um falso não exclui ciclos por fronteiras opacas/mutações futuras.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert_eq!(construir_local_no_modulo(&m, "f", 8)?.unwrap().componentes().count(), 0);
    /// # Ok::<(), String>(())
    /// ```
    pub fn components(&self) -> impl Iterator<Item = (&[AbstractNode], bool)> {
        self.components
            .iter()
            .map(|(nos, cycle)| (nos.as_slice(), *cycle))
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
/// use dartforge_emit_native::{hir::Module, optimize::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "ausente", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
pub fn build_local_in_module(
    m: &Module,
    symbol: &str,
    limit: usize,
) -> Result<Option<ArcHeapGraph>, String> {
    let Some(a) = super::module::analyze_in_module(m, symbol, limit)? else {
        return Ok(None);
    };
    let f = m
        .functions
        .iter()
        .find(|f| f.symbol == symbol)
        .expect("corpo analisado");
    let g = build(f, a);
    g.check_in_module(m, symbol)?;
    Ok(Some(g))
}

fn build(f: &Function, a: HirAnalysis) -> ArcHeapGraph {
    let mut edges: BTreeMap<_, BTreeSet<_>> = a
        .schemas
        .keys()
        .cloned()
        .map(|no| (no, BTreeSet::new()))
        .collect();
    let mut opaque = !a.unknown_set().is_empty() || a.opaque_implicit_emission;
    // Valores que podem ser handles/endereços sem origem fechada conservam
    // uma fronteira, mesmo quando nenhuma aresta desse valor foi materializada.
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.value(*v).is_none_or(|p| p.nos().is_none())
        {
            opaque = true;
        }
    }
    for (no, fields) in &a.schemas {
        for field in fields {
            let Some(dests) = a.points().field(no, field).and_then(|p| p.nos()) else {
                opaque = true;
                continue;
            };
            for dest in dests {
                edges
                    .entry(no.clone())
                    .or_default()
                    .insert(dest.clone());
                edges.entry(dest.clone()).or_default();
                if !a.schemas.contains_key(dest) {
                    opaque = true;
                }
            }
        }
    }
    // Um escritor coberto apenas na HIR explícita pode manter campos topo.
    // Mesmo assim suas inclusões positivas não desaparecem do supergrafo;
    // isso não fecha a fronteira ou fornece uma prova de não participação.
    for &(object, value) in &a.write_inclusions {
        match (
            a.points().variable(object).and_then(|p| p.nos()),
            a.points().variable(value).and_then(|p| p.nos()),
        ) {
            (Some(origins), Some(dests)) => {
                for origin in origins {
                    edges
                        .entry(origin.clone())
                        .or_default()
                        .extend(dests.iter().cloned());
                    for dest in dests {
                        edges.entry(dest.clone()).or_default();
                    }
                }
            }
            _ => opaque = true,
        }
    }
    let nos: Vec<_> = edges.keys().cloned().collect();
    let ids: BTreeMap<_, _> = nos.iter().enumerate().map(|(i, no)| (no, i)).collect();
    let adjacent: Vec<Vec<_>> = nos
        .iter()
        .map(|no| edges[no].iter().map(|dest| ids[dest]).collect())
        .collect();
    let components = crate::optimize::scc::components(&adjacent)
        .into_iter()
        .map(|c| {
            let cycle = c.len() > 1 || adjacent[c[0]].contains(&c[0]);
            let mut members: Vec<_> = c.into_iter().map(|v| nos[v].clone()).collect();
            members.sort();
            (members, cycle)
        })
        .collect();
    ArcHeapGraph {
        edges,
        components,
        opaque,
        analysis: a,
    }
}

// Confere obrigações por inclusão, não por igualdade com uma nova execução
// do produtor. A análise é premissa local imutável, não certificado global.
fn check_projection(f: &Function, g: &ArcHeapGraph) -> Result<(), String> {
    let a = &g.analysis;
    let require_opaque = || {
        if g.opaque {
            Ok(())
        } else {
            Err("grafo ARC: fronteira opaca apagada".to_string())
        }
    };
    if !a.unknown_set().is_empty() || a.opaque_implicit_emission {
        require_opaque()?;
    }
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.value(*v).is_none_or(|p| p.nos().is_none())
        {
            require_opaque()?;
        }
    }
    for (no, fields) in &a.schemas {
        let exits = g
            .edges
            .get(no)
            .ok_or("grafo ARC: nó de esquema omitido")?;
        for field in fields {
            match a.points().field(no, field).and_then(|p| p.nos()) {
                None => require_opaque()?,
                Some(dests) => {
                    for dest in dests {
                        if !exits.contains(dest) {
                            return Err("grafo ARC: aresta inferida omitida".into());
                        }
                    }
                }
            }
        }
    }
    for &(object, value) in &a.write_inclusions {
        match (
            a.points().variable(object).and_then(|p| p.nos()),
            a.points().variable(value).and_then(|p| p.nos()),
        ) {
            (Some(origins), Some(dests)) => {
                for origin in origins {
                    let exits = g
                        .edges
                        .get(origin)
                        .ok_or("grafo ARC: receiver de store omitido")?;
                    if !dests.is_subset(exits) {
                        return Err("grafo ARC: inclusão de store omitida".into());
                    }
                }
            }
            _ => require_opaque()?,
        }
    }
    // Inclui nós extras de sobreaproximações: um destino sem esquema não
    // pode ser tratado como folha fechada só por não ter arestas explícitas.
    for (no, exits) in &g.edges {
        if !a.schemas.contains_key(no) {
            require_opaque()?;
        }
        if exits
            .iter()
            .any(|dest| !g.edges.contains_key(dest))
        {
            return Err("grafo ARC: destino fora do grafo".into());
        }
    }
    let ids: BTreeMap<_, _> = g
        .edges
        .keys()
        .enumerate()
        .map(|(i, no)| (no, i))
        .collect();
    let adjacent: Vec<Vec<_>> = g
        .edges
        .values()
        .map(|exits| exits.iter().map(|no| ids[no]).collect())
        .collect();
    let mut cs = Vec::new();
    for (members, cycle) in &g.components {
        let c: Vec<_> = members
            .iter()
            .map(|no| ids.get(no).copied().ok_or("grafo ARC: membro SCC ausente"))
            .collect::<Result<_, _>>()?;
        if c.is_empty() {
            return Err("grafo ARC: SCC vazia".into());
        }
        if *cycle != (c.len() > 1 || adjacent[c[0]].contains(&c[0])) {
            return Err("grafo ARC: marca de ciclo inválida".into());
        }
        cs.push(c);
    }
    crate::optimize::scc::check(&adjacent, &cs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::*;

    fn module(mask: usize) -> Module {
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
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
                if mask & (1 << (3 * v + w)) != 0 {
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
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        m
    }

    #[test]
    fn stores_por_chamada_preservam_os_512_grafos_sem_fechar_fronteiras() {
        for mask in 0..512 {
            let mut m = module(mask);
            for (_, i, _) in &mut m.functions[0].blocks[0].instructions {
                if let Instruction::SetField { object, value, .. } = i {
                    *i = Instruction::CallStatic {
                        symbol: "ligar".into(),
                        args: vec![object.clone(), value.clone()],
                        ret_ty: Type::Void,
                    };
                }
            }
            m.functions.push(Function {
                symbol: "ligar".into(),
                name: "ligar".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "a".into(), Type::Ref),
                    (ValueId(1), "b".into(), Type::Ref),
                ],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut g = build_local_in_module(&m, "f", 16).unwrap().unwrap();
            assert!(g.opaque());
            assert_eq!(g.edges.len(), 3);
            let nos: Vec<_> = (0..3)
                .map(|v| AbstractNode::Allocation {
                    site: m.arc_sites["f"][&ValueId(v)].clone(),
                    context: "".into(),
                })
                .collect();
            let mut reachability = [[false; 3]; 3];
            for v in 0..3 {
                for w in 0..3 {
                    reachability[v][w] = mask & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.edges[&nos[v]].contains(&nos[w]), reachability[v][w]);
                    if reachability[v][w] {
                        g.edges.get_mut(&nos[v]).unwrap().remove(&nos[w]);
                        assert!(g.check_in_module(&m, "f").is_err());
                        g.edges.get_mut(&nos[v]).unwrap().insert(nos[w].clone());
                    }
                }
            }
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            for v in 0..3 {
                for w in 0..3 {
                    let together = g
                        .components
                        .iter()
                        .any(|(c, _)| c.contains(&nos[v]) && c.contains(&nos[w]));
                    assert_eq!(together, v == w || (reachability[v][w] && reachability[w][v]));
                }
            }
            g.check_in_module(&m, "f").unwrap();
            if mask != 0 {
                m.functions[1].blocks[0].instructions[0].1 = Instruction::SetField {
                    object: Operand::Val(ValueId(1)),
                    index: 0,
                    value: Operand::Val(ValueId(0)),
                };
                assert!(g.check_in_module(&m, "f").is_err());
            }
            m.arc_memory = false;
            assert!(build_local_in_module(&m, "f", 16).unwrap().is_none());
        }
    }

    #[test]
    fn field_scc_matches_floyd_in_512_hir_graphs() {
        for mask in 0..512 {
            let m = module(mask);
            let mut g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
            // Campos/alocação em linha exigem contexto: a conferência de
            // pilha do prólogo permanece opaca, sem apagar as arestas.
            assert!(g.opaque());
            assert_eq!(g.edges().len(), 3);
            let nos: Vec<_> = (0..3)
                .map(|v| AbstractNode::Allocation {
                    site: m.arc_sites["f"][&ValueId(v)].clone(),
                    context: "".into(),
                })
                .collect();
            let mut reachability = [[false; 3]; 3];
            for (v, line) in reachability.iter_mut().enumerate() {
                for (w, dest) in line.iter_mut().enumerate() {
                    *dest = mask & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.edges()[&nos[v]].contains(&nos[w]), *dest);
                }
            }
            // Adulterações são conferidas contra as obrigações inferidas,
            // não contra uma nova execução do produtor/Tarjan.
            g.check_in_module(&m, "f").unwrap();
            for no in &nos {
                let dests: Vec<_> = g.edges[no].iter().cloned().collect();
                for dest in dests {
                    g.edges.get_mut(no).unwrap().remove(&dest);
                    assert!(g.check_in_module(&m, "f").is_err());
                    g.edges.get_mut(no).unwrap().insert(dest);
                }
                let exits = g.edges.remove(no).unwrap();
                assert!(g.check_in_module(&m, "f").is_err());
                g.edges.insert(no.clone(), exits);
            }
            g.opaque = false;
            assert!(g.check_in_module(&m, "f").is_err());
            g.opaque = true;
            for id in 0..g.components.len() {
                g.components[id].1 = !g.components[id].1;
                assert!(g.check_in_module(&m, "f").is_err());
                g.components[id].1 = !g.components[id].1;
            }
            let last_one = g.components.pop().unwrap();
            assert!(g.check_in_module(&m, "f").is_err());
            g.components.push(last_one);
            g.check_in_module(&m, "f").unwrap();
            // Fecho sem identidade: diagonal só verdadeira quando há ciclo.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            let mut owners = [usize::MAX; 3];
            for (id, (members, cycle)) in g.components().enumerate() {
                for no in members {
                    let v = nos.iter().position(|n| n == no).unwrap();
                    assert_eq!(owners[v], usize::MAX);
                    owners[v] = id;
                    assert_eq!(cycle, reachability[v][v], "grafo {mask}, nó {v}");
                }
            }
            for v in 0..3 {
                for w in 0..3 {
                    assert_eq!(
                        owners[v] == owners[w],
                        v == w || (reachability[v][w] && reachability[w][v])
                    );
                }
            }
        }
    }

    #[test]
    fn check_graph_accepts_supergraph_and_rejects_tampered_sccs() {
        let m = module(0);
        let mut g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        let nos: Vec<_> = g.edges.keys().cloned().collect();
        g.edges.get_mut(&nos[0]).unwrap().insert(nos[1].clone());
        g.components = vec![
            (vec![nos[1].clone()], false),
            (vec![nos[0].clone()], false),
            (vec![nos[2].clone()], false),
        ];
        g.check_in_module(&m, "f").unwrap();
        // União de nós sem alcance de volta não é componente forte.
        g.components = vec![
            (vec![nos[0].clone(), nos[1].clone()], true),
            (vec![nos[2].clone()], false),
        ];
        assert!(g.check_in_module(&m, "f").is_err());
        // Acrescentar aresta de volta torna a união forte, sem executar Tarjan.
        g.edges.get_mut(&nos[1]).unwrap().insert(nos[0].clone());
        g.check_in_module(&m, "f").unwrap();
        // Dividir o ciclo em duas SCCs é inválido mesmo com cobertura única.
        g.components = vec![
            (vec![nos[1].clone()], false),
            (vec![nos[0].clone()], false),
            (vec![nos[2].clone()], false),
        ];
        assert!(g.check_in_module(&m, "f").is_err());
        g.components = vec![
            (vec![nos[0].clone(), nos[1].clone()], true),
            (vec![nos[2].clone()], false),
        ];
        g.components.push((vec![nos[2].clone()], false));
        assert!(g.check_in_module(&m, "f").is_err());
        g.components.pop();
        g.components.push((vec![], false));
        assert!(g.check_in_module(&m, "f").is_err());
        g.components.pop();
        let external = AbstractNode::Global("externo".into());
        g.edges.get_mut(&nos[0]).unwrap().insert(external.clone());
        assert!(g.check_in_module(&m, "f").is_err());
        g.edges.insert(external.clone(), BTreeSet::new());
        // Nó extra sem esquema exige fronteira opaca e partição completa.
        g.components.insert(0, (vec![external], false));
        g.check_in_module(&m, "f").unwrap();
        g.opaque = false;
        assert!(g.check_in_module(&m, "f").is_err());
    }

    #[test]
    fn check_graph_rejects_body_callee_layout_site_and_mode_changes() {
        let mut m = module(0);
        m.functions.push(Function {
            symbol: "id".into(),
            name: "id".into(),
            debug_info: None,
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
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        g.check_in_module(&m, "f").unwrap();
        // Reordenação nominal mantém dependências; símbolo é a identidade.
        m.functions.swap(0, 1);
        g.check_in_module(&m, "f").unwrap();
        m.functions.swap(0, 1);
        m.arc_memory = false;
        assert!(g.check_in_module(&m, "f").is_err());
        m.arc_memory = true;
        m.excecoes_por_tabelas = true;
        assert!(g.check_in_module(&m, "f").is_err());
        m.excecoes_por_tabelas = false;
        m.arc_field_layouts.insert(1, vec![Type::I64]);
        assert!(g.check_in_module(&m, "f").is_err());
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        let site = m
            .arc_sites
            .get_mut("f")
            .unwrap()
            .remove(&ValueId(0))
            .unwrap();
        assert!(g.check_in_module(&m, "f").is_err());
        m.arc_sites.get_mut("f").unwrap().insert(ValueId(0), site);
        let ret = m.functions[1].blocks[0].terminator.clone();
        m.functions[1].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(g.check_in_module(&m, "f").is_err());
        m.functions[1].blocks[0].terminator = ret;
        let ret = m.functions[0].blocks[0].terminator.clone();
        m.functions[0].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(g.check_in_module(&m, "f").is_err());
        m.functions[0].blocks[0].terminator = ret;
        let callee = m.functions.pop().unwrap();
        assert!(g.check_in_module(&m, "f").is_err());
        m.functions.push(callee.clone());
        m.functions.push(callee);
        assert!(g.check_in_module(&m, "f").is_err());
        m.functions.pop();
        g.check_in_module(&m, "f").unwrap();
        assert!(g.check_in_module(&m, "id").is_err());
    }

    #[test]
    fn opaque_boundaries_and_budget_do_not_become_closed_graph() {
        let mut m = module(511);
        assert!(
            build_local_in_module(&m, "f", 1)
                .unwrap()
                .unwrap()
                .opaque()
        );
        assert!(
            build_local_in_module(&m, "f", 0)
                .unwrap()
                .unwrap()
                .opaque()
        );
        m.arc_field_layouts.clear();
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaque());
        assert!(g.edges().is_empty());
        m.arc_memory = false;
        assert!(build_local_in_module(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn summarized_instances_at_same_site_do_not_prove_singleton_or_cycle_absence() {
        let mut m = module(1 << 1); // Instância 0 aponta para instância 1.
        let origin = m.arc_sites["f"][&ValueId(0)].clone();
        m.arc_sites
            .get_mut("f")
            .unwrap()
            .insert(ValueId(1), origin);
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        assert_eq!(g.edges().len(), 2);
        assert!(g.components().any(|(_, cycle)| cycle));
        assert!(g.opaque()); // O prólogo com contexto ainda não possui resumo.
    }

    #[test]
    fn opaque_parameter_does_not_become_empty_closed_graph() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(Function {
            symbol: "id".into(),
            name: "id".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        let g = build_local_in_module(&m, "id", 8).unwrap().unwrap();
        assert!(g.opaque());
        assert!(g.edges().is_empty());
    }

    #[test]
    fn call_recursion_does_not_forge_object_self_edge() {
        let mut m = module(0);
        m.functions[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "f".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaque());
        assert!(g.components().all(|(_, cycle)| !cycle));
        assert_eq!(g.edges().len(), 3);
    }
}
