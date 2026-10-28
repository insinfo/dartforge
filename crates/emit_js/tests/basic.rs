//! Differential test: each `programs/*.dart` is compiled by DartForge, run on
//! Node and compared with `dart run` of the same file.

use std::path::{Path, PathBuf};
use std::process::Command;

fn programs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs")
}

fn dart_run(file: &Path) -> String {
    let out = Command::new("dart")
        .arg("run")
        .arg("--enable-asserts")
        .arg(file)
        .output()
        .expect("dart on PATH");
    assert!(
        out.status.success(),
        "dart run failed on {}: {}",
        file.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

fn compile_and_run(file: &Path) -> String {
    let name = file.file_stem().unwrap().to_string_lossy().to_string();
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/emit_js_tests")
        .join(&name);
    let _ = std::fs::remove_dir_all(&out_dir);
    let emitted = dartforge_emit_js::compile(file, None, None).expect("compilation");
    dartforge_emit_js::write(
        &emitted,
        &out_dir,
        &dartforge_emit_js::default_dart_sdk_js(),
    )
    .expect("write");
    let out = Command::new("node")
        .arg(out_dir.join("main.mjs"))
        .output()
        .expect("node on PATH");
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    if !out.status.success() {
        panic!(
            "node failed on {}:\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            name,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    stdout
}

fn check(name: &str) {
    let file = programs_dir().join(format!("{name}.dart"));
    let expected = dart_run(&file);
    let actual = compile_and_run(&file);
    if expected != actual {
        let e: Vec<&str> = expected.lines().collect();
        let o: Vec<&str> = actual.lines().collect();
        let mut diff = String::new();
        for i in 0..e.len().max(o.len()) {
            let a = e.get(i).copied().unwrap_or("<end>");
            let b = o.get(i).copied().unwrap_or("<end>");
            if a != b {
                diff.push_str(&format!("line {}: dart=`{a}` node=`{b}`\n", i + 1));
            }
        }
        panic!("output diverges in {name}:\n{diff}");
    }
}

#[test]
fn p1_basic() {
    check("p1_basic");
}
#[test]
fn p2_functions() {
    check("p2_functions");
}
#[test]
fn p3_classes() {
    check("p3_classes");
}
#[test]
fn p4_collections() {
    check("p4_collections");
}
#[test]
fn p5_exceptions() {
    check("p5_exceptions");
}
#[test]
fn p6_async() {
    check("p6_async");
}
#[test]
fn main_arguments() {
    check("main_arguments");
    check("main_arguments_message");
}
#[test]
fn p7_dynamic() {
    check("p7_dynamic");
}
#[test]
fn p7_patterns() {
    check("p7_patterns");
}
#[test]
fn p7_libraries() {
    check("p7_libraries/main");
}
#[test]
fn p3_generics() {
    check("p3_generics");
}
#[test]
fn p8_nosuchmethod_order() {
    check("p8_nosuchmethod_order");
}

/// Identifier resolved through the lexical scope (the resolution of the shared
/// inference): the top-level declaration wins over the inherited member with the
/// same name.
#[test]
fn p9_lexical_scope() {
    check("p9_lexical_scope");
}

/// Reified type arguments coming from the shared inference: instantiation of
/// generic calls, collection literals, constructors and closures, observed
/// through `is`, `as`, `runtimeType` and the printed `T`
/// (docs/INFERENCIA-JS-ALINHAMENTO.md §4). Previously, 12 lines diverged from the VM.
#[test]
fn p10_reified_types() {
    check("p10_reified_types");
}

#[test]
fn indexed_assignment_order() {
    check("index_assignment_order");
}

#[test]
fn property_assignment_order() {
    check("property_assignment_order");
}

/// A dynamic call of a private name shows the Dart name in the
/// `NoSuchMethodError` and in `Invocation.memberName`.
#[test]
fn private_dynamic_names() {
    check("private_dynamic_names");
}

/// Printed function types as on the VM: the type parameters of generic
/// function types are `Y0`, `F1Y0`… (DDC's runtime prints `T1`, corrected in
/// the copied `dart_sdk.js`, `patch_dart_sdk`) and named parameters keep
/// their source names.
#[test]
fn function_type_names() {
    check("function_type_names");
}
