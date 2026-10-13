//! `dartforge-differential [options]` — corpus report.
//! `dartforge-differential contract [-o docs/CONTRATO-DDC.md]` — generates the contract document.
//!
//! Some lines this binary prints are parsed by CI (`.github/actions/harness`,
//! `.github/workflows/*.yml`): `(N programas em X s)`, the
//! `determinismo…: ` verdicts and, on stderr in determinism mode, the lines
//! with `trabalhador`, `linha N:`, `  a: `/`  b: ` and `aviso`. Those stay in
//! Portuguese, byte for byte, until the CI scripts are migrated with them.

use std::path::PathBuf;
use std::time::Duration;

use dartforge_differential::{
    Environment, Limiter, Options, Program, ProgramIr, contract, corpus, dartforge_native_ir, emit_corpus_ir, format_ir_report,
    format_report, in_parallel, ir_differences, list, run_corpus,
};

const USAGE: &str = "usage:
  dartforge-differential [--nativo [--gc-stress] [--otimizar]] [--producao] [--corpus DIR] [--filtro TEXT] [--sem-forge] [--sem-cache] [--jobs N] [--limite SEC] [--limite-exec SEC] [--silencioso]
      runs dart run × [ddc+node or native] × dartforge on each program and prints the report
      (two oracle SDKs: DARTFORGE_DART_SDK, the 3.6.2 floor, and DARTFORGE_DART_SDK_3_13;
      each program goes to the smallest SDK that covers its `// requer-dart: x.y` and
      `// @dart=x.y`, and DartForge receives the same --versao-linguagem; `// erro-de-compilacao`
      marks a negative program; `PENDENTES` in the corpus directory lists what still fails
      because of an unimplemented feature — docs/VERSOES-LINGUAGEM.md §5)
      (--gc-stress: the native executable runs with DARTFORGE_GC_STRESS=1, collecting before
      every allocation — a program only passes if it also passes this way; docs/NATIVO-PLANO.md G7)
      (--otimizar: the native executable is the production one, `aot --optimize` — LTO and
      method-table pruning; docs/NATIVO-PODA-DE-TABELAS.md)
      (exit code 0 if all match; 1 if any fails)
      --fragmento K/N runs only shard K of N of the corpus (index % N == K-1;
      works in every mode — it is how CI splits the native corpus across machines)
      --producao adds the fourth executor: the production profile
      (dartforge-jsprod, single pruned file) — the report then compares
      VM × our development × our production (docs/JS-PRODUCAO.md)
      --jit runs the native backend through the ORCv2 JIT (dartforge-executar-ir, next
      to this binary or in DARTFORGE_RUN_IR) instead of AOT; --jit-aot
      also runs the AOT of the SAME IR, lists every program where the two
      diverge (JIT≠AOT) and measures the time of each profile (docs/JIT.md)
      --matriz FILE appends to FILE (TSV) the state of each program in the
      profile — suportado, recusado, divergente, não medido (supported, rejected,
      divergent, not measured); the profile is `--perfil NAME`, otherwise aot,
      aot-gc-stress, jit or js-dev (and js-prod with --producao). scripts/matrix.py
      merges the files into docs/MATRIZ.md (V02)
  dartforge-differential contrato [--corpus DIR] [-o FILE]
      compiles each program with dartdevc and writes docs/CONTRATO-DDC.md
  dartforge-differential verificar [--corpus DIR] [--filtro TEXT]
      only the oracles: each program must run under dart run and match ddc+node
  dartforge-differential determinismo [--nativo [--executar]] [--producao] [--filtro TEXT] [--trabalhadores 1,4,8]
      runs the same corpus with each number of workers and requires an
      identical report. With --nativo it compares the LLVM IR emitted for each
      program, without Clang, linking or running (DARTFORGE_IR_PARALLEL_MAX limits
      the concurrent emissions; default 2); --executar goes back to compiling and
      running everything and comparing the report (and the IR, with DARTFORGE_KEEP_IR=1)";

/// Digest of all the emitted LLVM IR, to compare runs.
///
/// Each program's `.ll` stays in `<output>/.df_tmp/*.ll` when
/// `DARTFORGE_KEEP_IR` is set. Sorts by path before hashing, otherwise the
/// directory read order would enter the digest and the test would report a
/// difference where there is none.
fn digest_ir(root: &std::path::Path) -> u64 {
    let mut files: Vec<PathBuf> = Vec::new();
    fn collect(dir: &std::path::Path, acc: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, acc);
            } else if p.extension().is_some_and(|x| x == "ll") {
                acc.push(p);
            }
        }
    }
    collect(root, &mut files);
    files.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    for a in &files {
        let name = a.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let contents = std::fs::read(a).unwrap_or_default();
        for b in name.as_bytes().iter().chain(contents.iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// Prints the first differing line of `a` and `b` on stderr. The `linha N:`,
/// `a: ` and `b: ` lines are picked up by CI (see the module docs).
fn print_first_difference(a: &str, b: &str) {
    let mut la = a.lines();
    let mut lb = b.lines();
    let mut n = 1;
    loop {
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => n += 1,
            (x, y) => {
                eprintln!("  linha {n}:");
                eprintln!("    a: {}", x.unwrap_or("(end)"));
                eprintln!("    b: {}", y.unwrap_or("(end)"));
                return;
            }
        }
    }
}

/// Determinism of the native backend without running (PESQUISA-OTIMIZACAO
/// §11): with each number of workers, emits the LLVM IR of all programs and
/// requires the same digest (or the same error) program by program. The IR is
/// the input of the object cache key: if it changes with the completion order,
/// the cache misses and Clang runs for nothing. The executable is a function
/// of the IR, of the Clang version and of the runtime `.lib`; what can change
/// with the completion order is our code, and it shows up entirely in the IR.
/// The executable's correctness is the business of `--nativo` without
/// `determinismo` (ESTADO.md §3.2).
fn ir_determinism(env: &Environment, programs: &[Program], workers: &[usize]) {
    let limiter = Limiter::from_env();
    eprintln!(
        "IR mode: emission only (no Clang, linking or running), at most {} concurrent emission(s)",
        limiter.max()
    );
    let mut reference: Option<(usize, Vec<ProgramIr>)> = None;
    let mut first: Option<(String, usize)> = None;
    for n in workers {
        let start = std::time::Instant::now();
        let current = emit_corpus_ir(programs, *n, &limiter);
        let sec = start.elapsed().as_secs_f64();
        // The `trabalhador(es)` lines are picked up by CI (see the module docs).
        match &reference {
            None => {
                eprintln!("{n} trabalhador(es): referência ({} programas, {sec:.1} s)", programs.len());
                reference = Some((*n, current));
            }
            Some((n0, r0)) => {
                let diff = ir_differences(r0, &current);
                if diff.is_empty() {
                    eprintln!("{n} trabalhador(es): idêntico a {n0} ({sec:.1} s)");
                    continue;
                }
                eprintln!("{n} trabalhador(es): LLVM IR DIFERENTE de {n0} em {} programa(s) ({sec:.1} s):", diff.len());
                for d in &diff {
                    eprintln!("  {d}");
                }
                if first.is_none() {
                    first = r0.iter().zip(&current).find(|(x, y)| x != y).map(|(x, _)| (x.name.clone(), *n));
                }
            }
        }
    }
    let Some((_, r0)) = reference else { return };
    print!("{}", format_ir_report(&r0));
    // The verdict lines are parsed by CI (see the module docs).
    match first {
        None => println!("determinismo (IR): idêntico com {workers:?} trabalhadores"),
        Some((name, n)) => {
            if let Some(p) = programs.iter().find(|p| p.name == name) {
                investigate_divergence(env, p, n, &limiter);
            }
            println!("determinismo (IR): FALHOU");
            std::process::exit(1);
        }
    }
}

/// For the first divergent program: emits it alone and then `n` times in
/// parallel, writes both texts to `target/diferencial/determinismo/` and
/// shows the first differing line. If the difference does not reproduce with
/// the program in isolation, it depends on the rest of the corpus (state
/// shared between emissions), and that is said too.
fn investigate_divergence(env: &Environment, program: &Program, n: usize, limiter: &Limiter) {
    // On-disk directory name: kept as is.
    let dir = env.work_dir.join("determinismo");
    let _ = std::fs::create_dir_all(&dir);
    let text = |r: Result<String, String>| r.unwrap_or_else(|e| format!("ERROR: {e}\n"));
    let alone = text(limiter.run(|| dartforge_native_ir(program)));
    let copies = vec![program.clone(); n.max(1)];
    let concurrent = in_parallel(&copies, n, |p| text(limiter.run(|| dartforge_native_ir(p))), |_| {});
    let other = concurrent.iter().find(|t| **t != alone).unwrap_or(&concurrent[0]);
    let (a, b) = (dir.join(format!("{}.1.ll", program.name)), dir.join(format!("{}.{n}.ll", program.name)));
    let _ = std::fs::write(&a, &alone);
    let _ = std::fs::write(&b, other);
    eprintln!("{}: emitted alone into {} and {n}× in parallel into {}", program.name, a.display(), b.display());
    if *other == alone {
        eprintln!("  identical in isolation: the difference depends on the rest of the corpus");
    } else {
        print_first_difference(&alone, other);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut env = Environment::detect();
    let mut corpus = env.root.join("corpus/js");
    let mut filter: Option<String> = None;
    let mut shard: Option<(usize, usize)> = None;
    let mut op = Options::default();
    let mut doc_output = env.root.join("docs/CONTRATO-DDC.md");
    let mut quiet = false;
    let mut mode = "report";
    let mut workers: Vec<usize> = vec![1, 4, 8];
    let mut execute = false;
    let mut matrix: Option<PathBuf> = None;
    let mut profile: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "contrato" | "verificar" | "determinismo" => mode = match args[i].as_str() {
                "contrato" => "contract",
                "verificar" => "verify",
                _ => "determinism",
            },
            "--trabalhadores" => {
                i += 1;
                workers = args[i]
                    .split(',')
                    .map(|n| n.trim().parse().expect("--trabalhadores 1,4,8"))
                    .collect();
            }
            "--corpus" => {
                i += 1;
                // Absolute: `dartforge` and `dart run` run in the program's
                // directory, and a path relative to the root would not be
                // found from there.
                corpus = std::path::absolute(&args[i]).unwrap_or_else(|_| PathBuf::from(&args[i]));
            }
            "--filtro" => {
                i += 1;
                filter = Some(args[i].clone());
            }
            "--fragmento" => {
                i += 1;
                shard = Some(corpus::parse_shard(&args[i]).unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(2)
                }));
            }
            "--matriz" => {
                i += 1;
                matrix = Some(PathBuf::from(&args[i]));
            }
            "--perfil" => {
                i += 1;
                profile = Some(args[i].clone());
            }
            "-o" => {
                i += 1;
                doc_output = PathBuf::from(&args[i]);
            }
            "--jobs" => {
                i += 1;
                op.threads = args[i].parse().expect("--jobs N");
            }
            "--limite" => {
                i += 1;
                env.timeout = Duration::from_secs(args[i].parse().expect("--limite SEC"));
            }
            "--limite-exec" => {
                i += 1;
                env.native_timeout =
                    Duration::from_secs(args[i].parse().expect("--limite-exec SEC"));
            }
            "--sem-forge" => op.with_forge = false,
            "--nativo" => op.native = true,
            // JIT: the same native mode (VM as the reference), run by
            // dartforge-executar-ir; --jit-aot also links the AOT of the same IR.
            "--jit" => (op.native, op.jit) = (true, true),
            "--jit-aot" => (op.native, op.jit, op.jit_aot) = (true, true, true),
            "--gc-stress" => env.gc_stress = true,
            "--otimizar" => env.optimize = true,
            "--executar" => execute = true,
            "--producao" => op.with_production = true,
            "--sem-cache" => env.use_cache = false,
            "--silencioso" => quiet = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => {
                eprintln!("unknown argument: {other}\n{USAGE}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    // In native mode each program becomes a process that allocates on its own
    // heap; six in parallel have already taken the whole machine's memory.
    // While the native backend is not stable, the default is two — whoever
    // wants more passes `--jobs` explicitly and takes the risk.
    // The same goes for the JS production profile: each program becomes a
    // `dartforge-jsprod` process, which loads the SDK and classifies the 7 MB
    // of `dart_sdk.js`. The machine has 7.7 GB and is shared.
    if (op.native || op.with_production) && op.threads == 0 {
        op.threads = 2;
    }
    let mut programs = list(&corpus, filter.as_deref());
    if let Some((k, n)) = shard {
        programs = corpus::shard(programs, k, n);
        eprintln!("shard {k}/{n}: {} programs", programs.len());
    }
    if programs.is_empty() {
        eprintln!("no programs in {}", corpus.display());
        std::process::exit(2);
    }
    match mode {
        "contract" => {
            let doc = contract::generate(&env, &programs);
            std::fs::write(&doc_output, doc).expect("write the document");
            println!("{} programs → {}", programs.len(), doc_output.display());
        }
        "determinism" if op.native && !execute => ir_determinism(&env, &programs, &workers),
        "determinism" => {
            // Without `--nativo`, or with `--executar`: the whole report.
            // mold's determinism test (PESQUISA-OTIMIZACAO §11): the same
            // input and the same configuration must give the same output with
            // any number of workers. Here that applies to the report and, in
            // native mode, to the emitted LLVM IR — if the completion order
            // changes the IR, the hash-keyed object cache misses and Clang
            // runs for nothing, which is precisely what dominates compile time.
            // The `.ll` only survives with DARTFORGE_KEEP_IR set, and the
            // variable has to come from the environment: `set_var` is `unsafe`
            // since the 2024 edition (another thread may be reading the
            // environment) and the workspace denies `unsafe_code`. Without it,
            // the IR comparison is skipped and that is said here.
            let compare_ir = op.native && std::env::var_os("DARTFORGE_KEEP_IR").is_some();
            if op.native && !compare_ir {
                // Picked up by CI (`aviso`; see the module docs).
                eprintln!("aviso: defina DARTFORGE_KEEP_IR=1 para comparar tambem o LLVM IR emitido");
            }
            let mut reference: Option<(usize, String, u64)> = None;
            let mut same = true;
            for n in &workers {
                op.threads = *n;
                let results = run_corpus(&env, &programs, op, |_| {});
                let text = format_report(&results);
                let ir = if compare_ir { digest_ir(&env.work_dir.join("nativo")) } else { 0 };
                // The `trabalhador(es)` lines are picked up by CI (see the module docs).
                match &reference {
                    None => {
                        eprintln!("{n} trabalhador(es): referência ({} programas)", programs.len());
                        reference = Some((*n, text, ir));
                    }
                    Some((n0, t0, ir0)) => {
                        let report_ok = *t0 == text;
                        let ir_ok = *ir0 == ir;
                        if report_ok && ir_ok {
                            eprintln!("{n} trabalhador(es): idêntico a {n0}");
                        } else {
                            same = false;
                            if !report_ok {
                                eprintln!("{n} trabalhador(es): RELATÓRIO DIFERENTE de {n0}");
                                print_first_difference(t0, &text);
                            }
                            if !ir_ok {
                                eprintln!("{n} trabalhador(es): LLVM IR DIFERENTE de {n0} ({ir0:016x} != {ir:016x})");
                            }
                        }
                    }
                }
            }
            // The verdict lines are parsed by CI (see the module docs).
            if same {
                println!("determinismo: saída idêntica com {workers:?} trabalhadores");
            } else {
                println!("determinismo: FALHOU");
                std::process::exit(1);
            }
        }
        _ => {
            if mode == "verify" {
                op.with_forge = false;
            }
            if op.with_forge {
                match &env.dartforge_bin {
                    Some(b) => eprintln!("dartforge: {}", b.display()),
                    None => eprintln!("dartforge: binary not found; using `cargo run -p dartforge-cli`"),
                }
            }
            let start = std::time::Instant::now();
            let results = run_corpus(&env, &programs, op, |r| {
                if quiet {
                    return;
                }
                // The same status tags as the report (`report.rs`).
                let status = if r.program.pending && r.forge.is_some() {
                    if r.ok() { "PASSED" } else { "PEND" }
                } else if r.production.is_some() {
                    if r.ok() { "ok" } else if r.forge_vs_reference().is_some() { "FALHA" } else { "PROD!" }
                } else if r.forge.is_some() {
                    if r.ok() { "ok" } else { "FALHA" }
                } else if r.dart.code != 0 {
                    "DART!"
                } else if r.native {
                    "DART"
                } else if r.ddc_vs_dart().is_none() == r.program.diverge_ddc.is_none() {
                    "ok"
                } else {
                    "DDC≠VM"
                };
                eprintln!("  {status:<6} {}", r.program.name);
            });
            print!("{}", format_report(&results));
            // Parsed by CI (see the module docs).
            println!("({} programas em {:.1} s)", results.len(), start.elapsed().as_secs_f64());
            if let Some(file) = &matrix {
                // The four native combinations (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
                // §7.1): the roots mode and the exceptions mode come from the
                // environment, which the emitter reads (`DARTFORGE_ROOTS`,
                // `DARTFORGE_EXCEPTIONS`); the default (shadow stack, checks)
                // does not enter the name. The profile names are values of the
                // TSV read by `scripts/matrix.py` and stay as they are.
                let suffix = {
                    let mut m = String::new();
                    if std::env::var("DARTFORGE_ROOTS").is_ok_and(|v| v == "mapas") {
                        m.push_str("-mapas");
                    }
                    if std::env::var("DARTFORGE_EXCEPTIONS").is_ok_and(|v| v == "tabelas") {
                        m.push_str("-tabelas");
                    }
                    m
                };
                let name = profile.clone().unwrap_or_else(|| match (op.native, op.jit, env.gc_stress) {
                    (true, true, _) => format!("jit{suffix}"),
                    (true, false, true) => format!("aot{suffix}-gc-stress"),
                    (true, false, false) if env.optimize => format!("aot{suffix}-otimizado"),
                    (true, false, false) => format!("aot{suffix}"),
                    (false, ..) => "js-dev".to_string(),
                });
                let lines = dartforge_differential::matrix::tsv(&results, &name);
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)
                    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
                std::io::Write::write_all(&mut f, lines.as_bytes()).expect("write the matrix");
            }
            let all_ok = if op.with_forge {
                // A pending program (`PENDENTES`) must fail; the others, pass.
                results.iter().all(|r| r.ok() != r.program.pending)
            } else if op.native {
                results.iter().all(|r| r.dart.code == 0)
            } else {
                results.iter().all(|r| r.dart.code == 0 && (r.ddc_vs_dart().is_none() != r.program.diverge_ddc.is_some()))
            };
            if !all_ok {
                std::process::exit(1);
            }
        }
    }
}
