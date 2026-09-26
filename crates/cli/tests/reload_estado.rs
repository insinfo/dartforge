#![cfg(feature = "jit")]

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Grava a fixture como um editor grava: conteúdo novo no mesmo arquivo, com
/// o mtime de agora. (`fs::copy` no Windows — `CopyFileW` — preserva o mtime
/// da origem, e duas fixtures do mesmo tamanho, gravadas juntas no checkout,
/// deixariam o arquivo com a mesma assinatura de antes.)
fn editar(origem: &std::path::Path, destino: &std::path::Path) {
    let conteudo = std::fs::read(origem).expect("fixture");
    std::fs::write(destino, conteudo).expect("edição");
}

#[test]
fn cli_preserva_estatico_apos_editar_o_mesmo_arquivo_dart() {
    verificar_recarga(false);
}

#[test]
#[ignore = "compila DLL do SDK da fonte; executar no job sdk-fonte do Pesado"]
fn cli_preserva_estatico_com_sdk_da_fonte() {
    verificar_recarga(true);
}

fn verificar_recarga(com_sdk_da_fonte: bool) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    // Um diretório por teste: os dois rodam em paralelo no mesmo processo, e
    // a limpeza de um apagava a fixture do outro.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-cli-{}-{}", std::process::id(), u8::from(com_sdk_da_fonte)));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_estado_v1.dart"), &entrada);

    let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    command.arg("reload").arg(&entrada).arg("--preservar-estado")
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    if com_sdk_da_fonte {
        let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("DLL do SDK da fonte");
        command.env("DARTFORGE_SDK_DA_FONTE", "1").env("DARTFORGE_SDK_DLL", dll);
    } else {
        command.env_remove("DARTFORGE_SDK_DA_FONTE").env_remove("DARTFORGE_SDK_DLL");
    }
    let mut child = command.spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    // A primeira geração compila o programa inteiro (o perfil `test` do CI
    // é debug): o mesmo prazo do teste ao vivo.
    let first = rx.recv_timeout(Duration::from_secs(60));
    let first_len = rx.recv_timeout(Duration::from_secs(60));
    if first.as_deref() == Ok("1") && first_len.as_deref() == Ok("1") {
        editar(&fixtures.join("reload_estado_v2.dart"), &entrada);
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

/// Hot reload ao vivo: a edição chega ao programa em execução (um timer
/// periódico), no ponto seguro do laço, sem executar o `main` de novo, e o
/// estático continua a contagem.
#[test]
fn cli_recarrega_o_programa_em_execucao() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-reload-vivo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório da fixture");
    let entrada = dir.join("main.dart");
    editar(&fixtures.join("reload_vivo_v1.dart"), &entrada);

    let mut child = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("reload").arg(&entrada)
        .arg("--intervalo").arg("50")
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("CLI");
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let _ = tx.send(line.expect("linha"));
        }
    });

    let mut linhas = Vec::new();
    let mut ultimo_v1 = None;
    let mut primeiro_v2 = None;
    let mut editado = false;
    while let Ok(linha) = rx.recv_timeout(Duration::from_secs(60)) {
        linhas.push(linha.clone());
        if let Some(n) = linha.strip_prefix("v1 ").and_then(|n| n.parse::<u32>().ok()) {
            ultimo_v1 = Some(n);
            if n == 3 && !editado {
                editado = true;
                editar(&fixtures.join("reload_vivo_v2.dart"), &entrada);
            }
        }
        if let Some(n) = linha.strip_prefix("v2 ").and_then(|n| n.parse::<u32>().ok()) {
            primeiro_v2 = Some(n);
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    reader.join().expect("leitor");
    let mut stderr = String::new();
    child.stderr.take().expect("stderr").read_to_string(&mut stderr).expect("diagnósticos");
    std::fs::remove_dir_all(&dir).expect("limpeza da fixture");

    let (Some(v1), Some(v2)) = (ultimo_v1, primeiro_v2) else {
        panic!("sem as duas versões na saída: {linhas:?}\n{stderr}");
    };
    assert_eq!(v2, v1 + 1, "a contagem continua do estático vivo: {linhas:?}");
    assert_eq!(linhas.iter().filter(|l| *l == "main").count(), 1, "o main não roda de novo: {linhas:?}");
    assert!(stderr.contains("geração 2: publicada no programa em execução"), "{stderr}");
}
