//! Builds the Windows-only bridge for RTDyld's COFF symbol ownership flags.
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/coff_layer.cpp");
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_DIR");
    println!("cargo::rerun-if-env-changed=LLVM_SYS_221_PREFIX");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let prefix = std::env::var_os("DARTFORGE_LLVM_DIR")
        .or_else(|| std::env::var_os("LLVM_SYS_221_PREFIX"))
        .map(PathBuf::from)
        .expect("Windows JIT requires DARTFORGE_LLVM_DIR or LLVM_SYS_221_PREFIX");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    let object = out.join("coff_layer.obj");
    let library = out.join("dartforge_coff_layer.lib");
    // The bridge only invokes inline setters on an object owned by LLVM-C.dll;
    // no C++ allocation or ownership crosses the DLL boundary.
    let status = Command::new(prefix.join("bin/clang-cl.exe"))
        .arg(format!(
            "--target={}",
            std::env::var("TARGET").expect("Cargo TARGET")
        ))
        .args([
            "/c",
            "/O2",
            "/MD",
            "/GR-",
            "/EHsc-",
            "/std:c++17",
            "/DLLVM_DISABLE_ABI_BREAKING_CHECKS_ENFORCING=1",
        ])
        .arg(format!("/I{}", prefix.join("include").display()))
        .arg("src/coff_layer.cpp")
        .arg(format!("/Fo{}", object.display()))
        .status()
        .expect("run LLVM clang-cl for the COFF JIT bridge");
    assert!(status.success(), "compiling the COFF JIT bridge failed");
    let status = Command::new(prefix.join("bin/llvm-lib.exe"))
        .arg(format!("/OUT:{}", library.display()))
        .arg(&object)
        .status()
        .expect("run LLVM librarian for the COFF JIT bridge");
    assert!(status.success(), "archiving the COFF JIT bridge failed");
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=dartforge_coff_layer");
}
