//! Linking on Linux through `ld.lld`, without the Clang driver and without
//! GCC/`libc6-dev` on the machine of whoever uses dartforge.
//!
//! The Clang driver only added to the link what glibc and libgcc
//! require: the startup objects (`Scrt1.o`, `crti.o`, `crtbeginS.o`…), the
//! dynamic loader and the system libraries. Here those files come
//! from a **link sysroot** ([`SysrootLinux`]): the distribution one
//! (`lib/sysroot/<triple>/`, copied by `dartforge pack` from the machine
//! that assembled the distribution), or, in a development tree, the
//! system ones, located through `clang -print-file-name`. The shared libraries
//! of the sysroot are only link inputs (the symbols and the
//! versions); the program loads the system ones when it runs — and dartforge runs
//! on the same minimal glibc as the machine that assembled it, so the versions of the
//! symbols are compatible.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The files of the link sysroot, in the order the Clang driver
/// passes them.
pub const SYSROOT_FILES: &[&str] = &[
    "Scrt1.o",
    "crti.o",
    "crtbeginS.o",
    "crtendS.o",
    "crtn.o",
    "libc.so.6",
    "libc_nonshared.a",
    "libm.so.6",
    "libpthread.so.0",
    "libdl.so.2",
    "librt.so.1",
    "libutil.so.1",
    "libgcc_s.so.1",
    "libgcc.a",
    // The dynamic loader (`__tls_get_addr`…), which the development `libc.so`
    // brings as `AS_NEEDED`.
    LOADER,
];

/// The dynamic loader file of the host.
const LOADER: &str = if cfg!(target_arch = "aarch64") { "ld-linux-aarch64.so.1" } else { "ld-linux-x86-64.so.2" };

/// The sysroot triple of this host.
pub fn sysroot_triple() -> &'static str {
    if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" }
}

/// The dynamic loader of the host and the `ld.lld` emulation.
fn loader_and_emulation() -> (&'static str, &'static str) {
    if cfg!(target_arch = "aarch64") {
        ("/lib/ld-linux-aarch64.so.1", "aarch64linux")
    } else {
        ("/lib64/ld-linux-x86-64.so.2", "elf_x86_64")
    }
}

/// Where the files of [`SYSROOT_FILES`] are.
#[derive(Debug, Clone)]
pub struct SysrootLinux {
    files: Vec<PathBuf>,
}

impl SysrootLinux {
    /// The distribution one, otherwise the system one (through the Clang `clang`).
    pub fn locate(clang: &Path) -> Result<&'static SysrootLinux, String> {
        static S: std::sync::OnceLock<Result<SysrootLinux, String>> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            if let Some(dir) = dartforge_elements::distribution::in_lib(&format!("sysroot/{}", sysroot_triple())) {
                return Self::from_dir(&dir);
            }
            Self::from_system(clang)
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    fn from_dir(dir: &Path) -> Result<SysrootLinux, String> {
        let files: Vec<PathBuf> = SYSROOT_FILES.iter().map(|a| dir.join(a)).collect();
        if let Some(missing) = files.iter().find(|a| !a.is_file()) {
            return Err(format!("the distribution sysroot is incomplete: {} is missing", missing.display()));
        }
        Ok(SysrootLinux { files })
    }

    /// The files that the Clang driver would use (`-print-file-name`).
    pub fn from_system(clang: &Path) -> Result<SysrootLinux, String> {
        let mut files = Vec::new();
        for a in SYSROOT_FILES {
            let exit = Command::new(clang)
                .arg(format!("-print-file-name={a}"))
                .output()
                .map_err(|e| format!("failed to run {} to find {a}: {e}", clang.display()))?;
            let path = PathBuf::from(String::from_utf8_lossy(&exit.stdout).trim());
            if !path.is_file() {
                return Err(format!(
                    "{a} not found on the system: install the glibc and GCC development packages, \
                     or use the dartforge distribution (which brings the link sysroot)"
                ));
            }
            files.push(path);
        }
        Ok(SysrootLinux { files })
    }

    fn file(&self, name: &str) -> &Path {
        let i = SYSROOT_FILES.iter().position(|a| *a == name).expect("sysroot file");
        &self.files[i]
    }

    /// Copies the files to `dest` (`dartforge pack`).
    pub fn copy_to(&self, dest: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        for (name, of) in SYSROOT_FILES.iter().zip(&self.files) {
            std::fs::copy(of, dest.join(name)).map_err(|e| format!("copiar {}: {e}", of.display()))?;
        }
        Ok(())
    }
}

/// What gets linked.
pub enum Product<'a> {
    /// A PIE executable.
    Executable,
    /// A shared library with the `soname`, exporting (and pulling
    /// from the `staticlib`s) the given symbols.
    Shared { soname: &'a str, exported: &'a [String] },
}

/// A link through `ld.lld`.
pub struct Linkage<'a> {
    pub product: Product<'a>,
    /// Objects and libraries of the program (in order), before the system ones.
    pub entries: Vec<PathBuf>,
    /// Look for the shared libraries next to the executable.
    pub origin_rpath: bool,
    /// LTO of the input bitcodes, with code generation in partitions.
    pub lto: bool,
    /// The target CPU of the LTO code generation (`--cpu`, the LLVM name);
    /// `None`, the baseline (`x86-64` on x86-64).
    pub cpu: Option<&'static str>,
    /// Strip the sections nothing reaches and the symbol table (production).
    pub prune: bool,
    /// Keep the symbols and the debug sections even when pruning (J05).
    pub keep_debug_info: bool,
    pub exit: &'a Path,
}

/// `ld.lld`: the distribution one, the one next to Clang, or the one on `PATH`.
pub fn ld_lld(clang: &Path) -> PathBuf {
    if let Some(l) = dartforge_elements::distribution::llvm_tool("ld.lld") {
        return l;
    }
    let alongside = clang.with_file_name("ld.lld");
    if alongside.is_file() { alongside } else { PathBuf::from("ld.lld") }
}

/// Links with `ld.lld` and the link sysroot.
pub fn link(ld: &Path, sysroot: &SysrootLinux, l: &Linkage<'_>) -> Result<(), String> {
    let mut cmd = statement(ld, sysroot, l)?;
    let exit = cmd.output().map_err(|e| format!("failed to run {}: {e}", ld.display()))?;
    if !exit.status.success() {
        let text = String::from_utf8_lossy(&exit.stderr);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("ld.lld failed to link ({}):\n{}", exit.status, lines.join("\n")));
    }
    Ok(())
}

/// The `ld.lld` command for link `l`, ready to run (the
/// [`link`]) or to receive more options (the `--thinlto-index-only` of the
/// distributed ThinLTO, `distributed_lto.rs`).
pub fn statement(ld: &Path, sysroot: &SysrootLinux, l: &Linkage<'_>) -> Result<Command, String> {
    let (loader, emulation) = loader_and_emulation();
    let mut cmd = Command::new(ld);
    cmd.args(["-z", "relro", "--hash-style=gnu", "--eh-frame-hdr", "-m", emulation]);
    match &l.product {
        Product::Executable => {
            cmd.args(["-pie", "-dynamic-linker", loader]);
            cmd.arg(sysroot.file("Scrt1.o"));
        }
        Product::Shared { soname, .. } => {
            cmd.args(["-shared", "-soname", soname]);
            // Roots by maps: the index of the compact map (`dfgcm`) points to
            // each function through a 32-bit relative relocation, which
            // `ld.lld` only resolves against a symbol that cannot be replaced at
            // load time — in a shared library, the exported functions
            // now bind to themselves (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md,
            // Step 4).
            if crate::target::roots_by_maps().unwrap_or(false) {
                cmd.arg("-Bsymbolic-functions");
            }
        }
    }
    cmd.arg("-o").arg(l.exit);
    cmd.arg(sysroot.file("crti.o")).arg(sysroot.file("crtbeginS.o"));
    if let Product::Shared { exported, .. } = &l.product {
        // Each name goes in as undefined, so that the linker also pulls from the
        // `staticlib` what the SDK does not use (the program does); the
        // symbol is exported.
        let mut rsp = String::new();
        for n in exported.iter() {
            rsp.push_str(&format!("--undefined={n}\n"));
        }
        let file = l.exit.with_extension("rsp");
        std::fs::write(&file, rsp).map_err(|e| format!("{}: {e}", file.display()))?;
        cmd.arg(format!("@{}", file.display()));
    }
    if l.origin_rpath {
        cmd.args(["-rpath", "$ORIGIN"]);
    }
    if l.lto {
        let partitions = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
        let cpu = l.cpu.unwrap_or(if cfg!(target_arch = "x86_64") { "x86-64" } else { "generic" });
        cmd.arg(format!("--lto-O{}", crate::driver::lto_level())).arg(format!("--lto-partitions={partitions}")).arg(format!("-plugin-opt=mcpu={cpu}"));
        cmd.arg(format!("--thinlto-jobs={}", crate::driver::generation_tasks()));
        // MergeFunctions (`driver::merge_functions_at_link`): a global LLVM
        // option the ThinLTO cache key does not see — its own subdirectory.
        let merge = crate::driver::merge_functions_at_link();
        if merge {
            cmd.arg("-mllvm").arg("-enable-merge-functions");
        }
        if let Some(d) = crate::driver::thinlto_cache() {
            let d = if merge { d.join("mergefunc") } else { d };
            cmd.arg(format!("--thinlto-cache-dir={}", d.display()));
        }
    }
    if l.prune {
        cmd.arg("--gc-sections");
        // Safe ICF: merges only identical functions whose address nobody
        // takes (`.llvm_addrsig`), so the `==` of tear-offs stays correct
        // (`linker_windows.rs`, corpus/native 80).
        cmd.arg("--icf=safe");
        // `DARTFORGE_KEEP_SYMBOLS=1`: the symbol table stays (profiling).
        if !keep_symbols() && !l.keep_debug_info {
            cmd.arg("--strip-all");
        }
    }
    if l.prune
        && let Some(m) = link_map(l.exit)
    {
        cmd.arg(format!("-Map={}", m.display()));
    }
    cmd.args(&l.entries);
    // The system libraries (the runtime's `-lgcc_s -lutil -lrt -lpthread -lm -ldl
    // -lc`), through the sysroot files.
    cmd.arg("--as-needed");
    for b in ["libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1", "libutil.so.1", "libgcc_s.so.1"] {
        cmd.arg(sysroot.file(b));
    }
    cmd.arg("--no-as-needed");
    cmd.arg(sysroot.file("libgcc.a"));
    cmd.arg(sysroot.file("libc.so.6")).arg(sysroot.file("libc_nonshared.a"));
    cmd.arg("--as-needed").arg(sysroot.file(LOADER)).arg("--no-as-needed");
    cmd.arg(sysroot.file("libgcc.a"));
    cmd.arg(sysroot.file("crtendS.o")).arg(sysroot.file("crtn.o"));
    Ok(cmd)
}

/// `DARTFORGE_LINK_MAP=1`: the link map of a production executable
/// next to it (`<output>.map`), with the address and the size of each
/// symbol after LTO — what `tools/size/breakdown.py` reads and the code
/// absence test checks (docs/NATIVO-PODA-DE-TABELAS.md §3.10).
/// [`request_link_maps`] also turns it on.
pub fn link_map(exit: &Path) -> Option<PathBuf> {
    let request = MAPS.load(std::sync::atomic::Ordering::Relaxed)
        || std::env::var("DARTFORGE_LINK_MAP").is_ok_and(|v| v == "1");
    request.then(|| exit.with_extension("map"))
}

/// The maps requested by the process itself (the tests), without touching the
/// environment.
static MAPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Liga [`link_map`] neste processo.
pub fn request_link_maps() {
    MAPS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// `DARTFORGE_KEEP_SYMBOLS=1` leaves the symbol table in the executable,
/// for profiling (`perf`, `callgrind`) the generated code.
pub fn keep_symbols() -> bool {
    std::env::var("DARTFORGE_KEEP_SYMBOLS").is_ok_and(|v| v == "1")
}
