//! O executor **nativo** de macros (D4, `docs/MACROS-PROTOCOLO.md` §3): o
//! mesmo *bootstrap* e o mesmo `package_config.json` do executor pela VM
//! ([`crate::vm`]), compilados pelo backend nativo do próprio DartForge
//! (`dartforge compile-native`, o `emit_native`) num executável que fala
//! `dfexec/1` por stdio. Nenhuma VM Dart entra no caminho.
//!
//! O executável fica em cache no diretório de trabalho, pela chave de tudo o
//! que ele compila: o compilador (caminho, tamanho e data do executável), o
//! SDK, e o conteúdo de cada arquivo que o programa do bootstrap carrega fora
//! do SDK — a API de macros, as bibliotecas das macros e o que elas
//! importam. Mudou qualquer um, compila de novo.
use crate::aplicacoes::Aplicacao;
use crate::executor::ExecutorDfexec;
use crate::protocolo::CanalDeProcesso;
use std::path::{Path, PathBuf};

/// Quem compila e onde.
#[derive(Debug, Clone)]
pub struct ConfigNativa {
    /// O `dartforge` com o backend nativo (a feature `nativo`).
    pub compilador: PathBuf,
    /// O `lib/` do SDK (o mesmo que o `compile-native` recebe em `--sdk`).
    pub sdk_lib: PathBuf,
    /// A raiz do pacote da nossa API (`pacotes/macros`).
    pub api: PathBuf,
    /// Onde gravar o bootstrap, o `package_config.json` e o executável.
    pub trabalho: PathBuf,
}

/// Grava o bootstrap, compila (ou reaproveita) o executável nativo e o
/// inicia.
///
/// # Erros
///
/// Escrita no diretório de trabalho, carga do programa do bootstrap,
/// compilação (a mensagem do compilador vai no erro) ou início do processo.
pub fn iniciar(
    cfg: &ConfigNativa,
    apps: &[Aplicacao],
    package_config_do_projeto: Option<&Path>,
) -> Result<ExecutorDfexec<CanalDeProcesso>, String> {
    let exe = compilar(cfg, apps, package_config_do_projeto)?;
    Ok(ExecutorDfexec::novo(CanalDeProcesso::iniciar(std::process::Command::new(&exe))?))
}

/// O executável nativo do bootstrap das `apps` (compilado agora ou do
/// cache).
///
/// # Erros
///
/// Os de [`iniciar`], menos o início do processo.
pub fn compilar(
    cfg: &ConfigNativa,
    apps: &[Aplicacao],
    package_config_do_projeto: Option<&Path>,
) -> Result<PathBuf, String> {
    let erro = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    std::fs::create_dir_all(&cfg.trabalho).map_err(|e| erro(&cfg.trabalho, e))?;
    let principal = cfg.trabalho.join("bootstrap.dart");
    let texto = crate::vm::bootstrap(apps);
    if std::fs::read_to_string(&principal).ok().as_deref() != Some(texto.as_str()) {
        std::fs::write(&principal, &texto).map_err(|e| erro(&principal, e))?;
    }
    let pc = cfg.trabalho.join("package_config.json");
    let json = serde_json::to_string_pretty(&crate::vm::package_config(package_config_do_projeto, &cfg.api)?)
        .unwrap_or_default();
    if std::fs::read_to_string(&pc).ok().as_deref() != Some(json.as_str()) {
        std::fs::write(&pc, &json).map_err(|e| erro(&pc, e))?;
    }
    let chave = chave(cfg, &principal, &pc)?;
    let exe = cfg.trabalho.join(format!("executor-{chave}{}", std::env::consts::EXE_SUFFIX));
    if exe.is_file() {
        return Ok(exe);
    }
    let provisorio = cfg.trabalho.join(format!("executor-{chave}.tmp{}", std::env::consts::EXE_SUFFIX));
    let saida = std::process::Command::new(&cfg.compilador)
        .arg("compile-native")
        .arg(&principal)
        .arg("-o")
        .arg(&provisorio)
        .arg("--packages")
        .arg(&pc)
        .arg("--sdk")
        .arg(&cfg.sdk_lib)
        // As macros são da linguagem 3.6 (o experimento saiu das versões
        // seguintes): a versão corrente do programa é a do alvo, 3.6.
        .arg("--versao-linguagem")
        .arg("3.6")
        .arg("--enable-experiment=macros")
        .output()
        .map_err(|e| format!("não foi possível executar {}: {e}", cfg.compilador.display()))?;
    if !saida.status.success() || !provisorio.is_file() {
        return Err(format!(
            "a compilação nativa do executor de macros falhou:\n{}{}",
            String::from_utf8_lossy(&saida.stdout),
            String::from_utf8_lossy(&saida.stderr)
        ));
    }
    std::fs::rename(&provisorio, &exe).map_err(|e| erro(&exe, e))?;
    // Executáveis de chaves antigas não servem mais.
    if let Ok(ls) = std::fs::read_dir(&cfg.trabalho) {
        for e in ls.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("executor-") && !n.contains(&chave) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(exe)
}

/// A chave do executável: o compilador, o SDK e o conteúdo de cada arquivo
/// que o programa do bootstrap carrega fora do SDK (a carga é a do
/// `elements`, com o experimento de macros, a mesma que o `compile-native`
/// faz).
fn chave(cfg: &ConfigNativa, principal: &Path, pc: &Path) -> Result<String, String> {
    let mut h = blake3::Hasher::new();
    let meta = std::fs::metadata(&cfg.compilador).map_err(|e| format!("{}: {e}", cfg.compilador.display()))?;
    let data = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    h.update(format!("{}|{}|{data}\n", cfg.compilador.display(), meta.len()).as_bytes());
    h.update(cfg.sdk_lib.to_string_lossy().as_bytes());
    let mut sdk = dartforge_elements::sdk::SdkLayout::load(&cfg.sdk_lib, "vm")?;
    sdk.versao_corrente = dartforge_frontend::LanguageVersion::new(3, 6);
    sdk.experimentos.push(dartforge_frontend::Feature::Macros);
    let mut nomes = dartforge_intern::Interner::new();
    let (programa, diags) = dartforge_elements::load::load_lenient(principal, &sdk, Some(pc), &mut nomes);
    if let Some(d) = diags.first() {
        return Err(format!("o bootstrap do executor de macros não carrega: {d}"));
    }
    let mut arquivos: Vec<&Path> = programa
        .units
        .iter()
        .filter(|u| !programa.library(u.library).is_sdk)
        .filter_map(|u| u.path.as_deref())
        .collect();
    arquivos.sort();
    arquivos.dedup();
    for a in arquivos {
        let bytes = std::fs::read(a).map_err(|e| format!("{}: {e}", a.display()))?;
        h.update(a.to_string_lossy().as_bytes());
        h.update(b"\0");
        h.update(blake3::hash(&bytes).as_bytes());
    }
    Ok(h.finalize().to_hex()[..16].to_string())
}
