#![cfg(feature = "jit")]

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn cli_preserva_estatico_apos_editar_o_mesmo_arquivo_dart() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    std::fs::copy(fixtures.join("reload_estado_v1.dart"), &entrada).expect("versão 1");

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload")
        .arg(&entrada)
        .arg("--preservar-estado")
        .arg("--intervalo")
        .arg("50")
        .env_remove("DARTFORGE_SDK_DA_FONTE")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let first = rx.recv_timeout(Duration::from_secs(20));
    let first_len = rx.recv_timeout(Duration::from_secs(20));
    if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        std::fs::copy(fixtures.join("reload_estado_v2.dart"), &entrada).expect("versão 2");
    }
    let second = if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        rx.recv_timeout(Duration::from_secs(20))
    } else {
        Err(mpsc::RecvTimeoutError::Disconnected)
    };
    let second_len = if second.as_deref() == Ok("11") {
        rx.recv_timeout(Duration::from_secs(20))
    } else {
        Err(mpsc::RecvTimeoutError::Disconnected)
    };
    let _ = child.kill();
    let _ = child.wait();
    reader.join().expect("leitor");
    let mut stderr = String::new();
    child.stderr.take().expect("stderr").read_to_string(&mut stderr).expect("diagnósticos");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    assert_eq!(first.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(first_len.as_deref(), Ok("1"), "{stderr}");
    assert_eq!(second.as_deref(), Ok("11"), "{stderr}");
    assert_eq!(second_len.as_deref(), Ok("2"), "{stderr}");
    assert!(stderr.contains("geração 1: estado preservado"), "{stderr}");
    assert!(stderr.contains("geração 2: estado preservado"), "{stderr}");
}
