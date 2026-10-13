//! Real check of the LLVM declaration against a C function in the host ABI.
use dartforge_abi::{NativeType, Signature, Target};
use std::{path::PathBuf, process::Command};

/// Exclusive directory created by the test, never given external paths to remove.
struct Scratch(PathBuf);
impl Drop for Scratch {
    /// Frees only the files produced in this exclusive directory.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Validates pointer, double, int32 and int64 in a real call, without the dart:ffi frontend.
#[test]
#[ignore = "requires clang with a native C linker configured"]
fn llvm_scalar_declaration_links_and_calls_c() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        Scratch(std::env::temp_dir().join(format!("dartforge-abi-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let signature = Signature {
        result: NativeType::Int64,
        parameters: vec![
            NativeType::Pointer,
            NativeType::Double,
            NativeType::Int32,
            NativeType::Int64,
        ],
    };
    let ir = signature.llvm_declaration("native_mix").unwrap()
        + r#"
@input = internal global i32 7
define i32 @main() {
entry:
  %value = call i64 @native_mix(ptr @input, double 3.0, i32 2, i64 30)
  %ok = icmp eq i64 %value, 42
  %exit = select i1 %ok, i32 0, i32 1
  ret i32 %exit
}
"#;
    let ll = dir.0.join("main.ll");
    let c = dir.0.join("native.c");
    let executable = dir.0.join(format!("abi{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&ll, ir).unwrap();
    std::fs::write(&c, "long long native_mix(int* p, double d, int n, long long big) { return *p + n + (long long)d + big; }\n").unwrap();
    let clang = std::env::var_os("DARTFORGE_CLANG").unwrap_or_else(|| "clang".into());
    let output = Command::new(&clang)
        .arg(&ll)
        .arg(&c)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let run = Command::new(executable).output().unwrap();
    assert!(run.status.success(), "{run:?}");
    // The same contract can produce a wasm32 object, without assuming a ready Dart/Wasm runtime.
    let wasm = dir.0.join("contract.wasm.o");
    let output = Command::new(&clang)
        .arg(format!("--target={}", Target::Wasm32.triple()))
        .arg("-c")
        .arg(&ll)
        .arg("-o")
        .arg(&wasm)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let bytes = std::fs::read(wasm).unwrap();
    assert!(bytes.starts_with(b"\0asm\x01\0\0\0"));
}
