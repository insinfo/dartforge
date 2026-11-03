//! A hello world stays small: the closed world of the production compilers
//! must not keep the whole SDK alive. 863d4159 registered every SDK tear-off
//! at startup, which kept 20 thousand symbols of a hello world (JavaScript
//! interop included) and took its WebAssembly module from 1.29 to 12.75 MB
//! and its Windows executable from 3.5 to 12.9 MB.
#![cfg(feature = "native")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The budget of the hello world's module, as LLVM leaves it (without
/// Binaryen's `wasm-opt`, which the test turns off so that the number does not
/// depend on the machine): 1.30 MB on 02/11/2026, 0.78 MB on 03/11/2026 once
/// `noSuchMethod` stays only for programs that invoke dynamically
/// (`lower/dynamic_invocation.rs`), only the loaded SDK libraries are linked
/// and the runtime's zero tables left the data section.
const HELLO_WASM_BUDGET: u64 = 860_000;

/// The budget of the hello world's executable: 3.49 MB on Windows on
/// 02/11/2026, under the ~5.8 MB of `dart compile exe` on every host.
const HELLO_NATIVE_BUDGET: u64 = 5_000_000;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("size-budget-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hello(dir: &Path) -> PathBuf {
    let source = dir.join("hello.dart");
    std::fs::write(&source, "void main() {\n  print('Hello, World!');\n}\n").unwrap();
    source
}

#[test]
#[ignore = "builds a wasm module with the source-built SDK and LLVM; run in CI"]
fn hello_world_wasm_stays_small_and_without_javascript_interop() {
    let dir = scratch("wasm");
    let source = hello(&dir);
    let module = dir.join("hello.wasm");
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .env("DARTFORGE_WASM_OPT", "0")
        .arg("compile-wasm")
        .arg(&source)
        .arg("-o")
        .arg(&module)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&module).unwrap();
    assert!(
        (bytes.len() as u64) <= HELLO_WASM_BUDGET,
        "hello world: {} bytes of wasm, over the budget of {HELLO_WASM_BUDGET}",
        bytes.len()
    );
    // The interop imports come from the `dartforge_js` module and bring a
    // `.mjs` companion; a hello world has neither. (The runtime's own
    // `dartforge_js_*` exports for the adapter are in every module.)
    let modules = imported_modules(&bytes);
    assert!(
        !modules.iter().any(|m| m == "dartforge_js"),
        "hello world: the module imports the JavaScript interop ({modules:?})"
    );
    assert!(
        !dir.join("hello.mjs").exists() && !dir.join("main.mjs").exists(),
        "hello world: the module has a JavaScript companion"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "builds an executable with the source-built SDK and LLVM; run in CI"]
fn hello_world_executable_stays_small() {
    let dir = scratch("native");
    let source = hello(&dir);
    let executable = dir.join(if cfg!(windows) { "hello.exe" } else { "hello" });
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("aot")
        .arg(&source)
        .arg(&executable)
        .arg("--optimize")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let size = std::fs::metadata(&executable).unwrap().len();
    assert!(
        size <= HELLO_NATIVE_BUDGET,
        "hello world: a {size}-byte executable, over the budget of {HELLO_NATIVE_BUDGET}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn leb(bytes: &[u8], at: &mut usize) -> u64 {
    let (mut value, mut shift) = (0u64, 0);
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn name(bytes: &[u8], at: &mut usize) -> String {
    let n = leb(bytes, at) as usize;
    let text = String::from_utf8_lossy(&bytes[*at..*at + n]).into_owned();
    *at += n;
    text
}

/// The module of each import of a WebAssembly binary (its import section).
fn imported_modules(bytes: &[u8]) -> Vec<String> {
    assert_eq!(&bytes[..4], b"\0asm", "not a WebAssembly module");
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let size = leb(bytes, &mut at) as usize;
        let end = at + size;
        if id == 2 {
            let mut modules = Vec::new();
            for _ in 0..leb(bytes, &mut at) {
                modules.push(name(bytes, &mut at));
                name(bytes, &mut at);
                let kind = bytes[at];
                at += 1;
                let limits = |at: &mut usize| {
                    let flags = bytes[*at];
                    *at += 1;
                    leb(bytes, at);
                    if flags & 1 != 0 {
                        leb(bytes, at);
                    }
                };
                match kind {
                    0 => {
                        leb(bytes, &mut at);
                    }
                    1 => {
                        at += 1;
                        limits(&mut at);
                    }
                    2 => limits(&mut at),
                    3 => at += 2,
                    4 => {
                        at += 1;
                        leb(bytes, &mut at);
                    }
                    _ => panic!("import kind {kind}"),
                }
            }
            return modules;
        }
        at = end;
    }
    Vec::new()
}
