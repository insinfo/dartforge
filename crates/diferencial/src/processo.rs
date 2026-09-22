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

/// Executa `programa args…` em `cwd`, devolvendo a saída. Mata o processo após `limite`.
pub fn executar(programa: &str, args: &[String], cwd: &Path, limite: Duration) -> Saida {
    executar_com_path(programa, args, cwd, limite, &[])
}

/// Como `executar`, prefixando `path_extra` ao `PATH` do filho (DLLs do LLVM para o `dartforge`).
pub fn executar_com_path(programa: &str, args: &[String], cwd: &Path, limite: Duration, path_extra: &[std::path::PathBuf]) -> Saida {
    let mut cmd = Command::new(programa);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
    let leitor_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out.read_to_end(&mut v);
        v
    });
    let leitor_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err.read_to_end(&mut v);
        v
    });
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
