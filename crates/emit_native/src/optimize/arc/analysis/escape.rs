//! Local publications and their transitive reach (§28.2 and §30.1).
//! Does not compute regions/observers or interprocedural effects of calls.

use super::{hir::*, model::*};
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Conservative publication destination; an opaque operation may retain or reenter.
/// This does not mean every possible publication happens in an execution.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::escape::EscapeCause;
/// assert_ne!(EscapeCause::Return, EscapeCause::Exception);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EscapeCause {
    /// A value returned to the caller and its possible descendants.
    Return,
    /// A thrown value, including the objects reachable from the payload.
    Exception,
    /// Publication in a nominal global root.
    Global(String),
    /// Local ID of an operation without a valid heap/retention summary.
    OpaqueOperation(u32),
    /// Prologue/exit inserted at emission, without a validated heap summary.
    ImplicitEmission,
}

/// Possible published reach, grouped by cause. Top prevents a negative proof.
/// The absence of a cause here does not prove confinement: observers/externs/SDK,
/// callbacks, lifetimes and exceptional exits still require full coverage.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::escape::EscapeResult;
/// assert!(EscapeResult::default().publications.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct EscapeResult {
    /// Transitive sets, or top when any boundary is unknown.
    pub publications: BTreeMap<EscapeCause, PointSet>,
}

/// Computes publications over the solution of the same local version of the function.
/// Walks every field of the schema of each node and keeps cycles/aliases;
/// a missing schema/field, an opaque field and an exceeded budget propagate top.
/// An opaque operation gets unknown reach, even without arguments,
/// since it may access unsummarized globals/callbacks. Does not infer noescape
/// from the Borrowed contract of the ABI. Does not transform the HIR nor select a policy.
///
/// # Errors
/// The body changed after the points-to solution, or there are callee dependencies
/// that must be checked by [`compute_in_module`]. Recompute when needed.
/// The hash is a local guard, not a versioned SDK/JIT dependency.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::{hir::*, escape::*}};
/// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let a = analyze(&f, &HirFacts::default(), 8)?;
/// assert!(compute(&f, &a)?.publications[&EscapeCause::Return].nodes().is_none());
/// # Ok::<(), String>(())
/// ```
pub fn compute(f: &Function, a: &HirAnalysis) -> Result<EscapeResult, String> {
    if !a.body_dependencies.is_empty()
        || !a.table_dependencies.is_empty()
        || a.module_premises.is_some()
    {
        return Err("ARC escape: validate dependencies through the module".into());
    }
    compute_validated(f, a)
}

/// Checks the bodies and tables of the callees used for aliases/fields, before the escape.
/// Checks the memory mode, the caller's sites and local layouts when the
/// solution came from the module. Does not validate pins, JIT generations or SDK summaries.
/// Implicit paths recorded by the module publish top under
/// [`EscapeCause::ImplicitEmission`], without forging an HIR instruction.
///
/// # Errors
/// Missing, duplicated or changed body/callee, modified emission table
/// or different local premises of mode/sites/layouts.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::{hir::*, escape::*}};
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// let a = analyze(&f, &HirFacts::default(), 8)?;
/// let mut m = Module::new(); m.functions.push(f);
/// assert!(compute_in_module(&m, "f", &a)?.publications.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn compute_in_module(
    m: &Module,
    symbol: &str,
    a: &HirAnalysis,
) -> Result<EscapeResult, String> {
    compute_validated(check_analysis_in_module(m, symbol, a)?, a)
}

// A guard shared by the escape and graph consumers, without resolving points-to.
pub(super) fn check_analysis_in_module<'m>(
    m: &'m Module,
    symbol: &str,
    a: &HirAnalysis,
) -> Result<&'m Function, String> {
    if let Some(signature) = a.module_premises
        && signature != super::module::premises_signature(m, symbol)
    {
        return Err("ARC escape: mode, sites or layouts changed".into());
    }
    let mut index = std::collections::HashMap::new();
    let positions: std::collections::HashMap<_, _> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    for f in &m.functions {
        if index.insert(&f.symbol, f).is_some() {
            return Err("ARC escape: duplicate body".into());
        }
    }
    for (s, signature) in &a.body_dependencies {
        let f = index.get(s).ok_or("ARC escape: missing callee")?;
        if body_signature(f) != *signature {
            return Err(format!("escape ARC: callee mudou {s}"));
        }
    }
    for (s, signature) in &a.table_dependencies {
        let index = positions
            .get(s.as_str())
            .ok_or("ARC escape: missing callee")?;
        if super::hir::table_state(m, *index) != *signature {
            return Err(format!("escape ARC: tabela mudou {s}"));
        }
    }
    let f = index
        .get(&symbol.to_string())
        .ok_or("ARC escape: missing body")?;
    if a.body != body_signature(f) {
        return Err("ARC escape: body changed after points-to".into());
    }
    Ok(f)
}

fn compute_validated(f: &Function, a: &HirAnalysis) -> Result<EscapeResult, String> {
    if a.body != body_signature(f) {
        return Err("ARC escape: body changed after points-to".into());
    }
    let mut r = EscapeResult::default();
    let operand = |o: &Operand| match o {
        Operand::Val(v) => a
            .value(*v)
            .cloned()
            .unwrap_or_else(|| PointSet::unknown(a.limit)),
        Operand::Constant(Constant::Null) => PointSet::empty(a.limit),
        _ => PointSet::unknown(a.limit),
    };
    let mut publish = |cause, roots: PointSet| {
        let dests = reachability(a, &roots);
        r.publications
            .entry(cause)
            .or_insert_with(|| PointSet::empty(a.limit))
            .join_set(&dests);
    };
    for b in &f.blocks {
        match &b.terminator {
            Terminator::Return(Some(o)) => publish(EscapeCause::Return, operand(o)),
            Terminator::Throw(o) => publish(EscapeCause::Exception, operand(o)),
            _ => {}
        }
        for (_, i, _) in &b.instructions {
            match i {
                Instruction::StoreGlobal { symbol, val, .. }
                | Instruction::ArcStoreStrong {
                    slot: StrongSlot::Global { symbol },
                    value: val,
                    ..
                } => publish(EscapeCause::Global(symbol.clone()), operand(val)),
                _ => {}
            }
        }
    }
    // Inclusions do not waive the opaque publication of the call nor of the emission.
    for (cause, variable) in &a.publication_inclusions {
        let points = a
            .points()
            .variable(*variable)
            .ok_or("ARC escape: invalid publication variable")?;
        publish(cause.clone(), points.clone());
    }
    for id in a.unknown_set() {
        publish(
            EscapeCause::OpaqueOperation(id.0),
            PointSet::unknown(a.limit),
        );
    }
    if a.opaque_implicit_emission {
        publish(EscapeCause::ImplicitEmission, PointSet::unknown(a.limit));
    }
    Ok(r)
}

fn reachability(a: &HirAnalysis, roots: &PointSet) -> PointSet {
    let mut result = roots.clone();
    let Some(nodes) = roots.nodes() else {
        return result;
    };
    let mut queue: VecDeque<_> = nodes.iter().cloned().collect();
    let mut seen_set = BTreeSet::new();
    while let Some(node) = queue.pop_front() {
        if !seen_set.insert(node.clone()) {
            continue;
        }
        let Some(fields) = a.schemas.get(&node) else {
            return PointSet::unknown(a.limit);
        };
        // Inclusions may represent side references (attachments), without a
        // physical field. They also publish their destinations transitively.
        for &(object, value) in &a.write_inclusions {
            let Some(dests) = a.points().variable(value) else {
                return PointSet::unknown(a.limit);
            };
            if dests.nodes().is_some_and(|ns| ns.is_empty()) {
                continue;
            }
            let Some(origins) = a.points().variable(object).and_then(|p| p.nodes()) else {
                return PointSet::unknown(a.limit);
            };
            if !origins.contains(&node) {
                continue;
            }
            let Some(nodes) = dests.nodes() else {
                return PointSet::unknown(a.limit);
            };
            queue.extend(nodes.iter().cloned());
            result.join_set(dests);
            if result.nodes().is_none() {
                return result;
            }
        }
        for field in fields {
            let Some(dests) = a.points().field(&node, field) else {
                return PointSet::unknown(a.limit);
            };
            let Some(nodes) = dests.nodes() else {
                return PointSet::unknown(a.limit);
            };
            for child in nodes {
                queue.push_back(child.clone());
            }
            result.join_set(dests);
            if result.nodes().is_none() {
                return result;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::{hir::ObjectOrigin, points_to::ArcField};
    use super::*;
    use std::collections::HashMap;

    fn graph(mask: usize) -> (Function, HirFacts) {
        let field = ArcField {
            layout: "No".into(),
            position: 0,
        };
        let mut facts = HirFacts::default();
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
            facts.objects.insert(
                ValueId(v),
                ObjectOrigin {
                    node: AbstractNode::Allocation {
                        site: ArcSite {
                            function: "f".into(),
                            origin: v as u64,
                            specialization: "".into(),
                        },
                        context: "".into(),
                    },
                    fields: vec![field.clone()],
                },
            );
        }
        for v in 0..3 {
            for w in 0..3 {
                if mask & (1 << (3 * v + w)) != 0 {
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
                    facts.accesses.insert(id, field.clone());
                }
            }
        }
        (
            Function {
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
            },
            facts,
        )
    }

    #[test]
    fn return_publishes_descendants_in_all_three_node_graphs() {
        for mask in 0..512 {
            let (f, facts) = graph(mask);
            let a = analyze(&f, &facts, 3).unwrap();
            let r = compute(&f, &a).unwrap();
            // Oracle by reach expansion, without consulting the solver's fields.
            let mut live_set = BTreeSet::from([0]);
            loop {
                let before = live_set.clone();
                for v in before.iter().copied() {
                    for w in 0..3 {
                        if mask & (1 << (3 * v + w)) != 0 {
                            live_set.insert(w);
                        }
                    }
                }
                if live_set == before {
                    break;
                }
            }
            let expected: BTreeSet<_> = live_set
                .iter()
                .map(|v| facts.objects[&ValueId(*v as u32)].node.clone())
                .collect();
            assert_eq!(
                r.publications[&EscapeCause::Return].nodes().unwrap(),
                &expected
            );
        }
    }

    #[test]
    fn exception_publishes_payload_and_stale_analysis_is_rejected() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        assert_eq!(
            r.publications[&EscapeCause::Exception]
                .nodes()
                .unwrap()
                .len(),
            2
        );
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(compute(&f, &a).is_err());
    }

    #[test]
    fn opacity_and_budget_never_prove_confinement() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::CallRuntime {
                name: "opaca".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        assert!(r.publications[&EscapeCause::Return].nodes().is_none());
        assert!(
            r.publications[&EscapeCause::OpaqueOperation(9)]
                .nodes()
                .is_none()
        );
        f.blocks[0].instructions.pop();
        let a = analyze(&f, &facts, 1).unwrap();
        assert!(
            compute(&f, &a).unwrap().publications[&EscapeCause::Return]
                .nodes()
                .is_none()
        );
        let a = analyze(
            &f,
            &HirFacts {
                objects: HashMap::new(),
                accesses: HashMap::new(),
            },
            3,
        )
        .unwrap();
        assert!(
            compute(&f, &a).unwrap().publications[&EscapeCause::Return]
                .nodes()
                .is_none()
        );
    }

    #[test]
    fn global_publication_is_registered_even_without_object_return() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::StoreGlobal {
                symbol: "g".into(),
                val: Operand::Val(ValueId(0)),
                ty: Type::Ref,
                root: Some(0),
            },
            Type::Void,
        ));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        // A global store is still opaque to the heap effects domain: do not
        // forge transitive precision before having that contract.
        assert!(
            r.publications[&EscapeCause::Global("g".into())]
                .nodes()
                .is_none()
        );
        assert!(
            r.publications[&EscapeCause::Return]
                .nodes()
                .unwrap()
                .is_empty()
        );
    }
}
