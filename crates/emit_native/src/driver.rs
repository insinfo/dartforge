//! Compilation driver LLVM IR -> Clang -> link with the cached runtime.

use crate::cache::{RuntimeCache, native_cache_dir};
use crate::generator::{Generation, Generator};
use crate::object_cache::{self, ObjectCache};
use crate::target::Os;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NativeDriverOptions {
    pub clang: PathBuf,
    pub optimize: bool,
    pub timings: bool,
    /// J05: the object carries the line tables; on Windows the link generates the
    /// PDB (`/DEBUG`), which is where the debugger looks for them.
    pub debug_info: bool,
    /// The target CPU (`--cpu`, `generator::Cpu`); `None`, the baseline of the target.
    pub cpu: Option<crate::generator::Cpu>,
    /// SDK library directory matching the emitted IR. Raw IR callers may use discovery.
    pub sdk_lib: Option<PathBuf>,
    /// Asserts and `dart.vm.product` of the emitted IR ([`crate::DartFlags`]):
    /// the linked SDK objects come out with the same ones (the cache key).
    pub flags: crate::DartFlags,
}

impl Default for NativeDriverOptions {
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG").map_or_else(default_clang, PathBuf::from),
            optimize: false,
            timings: false,
            debug_info: false,
            cpu: None,
            sdk_lib: None,
            flags: crate::DartFlags::DEVELOPMENT,
        }
    }
}

/// Clang when `DARTFORGE_CLANG` is not set: the distribution one
/// (`lib/llvm/bin`), the `bin` of `DARTFORGE_LLVM_DIR`/`LLVM_SYS_221_PREFIX`,
/// and finally the `clang` on `PATH` (`scripts/env.ps1` points to the one of the
/// development machine on Windows).
fn default_clang() -> PathBuf {
    if let Some(c) = dartforge_elements::distribution::llvm_tool("clang") {
        return c;
    }
    for var in ["DARTFORGE_LLVM_DIR", "LLVM_SYS_221_PREFIX"] {
        if let Some(d) = std::env::var_os(var) {
            let c = PathBuf::from(d)
                .join("bin")
                .join(crate::target::clang_name());
            if c.is_file() {
                return c;
            }
        }
    }
    PathBuf::from(crate::target::clang_name())
}

#[derive(Debug, Clone, Default)]
pub struct TimingsReport {
    pub frontend: Duration,
    pub hir: Duration,
    pub llvm_ir: Duration,
    pub clang: Duration,
    pub link: Duration,
    pub total: Duration,
    pub peak_memory_bytes: usize,
}

/// Clang and link times, and whether the object came from the cache (then `clang` is
/// only the time to find the entry).
#[derive(Debug, Clone, Copy, Default)]
pub struct LinkTimes {
    pub clang: Duration,
    pub link: Duration,
    pub cache_object: bool,
}

pub fn compile_and_link(
    llvm_ir: &str,
    output: &Path,
    options: &NativeDriverOptions,
) -> Result<LinkTimes, String> {
    crate::linker_windows::Subsystem::from_environment()?;
    let wasm = crate::target::is_wasm();
    // A program with the source SDK (P5c): the entry calls the registration of the
    // SDK libraries, which live in the cached objects.
    let sdk = if llvm_ir.contains("declare void @df.register.") {
        let dir = match &options.sdk_lib {
            Some(directory) => directory.clone(),
            None => crate::dart_sdk()?,
        };
        // WebAssembly links the SDK statically in every profile.
        let profile = if options.optimize || wasm {
            crate::sdk_module::SdkProfile::Production
        } else {
            crate::sdk_module::SdkProfile::Development
        };
        let sdk =
            crate::sdk_module::compiled_sdk_for(&dir, &options.clang, profile, options.flags)?;
        if let Some(t) = sdk.cold
            && options.timings
        {
            eprintln!("  Cold SDK:  {t:?} (compiled once per content)");
        }
        Some(sdk)
    } else {
        None
    };
    // With the source SDK there are two profiles (docs/NATIVO-PLANO.md §7.9):
    // * development and test: the runtime and the SDK live in the cached DLL, and the
    //   executable links only the program object and the import library
    //   (fast link; the DLL goes next to the executable);
    // * production (`optimize`): ONE self-contained executable — the program
    //   object, the SDK objects (cached, compiled with
    //   `-ffunction-sections`) and the static runtime, with `/OPT:REF` removing what
    //   the program does not reach.
    let production = (options.optimize || wasm) && sdk.is_some();
    // Production: the method tables assembled in the program module, with only
    // the live pairs (docs/NATIVO-PODA-DE-TABELAS.md). The cached object is
    // that of this already assembled IR.
    let assembled;
    let llvm_ir = match &sdk {
        Some(s) if production => {
            assembled =
                crate::pruning::assemble_production(llvm_ir, &s.summaries, options.timings)?;
            assembled.as_str()
        }
        _ => llvm_ir,
    };
    // A WebAssembly module always links its own runtime statically
    // (`RuntimeCache::for_wasm`), the SDK objects included.
    let (link_with, sdk_objects): (Linkage, Vec<PathBuf>) = match &sdk {
        Some(s) if production && wasm => (
            Linkage::Production(RuntimeCache::for_wasm()?.lib_path),
            s.objects.clone(),
        ),
        Some(s) if production => (
            Linkage::Production(crate::cache::RuntimeCache::for_production()?.lib_path),
            s.objects.clone(),
        ),
        Some(s) => (Linkage::SharedSdk(s.import.clone()), Vec::new()),
        None if wasm => (
            Linkage::Runtime(RuntimeCache::for_wasm()?.lib_path),
            Vec::new(),
        ),
        None => (
            Linkage::Runtime(RuntimeCache::get_or_compile()?.lib_path),
            Vec::new(),
        ),
    };

    // A safe temporary directory in the target. With the object cache it is only
    // used with DARTFORGE_KEEP_IR or if the link rejects the cached object,
    // and it is only created in those cases.
    let staging = output.parent().unwrap_or(Path::new(".")).join(".df_tmp");
    let create_staging = || {
        std::fs::create_dir_all(&staging).map_err(|e| {
            format!(
                "could not create temporary directory {}: {e}",
                staging.display()
            )
        })
    };

    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ll_file = staging.join(format!("{stem}.ll"));
    let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    // Production with the source SDK: bitcode, optimized together with the SDK one at
    // link time (LTO).
    let generator = Generator::choose(&options.clang);
    // `DARTFORGE_PRODUCTION_NO_LTO=1` (measurement, docs/NATIVO-PRODUCAO-GRANDE.md
    // §3.2): the parts of the program go as `-O2` objects, and only the SDK
    // bitcode goes through the linker's LTO.
    let without_lto =
        production && std::env::var("DARTFORGE_PRODUCTION_NO_LTO").is_ok_and(|v| v == "1");
    let generation = Generation {
        cpu: options.cpu,
        ..Generation::of_program(options.optimize, production && !without_lto)
    };
    if keep_ir {
        // The copy in `.df_tmp` only exists for those who asked for DARTFORGE_KEEP_IR
        // (the harness's `determinism --run` reads those copies).
        create_staging()?;
        std::fs::write(&ll_file, llvm_ir)
            .map_err(|e| format!("failed to write LLVM IR to {}: {e}", ll_file.display()))?;
    }

    // Phase 1': the large module goes in parts (C9, `partition.rs`): each one
    // an object, with its own key in the cache, compiled a few at a time.
    let t_clang = Instant::now();
    let cache = ObjectCache::from_env();
    // Also in production: each part becomes a bitcode, and the linker's ThinLTO
    // optimizes them together (the IR already assembled with the pruned tables).
    if let Some((threshold, target)) = crate::partition::limits()
        && llvm_ir.len() > threshold
    {
        let plan = crate::partition::make_plan(llvm_ir, target);
        // Production in parts: the bitcode has to carry the ThinLTO summary,
        // which only Clang writes (the embedded generator does not: `lld` would do
        // the full LTO in a single module, on one thread — 1 600 s on
        // new_sali/backend, docs/NATIVO-PRODUCAO-GRANDE.md §1.2).
        // `DARTFORGE_GENERATOR` chooses by hand.
        let parts_generator =
            if production && !without_lto && std::env::var_os("DARTFORGE_GENERATOR").is_none() {
                Generator::clang_if_any(&options.clang)
            } else {
                generator.clone()
            };
        let objects = generate_parts(&plan, &parts_generator, generation, cache, &staging, &stem)?;
        drop(plan);
        let clang_duration = t_clang.elapsed();
        let t_link = Instant::now();
        let mut extras: Vec<PathBuf> = objects[1..].to_vec();
        extras.extend(sdk_objects.iter().cloned());
        LINK_IN_PARTS.set(true);
        // Roots by maps in production in parts: the LTO leaves the linker (
        // `lld-link` does not run the maps pass) and becomes the distributed
        // ThinLTO, which hands over native objects with the compact map
        // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Step 3).
        let distributed = cfg!(feature = "embedded-llvm")
            && production
            && !without_lto
            && matches!(crate::target::target_os(), Os::Windows | Os::Linux)
            && crate::target::roots_by_maps()?;
        let linked_done = if distributed {
            link_distributed(
                &options.clang,
                &objects,
                &sdk_objects,
                &link_with,
                output,
                options.debug_info,
                options.cpu,
                &staging,
                cache,
            )
        } else {
            link(
                &options.clang,
                &objects[0],
                &extras,
                &link_with,
                output,
                options.debug_info,
                options.cpu,
            )
        };
        LINK_IN_PARTS.set(false);
        linked_done?;
        if let Some(s) = sdk.as_ref().filter(|_| !production) {
            let dest = output
                .parent()
                .unwrap_or(Path::new("."))
                .join(s.dll.file_name().unwrap_or_default());
            if !dest.is_file() && std::fs::hard_link(&s.dll, &dest).is_err() {
                std::fs::copy(&s.dll, &dest)
                    .map_err(|e| format!("could not put the SDK DLL in {}: {e}", dest.display()))?;
            }
        }
        if options.timings {
            eprintln!("  Parts:     {} objects", objects.len());
        }
        if cache.is_none() {
            for o in &objects {
                let _ = std::fs::remove_file(o);
            }
        }
        return Ok(LinkTimes {
            clang: clang_duration,
            link: t_link.elapsed(),
            cache_object: false,
        });
    }

    // Roots by maps in single-module production, through the distributed ThinLTO,
    // which runs the maps pass after the link optimization and hands over
    // objects with the compact map (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §3.4): on Windows, because the `lld-link` LTO does not run the pass; on
    // Linux, also because the LLVM map has absolute relocations in a
    // read-only section, which `ld.lld` rejects in a PIE executable (Step 4).
    // `DARTFORGE_LTO_DISTRIBUTED=1` (measurement, §8): the same path with the
    // shadow stack, to compare the root modes in the same pipeline.
    let single_distributed = cfg!(feature = "embedded-llvm")
        && production
        && !without_lto
        && matches!(crate::target::target_os(), Os::Windows | Os::Linux)
        && (crate::target::roots_by_maps()?
            || std::env::var("DARTFORGE_LTO_DISTRIBUTED").is_ok_and(|v| v == "1"));
    // The bitcode that goes to the distributed ThinLTO has to carry the ThinLTO
    // summary, which only Clang writes: without it, the backend with the index of the
    // link returns an empty module. `DARTFORGE_GENERATOR` chooses by hand.
    let generator = if single_distributed && std::env::var_os("DARTFORGE_GENERATOR").is_none() {
        Generator::Clang(options.clang.clone())
    } else {
        generator
    };

    // Phase 1: the generator compiles LLVM IR -> object, or the cache already has the
    // object of this IR with this generator and this generation.
    let obj_staging = || -> Result<PathBuf, String> {
        create_staging()?;
        Ok(staging.join(format!("{stem}.{}", crate::target::ext_object())))
    };
    let (mut obj_file, mut from_cache) = match cache {
        Some(c) => {
            let description = generation.description();
            let key = object_cache::key(llvm_ir, &generator.identity()?, &[description.as_str()]);
            let (obj, hit) =
                c.get_or_create(key, |tmp| generator.generate(llvm_ir, generation, tmp))?;
            (obj, Some((c, key, hit)))
        }
        None => {
            let obj = obj_staging()?;
            generator.generate(llvm_ir, generation, &obj)?;
            (obj, None)
        }
    };
    let clang_duration = t_clang.elapsed();

    // Phase 2: link the object with the static runtime
    let t_link = Instant::now();
    let link_object = |obj: &Path| {
        if single_distributed {
            link_distributed(
                &options.clang,
                std::slice::from_ref(&obj.to_path_buf()),
                &sdk_objects,
                &link_with,
                output,
                options.debug_info,
                options.cpu,
                &staging,
                cache,
            )
        } else {
            link(
                &options.clang,
                obj,
                &sdk_objects,
                &link_with,
                output,
                options.debug_info,
                options.cpu,
            )
        }
    };
    let mut linked_done = link_object(&obj_file);
    if linked_done.is_err()
        && let Some((c, key, true)) = from_cache
    {
        // A cached object the linker rejects cannot stay there: it leaves the
        // cache, and the link is redone once with a new object.
        c.remove(key);
        obj_file = obj_staging()?;
        generator.generate(llvm_ir, generation, &obj_file)?;
        from_cache = None;
        linked_done = link_object(&obj_file);
    }
    linked_done?;
    if let Some(s) = sdk.as_ref().filter(|_| !production) {
        // The shared library next to the executable (Windows looks there
        // first; on Linux and macOS the executable carries the `rpath` of its own
        // directory): a physical link, without copying; a copy only if the volume
        // is another.
        let dest = output
            .parent()
            .unwrap_or(Path::new("."))
            .join(s.dll.file_name().unwrap_or_default());
        if !dest.is_file() && std::fs::hard_link(&s.dll, &dest).is_err() {
            std::fs::copy(&s.dll, &dest)
                .map_err(|e| format!("could not put the SDK DLL in {}: {e}", dest.display()))?;
        }
    }
    let link_duration = t_link.elapsed();

    // Cleanup of temporary files (kept if DARTFORGE_KEEP_IR is
    // set); a cached object is never deleted here.
    for p in files_to_remove(&ll_file, &obj_file, &native_cache_dir(), keep_ir) {
        let _ = std::fs::remove_file(p);
    }

    Ok(LinkTimes {
        clang: clang_duration,
        link: link_duration,
        cache_object: matches!(from_cache, Some((_, _, true))),
    })
}

/// The link of production in parts with roots by maps: the distributed ThinLTO
/// (`distributed_lto.rs`) replaces each bitcode with its native object,
/// and the linker receives only objects, without LTO. The indices ask for the whole
/// link, so the runtime library enters the list and leaves it before
/// the final link (which adds it again).
#[cfg(feature = "embedded-llvm")]
#[allow(clippy::too_many_arguments)]
fn link_distributed(
    clang: &Path,
    parts: &[PathBuf],
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
    staging: &Path,
    cache: Option<&'static ObjectCache>,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = parts.to_vec();
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let mut natives = crate::distributed_lto::objects(&crate::distributed_lto::Request {
        clang,
        entries: &entries,
        cpu,
        staging,
        cache,
    })?;
    natives.pop();
    let Some((first_one, rest)) = natives.split_first() else {
        return Err("the distributed LTO returned no object".to_string());
    };
    NO_LTO_AT_LINK.set(true);
    let r = link(clang, first_one, rest, linkage, output, debug_info, cpu);
    NO_LTO_AT_LINK.set(false);
    r
}

#[cfg(not(feature = "embedded-llvm"))]
#[allow(clippy::too_many_arguments)]
fn link_distributed(
    _clang: &Path,
    _parts: &[PathBuf],
    _sdk: &[PathBuf],
    _link: &Linkage,
    _output: &Path,
    _debug: bool,
    _cpu: Option<crate::generator::Cpu>,
    _staging: &Path,
    _cache: Option<&'static ObjectCache>,
) -> Result<(), String> {
    Err("the distributed LTO requires dartforge's embedded generator".to_string())
}

thread_local! {
    /// The link in progress on this thread is that of the program in parts (ThinLTO).
    static LINK_IN_PARTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The link in progress receives already optimized native objects (the distributed
    /// ThinLTO, `distributed_lto.rs`): the linker does no LTO.
    static NO_LTO_AT_LINK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The optimization level of the production LTO (`DARTFORGE_LTO_LEVEL`, 0–3):
/// `-O1` for the program in parts (ThinLTO; docs/NATIVO-PRODUCAO-GRANDE.md
/// §6.2: optimization falls from 275 to 150 s of CPU on new_sali/backend,
/// `bench/performance` does not change), `-O2` for the rest.
pub fn lto_level() -> u8 {
    let pattern = if LINK_IN_PARTS.get() { 1 } else { 2 };
    std::env::var("DARTFORGE_LTO_LEVEL")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .filter(|n| *n <= 3)
        .unwrap_or(pattern)
}

/// Whether the LTO of this link runs LLVM's *machine outliner* on every
/// function (`-enable-machine-outliner=always`): the large program (in
/// parts) in the default mode — exceptions by check, shadow stack, tracing
/// memory, without the symbolic stack trace — on aarch64. Dart code
/// repeats the same sequences a lot (the shadow-stack frame, the pending
/// exception check, the assembly of the arguments of a selector) and the
/// outliner replaces them with a call: `new_sali/backend` falls from 53.1 to
/// 38.5 MB (`__text` 44.6 → 30.0 MB). It stays out wherever the return address
/// of a call matters: exceptions by tables and roots by
/// maps unwind the frames, and the symbolic stack trace finds the position by the
/// label before the call (`llvm/stack_trace.rs`); a call moved into the
/// extracted function would have its return outside its own function. A small program
/// does not pay the extra jump (its LTO is already a single module, and its size already
/// is below `dart compile exe`). `DARTFORGE_OUTLINER=0` turns it off;
/// `=1` also turns it on for a small program (measurement).
pub fn outliner_at_link() -> bool {
    let request = std::env::var("DARTFORGE_OUTLINER").ok();
    if request.as_deref() == Some("0") {
        return false;
    }
    let default_mode = !crate::target::table_exceptions().unwrap_or(true)
        && !crate::target::roots_by_maps().unwrap_or(true)
        && !crate::target::symbolic_stack_trace().unwrap_or(true)
        && !crate::target::arc_memory().unwrap_or(true);
    crate::target::is_aarch64()
        && default_mode
        && (LINK_IN_PARTS.get() || request.as_deref() == Some("1"))
}

/// Whether the production LTO runs LLVM's `MergeFunctions` pass
/// (`-mllvm -enable-merge-functions`): identical bodies become one function
/// and the others thunks to it, so every tear-off keeps its own address —
/// unlike the linker's ICF, which has no `addrsig` to respect in this pipeline
/// (`linker_macos.rs`). Measured on the File Explorer (18 ThinLTO parts, 18
/// October 2026): `__text` −293,340 B with the same cold link time; bench.dart
/// −16.6 KB with the kernels unchanged; corpus/native/80 equal to the VM. The
/// option is global to LLVM and invisible to the ThinLTO cache key, so each
/// linker keeps these modules in a subdirectory of their own. It stays out of
/// the symbolic stack trace (§13.14): a body folded into a thunk would report
/// the frame of the function it was merged into. `DARTFORGE_NO_MERGE_FUNCTIONS=1`
/// turns it off (measurement).
pub fn merge_functions_at_link() -> bool {
    if std::env::var("DARTFORGE_NO_MERGE_FUNCTIONS").is_ok_and(|v| v == "1") {
        return false;
    }
    !crate::target::symbolic_stack_trace().unwrap_or(true)
}

/// The directory of the ThinLTO cache of the production link (next to the object
/// cache); `None` with `DARTFORGE_NO_THINLTO_CACHE=1` or without an object
/// cache.
pub fn thinlto_cache() -> Option<PathBuf> {
    if std::env::var("DARTFORGE_NO_THINLTO_CACHE").is_ok_and(|v| v == "1") {
        return None;
    }
    ObjectCache::from_env()?;
    let d = native_cache_dir().join("thinlto");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// How many code generations (parts, or ThinLTO modules in the linker) at
/// the same time: `DARTFORGE_PARALLEL_PARTS`, otherwise half the threads of the
/// machine, at most 3.
pub fn generation_tasks() -> usize {
    std::env::var("DARTFORGE_PARALLEL_PARTS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).clamp(1, 3))
        })
        .max(1)
}

/// The objects of the parts of a large module (C9), in order: each part
/// assembled only when its turn comes, through the object cache (or in `staging`),
/// `DARTFORGE_PARALLEL_PARTS` at a time (default: half the threads of the
/// machine, at most 3 — the memory of a generator, ~44 bytes per byte of
/// IR at `-O0`, is the measure, not the cores).
fn generate_parts(
    plan: &crate::partition::Plan<'_>,
    generator: &Generator,
    generation: Generation,
    cache: Option<&'static ObjectCache>,
    staging: &Path,
    stem: &str,
) -> Result<Vec<PathBuf>, String> {
    let parallel = generation_tasks();
    let identity = generator.identity()?;
    let description = generation.description();
    if cache.is_none() {
        std::fs::create_dir_all(staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let next_one = std::sync::atomic::AtomicUsize::new(0);
    let n = plan.len();
    let results: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> =
        std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..parallel.min(n) {
            s.spawn(|| {
                loop {
                    let i = next_one.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= n {
                        break;
                    }
                    let text = plan.part(i);
                    let ir = text.as_str();
                    let r = match cache {
                        Some(c) => {
                            let key = object_cache::key(ir, &identity, &[description.as_str()]);
                            c.get_or_create(key, |tmp| generator.generate(ir, generation, tmp))
                                .map(|(o, _)| o)
                        }
                        None => {
                            let o = staging
                                .join(format!("{stem}.parte{i}.{}", crate::target::ext_object()));
                            generator.generate(ir, generation, &o).map(|()| o)
                        }
                    };
                    results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("part {i} was not generated"))))
        .collect()
}

/// What the program object is linked with.
enum Linkage {
    /// Without the source SDK: the static runtime (with the C `main`).
    Runtime(PathBuf),
    /// Development with the source SDK: the shared library (runtime
    /// and SDK) — on Windows, the import library of the DLL; on the others, the
    /// `.so`/`.dylib` itself.
    SharedSdk(PathBuf),
    /// Production with the source SDK: the static runtime without the C `main`; the
    /// SDK objects come separately.
    Production(PathBuf),
}

impl Linkage {
    fn library(&self) -> &Path {
        match self {
            Linkage::Runtime(p) | Linkage::SharedSdk(p) | Linkage::Production(p) => p,
        }
    }
}

/// Linking on Linux: `ld.lld` directly, with the link sysroot
/// (`linker.rs`) — without the Clang driver nor GCC on the machine.
fn link_on_linux(
    clang: &Path,
    obj: &Path,
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
) -> Result<(), String> {
    use crate::linker;
    let sysroot = linker::SysrootLinux::locate(clang)?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    linker::link(
        &linker::ld_lld(clang),
        sysroot,
        &linker::Linkage {
            product: linker::Product::Executable,
            entries,
            origin_rpath: matches!(linkage, Linkage::SharedSdk(_)),
            lto: production,
            cpu: cpu.map(crate::generator::Cpu::name),
            prune: production,
            keep_debug_info: debug_info,
            exit: output,
        },
    )
}

/// Linking on Windows: `lld-link` directly, with the import libraries
/// and the minimal CRT generated by dartforge (`linker_windows.rs`) —
/// without Visual Studio nor the Windows SDK on the machine (N15).
fn link_on_windows(
    clang: &Path,
    obj: &Path,
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
) -> Result<(), String> {
    use crate::linker_windows as lw;
    let sysroot = lw::SysrootWindows::locate(clang)?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    lw::link(
        &lw::lld_link(clang),
        sysroot,
        &lw::Linkage {
            product: lw::Product::Executable {
                subsystem: lw::Subsystem::from_environment()?,
            },
            entries,
            lto: production && !NO_LTO_AT_LINK.get(),
            cpu: cpu.map(crate::generator::Cpu::name),
            prune: production,
            debug_info,
            exit: output,
        },
    )
}

/// Linking on macOS: `ld64.lld` directly, with the `.tbd` files of the link
/// sysroot (`linker_macos.rs`) — without `xcrun`, Apple's `ld` nor the
/// Command Line Tools on the machine (N16).
fn link_on_macos(
    clang: &Path,
    obj: &Path,
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
) -> Result<(), String> {
    use crate::linker_macos as lm;
    let sysroot = lm::SysrootMacos::locate()?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    lm::link(
        &lm::ld64_lld(clang),
        sysroot,
        &lm::Linkage {
            product: lm::Product::Executable,
            entries,
            executable_rpath: matches!(linkage, Linkage::SharedSdk(_)),
            lto: production,
            cpu: cpu.map(crate::generator::Cpu::name),
            prune: production,
            keep_debug_info: debug_info,
            exit: output,
        },
    )
}

/// Links the program object: the LLVM linker directly on each system, with
/// what the system requires coming from dartforge (no C driver, no
/// system toolchain). The `lld` is the one from the same LLVM as the generator — the
/// production bitcode (LTO) is only read by the same version.
fn link(
    clang: &Path,
    obj: &Path,
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
) -> Result<(), String> {
    match crate::target::target_os() {
        Os::Linux => link_on_linux(clang, obj, sdk, linkage, output, debug_info, cpu),
        // The link from before N15 (the Clang driver with `link.exe` and the
        // Visual C++ CRT), only for comparison: it is the reference of
        // `no-toolchain.yml`, in the same build.
        Os::Windows if link_with_clang() => {
            link_old_with_clang(clang, obj, sdk, linkage, output, debug_info, cpu)
        }
        Os::Windows => link_on_windows(clang, obj, sdk, linkage, output, debug_info, cpu),
        Os::MacOs => link_on_macos(clang, obj, sdk, linkage, output, debug_info, cpu),
        Os::Wasm => {
            let mut objects = vec![obj.to_path_buf()];
            objects.extend(sdk.iter().cloned());
            crate::linker_wasm::link(&crate::linker_wasm::Linkage {
                lld: &crate::linker_wasm::lld(clang),
                objects: &objects,
                runtime: linkage.library(),
                output,
                debug_info,
            })
        }
    }
}

/// Whether the link on Windows is the one from before N15 (`DARTFORGE_LINK_WITH_CLANG`
/// set): the executable and the DLL of the source SDK.
pub fn link_with_clang() -> bool {
    cfg!(windows) && std::env::var_os("DARTFORGE_LINK_WITH_CLANG").is_some()
}

/// The link through the Clang driver, as it was before N15 (the Visual Studio
/// toolchain on the machine): `DARTFORGE_LINK_WITH_CLANG=1` on Windows. Only
/// to compare with the link without the system toolchain.
fn link_old_with_clang(
    clang: &Path,
    obj: &Path,
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::generator::Cpu>,
) -> Result<(), String> {
    if crate::target::target_os() == Os::Linux {
        return link_on_linux(clang, obj, sdk, linkage, output, debug_info, cpu);
    }
    let mut cmd = std::process::Command::new(clang);
    cmd.arg(obj).args(sdk).arg(linkage.library());
    let target_os = crate::target::target_os();
    match linkage {
        Linkage::SharedSdk(_) if target_os == Os::Windows => {
            // Executable of the source SDK (development): the runtime is in the
            // DLL, which uses the dynamic CRT (that of `rustc`); the executable uses the
            // same one.
            cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt"]);
        }
        Linkage::SharedSdk(_) => {
            // The shared library goes next to the executable; the `rpath`
            // points the loader to the directory of the executable itself.
            cmd.arg(if target_os == Os::MacOs {
                "-Wl,-rpath,@executable_path"
            } else {
                "-Wl,-rpath,$ORIGIN"
            });
            // Clang adds `-lSystem` on macOS: without the SDK root
            // (`-isysroot`), the linker does not find it.
            cmd.args(crate::target::link_arguments());
        }
        Linkage::Production(_) => {
            // Production with the source SDK: everything static in the executable,
            // ThinLTO between the program and the SDK (lld), and the linker removes the
            // sections nothing reaches.
            // The lld has to be from the same LLVM as Clang (ThinLTO bitcode is only
            // read by the same version). Clang with ThinLTO requires the literal
            // `lld` in `-fuse-ld=` (an absolute path gives `clang: error: LTO
            // requires -fuse-ld=lld` on Windows); so the sibling bin directory
            // of Clang itself goes into PATH only for this spawn, so that the resolved
            // `lld` is of the same version. Rejecting its absence avoids silently falling
            // into a wrong lld on PATH (measured: LLVM 20 reading
            // bitcode 22 — `Unknown attribute kind (105)`).
            let lld_name = match target_os {
                Os::Windows => "lld-link.exe",
                Os::Linux => "ld.lld",
                Os::MacOs => "ld64.lld",
                Os::Wasm => unreachable!("the Clang-driver link is Windows only"),
            };
            let lld = clang.with_file_name(lld_name);
            if !lld.is_file() {
                return Err(format!(
                    "ThinLTO requires {lld_name} next to {}",
                    clang.display()
                ));
            }
            if let Some(bin) = clang.parent()
                && !bin.as_os_str().is_empty()
            {
                let mut paths = vec![bin.to_path_buf()];
                if let Some(current) = std::env::var_os("PATH") {
                    paths.extend(std::env::split_paths(&current));
                }
                if let Ok(new) = std::env::join_paths(paths) {
                    cmd.env("PATH", new);
                }
            }
            cmd.args(["-fuse-ld=lld", "-flto=thin"]);
            cmd.arg(format!("-O{}", lto_level()));
            // The ThinLTO cache: on a relink, only the modules that changed
            // (or whose imports changed) go through optimization
            // and code generation again (docs/NATIVO-PRODUCAO-GRANDE.md).
            if let Some(d) = thinlto_cache() {
                match target_os {
                    Os::Windows => {
                        cmd.arg(format!("-Wl,/lldltocache:{}", d.display()));
                        cmd.arg("-Wl,/lldltocachepolicy:cache_size_bytes=4g:prune_after=168h");
                    }
                    Os::Linux => {
                        cmd.arg(format!("-Wl,--thinlto-cache-dir={}", d.display()));
                    }
                    Os::MacOs => {
                        cmd.arg(format!("-Wl,-cache_path_lto,{}", d.display()));
                    }
                    Os::Wasm => unreachable!("the Clang-driver link is Windows only"),
                }
            }
            // The target CPU of the LTO code generation (`--cpu`).
            if let Some(c) = cpu {
                cmd.arg(format!("-march={}", c.name()));
            }
            cmd.args(crate::target::link_arguments());
            // The bitcode of the embedded generator has no ThinLTO summary:
            // `lld` does the full LTO, and its code generation is split
            // into parallel partitions (Mach-O does not have the option).
            let partitions = std::thread::available_parallelism()
                .map_or(4, |n| n.get())
                .clamp(2, 16);
            match target_os {
                Os::Windows => {
                    // `/OPT:REF` also turns on `lld-link`'s `/OPT:ICF`, which
                    // merges functions with identical bodies into a single address; the
                    // `/OPT:NOICF` goes right below, for every profile
                    // (`left == right` would give `true`, corpus/js 147 and 184).
                    cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt", "-Wl,/OPT:REF"]);
                    cmd.arg(format!("-Wl,/opt:lldltopartitions={partitions}"));
                }
                // Without the symbol table, like the Windows `.exe` (which leaves it
                // in the PDB): half the size in ELF (measured: 9.4 → 4.4 MB).
                Os::Linux => {
                    cmd.arg("-Wl,--gc-sections");
                    if !crate::linker::keep_symbols() && !debug_info {
                        cmd.arg("-Wl,--strip-all");
                    }
                    cmd.arg(format!("-Wl,--lto-partitions={partitions}"));
                }
                Os::MacOs => {
                    cmd.arg("-Wl,-dead_strip");
                    // `-S` strips the debug map (J05).
                    if !debug_info {
                        cmd.args(["-Wl,-S", "-Wl,-x"]);
                    }
                }
                Os::Wasm => unreachable!("the Clang-driver link is Windows only"),
            }
        }
        Linkage::Runtime(_) => {
            cmd.args(crate::target::link_arguments());
        }
    }
    if target_os == Os::Windows && debug_info {
        // The `-g` at link time becomes the linker's `/DEBUG`: the PDB next to the
        // executable, with the CodeView tables of the object.
        cmd.arg("-g");
    }
    if target_os == Os::Windows {
        // No ICF in any profile: `link.exe` without `/DEBUG` (and `lld-link`
        // with `/OPT:REF`) merges functions with identical bodies, and the tear-off of a
        // top-level function is compared by address.
        cmd.arg("-Wl,/OPT:NOICF");
    }
    let exit = cmd
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("linking with Clang at {clang:?} failed: {e}"))?;
    if !exit.status.success() {
        // What the linker said goes into the error (without it a CI failure has
        // no diagnostic).
        let text = String::from_utf8_lossy(&exit.stderr);
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(40)
            .collect();
        return Err(format!(
            "Clang failed to link the executable ({}):\n{}",
            exit.status,
            lines.join("\n")
        ));
    }
    Ok(())
}

/// What the cleanup after the link deletes: nothing with `keep_ir`, and never a
/// path inside the cache — the object that came from there belongs to everyone.
fn files_to_remove(ll: &Path, obj: &Path, dir_cache: &Path, keep_ir: bool) -> Vec<PathBuf> {
    if keep_ir {
        return Vec::new();
    }
    [ll, obj]
        .into_iter()
        .filter(|p| !p.starts_with(dir_cache))
        .map(Path::to_path_buf)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_never_erases_cache() {
        let cache = Path::new("D:/x/target/native_cache");
        let ll = Path::new("D:/x/target/differential/native/p/.df_tmp/p.ll");
        let obj_cache = cache.join("obj/ab/ab00.obj");
        let obj_local = Path::new("D:/x/target/differential/native/p/.df_tmp/p.obj");
        assert_eq!(
            files_to_remove(ll, &obj_cache, cache, false),
            vec![ll.to_path_buf()]
        );
        assert_eq!(
            files_to_remove(ll, obj_local, cache, false),
            vec![ll.to_path_buf(), obj_local.to_path_buf()]
        );
        assert!(files_to_remove(ll, obj_local, cache, true).is_empty());
    }
}
