//! DartForge's LLVM native compiler on the new track.

pub mod alvo;
pub mod erasure;
pub mod cache;
pub mod object_cache;
pub mod context;
pub mod driver;
pub mod source;
mod finalizables;
pub mod gcmap;
pub mod compact_stack_trace;
pub mod gerador;
pub mod ligador;
pub mod ligador_macos;
pub mod ligador_windows;
#[cfg(feature = "llvm-embutido")]
pub mod distributed_lto;
pub mod hir;
pub mod llvm;
pub mod lower;
pub mod native_world;
pub mod partition;
pub mod natives;
pub mod optimize;
pub mod pruning;
pub mod resumo;
pub mod sdk_modulo;

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
    pub sdk: Option<&'a Path>,
    pub packages: Option<&'a Path>,
    pub timings: bool,
    pub optimize: bool,
    /// Current language version (`--versao-linguagem`, docs/VERSOES-LINGUAGEM.md);
    /// `None` = the tool's (3.13).
    pub versao_linguagem: Option<dartforge_frontend::LanguageVersion>,
    /// Experiments requested explicitly for libraries at the current version.
    pub experimentos: Vec<dartforge_frontend::Feature>,
    /// J05: line tables for the native debugger (`llvm/debug_info.rs`).
    pub depuracao: bool,
    /// Who produces the generated sources of the project (the build engine,
    /// `crates/build`) from the program loaded without them — the same
    /// contract as `compile-js` (`dartforge_emit_js::Gerador`). `None`: the
    /// project does not use builders, or the caller does not enable them.
    pub gerador: Option<Gerador<'a>>,
    /// The target CPU of the generated code (`--cpu`, [`gerador::Cpu`]); `None`, the
    /// baseline of the target (on x86-64, SSE2).
    pub cpu: Option<gerador::Cpu>,
}

/// Veja [`CompileOptions::gerador`].
pub type Gerador<'a> = &'a (dyn Fn(
    &dartforge_elements::model::Program,
    &Interner,
) -> Result<std::sync::Arc<dartforge_elements::generated::Generation>, String>
         + Sync);

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
pub struct IrEmitido {
    pub texto: String,
    pub tempos: EmissionTimes,
    /// Bytes of `texto` that are function bodies of the SDK (`dart:`); see
    /// `sdk_bytes`.
    pub sdk_byte_count: usize,
}

impl IrEmitido {
    /// The `--timings` lines of the emission, in the format of [`compilar`].
    pub fn imprimir_tempos(&self) {
        eprintln!("  Front-end: {:?}", self.tempos.frontend);
        let (live_set, all) = self.tempos.live_functions;
        eprintln!("  World:     {:?} ({live_set} of {all} program functions live)", self.tempos.world);
        eprintln!("  HIR:       {:?}", self.tempos.hir);
        eprintln!("  LLVM IR:   {:?}", self.tempos.llvm_ir);
        let pct = if self.texto.is_empty() { 0.0 } else { 100.0 * self.sdk_byte_count as f64 / self.texto.len() as f64 };
        eprintln!("  IR:        {} bytes, {} from the SDK ({pct:.1}%)", self.texto.len(), self.sdk_byte_count);
    }
}

/// Start of every diagnostic of a construct that the lowering cannot lower
/// (`FnBuilder::unsupported`, N1). [`construtos_do_erro`] is what reads it.
pub const UNSUPPORTED_PREFIX: &str = "não suportado no backend nativo: ";

/// [`pruning::prune_hir`] with the summaries of the SDK libraries of the profile (the
/// SDK comes out of the cache, or is compiled now, as the link would do right
/// after). `None` when the module does not use the source SDK.
fn prune_program_hir(module: &mut hir::Module, production: bool) -> Result<Option<pruning::HirPruning>, String> {
    if module.sdk_library || module.sdk_records.is_empty() {
        return Ok(None);
    }
    let profile = if production { sdk_modulo::SdkProfile::Production } else { sdk_modulo::SdkProfile::Development };
    let clang = driver::NativeDriverOptions::default().clang;
    // The same directory and the same Clang as the link (`driver::compile_and_link`):
    // the same cache key.
    let sdk = sdk_modulo::compiled_sdk_in_profile(&sdk_do_dart()?, &clang, profile)?;
    if !sdk.summaries.iter().all(|r| r.is_file()) {
        return Ok(None);
    }
    let summaries = pruning::read_summaries(&sdk.summaries)?;
    let refs: Vec<&pruning::Summary> = summaries.iter().map(|r| r.as_ref()).collect();
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
    let number = std::fs::read_to_string(root.join("version")).ok()?.trim().to_string();
    let saved = cache::native_cache_dir().join(format!("versao_do_sdk_{number}.txt"));
    if let Ok(v) = std::fs::read_to_string(&saved)
        && v.starts_with(&number)
    {
        return Some(v.trim().to_string());
    }
    let dart = root.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let complete = std::process::Command::new(&dart)
        .arg("--version")
        .output()
        .ok()
        .and_then(|s| {
            // `Dart SDK version: 3.6.2 (stable) (…) on "windows_x64"` (on
            // standard output since 2.15; before, on standard error).
            let text = format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
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
/// The first line is `erro de compilação: ` and the first diagnostic **without the
/// position**: it is the grouping key of the harness, and the same construct in
/// different programs has to fall into the same group. Then comes one line per
/// diagnostic, all of them, with the position and indented by two spaces.
fn compile_error(errors: &[String]) -> String {
    let first_one = &errors[0];
    let resumo = first_one.rsplit_once(" (").map_or(first_one.as_str(), |(a, _)| a);
    let mut text = format!("erro de compilação: {resumo}");
    for e in errors {
        text.push_str("\n  ");
        text.push_str(e);
    }
    text
}

/// The unsupported constructs of an error of [`emitir_ir`]/[`compilar`], or
/// of a text that contains it (the stderr the harness builds with it): one per
/// diagnostic, in order, without the position. Empty when the error is not about a
/// construct (loading, panic, HIR verifier).
pub fn construtos_do_erro(text: &str) -> Vec<String> {
    text
        .lines()
        .filter_map(|l| {
            let d = l.strip_prefix("  ")?.strip_prefix(UNSUPPORTED_PREFIX)?;
            Some(d.rsplit_once(" (").map_or(d, |(a, _)| a).to_string())
        })
        .collect()
}

/// Loads, analyzes and lowers a program down to LLVM IR, without Clang or linking.
///
/// It is the part of [`compilar`] that is ours: the determinism test of the harness
/// compares only this (`dartforge-differential determinismo --nativo`), and
/// `compile-native --emit-ir` writes this. Loading diagnostics go into the
/// error message, and not to stderr, so that they do not interleave when several
/// emissions run in the same process. The first line is the first
/// diagnostic, because it is what the harness report shows and groups — a
/// count ("1 erro(s)") put different causes into a single group.
///
/// An unsupported construct is also an `Err`, with **all** the diagnostics of the
/// module (format in `compile_error`); no IR is emitted.
pub fn emitir_ir(entry: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emit_ir_with(entry, options)
}

/// The `lib/` of the Dart SDK: what [`SdkLayout::discover`] finds (the environment
/// variable, the distribution, the `dart` on `PATH`), or the error that says how
/// to point to it.
pub fn sdk_do_dart() -> Result<PathBuf, String> {
    SdkLayout::discover().ok_or_else(|| {
        "Dart SDK not found: the dartforge distribution carries it in lib/dart-sdk; \
         in a development tree, set DARTFORGE_SDK_LIB (the lib/ of the SDK) or DART_SDK"
            .to_string()
    })
}

/// [`emitir_ir`] (the name stays for its existing callers; the SDK always comes from
/// source, docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
///
/// Literals as static objects (§2.11) only in production (`optimize`), which
/// is a single image. In development the SDK lives in the DLL, with its
/// statics: a static literal in the executable would be an object other than the equal one of the
/// DLL (`identical('a'.substring(1, 1), '')` gave false). Without statics, the
/// literal of the program is interned in the heap and canonicalized against the
/// statics of the DLL, as in the JIT.
pub fn emit_ir_with(entry: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emit_ir_internal(entry, options, None, None, options.optimize, true)
}

/// [`emitir_ir`] of a new generation of a running program (the JIT hot
/// reload): `previous_ir` is the IR of the live generation, from which the layout
/// of the global area that the new generation extends comes ([`ir_area`]).
pub fn emitir_ir_recarregavel(entry: &Path, options: &CompileOptions, previous_ir: Option<&str>) -> Result<IrEmitido, String> {
    let area = previous_ir.and_then(ir_area);
    // J03: the same class with the same id as in the live generation.
    let ids = previous_ir.map(context::ir_ids);
    // The JIT frees the memory of a generation (J02): no static objects in the
    // program modules (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
    emit_ir_internal(entry, options, area, ids, false, false)
}

/// The names (hashes) of the slots of the global area of the program in IR `ir`:
/// the `@df.area = … [key, n, name_0…]` that the emitter writes.
pub fn ir_area(ir: &str) -> Option<Vec<i64>> {
    let line = ir.lines().find(|l| l.starts_with("@df.area = "))?;
    let list = &line[line.rfind('[')? + 1..line.rfind(']')?];
    let values: Vec<i64> = list
        .split(',')
        .map(|v| v.trim().strip_prefix("i64 ").and_then(|n| n.trim().parse().ok()))
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
) -> Result<IrEmitido, String> {
    // The exception model (`alvo::excecoes_por_tabelas`). The JIT (the only one
    // that emits without pruning) does not have the tables: its code does not register
    // the unwind tables with the system.
    let excecoes_por_tabelas = alvo::excecoes_por_tabelas()?;
    if excecoes_por_tabelas && !prune {
        return Err("exceptions by tables (DARTFORGE_EXCECOES=tabelas) do not exist in the JIT: use compile-native".to_string());
    }
    // The roots mode (`alvo::raizes_por_mapas`). In the JIT (the only one that emits
    // without pruning) the map of each object is found and registered by the
    // memory manager of the session (`crates/jit`, §3.8).
    let raizes_por_mapas = alvo::raizes_por_mapas()?;
    // The memory policy (`alvo::arc_memory`), in AOT and in the JIT: the
    // instance migration of reload counts through the snapshot
    // (`Heap::trocar_campos`, docs/ARC-IMPLEMENTACAO.md).
    let arc_memory = alvo::arc_memory()?;
    let maps_in_jit = raizes_por_mapas && !prune;
    // The symbolic stack trace (§13.14). In the JIT the table of each object is
    // registered by the memory manager of the session (`crates/jit`), and the
    // module comes out without the linker symbols and without the registration call.
    let symbolic_stack_trace = alvo::symbolic_stack_trace()?;
    let stack_trace_in_jit = symbolic_stack_trace && !prune;
    // 1. Loading and inference (front end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => sdk_do_dart()?,
    };

    // The `vm` section with the `native_sdk/` overlay (P5a): the libraries of
    // `SOURCE_LIBRARIES` are its own, linked as cached objects
    // (`sdk_modulo::compiled_sdk`). `mut`: the
    // current language version may come from `--versao-linguagem` (modern
    // Dart, P2).
    let mut sdk = sdk_modulo::load_native_sdk(&sdk_dir)
        .map_err(|e| format!("failed to load the VM SDK: {e}"))?;
    if let Some(v) = options.versao_linguagem {
        sdk.current_version = v;
    }
    for experiment in &options.experimentos {
        if !sdk.experiments.contains(experiment) {
            sdk.experiments.push(*experiment);
        }
    }

    let mut interner = Interner::new();
    // With the build engine (DF-BUILD-009): a tolerant load without the
    // generated files serves as `BuildStep.solve`; the generation comes back in memory and
    // the real load reads it before the disk.
    let (program, elements_diags) = match options.gerador {
        Some(gerar) => {
            let mut resolution_names = Interner::new();
            let (resolution, _) = load_lenient(entry, &sdk, options.packages, &mut resolution_names);
            let generation = gerar(&resolution, &resolution_names)?;
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
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
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

    let te = erasure::compute(&program, &outline, &mut table);
    let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    ctx.arc_memory = arc_memory;
    ctx.te = te;
    if options.depuracao {
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
    let world_stat = ctx.world.as_ref().map_or((0, 0), |m| (m.estat.funcoes_vivas, m.estat.funcoes_usuario));
    // A program without `main` in the entry library does not run: the VM
    // rejects it ("Invoked Dart programs must have a 'main' function defined").
    // On reload, it is the file read in the middle of a write (empty), which cannot
    // become a generation.
    let has_main = program.functions.iter().any(|f| {
        f.class.is_none()
            && f.extension.is_none()
            && f.kind == dartforge_elements::model::FunctionKind::Function
            && Some(f.library) == ctx.entry_lib
            && interner.resolve(f.name) == "main"
    });
    if !has_main {
        return Err("erro de compilação: o programa não define a função `main` \
                    (Invoked Dart programs must have a 'main' function defined)"
            .to_string());
    }
    // The ids of the SDK classes are those of the compiled SDK, not those of what this
    // program loaded (`context::IdTable`).
    let ids = sdk_modulo::sdk_class_ids(&sdk_dir)?;
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
    hir_module.sdk_records = sdk_modulo::sdk_records();
    hir_module.runtime_cids = sdk_modulo::runtime_cids(&ctx);
    hir_module.sdk_version = sdk_dir.parent().and_then(sdk_version);
    if !hir_module.errors.is_empty() {
        return Err(compile_error(&hir_module.errors));
    }
    // 2a. Pruning of the HIR of the program through the graph of symbols and selectors with
    // the SDK summaries (`pruning::prune_hir`): what the production link would
    // remove later leaves before optimizing and emitting. Not in the JIT (the next
    // generation of a reload may call any selector).
    // `DARTFORGE_SEM_PODA_HIR=1` turns it off.
    if prune && !std::env::var("DARTFORGE_SEM_PODA_HIR").is_ok_and(|v| v == "1") {
        let t_prune = Instant::now();
        if let Some(e) = prune_program_hir(&mut hir_module, options.optimize)?
            && options.timings
        {
            eprintln!(
                "  HIR pruning: {} of {} functions, {} of {} pairs, {} FFI signatures ({:?})",
                e.live_functions,
                e.functions,
                e.live_pairs,
                e.pairs,
                if e.ffi.1 { e.ffi.0.to_string() } else { "todas as".to_string() },
                t_prune.elapsed()
            );
        }
    }
    // 2b. HIR optimization (inlining, scalar replacement: `optimize/`).
    let t_optimize = Instant::now();
    // The same pipeline prepares the SDK libraries and materializes tables
    // only after the HIR transformations.
    optimize::prepare_for_emission(&mut hir_module, arc_memory, excecoes_por_tabelas);
    // The published ARC preparation (`DARTFORGE_ARC_PREPARADO=1`): the program
    // alone, calling the prepared SDK (`sdk_modulo`, the same option in the
    // cache key) through the summaries of its own calls.
    if alvo::arc_prepared()? {
        hir_module.arc_external_callees = true;
        hir_module.arc_external_layouts = lower::sdk_arc_layouts(&ctx);
        let mut plans: std::collections::HashMap<_, _> = hir_module
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), optimize::arc::DartFunctionPlan::default()))
            .collect();
        // With tables, the plans start from the already materialized invokes and landing pads.
        if excecoes_por_tabelas {
            optimize::arc::prepare_arc_tabled_module(&mut hir_module, &mut plans)
        } else {
            optimize::arc::prepare_arc_dart_module(&mut hir_module, &mut plans)
        }
        .map_err(|e| format!("ARC preparation of the program: {e}"))?;
        optimize::arc::certify_arc_aot_ports(&mut hir_module, &plans)
            .map_err(|e| format!("ARC ports of the program: {e}"))?;
    }
    if options.timings {
        eprintln!("  HIR:       baixar {:?}, otimizar {:?}", t_optimize.duration_since(t_hir), t_optimize.elapsed());
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
    // (`bench/performance/chamadas.dart`: 1.4–2.5× slower); with
    // `DARTFORGE_AJUDANTES_FORA=` (empty) and `DARTFORGE_CAMPOS_POR_CHAMADA=0`
    // the large program goes back fully inline. A small program stays
    // inline (LTO removes what is left).
    let large_size = hir_module.functions.len() >= LARGE_PROGRAM_FUNCTIONS;
    let large = options.optimize && large_size;
    if large && options.timings {
        eprintln!("  Large program: {} HIR functions (fields and helpers by call)", hir_module.functions.len());
    }
    let fields_per_call = match std::env::var("DARTFORGE_CAMPOS_POR_CHAMADA") {
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
        .with_outlined_helpers(llvm::out_of_line_helpers(!options.optimize && large_size, large))
        // `optsize` on the functions of the large program: −8.7% in the executable of the
        // backend, without changing the time (docs/NATIVO-PRODUCAO-GRANDE.md §5);
        // `DARTFORGE_OPTSIZE_PROGRAMA=0` turns it off.
        .with_optimize_size(large && !std::env::var("DARTFORGE_OPTSIZE_PROGRAMA").is_ok_and(|v| v == "0"))
        .with_production(options.optimize)
        // Without optimization there is no inlining: one roots frame per native
        // frame, and the runtime checks the order (the dead frame).
        .with_ordered_frames(!options.optimize)
        .with_roots_by_maps(raizes_por_mapas)
        .with_maps_in_jit(maps_in_jit)
        .with_stack_trace(symbolic_stack_trace)
        .with_stack_trace_in_jit(stack_trace_in_jit);
    let llvm_ir = emitter.emit_all();
    // The verifier of maps mode (§7.4), before the maps pass.
    if raizes_por_mapas {
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
    let llvm_ir = if (excecoes_por_tabelas || raizes_por_mapas) && cfg!(unix) { sdk_modulo::with_uwtable(&llvm_ir) } else { llvm_ir };
    // With exceptions by check and the shadow stack nothing unwinds Dart
    // code: without the CFI of each function (`sdk_modulo::with_nounwind`).
    // Only on Itanium targets: on x64 COFF every function has an unwind
    // table anyway, and Windows was not measured.
    let llvm_ir = if cfg!(unix) && !excecoes_por_tabelas && !raizes_por_mapas { sdk_modulo::with_nounwind(&llvm_ir) } else { llvm_ir };
    // The thread context in a `thread_local` variable of the module, in the default
    // AOT (`sdk_modulo::with_context_in_tls`): inline in a small
    // program, a call in a large one.
    let llvm_ir = if cfg!(unix) && prune && !excecoes_por_tabelas && !raizes_por_mapas && !arc_memory {
        let mode = if large { sdk_modulo::ContextInTls::PerCall } else { sdk_modulo::ContextInTls::Inline };
        sdk_modulo::with_context_in_tls(&llvm_ir, mode, "df.ctx.tls.programa")
    } else {
        llvm_ir
    };
    let llvm_duration = t_llvm.elapsed();

    let sdk_byte_count = sdk_bytes(&llvm_ir, &program);
    Ok(IrEmitido {
        texto: llvm_ir,
        tempos: EmissionTimes {
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
pub fn compilar(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    compile_with(entry, exit, options)
}

/// [`compilar`] choosing the SDK (see [`emit_ir_with`]).
pub fn compile_with(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    let t_total = Instant::now();

    let ir = emit_ir_with(entry, options)?;

    // 4. Clang and linking
    let driver_opts = driver::NativeDriverOptions {
        clang: driver::NativeDriverOptions::default().clang,
        optimize: options.optimize,
        timings: options.timings,
        debug_info: options.depuracao,
        cpu: options.cpu,
    };

    let linkage = driver::compile_and_link(&ir.texto, exit, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Native Compilation Timings ---");
        ir.imprimir_tempos();
        let origin = if linkage.objeto_do_cache { " (cache)" } else { "" };
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
            .unwrap_or_else(|| sdk_do_dart().expect("SDK required by the emission tests"));
        assert!(
            sdk.join("libraries.json").is_file(),
            "test SDK without libraries.json: {}",
            sdk.display()
        );
        sdk
    }).as_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(entry: &Path) -> IrEmitido {
        let options = CompileOptions { sdk: Some(sdk_tests()), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        emitir_ir(entry, &options).expect("emitir IR")
    }

    #[test]
    fn bigint_operator_excludes_smi_without_removing_num_and_dynamic_path() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "BigInt big(BigInt a, BigInt b) => a - b;\nnum numero(num a, num b) => a - b;\ndynamic aberto(dynamic a, dynamic b) => a - b;\nBigInt composto(BigInt a, BigInt b) { a += b; return a; }\nnum compostoNumero(num a, num b) { a += b; return a; }\nvoid main() { print(big(BigInt.from(9), BigInt.from(4))); print(numero(9, 4)); print(aberto(9, 4)); print(composto(BigInt.from(9), BigInt.from(4))); print(compostoNumero(9, 4)); }\n").unwrap();
            let ir = emit(&entry).texto;
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
        assert!(a.texto.contains("@dart_main"), "{}", a.texto);
        assert!(a.sdk_byte_count <= a.texto.len());
        assert_eq!(a.texto, emit(&entry).texto);
        let parallel: Vec<String> = std::thread::scope(|s| {
            let alcas: Vec<_> = (0..4)
                .map(|_| {
                    std::thread::Builder::new()
                        .stack_size(64 << 20)
                        .spawn_scoped(s, || emit(&entry).texto)
                        .unwrap()
                })
                .collect();
            alcas.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for p in parallel {
            assert_eq!(p, a.texto);
        }
    }

    /// An unsupported construct is an `Err` of `emitir_ir`, with all the
    /// diagnostics: no IR, and no executable that only prints them.
    #[test]
    fn unsupported_construct_is_error_with_all_diagnostics() {
        sdk_tests();
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() {\n  int? k = 1;\n  var a = {?k: 1};\n  var b = {?k: 2};\n  print(a.length + b.length);\n}\n").unwrap();
        let options = CompileOptions { sdk: Some(sdk_tests()), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        let error = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || emitir_ir(&entry, &options).map(|ir| ir.texto))
            .unwrap()
            .join()
            .unwrap()
            .expect_err("null-aware map entry is rejected in native");
        let lines: Vec<&str> = error.lines().collect();
        assert_eq!(lines[0], "erro de compilação: não suportado no backend nativo: entrada de mapa null-aware", "{error}");
        assert_eq!(lines[1], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:3:11)", "{error}");
        assert_eq!(lines[2], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:4:11)", "{error}");
        assert_eq!(construtos_do_erro(&error), ["entrada de mapa null-aware", "entrada de mapa null-aware"]);
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
                    emit(&entry).texto
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let (ia, ib, ia2) = (emit_into(a), emit_into(b), emit_into(a));
        let (sa, sb) = (symbols(&ia), symbols(&ib));
        for s in ["df.main$2edart..a", "df.main$2edart.C.m", "df.main$2edart.C.s", "df.main$2edart.C.new", "df.main$2edart..g", "dart_main"] {
            assert!(sa.contains(s), "{s} in {sa:?}");
        }
        assert!(sa.is_subset(&sb), "symbols of A that disappeared in B: {:?}", sa.difference(&sb).collect::<Vec<_>>());
        // The same program in another directory gives the same IR, except for the
        // data constants (`@df.arr.*`): the RTI table carries the `file:` URI
        // of the library of each class (`dart:mirrors`), which depends
        // on the directory by definition. The table goes compressed
        // (`dartforge_runtime::rti_table`), so the size of a
        // constant (the `[N x i64]` type) and the argument of the call that
        // registers the table may also change.
        let (la, la2): (Vec<_>, Vec<_>) = (ia.lines().collect(), ia2.lines().collect());
        assert_eq!(la.len(), la2.len(), "the same program in another directory gives the same IR");
        let rti_table = |l: &str| l.contains("@dartforge_rti_iniciar_tabela(");
        for (x, y) in la.iter().zip(&la2) {
            if x != y {
                assert!(
                    (x.starts_with("@df.arr.") && y.starts_with("@df.arr.")) || (rti_table(x) && rti_table(y)),
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
        assert!(!synchronous.texto.contains("call void @dartforge_laco_de_eventos"));
        assert!(!synchronous.texto.contains("@df.dart$3aasync"));
        assert_eq!(synchronous.sdk_byte_count, 0);

        let asynchronous = emit_source("Future<int> f() async { await null; return 2; }\nFuture<void> main() async { print(await f()); }\n");
        assert!(asynchronous.texto.contains("call void @dartforge_laco_de_eventos(ptr @dartforge_chamar_dart0)"));
        assert!(asynchronous.texto.contains("define i64 @df.main$2edart..f$async$q"), "the body of the state machine");
        // `dart:async` comes from the cached SDK module (`sdk_modulo.rs`): the IR
        // of the program only calls it, it does not define its functions (`sdk_byte_count`
        // may be 0).
        assert!(asynchronous.texto.contains("@df.dart$3aasync.._asyncAwait("));
    }

    #[test]
    fn error_constructs_ignores_non_constructs() {
        let text = "[compile-native] erro de compilação: não suportado no backend nativo: membro `hash`\n\
                     erro de compilação: não suportado no backend nativo: membro `hash`\n  \
                     não suportado no backend nativo: membro `hash` (a.dart:1:2)\n  \
                     verificador da HIR (main): tag 3 para um valor I64\n  \
                     não suportado no backend nativo: chamada `sort` (a.dart:3:4)";
        assert_eq!(construtos_do_erro(text), ["membro `hash`", "chamada `sort`"]);
        assert!(construtos_do_erro("error loading the program: x").is_empty());
    }
}
