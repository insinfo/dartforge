//! Liga o crate à biblioteca **compartilhada** da API C do LLVM.
//!
//! `llvm-sys` entra com a feature `no-llvm-linking`: dele aproveitamos as
//! assinaturas `extern "C"` e os invólucros de `LLVMInitializeNative*`, mas a
//! ligação é feita aqui, contra `LLVM-C`.
//!
//! # Por que não a ligação estática padrão do llvm-sys
//!
//! A distribuição oficial `clang+llvm-22.1.8-x86_64-pc-windows-msvc` traz as
//! bibliotecas estáticas compiladas com a CRT **estática** (`libcmt`), enquanto
//! o Rust usa a CRT dinâmica (`msvcrt`). Ligar as duas produz
//! `LINK : warning LNK4098: defaultlib 'libcmt.lib' conflita` e um binário com
//! **dois heaps**. Isso não é teórico: com a ligação estática, uma mensagem de
//! erro devolvida por `LLVMParseIRInContext2` é lida corretamente e derruba o
//! processo com `STATUS_ACCESS_VIOLATION` no `LLVMDisposeMessage` seguinte —
//! alocada por uma CRT, liberada pela outra. O mesmo pacote também não permite
//! `llvm-config --link-shared`, que procura um `LLVM-22.dll` inexistente.
//!
//! Com `LLVM-C.dll`, alocação e liberação acontecem as duas dentro da DLL, com
//! a CRT dela. O preço é uma dependência de execução: a DLL precisa estar
//! alcançável pelo carregador. Veja `docs/JIT.md`.
//!
//! # Linux e macOS: escolha do projeto, não do sistema
//!
//! `DARTFORGE_LLVM_LINK` escolhe: `shared` (a `libLLVM` pelo
//! `llvm-config --link-shared`; falha se a instalação não a tiver), `static`
//! (as `libLLVM*.a` pelo `llvm-config --link-static`) ou `auto`, o padrão —
//! a compartilhada quando a instalação a oferece, senão a estática.
use std::path::{Path, PathBuf};

/// Emite as diretivas de ligação e as dependências de reexecução do script.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_DIR");
    println!("cargo::rerun-if-env-changed=LLVM_SYS_221_PREFIX");

    let prefix = prefix();
    let libdir = prefix.join("lib");
    if !libdir.is_dir() {
        println!(
            "cargo::warning=diretório de bibliotecas do LLVM não encontrado: {}",
            libdir.display()
        );
    }
    println!("cargo::rustc-link-search=native={}", libdir.display());
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_LINK");
    let modo = std::env::var("DARTFORGE_LLVM_LINK").unwrap_or_else(|_| "auto".to_owned());
    let alvo_windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if alvo_windows {
        // O pacote oficial de Windows só permite a DLL da API C (topo deste arquivo).
        assert!(modo != "static", "DARTFORGE_LLVM_LINK=static não é suportado no Windows: ver o topo de crates/jit/build.rs");
        println!("cargo::rustc-link-lib=dylib={}", shared_library_name());
        return;
    }
    match modo.as_str() {
        "shared" => {
            if let Err(motivo) = link_shared(&prefix) {
                panic!(
                    "DARTFORGE_LLVM_LINK=shared, mas o LLVM em {} não oferece a biblioteca compartilhada: {motivo}",
                    prefix.display()
                );
            }
        }
        "static" => link_static(&prefix),
        "auto" => {
            if link_shared(&prefix).is_err() {
                link_static(&prefix);
            }
        }
        outro => panic!("DARTFORGE_LLVM_LINK={outro}: use shared, static ou auto"),
    }
}

/// Componentes do LLVM que o JIT usa.
const COMPONENTES: [&str; 4] = ["orcjit", "native", "irreader", "passes"];

/// Executa o `llvm-config` do prefixo escolhido (nunca o do `PATH`, que pode
/// ser de outra instalação) e devolve a saída.
fn llvm_config(prefix: &Path, args: &[&str]) -> Result<String, String> {
    let config = prefix.join("bin").join("llvm-config");
    let out = std::process::Command::new(&config)
        .args(args)
        .output()
        .map_err(|e| format!("não foi possível executar {}: {e}", config.display()))?;
    if !out.status.success() {
        return Err(format!("{} {args:?}: {}", config.display(), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Ligação dinâmica fora do Windows, pelo `llvm-config --link-shared`.
///
/// Serve quando a instalação escolhida foi construída com
/// `LLVM_BUILD_LLVM_DYLIB=ON` (os pacotes do apt.llvm.org e do Debian trazem
/// `libLLVM-22.so`; o Homebrew, `libLLVM.dylib`). `Err` quando ela não a
/// oferece: o próprio `llvm-config` recusa a consulta.
///
/// A biblioteca precisa estar alcançável pelo carregador na execução (no
/// caminho do sistema, ou por `LD_LIBRARY_PATH`/`DYLD_LIBRARY_PATH`); os
/// binários e testes deste crate levam o `rpath` do `lib` do prefixo.
fn link_shared(prefix: &Path) -> Result<(), String> {
    let mut args = vec!["--link-shared", "--libs"];
    args.extend(COMPONENTES);
    let libs = llvm_config(prefix, &args)?;
    let sistema = llvm_config(prefix, &["--link-shared", "--system-libs"]).unwrap_or_default();
    for lib in libs.split_whitespace().chain(sistema.split_whitespace()) {
        if let Some(nome) = lib.strip_prefix("-l") {
            println!("cargo::rustc-link-lib=dylib={nome}");
        }
    }
    println!("cargo::rustc-link-arg=-Wl,-rpath,{}", prefix.join("lib").display());
    Ok(())
}

/// Ligação estática fora do Windows, pelo `llvm-config --link-static`.
///
/// É o caminho de `auto` quando a instalação escolhida não tem a biblioteca
/// compartilhada — o caso, verificado, do pacote pré-compilado
/// `LLVM-22.1.8-Linux-X64.tar.xz` (o `lib/` dele só tem `libLLVM*.a`). Não é
/// limitação do Linux: é como aquele pacote foi construído
/// (`LLVM_BUILD_LLVM_DYLIB` é `OFF` por padrão). O conflito de CRT que obriga
/// a DLL no Windows (`/MT` × `/MD`) não se aplica a esta configuração; o que
/// continua valendo é a ABI da biblioteca C++ do sistema (`libstdc++` ou
/// `libc++`), com que o pacote foi compilado.
fn link_static(prefix: &Path) {
    let run = |args: &[&str]| llvm_config(prefix, args).unwrap_or_else(|e| panic!("{e}"));
    let mut args = vec!["--link-static", "--libs"];
    args.extend(COMPONENTES);
    let libs = run(&args);
    for lib in libs.split_whitespace() {
        if let Some(nome) = lib.strip_prefix("-l") {
            println!("cargo::rustc-link-lib=static={nome}");
        }
    }
    // As bibliotecas do sistema que o pacote declara valem para o LLVM
    // inteiro; só entram as que os componentes acima usam, e as ausentes
    // desta máquina não derrubam a ligação:
    // * `xml2` só serve ao `LLVMWindowsManifest`, que não está na lista;
    // * `zstd` vem como caminho absoluto do `.a` da máquina que empacotou;
    //   usa-se o arquivo se existir aqui, senão a compressão fica de fora
    //   (o JIT não lê seção comprimida).
    for lib in run(&["--link-static", "--system-libs"]).split_whitespace() {
        if let Some(nome) = lib.strip_prefix("-l") {
            if nome == "xml2" && !libs.contains("WindowsManifest") {
                continue;
            }
            println!("cargo::rustc-link-lib=dylib={nome}");
        } else {
            let caminho = Path::new(lib);
            let nome = caminho.file_stem().and_then(|n| n.to_str()).unwrap_or_default().trim_start_matches("lib");
            if lib.ends_with(".tbd") || lib.ends_with(".dylib") || lib.ends_with(".so") {
                // O macOS devolve caminhos (`/usr/lib/libz.tbd`); o nome basta.
                println!("cargo::rustc-link-lib=dylib={nome}");
            } else if lib.ends_with(".a") && caminho.is_file() {
                if let Some(dir) = caminho.parent() {
                    println!("cargo::rustc-link-search=native={}", dir.display());
                }
                println!("cargo::rustc-link-lib=static={nome}");
            } else if let Some(diretiva) = shared_system_library(nome) {
                println!("{diretiva}");
            } else {
                println!("cargo::warning=biblioteca do sistema {lib} ausente; instale o pacote de desenvolvimento dela");
            }
        }
    }
    let macos = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    println!("cargo::rustc-link-lib=dylib={}", if macos { "c++" } else { "stdc++" });
}

/// Prefixo da distribuição completa do LLVM 22.1.x.
///
/// `DARTFORGE_LLVM_DIR` é o nome preferido do projeto, no mesmo espírito de
/// `DARTFORGE_CLANG` em `crates/emit_native`. `LLVM_SYS_221_PREFIX` é aceito porque é
/// o nome que o `build.rs` do `llvm-sys` já exige, e manter as duas apontando
/// para lugares diferentes só produziria confusão.
fn prefix() -> PathBuf {
    for variable in ["DARTFORGE_LLVM_DIR", "LLVM_SYS_221_PREFIX"] {
        if let Some(value) = std::env::var_os(variable) {
            let path = PathBuf::from(value);
            if path.as_os_str().is_empty() {
                continue;
            }
            return path;
        }
    }
    PathBuf::from(FALLBACK_PREFIX)
}

/// Instalação verificada nesta máquina, usada quando nada foi configurado.
const FALLBACK_PREFIX: &str = if cfg!(windows) {
    "E:/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
} else {
    "/usr/lib/llvm-22"
};

/// A biblioteca compartilhada `lib<nome>` do sistema, quando o `.a` que o
/// pacote do LLVM declara não existe aqui: `lib<nome>.so` se o pacote de
/// desenvolvimento estiver instalado, senão a versão de execução
/// (`lib<nome>.so.1`) pelo nome literal.
fn shared_system_library(nome: &str) -> Option<String> {
    let dirs = ["/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu", "/usr/lib64", "/usr/lib", "/usr/local/lib", "/lib/x86_64-linux-gnu"];
    for dir in dirs {
        let d = Path::new(dir);
        if d.join(format!("lib{nome}.so")).is_file() {
            return Some(format!("cargo::rustc-link-search=native={dir}\ncargo::rustc-link-lib=dylib={nome}"));
        }
        let Ok(entradas) = std::fs::read_dir(d) else { continue };
        let mut versoes: Vec<String> = entradas
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with(&format!("lib{nome}.so.")))
            .collect();
        versoes.sort_by_key(|n| n.len());
        if let Some(v) = versoes.first() {
            return Some(format!("cargo::rustc-link-search=native={dir}\ncargo::rustc-link-lib=dylib:+verbatim={v}"));
        }
    }
    None
}

/// Nome da biblioteca compartilhada da API C no pacote de Windows:
/// `bin/LLVM-C.dll` com a biblioteca de importação `lib/LLVM-C.lib`.
fn shared_library_name() -> &'static str {
    "LLVM-C"
}
