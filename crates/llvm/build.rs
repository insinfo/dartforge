//! Links the crate against the **shared** library of the LLVM C API.
//!
//! `llvm-sys` comes in with the `no-llvm-linking` feature: we use its
//! `extern "C"` signatures and the `LLVMInitializeNative*` wrappers, but linking
//! is done here, against `LLVM-C`.
//!
//! # Why not llvm-sys's default static linking
//!
//! The official `clang+llvm-22.1.8-x86_64-pc-windows-msvc` distribution ships the
//! static libraries compiled with the **static** CRT (`libcmt`), while Rust uses
//! the dynamic CRT (`msvcrt`). Linking both produces
//! `LINK : warning LNK4098: defaultlib 'libcmt.lib' conflicts` and a binary with
//! **two heaps**. This is not theoretical: with static linking, an error message
//! returned by `LLVMParseIRInContext2` is read correctly and then brings the
//! process down with `STATUS_ACCESS_VIOLATION` in the next `LLVMDisposeMessage`
//! — allocated by one CRT, freed by the other. The same package also does not
//! allow `llvm-config --link-shared`, which looks for a nonexistent `LLVM-22.dll`.
//!
//! With `LLVM-C.dll`, allocation and freeing both happen inside the DLL, with its
//! CRT. The price is a runtime dependency: the DLL must be reachable by the
//! loader. See `docs/JIT.md`.
//!
//! # Linux and macOS: the project's choice, not the system's
//!
//! `DARTFORGE_LLVM_LINK` chooses: `shared` (`libLLVM` through
//! `llvm-config --link-shared`; fails if the installation does not have it),
//! `static` (the `libLLVM*.a` through `llvm-config --link-static`) or `auto`, the
//! default — the shared one when the installation offers it, otherwise static.
use std::path::{Path, PathBuf};

#[path = "build_support/installation.rs"]
mod installation;

#[path = "build_support/zstd.rs"]
mod zstd;

/// Emits the link directives and the script's rerun dependencies.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_DIR");
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_ZSTD_LIBRARY");
    println!("cargo::rerun-if-env-changed=LLVM_SYS_221_PREFIX");
    println!("cargo::rerun-if-env-changed=PATH");

    let prefix = prefix();
    println!(
        "cargo::rerun-if-changed={}",
        prefix
            .join("bin")
            .join(format!("llvm-config{}", std::env::consts::EXE_SUFFIX))
            .display()
    );
    let version = llvm_config(&prefix, &["--version"])
        .unwrap_or_else(|error| panic!("cannot validate selected LLVM installation: {error}"));
    assert!(
        installation::compatible_version(&version),
        "the selected LLVM at {} reports {}; DartForge requires LLVM 22.1.x",
        prefix.display(),
        version.trim()
    );
    // The LLVM library is loaded by the Rust compiler executable, not by
    // programs it emits. Catch an ARM64/x64 mismatch before compiling the
    // entire workspace and reporting hundreds of unresolved link symbols.
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let triple = llvm_config(&prefix, &["--host-target"])
            .unwrap_or_else(|error| panic!("cannot determine LLVM library architecture: {error}"));
        let library_arch = triple.trim().split('-').next().unwrap_or("");
        let library_arch = if library_arch == "arm64" {
            "aarch64"
        } else {
            library_arch
        };
        let rust_arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("Rust target architecture");
        assert_eq!(
            library_arch, rust_arch,
            "LLVM library architecture does not match the Rust build target; install matching LLVM or build DartForge with --target matching LLVM (for x64: --target x86_64-pc-windows-msvc)"
        );
    }
    let libdir = prefix.join("lib");
    if !libdir.is_dir() {
        println!(
            "cargo::warning=LLVM library directory not found: {}",
            libdir.display()
        );
    }
    println!("cargo::rustc-link-search=native={}", libdir.display());
    println!("cargo::rerun-if-env-changed=DARTFORGE_LLVM_LINK");
    let mode = std::env::var("DARTFORGE_LLVM_LINK").unwrap_or_else(|_| "auto".to_owned());
    let windows_target = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows_target {
        // The official Windows package only allows the C API DLL (top of this file).
        assert!(
            mode != "static",
            "DARTFORGE_LLVM_LINK=static is not supported on Windows: see the top of crates/llvm/build.rs"
        );
        println!("cargo::rustc-link-lib=dylib={}", shared_library_name());
        return;
    }
    match mode.as_str() {
        "shared" => {
            if let Err(reason) = link_shared(&prefix) {
                panic!(
                    "DARTFORGE_LLVM_LINK=shared, but the LLVM at {} does not offer the shared library: {reason}",
                    prefix.display()
                );
            }
        }
        "static" => {
            if let Err(reason) = link_static(&prefix) {
                panic!(
                    "DARTFORGE_LLVM_LINK=static, but the LLVM at {} does not offer the static libraries: {reason}",
                    prefix.display()
                );
            }
        }
        "auto" => {
            if let Err(shared_one) = link_shared(&prefix)
                && let Err(static_one) = link_static(&prefix)
            {
                panic!(
                    "the LLVM at {} offers neither shared linking ({shared_one}) nor static linking ({static_one})",
                    prefix.display()
                );
            }
        }
        other_one => panic!("DARTFORGE_LLVM_LINK={other_one}: use shared, static or auto"),
    }
}

/// LLVM components that dartforge uses: the JIT (`orcjit`) and the AOT's object
/// and bitcode generation, including both supported cross-compiler CPU families.
const COMPONENTS: [&str; 7] = [
    "orcjit",
    "native",
    "x86",
    "aarch64",
    "irreader",
    "passes",
    "bitwriter",
];

/// Runs the `llvm-config` of the chosen prefix (never the one on `PATH`, which may
/// belong to another installation) and returns its output.
fn llvm_config(prefix: &Path, args: &[&str]) -> Result<String, String> {
    let config = prefix.join("bin").join("llvm-config");
    let out = std::process::Command::new(&config)
        .args(args)
        .output()
        .map_err(|e| format!("could not run {}: {e}", config.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} {args:?}: {}",
            config.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Dynamic linking outside Windows, through `llvm-config --link-shared`.
///
/// Works when the chosen installation was built with `LLVM_BUILD_LLVM_DYLIB=ON`
/// (the apt.llvm.org and Debian packages ship `libLLVM-22.so`; Homebrew,
/// `libLLVM.dylib`). `Err` when it does not offer it: `llvm-config` itself
/// refuses the query.
///
/// The library must be reachable by the loader at run time (on the system path,
/// or through `LD_LIBRARY_PATH`/`DYLD_LIBRARY_PATH`); this crate's binaries and
/// tests carry the `rpath` of the prefix's `lib`.
fn link_shared(prefix: &Path) -> Result<(), String> {
    require_files(prefix, "--link-shared")?;
    let mut args = vec!["--link-shared", "--libs"];
    args.extend(COMPONENTS);
    let libs = llvm_config(prefix, &args)?;
    let system = llvm_config(prefix, &["--link-shared", "--system-libs"]).unwrap_or_default();
    for lib in libs.split_whitespace().chain(system.split_whitespace()) {
        if let Some(name) = lib.strip_prefix("-l") {
            println!("cargo::rustc-link-lib=dylib={name}");
        }
    }
    println!(
        "cargo::rustc-link-arg=-Wl,-rpath,{}",
        prefix.join("lib").display()
    );
    Ok(())
}

/// Static linking outside Windows, through `llvm-config --link-static`.
///
/// It is the `auto` path when the chosen installation does not have the shared
/// library — the verified case of the prebuilt package
/// `LLVM-22.1.8-Linux-X64.tar.xz` (its `lib/` only has `libLLVM*.a`). It is not a
/// Linux limitation: it is how that package was built (`LLVM_BUILD_LLVM_DYLIB` is
/// `OFF` by default). The CRT conflict that forces the DLL on Windows (`/MT` ×
/// `/MD`) does not apply to this configuration; what still matters is the ABI of
/// the system C++ library (`libstdc++` or `libc++`) the package was compiled
/// with.
fn link_static(prefix: &Path) -> Result<(), String> {
    require_files(prefix, "--link-static")?;
    let mut args = vec!["--link-static", "--libs"];
    args.extend(COMPONENTS);
    let libs = llvm_config(prefix, &args)?;
    for lib in libs.split_whitespace() {
        if let Some(name) = lib.strip_prefix("-l") {
            println!("cargo::rustc-link-lib=static={name}");
        }
    }
    // Static LLVMCompression requires zstd even if this application's inputs
    // contain no compressed sections. An SSD-local library override avoids
    // installing a dependency at the packaging machine's absolute path.
    for lib in llvm_config(prefix, &["--link-static", "--system-libs"])?.split_whitespace() {
        let library_name = lib.strip_prefix("-l").unwrap_or_else(|| {
            Path::new(lib)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .trim_start_matches("lib")
        });
        if library_name == "zstd"
            && let Some(path) = std::env::var_os("DARTFORGE_LLVM_ZSTD_LIBRARY")
        {
            for directive in zstd::override_directives(&PathBuf::from(path))? {
                println!("{directive}");
            }
            continue;
        }
        if let Some(name) = lib.strip_prefix("-l") {
            if name == "xml2" && !libs.contains("WindowsManifest") {
                continue;
            }
            println!("cargo::rustc-link-lib=dylib={name}");
        } else {
            let file_path = Path::new(lib);
            let name = file_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .trim_start_matches("lib");
            if lib.ends_with(".tbd") || lib.ends_with(".dylib") || lib.ends_with(".so") {
                // macOS returns paths (`/usr/lib/libz.tbd`); the name is enough.
                println!("cargo::rustc-link-lib=dylib={name}");
            } else if lib.ends_with(".a") && file_path.is_file() {
                if let Some(dir) = file_path.parent() {
                    println!("cargo::rustc-link-search=native={}", dir.display());
                }
                println!("cargo::rustc-link-lib=static={name}");
            } else if let Some(directive) = shared_system_library(name) {
                println!("{directive}");
            } else if name == "zstd" {
                return Err(format!(
                    "required LLVM system library {lib} is missing; set DARTFORGE_LLVM_ZSTD_LIBRARY to an existing libzstd.a, libzstd.dylib or libzstd.so"
                ));
            } else {
                println!(
                    "cargo::warning=system library {lib} missing; install its development package"
                );
            }
        }
    }
    let macos = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    println!(
        "cargo::rustc-link-lib=dylib={}",
        if macos { "c++" } else { "stdc++" }
    );
    Ok(())
}

/// Checks, before emitting any directive, that the library files of the
/// [`COMPONENTS`] exist in the requested mode: through the exit code of
/// `llvm-config <modo> --libfiles` (and not through the message text) and the
/// presence of each listed file.
fn require_files(prefix: &Path, mode: &str) -> Result<(), String> {
    let mut args = vec![mode, "--libfiles"];
    args.extend(COMPONENTS);
    let files = llvm_config(prefix, &args)?;
    if let Some(missing) = files.split_whitespace().find(|a| !Path::new(a).is_file()) {
        return Err(format!("{missing} does not exist"));
    }
    Ok(())
}

/// Prefix of the full LLVM 22.1.x distribution.
///
/// `DARTFORGE_LLVM_DIR` is the project's preferred name, in the same spirit as
/// `DARTFORGE_CLANG` in `crates/emit_native`. `LLVM_SYS_221_PREFIX` is accepted
/// because it is the name that `llvm-sys`'s `build.rs` already requires, and
/// keeping the two pointing to different places would only cause confusion.
fn prefix() -> PathBuf {
    installation::select_prefix(
        std::env::var_os("DARTFORGE_LLVM_DIR"),
        std::env::var_os("LLVM_SYS_221_PREFIX"),
        |binary, argument| {
            let output = std::process::Command::new(binary)
                .arg(argument)
                .output()
                .map_err(|error| error.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned());
            }
            String::from_utf8(output.stdout).map_err(|error| error.to_string())
        },
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The system shared library `lib<name>`, when the `.a` that the LLVM package
/// declares does not exist here: `lib<name>.so` if the development package is
/// installed, otherwise the runtime version (`lib<name>.so.1`) by its literal
/// name.
fn shared_system_library(name: &str) -> Option<String> {
    let dirs = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/usr/local/lib",
        "/lib/x86_64-linux-gnu",
    ];
    for dir in dirs {
        let d = Path::new(dir);
        if d.join(format!("lib{name}.so")).is_file() {
            return Some(format!(
                "cargo::rustc-link-search=native={dir}\ncargo::rustc-link-lib=dylib={name}"
            ));
        }
        let Ok(entries) = std::fs::read_dir(d) else {
            continue;
        };
        let mut versions: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with(&format!("lib{name}.so.")))
            .collect();
        versions.sort_by_key(|n| n.len());
        if let Some(v) = versions.first() {
            return Some(format!(
                "cargo::rustc-link-search=native={dir}\ncargo::rustc-link-lib=dylib:+verbatim={v}"
            ));
        }
    }
    None
}

/// Name of the C API shared library in the Windows package: `bin/LLVM-C.dll`
/// with the import library `lib/LLVM-C.lib`.
fn shared_library_name() -> &'static str {
    "LLVM-C"
}
