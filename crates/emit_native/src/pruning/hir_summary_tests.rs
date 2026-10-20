//! Snapshot equality and transitive FFI reachability, independently of emitted code.
use super::*;
use crate::hir::*;

fn function(symbol: &str, recipe: Option<&str>, callee: Option<&str>) -> Function {
    let mut instructions = Vec::new();
    if let Some(recipe) = recipe {
        instructions.push((
            ValueId(0),
            Instruction::Const(Constant::String(recipe.into())),
            Type::Ref,
        ));
    }
    if let Some(callee) = callee {
        instructions.push((
            ValueId(1),
            Instruction::CallStatic {
                symbol: callee.into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
    }
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![],
        return_ty: Type::Void,
        debug_info: None,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions,
            terminator: Terminator::Return(None),
        }],
    }
}

fn fixture() -> (Module, Summary) {
    let mut module = Module::new();
    module.entry_symbol = Some("df.test.main".into());
    module.ffi_types = vec![(703, 'i'), (737, 'v'), (719, 'p'), (698, 'd')];
    module.functions = vec![
        function("df.test.main", None, Some("sdk.factory")),
        function("ffi_a", Some("F<0;C737;1;C719;>"), None),
        function("ffi_b", Some("F<0;C737;0;;>"), None),
        function("callback_c", None, Some("leaf")),
        function("leaf", None, None),
        function("unused", Some("F<0;C703;0;;>"), None),
        function("unused_callback", None, None),
    ];
    module.ffi_trampolines = vec![
        ("i_i".into(), "ffi_a".into()),
        ("v_p".into(), "ffi_b".into()),
        ("v_d".into(), "unused".into()),
    ];
    module.ffi_callbacks = vec![
        FfiCallback {
            key: "v_".into(),
            body: "callback_c".into(),
            ret: NativeType::Prim(CType::Void),
            params: vec![],
        },
        FfiCallback {
            key: "v_d".into(),
            body: "unused_callback".into(),
            ret: NativeType::Prim(CType::Void),
            params: vec![NativeType::Prim(CType::F64)],
        },
    ];
    let mut sdk = Summary::new("sdk");
    let node = sdk.id("sdk.factory");
    sdk.definitions.push(Definition {
        name: node,
        ..Default::default()
    });
    sdk.recipes.push((node, "F<0;C703;1;C703;>".into()));
    (module, sdk)
}

/// Reconstruct every summary as the previous fixed-point implementation did.
fn fresh_ffi_signatures(module: &Module, sdk: &Summary) -> (HashSet<String>, usize) {
    let letters = module.ffi_types.iter().copied().collect();
    let composites = module.ffi_composites.iter().map(|c| c.rti).collect();
    let mut live = HashSet::new();
    let mut rounds = 0;
    loop {
        rounds += 1;
        let program = hir_summary_with(module, Some(&live));
        let all = [sdk, &program];
        let reachable = reach(&all, true);
        let index: HashMap<_, _> = reachable
            .names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i as u32))
            .collect();
        let before = live.len();
        for summary in all {
            for (node, recipe) in &summary.recipes {
                if index
                    .get(summary.names[*node as usize].as_str())
                    .is_some_and(|node| reachable.live(*node))
                {
                    recipe_ffi_keys(recipe, &letters, &composites, &mut live);
                }
            }
        }
        if before == live.len() {
            return (live, rounds);
        }
    }
}

#[test]
fn transitive_sdk_recipes_and_callbacks_stabilize_without_rebuilding_bodies() {
    let (mut module, sdk) = fixture();
    let before = HIR_BODY_BUILDS.with(|n| n.get());
    let (expected, rounds) = fresh_ffi_signatures(&module, &sdk);
    let reference_builds = HIR_BODY_BUILDS.with(|n| n.get()) - before;
    assert_eq!(
        expected,
        HashSet::from(["i_i".into(), "v_p".into(), "v_".into()])
    );
    assert_eq!((rounds, reference_builds), (4, 4));
    let before = HIR_BODY_BUILDS.with(|n| n.get());
    let result = prune_hir(&mut module, &[&sdk]);
    let builds = HIR_BODY_BUILDS.with(|n| n.get()) - before;
    assert_eq!(result.reachability_rounds, rounds);
    assert_eq!(builds, 1);
    assert_eq!(result.ffi, (3, true));
    assert_eq!(
        module
            .ffi_trampolines
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["i_i", "v_p"]
    );
    assert_eq!(
        module
            .ffi_callbacks
            .iter()
            .map(|callback| callback.body.as_str())
            .collect::<Vec<_>>(),
        ["callback_c"]
    );
    assert_eq!(
        module
            .functions
            .iter()
            .map(|f| f.symbol.as_str())
            .collect::<Vec<_>>(),
        ["df.test.main", "ffi_a", "ffi_b", "callback_c", "leaf"]
    );
    println!(
        "{}",
        serde_json::json!({"ffi_rounds":rounds,"fresh_body_builds":reference_builds,"snapshot_body_builds":builds,"live_signatures":3})
    );
}

#[test]
fn root_growth_shrink_and_missing_external_names_match_fresh_summaries_exactly() {
    let (mut module, _) = fixture();
    module
        .ffi_trampolines
        .push(("missing".into(), "external_trampoline".into()));
    module.ffi_callbacks.push(FfiCallback {
        key: "missing".into(),
        body: "external_callback".into(),
        ret: NativeType::Prim(CType::Void),
        params: vec![],
    });
    module.source_roots.push("permanent_source_root".into());
    module
        .helpers
        .push(("helper".into(), "external_helper".into()));
    module.globals.push((42, Type::Ref, "state".into()));
    let sets = [
        None,
        Some(HashSet::new()),
        Some(HashSet::from(["missing".into()])),
        Some(HashSet::from(["i_i".into(), "v_".into()])),
        Some(HashSet::new()),
        None,
    ];
    let mut snapshot = HirSummarySnapshot::new(&module);
    for live in sets.iter().cycle().take(24) {
        let fresh = hir_summary_with(&module, live.as_ref());
        let cached = snapshot.with_ffis(live.as_ref());
        assert_eq!(cached, &fresh);
        assert_eq!(cached.to_text(), fresh.to_text());
        if live.as_ref().is_some_and(HashSet::is_empty) {
            assert!(
                !cached
                    .names
                    .iter()
                    .any(|name| name == "external_trampoline" || name == "external_callback")
            );
        }
        assert!(
            cached
                .names
                .iter()
                .any(|name| name == "permanent_source_root")
        );
    }
}

#[test]
fn existing_entry_names_and_unpruned_roots_keep_the_original_identity() {
    let (mut module, _) = fixture();
    module
        .functions
        .push(function("entry_caller", None, Some("dartforge_entry")));
    module
        .functions
        .push(function("dartforge_entry", None, None));
    let fresh = hir_summary(&module);
    let mut snapshot = HirSummarySnapshot::new(&module);
    snapshot.with_ffis(Some(&HashSet::new()));
    assert_eq!(snapshot.with_ffis(None), &fresh);
    assert_eq!(
        fresh
            .names
            .iter()
            .filter(|name| name.as_str() == "dartforge_entry")
            .count(),
        1
    );
    drop(snapshot);
    module.ffi_trampolines.clear();
    module.ffi_callbacks.clear();
    let before = HIR_BODY_BUILDS.with(|n| n.get());
    let result = prune_hir(&mut module, &[]);
    assert_eq!(result.reachability_rounds, 1);
    assert!(!result.ffi.1);
    assert_eq!(HIR_BODY_BUILDS.with(|n| n.get()) - before, 1);
}

#[test]
fn a_new_snapshot_reaudits_changed_bodies_and_registry_exposure() {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    let mut body = function("receiver", None, None);
    body.blocks[0].instructions = vec![
        (
            ValueId(0),
            Instruction::AllocObject {
                class_id: 300,
                fields: vec![],
            },
            Type::Ref,
        ),
        (
            ValueId(1),
            Instruction::CallSelector {
                selector: "c:used".into(),
                recv: Operand::Val(ValueId(0)),
                args: vec![],
                names: vec![],
                tuple_types: Operand::Constant(Constant::Int(0)),
            },
            Type::Ref,
        ),
    ];
    module.functions.push(body);
    for cid in [300, 301] {
        module.pruning_class_ids.insert(cid);
        module.classes.push(ClassDef {
            id: cid,
            name: format!("Class{cid}"),
            field_count: 0,
            vtable: vec![],
            to_string_symbol: None,
        });
    }
    let before = {
        let mut snapshot = HirSummarySnapshot::new(&module);
        snapshot.with_ffis(None).clone()
    };
    assert!(
        before
            .exact_receiver_calls
            .iter()
            .any(|(_, _, cid)| *cid == 300)
    );
    module.functions[0].blocks[0].instructions[0].1 = Instruction::AllocObject {
        class_id: 301,
        fields: vec![],
    };
    let changed = {
        let mut snapshot = HirSummarySnapshot::new(&module);
        snapshot.with_ffis(None).clone()
    };
    assert!(
        changed
            .exact_receiver_calls
            .iter()
            .any(|(_, _, cid)| *cid == 301)
    );
    assert!(
        !changed
            .exact_receiver_calls
            .iter()
            .any(|(_, _, cid)| *cid == 300)
    );
    module
        .functions
        .push(function("dartforge_register_methods", None, None));
    let opened = {
        let mut snapshot = HirSummarySnapshot::new(&module);
        snapshot.with_ffis(None).clone()
    };
    assert!(opened.exact_receiver_calls.is_empty());
    assert!(
        opened
            .definitions
            .iter()
            .any(|d| d.calls.contains(&selector_hash("c:used")))
    );
}
