//! Corpus tests (they need `dart` and `node` on the PATH; run with `--ignored`):
//!
//! ```
//! cargo test -p dartforge-differential --test corpus -- --ignored --nocapture
//! ```

use dartforge_differential::{Environment, Options, format_report, list, run_corpus};

fn programs(env: &Environment) -> Vec<dartforge_differential::Program> {
    let p = list(&env.root.join("corpus/js"), None);
    assert!(p.len() >= 120, "the corpus has {} programs; minimum 120", p.len());
    p
}

/// The whole harness on the corpus: prints the report (DartForge included when
/// the binary exists) and fails if any program does not run under `dart run` —
/// the oracle must be valid.
#[test]
#[ignore]
fn corpus_runs_under_dart_run() {
    let env = Environment::detect();
    let programs = programs(&env);
    let op = Options { with_forge: env.dartforge_bin.is_some(), threads: 0, ..Default::default() };
    let results = run_corpus(&env, &programs, op, |_| {});
    print!("{}", format_report(&results));
    let invalid: Vec<String> = results
        .iter()
        .filter(|r| r.dart.code != 0)
        .map(|r| format!("{} (exit code {}): {}", r.program.name, r.dart.code, r.dart.first_stderr_line()))
        .collect();
    assert!(invalid.is_empty(), "programs that do not run under dart run:\n{}", invalid.join("\n"));
}

/// `dartdevc`+Node reproduces the VM on 100% of the corpus, except the
/// programs whose header declares `// diverge-ddc: reason` — those must
/// actually diverge.
#[test]
#[ignore]
fn ddc_matches_dart_run() {
    let env = Environment::detect();
    let programs = programs(&env);
    let results = run_corpus(&env, &programs, Options { with_forge: false, threads: 0, ..Default::default() }, |_| {});
    let mut problems = Vec::new();
    for r in &results {
        match (r.ddc_vs_dart(), &r.program.diverge_ddc) {
            (Some(d), None) => problems.push(format!("{}: DDC ≠ VM ({d:?}); ddc stderr: {}", r.program.name, r.ddc.first_stderr_line())),
            (None, Some(m)) => problems.push(format!("{}: declares a divergence ({m}) but the outputs match", r.program.name)),
            _ => {}
        }
    }
    let declared: Vec<&str> = results.iter().filter_map(|r| r.program.diverge_ddc.as_ref().map(|m| m.as_str())).collect();
    println!("{} programs; {} with a declared divergence: {:?}", results.len(), declared.len(), declared);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
