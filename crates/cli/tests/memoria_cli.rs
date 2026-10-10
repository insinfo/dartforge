//! A política é selecionada pela opção, inclusive com ambiente ARC herdado.
#![cfg(feature = "nativo")]

use std::{
    path::Path,
    process::{Command, Output},
};

fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    cmd.env("DARTFORGE_MEMORIA", "arc");
    cmd
}

fn conferir(output: Output, esperado: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        esperado,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn valor_invalido_e_recusado_antes_de_carregar_fonte() {
    let mut casos = vec![
        vec!["compile-native", "--memoria=errada", "ausente.dart"],
        vec!["aot", "ausente.dart", "ausente.exe", "--memoria=errada"],
    ];
    if cfg!(feature = "jit") {
        casos.extend([
            vec!["run", "--memoria=errada", "ausente.dart"],
            vec!["reload", "ausente.dart", "--memoria=errada"],
        ]);
    }
    for args in casos {
        let output = cli().args(&args).output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--memoria=errada: as políticas"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "prova nativa com SDK da fonte e LLVM; executada na CI"]
fn aot_so_ativa_arc_com_opcao_explicita() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonte = raiz.join("tests/fixtures/memoria_cli.dart");
    let dir = raiz.join(format!("../../target/memoria-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for arc in [false, true] {
        let exe = dir.join(format!(
            "{}.{}",
            if arc { "arc" } else { "padrao" },
            if cfg!(windows) { "exe" } else { "bin" }
        ));
        let mut cmd = cli();
        cmd.arg("compile-native")
            .arg(&fonte)
            .arg("-o")
            .arg(&exe)
            .env("TEMP", &dir)
            .env("TMP", &dir);
        if arc {
            cmd.arg("--memoria=arc");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        conferir(
            Command::new(exe)
                .env("DARTFORGE_MEMORIA", "arc")
                .output()
                .unwrap(),
            if arc { "1\n\n" } else { "0\n\n" },
        );
    }
}

#[cfg(feature = "jit")]
#[test]
#[ignore = "prova JIT e reload com SDK da fonte e LLVM; executada na CI"]
fn jit_e_reload_selecionam_politica_e_preservam_argumentos_do_programa() {
    let fonte = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/memoria_cli.dart");
    for arc in [false, true] {
        let mut run = cli();
        run.arg("run");
        if arc {
            run.arg("--memoria=arc");
        }
        run.arg(&fonte).arg("--memoria=arc");
        conferir(
            run.output().unwrap(),
            if arc {
                "1\n--memoria=arc\n"
            } else {
                "0\n--memoria=arc\n"
            },
        );
        for reiniciar in [false, true] {
            let mut reload = cli();
            reload.arg("reload").arg(&fonte).arg("--uma-vez");
            if arc {
                reload.arg("--memoria=arc");
            }
            if reiniciar {
                reload.arg("--reiniciar");
            }
            conferir(
                reload.output().unwrap(),
                if arc { "1\n\n" } else { "0\n\n" },
            );
        }
    }
}
