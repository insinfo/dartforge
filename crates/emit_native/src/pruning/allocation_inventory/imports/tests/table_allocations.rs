//! A finite allocator footprint cannot authorize escaped method addresses.
use super::*;

#[test]
fn finite_table_allocations_never_become_pure_imports_or_entry_certificates() {
    for call in [
        "call i64 @dartforge_object_new_t(i64 900, i64 0, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_mt(i64 900, i64 0, i64 0, ptr @df.mt.example)",
    ] {
        let ir = format!(
            "@df.mt.example$d = private unnamed_addr constant {{ i64, i64 }} {{ i64 900, i64 0 }}\ndefine ptr @df.mt.example() nounwind {{\nb0:\n  ret ptr @df.mt.example$d\n}}\ndefine i64 @{LEAF}() {{\n %object = {call}\n ret i64 %object\n}}\n"
        );
        let bundle = super::super::super::closure::audit(&[(MODULE, &ir)]).to_json();
        assert_eq!(
            bundle["functions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|function| function["symbol"] == LEAF)
                .unwrap()["transitive_classes"],
            serde_json::json!([900])
        );
        let (dir, names, runtime) = fixtures(&ir);
        let inventory = Inventory::load(dir.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(!inventory.call(LEAF, &[], Type::Ref));
        assert!(!inventory.call(LEAF, &[], Type::I64));
        assert!(!inventory.known_node(LEAF));
        assert!(!inventory.entry(LEAF));
        assert!(!inventory.call("df.mt.example", &[], Type::Ptr));
        assert!(!inventory.known_node("df.mt.example"));
        assert!(!inventory.entry("df.mt.example"));
    }
}
