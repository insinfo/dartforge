//! Nominal origin assigned before the passes that renumber/clone SSA.
//! The identity holds for the body version; it does not certify pins/reload,
//! specialized context or singleton. Opaque operations still have no site.

use super::model::ArcSite;
use crate::hir::*;

pub(crate) fn allocation(i: &Instruction) -> bool {
    matches!(
        i,
        Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::AllocClosureTyped { .. }
            | Instruction::Box { .. }
            | Instruction::ConcatStrings { .. }
    ) || matches!(i, Instruction::CallRuntime { name, .. }
            if matches!(name.as_str(), "dartforge_object_new" | "dartforge_object_new_t"
                | "dartforge_arc_objeto_owned_v1" | "dartforge_arc_objeto_owned_t_v1"))
}

/// Records the origin of freshly produced bodies, only in the ARC lowering.
/// The ordinal identifies the original allocation, not its future SSA ValueId.
pub(crate) fn register(m: &mut Module, start: usize, arc_memory: bool) {
    if !arc_memory {
        return;
    }
    for f in &m.functions[start..] {
        let sites = m.arc_sites.entry(f.symbol.clone()).or_default();
        let mut next_item = sites.values().map(|s| s.origin).max().map_or(0, |v| v + 1);
        for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if allocation(i) && !sites.contains_key(v) {
                sites.insert(
                    *v,
                    ArcSite {
                        function: f.symbol.clone(),
                        origin: next_item,
                        specialization: String::new(),
                    },
                );
                next_item += 1;
            }
        }
    }
}

/// Removes facts of eliminated instructions, without forging origins for new operations.
pub(crate) fn prune(m: &mut Module) {
    let live_set: std::collections::HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| {
            let ids: std::collections::HashSet<_> = f
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .filter(|(_, i, _)| allocation(i))
                .map(|(v, _, _)| *v)
                .collect();
            (f.symbol.clone(), ids)
        })
        .collect();
    m.arc_sites.retain(|s, sites| {
        let Some(ids) = live_set.get(s) else {
            return false;
        };
        sites.retain(|v, _| ids.contains(v));
        !sites.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inliner_remaps_ssa_but_keeps_site_and_pruning_does_not_forge_facts() {
        let mut m = Module::new();
        m.arc_memory = true;
        let factory = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(57),
                    Instruction::AllocObject {
                        class_id: 1,
                        fields: vec![],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(57)))),
            }],
        };
        let mut caller = Function {
            symbol: "chamar".into(),
            name: "chamar".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(5),
                        Instruction::CallStatic {
                            symbol: "criar".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(6),
                        Instruction::CallStatic {
                            symbol: "criar".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(6)))),
            }],
        };
        m.functions.push(factory.clone());
        register(&mut m, 0, true);
        let site = m.arc_sites["criar"][&ValueId(57)].clone();
        assert_eq!(site.origin, 0); // Not ValueId 57 of the original body.
        let mut locals = std::collections::HashMap::new();
        let copies = std::collections::HashMap::from([("criar".into(), (factory, false))]);
        assert!(crate::optimize::inline::inlining_with_origins(
            &mut caller,
            &copies,
            &m.arc_sites,
            &mut locals
        ));
        assert_eq!(locals.len(), 2);
        assert!(locals.values().all(|s| s == &site));
        assert!(!locals.contains_key(&ValueId(57)));
        m.arc_sites.insert(caller.symbol.clone(), locals);
        m.functions.push(caller);
        let before = m.arc_sites.clone();
        prune(&mut m);
        assert_eq!(m.arc_sites, before);
        crate::optimize::optimize(&mut m);
        assert!(m.arc_sites["chamar"].values().all(|s| s == &site));
        let optimized = m.arc_sites.clone();
        crate::optimize::optimize(&mut m);
        assert_eq!(m.arc_sites, optimized);
        m.functions[1]
            .blocks
            .iter_mut()
            .for_each(|b| b.instructions.retain(|(_, i, _)| !allocation(i)));
        prune(&mut m);
        assert!(!m.arc_sites.contains_key("chamar"));
        let mut tracing = Module::new();
        tracing.functions.push(m.functions[0].clone());
        register(&mut tracing, 0, false);
        assert!(tracing.arc_sites.is_empty());
    }
}
