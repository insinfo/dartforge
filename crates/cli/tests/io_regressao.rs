//! Regressões do `dart:io` nativo que o corpus diferencial não cobre (a VM
//! não serve de referência ou o defeito só aparece sob carga): cada
//! programa é compilado pelo AOT e tem de terminar, no prazo, com a saída
//! esperada.
#![cfg(feature = "nativo")]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Compila `fixtures/<nome>.dart` com o SDK da fonte, roda e compara a
/// saída; falha se o programa não termina em `prazo`.
fn compilar_e_rodar(nome: &str, esperado: &str, prazo: Duration) {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = raiz.join(format!("../../target/tmp-io-regressao-{}-{nome}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("diretório do teste");
    let exe = dir.join(if cfg!(windows) { format!("{nome}.exe") } else { nome.to_string() });
    let compilacao = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(raiz.join("tests/fixtures").join(format!("{nome}.dart")))
        .arg("-o")
        .arg(&exe)
        .env("DARTFORGE_SDK_DA_FONTE", "1")
        .output()
        .expect("CLI");
    assert!(compilacao.status.success(), "compilação de {nome}: {}", String::from_utf8_lossy(&compilacao.stderr));

    let mut filho = Command::new(&exe).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("programa");
    let inicio = Instant::now();
    let status = loop {
        if let Some(s) = filho.try_wait().expect("estado do programa") {
            break Some(s);
        }
        if inicio.elapsed() > prazo {
            let _ = filho.kill();
            let _ = filho.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut saida = String::new();
    let mut erro = String::new();
    filho.stdout.take().expect("stdout").read_to_string(&mut saida).expect("saída");
    filho.stderr.take().expect("stderr").read_to_string(&mut erro).expect("erro");
    let _ = std::fs::remove_dir_all(&dir);
    let status = status.unwrap_or_else(|| panic!("{nome} não terminou em {prazo:?} (travou)\nsaída: {saida}\nerro: {erro}"));
    assert!(status.success(), "{nome} saiu com {status}\nsaída: {saida}\nerro: {erro}");
    assert_eq!(saida.replace("\r\n", "\n"), esperado, "{nome}\nerro: {erro}");
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn soquete_de_escuta_compartilhado_so_fecha_com_o_ultimo() {
    compilar_e_rodar("escuta_compartilhada", "o compartilhado sobrevive: ok\no ultimo fecha: ok\n", Duration::from_secs(60));
}

#[test]
#[ignore = "compila com o SDK da fonte e liga com o Clang; roda no CI de cada sistema"]
fn rajada_de_sinais_nao_trava_o_tratador() {
    compilar_e_rodar("rajada_de_sinais", "rajada: ok\nreinscricao: ok\n", Duration::from_secs(60));
}
