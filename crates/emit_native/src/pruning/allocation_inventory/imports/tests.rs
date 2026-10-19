use super::*;

const MODULE: &str = "dart:test";
const LEAF: &str = "df.dart$3atest..leaf";
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
fn verified_scalar_closure_requires_its_actual_abi_and_graph_snapshot() {
    let ir = format!("define i64 @{LEAF}(i64 %x) {{\n %v = add i64 %x, 1\n ret i64 %v\n}}\n");
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.call(LEAF, &[Type::I64], Type::I64));
    assert!(!inventory.call(LEAF, &[], Type::I64));
    assert!(!inventory.call(LEAF, &[Type::F64], Type::I64));
    let summary = crate::pruning::summarize(&ir, MODULE, &[]);
    assert!(inventory.reconcile(&[&summary], &HashSet::new(), &HashSet::new()));
    let mut changed = summary.clone();
    changed.definitions[0].refs.push(0);
    assert!(!inventory.reconcile(&[&changed], &HashSet::new(), &HashSet::new()));
    assert!(!inventory.reconcile(
        &[&summary],
        &HashSet::from([LEAF.to_string()]),
        &HashSet::new()
    ));
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
    std::fs::write(dir.join("allocation-inventory.ready"),format!("{}\n",serde_json::json!({"schema":1,"cache_key":"test-key","inventory_digest":blake3::hash(text.as_bytes()).to_hex().to_string()}))).unwrap();
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
    assert!(!i.reconcile(&[&summary], &HashSet::new(), &HashSet::new()));
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
    assert!(inventory.reconcile(&refs, &HashSet::new(), &HashSet::new()));
    let (unique, known, pure) = inventory.counts();
    assert!(unique > 0 && known > 0 && pure > 0);
    let startup = crate::sdk_module::sdk_records();
    assert!(!startup.is_empty());
    assert!(
        startup
            .iter()
            .any(|symbol| !inventory.call(symbol, &[], Type::Void))
    );
    println!(
        "{}",
        serde_json::json!({"key":sdk.key,"unique_definitions":unique,"allocation_closures":known,"reviewed_empty_closures":pure,"startup_complete":false})
    );
}
