//! Byte-for-byte comparison and the text report.
//!
//! Several report lines are parsed by CI (`scripts/ci-placar.ps1`,
//! `.github/actions/harness/action.yml`, `.github/workflows/*.yml`): the
//! status tags `DART!` and `FALHA`, the scoreboard lines (`DartForge …: N/M
//! ok`, `Pendentes (PENDENTES): …`, `DDC×VM: …`), the failure-group headings
//! (`Falhas por primeira linha do stderr …`), the `--jit-aot` section
//! (`JIT × AOT: …`, `JIT≠AOT `, `Tempos pós-IR`, `  JIT `/`  AOT `,
//! `  razão `, `  tempo esgotado nos dois`) and the IR report footer
//! (`N programas: …`). Those texts stay in Portuguese, byte for byte, until
//! the CI scripts are migrated with them.

use std::collections::BTreeMap;
use std::fmt::Write;

use dartforge_emit_native::resumo::ResumoIr;

use crate::corpus::Program;
use crate::process::Output;

/// Result of a program across the executors. `forge` is `None` with
/// `--sem-forge`; `production` only exists with `--producao`.
#[derive(Debug, Clone)]
pub struct ProgramResult {
    pub program: Program,
    pub dart: Output,
    pub ddc: Output,
    pub forge: Option<Output>,
    /// `forge` is the native backend, not the development JS.
    pub native: bool,
    /// The JS production profile (`dartforge-jsprod`, single pruned file).
    pub production: Option<Output>,
    /// With `--jit`/`--jit-aot`: `forge` came from the ORCv2 JIT, and this
    /// holds its time and, with `--jit-aot`, the AOT of the same IR.
    pub jit: Option<JitRun>,
}

/// What JIT mode records for a program, besides the output.
#[derive(Debug, Clone)]
pub struct JitRun {
    /// The emitter produced IR (otherwise no profile ran anything).
    pub has_ir: bool,
    /// Wall-clock time of the executor process: loading `LLVM-C.dll`, opening
    /// the session, generating the code and running the program.
    pub time: std::time::Duration,
    /// Only the program's run inside the executor (`execute_ns` of
    /// `--timings`), when the executor got to run it.
    pub run_time: Option<std::time::Duration>,
    /// With `--jit-aot`: the same IR text through the AOT driver.
    pub aot: Option<AotFromSameIr>,
}

/// The AOT executable linked from the same IR the JIT ran.
#[derive(Debug, Clone)]
pub struct AotFromSameIr {
    pub output: Output,
    /// Clang + linking (`driver::compile_and_link`), measured around the call.
    pub link_time: std::time::Duration,
    /// Running the executable.
    pub run_time: std::time::Duration,
    /// The object came from the object cache (then Clang did not run).
    pub object_from_cache: bool,
}

/// Where two outputs diverge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergence {
    /// Identical stdout, different exit code.
    ExitCode,
    /// First (1-based) stdout line that differs (or that exists on one side only).
    Line(usize),
}

/// Removes from the grouping key whatever changes from run to run.
///
/// Two reasons. The first is determinism: the report must be identical with 1,
/// 4 and 8 workers, and a process or thread id in the message makes the text
/// change on its own. The second is usefulness: a runtime panic carries
/// `thread '<unnamed>' (11220) panicked at ...`, and the number split ONE
/// defect into several groups of two programs, hiding its real size.
///
/// Only numbers in parentheses right after `thread` and paths of the runtime
/// cache (which have the source hash in the name) are normalized; the rest of
/// the line stays as it is, because it is what identifies the defect.
pub fn failure_key(s: &Output) -> String {
    let mut lines = s
        .stderr
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty());
    let Some(first) = lines.next() else {
        return "(empty stderr)".to_string();
    };
    // A Rust panic has the useful message on the SECOND line; the first is
    // `thread '<unnamed>' (11220) panicked at <path>:346:14:`, which only
    // names the runtime cache file. Grouping by it puts 89 programs in a
    // single group and hides which defect it is.
    if first.starts_with("thread '") && first.contains("panicked at") {
        if let Some(msg) = lines.next() {
            return format!("runtime panic: {}", stabilize_key(msg));
        }
    }
    stabilize_key(first)
}

pub fn stabilize_key(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        // `runtime_cc150463840c488f.rs` -> `runtime_<hash>.rs`
        if let Some(tail) = rest.strip_prefix("runtime_") {
            let hex: usize = tail.chars().take_while(|c| c.is_ascii_hexdigit()).count();
            if hex > 0 {
                out.push_str("runtime_<hash>");
                rest = &tail[hex..];
                continue;
            }
        }
        // `thread '<unnamed>' (11220) panicked` -> `thread '<unnamed>' (N) panicked`
        if let Some(tail) = rest.strip_prefix('(') {
            let dig: usize = tail.chars().take_while(|c| c.is_ascii_digit()).count();
            if dig > 0 && tail[dig..].starts_with(')') {
                out.push_str("(N)");
                rest = &tail[dig + 1..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `None` when stdout and exit code are identical.
pub fn compare(expected: &Output, actual: &Output) -> Option<Divergence> {
    if expected.stdout != actual.stdout {
        return Some(Divergence::Line(first_differing_line(&expected.stdout, &actual.stdout)));
    }
    if expected.code != actual.code {
        return Some(Divergence::ExitCode);
    }
    None
}

fn first_differing_line(a: &str, b: &str) -> usize {
    let mut la = a.split_inclusive('\n');
    let mut lb = b.split_inclusive('\n');
    let mut n = 1;
    loop {
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => n += 1,
            _ => return n,
        }
    }
}

impl ProgramResult {
    /// DartForge's reference output (the VM, or DDC if the header declares a divergence).
    pub fn reference(&self) -> &Output {
        if !self.native && self.program.reference_is_ddc() { &self.ddc } else { &self.dart }
    }

    /// DDC × VM divergence (only relevant when the header does not declare it).
    pub fn ddc_vs_dart(&self) -> Option<Divergence> {
        compare(&self.dart, &self.ddc)
    }

    /// DartForge × reference divergence; also `None` when DartForge was not run.
    pub fn forge_vs_reference(&self) -> Option<Divergence> {
        self.forge.as_ref().and_then(|f| compare(self.reference(), f))
    }

    /// Production × reference divergence.
    pub fn production_vs_reference(&self) -> Option<Divergence> {
        self.production.as_ref().and_then(|f| compare(self.reference(), f))
    }

    /// Production × development divergence. This is the one the production
    /// profile cares about most: if it shows up, it was pruning (or the
    /// assembly of the single file) that changed the behavior, not emission.
    pub fn production_vs_forge(&self) -> Option<Divergence> {
        match (&self.forge, &self.production) {
            (Some(d), Some(p)) => compare(d, p),
            _ => None,
        }
    }

    /// `true` when DartForge reproduces the reference — in both profiles that
    /// were run.
    pub fn ok(&self) -> bool {
        self.forge.is_some()
            && self.forge_vs_reference().is_none()
            && self.production.as_ref().is_none_or(|_| self.production_vs_reference().is_none())
    }
}

const COLUMN_WIDTH: usize = 38;
const LINES_BEFORE: usize = 2;
const LINES_AFTER: usize = 6;

fn truncate(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        t.pop();
        t.push('…');
    }
    t
}

/// Three stdouts side by side, a window around line `focus` (1-based).
pub fn side_by_side(columns: &[(&str, &str)], focus: usize) -> String {
    let lines: Vec<Vec<&str>> = columns.iter().map(|(_, s)| s.lines().collect()).collect();
    let max = lines.iter().map(Vec::len).max().unwrap_or(0);
    let start = focus.saturating_sub(1).saturating_sub(LINES_BEFORE);
    let end = (focus + LINES_AFTER).min(max);
    let mut out = String::new();
    let head: Vec<String> = columns.iter().map(|(n, _)| format!("{:<w$}", truncate(n, COLUMN_WIDTH), w = COLUMN_WIDTH)).collect();
    let _ = writeln!(out, "         {}", head.join(" | "));
    for i in start..end {
        let mark = if i + 1 == focus { ">" } else { " " };
        let cells: Vec<String> = lines
            .iter()
            .map(|l| format!("{:<w$}", truncate(l.get(i).copied().unwrap_or("∅"), COLUMN_WIDTH), w = COLUMN_WIDTH))
            .collect();
        let _ = writeln!(out, "  {mark}{:>4}  {}", i + 1, cells.join(" | "));
    }
    if end < max {
        let _ = writeln!(out, "         … ({} lines in total)", max);
    }
    out
}

/// The report: one line per program, details on failures, summary and grouping.
pub fn format_report(results: &[ProgramResult]) -> String {
    let mut out = String::new();
    let with_forge = results.iter().any(|r| r.forge.is_some());
    let native = results.first().map_or(false, |r| r.native);
    let mut ok = 0usize;
    let mut ddc_warnings = 0usize;
    let mut ok_prod = 0usize;
    let with_production = results.iter().any(|r| r.production.is_some());
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut prod_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut pending_failing = 0usize;
    let mut pending_passed: Vec<String> = Vec::new();
    // Native: the unsupported constructs of each program that did not
    // compile, all the diagnostics (the failure key only shows the first).
    let mut constructs: Vec<(&str, Vec<String>)> = Vec::new();
    for r in results {
        let name = &r.program.name;
        if r.dart.code != 0 {
            let _ = writeln!(out, "DART!  {name}  (dart run exited with {}: {})", r.dart.code, truncate(r.dart.first_stderr_line(), 90));
        }
        if !native {
            match (r.ddc_vs_dart(), &r.program.diverge_ddc) {
                (Some(d), None) => {
                    ddc_warnings += 1;
                    let _ = writeln!(out, "DDC≠VM {name}  ({})", describe(d));
                    let _ = write!(out, "{}", side_by_side(&[("dart run", &r.dart.stdout), ("ddc+node", &r.ddc.stdout)], focus(d)));
                    let _ = writeln!(out, "       exit codes: dart={} ddc={}  ddc stderr: {}", r.dart.code, r.ddc.code, truncate(r.ddc.first_stderr_line(), 80));
                }
                (None, Some(reason)) => {
                    let _ = writeln!(out, "WARN   {name}  header declares a DDC divergence but the outputs match: {reason}");
                }
                _ => {}
            }
        }
        let Some(forge) = &r.forge else { continue };
        if r.program.pending {
            // A feature not implemented yet (`PENDENTES`): failing is
            // expected and does not enter the groups; passing fails the run,
            // so that the list gets updated.
            if r.ok() {
                ok += 1;
                pending_passed.push(name.clone());
                let _ = writeln!(out, "PASSED {name}  (listed in PENDENTES: remove it from the list)");
            } else {
                pending_failing += 1;
                let _ = writeln!(out, "PEND   {name}");
            }
            continue;
        }
        match r.forge_vs_reference() {
            None => {
                ok += 1;
                if r.production.is_none() {
                    let _ = writeln!(out, "ok     {name}");
                }
            }
            Some(d) => {
                let _ = writeln!(out, "FALHA  {name}  ({})", describe(d));
                let columns: Vec<(&str, &str)> = if native {
                    vec![("dart run", &r.dart.stdout), ("dartforge native", &forge.stdout)]
                } else {
                    vec![("dart run", &r.dart.stdout), ("ddc+node", &r.ddc.stdout), ("dartforge", &forge.stdout)]
                };
                let _ = write!(out, "{}", side_by_side(&columns, focus(d)));
                let codes = if native {
                    format!("exit codes: dart={} forge={}", r.dart.code, forge.code)
                } else {
                    format!("exit codes: dart={} ddc={} forge={}", r.dart.code, r.ddc.code, forge.code)
                };
                let _ = writeln!(out, "       {codes}");
                let key = truncate(&failure_key(forge), 120);
                let _ = writeln!(out, "       stderr: {key}");
                groups.entry(key).or_default().push(name.clone());
                if native {
                    let cs = dartforge_emit_native::construtos_do_erro(&forge.stderr);
                    if !cs.is_empty() {
                        let _ = writeln!(out, "       constructs: {}", describe_constructs(&cs));
                        constructs.push((name.as_str(), cs));
                    }
                }
            }
        }
        let Some(prod) = &r.production else { continue };
        match r.production_vs_reference() {
            None => {
                ok_prod += 1;
                let _ = writeln!(out, "ok     {name}");
            }
            Some(d) => {
                // Tells apart the defect that matters: production differing
                // from the other executor is a **pruning** defect; production
                // differing only from the VM, with the other one equal, is
                // too — but grouping is done by the failure key.
                let blame = if r.production_vs_forge().is_some() { "production≠executor" } else { "production≠VM" };
                let _ = writeln!(out, "PROD!  {name}  ({}, {blame})", describe(d));
                let label = if native { "dartforge native" } else { "dartforge dev" };
                let _ = write!(out, "{}", side_by_side(&[("dart run", &r.dart.stdout), (label, &forge.stdout), ("dartforge prod", &prod.stdout)], focus(d)));
                let _ = writeln!(out, "       exit codes: dart={} other={} prod={}", r.dart.code, forge.code, prod.code);
                // The same key as the main executor, and for the same reason:
                // a `dartforge-jsprod` that panics carries the thread id on
                // the first line, which would split one defect into several
                // groups and make the report change from run to run.
                let key = truncate(&failure_key(prod), 120);
                let _ = writeln!(out, "       stderr: {key}");
                prod_groups.entry(key).or_default().push(name.clone());
            }
        }
    }
    let total = results.len();
    let _ = writeln!(out);
    // The scoreboard lines below are parsed by CI (see the module docs).
    if with_forge {
        let jit = results.iter().any(|r| r.jit.is_some());
        let label = if jit {
            "DartForge JIT:             "
        } else if native {
            "DartForge Nativo:          "
        } else {
            "DartForge desenvolvimento: "
        };
        let _ = writeln!(out, "{label}{ok}/{total} ok");
    }
    if with_production {
        let _ = writeln!(out, "DartForge produção:        {ok_prod}/{total} ok");
    }
    if pending_failing + pending_passed.len() > 0 {
        let _ = writeln!(
            out,
            "Pendentes (PENDENTES): {} ainda falham, {} passaram{}",
            pending_failing,
            pending_passed.len(),
            if pending_passed.is_empty() { String::new() } else { format!(" — retire da lista: {}", pending_passed.join(", ")) }
        );
    }
    if !native {
        let _ = writeln!(out, "DDC×VM: {}/{total} batem (sem contar {} com divergência declarada)", total - ddc_warnings - results.iter().filter(|r| r.program.diverge_ddc.is_some()).count(), results.iter().filter(|r| r.program.diverge_ddc.is_some()).count());
    }
    // Two groupings, because the causes differ: a failure of the main
    // executor is a construct missing in the emitter; a production failure is
    // pruning or the assembly of the single file.
    let forge_title = if results.iter().any(|r| r.jit.is_some()) {
        "do DartForge JIT"
    } else if native {
        "do DartForge Nativo"
    } else {
        "do DartForge"
    };
    for (title, groups) in [(forge_title, groups), ("do perfil de produção", prod_groups)] {
        if groups.is_empty() {
            continue;
        }
        let mut list: Vec<(String, Vec<String>)> = groups.into_iter().collect();
        list.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        let _ = writeln!(out, "\nFalhas por primeira linha do stderr {title} (prioridade = tamanho do grupo):");
        for (key, names) in list {
            let _ = writeln!(out, "  {:>4}  {key}", names.len());
            let _ = writeln!(out, "        {}", names.join(", "));
        }
    }
    out.push_str(&constructs_section(&constructs));
    out.push_str(&jit_aot_section(results));
    out
}

/// A program's distinct constructs, in order of first occurrence, with the
/// count when they repeat: ``closure ×3, chamada `sort` ``.
fn describe_constructs(cs: &[String]) -> String {
    let mut distinct: Vec<(&str, usize)> = Vec::new();
    for c in cs {
        match distinct.iter_mut().find(|(d, _)| *d == c) {
            Some((_, n)) => *n += 1,
            None => distinct.push((c, 1)),
        }
    }
    let parts: Vec<String> =
        distinct.iter().map(|(c, n)| if *n > 1 { format!("{c} ×{n}") } else { (*c).to_string() }).collect();
    parts.join(", ")
}

/// The "constructs (all diagnostics)" section: the exact matrix of what is missing.
///
/// The failure key groups by the first line, which is the program's **first**
/// unsupported construct; it hides the others. This section has every
/// diagnostic of each program: per construct, how many programs use it and
/// how many occurrences they add up to, and — the number that orders the
/// work — the programs blocked **only** by it (the ones that would start
/// compiling if it alone were added). Order: programs, then occurrences, then
/// the text. Empty when no program has a construct diagnostic.
pub fn constructs_section(programs: &[(&str, Vec<String>)]) -> String {
    let mut by: BTreeMap<&str, (Vec<&str>, usize)> = BTreeMap::new();
    let mut only: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, cs) in programs {
        let mut distinct: Vec<&str> = cs.iter().map(String::as_str).collect();
        distinct.sort_unstable();
        distinct.dedup();
        for c in cs {
            by.entry(c.as_str()).or_default().1 += 1;
        }
        for c in &distinct {
            by.entry(*c).or_default().0.push(name);
        }
        if let [single] = distinct[..] {
            only.entry(single).or_default().push(name);
        }
    }
    if by.is_empty() {
        return String::new();
    }
    let mut list: Vec<(&str, Vec<&str>, usize)> = by.into_iter().map(|(c, (p, n))| (c, p, n)).collect();
    list.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\nConstructs (all diagnostics): {} programs with diagnostics, {} distinct constructs, {} blocked by a single construct",
        programs.len(),
        list.len(),
        only.values().map(Vec::len).sum::<usize>()
    );
    let _ = writeln!(out, "   programs  occurrences  only it  construct");
    for (c, progs, n) in &list {
        let only_it = only.get(c).map_or(&[][..], Vec::as_slice);
        let _ = writeln!(out, "  {:>9}  {:>11}  {:>7}  {c}", progs.len(), n, only_it.len());
        if !only_it.is_empty() {
            let _ = writeln!(out, "             only it: {}", only_it.join(", "));
        }
    }
    out
}

/// The `--jit-aot` section: JIT × AOT agreement program by program, and the times.
///
/// The contract of `crates/jit` is that the same IR gives the same stdout and
/// the same exit code in both profiles, **including when both fail**. That is
/// why the comparison is between the two, and not of each one with the VM
/// (that is the scoreboard). The line `JIT × AOT: … N divergentes` is what CI
/// requires to be zero.
///
/// A program that times out **in both** does not count as a divergence: the
/// partial stdout of an endless loop depends on how long each one ran (and the
/// JIT has the code generation slack). It is listed separately.
///
/// The times are wall-clock, only of programs that finished in both
/// profiles. «JIT until run» is the executor process minus the program's run
/// (`execute_ns`): DLL loading, session, IR parsing and code generation —
/// what corresponds to AOT's Clang + linking.
fn jit_aot_section(results: &[ProgramResult]) -> String {
    use crate::process::TIMEOUT_CODE;
    let pairs: Vec<(&ProgramResult, &Output, &JitRun, &AotFromSameIr)> = results
        .iter()
        .filter_map(|r| {
            let jit = r.jit.as_ref()?;
            Some((r, r.forge.as_ref()?, jit, jit.aot.as_ref()?))
        })
        .collect();
    if pairs.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut divergent = Vec::new();
    let mut both_timed_out = Vec::new();
    for (r, jit, _, aot) in &pairs {
        if jit.code == TIMEOUT_CODE && aot.output.code == TIMEOUT_CODE {
            both_timed_out.push(r.program.name.as_str());
        } else if jit.stdout != aot.output.stdout || jit.code != aot.output.code {
            divergent.push((*r, *jit, &aot.output));
        }
    }
    let without_ir = pairs.iter().filter(|(_, _, j, _)| !j.has_ir).count();
    // Parsed by CI (see the module docs).
    let _ = writeln!(
        out,
        "\nJIT × AOT: {}/{} idênticos, {} divergentes ({} sem IR, iguais por construção; {} estouraram o tempo nos dois)",
        pairs.len() - divergent.len(),
        pairs.len(),
        divergent.len(),
        without_ir,
        both_timed_out.len()
    );
    if !both_timed_out.is_empty() {
        let _ = writeln!(out, "  tempo esgotado nos dois: {}", both_timed_out.join(", "));
    }
    for (r, jit, aot) in &divergent {
        let _ = writeln!(out, "JIT≠AOT {}  (exit codes: jit={} aot={})", r.program.name, jit.code, aot.code);
        let first = first_differing_line(&jit.stdout, &aot.stdout);
        let _ = write!(out, "{}", side_by_side(&[("jit", &jit.stdout), ("aot", &aot.stdout)], first));
        let _ = writeln!(out, "       jit stderr: {}", truncate(&failure_key(jit), 110));
        let _ = writeln!(out, "       aot stderr: {}", truncate(&failure_key(aot), 110));
    }

    // Only the ones that finished in both and have the run measured in the executor.
    let measured: Vec<_> = pairs
        .iter()
        .filter(|(_, jit, j, a)| {
            j.has_ir
                && j.run_time.is_some()
                && jit.code != TIMEOUT_CODE
                && a.output.code != TIMEOUT_CODE
        })
        .collect();
    if measured.is_empty() {
        return out;
    }
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    let summary = |mut v: Vec<f64>| -> String {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let sum: f64 = v.iter().sum();
        let p = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
        format!("median {:>8.1} ms  p95 {:>8.1} ms  sum {:>8.1} s", p(0.5), p(0.95), sum / 1000.0)
    };
    let run_time = |j: &JitRun| j.run_time.unwrap_or_default();
    let until_run: Vec<f64> = measured.iter().map(|(_, _, j, _)| ms(j.time.saturating_sub(run_time(j)))).collect();
    let jit_run: Vec<f64> = measured.iter().map(|(_, _, j, _)| ms(run_time(j))).collect();
    let jit_total: Vec<f64> = measured.iter().map(|(_, _, j, _)| ms(j.time)).collect();
    let link: Vec<f64> = measured.iter().map(|(_, _, _, a)| ms(a.link_time)).collect();
    let aot_run: Vec<f64> = measured.iter().map(|(_, _, _, a)| ms(a.run_time)).collect();
    let aot_total: Vec<f64> = measured.iter().map(|(_, _, _, a)| ms(a.link_time + a.run_time)).collect();
    let from_cache = measured.iter().filter(|(_, _, _, a)| a.object_from_cache).count();
    let sum = |v: &[f64]| v.iter().sum::<f64>();
    // The heading, the `  JIT `/`  AOT ` prefixes and the `  razão ` line are
    // matched by CI (see the module docs).
    let _ = writeln!(
        out,
        "\nTempos pós-IR, {} programas que terminaram nos dois perfis ({} com objeto do cache de objetos):",
        measured.len(),
        from_cache
    );
    let _ = writeln!(out, "  {:<51}{}", "JIT until run (DLL + session + IR + codegen)", summary(until_run.clone()));
    let _ = writeln!(out, "  {:<51}{}", "AOT Clang + linking", summary(link.clone()));
    let _ = writeln!(out, "  {:<51}{}", "JIT program run (inside the process)", summary(jit_run));
    let _ = writeln!(out, "  {:<51}{}", "AOT .exe run (whole process)", summary(aot_run));
    let _ = writeln!(out, "  {:<51}{}", "JIT total (executor process)", summary(jit_total.clone()));
    let _ = writeln!(out, "  {:<51}{}", "AOT total (Clang + linking + run)", summary(aot_total.clone()));
    let _ = writeln!(
        out,
        "  razão (somas): Clang+ligação / JIT até executar = {:.1}x; AOT total / JIT total = {:.1}x",
        sum(&link) / sum(&until_run).max(f64::EPSILON),
        sum(&aot_total) / sum(&jit_total).max(f64::EPSILON)
    );
    let _ = writeln!(out, "\nPer-program times (ms): jit-until-run  jit-run  aot-link  aot-run  name");
    for (r, _, j, a) in &measured {
        let cache = if a.object_from_cache { "  (object from cache)" } else { "" };
        let _ = writeln!(
            out,
            "  {:>10.1} {:>13.1} {:>12.1} {:>13.1}  {}{cache}",
            ms(j.time.saturating_sub(run_time(j))),
            ms(run_time(j)),
            ms(a.link_time),
            ms(a.run_time),
            r.program.name
        );
    }
    out
}

/// The LLVM IR emitted for a program, summarized (determinism mode without running).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramIr {
    pub name: String,
    /// The IR summary, or the whole emission error message — an error is a
    /// result too, and it too must be the same with any number of workers.
    pub result: Result<ResumoIr, String>,
}

/// One line per program, in corpus order: `<hash> <bytes> <name>` or
/// `ERROR <name>: <failure key>`, and the footer with the count. It has no
/// times or paths, so it can be compared between runs and between emitter
/// versions (a `diff` of two reports says which programs changed IR).
pub fn format_ir_report(programs: &[ProgramIr]) -> String {
    let mut out = String::new();
    let mut with_ir = 0usize;
    let mut constructs: Vec<(&str, Vec<String>)> = Vec::new();
    for p in programs {
        match &p.result {
            Ok(r) => {
                with_ir += 1;
                let _ = writeln!(out, "{} {:>9} {}", r.hex(), r.bytes, p.name);
            }
            Err(e) => {
                let key = truncate(&failure_key(&Output { stdout: String::new(), stderr: e.clone(), code: 1 }), 120);
                let _ = writeln!(out, "ERROR {}: {key}", p.name);
                let cs = dartforge_emit_native::construtos_do_erro(e);
                if !cs.is_empty() {
                    constructs.push((p.name.as_str(), cs));
                }
            }
        }
    }
    // Parsed by CI (see the module docs).
    let _ = writeln!(out, "\n{} programas: {with_ir} com IR, {} com erro de emissão", programs.len(), programs.len() - with_ir);
    out.push_str(&constructs_section(&constructs));
    out
}

/// Every program whose result differs between two runs (empty = identical).
/// Compares the whole result, including the full error message, and not just
/// the line the report shows.
pub fn ir_differences(a: &[ProgramIr], b: &[ProgramIr]) -> Vec<String> {
    let mut out = Vec::new();
    if a.len() != b.len() {
        out.push(format!("number of programs: {} × {}", a.len(), b.len()));
    }
    for (x, y) in a.iter().zip(b) {
        if x.name != y.name {
            out.push(format!("corpus order: {} × {}", x.name, y.name));
        } else if x.result != y.result {
            out.push(format!("{}: {} × {}", x.name, describe_ir(&x.result), describe_ir(&y.result)));
        }
    }
    out
}

fn describe_ir(r: &Result<ResumoIr, String>) -> String {
    match r {
        Ok(r) => format!("{} ({} bytes)", r.hex(), r.bytes),
        Err(e) => format!("error «{}»", truncate(e.lines().next().unwrap_or(""), 80)),
    }
}

fn describe(d: Divergence) -> String {
    match d {
        Divergence::ExitCode => "exit code".into(),
        Divergence::Line(n) => format!("stdout line {n}"),
    }
}

fn focus(d: Divergence) -> usize {
    match d {
        Divergence::ExitCode => 1,
        Divergence::Line(n) => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(stdout: &str, code: i32) -> Output {
        Output { stdout: stdout.into(), stderr: String::new(), code }
    }

    #[test]
    fn compares() {
        assert_eq!(compare(&s("a\nb\n", 0), &s("a\nb\n", 0)), None);
        assert_eq!(compare(&s("a\nb\n", 0), &s("a\nc\n", 0)), Some(Divergence::Line(2)));
        assert_eq!(compare(&s("a\nb\n", 0), &s("a\n", 0)), Some(Divergence::Line(2)));
        assert_eq!(compare(&s("a\n", 0), &s("a\n", 255)), Some(Divergence::ExitCode));
        assert_eq!(compare(&s("a\nb", 0), &s("a\nb\n", 0)), Some(Divergence::Line(2)));
    }

    #[test]
    fn report_groups() {
        let p = Program::test;
        let r = vec![
            ProgramResult { program: p("a"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(s("1\n", 0)), native: false, production: None, jit: None },
            ProgramResult { program: p("b"), dart: s("1\n2\n", 0), ddc: s("1\n2\n", 0), forge: Some(Output { stdout: "1\n".into(), stderr: "erro: X\n".into(), code: 1 }), native: false, production: None, jit: None },
            ProgramResult { program: p("c"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(Output { stdout: String::new(), stderr: "erro: X\n".into(), code: 1 }), native: false, production: None, jit: None },
        ];
        let t = format_report(&r);
        assert!(t.contains("ok     a"), "{t}");
        assert!(t.contains("FALHA  b  (stdout line 2)"), "{t}");
        assert!(t.contains("DartForge desenvolvimento: 1/3 ok"), "{t}");
        assert!(t.contains("   2  erro: X"), "{t}");
        assert!(t.contains("b, c"), "{t}");
    }

    fn ir(name: &str, result: Result<&str, &str>) -> ProgramIr {
        ProgramIr { name: name.into(), result: result.map(ResumoIr::de).map_err(str::to_string) }
    }

    #[test]
    fn ir_report_format() {
        let r = vec![
            ir("01_a", Ok("define void @dart_main()")),
            ir("02_b", Err("[emit-ir] the thread aborted: index out of range\ndetail")),
            ir("10_c", Ok("")),
        ];
        let t = format_ir_report(&r);
        let lines: Vec<&str> = t.lines().collect();
        let a = ResumoIr::de("define void @dart_main()");
        assert_eq!(lines[0], format!("{}        24 01_a", a.hex()));
        assert_eq!(lines[1], "ERROR 02_b: [emit-ir] the thread aborted: index out of range");
        assert_eq!(lines[2], "6c62272e07bb014262b821756295c58d         0 10_c");
        assert_eq!(lines[4], "3 programas: 2 com IR, 1 com erro de emissão");
    }

    fn cs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn constructs_described_in_order_with_count() {
        assert_eq!(describe_constructs(&cs(&["closure", "cascata", "closure"])), "closure ×2, cascata");
    }

    #[test]
    fn constructs_section_counts_all_and_those_blocked_by_one() {
        let progs = vec![
            ("a", cs(&["closure", "cascata", "closure"])),
            ("b", cs(&["closure"])),
            ("c", cs(&["closure", "closure"])),
            ("d", cs(&["await"])),
        ];
        let t = constructs_section(&progs);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(
            lines[1],
            "Constructs (all diagnostics): 4 programs with diagnostics, 3 distinct constructs, 3 blocked by a single construct"
        );
        assert_eq!(lines[3], "          3            5        2  closure");
        assert_eq!(lines[4], "             only it: b, c");
        assert_eq!(lines[5], "          1            1        1  await");
        assert_eq!(lines[6], "             only it: d");
        assert_eq!(lines[7], "          1            1        0  cascata");
        assert!(constructs_section(&[]).is_empty());
    }

    #[test]
    fn native_report_lists_the_failure_constructs() {
        let p = Program::test("a");
        // The stderr of the native emitter (other crate): its text is what
        // `construtos_do_erro` parses, so it stays in Portuguese.
        let error = "[compile-native] erro de compilação: não suportado no backend nativo: closure\n\
                    erro de compilação: não suportado no backend nativo: closure\n  \
                    não suportado no backend nativo: closure (a.dart:1:1)\n  \
                    não suportado no backend nativo: cascata (a.dart:2:1)\n";
        let r = vec![ProgramResult {
            program: p,
            dart: s("1\n", 0),
            ddc: s("", 0),
            forge: Some(Output { stdout: String::new(), stderr: error.into(), code: 1 }),
            native: true,
            production: None,
            jit: None,
        }];
        let t = format_report(&r);
        assert!(t.contains("       constructs: closure, cascata"), "{t}");
        assert!(t.contains("          1            1        0  cascata"), "{t}");
    }

    #[test]
    fn ir_differences_lists_all() {
        let a = vec![ir("a", Ok("x")), ir("b", Ok("y")), ir("c", Err("e\n1")), ir("d", Ok("z"))];
        assert!(ir_differences(&a, &a.clone()).is_empty());
        let b = vec![ir("a", Ok("x")), ir("b", Ok("Y")), ir("c", Err("e\n2")), ir("d", Ok("Z"))];
        let d = ir_differences(&a, &b);
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d[0].starts_with("b: ") && d[1].starts_with("c: ") && d[2].starts_with("d: "), "{d:?}");
        assert_eq!(ir_differences(&a, &b[..3]).len(), 3);
    }
}
