//! A matriz AOT × JIT das operações de programa (J06), pelo `dartforge run`:
//! `Platform.script` é o `.dart`, `Isolate.spawnUri` é recusado com a
//! mensagem do JIT e `Isolate.spawn` funciona.
#![cfg(feature = "jit")]

use std::process::Command;

#[test]
#[ignore = "compila DLL do SDK da fonte; executar no job sdk-fonte do Pesado"]
fn script_e_spawn_uri_no_jit() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jit_script.dart");
    let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("DLL do SDK da fonte");
    let saida = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(&fixture)
        .env("DARTFORGE_SDK_DA_FONTE", "1")
        .env("DARTFORGE_SDK_DLL", dll)
        .output()
        .expect("CLI");
    let stdout = String::from_utf8_lossy(&saida.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "jit_script.dart\ntrue\ndo filho\n", "stderr: {}", String::from_utf8_lossy(&saida.stderr));
    assert!(saida.status.success());
}
