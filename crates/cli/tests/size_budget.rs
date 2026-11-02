//! A hello world stays small: the closed world of the production compilers
//! must not keep the whole SDK alive. 863d4159 registered every SDK tear-off
//! at startup, which kept 20 thousand symbols of a hello world (JavaScript
//! interop included) and took its WebAssembly module from 1.29 to 12.75 MB.
#![cfg(feature = "native")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The budget of the hello world's module: 1.29 MB on 31/10/2026.
const HELLO_WASM_BUDGET: u64 = 1_400_000;

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
    // `.mjs` companion; a hello world has neither.
    assert!(
        !bytes.windows(b"dartforge_js".len()).any(|w| w == b"dartforge_js"),
        "hello world: the module imports the JavaScript interop"
    );
    assert!(
        !dir.join("hello.mjs").exists() && !dir.join("main.mjs").exists(),
        "hello world: the module has a JavaScript companion"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
