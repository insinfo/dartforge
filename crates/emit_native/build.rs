//! The compiler fingerprint that goes into the cache key of the source SDK
//! (P5c, `src/sdk_module.rs`): the blake3 of the sources of whatever decides the
//! code of an SDK library — the lowering and the emitter (this crate), the
//! inference (`types`), the element model (`elements`) and the front end.
//! A change in any of them invalidates the cached SDK objects; the
//! package version alone would not be enough (it does not change on every commit).
//!
//! And the **precompiled** AOT runtime (`precompile_runtime`): the two
//! variants of the `staticlib` (with the C `main` and the one for the DLL of the source SDK) are
//! compiled here, by the build's own `cargo` and for the build target, and
//! go into the distribution next to the executable. Users of dartforge do not
//! need Rust installed.

use std::path::{Path, PathBuf};

fn join(dir: &Path, exit: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            join(&p, exit);
        } else if p.extension().is_some_and(|x| x == "rs") {
            exit.push(p);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for d in ["src", "../types/src", "../elements/src", "../frontend/src", "../runtime/src"] {
        let dir = root.join(d);
        println!("cargo::rerun-if-changed={}", dir.display());
        join(&dir, &mut files);
    }
    // The effects table of the externs decides where the pending exception is
    // checked: it changes the SDK code like a source does.
    // Ownership also decides retains, cleanup and the ABI of ARC bodies.
    // An isolated change of the catalog must invalidate the cached SDK.
    for table in ["../runtime/effects.tsv", "../runtime/ownership.tsv"] {
        let path = root.join(table);
        println!("cargo::rerun-if-changed={}", path.display());
        files.push(path);
    }
    files.sort();
    let mut h = blake3::Hasher::new();
    for a in &files {
        let rel = a.strip_prefix(root).unwrap_or(a).to_string_lossy().replace('\\', "/");
        h.update(rel.as_bytes());
        h.update(&[0]);
        // Normalized line endings: the same compiler in checkouts with
        // different `core.autocrlf` has the same fingerprint.
        let text = std::fs::read(a).unwrap_or_default();
        let text: Vec<u8> = text.into_iter().filter(|&b| b != b'\r').collect();
        h.update(&text);
        h.update(&[0]);
    }
    println!("cargo::rustc-env=DARTFORGE_EMITTER_HASH={}", h.finalize().to_hex());
    precompile_runtime();
}

/// The profile of a variant of the runtime `staticlib`, on top of the workspace
/// `release` (it goes into its name). `panic=abort`, in the variants that only
/// AOT links (the executable one with the C `main` and the production one): without the
/// unwind tables and the Rust cleanup paths (~0.6 MB less per
/// executable). Almost nothing changes: a panic in code that Dart calls already
/// terminated the process, because it reaches an `extern "C"` boundary (the
/// runtime entries, the C `main`, the Dart code that an isolate thread
/// runs) and Rust aborts there. What changes is a panic in pure Rust of an
/// auxiliary thread (the preparation of an isolate, the `dart:io` threads):
/// before it killed only the thread, now it terminates the process. No `catch_unwind`
/// nor `join` of the runtime relies on unwinding (docs/NATIVO.md). The
/// variant of the development DLL, which the JIT also loads, stays as it
/// was.
const ABORT: &[&str] = &["--config", "profile.release.panic=\"abort\""];

/// Compiles the three variants of the runtime `staticlib`
/// (`crates/runtime_static`): the executable one without the source SDK (feature
/// `aot`, with the C `main`), the one for the DLL of the source SDK (feature `dll`; the
/// development profile and the JIT) and the production one (feature `dll` with
/// [`ABORT`], linked statically into the executable), for the build target,
/// with its own `cargo build` (another target directory: nothing contends for the
/// lock of the outer build), and publishes, for the binary, the directory and their
/// names. The name carries the blake3 of the runtime source, of `Cargo.lock`,
/// of the target and of the variant: a runtime of another version is never confused with
/// this compiler's. `DARTFORGE_RUNTIME_NO_PRECOMPILE=1` skips the
/// compilation (for `cargo check`/`clippy`, which do not produce executables).
fn precompile_runtime() {
    println!("cargo::rerun-if-env-changed=DARTFORGE_RUNTIME_NO_PRECOMPILE");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = root.join("../..");
    let exit = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let target = std::env::var("TARGET").expect("TARGET");
    let windows = target.contains("windows");
    let skip = std::env::var("DARTFORGE_RUNTIME_NO_PRECOMPILE").is_ok_and(|v| v == "1");
    // macOS: the C code of the dependencies (zlib, `ring`) targets the same minimum that
    // `ld64.lld` declares in the executable (`linker_macos.rs`); without this,
    // `cc` uses the one from the machine's SDK (26.x), and the program could call what
    // an older macOS does not have.
    println!("cargo::rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let macos_minimum = target.contains("apple-darwin").then(|| {
        std::env::var("MACOSX_DEPLOYMENT_TARGET")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| if target.starts_with("aarch64") { "11.0" } else { "10.12" }.to_string())
    });

    let mut sources = Vec::new();
    for d in ["../runtime/src", "../runtime_static/src"] {
        join(&root.join(d), &mut sources);
    }
    for f in ["../runtime/build.rs", "../runtime/Cargo.toml", "../runtime/effects.tsv",
        "../runtime/ownership.tsv", "../runtime_static/Cargo.toml", "../../Cargo.lock"] {
        let p = root.join(f);
        println!("cargo::rerun-if-changed={}", p.display());
        sources.push(p);
    }
    println!("cargo::rerun-if-changed={}", root.join("../runtime_static/src").display());
    sources.sort();

    let cargo_file = if windows { "dartforge_runtime_static.lib" } else { "libdartforge_runtime_static.a" };
    let ext = if windows { "lib" } else { "a" };
    for (variant, feature, prefix, profile) in [
        ("MAIN", "aot", "dartforge_runtime_", ABORT),
        ("DLL", "dll", "dartforge_rtdll_", &[][..]),
        ("PRODUCTION", "dll", "dartforge_rtprod_", ABORT),
    ] {
        let mut h = blake3::Hasher::new();
        h.update(b"dartforge-runtime-cargo\0");
        h.update(target.as_bytes());
        h.update(b"\0");
        h.update(feature.as_bytes());
        h.update(b"\0");
        if let Some(m) = &macos_minimum {
            h.update(m.as_bytes());
            h.update(b"\0");
        }
        for c in profile {
            h.update(c.as_bytes());
            h.update(b"\0");
        }
        for f in &sources {
            let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
            h.update(rel.as_bytes());
            h.update(&[0]);
            let text: Vec<u8> = std::fs::read(f).unwrap_or_default().into_iter().filter(|&b| b != b'\r').collect();
            h.update(&text);
            h.update(&[0]);
        }
        let name = format!("{prefix}{}.{ext}", &h.finalize().to_hex()[..32]);
        println!("cargo::rustc-env=DARTFORGE_RUNTIME_{variant}={name}");
        let lib = exit.join(&name);
        if skip || lib.is_file() {
            continue;
        }
        // One target directory per profile: the dependencies of one do not
        // invalidate those of the other.
        let target_dir = exit.join(if profile.is_empty() { "cargo-runtime" } else { "cargo-runtime-abort" });
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        cmd.arg("build")
            .arg("--release")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(workspace.join("Cargo.toml"))
            .args(["-p", "dartforge-runtime-static", "--features", feature, "--target", &target])
            .args(profile)
            .arg("--target-dir")
            .arg(&target_dir);
        if std::env::var_os("CARGO_NET_OFFLINE").is_some() {
            cmd.arg("--offline");
        }
        // `clippy-driver` (and any workspace wrapper) belongs to the outer
        // build; the target directory is the one here.
        for v in ["RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_TARGET"] {
            cmd.env_remove(v);
        }
        // Without the path of the compiling machine in panic messages (and the
        // same runtime in any checkout).
        let mut flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
        if !flags.is_empty() {
            flags.push('\x1f');
        }
        let ws = workspace.canonicalize().unwrap_or_else(|_| workspace.clone());
        flags.push_str(&format!("--remap-path-prefix={}=dartforge", ws.display()));
        // Unwind tables in every runtime frame on the Itanium
        // unwinder targets, also with `panic=abort`: the walk of
        // roots by maps (`_Unwind_Backtrace`, `runtime/src/heap.rs`) and
        // the unwinding of exceptions by tables cross the runtime
        // frames; without the table, the unwinder stops at the first of them
        // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.6).
        if target.contains("linux") || target.contains("apple") {
            flags.push_str("\x1f-Cforce-unwind-tables=yes");
        }
        cmd.env("CARGO_ENCODED_RUSTFLAGS", flags).env_remove("RUSTFLAGS");
        if let Some(m) = &macos_minimum {
            cmd.env("MACOSX_DEPLOYMENT_TARGET", m);
        }
        let status = cmd.status().expect("run the build's cargo");
        assert!(status.success(), "the runtime compilation ({feature}) for {target} failed");
        let produced = target_dir.join(&target).join("release").join(cargo_file);
        let tmp = exit.join(format!("{name}.tmp"));
        std::fs::copy(&produced, &tmp).unwrap_or_else(|e| panic!("copiar {}: {e}", produced.display()));
        std::fs::rename(&tmp, &lib).expect("publish the precompiled runtime");
    }
    println!("cargo::rustc-env=DARTFORGE_RUNTIME_BUILD_DIR={}", exit.display());
}
