//! Stack subobjects do not grant a heap or invocation certificate.
use super::*;
fn observed(body: &str) -> Value {
    let ir = format!(
        "define i64 @df.dart$3atest..stack(i64 %value, ptr %unknown) {{\n{body}\n ret i64 0\n}}\n"
    );
    audit(&[("dart:test", &ir)]).to_json()["functions"][0].clone()
}
#[test]
fn stack_storage_observes_typed_local_writes_and_preserves_exposure() {
    for body in [
        "%p = alloca i64\n store i64 %value, ptr %p",
        "%p = alloca i64, i64 3\n %q = getelementptr i64, ptr %p, i64 2\n store i64 0, ptr %q",
        "%p = alloca [3 x i64]\n %q = getelementptr [3 x i64], ptr %p, i64 0, i64 2\n store i64 0, ptr %q",
        "%p = alloca [3 x i64]\n %q = getelementptr i64, ptr %p, i64 1\n %r = getelementptr i64, ptr %q, i64 1\n store i64 0, ptr %r",
        "%p = alloca { ptr, i64, [3 x i64] }, align 8\n %q = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %p, i64 0, i32 2, i64 2\n store i64 %value, ptr %q, align 8",
        "%\"p\" = alloca ptr\n store ptr %unknown, ptr %p",
    ] {
        let value = observed(body);
        assert_eq!(value["transitive_classes"], json!([]), "{body}");
        assert_eq!(
            value["exposure_obligations"],
            json!(["raw_memory_or_pointer_operation"])
        );
    }
}
#[test]
fn stack_storage_rejects_unbounded_or_mismatched_regions_and_hidden_effects() {
    for body in [
        "store i64 0, ptr %unknown",
        "%unknown = alloca i64\n store i64 0, ptr %unknown",
        "%p = alloca i64\n store i64 0, ptr %unknown",
        "%p = alloca i8\n store i64 0, ptr %p",
        "%p = alloca [3 x i64]\n %q = getelementptr i64, ptr %p, i64 3\n store i64 0, ptr %q",
        "%p = alloca [3 x i64]\n %q = getelementptr i64, ptr %p, i64 -1\n store i64 0, ptr %q",
        "%p = alloca [3 x i64]\n %q = getelementptr i64, ptr %p, i64 %value\n store i64 0, ptr %q",
        "%p = alloca i64, i64 %value\n store i64 0, ptr %p",
        "%p = alloca ptr\n %q = load ptr, ptr %p\n store i64 0, ptr %q",
        "%p = alloca [3 x i64]\n %q = getelementptr i8, ptr %p, i64 0\n store i64 0, ptr %q",
        "%p = alloca { ptr, [2 x i64] }\n %q = getelementptr { ptr, [2 x i64] }, ptr %p, i64 0, i32 1, i64 2\n store i64 0, ptr %q",
        "%p = alloca i64\n %\"p\" = inttoptr i64 %value to ptr\n store i64 0, ptr %p",
        "%p = alloca i64\n store i64 0, ptr %p store i64 0, ptr %unknown",
        "%p = alloca i64\n store atomic i64 0, ptr %p monotonic, align 8",
        "%p = alloca i64\n store i64 0, ptr %p\n call void @opaque()",
    ] {
        assert!(observed(body)["transitive_classes"].is_null(), "{body}");
    }
}
#[test]
fn stack_storage_alloca_itself_retains_pointer_exposure() {
    let value = observed("%p = alloca i64");
    assert_eq!(value["transitive_classes"], json!([]));
    assert_eq!(
        value["exposure_obligations"],
        json!(["raw_memory_or_pointer_operation"])
    );
}

#[test]
fn stack_storage_finite_callees_and_budgets_remain_conservative() {
    let value = observed(
        "%p = alloca i64\n store i64 0, ptr %p\n %n = call i64 @dartforge_object_new(i64 900, i64 1)",
    );
    assert_eq!(value["transitive_classes"], json!([900]));
    for body in [
        "%p = alloca [1048577 x i64]\n store i64 0, ptr %p".to_string(),
        format!(
            "%p = alloca i64\n{}\nstore i64 0, ptr %p",
            "\n".repeat(8193)
        ),
        format!(
            "%p = alloca {}i64{}\n store i64 0, ptr %p",
            "[1 x ".repeat(18),
            "]".repeat(18)
        ),
    ] {
        assert!(observed(&body)["transitive_classes"].is_null());
    }
}
