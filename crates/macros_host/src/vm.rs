//! O executor de **materialização**: roda a mesma macro — com a nossa API
//! (`pacotes/macros`, Dart puro) — numa VM Dart, pelo mesmo protocolo
//! `dfexec/1`, para gravar a augmentation num arquivo que o SDK oficial aceita
//! com a flag experimental (docs/MACROS-COMPATIBILIDADE.md). É o caminho do
//! builder para o `build_runner` e da compatibilidade com a toolchain
//! oficial. O compilador JS também o usa provisoriamente para executar
//! aplicações diretamente da fonte; o executor nativo D4 continua pendente.
//!
//! O hospedeiro gera o *bootstrap* (spec, "Macro execution"): um `main` que
//! importa as bibliotecas das macros aplicadas e mapeia `uri#Classe` →
//! construtor por *tear-off*, sem mirrors; e um `package_config.json` que é o
//! do projeto com `macros` apontando para a nossa API.
use crate::aplicacoes::Aplicacao;
use crate::executor::ExecutorDfexec;
use crate::protocolo::CanalDeProcesso;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Onde estão a VM e a nossa API.
#[derive(Debug, Clone)]
pub struct ConfigDaVm {
    /// O executável `dart`.
    pub dart: PathBuf,
    /// A raiz do pacote da nossa API (`pacotes/macros`).
    pub api: PathBuf,
    /// Onde gravar o bootstrap e o `package_config.json`.
    pub trabalho: PathBuf,
}

/// O texto do bootstrap para as macros de `apps`.
pub fn bootstrap(apps: &[Aplicacao]) -> String {
    let mut bibliotecas: Vec<String> = Vec::new();
    let mut macros: Vec<(String, Vec<String>)> = Vec::new();
    for a in apps {
        let (uri, _) = a.macro_.split_once('#').unwrap_or((&a.macro_, ""));
        if !bibliotecas.iter().any(|b| b == uri) {
            bibliotecas.push(uri.to_string());
        }
        match macros.iter_mut().find(|(m, _)| *m == a.macro_) {
            Some((_, cs)) if !cs.contains(&a.construtor) => cs.push(a.construtor.clone()),
            Some(_) => {}
            None => macros.push((a.macro_.clone(), vec![a.construtor.clone()])),
        }
    }
    let mut s = String::from(
        "// Gerado pelo DartForge: bootstrap do executor de macros (dfexec/1, serviço macro.*).\n\
         import 'package:macros/src/executor/canal_stdio.dart';\n\
         import 'package:macros/src/executor/servico.dart';\n",
    );
    for (i, b) in bibliotecas.iter().enumerate() {
        s.push_str(&format!("import '{b}' as m{i};\n"));
    }
    s.push_str("\nFuture<void> main() => servir({\n");
    for (m, construtores) in &macros {
        let (uri, classe) = m.split_once('#').unwrap_or((m, ""));
        let i = bibliotecas.iter().position(|b| b == uri).unwrap_or(0);
        s.push_str(&format!("  '{m}': {{\n"));
        for c in construtores {
            let tearoff = if c.is_empty() { format!("m{i}.{classe}.new") } else { format!("m{i}.{classe}.{c}") };
            let sem_argumentos = apps.iter().filter(|a| a.macro_ == *m && a.construtor == *c)
                .all(|a| {
                    a.argumentos.get("posicionais").and_then(Value::as_array).is_some_and(Vec::is_empty)
                        && a.argumentos.get("nomeados").and_then(Value::as_object).is_some_and(|n| n.is_empty())
                });
            if sem_argumentos {
                // A fábrica é usada apenas como @Macro(): invocar o
                // construtor diretamente preserva defaults sem depender de
                // Function.apply, Symbol ou Map.entries no executor nativo.
                s.push_str(&format!("    '{c}': (_, _) => {tearoff}(),\n"));
            } else {
                s.push_str(&format!(
                    "    '{c}': (p, n) => Function.apply({tearoff}, p, {{for (final e in n.entries) Symbol(e.key): e.value}}),\n"
                ));
            }
        }
        s.push_str("  },\n");
    }
    s.push_str("}, CanalStdio());\n");
    s
}

/// O `package_config.json` do projeto com `macros` trocado pela nossa API.
pub fn package_config(do_projeto: Option<&Path>, api: &Path) -> Result<Value, String> {
    let mut cfg: Value = match do_projeto {
        Some(p) => {
            let texto = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut v: Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", p.display()))?;
            // URIs relativas do projeto passam a absolutas: o arquivo novo mora
            // em outro diretório.
            let base = p.parent().unwrap_or(Path::new("."));
            if let Some(ps) = v.get_mut("packages").and_then(Value::as_array_mut) {
                for pk in ps.iter_mut() {
                    if let Some(r) = pk.get("rootUri").and_then(Value::as_str) {
                        if !r.contains(':') {
                            let abs = base.join(r);
                            pk["rootUri"] = json!(uri_de_arquivo(&abs, true));
                        }
                    }
                }
            }
            v
        }
        None => json!({"configVersion": 2, "packages": []}),
    };
    let ps = cfg["packages"].as_array_mut().ok_or("package_config sem 'packages'")?;
    ps.retain(|p| p["name"] != "macros" && p["name"] != "_macros");
    ps.push(json!({"name": "macros", "rootUri": uri_de_arquivo(api, true), "packageUri": "lib/", "languageVersion": "3.6"}));
    Ok(cfg)
}

fn uri_de_arquivo(p: &Path, diretorio: bool) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut s = abs.to_string_lossy().replace('\\', "/");
    if let Some(r) = s.strip_prefix("//?/") {
        s = r.to_string();
    }
    if diretorio && !s.ends_with('/') {
        s.push('/');
    }
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// Grava o bootstrap e inicia a VM com ele. O canal é o de qualquer
/// processo que fale `dfexec/1`.
pub fn iniciar(cfg: &ConfigDaVm, apps: &[Aplicacao], package_config_do_projeto: Option<&Path>) -> Result<ExecutorDfexec<CanalDeProcesso>, String> {
    std::fs::create_dir_all(&cfg.trabalho).map_err(|e| format!("{}: {e}", cfg.trabalho.display()))?;
    let principal = cfg.trabalho.join("bootstrap.dart");
    std::fs::write(&principal, bootstrap(apps)).map_err(|e| format!("{}: {e}", principal.display()))?;
    let pc = cfg.trabalho.join("package_config.json");
    let json = package_config(package_config_do_projeto, &cfg.api)?;
    std::fs::write(&pc, serde_json::to_string_pretty(&json).unwrap_or_default()).map_err(|e| format!("{}: {e}", pc.display()))?;
    let mut cmd = std::process::Command::new(&cfg.dart);
    cmd.arg("--enable-experiment=macros").arg(format!("--packages={}", pc.display())).arg(&principal);
    Ok(ExecutorDfexec::novo(CanalDeProcesso::iniciar(cmd)?))
}
