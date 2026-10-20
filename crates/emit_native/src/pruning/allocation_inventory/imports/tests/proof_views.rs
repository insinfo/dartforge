//! Verified byte contents and metadata edges have independent proof ownership.
use super::*;

fn references(summary: &Summary, symbol: &str) -> Vec<String> {
    let definition = summary
        .definitions
        .iter()
        .find(|definition| summary.names[definition.name as usize] == symbol)
        .unwrap();
    definition
        .refs
        .iter()
        .map(|node| summary.names[*node as usize].clone())
        .collect()
}

#[test]
fn byte_literal_edges_are_removed_without_any_metadata_startup() {
    let ir = "@bytes = private constant [7 x i8] c\"@opaque\"\n@address = private constant ptr @opaque\ndefine i64 @df.dart$3atest..leaf() {\n ret i64 7\n}\n";
    let (directory, names, runtime) = fixtures(ir);
    let inventory = Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
    assert!(inventory.metadata_startups.is_empty());
    assert!(!references(&inventory.summaries[0], "dart:test#bytes").is_empty());
    let proof = inventory.class_proof_summaries();
    assert!(references(&proof[0], "dart:test#bytes").is_empty());
    assert_eq!(references(&proof[0], "dart:test#address"), vec!["opaque"]);
    assert!(!references(&inventory.summaries[0], "dart:test#bytes").is_empty());
    assert!(!inventory.known_node("opaque"));
    assert!(!inventory.call("opaque", &[], Type::I64));
}

#[test]
fn unchanged_summaries_borrow_the_original_verified_storage() {
    let ir = "@bytes = private constant [3 x i8] c\"abc\"\n@public = constant [7 x i8] c\"@opaque\"\n@address = private constant ptr @opaque\ndefine i64 @df.dart$3atest..leaf() {\n ret i64 7\n}\n";
    let (directory, names, runtime) = fixtures(ir);
    let inventory = Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
    for _ in 0..2 {
        let proof = inventory.class_proof_summaries();
        assert!(matches!(&proof[0], std::borrow::Cow::Borrowed(_)));
        assert!(std::ptr::eq(proof[0].as_ref(), &inventory.summaries[0]));
        assert_eq!(proof[0].as_ref(), &inventory.summaries[0]);
        assert_eq!(references(&proof[0], "public"), vec!["opaque"]);
    }
}

#[test]
fn only_changed_modules_are_owned_and_other_calls_keep_metadata_edges() {
    let ir = format!(
        "{SUBTYPE_STARTUP}define void @df.dart$3atest..other() {{\n call void @dartforge_register_subclass(i64 900, i64 901)\n ret void\n}}\n"
    );
    let other = "define i64 @df.dart$3aother..leaf() {\n ret i64 9\n}\n";
    let (directory, mut names, runtime) = fixtures(&ir);
    std::fs::write(
        directory.path().join("other.o"),
        b"test object; not executable",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("other.poda"),
        crate::pruning::summarize(other, "dart:other", &[]).to_text(),
    )
    .unwrap();
    names.extend(["other.o".into(), "other.poda".into()]);
    let bundle = crate::pruning::allocation_inventory::closure::audit(&[
        (MODULE, &ir),
        ("dart:other", other),
    ]);
    crate::pruning::allocation_inventory::publication::publish(
        directory.path(),
        "test-key",
        true,
        &names,
        &runtime,
        &bundle,
    )
    .unwrap();
    let inventory = Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
    let originals = inventory.summaries.clone();
    let proof = inventory.class_proof_summaries();
    assert!(matches!(&proof[0], std::borrow::Cow::Owned(_)));
    assert!(matches!(&proof[1], std::borrow::Cow::Borrowed(_)));
    assert!(std::ptr::eq(proof[1].as_ref(), &inventory.summaries[1]));
    assert!(references(&proof[0], "df.register.dart$3atest").is_empty());
    assert_eq!(
        references(&proof[0], "df.dart$3atest..other"),
        vec!["dartforge_register_subclass"]
    );
    assert_eq!(inventory.summaries, originals);
    for (before, after) in inventory.summaries[0]
        .definitions
        .iter()
        .zip(&proof[0].definitions)
    {
        if inventory.summaries[0].names[before.name as usize] != "df.register.dart$3atest" {
            assert_eq!(before, after);
        }
    }
    assert_eq!(inventory.class_proof_summaries(), proof);
}

#[test]
fn rebuilding_the_inventory_observes_changed_byte_and_pointer_initializers() {
    for (initializer, cleared) in [("[7 x i8] c\"@opaque\"", true), ("ptr @opaque", false)] {
        let ir = format!("@data = private constant {initializer}\n");
        let (directory, names, runtime) = fixtures(&ir);
        let inventory =
            Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
        let proof = inventory.class_proof_summaries();
        assert_eq!(references(&proof[0], "dart:test#data").is_empty(), cleared);
        assert_eq!(matches!(&proof[0], std::borrow::Cow::Owned(_)), cleared);
        assert!(!references(&inventory.summaries[0], "dart:test#data").is_empty());
    }
}
