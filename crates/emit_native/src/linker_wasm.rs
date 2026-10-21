//! Linking a `wasm32-linear` module (docs/PLANO-WASM-MVP.md §3.1) with LLD's
//! WebAssembly flavor, then the binary gate.
//!
//! The module owns its memory and exports exactly the host ABI v1 entries
//! (docs/WASM-HOST-ABI-V1.md): `memory`, `dartforge_abi_version`,
//! `dartforge_run`, `dartforge_resume` and `dartforge_delay_us`, plus
//! `dartforge_js_throw` when it uses the JavaScript interop. Every other symbol
//! is resolved at link time: an import is only what the code declares with
//! `wasm-import-module` (the runtime's `dartforge_host_v1` functions, the
//! interop's `dartforge_js` ones), never an undefined symbol let through, so a
//! runtime piece still missing fails the link instead of becoming an import. The
//! snippet imports then get short names and the companion `.mjs`
//! (`crate::js_interop`), and the file goes through `dartforge-wasm-gate`, the
//! check the executor applies: a module outside the profile never leaves the
//! compiler.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The linear-memory stack of the generated code and the runtime (LLVM's
/// `__stack_pointer`): the 8 MiB the native code uses per thread.
pub const STACK_BYTES: u64 = 8 << 20;

/// The memory ceiling: the whole 32-bit address space (65 536 pages).
pub const MAX_MEMORY_BYTES: u64 = 4 << 30;

/// The exports of host ABI v1 besides the memory, kept under dead stripping.
pub const ABI_EXPORTS: [&str; 4] = [
    "dartforge_abi_version",
    "dartforge_run",
    "dartforge_resume",
    "dartforge_delay_us",
];

/// The inputs of a module link.
pub struct Linkage<'a> {
    /// LLD (`lld`, run as `-flavor wasm`).
    pub lld: &'a Path,
    /// The program object first, then the SDK objects.
    pub objects: &'a [PathBuf],
    /// The `wasm32-unknown-unknown` runtime (a Rust `staticlib`).
    pub runtime: &'a Path,
    pub output: &'a Path,
    /// Keeps the DWARF and the name section (the browsers' debuggers read them).
    pub debug_info: bool,
}

/// LLD of the same LLVM as `clang`: the distribution's, otherwise the one next to
/// `clang`, otherwise `lld` on `PATH`. The packages ship `lld` without the
/// `wasm-ld` alias, so the flavor goes as an argument.
pub fn lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribution::llvm_tool("lld") {
        return l;
    }
    let alongside = clang.with_file_name(if cfg!(windows) { "lld.exe" } else { "lld" });
    if alongside.is_file() {
        alongside
    } else {
        PathBuf::from("lld")
    }
}

/// The LLD arguments of `l`, without the program name.
pub fn arguments(l: &Linkage<'_>) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec!["-flavor".into(), "wasm".into()];
    args.extend(l.objects.iter().map(|o| o.as_os_str().to_owned()));
    args.push(l.runtime.as_os_str().to_owned());
    args.push("-o".into());
    args.push(l.output.as_os_str().to_owned());
    // No `_start`: the host calls `dartforge_run` once (host ABI v1).
    args.push("--no-entry".into());
    for export in ABI_EXPORTS {
        args.push(format!("--export={export}").into());
    }
    args.push("-z".into());
    args.push(format!("stack-size={STACK_BYTES}").into());
    args.push(format!("--max-memory={MAX_MEMORY_BYTES}").into());
    // The LTO code generation of bitcode inputs (the SDK, production parts)
    // with the profile's features, as the objects get them.
    args.push("-mllvm".into());
    args.push(format!("-mattr={}", crate::target::WASM_FEATURES).into());
    if !l.debug_info {
        args.push("--strip-all".into());
    }
    args
}

/// Links the module and checks it against the profile and the host ABI.
///
/// # Errors
/// LLD's diagnostics (an unresolved symbol among them), or the gate's.
pub fn link(l: &Linkage<'_>) -> Result<(), String> {
    let exit = Command::new(l.lld)
        .args(arguments(l))
        .output()
        .map_err(|e| format!("failed to run {}: {e}", l.lld.display()))?;
    if !exit.status.success() {
        let text = String::from_utf8_lossy(&exit.stderr);
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(30)
            .collect();
        return Err(format!(
            "the WebAssembly link failed ({}):\n{}",
            exit.status,
            lines.join("\n")
        ));
    }
    // The JavaScript interop: short import names and the companion `.mjs`.
    crate::js_interop::finish_module(l.output)?;
    check(l.output)
}

/// The gate of a linked module: the `wasm32-linear` profile, imports only from
/// the host ABI and the JavaScript interop, and the ABI exports present.
///
/// # Errors
/// Every violation the gate lists.
pub fn check(module: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(module).map_err(|e| format!("cannot read {}: {e}", module.display()))?;
    let mut required = vec!["memory".to_string()];
    required.extend(ABI_EXPORTS.iter().map(|e| e.to_string()));
    let policy = dartforge_wasm_gate::Policy {
        required_exports: required,
        import_modules: vec![
            dartforge_wasm_gate::HOST_MODULE.to_string(),
            dartforge_wasm_gate::JS_MODULE.to_string(),
        ],
        ..dartforge_wasm_gate::Policy::default()
    };
    dartforge_wasm_gate::validate(&bytes, &policy)
        .map(drop)
        .map_err(|errors| {
            format!(
                "{} is outside the {} profile:\n{}",
                module.display(),
                dartforge_wasm_gate::PROFILE_NAME,
                errors.join("\n")
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_export_the_abi_and_never_allow_undefined() {
        let objects = [PathBuf::from("app.o"), PathBuf::from("sdk.o")];
        let l = Linkage {
            lld: Path::new("lld"),
            objects: &objects,
            runtime: Path::new("libdfrt.a"),
            output: Path::new("app.wasm"),
            debug_info: false,
        };
        let args: Vec<String> = arguments(&l)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], ["-flavor", "wasm", "app.o", "sdk.o"]);
        assert!(args.contains(&"--export=dartforge_run".to_string()));
        assert!(args.contains(&"--export=dartforge_abi_version".to_string()));
        assert!(args.contains(&"--strip-all".to_string()));
        assert!(!args.iter().any(|a| a.contains("allow-undefined")
            || a.contains("import-undefined")
            || a.contains("unresolved-symbols")));
    }

    /// The real wasm32 runtime (`RuntimeCache::for_wasm`) with the W2 proof
    /// program (tests/wasm/w2/runtime.ll): no symbol left undefined, and the
    /// module imports only host ABI v1 functions.
    #[test]
    fn links_the_wasm_runtime_with_only_host_imports() {
        let dir = tempfile::tempdir().unwrap();
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let object = dir.path().join("runtime.o");
        let status = Command::new(&clang)
            .args(["--target=wasm32-unknown-unknown", "-O2", "-c"])
            .args(crate::target::object_flags_for(crate::target::Os::Wasm))
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/wasm/w2/runtime.ll"))
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap();
        assert!(status.success());
        let output = dir.path().join("runtime.wasm");
        link(&Linkage {
            lld: &lld(&clang),
            objects: std::slice::from_ref(&object),
            runtime: &crate::cache::RuntimeCache::for_wasm().unwrap().lib_path,
            output: &output,
            debug_info: false,
        })
        .unwrap();
        let report = dartforge_wasm_gate::validate(
            &std::fs::read(&output).unwrap(),
            &dartforge_wasm_gate::Policy::default(),
        )
        .unwrap();
        let known = [
            "write",
            "clock_realtime_us",
            "clock_monotonic_us",
            "random_fill",
            "time_zone",
            "exit",
            "option",
        ];
        for (module, name) in &report.imports {
            assert_eq!(module, dartforge_wasm_gate::HOST_MODULE);
            assert!(known.contains(&name.as_str()), "unexpected import {name}");
        }
    }

    /// Clang's object with the profile's flags, linked by LLD with an empty
    /// runtime archive: a module the gate accepts. Then the same program with an
    /// unresolved symbol: the link fails instead of inventing an import.
    #[test]
    fn links_a_module_the_gate_accepts_and_refuses_undefined_symbols() {
        let dir = tempfile::tempdir().unwrap();
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let header = "target triple = \"wasm32-unknown-unknown\"\n\
            declare i32 @write(i32, i32, i32) #0\n\
            attributes #0 = { \"wasm-import-module\"=\"dartforge_host_v1\" \"wasm-import-name\"=\"write\" }\n\
            @msg = private constant [3 x i8] c\"ok\\0A\"\n\
            define i32 @dartforge_abi_version() { ret i32 1 }\n\
            define i32 @dartforge_resume() { ret i32 0 }\n\
            define double @dartforge_delay_us() { ret double 0.0 }\n";
        let runtime = dir.path().join("libempty.a");
        std::fs::write(&runtime, b"!<arch>\n").unwrap();
        let lld = lld(&clang);
        let build = |name: &str, body: &str, extra: &str| {
            let object = dir.path().join(format!("{name}.o"));
            let ll = dir.path().join(format!("{name}.ll"));
            let ir = format!("{header}{extra}define i32 @dartforge_run() {{\n{body}}}\n");
            std::fs::write(&ll, ir).unwrap();
            let status = Command::new(&clang)
                .args(["--target=wasm32-unknown-unknown", "-O2", "-x", "ir", "-c"])
                .args(crate::target::object_flags_for(crate::target::Os::Wasm))
                .arg(&ll)
                .arg("-o")
                .arg(&object)
                .status()
                .unwrap();
            assert!(status.success());
            let output = dir.path().join(format!("{name}.wasm"));
            let result = link(&Linkage {
                lld: &lld,
                objects: std::slice::from_ref(&object),
                runtime: &runtime,
                output: &output,
                debug_info: false,
            });
            (result, output)
        };
        let (result, output) = build(
            "good",
            "  %w = call i32 @write(i32 1, i32 ptrtoint (ptr @msg to i32), i32 3)\n  ret i32 0\n",
            "",
        );
        result.unwrap();
        let report = dartforge_wasm_gate::validate(
            &std::fs::read(&output).unwrap(),
            &dartforge_wasm_gate::Policy::default(),
        )
        .unwrap();
        assert_eq!(
            report.imports,
            vec![("dartforge_host_v1".to_string(), "write".to_string())]
        );
        let (result, _) = build(
            "missing",
            "  %v = call i32 @dartforge_not_ported()\n  ret i32 %v\n",
            "declare i32 @dartforge_not_ported()\n",
        );
        let error = result.unwrap_err();
        assert!(error.contains("dartforge_not_ported"), "{error}");
    }
}
