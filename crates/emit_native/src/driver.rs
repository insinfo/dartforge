//! Driver de compilação LLVM IR -> Clang -> Link com o runtime em cache.

use crate::cache::{RuntimeCache, dir_cache_nativo};
use crate::cache_objeto::{self, CacheObjeto};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NativeDriverOptions {
    pub clang: PathBuf,
    pub optimize: bool,
    pub timings: bool,
}

impl Default for NativeDriverOptions {
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG")
                .map_or_else(|| PathBuf::from("D:/LLVM/22.1.8/bin/clang.exe"), PathBuf::from),
            optimize: false,
            timings: false,
        }
    }
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
        let dir = dartforge_elements::sdk::SdkLayout::discover()
            .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
        let sdk = crate::sdk_modulo::sdk_compilado(&dir, &options.clang)?;
        if let Some(t) = sdk.frio
            && options.timings
        {
            eprintln!("  SDK frio:  {t:?} (compilado uma vez por conteúdo)");
        }
        Some(sdk)
    } else {
        None
    };
    // Com o SDK da fonte, o runtime mora na DLL do SDK: o executável liga o
    // objeto do programa e a biblioteca de importação dela.
    let (ligar_com, sdk_objetos): (PathBuf, Vec<PathBuf>) = match &sdk {
        Some(s) => (s.importacao.clone(), Vec::new()),
        None => (runtime.lib_path.clone(), Vec::new()),
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
    let opt_flag = if options.optimize { "-O2" } else { "-O0" };
    // `-mno-incremental-linker-compatible` zera o TimeDateStamp do cabeçalho
    // COFF: sem ele, o mesmo IR dava objetos diferentes no byte 4 (medido),
    // e o objeto deixava de ser função da chave.
    let args = ["-x", "ir", "-c", opt_flag, "-mno-incremental-linker-compatible"];

    // Fase 1: Clang compila LLVM IR -> Objeto, ou o cache já tem o objeto
    // deste IR com este Clang e estas bandeiras.
    let t_clang = Instant::now();
    let cache = CacheObjeto::do_ambiente();
    let (mut obj_file, mut do_cache) = match cache {
        Some(c) => {
            // Com cache, o `.ll` vive no diretório temporário da entrada; a
            // cópia em `.df_tmp` só existe para quem pediu DARTFORGE_KEEP_IR
            // (o `determinismo --executar` do harness lê essas cópias).
            if manter_ir {
                criar_staging()?;
                std::fs::write(&ll_file, llvm_ir)
                    .map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll_file.display()))?;
            }
            let chave = cache_objeto::chave(llvm_ir, &cache_objeto::identidade_clang(&options.clang)?, &args);
            let (obj, acerto) = c.obter_ou_criar(chave, |tmp| compilar_objeto(&options.clang, &args, llvm_ir, tmp))?;
            (obj, Some((c, chave, acerto)))
        }
        None => {
            criar_staging()?;
            let obj = staging.join(format!("{stem}.obj"));
            compilar_objeto(&options.clang, &args, llvm_ir, &obj)?;
            (obj, None)
        }
    };
    let clang_duration = t_clang.elapsed();

    // Fase 2: Link do objeto com o runtime estático
    let t_link = Instant::now();
    let mut ligou = ligar(&options.clang, &obj_file, &sdk_objetos, &ligar_com, output);
    if ligou.is_err()
        && let Some((c, chave, true)) = do_cache
    {
        // Um objeto do cache que o ligador recusa não pode ficar lá: sai do
        // cache, e a ligação é refeita uma vez com um objeto novo.
        c.remover(chave);
        criar_staging()?;
        obj_file = staging.join(format!("{stem}.obj"));
        compilar_objeto(&options.clang, &args, llvm_ir, &obj_file)?;
        do_cache = None;
        ligou = ligar(&options.clang, &obj_file, &sdk_objetos, &ligar_com, output);
    }
    ligou?;
    if let Some(s) = &sdk {
        // A DLL ao lado do executável (o Windows procura primeiro ali):
        // ligação física, sem cópia; cópia só se o volume for outro.
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

/// Escreve o IR ao lado de `obj` e compila com o Clang. O Clang roda no
/// diretório do objeto com nomes relativos, para que o `source_filename` e o
/// `.file` do objeto não carreguem o caminho de quem compilou: com o cache,
/// o nome é o hash, e o mesmo IR dá o mesmo objeto byte a byte.
fn compilar_objeto(clang: &Path, args: &[&str], llvm_ir: &str, obj: &Path) -> Result<(), String> {
    let ll = obj.with_extension("ll");
    std::fs::write(&ll, llvm_ir).map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll.display()))?;
    let dir = obj.parent().unwrap_or(Path::new("."));
    let status = Command::new(clang)
        .current_dir(dir)
        .args(args)
        .arg(ll.file_name().unwrap_or_default())
        .arg("-o")
        .arg(obj.file_name().unwrap_or_default())
        .status()
        .map_err(|e| format!("falha ao executar Clang em {clang:?}: {e}"))?;
    if !status.success() {
        return Err(format!("Clang falhou na compilação do IR (status {status:?})"));
    }
    Ok(())
}

fn ligar(clang: &Path, obj: &Path, sdk: &[PathBuf], runtime_lib: &Path, output: &Path) -> Result<(), String> {
    let mut cmd = Command::new(clang);
    cmd.arg(obj).args(sdk).arg(runtime_lib);
    if runtime_lib.file_name().is_some_and(|n| n.to_string_lossy().starts_with("dfsdk_")) {
        // Executável do SDK da fonte: o runtime está na DLL, que usa a CRT
        // dinâmica (a do `rustc`); o executável usa a mesma.
        cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt"]);
    } else {
        cmd.args(["-lws2_32", "-luserenv", "-lntdll"]);
    }
    let status = cmd
        .arg("-o")
        .arg(output)
        .status()
        .map_err(|e| format!("falha na ligação com Clang em {clang:?}: {e}"))?;
    if !status.success() {
        return Err(format!("Clang falhou na ligação do executável (status {status:?})"));
    }
    Ok(())
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
