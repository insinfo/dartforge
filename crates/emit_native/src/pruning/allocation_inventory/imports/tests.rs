use super::*;

const MODULE: &str = "dart:test";
const LEAF: &str = "df.dart$3atest..leaf";
const NAME_STARTUP: &str = "; df.cone-class 900\n; df.cone-class 901\n; df.cone-parent 900 901\n@cls = private constant [6 x i8] c\"Na\\C3\\AFve\"\n@arg = private constant [5 x i8] c\"a\\22\\5C\\40;\"\ndefine void @df.register.dart$3atest() {\n call void @dartforge_register_class_name(i64 900, ptr @cls, i64 6)\n call void @dartforge_register_subclass(i64 900, i64 901)\n call void @dartforge_register_argument_name(ptr @arg, i64 5)\n ret void\n}\n";
const SUBTYPE_STARTUP: &str = "; df.cone-class 900\n; df.cone-class 901\n; df.cone-parent 900 901\ndefine void @df.register.dart$3atest() {\n call void @dartforge_register_subclass(i64 900, i64 901)\n ret void\n}\n";
fn fixtures(ir: &str) -> (tempfile::TempDir, Vec<String>, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let runtime = dir.path().join("runtime.a");
    std::fs::write(&runtime, b"test runtime artifact; not executable code").unwrap();
    std::fs::write(
        dir.path().join("test.o"),
        b"test object artifact; not executable code",
    )
    .unwrap();
    let summary = crate::pruning::summarize(ir, MODULE, &[]);
    std::fs::write(dir.path().join("test.poda"), summary.to_text()).unwrap();
    let names = vec!["test.o".to_string(), "test.poda".to_string()];
    let bundle = crate::pruning::allocation_inventory::closure::audit(&[(MODULE, ir)]);
    crate::pruning::allocation_inventory::publication::publish(
        dir.path(),
        "test-key",
        true,
        &names,
        &runtime,
        &bundle,
    )
    .unwrap();
    (dir, names, runtime)
}
#[test]
fn static_block_observations_do_not_authorize_handles_or_image_registration() {
    let literal = super::super::images::tests::emitted_literal("λ".as_bytes());
    let symbol = crate::pruning::global_name(&literal).unwrap().0;
    let (markers, start, end) = crate::target::image_markers();
    let ir = format!(
        "{literal}\n{markers}\ndefine void @df.register.dart$3atest() {{\n call void @dartforge_register_image(ptr {start}, ptr {end})\n ret void\n}}\ndefine i64 @{LEAF}() {{\n ret i64 ptrtoint (ptr getelementptr (i8, ptr @\"{symbol}\", i64 2) to i64)\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    let (count, cids) = inventory.image_string_counts();
    assert_eq!(inventory.clean_source_image_count(), 1);
    assert_eq!(count, 1);
    assert_eq!(
        cids,
        &BTreeSet::from([dartforge_runtime::layout::cid::TWO_BYTE_STRING as u32])
    );
    assert!(!inventory.entry("df.register.dart$3atest"));
    assert!(!inventory.known_node(symbol));
    assert!(!inventory.call(LEAF, &[], Type::Ref));
    assert!(!inventory.call(LEAF, &[], Type::I64));
}

#[test]
fn source_image_census_is_bound_to_the_module_and_stored_global_observations() {
    let literal = super::super::images::tests::emitted_literal(b"census");
    let ir = format!("{literal}\n{}", crate::target::image_markers().0);
    for variant in 0..4 {
        let (dir, names, runtime) = fixtures(&ir);
        rewrite(dir.path(), |value| {
            let census = &mut value["inventory"]["modules"][0]["image_census"];
            match variant {
                0 => census["source_digest"] = serde_json::json!("0".repeat(64)),
                1 => census["members"] = serde_json::json!([]),
                2 => census["marker_lines"] = serde_json::json!([]),
                _ => census["members"][0]["kind"] = serde_json::json!("anchor"),
            }
        });
        assert!(super::super::publication::verify(
            dir.path(),
            "test-key",
            true,
            &names,
            &runtime
        ));
        assert!(
            Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none(),
            "variant {variant}"
        );
    }
}

#[test]
fn managed_initializer_classification_is_recomputed_after_integrity_validation() {
    let literal = super::super::images::tests::emitted_literal(b"abc");
    for variant in 0..3 {
        let (dir, names, runtime) = fixtures(&literal);
        rewrite(dir.path(), |value| {
            let global = &mut value["inventory"]["globals"][0];
            if variant == 0 {
                global["kind"] = serde_json::json!("native_bytes");
            } else if variant == 1 {
                global["kind"] = serde_json::json!("unclassified_initializer");
            } else {
                let altered = literal.replace("align 8", "align 1");
                global["initializer_digest"] =
                    serde_json::json!(blake3::hash(altered.as_bytes()).to_hex().to_string());
                global["initializer"] = serde_json::json!(altered);
            }
        });
        assert!(super::super::publication::verify(
            dir.path(),
            "test-key",
            true,
            &names,
            &runtime
        ));
        assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
    }
}

#[test]
fn exact_private_utf8_names_extend_the_entry_only_metadata_domain() {
    let (dir, names, runtime) = fixtures(NAME_STARTUP);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.entry("df.register.dart$3atest"));
    assert!(!inventory.call("df.register.dart$3atest", &[], Type::Void));
    for symbol in [
        "dartforge_register_class_name",
        "dartforge_register_argument_name",
    ] {
        assert!(!inventory.known_node(symbol));
        assert!(!inventory.call(symbol, &[], Type::Void));
    }
}

#[test]
fn text_metadata_startups_do_not_require_a_subtype_call() {
    for api in [
        "dartforge_register_class_name",
        "dartforge_register_argument_name",
    ] {
        let ir = NAME_STARTUP
            .lines()
            .filter(|line| !line.contains("call void @dartforge_register_") || line.contains(api))
            .collect::<Vec<_>>()
            .join("\n");
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(inventory.entry("df.register.dart$3atest"), "{api}");
    }
}

#[test]
fn literal_at_signs_do_not_publish_address_edges_from_name_bytes() {
    let ir = NAME_STARTUP.replace("a\\22\\5C\\40;", "a@b;c");
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.entry("df.register.dart$3atest"));
    let refs = |summary: &Summary| {
        summary
            .definitions
            .iter()
            .find(|d| summary.names[d.name as usize] == "dart:test#arg")
            .unwrap()
            .refs
            .clone()
    };
    assert!(
        !refs(&inventory.summaries[0]).is_empty(),
        "legacy summary observes a literal @"
    );
    assert!(refs(&inventory.class_proof_summaries()[0]).is_empty());
    assert!(
        !refs(&inventory.summaries[0]).is_empty(),
        "original snapshot stays intact"
    );
}

#[test]
fn metadata_names_require_exact_lengths_local_literals_and_valid_utf8() {
    for variant in 0..10 {
        let ir = match variant {
            0 => NAME_STARTUP.replace("ptr @cls, i64 6", "ptr @cls, i64 5"),
            1 => NAME_STARTUP.replace("ptr @arg, i64 5", "ptr @arg, i64 6"),
            2 => NAME_STARTUP.replace("ptr @arg, i64 5", "ptr @arg, i64 -1"),
            3 => NAME_STARTUP.replace("Na\\C3\\AFve", "Na\\FF\\FEve"),
            4 => NAME_STARTUP.replace("private constant", "constant"),
            5 => NAME_STARTUP.replace("ptr @cls,", "ptr null,"),
            6 => NAME_STARTUP.replace(
                "ptr @cls,",
                "ptr getelementptr ([6 x i8], ptr @cls, i64 0, i64 1),",
            ),
            7 => NAME_STARTUP.replace("i64 900, ptr @cls", "i64 999, ptr @cls"),
            8 => format!("{NAME_STARTUP}@cls = private constant [6 x i8] c\"Other!\"\n"),
            _ => NAME_STARTUP.replace(
                " ret void",
                " call void @dartforge_register_helper(ptr @arg, i64 5, ptr null)\n ret void",
            ),
        };
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(
            !inventory.entry("df.register.dart$3atest"),
            "variant {variant}"
        );
    }
}

#[test]
fn initializer_records_are_rechecked_after_integrity_validation() {
    for variant in 0..5 {
        let (dir, names, runtime) = fixtures(NAME_STARTUP);
        rewrite(dir.path(), |value| {
            let record = &mut value["inventory"]["globals"][0];
            match variant {
                0 => {
                    record["initializer"] =
                        serde_json::json!("@cls = private constant [6 x i8] c\"Other!\"")
                }
                1 => record["initializer"] = serde_json::Value::Null,
                2 => {
                    record.as_object_mut().unwrap().remove("initializer");
                }
                3 => record["symbol"] = serde_json::json!("different"),
                _ => {
                    let initializer = "@cls = private constant [6 x i8] c\"short\"";
                    record["initializer"] = serde_json::json!(initializer);
                    record["initializer_digest"] = serde_json::json!(
                        blake3::hash(initializer.as_bytes()).to_hex().to_string()
                    );
                }
            }
        });
        assert!(super::super::publication::verify(
            dir.path(),
            "test-key",
            true,
            &names,
            &runtime
        ));
        assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
    }
}

#[test]
fn nominal_startup_certificate_is_entry_only_and_owns_its_graph_edges() {
    let ir = format!(
        "{SUBTYPE_STARTUP}define void @df.dart$3atest..other() {{\n call void @dartforge_register_subclass(i64 900, i64 901)\n ret void\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    let startup = "df.register.dart$3atest";
    assert!(inventory.entry(startup));
    assert!(!inventory.call(startup, &[], Type::Void));
    assert!(inventory.known_node(startup));
    assert!(!inventory.known_node("dartforge_register_subclass"));
    assert!(!inventory.entry("df.dart$3atest..other"));
    let proof = inventory.class_proof_summaries();
    for (symbol, keeps_edge) in [(startup, false), ("df.dart$3atest..other", true)] {
        let edges = |summary: &Summary| {
            let definition = summary
                .definitions
                .iter()
                .find(|d| summary.names[d.name as usize] == symbol)
                .unwrap();
            definition
                .refs
                .iter()
                .map(|n| summary.names[*n as usize].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            edges(&proof[0]).contains(&"dartforge_register_subclass".into()),
            keeps_edge
        );
        assert!(edges(&inventory.summaries[0]).contains(&"dartforge_register_subclass".into()));
    }
}

#[test]
fn nominal_startup_rejects_unproved_operands_effects_and_identities() {
    for variant in 0..18 {
        let ir = match variant {
            0 => SUBTYPE_STARTUP.replace("; df.cone-class 901\n", ""),
            1 => SUBTYPE_STARTUP.replace("; df.cone-parent 900 901\n", ""),
            2 => SUBTYPE_STARTUP.replace("i64 900, i64 901", "i64 900, i64 902"),
            3 => SUBTYPE_STARTUP.replace("i64 900, i64 901", "i64 -1, i64 901"),
            4 => SUBTYPE_STARTUP.replace("i64 900, i64 901", "i64 2147483648, i64 901"),
            5 => SUBTYPE_STARTUP
                .replace("() {", "(i64 %cid) {")
                .replace("i64 900,", "i64 %cid,"),
            6 => SUBTYPE_STARTUP.replace(
                "call void @dartforge_register_subclass",
                "call void @opaque",
            ),
            7 => SUBTYPE_STARTUP.replace(
                " ret void",
                " call void @dartforge_register_image(ptr null, ptr null)\n ret void",
            ),
            8 => SUBTYPE_STARTUP.replace(
                " ret void",
                " %o = call i64 @dartforge_object_new(i64 900, i64 0)\n ret void",
            ),
            9 => SUBTYPE_STARTUP.replace("define void", "define internal void"),
            10 => SUBTYPE_STARTUP.replace("define void", "define weak void"),
            11 => SUBTYPE_STARTUP.replace(
                "call void @dartforge_register_subclass(i64 900, i64 901)",
                "call void @dartforge_register_subclass(i32 900, i32 901)",
            ),
            12 => format!(
                "{SUBTYPE_STARTUP}define void @dartforge_register_subclass(i64 %a, i64 %b) {{\n ret void\n}}\n"
            ),
            13 => format!(
                "{SUBTYPE_STARTUP}define internal void @dartforge_register_subclass(i64 %a, i64 %b) {{\n ret void\n}}\n"
            ),
            14 => SUBTYPE_STARTUP.replace(
                " ret void",
                " call void @dartforge_register_helper(ptr null, i64 0, ptr null)\n ret void",
            ),
            15 => SUBTYPE_STARTUP.replace(
                " ret void",
                " call void @dartforge_register_table(i64 900, ptr null)\n ret void",
            ),
            16 => SUBTYPE_STARTUP.replace(
                " ret void",
                " call void @dartforge_register_class_name(i64 900, ptr null, i64 0)\n ret void",
            ),
            _ => SUBTYPE_STARTUP.replace(" ret void", " fence seq_cst\n ret void"),
        };
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(
            !inventory.entry("df.register.dart$3atest"),
            "variant {variant}"
        );
    }
}

#[test]
fn quoted_header_cannot_turn_an_allocator_into_an_empty_import() {
    let ir = format!(
        "define i64 @{LEAF}(i64 %\"{{ ret i64 0 }}\") {{\n %o = call i64 @dartforge_object_new(i64 399, i64 0)\n ret i64 7\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(!inventory.call(LEAF, &[Type::I64], Type::I64));
}

#[test]
fn verified_scalar_closure_requires_its_actual_abi_and_graph_snapshot() {
    let ir = format!("define i64 @{LEAF}(i64 %x) {{\n %v = add i64 %x, 1\n ret i64 %v\n}}\n");
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.call(LEAF, &[Type::I64], Type::I64));
    assert!(!inventory.call(LEAF, &[], Type::I64));
    assert!(!inventory.call(LEAF, &[Type::F64], Type::I64));
    let summary = crate::pruning::summarize(&ir, MODULE, &[]);
    assert!(
        inventory
            .reconcile(&[&summary], &HashSet::new(), &HashSet::new(), false)
            .is_ok()
    );
    let mut changed = summary.clone();
    changed.definitions[0].refs.push(0);
    assert!(
        inventory
            .reconcile(&[&changed], &HashSet::new(), &HashSet::new(), false)
            .is_err()
    );
    assert!(
        inventory
            .reconcile(
                &[&summary],
                &HashSet::from([LEAF.to_string()]),
                &HashSet::new(),
                false
            )
            .is_err()
    );
    std::fs::write(dir.path().join("test.o"), b"changed object").unwrap();
    assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
}

#[test]
fn runtime_key_profile_and_manifest_bind_the_snapshot() {
    let ir = format!("define i64 @{LEAF}() {{\n ret i64 1\n}}\n");
    let (dir, names, runtime) = fixtures(&ir);
    for (key, production) in [("other", true), ("test-key", false)] {
        assert!(Inventory::load(dir.path(), key, production, &names, &runtime).is_none());
    }
    assert!(Inventory::load(dir.path(), "test-key", true, &[], &runtime).is_none());
    std::fs::write(&runtime, b"different runtime").unwrap();
    assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
    std::fs::write(&runtime, b"test runtime artifact; not executable code").unwrap();
    std::fs::remove_file(dir.path().join("allocation-inventory.ready")).unwrap();
    assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
}

#[test]
fn private_dependencies_and_recursive_scalar_components_are_checked() {
    let ir = format!(
        "define private i64 @private_leaf(i64 %x) {{\n ret i64 %x\n}}\ndefine i64 @{LEAF}(i64 %x) {{\n %v = call i64 @private_leaf(i64 %x)\n ret i64 %v\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(i.call(LEAF, &[Type::I64], Type::I64));
    assert!(i.known_node("dart:test#private_leaf"));
    assert!(!i.call("private_leaf", &[Type::I64], Type::I64));
    for unknown in [false, true] {
        let ir = format!(
            "define i64 @{LEAF}(i64 %x) {{\n %v = call i64 @df.dart$3atest..next(i64 %x)\n ret i64 %v\n}}\ndefine i64 @df.dart$3atest..next(i64 %x) {{\n %v = call i64 @{}(i64 %x)\n ret i64 %v\n}}\n",
            if unknown { "opaque_native" } else { LEAF }
        );
        let (dir, names, runtime) = fixtures(&ir);
        let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert_eq!(i.call(LEAF, &[Type::I64], Type::I64), !unknown);
    }
}

#[test]
fn allocation_coverage_does_not_imply_exposure_coverage() {
    for (result, body) in [
        (
            "i64",
            "%v = call i64 @dartforge_object_new(i64 300, i64 0)\n ret i64 %v",
        ),
        (
            "i8",
            "%v = call i8 @dartforge_exception_pending()\n ret i8 %v",
        ),
        ("i64", "%v = call i64 @opaque_native()\n ret i64 %v"),
        ("i64", "%v = load i64, ptr null\n ret i64 %v"),
        (
            "void",
            "call void @dartforge_register_image(ptr null, ptr null)\n ret void",
        ),
    ] {
        let ir = format!("define {result} @{LEAF}() {{\n {body}\n}}\n");
        let (dir, names, runtime) = fixtures(&ir);
        let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(!i.known_node(LEAF), "{body}");
    }
}

fn rewrite(dir: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let file = dir.join("allocation-inventory.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    change(&mut value);
    let text = format!("{value}\n");
    std::fs::write(&file, &text).unwrap();
    // Deliberately recompute integrity: semantic consistency is a separate check.
    std::fs::write(dir.join("allocation-inventory.ready"),format!("{}\n",serde_json::json!({"schema":super::super::publication::SCHEMA,"cache_key":"test-key","inventory_digest":blake3::hash(text.as_bytes()).to_hex().to_string()}))).unwrap();
}
#[test]
fn inconsistent_graph_claims_do_not_become_facts_even_with_a_matching_marker() {
    let ir = format!(
        "define private i64 @private_leaf(i64 %x) {{\n ret i64 %x\n}}\ndefine i64 @{LEAF}(i64 %x) {{\n %v = call i64 @private_leaf(i64 %x)\n ret i64 %v\n}}\n"
    );
    for variant in 0..11 {
        let (dir, names, runtime) = fixtures(&ir);
        rewrite(dir.path(), |value| {
            let f = &mut value["inventory"]["functions"][1];
            match variant {
                0 => f["transitive_classes"] = serde_json::json!([300]),
                1 => f["dependencies"] = serde_json::json!([]),
                2 => f["calls"][0]["abi"]["result"] = serde_json::json!("double"),
                3 => f["abi"] = serde_json::Value::Null,
                4 => f["direct_classes"] = serde_json::json!([301]),
                5 => {
                    f.as_object_mut().unwrap().remove("transitive_classes");
                }
                6 => f["complete_body"] = serde_json::json!(false),
                7 => f["body_digest"] = serde_json::json!("not a digest"),
                8 => f["local"] = serde_json::json!(true),
                9 => f["extra"] = serde_json::json!(true),
                _ => value["inventory"]["scope"] = serde_json::json!("another schema"),
            }
        });
        assert!(
            Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none(),
            "variant {variant}"
        );
    }
}

#[test]
fn weak_or_ambiguous_definitions_and_runtime_interposition_are_not_authorized() {
    for prefix in ["weak ", "linkonce_odr "] {
        let ir = format!("define {prefix}i64 @{LEAF}() {{\n ret i64 1\n}}\n");
        let (dir, names, runtime) = fixtures(&ir);
        let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(!i.call(LEAF, &[], Type::I64));
    }
    let ir = format!(
        "define i64 @{LEAF}() {{\n ret i64 1\n}}\ndefine i64 @{LEAF}() {{\n ret i64 2\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(!i.call(LEAF, &[], Type::I64));
    let ir = format!(
        "define i64 @{LEAF}() {{\n ret i64 1\n}}\ndefine i64 @dartforge_object_get(i64 %x, i64 %y) {{\n ret i64 0\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    let summary = crate::pruning::summarize(&ir, MODULE, &[]);
    assert!(
        i.reconcile(&[&summary], &HashSet::new(), &HashSet::new(), false)
            .is_err()
    );
}

#[test]
fn local_native_bytes_are_distinct_from_managed_or_public_images() {
    for (linkage, kind, known) in [
        ("private ", "constant [1 x i8] c\"x\"", true),
        ("", "constant [1 x i8] c\"x\"", false),
        ("private ", "constant [1 x i64] [i64 300]", false),
    ] {
        let ir =
            format!("@bytes = {linkage}{kind}\ndefine ptr @{LEAF}() {{\n ret ptr @bytes\n}}\n");
        let (dir, names, runtime) = fixtures(&ir);
        let i = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert_eq!(i.known_node("dart:test#bytes"), known);
        // A public byte global cannot be silently admitted as a graph root.
        assert!(!i.known_node("bytes"));
    }
}

#[test]
#[ignore = "requires the source SDK, native runtime and Clang"]
fn actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup() {
    let options = crate::driver::NativeDriverOptions::default();
    let sdk = crate::sdk_module::compiled_sdk_for(
        crate::sdk_tests(),
        &options.clang,
        crate::sdk_module::SdkProfile::Production,
        crate::DartFlags::PRODUCTION,
    )
    .unwrap();
    let directory = sdk.objects[0].parent().unwrap();
    let names: Vec<_> = sdk
        .objects
        .iter()
        .zip(&sdk.summaries)
        .flat_map(|(object, summary)| {
            [
                object.file_name().unwrap().to_str().unwrap().to_string(),
                summary.file_name().unwrap().to_str().unwrap().to_string(),
            ]
        })
        .collect();
    let runtime = crate::cache::RuntimeCache::for_production()
        .unwrap()
        .lib_path;
    let inventory = Inventory::load(directory, &sdk.key, true, &names, &runtime)
        .expect("selected SDK inventory must be readable");
    let summaries = crate::pruning::read_summaries(&sdk.summaries).unwrap();
    let refs: Vec<_> = summaries.iter().map(|s| s.as_ref()).collect();
    assert!(
        inventory
            .reconcile(&refs, &HashSet::new(), &HashSet::new(), false)
            .is_ok()
    );
    let (unique, known, pure) = inventory.counts();
    let (image_strings, image_cids) = inventory.image_string_counts();
    let clean_source_images = inventory.clean_source_image_count();
    assert!(clean_source_images <= crate::sdk_module::SOURCE_LIBRARIES.len());
    assert!(image_strings > 0);
    assert!(image_cids.iter().all(|cid| {
        [
            dartforge_runtime::layout::cid::ONE_BYTE_STRING as u32,
            dartforge_runtime::layout::cid::TWO_BYTE_STRING as u32,
        ]
        .contains(cid)
    }));
    assert!(unique > 0 && known > 0 && pure > 0);
    let declarations = inventory.table_declarations.len();
    assert!(
        declarations > 0,
        "selected SDK has verified virtual table declarations"
    );
    for (symbol, cid) in &inventory.table_declarations {
        assert!(inventory.table_declaration(*cid, symbol));
        assert!(!inventory.call(symbol, &[], Type::Ptr));
        assert!(!inventory.known_node(symbol));
    }
    let startup = crate::sdk_module::sdk_records();
    #[cfg(feature = "embedded-llvm")]
    super::bitcode_tests::compare_selected_objects(
        directory,
        &sdk.key,
        &names,
        &runtime,
        &sdk.objects,
    );
    assert!(!startup.is_empty());
    let certified: Vec<_> = startup
        .iter()
        .filter(|symbol| inventory.entry(symbol))
        .cloned()
        .collect();
    let expected: Vec<_> = [
        "collection",
        "math",
        "convert",
        "typed_data",
        "developer",
        "nativewrappers",
        "_http",
        "mirrors",
    ]
    .iter()
    .map(|name| crate::sdk_module::registration_symbol(&format!("dart:{name}")))
    .collect();
    assert_eq!(
        certified, expected,
        "only nominal/text-only startup bodies are certified"
    );
    assert!(
        !inventory.entry(&startup[0]),
        "the SDK image owner still needs coverage"
    );
    let selected =
        super::super::publication::read_verified(directory, &sdk.key, true, &names, &runtime)
            .unwrap();
    let functions = selected["inventory"]["functions"].as_array().unwrap();
    // The reserved display pair has its own entry-only certificate. It is
    // still absent from the nominal universe and ordinary call capabilities.
    let developer = crate::sdk_module::registration_symbol("dart:developer");
    assert!(inventory.entry(&developer));
    assert!(!inventory.call(&developer, &[], Type::Void));
    let developer_summary = summaries
        .iter()
        .find(|summary| summary.module == "dart:developer")
        .unwrap();
    let developer_record = functions
        .iter()
        .find(|record| record["symbol"] == developer)
        .unwrap();
    let frame_name = developer_record["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|call| {
            call["symbol"] == "dartforge_register_class_name"
                && call["arguments"][0]
                    .as_str()
                    .and_then(|arg| arg.strip_prefix("i64 "))
                    .and_then(|cid| cid.parse::<i64>().ok())
                    .is_some_and(|cid| !developer_summary.nominal_classes.contains(&cid))
        })
        .expect("generated frame remains outside nominal declarations");
    let frame_global = frame_name["arguments"][1]
        .as_str()
        .unwrap()
        .strip_prefix("ptr @")
        .unwrap();
    assert!(
        selected["inventory"]["globals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|global| global["module"] == "dart:developer"
                && global["symbol"] == frame_global
                && global["initializer"]
                    .as_str()
                    .is_some_and(|text| text.ends_with("c\"_AsyncFrame\"")))
    );
    for (index, symbol) in startup.iter().enumerate() {
        let records: Vec<_> = functions
            .iter()
            .filter(|record| record["symbol"] == *symbol)
            .collect();
        assert_eq!(records.len(), 1);
        let calls = records[0]["calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call["symbol"] == "dartforge_register_image")
            .count();
        assert_eq!(calls, usize::from(index == 0), "image owner: {symbol}");
    }
    assert!(
        startup
            .iter()
            .any(|symbol| !inventory.call(symbol, &[], Type::Void))
    );
    println!(
        "{}",
        serde_json::json!({"key":sdk.key,"unique_definitions":unique,"allocation_closures":known,"reviewed_empty_closures":pure,"startup_complete":false,"metadata_startups":certified,"image_registrations":1,"table_declarations":declarations,"checked_static_strings":image_strings,"image_string_cids":image_cids,"clean_source_images":clean_source_images})
    );
}

#[test]
fn reconciliation_reports_the_rejected_snapshot_symbol_or_class_boundary() {
    let ir = format!("define i64 @{LEAF}() {{\n ret i64 1\n}}\n");
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    let summary = crate::pruning::summarize(&ir, MODULE, &[]);
    let empty = HashSet::new();
    assert_eq!(
        inventory
            .reconcile(&[], &empty, &HashSet::new(), false)
            .unwrap_err(),
        "module count differs: selected 0, inventory 1"
    );
    let mut duplicate = inventory.clone();
    duplicate.summaries.push(summary.clone());
    assert_eq!(
        duplicate
            .reconcile(&[&summary, &summary], &empty, &HashSet::new(), false)
            .unwrap_err(),
        "module dart:test is selected more than once"
    );
    let mut changed = summary.clone();
    changed.definitions[0].refs.push(0);
    assert_eq!(
        inventory
            .reconcile(&[&changed], &empty, &HashSet::new(), false)
            .unwrap_err(),
        "module dart:test differs from the verified snapshot"
    );
    assert_eq!(
        inventory
            .reconcile(
                &[&summary],
                &HashSet::from([LEAF.into()]),
                &HashSet::new(),
                false
            )
            .unwrap_err(),
        format!("SDK definition {LEAF} overlaps the local image")
    );
    for (symbol, owner) in [
        ("dartforge_object_get", "native runtime"),
        ("llvm.unreviewed", "LLVM intrinsic namespace"),
    ] {
        let mut reserved = inventory.clone();
        reserved.definitions.insert(symbol.into());
        assert_eq!(
            reserved
                .reconcile(&[&summary], &empty, &HashSet::new(), false)
                .unwrap_err(),
            format!("SDK definition {symbol} overlaps the {owner}")
        );
    }
    for kind in ["table", "nominal class", "ambiguous class"] {
        for cid in [7, -1, i64::from(u32::MAX) + 1] {
            let mut selected = summary.clone();
            match kind {
                "table" => selected.tables.push(crate::pruning::SummarizedTable {
                    node: 0,
                    cid,
                    pairs: vec![],
                }),
                "nominal class" => selected.nominal_classes.push(cid),
                _ => selected.ambiguous_classes.push(cid),
            }
            let mut bound = inventory.clone();
            bound.summaries = vec![selected.clone()];
            let reason = bound
                .reconcile(&[&selected], &empty, &HashSet::from([7]), false)
                .unwrap_err();
            assert_eq!(
                reason,
                if cid == 7 {
                    format!("SDK dart:test {kind} 7 overlaps a local class")
                } else {
                    format!("SDK dart:test {kind} has invalid class id {cid}")
                }
            );
            if cid == 7 {
                assert!(
                    bound
                        .reconcile(&[&selected], &empty, &HashSet::from([8]), false)
                        .is_ok()
                );
            }
        }
    }
    let mut deterministic = inventory.clone();
    deterministic
        .definitions
        .extend(["llvm.z".into(), "llvm.a".into()]);
    for _ in 0..8 {
        assert_eq!(
            deterministic
                .reconcile(&[&summary], &empty, &HashSet::new(), false)
                .unwrap_err(),
            "SDK definition llvm.a overlaps the LLVM intrinsic namespace"
        );
    }
}

#[test]
fn observed_call_operands_require_matching_abi_syntax_even_with_recomputed_integrity() {
    let ir = format!(
        "define private i64 @leaf(i64 %x) {{\n ret i64 %x\n}}\ndefine i64 @{LEAF}(i64 %x) {{\n %v = call i64 @leaf(i64 %x)\n ret i64 %v\n}}\n"
    );
    for variant in 0..6 {
        let (dir, names, runtime) = fixtures(&ir);
        rewrite(dir.path(), |value| {
            let call = &mut value["inventory"]["functions"][1]["calls"][0];
            match variant {
                0 => {
                    call.as_object_mut().unwrap().remove("arguments");
                }
                1 => call["arguments"] = serde_json::json!([]),
                2 => call["arguments"] = serde_json::json!(["double %x"]),
                3 => call["arguments"] = serde_json::json!(["i64 %x, i64 0"]),
                4 => call["arguments"] = serde_json::json!(["i64 @\"bad\\ZZ\""]),
                _ => call["arguments"] = serde_json::json!(["i64 %x) call void @opaque() ("]),
            }
        });
        assert!(super::super::publication::verify(
            dir.path(),
            "test-key",
            true,
            &names,
            &runtime
        ));
        assert!(
            Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none(),
            "variant {variant}"
        );
    }
}

#[test]
fn previous_inventory_schema_does_not_supply_missing_operand_observations() {
    let ir = format!("define i64 @{LEAF}() {{\n ret i64 1\n}}\n");
    let (dir, names, runtime) = fixtures(&ir);
    rewrite(dir.path(), |value| {
        value["schema"] = serde_json::json!(super::super::publication::SCHEMA - 1)
    });
    let ready = dir.path().join("allocation-inventory.ready");
    let mut marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ready).unwrap()).unwrap();
    marker["schema"] = serde_json::json!(super::super::publication::SCHEMA - 1);
    std::fs::write(ready, format!("{marker}\n")).unwrap();
    assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
}

#[test]
fn quoted_scalar_ssa_operands_can_close_a_verified_local_call() {
    let ir = format!(
        "define private i64 @leaf(i64 %\"value,(scalar)\") {{\n ret i64 7\n}}\ndefine i64 @{LEAF}(i64 %\"value,(scalar)\") {{\n %v = call i64 @leaf(i64 %\"value,(scalar)\")\n ret i64 %v\n}}\n"
    );
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.call(LEAF, &[Type::I64], Type::I64));
    assert!(!inventory.call(LEAF, &[Type::F64], Type::I64));
}

#[test]
fn llvm_scalar_results_are_not_managed_reference_or_packet_certificates() {
    for (result, value, requested) in [("i64", "2", Type::Ref), ("ptr", "null", Type::Ptr)] {
        let ir = format!("define {result} @{LEAF}() {{\n ret {result} {value}\n}}\n");
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(
            inventory.known_node(LEAF),
            "the allocation-free observation remains valid"
        );
        assert!(
            !inventory.call(LEAF, &[], requested),
            "{requested:?} requires a value/region proof"
        );
        if result == "i64" {
            assert!(inventory.call(LEAF, &[], Type::I64));
        }
    }
}

#[path = "tests/frame_names.rs"]
mod frame_names;

#[path = "tests/startup_diagnostics.rs"]
mod startup_diagnostics;

#[path = "tests/proof_views.rs"]
mod proof_views;

mod table_allocations;

#[path = "tests/context_lookups.rs"]
mod context_lookups;

#[path = "tests/read_only.rs"]
mod read_only;
