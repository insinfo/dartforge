//! Stack register reads carry machine-state exposure, not import authority.
use super::*;

fn observation(operation: &str, extra: &str) -> Value {
    let ir = format!(
        "define i64 @df.dart$3atest..read() {{\n %sp = {operation}\n ret i64 %sp\n}}\n{extra}"
    );
    audit(&[("dart:test", &ir)]).to_json()["functions"][0].clone()
}

#[test]
fn stack_read_preserves_the_exact_metadata_call_and_exposure() {
    for register in ["sp", "rsp"] {
        let argument = format!("metadata !{{!\"{register}\"}}");
        let value = observation(&format!("call i64 @llvm.read_register.i64({argument})"), "");
        assert_eq!(value["transitive_classes"], json!([]));
        assert_eq!(
            value["calls"],
            json!([{"symbol":"llvm.read_register.i64","abi":{"result":"i64","parameters":["metadata"]},"arguments":[argument]}])
        );
        assert_eq!(
            value["exposure_obligations"],
            json!(["machine_register_read"])
        );
    }
}

#[test]
fn stack_read_does_not_generalize_registers_abi_writes_or_shadowed_names() {
    for operation in [
        "call i32 @llvm.read_register.i32(metadata !{!\"sp\"})",
        "call i64 @llvm.read_register.i64(metadata !{!\"x0\"})",
        "call i64 @llvm.read_register.i64(metadata !0)",
        "call i64 @llvm.read_register.i64(metadata !{!\"sp\"}, i64 0)",
        "call i64 @llvm.read_register.i64(metadata !{!\"sp\"}) store i64 0, ptr null",
        "call i64 @llvm.read_volatile_register.i64(metadata !{!\"sp\"})",
        "call i64 %callback(metadata !{!\"sp\"})",
    ] {
        assert!(
            observation(operation, "")["transitive_classes"].is_null(),
            "{operation}"
        );
    }
    let value = observation(
        "call i64 @llvm.read_register.i64(metadata !{!\"sp\"})",
        "define i64 @llvm.read_register.i64(ptr %p) { ret i64 0 }\n",
    );
    assert!(value["transitive_classes"].is_null());
}
