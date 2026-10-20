//! Local graph of sites/fields and object SCCs (§29.1).
//! The absence of a known cycle is not a certificate for the whole lifetime of the object.

use super::{hir::HirAnalysis, model::AbstractNode};
use crate::hir::{Function, Module, Type};
use std::collections::{BTreeMap, BTreeSet};

/// An immutable snapshot of the inferred local edges, including opaque boundaries.
/// An SCC represents an abstract possibility, not a concrete group of instances.
/// Does not select a policy nor certify NaoParticipa/AciclicoProvado: future
/// mutations, external entries and observers require the global coverage of §29.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::analysis::graph::*};
/// assert!(build_local_in_module(&Module::new(), "f", 8)?.is_none());
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
    /// Checks local dependencies and the points-to projection without repeating the solver
    /// nor Tarjan. Requires every inferred node/edge, preserves lack of knowledge
    /// and checks the SCC partition and its cycle marks independently.
    /// Over-approximations are accepted, but cannot erase opaque boundaries.
    /// Does not prove global coverage, lifetime, SDK, observers or policy.
    ///
    /// # Errors
    /// Stale module/body/tables, an omitted node/edge, a missing destination,
    /// erased opacity or an invalid partition/cycle mark.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::graph::*};
    /// let mut m = Module::new(); m.arc_memory = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// build_local_in_module(&m, "f", 8)?.unwrap().check_in_module(&m, "f")?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn check_in_module(&self, m: &Module, symbol: &str) -> Result<(), String> {
        let f = super::escape::check_analysis_in_module(m, symbol, &self.analysis)?;
        check_projection(f, self)
    }

    /// Queries the known edges without erasing the opaque boundaries.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::graph::*};
    /// let mut m = Module::new(); m.arc_memory = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(build_local_in_module(&m, "f", 8)?.unwrap().edges().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn edges(&self) -> &BTreeMap<AbstractNode, BTreeSet<AbstractNode>> {
        &self.edges
    }

    /// Queries whether edges/schemas or effects are missing from the local snapshot.
    /// False does not prove coverage during the whole lifetime of any instance.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::graph::*};
    /// let mut m = Module::new(); m.arc_memory = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(!build_local_in_module(&m, "f", 8)?.unwrap().opaque());
    /// # Ok::<(), String>(())
    /// ```
    pub fn opaque(&self) -> bool {
        self.opaque
    }

    /// Enumerates SCCs and the presence of an abstract cycle in the known edges.
    /// A unit component is cyclic only when it contains a self-edge.
    /// A false does not exclude cycles through opaque boundaries/future mutations.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::graph::*};
    /// let mut m = Module::new(); m.arc_memory = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert_eq!(build_local_in_module(&m, "f", 8)?.unwrap().components().count(), 0);
    /// # Ok::<(), String>(())
    /// ```
    pub fn components(&self) -> impl Iterator<Item = (&[AbstractNode], bool)> {
        self.components
            .iter()
            .map(|(nodes, cycle)| (nodes.as_slice(), *cycle))
    }
}

/// Analyzes the body and extracts SCCs of field edges, without using call SCCs.
/// Only in the ARC module; tracing returns None. The result is local to the current version,
/// not a certificate of policy, concrete group, lifetime or program coverage.
///
/// # Errors
/// Invalid body/SSA or site/layout/summary premises in the module analysis.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::analysis::graph::*};
/// assert!(build_local_in_module(&Module::new(), "absent", 8)?.is_none());
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
        .expect("analyzed body");
    let g = build(f, a);
    g.check_in_module(m, symbol)?;
    Ok(Some(g))
}

fn build(f: &Function, a: HirAnalysis) -> ArcHeapGraph {
    let mut edges: BTreeMap<_, BTreeSet<_>> = a
        .schemas
        .keys()
        .cloned()
        .map(|node| (node, BTreeSet::new()))
        .collect();
    let mut opaque = !a.unknown_set().is_empty() || a.opaque_implicit_emission;
    // Values that may be handles/addresses without a closed origin keep
    // a boundary, even when no edge of that value was materialized.
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.value(*v).is_none_or(|p| p.nodes().is_none())
        {
            opaque = true;
        }
    }
    for (node, fields) in &a.schemas {
        for field in fields {
            let Some(dests) = a.points().field(node, field).and_then(|p| p.nodes()) else {
                opaque = true;
                continue;
            };
            for dest in dests {
                edges.entry(node.clone()).or_default().insert(dest.clone());
                edges.entry(dest.clone()).or_default();
                if !a.schemas.contains_key(dest) {
                    opaque = true;
                }
            }
        }
    }
    // A writer covered only in the explicit HIR may keep top fields.
    // Even so its positive inclusions do not disappear from the supergraph;
    // that does not close the boundary nor provide a proof of non-participation.
    for &(object, value) in &a.write_inclusions {
        match (
            a.points().variable(object).and_then(|p| p.nodes()),
            a.points().variable(value).and_then(|p| p.nodes()),
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
    let nodes: Vec<_> = edges.keys().cloned().collect();
    let ids: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node, i))
        .collect();
    let adjacent: Vec<Vec<_>> = nodes
        .iter()
        .map(|node| edges[node].iter().map(|dest| ids[dest]).collect())
        .collect();
    let components = crate::optimize::scc::components(&adjacent)
        .into_iter()
        .map(|c| {
            let cycle = c.len() > 1 || adjacent[c[0]].contains(&c[0]);
            let mut members: Vec<_> = c.into_iter().map(|v| nodes[v].clone()).collect();
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

// Checks obligations by inclusion, not by equality with a new run
// of the producer. The analysis is an immutable local premise, not a global certificate.
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
            && a.value(*v).is_none_or(|p| p.nodes().is_none())
        {
            require_opaque()?;
        }
    }
    for (node, fields) in &a.schemas {
        let exits = g.edges.get(node).ok_or("ARC graph: schema node omitted")?;
        for field in fields {
            match a.points().field(node, field).and_then(|p| p.nodes()) {
                None => require_opaque()?,
                Some(dests) => {
                    for dest in dests {
                        if !exits.contains(dest) {
                            return Err("ARC graph: inferred edge omitted".into());
                        }
                    }
                }
            }
        }
    }
    for &(object, value) in &a.write_inclusions {
        match (
            a.points().variable(object).and_then(|p| p.nodes()),
            a.points().variable(value).and_then(|p| p.nodes()),
        ) {
            (Some(origins), Some(dests)) => {
                for origin in origins {
                    let exits = g
                        .edges
                        .get(origin)
                        .ok_or("ARC graph: store receiver omitted")?;
                    if !dests.is_subset(exits) {
                        return Err("ARC graph: store inclusion omitted".into());
                    }
                }
            }
            _ => require_opaque()?,
        }
    }
    // Includes extra nodes from over-approximations: a destination without a schema cannot
    // be treated as a closed leaf just because it has no explicit edges.
    for (node, exits) in &g.edges {
        if !a.schemas.contains_key(node) {
            require_opaque()?;
        }
        if exits.iter().any(|dest| !g.edges.contains_key(dest)) {
            return Err("ARC graph: destination outside the graph".into());
        }
    }
    let ids: BTreeMap<_, _> = g
        .edges
        .keys()
        .enumerate()
        .map(|(i, node)| (node, i))
        .collect();
    let adjacent: Vec<Vec<_>> = g
        .edges
        .values()
        .map(|exits| exits.iter().map(|node| ids[node]).collect())
        .collect();
    let mut cs = Vec::new();
    for (members, cycle) in &g.components {
        let c: Vec<_> = members
            .iter()
            .map(|node| {
                ids.get(node)
                    .copied()
                    .ok_or("ARC graph: missing SCC member")
            })
            .collect::<Result<_, _>>()?;
        if c.is_empty() {
            return Err("grafo ARC: SCC vazia".into());
        }
        if *cycle != (c.len() > 1 || adjacent[c[0]].contains(&c[0])) {
            return Err("ARC graph: invalid cycle mark".into());
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
    fn per_call_stores_preserve_the_512_graphs_without_closing_boundaries() {
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
            let nodes: Vec<_> = (0..3)
                .map(|v| AbstractNode::Allocation {
                    site: m.arc_sites["f"][&ValueId(v)].clone(),
                    context: "".into(),
                })
                .collect();
            let mut reachability = [[false; 3]; 3];
            for v in 0..3 {
                for w in 0..3 {
                    reachability[v][w] = mask & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.edges[&nodes[v]].contains(&nodes[w]), reachability[v][w]);
                    if reachability[v][w] {
                        g.edges.get_mut(&nodes[v]).unwrap().remove(&nodes[w]);
                        assert!(g.check_in_module(&m, "f").is_err());
                        g.edges.get_mut(&nodes[v]).unwrap().insert(nodes[w].clone());
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
                        .any(|(c, _)| c.contains(&nodes[v]) && c.contains(&nodes[w]));
                    assert_eq!(
                        together,
                        v == w || (reachability[v][w] && reachability[w][v])
                    );
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
            // Inline fields/allocation require a context: the stack check
            // of the prologue stays opaque, without erasing the edges.
            assert!(g.opaque());
            assert_eq!(g.edges().len(), 3);
            let nodes: Vec<_> = (0..3)
                .map(|v| AbstractNode::Allocation {
                    site: m.arc_sites["f"][&ValueId(v)].clone(),
                    context: "".into(),
                })
                .collect();
            let mut reachability = [[false; 3]; 3];
            for (v, line) in reachability.iter_mut().enumerate() {
                for (w, dest) in line.iter_mut().enumerate() {
                    *dest = mask & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.edges()[&nodes[v]].contains(&nodes[w]), *dest);
                }
            }
            // Tampering is checked against the inferred obligations,
            // not against a new run of the producer/Tarjan.
            g.check_in_module(&m, "f").unwrap();
            for node in &nodes {
                let dests: Vec<_> = g.edges[node].iter().cloned().collect();
                for dest in dests {
                    g.edges.get_mut(node).unwrap().remove(&dest);
                    assert!(g.check_in_module(&m, "f").is_err());
                    g.edges.get_mut(node).unwrap().insert(dest);
                }
                let exits = g.edges.remove(node).unwrap();
                assert!(g.check_in_module(&m, "f").is_err());
                g.edges.insert(node.clone(), exits);
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
            // Closure without identity: the diagonal is only true when there is a cycle.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            let mut owners = [usize::MAX; 3];
            for (id, (members, cycle)) in g.components().enumerate() {
                for node in members {
                    let v = nodes.iter().position(|n| n == node).unwrap();
                    assert_eq!(owners[v], usize::MAX);
                    owners[v] = id;
                    assert_eq!(cycle, reachability[v][v], "graph {mask}, node {v}");
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
        let nodes: Vec<_> = g.edges.keys().cloned().collect();
        g.edges.get_mut(&nodes[0]).unwrap().insert(nodes[1].clone());
        g.components = vec![
            (vec![nodes[1].clone()], false),
            (vec![nodes[0].clone()], false),
            (vec![nodes[2].clone()], false),
        ];
        g.check_in_module(&m, "f").unwrap();
        // A union of nodes without reachability back is not a strong component.
        g.components = vec![
            (vec![nodes[0].clone(), nodes[1].clone()], true),
            (vec![nodes[2].clone()], false),
        ];
        assert!(g.check_in_module(&m, "f").is_err());
        // Adding a back edge makes the union strong, without running Tarjan.
        g.edges.get_mut(&nodes[1]).unwrap().insert(nodes[0].clone());
        g.check_in_module(&m, "f").unwrap();
        // Splitting the cycle into two SCCs is invalid even with unique coverage.
        g.components = vec![
            (vec![nodes[1].clone()], false),
            (vec![nodes[0].clone()], false),
            (vec![nodes[2].clone()], false),
        ];
        assert!(g.check_in_module(&m, "f").is_err());
        g.components = vec![
            (vec![nodes[0].clone(), nodes[1].clone()], true),
            (vec![nodes[2].clone()], false),
        ];
        g.components.push((vec![nodes[2].clone()], false));
        assert!(g.check_in_module(&m, "f").is_err());
        g.components.pop();
        g.components.push((vec![], false));
        assert!(g.check_in_module(&m, "f").is_err());
        g.components.pop();
        let external = AbstractNode::Global("externo".into());
        g.edges.get_mut(&nodes[0]).unwrap().insert(external.clone());
        assert!(g.check_in_module(&m, "f").is_err());
        g.edges.insert(external.clone(), BTreeSet::new());
        // An extra node without a schema requires an opaque boundary and a complete partition.
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
        // Nominal reordering keeps the dependencies; the symbol is the identity.
        m.functions.swap(0, 1);
        g.check_in_module(&m, "f").unwrap();
        m.functions.swap(0, 1);
        m.arc_memory = false;
        assert!(g.check_in_module(&m, "f").is_err());
        m.arc_memory = true;
        m.table_exceptions = true;
        assert!(g.check_in_module(&m, "f").is_err());
        m.table_exceptions = false;
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
        assert!(build_local_in_module(&m, "f", 1).unwrap().unwrap().opaque());
        assert!(build_local_in_module(&m, "f", 0).unwrap().unwrap().opaque());
        m.arc_field_layouts.clear();
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaque());
        assert!(g.edges().is_empty());
        m.arc_memory = false;
        assert!(build_local_in_module(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn summarized_instances_at_same_site_do_not_prove_singleton_or_cycle_absence() {
        let mut m = module(1 << 1); // Instance 0 points to instance 1.
        let origin = m.arc_sites["f"][&ValueId(0)].clone();
        m.arc_sites.get_mut("f").unwrap().insert(ValueId(1), origin);
        let g = build_local_in_module(&m, "f", 8).unwrap().unwrap();
        assert_eq!(g.edges().len(), 2);
        assert!(g.components().any(|(_, cycle)| cycle));
        assert!(g.opaque()); // The prologue with a context has no summary yet.
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
