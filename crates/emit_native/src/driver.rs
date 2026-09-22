//! Driver de compilação LLVM IR -> Clang -> Link com o runtime em cache.

use crate::cache::RuntimeCache;
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

pub fn compile_and_link(
    llvm_ir: &str,
    output: &Path,
    options: &NativeDriverOptions,
) -> Result<(Duration, Duration), String> {
    let runtime = RuntimeCache::get_or_compile()?;

    // Diretório temporário seguro no target
    let staging = output.parent().unwrap_or(Path::new(".")).join(".df_tmp");
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("não foi possível criar diretório temporário {}: {e}", staging.display()))?;

    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ll_file = staging.join(format!("{stem}.ll"));
    let obj_file = staging.join(format!("{stem}.obj"));

    std::fs::write(&ll_file, llvm_ir)
        .map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll_file.display()))?;

    // Fase 1: Clang compila LLVM IR -> Objeto
    let t_clang = Instant::now();
    let opt_flag = if options.optimize { "-O2" } else { "-O0" };
    let clang_status = Command::new(&options.clang)
        .arg("-x")
        .arg("ir")
        .arg("-c")
        .arg(opt_flag)
        .arg(&ll_file)
        .arg("-o")
        .arg(&obj_file)
        .status()
        .map_err(|e| format!("falha ao executar Clang em {:?}: {e}", options.clang))?;

    if !clang_status.success() {
        return Err(format!("Clang falhou na compilação do IR (status {clang_status:?})"));
    }
    let clang_duration = t_clang.elapsed();

    // Fase 2: Link do objeto com o runtime estático
    let t_link = Instant::now();
    let link_status = Command::new(&options.clang)
        .arg(&obj_file)
        .arg(&runtime.lib_path)
        .arg("-lws2_32")
        .arg("-luserenv")
        .arg("-lntdll")
        .arg("-o")
        .arg(output)
        .status()
        .map_err(|e| format!("falha na ligação com Clang em {:?}: {e}", options.clang))?;

    if !link_status.success() {
        return Err(format!("Clang falhou na ligação do executável (status {link_status:?})"));
    }
    let link_duration = t_link.elapsed();

    // Limpeza de arquivos temporários
    let _ = std::fs::remove_file(&ll_file);
    let _ = std::fs::remove_file(&obj_file);

    Ok((clang_duration, link_duration))
}

