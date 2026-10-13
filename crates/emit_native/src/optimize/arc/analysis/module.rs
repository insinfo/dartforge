//! Binds lowering origins/layouts to the local extraction, without selecting policies.
//! Accesses select the nominal field through the schema of each possible receiver.

use super::{hir::*, model::*, points_to::*};
use crate::hir::*;
use std::collections::HashMap;

/// Analyzes an ARC body using origins and layouts already recorded in the module.
/// Tracing returns None without running the analysis. A missing origin/layout keeps
/// lack of knowledge; never creates a site from the current SSA ID. Fields of a
/// receiver select the field of its own layout, including unions of classes
/// and chained reads, in the same fixpoint as the aliases. A missing schema/index
/// and an opaque or empty receiver keep an unknown boundary.
/// Copies/Phis preserve the union; a parameter and a call without a summary do not prove a layout.
/// Layout keys are local to the module version, with no validity across reloads.
/// CallStatic uses the normal aliases of local bodies up to a fixpoint per SCC.
/// A leaf with field coverage and an exact ABI preserves the precision of the fields;
/// publication may be limited to the result by its own coverage; other
/// escapes/retentions stay opaque. Guards transitive bodies and consulted emission
/// tables, including the implicit stack check.
/// An explicit context and table paths without a summary keep an opaque emission
/// boundary, separate from the instruction IDs. Frames by liveness
/// also keep that boundary before reductions by maps. Instrumentation
/// and other emitter options still require their own coverage.
///
/// # Errors
/// Missing/duplicated symbol, stale site fact, an origin with conflicting
/// classes, an incompatible initializer or a failure of the extraction/SSA.
/// The module is not changed; the result is not a policy certificate.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::module::analyze_in_module};
/// let m = Module::new();
/// assert!(analyze_in_module(&m, "absent", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
pub fn analyze_in_module(
    m: &Module,
    sym_name: &str,
    limit: usize,
) -> Result<Option<HirAnalysis>, String> {
    if !m.arc_memory {
        return Ok(None);
    }
    let mut bodies = m.functions.iter().filter(|f| f.symbol == sym_name);
    let f = bodies
        .next()
        .ok_or_else(|| format!("points-to: missing body {sym_name}"))?;
    if bodies.next().is_some() {
        return Err(format!("points-to: duplicate body {sym_name}"));
    }
    super::calls::check_globals(m, f)?;
    let calls: std::collections::HashSet<_> = f
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
    let summaries = super::calls::solve(m, &calls, limit)?;
    let facts = facts_in_module(m, f)?;
    // The selection follows aliases and fields in the same solution. Do not refine
    // the top of a first pass nor assume a single layout for the Phi.
    let mut analysis = analyze_with_summaries(f, &facts, limit, &summaries, true)?;
    analysis.module_premises = Some(premises_signature(m, sym_name));
    // A conservative guard of the whole consulted reach, including transitive
    // bodies. It is neither a persistent cache nor a JIT generation dependency.
    let positions: HashMap<_, _> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    // The prologue/cleanup of the caller itself also depends on these tables,
    // even when the body has no local direct call.
    analysis
        .table_dependencies
        .insert(sym_name.to_string(), table_state(m, positions[sym_name]));
    let table = if m.excecoes_por_tabelas {
        m.tables.get(positions[sym_name])
    } else {
        None
    };
    // A context implies CheckStackOverflow in the prologue, even if the whole explicit
    // HIR has coverage. Landing pads/exits/cleanup also insert paths
    // without a heap contract here. Do not invent a ValueId for these effects.
    analysis.opaque_implicit_emission = (m.excecoes_por_tabelas && table.is_none())
        || crate::llvm::LlvmEmitter::requires_explicit_context(f, table)
        || crate::llvm::LlvmEmitter::may_require_context_for_roots(f)
        || table.is_some_and(|t| {
            !t.invocations.is_empty()
                || !t.landing_pads.is_empty()
                || !t.exits.is_empty()
                || t.foreign_cleanup.is_some()
        });
    for (s, (body, _)) in &summaries {
        analysis
            .body_dependencies
            .insert(s.clone(), body_signature(body));
        let index = positions[s.as_str()];
        analysis
            .table_dependencies
            .insert(s.clone(), table_state(m, index));
    }
    Ok(Some(analysis))
}

// A single validation of origins/layouts for the local body and for the
// objects mentioned by the callee summaries. Does not forge an origin from SSA.
pub(super) fn facts_in_module(m: &Module, f: &Function) -> Result<HirFacts, String> {
    let instructions: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let constants = super::hir::integer_constants(f);
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, t)| (*v, *t)),
        )
        .collect();
    let mut facts = HirFacts::default();
    let mut classes = HashMap::new();
    if let Some(sites) = m.arc_sites.get(&f.symbol) {
        for (v, site) in sites {
            let i = instructions
                .get(v)
                .ok_or_else(|| format!("points-to: stale site v{}", v.0))?;
            if !super::origins::allocation(i) {
                return Err(format!("points-to: site does not allocate v{}", v.0));
            }
            if let Some((role, captures)) = super::hir::synthetic_captures(i, &types) {
                if types[v] != Type::Ref {
                    return Err(format!("points-to: non-Ref synthetic unit v{}", v.0));
                }
                let node = AbstractNode::SyntheticUnit {
                    site: site.clone(),
                    context: String::new(),
                    role: role.into(),
                };
                let field_list = (0..captures.len())
                    .map(|position| ArcField {
                        layout: format!("{role}:{site:?}"),
                        position: position as u32,
                    })
                    .collect();
                facts.objects.insert(*v, ObjectOrigin { node, fields: field_list });
                continue;
            }
            let (class, quantity) = match i {
                Instruction::AllocObject { class_id, fields } => (*class_id, fields.len()),
                _ => match super::hir::zeroed_factory(i, &constants) {
                    Some(c) => c,
                    None => continue,
                },
            };
            let Some(layout) = m.arc_field_layouts.get(&class) else {
                continue;
            };
            if layout.len() != quantity {
                return Err(format!("points-to: incompatible layout v{}", v.0));
            }
            let node = AbstractNode::Allocation {
                site: site.clone(),
                context: String::new(),
            };
            if classes
                .insert(node.clone(), class)
                .is_some_and(|previous| previous != class)
            {
                return Err("points-to: origin with conflicting classes".into());
            }
            let field_list = (0..quantity)
                .map(|position| ArcField {
                    layout: format!("classe:{class}"),
                    position: position as u32,
                })
                .collect();
            facts.objects.insert(*v, ObjectOrigin { node, fields: field_list });
        }
    }
    Ok(facts)
}

// A local guard of the facts read; nominal ordering avoids depending on the
// insertion order/capacity of the HashMaps. Unused layouts also invalidate,
// conservatively, until we have fine-grained/versioned schema dependencies.
pub(super) fn premises_signature(m: &Module, sym_name: &str) -> blake3::Hash {
    let layouts: std::collections::BTreeMap<_, _> = m.arc_field_layouts.iter().collect();
    // The identity of an allocated result may come from another body. A nominal
    // guard of the module before consuming those origins across calls;
    // changing only the provenance must also invalidate the previous solution.
    let sites: std::collections::BTreeMap<_, _> = m
        .arc_sites
        .iter()
        .map(|(function, origins)| {
            let origins: std::collections::BTreeMap<_, _> =
                origins.iter().map(|(v, site)| (v.0, site)).collect();
            (function, origins)
        })
        .collect();
    // Keep duplicates: a repeated declaration is no longer a valid ABI.
    let mut globals: Vec<_> = m
        .globals
        .iter()
        .map(|(slot, ty, name)| (name, slot, format!("{ty:?}")))
        .collect();
    globals.sort();
    blake3::hash(format!("{:?}", (m.arc_memory, sym_name, layouts, sites, globals)).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_and_local_capture_preserve_known_cycle_despite_opaque_fields() {
        use super::super::graph::build_local_in_module;
        use std::collections::BTreeSet;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        m.functions.push(Function {
            symbol: "anexo_local".into(),
            name: "anexo_local".into(),
            debug_info: None,
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
                        Instruction::AllocClosureTyped {
                            code_symbol: "entrada".into(),
                            env: Operand::Val(ValueId(0)),
                            typed: "acao".into(),
                            abi: 0,
                            direct: true,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallRuntime {
                            name: "dartforge_native_DartForge_finalizer_attach".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Null), Type::Ref),
                                (Operand::Val(ValueId(1)), Type::Ref),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "anexo_local", 8).unwrap().unwrap();
        let owner = a
            .value(ValueId(0))
            .unwrap()
            .nodes()
            .unwrap()
            .first()
            .unwrap()
            .clone();
        let action = a
            .value(ValueId(1))
            .unwrap()
            .nodes()
            .unwrap()
            .first()
            .unwrap()
            .clone();
        let g = build_local_in_module(&m, "anexo_local", 8)
            .unwrap()
            .unwrap();
        assert_eq!(g.edges()[&owner], BTreeSet::from([action.clone()]));
        assert_eq!(g.edges()[&action], BTreeSet::from([owner.clone()]));
        assert!(g.components().any(|(nodes, cycle)| cycle
            && nodes.iter().cloned().collect::<BTreeSet<_>>()
                == BTreeSet::from([owner.clone(), action.clone()])));
        assert!(g.opaque());
        g.check_in_module(&m, "anexo_local").unwrap();
        // A null capture does not close the cycle, nor covers the effects of the attachment.
        if let Instruction::AllocClosureTyped { env, .. } =
            &mut m.functions[0].blocks[0].instructions[1].1
        {
            *env = Operand::Constant(Constant::Null);
        }
        assert!(g.check_in_module(&m, "anexo_local").is_err());
        let g = build_local_in_module(&m, "anexo_local", 8)
            .unwrap()
            .unwrap();
        assert!(g.edges()[&action].is_empty());
        assert!(!g.components().any(|(_, cycle)| cycle));
        assert!(g.opaque());
    }

    #[test]
    fn finalizer_attachment_composes_side_edge_without_inventing_physical_field() {
        use super::super::{writes::WriteInclusions, graph::build_local_in_module};
        use std::collections::{BTreeSet, HashSet};
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        let mut attach = Function {
            symbol: "anexar".into(),
            name: "anexar".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "dono".into(), Type::Ref),
                (ValueId(1), "acao".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_native_DartForge_finalizer_attach".into(),
                        args: vec![
                            (Operand::Val(ValueId(0)), Type::Ref),
                            (Operand::Constant(Constant::Null), Type::Ref),
                            (Operand::Val(ValueId(1)), Type::Ref),
                            (Operand::Constant(Constant::Null), Type::Ref),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let mut wrapper = attach.clone();
        wrapper.symbol = "wrapper".into();
        wrapper.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "anexar".into(),
            args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
            ret_ty: Type::Void,
        };
        let mut caller = wrapper.clone();
        caller.symbol = "caller".into();
        caller.params.clear();
        caller.return_ty = Type::Ref;
        caller.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        caller.blocks[0].instructions = vec![
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
                Instruction::AllocObject {
                    class_id: 1,
                    fields: vec![],
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::CallStatic {
                    symbol: "wrapper".into(),
                    args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        m.functions = vec![caller, wrapper, attach.clone()];
        super::super::origins::register(&mut m, 0, true);
        let node = |id| AbstractNode::Allocation {
            site: m.arc_sites["caller"][&ValueId(id)].clone(),
            context: String::new(),
        };
        let owner = node(0);
        let action = node(1);
        for reverse in [false, true] {
            if reverse {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            let WriteInclusions::Partial(edges) = &r["wrapper"].1.write_inclusions else {
                panic!("attachment cannot certify complete coverage");
            };
            assert_eq!(edges.len(), 1);
            assert_eq!(edges.first().unwrap().index, None);
            let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(g.edges()[&owner], BTreeSet::from([action.clone()]));
            assert!(g.opaque());
            g.check_in_module(&m, "caller").unwrap();
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(a.schemas[&owner].is_empty());
            assert!(a.schemas[&action].is_empty());
            let e = super::super::escape::compute_in_module(&m, "caller", &a).unwrap();
            assert_eq!(
                e.publications[&super::super::escape::EscapeCause::Return].nodes(),
                Some(&BTreeSet::from([owner.clone(), action.clone()]))
            );
            let a = analyze_in_module(&m, "caller", 1).unwrap().unwrap();
            let e = super::super::escape::compute_in_module(&m, "caller", &a).unwrap();
            assert!(
                e.publications[&super::super::escape::EscapeCause::Return]
                    .nodes()
                    .is_none()
            );
        }
        let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut attach.blocks[0].instructions[0].1 {
            args[2].1 = Type::I64;
        }
        *m.functions
            .iter_mut()
            .find(|f| f.symbol == "anexar")
            .unwrap() = attach;
        assert!(g.check_in_module(&m, "caller").is_err());
        let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(g.edges()[&owner].is_empty());
        assert!(g.opaque());
        m.arc_memory = false;
        assert!(
            build_local_in_module(&m, "caller", 8)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn env_resolves_cell_and_weak_write_without_confusing_scalar_capture() {
        use std::collections::BTreeSet;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        m.functions.push(Function {
            symbol: "captura".into(),
            name: "captura".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
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
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::AllocCell {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::AllocEnv {
                            values: vec![
                                Operand::Constant(Constant::Int(41)),
                                Operand::Val(ValueId(2)),
                            ],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::EnvGet {
                            env: Operand::Val(ValueId(3)),
                            index: 1,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::CellSet {
                            cell: Operand::Val(ValueId(4)),
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(6),
                        Instruction::CellGet {
                            cell: Operand::Val(ValueId(4)),
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(6)))),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "captura", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(4)), a.value(ValueId(2)));
        let mut expected_set = a.value(ValueId(0)).unwrap().nodes().unwrap().clone();
        expected_set.extend(a.value(ValueId(1)).unwrap().nodes().unwrap().iter().cloned());
        assert_eq!(a.value(ValueId(6)).unwrap().nodes(), Some(&expected_set));
        assert!(a.unknown_set().is_empty());
        let g = super::super::graph::build_local_in_module(&m, "captura", 8)
            .unwrap()
            .unwrap();
        let cell = a.value(ValueId(2)).unwrap().nodes().unwrap().first().unwrap();
        assert_eq!(g.edges()[cell], expected_set);
        g.check_in_module(&m, "captura").unwrap();
        // Neither the scalar bit nor a missing index become a precise null reference.
        for index in [0, 2] {
            if let Instruction::EnvGet { index: alvo, .. } =
                &mut m.functions[0].blocks[0].instructions[4].1
            {
                *alvo = index;
            }
            let a = analyze_in_module(&m, "captura", 8).unwrap().unwrap();
            assert!(a.value(ValueId(4)).unwrap().nodes().is_none());
            assert!(a.value(ValueId(6)).unwrap().nodes().is_none());
        }
        if let Instruction::EnvGet { index, .. } = &mut m.functions[0].blocks[0].instructions[4].1 {
            *index = 1;
        }
        if let Instruction::CellSet { value, .. } = &mut m.functions[0].blocks[0].instructions[5].1
        {
            *value = Operand::Constant(Constant::Null);
        }
        let a = analyze_in_module(&m, "captura", 8).unwrap().unwrap();
        assert_eq!(
            a.value(ValueId(6)).unwrap().nodes(),
            Some(&BTreeSet::from([AbstractNode::Allocation {
                site: m.arc_sites["captura"][&ValueId(0)].clone(),
                context: String::new()
            }]))
        );
    }

    #[test]
    fn composite_synthetic_captures_follow_physical_slots_and_empty_env_is_null() {
        use super::super::graph::build_local_in_module;
        use std::collections::BTreeSet;
        for variant in 0..3 {
            let mut m = Module::new();
            m.arc_memory = true;
            m.arc_field_layouts.insert(1, vec![]);
            let forward = |name: &str, alvo: &str| Function {
                symbol: name.into(),
                name: name.into(),
                debug_info: None,
                params: vec![(ValueId(0), "captura".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: alvo.into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            };
            let mut create = forward("criar", "externo");
            let env = Operand::Val(ValueId(if variant == 2 { 1 } else { 2 }));
            create.blocks[0].instructions = vec![
                (
                    ValueId(1),
                    Instruction::AllocCell {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::AllocEnv {
                        values: vec![
                            Operand::Constant(Constant::Int(41)),
                            Operand::Val(ValueId(1)),
                        ],
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    if variant == 0 {
                        Instruction::AllocClosure {
                            code_symbol: "entrada".into(),
                            env,
                        }
                    } else {
                        Instruction::AllocClosureTyped {
                            code_symbol: "entrada".into(),
                            env,
                            typed: "corpo".into(),
                            abi: 7,
                            direct: variant == 2,
                        }
                    },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::AllocEnv { values: vec![] },
                    Type::Ref,
                ),
            ];
            create.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
            let mut caller = forward("caller", "wrapper");
            caller.params.clear();
            caller.blocks[0].instructions.insert(
                0,
                (
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 1,
                        fields: vec![],
                    },
                    Type::Ref,
                ),
            );
            m.functions = vec![caller, forward("wrapper", "criar"), create];
            super::super::origins::register(&mut m, 0, true);
            let unit = |id, role: &str| AbstractNode::SyntheticUnit {
                site: m.arc_sites["criar"][&ValueId(id)].clone(),
                context: String::new(),
                role: role.into(),
            };
            let cell = unit(1, "celula:v1");
            let environment = unit(2, "ambiente:v1");
            let closure = unit(3, "closure:v1");
            let child = AbstractNode::Allocation {
                site: m.arc_sites["caller"][&ValueId(0)].clone(),
                context: String::new(),
            };
            for reverse in [false, true] {
                if reverse {
                    m.functions.reverse();
                }
                let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
                g.check_in_module(&m, "caller").unwrap();
                assert_eq!(g.edges().len(), 4);
                assert_eq!(g.edges()[&cell], BTreeSet::from([child.clone()]));
                assert_eq!(g.edges()[&environment], BTreeSet::from([cell.clone()]));
                assert_eq!(
                    g.edges()[&closure],
                    BTreeSet::from([if variant == 2 {
                        cell.clone()
                    } else {
                        environment.clone()
                    }])
                );
                assert!(g.opaque());
                let a = analyze_in_module(&m, "criar", 8).unwrap().unwrap();
                assert!(a.value(ValueId(4)).unwrap().nodes().unwrap().is_empty());
                assert!(!a.schemas.keys().any(|n| matches!(n,
                    AbstractNode::SyntheticUnit { site, .. }
                        if site == &m.arc_sites["criar"][&ValueId(4)])));
            }
            let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
            let create = m
                .functions
                .iter_mut()
                .find(|f| f.symbol == "criar")
                .unwrap();
            if let Instruction::AllocCell { value } = &mut create.blocks[0].instructions[0].1 {
                *value = Operand::Constant(Constant::Int(41));
            }
            assert!(g.check_in_module(&m, "caller").is_err());
            let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(g.edges()[&cell].is_empty());
            m.arc_memory = false;
            assert!(
                build_local_in_module(&m, "caller", 8)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn wrapper_initializer_preserves_edge_without_closing_external_effects() {
        use super::super::graph::build_local_in_module;
        use std::collections::BTreeSet;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.arc_field_layouts.insert(2, vec![]);
        let forward = |name: &str, alvo: &str| Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![(ValueId(0), "filho".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::CallStatic {
                        symbol: alvo.into(),
                        args: vec![Operand::Val(ValueId(0))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let mut create = forward("criar", "externo");
        create.blocks[0].instructions = vec![
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
                    symbol: "externo".into(),
                    args: vec![Operand::Val(ValueId(1))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        let mut caller = forward("caller", "wrapper");
        caller.params.clear();
        caller.blocks[0].instructions.insert(
            0,
            (
                ValueId(0),
                Instruction::AllocObject {
                    class_id: 2,
                    fields: vec![],
                },
                Type::Ref,
            ),
        );
        m.functions = vec![caller, forward("wrapper", "criar"), create];
        super::super::origins::register(&mut m, 0, true);
        let parent = AbstractNode::Allocation {
            site: m.arc_sites["criar"][&ValueId(1)].clone(),
            context: String::new(),
        };
        let child = AbstractNode::Allocation {
            site: m.arc_sites["caller"][&ValueId(0)].clone(),
            context: String::new(),
        };
        for reverse in [false, true] {
            if reverse {
                m.functions.reverse();
            }
            let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
            g.check_in_module(&m, "caller").unwrap();
            assert_eq!(g.edges()[&parent], BTreeSet::from([child.clone()]));
            assert!(g.opaque());
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(
                a.value(ValueId(1)).unwrap().nodes(),
                Some(&BTreeSet::from([parent.clone()]))
            );
            // The known edge does not make the imported field completely known.
            assert!(
                a.points()
                    .field(
                        &parent,
                        &ArcField {
                            layout: "classe:1".into(),
                            position: 0
                        }
                    )
                    .is_none_or(|p| p.nodes().is_none())
            );
        }
        let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
        let create = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "criar")
            .unwrap();
        if let Instruction::AllocObject { fields, .. } = &mut create.blocks[0].instructions[0].1 {
            fields[0] = Operand::Constant(Constant::Null);
        }
        assert!(g.check_in_module(&m, "caller").is_err());
        let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(g.edges()[&parent].is_empty());
        assert!(g.opaque());
        m.arc_memory = false;
        assert!(
            build_local_in_module(&m, "caller", 8)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn callee_origin_invalidates_escape_independent_of_map_order() {
        use super::super::escape::compute_in_module;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        m.functions = vec![
            Function {
                symbol: "caller".into(),
                name: "caller".into(),
                debug_info: None,
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
                debug_info: None,
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
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(
            a.value(ValueId(0)).unwrap().nodes(),
            Some(&std::collections::BTreeSet::from([AbstractNode::Allocation {
                site: m.arc_sites["criar"][&ValueId(37)].clone(),
                context: String::new(),
            }]))
        );
        compute_in_module(&m, "caller", &a).unwrap();
        // Rebuilding tables with another order/capacity does not change premises.
        let mut origins: Vec<_> = m.arc_sites.drain().collect();
        origins.reverse();
        m.arc_sites = origins.into_iter().collect();
        compute_in_module(&m, "caller", &a).unwrap();
        let body = body_signature(&m.functions[1]);
        m.arc_sites
            .get_mut("criar")
            .unwrap()
            .get_mut(&ValueId(37))
            .unwrap()
            .origin += 1;
        assert_eq!(body_signature(&m.functions[1]), body);
        assert!(compute_in_module(&m, "caller", &a).is_err());
        let new_one = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        compute_in_module(&m, "caller", &new_one).unwrap();
        m.arc_sites.remove("criar");
        assert!(compute_in_module(&m, "caller", &new_one).is_err());
    }

    #[test]
    fn zeroed_factory_publishes_allocated_origin_in_global_and_exception_by_wrapper() {
        use super::super::{escape::*, graph::build_local_in_module};
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        m.globals.push((0, Type::Ref, "g".into()));
        let wrapper = |name: &str, alvo: &str| Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
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
                debug_info: None,
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
                                symbol: "g".into(),
                                val: Operand::Val(ValueId(37)),
                                ty: Type::Ref,
                                root: Some(0),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Throw(Operand::Val(ValueId(37))),
                }],
            },
        ];
        super::super::origins::register(&mut m, 0, true);
        let expected = std::collections::BTreeSet::from([AbstractNode::Allocation {
            site: m.arc_sites["criar"][&ValueId(37)].clone(),
            context: String::new(),
        }]);
        for reverse in [false, true] {
            if reverse {
                m.functions.reverse();
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            let e = compute_in_module(&m, "caller", &a).unwrap();
            assert_eq!(
                e.publications[&EscapeCause::Global("g".into())].nodes(),
                Some(&expected)
            );
            assert_eq!(e.publications[&EscapeCause::Exception].nodes(), Some(&expected));
            assert!(
                e.publications[&EscapeCause::OpaqueOperation(0)]
                    .nodes()
                    .is_none()
            );
            let g = build_local_in_module(&m, "caller", 8).unwrap().unwrap();
            g.check_in_module(&m, "caller").unwrap();
            assert_eq!(
                g.edges()
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
                expected
            );
            assert!(g.opaque());
        }
        let create = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "criar")
            .unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut create.blocks[0].instructions[0].1 {
            args[0].1 = Type::Ref;
        }
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        let e = compute_in_module(&m, "caller", &a).unwrap();
        assert!(e.publications[&EscapeCause::Exception].nodes().is_none());
    }

    #[test]
    fn allocated_return_composes_null_parameter_recursion_and_opaque_imported_fields() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.arc_field_layouts.insert(2, vec![]);
        let create = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
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
        let call = |x| Instruction::CallStatic {
            symbol: "criar".into(),
            args: vec![x, Operand::Constant(Constant::Bool(true))],
            ret_ty: Type::Ref,
        };
        let caller = Function {
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
                    (ValueId(1), call(Operand::Val(ValueId(0))), Type::Ref),
                    (
                        ValueId(2),
                        call(Operand::Constant(Constant::Null)),
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
        m.functions = vec![caller, create];
        super::super::origins::register(&mut m, 0, true);
        let node = |function: &str, id| AbstractNode::Allocation {
            site: m.arc_sites[function][&ValueId(id)].clone(),
            context: String::new(),
        };
        let allocated = node("criar", 37);
        let parameter = node("caller", 0);
        for reverse in [false, true] {
            if reverse {
                m.functions.reverse();
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(
                a.value(ValueId(1)).unwrap().nodes(),
                Some(&std::collections::BTreeSet::from([
                    parameter.clone(),
                    allocated.clone()
                ]))
            );
            assert_eq!(
                a.value(ValueId(2)).unwrap().nodes(),
                Some(&std::collections::BTreeSet::from([allocated.clone()]))
            );
            assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
            assert!(
                compute_in_module(&m, "caller", &a).unwrap().publications
                    [&EscapeCause::OpaqueOperation(1)]
                    .nodes()
                    .is_none()
            );
            let a = analyze_in_module(&m, "caller", 1).unwrap().unwrap();
            assert!(a.value(ValueId(1)).unwrap().nodes().is_none());
        }
        let create = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "criar")
            .unwrap();
        create.blocks[2].instructions.push((
            ValueId(39),
            Instruction::CallStatic {
                symbol: "criar".into(),
                args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut create.blocks[3].instructions[0].1 {
            incoming[1].1 = Operand::Val(ValueId(39));
        }
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(
            a.value(ValueId(1)).unwrap().nodes(),
            Some(&std::collections::BTreeSet::from([allocated]))
        );
        m.arc_sites.remove("criar");
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(a.value(ValueId(1)).unwrap().nodes().is_none());
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn per_call_exception_preserves_payload_without_closing_boundary_and_validates_bodies() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![]);
        let explode = Function {
            symbol: "explode".into(),
            name: "explode".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Throw(Operand::Val(ValueId(0))),
            }],
        };
        let bridge = Function {
            symbol: "ponte".into(),
            name: "ponte".into(),
            debug_info: None,
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
            debug_info: None,
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
        m.functions = vec![caller, bridge, explode];
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        let e = compute_in_module(&m, "caller", &a).unwrap();
        assert_eq!(
            e.publications[&EscapeCause::Exception].nodes(),
            a.value(ValueId(0)).unwrap().nodes()
        );
        assert!(
            e.publications[&EscapeCause::OpaqueOperation(1)]
                .nodes()
                .is_none()
        );
        m.functions[2].blocks[0].terminator =
            Terminator::Throw(Operand::Constant(Constant::String("erro".into())));
        assert!(compute_in_module(&m, "caller", &a).is_err());
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        let e = compute_in_module(&m, "caller", &a).unwrap();
        assert!(e.publications[&EscapeCause::Exception].nodes().is_none());
        let a = analyze_in_module(&m, "caller", 0).unwrap().unwrap();
        let e = compute_in_module(&m, "caller", &a).unwrap();
        assert!(
            e.publications[&EscapeCause::OpaqueOperation(1)]
                .nodes()
                .is_none()
        );
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn per_call_publication_keeps_identity_and_opacity_and_checks_globals() {
        use super::super::escape::*;
        for mode in [StrongStoreMode::Copy, StrongStoreMode::Move] {
            let mut m = Module::new();
            m.arc_memory = true;
            m.globals = vec![(0, Type::Ref, "g".into()), (1, Type::Ref, "h".into())];
            m.arc_field_layouts.insert(1, vec![]);
            let publishes = Function {
                symbol: "pub".into(),
                name: "pub".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::ArcStoreStrong {
                            slot: StrongSlot::Global {
                                symbol: "g".into(),
                            },
                            value: Operand::Val(ValueId(0)),
                            mode,
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                }],
            };
            let caller = Function {
                symbol: "caller".into(),
                name: "caller".into(),
                debug_info: None,
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
            m.functions = vec![caller, publishes];
            super::super::origins::register(&mut m, 0, true);
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            let e = compute_in_module(&m, "caller", &a).unwrap();
            assert_eq!(
                e.publications[&EscapeCause::Global("g".into())].nodes(),
                a.value(ValueId(0)).unwrap().nodes()
            );
            assert!(
                e.publications[&EscapeCause::OpaqueOperation(1)]
                    .nodes()
                    .is_none()
            );
            // The nominal order of the declarations does not change the premise; duplication does.
            m.globals.reverse();
            assert!(compute_in_module(&m, "caller", &a).is_ok());
            m.globals.push((0, Type::Ref, "g".into()));
            assert!(compute_in_module(&m, "caller", &a).is_err());
            assert!(analyze_in_module(&m, "caller", 0).is_err());
            m.globals.pop();
            m.globals.iter_mut().find(|(_, _, s)| s == "g").unwrap().1 = Type::I64;
            assert!(compute_in_module(&m, "caller", &a).is_err());
            m.globals.iter_mut().find(|(_, _, s)| s == "g").unwrap().1 = Type::Ref;
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[1].1
            {
                args[0] = Operand::Constant(Constant::Null);
            }
            let n = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            let e = compute_in_module(&m, "caller", &n).unwrap();
            assert!(
                e.publications[&EscapeCause::Global("g".into())]
                    .nodes()
                    .unwrap()
                    .is_empty()
            );
            m.arc_memory = false;
            assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
        }
    }

    #[test]
    fn direct_publication_in_strong_slot_requires_declaration_and_keeps_boundary() {
        use super::super::escape::*;
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(Function {
            symbol: "pub".into(),
            name: "pub".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::ArcStoreStrong {
                        slot: StrongSlot::Global {
                            symbol: "g".into(),
                        },
                        value: Operand::Constant(Constant::Null),
                        mode: StrongStoreMode::Copy,
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        assert!(analyze_in_module(&m, "pub", 0).is_err());
        m.globals.push((0, Type::Ref, "g".into()));
        let a = analyze_in_module(&m, "pub", 8).unwrap().unwrap();
        let e = compute_in_module(&m, "pub", &a).unwrap();
        assert!(
            e.publications[&EscapeCause::Global("g".into())]
                .nodes()
                .unwrap()
                .is_empty()
        );
        assert!(
            e.publications[&EscapeCause::OpaqueOperation(0)]
                .nodes()
                .is_none()
        );
        m.globals[0].1 = Type::I64;
        assert!(analyze_in_module(&m, "pub", 0).is_err());
        assert!(compute_in_module(&m, "pub", &a).is_err());
    }

    #[test]
    fn field_return_keeps_opacity_body_guard_and_exact_abi() {
        let mut m = Module::new();
        m.arc_memory = true;
        let id = Function {
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
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
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
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(a.value(ValueId(0)).unwrap().nodes().unwrap().is_empty());
        if let Instruction::CallStatic { args, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[0] = Operand::Constant(Constant::Int(0));
        }
        let b = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(b.value(ValueId(0)).unwrap().nodes().is_none());
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
        assert!(super::super::escape::compute_in_module(&m, "caller", &a).is_err());
        let c = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(c.value(ValueId(0)).unwrap().nodes().is_none());
        assert_eq!(c.unknown_set(), &[ValueId(0)]);
        let e = super::super::escape::compute_in_module(&m, "caller", &c).unwrap();
        assert!(
            e.publications[&super::super::escape::EscapeCause::OpaqueOperation(0)]
                .nodes()
                .is_none()
        );
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn chained_reads_and_alias_write_resolve_at_same_fixpoint() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.arc_field_layouts.insert(2, vec![Type::Ref]);
        m.arc_field_layouts.insert(3, vec![]);
        m.functions.push(Function {
            symbol: "cadeia".into(),
            name: "cadeia".into(),
            debug_info: None,
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
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "cadeia", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(3)), a.value(ValueId(1)));
        assert_eq!(a.value(ValueId(5)), a.value(ValueId(0)));
        assert!(a.unknown_set().is_empty());
        let g = super::super::graph::build_local_in_module(&m, "cadeia", 8)
            .unwrap()
            .unwrap();
        // The prologue is still opaque; this step only resolves the known edges.
        assert!(g.opaque());
        // An index outside the schema cannot produce a precise empty result.
        if let Instruction::GetField { index, .. } = &mut m.functions[0].blocks[0].instructions[5].1
        {
            *index = 1;
        }
        let a = analyze_in_module(&m, "cadeia", 8).unwrap().unwrap();
        assert!(a.value(ValueId(5)).unwrap().nodes().is_none());
        assert!(a.unknown_set().contains(&ValueId(5)));
    }

    #[test]
    fn covered_call_still_requires_context_for_live_roots() {
        use super::super::escape::*;
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
        assert!(!crate::llvm::LlvmEmitter::requires_explicit_context(
            &f, None
        ));
        assert!(crate::llvm::LlvmEmitter::may_require_context_for_roots(
            &f
        ));
        assert!(!crate::llvm::LlvmEmitter::may_require_context_for_roots(
            &m.functions[0]
        ));
        m.functions.push(f);
        let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(a.unknown_set().is_empty()); // The id leaf is covered.
        assert!(
            compute_in_module(&m, "f", &a).unwrap().publications[&EscapeCause::ImplicitEmission]
                .nodes()
                .is_none()
        );
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("pilha.estouro:"));
        assert!(ir.contains("%gcq = alloca"));
    }

    #[test]
    fn implicit_prologue_publishes_top_without_inventing_hir_instruction() {
        use super::super::{escape::*, graph::build_local_in_module};
        let mut m = Module::new();
        m.arc_memory = true;
        m.excecoes_por_tabelas = true;
        m.functions.push(Function {
            symbol: "f".into(),
            name: "f".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(None),
            }],
        });
        for checks in [false, true] {
            m.tables = vec![FunctionTables {
                checks_stack: checks,
                ..Default::default()
            }];
            let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
            assert!(a.unknown_set().is_empty()); // There is no explicit instruction.
            let e = compute_in_module(&m, "f", &a).unwrap();
            if checks {
                assert!(
                    e.publications[&EscapeCause::ImplicitEmission]
                        .nodes()
                        .is_none()
                );
            } else {
                assert!(e.publications.is_empty());
            }
            assert_eq!(
                build_local_in_module(&m, "f", 8)
                    .unwrap()
                    .unwrap()
                    .opaque(),
                checks
            );
            // Checks the path really inserted at emission, besides the metadata.
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert_eq!(ir.contains("pilha.estouro:"), checks);
        }
        m.tables.clear(); // Without a table, no emission coverage is available.
        let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(
            compute_in_module(&m, "f", &a).unwrap().publications[&EscapeCause::ImplicitEmission]
                .nodes()
                .is_none()
        );
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn own_body_table_invalidates_callless_consumption_and_follows_symbol() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.excecoes_por_tabelas = true;
        for s in ["f", "g"] {
            m.functions.push(Function {
                symbol: s.into(),
                name: s.into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            });
        }
        m.tables = vec![FunctionTables::default(); 2];
        let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_ok());
        m.tables[0].checks_stack = true;
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_err());
        m.tables[0].checks_stack = false;
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_ok());
        m.functions.reverse();
        m.tables.reverse();
        m.tables[0].checks_stack = true; // A change only in g, outside the dependencies.
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_ok());
        m.tables[1].checks_stack = true;
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_err());
        m.excecoes_por_tabelas = false;
        assert!(super::super::escape::compute_in_module(&m, "f", &a).is_err());
    }

    #[test]
    fn leaf_preserves_fields_but_conversions_and_opaque_paths_invalidate() {
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
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(0)), a.value(ValueId(3)));
        assert_eq!(a.value(ValueId(3)).unwrap().nodes().unwrap().len(), 1);
        let e = super::super::escape::compute_in_module(&m, "caller", &a).unwrap();
        assert!(
            !e.publications
                .contains_key(&super::super::escape::EscapeCause::OpaqueOperation(2))
        );
        assert_eq!(
            e.publications[&super::super::escape::EscapeCause::Return],
            *a.value(ValueId(0)).unwrap()
        );
        // Ignoring the result of the identity does not publish the argument.
        m.functions[0].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let discarded = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        let e = super::super::escape::compute_in_module(&m, "caller", &discarded).unwrap();
        assert!(
            e.publications[&super::super::escape::EscapeCause::Return]
                .nodes()
                .unwrap()
                .is_empty()
        );
        assert!(
            !e.publications
                .contains_key(&super::super::escape::EscapeCause::OpaqueOperation(2))
        );
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
        // Emission metadata may introduce an implicit call, even
        // without a change of the HIR body; they must invalidate the previous consumption.
        m.excecoes_por_tabelas = true;
        m.tables = vec![FunctionTables::default(); 2];
        assert!(super::super::escape::compute_in_module(&m, "caller", &a).is_err());
        let with_tables = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(with_tables.value(ValueId(0)), with_tables.value(ValueId(3)));
        m.tables[1].checks_stack = true;
        assert!(super::super::escape::compute_in_module(&m, "caller", &with_tables).is_err());
        let with_guard = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(with_guard.unknown_set().contains(&ValueId(2)));
        assert!(
            analyze_in_module(&m, "caller", 8)
                .unwrap()
                .unwrap()
                .value(ValueId(3))
                .unwrap()
                .nodes()
                .is_none()
        );
        m.excecoes_por_tabelas = false;
        m.tables.clear();
        let leaf = m.functions[1].clone();
        // A normal alias does not erase an explicit write.
        m.functions[1].blocks[0].instructions.push((
            ValueId(1),
            Instruction::SetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        assert!(super::super::escape::compute_in_module(&m, "caller", &a).is_err());
        assert!(
            analyze_in_module(&m, "caller", 8)
                .unwrap()
                .unwrap()
                .value(ValueId(3))
                .unwrap()
                .nodes()
                .is_none()
        );
        m.functions[1] = leaf.clone();
        m.globals.push((0, Type::Ref, "g".into()));
        m.functions[1].blocks[0].instructions.push((
            ValueId(1),
            Instruction::StoreGlobal {
                symbol: "g".into(),
                val: Operand::Val(ValueId(0)),
                ty: Type::Ref,
                root: Some(0),
            },
            Type::Void,
        ));
        let global = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(global.unknown_set().contains(&ValueId(2)));
        let e = super::super::escape::compute_in_module(&m, "caller", &global).unwrap();
        assert!(
            e.publications[&super::super::escape::EscapeCause::OpaqueOperation(2)]
                .nodes()
                .is_none()
        );
        m.functions[1] = leaf;
        // Field preservation does not depend on the existence of a Ref return/alias.
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
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(0)), a.value(ValueId(3)));
        for constant in [Constant::Int(42), Constant::String("texto".into())] {
            // Converting/materializing an argument may collect before the leaf.
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[2].1
            {
                args[0] = Operand::Constant(constant);
            }
            assert!(
                analyze_in_module(&m, "caller", 8)
                    .unwrap()
                    .unwrap()
                    .value(ValueId(3))
                    .unwrap()
                    .nodes()
                    .is_none()
            );
        }
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    #[test]
    fn direct_call_preserves_alias_and_checks_dependency_without_proving_noescape() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
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
            debug_info: None,
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
        let mut bridge = m.functions[1].clone();
        bridge.symbol = "ponte".into();
        bridge.name = "ponte".into();
        bridge.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "id".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        bridge.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        m.functions.push(bridge);
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(0)), a.value(ValueId(1)));
        assert_eq!(a.value(ValueId(1)).unwrap().nodes().unwrap().len(), 1);
        // Returning the argument does not guarantee that the field stays null:
        // the callee above writes a self-edge before the normal return.
        assert!(a.value(ValueId(2)).unwrap().nodes().is_none());
        assert_eq!(a.unknown_set(), &[ValueId(1)]);
        assert!(super::super::escape::compute(&m.functions[0], &a).is_err());
        let r = super::super::escape::compute_in_module(&m, "caller", &a).unwrap();
        assert!(
            r.publications[&super::super::escape::EscapeCause::OpaqueOperation(1)]
                .nodes()
                .is_none()
        );
        m.functions[1].blocks[0].terminator =
            Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(super::super::escape::compute_in_module(&m, "caller", &a).is_err());
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(a.value(ValueId(1)).unwrap().nodes().unwrap().is_empty());
        m.functions.pop();
        assert!(super::super::escape::compute_in_module(&m, "caller", &a).is_err());
        let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        assert!(a.value(ValueId(1)).unwrap().nodes().is_none());
    }

    #[test]
    fn zeroed_factory_uses_origin_layout_and_stores_without_manual_plan() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.functions.push(Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
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
        super::super::origins::register(&mut m, 0, true);
        let a = analyze_in_module(&m, "criar", 8).unwrap().unwrap();
        assert_eq!(a.value(ValueId(0)), a.value(ValueId(2)));
        assert_eq!(a.value(ValueId(0)).unwrap().nodes().unwrap().len(), 1);
        assert!(a.unknown_set().is_empty());
        assert!(super::super::escape::compute(&m.functions[0], &a).is_err());
        assert!(super::super::escape::compute_in_module(&m, "criar", &a).is_ok());
        m.arc_field_layouts.insert(1, vec![Type::I64]);
        assert!(super::super::escape::compute_in_module(&m, "criar", &a).is_err());
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        let origin = m.arc_sites["criar"][&ValueId(0)].clone();
        m.arc_sites
            .get_mut("criar")
            .unwrap()
            .get_mut(&ValueId(0))
            .unwrap()
            .origin += 1;
        assert!(super::super::escape::compute_in_module(&m, "criar", &a).is_err());
        m.arc_sites
            .get_mut("criar")
            .unwrap()
            .insert(ValueId(0), origin);
        m.arc_memory = false;
        assert!(super::super::escape::compute_in_module(&m, "criar", &a).is_err());
        m.arc_memory = true;
        assert!(super::super::escape::compute_in_module(&m, "criar", &a).is_ok());
        // Rebuilding HashMaps in another order does not change the nominal premises.
        m.arc_field_layouts.insert(2, vec![Type::F64]);
        m.arc_field_layouts.insert(3, vec![Type::I1]);
        let sorted = analyze_in_module(&m, "criar", 8).unwrap().unwrap();
        let mut layouts: Vec<_> = m.arc_field_layouts.drain().collect();
        layouts.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
        m.arc_field_layouts.extend(layouts);
        assert!(super::super::escape::compute_in_module(&m, "criar", &sorted).is_ok());
        // A changed ABI/count invalidates the recognition, without reinterpreting bits.
        if let Instruction::CallRuntime { args, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[1].0 = Operand::Constant(Constant::Int(2));
        }
        assert!(analyze_in_module(&m, "criar", 8).is_err());
        if let Instruction::CallRuntime { args, ret_ty, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            args[1].0 = Operand::Constant(Constant::Int(1));
            *ret_ty = Type::I64;
        }
        let a = analyze_in_module(&m, "criar", 8).unwrap().unwrap();
        assert!(a.value(ValueId(0)).unwrap().nodes().is_none());
        if let Instruction::CallRuntime { ret_ty, .. } =
            &mut m.functions[0].blocks[0].instructions[0].1
        {
            *ret_ty = Type::Ref;
        }
        m.arc_field_layouts.clear();
        let a = analyze_in_module(&m, "criar", 8).unwrap().unwrap();
        assert!(a.value(ValueId(2)).unwrap().nodes().is_none());
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "criar", 8).unwrap().is_none());
    }
}
