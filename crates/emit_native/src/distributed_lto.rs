//! The **distributed** ThinLTO of production with roots by maps
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4 and Step 3, E3.1).
//!
//! `lld-link` does not accept extra passes in its LTO (`-lto-newpm-passes`
//! is an "unknown argument"), and `rewrite-statepoints-for-gc` has to run
//! **after** all the optimization, including the inlining between the program and
//! the SDK that ThinLTO does. So the LTO leaves the linker:
//!
//! 1. `lld-link /thinlto-index-only` resolves the symbols of the whole program
//!    and writes, next to each input bitcode, its index
//!    (`<entry>.thinlto.bc`): what to import from whom;
//! 2. for each bitcode, `clang -x ir <input> -fthinlto-index=<index>
//!    -emit-llvm -c` runs the ThinLTO backend (import and optimization) and
//!    returns the module **as bitcode**, with what was imported already inlined;
//! 3. the embedded LLVM applies the maps pass and the verifier, generates the
//!    object, and the converter replaces `.llvm_stackmaps` with the compact map
//!    (`gcmap.rs`) — which removes from production the ~98 bytes per record of the
//!    raw map;
//! 4. the caller links the native objects, without LTO.
//!
//! The linker's ThinLTO cache (`/lldltocache`) no longer applies; the cache is
//! the object one, keyed by the bitcode **after** the import (step
//! 2): two links in which the part imported the same things reuse
//! step 3. Step 2 always runs — the index of a part summarizes what
//! it imports, but not the content, and a key made from it alone would hand back a
//! stale object.
//!
//! Written without compiling or running: the experiment that supports steps
//! 1 and 2 is the one in `E:\dftemp\spec-maps\dtlto` (two C files); the scale
//! (the dozens of parts of a large program) was not measured.

use crate::object_cache::{self, ObjectCache};
use crate::generator::Cpu;
use crate::linker_windows as lw;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What the distributed link receives.
pub struct Request<'a> {
    pub clang: &'a Path,
    /// All the inputs of the link, in linker order: the parts of the
    /// program, the SDK modules and the runtime library.
    pub entries: &'a [PathBuf],
    pub cpu: Option<Cpu>,
    /// Where the intermediate files live.
    pub staging: &'a Path,
    pub cache: Option<&'static ObjectCache>,
}

/// Whether the file starts as LLVM bitcode (raw or in the wrapper).
fn is_bitcode(path: &Path) -> bool {
    use std::io::Read;
    let mut start = [0u8; 4];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    if f.read_exact(&mut start).is_err() {
        return false;
    }
    start == *b"BC\xC0\xDE" || start == [0xDE, 0xC0, 0x17, 0x0B]
}

/// The index that `lld-link /thinlto-index-only` writes for an input.
fn index_of(entry: &Path) -> PathBuf {
    let mut name = entry.as_os_str().to_os_string();
    name.push(".thinlto.bc");
    PathBuf::from(name)
}

fn failure_of(statement: &str, exit: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&exit.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&exit.stdout));
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
    format!("{statement} falhou ({}):\n{}", exit.status, lines.join("\n"))
}

/// The inputs of the final link: each bitcode replaced by its native object
/// (with the compact map), the others as they came, in the same order.
///
/// # Errors
/// `lld-link` did not generate the indices, Clang rejected a part, or the
/// maps pass, the verifier or the code generator failed on it.
pub fn objects(p: &Request<'_>) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(p.staging).map_err(|e| format!("{}: {e}", p.staging.display()))?;
    let bitcodes: Vec<bool> = p.entries.iter().map(|e| is_bitcode(e)).collect();
    if !bitcodes.iter().any(|b| *b) {
        return Ok(p.entries.to_vec());
    }
    // A directory for this link only. The linker writes the index of each
    // input next to it, and the SDK bitcodes belong to every compilation:
    // two links at the same time (the harness with `--jobs`) wrote each other's
    // index, and the link failed or imported wrongly. Each bitcode
    // input enters through a physical link (a copy, on another volume) in here,
    // and the intermediates stay here too.
    static SEQUENCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let private = p.staging.join(format!(
        "lto-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&private).map_err(|e| format!("{}: {e}", private.display()))?;
    let mut entries: Vec<PathBuf> = Vec::with_capacity(p.entries.len());
    for (i, e) in p.entries.iter().enumerate() {
        if !bitcodes[i] {
            entries.push(e.clone());
            continue;
        }
        let name = e.file_name().map_or_else(|| format!("e{i}"), |n| format!("e{i}-{}", n.to_string_lossy()));
        let dest = private.join(name);
        if std::fs::hard_link(e, &dest).is_err() {
            std::fs::copy(e, &dest).map_err(|x| format!("{} → {}: {x}", e.display(), dest.display()))?;
        }
        entries.push(dest);
    }
    let p = &Request { clang: p.clang, entries: &entries, cpu: p.cpu, staging: &private, cache: p.cache };
    // 1. The indices: the whole link, only up to symbol resolution.
    // `lld-link` (Windows) and `ld.lld` (Linux) accept the mode; `ld64.lld`
    // does not (on macOS production with maps uses the linker's LTO with the
    // maps pass, `linker_macos.rs`).
    let (fictitious_exit, mut statement) = match crate::target::target_os() {
        crate::target::Os::Windows => {
            let sysroot = lw::SysrootWindows::locate(p.clang)?;
            let lld = lw::lld_link(p.clang);
            let fictitious_exit = p.staging.join("indice-thinlto.exe");
            let mut args = lw::arguments(
                sysroot,
                &lw::Linkage {
                    product: lw::Product::Executable { subsystem: lw::Subsystem::from_environment()? },
                    entries: p.entries.to_vec(),
                    lto: true,
                    cpu: p.cpu.map(Cpu::name),
                    prune: false,
                    debug_info: false,
                    exit: &fictitious_exit,
                },
            );
            args.push("/thinlto-index-only".into());
            let mut c = Command::new(&lld);
            c.args(&args);
            (fictitious_exit, c)
        }
        crate::target::Os::Linux => {
            let sysroot = crate::linker::SysrootLinux::locate(p.clang)?;
            let ld = crate::linker::ld_lld(p.clang);
            let fictitious_exit = p.staging.join("indice-thinlto");
            let mut c = crate::linker::statement(
                &ld,
                sysroot,
                &crate::linker::Linkage {
                    product: crate::linker::Product::Executable,
                    entries: p.entries.to_vec(),
                    origin_rpath: false,
                    lto: true,
                    cpu: p.cpu.map(Cpu::name),
                    prune: false,
                    keep_debug_info: false,
                    exit: &fictitious_exit,
                },
            )?;
            c.arg("--thinlto-index-only");
            (fictitious_exit, c)
        }
        crate::target::Os::MacOs => {
            return Err("distributed ThinLTO does not exist on macOS (ld64.lld does not split the indices)".to_string());
        }
    };
    let exit = statement.output().map_err(|e| format!("failed to run the linker (ThinLTO indices): {e}"))?;
    if !exit.status.success() {
        return Err(failure_of("the linker (ThinLTO indices)", &exit));
    }
    // 2 and 3. The ThinLTO backend of each bitcode and the closing with the maps.
    let identity = format!("{}|{}", dartforge_llvm::identity()?, object_cache::clang_identity(p.clang)?);
    let level = format!("-O{}", crate::driver::lto_level());
    let cpu_description = p.cpu.map_or(String::new(), Cpu::description);
    let convert_map = crate::gcmap::convert_here();
    let convert_stack_trace = crate::compact_stack_trace::convert_here();
    let keep = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    let n = p.entries.len();
    let next_one = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> = std::sync::Mutex::new((0..n).map(|_| None).collect());
    let one = |i: usize| -> Result<PathBuf, String> {
        let entry = &p.entries[i];
        if !bitcodes[i] {
            return Ok(entry.clone());
        }
        let index = index_of(entry);
        let pos = p.staging.join(format!("parte{i}.pos.bc"));
        let mut statement = Command::new(p.clang);
        statement.arg(&level).args(["-x", "ir"]).arg(entry);
        // Without an index, the bitcode has no ThinLTO summary (the one from the embedded
        // generator): it is optimized alone, importing nothing.
        if index.is_file() {
            let mut option = std::ffi::OsString::from("-fthinlto-index=");
            option.push(index.as_os_str());
            statement.arg(option);
        }
        if let Some(c) = p.cpu {
            statement.arg(c.march());
        }
        statement.args(["-emit-llvm", "-c", "-o"]).arg(&pos);
        let exit = statement.output().map_err(|e| format!("failed to run {}: {e}", p.clang.display()))?;
        if !exit.status.success() {
            return Err(failure_of(&format!("the ThinLTO backend of {}", entry.display()), &exit));
        }
        let bitcode = std::fs::read(&pos).map_err(|e| format!("{}: {e}", pos.display()))?;
        let name = format!("parte{i}");
        let close = |dest: &Path| -> Result<(), String> {
            let (mut object, with_maps) = dartforge_llvm::generate_from_bitcode(&name, &bitcode, p.cpu.map(Cpu::name))?;
            if with_maps && convert_map {
                crate::gcmap::convert(&mut object)?;
            }
            // The stack trace table, when the object has it (the section is found by
            // name; without it, nothing changes).
            if convert_stack_trace {
                crate::compact_stack_trace::convert(&mut object)?;
            }
            std::fs::write(dest, object).map_err(|e| format!("failed to write {}: {e}", dest.display()))
        };
        let object = match p.cache {
            Some(c) => {
                let compact = if convert_map { "mapa-compacto" } else { "mapa-cru" };
                let stack_trace = if convert_stack_trace { "rastro-compacto" } else { "rastro-cru" };
                let key = object_cache::bytes_key(&bitcode, &identity, &["lto-distribuida", compact, stack_trace, cpu_description.as_str()]);
                c.get_or_create(key, close).map(|(o, _)| o)
            }
            None => {
                let o = p.staging.join(format!("parte{i}.{}", crate::target::ext_object()));
                close(&o).map(|()| o)
            }
        };
        if !keep {
            let _ = std::fs::remove_file(&pos);
            let _ = std::fs::remove_file(&index);
        }
        object
    };
    let parallel = crate::driver::generation_tasks();
    std::thread::scope(|s| {
        for _ in 0..parallel.min(n) {
            s.spawn(|| loop {
                let i = next_one.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let r = one(i);
                results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    if !keep {
        let _ = std::fs::remove_file(&fictitious_exit);
    }
    let objects: Result<Vec<PathBuf>, String> = results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("part {i} of the distributed LTO was not generated"))))
        .collect();
    // With the cache, the objects live in it and the directory of this link goes away;
    // without it, the objects are from here and stay for the final link.
    if !keep && p.cache.is_some() {
        let _ = std::fs::remove_dir_all(p.staging);
    }
    objects
}
