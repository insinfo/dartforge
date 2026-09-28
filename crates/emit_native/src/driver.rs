//! Driver de compilação LLVM IR -> Clang -> Link com o runtime em cache.

use crate::alvo::Sistema;
use crate::cache::{RuntimeCache, dir_cache_nativo};
use crate::cache_objeto::{self, CacheObjeto};
use crate::gerador::{Geracao, Gerador};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NativeDriverOptions {
    pub clang: PathBuf,
    pub optimize: bool,
    pub timings: bool,
    /// J05: o objeto leva as tabelas de linha; no Windows a ligação gera o
    /// PDB (`/DEBUG`), que é onde o depurador as procura.
    pub depuracao: bool,
}

impl Default for NativeDriverOptions {
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG").map_or_else(clang_padrao, PathBuf::from),
            optimize: false,
            timings: false,
            depuracao: false,
        }
    }
}

/// O Clang quando `DARTFORGE_CLANG` não está definido: o da distribuição
/// (`lib/llvm/bin`), o `bin` de `DARTFORGE_LLVM_DIR`/`LLVM_SYS_221_PREFIX`,
/// e por fim o `clang` do `PATH` (`scripts/env.ps1` aponta o da máquina de
/// desenvolvimento no Windows).
fn clang_padrao() -> PathBuf {
    if let Some(c) = dartforge_elements::distribuicao::ferramenta_llvm("clang") {
        return c;
    }
    for var in ["DARTFORGE_LLVM_DIR", "LLVM_SYS_221_PREFIX"] {
        if let Some(d) = std::env::var_os(var) {
            let c = PathBuf::from(d).join("bin").join(crate::alvo::nome_clang());
            if c.is_file() {
                return c;
            }
        }
    }
    PathBuf::from(crate::alvo::nome_clang())
}

#[derive(Debug, Clone, Default)]
pub struct TimingsReport {
    pub frontend: Duration,
    pub hir: Duration,
    pub llvm_ir: Duration,
    pub clang: Duration,
    pub link: Duration,
    pub total: Duration,
    pub peak_memory_bytes: usize,
}

/// Tempos do Clang e da ligação, e se o objeto veio do cache (aí `clang` é
/// só o tempo de achar a entrada).
#[derive(Debug, Clone, Copy, Default)]
pub struct TemposLigacao {
    pub clang: Duration,
    pub link: Duration,
    pub objeto_do_cache: bool,
}

pub fn compile_and_link(
    llvm_ir: &str,
    output: &Path,
    options: &NativeDriverOptions,
) -> Result<TemposLigacao, String> {
    let runtime = RuntimeCache::get_or_compile()?;
    // Programa com o SDK da fonte (P5c): a entrada chama o registro das
    // bibliotecas do SDK, que moram nos objetos em cache.
    let sdk = if llvm_ir.contains("declare void @df.registrar.") {
        let dir = crate::sdk_do_dart()?;
        let perfil = if options.optimize {
            crate::sdk_modulo::PerfilDoSdk::Producao
        } else {
            crate::sdk_modulo::PerfilDoSdk::Desenvolvimento
        };
        let sdk = crate::sdk_modulo::sdk_compilado_no_perfil(&dir, &options.clang, perfil)?;
        if let Some(t) = sdk.frio
            && options.timings
        {
            eprintln!("  SDK frio:  {t:?} (compilado uma vez por conteúdo)");
        }
        Some(sdk)
    } else {
        None
    };
    // Com o SDK da fonte há dois perfis (docs/NATIVO-PLANO.md §7.9):
    // * desenvolvimento e teste: o runtime e o SDK moram na DLL em cache, e o
    //   executável liga só o objeto do programa e a biblioteca de importação
    //   (ligação rápida; a DLL vai ao lado do executável);
    // * produção (`optimize`): UM executável autocontido — o objeto do
    //   programa, os objetos do SDK (em cache, compilados com
    //   `-ffunction-sections`) e o runtime estático, com `/OPT:REF` tirando o
    //   que o programa não alcança.
    let producao = options.optimize && sdk.is_some();
    let (ligar_com, sdk_objetos): (Ligacao, Vec<PathBuf>) = match &sdk {
        Some(s) if producao => (Ligacao::Producao(crate::cache::RuntimeCache::para_dll()?.lib_path), s.objetos.clone()),
        Some(s) => (Ligacao::SdkCompartilhado(s.importacao.clone()), Vec::new()),
        None => (Ligacao::Runtime(runtime.lib_path.clone()), Vec::new()),
    };

    // Diretório temporário seguro no target. Com o cache de objeto ele só é
    // usado com DARTFORGE_KEEP_IR ou se a ligação recusar o objeto do cache,
    // e só é criado nesses casos.
    let staging = output.parent().unwrap_or(Path::new(".")).join(".df_tmp");
    let criar_staging = || {
        std::fs::create_dir_all(&staging)
            .map_err(|e| format!("não foi possível criar diretório temporário {}: {e}", staging.display()))
    };

    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ll_file = staging.join(format!("{stem}.ll"));
    let manter_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    // Produção com o SDK da fonte: bitcode, otimizado junto com o do SDK na
    // ligação (LTO).
    let gerador = Gerador::escolher(&options.clang);
    let geracao = Geracao::do_programa(options.optimize, producao);
    if manter_ir {
        // A cópia em `.df_tmp` só existe para quem pediu DARTFORGE_KEEP_IR
        // (o `determinismo --executar` do harness lê essas cópias).
        criar_staging()?;
        std::fs::write(&ll_file, llvm_ir)
            .map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll_file.display()))?;
    }

    // Fase 1: o gerador compila LLVM IR -> objeto, ou o cache já tem o
    // objeto deste IR com este gerador e esta geração.
    let t_clang = Instant::now();
    let cache = CacheObjeto::do_ambiente();
    let obj_staging = || -> Result<PathBuf, String> {
        criar_staging()?;
        Ok(staging.join(format!("{stem}.{}", crate::alvo::ext_objeto())))
    };
    let (mut obj_file, mut do_cache) = match cache {
        Some(c) => {
            let descricao = geracao.descricao();
            let chave = cache_objeto::chave(llvm_ir, &gerador.identidade()?, &[descricao.as_str()]);
            let (obj, acerto) = c.obter_ou_criar(chave, |tmp| gerador.gerar(llvm_ir, geracao, tmp))?;
            (obj, Some((c, chave, acerto)))
        }
        None => {
            let obj = obj_staging()?;
            gerador.gerar(llvm_ir, geracao, &obj)?;
            (obj, None)
        }
    };
    let clang_duration = t_clang.elapsed();

    // Fase 2: Link do objeto com o runtime estático
    let t_link = Instant::now();
    let mut ligou = ligar(&options.clang, &obj_file, &sdk_objetos, &ligar_com, output, options.depuracao);
    if ligou.is_err()
        && let Some((c, chave, true)) = do_cache
    {
        // Um objeto do cache que o ligador recusa não pode ficar lá: sai do
        // cache, e a ligação é refeita uma vez com um objeto novo.
        c.remover(chave);
        obj_file = obj_staging()?;
        gerador.gerar(llvm_ir, geracao, &obj_file)?;
        do_cache = None;
        ligou = ligar(&options.clang, &obj_file, &sdk_objetos, &ligar_com, output, options.depuracao);
    }
    ligou?;
    if let Some(s) = sdk.as_ref().filter(|_| !producao) {
        // A biblioteca compartilhada ao lado do executável (o Windows procura
        // primeiro ali; no Linux e no macOS o executável leva o `rpath` do
        // próprio diretório): ligação física, sem cópia; cópia só se o volume
        // for outro.
        let destino = output.parent().unwrap_or(Path::new(".")).join(s.dll.file_name().unwrap_or_default());
        if !destino.is_file() && std::fs::hard_link(&s.dll, &destino).is_err() {
            std::fs::copy(&s.dll, &destino).map_err(|e| format!("não foi possível pôr a DLL do SDK em {}: {e}", destino.display()))?;
        }
    }
    let link_duration = t_link.elapsed();

    // Limpeza de arquivos temporários (mantém se DARTFORGE_KEEP_IR estiver
    // definido); um objeto do cache nunca é apagado aqui.
    for p in arquivos_a_remover(&ll_file, &obj_file, &dir_cache_nativo(), manter_ir) {
        let _ = std::fs::remove_file(p);
    }

    Ok(TemposLigacao {
        clang: clang_duration,
        link: link_duration,
        objeto_do_cache: matches!(do_cache, Some((_, _, true))),
    })
}

/// Com o que o objeto do programa é ligado.
enum Ligacao {
    /// Sem o SDK da fonte: o runtime estático (com o `main` C).
    Runtime(PathBuf),
    /// Desenvolvimento com o SDK da fonte: a biblioteca compartilhada (runtime
    /// e SDK) — no Windows, a biblioteca de importação da DLL; nos outros, o
    /// próprio `.so`/`.dylib`.
    SdkCompartilhado(PathBuf),
    /// Produção com o SDK da fonte: o runtime estático sem o `main` C; os
    /// objetos do SDK vêm à parte.
    Producao(PathBuf),
}

impl Ligacao {
    fn biblioteca(&self) -> &Path {
        match self {
            Ligacao::Runtime(p) | Ligacao::SdkCompartilhado(p) | Ligacao::Producao(p) => p,
        }
    }
}

/// A ligação no Linux: o `ld.lld` direto, com o sysroot de ligação
/// (`ligador.rs`) — sem o driver do Clang nem o GCC na máquina.
fn ligar_no_linux(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool) -> Result<(), String> {
    use crate::ligador;
    let sysroot = ligador::SysrootLinux::localizar(clang)?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    ligador::ligar(
        &ligador::ld_lld(clang),
        sysroot,
        &ligador::Ligacao {
            produto: ligador::Produto::Executavel,
            entradas,
            rpath_origem: matches!(ligacao, Ligacao::SdkCompartilhado(_)),
            lto: producao,
            podar: producao,
            manter_depuracao: depuracao,
            saida: output,
        },
    )
}

/// A ligação no Windows: o `lld-link` direto, com as bibliotecas de
/// importação e a CRT mínima geradas pelo dartforge (`ligador_windows.rs`) —
/// sem o Visual Studio nem o Windows SDK na máquina (N15).
fn ligar_no_windows(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool) -> Result<(), String> {
    use crate::ligador_windows as lw;
    let sysroot = lw::SysrootWindows::localizar(clang)?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    lw::ligar(
        &lw::lld_link(clang),
        sysroot,
        &lw::Ligacao { produto: lw::Produto::Executavel, entradas, lto: producao, podar: producao, depuracao, saida: output },
    )
}

/// A ligação no macOS: o `ld64.lld` direto, com os `.tbd` do sysroot de
/// ligação (`ligador_macos.rs`) — sem o `xcrun`, o `ld` da Apple nem as
/// Command Line Tools na máquina (N16).
fn ligar_no_macos(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool) -> Result<(), String> {
    use crate::ligador_macos as lm;
    let sysroot = lm::SysrootMacos::localizar()?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    lm::ligar(
        &lm::ld64_lld(clang),
        sysroot,
        &lm::Ligacao {
            produto: lm::Produto::Executavel,
            entradas,
            rpath_executavel: matches!(ligacao, Ligacao::SdkCompartilhado(_)),
            lto: producao,
            podar: producao,
            manter_depuracao: depuracao,
            saida: output,
        },
    )
}

/// Liga o objeto do programa: o ligador do LLVM direto em cada sistema, com
/// o que o sistema exige vindo do dartforge (nenhum driver de C, nenhum
/// toolchain do sistema). O `lld` é o do mesmo LLVM do gerador — o bitcode
/// da produção (LTO) só é lido pela mesma versão.
fn ligar(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool) -> Result<(), String> {
    match crate::alvo::sistema() {
        Sistema::Linux => ligar_no_linux(clang, obj, sdk, ligacao, output, depuracao),
        Sistema::Windows => ligar_no_windows(clang, obj, sdk, ligacao, output, depuracao),
        Sistema::MacOs => ligar_no_macos(clang, obj, sdk, ligacao, output, depuracao),
    }
}

/// O que a limpeza depois da ligação apaga: nada com `manter_ir`, e nunca um
/// caminho dentro do cache — o objeto que veio de lá é de todos.
fn arquivos_a_remover(ll: &Path, obj: &Path, dir_cache: &Path, manter_ir: bool) -> Vec<PathBuf> {
    if manter_ir {
        return Vec::new();
    }
    [ll, obj].into_iter().filter(|p| !p.starts_with(dir_cache)).map(Path::to_path_buf).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limpeza_nunca_apaga_o_cache() {
        let cache = Path::new("D:/x/target/native_cache");
        let ll = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.ll");
        let obj_cache = cache.join("obj/ab/ab00.obj");
        let obj_local = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.obj");
        assert_eq!(arquivos_a_remover(ll, &obj_cache, cache, false), vec![ll.to_path_buf()]);
        assert_eq!(arquivos_a_remover(ll, obj_local, cache, false), vec![ll.to_path_buf(), obj_local.to_path_buf()]);
        assert!(arquivos_a_remover(ll, obj_local, cache, true).is_empty());
    }
}
