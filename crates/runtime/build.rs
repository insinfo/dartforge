//! The single source of the native runtime, also compiled as a module of this crate.
//!
//! The runtime that generated code calls is the concatenation, in the order of
//! [`FRAGMENTOS`], of the files `src/<fragment>.rs` (one per topic: core, collector
//! roots, exceptions, output, strings, collections, closures…). The fragments are
//! not Rust modules: they are pieces of ONE program, which see each other without
//! `use`. There is a single list, and it is this one; the text is compiled in two
//! ways:
//!
//! * by the AOT, as text (`RUNTIME_MAIN`), with a standalone `rustc` and `-O`
//!   (`crates/emit_native/src/cache.rs`), becoming the `.lib` that the executable
//!   links — with the C `main` that calls `@dartforge_entry`;
//! * by this crate, as the `abi` module, for the JIT (`crates/jit`), which publishes
//!   the functions' addresses as absolute symbols — **without** the C `main`, which
//!   would clash with the `main` of any Rust binary.
//!
//! This script:
//!
//! 1. writes into `OUT_DIR` the concatenated text (`runtime_main.rs`, which
//!    `RUNTIME_MAIN` includes) and the body of the `abi` module (`abi.rs`: one
//!    `include!` per fragment, in the same order, so that compile errors point to
//!    the real file);
//! 2. enables the `dartforge_runtime_embutido` cfg, which removes the C `main` and
//!    the declaration of `dartforge_entry` (the AOT's standalone `rustc` does not
//!    receive it);
//! 3. generates `simbolos.rs`: the `(name, address)` table of every
//!    `#[unsafe(no_mangle)] pub [unsafe] extern "C" fn` of **all** the fragments,
//!    except `main`. No name is written by hand; taking the address also prevents
//!    the linker from discarding the rlib's functions.
//!
//! A new file in `src/` that is neither `lib.rs`, one of the `MODULOS` nor a listed
//! fragment breaks the build: a fragment forgotten outside the list would be
//! runtime code that neither profile compiles.
use std::path::PathBuf;

/// The runtime fragments, in concatenation order. Adding a fragment means adding
/// its name here (and the file in `src/`).
const FRAGMENTOS: &[&str] = &[
    "core",
    "gc_roots",
    "arc_abi",
    "arc_native_ports",
    "exceptions",
    // The Rust → Dart ports and the personality of table-based exceptions.
    "exceptions_tables",
    "stack_trace",
    // `DARTFORGE_EFEITOS=conferir`: the effects table check.
    "effects_check",
    "output",
    "strings",
    "collections",
    "closures",
    // δ (P5b): the natives of the SDK from source (crates/emit_native/src/nativos.rs).
    "natives_numbers",
    "natives_strings",
    // The fast path of the default `_Map`/`_Set` for `int` and `String` keys.
    "natives_hash",
    // Clock, time zone and entropy.
    "natives_system",
    // The `RegExp` regular expression engine.
    "regexp",
    // P2 (α): operators on dynamic/num (a stopgap until P5).
    "dispatch",
    // δ (P5c): method tables and lookup by selector for the SDK from source.
    "selectors",
    "natives_lists",
    // The typed lists of the VM's `typed_data_patch.dart`.
    "typed_data",
    // The SIMD types (`Float32x4`, `Int32x4`, `Float64x2`).
    "simd",
    // P6: the event loop (microtasks and timers).
    "events",
    // Ports and the isolate's message queue.
    "ports",
    // Isolates (`Isolate.spawn`, the control port).
    "isolates",
    // dart:ffi (dynamic libraries, native memory, call trampolines).
    "ffi",
    "ffi_callbacks",
    // The native port API (Dart_PostCObject…) and the DL API data.
    "ffi_api_nativa",
    // Finalizer and NativeFinalizer on top of the collector's attachments.
    "finalizers",
    // The VM's `dart:io`: files, directories, the IOService and the platform.
    "io_files",
    "io_directories",
    "io_service",
    "io_platform",
    // The event handler, the sockets and the processes.
    "io_events",
    "io_sockets",
    "io_sockets_unix",
    // File watching (`FileSystemEntity.watch`).
    "io_observador",
    "io_processes",
    // The same on Windows: the completion port, Winsock and the processes.
    "io_windows_eventos",
    "io_windows_sockets",
    "io_windows_processes",
    // The `dart:io` TLS (`SecureSocket`, `SecurityContext`), on top of rustls.
    "tls",
    // The key and certificate formats of `SecurityContext` (PKCS#12, encrypted
    // keys).
    "tls_formats",
    // The `dart:io` zlib/gzip filters (`ZLibEncoder`, `GZipCodec`).
    "zlib",
    // What a program sees in the JIT VM that the AOT does not provide, for the
    // builders: the `dart:mirrors` subset (`reflectClass`) and the package config.
    "compat_jit",
    // `dart:developer` and the timeline in the production profile.
    "natives_developer",
    // RTI: run-time types.
    "types",
    // The executable's global allocator, with per-thread free lists (N17).
    "allocator",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_dll)");
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rustc-check-cfg=cfg(dartforge_runtime_embutido)");
    // The features choose the profile of the `abi` module: with none, the JIT
    // (without the C `main`); `aot`, the executable's `staticlib` (with `main`);
    // `dll`, that of the shared library of the SDK from source (without `main`).
    // Both `staticlib`s come out of `crates/runtime_static`.
    let aot = std::env::var_os("CARGO_FEATURE_AOT").is_some();
    let dll = std::env::var_os("CARGO_FEATURE_DLL").is_some();
    assert!(!(aot && dll), "the runtime's aot and dll features are mutually exclusive");
    if dll {
        println!("cargo::rustc-cfg=dartforge_runtime_dll");
    } else if !aot {
        println!("cargo::rustc-cfg=dartforge_runtime_embutido");
    }

    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    check_list(&src);

    let mut text = String::new();
    let mut abi = String::from("// GENERATED by crates/runtime/build.rs — do not edit.\n");
    for f in FRAGMENTOS {
        let file_path = src.join(format!("{f}.rs"));
        let content = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", file_path.display()));
        assert!(
            content.ends_with('\n'),
            "{} does not end with a newline",
            file_path.display()
        );
        text.push_str(&content);
        let literal = format!("{:?}", file_path.to_str().expect("UTF-8 path"));
        abi.push_str(&format!("include!({literal});\n"));
    }

    let mut names = exported_names(&text);
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert!(
        names.len() - sorted.len() == 1
            && names.iter().filter(|nome| nome.as_str() == "dartforge_personalidade").count() == 2
            || sorted.len() == names.len(),
        "the runtime defines some #[unsafe(no_mangle)] symbol twice"
    );
    // The two personality signatures are mutually exclusive per platform.
    let mut seen_set = std::collections::HashSet::new();
    names.retain(|nome| seen_set.insert(nome.clone()));

    let mut output = String::from(
        "// GENERATED by crates/runtime/build.rs from the fragments in src/ — do not edit.\n\n\
         /// The runtime fragments, in the order they are concatenated.\n\
         pub const FRAGMENTOS: &[&str] = &[\n",
    );
    for f in FRAGMENTOS {
        output.push_str(&format!("    \"{f}\",\n"));
    }
    output.push_str(
        "];\n\n\
         /// Names of the symbols the runtime exports to generated code, in source order.\n\
         pub const NOMES: &[&str] = &[\n",
    );
    for nome in &names {
        // Unix only, as in [`tabela`].
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            output.push_str("    #[cfg(unix)]\n");
        }
        output.push_str(&format!("    \"{nome}\",\n"));
    }
    output.push_str(
        "];\n\n/// Address of each runtime function in this process, in the order of [`NOMES`].\n\
         pub fn tabela() -> Vec<(&'static str, usize)> {\n    vec![\n",
    );
    for nome in &names {
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            output.push_str("        #[cfg(unix)]\n");
        }
        output.push_str(&format!(
            "        (\"{nome}\", crate::abi::{nome} as *const () as usize),\n"
        ));
    }
    output.push_str("    ]\n}\n");
    // @Native for the reserved ABI needs the runtime that serves the program, not a
    // same-named one in the host executable of an RTLD_LOCAL library. The same
    // selection is generated in the Cargo module and in the AOT's standalone text.
    let mut resolver = String::from("fn endereco_nativo_do_runtime(nome: &str) -> Option<usize> {\n    match nome {\n");
    for nome in &names {
        if matches!(nome.as_str(), "dartforge_objeto_de_desenrolamento" | "dartforge_desenrolamento_falhou" | "dartforge_personalidade_cleanup_itanium") {
            resolver.push_str("        #[cfg(unix)]\n");
        }
        resolver.push_str(&format!("        \"{nome}\" => Some({nome} as *const () as usize),\n"));
    }
    resolver.push_str("        _ => None,\n    }\n}\n");
    text.insert_str(0, &resolver);
    abi.push_str(&resolver);
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("simbolos.rs"), output).expect("write simbolos.rs");
    std::fs::write(out.join("efeitos.rs"), effects_table(&manifest, &names)).expect("write efeitos.rs");
    std::fs::write(out.join("ownership.rs"), ownership_table(&manifest, &names)).expect("write ownership.rs");
    std::fs::write(out.join("runtime_main.rs"), text).expect("write runtime_main.rs");
    std::fs::write(out.join("abi.rs"), abi).expect("write abi.rs");
}

/// Audited ARC contracts; does not invent contracts for the externs still missing.
/// The coercion of the generated pointers checks arity and representation against Rust.
fn ownership_table(manifest: &std::path::Path, names: &[String]) -> String {
    let file_path = manifest.join("ownership.tsv");
    println!("cargo::rerun-if-changed={}", file_path.display());
    let text = std::fs::read_to_string(file_path).expect("read ownership.tsv");
    let efeitos = std::fs::read_to_string(manifest.join("efeitos.tsv")).expect("read efeitos.tsv");
    let mut line_list = std::collections::BTreeMap::new();
    let tipo = |s: &str, resultado: bool| -> (&str, String) {
        if resultado
            && let Some(n) = s
                .strip_prefix("ref:borrow(")
                .and_then(|s| s.strip_suffix(')'))
        {
            let n: usize = n.parse().expect("ownership.tsv: invalid borrow index");
            return ("i64", format!("ModoResultado::BorrowArg({n})"));
        }
        let (ty, mode) = match (s, resultado) {
            ("ref:borrow", false) => ("i64", "ModoParametro::Borrow"),
            ("ref:consume", false) => ("i64", "ModoParametro::Consume"),
            ("ref:consume-success", false) => ("i64", "ModoParametro::ConsumeSuccess"),
            ("ref:consume-error", false) => ("i64", "ModoParametro::ConsumeError"),
            ("i64:scalar", false) => ("i64", "ModoParametro::Scalar"),
            ("f64:scalar", false) => ("f64", "ModoParametro::ScalarF64"),
            ("u8:scalar", false) => ("u8", "ModoParametro::ScalarI8"),
            ("i8:scalar", false) => ("i8", "ModoParametro::ScalarI8"),
            ("i64:native", false) => ("i64", "ModoParametro::Native"),
            ("ptr:native", false) => ("*const i64", "ModoParametro::EnderecoNative"),
            // Constant module bytes (a UTF-8 name), the same native address.
            ("bytes:native", false) => ("*const u8", "ModoParametro::EnderecoNative"),
            ("vetor:native", false) => ("*const i64", "ModoParametro::VetorEstatico"),
            ("tabela:native", false) => ("extern \"C\" fn() -> *const i64", "ModoParametro::TabelaEstatica"),
            ("ref:owned", true) => ("i64", "ModoResultado::Owned"),
            ("i64:scalar", true) => ("i64", "ModoResultado::ScalarI64"),
            ("f64:scalar", true) => ("f64", "ModoResultado::ScalarF64"),
            ("i8:scalar", true) => ("i8", "ModoResultado::ScalarI8"),
            ("i1:scalar", true) => ("bool", "ModoResultado::ScalarI1"),
            ("u8:scalar", true) => ("u8", "ModoResultado::ScalarI8"),
            ("void:scalar", true) => ("()", "ModoResultado::Void"),
            _ => panic!("ownership.tsv: unsupported type/contract: {s}"),
        };
        (ty, mode.to_string())
    };
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 6, "ownership.tsv:{}: six fields", n + 1);
        assert!(
            names.iter().any(|nome| nome == c[0]),
            "ownership.tsv: unknown symbol {}",
            c[0]
        );
        // New entries require an explicit audit. Absence does not produce a default.
        let effect = efeitos
            .lines()
            .find(|line| line.split('\t').next() == Some(c[0]))
            .expect("ownership.tsv: symbol without effects");
        let effect: Vec<_> = effect.split('\t').collect();
        let pode_falhar = match c[5] {
            "normal" => false,
            "pending" | "pending-dart" => true,
            _ => panic!("ownership.tsv: unknown exit {}", c[5]),
        };
        assert!(
            effect.len() == 4
                && effect[2] == if pode_falhar { "1" } else { "0" }
                && effect[3] == if c[5] == "pending-dart" { "1" } else { "0" },
            "ownership.tsv: exits incompatible with the effects of {}",
            c[0]
        );
        let parametros: Vec<_> = if c[1] == "-" {
            vec![]
        } else {
            c[1].split(',').map(|s| tipo(s, false)).collect()
        };
        let resultado = tipo(c[2], true);
        if let Some(n) = c[2]
            .strip_prefix("ref:borrow(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n: usize = n.parse().unwrap();
            assert!(
                parametros
                    .get(n)
                    .is_some_and(|(_, mode)| mode == "ModoParametro::Borrow"),
                "ownership.tsv: a borrowed result requires an existing borrowed Ref parameter"
            );
        }
        assert!(
            pode_falhar
                || parametros.iter().all(|(_, mode)| !matches!(
                    mode.as_str(),
                    "ModoParametro::ConsumeSuccess" | "ModoParametro::ConsumeError"
                )),
            "ownership.tsv: consumption by edge requires pending"
        );
        let mark = |s: &str| match s {
            "0" => false,
            "1" => true,
            _ => panic!("ownership.tsv: invalid mark {s}"),
        };
        let chama_dart = c[5] == "pending-dart";
        assert!(!chama_dart || mark(c[4]), "ownership.tsv: Dart reentry requires borrow invalidation");
        assert!(
            line_list
                .insert(
                    c[0],
                    (parametros, resultado, mark(c[3]), mark(c[4]), pode_falhar, chama_dart)
                )
                .is_none(),
            "ownership.tsv: duplicate symbol {}",
            c[0]
        );
    }
    for nome in names.iter().filter(|n| {
        n.starts_with("dartforge_arc_")
            || matches!(
                n.as_str(),
                "dartforge_gc_global_root"
                    | "dartforge_marcar_constante"
                    | "dartforge_gc_collect"
                    | "dartforge_marcar_permanente"
                    | "dartforge_nativo_DartForge_record_fieldAt"
                    | "dartforge_nativo_DartForge_record_numFields"
                    | "dartforge_nativo_DartForge_record_shape"
                    | "dartforge_print_handle"
                    | "dartforge_exception_pending"
                    | "dartforge_unbox_int"
                    | "dartforge_unbox_double"
                    | "dartforge_unbox_bool"
            )
    }) {
        assert!(
            line_list.contains_key(nome.as_str()),
            "ownership.tsv: audited extern without a contract {nome}"
        );
    }
    let mut output = String::from(
        r#"
/// Semantic mode of the argument, independent of the i64 width.
///
/// ```
/// use dartforge_runtime::ownership::ModoParametro;
/// assert_ne!(ModoParametro::Borrow, ModoParametro::Consume);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoParametro { Borrow, Consume, ConsumeSuccess, ConsumeError, Scalar, ScalarF64, ScalarI8, Native, EnderecoNative, TabelaEstatica, VetorEstatico }
/// Result of the audited externs, with no implicit convention.
///
/// ```
/// use dartforge_runtime::ownership::ModoResultado;
/// assert_ne!(ModoResultado::Owned, ModoResultado::Void);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoResultado { Owned, BorrowArg(usize), ScalarI64, ScalarF64, ScalarI8, ScalarI1, Void }
/// Audited contract, with the result available only on success.
///
/// ```
/// use dartforge_runtime::ownership::{CONTRATOS, ModoResultado};
/// let c = CONTRATOS.iter().find(|c| c.nome == "dartforge_arc_quadro_carregar_v1").unwrap();
/// assert_eq!(c.resultado, ModoResultado::Owned);
/// ```
#[derive(Debug)]
pub struct Contrato {
    /// Exact C symbol, associated with the effects table.
    pub nome: &'static str,
    /// One mode per parameter, in the order of the Rust signature.
    pub parametros: &'static [ModoParametro],
    /// Semantic convention of the normal result.
    pub resultado: ModoResultado,
    /// May keep owning occurrences after returning.
    pub retencao_persistente: bool,
    /// May remove owners or invalidate associated borrows.
    pub invalida_borrows: bool,
    /// Requires a pending-exception exit prepared in the CFG.
    pub pode_falhar: bool,
    /// May reenter Dart code; requires borrow invalidation.
    pub chama_dart: bool,
}
/// Partial audited catalog; absent names still have no contract.
pub const CONTRATOS: &[Contrato] = &[
"#,
    );
    for (nome, (params, (_, resultado), retention, invalid, pode_falhar, chama_dart)) in &line_list {
        let modes = params
            .iter()
            .map(|(_, mode)| mode.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("Contrato {{ nome: {nome:?}, parametros: &[{modes}], resultado: {resultado}, retencao_persistente: {retention}, invalida_borrows: {invalid}, pode_falhar: {pode_falhar}, chama_dart: {chama_dart} }},\n"));
    }
    output.push_str("];\n");
    for (nome, (params, (ret, _), _, _, _, _)) in &line_list {
        let types = params
            .iter()
            .map(|(ty, _)| *ty)
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "const _: unsafe extern \"C\" fn({types}) -> {ret} = crate::abi::{nome};\n"
        ));
    }
    output
}

#[cfg(test)]
mod testes_ownership {
    use super::*;

    #[test]
    fn table_rejects_incomplete_duplicate_and_invalid_contracts() {
        let root = std::env::temp_dir().join(format!("dartforge-ownership-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t0\t0\t0\n").unwrap();
        let good = "dartforge_arc_retain\tref:borrow\tvoid:scalar\t1\t0\tnormal\n";
        let names = vec!["dartforge_arc_retain".to_string()];
        for bad in [
            String::new(),
            format!("{good}{good}"),
            good.replace("ref:borrow", "ref:inventado"),
            good.replace("\t1\t0", "\t2\t0"),
            good.replace("arc_retain", "arc_ausente"),
            good.replace("normal", "inventado"),
            good.replace("ref:borrow", "ref:consume-success"),
            good.replace("void:scalar", "ref:borrow(1)"),
            good.replace("ref:borrow", "i64:scalar")
                .replace("void:scalar", "ref:borrow(0)"),
        ] {
            std::fs::write(root.join("ownership.tsv"), bad).unwrap();
            assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        }
        std::fs::write(root.join("ownership.tsv"), good).unwrap();
        assert!(ownership_table(&root, &names).contains("const _: unsafe extern \"C\" fn(i64) -> () = crate::abi::dartforge_arc_retain;"));
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        for mode in ["consume-success", "consume-error", "consume"] {
            std::fs::write(
                root.join("ownership.tsv"),
                good.replace("ref:borrow", &format!("ref:{mode}"))
                    .replace("normal", "pending"),
            )
            .unwrap();
            let generated = ownership_table(&root, &names);
            assert!(generated.contains("pode_falhar: true"));
        }
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t1\n").unwrap();
        let reentry = good.replace("normal", "pending-dart").replace("\t1\t0\t", "\t1\t1\t");
        std::fs::write(root.join("ownership.tsv"), &reentry).unwrap();
        assert!(ownership_table(&root, &names).contains("chama_dart: true"));
        std::fs::write(root.join("ownership.tsv"), reentry.replace("\t1\t1\t", "\t1\t0\t")).unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        std::fs::write(root.join("ownership.tsv"), &reentry).unwrap();
        std::fs::write(root.join("efeitos.tsv"), "dartforge_arc_retain\t1\t1\t0\n").unwrap();
        assert!(std::panic::catch_unwind(|| ownership_table(&root, &names)).is_err());
        // The callback has its own Rust representation: do not accept a generic
        // integer or pointer in place of the function that returns the static table.
        let factory = "dartforge_arc_objeto_owned_t_v1".to_string();
        std::fs::write(root.join("efeitos.tsv"), format!("{factory}\t1\t0\t0\n")).unwrap();
        std::fs::write(root.join("ownership.tsv"), format!("{factory}\ti64:native,i64:scalar,tabela:native\tref:owned\t0\t1\tnormal\n")).unwrap();
        let generated = ownership_table(&root, &[factory]);
        assert!(generated.contains("const _: unsafe extern \"C\" fn(i64, i64, extern \"C\" fn() -> *const i64) -> i64 = crate::abi::dartforge_arc_objeto_owned_t_v1;"));
        assert!(generated.contains("ModoParametro::TabelaEstatica"));
        std::fs::remove_file(root.join("ownership.tsv")).unwrap();
        std::fs::remove_file(root.join("efeitos.tsv")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

/// The extern effects table (`efeitos.tsv`, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §13.8) as code for the emitter: `(name, collects, throws, runs Dart)`, in
/// alphabetical order.
///
/// The file is the single source and must have exactly one line per
/// `#[unsafe(no_mangle)]` function of the fragments: a new extern without a line, a
/// line for an extern that is gone, a repeated name or a mark other than `0`/`1`
/// break the build — an extern without a mark would be handled blindly.
/// `roda_dart = 1` requires `coleta = 1` and `lanca = 1`.
fn effects_table(manifest: &std::path::Path, names: &[String]) -> String {
    let file_path = manifest.join("efeitos.tsv");
    println!("cargo::rerun-if-changed={}", file_path.display());
    let text = std::fs::read_to_string(&file_path).unwrap_or_else(|e| panic!("read {}: {e}", file_path.display()));
    let mut line_list: Vec<(String, bool, bool, bool)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let mark = |i: usize| match fields.get(i).copied() {
            Some("0") => false,
            Some("1") => true,
            _ => panic!("efeitos.tsv:{}: the line is `nome<TAB>coleta<TAB>lanca<TAB>roda_dart`, with marks 0 or 1", n + 1),
        };
        assert!(fields.len() == 4, "efeitos.tsv:{}: four TAB-separated fields", n + 1);
        let (collection, throws, roda_dart) = (mark(1), mark(2), mark(3));
        assert!(!roda_dart || (collection && throws), "efeitos.tsv:{}: roda_dart = 1 requires coleta = 1 and lanca = 1", n + 1);
        line_list.push((fields[0].to_string(), collection, throws, roda_dart));
    }
    line_list.sort();
    for pair in line_list.windows(2) {
        assert!(pair[0].0 != pair[1].0, "efeitos.tsv: `{}` appears twice", pair[0].0);
    }
    let in_table: std::collections::BTreeSet<&str> = line_list.iter().map(|l| l.0.as_str()).collect();
    let in_runtime: std::collections::BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let without_line: Vec<&&str> = in_runtime.difference(&in_table).collect();
    let without_extern: Vec<&&str> = in_table.difference(&in_runtime).collect();
    assert!(
        without_line.is_empty() && without_extern.is_empty(),
        "crates/runtime/efeitos.tsv does not match the runtime's externs.\n  \
         without a line in the table (add `nome<TAB>1<TAB>1<TAB>0`, the conservative mark): {without_line:?}\n  \
         in the table but not in the runtime (remove the line): {without_extern:?}"
    );
    let mut output = String::from(
        "// GENERATED by crates/runtime/build.rs from efeitos.tsv — do not edit.\n\n\
         /// `(name, collects, throws, runs Dart)` of each runtime extern, in\n\
         /// alphabetical order of the name.\n\
         pub const EFEITOS: &[(&str, bool, bool, bool)] = &[\n",
    );
    for (nome, collection, throws, roda_dart) in &line_list {
        output.push_str(&format!("    (\"{nome}\", {collection}, {throws}, {roda_dart}),\n"));
    }
    output.push_str("];\n");
    output
}

/// The files in `src/` that are Rust modules (and not fragments): `lib.rs` declares
/// them with `pub mod` and `RUNTIME_MAIN` wraps them in `mod x { … }`. The unified
/// space (docs/NATIVO-ESPACO-UNIFICADO.md §3.1–§3.3) adds the layout contract, the
/// object space and the per-kind views; the compressed form of the RTI table
/// (`tabela_rti`) serves the compiler and the runtime.
const MODULOS: &[&str] = &["heap", "hash", "arc", "layout", "space", "textos", "boxes", "lists", "tipadas", "tabela_rti"];

/// Every `src/*.rs` is `lib.rs`, one of the [`MODULOS`] or a listed fragment.
fn check_list(src: &std::path::Path) {
    let entries = std::fs::read_dir(src).unwrap_or_else(|e| panic!("read {}: {e}", src.display()));
    for input in entries {
        let file_path = input.expect("entry of src/").path();
        if file_path.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let nome = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            nome == "lib" || MODULOS.contains(&nome) || FRAGMENTOS.contains(&nome),
            "{} is not a runtime fragment: add \"{nome}\" to FRAGMENTOS in crates/runtime/build.rs",
            file_path.display()
        );
    }
}

/// Names of the runtime's `#[unsafe(no_mangle)]` functions, except the C `main`.
///
/// The recognized form is the one the fragments use: the attribute on one line and,
/// on the next, `pub [unsafe] extern "C" fn name(`. Other attributes
/// (`#[cfg(...)]`) may come between the two. Any other form breaks the build,
/// because it would be a symbol that the table could not publish.
fn exported_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut line_list = text.lines();
    while let Some(line) = line_list.next() {
        if line.trim() != "#[unsafe(no_mangle)]" {
            continue;
        }
        let next_one = loop {
            match line_list.next() {
                Some(l) if l.trim().starts_with("#[") => continue,
                Some(l) => break l.trim(),
                None => break "",
            }
        };
        let fn_signature = next_one
            .strip_prefix("pub unsafe extern \"C\" fn ")
            .or_else(|| next_one.strip_prefix("pub extern \"C\" fn "))
            .unwrap_or_else(|| {
                panic!(
                    "#[unsafe(no_mangle)] followed by an unrecognized form in the runtime: `{next_one}`"
                )
            });
        let nome: String = fn_signature
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if nome != "main" {
            names.push(nome);
        }
    }
    names
}
