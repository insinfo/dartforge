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
