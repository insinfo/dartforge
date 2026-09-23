//! Execução de processos externos com captura de stdout/stderr e limite de tempo.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Saída de um processo (ou de um oráculo inteiro): stdout, stderr e código.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saida {
    pub stdout: String,
    pub stderr: String,
    pub codigo: i32,
}

impl Saida {
    /// Saída sintética de falha do próprio harness (ferramenta ausente, tempo esgotado…).
    pub fn erro(mensagem: impl Into<String>) -> Saida {
        Saida { stdout: String::new(), stderr: mensagem.into(), codigo: -1 }
    }

    /// Primeira linha não vazia do stderr — a chave de agrupamento do relatório.
    pub fn primeira_linha_stderr(&self) -> &str {
        self.stderr.lines().map(str::trim_end).find(|l| !l.trim().is_empty()).unwrap_or("")
    }
}

/// Código devolvido quando o processo estoura o tempo limite.
pub const CODIGO_TEMPO_ESGOTADO: i32 = -2;

/// Teto do que o harness guarda de stdout/stderr de UM processo.
///
/// Um programa em laço que imprime enche a memória do harness, não só a dele:
/// `read_to_end` cresce sem limite. O que interessa ao relatório é a primeira
/// divergência e a primeira linha do stderr, e as duas cabem de sobra aqui.
/// O resto continua sendo lido e descartado, senão o cano enche e o filho
/// trava antes do tempo-limite poder matá-lo.
const TETO_CAPTURA: usize = 4 * 1024 * 1024;

/// Lê tudo até o fim, guardando no máximo [`TETO_CAPTURA`] bytes.
fn ler_limitado(mut r: impl Read) -> Vec<u8> {
    let mut acumulado: Vec<u8> = Vec::new();
    let mut pedaco = [0u8; 64 * 1024];
    let mut descartados: usize = 0;
    loop {
        match r.read(&mut pedaco) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if acumulado.len() < TETO_CAPTURA {
                    let cabe = (TETO_CAPTURA - acumulado.len()).min(n);
                    acumulado.extend_from_slice(&pedaco[..cabe]);
                    descartados += n - cabe;
                } else {
                    descartados += n;
                }
            }
        }
    }
    if descartados > 0 {
        acumulado.extend_from_slice(
            format!("\n[saída truncada pelo harness: mais {descartados} bytes]\n").as_bytes(),
        );
    }
    acumulado
}

/// Executa `programa args…` em `cwd`, devolvendo a saída. Mata o processo após `limite`.
pub fn executar(programa: &str, args: &[String], cwd: &Path, limite: Duration) -> Saida {
    executar_com_path(programa, args, cwd, limite, &[])
}

/// Como `executar`, prefixando `path_extra` ao `PATH` do filho (DLLs do LLVM para o `dartforge`).
pub fn executar_com_path(programa: &str, args: &[String], cwd: &Path, limite: Duration, path_extra: &[std::path::PathBuf]) -> Saida {
    executar_com_ambiente(programa, args, cwd, limite, path_extra, &[])
}

/// Como `executar_com_path`, com variáveis de ambiente a mais só para o filho
/// (o `--gc-stress` do modo nativo). `set_var` no próprio harness seria
/// `unsafe` e valeria para as outras threads também.
pub fn executar_com_ambiente(
    programa: &str,
    args: &[String],
    cwd: &Path,
    limite: Duration,
    path_extra: &[std::path::PathBuf],
    ambiente: &[(&str, &str)],
) -> Saida {
    let mut cmd = Command::new(programa);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in ambiente {
        cmd.env(k, v);
    }
    if !path_extra.is_empty() {
        let atual = std::env::var_os("PATH").unwrap_or_default();
        let mut novo: Vec<std::path::PathBuf> = path_extra.to_vec();
        novo.extend(std::env::split_paths(&atual));
        if let Ok(j) = std::env::join_paths(novo) {
            cmd.env("PATH", j);
        }
    }
    let mut filho = match cmd.spawn() {
        Ok(f) => f,
        Err(e) => return Saida::erro(format!("não foi possível executar `{programa}`: {e}")),
    };
    let mut out = filho.stdout.take().expect("stdout piped");
    let mut err = filho.stderr.take().expect("stderr piped");
    let leitor_out = std::thread::spawn(move || ler_limitado(&mut out));
    let leitor_err = std::thread::spawn(move || ler_limitado(&mut err));
    let inicio = Instant::now();
    let mut estourou = false;
    let status = loop {
        match filho.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if inicio.elapsed() > limite {
                    let _ = filho.kill();
                    let _ = filho.wait();
                    estourou = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break None,
        }
    };
    let stdout = String::from_utf8_lossy(&leitor_out.join().unwrap_or_default()).into_owned();
    let mut stderr = String::from_utf8_lossy(&leitor_err.join().unwrap_or_default()).into_owned();
    let codigo = match status {
        Some(s) => s.code().unwrap_or(-3),
        None if estourou => {
            stderr = format!("tempo esgotado ({} s): {programa} {}\n{stderr}", limite.as_secs(), args.join(" "));
            CODIGO_TEMPO_ESGOTADO
        }
        None => -3,
    };
    Saida { stdout, stderr, codigo }
}
