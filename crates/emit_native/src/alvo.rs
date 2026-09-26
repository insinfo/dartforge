//! O sistema para o qual o backend nativo compila: o do processo que compila.
//!
//! Tudo o que muda entre Windows, Linux e macOS mora aqui — o cabeçalho do
//! LLVM IR, as extensões dos artefatos, as bandeiras do Clang e da ligação e o
//! formato da biblioteca compartilhada do SDK da fonte. O resto do backend
//! pergunta a este módulo em vez de testar `cfg!` espalhado.
//!
//! Não há compilação cruzada: o alvo é sempre o hospedeiro. O IR no Windows é
//! byte a byte o de antes do porte (mesmo cabeçalho, mesmas bandeiras), para
//! que os resumos de determinismo e as chaves de cache continuem valendo.

/// Os sistemas suportados pelo backend nativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sistema {
    /// COFF, MSVC ABI, `.obj`/`.lib`/`.dll`/`.exe`.
    Windows,
    /// ELF, `.o`/`.a`/`.so`, executável sem extensão.
    Linux,
    /// Mach-O, `.o`/`.a`/`.dylib`, executável sem extensão.
    MacOs,
}

/// O sistema do processo corrente.
///
/// ```
/// use dartforge_emit_native::alvo::{Sistema, sistema};
/// let s = sistema();
/// assert_eq!(s == Sistema::Windows, cfg!(windows));
/// ```
pub const fn sistema() -> Sistema {
    if cfg!(windows) {
        Sistema::Windows
    } else if cfg!(target_os = "macos") {
        Sistema::MacOs
    } else {
        Sistema::Linux
    }
}

/// O cabeçalho `target datalayout`/`target triple` do módulo.
///
/// Windows e Linux x86-64 fixam as strings exatas que a `LLJIT` do processo
/// usa (o JIT recusa um módulo com alvo diferente do seu). Nos demais
/// hospedeiros (macOS, Linux aarch64) o módulo sai sem cabeçalho e o Clang e
/// a `LLJIT` impõem o do hospedeiro: o triple do macOS carrega a versão do
/// sistema (`arm64-apple-darwin24.1.0`), que não cabe numa constante.
pub fn cabecalho_ir() -> &'static str {
    match sistema() {
        Sistema::Windows => {
            "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n\n"
        }
        Sistema::Linux if cfg!(target_arch = "x86_64") => {
            "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\n"
        }
        _ => "",
    }
}

/// Se o formato de objeto tem `comdat`. O Mach-O não tem: lá a definição
/// `linkonce_odr` sozinha vira símbolo fraco, e o ligador fica com uma.
pub const fn tem_comdat() -> bool {
    !matches!(sistema(), Sistema::MacOs)
}

/// Extensão do objeto (sem o ponto).
pub const fn ext_objeto() -> &'static str {
    match sistema() {
        Sistema::Windows => "obj",
        _ => "o",
    }
}

/// Extensão da biblioteca estática (sem o ponto).
pub const fn ext_estatica() -> &'static str {
    match sistema() {
        Sistema::Windows => "lib",
        _ => "a",
    }
}

/// Nome do arquivo da biblioteca compartilhada `base` (`base.dll`,
/// `libbase.so`, `libbase.dylib`).
///
/// ```
/// let n = dartforge_emit_native::alvo::nome_compartilhada("dfsdk_0");
/// assert!(n.contains("dfsdk_0"));
/// ```
pub fn nome_compartilhada(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.dll"),
        Sistema::Linux => format!("lib{base}.so"),
        Sistema::MacOs => format!("lib{base}.dylib"),
    }
}

/// Nome do executável `base` (`base.exe` no Windows, `base` nos outros).
pub fn nome_executavel(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.exe"),
        _ => base.to_string(),
    }
}

/// Nome do Clang na pasta `bin` de uma distribuição do LLVM.
pub const fn nome_clang() -> &'static str {
    match sistema() {
        Sistema::Windows => "clang.exe",
        _ => "clang",
    }
}

/// Bandeiras do Clang que dependem do formato de objeto, para todo IR.
///
/// No Windows, `-mno-incremental-linker-compatible` zera o `TimeDateStamp` do
/// cabeçalho COFF: sem ele, o mesmo IR dava objetos diferentes no byte 4
/// (medido), e o objeto deixava de ser função da chave. ELF e Mach-O não têm
/// carimbo de tempo.
pub const fn bandeiras_objeto() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &["-mno-incremental-linker-compatible"],
        _ => &[],
    }
}

/// Bandeiras do Clang para os objetos que vão para a biblioteca
/// compartilhada do SDK: código independente de posição fora do Windows.
pub const fn bandeiras_objeto_compartilhado() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &[],
        _ => &["-fPIC"],
    }
}

/// Bibliotecas do sistema que o runtime (uma `staticlib` do Rust) exige na
/// ligação, no formato do driver do Clang.
///
/// É a lista que o `rustc --print native-static-libs` imprime para cada
/// hospedeiro, fixada aqui para a ligação não depender de reexecutar o
/// `rustc`. No Windows o CRT é escolhido por quem liga (ver `driver.rs`).
pub const fn bibliotecas_do_sistema() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &["-lws2_32", "-luserenv", "-lntdll", "-liphlpapi", "-lbcrypt", "-ladvapi32", "-lkernel32"],
        Sistema::Linux => &["-lgcc_s", "-lutil", "-lrt", "-lpthread", "-lm", "-ldl", "-lc"],
        Sistema::MacOs => &["-lSystem", "-lc", "-lm", "-liconv"],
    }
}

/// A raiz do SDK do macOS para o ligador: o `SDKROOT` do ambiente ou o SDK
/// que o `xcrun` indica (consultado uma vez). O `ld` da Apple acha o SDK
/// sozinho; o `ld64.lld` (ThinLTO) só acha `-lSystem` com o `-isysroot`
/// que o Clang lhe repassa. `None` fora do macOS ou sem Xcode.
pub fn raiz_do_sdk_macos() -> Option<&'static std::path::Path> {
    static RAIZ: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    RAIZ.get_or_init(|| {
        if sistema() != Sistema::MacOs {
            return None;
        }
        if let Some(r) = std::env::var_os("SDKROOT").filter(|r| !r.is_empty()) {
            return Some(std::path::PathBuf::from(r));
        }
        let saida = std::process::Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().ok()?;
        let caminho = String::from_utf8(saida.stdout).ok()?;
        let caminho = caminho.trim();
        (saida.status.success() && !caminho.is_empty()).then(|| std::path::PathBuf::from(caminho))
    })
    .as_deref()
}

/// Os argumentos de ligação que o sistema exige em toda ligação do Clang
/// (executável, DLL do SDK, runtime): as bibliotecas do sistema e, no
/// macOS, a raiz do SDK (`-isysroot`), sem a qual nem o `ld` da Apple
/// chamado pelo Clang do LLVM nem o `ld64.lld` acham `-lSystem`.
pub fn argumentos_de_ligacao() -> Vec<std::ffi::OsString> {
    let mut v: Vec<std::ffi::OsString> = Vec::new();
    if let Some(raiz) = raiz_do_sdk_macos() {
        v.push("-isysroot".into());
        v.push(raiz.as_os_str().to_owned());
    }
    v.extend(bibliotecas_do_sistema().iter().map(std::ffi::OsString::from));
    v
}
