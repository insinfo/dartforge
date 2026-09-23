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
    println!("cargo::rustc-link-lib=dylib={}", shared_library_name());
}

/// Prefixo da distribuição completa do LLVM 22.1.x.
///
/// `DARTFORGE_LLVM_DIR` é o nome preferido do projeto, no mesmo espírito de
/// `DARTFORGE_CLANG` em `crates/native`. `LLVM_SYS_221_PREFIX` é aceito porque é
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
    "D:/DartSDKs/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
} else {
    "/usr/lib/llvm-22"
};

/// Nome da biblioteca compartilhada da API C, tal como cada pacote a publica.
///
/// Windows publica `bin/LLVM-C.dll` com a import library `lib/LLVM-C.lib`. Os
/// pacotes Unix publicam a biblioteca completa como `libLLVM-22.so`/`.dylib`, e
/// a API C está dentro dela — não há `libLLVM-C` separada.
fn shared_library_name() -> &'static str {
    if Path::new(&prefix()).join("lib/LLVM-C.lib").is_file() || cfg!(windows) {
        "LLVM-C"
    } else {
        "LLVM-22"
    }
}
