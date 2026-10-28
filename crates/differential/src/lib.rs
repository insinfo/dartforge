//! Differential harness: `dart run` (semantics) × `dartdevc`+Node (contract) × DartForge.
//!
//! Corpus in `corpus/js/`: one program per file (`NN_theme.dart`) or per
//! directory with `main.dart`. See `docs/EMISSAO-DDC.md` and
//! `docs/CONTRATO-DDC.md`.

pub mod contract;
pub mod corpus;
pub mod levels;
pub mod matrix;
mod oracle_validation;
pub mod oracles;
pub mod process;
pub mod report;

pub use corpus::{Program, list};
pub use oracles::{
    Environment, dart_oracle, dart2wasm_oracle, dartforge, dartforge_jit, dartforge_native,
    dartforge_native_ir, dartforge_production, ddc_oracle,
};
pub use process::Output;
pub use report::{
    AotFromSameIr, Divergence, JitRun, ProgramIr, ProgramResult, compare, format_ir_report,
    format_report, ir_differences,
};

use dartforge_emit_native::summary::IrSummary;
use std::sync::{Condvar, Mutex};

/// What to run for each program.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Also run DartForge (otherwise only the two oracles).
    pub with_forge: bool,
    /// Number of threads; 0 = available cores.
    pub threads: usize,
    /// Run in native AOT mode (comparing against the Dart VM oracle).
    pub native: bool,
    /// Also run the JavaScript backend's **production profile**
    /// (`dartforge-jsprod`): a single file, `dart_sdk.js` pruned by the closed
    /// world. See `docs/JS-PRODUCAO.md`. Independent of `native`: the two
    /// executors are different backends of the same program.
    pub with_production: bool,
    /// Run the native backend through the **JIT** (`dartforge-run-ir`)
    /// instead of AOT. Implies `native`: the reference is still the VM.
    pub jit: bool,
    /// With `jit`: also link and run the AOT of the **same** IR, require both
    /// to be equal program by program and measure the times (`--jit-aot`).
    pub jit_aot: bool,
    /// Native mode compiled to a WebAssembly module (`wasm32-linear`) and run by
    /// the independent executor (`dartforge-wasm-run`); the process selects the
    /// destination once (`--wasm`).
    pub wasm: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            with_forge: true,
            threads: 0,
            native: false,
            with_production: false,
            jit: false,
            jit_aot: false,
            wasm: false,
        }
    }
}

/// Runs a program on the requested executors.
pub fn run_program(env: &Environment, program: &Program, op: Options) -> ProgramResult {
    // The WebAssembly target answers to dart2wasm, where the web semantics
    // differ from the VM (`dart:io`, isolates, `dart:ffi`).
    let dart = dart_oracle(env, program);
    // The WebAssembly target also answers to dart2wasm (`ProgramResult::alternative`).
    let alternative = op
        .wasm
        .then(|| dart2wasm_oracle(env, program, &env.output_dir("dart2wasm", program)));
    // The JS production profile is independent of the chosen backend: it is a
    // fourth executor of the same program, and it can run alongside native.
    // The executor names passed to `output_dir` are directories on disk.
    let production = op
        .with_production
        .then(|| dartforge_production(env, program, &env.output_dir("production", program)));
    if op.native {
        // In native mode there is no DDC contract to compare; the reference
        // is always the VM (see `ProgramResult::reference`).
        let ddc = Output {
            stdout: String::new(),
            stderr: String::new(),
            code: 0,
        };
        if op.jit {
            let (forge, jit) = if op.with_forge {
                let (s, j) =
                    dartforge_jit(env, program, &env.output_dir("jit", program), op.jit_aot);
                (Some(s), Some(j))
            } else {
                (None, None)
            };
            return ProgramResult {
                program: program.clone(),
                dart,
                ddc,
                forge,
                native: true,
                production,
                jit,
                alternative: None,
            };
        }
        let forge = op
            .with_forge
            .then(|| dartforge_native(env, program, &env.output_dir("native", program)));
        ProgramResult {
            program: program.clone(),
            dart,
            ddc,
            forge,
            native: true,
            production,
            jit: None,
            alternative,
        }
    } else {
        let ddc = ddc_oracle(env, program, &env.output_dir("ddc", program));
        let forge = op
            .with_forge
            .then(|| dartforge(env, program, &env.output_dir("forge", program)));
        ProgramResult {
            program: program.clone(),
            dart,
            ddc,
            forge,
            native: false,
            production,
            jit: None,
            alternative: None,
        }
    }
}

/// Applies `f` to each program with `threads` workers (0 = available cores)
/// pulling from a shared queue, and returns the results in **corpus order**,
/// whatever the completion order was. `progress` is called at the end of each
/// program (to print progress).
pub fn in_parallel<T: Send>(
    programs: &[Program],
    threads: usize,
    f: impl Fn(&Program) -> T + Sync,
    progress: impl Fn(&T) + Sync,
) -> Vec<T> {
    let n = if threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        threads
    };
    let n = n.max(1).min(programs.len().max(1));
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<Option<T>>> = Mutex::new((0..programs.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut g = next.lock().unwrap();
                        let i = *g;
                        *g += 1;
                        i
                    };
                    if i >= programs.len() {
                        break;
                    }
                    let r = f(&programs[i]);
                    progress(&r);
                    results.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.expect("every program was run"))
        .collect()
}

/// Runs the whole corpus with `std::thread` (shared queue), preserving the order.
/// `progress` is called at the end of each program (to print progress).
pub fn run_corpus(
    env: &Environment,
    programs: &[Program],
    op: Options,
    progress: impl Fn(&ProgramResult) + Sync,
) -> Vec<ProgramResult> {
    in_parallel(programs, op.threads, |p| run_program(env, p, op), progress)
}

/// Concurrent emissions when `DARTFORGE_IR_PARALLEL_MAX` does not say otherwise.
///
/// Each emission loads the SDK, analyzes and lowers the program inside the
/// harness process. Today that is cheap (measured in `STATE.md` §3.2: the
/// whole corpus in under half a second, a peak of 10 MB with eight at the
/// same time), because the `vm` section of `libraries.json` only declares
/// `dart:cli` and the `include` of `vm_common` is not followed yet. When it
/// is, each emission starts analyzing the whole SDK — in JS that costs
/// hundreds of MB (§1.3) —, and eight at the same time do not fit in the
/// shared 7.7 GB machine. The determinism test does not need eight *physical*
/// emissions: it needs eight workers competing for the queue, so that the
/// completion order changes — and it still changes with the limit, because who
/// takes the next slot is decided by the scheduler.
pub const DEFAULT_CONCURRENT_EMISSIONS: usize = 2;

/// Counting semaphore: at most `max` protected sections running at the same
/// time, whatever the number of workers.
#[derive(Debug)]
pub struct Limiter {
    max: usize,
    free: Mutex<usize>,
    slot: Condvar,
}

impl Limiter {
    pub fn new(max: usize) -> Limiter {
        let max = max.max(1);
        Limiter {
            max,
            free: Mutex::new(max),
            slot: Condvar::new(),
        }
    }

    /// `DARTFORGE_IR_PARALLEL_MAX` when it is a positive number, otherwise
    /// [`DEFAULT_CONCURRENT_EMISSIONS`].
    pub fn from_env() -> Limiter {
        let max = std::env::var("DARTFORGE_IR_PARALLEL_MAX")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_CONCURRENT_EMISSIONS);
        Limiter::new(max)
    }

    pub fn max(&self) -> usize {
        self.max
    }

    /// Runs `f` holding a slot; waits if there is none. The slot is returned
    /// even if `f` panics.
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        struct SlotGuard<'a>(&'a Limiter);
        impl Drop for SlotGuard<'_> {
            fn drop(&mut self) {
                *self.0.free.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                self.0.slot.notify_one();
            }
        }
        {
            let mut free = self.free.lock().unwrap();
            while *free == 0 {
                free = self.slot.wait(free).unwrap();
            }
            *free -= 1;
        }
        let _slot = SlotGuard(self);
        f()
    }
}

/// Emits the LLVM IR of each program (without Clang, linking or running) and
/// keeps only the summary: the text is discarded while still inside the
/// `limiter`'s slot, so that its memory does not add up with the next
/// emission's.
pub fn emit_corpus_ir(programs: &[Program], threads: usize, limiter: &Limiter) -> Vec<ProgramIr> {
    in_parallel(
        programs,
        threads,
        |p| ProgramIr {
            name: p.name.clone(),
            result: limiter.run(|| dartforge_native_ir(p).map(|text| IrSummary::of(&text))),
        },
        |_| {},
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn programs(n: usize) -> Vec<Program> {
        (0..n).map(|i| Program::test(&format!("p{i}"))).collect()
    }

    #[test]
    fn in_parallel_preserves_order() {
        let ps = programs(24);
        let expected: Vec<String> = ps.iter().map(|p| p.name.clone()).collect();
        for threads in [1, 4, 8] {
            let called = AtomicUsize::new(0);
            // The first ones take longer: with several workers, the completion
            // order differs from the corpus order.
            let r = in_parallel(
                &ps,
                threads,
                |p| {
                    let i: u64 = p.name[1..].parse().unwrap();
                    std::thread::sleep(Duration::from_millis(24 - i));
                    p.name.clone()
                },
                |_| {
                    called.fetch_add(1, Ordering::SeqCst);
                },
            );
            assert_eq!(r, expected, "{threads} workers");
            assert_eq!(called.load(Ordering::SeqCst), ps.len());
        }
    }

    #[test]
    fn limiter_respects_the_maximum() {
        let limiter = Limiter::new(2);
        let inside = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        in_parallel(
            &programs(16),
            8,
            |_| {
                limiter.run(|| {
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    inside.fetch_sub(1, Ordering::SeqCst);
                })
            },
            |_| {},
        );
        let peak = peak.load(Ordering::SeqCst);
        assert!(
            (1..=2).contains(&peak),
            "peak of {peak} inside the limiter of 2"
        );
    }
}
