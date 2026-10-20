//! DF-BUILD-009: `compile-native` and `run` (JIT) hook up the project's build
//! engine. `corpus/builders/i18n` imports `mensagens*.i18n.dart`, which do
//! not exist on disk (the i18n builder generates them into `source`, and the
//! oracle keeps them apart); the engine generates them in memory through the
//! native generator, without `build_runner` having run before and without a
//! fallback, and the program output is that of the official VM
//! (`oraculo/saida.txt`).
#![cfg(feature = "native")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn case() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders/i18n");
    if !root.join(".dart_tool/package_config.json").is_file() {
        eprintln!(
            "no package_config.json in {}: run `dart pub get` in the case",
            root.display()
        );
        return None;
    }
    // Nothing materialized beforehand: neither the `.i18n.dart` in `lib/`, nor
    // the `build_runner` tree.
    for p in ["lib/mensagens.i18n.dart", ".dart_tool/build"] {
        assert!(
            !root.join(p).exists(),
            "{p} exists: the test requires generation in memory only"
        );
    }
    Some(root)
}

fn expected(root: &Path) -> String {
    std::fs::read_to_string(root.join("oraculo/saida.txt"))
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn compile_native_generates_the_sources_through_the_engine() {
    let Some(root) = case() else { return };
    let dir = std::env::temp_dir().join(format!("dartforge-gerado-nativo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(if cfg!(windows) { "app.exe" } else { "app" });
    let c = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(root.join("bin/main.dart"))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("CLI");
    assert!(
        c.status.success(),
        "compile-native: {}",
        String::from_utf8_lossy(&c.stderr)
    );
    let s = Command::new(&exe).output().expect("executable");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        String::from_utf8_lossy(&s.stdout).replace("\r\n", "\n"),
        expected(&root)
    );
}

#[cfg(feature = "jit")]
#[test]
fn run_generates_the_sources_through_the_engine() {
    let Some(root) = case() else { return };
    let s = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(root.join("bin/main.dart"))
        .output()
        .expect("CLI");
    assert!(
        s.status.success(),
        "run: {}",
        String::from_utf8_lossy(&s.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&s.stdout).replace("\r\n", "\n"),
        expected(&root)
    );
}
