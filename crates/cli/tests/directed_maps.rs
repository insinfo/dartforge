//! The targeted tests of roots through stack maps
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3, §7.5 and §14.7).
//!
//! Each case is a program from `corpus/native/gc_*` (which the differential
//! harness also runs against the VM) with the output computed by hand. It is
//! compiled with `--raizes=mapas` and run with `--gc-stress`, poisoning with
//! quarantine and the proof that it collected (`DARTFORGE_GC_STATS=exigir`:
//! without a collection, or without a root read from a map, the executable
//! exits with 70). Then, with its sabotage, the case **has to fail** — at
//! compilation, at run time or in the output —; a case that passes with the
//! sabotage proves nothing.
//!
//! Also: the checking build (`DARTFORGE_ROOTS_CHECK=1` with
//! `DARTFORGE_GC_WALK=conferir`, E2.5) and the seed-scheduled collection
//! (`DARTFORGE_GC_SCHEDULE`).
//!
//! Heavy (one compilation per case and per sabotage): runs with
//! `DARTFORGE_MAPS_TESTS=1`, on the targets that have the mode.
//! Written without compiling or running (2026-10-05).
#![cfg(feature = "jit")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Case {
    file: &'static str,
    expected: &'static str,
    /// Compile with `--excecoes=tabelas`.
    tables: bool,
    /// Compile with the roots on the shadow stack (`--raizes=sombra`): the case
    /// whose sabotage targets the shadow stack frame (D5).
    shadow: bool,
    /// The emitter's sabotage (`DARTFORGE_SABOTAGE` at compile time).
    emitter_sabotage: Option<&'static str>,
    /// The runtime's sabotage (`DARTFORGE_SABOTAGE` at run time).
    runtime_sabotage: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        file: "gc_d01_argumento_vivo.dart",
        expected: "18335 1999000 2000\n",
        tables: false, shadow: false,
        emitter_sabotage: Some("sem_uso_ficticio"),
        runtime_sabotage: None,
    },
    // D7: the fake leaf, in the same program.
    Case {
        file: "gc_d01_argumento_vivo.dart",
        expected: "18335 1999000 2000\n",
        tables: false, shadow: false,
        emitter_sabotage: Some("folha:dartforge_string_concat"),
        runtime_sabotage: None,
    },
    // §14.7: the reader without the `nop` rule (only Windows x64 has it). D1
    // stops no collection at a return with `nop`; D15 does.
    Case {
        file: "gc_d15_retorno_no_nop.dart",
        expected: "double\nString\n",
        tables: false, shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: if cfg!(windows) { Some("sem_nop") } else { None },
    },
    Case {
        file: "gc_d02_muitos_vivos.dart",
        expected: "4320 h-49 2.0\n",
        tables: false, shadow: false,
        emitter_sabotage: Some("sem_uso_ficticio"),
        runtime_sabotage: None,
    },
    // The landing pad without restoring the shadow stack top (§14.7,
    // `x16\a1_sab.ll`) belongs to combination A1: with the maps the function
    // has no shadow stack frame and the top does not change.
    Case {
        file: "gc_d03_excecao_profunda.dart",
        expected: "32255\n",
        tables: true, shadow: true,
        emitter_sabotage: Some("pouso_sem_topo"),
        runtime_sabotage: None,
    },
    Case {
        file: "gc_d04_finally.dart",
        expected: "336 10 1566\n",
        tables: true, shadow: false,
        emitter_sabotage: Some("sem_uso_ficticio"),
        runtime_sabotage: None,
    },
    Case {
        file: "gc_d06_valores_brutos.dart",
        expected: "249750.0 999000 3890 1248750.0\n",
        tables: false, shadow: false,
        emitter_sabotage: Some("bruto_no_mapa"),
        runtime_sabotage: None,
    },
    Case { file: "gc_d10_closures_async.dart", expected: "4050\n", tables: false, shadow: false, emitter_sabotage: None, runtime_sabotage: None },
    Case {
        file: "gc_d11_campos_de_objeto_morto.dart",
        expected: "26800\n",
        tables: false, shadow: false,
        emitter_sabotage: Some("sem_uso_ficticio"),
        runtime_sabotage: None,
    },
    Case {
        file: "gc_d13_estaticos_e_constantes.dart",
        expected: "13890\n",
        tables: false, shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: None,
    },
    Case { file: "gc_d14_quadro_residual.dart", expected: "499500\n", tables: false, shadow: false, emitter_sabotage: None, runtime_sabotage: None },
    // D5: the slot read before being written, on the shadow stack; with the
    // dirty frame (4098 instead of zero), the not-yet-written slot becomes an
    // invalid root.
    Case {
        file: "gc_d05_slot_antes_de_escrito.dart",
        expected: "1729 140\n",
        tables: false,
        shadow: true,
        emitter_sabotage: Some("quadro_sujo"),
        runtime_sabotage: None,
    },
    // D5 also with the maps (the `phi` with the null entry, the `async` continuation).
    Case {
        file: "gc_d05_slot_antes_de_escrito.dart",
        expected: "1729 140\n",
        tables: false,
        shadow: false,
        emitter_sabotage: Some("sem_uso_ficticio"),
        runtime_sabotage: None,
    },
    // D8: the caught `StackOverflowError`, in both exception models; with the
    // 4 KiB slack, building and throwing the error really overflows the stack.
    Case {
        file: "gc_d08_estouro_de_pilha.dart",
        expected: "3003 0\n",
        tables: false,
        shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: Some("folga"),
    },
    Case {
        file: "gc_d08_estouro_de_pilha.dart",
        expected: "3003 0\n",
        tables: true,
        shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: Some("folga"),
    },
    // D12: the runtime's static caches with a handle are roots.
    Case {
        file: "gc_d12_cache_do_runtime.dart",
        expected: "1000 1000 4000\n",
        tables: false,
        shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: Some("tipo_sem_raiz"),
    },
    // D9: the exception that leaves a Dart function the runtime called;
    // without the gates, it crosses the Rust frames.
    Case {
        file: "gc_d09_fronteira_rust.dart",
        expected: "925\n",
        tables: true,
        shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: Some("sem_porta"),
    },
    // D9: `Isolate.exit` inside `finally`, with the exception coming from the
    // gate still in flight; the isolate's exit is not catchable.
    Case {
        file: "gc_d09_isolate_exit_em_finally.dart",
        expected: "870\n",
        tables: true,
        shadow: false,
        emitter_sabotage: None,
        runtime_sabotage: Some("sem_porta"),
    },
    // D9: the throwing FFI callback, called by libc's `qsort` (a C frame) and
    // by Dart itself through the native pointer. Without the landing pad at
    // the callback's entry, the exception crosses the C frame and reaches `main`.
    Case {
        file: "gc_d09_callback_ffi.dart",
        expected: "[1, 1, 2, 3, 3, 3, 5, 8, 8, 9] 0 14 2000\n",
        tables: true,
        shadow: false,
        emitter_sabotage: Some("callback_sem_pouso"),
        runtime_sabotage: None,
    },
];

fn enabled() -> bool {
    let target = (cfg!(windows) && cfg!(target_arch = "x86_64"))
        || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
    let requested = std::env::var("DARTFORGE_MAPS_TESTS").is_ok_and(|v| v == "1");
    if !requested {
        eprintln!("DARTFORGE_MAPS_TESTS=1 not set: stack map root tests skipped");
    }
    target && requested
}

fn source(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/native").join(file)
}

/// The test executables and SDKs are removed even when an assertion fails.
struct WorkDir(PathBuf);

impl std::ops::Deref for WorkDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn work_dir(name: &str) -> WorkDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let d = WorkDir(base.join(format!("tmp-mapas-{}-{name}", std::process::id())));
    std::fs::create_dir_all(&d.0).unwrap();
    d
}

/// Compiles `file` in maps mode (or on the shadow stack, with `shadow`); the
/// error carries the compiler's message. In maps mode, the root checker after
/// RS4GC (§7.4) runs on each module; on the shadow stack, the slot dominance
/// checker.
fn compile(file: &str, tables: bool, shadow: bool, env: &[(&str, &str)], output: &Path) -> Result<(), String> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_dartforge"));
    c.arg("compile-native").arg(source(file)).arg("-o").arg(output);
    c.arg(if shadow { "--raizes=sombra" } else { "--raizes=mapas" });
    if tables {
        c.arg("--excecoes=tabelas");
    }
    c.env_remove("DARTFORGE_SABOTAGE").env_remove("DARTFORGE_ROOTS_CHECK");
    if shadow {
        // The dominance checker of the shadow mode (§7.4).
        c.env("DARTFORGE_CHECK_SHADOW", "1");
    } else {
        c.env("DARTFORGE_CHECK_RS4GC", "1");
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let r = c.output().expect("run dartforge");
    if r.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&r.stderr).into_owned()) }
}

/// Runs with stress (one collection before each allocation) or without it
/// (the schedule decides), poisoning and the proof that it collected.
fn run_with(exe: &Path, stress: bool, env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(exe);
    c.env_remove("DARTFORGE_GC_STRESS");
    if stress {
        c.env("DARTFORGE_GC_STRESS", "1");
    }
    c.env("DARTFORGE_GC_STATS", "exigir").env("DARTFORGE_GC_POISON", "1");
    c.env_remove("DARTFORGE_SABOTAGE").env_remove("DARTFORGE_GC_WALK").env_remove("DARTFORGE_GC_SCHEDULE");
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().expect("run the program")
}

fn run(exe: &Path, env: &[(&str, &str)]) -> Output {
    run_with(exe, true, env)
}

/// The number `field` of the statistics line (`"map_roots":123`).
fn statistic(stderr: &str, field: &str) -> Option<u64> {
    let key = format!("\"{field}\":");
    let i = stderr.find(&key)? + key.len();
    stderr[i..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

fn right_output(r: &Output, expected: &str) -> bool {
    r.status.success() && String::from_utf8_lossy(&r.stdout).replace("\r\n", "\n") == expected
}

#[test]
fn targeted_cases_pass_and_fail_with_the_sabotage() {
    if !enabled() {
        return;
    }
    let dir = work_dir("dirigidos");
    for (k, case) in CASES.iter().enumerate() {
        let exe = dir.join(format!("case{k}{}", std::env::consts::EXE_SUFFIX));
        compile(case.file, case.tables, case.shadow, &[], &exe).unwrap_or_else(|e| panic!("{}: did not compile:\n{e}", case.file));
        let r = run(&exe, &[]);
        let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
        assert!(right_output(&r, case.expected), "{}: wrong output ({}):\n{}\n{stderr}", case.file, r.status, String::from_utf8_lossy(&r.stdout));
        // The proof that it collected and read roots from maps (`exigir`
        // already exits with 70 without it; here the check is explicit).
        assert!(statistic(&stderr, "collections").is_some_and(|n| n > 0), "{}: no collection:\n{stderr}", case.file);
        assert!(
            case.shadow || statistic(&stderr, "map_roots").is_some_and(|n| n > 0),
            "{}: no root read from a map:\n{stderr}",
            case.file
        );

        if let Some(s) = case.emitter_sabotage {
            let sab = dir.join(format!("case{k}-sab{}", std::env::consts::EXE_SUFFIX));
            let failed = match compile(case.file, case.tables, case.shadow, &[("DARTFORGE_SABOTAGE", s)], &sab) {
                Err(_) => true,
                Ok(()) => !right_output(&run(&sab, &[]), case.expected),
            };
            assert!(failed, "{}: passed with the sabotage `{s}` — the case does not prove what it claims", case.file);
        }
        if let Some(s) = case.runtime_sabotage {
            let r = run(&exe, &[("DARTFORGE_SABOTAGE", s)]);
            assert!(!right_output(&r, case.expected), "{}: passed with the runtime sabotage `{s}`", case.file);
        }
    }
}

/// E2.5: the checking build writes, before each collection point, its live
/// values onto the shadow stack; the runtime checks that the walk through the
/// maps visited them all.
#[test]
fn walk_checked_against_the_shadow_stack() {
    if !enabled() {
        return;
    }
    let dir = work_dir("conferencia");
    for case in CASES.iter().filter(|c| !c.shadow && c.runtime_sabotage.is_none() && c.emitter_sabotage.is_some()) {
        let exe = dir.join(format!("conf{}", std::env::consts::EXE_SUFFIX));
        compile(case.file, case.tables, false, &[("DARTFORGE_ROOTS_CHECK", "1")], &exe)
            .unwrap_or_else(|e| panic!("{}: did not compile:\n{e}", case.file));
        let r = run(&exe, &[("DARTFORGE_GC_WALK", "conferir")]);
        let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
        assert!(right_output(&r, case.expected), "{}: the check failed ({}):\n{stderr}", case.file, r.status);
        assert!(statistic(&stderr, "checked_roots").is_some_and(|n| n > 0), "{}: no root checked:\n{stderr}", case.file);
    }
}

/// D7 per extern (§7.3, §7.5): with the effects table check
/// (`DARTFORGE_EFFECTS=conferir`), each extern marked as a leaf runs watched,
/// and a collection inside it ends the process naming it. D1 passes; with the
/// fake leaf (`folha:dartforge_string_concat`), it fails, and the message
/// names the extern.
#[test]
fn leaf_that_collects_is_found_by_name() {
    if !enabled() {
        return;
    }
    let dir = work_dir("efeitos");
    let case = &CASES[0];
    let exe = dir.join(format!("efeitos{}", std::env::consts::EXE_SUFFIX));
    compile(case.file, false, false, &[("DARTFORGE_EFFECTS", "conferir")], &exe).unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    let r = run(&exe, &[]);
    assert!(right_output(&r, case.expected), "with the check: {}", String::from_utf8_lossy(&r.stderr));
    let sab = dir.join(format!("efeitos-sab{}", std::env::consts::EXE_SUFFIX));
    compile(case.file, false, false, &[("DARTFORGE_EFFECTS", "conferir"), ("DARTFORGE_SABOTAGE", "folha:dartforge_string_concat")], &sab)
        .unwrap_or_else(|e| panic!("did not compile with the sabotage:\n{e}"));
    let r = run(&sab, &[]);
    let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
    assert!(!r.status.success() && stderr.contains("dartforge_string_concat"), "the fake leaf was not found ({}):\n{stderr}", r.status);
}

/// §7.5: the seed-scheduled collection, instead of full stress.
#[test]
fn seed_scheduled_collection() {
    if !enabled() {
        return;
    }
    let dir = work_dir("agenda");
    let case = &CASES[0];
    let exe = dir.join(format!("agenda{}", std::env::consts::EXE_SUFFIX));
    compile(case.file, false, false, &[], &exe).unwrap_or_else(|e| panic!("did not compile:\n{e}"));
    for seed in ["1,7", "42,13", "9,3"] {
        let r = run_with(&exe, false, &[("DARTFORGE_GC_SCHEDULE", seed)]);
        assert!(right_output(&r, case.expected), "schedule {seed}: {}", String::from_utf8_lossy(&r.stderr));
    }
}
