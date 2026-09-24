//! O oráculo: `dart analyze --format=json` do SDK oficial, e o registro do
//! que ele disse, gravado em disco ao lado do corpus.
//!
//! Dois SDKs, escolhidos pela versão de linguagem da biblioteca (plano §2.4):
//! o 3.6.2 para as bibliotecas até 3.6, o 3.13.4 para as que declaram 3.7 ou
//! mais (`// @dart = 3.7`). O registro (`oraculo.jsonl`) guarda, por linha,
//! um diagnóstico com o caminho relativo ao grupo; o CI compara contra ele
//! sem precisar do `dart`. `oraculo.json` guarda o SDK e o hash das fontes:
//! fonte mudada sem regravar é acusada pelo placar.

use crate::json::{DiagJson, Relatorio};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Um SDK oficial usado como oráculo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SdkOraculo {
    /// Dart 3.6.2 (`C:/tools/dartsdk-3.6.2`).
    #[serde(rename = "3.6.2")]
    V362,
    /// Dart 3.13.4 (`D:/DartSDKs/3.13.4/dart-sdk`).
    #[serde(rename = "3.13.4")]
    V3134,
}

impl SdkOraculo {
    pub fn versao(self) -> &'static str {
        match self {
            SdkOraculo::V362 => "3.6.2",
            SdkOraculo::V3134 => "3.13.4",
        }
    }

    /// Restrição `environment: sdk:` do pubspec do grupo.
    pub fn restricao(self) -> &'static str {
        match self {
            SdkOraculo::V362 => "^3.6.0",
            SdkOraculo::V3134 => "^3.13.0",
        }
    }

    /// Versão de linguagem padrão do grupo.
    pub fn linguagem(self) -> &'static str {
        match self {
            SdkOraculo::V362 => "3.6",
            SdkOraculo::V3134 => "3.13",
        }
    }

    /// Onde o SDK está nesta máquina (`DARTFORGE_ORACULO_3_6`/`_3_13` mudam).
    pub fn raiz(self) -> PathBuf {
        let (var, padrao) = match self {
            SdkOraculo::V362 => ("DARTFORGE_ORACULO_3_6", "E:/DartSDKs/3.6.2"),
            SdkOraculo::V3134 => ("DARTFORGE_ORACULO_3_13", "E:/DartSDKs/3.13.4/dart-sdk"),
        };
        std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(padrao))
    }
}

/// Uma linha do registro do oráculo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registro {
    /// Caminho relativo à raiz do grupo, com `/`.
    pub arquivo: String,
    pub offset: usize,
    pub length: usize,
    pub code: String,
    pub severity: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub line: usize,
    pub column: usize,
    pub problem_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction_message: Option<String>,
}

impl Registro {
    /// De um diagnóstico JSON v1, relativo a `raiz` (`None` fora dela ou fora de `.dart`).
    pub fn de_json(d: &DiagJson, raiz: &Path) -> Option<Registro> {
        let arquivo = relativo(Path::new(&d.location.file), raiz)?;
        if !arquivo.ends_with(".dart") {
            return None;
        }
        let r = &d.location.range;
        Some(Registro {
            arquivo,
            offset: r.start.offset,
            length: r.end.offset.saturating_sub(r.start.offset),
            code: d.code.clone(),
            severity: d.severity.clone(),
            tipo: d.tipo.clone(),
            line: r.start.line,
            column: r.start.column,
            problem_message: d.problem_message.clone(),
            correction_message: d.correction_message.clone(),
        })
    }
}

/// Caminho relativo com `/`, sem diferenciar maiúsculas na raiz (Windows).
pub fn relativo(p: &Path, raiz: &Path) -> Option<String> {
    let p = crate::analise::chave(p).to_string_lossy().replace('\\', "/");
    let r = crate::analise::chave(raiz).to_string_lossy().replace('\\', "/");
    let r = r.trim_end_matches('/');
    if p.len() > r.len() && p[..r.len()].eq_ignore_ascii_case(r) && p.as_bytes()[r.len()] == b'/' {
        Some(p[r.len() + 1..].to_string())
    } else {
        None
    }
}

/// Metadados do registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub sdk: SdkOraculo,
    /// Hash (FNV-1a 64, hexadecimal) das fontes `.dart` do grupo.
    pub hash: String,
    pub arquivos: usize,
    pub diagnosticos: usize,
    /// Arquivos retirados do grupo porque derrubam o servidor de análise do
    /// oráculo (não há o que comparar).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluidos: Vec<String>,
}

/// FNV-1a 64: estável entre versões do Rust (o `DefaultHasher` não é).
pub fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub const FNV_INICIO: u64 = 0xcbf29ce484222325;

/// Roda `dart analyze --format=json <alvos>...` com o SDK dado, no diretório do
/// pacote. `cache` vai em `--cache` (fora do C:). Devolve os diagnósticos.
pub fn rodar(sdk: SdkOraculo, pacote: &Path, alvos: &[PathBuf], cache: &Path) -> Result<Vec<DiagJson>, String> {
    let dart = sdk.raiz().join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" });
    if !dart.exists() {
        return Err(format!("SDK {} não encontrado em {}", sdk.versao(), dart.display()));
    }
    let saida = Command::new(&dart)
        .arg("analyze")
        .arg("--format=json")
        .arg("--cache")
        .arg(cache)
        .args(alvos.iter().map(|a| std::path::absolute(a).unwrap_or_else(|_| a.clone())))
        .current_dir(pacote)
        .env("DART_DISABLE_ANALYTICS", "1")
        .output()
        .map_err(|e| format!("{}: {e}", dart.display()))?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let codigo = saida.status.code().unwrap_or(-1);
    if !(0..=3).contains(&codigo) {
        return Err(format!(
            "dart analyze saiu com {codigo}: {}{}",
            texto.chars().take(2000).collect::<String>(),
            String::from_utf8_lossy(&saida.stderr).chars().take(2000).collect::<String>()
        ));
    }
    // A saída pode vir precedida de avisos; o JSON é a linha que começa com `{`.
    let json = texto.lines().find(|l| l.starts_with('{')).unwrap_or("");
    if json.is_empty() {
        // Sem JSON: o servidor de análise caiu (ex.: `FormatException` do
        // 3.6.2 com separador de dígitos em `1.234_456e0`).
        return Err(format!("sem JSON (código {codigo}): {}", texto.lines().take(6).collect::<Vec<_>>().join(" | ")));
    }
    let r: Relatorio = serde_json::from_str(json).map_err(|e| format!("JSON do oráculo: {e}"))?;
    Ok(r.diagnostics)
}

/// Grava o registro (ordenado) e os metadados.
pub fn gravar(dir: &Path, mut regs: Vec<Registro>, meta: &Meta) -> std::io::Result<()> {
    regs.sort();
    let mut s = String::new();
    for r in &regs {
        s.push_str(&serde_json::to_string(r).expect("JSON"));
        s.push('\n');
    }
    std::fs::write(dir.join("oraculo.jsonl"), s)?;
    std::fs::write(dir.join("oraculo.json"), serde_json::to_string_pretty(meta).expect("JSON") + "\n")
}

/// Lê o registro e os metadados de um grupo.
pub fn ler(dir: &Path) -> Result<(Vec<Registro>, Meta), String> {
    let meta: Meta = serde_json::from_str(
        &std::fs::read_to_string(dir.join("oraculo.json")).map_err(|e| format!("{}: {e}", dir.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let texto = std::fs::read_to_string(dir.join("oraculo.jsonl")).map_err(|e| e.to_string())?;
    let mut regs = Vec::new();
    for l in texto.lines().filter(|l| !l.is_empty()) {
        regs.push(serde_json::from_str(l).map_err(|e| format!("{l}: {e}"))?);
    }
    Ok((regs, meta))
}
