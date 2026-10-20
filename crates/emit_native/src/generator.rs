//! What turns the AOT LLVM IR into a native object (or into bitcode for
//! link-time optimization).
//!
//! Two generators with the same contract:
//!
//! * **embedded** (feature `embedded-llvm`, the distribution default): the LLVM
//!   linked into dartforge itself (`crates/llvm`), in process — without Clang
//!   on the machine of whoever uses dartforge, without a process per module and without
//!   writing the IR to disk;
//! * **Clang** (`DARTFORGE_GENERATOR=clang`, or a build without the feature): the
//!   `clang -x ir -c` of before.
//!
//! What a generation is asked for is a [`Generation`]; the key of the caches
//! (program object, source SDK) carries the [`Generator::identity`] and the
//! [`Generation::description`], so objects from one generator never serve the other.
//!
//! Without `-ffunction-sections`: in production the `lld` LTO already emits each
//! function in its own section, and the shared library of the SDK (development)
//! is linked whole.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A native object of the host.
    Object,
    /// Bitcode, optimized with the whole program at link time (production).
    Bitcode,
}

/// What a generation is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation {
    /// `-O2` instead of `-O0`.
    pub optimize: bool,
    pub format: Format,
    /// The object goes into a shared library (position-independent
    /// code; the embedded generator already emits it that way outside Windows).
    pub shared: bool,
    /// The target CPU (`--cpu`); `None`, the baseline of the target.
    pub cpu: Option<Cpu>,
    /// The optimization level of the bitcode before linking (0–2; ThinLTO
    /// redoes the optimization in the module of each part). Only the program one changes
    /// (`DARTFORGE_PRELINK_O`, measurement); the SDK stays at 2.
    pub prelink: u8,
}

impl Generation {
    /// The program object in the requested profile: `-O0` in development;
    /// in production with the source SDK, bitcode (`lto`).
    pub fn of_program(optimize: bool, lto: bool) -> Self {
        let prelink = std::env::var("DARTFORGE_PRELINK_O")
            .ok()
            .and_then(|v| v.trim().parse::<u8>().ok())
            .filter(|n| *n <= 2)
            .unwrap_or(2);
        Generation {
            optimize,
            format: if lto { Format::Bitcode } else { Format::Object },
            shared: false,
            cpu: None,
            prelink,
        }
    }

    /// The description that goes into the cache keys.
    pub fn description(&self) -> String {
        // The pipeline swapped for measurement (`dartforge_llvm::generate`) changes the
        // object: it goes into the key.
        let pipeline = match std::env::var("DARTFORGE_OBJECT_PIPELINE") {
            Ok(p) if self.optimize && self.format == Format::Object && !p.is_empty() => {
                format!(" {p}")
            }
            _ => String::new(),
        };
        format!(
            "{} {}{}{}{pipeline}",
            self.level(),
            match self.format {
                Format::Object => "objeto",
                Format::Bitcode => "bitcode-lto",
            },
            if self.shared { " compartilhado" } else { "" },
            self.cpu
                .map_or(String::new(), |c| format!(" {}", c.description()))
        )
    }

    /// The level of the generation: `-O0` without optimizing; `-O2` with it, except for bitcode
    /// with another [`Generation::prelink`].
    fn level(&self) -> &'static str {
        &self.clang_level()[1..]
    }

    fn clang_level(&self) -> &'static str {
        if !self.optimize {
            return "-O0";
        }
        match (self.format, self.prelink) {
            (Format::Bitcode, 0) => "-O0",
            (Format::Bitcode, 1) => "-O1",
            _ => "-O2",
        }
    }

    /// The equivalent `clang -x ir -c` flags.
    pub fn args_clang(&self) -> Vec<&'static str> {
        let mut args = vec!["-x", "ir", "-c", self.clang_level()];
        // In COFF, zero the TimeDateStamp: the object is a function of the key.
        args.extend(crate::target::object_flags());
        if self.shared {
            args.extend(crate::target::shared_object_flags());
        }
        if self.format == Format::Bitcode {
            args.push("-flto=thin");
        }
        if let Some(c) = self.cpu {
            args.push(c.march());
        }
        args
    }
}

/// The requested target CPU (`--cpu`): without it, the baseline of the target (on x86-64,
/// `x86-64` — SSE2 —, the same as Clang without `-march`). It applies to the
/// program object and, in production, to the LTO code generation (program and
/// SDK); the runtime (Rust) and the development SDK library
/// stay at the baseline. An executable generated with a CPU above the baseline only runs
/// on machines that have it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    /// `x86-64`: SSE2 (the baseline).
    X86_64,
    /// `x86-64-v2`: + SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT.
    X86_64V2,
    /// `x86-64-v3`: + AVX, AVX2, BMI1/2, FMA, F16C, LZCNT, MOVBE.
    X86_64V3,
    /// `x86-64-v4`: + AVX-512 (F, BW, CD, DQ, VL).
    X86_64V4,
    /// `native`: the CPU of this machine (LLVM's, `sys::getHostCPUName`).
    Native,
}

impl Cpu {
    /// The CPU of `--cpu <name>`.
    pub fn from_name(name: &str) -> Result<Cpu, String> {
        let x86 = !crate::target::is_aarch64();
        match name {
            "native" if !crate::target::is_cross() => Ok(Cpu::Native),
            "native" => Err(
                "--cpu native cannot be used for a foreign target; select its baseline CPU".into(),
            ),
            "x86-64" if x86 => Ok(Cpu::X86_64),
            "x86-64-v2" if x86 => Ok(Cpu::X86_64V2),
            "x86-64-v3" if x86 => Ok(Cpu::X86_64V3),
            "x86-64-v4" if x86 => Ok(Cpu::X86_64V4),
            _ if x86 => Err(format!(
                "--cpu {name}: use x86-64, x86-64-v2, x86-64-v3, x86-64-v4 or native"
            )),
            _ => Err(format!("--cpu {name}: on this target, only `native`")),
        }
    }

    /// The LLVM name (`-march=`/`-mcpu=`).
    pub fn name(self) -> &'static str {
        match self {
            Cpu::X86_64 => "x86-64",
            Cpu::X86_64V2 => "x86-64-v2",
            Cpu::X86_64V3 => "x86-64-v3",
            Cpu::X86_64V4 => "x86-64-v4",
            Cpu::Native => "native",
        }
    }

    /// Clang's `-march=<cpu>`.
    pub(crate) fn march(self) -> &'static str {
        match self {
            Cpu::X86_64 => "-march=x86-64",
            Cpu::X86_64V2 => "-march=x86-64-v2",
            Cpu::X86_64V3 => "-march=x86-64-v3",
            Cpu::X86_64V4 => "-march=x86-64-v4",
            Cpu::Native => "-march=native",
        }
    }

    /// What goes into the cache keys: `native` carries the CPU features
    /// of this machine (the cache may be copied to another one).
    pub fn description(self) -> String {
        match self {
            Cpu::Native => format!("cpu=native({})", machine_resources()),
            c => format!("cpu={}", c.name()),
        }
    }
}

/// The vector and bit features of the CPU of this machine, to tell apart the
/// `native` of two machines in the cache keys.
fn machine_resources() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let mut r: Vec<&str> = Vec::new();
        macro_rules! resources {
            ($($f:tt),*) => { $( if std::arch::is_x86_feature_detected!($f) { r.push($f); } )* };
        }
        resources!(
            "sse4.2",
            "popcnt",
            "avx",
            "avx2",
            "bmi2",
            "fma",
            "f16c",
            "avx512f",
            "avx512bw",
            "avx512vl",
            "avx512dq",
            "gfni",
            "vpclmulqdq"
        );
        r.join(",")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        std::env::consts::ARCH.to_string()
    }
}

/// Whether this build has the embedded generator (and does not need Clang to generate
/// objects).
pub const EMBEDDED_GENERATOR: bool = cfg!(feature = "embedded-llvm");

/// The generator of a compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generator {
    /// The LLVM linked into dartforge.
    #[cfg(feature = "embedded-llvm")]
    Builtin,
    /// The given `clang`.
    Clang(PathBuf),
}

impl Generator {
    /// The generator of this build and this environment: the embedded one when the build
    /// has it, unless `DARTFORGE_GENERATOR=clang`; otherwise `clang`.
    pub fn choose(clang: &Path) -> Generator {
        #[cfg(feature = "embedded-llvm")]
        if std::env::var("DARTFORGE_GENERATOR").map_or(true, |g| g != "clang") {
            return Generator::Builtin;
        }
        Generator::Clang(clang.to_path_buf())
    }

    /// Clang, for bitcode with the ThinLTO summary, when it exists;
    /// otherwise the usual generator ([`Generator::choose`]). The distribution without
    /// the system toolchain (`dartforge pack`, N15/N16) does not ship
    /// Clang: production uses the embedded generator, and `lld` does the full LTO
    /// (slower, the same program).
    pub fn clang_if_any(clang: &Path) -> Generator {
        if crate::object_cache::clang_identity(clang).is_ok() {
            Generator::Clang(clang.to_path_buf())
        } else {
            Generator::choose(clang)
        }
    }

    /// Whether the bitcode of this generator carries the ThinLTO summary. The embedded one emits
    /// bitcode without a summary (the LLVM C API does not write the index): `lld`
    /// does the full LTO, split into parallel partitions during code
    /// generation.
    pub fn thin_bitcode(&self) -> bool {
        matches!(self, Generator::Clang(_))
    }

    /// The identity for the cache keys (version, target).
    pub fn identity(&self) -> Result<String, String> {
        match self {
            #[cfg(feature = "embedded-llvm")]
            Generator::Builtin => dartforge_llvm::identity()
                .map(|id| format!("{id} target={}", crate::target::current().triple())),
            Generator::Clang(clang) => crate::object_cache::clang_identity(clang)
                .map(|id| format!("{id} target={}", crate::target::current().triple())),
        }
    }

    /// Generates `exit` from the IR. With Clang, the IR is written next to the
    /// output and Clang runs in its directory with relative names, so that the
    /// `source_filename` and the `.file` of the object do not carry the path of
    /// whoever compiled (with the cache, the name is the hash, and the same IR gives the same
    /// object byte for byte).
    pub fn generate(&self, ir: &str, generation: Generation, exit: &Path) -> Result<(), String> {
        match self {
            #[cfg(feature = "embedded-llvm")]
            Generator::Builtin => {
                let name = exit.file_stem().unwrap_or_default().to_string_lossy();
                let options = dartforge_llvm::Options {
                    optimize: generation.optimize,
                    format: match generation.format {
                        Format::Object => dartforge_llvm::Format::Object,
                        Format::Bitcode => dartforge_llvm::Format::Bitcode,
                    },
                    cpu: generation.cpu.map(Cpu::name),
                };
                #[allow(unused_mut)]
                let mut bytes = dartforge_llvm::generate(&name, ir, &options)?;
                // Roots by maps: the stack map of the object, from the LLVM
                // format (~98 bytes per record) to the compact one (`gcmap.rs`).
                // `DARTFORGE_NO_COMPACT_MAP=1` leaves the LLVM one in COFF
                // (measurement); in ELF the conversion is mandatory, and Mach-O stays
                // in the LLVM format.
                if generation.format == Format::Object
                    && ir.contains(dartforge_llvm::GC_MARK)
                    && crate::gcmap::convert_here()
                {
                    crate::gcmap::convert(&mut bytes)?;
                }
                // The symbolic stack trace: the 12-byte-per-entry table becomes
                // DFPC (`compact_stack_trace.rs`); `DARTFORGE_STACK_TRACE_RAW=1` leaves it.
                if generation.format == Format::Object
                    && ir.contains("@df.pcf.")
                    && crate::compact_stack_trace::convert_here()
                {
                    crate::compact_stack_trace::convert(&mut bytes)?;
                }
                std::fs::write(exit, bytes)
                    .map_err(|e| format!("failed to write {}: {e}", exit.display()))
            }
            Generator::Clang(clang) => {
                // Roots by maps: `clang -x ir` does not run
                // `rewrite-statepoints-for-gc`, and the object would come out without a map —
                // roots silently lost.
                // In bitcode it can: it is the part of a large program in
                // production, and the pass runs at the closing of the distributed ThinLTO
                // (`distributed_lto.rs`), which is the only path that takes
                // that bitcode to the linker.
                if ir.contains("gc \"statepoint-example\"") && generation.format != Format::Bitcode
                {
                    return Err("roots by maps (--roots=maps) require dartforge's embedded generator, not Clang".to_string());
                }
                let ll = exit.with_extension("ll");
                std::fs::write(&ll, ir)
                    .map_err(|e| format!("failed to write LLVM IR to {}: {e}", ll.display()))?;
                let dir = exit.parent().unwrap_or(Path::new("."));
                let result = Command::new(clang)
                    .current_dir(dir)
                    .args(generation.args_clang())
                    .args(
                        crate::target::is_cross()
                            .then(|| format!("--target={}", crate::target::current().triple())),
                    )
                    .arg(ll.file_name().unwrap_or_default())
                    .arg("-o")
                    .arg(exit.file_name().unwrap_or_default())
                    .output()
                    .map_err(|e| format!("failed to run Clang at {clang:?}: {e}"));
                let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
                if !keep_ir {
                    let _ = std::fs::remove_file(&ll);
                }
                let result = result?;
                if !result.status.success() {
                    let text = String::from_utf8_lossy(&result.stderr);
                    let lines: Vec<&str> = text
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .take(20)
                        .collect();
                    return Err(format!(
                        "Clang failed to compile the IR ({}):\n{}",
                        result.status,
                        lines.join("\n")
                    ));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_distinguishes_profiles() {
        let a = Generation::of_program(false, false).description();
        let b = Generation::of_program(true, false).description();
        let c = Generation::of_program(true, true).description();
        assert!(a != b && b != c && a != c);
    }

    #[test]
    fn clang_flags() {
        let g = Generation {
            optimize: true,
            format: Format::Bitcode,
            shared: false,
            cpu: None,
            prelink: 2,
        };
        assert!(g.args_clang().contains(&"-flto=thin"));
        assert!(
            Generation::of_program(false, false)
                .args_clang()
                .contains(&"-O0")
        );
    }

    #[cfg(feature = "embedded-llvm")]
    #[test]
    fn embedded_generates_object() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir
            .path()
            .join(format!("x.{}", crate::target::ext_object()));
        let ir = format!(
            "{}define i64 @f() {{\n  ret i64 7\n}}\n",
            crate::target::ir_header()
        );
        Generator::Builtin
            .generate(&ir, Generation::of_program(true, false), &obj)
            .unwrap();
        assert!(std::fs::metadata(&obj).unwrap().len() > 0);
    }
}
