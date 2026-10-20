//! Empty allocation footprints cannot certify opaque input/output regions.
use super::*;

#[test]
fn read_only_memory_footprints_do_not_authorize_pointer_or_reference_imports() {
    let ir = format!(
        "define i64 @{LEAF}(ptr %p) {{\n %field = getelementptr i8, ptr %p, i64 16\n %value = load i64, ptr %field, align 8\n ret i64 %value\n}}\n"
    );
    let raw = super::super::super::closure::audit(&[(MODULE, &ir)]).to_json();
    assert_eq!(
        raw["functions"][0]["transitive_classes"],
        serde_json::json!([])
    );
    let (dir, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.contains_definition(LEAF));
    assert!(!inventory.known_node(LEAF));
    assert!(!inventory.entry(LEAF));
    for result in [Type::Ptr, Type::Ref, Type::I64] {
        assert!(!inventory.call(LEAF, &[Type::Ptr], result));
    }
}

#[test]
fn stack_read_footprints_never_authorize_machine_addresses_as_imports() {
    for register in ["sp", "rsp"] {
        let ir = format!(
            "define i64 @{LEAF}() {{\n %sp = call i64 @llvm.read_register.i64(metadata !{{!\"{register}\"}})\n ret i64 %sp\n}}\n"
        );
        for remove_exposure in [false, true] {
            let (dir, names, runtime) = fixtures(&ir);
            if remove_exposure {
                rewrite(dir.path(), |value| {
                    value["inventory"]["functions"][0]["exposure_obligations"] =
                        serde_json::json!([]);
                });
            }
            let inventory =
                Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
            assert!(!inventory.known_node(LEAF));
            assert!(!inventory.entry(LEAF));
            for result in [Type::I64, Type::Ref, Type::Ptr] {
                assert!(!inventory.call(LEAF, &[], result));
            }
        }
    }
}

#[test]
fn stack_read_metadata_is_not_a_general_published_abi_type() {
    let ir = format!(
        "define i64 @{LEAF}() {{\n %sp = call i64 @llvm.read_register.i64(metadata !{{!\"sp\"}})\n ret i64 %sp\n}}\n"
    );
    for variant in 0..4 {
        let (dir, names, runtime) = fixtures(&ir);
        rewrite(dir.path(), |value| {
            let call = &mut value["inventory"]["functions"][0]["calls"][0];
            match variant {
                0 => call["symbol"] = serde_json::json!("opaque"),
                1 => call["abi"]["result"] = serde_json::json!("ptr"),
                2 => call["arguments"][0] = serde_json::json!("metadata !{!\"x0\"}"),
                _ => call["abi"]["parameters"] = serde_json::json!(["ptr"]),
            }
        });
        assert!(Inventory::load(dir.path(), "test-key", true, &names, &runtime).is_none());
    }
}

#[test]
fn stack_storage_does_not_authorize_stack_addresses_or_invocations() {
    for body in [
        "%p = alloca i64\n ret ptr %p",
        "%p = alloca i64\n store i64 0, ptr %p\n ret ptr %p",
        "%p = alloca { ptr, i64, [3 x i64] }\n %q = getelementptr { ptr, i64, [3 x i64] }, ptr %p, i64 0, i32 2, i64 1\n store i64 0, ptr %q\n ret ptr %q",
    ] {
        let ir = format!("define ptr @{LEAF}() {{\n{body}\n}}\n");
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(!inventory.known_node(LEAF));
        assert!(!inventory.entry(LEAF));
        for result in [Type::I64, Type::Ref, Type::Ptr] {
            assert!(!inventory.call(LEAF, &[], result));
        }
    }
}
