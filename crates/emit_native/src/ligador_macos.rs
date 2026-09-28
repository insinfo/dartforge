//! A ligação no macOS pelo `ld64.lld` direto, sem o driver do Clang, o `ld`
//! da Apple nem as Command Line Tools (ou o Xcode) na máquina de quem usa o
//! dartforge (N16).
//!
//! O driver do Clang só dava à ligação a raiz do SDK do macOS (achada pelo
//! `xcrun`), de onde vêm os *stubs* `.tbd` das bibliotecas do sistema
//! (`libSystem`, `libiconv`, os frameworks `CoreFoundation` e `Security`), e
//! as versões de plataforma. Aqui os `.tbd` vêm de um **sysroot de ligação**
//! ([`SysrootMacos`]): o da distribuição (`lib/sysroot/<arch>-apple-darwin/`,
//! copiado pelo `dartforge empacotar` do SDK da máquina que montou a
//! distribuição, como o sysroot do Linux), ou, numa árvore de
//! desenvolvimento, o SDK do sistema (`SDKROOT` ou `xcrun`). Um `.tbd` é
//! texto: a lista dos símbolos e o caminho de instalação da biblioteca; o
//! programa carrega as do sistema ao rodar.

use std::path::{Path, PathBuf};
use std::process::Command;

/// O triple do sysroot deste hospedeiro.
pub fn triple_do_sysroot() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64-apple-darwin" } else { "x86_64-apple-darwin" }
}

/// A arquitetura do `-arch` e a versão mínima do macOS (as do `rustc` para o
/// alvo: 11.0 no arm64, 10.12 no x86-64).
fn arquitetura_e_minimo() -> (&'static str, &'static str) {
    if cfg!(target_arch = "aarch64") { ("arm64", "11.0") } else { ("x86_64", "10.12") }
}

/// Os `.tbd` que a ligação usa, relativos à raiz do SDK: o nome com que o
/// `ld64.lld` os procura (`-lSystem` → `usr/lib/libSystem.tbd`) e os que o
/// caminho de instalação indica (reexportações).
pub const ARQUIVOS_DO_SYSROOT: &[&str] = &[
    "usr/lib/libSystem.tbd",
    "usr/lib/libSystem.B.tbd",
    "usr/lib/libiconv.tbd",
    "usr/lib/libiconv.2.tbd",
    "System/Library/Frameworks/CoreFoundation.framework/CoreFoundation.tbd",
    "System/Library/Frameworks/Security.framework/Security.tbd",
];

/// O arquivo com a versão do SDK de onde os `.tbd` vieram (o
/// `-platform_version`).
const ARQUIVO_VERSAO: &str = "versao-do-sdk";

/// As bibliotecas do sistema, no formato do `ld64.lld`: as de
/// `alvo::bibliotecas_do_sistema` (`-lc` e `-lm` são a própria `libSystem`).
const BIBLIOTECAS: &[&str] = &["-lSystem", "-liconv", "-framework", "CoreFoundation", "-framework", "Security"];

/// A raiz de um SDK do macOS com os `.tbd`, e a versão dele.
#[derive(Debug, Clone)]
pub struct SysrootMacos {
    raiz: PathBuf,
    versao: String,
}

impl SysrootMacos {
    /// O da distribuição, senão o SDK do sistema.
    ///
    /// # Erros
    ///
    /// Nem distribuição nem SDK (sem Xcode nem Command Line Tools).
    pub fn localizar() -> Result<&'static SysrootMacos, String> {
        static S: std::sync::OnceLock<Result<SysrootMacos, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            if let Some(dir) = dartforge_elements::distribuicao::em_lib(&format!("sysroot/{}", triple_do_sysroot())) {
                let versao = std::fs::read_to_string(dir.join(ARQUIVO_VERSAO)).unwrap_or_default().trim().to_string();
                return Ok(SysrootMacos { raiz: dir, versao });
            }
            Self::do_sistema()
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    /// O SDK do sistema (`SDKROOT` ou `xcrun --show-sdk-path`).
    ///
    /// # Erros
    ///
    /// Sem SDK, ou sem a `libSystem` nele.
    pub fn do_sistema() -> Result<SysrootMacos, String> {
        let raiz = crate::alvo::raiz_do_sdk_macos().ok_or(
            "SDK do macOS não encontrado: instale as Command Line Tools (xcode-select --install) \
             ou use a distribuição do dartforge (que traz os .tbd de ligação)",
        )?;
        if !raiz.join("usr/lib/libSystem.tbd").is_file() {
            return Err(format!("{} não tem usr/lib/libSystem.tbd", raiz.display()));
        }
        Ok(SysrootMacos { raiz: raiz.to_path_buf(), versao: versao_do_sdk(raiz) })
    }

    /// Copia os `.tbd` para `destino` (o `dartforge empacotar`), com as
    /// bibliotecas que eles reexportam sem trazer no mesmo arquivo.
    ///
    /// # Erros
    ///
    /// Arquivo ausente no SDK ou falha de escrita.
    pub fn copiar_para(&self, destino: &Path) -> Result<(), String> {
        let mut pendentes: Vec<String> = ARQUIVOS_DO_SYSROOT.iter().map(|s| s.to_string()).collect();
        let mut feitos = std::collections::BTreeSet::new();
        while let Some(rel) = pendentes.pop() {
            if !feitos.insert(rel.clone()) {
                continue;
            }
            let de = self.raiz.join(&rel);
            let texto = std::fs::read_to_string(&de).map_err(|e| format!("{}: {e}", de.display()))?;
            let para = destino.join(&rel);
            if let Some(p) = para.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            std::fs::write(&para, &texto).map_err(|e| format!("{}: {e}", para.display()))?;
            for r in reexportacoes_externas(&texto) {
                if self.raiz.join(&r).is_file() {
                    pendentes.push(r);
                }
            }
        }
        std::fs::write(destino.join(ARQUIVO_VERSAO), format!("{}\n", self.versao)).map_err(|e| e.to_string())
    }
}

/// A versão de um SDK (`SDKSettings.json`, `"Version": "15.2"`); a mínima
/// quando não há.
fn versao_do_sdk(raiz: &Path) -> String {
    let texto = std::fs::read_to_string(raiz.join("SDKSettings.json")).unwrap_or_default();
    let achada = texto.split("\"Version\"").nth(1).and_then(|r| r.split('"').nth(1)).map(str::to_string);
    achada.filter(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.') && !v.is_empty()).unwrap_or_default()
}

/// Os `.tbd` (relativos à raiz do SDK) das bibliotecas citadas num `.tbd`
/// que não são documentos dele mesmo: o `ld64.lld` procura uma reexportação
/// pelo caminho de instalação dentro do `-syslibroot`.
fn reexportacoes_externas(texto: &str) -> Vec<String> {
    let proprios: Vec<&str> = texto
        .lines()
        .filter_map(|l| l.trim().strip_prefix("install-name:"))
        .map(|v| v.trim().trim_matches(|c| c == '\'' || c == '"'))
        .collect();
    let mut saida = Vec::new();
    for pedaco in texto.split(['\'', '"', ' ', ',', '[', ']', '\n']) {
        let caminho = pedaco.trim();
        if !(caminho.starts_with("/usr/lib/") || caminho.starts_with("/System/Library/")) || proprios.contains(&caminho) {
            continue;
        }
        let rel = caminho.trim_start_matches('/');
        let tbd = match rel.strip_suffix(".dylib") {
            Some(base) => format!("{base}.tbd"),
            None => format!("{rel}.tbd"),
        };
        if !saida.contains(&tbd) {
            saida.push(tbd);
        }
    }
    saida
}

/// O que se liga.
pub enum Produto<'a> {
    /// Um executável.
    Executavel,
    /// Uma biblioteca dinâmica com o `install_name` dado, exportando (e
    /// trazendo das `staticlib`) os símbolos dados.
    Dinamica { install_name: &'a str, exportados: &'a [String] },
}

/// Uma ligação pelo `ld64.lld`.
pub struct Ligacao<'a> {
    pub produto: Produto<'a>,
    /// Objetos e bibliotecas do programa, na ordem.
    pub entradas: Vec<PathBuf>,
    /// Procurar as bibliotecas dinâmicas ao lado do executável.
    pub rpath_executavel: bool,
    /// LTO dos bitcodes de entrada (produção).
    pub lto: bool,
    /// Tirar o que nada alcança (`-dead_strip`) e os símbolos locais (produção).
    pub podar: bool,
    /// Manter o mapa de depuração mesmo podando (J05).
    pub manter_depuracao: bool,
    pub saida: &'a Path,
}

/// O `ld64.lld`: o da distribuição, o ao lado do Clang, ou o do `PATH`.
pub fn ld64_lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribuicao::ferramenta_llvm("ld64.lld") {
        return l;
    }
    let ao_lado = clang.with_file_name("ld64.lld");
    if ao_lado.is_file() { ao_lado } else { PathBuf::from("ld64.lld") }
}

/// Liga com o `ld64.lld` e os `.tbd` do sysroot.
///
/// # Erros
///
/// O `ld64.lld` não executou ou recusou a ligação (a mensagem leva o que ele
/// disse).
pub fn ligar(ld: &Path, sysroot: &SysrootMacos, l: &Ligacao<'_>) -> Result<(), String> {
    let (arch, minimo) = arquitetura_e_minimo();
    let minimo = std::env::var("MACOSX_DEPLOYMENT_TARGET").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| minimo.to_string());
    let versao_sdk = if sysroot.versao.is_empty() { minimo.clone() } else { sysroot.versao.clone() };
    let mut cmd = Command::new(ld);
    cmd.args(["-arch", arch, "-platform_version", "macos", &minimo, &versao_sdk]);
    cmd.arg("-syslibroot").arg(&sysroot.raiz);
    match &l.produto {
        Produto::Executavel => {
            cmd.args(["-execute", "-dynamic"]);
        }
        Produto::Dinamica { install_name, exportados } => {
            cmd.args(["-dylib", "-install_name", install_name]);
            let mut rsp = String::new();
            for n in exportados.iter() {
                rsp.push_str(&format!("-u _{n}\n"));
            }
            let arquivo = l.saida.with_extension("rsp");
            std::fs::write(&arquivo, rsp).map_err(|e| format!("{}: {e}", arquivo.display()))?;
            cmd.arg(format!("@{}", arquivo.display()));
        }
    }
    cmd.arg("-o").arg(l.saida);
    if l.rpath_executavel {
        cmd.args(["-rpath", "@executable_path"]);
    }
    if l.lto {
        cmd.arg("--lto-O2");
    }
    if l.podar {
        cmd.arg("-dead_strip");
        // `-S` tira o mapa de depuração (J05); `-x`, os símbolos locais.
        if !crate::ligador::manter_simbolos() && !l.manter_depuracao {
            cmd.args(["-S", "-x"]);
        }
    }
    cmd.args(&l.entradas);
    cmd.args(BIBLIOTECAS);
    let saida = cmd.output().map_err(|e| format!("falha ao executar {}: {e}", ld.display()))?;
    if !saida.status.success() {
        let texto = String::from_utf8_lossy(&saida.stderr);
        let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("o ld64.lld falhou na ligação ({}):\n{}", saida.status, linhas.join("\n")));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reexportacoes_fora_do_proprio_arquivo() {
        let tbd = "--- !tapi-tbd\ninstall-name: '/usr/lib/libSystem.B.dylib'\nreexported-libraries:\n  - targets: [ arm64-macos ]\n    libraries: [ '/usr/lib/system/libcache.dylib', '/usr/lib/libobjc.A.dylib' ]\n--- !tapi-tbd\ninstall-name: '/usr/lib/system/libcache.dylib'\n...\n";
        assert_eq!(reexportacoes_externas(tbd), vec!["usr/lib/libobjc.A.tbd".to_string()]);
        let fw = "install-name: '/System/Library/Frameworks/A.framework/Versions/A/A'\nreexported-libraries:\n  - libraries: [ '/System/Library/Frameworks/B.framework/Versions/A/B' ]\n";
        assert_eq!(reexportacoes_externas(fw), vec!["System/Library/Frameworks/B.framework/Versions/A/B.tbd".to_string()]);
    }

    #[test]
    fn versao_do_sdk_do_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("SDKSettings.json"), "{\"CanonicalName\":\"macosx15.2\",\"Version\":\"15.2\"}").unwrap();
        assert_eq!(versao_do_sdk(dir.path()), "15.2");
        assert_eq!(versao_do_sdk(Path::new("/nao/existe")), "");
    }
}
