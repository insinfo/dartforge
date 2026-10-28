//! DartForge's LLVM native compiler on the new track.

pub mod cache;
pub mod compact_stack_trace;
pub mod context;
pub mod contracts;
pub mod cross;
#[cfg(feature = "embedded-llvm")]
pub mod distributed_lto;
pub mod driver;
pub mod erasure;
mod finalizables;
pub mod gcmap;
pub mod generator;
pub mod hir;
pub mod js_interop;
pub mod linked_images;
pub mod linker;
pub mod linker_macos;
pub mod linker_wasm;
pub mod linker_windows;
pub mod llvm;
pub mod lower;
pub mod native_inputs;
pub mod native_world;
pub mod natives;
pub mod object_cache;
pub mod optimize;
pub mod partition;
pub mod pruning;
pub mod sdk_module;
pub mod source;
pub mod summary;
pub mod target;
pub mod wasm_frames;

use context::Context;
use dartforge_elements::Program;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Native compilation options.
pub struct CompileOptions<'a> {
    /// Explicit verification policy, independent of ambient environment state.
    pub contracts: contracts::ContractMode,
    pub sdk: Option<&'a Path>,
    pub packages: Option<&'a Path>,
    pub timings: bool,
    pub optimize: bool,
    /// Current language version (`--language-version`, docs/VERSOES-LINGUAGEM.md);
    /// `None` = the tool's (3.13).
    pub language_version: Option<dartforge_frontend::LanguageVersion>,
    /// Experiments requested explicitly for libraries at the current version.
    pub experiments: Vec<dartforge_frontend::Feature>,
    /// J05: line tables for the native debugger (`llvm/debug_info.rs`).
    pub debug: bool,
    /// Who produces the generated sources of the project (the build engine,
    /// `crates/build`) from the program loaded without them — the same
    /// contract as `compile-js` (`dartforge_emit_js::Generator`). `None`: the
    /// project does not use builders, or the caller does not enable them.
    pub generator: Option<Generator<'a>>,
    /// The target CPU of the generated code (`--cpu`, [`generator::Cpu`]); `None`, the
    /// baseline of the target (on x86-64, SSE2).
    pub cpu: Option<generator::Cpu>,
}

/// What the compiled Dart assumes about asserts and `dart.vm.product`,
/// independently of `optimize` (which decides the shape of the code) — the
/// two profiles of Dart itself:
///
/// * [`DartFlags::DEVELOPMENT`]: asserts on, `dart.vm.product` false — the
///   `dart run --enable-asserts` oracle of the differential harness, the JIT
///   and `run`; also what the API entry points without flags assume;
/// * [`DartFlags::PRODUCTION`]: asserts off, `dart.vm.product` true — what
///   `dart compile exe` produces; `aot --optimize` and `compile-native
///   --optimize` use it (`--enable-asserts` turns the asserts back on).
///
/// With the asserts off, `assert(c, m)` emits nothing: neither `c` nor `m` is
/// evaluated (`lower/statements.rs`). `bool.hasEnvironment("dart.vm.product")`
/// is true in both profiles, as in the VM. The flags are part of the key of
/// the compiled SDK (`sdk_module::sdk_key`): a program links only with an SDK
/// compiled with the same ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DartFlags {
    /// The `assert` statements and initializers are compiled and run.
    pub enable_asserts: bool,
    /// `const bool.fromEnvironment("dart.vm.product")`.
    pub product: bool,
}

impl DartFlags {
    /// Asserts on, `dart.vm.product` false (`dart run --enable-asserts`).
    pub const DEVELOPMENT: DartFlags = DartFlags {
        enable_asserts: true,
        product: false,
    };
    /// Asserts off, `dart.vm.product` true (`dart compile exe`).
    pub const PRODUCTION: DartFlags = DartFlags {
        enable_asserts: false,
        product: true,
    };

    /// The tag of the flags in the key of the compiled SDK.
    pub fn cache_tag(self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "asserts={};product={}",
            on(self.enable_asserts),
            on(self.product)
        )
    }
}

impl Default for DartFlags {
    fn default() -> Self {
        Self::DEVELOPMENT
    }
}

/// Veja [`CompileOptions::generator`].
pub type Generator<'a> = &'a (
        dyn Fn(
    &dartforge_elements::model::Program,
    &Interner,
) -> Result<std::sync::Arc<dartforge_elements::generated::Generation>, String>
            + Sync
    );

/// Time of each phase of the emission (everything before Clang).
#[derive(Debug, Clone, Copy, Default)]
pub struct EmissionTimes {
    pub frontend: Duration,
    /// The closed world of the program (`native_world.rs`).
    pub world: Duration,
    /// Live program functions in the closed world / all of them (0/0 without pruning).
    pub live_functions: (usize, usize),
    pub hir: Duration,
    pub llvm_ir: Duration,
}

/// The LLVM IR of a program, ready for Clang.
#[derive(Debug, Clone)]
pub struct EmittedIr {
    /// Contract provenance for this exact emission; never an implicit proof.
    pub contracts: contracts::ContractReport,
    pub text: String,
    /// SDK library directory selected for this emission and its linked SDK objects.
    pub sdk_lib: PathBuf,
    pub times: EmissionTimes,
    /// Bytes of `text` that are function bodies of the SDK (`dart:`); see
    /// `sdk_bytes`.
    pub sdk_byte_count: usize,
}

impl EmittedIr {
    /// The `--timings` lines of the emission, in the format of [`compile`].
    pub fn print_times(&self) {
        eprintln!("  Front-end: {:?}", self.times.frontend);
        let (live_set, all) = self.times.live_functions;
        eprintln!(
            "  World:     {:?} ({live_set} of {all} program functions live)",
            self.times.world
        );
        eprintln!("  HIR:       {:?}", self.times.hir);
        eprintln!("  LLVM IR:   {:?}", self.times.llvm_ir);
        let pct = if self.text.is_empty() {
            0.0
        } else {
            100.0 * self.sdk_byte_count as f64 / self.text.len() as f64
        };
        eprintln!(
            "  IR:        {} bytes, {} from the SDK ({pct:.1}%)",
            self.text.len(),
            self.sdk_byte_count
        );
    }
}

/// Start of every diagnostic of a construct that the lowering cannot lower
/// (`FnBuilder::unsupported`, N1). [`error_constructs`] is what reads it.
pub const UNSUPPORTED_PREFIX: &str = "not supported by the native backend: ";

/// [`pruning::prune_hir`] with the summaries of the SDK libraries of the profile (the
/// SDK comes out of the cache, or is compiled now, as the link would do right
/// after). `None` when the module does not use the source SDK.
fn prune_program_hir(
    module: &mut hir::Module,
    production: bool,
    sdk_lib: &Path,
    flags: DartFlags,
    timings: bool,
) -> Result<Option<pruning::HirPruning>, String> {
    module.imported_allocation_inventory = None;
    if module.sdk_library || module.sdk_records.is_empty() {
        return Ok(None);
    }
    // WebAssembly links the SDK statically in every profile.
    let profile = if production || crate::target::is_wasm() {
        sdk_module::SdkProfile::Production
    } else {
        sdk_module::SdkProfile::Development
    };
    let clang = driver::NativeDriverOptions::default().clang;
    // The same directory, the same Clang and the same flags as the link
    // (`driver::compile_and_link`): the same cache key.
    let sdk = sdk_module::compiled_sdk_for(sdk_lib, &clang, profile, flags)?;
    if !sdk.summaries.iter().all(|r| r.is_file()) {
        return Ok(None);
    }
    let summaries = pruning::read_summaries(&sdk.summaries)?;
    let refs: Vec<&pruning::Summary> = summaries.iter().map(|r| r.as_ref()).collect();
    module.arc_external_ancestry = hir::ArcExternalAncestry::from_summaries(&refs);
    let artifacts: Vec<_> = sdk
        .objects
        .iter()
        .zip(&sdk.summaries)
        .flat_map(|(object, summary)| {
            [
                object.file_name().unwrap().to_string_lossy().to_string(),
                summary.file_name().unwrap().to_string_lossy().to_string(),
            ]
        })
        .collect();
    if let Some(directory) = sdk.objects.first().and_then(|path| path.parent()) {
        module.imported_allocation_returns =
            Some(pruning::receiver_provenance::allocation_certificates::load(
                directory, &sdk.key, production, &artifacts,
            ));
        if production && std::env::var("DARTFORGE_CLOSED_CLASS_REACHABILITY").as_deref() == Ok("1")
        {
            let runtime = cache::RuntimeCache::for_production()?.lib_path;
            module.imported_allocation_inventory =
                pruning::allocation_inventory::imports::Inventory::load(
                    directory, &sdk.key, production, &artifacts, &runtime,
                )
                .map(std::sync::Arc::new);
            if timings {
                match &module.imported_allocation_inventory {
                    Some(inventory) => {
                        let (unique, known, pure) = inventory.counts();
                        eprintln!(
                            "  SDK inventory: {unique} unique definitions, {known} covered allocation closures, {pure} exposure-free empty closures"
                        );
                        let (strings, cids) = inventory.image_string_counts();
                        eprintln!(
                            "  SDK static strings: {strings} checked literal definitions, CIDs {cids:?}; linked image ranges remain unverified"
                        );
                        let source_images = inventory.clean_source_image_count();
                        eprintln!(
                            "  SDK source image census: {source_images} modules without opaque source contributions; linker inputs remain unverified"
                        );
                    }
                    None => eprintln!(
                        "  SDK inventory unavailable; imported class coverage remains open"
                    ),
                }
            }
        }
    }
    Ok(Some(pruning::prune_hir(module, &refs)))
}

/// The SDK version in `root` as the VM writes it in `Platform.version`
/// (`VM/runtime/vm/version_in.cc:33`, `{{VERSION_STR}} ({{CHANNEL}})
/// ({{COMMIT_TIME}})`): `3.6.2 (stable) (Wed Jan 29 01:20:39 2025 -0800)`,
/// without the ` on "<system>"` that the runtime appends. Only `dart` itself knows the commit
/// date: the first compilation asks it (`dart --version`) and
/// keeps it in the native cache, by version; without `dart`, only the number of the
/// `version` file (`sqlite3` prints `Platform.version`).
fn sdk_version(root: &Path) -> Option<String> {
    let number = std::fs::read_to_string(root.join("version"))
        .ok()?
        .trim()
        .to_string();
    let saved = cache::native_cache_dir().join(format!("sdk_version_{number}.txt"));
    if let Ok(v) = std::fs::read_to_string(&saved)
        && v.starts_with(&number)
    {
        return Some(v.trim().to_string());
    }
    let dart = root
        .join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let complete = std::process::Command::new(&dart)
        .arg("--version")
        .output()
        .ok()
        .and_then(|s| {
            // `Dart SDK version: 3.6.2 (stable) (…) on "windows_x64"` (on
            // standard output since 2.15; before, on standard error).
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&s.stdout),
                String::from_utf8_lossy(&s.stderr)
            );
            let rest = text.split("Dart SDK version: ").nth(1)?;
            let v = rest.split(" on \"").next()?.trim().to_string();
            v.starts_with(&number).then_some(v)
        });
    match complete {
        Some(v) => {
            let _ = std::fs::create_dir_all(cache::native_cache_dir());
            let _ = std::fs::write(&saved, &v);
            Some(v)
        }
        None => Some(number),
    }
}

/// From how many functions in the HIR module of the program production treats it
/// as large (`compile`, fields and helpers by call): new_sali/backend
/// has ~200 thousand; the corpus programs and the benchmarks, a
/// few thousand.
pub const LARGE_PROGRAM_FUNCTIONS: usize = 50_000;

/// The error of a module with lowering diagnostics.
///
/// The first line is `erro of compilação: ` and the first diagnostic **without the
/// position**: it is the grouping key of the harness, and the same construct in
/// different programs has to fall into the same group. Then comes one line per
/// diagnostic, all of them, with the position and indented by two spaces.
fn compile_error(errors: &[String]) -> String {
    let first_one = &errors[0];
    let summary = first_one
        .rsplit_once(" (")
        .map_or(first_one.as_str(), |(a, _)| a);
    let mut text = format!("compile error: {summary}");
    for e in errors {
        text.push_str("\n  ");
        text.push_str(e);
    }
    text
}

/// The unsupported constructs of an error of [`emit_ir`]/[`compile`], or
/// of a text that contains it (the stderr the harness builds with it): one per
/// diagnostic, in order, without the position. Empty when the error is not about a
/// construct (loading, panic, HIR verifier).
pub fn error_constructs(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let d = l.strip_prefix("  ")?.strip_prefix(UNSUPPORTED_PREFIX)?;
            Some(d.rsplit_once(" (").map_or(d, |(a, _)| a).to_string())
        })
        .collect()
}

/// Loads, analyzes and lowers a program down to LLVM IR, without Clang or linking.
///
/// It is the part of [`compile`] that is ours: the determinism test of the harness
/// compares only this (`dartforge-differential determinism --native`), and
/// `compile-native --emit-ir` writes this. Loading diagnostics go into the
/// error message, and not to stderr, so that they do not interleave when several
/// emissions run in the same process. The first line is the first
/// diagnostic, because it is what the harness report shows and groups — a
/// count ("1 erro(s)") put different causes into a single group.
///
/// An unsupported construct is also an `Err`, with **all** the diagnostics of the
/// module (format in `compile_error`); no IR is emitted.
pub fn emit_ir(entry: &Path, options: &CompileOptions) -> Result<EmittedIr, String> {
    emit_ir_with(entry, options)
}

/// The `lib/` of the Dart SDK: what [`SdkLayout::discover`] finds (the environment
/// variable, the distribution, the `dart` on `PATH`), or the error that says how
/// to point to it.
pub fn dart_sdk() -> Result<PathBuf, String> {
    SdkLayout::discover().ok_or_else(|| {
        "Dart SDK not found: the dartforge distribution carries it in lib/dart-sdk; \
         in a development tree, set DARTFORGE_SDK_LIB (the lib/ of the SDK) or DART_SDK"
            .to_string()
    })
}

/// [`emit_ir`] (the name stays for its existing callers; the SDK always comes from
/// source, docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
///
/// Literals as static objects (§2.11) only in production (`optimize`), which
/// is a single image. In development the SDK lives in the DLL, with its
/// statics: a static literal in the executable would be an object other than the equal one of the
/// DLL (`identical('a'.substring(1, 1), '')` gave false). Without statics, the
/// literal of the program is interned in the heap and canonicalized against the
/// statics of the DLL, as in the JIT.
pub fn emit_ir_with(entry: &Path, options: &CompileOptions) -> Result<EmittedIr, String> {
    emit_ir_with_flags(entry, options, DartFlags::DEVELOPMENT)
}

/// [`emit_ir_with`] with explicit [`DartFlags`] (asserts, `dart.vm.product`):
/// the CLI passes [`DartFlags::PRODUCTION`] with `--optimize`; the
/// differential harness keeps [`DartFlags::DEVELOPMENT`], the oracle's.
pub fn emit_ir_with_flags(
    entry: &Path,
    options: &CompileOptions,
    flags: DartFlags,
) -> Result<EmittedIr, String> {
    emit_ir_internal(entry, options, None, None, options.optimize, true, flags)
}

/// [`emit_ir`] of a new generation of a running program (the JIT hot
/// reload): `previous_ir` is the IR of the live generation, from which the layout
/// of the global area that the new generation extends comes ([`ir_area`]).
pub fn emit_reloadable_ir(
    entry: &Path,
    options: &CompileOptions,
    previous_ir: Option<&str>,
) -> Result<EmittedIr, String> {
    let area = previous_ir.and_then(ir_area);
    // J03: the same class with the same id as in the live generation.
    let ids = previous_ir.map(context::ir_ids);
    // The JIT frees the memory of a generation (J02): no static objects in the
    // program modules (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
    emit_ir_internal(
        entry,
        options,
        area,
        ids,
        false,
        false,
        DartFlags::DEVELOPMENT,
    )
}

/// The names (hashes) of the slots of the global area of the program in IR `ir`:
/// the `@df.area = … [key, n, name_0…]` that the emitter writes.
pub fn ir_area(ir: &str) -> Option<Vec<i64>> {
    let line = ir.lines().find(|l| l.starts_with("@df.area = "))?;
    let list = &line[line.rfind('[')? + 1..line.rfind(']')?];
    let values: Vec<i64> = list
        .split(',')
        .map(|v| {
            v.trim()
                .strip_prefix("i64 ")
                .and_then(|n| n.trim().parse().ok())
        })
        .collect::<Option<_>>()?;
    let n = usize::try_from(*values.get(1)?).ok()?;
    (values.len() == n + 2).then(|| values[2..].to_vec())
}

fn emit_ir_internal(
    entry: &Path,
    options: &CompileOptions,
    previous_area: Option<Vec<i64>>,
    previous_ids: Option<std::collections::HashMap<(String, String), u32>>,
    static_objects: bool,
    prune: bool,
    flags: DartFlags,
) -> Result<EmittedIr, String> {
    // The exception model (`target::table_exceptions`). The JIT (the only one
    // that emits without pruning) does not have the tables: its code does not register
    // the unwind tables with the system.
    let table_exceptions = target::table_exceptions()?;
    if table_exceptions && !prune {
        return Err("exceptions by tables (DARTFORGE_EXCEPTIONS=tables) do not exist in the JIT: use compile-native".to_string());
    }
    // The roots mode (`target::roots_by_maps`). In the JIT (the only one that emits
    // without pruning) the map of each object is found and registered by the
    // memory manager of the session (`crates/jit`, §3.8).
    let roots_by_maps = target::roots_by_maps()?;
    // The memory policy (`target::arc_memory`), in AOT and in the JIT: the
    // instance migration of reload counts through the snapshot
    // (`Heap::trocar_campos`, docs/ARC-IMPLEMENTACAO.md).
    let arc_memory = target::arc_memory()?;
    let maps_in_jit = roots_by_maps && !prune;
    // The symbolic stack trace (§13.14). In the JIT the table of each object is
    // registered by the memory manager of the session (`crates/jit`), and the
    // module comes out without the linker symbols and without the registration call.
    let symbolic_stack_trace = target::symbolic_stack_trace()?;
    let stack_trace_in_jit = symbolic_stack_trace && !prune;
    // 1. Loading and inference (front end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => dart_sdk()?,
    };

    // The `vm` section with the `native_sdk/` overlay (P5a): the libraries of
    // `SOURCE_LIBRARIES` are its own, linked as cached objects
    // (`sdk_module::compiled_sdk`). `mut`: the
    // current language version may come from `--language-version` (modern
    // Dart, P2).
    let mut sdk = sdk_module::load_native_sdk(&sdk_dir)
        .map_err(|e| format!("failed to load the VM SDK: {e}"))?;
    if let Some(v) = options.language_version {
        sdk.current_version = v;
    }
    for experiment in &options.experiments {
        if !sdk.experiments.contains(experiment) {
            sdk.experiments.push(*experiment);
        }
    }

    let mut interner = Interner::new();
    // With the build engine (DF-BUILD-009): a tolerant load without the
    // generated files serves as `BuildStep.solve`; the generation comes back in memory and
    // the real load reads it before the disk.
    let (program, elements_diags) = match options.generator {
        Some(generate) => {
            let mut resolution_names = Interner::new();
            let (resolution, _) =
                load_lenient(entry, &sdk, options.packages, &mut resolution_names);
            let generation = generate(&resolution, &resolution_names)?;
            dartforge_elements::load::load_lenient_generated(
                entry,
                &sdk,
                options.packages,
                &mut interner,
                None,
                None,
                Some(generation),
            )
        }
        None => load_lenient(entry, &sdk, options.packages, &mut interner),
    };
    if let Some((first_one, rest)) = elements_diags.split_first() {
        let mut msg = format!("error loading the program: {first_one}");
        for d in rest {
            msg.push_str(&format!("\nerror: {d}"));
        }
        return Err(msg);
    }
    // The SDK libraries compiled from source (P5c) are separate modules, in the
    // cache: the program does not lower them again; a used `dart:async` turns on the event
    // loop.
    let uses_dart_async = !source::source_libraries(&program, &interner).is_empty();
    let source_libraries: Vec<dartforge_elements::model::LibraryId> = Vec::new();

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) =
        dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    // Local declaration bindings are also needed by constant evaluation (a
    // later const initializer may refer to an earlier local constant).
    let user_libraries: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, lib)| !lib.is_sdk)
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (bodies, body_diags) = dartforge_types::infer_library_bodies_with_locals(
        &program,
        &interner,
        &mut table,
        &core,
        &mut outline,
        &user_libraries,
    );
    // Language errors of the 3.7–3.13 features (docs/VERSOES-LINGUAGEM.md §3) and
    // reads of a local that is not definitely assigned abort
    // (`codes::is_compile_error`); the rest of `types` is a warning and does not appear here.
    let mut errors = outline_diags
        .iter()
        .chain(body_diags.iter())
        .filter(|d| dartforge_types::codes::is_compile_error(&d.message));
    if let Some(first_one) = errors.next() {
        let mut msg = format!("error: {first_one}");
        for d in errors {
            msg.push_str(&format!("\nerror: {d}"));
        }
        return Err(msg);
    }

    // Constant legality is independent of reachability and lowering support:
    // canonicalizing a non-const constructor would otherwise silently accept
    // invalid Dart. Use the same verifier as the analyzer, with SDK bodies
    // opaque but their public constructor declarations available.
    let inferred = user_libraries.iter().copied().collect();
    let constant_diags = dartforge_types::const_eval::verify(
        &program,
        &interner,
        &mut table,
        &core,
        &outline,
        &bodies,
        &inferred,
        &user_libraries,
    );
    let mut constant_errors = constant_diags
        .iter()
        .filter(|(_, d)| d.severity == dartforge_diagnostics::Severity::Error);
    if let Some((unit, first)) = constant_errors.next() {
        let mut message = format!("error: {}: {first}", program.unit_path(*unit));
        for (unit, diagnostic) in constant_errors {
            message.push_str(&format!(
                "\nerror: {}: {diagnostic}",
                program.unit_path(*unit)
            ));
        }
        return Err(message);
    }

    let normalized_annotations = dartforge_types::optimization_annotations::normalize(
        &program,
        &interner,
        &mut table,
        &core,
        &outline,
        &bodies,
        &inferred,
        &user_libraries,
    );
    let contract_report = contracts::inspect(&normalized_annotations, &program, options.contracts)?;

    let te = erasure::compute(&program, &outline, &mut table);
    let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    ctx.arc_memory = arc_memory;
    ctx.arc_nursery_writes = arc_memory && target::arc_nursery_writes();
    ctx.enable_asserts = flags.enable_asserts;
    ctx.product = flags.product;
    ctx.library_support = sdk_module::library_support(&sdk);
    // Production (`--optimize`): the platform constants of the target fold
    // (`lower/platform.rs`); development and the JIT read them at run time.
    ctx.production = options.optimize;
    ctx.te = te;
    if options.debug {
        ctx.link_debug();
        ctx.dwarf = true;
    } else if symbolic_stack_trace {
        // The positions without DWARF: only the stack trace table (§13.14).
        ctx.link_debug();
    }
    ctx.stack_trace = symbolic_stack_trace;
    ctx.from_source = source_libraries.into_iter().collect();
    ctx.uses_dart_async = uses_dart_async;
    // The closed world of the program (C7, `native_world.rs`): what `main`
    // does not reach is not lowered.
    let t_world = Instant::now();
    ctx.world = native_world::compute(&program, &interner, &table, &outline, &bodies);
    let world_duration = t_world.elapsed();
    let world_stat = ctx
        .world
        .as_ref()
        .map_or((0, 0), |m| (m.stats.live_functions, m.stats.user_functions));
    // A program whose entry library neither declares nor exports `main` does
    // not run: the VM rejects it ("Invoked Dart programs must have a 'main'
    // function defined"). On reload, it is the file read in the middle of a
    // write (empty), which cannot become a generation.
    if ctx.entry_main.is_none() {
        return Err(
            "compile error: the program does not define a `main` function \
                    (Invoked Dart programs must have a 'main' function defined)"
                .to_string(),
        );
    }
    // The ids of the SDK classes are those of the compiled SDK, not those of what this
    // program loaded (`context::IdTable`).
    let ids = sdk_module::sdk_class_ids(&sdk_dir)?;
    let ctx = ctx.with_source_sdk_and_ids(ids);
    let ctx = match previous_ids {
        Some(ids) => ctx.with_previous_ids(ids),
        None => ctx,
    };
    let front_duration = t_front.elapsed();

    // 2. Lowering to HIR
    let t_hir = Instant::now();
    let mut hir_module = lower::lower_program(&ctx);
    hir_module.program_ids = ctx.program_ids();
    hir_module.program_fields = ctx.program_fields();
    hir_module.sdk_records = sdk_module::sdk_records();
    hir_module.runtime_cids = sdk_module::runtime_cids(&ctx);
    // `Platform.version` is `dart:io`'s, which the WebAssembly target does not have.
    hir_module.sdk_version = if crate::target::is_wasm() {
        None
    } else {
        sdk_dir.parent().and_then(sdk_version)
    };
    if !hir_module.errors.is_empty() {
        return Err(compile_error(&hir_module.errors));
    }
    hir_module.arc_fixed_ancestry = prune;
    // 2a. Pruning of the HIR of the program through the graph of symbols and selectors with
    // the SDK summaries (`pruning::prune_hir`): what the production link would
    // remove later leaves before optimizing and emitting. Not in the JIT (the next
    // generation of a reload may call any selector).
    // `DARTFORGE_NO_HIR_PRUNING=1` turns it off.
    if prune && !std::env::var("DARTFORGE_NO_HIR_PRUNING").is_ok_and(|v| v == "1") {
        let t_prune = Instant::now();
        optimize::prepare_for_pruning(&mut hir_module);
        if let Some(e) = prune_program_hir(
            &mut hir_module,
            options.optimize,
            &sdk_dir,
            flags,
            options.timings,
        )? && options.timings
        {
            eprintln!(
                "  HIR pruning: {} of {} functions, {} of {} pairs, {} FFI signatures, {} reachability rounds ({:?})",
                e.live_functions,
                e.functions,
                e.live_pairs,
                e.pairs,
                if e.ffi.1 {
                    e.ffi.0.to_string()
                } else {
                    "all".to_string()
                },
                e.reachability_rounds,
                t_prune.elapsed()
            );
        }
        if optimize::specialize_constants(&mut hir_module, table_exceptions) > 0 {
            // New bodies and folded branches change reachability. Reuse the
            // same SDK summaries before computing effect or escape facts.
            prune_program_hir(
                &mut hir_module,
                options.optimize,
                &sdk_dir,
                flags,
                options.timings,
            )?;
        }
    }
    // 2b. HIR optimization (inlining, scalar replacement: `optimize/`).
    let t_optimize = Instant::now();
    // The same pipeline prepares the SDK libraries and materializes tables
    // only after the HIR transformations.
    optimize::prepare_for_emission(&mut hir_module, arc_memory, table_exceptions);
    // The published ARC preparation (`DARTFORGE_ARC_PREPARED=1`): the program
    // alone, calling the prepared SDK (`sdk_module`, the same option in the
    // cache key) through the summaries of its own calls.
    if target::arc_prepared()? {
        hir_module.arc_external_callees = true;
        hir_module.arc_external_layouts = lower::sdk_arc_layouts(&ctx);
        let mut plans: std::collections::HashMap<_, _> = hir_module
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), optimize::arc::DartFunctionPlan::default()))
            .collect();
        // With tables, the plans start from the already materialized invokes and landing pads.
        if table_exceptions {
            optimize::arc::prepare_arc_tabled_module(&mut hir_module, &mut plans)
        } else {
            optimize::arc::prepare_arc_dart_module(&mut hir_module, &mut plans)
        }
        .map_err(|e| format!("ARC preparation of the program: {e}"))?;
        optimize::arc::certify_arc_aot_ports(&mut hir_module, &plans)
            .map_err(|e| format!("ARC ports of the program: {e}"))?;
        hir_module.arc_virtual_tokens = target::arc_virtual_tokens();
        hir_module.arc_prompt_death = target::arc_prompt_death();
        // Virtual tokens over the nursery: the stores into young objects come out
        // inline, as in the unprepared code (`llvm::LlvmEmitter::prepared_inline_field`).
        hir_module.arc_nursery_writes = hir_module.arc_virtual_tokens && target::arc_nursery();
    }
    if options.timings {
        eprintln!(
            "  HIR:       baixar {:?}, otimizar {:?}",
            t_optimize.duration_since(t_hir),
            t_optimize.elapsed()
        );
    }
    let hir_duration = t_hir.elapsed();

    // 3. LLVM IR emission
    let t_llvm = Instant::now();
    // Production (`--optimize`) never reloads: the area descriptor goes without
    // the names of the slots (`with_lean_area`). The JIT emits without `optimize`.
    let lean = options.optimize && previous_area.is_none();
    // The large program (new_sali/backend: ~180 thousand HIR functions) in
    // production: the address of the fields and the helpers (dispatch, allocation,
    // barrier, area, subtype) by call, out of line — the executable
    // falls from 94.6 to ~71 MB and generation from 707 to ~460 s
    // (docs/NATIVO-PRODUCAO-GRANDE.md §5). The price is the performance of code
    // that calls dynamic dispatch a lot or reads many fields
    // (`bench/performance/calls.dart`: 1.4–2.5× slower); with
    // `DARTFORGE_OUTLINED_HELPERS=` (empty) and `DARTFORGE_FIELDS_BY_CALL=0`
    // the large program goes back fully inline. A small program stays
    // inline (LTO removes what is left).
    let large_size = hir_module.functions.len() >= LARGE_PROGRAM_FUNCTIONS;
    let large = options.optimize && large_size;
    if large && options.timings {
        eprintln!(
            "  Large program: {} HIR functions (fields and helpers by call)",
            hir_module.functions.len()
        );
    }
    let fields_per_call = match std::env::var("DARTFORGE_FIELDS_BY_CALL") {
        Ok(v) => v == "1",
        Err(_) => large,
    };
    let emitter = llvm::LlvmEmitter::new(&hir_module)
        .with_previous_area(previous_area)
        .with_lean_area(lean)
        .with_static_objects(static_objects)
        // AOT does not reload: no object gets a body from outside.
        .with_body_without_outside(prune)
        // Development (`-O0`): the address of the fields by call (C9).
        .with_fields_per_call(!options.optimize || fields_per_call)
        .with_outlined_helpers(llvm::out_of_line_helpers(
            !options.optimize && large_size,
            large,
        ))
        // `optsize` on the functions of the large program: −8.7% in the executable of the
        // backend, without changing the time (docs/NATIVO-PRODUCAO-GRANDE.md §5);
        // `DARTFORGE_OPTSIZE_PROGRAM=0` turns it off.
        .with_optimize_size(
            large && !std::env::var("DARTFORGE_OPTSIZE_PROGRAM").is_ok_and(|v| v == "0"),
        )
        .with_production(options.optimize)
        // Production: the SDK tables assembled at link time are registered when
        // the isolate is prepared; the inline allocation has no registration test.
        .with_tables_registered_at_startup(options.optimize && prune)
        .with_arc_ports_at_link(options.optimize && prune && !hir_module.sdk_records.is_empty())
        // Without optimization there is no inlining: one roots frame per native
        // frame, and the runtime checks the order (the dead frame).
        .with_ordered_frames(!options.optimize)
        .with_roots_by_maps(roots_by_maps)
        .with_maps_in_jit(maps_in_jit)
        .with_stack_trace(symbolic_stack_trace)
        .with_stack_trace_in_jit(stack_trace_in_jit)
        // Production AOT: direct Dart calls pass the thread context.
        .with_context_parameter(options.optimize && prune);
    let llvm_ir = emitter.emit_all();
    // The verifier of maps mode (§7.4), before the maps pass.
    if roots_by_maps {
        llvm::verify_maps::verify(&llvm_ir)?;
    } else if llvm::verify_shadow::linked() {
        // The dominance checker of shadow mode (§7.4).
        llvm::verify_shadow::verify(&llvm_ir)?;
    }
    // On Itanium targets, every function needs the unwind table:
    // for exceptions by tables (the unwinding) and for roots by maps (the
    // collector's walk through `_Unwind_Backtrace`, which stops at a frame without a
    // table — macOS libunwind ends it there, and the roots of the outer frames
    // went unvisited).
    let llvm_ir = if (table_exceptions || roots_by_maps) && crate::target::is_unix() {
        sdk_module::with_uwtable(&llvm_ir)
    } else {
        llvm_ir
    };
    // With exceptions by check and the shadow stack nothing unwinds Dart
    // code: without the CFI of each function (`sdk_module::with_nounwind`).
    // Only on Itanium targets and WebAssembly (where nothing unwinds without
    // Wasm exceptions): on x64 COFF every function has an unwind table anyway,
    // and Windows was not measured.
    let llvm_ir = if (crate::target::is_unix() || crate::target::is_wasm())
        && !table_exceptions
        && !roots_by_maps
    {
        sdk_module::with_nounwind(&llvm_ir)
    } else {
        llvm_ir
    };
    // The thread context read inline (`sdk_module::with_context_in_tls`) in
    // every Unix AOT profile — exceptions by check or by tables, tracing or ARC
    // memory —: inline in a small program, a call in a large one. Only roots by
    // maps keep the runtime call: there the helper (and its `asm`) would have to
    // be a declared leaf of `rewrite-statepoints-for-gc` (`"gc-leaf-function"`),
    // which runs inside the linker's LTO on Mach-O and could not be validated
    // on this host. `options.optimize` links the runtime statically (a single
    // image): the slot of the context is then `dso_local`.
    let llvm_ir = if crate::target::is_unix() && prune && !roots_by_maps {
        let mode = if large {
            sdk_module::ContextInTls::PerCall
        } else {
            sdk_module::ContextInTls::Inline
        };
        sdk_module::with_context_in_tls(&llvm_ir, mode, "df.ctx.tls.program", options.optimize)
    } else {
        llvm_ir
    };
    let llvm_duration = t_llvm.elapsed();

    let sdk_byte_count = sdk_bytes(&llvm_ir, &program);
    Ok(EmittedIr {
        text: format!(
            "; DartForge contract provenance {}\n{llvm_ir}",
            contract_report.cache_tag()
        ),
        contracts: contract_report,
        sdk_lib: sdk_dir,
        times: EmissionTimes {
            frontend: front_duration,
            world: world_duration,
            live_functions: world_stat,
            hir: hir_duration,
            llvm_ir: llvm_duration,
        },
        sdk_byte_count,
    })
}

/// Bytes of the IR that are function bodies declared in a `dart:` library.
///
/// Measures how much of the module would be shareable between programs if the SDK became
/// a separate module (ESTADO.md §2.5). Each `define` is attributed to the element
/// by the library name in the stable symbol (`df.<library>.…`, which the
/// local closures inherit from the function that contains them); the rest — entry, dispatch, runtime
/// declarations, strings — counts as the program's.
fn sdk_bytes(text: &str, program: &Program) -> usize {
    let mut total = 0usize;
    let mut in_sdk = false;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("define ") {
            // The stable symbol starts with the library name (P2):
            // `@df.dart$3a…` is a function of a `dart:` library.
            let _ = program;
            in_sdk = rest.contains("@df.dart$3a");
        }
        if in_sdk {
            total += line.len();
            if line.trim_end() == "}" {
                in_sdk = false;
            }
        }
    }
    total
}

/// Compiles a Dart program into a native executable.
pub fn compile(entry: &Path, exit: &Path, options: &CompileOptions) -> Result<PathBuf, String> {
    compile_with(entry, exit, options)
}

/// [`compile`] choosing the SDK (see [`emit_ir_with`]).
pub fn compile_with(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    compile_with_flags(entry, exit, options, DartFlags::DEVELOPMENT)
}

/// [`compile`] with explicit [`DartFlags`] (asserts, `dart.vm.product`): the
/// production profile of the CLI, or the oracle's for the harness.
pub fn compile_with_flags(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
    flags: DartFlags,
) -> Result<PathBuf, String> {
    let t_total = Instant::now();

    let ir = emit_ir_with_flags(entry, options, flags)?;

    // 4. Clang and linking
    let driver_opts = driver::NativeDriverOptions {
        clang: driver::NativeDriverOptions::default().clang,
        optimize: options.optimize,
        timings: options.timings,
        debug_info: options.debug,
        cpu: options.cpu,
        sdk_lib: Some(ir.sdk_lib.clone()),
        flags,
    };

    let linkage = driver::compile_and_link(&ir.text, exit, &driver_opts)?;

    // A successful build records explicit ignore mode and package dependencies;
    // this report never certifies a request merely because code was emitted.
    let report_path = contracts::manifest_path(exit);
    let mut manifest = ir.contracts.json().clone();
    let image = std::fs::read(exit).map_err(|e| {
        format!(
            "could not bind contract provenance to {}: {e}",
            exit.display()
        )
    })?;
    manifest["artifact"] = serde_json::json!({
        "path": exit.display().to_string(),
        "blake3": blake3::hash(&image).to_hex().to_string(),
        "ir_blake3": blake3::hash(ir.text.as_bytes()).to_hex().to_string(),
        "optimized": options.optimize, "debug": options.debug,
        "cpu": format!("{:?}", options.cpu),
    });
    let report = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("could not serialize contract provenance: {e}"))?;
    std::fs::write(&report_path, report).map_err(|e| {
        format!(
            "could not write contract provenance {}: {e}",
            report_path.display()
        )
    })?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Native Compilation Timings ---");
        ir.print_times();
        let origin = if linkage.cache_object { " (cache)" } else { "" };
        eprintln!("  Objeto:    {:?}{origin}", linkage.clang);
        eprintln!("  Link:      {:?}", linkage.link);
        eprintln!("  Total:     {:?}", total_duration);
        eprintln!("  Pico Mem:  {} KB", peak_memory / 1024);
    }

    Ok(exit.to_path_buf())
}

#[cfg(test)]
/// The same paths as the real emission, also on the Unix runners.
pub(crate) fn sdk_tests() -> &'static Path {
    static SDK: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    SDK.get_or_init(|| {
        let sdk = std::env::var_os("DARTFORGE_TEST_SDK_LIB")
            .map(PathBuf::from)
            .unwrap_or_else(|| dart_sdk().expect("SDK required by the emission tests"));
        assert!(
            sdk.join("libraries.json").is_file(),
            "test SDK without libraries.json: {}",
            sdk.display()
        );
        sdk
    })
    .as_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(entry: &Path) -> EmittedIr {
        let options = CompileOptions {
            contracts: crate::contracts::ContractMode::Verify,
            sdk: Some(sdk_tests()),
            packages: None,
            timings: false,
            optimize: false,
            language_version: None,
            experiments: Vec::new(),
            debug: false,
            generator: None,
            cpu: None,
        };
        emit_ir(entry, &options).expect("emitir IR")
    }

    #[test]
    fn native_compilation_environment_uses_reserved_definition_and_defaults() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(dir.path().join("keys.dart"), "const compilerKey = 'dartforge.' + 'compiler';\n").unwrap();
            std::fs::write(&entry, r#"
import 'keys.dart' as keys;
const native = bool.fromEnvironment(keys.compilerKey, defaultValue: false);
const available = bool.hasEnvironment(keys.compilerKey);
const missing = bool.hasEnvironment('undefined.key');
const absent = bool.fromEnvironment('undefined.key', defaultValue: true);
const number = int.fromEnvironment(keys.compilerKey, defaultValue: 7);
void main() {
  const localKey = 'dartforge.compiler';
  const local = bool.fromEnvironment(localKey);
  const choice = true;
  print(const bool.fromEnvironment(choice ? 'dartforge.compiler' : 'missing'));
  print(const bool.fromEnvironment('dartforge.${'compiler'}'));
  print(const bool.fromEnvironment(bool.hasEnvironment('dartforge.compiler') ? 'dartforge.compiler' : 'missing'));
  print(native); print(available); print(missing); print(absent); print(number);
  print(const bool.fromEnvironment('dartforge.compiler'));
  print(const String.fromEnvironment('dartforge.compiler', defaultValue: 'fallback'));
  print(identical(const [local], const [true]));
  print(identical(const [native], const [true]));
  print(identical(const [number], const [7]));
}
"#).unwrap();
            let ir = emit(&entry).text;
            assert!(!ir.contains("Bool_fromEnvironment"));
            assert!(!ir.contains("Bool_hasEnvironment"));
            assert!(!ir.contains("String_fromEnvironment"));
            assert!(!ir.contains("Integer_fromEnvironment"));
            assert!(ir.contains("true"));
        }).unwrap().join().unwrap();
    }

    #[test]
    fn native_compilation_environment_rejects_unknown_const_names_in_nested_queries() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            let options = CompileOptions { contracts: crate::contracts::ContractMode::Verify, sdk: Some(sdk_tests()), packages: None, timings: false,
                optimize: true, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
            for value in [
                "const bool.fromEnvironment(1 == 1 ? 'dartforge.compiler' : 'missing')",
                "bool.fromEnvironment(1 == 1 ? 'dartforge.compiler' : 'missing')",
                "bool.fromEnvironment(bool.hasEnvironment(1 == 1 ? 'dartforge.compiler' : 'missing') ? 'dartforge.compiler' : 'missing')",
            ] {
                std::fs::write(&entry, format!("void main() {{ const result = {value}; print(result); }}\n")).unwrap();
                let error = emit_ir(&entry, &options).expect_err("unsupported const name must reject IR emission");
                assert!(error.contains("constant compilation environment name"), "{value}: {error}");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn constant_validation_rejects_non_const_constructors_before_reachability() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            let prefix = "class C { C(); C.named(); factory C.make() => C(); }\n";
            let cases = [
                "void main() { print(const C()); }",
                "void main() { print(const C.named()); }",
                "void main() { print(const C.make()); }",
                "void main() { const value = C(); print(value); }",
                "void main() { print(const [C()]); }",
                "void main() { const value = (C(), 1); print(value); }",
                "typedef Alias = C; void main() { print(const Alias()); }",
                "void unused() { print(const C()); } void main() {}",
            ];
            for body in cases {
                std::fs::write(&entry, format!("{prefix}{body}\n")).unwrap();
                let options = CompileOptions { contracts: crate::contracts::ContractMode::Verify, sdk: Some(sdk_tests()), packages: None, timings: false,
                    optimize: true, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
                let error = emit_ir(&entry, &options).expect_err("invalid constant must reject IR emission");
                assert!(error.contains("const") || error.contains("constant"), "{body}: {error}");
            }
            std::fs::write(&entry,
                "import 'dart:convert'; void main() { print(const JsonUtf8Encoder()); }\n").unwrap();
            let options = CompileOptions { contracts: crate::contracts::ContractMode::Verify, sdk: Some(sdk_tests()), packages: None, timings: false,
                optimize: true, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
            let error = emit_ir(&entry, &options).expect_err("SDK non-const constructor must reject IR emission");
            assert!(error.contains("isn't a const constructor"), "{error}");
            std::fs::write(dir.path().join("dependency.dart"), "class Token { Token(); Token.named(); }\n").unwrap();
            for body in ["print(const dep.Token());", "const value = dep.Token.named(); print(value);"] {
                std::fs::write(&entry, format!("import 'dependency.dart' as dep; void main() {{ {body} }}\n")).unwrap();
                assert!(emit_ir(&entry, &options).is_err(), "imported constructor: {body}");
            }
            std::fs::write(&entry,
                "class Base { Base(); } mixin M {} class Alias = Base with M; void main() { print(const Alias()); }\n").unwrap();
            assert!(emit_ir(&entry, &options).is_err(), "forwarded non-const constructor");
        }).unwrap().join().unwrap();
    }

    #[test]
    fn constant_validation_preserves_local_bindings_and_sdk_public_const_factories() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entry = dir.path().join("main.dart");
                std::fs::write(
                    &entry,
                    "class C { final int n; const C(this.n); }\n\
                void main() { const n = 4; const first = C(n); const values = [first, C(n)]; \
                const flag = bool.fromEnvironment('flag'); print(values.length); print(flag); }\n",
                )
                .unwrap();
                assert!(emit(&entry).text.contains("@dart_main"));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn bigint_operator_excludes_smi_without_removing_num_and_dynamic_path() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "BigInt big(BigInt a, BigInt b) => a - b;\nnum numero(num a, num b) => a - b;\ndynamic aberto(dynamic a, dynamic b) => a - b;\nBigInt composto(BigInt a, BigInt b) { a += b; return a; }\nnum compostoNumero(num a, num b) { a += b; return a; }\nvoid main() { print(big(BigInt.from(9), BigInt.from(4))); print(numero(9, 4)); print(aberto(9, 4)); print(composto(BigInt.from(9), BigInt.from(4))); print(compostoNumero(9, 4)); }\n").unwrap();
            let ir = emit(&entry).text;
            let body = |name: &str| {
                let symbol = format!("@df.main$2edart..{name}(");
                let start = ir.lines().find(|l| l.starts_with("define ") && l.contains(&symbol))
                    .unwrap_or_else(|| panic!("function {name} not emitted"));
                let pos = ir.find(start).unwrap();
                ir[pos..].split("\n}").next().unwrap()
            };
            for name in ["big", "composto"] { assert!(!body(name).contains("ashr i64"), "{}", body(name)); }
            assert!(body("big").contains("call "));
            for name in ["numero", "aberto", "compostoNumero"] {
                assert!(body(name).contains("ashr i64"), "{}", body(name));
            }
        }).unwrap().join().unwrap();
    }

    /// The same program emitted twice in sequence and four times at the
    /// same time gives the same IR: nothing in the emission depends on global state,
    /// addresses or completion order.
    #[test]
    fn emit_ir_is_deterministic() {
        sdk_tests();
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() { print(1); }\n").unwrap();
        let a = emit(&entry);
        assert!(a.text.contains("@dart_main"), "{}", a.text);
        assert!(a.sdk_byte_count <= a.text.len());
        assert_eq!(a.text, emit(&entry).text);
        let parallel: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    std::thread::Builder::new()
                        .stack_size(64 << 20)
                        .spawn_scoped(s, || emit(&entry).text)
                        .unwrap()
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for p in parallel {
            assert_eq!(p, a.text);
        }
    }

    /// An unsupported construct is an `Err` of `emit_ir`, with all the
    /// diagnostics: no IR, and no executable that only prints them.
    #[test]
    fn unsupported_construct_is_error_with_all_diagnostics() {
        sdk_tests();
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() {\n  int? k = 1;\n  var a = {?k: 1};\n  var b = {?k: 2};\n  print(a.length + b.length);\n}\n").unwrap();
        let options = CompileOptions {
            contracts: crate::contracts::ContractMode::Verify,
            sdk: Some(sdk_tests()),
            packages: None,
            timings: false,
            optimize: false,
            language_version: None,
            experiments: Vec::new(),
            debug: false,
            generator: None,
            cpu: None,
        };
        let error = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || emit_ir(&entry, &options).map(|ir| ir.text))
            .unwrap()
            .join()
            .unwrap()
            .expect_err("null-aware map entry is rejected in native");
        let lines: Vec<&str> = error.lines().collect();
        assert_eq!(
            lines[0], "compile error: not supported by the native backend: null-aware map entry",
            "{error}"
        );
        assert_eq!(
            lines[1],
            "  not supported by the native backend: null-aware map entry (main.dart:3:11)",
            "{error}"
        );
        assert_eq!(
            lines[2],
            "  not supported by the native backend: null-aware map entry (main.dart:4:11)",
            "{error}"
        );
        assert_eq!(
            error_constructs(&error),
            ["null-aware map entry", "null-aware map entry"]
        );
    }

    /// The defined symbols (`define … @<symbol>(`) of an IR.
    fn symbols(ir: &str) -> std::collections::BTreeSet<String> {
        ir.lines()
            .filter_map(|l| {
                let r = l.strip_prefix("define ")?;
                let i = r.find('@')? + 1;
                let f = r[i..].find('(')? + i;
                Some(r[i..f].to_string())
            })
            .collect()
    }

    /// T-ID (PESQUISA-HOT-RELOAD §4.2, P2): the symbol comes from the path of the
    /// declaration. Inserting a function and a class does not change the symbol of
    /// any other declaration, and the same program in another directory has the
    /// same symbols.
    #[test]
    fn t_id_stable_symbols() {
        sdk_tests();
        let a = "int a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g);\n}\n";
        let b = "int z() => 0;\nclass D {}\nint a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint h = 6;\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g + z() + h);\n}\n";
        let emit_into = |source: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entry = dir.path().join("main.dart");
                    std::fs::write(&entry, source).unwrap();
                    emit(&entry).text
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let (ia, ib, ia2) = (emit_into(a), emit_into(b), emit_into(a));
        let (sa, sb) = (symbols(&ia), symbols(&ib));
        for s in [
            "df.main$2edart..a",
            "df.main$2edart.C.m",
            "df.main$2edart.C.s",
            "df.main$2edart.C.new",
            "df.main$2edart..g",
            "dart_main",
        ] {
            assert!(sa.contains(s), "{s} in {sa:?}");
        }
        assert!(
            sa.is_subset(&sb),
            "symbols of A that disappeared in B: {:?}",
            sa.difference(&sb).collect::<Vec<_>>()
        );
        // The same program in another directory gives the same IR, except for the
        // data constants (`@df.arr.*`): the RTI table carries the `file:` URI
        // of the library of each class (`dart:mirrors`), which depends
        // on the directory by definition. The table goes compressed
        // (`dartforge_runtime::rti_table`), so the size of a
        // constant (the `[N x i64]` type) and the argument of the call that
        // registers the table may also change.
        // A content-addressed numeric constant changes its symbol together
        // with those URI bytes. Restore registration-ordinal identities only
        // for definitions actually emitted in this IR; declaration/function
        // identities and instructions remain subject to the exact comparison.
        let normalize_vectors = |ir: &str| {
            let definitions: Vec<_> = ir
                .lines()
                .filter_map(|line| {
                    let name = line.strip_prefix('@')?.split_once(" = ")?.0;
                    name.starts_with("df.const.i64.").then_some(name)
                })
                .collect();
            let mut normalized = ir.to_string();
            for (ordinal, name) in definitions.into_iter().enumerate() {
                let old = format!("@{name}");
                let new = format!("@df.arr.{ordinal}");
                let mut text = String::with_capacity(normalized.len());
                let mut start = 0;
                for (offset, _) in normalized.match_indices(&old) {
                    let end = offset + old.len();
                    // Do not rewrite a longer declaration/function identifier
                    // merely because it begins with a pooled data symbol.
                    if normalized.as_bytes().get(end).is_some_and(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'-')
                    }) {
                        continue;
                    }
                    text.push_str(&normalized[start..offset]);
                    text.push_str(&new);
                    start = end;
                }
                text.push_str(&normalized[start..]);
                normalized =
                    text.replace(&format!("$\"{name}\""), &format!("$\"df.arr.{ordinal}\""));
            }
            normalized
        };
        let (normalized_a, normalized_a2) = (normalize_vectors(&ia), normalize_vectors(&ia2));
        let (la, la2): (Vec<_>, Vec<_>) = (
            normalized_a.lines().collect(),
            normalized_a2.lines().collect(),
        );
        assert_eq!(
            la.len(),
            la2.len(),
            "the same program in another directory gives the same IR"
        );
        let rti_table = |l: &str| l.contains("@dartforge_rti_init_table(");
        for (x, y) in la.iter().zip(&la2) {
            if x != y {
                assert!(
                    (x.starts_with("@df.arr.") && y.starts_with("@df.arr."))
                        || (rti_table(x) && rti_table(y)),
                    "the same program in another directory gives the same IR:\n{x}\n{y}"
                );
            }
        }
    }

    /// P6 and the zero-cost rule: only a program that uses `dart:async` compiles
    /// the source `dart:async` and links the event loop; one that does not has
    /// none of it in the IR.
    #[test]
    fn source_dart_async_only_for_users() {
        sdk_tests();
        let emit_source = |source: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entry = dir.path().join("main.dart");
                    std::fs::write(&entry, source).unwrap();
                    emit(&entry)
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let synchronous = emit_source("void main() { print(1); }\n");
        assert!(!synchronous.text.contains("call void @dartforge_event_loop"));
        assert!(!synchronous.text.contains("@df.dart$3aasync"));
        assert_eq!(synchronous.sdk_byte_count, 0);

        let asynchronous = emit_source(
            "Future<int> f() async { await null; return 2; }\nFuture<void> main() async { print(await f()); }\n",
        );
        assert!(
            asynchronous
                .text
                .contains("call void @dartforge_event_loop(ptr @df.call_dart0)")
        );
        assert!(
            asynchronous
                .text
                .contains("define i64 @df.main$2edart..f$async$q"),
            "the body of the state machine"
        );
        // `dart:async` comes from the cached SDK module (`sdk_module.rs`): the IR
        // of the program only calls it, it does not define its functions (`sdk_byte_count`
        // may be 0). An `async` body awaits through its completer (`_asyncAwaitIn`).
        assert!(
            asynchronous
                .text
                .contains("@df.dart$3aasync.._asyncAwaitIn(")
        );
    }

    #[test]
    fn error_constructs_ignores_non_constructs() {
        let text = "[compile-native] compile error: not supported by the native backend: member `hash`\n\
                     compile error: not supported by the native backend: member `hash`\n  \
                     not supported by the native backend: member `hash` (a.dart:1:2)\n  \
                     verificador da HIR (main): tag 3 para um valor I64\n  \
                     not supported by the native backend: call `sort` (a.dart:3:4)";
        assert_eq!(error_constructs(text), ["member `hash`", "call `sort`"]);
        assert!(error_constructs("error loading the program: x").is_empty());
    }
}

#[cfg(test)]
mod inline_pragma_tests {
    use super::*;

    #[test]
    fn source_inline_requests_reach_native_bodies_and_async_state_machines() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entry = dir.path().join("inline.dart");
                std::fs::write(
                    &entry,
                    r#"
@pragma('vm:prefer-inline') int preferred(int x) => x + 1;
@pragma('vm:prefer-inline') @pragma('vm:never-inline') int forbidden(int x) => x + 2;
class C {
  @pragma('vm:never-inline') C();
  @pragma('vm:prefer-inline') int method(int x) => x + 3;
  @pragma('vm:never-inline') int get value => 4;
}
@pragma('vm:never-inline') Future<int> asyncValue() async { await null; return 5; }
Future<void> main() async {
  @pragma('vm:never-inline') int localDirect(int x) => x + 6;
  final captured = 7;
  @pragma('vm:never-inline') int localClosure(int x) => captured + x;
  @pragma('vm:never-inline') Future<int> localAsync() async { await null; return 8; }
  final escaped = localClosure;
  print(localDirect(1)); print(escaped(1)); print(await localAsync());
  final c = C();
  print(preferred(1)); print(forbidden(1)); print(c.method(1)); print(c.value);
  print(await asyncValue());
}
"#,
                )
                .unwrap();
                for optimize in [false, true] {
                    let options = CompileOptions {
                        contracts: crate::contracts::ContractMode::Verify,
                        sdk: Some(sdk_tests()),
                        packages: None,
                        timings: false,
                        optimize,
                        language_version: None,
                        experiments: Vec::new(),
                        debug: false,
                        generator: None,
                        cpu: None,
                    };
                    let ir = emit_ir(&entry, &options).unwrap().text;
                    let headers: Vec<_> = ir.lines().filter(|l| l.starts_with("define ")).collect();
                    // The body: `<symbol>$cx` when it takes the context parameter
                    // (production), not its `alwaysinline` thunk.
                    let check = |symbol: &str, attribute: &str| {
                        let header = headers
                            .iter()
                            .find(|l| {
                                l.contains(&format!("@{symbol}$cx("))
                                    && !l.contains(" alwaysinline")
                            })
                            .or_else(|| headers.iter().find(|l| l.contains(&format!("@{symbol}("))))
                            .unwrap();
                        assert!(header.contains(attribute), "{header}");
                    };
                    check("df.inline$2edart..forbidden", " noinline");
                    assert!(
                        ir.contains("call i64 @df.inline$2edart..forbidden(")
                            || ir.contains("call i64 @df.inline$2edart..forbidden$cx("),
                        "never-inline must retain the call boundary"
                    );
                    check("df.inline$2edart.C.new", " noinline");
                    check("df.inline$2edart.C.value", " noinline");
                    check("df.inline$2edart..asyncValue", " noinline");
                    let body = headers
                        .iter()
                        .find(|l| {
                            l.contains("@df.inline$2edart..asyncValue$async$q")
                                && !l.contains("$ent(")
                        })
                        .unwrap();
                    assert!(body.contains(" noinline"), "{body}");
                    for local in ["$localDirect", "$localClosure", "$localAsync"] {
                        let body = headers
                            .iter()
                            .find(|l| {
                                l.contains(local) && !l.contains("$ent(") && !l.contains("$async$q")
                            })
                            .unwrap();
                        assert!(body.contains(" noinline"), "{body}");
                    }
                    if !optimize {
                        check("df.inline$2edart..preferred", " inlinehint");
                        check("df.inline$2edart.C.method", " inlinehint");
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod native_pump_contract_tests {
    use super::*;

    #[test]
    fn dispatcher_bridge_requires_the_exact_callback_and_contract_version() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let package = dir.path().join("pkg/lib/src");
            std::fs::create_dir_all(&package).unwrap();
            let config = dir.path().join("packages.json");
            std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"dartforge","rootUri":"pkg/","packageUri":"lib/","languageVersion":"3.6"}]}"#).unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "import 'package:dartforge/src/native_pump.dart'; void main() { print(run()); }").unwrap();
            let options = CompileOptions { contracts: crate::contracts::ContractMode::Verify,
                sdk: Some(sdk_tests()), packages: Some(&config), timings: false, optimize: false,
                language_version: None, experiments: vec![], debug: false, generator: None, cpu: None };
            for (version, ty, argument) in [
                ("v1", "void Function(int)", "(int value) {}"),
                ("v1", "void Function()?", "null"),
                ("v1", "Function", "() {}"),
                ("v2", "void Function()", "() {}"),
            ] {
                std::fs::write(package.join("native_pump.dart"), format!(
                    "@pragma('dartforge:native-pump-{version}') int _setNativeEventDispatcherV1({ty} callback) => 7; int run() => _setNativeEventDispatcherV1({argument});"
                )).unwrap();
                let ir = emit_ir(&entry, &options).unwrap().text;
                assert!(!ir.contains("call i64 @dartforge_native_DartForge_native_pump_set_dispatcher("), "{version}, {ty}");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn versioned_package_body_is_native_but_arbitrary_pragmas_are_not() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/dartforge").canonicalize().unwrap();
            let config = dir.path().join("packages.json");
            let root = url::Url::from_directory_path(&package).unwrap();
            std::fs::write(&config, serde_json::json!({"configVersion": 2, "packages": [{
                "name": "dartforge", "rootUri": root.as_str(), "packageUri": "lib/", "languageVersion": "3.6"
            }]}).to_string()).unwrap();
            let entry = dir.path().join("main.dart");
            let options = CompileOptions { contracts: crate::contracts::ContractMode::Verify, sdk: Some(sdk_tests()), packages: Some(&config), timings: false,
                optimize: false, language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None };
            // Test the defining library without requiring unrelated optional
            // package dependencies in this standalone package configuration.
            std::fs::write(&entry, "import 'package:dartforge/src/native_pump.dart'; void dispatch() {} void main() { print(EventLoop.supportsNativePump); EventLoop.enableNativePump(); print(EventLoop.supportsNativeEventDispatcher); EventLoop.setNativeEventDispatcher(dispatch); EventLoop.clearNativeEventDispatcher(); EventLoop.disableNativePump(); }\n").unwrap();
            let ir = emit_ir(&entry, &options).unwrap().text;
            for name in ["supported", "enable", "disable"] {
                let abi = if name == "supported" { "i8" } else { "i64" };
                assert!(ir.contains(&format!("call {abi} @dartforge_native_DartForge_native_pump_{name}(")), "{name}");
            }
            for (name, abi) in [("dispatcher_supported", "i8"), ("set_dispatcher", "i64"), ("clear_dispatcher", "i64")] {
                assert!(ir.contains(&format!("call {abi} @dartforge_native_DartForge_native_pump_{name}(")), "{name}");
            }
            assert!(ir.contains("call void @dartforge_event_loop"));
            std::fs::write(&entry, "@pragma('dartforge:native-pump-v1') bool _supportsNativePumpV1() => true; void main() { print(_supportsNativePumpV1()); }\n").unwrap();
            let ir = emit_ir(&entry, &options).unwrap().text;
            assert!(!ir.contains("call i8 @dartforge_native_DartForge_native_pump_supported("));
        }).unwrap().join().unwrap();
    }
}
