//! `dartforge empacotar <destino>`: monta a distribuição do dartforge — o
//! executável e tudo o que ele precisa em tempo de execução, no leiaute de
//! `dartforge_elements::distribuicao` — a partir da árvore de desenvolvimento
//! que compilou este binário. Quem recebe a distribuição não instala Rust,
//! LLVM nem o SDK do Dart.
//!
//! ```text
//! bin/dartforge[.exe]            este executável (e a LLVM-C.dll do JIT no Windows)
//! lib/dartforge-distribuicao.json
//! lib/runtime/                   as duas staticlib do runtime (build.rs do emit_native)
//! lib/sdk_nativo/                a sobreposição do backend nativo
//! lib/dart-sdk/{lib,version}     as bibliotecas do SDK do Dart
//! lib/llvm/bin/                  o lld (e, fora do Linux, o Clang, driver da ligação)
//! lib/sysroot/<triple>/          no Linux: a glibc e a libgcc para a ligação
//! ```
use std::path::{Path, PathBuf};

type Resultado = Result<(), Box<dyn std::error::Error>>;

pub fn run(args: &[std::ffi::OsString]) -> Resultado {
    let [destino] = args else {
        return Err("usage: dartforge empacotar <destino>".into());
    };
    let destino = PathBuf::from(destino);
    if destino.exists() && std::fs::read_dir(&destino)?.next().is_some() {
        return Err(format!("{} já existe e não está vazio", destino.display()).into());
    }
    let bin = destino.join("bin");
    let lib = destino.join("lib");
    std::fs::create_dir_all(&bin)?;
    std::fs::create_dir_all(lib.join("runtime"))?;
    std::fs::create_dir_all(lib.join("llvm").join("bin"))?;

    // O executável.
    let exe = std::env::current_exe()?;
    copiar(&exe, &bin.join(exe.file_name().ok_or("executável sem nome")?))?;

    // O runtime pré-compilado (as duas variantes).
    for r in [
        dartforge_emit_native::cache::RuntimeCache::get_or_compile()?,
        dartforge_emit_native::cache::RuntimeCache::para_dll()?,
    ] {
        copiar(&r.lib_path, &lib.join("runtime").join(r.lib_path.file_name().ok_or("runtime sem nome")?))?;
    }

    // A sobreposição e o SDK do Dart.
    copiar_arvore(&dartforge_emit_native::sdk_modulo::dir_sobreposicao(), &lib.join("sdk_nativo"))?;
    let sdk = dartforge_emit_native::sdk_do_dart()?;
    copiar_arvore(&sdk, &lib.join("dart-sdk").join("lib"))?;
    if let Some(raiz) = sdk.parent()
        && raiz.join("version").is_file()
    {
        copiar(&raiz.join("version"), &lib.join("dart-sdk").join("version"))?;
    }

    // O Clang (driver da ligação) e o lld do sistema-alvo, do mesmo LLVM.
    let clang = dartforge_emit_native::driver::NativeDriverOptions::default().clang;
    let clang = localizar(&clang).ok_or_else(|| format!("Clang não encontrado ({})", clang.display()))?;
    let dir_llvm = clang.parent().ok_or("Clang sem diretório")?.to_path_buf();
    let llvm_bin = lib.join("llvm").join("bin");
    let linux = dartforge_emit_native::alvo::sistema() == dartforge_emit_native::alvo::Sistema::Linux;
    if linux {
        // No Linux a ligação é o `ld.lld` direto: vai o sysroot de ligação
        // (glibc e libgcc desta máquina), não o Clang.
        let sysroot = dartforge_emit_native::ligador::SysrootLinux::do_sistema(&clang)?;
        sysroot.copiar_para(&lib.join("sysroot").join(dartforge_emit_native::ligador::triple_do_sysroot()))?;
    } else {
        copiar(&clang, &llvm_bin.join(dartforge_emit_native::alvo::nome_clang()))?;
    }
    let lld = if cfg!(windows) {
        "lld-link.exe"
    } else if cfg!(target_os = "macos") {
        "ld64.lld"
    } else {
        "ld.lld"
    };
    copiar(&dir_llvm.join(lld), &llvm_bin.join(lld))?;
    // Os executáveis do pacote oficial do LLVM vêm com a tabela de símbolos
    // (o `ld.lld` do Linux: 200 MB → 1/3 disso sem ela).
    let strip = dir_llvm.join(if cfg!(windows) { "llvm-strip.exe" } else { "llvm-strip" });
    if strip.is_file() {
        let mut executaveis: Vec<PathBuf> = std::fs::read_dir(&llvm_bin)?.flatten().map(|e| e.path()).collect();
        if !cfg!(windows) {
            // No Windows os símbolos já ficam fora, no PDB.
            executaveis.push(bin.join(exe.file_name().ok_or("executável sem nome")?));
        }
        for e in executaveis {
            let st = std::process::Command::new(&strip).arg("--strip-all").arg(&e).status()?;
            if !st.success() {
                return Err(format!("llvm-strip falhou em {}", e.display()).into());
            }
        }
    }
    // O JIT no Windows carrega a DLL da API C do LLVM ao lado do executável.
    if cfg!(windows) && dir_llvm.join("LLVM-C.dll").is_file() {
        copiar(&dir_llvm.join("LLVM-C.dll"), &bin.join("LLVM-C.dll"))?;
    }

    let marca = format!(
        "{{\n  \"versao\": \"{}\",\n  \"alvo\": \"{}-{}\",\n  \"sdk_do_dart\": \"{}\"\n}}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        std::env::consts::OS,
        std::fs::read_to_string(lib.join("dart-sdk").join("version")).unwrap_or_default().trim(),
    );
    std::fs::write(lib.join(dartforge_elements::distribuicao::MARCA), marca)?;
    println!("distribuição em {}", destino.display());
    Ok(())
}

/// O caminho real de um executável: como dado, ou pelo `PATH`; links
/// simbólicos resolvidos (o `clang` do pacote do LLVM aponta `clang-22`).
fn localizar(p: &Path) -> Option<PathBuf> {
    let achado = if p.components().count() > 1 {
        p.is_file().then(|| p.to_path_buf())
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(p)).find(|c| c.is_file())
    }?;
    Some(achado.canonicalize().unwrap_or(achado))
}

/// Copia um arquivo, seguindo links simbólicos, com as permissões.
fn copiar(de: &Path, para: &Path) -> Result<(), String> {
    std::fs::copy(de, para).map(|_| ()).map_err(|e| format!("copiar {} → {}: {e}", de.display(), para.display()))
}

fn copiar_arvore(de: &Path, para: &Path) -> Result<(), String> {
    std::fs::create_dir_all(para).map_err(|e| format!("{}: {e}", para.display()))?;
    let entradas = std::fs::read_dir(de).map_err(|e| format!("{}: {e}", de.display()))?;
    for e in entradas.flatten() {
        let caminho = e.path();
        let alvo = para.join(e.file_name());
        if caminho.is_dir() {
            copiar_arvore(&caminho, &alvo)?;
        } else {
            copiar(&caminho, &alvo)?;
        }
    }
    Ok(())
}
