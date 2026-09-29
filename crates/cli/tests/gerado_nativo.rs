//! DF-BUILD-009: o `compile-native` e o `run` (JIT) ligam o motor de build
//! do projeto. `corpus/builders/i18n` importa `mensagens*.i18n.dart`, que não
//! existem no disco (o builder do i18n os gera em `source`, e o oráculo os
//! guarda à parte); o motor os gera em memória pelo gerador nativo, sem
//! `build_runner` executado antes nem apoio, e a saída do programa é a da VM
//! oficial (`oraculo/saida.txt`).
#![cfg(feature = "nativo")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn caso() -> Option<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders/i18n");
    if !raiz.join(".dart_tool/package_config.json").is_file() {
        eprintln!("sem package_config.json em {}: rode `dart pub get` no caso", raiz.display());
        return None;
    }
    // Nada materializado antes: nem o `.i18n.dart` em `lib/`, nem a árvore
    // do `build_runner`.
    for p in ["lib/mensagens.i18n.dart", ".dart_tool/build"] {
        assert!(!raiz.join(p).exists(), "{p} existe: o teste exige geração só em memória");
    }
    Some(raiz)
}

fn esperado(raiz: &Path) -> String {
    std::fs::read_to_string(raiz.join("oraculo/saida.txt")).unwrap().replace("\r\n", "\n")
}

#[test]
fn compile_native_gera_as_fontes_pelo_motor() {
    let Some(raiz) = caso() else { return };
    let dir = std::env::temp_dir().join(format!("dartforge-gerado-nativo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(if cfg!(windows) { "app.exe" } else { "app" });
    let c = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(raiz.join("bin/main.dart"))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("CLI");
    assert!(c.status.success(), "compile-native: {}", String::from_utf8_lossy(&c.stderr));
    let s = Command::new(&exe).output().expect("executável");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(String::from_utf8_lossy(&s.stdout).replace("\r\n", "\n"), esperado(&raiz));
}

#[cfg(feature = "jit")]
#[test]
fn run_gera_as_fontes_pelo_motor() {
    let Some(raiz) = caso() else { return };
    let s = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(raiz.join("bin/main.dart"))
        .output()
        .expect("CLI");
    assert!(s.status.success(), "run: {}", String::from_utf8_lossy(&s.stderr));
    assert_eq!(String::from_utf8_lossy(&s.stdout).replace("\r\n", "\n"), esperado(&raiz));
}
