//! The AOT × JIT matrix of the program operations (J06), through
//! `dartforge run`: `Platform.script` is the `.dart`, `Isolate.spawnUri` is
//! rejected with the JIT's message and `Isolate.spawn` works.
#![cfg(feature = "jit")]

use std::process::Command;

#[test]
#[ignore = "compiles the source-built SDK DLL; run in the sdk-fonte job of the Pesado workflow"]
fn script_and_spawn_uri_in_the_jit() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jit_script.dart");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(&fixture)
        .env("DARTFORGE_SDK_DLL", dll)
        .output()
        .expect("CLI");
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "jit_script.dart\ntrue\ndo filho\n", "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success());
}
