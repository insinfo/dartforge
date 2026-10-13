//! Monotonic local solution of normal aliases per call SCC (§28.3).
//! Composes preservation/publication of positive bodies through direct calls.
//! Other effects stay opaque; does not certify ARC policies.

use super::summaries::{ArcHeapSummary, ArcHeapReturn, extract_with_calls};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Summarizes the bodies reachable through the direct calls of the given roots.
/// External targets are not invented. Bottom is internal: cycles without evidence
/// of a normal return are raised to top before the SCC is made available.
pub(super) fn solve<'a>(
    m: &'a Module,
    roots: &HashSet<&str>,
    limit: usize,
) -> Result<HashMap<String, (&'a Function, ArcHeapSummary)>, String> {
    let mut index = HashMap::new();
    let mut positions = HashMap::new();
    for (position, f) in m.functions.iter().enumerate() {
        positions.insert(f.symbol.as_str(), position);
        if index.insert(f.symbol.as_str(), f).is_some() {
            return Err(format!("summary: duplicate body {}", f.symbol));
        }
    }
    let mut seen_set = HashSet::new();
    let mut queue: VecDeque<_> = roots.iter().copied().collect();
    let mut bodies = Vec::new();
    while let Some(s) = queue.pop_front() {
        if !seen_set.insert(s) {
            continue;
        }
        let Some(&f) = index.get(s) else { continue };
        bodies.push(f);
        queue.extend(targets(f));
    }
    // A nominal order makes the solution and the diagnostics independent of the HashSet.
    bodies.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    // A declaration is an ABI premise, independent of the precision budget.
    for f in &bodies {
        check_globals(m, f)?;
    }
    let facts: Vec<_> = bodies
        .iter()
        .map(|f| super::module::facts_in_module(m, f))
        .collect::<Result<_, _>>()?;
    let ids: HashMap<_, _> = bodies
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    let edges: Vec<Vec<usize>> = bodies
        .iter()
        .map(|f| {
            let mut dests: Vec<_> = targets(f).filter_map(|s| ids.get(s).copied()).collect();
            dests.sort_unstable();
            dests.dedup();
            dests
        })
        .collect();
    let mut dependents = vec![Vec::new(); bodies.len()];
    for (v, dests) in edges.iter().enumerate() {
        for &w in dests {
            dependents[w].push(v);
        }
    }
    let empty = || ArcHeapReturn::Aliases {
        parameters: BTreeSet::new(),
        null: false,
    };
    let mut returns: Vec<_> = bodies
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                empty()
            } else {
                ArcHeapReturn::UnknownValue
            }
        })
        .collect();
    for component in crate::optimize::scc::components(&edges) {
        let members: HashSet<_> = component.iter().copied().collect();
        let mut queue: VecDeque<_> = component.iter().copied().collect();
        let mut queued = members.clone();
        loop {
            while let Some(v) = queue.pop_front() {
                queued.remove(&v);
                let new = extract_with_calls(bodies[v], limit, true, |s| {
                    ids.get(s).map(|&w| (bodies[w], &returns[w]))
                })?;
                if join_set(&mut returns[v], new.ret(), limit) {
                    for &w in &dependents[v] {
                        if members.contains(&w) && queued.insert(w) {
                            queue.push_back(w);
                        }
                    }
                }
            }
            // Do not expose bottom as a proven absence of aliases/null.
            let without_evidence: Vec<_> = component
                .iter()
                .copied()
                .filter(|&v| returns[v] == empty())
                .collect();
            if without_evidence.is_empty() {
                break;
            }
            for v in without_evidence {
                returns[v] = ArcHeapReturn::UnknownValue;
                for &w in &dependents[v] {
                    if members.contains(&w) && queued.insert(w) {
                        queue.push_back(w);
                    }
                }
            }
        }
    }
    // Re-extraction with stabilized dependencies builds the summary guarded
    // by the signature of the body itself, without publishing worklist approximations.
    let mut opaque_context = vec![false; bodies.len()];
    let mut summaries: Vec<_> = bodies
        .iter()
        .enumerate()
        .map(|(v, &f)| {
            let mut summary = extract_with_calls(f, limit, false, |s| {
                ids.get(s).map(|&w| (bodies[w], &returns[w]))
            })?;
            let index = positions[f.symbol.as_str()];
            let table = if m.table_exceptions {
                m.tables.get(index)
            } else {
                None
            };
            if (m.table_exceptions && table.is_none())
                || crate::llvm::LlvmEmitter::requires_explicit_context(f, table)
                || crate::llvm::LlvmEmitter::may_require_context_for_roots(f)
                || table.is_some_and(|t| {
                    !t.invocations.is_empty()
                        || !t.landing_pads.is_empty()
                        || !t.exits.is_empty()
                        || t.foreign_cleanup.is_some()
                })
            {
                opaque_context[v] = true;
                summary.invalidate_coverage();
            }
            debug_assert_eq!(summary.ret(), &returns[v]);
            Ok(summary)
        })
        .collect::<Result<_, String>>()?;
    // The initial state is unknown at the calls. Only promote when
    // all dependencies already have positive coverage; never assume a
    // pure recursive SCC just because no store was found in it.
    let mut queue: VecDeque<_> = (0..bodies.len()).collect();
    let mut queued: HashSet<_> = (0..bodies.len()).collect();
    while let Some(v) = queue.pop_front() {
        queued.remove(&v);
        if opaque_context[v]
            || (summaries[v].fields() == super::summaries::ArcHeapFields::Preserved
                && summaries[v].publications() == super::summaries::ArcHeapPublications::ResultOnly)
        {
            continue;
        }
        if super::summaries::coverage_with_calls(bodies[v], |s| {
            ids.get(s).map(|&w| (bodies[w], &summaries[w]))
        }) {
            summaries[v].mark_coverage();
            for &w in &dependents[v] {
                if queued.insert(w) {
                    queue.push_back(w);
                }
            }
        }
    }
    // Store inclusions have their own fixpoint, without promoting the coverage
    // of fields/publications nor ignoring opaque prologues/exits. Recursion
    // propagates positive inclusions even with opacity; an exceeded budget
    // is absorbing and does not allow rebuilding a truncated set.
    let mut writes = vec![super::writes::WriteInclusions::empty(); bodies.len()];
    let mut globals = vec![super::writes::PublicationInclusions::empty(); bodies.len()];
    let mut paths: Vec<_> = bodies
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                super::writes::ReturnPaths::empty()
            } else {
                super::writes::ReturnPaths::unknown()
            }
        })
        .collect();
    for component in crate::optimize::scc::components(&edges) {
        let members: HashSet<_> = component.iter().copied().collect();
        let mut queue: VecDeque<_> = component.iter().copied().collect();
        let mut queued = members.clone();
        loop {
            while let Some(v) = queue.pop_front() {
                queued.remove(&v);
                let (result, new, publications) = super::writes::extract_with_origins(
                    bodies[v],
                    limit,
                    true,
                    &facts[v].objects,
                    |s| {
                        ids.get(s).map(|&w| {
                            (
                                bodies[w],
                                paths[w].clone(),
                                &writes[w],
                                globals[w].clone(),
                            )
                        })
                    },
                )?;
                let write_changed = writes[v].join_set(&new, limit);
                let global_changed = globals[v].join_set(&publications, limit);
                if paths[v].join_set(&result, limit) || write_changed || global_changed {
                    for &w in &dependents[v] {
                        if members.contains(&w) && queued.insert(w) {
                            queue.push_back(w);
                        }
                    }
                }
            }
            let without_evidence: Vec<_> = component
                .iter()
                .copied()
                .filter(|&v| paths[v] == super::writes::ReturnPaths::empty())
                .collect();
            if without_evidence.is_empty() {
                break;
            }
            for v in without_evidence {
                paths[v] = super::writes::ReturnPaths::unknown();
                for &w in &dependents[v] {
                    if members.contains(&w) && queued.insert(w) {
                        queue.push_back(w);
                    }
                }
            }
        }
    }
    for (((summary, inclusions), ret), publications) in
        summaries.iter_mut().zip(writes).zip(paths).zip(globals)
    {
        summary.write_inclusions = inclusions;
        summary.return_paths = ret;
        summary.publication_inclusions = publications;
    }
    Ok(bodies
        .into_iter()
        .zip(summaries)
        .map(|(f, summary)| (f.symbol.clone(), (f, summary)))
        .collect())
}

pub(super) fn check_globals(m: &Module, f: &Function) -> Result<(), String> {
    for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let symbol = match i {
            Instruction::StoreGlobal {
                symbol,
                ty: Type::Ref,
                ..
            }
            | Instruction::ArcStoreStrong {
                slot: StrongSlot::Global { symbol },
                ..
            }
            | Instruction::ArcLoadStrong {
                slot: StrongSlot::Global { symbol },
            } => Some(symbol),
            _ => None,
        };
        if let Some(symbol) = symbol {
            let declarations: Vec<_> = m.globals.iter().filter(|(_, _, s)| s == symbol).collect();
            if declarations.len() != 1 || declarations[0].1 != Type::Ref {
                return Err(format!(
                    "ARC publications: global @{symbol} requires a single Ref declaration"
                ));
            }
        }
    }
    Ok(())
}

fn targets(f: &Function) -> impl Iterator<Item = &str> {
    f.blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, i, _)| {
            if let Instruction::CallStatic { symbol, .. } = i {
                Some(symbol.as_str())
            } else {
                None
            }
        })
}

fn join_set(dest: &mut ArcHeapReturn, origin: &ArcHeapReturn, limit: usize) -> bool {
    if *dest == ArcHeapReturn::UnknownValue {
        return false;
    }
    let previous = dest.clone();
    match origin {
        ArcHeapReturn::UnknownValue => *dest = ArcHeapReturn::UnknownValue,
        ArcHeapReturn::Aliases {
            parameters: ps,
            null: ns,
        } => {
            if let ArcHeapReturn::Aliases { parameters, null } = dest {
                parameters.extend(ps);
                *null |= ns;
                if parameters.len() > limit {
                    *dest = ArcHeapReturn::UnknownValue;
                }
            }
        }
    }
    *dest != previous
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(name: &str, target: Option<&str>) -> Function {
        Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::I64)],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: target
                    .into_iter()
                    .map(|symbol| {
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: symbol.into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return(Some(Operand::Val(ValueId(if target.is_some() {
                    1
                } else {
                    0
                })))),
            }],
        }
    }

    #[test]
    fn composite_coverage_deep_chain_and_body_order() {
        let mut m = Module::new();
        for v in 0..1000 {
            let target = (v < 999).then(|| format!("f{}", v + 1));
            m.functions.push(scalar(&format!("f{v}"), target.as_deref()));
        }
        for _ in 0..2 {
            let r = solve(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for (_, summary) in r.values() {
                assert_eq!(
                    summary.fields(),
                    super::super::summaries::ArcHeapFields::Preserved
                );
                assert_eq!(
                    summary.publications(),
                    super::super::summaries::ArcHeapPublications::ResultOnly
                );
            }
            m.functions.reverse();
        }
    }

    #[test]
    fn composite_coverage_rejects_opaque_dependency_recursion_and_abi() {
        use super::super::summaries::*;
        let base = vec![
            scalar("f", Some("g")),
            scalar("g", Some("h")),
            scalar("h", None),
        ];
        for case in 0..4 {
            let mut m = Module::new();
            m.functions = base.clone();
            match case {
                0 => m.functions[2] = scalar("h", Some("externo")),
                1 => m.functions[2] = scalar("h", Some("g")),
                2 => {
                    m.functions[2].blocks[0].terminator =
                        Terminator::Throw(Operand::Constant(Constant::Null))
                }
                3 => {
                    if let Instruction::CallStatic { args, .. } =
                        &mut m.functions[1].blocks[0].instructions[0].1
                    {
                        args[0] = Operand::Constant(Constant::Null);
                    }
                }
                _ => unreachable!(),
            }
            let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(
                r["f"].1.fields(),
                ArcHeapFields::UnknownSet,
                "caso {case}"
            );
            assert_eq!(
                r["f"].1.publications(),
                ArcHeapPublications::UnknownValues,
                "caso {case}"
            );
        }
    }

    #[test]
    fn composite_coverage_does_not_reapprove_missing_context_or_table() {
        use super::super::summaries::*;
        let mut m = Module::new();
        m.functions = vec![scalar("f", Some("g")), scalar("g", None)];
        m.table_exceptions = true;
        for tables in [
            vec![],
            vec![
                FunctionTables::default(),
                FunctionTables {
                    checks_stack: true,
                    ..Default::default()
                },
            ],
        ] {
            m.tables = tables;
            let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(r["f"].1.fields(), ArcHeapFields::UnknownSet);
            assert_eq!(r["g"].1.publications(), ArcHeapPublications::UnknownValues);
        }
        m.tables = vec![FunctionTables::default(); 2];
        let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.fields(), ArcHeapFields::Preserved);
        // Changing the prologue itself prevents propagation even with g covered.
        m.tables[0].checks_stack = true;
        let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.fields(), ArcHeapFields::UnknownSet);
        assert_eq!(r["g"].1.fields(), ArcHeapFields::Preserved);
    }

    #[test]
    fn composite_coverage_consumed_in_module_invalidates_transitive_dependency() {
        use super::super::{escape::*, module::analyze_in_module};
        let mut m = Module::new();
        m.functions = vec![
            scalar("f", Some("g")),
            scalar("g", Some("h")),
            scalar("h", None),
        ];
        assert!(analyze_in_module(&m, "f", 8).unwrap().is_none());
        m.arc_memory = true;
        let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(a.unknown_set().is_empty());
        let e = compute_in_module(&m, "f", &a).unwrap();
        assert!(
            !e.publications
                .keys()
                .any(|c| matches!(c, EscapeCause::OpaqueOperation(_)))
        );
        // Implicit materialization effect in h, despite the scalar return:
        // invalidates the old solution of f and makes the new call opaque.
        m.functions[2].blocks[0].instructions.push((
            ValueId(2),
            Instruction::Const(Constant::String("novo efeito".into())),
            Type::Ref,
        ));
        assert!(compute_in_module(&m, "f", &a).is_err());
        let new_one = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert_eq!(new_one.unknown_set(), &[ValueId(1)]);
        assert!(
            compute_in_module(&m, "f", &new_one)
                .unwrap()
                .publications
                .contains_key(&EscapeCause::OpaqueOperation(1))
        );
    }

    #[test]
    fn chain_with_computation_preserves_fields_and_rejects_conversion_or_fallible_effect() {
        use super::super::{escape::*, module::analyze_in_module, summaries::*};
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.arc_field_layouts.insert(2, vec![]);
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
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
                            symbol: "f0".into(),
                            args: vec![Operand::Constant(Constant::Int(4))],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(1)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        for v in 0..64 {
            let target = (v < 63).then(|| format!("f{}", v + 1));
            m.functions.push(scalar(&format!("f{v}"), target.as_deref()));
        }
        m.functions[64].blocks[0].instructions.push((
            ValueId(1),
            Instruction::Add(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(1)),
            ),
            Type::I64,
        ));
        m.functions[64].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        for _ in 0..2 {
            let r = solve(&m, &HashSet::from(["f0"]), 8).unwrap();
            for (_, summary) in r.values() {
                assert_eq!(summary.fields(), ArcHeapFields::Preserved);
                assert_eq!(summary.publications(), ArcHeapPublications::ResultOnly);
                assert_eq!(summary.ret(), &ArcHeapReturn::UnknownValue);
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(a.value(ValueId(0)), a.value(ValueId(3)));
            assert_eq!(a.value(ValueId(3)).unwrap().nodes().unwrap().len(), 1);
            assert!(a.unknown_set().is_empty());
            // An I64 result gets no proof of trivial representation/freshness.
            assert!(a.value(ValueId(2)).unwrap().nodes().is_none());
            assert!(
                !compute_in_module(&m, "caller", &a)
                    .unwrap()
                    .publications
                    .contains_key(&EscapeCause::OpaqueOperation(2))
            );
            m.functions.reverse();
            compute_in_module(&m, "caller", &a).unwrap();
        }
        let previous = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        for i in [
            Instruction::SDiv(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
            Instruction::SRem(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
        ] {
            m.functions[64].blocks[0].instructions[0].1 = i;
            assert!(compute_in_module(&m, "caller", &previous).is_err());
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
            assert_eq!(a.unknown_set(), &[ValueId(2)]);
            assert!(
                compute_in_module(&m, "caller", &a)
                    .unwrap()
                    .publications
                    .contains_key(&EscapeCause::OpaqueOperation(2))
            );
        }
        m.functions[64].blocks[0].instructions[0].1 = Instruction::Add(
            Operand::Val(ValueId(0)),
            Operand::Constant(Constant::Int(1)),
        );
        for arg in [
            Constant::Double(4.0),
            Constant::Null,
            Constant::String("4".into()),
        ] {
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[2].1
            {
                args[0] = Operand::Constant(arg);
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
            assert_eq!(a.unknown_set(), &[ValueId(2)]);
        }
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    fn body(name: &str, base: Option<Operand>, targets: &[String]) -> Function {
        let mut options: Vec<_> = base.into_iter().map(|o| (vec![], o)).collect();
        for (k, s) in targets.iter().enumerate() {
            let v = ValueId(3 + k as u32);
            options.push((
                vec![(
                    v,
                    Instruction::CallStatic {
                        symbol: s.clone(),
                        args: vec![
                            Operand::Val(ValueId(0)),
                            Operand::Val(ValueId(1)),
                            Operand::Val(ValueId(2)),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Operand::Val(v),
            ));
        }
        let mut blocks = Vec::new();
        let n = options.len();
        for (k, (instructions, ret)) in options.into_iter().enumerate() {
            let id = BlockId(2 * k as u32);
            let last_one = k + 1 == n;
            if !last_one {
                blocks.push(BasicBlock {
                    id,
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(2)),
                        then_block: BlockId(id.0 + 1),
                        else_block: BlockId(id.0 + 2),
                    },
                });
            }
            blocks.push(BasicBlock {
                id: if last_one { id } else { BlockId(id.0 + 1) },
                instructions,
                terminator: Terminator::Return(Some(ret)),
            });
        }
        if blocks.is_empty() {
            blocks.push(BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(0)),
            });
        }
        Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "x".into(), Type::Ref),
                (ValueId(1), "y".into(), Type::Ref),
                (ValueId(2), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks,
        }
    }

    #[test]
    fn scc_matches_aliases_with_independent_reach_in_512_graphs() {
        for mask in 0..512 {
            let mut m = Module::new();
            let mut reachability = [[false; 3]; 3];
            for (v, line) in reachability.iter_mut().enumerate() {
                line[v] = true;
                let targets: Vec<_> = (0..3)
                    .filter(|&w| mask & (1 << (3 * v + w)) != 0)
                    .map(|w| {
                        line[w] = true;
                        format!("f{w}")
                    })
                    .collect();
                m.functions.push(body(
                    &format!("f{v}"),
                    Some(Operand::Val(ValueId((v % 2) as u32))),
                    &targets,
                ));
            }
            // Floyd-Warshall over the original graph, without using the SCC/worklist.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            for limit in 0..=2 {
                let r = solve(&m, &HashSet::from(["f0", "f1", "f2"]), limit).unwrap();
                for (v, line) in reachability.iter().enumerate() {
                    let parameters: BTreeSet<_> =
                        (0..3).filter(|&w| line[w]).map(|w| w % 2).collect();
                    let expected = if parameters.len() > limit {
                        ArcHeapReturn::UnknownValue
                    } else {
                        ArcHeapReturn::Aliases {
                            parameters,
                            null: false,
                        }
                    };
                    assert_eq!(
                        r[&format!("f{v}")].1.ret(),
                        &expected,
                        "graph {mask}, limit {limit}, node {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn permuted_arguments_in_recursion_join_real_aliases() {
        let mut m = Module::new();
        m.functions.push(body("f", None, &["g".into()]));
        let mut g = body("g", Some(Operand::Val(ValueId(0))), &["f".into()]);
        let call = g
            .blocks
            .last_mut()
            .unwrap()
            .instructions
            .last_mut()
            .unwrap();
        if let Instruction::CallStatic { args, .. } = &mut call.1 {
            args.swap(0, 1);
        }
        m.functions.push(g);
        let r = solve(&m, &HashSet::from(["f"]), 2).unwrap();
        assert!(r.values().all(|(_, r)| r.ret()
            == &ArcHeapReturn::Aliases {
                parameters: BTreeSet::from([0, 1]),
                null: false,
            }));
        let r = solve(&m, &HashSet::from(["f"]), 1).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
    }

    #[test]
    fn baseless_null_recursion_and_external_target_do_not_forge_empty_facts() {
        let mut m = Module::new();
        m.functions.push(body("f", None, &["g".into()]));
        m.functions.push(body(
            "g",
            Some(Operand::Constant(Constant::Null)),
            &["f".into()],
        ));
        let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
        for (_, summary) in r.values() {
            assert_eq!(
                summary.ret(),
                &ArcHeapReturn::Aliases {
                    parameters: BTreeSet::new(),
                    null: true
                }
            );
        }
        m.functions[1] = body("g", None, &["f".into()]);
        let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
        m.functions[1] = body("g", Some(Operand::Val(ValueId(0))), &["externo".into()]);
        let r = solve(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
        assert!(!r.contains_key("externo"));
    }

    #[test]
    fn deep_chain_independent_of_body_order_without_rust_recursion() {
        let mut m = Module::new();
        for v in 0..2000 {
            m.functions.push(if v == 1999 {
                body(&format!("f{v}"), Some(Operand::Val(ValueId(1))), &[])
            } else {
                body(&format!("f{v}"), None, &[format!("f{}", v + 1)])
            });
        }
        for _ in 0..2 {
            let r = solve(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 2000);
            assert!(r.values().all(|(_, r)| r.ret()
                == &ArcHeapReturn::Aliases {
                    parameters: BTreeSet::from([1]),
                    null: false
                }));
            m.functions.reverse();
        }
    }
}
