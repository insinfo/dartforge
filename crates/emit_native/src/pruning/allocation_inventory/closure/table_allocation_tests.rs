//! Table callbacks need a complete constant-storage witness, not a symbol hint.
use super::*;

pub(crate) const TABLE: &str = "@df.mt.example$d = private unnamed_addr constant { i64, i64, i64, ptr } { i64 900, i64 1, i64 7, ptr @method }\ndefine ptr @df.mt.example() nounwind {\nb0:\n  ret ptr @df.mt.example$d\n}\n";

fn program(call: &str) -> String {
    format!(
        "{TABLE}define i64 @df.dart$3atest..allocate() {{\n %object = {call}\n ret i64 %object\n}}\ndefine i64 @df.dart$3atest..wrapper() {{\n %object = call i64 @df.dart$3atest..allocate()\n ret i64 %object\n}}\n"
    )
}
fn footprint(ir: &str, name: &str) -> serde_json::Value {
    audit(&[("dart:test", ir)]).to_json()["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|function| function["symbol"] == name)
        .unwrap()["transitive_classes"]
        .clone()
}

#[test]
fn constant_table_callbacks_have_finite_allocation_but_preserve_exposure() {
    for call in [
        "call i64 @dartforge_object_new_t(i64 900, i64 2, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_mt(i64 900, i64 2, i64 3, ptr @df.mt.example)",
    ] {
        let ir = program(call);
        assert_eq!(footprint(&ir, "df.dart$3atest..allocate"), json!([900]));
        assert_eq!(footprint(&ir, "df.dart$3atest..wrapper"), json!([900]));
        assert_eq!(footprint(&ir, "df.mt.example"), json!([]));
        let bundle = audit(&[("dart:test", &ir)]).to_json();
        let functions = bundle["functions"].as_array().unwrap();
        assert!(
            functions
                .iter()
                .filter(|function| function["symbol"] != "df.dart$3atest..wrapper")
                .all(|function| !function["exposure_obligations"]
                    .as_array()
                    .unwrap()
                    .is_empty())
        );
    }
}

#[test]
fn unknown_callbacks_and_invalid_allocation_operands_remain_open() {
    for call in [
        "call i64 @dartforge_object_new_t(i64 901, i64 2, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_t(i64 %cid, i64 2, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_t(i64 900, i64 -1, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_t(i64 900, i64 2, ptr %getter)",
        "call i64 @dartforge_object_new_t(i64 900, i64 2, ptr @unknown)",
        "call i64 @dartforge_object_new_mt(i64 900, i64 2, i64 -1, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_mt(i64 900, i64 2, i64 4294967296, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_mt(i64 900, i64 2, i64 %map, ptr @df.mt.example)",
        "call i64 @dartforge_object_new_t(i32 900, i64 2, ptr @df.mt.example)",
    ] {
        assert!(
            footprint(&program(call), "df.dart$3atest..allocate").is_null(),
            "{call}"
        );
    }
}

#[test]
fn changed_callback_bodies_storage_and_symbol_ownership_are_not_certified() {
    let valid = program("call i64 @dartforge_object_new_t(i64 900, i64 2, ptr @df.mt.example)");
    for ir in [
        valid.replace("  ret ptr", "  call void @opaque()\n  ret ptr"),
        valid.replace("  ret ptr @df.mt.example$d", "  ret ptr null"),
        valid.replace("define ptr", "define weak ptr"),
        valid.replace(
            "private unnamed_addr constant",
            "private unnamed_addr global",
        ),
        valid.replace("i64 900, i64 1", "i64 900, i64 2"),
        valid.replace("i64 900, i64 1", "i64 -1, i64 1"),
        valid.replace("ptr @method", "ptr null"),
        valid.replace("{ i64, i64, i64, ptr }", "{ i64, i64, i64, i64 }"),
        valid.replace("  ret ptr @df.mt.example$d", "  ret ptr @different"),
        format!("{valid}{TABLE}"),
        format!("{valid}@df.mt.example = alias ptr (), ptr @other\n"),
        format!("{valid}@\"df.mt.example$d\" = external global i64\n"),
        format!("{valid}@\"df.mt.\\65xample$d\" = external global i64\n"),
        format!("{valid}@\"df.mt.\\65xample\" = alias ptr (), ptr @other\n"),
        format!("{valid}define ptr @\"df.mt.\\65xample\"() {{ ret ptr null }}\n"),
        format!("{valid}define ptr @df.mt.example$d() {{ ret ptr null }}\n"),
    ] {
        assert!(footprint(&ir, "df.dart$3atest..allocate").is_null(), "{ir}");
    }
    let bundle = audit(&[("dart:test", &valid), ("dart:other", TABLE)]).to_json();
    assert!(
        bundle["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| function["symbol"] == "df.dart$3atest..allocate")
            .unwrap()["transitive_classes"]
            .is_null()
    );
}

#[test]
fn table_entries_are_complete_ordered_pairs_and_empty_tables_are_valid() {
    let valid = program("call i64 @dartforge_object_new_t(i64 900, i64 2, ptr @df.mt.example)");
    let empty = valid.replace(
        "{ i64, i64, i64, ptr } { i64 900, i64 1, i64 7, ptr @method }",
        "{ i64, i64 } { i64 900, i64 0 }",
    );
    assert_eq!(footprint(&empty, "df.dart$3atest..allocate"), json!([900]));
    let two = valid.replace("{ i64, i64, i64, ptr } { i64 900, i64 1, i64 7, ptr @method }", "{ i64, i64, i64, ptr, i64, ptr } { i64 900, i64 2, i64 -7, ptr @first, i64 9, ptr @second }");
    assert_eq!(footprint(&two, "df.dart$3atest..allocate"), json!([900]));
    for invalid in [
        two.replace("i64 9", "i64 -7"),
        two.replace("i64 9", "i64 -9"),
    ] {
        assert!(footprint(&invalid, "df.dart$3atest..allocate").is_null());
    }
}

#[test]
fn actual_emitter_table_bodies_supply_the_same_bounded_storage_witness() {
    use crate::hir::Module;
    let mut module = Module {
        sdk_library: true,
        record: Some("df.register.example".into()),
        ..Module::default()
    };
    module
        .method_tables
        .push((900, "df.mt.example".into(), vec![]));
    let plain = crate::llvm::LlvmEmitter::new(&module).emit_all();
    let no_unwind = crate::sdk_module::with_nounwind(&plain);
    for ir in [plain, no_unwind] {
        assert_eq!(
            table_getters::observe(&[("program", &ir)]).get("df.mt.example"),
            Some(&900)
        );
        let caller = "define i64 @allocate() {\n %object = call i64 @dartforge_object_new_t(i64 900, i64 0, ptr @df.mt.example)\n ret i64 %object\n}\n";
        assert_eq!(
            footprint(&format!("{ir}\n{caller}"), "allocate"),
            json!([900])
        );
    }
}
