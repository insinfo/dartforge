//! Context allocation facts never imply pointer purity or runtime invocation.
use super::*;

fn tsd(attribute: &str, single_image: bool) -> String {
    crate::sdk_module::context_in_tsd(attribute, single_image)
}

fn tls(variable: &str, attribute: &str) -> String {
    format!(
        "{}declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)\n",
        crate::sdk_module::context_in_native_tls(variable, attribute)
    )
}

fn context(ir: &str) -> Value {
    audit(&[("dart:test", ir)]).to_json()["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["symbol"] == "df.context")
        .unwrap()
        .clone()
}

#[test]
fn context_lookup_exact_tsd_bodies_have_empty_footprints_but_keep_exposure() {
    for attribute in ["", " alwaysinline", " noinline"] {
        for single_image in [false, true] {
            let f = context(&tsd(attribute, single_image));
            assert_eq!(f["transitive_classes"], json!([]));
            assert_eq!(f["calls"], json!([]));
            assert!(!f["exposure_obligations"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn context_lookup_exact_tls_bodies_preserve_native_calls() {
    for variable in ["df.ctx.tls", "df.ctx.tls.program", "other.cache"] {
        for attribute in ["", " alwaysinline", " noinline"] {
            let ir = tls(variable, attribute);
            let f = context(&ir);
            assert_eq!(f["transitive_classes"], json!([]));
            assert_eq!(f["calls"].as_array().unwrap().len(), 2);
            assert_eq!(f["calls"][0]["symbol"], "llvm.threadlocal.address.p0");
            assert_eq!(f["calls"][1]["symbol"], "dartforge_context");
            assert!(!f["exposure_obligations"].as_array().unwrap().is_empty());
            assert_eq!(
                context(&ir.replace(
                    "declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)",
                    "declare ptr @llvm.threadlocal.address.p0(ptr)"
                ))["transitive_classes"],
                json!([])
            );
        }
    }
}

#[test]
fn context_lookup_changed_bodies_and_storage_remain_unknown() {
    let a = tsd("", true);
    let b = tls("cache", "");
    for ir in [
        a.replace(
            "ret ptr %c",
            "call i64 @dartforge_object_new(i64 900, i64 0)\n  ret ptr %c",
        ),
        a.replace("mrs $0, TPIDRRO_EL0", "mov $0, #0"),
        a.replace("asm ", "asm sideeffect "),
        a.replace(
            "external dso_local global i64, align 8",
            "external global i32, align 4",
        ),
        a.lines()
            .filter(|line| !line.starts_with("@dartforge_context_slot"))
            .collect::<Vec<_>>()
            .join("\n"),
        a.replace("define internal ptr", "define ptr"),
        b.replace("thread_local global", "global"),
        b.replace("store ptr %n, ptr %p", "store ptr %n, ptr %n"),
        b.replace("call ptr @dartforge_context()", "call ptr @opaque()"),
        b.replace(
            "declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)",
            "declare i64 @llvm.threadlocal.address.p0(ptr)",
        ),
        format!("{a}module asm \"opaque\"\n"),
        format!("{a}module\tasm \"opaque\"\n"),
        format!("{a}module\nasm \"opaque\"\n"),
        format!(
            "{a}define void @opaque() {{\n call void asm sideeffect \"opaque\", \"\"()\n ret void\n}}\n"
        ),
        format!("{b}define\nptr @hidden() {{\n ret ptr null\n}}\n"),
        format!("{a}define i64 @raw(ptr %p) {{\n %x = load i64, ptr %p\n ret i64 %x\n}}\n")
            .replace("  %c = load ptr", "  %c = load volatile ptr"),
    ] {
        assert!(context(&ir)["transitive_classes"].is_null(), "{ir}");
    }
    let ir = format!("{a}define i64 @raw(ptr %p) {{\n %x = load i64, ptr %p\n ret i64 %x\n}}\n");
    let data = audit(&[("dart:test", &ir)]).to_json();
    assert_eq!(
        data["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["symbol"] == "raw")
            .unwrap()["transitive_classes"],
        json!([])
    );
    let ir = format!("{a}@text = private constant [4 x i8] c\"asm\\00\"\n");
    assert_eq!(context(&ir)["transitive_classes"], json!([]));
}

#[test]
fn context_lookup_aliases_duplicate_owners_and_shadowed_boundaries_are_rejected() {
    for ir in [tsd("", true), tls("cache", "")] {
        for extra in [
            "define ptr @\"df\\2econtext\"() { ret ptr null }\n",
            "@\"df\\2econtext\" = alias ptr (), ptr @other\n",
            "@df.context = external global ptr\n",
        ] {
            assert!(context(&format!("{ir}{extra}"))["transitive_classes"].is_null());
        }
    }
    let ir = tls("cache", "");
    for shadow in [
        "@\"dartforge\\5fcontext\" = alias ptr (), ptr @other\n",
        "define ptr @\"dartforge\\5fcontext\"() { ret ptr null }\n",
        "@\"llvm.threadlocal.address.p0\" = alias ptr (ptr), ptr @other\n",
        "define ptr @llvm.threadlocal.address.p0(ptr %p) { ret ptr %p }\n",
    ] {
        let data = audit(&[("dart:test", &ir), ("dart:other", shadow)]).to_json();
        assert!(
            data["functions"][0]["transitive_classes"].is_null(),
            "{shadow}"
        );
    }
    for extra in [
        "@cache = external global ptr\n",
        "@\"ca\\63he\" = global ptr null\n",
    ] {
        assert!(context(&format!("{ir}{extra}"))["transitive_classes"].is_null());
    }
    let data = audit(&[("dart:test", &ir), ("dart:test", &ir)]).to_json();
    assert!(
        data["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["transitive_classes"].is_null())
    );
}

#[test]
fn context_lookup_modules_keep_distinct_local_contexts_and_the_host_emitter_matches() {
    let a = tsd("", true);
    let b = tls("cache", " noinline");
    let data = audit(&[("dart:first", &a), ("dart:second", &b)]).to_json();
    assert!(
        data["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["transitive_classes"] == json!([]))
    );
    let ir = crate::sdk_module::with_context_in_tls(
        "define ptr @f() {\n %c = call ptr @dartforge_context()\n ret ptr %c\n}\n",
        crate::sdk_module::ContextInTls::Free,
        "cache",
        true,
    );
    assert_eq!(context(&ir)["transitive_classes"], json!([]));
}
