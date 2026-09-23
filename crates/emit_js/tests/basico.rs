//! Teste diferencial: cada `programas/*.dart` é compilado pelo DartForge,
//! executado no Node e comparado com `dart run` do mesmo arquivo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn programas_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programas")
}

fn dart_run(arquivo: &Path) -> String {
    let out = Command::new("dart").arg("run").arg("--enable-asserts").arg(arquivo).output().expect("dart no PATH");
    assert!(out.status.success(), "dart run falhou em {}: {}", arquivo.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

fn compila_e_roda(arquivo: &Path) -> String {
    let nome = arquivo.file_stem().unwrap().to_string_lossy().to_string();
    let saida = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/emit_js_tests").join(&nome);
    let _ = std::fs::remove_dir_all(&saida);
    let emitido = dartforge_emit_js::compilar(arquivo, None, None).expect("compilação");
    dartforge_emit_js::escrever(&emitido, &saida, &dartforge_emit_js::dart_sdk_js_padrao()).expect("escrita");
    let out = Command::new("node").arg(saida.join("main.mjs")).output().expect("node no PATH");
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    if !out.status.success() {
        panic!(
            "node falhou em {}:\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            nome,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    stdout
}

fn verifica(nome: &str) {
    let arquivo = programas_dir().join(format!("{nome}.dart"));
    let esperado = dart_run(&arquivo);
    let obtido = compila_e_roda(&arquivo);
    if esperado != obtido {
        let e: Vec<&str> = esperado.lines().collect();
        let o: Vec<&str> = obtido.lines().collect();
        let mut diff = String::new();
        for i in 0..e.len().max(o.len()) {
            let a = e.get(i).copied().unwrap_or("<fim>");
            let b = o.get(i).copied().unwrap_or("<fim>");
            if a != b {
                diff.push_str(&format!("linha {}: dart=`{a}` node=`{b}`\n", i + 1));
            }
        }
        panic!("saída diverge em {nome}:\n{diff}");
    }
}

#[test]
fn p1_basico() {
    verifica("p1_basico");
}
#[test]
fn p2_funcoes() {
    verifica("p2_funcoes");
}
#[test]
fn p3_classes() {
    verifica("p3_classes");
}
#[test]
fn p4_colecoes() {
    verifica("p4_colecoes");
}
#[test]
fn p5_excecoes() {
    verifica("p5_excecoes");
}
#[test]
fn p6_async() {
    verifica("p6_async");
}
#[test]
fn p7_dinamico() {
    verifica("p7_dinamico");
}
#[test]
fn p7_padroes() {
    verifica("p7_padroes");
}
#[test]
fn p7_bibliotecas() {
    verifica("p7_bibliotecas/main");
}
#[test]
fn p3_genericas() {
    verifica("p3_genericas");
}
#[test]
fn p8_nosuchmethod_ordem() {
    verifica("p8_nosuchmethod_ordem");
}
