//! The SDK compiled from source, per library (P5, docs/NATIVO-PLANO.md §7).
//!
//! The native `dart:core` comes from the source of SDK 3.6.2 — the `vm` section of
//! `libraries.json` with its patches — and from a thin overlay of ours,
//! `native_sdk/` at the root of the repository, which replaces only the files that depend
//! on the VM's internal machinery (`_SuspendState`, `Timer`/microtasks tied to
//! `dart:isolate`, the `Finalizer` of the weak GC). The replacement is done at
//! load time (`SdkLayout::load_with_overlay`, `crates/elements`).
//!
//! This module gathers what belongs to the SDK as a compilation unit: where the
//! overlay is, which libraries come from source, and the 5a measurement — the
//! inference diagnostics in the bodies of those libraries, which must be
//! zero before the lowering compiles them.

use crate::generator::{Format, Generation, Generator};
use dartforge_elements::sdk::SdkLayout;
use std::path::{Path, PathBuf};

/// The overlay target in `native_sdk/libraries.json`.
pub const OVERLAY_TARGET: &str = "dartforge_native";

/// The libraries that the native backend compiles from the SDK source (decision 1 of
/// NATIVO-PLANO §7.1), in approximate dependency order. `typed_data`
/// comes later (P9).
pub const SOURCE_LIBRARIES: &[&str] =
    &[
        "_internal",
        "core",
        "_compact_hash",
        "collection",
        "math",
        "convert",
        "async",
        "typed_data",
        "isolate",
        "developer",
        "nativewrappers",
        "ffi",
        "_http",
        "io",
        "mirrors",
    ];

/// Overlay directory: `DARTFORGE_NATIVE_SDK`, otherwise the `native_sdk/`
/// of the repository that compiled this binary.
pub fn overlay_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_NATIVE_SDK") {
        return PathBuf::from(d);
    }
    if let Some(d) = dartforge_elements::distribution::native_overlay() {
        return d;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native_sdk")
}

/// The native SDK: the `vm` section of `lib_dir` with the overlay.
pub fn load_native_sdk(lib_dir: &Path) -> Result<SdkLayout, String> {
    SdkLayout::load_with_overlay(lib_dir, &overlay_dir(), OVERLAY_TARGET)
}

/// Inference diagnostics in the bodies of an SDK library.
#[derive(Debug, Clone)]
pub struct LibraryInference {
    pub library: String,
    /// Functions (and methods, constructors, accessors) declared in it.
    pub functions: usize,
    /// The messages, in order.
    pub diagnostics: Vec<String>,
}

/// The 5a measurement: loads a program that imports all the
/// [`SOURCE_LIBRARIES`] with the overlay, resolves the outline and infers
/// the bodies of each library **separately** (the diagnostics do not
/// carry the file; one inference per library attributes them).
///
/// Depends on the body inference (`crates/types`) inferring the requested SDK
/// libraries (requested in docs/NATIVO-PEDIDOS.md); without that, only the
/// initializers without a written type are visited and the diagnostics stay
/// near zero without saying anything.
pub fn measure_sdk_inference(lib_dir: &Path) -> Result<Vec<LibraryInference>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-medir-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;

    let mut interner = Interner::new();
    let (program, load_diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = load_diags.first() {
        return Err(format!("loading the SDK with the overlay: {} ({} diagnostics)", d.message, load_diags.len()));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);

    let mut exit = Vec::new();
    for name in SOURCE_LIBRARIES {
        let uri = format!("dart:{name}");
        let Some(idx) = program.libraries.iter().position(|l| l.uri == uri) else {
            return Err(format!("{uri} did not load"));
        };
        let lib = dartforge_elements::model::LibraryId(idx as u32);
        let functions = program.functions.iter().filter(|f| f.library == lib).count();
        let (_bodies, diags) =
            dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &[lib]);
        exit.push(LibraryInference {
            library: uri,
            functions,
            diagnostics: diags.into_iter().map(|d| d.message).collect(),
        });
    }
    Ok(exit)
}


/// The function that registers in the runtime the classes of a source SDK
/// library (names, subtypes, method tables); the program entry
/// calls it.
pub fn registration_symbol(uri: &str) -> String {
    format!("df.register.{}", crate::context::escape_str(uri))
}

/// The registration functions of the [`SOURCE_LIBRARIES`], in order.
pub fn sdk_records() -> Vec<String> {
    SOURCE_LIBRARIES.iter().map(|b| registration_symbol(&format!("dart:{b}"))).collect()
}

/// The class ids (of the source SDK) of the values that the runtime represents,
/// in the order of `runtime/src/selectors.rs` (`CID_*`).
pub fn runtime_cids(ctx: &crate::context::Context) -> Vec<i64> {
    const CLASSES: &[(&str, &str)] = &[
        ("core", "Null"),
        ("core", "_Smi"),
        ("core", "_Mint"),
        ("core", "_Double"),
        ("core", "bool"),
        ("core", "_OneByteString"),
        ("core", "_TwoByteString"),
        ("core", "_GrowableList"),
        ("core", "_List"),
        ("core", "_ImmutableList"),
        ("core", "_Closure"),
        ("core", "_Record"),
        ("typed_data", "_Uint8List"),
        ("typed_data", "_Uint8ArrayView"),
        ("typed_data", "_Int64List"),
        ("typed_data", "_Float32x4"),
        ("typed_data", "_Int32x4"),
        ("typed_data", "_Float64x2"),
        ("typed_data", "_Int8List"),
        ("typed_data", "_Uint8ClampedList"),
        ("typed_data", "_Int16List"),
        ("typed_data", "_Uint16List"),
        ("typed_data", "_Int32List"),
        ("typed_data", "_Uint32List"),
        ("typed_data", "_Uint64List"),
        ("typed_data", "_Float32List"),
        ("typed_data", "_Float64List"),
        ("typed_data", "_Int32x4List"),
        ("typed_data", "_Float32x4List"),
        ("typed_data", "_Float64x2List"),
        ("isolate", "_SendPort"),
        ("isolate", "_Capability"),
    ];
    CLASSES
        .iter()
        .map(|(l, c)| ctx.sdk_class(l, c).and_then(|k| ctx.class_id_of(k)).map_or(-1, i64::from))
        .collect()
}

/// A minimal program that imports the source libraries, loaded with the
/// overlay.
fn load_source_libraries(
    lib_dir: &Path,
) -> Result<(dartforge_elements::Program, dartforge_intern::Interner), String> {
    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!(
        "dartforge-sdk-fonte-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;
    let mut interner = dartforge_intern::Interner::new();
    let (program, load_diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = load_diags.first() {
        return Err(format!("loading the SDK with the overlay: {}", d.message));
    }
    Ok((program, interner))
}

/// The id table of the source SDK classes (`context::IdTable`),
/// computed once per content of the SDK, of the overlay and of the compiler,
/// and kept in `<cache native>/sdk_ids/<key>.tsv`. The program uses it to
/// number the SDK classes as the SDK module numbers them, without loading
/// the SDK libraries it does not import.
///
/// # Errors
/// Failure to load the SDK with the overlay, or to write the cache.
pub fn sdk_class_ids(lib_dir: &Path) -> Result<std::sync::Arc<crate::context::IdTable>, String> {
    use crate::context::IdTable;
    use std::sync::{Arc, Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<std::collections::HashMap<PathBuf, Arc<IdTable>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    if let Some(t) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(lib_dir) {
        return Ok(t.clone());
    }
    let key = sdk_key(lib_dir, "class-ids", &[]);
    let dir = crate::cache::native_cache_dir().join("sdk_ids");
    let file = dir.join(format!("{key}.tsv"));
    let table = match std::fs::read_to_string(&file).ok().and_then(|t| IdTable::from_string(&t)) {
        Some(t) => t,
        None => {
            let lib_dir_c = lib_dir.to_path_buf();
            let t = std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || -> Result<IdTable, String> {
                    let (program, interner) = load_source_libraries(&lib_dir_c)?;
                    let compiled: Vec<bool> = program
                        .libraries
                        .iter()
                        .map(|l| l.uri.strip_prefix("dart:").is_some_and(|n| SOURCE_LIBRARIES.contains(&n)))
                        .collect();
                    Ok(crate::context::sdk_class_ids(&program, &interner, &compiled))
                })
                .map_err(|e| e.to_string())?
                .join()
                .map_err(|_| "panic while numbering the SDK classes".to_string())??;
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            // Atomic publication: another process may be writing the same
            // key (the content is the same).
            let tmp = dir.join(format!("{key}.{}.tmp", std::process::id()));
            std::fs::write(&tmp, t.to_text()).map_err(|e| format!("{}: {e}", tmp.display()))?;
            if std::fs::rename(&tmp, &file).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            t
        }
    };
    let t = Arc::new(table);
    memo.lock().unwrap_or_else(|e| e.into_inner()).insert(lib_dir.to_path_buf(), t.clone());
    Ok(t)
}

/// A source SDK library lowered and emitted.
#[derive(Debug, Clone)]
pub struct SdkLibrary {
    /// `dart:core`…
    pub uri: String,
    /// The LLVM IR of its module.
    pub ir: String,
    /// The rejected members: (symbol, reason).
    pub rejected: Vec<(String, String)>,
    /// The method tables that the module cites but does not define (the
    /// production profile, docs/NATIVO-PODA-DE-TABELAS.md §3.2): they go into the summary of the
    /// library. Empty in development.
    pub tables: Vec<crate::pruning::ModuleTable>,
}

/// Lowers and emits each of the [`SOURCE_LIBRARIES`] in its own module (P5c):
/// open world, stable symbols, the members that do not lower rejected one
/// by one. It is a function only of the SDK sources, the overlay and the compiler.
/// `production`: the production profile ([`SdkProfile::Production`]), which never
/// reloads. The global area descriptor goes without the slot names
/// (`LlvmEmitter::with_lean_area`), every function gets `optsize`
/// ([`with_optsize`]) and the method tables are left for the link
/// (`LlvmEmitter::with_tables_at_link`, docs/NATIVO-PODA-DE-TABELAS.md).
pub fn emit_sdk_libraries(lib_dir: &Path, production: bool) -> Result<Vec<SdkLibrary>, String> {
    use dartforge_types::table::{CoreTypes, TypeTable};
    let (program, interner) = load_source_libraries(lib_dir)?;
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = SOURCE_LIBRARIES
        .iter()
        .filter_map(|b| {
            let uri = format!("dart:{b}");
            program
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| dartforge_elements::model::LibraryId(i as u32))
        })
        .collect();
    if libs.len() != SOURCE_LIBRARIES.len() {
        return Err("source SDK library outside the program".to_string());
    }
    let (bodies, _) =
        dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &libs);
    let base = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies);
    let base = base.with_source_sdk();
    // The SDK comes out in the exception model of the program (`target::table_exceptions`).
    let table_exceptions = crate::target::table_exceptions()?;
    let roots_by_maps = crate::target::roots_by_maps()?;
    // The SDK comes out in the memory policy of the program (`target::arc_memory`).
    let arc_memory = crate::target::arc_memory()?;
    // The symbolic stack trace (§13.14): the positions of the SDK functions and the table.
    let symbolic_stack_trace = crate::target::symbolic_stack_trace()?;
    let arc_prepared = crate::target::arc_prepared()?;
    let mut modules = Vec::new();
    for lib in libs {
        let uri = program.library(lib).uri.clone();
        let mut ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk()
            .only_the_library(lib);
        ctx.arc_memory = arc_memory;
        if symbolic_stack_trace {
            ctx.link_debug();
            ctx.stack_trace = true;
        }
        debug_assert_eq!(ctx.class_ids, base.class_ids);
        let mut module = crate::lower::lower_program(&ctx);
        module.sdk_library = true;
        // The classes of `null` and of the `Smi` inline (`llvm/mod.rs`, `df.class`).
        module.runtime_cids = runtime_cids(&ctx);
        module.record = Some(registration_symbol(&uri));
        if let Some(e) = module.errors.first() {
            return Err(format!("{uri}: the SDK module has a diagnostic outside a member: {e}"));
        }
        // The HIR optimizer (`optimize/`) also in the SDK: that is where the
        // code of `List`, `Map`, `String`… that programs call the most lives.
        crate::optimize::prepare_for_emission(&mut module, arc_memory, table_exceptions);
        modules.push((uri, module));
    }
    // The published ARC preparation: the libraries together (the SDK does not call the
    // program directly), in the fixed order of `SOURCE_LIBRARIES`.
    if arc_prepared {
        let mut hir: Vec<_> = modules.iter_mut().map(|(_, m)| std::mem::take(m)).collect();
        let mut plans: Vec<std::collections::HashMap<_, _>> = hir.iter().map(|m| {
            m.functions.iter().map(|f| (f.symbol.clone(), crate::optimize::arc::DartFunctionPlan::default())).collect()
        }).collect();
        crate::optimize::arc::prepare_arc_dart_modules(&mut hir, &mut plans)
            .map_err(|e| format!("ARC preparation of the source SDK: {e}"))?;
        crate::optimize::arc::certify_arc_module_ports(&mut hir, &plans)
            .map_err(|e| format!("ARC ports of the source SDK: {e}"))?;
        for ((_, m), h) in modules.iter_mut().zip(hir) {
            *m = h;
        }
    }
    let mut exit = Vec::new();
    for (uri, mut module) in modules {
        // The SDK DLL stays loaded for the whole process: the literals are
        // static objects in the JIT too (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
        let ir = crate::llvm::LlvmEmitter::new(&module)
            .with_lean_area(production)
            .with_static_objects(true)
            // The production SDK only goes to AOT; the development one (the DLL)
            // also serves the JIT, which migrates objects on reload.
            .with_body_without_outside(production)
            .with_tables_at_link(production)
            .with_roots_by_maps(roots_by_maps)
            .with_stack_trace(symbolic_stack_trace)
            .emit_all();
        if roots_by_maps {
            crate::llvm::verify_maps::verify(&ir).map_err(|e| format!("{uri}: {e}"))?;
        } else if crate::llvm::verify_shadow::linked() {
            crate::llvm::verify_shadow::verify(&ir).map_err(|e| format!("{uri}: {e}"))?;
        }
        let ir = if production { with_optsize(&ir) } else { ir };
        // Unwind table with exceptions by tables and with roots by
        // maps (the collector's walk; see `lib.rs`).
        let ir = if (table_exceptions || roots_by_maps) && cfg!(unix) { with_uwtable(&ir) } else { ir };
        let ir = if cfg!(unix) && !table_exceptions && !roots_by_maps { with_nounwind(&ir) } else { ir };
        // Only on Unix (macOS measured; Windows not).
        let ir = if cfg!(unix) && production && !table_exceptions && !roots_by_maps && !arc_memory {
            with_context_in_tls(&ir, ContextInTls::Free, "df.ctx.tls")
        } else {
            ir
        };
        let tables = if production { std::mem::take(&mut module.method_tables) } else { Vec::new() };
        exit.push(SdkLibrary { uri, ir, rejected: std::mem::take(&mut module.rejected), tables });
    }
    Ok(exit)
}

/// The source SDK compiled: a DLL with the runtime and the source
/// libraries, and the import library that the executable links.
///
/// **Why a DLL.** Linking the SDK objects (~12 MB) into each executable
/// cost ~450 ms of linking per program (measured; the previous link takes
/// ~60 ms), and the method tables, registered at startup, reach all the
/// code — `/OPT:REF` has nothing to remove. It is the "group = shared
/// code" of docs/NATIVO-PLANO §4.4: the runtime and the SDK are linked
/// **once per content**, and the executable links only the program object and the
/// import library. The DLL goes next to the executable (a hard
/// link, without a copy).
#[derive(Debug, Clone)]
pub struct CompiledSdk {
    pub objects: Vec<PathBuf>,
    pub dll: PathBuf,
    pub import: PathBuf,
    /// The pruning summaries of the method tables, one per library
    /// (`<library>.pruning`, docs/NATIVO-PODA-DE-TABELAS.md §3.3): only in the
    /// production profile.
    pub summaries: Vec<PathBuf>,
    /// How long it took to compile (the cold cost); `None` when it came from the
    /// cache.
    pub cold: Option<std::time::Duration>,
}

/// The symbols that an IR defines with external linkage (what the DLL exports).
fn defined_symbols(ir: &str, exit: &mut Vec<String>) {
    for l in ir.lines() {
        let Some(r) = l.strip_prefix("define ") else { continue };
        if r.starts_with("internal ") || r.starts_with("private ") || r.starts_with("linkonce_odr ") {
            continue;
        }
        let Some(i) = r.find('@') else { continue };
        let Some(f) = r[i..].find('(') else { continue };
        exit.push(r[i + 1..i + f].to_string());
    }
}

/// The key of the compiled SDK: blake3 of the SDK sources that enter (the
/// source libraries, the `vm` patches, `libraries.json`), of the
/// overlay, of the compiler (`DARTFORGE_EMITTER_HASH`, `build.rs`), of the
/// identity of Clang and of the flags.
fn sdk_key(lib_dir: &Path, clang_id: &str, args: &[&str]) -> String {
    fn join(dir: &Path, exit: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                join(&p, exit);
            } else if p.extension().is_some_and(|x| x == "dart" || x == "json") {
                exit.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in [
        "core",
        "async",
        "collection",
        "convert",
        "math",
        "internal",
        "typed_data",
        "isolate",
        "developer",
        "html/dartium",
        "_http",
        "io",
        "_internal/vm/lib",
        "_internal/vm/bin",
        "_internal/vm_shared/lib",
    ] {
        join(&lib_dir.join(d), &mut files);
    }
    files.push(lib_dir.join("libraries.json"));
    let overlay = overlay_dir();
    join(&overlay, &mut files);
    files.sort();
    let mut h = blake3::Hasher::new();
    h.update(b"dartforge-sdk-fonte\0");
    h.update(env!("CARGO_PKG_VERSION").as_bytes());
    h.update(b"\0");
    h.update(env!("DARTFORGE_EMITTER_HASH").as_bytes());
    h.update(b"\0");
    // The debug trace changes the code (`llvm/mod.rs`).
    h.update(std::env::var("DARTFORGE_CALL_TRACE").unwrap_or_default().as_bytes());
    h.update(b"\0");
    // The symbolic stack trace adds the labels and the table (`llvm/stack_trace.rs`);
    // the default does not enter.
    if std::env::var("DARTFORGE_STACK_TRACE").is_ok_and(|v| v == "symbolic") {
        h.update(b"stack_trace=symbolic\0");
    }
    // The exception model changes the code (`optimize/tables.rs`). The default
    // does not enter the key: the usual SDK keeps the usual key.
    if std::env::var("DARTFORGE_EXCEPTIONS").is_ok_and(|v| v == "tables") {
        h.update(b"exceptions=tables\0");
    }
    // ARC memory changes the code (the stores through the runtime, the activation at the
    // entry); the default does not enter. The version is that of the memory ABI.
    if std::env::var("DARTFORGE_MEMORY").is_ok_and(|v| v == "arc") {
        h.update(b"memoria=arc_v1\0");
    }
    // The published ARC preparation changes the bodies (tokens, cleanup); the
    // prepared program only links with this SDK.
    if crate::target::arc_prepared().unwrap_or(false) {
        h.update(b"arc=preparado_v1\0");
    }
    // The roots mode changes the code (`llvm/mod.rs`); the default does not enter.
    if std::env::var("DARTFORGE_ROOTS").is_ok_and(|v| v == "maps") {
        // `_v2`: with the unwind table on Unix (`with_uwtable`).
        h.update(b"roots=maps_v2\0");
        // The budget ceiling decides which functions come out with a map, and the
        // checking build adds slots to the frame.
        h.update(crate::target::maps_budget().to_string().as_bytes());
        h.update(if crate::target::check_roots() { b"check\0" } else { b"\0\0\0\0\0\0" });
    }
    // The test sabotages change the code (`target::sabotage`).
    if let Ok(s) = std::env::var("DARTFORGE_SABOTAGE")
        && !s.is_empty()
    {
        h.update(b"sabotage=");
        h.update(s.as_bytes());
        h.update(b"\0");
    }
    // The check of the effects table instruments the calls to the externs.
    if crate::llvm::externs::check_effects() {
        h.update(b"effects=check\0");
    }
    // The measurement switches of the emitter (`DARTFORGE_NO_…`, which turn off an
    // optimization to compare before and after) change the code.
    let mut keys: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("DARTFORGE_NO_"))
        .collect();
    keys.sort();
    for (k, v) in keys {
        h.update(format!("{k}={v}\0").as_bytes());
    }
        // The comparison link (`DARTFORGE_LINK_WITH_CLANG`) gives another DLL.
    h.update(if crate::driver::link_with_clang() { b"clang" } else { b"lld" });
    h.update(b"\0");
    h.update(clang_id.as_bytes());
    for a in args {
        h.update(a.as_bytes());
        h.update(b"\0");
    }
    for a in &files {
        let rel = a
            .strip_prefix(lib_dir)
            .or_else(|_| a.strip_prefix(&overlay))
            .unwrap_or(a)
            .to_string_lossy()
            .replace('\\', "/");
        h.update(rel.as_bytes());
        h.update(b"\0");
        h.update(&std::fs::read(a).unwrap_or_default());
        h.update(b"\0");
    }
    h.finalize().to_hex()[..32].to_string()
}

/// The generation of the SDK modules in the requested profile.
///
/// Development: `-O0` objects for the shared library (position-independent
/// code outside Windows). Production: `-O2` bitcode for the
/// cross-module optimization in the link of the executable
/// (docs/PESQUISA-LLVM-DART-AOT.md §5: development in separate modules,
/// production with LTO).
///
/// ```
/// use dartforge_emit_native::generator::Format;
/// use dartforge_emit_native::sdk_module::{SdkProfile, sdk_generation};
/// assert_eq!(sdk_generation(SdkProfile::Production).format, Format::Bitcode);
/// assert!(!sdk_generation(SdkProfile::Development).optimize);
/// ```
pub fn sdk_generation(profile: SdkProfile) -> Generation {
    match profile {
        SdkProfile::Development => Generation { optimize: false, format: Format::Object, shared: true, cpu: None, prelink: 2 },
        SdkProfile::Production => Generation { optimize: true, format: Format::Bitcode, shared: false, cpu: None, prelink: 2 },
    }
}

/// The profile in which the source SDK is compiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkProfile {
    /// `-O0` objects and the DLL (runtime + SDK) that the executable imports.
    Development,
    /// ThinLTO `-O2` bitcode, statically linked into the executable.
    Production,
}

/// The source SDK DLL (development profile) for the installed SDK and
/// the usual Clang: it is what the JIT loads (`crates/jit`, `DARTFORGE_SDK_DLL`).
pub fn source_sdk_dll() -> Result<PathBuf, String> {
    let dir = crate::dart_sdk()?;
    let clang = crate::driver::NativeDriverOptions::default().clang;
    Ok(compiled_sdk(&dir, &clang)?.dll)
}

/// The source SDK objects: from the cache (`<cache native>/sdk/<key>/`), or
/// compiled now — once per content, the libraries in parallel.
pub fn compiled_sdk(lib_dir: &Path, clang: &Path) -> Result<CompiledSdk, String> {
    compiled_sdk_in_profile(lib_dir, clang, SdkProfile::Development)
}

/// [`compiled_sdk`] in the requested profile.
pub fn compiled_sdk_in_profile(lib_dir: &Path, clang: &Path, profile: SdkProfile) -> Result<CompiledSdk, String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // In production, the SDK bitcode comes out of Clang, with the ThinLTO summary
    // (like the parts of the program, `driver.rs`): from the embedded generator it
    // does not have the summary, and `lld` did the full LTO of the SDK in a single
    // module, in one thread, before the parts (docs/NATIVO-PRODUCAO-GRANDE.md).
    // `DARTFORGE_GENERATOR` chooses by hand. Without Clang (the distribution without the
    // system toolchain), the embedded generator.
    let generator = if profile == SdkProfile::Production && std::env::var_os("DARTFORGE_GENERATOR").is_none() {
        Generator::clang_if_any(clang)
    } else {
        Generator::choose(clang)
    };
    let generation = sdk_generation(profile);
    let key = sdk_key(lib_dir, &generator.identity()?, &[generation.description().as_str()]);
    let root = crate::cache::native_cache_dir().join("sdk");
    let dir = root.join(&key);
    let dll_name = format!("dfsdk_{}", &key[..16]);
    let dll_file = crate::target::shared_name(&dll_name);
    let ext_obj = crate::target::ext_object();
    let ready = |d: &Path, cold| CompiledSdk {
        objects: SOURCE_LIBRARIES.iter().map(|b| d.join(format!("{b}.{ext_obj}"))).collect(),
        // Both profiles: production prunes at link time (`pruning::assemble`), and
        // both prune the HIR of the program (`pruning::prune_hir`).
        summaries: SOURCE_LIBRARIES.iter().map(|b| d.join(format!("{b}.poda"))).collect(),
        dll: d.join(&dll_file),
        // On Windows it links against the import library; on ELF and on
        // Mach-O, against the shared library itself.
        import: if crate::target::target_os() == crate::target::Os::Windows {
            d.join(format!("{dll_name}.lib"))
        } else {
            d.join(&dll_file)
        },
        cold,
    };
    if dir.join("pronto").is_file() {
        // The mark records the last use (the cache pruning respects it).
        if let Ok(f) = std::fs::File::options().write(true).open(dir.join("pronto")) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        return Ok(ready(&dir, None));
    }
    let runtime_dll = crate::cache::RuntimeCache::for_dll()?;
    let t0 = std::time::Instant::now();
    let tmp = root.join(format!("{key}.tmp.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let libs = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn({
            let lib_dir = lib_dir.to_path_buf();
            move || emit_sdk_libraries(&lib_dir, profile == SdkProfile::Production)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "panic while emitting the source SDK".to_string())??;
    let mut exported = Vec::new();
    for lib in &libs {
        defined_symbols(&lib.ir, &mut exported);
    }
    exported.extend(
        dartforge_runtime::symbols::NAMES.iter().filter(|n| **n != "main").map(|n| n.to_string()),
    );
    let mut def = format!("LIBRARY {dll_name}.dll\nEXPORTS\n");
    for s in &exported {
        def.push_str(&format!("  {s}\n"));
    }
    std::fs::write(tmp.join("exportados.def"), def).map_err(|e| e.to_string())?;
    let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    let mut summary = String::new();
    for (b, lib) in SOURCE_LIBRARIES.iter().zip(&libs) {
        if keep_ir {
            std::fs::write(tmp.join(format!("{b}.ll")), &lib.ir).map_err(|e| e.to_string())?;
        }
        for (s, m) in &lib.rejected {
            summary.push_str(&format!("{}\t{s}\t{m}\n", lib.uri));
        }
        // The pruning summary: from the IR that becomes the bitcode (production) or the object
        // of the DLL (development).
        let pruning = crate::pruning::summarize(&lib.ir, &lib.uri, &lib.tables);
        std::fs::write(tmp.join(format!("{b}.poda")), pruning.to_text()).map_err(|e| e.to_string())?;
    }
    std::fs::write(tmp.join("recusados.tsv"), summary).map_err(|e| e.to_string())?;
    // Each library in a generator, at most 4 at the same time (the memory of
    // a large LLVM module is the measure, not the cores).
    let queue = std::sync::Mutex::new(SOURCE_LIBRARIES.iter().zip(&libs).collect::<Vec<_>>());
    let errors = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            // The stack of Clang (8 MB): recursive LLVM passes on a large
            // module overflow the default stack of a Rust thread.
            let r = std::thread::Builder::new().stack_size(64 << 20).spawn_scoped(s, || loop {
                let Some((b, lib)) = queue.lock().unwrap().pop() else { break };
                if let Err(e) = generator.generate(&lib.ir, generation, &tmp.join(format!("{b}.{ext_obj}"))) {
                    errors.lock().unwrap().push(format!("dart:{b}: {e}"));
                }
            });
            if let Err(e) = r {
                errors.lock().unwrap().push(format!("generator thread: {e}"));
            }
        }
    });
    if let Some(e) = errors.into_inner().unwrap().into_iter().next() {
        return Err(e);
    }
    // The DLL (development): the SDK objects and the runtime (the variant without
    // `main`). In production, the objects (bitcode) go directly into the
    // executable.
    if profile == SdkProfile::Production {
        // Nothing to link: the bitcode enters the link of each executable.
    } else if crate::target::target_os() == crate::target::Os::Linux {
        // `ld.lld` directly with the link sysroot (`linker.rs`).
        let exported: Vec<String> =
            dartforge_runtime::symbols::NAMES.iter().filter(|n| **n != "main").map(|n| n.to_string()).collect();
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        crate::linker::link(
            &crate::linker::ld_lld(clang),
            crate::linker::SysrootLinux::locate(clang)?,
            &crate::linker::Linkage {
                product: crate::linker::Product::Shared { soname: &dll_file, exported: &exported },
                entries,
                origin_rpath: false,
                lto: false,
                cpu: None,
                prune: false,
                keep_debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("linking the source SDK DLL failed: {e}"))?;
    } else if crate::target::target_os() == crate::target::Os::Windows && crate::driver::link_with_clang() {
        // The link from before N15 (the Clang driver, `link.exe` and the
        // Visual C++ CRT), only for comparison (`DARTFORGE_LINK_WITH_CLANG`).
        let exit = std::process::Command::new(clang)
            .current_dir(&tmp)
            .arg("-shared")
            .args(SOURCE_LIBRARIES.iter().map(|b| format!("{b}.{ext_obj}")))
            .arg(&runtime_dll.lib_path)
            .arg("-Wl,/DEF:exportados.def")
            .args(crate::target::link_arguments())
            .arg("-o")
            .arg(&dll_file)
            .output()
            .map_err(|e| format!("Clang: {e}"))?;
        if !exit.status.success() {
            let text = String::from_utf8_lossy(&exit.stderr);
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
            return Err(format!("linking the source SDK DLL failed ({}):\n{}", exit.status, lines.join("\n")));
        }
    } else if crate::target::target_os() == crate::target::Os::Windows {
        // `lld-link` directly, with the import libraries and dartforge's minimal
        // CRT (`linker_windows.rs`); the `.def` exports the
        // runtime and the SDK, and the import library comes out next to the DLL.
        use crate::linker_windows as lw;
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        lw::link(
            &lw::lld_link(clang),
            lw::SysrootWindows::locate(clang)?,
            &lw::Linkage {
                product: lw::Product::Dll { def: &tmp.join("exportados.def") },
                entries,
                lto: false,
                cpu: None,
                prune: false,
                debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("linking the source SDK DLL failed: {e}"))?;
    } else {
        // `ld64.lld` directly with the `.tbd`s of the sysroot (`linker_macos.rs`):
        // each runtime name enters as undefined, so that the linker brings
        // from the `staticlib` also what the SDK does not use (the program does).
        use crate::linker_macos as lm;
        let exported: Vec<String> =
            dartforge_runtime::symbols::NAMES.iter().filter(|n| **n != "main").map(|n| n.to_string()).collect();
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        let install_name = format!("@rpath/{dll_file}");
        lm::link(
            &lm::ld64_lld(clang),
            lm::SysrootMacos::locate()?,
            &lm::Linkage {
                product: lm::Product::Dynamic { install_name: &install_name, exported: &exported },
                entries,
                executable_rpath: false,
                lto: false,
                cpu: None,
                prune: false,
                keep_debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("linking the source SDK DLL failed: {e}"))?;
    }
    std::fs::write(tmp.join("pronto"), b"").map_err(|e| e.to_string())?;
    if std::fs::rename(&tmp, &dir).is_err() {
        // Another process finished first: its result stays.
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join("pronto").is_file() {
            return Err(format!("could not install the compiled SDK in {}", dir.display()));
        }
    }
    prune_old_sdks(&root, &dir);
    Ok(ready(&dir, Some(t0.elapsed())))
}

/// Removes from the cache the compiled SDKs of previous dartforge versions
/// (each one exceeds 1 GB): the four most recent ones stay (the development
/// and production profiles of this version and of the previous one), and none
/// used in the last hour (the `ready` mark keeps the last use) — another
/// process may be linking against it. A
/// removal that fails (the library open on Windows) is left for the next time.
fn prune_old_sdks(root: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p != current && p.join("pronto").is_file())
        .filter_map(|p| Some((std::fs::metadata(p.join("pronto")).ok()?.modified().ok()?, p)))
        .collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    let hour = std::time::Duration::from_secs(3600);
    for (when, dir) in dirs.into_iter().skip(3) {
        if when.elapsed().is_ok_and(|d| d > hour) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// The result of the lowering of a source SDK member.
#[derive(Debug, Clone)]
pub struct SdkMember {
    pub library: String,
    pub symbol: String,
    /// Lowering diagnostics (empty: it lowered), or the panic.
    pub diagnostics: Vec<String>,
}

/// P5c measurement: lowers each function of the [`SOURCE_LIBRARIES`] (open
/// world) and returns, per member, the lowering diagnostics.
pub fn measure_sdk_lowering(lib_dir: &Path) -> Result<Vec<SdkMember>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-lower-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;
    let mut interner = Interner::new();
    let (program, load_diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = load_diags.first() {
        return Err(format!("loading the SDK with the overlay: {}", d.message));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| l.uri.strip_prefix("dart:").is_some_and(|n| SOURCE_LIBRARIES.contains(&n)))
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (bodies, _) =
        dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &libs);
    let ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
    let mut exit = Vec::new();
    for f in 0..program.functions.len() {
        if !libs.contains(&program.functions[f].library) {
            continue;
        }
        let symbol = crate::lower::symbol_of(&ctx, f);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = crate::hir::Module::new();
            crate::lower::lower_function(&ctx, &mut m, f);
            m.errors
        }));
        let diagnostics = match r {
            Ok(e) => e,
            Err(p) => {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                vec![format!("panic: {msg}")]
            }
        };
        exit.push(SdkMember {
            library: program.library(program.functions[f].library).uri.clone(),
            symbol,
            diagnostics,
        });
    }
    Ok(exit)
}

/// The IR of an SDK library with `optsize` on every defined function (the
/// production profile): LLVM weighs size in the inlining,
/// unrolling and vectorization decisions. Measured on 2026-09-29: −453 KB in hello
/// world and −532 KB in the HTTP server, with no difference beyond noise in
/// `bench/performance/json.dart` and `bench/simd/bin/blend.dart`
/// (docs/PLANO-TAMANHO-DESEMPENHO.md §1). The SDK code that LTO
/// inlines into the program follows the attributes of the caller.
///
/// ```
/// use dartforge_emit_native::sdk_module::with_optsize;
/// let ir = "define i64 @f(i64 %x) comdat {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline {\n";
/// assert_eq!(
///     with_optsize(ir),
///     "define i64 @f(i64 %x) optsize comdat {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline optsize {\n"
/// );
/// ```
pub fn with_optsize(ir: &str) -> String {
    with_attribute(ir, " optsize")
}

/// The IR with `uwtable` on every defined function: with exceptions by tables on
/// Itanium targets (ELF, Mach-O) the unwinder only crosses a frame that
/// has an unwind table, and LLVM only emits it for the marked function
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.11). On x64 COFF every
/// function already has its own.
pub fn with_uwtable(ir: &str) -> String {
    with_attribute(ir, " uwtable")
}

/// The IR with `nounwind` on every defined function that does not have it yet: with
/// exceptions by check with the shadow stack (the default), nothing unwinds a
/// frame of Dart code — the exception is a flag in the context, the stack trace
/// comes from the shadow stack and the production runtime aborts on panic. Without the
/// attribute, LLVM emits the CFI of every non-leaf function and, on
/// Mach-O arm64 without a frame pointer, that becomes DWARF in `__eh_frame`
/// (1.4 MB of `new_sali/backend`). Exceptions by tables
/// ([`with_uwtable`]) and roots by maps (`_Unwind_Backtrace`)
/// cross the frames and do not go through here.
///
/// ```
/// use dartforge_emit_native::sdk_module::with_nounwind;
/// let ir = "define i64 @f(i64 %x) {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline nounwind {\n";
/// assert_eq!(
///     with_nounwind(ir),
///     "define i64 @f(i64 %x) nounwind {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline nounwind {\n"
/// );
/// ```
pub fn with_nounwind(ir: &str) -> String {
    with_absent_attribute(ir, " nounwind")
}

/// How Dart code finds the thread context in [`with_context_in_tls`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextInTls {
    /// The helper copied into each function (`alwaysinline`): performance.
    Inline,
    /// LLVM decides.
    Free,
    /// A call (`noinline`): size, in the large program.
    PerCall,
}

/// The IR with the thread context (`dartforge_context`, a Rust function with a
/// frame, the `thread_local!` variable and the stack limit check)
/// kept in a `thread_local` variable of the module itself: the prologue of each function reads the variable (on Mach-O, the
/// `tlv_get_addr`, which does not touch the other registers) and only calls the
/// runtime the first time on each thread. The address of the context of a
/// thread does not change while it lives (`runtime/src/gc_roots.rs`). Only in
/// AOT: the JIT has no `thread_local` in its modules. The `fib(32)` of
/// `bench/comparison` spent 1/3 of the time in the runtime function.
///
/// ```
/// use dartforge_emit_native::sdk_module::{with_context_in_tls, ContextInTls};
/// let ir = "define i64 @f() {\n  %ctx = call ptr @dartforge_context()\n  ret i64 0\n}\n";
/// let new_ir = with_context_in_tls(ir, ContextInTls::Inline, "df.ctx.tls");
/// assert!(new_ir.contains("%ctx = call ptr @df.context()") && new_ir.contains("@df.ctx.tls = internal thread_local global ptr null"));
/// let without = "define void @g() {\n  ret void\n}\n";
/// assert_eq!(with_context_in_tls(without, ContextInTls::Inline, "df.ctx.tls"), without);
/// ```
///
/// `variable` is the name of the variable: the program in parts promotes it to
/// external (`partition.rs`), and its name cannot be that of the internal one of an
/// SDK module (like `@df.area_id.program`).
pub fn with_context_in_tls(ir: &str, mode: ContextInTls, variable: &str) -> String {
    const CALL: &str = "call ptr @dartforge_context()";
    if !ir.contains(CALL) {
        return ir.to_string();
    }
    let attribute = match mode {
        ContextInTls::Inline => " alwaysinline",
        ContextInTls::Free => "",
        ContextInTls::PerCall => " noinline",
    };
    let mut s = ir.replace(CALL, "call ptr @df.context()");
    s.push_str(&format!(
        "\n@{variable} = internal thread_local global ptr null, align 8\n\
         define internal ptr @df.context() nounwind{attribute} {{\n\
         \x20 %p = call ptr @llvm.threadlocal.address.p0(ptr @{variable})\n\
         \x20 %c = load ptr, ptr %p, align 8\n\
         \x20 %z = icmp eq ptr %c, null\n\
         \x20 br i1 %z, label %lento, label %pronto\n\
         lento:\n\
         \x20 %n = {CALL}\n\
         \x20 store ptr %n, ptr %p, align 8\n\
         \x20 ret ptr %n\n\
         pronto:\n\
         \x20 ret ptr %c\n\
         }}\n"
    ));
    if !s.contains("declare ptr @llvm.threadlocal.address.p0(ptr") {
        s.push_str("declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)\n");
    }
    s
}

/// Adds a function attribute (with the leading space) to each
/// `define` line.
fn with_attribute(ir: &str, attribute: &str) -> String {
    with_attribute_if(ir, attribute, false)
}

/// [`with_attribute`] only on the `define` lines that do not have the attribute yet.
fn with_absent_attribute(ir: &str, attribute: &str) -> String {
    with_attribute_if(ir, attribute, true)
}

fn with_attribute_if(ir: &str, attribute: &str, only_absent: bool) -> String {
    let mut s = String::with_capacity(ir.len() + ir.len() / 64);
    for l in ir.split_inclusive('\n') {
        if only_absent && l.starts_with("define ") && l.contains(attribute) {
            s.push_str(l);
            continue;
        }
        // The attributes come after the parameters and before the `comdat` (or the
        // brace that opens the body).
        // The `gc "…"` (roots by maps), the personality (`personality ptr
        // @…`, of exceptions by tables) and the metadata attachments (`!dbg
        // !3`, with `--debug`) come after the attributes.
        let point = l
            .starts_with("define ")
            .then(|| {
                l.find(" comdat")
                    .or_else(|| l.find(" gc \""))
                    .or_else(|| l.find(" personality "))
                    .or_else(|| l.find(" !"))
                    .or_else(|| l.rfind(" {"))
            })
            .flatten();
        match point {
            Some(i) => {
                s.push_str(&l[..i]);
                s.push_str(attribute);
                s.push_str(&l[i..]);
            }
            None => s.push_str(l),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With debug info the attribute goes before the `!dbg` attachment,
    /// which LLVM only accepts after the attributes.
    #[test]
    fn attribute_before_debug_attachment() {
        let ir = "define i64 @f(i64 %x) !dbg !3 {\n  ret i64 %x\n}\n";
        assert_eq!(with_nounwind(ir), "define i64 @f(i64 %x) nounwind !dbg !3 {\n  ret i64 %x\n}\n");
    }

    /// The overlay exists, replaces the four files and points to files
    /// that exist.
    #[test]
    fn overlay_loads() {
        crate::sdk_tests();
        let sdk = load_native_sdk(crate::sdk_tests()).unwrap();
        // 26 with `mirrors_patch.dart` (the `reflectClass` of the builders
        // runner, corpus/native/45); 29 with `double.dart`, `integers.dart`
        // and `convert_patch.dart` (JSON, docs/NATIVO-PLANO.md); 30 with
        // `convert/json.dart` (writing in UTF-8, §13.2).
        assert_eq!(sdk.replacements.len(), 30);
        for b in SOURCE_LIBRARIES {
            assert!(sdk.library(b).is_some(), "dart:{b} outside the layout");
        }
    }

    /// The 5a measurement (docs/NATIVO-PLANO.md §7.4): prints, per library,
    /// the functions and the inference diagnostics of the bodies. Slow (infers the
    /// whole SDK); runs separately:
    /// `cargo test -p dartforge-emit-native measure_source_library_inference -- --ignored --nocapture`.
    #[test]
    #[ignore = "measurement; runs separately"]
    fn measure_source_library_inference() {
        crate::sdk_tests();
        let table = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| measure_sdk_inference(crate::sdk_tests()).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut total = 0;
        for b in &table {
            println!("{:<18} functions {:>5}  diagnostics {:>5}", b.library, b.functions, b.diagnostics.len());
            total += b.diagnostics.len();
            let mut by_code: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for d in &b.diagnostics {
                let key: String = d.split(':').next().unwrap_or(d).chars().take(60).collect();
                *by_code.entry(key).or_default() += 1;
            }
            let mut v: Vec<_> = by_code.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (c, n) in v.iter().take(8) {
                println!("    {n:>5}  {c}");
            }
        }
        println!("total diagnostics: {total}");
    }

    /// P5c: emits the source SDK modules and shows, per library, the
    /// IR size and the rejected members (by reason). With
    /// `DARTFORGE_SDK_COMPILE=1`, compiles and installs the objects in the cache.
    /// `cargo test -p dartforge-emit-native emit_the_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "measurement; runs separately"]
    fn emit_the_sdk() {
        crate::sdk_tests();
        let t = std::time::Instant::now();
        let libs = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| emit_sdk_libraries(crate::sdk_tests(), false).unwrap())
            .unwrap()
            .join()
            .unwrap();
        println!("emission: {:?}", t.elapsed());
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        let mut total = 0;
        for l in &libs {
            println!("{:<18} IR {:>9} bytes  rejected {:>5}", l.uri, l.ir.len(), l.rejected.len());
            total += l.rejected.len();
            for (_, m) in &l.rejected {
                *reasons.entry(m.chars().take(100).collect()).or_default() += 1;
            }
        }
        println!("rejected: {total}");
        let mut v: Vec<_> = reasons.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (m, n) in v.iter().take(60) {
            println!("{n:>5}  {m}");
        }
        if let Ok(dir) = std::env::var("DARTFORGE_SDK_IR_DIR") {
            for (b, l) in SOURCE_LIBRARIES.iter().zip(&libs) {
                std::fs::write(Path::new(&dir).join(format!("{b}.ll")), &l.ir).unwrap();
                let r: String = l.rejected.iter().map(|(s, m)| format!("{s}\t{m}\n")).collect();
                std::fs::write(Path::new(&dir).join(format!("{b}.recusados.tsv")), r).unwrap();
            }
        }
        if std::env::var("DARTFORGE_SDK_COMPILE").is_ok_and(|v| v == "1") {
            let clang = crate::driver::NativeDriverOptions::default().clang;
            let s = compiled_sdk(crate::sdk_tests(), &clang).unwrap();
            println!("dll: {:?} (frio: {:?})", s.dll, s.cold);
        }
    }

    /// Compiles and runs `DARTFORGE_PROGRAM` with the source SDK (P5c):
    /// `cargo test -p dartforge-emit-native compile_with_source_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "manual; runs separately"]
    fn compile_with_source_sdk() {
        let Ok(p) = std::env::var("DARTFORGE_PROGRAM") else { return };
        let entry = PathBuf::from(p);
        let exit = entry.with_extension("exe");
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let optimize = std::env::var("DARTFORGE_OPTIMIZE").is_ok_and(|v| v == "1");
                let options = crate::CompileOptions { sdk: Some(crate::sdk_tests()), packages: None, timings: true, optimize: optimize, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
                crate::compile(&entry, &exit, &options).map(|_| exit)
            })
            .unwrap()
            .join()
            .unwrap();
        match r {
            Ok(exe) => {
                let o = std::process::Command::new(&exe).output().unwrap();
                println!("--- code {:?}\n--- stdout\n{}--- stderr\n{}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            }
            Err(e) => println!("ERROR: {e}"),
        }
    }

    /// Production profile with the source SDK: ONE self-contained executable. The
    /// `.exe` is copied alone into an empty folder and runs without any DartForge
    /// DLL (docs/NATIVO-PLANO.md §7.9).
    #[test]
    #[ignore = "compiles the source SDK (slow when cold); runs in CI"]
    fn production_is_a_self_contained_executable() {
        // The installed SDK (in CI, that of `DART_HOME`); without it, the test is only
        // skipped outside CI — in CI, absence is a failure, not an empty success.
        let sdk_dir = crate::sdk_tests().to_path_buf();
        if !sdk_dir.join("libraries.json").is_file() {
            assert!(std::env::var_os("CI").is_none(), "Dart SDK missing in CI ({})", sdk_dir.display());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() {\n  print('oi');\n  print([1, 2, 3].map((x) => x * 2).toList());\n  print({'a': 1});\n}\n").unwrap();
        let exe = dir.path().join(crate::target::executable_name("prog"));
        let (e2, x2) = (entry.clone(), exe.clone());
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let options = crate::CompileOptions { sdk: Some(&sdk_dir), packages: None, timings: false, optimize: true, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
                crate::compile_with(&e2, &x2, &options)
            })
            .unwrap()
            .join()
            .unwrap();
        r.expect("compile in production");
        let alone = tempfile::tempdir().unwrap();
        let copy_inst = alone.path().join(crate::target::executable_name("prog"));
        std::fs::copy(&exe, &copy_inst).unwrap();
        let only_one: Vec<_> = std::fs::read_dir(alone.path()).unwrap().collect();
        assert_eq!(only_one.len(), 1, "the folder has only the executable");
        let o = std::process::Command::new(&copy_inst).current_dir(alone.path()).output().unwrap();
        assert!(o.status.success(), "code {:?}; stderr: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"), "oi\n[2, 4, 6]\n{a: 1}\n");
    }

    /// The code absence test (docs/NATIVO-PODA-DE-TABELAS.md §4, scriptc's
    /// `deadstrip.test.ts`): `corpus/native/81_poda_de_tabelas.dart`
    /// has an instantiated class with members that nothing uses. In the production
    /// executable (the link map) no symbol of them remains — neither the
    /// body nor the `$c`/`$tc`/`$g`/`$s`/`$tearm` entries —, the entries of the
    /// used ones remain, and the output is the VM's.
    #[test]
    #[ignore = "compiles the source SDK (slow when cold); runs in CI"]
    fn pruning_removes_unused_members() {
        let sdk_dir = crate::sdk_tests().to_path_buf();
        if !sdk_dir.join("libraries.json").is_file() {
            assert!(std::env::var_os("CI").is_none(), "Dart SDK missing in CI ({})", sdk_dir.display());
            return;
        }
        let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/native/81_poda_de_tabelas.dart");
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(crate::target::executable_name("poda"));
        crate::linker::request_link_maps();
        let (e2, x2) = (entry.clone(), exe.clone());
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {
                let options = crate::CompileOptions { sdk: Some(&sdk_dir), packages: None, timings: false, optimize: true, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
                crate::compile_with(&e2, &x2, &options)
            })
            .unwrap()
            .join()
            .unwrap()
            .expect("compile in production");
        let map = std::fs::read_to_string(exe.with_extension("map")).expect("the link map");
        let remained: Vec<&str> = map.lines().filter(|l| l.contains("naoUsadoPoda")).collect();
        assert!(remained.is_empty(), "unused members in the executable:\n{}", remained.join("\n"));
        // The entries of the used ones, one of each form (the setter `x=` has the
        // `=` escaped; the static tear-off is `$tearm`, the dynamic one goes
        // through `$g`).
        let used_set = [
            "Poda.usadoDinamicoPoda$c",
            "PodaFilha.usadoDinamicoPoda$c",
            "lidoPorGetterPoda$g",
            "gravadoPorSetterPoda$3d$s",
            "tearoffUsadoPoda$tearm",
            "tearoffDinamicoPoda$g",
            "genericoUsadoPoda$c",
        ];
        for used in used_set {
            assert!(map.contains(used), "{used} outside the link map");
        }
        let o = std::process::Command::new(&exe).output().unwrap();
        assert!(o.status.success(), "code {:?}; stderr: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"),
            "7\n13\n13\n7\n101\nt\nPoda(3, 9)\n4\n102\n13\n7\nnsm Symbol(\"qualquerCoisaPoda\") [1, 2] 1\nNoSuchMethodError\n"
        );
    }

    /// The P5c measurement: how many source SDK members the lowering lowers.
    /// `cargo test -p dartforge-emit-native measure_source_library_lowering -- --ignored --nocapture`.
    #[test]
    #[ignore = "measurement; runs separately"]
    fn measure_source_library_lowering() {
        crate::sdk_tests();
        // Without replacing the panic hook of the process: a silent hook
        // here (as it was) silenced the panic of ALL the tests that run together
        // — in the macOS job, `production_is_a_self_contained_executable` and
        // `pruning_removes_unused_members` failed without any message. The
        // lowering panics caught by `catch_unwind` only make noise.
        let members = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| measure_sdk_lowering(crate::sdk_tests()).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut by_lib: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
        let mut by_construct: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for m in &members {
            let e = by_lib.entry(&m.library).or_default();
            e.0 += 1;
            if m.diagnostics.is_empty() {
                e.1 += 1;
            }
            let mut seen_set = std::collections::BTreeSet::new();
            for d in &m.diagnostics {
                let d = d.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(d);
                let key: String = d.rsplit_once(" (").map_or(d, |(a, _)| a).chars().take(90).collect();
                let c = by_construct.entry(key.clone()).or_default();
                c.1 += 1;
                if seen_set.insert(key) {
                    c.0 += 1;
                }
            }
        }
        for (l, (n, ok)) in &by_lib {
            println!("{l:<20} membros {n:>5}  baixados {ok:>5}");
        }
        let mut v: Vec<_> = by_construct.into_iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        for (c, (members, occ)) in v.iter().take(80) {
            println!("{members:>5} {occ:>6}  {c}");
        }
        if let Ok(file) = std::env::var("DARTFORGE_MEASURE_OUTPUT") {
            let mut t = String::new();
            for m in &members {
                t.push_str(&format!("{}\t{}\t{}\n", m.library, m.symbol, m.diagnostics.join(" | ")));
            }
            std::fs::write(file, t).unwrap();
        }
    }
}
