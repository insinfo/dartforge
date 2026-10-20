//! An allocation-neutral context lookup still exposes an unproved pointer.
use super::*;

#[test]
fn context_lookup_footprints_do_not_authorize_imports_or_runtime_entrypoints() {
    for helper in [
        crate::sdk_module::context_in_tsd("", true),
        format!(
            "{}declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)\n",
            crate::sdk_module::context_in_native_tls("cache", "")
        ),
    ] {
        let ir = format!(
            "{helper}define ptr @{LEAF}() {{\n %c = call ptr @df.context()\n ret ptr %c\n}}\n"
        );
        let raw = super::super::super::closure::audit(&[(MODULE, &ir)]).to_json();
        assert!(
            raw["functions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["transitive_classes"] == serde_json::json!([]))
        );
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        for name in ["df.context", LEAF] {
            assert!(!inventory.known_node(name));
            assert!(!inventory.entry(name));
            for result in [Type::Ptr, Type::Ref, Type::I64] {
                assert!(!inventory.call(name, &[], result));
            }
        }
    }
}
