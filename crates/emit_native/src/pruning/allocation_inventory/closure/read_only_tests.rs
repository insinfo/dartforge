//! Reading an existing object is not an allocation or a value-region proof.
use super::*;

fn function(body: &str) -> Value {
    let ir = format!(
        "define i64 @df.dart$3atest..read(ptr %p, i64 %address) {{\n{body}\n ret i64 0\n}}\n"
    );
    audit(&[("dart:test", &ir)]).to_json()["functions"][0].clone()
}

#[test]
fn read_only_memory_observes_empty_allocations_and_preserves_pointer_exposure() {
    for body in [
        " %x = load i64, ptr %p, align 8",
        " %x = load i64, ptr %\"p\", align 8",
        " %x = load volatile i8, ptr %p, align 1",
        " %x = getelementptr inbounds i8, ptr %p, i64 8",
        " %x = inttoptr i64 %address to ptr",
        " %x = ptrtoint ptr %p to i64",
        " %x = bitcast i64 %address to double",
        " %q = getelementptr i8, ptr %p, i64 %address\n %x = load ptr, ptr %q, align 8",
    ] {
        let data = function(body);
        assert_eq!(data["transitive_classes"], json!([]), "{body}");
        assert!(
            data["exposure_obligations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == "raw_memory_or_pointer_operation")
        );
    }
}

#[test]
fn read_only_memory_does_not_hide_writes_allocations_or_unknown_boundaries() {
    for body in [
        " %x = load i64, ptr %p\n store i64 900, ptr %p",
        " %x = load i64, ptr %p store i64 900, ptr %p",
        " %x = load i64, ptr %\"p\"store i64 900, ptr %p",
        " %x = add i64 0, 0 store i64 900, ptr %p",
        " %x = add i64 0, 0store i64 900, ptr %p",
        " store i64 900, ptr %p %x = load i64, ptr %p",
        " %n = add i64 0, 0 %x = load i64, ptr %p",
        " %x = load i64, ptr %p\n %n = call i64 @dartforge_object_new(i64 900, i64 1)\n store i64 900, ptr %p",
        " %x = load i64, ptr %p\n call void @opaque()",
        " %x = load i64, ptr %p\n %n = call i64 %callback()",
        " %x = load i64, ptr %p\n %n = atomicrmw add ptr %p, i64 1 seq_cst",
        " %x = add i64 0, 0 atomicrmw add ptr %p, i64 1 seq_cst",
        " %x = add i64 0, 0 call void @opaque()",
        " %x = load i64, ptr %p\n %n = cmpxchg ptr %p, i64 0, i64 1 seq_cst seq_cst",
        " %x = load i64, ptr @managed_image",
        " %x = load i64, ptr %p, align 3",
    ] {
        assert!(function(body)["transitive_classes"].is_null(), "{body}");
    }
}

#[test]
fn read_only_memory_callees_still_contribute_finite_or_unknown_allocations() {
    let ir = "define i64 @read(ptr %p) {\n %x = load i64, ptr %p, align 8\n ret i64 %x\n}\ndefine i64 @allocate(ptr %p) {\n %x = call i64 @read(ptr %p)\n %n = call i64 @dartforge_object_new(i64 900, i64 1)\n ret i64 %n\n}\n";
    let data = audit(&[("dart:test", ir)]).to_json();
    assert_eq!(data["functions"][0]["transitive_classes"], json!([]));
    assert_eq!(data["functions"][1]["transitive_classes"], json!([900]));
    let ir = ir.replace("ret i64 %x", "call void @opaque()\n ret i64 %x");
    let data = audit(&[("dart:test", &ir)]).to_json();
    assert!(
        data["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["transitive_classes"].is_null())
    );
}

#[test]
fn read_only_memory_hidden_writes_are_valid_llvm_but_not_empty_footprints() {
    let ir = "define i64 @arithmetic(ptr %p) {\n %x = add i64 0, 0store i64 900, ptr %p\n ret i64 %x\n}\ndefine i64 @quoted(ptr %p) {\n %x = load i64, ptr %\"p\"store i64 900, ptr %p\n ret i64 %x\n}\ndefine i64 @unnamed(ptr %0) {\n %x = load i64, ptr %0store i64 900, ptr %0\n ret i64 %x\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("writes.ll");
    let object = dir.path().join("writes.o");
    std::fs::write(&source, ir).unwrap();
    let output = std::process::Command::new(crate::driver::NativeDriverOptions::default().clang)
        .args(["-c", "-x", "ir"])
        .arg(source)
        .arg("-o")
        .arg(object)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let data = audit(&[("dart:test", ir)]).to_json();
    assert!(
        data["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["transitive_classes"].is_null())
    );
}
