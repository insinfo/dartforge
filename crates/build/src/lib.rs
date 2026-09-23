//! Motor de geração de código do DartForge — o substituto do `build_runner`
//! no ciclo de desenvolvimento. Contrato: `docs/BUILD-MOTOR.md`; protocolo
//! do executor Dart: `docs/BUILD-PROTOCOLO.md`.
//!
//! O motor faz duas perguntas — **quando** (plano de fases, agenda,
//! impressão digital) e **o quê** (executores) — e publica o resultado numa
//! `Geracao` em memória (`dartforge_elements::gerado`).
//!
//! **Custo zero para quem não usa** (PLANO.md, regra governante): nada deste
//! crate é construído se o `package_config.json` não tem `build_runner`
//! ([`detectar`]); [`instancias`] conta os motores criados, para o portão.
pub mod config;
pub mod descritor;
pub mod extensoes;
pub mod glob;
pub mod oraculo;
pub mod pacotes;
pub mod plano;
pub mod valor;

use dartforge_elements::config::PackageConfig;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Versão do motor: entra na impressão digital de toda ação.
pub const VERSAO: &str = concat!("dartforge-build/", env!("CARGO_PKG_VERSION"), "/1");

static INSTANCIAS: AtomicUsize = AtomicUsize::new(0);

/// Quantos motores este processo já construiu (o portão de custo zero exige 0
/// num projeto sem builders).
pub fn instancias() -> usize {
    INSTANCIAS.load(Ordering::Relaxed)
}

pub(crate) fn contar_instancia() {
    INSTANCIAS.fetch_add(1, Ordering::Relaxed);
}

/// O projeto usa builders? Exatamente a condição em que a toolchain oficial
/// roda algum: `build_runner` resolvido no `package_config.json`. Custa uma
/// busca num `HashMap` já carregado.
pub fn detectar(cfg: &PackageConfig) -> bool {
    cfg.packages.contains_key("build_runner")
}

/// Diretório do pacote raiz: o do `pubspec.yaml` mais próximo acima de
/// `caminho` (arquivo ou diretório).
pub fn raiz_do_pacote(caminho: &Path) -> Option<PathBuf> {
    let inicio = if caminho.is_dir() { caminho } else { caminho.parent()? };
    inicio.ancestors().find(|d| d.join("pubspec.yaml").is_file()).map(Path::to_path_buf)
}

/// O plano do `build.dart` de um projeto (a lista de aplicações), com o
/// grafo de pacotes e as configurações lidas.
pub fn plano_do_projeto(
    raiz: &Path,
    cfg: &PackageConfig,
) -> Result<(pacotes::GrafoPacotes, plano::Configs, plano::Plano), String> {
    let grafo = pacotes::GrafoPacotes::montar(raiz, cfg, None)?;
    let configs = plano::Configs::ler(&grafo, false)?;
    let p = plano::Plano::do_script(&grafo, &configs)?;
    Ok((grafo, configs, p))
}
