//! `dartforge-parity`: diagnostic parity scoreboard against `dart analyze`.
//!
//! ```text
//! dartforge-parity placar [--grupo G]... [--trabalhadores N] [--lote N] [--detalhes]
//! dartforge-parity determinismo [--trabalhadores 1,4,8] [--grupo G]...
//! dartforge-parity oraculo [--grupo G]...          (re-records; needs dart)
//! dartforge-parity sintaxe-nova [--grupo G]...     (3.13.4 on files with syntax 3.6.2 does not know)
//! dartforge-parity gerar-corpus [--referencias DIR] [--pub-cache DIR]
//! dartforge-parity projetos [--oraculo] [--mutacoes N] [--regravar] [--trabalhadores N] [--lote N]
//! dartforge-parity saida [DIR] [--gravar]          (whole `analyze` output, three formats)
//! ```
//!
//! Exit code: 0 complete report; 1 oracle missing/out of date or
//! nondeterministic results; 2 invalid usage.

use dartforge_parity::analise::Motor;
use dartforge_parity::corpus::{self, Grupo};
use dartforge_parity::oraculo::{self, Meta, Record};
use dartforge_parity::projetos::{self, Mutacao, Projeto};
use dartforge_parity::scoreboard::Scoreboard;
use dartforge_parity::{Execution, filtros, run_ours};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

// The counting allocator gives the child process of each batch its memory
// watchdog (`run_child_batch`): an analysis that blows up does not take the
// machine with it.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

/// Live-memory cap of an isolated batch (`DARTFORGE_PARIDADE_TETO_MB`, 2048).
fn memory_cap() -> usize {
    std::env::var("DARTFORGE_PARIDADE_TETO_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(2048usize) << 20
}

/// Each batch in a child process of this executable, with at most 90 s (a
/// normal batch takes a few seconds).
fn execution(workers: usize, batch_size: usize) -> Execution {
    Execution {
        workers,
        batch_size,
        progress: true,
        isolate: std::env::current_exe().ok(),
        max_time: std::time::Duration::from_secs(90),
    }
}

/// `_lote <root> <packages|->`, files on standard input (internal use).
fn child_batch(args: &[String]) -> ExitCode {
    let (Some(root), Some(pk)) = (args.first(), args.get(1)) else { return ExitCode::from(2) };
    let mut input = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut input);
    let files: Vec<PathBuf> = input.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect();
    let packages = (pk != "-").then(|| PathBuf::from(pk));
    let c = dartforge_parity::run_child_batch(
        Path::new(root),
        packages.as_deref(),
        &files,
        dartforge_instrument::live_bytes,
        memory_cap(),
    );
    ExitCode::from(c as u8)
}

struct Args {
    cmd: String,
    groups: Vec<String>,
    workers: Vec<usize>,
    batch: usize,
    details: bool,
    references: PathBuf,
    pub_cache: PathBuf,
    oracle: bool,
    mutations: usize,
    rerecord: bool,
    corpus: PathBuf,
}

fn usage() -> ExitCode {
    eprintln!("{}", include_str!("main.rs").lines().skip(2).take(9).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
    ExitCode::from(2)
}

fn read_args() -> Option<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next()?;
    let pub_cache = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| Path::new(&l).join("Pub").join("Cache")))
        .unwrap_or_default()
        .join("hosted")
        .join("pub.dev");
    let mut a = Args {
        cmd,
        groups: Vec::new(),
        workers: vec![],
        batch: 0,
        details: false,
        references: PathBuf::from("E:/references"),
        pub_cache,
        oracle: false,
        mutations: 0,
        rerecord: false,
        corpus: corpus::raiz_corpus(),
    };
    while let Some(x) = it.next() {
        match x.as_str() {
            "--grupo" => a.groups.push(it.next()?),
            "--trabalhadores" => {
                a.workers = it.next()?.split(',').map(|n| n.trim().parse().ok()).collect::<Option<Vec<_>>>()?
            }
            "--lote" => a.batch = it.next()?.parse().ok()?,
            "--detalhes" => a.details = true,
            "--referencias" => a.references = PathBuf::from(it.next()?),
            "--pub-cache" => a.pub_cache = PathBuf::from(it.next()?),
            "--oraculo" => a.oracle = true,
            "--mutacoes" => a.mutations = it.next()?.parse().ok()?,
            "--regravar" => a.rerecord = true,
            "--corpus" => a.corpus = PathBuf::from(it.next()?),
            _ => return None,
        }
    }
    a.corpus = std::path::absolute(&a.corpus).unwrap_or(a.corpus);
    Some(a)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().is_some_and(|c| c == "_lote") {
        dartforge_parity::silence_panics();
        return child_batch(&raw[1..]);
    }
    // `saida [DIR] [--gravar]`: the project parity harness (III.1).
    if raw.first().is_some_and(|c| c == "saida") {
        let write = raw.iter().any(|x| x == "--gravar");
        let dir = raw.iter().skip(1).find(|x| !x.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| corpus::raiz_corpus().join("saida-analyze"));
        let ok = if write { dartforge_parity::output::record(&dir) } else { dartforge_parity::output::compare(&dir) };
        return if ok { ExitCode::SUCCESS } else { ExitCode::from(1) };
    }
    let Some(a) = read_args() else { return usage() };
    dartforge_parity::silence_panics();
    match a.cmd.as_str() {
        "placar" => {
            let (text, ok) = scoreboard(&a, a.workers.first().copied().unwrap_or(4));
            print!("{text}");
            if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        "determinismo" => determinism(&a),
        "oraculo" => rerecord_oracle(&a),
        "sintaxe-nova" => classify_new_syntax(&a),
        "gerar-corpus" => match corpus::gerar(&a.references, &a.corpus, &a.pub_cache) {
            Ok(r) => {
                print!("{r}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        },
        "projetos" => cmd_projects(&a),
        "encurtar" => {
            for (name, _) in chosen_groups(&a) {
                match corpus::encurtar(&a.corpus.join(&name)) {
                    Ok(n) => println!("{name}: {n} files renamed"),
                    Err(e) => eprintln!("{name}: {e}"),
                }
            }
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

fn chosen_groups(a: &Args) -> Vec<(String, Grupo)> {
    corpus::grupos(&a.corpus).into_iter().filter(|(n, _)| a.groups.is_empty() || a.groups.contains(n)).collect()
}

fn engine() -> Motor {
    Motor::descobrir().unwrap_or_else(|e| {
        eprintln!("error: SDK: {e}");
        std::process::exit(2)
    })
}

/// The whole (deterministic) report, and whether the oracle was up to date.
fn scoreboard(a: &Args, workers: usize) -> (String, bool) {
    let engine = engine();
    let mut total = Scoreboard::default();
    let mut body = String::new();
    let mut ok = true;
    let mut files = 0;
    let mut batches = 0;
    let mut panics = Vec::new();
    let mut ambiguous = 0;
    let groups = chosen_groups(a);
    let new_syntax = oraculo::read_new_syntax(&a.corpus);
    for (name, g) in &groups {
        let dir = a.corpus.join(name);
        let (oracle, meta) = match oraculo::read(&dir) {
            Ok(x) => x,
            Err(e) => {
                let _ = writeln!(body, "group {name}: NO RECORDED ORACLE ({e}) — run `dartforge-parity oraculo --grupo {name}`");
                ok = false;
                continue;
            }
        };
        if meta.hash != corpus::hash_fontes(&dir) {
            let _ = writeln!(body, "group {name}: ORACLE OUT OF DATE (sources changed) — re-record");
            ok = false;
        }
        let cfg = match corpus::preparar(&dir, name, g) {
            Ok(c) => c,
            Err(e) => {
                let _ = writeln!(body, "group {name}: {e}");
                ok = false;
                continue;
            }
        };
        let sources = corpus::arquivos_dart(&dir);
        let t = Instant::now();
        eprintln!("group {name}: {} files", sources.len());
        let batch = if a.batch > 0 { a.batch } else { 12 };
        let r = run_ours(&engine, &dir, &sources, Some(&cfg), &filtros::Opcoes::ler(&dir), &execution(workers, batch));
        eprintln!("group {name}: {:.1} s", t.elapsed().as_secs_f64());
        files += r.files;
        batches += r.batches;
        ambiguous += r.ambiguous;
        panics.extend(r.panics.iter().map(|p| format!("{name}/{p}")));
        let mut p = Scoreboard::default();
        p.compare(&oracle, &r.records);
        let tg = p.total();
        let _ = writeln!(
            body,
            "group {name} (oracle {}{}, {} files): oracle {} | ours {} | hits {} (wrong message {}) | wrong position {} | FP {} | FN {}",
            meta.sdk.version(),
            match new_syntax.get(name.as_str()).map_or(0, Vec::len) {
                0 => String::new(),
                n => format!("; {n} with new syntax through 3.13.4"),
            },
            r.files,
            tg.oracle,
            tg.ours,
            tg.hits,
            tg.wrong_message,
            tg.wrong_position,
            tg.false_positives,
            tg.false_negatives
        );
        // T2, done criterion: our parser's reference marker against the list
        // the 3.13.4 parser produced for the group.
        if meta.sdk == oraculo::OracleSdk::V362 {
            let ours = oraculo::marker_3_13(&dir, &sources);
            let from_oracle: std::collections::BTreeSet<String> =
                new_syntax.get(name.as_str()).map(|v| v.iter().cloned().collect()).unwrap_or_default();
            let only_ours: Vec<&String> = ours.difference(&from_oracle).collect();
            let only_oracle: Vec<&String> = from_oracle.difference(&ours).collect();
            let _ = writeln!(
                body,
                "  3.13.4 reference in {name}: ours {} | oracle {} | only ours {} | only oracle {}",
                ours.len(),
                from_oracle.len(),
                only_ours.len(),
                only_oracle.len()
            );
            for f in only_ours.iter().take(10) {
                let _ = writeln!(body, "    only ours: {f}");
            }
            for f in only_oracle.iter().take(10) {
                let _ = writeln!(body, "    only oracle: {f}");
            }
        }
        total.add(&p);
    }
    let t = total.total();
    let pct = if t.oracle > 0 { 100.0 * t.hits as f64 / t.oracle as f64 } else { 0.0 };
    let mut s = String::new();
    // The `placar paridade: ` prefix is matched by CI (`pesado.yml`).
    let _ = writeln!(
        s,
        "placar paridade: {}/{} oracle diagnostics at the exact position ({pct:.1}%), {} with the same message; FP {}, FN {}, wrong position {}",
        t.hits,
        t.oracle,
        t.hits - t.wrong_message,
        t.false_positives,
        t.false_negatives,
        t.wrong_position
    );
    let _ = writeln!(s, "DartForge parity — {} groups, {files} files, {batches} batches", groups.len());
    s.push_str(&body);
    s.push('\n');
    s.push_str(&total.table("whole corpus"));
    // Publication rule: a verified code that emits anything wrong in the
    // corpus (false positive, position or message) is an error. A false
    // negative is not.
    let verified = dartforge_parity::verificados();
    let mut broken = Vec::new();
    for c in &verified {
        if let Some(k) = total.by_code.get(*c) {
            if k.false_positives > 0 || k.wrong_position > 0 || k.wrong_message > 0 {
                broken.push(*c);
            }
        }
    }
    let _ = writeln!(s, "\npublished: syntax + {} verified codes {:?}", verified.len(), verified);
    if !broken.is_empty() {
        let _ = writeln!(s, "VERIFIED CODES WITH ERRORS EMITTED IN THE CORPUS: {broken:?}");
        ok = false;
    }
    // Syntactic codes are always published: they are not candidates.
    let syntactic = |k: &str| {
        dartforge_diagnostics::Codigo::por_nome(k)
            .is_some_and(|c| c.info().tipo == dartforge_diagnostics::TipoErro::SyntacticError)
    };
    let candidates: Vec<&String> = total
        .by_code
        .iter()
        .filter(|(k, c)| c.no_emitted_error() && !verified.contains(&k.as_str()) && !syntactic(k))
        .map(|(k, _)| k)
        .collect();
    let _ = writeln!(s, "no errors emitted in the corpus and not in the list (candidates, still need 0 FP in the projects): {candidates:?}");
    let _ = writeln!(s, "panics: {}{}", panics.len(), if panics.is_empty() { String::new() } else { format!(" {:?}", &panics[..panics.len().min(20)]) });
    let _ = writeln!(s, "ambiguous attribution (diagnostic without a unit, more than one candidate): {ambiguous}");
    if a.details {
        s.push_str("\nsamples:\n");
        s.push_str(&total.samples_text());
    }
    (s, ok)
}

fn determinism(a: &Args) -> ExitCode {
    let ns = if a.workers.is_empty() { vec![1, 4, 8] } else { a.workers.clone() };
    let mut texts = Vec::new();
    for &n in &ns {
        let t = Instant::now();
        eprintln!("determinism: {n} worker(s)…");
        texts.push(scoreboard(a, n).0);
        eprintln!("  {:.1} s", t.elapsed().as_secs_f64());
    }
    let same = texts.windows(2).all(|w| w[0] == w[1]);
    // The `determinismo: ` prefix is matched by CI (`pesado.yml`).
    if same {
        println!("determinismo: identical with {ns:?} workers");
        print!("{}", texts[0]);
        ExitCode::SUCCESS
    } else {
        println!("determinismo: DIVERGENT between {ns:?} workers");
        for (i, t) in texts.iter().enumerate() {
            println!("--- {} workers\n{t}", ns[i]);
        }
        ExitCode::from(1)
    }
}

fn oracle_cache() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/paridade-cache-dart");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn rerecord_oracle(a: &Args) -> ExitCode {
    // Clean cache: with the cache of an earlier run, 3.6.2 omitted the
    // `unexpected_separator_in_number` of `number/separators_error_no_experiment_test.dart`
    // (7 records fewer than the cold run). Recording always starts from scratch.
    let _ = std::fs::remove_dir_all(oracle_cache());
    let mut failed = false;
    for (name, g) in chosen_groups(a) {
        let dir = a.corpus.join(&name);
        if let Err(e) = corpus::preparar(&dir, &name, &g) {
            eprintln!("{name}: {e}");
            failed = true;
            continue;
        }
        let t = Instant::now();
        eprintln!("oracle {} on {name}…", g.sdk.version());
        // Large groups: one `dart analyze` per slice of subdirectories (memory).
        let slices = slices(&dir, 2500);
        let mut regs = Vec::new();
        let mut excluded = Vec::new();
        for target in &slices {
            bisect(g.sdk, &dir, target.clone(), &mut regs, &mut excluded);
        }
        let newly_excluded = excluded.clone();
        // Files removed in earlier recordings stay out of the group (the file
        // was already deleted); the list records why it is missing.
        if let Ok((_, previous)) = oraculo::read(&dir) {
            excluded.extend(previous.excluidos.into_iter().filter(|e| !dir.join(e).exists()));
        }
        excluded.sort();
        excluded.dedup();
        for e in &newly_excluded {
            eprintln!("  crashes the oracle, removed from the group: {e}");
            let _ = std::fs::remove_file(dir.join(e));
        }
        // Diagnostics in removed files do not count.
        regs.retain(|r| !excluded.contains(&r.arquivo));
        // Files with syntax 3.6.2 does not know: the 3.13.4 record.
        let list = oraculo::read_new_syntax(&a.corpus).remove(&name).unwrap_or_default();
        if g.sdk == oraculo::OracleSdk::V362 && !list.is_empty() {
            if oraculo::OracleSdk::V3134.available() {
                eprintln!("oracle 3.13.4 on {name} ({} file(s) with new syntax)…", list.len());
                let (new, _) = oracle_313_for_group(&dir);
                replace_files(&mut regs, new, &list);
            } else {
                // Without 3.13.4 on this machine: the files with new syntax
                // keep the 3.13.4 record made earlier, instead of getting the
                // 3.6.2 cascade or no record at all.
                eprintln!(
                    "SDK 3.13.4 missing: {} file(s) with new syntax in {name} keep the previous record",
                    list.len()
                );
                let previous = oraculo::read(&dir).map(|(r, _)| r).unwrap_or_default();
                replace_files(&mut regs, previous, &list);
            }
        }
        regs.sort();
        regs.dedup();
        let meta = Meta {
            sdk: g.sdk,
            hash: corpus::hash_fontes(&dir),
            arquivos: corpus::arquivos_dart(&dir).len(),
            diagnosticos: regs.len(),
            excluidos: excluded,
        };
        if let Err(e) = oraculo::write(&dir, regs, &meta) {
            eprintln!("{name}: {e}");
            failed = true;
            continue;
        }
        println!(
            "{name}: {} files, {} diagnostics from oracle {} ({} slice(s), {:.0} s)",
            meta.arquivos,
            meta.diagnosticos,
            g.sdk.version(),
            slices.len(),
            t.elapsed().as_secs_f64()
        );
    }
    if failed { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

/// 3.13.4 over the whole group, with the group's package as it is (the
/// language stays the group's): the same slices as `oraculo`. Also returns
/// the files that crash it (relative to `dir`).
fn oracle_313_for_group(dir: &Path) -> (Vec<Record>, Vec<String>) {
    let mut regs = Vec::new();
    let mut excluded = Vec::new();
    for target in slices(dir, 2500) {
        bisect(oraculo::OracleSdk::V3134, dir, target, &mut regs, &mut excluded);
    }
    (regs, excluded)
}

/// Replaces, in `regs`, the records of the files in `list` with those of `new`.
fn replace_files(regs: &mut Vec<Record>, new: Vec<Record>, list: &[String]) {
    let list: std::collections::BTreeSet<&str> = list.iter().map(String::as_str).collect();
    regs.retain(|r| !list.contains(r.arquivo.as_str()));
    regs.extend(new.into_iter().filter(|r| list.contains(r.arquivo.as_str())));
}

/// `sintaxe-nova`: runs 3.13.4 over each 3.6.2 group, picks the files where
/// it flags a feature 3.6.2 does not know ([`oraculo::uses_new_syntax`]),
/// writes the list to `sintaxe-nova.json` and replaces only those files'
/// records in the group's `oraculo.jsonl`. The hash and the other metadata of
/// the group stay as they are.
fn classify_new_syntax(a: &Args) -> ExitCode {
    let mut list = oraculo::read_new_syntax(&a.corpus);
    let mut failed = false;
    for (name, g) in chosen_groups(a) {
        if g.sdk != oraculo::OracleSdk::V362 {
            continue;
        }
        let dir = a.corpus.join(&name);
        if let Err(e) = corpus::preparar(&dir, &name, &g) {
            eprintln!("{name}: {e}");
            failed = true;
            continue;
        }
        let t = Instant::now();
        eprintln!("oracle 3.13.4 on {name} (language {})…", g.sdk.language());
        let (new, mut crashed) = oracle_313_for_group(&dir);
        // A server crash in a multi-file run does not always fail the run:
        // the file just gets no records. Those that our parser marks with the
        // 3.13.4 reference and that 3.13.4 did not pick run one by one; one
        // that crashes the oracle on its own counts as crashed.
        let chosen_313: std::collections::BTreeSet<&str> = new
            .iter()
            .filter(|r| r.code == "experiment_not_enabled")
            .map(|r| r.arquivo.as_str())
            .collect();
        let all = corpus::arquivos_dart(&dir);
        for e in oraculo::marker_3_13(&dir, &all) {
            if !chosen_313.contains(e.as_str())
                && !crashed.contains(&e)
                && oraculo::run(oraculo::OracleSdk::V3134, &dir, &[dir.join(&e)], &oracle_cache()).is_err()
            {
                crashed.push(e);
            }
        }
        // A file where 3.13.4 crashes and that our parser marks with the
        // 3.13.4 reference has no reference behaviour: it leaves the group,
        // like the one that crashes the 3.6.2 oracle (`oraculo`). The others
        // keep the 3.6.2 record.
        let sources: Vec<PathBuf> = crashed.iter().map(|e| dir.join(e)).collect();
        let marked = oraculo::marker_3_13(&dir, &sources);
        for e in &crashed {
            if marked.contains(e) {
                eprintln!("  3.13.4 crashes on {e}, which has new syntax; removed from the group");
                let _ = std::fs::remove_file(dir.join(e));
            } else {
                eprintln!("  3.13.4 crashes on {e}; the file keeps the 3.6.2 record");
            }
        }
        let mut by_file: std::collections::BTreeMap<&str, Vec<&Record>> = std::collections::BTreeMap::new();
        for r in &new {
            by_file.entry(r.arquivo.as_str()).or_default().push(r);
        }
        let chosen: Vec<String> =
            by_file.iter().filter(|(_, rs)| oraculo::uses_new_syntax(rs)).map(|(f, _)| f.to_string()).collect();
        let (mut regs, mut meta) = match oraculo::read(&dir) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("{name}: {e}");
                failed = true;
                continue;
            }
        };
        replace_files(&mut regs, new.clone(), &chosen);
        regs.retain(|r| !marked.contains(&r.arquivo));
        meta.excluidos.extend(marked.iter().cloned());
        meta.excluidos.sort();
        meta.excluidos.dedup();
        if !marked.is_empty() {
            meta.hash = corpus::hash_fontes(&dir);
            meta.arquivos = corpus::arquivos_dart(&dir).len();
        }
        regs.sort();
        regs.dedup();
        meta.diagnosticos = regs.len();
        if let Err(e) = oraculo::write(&dir, regs, &meta) {
            eprintln!("{name}: {e}");
            failed = true;
            continue;
        }
        println!("{name}: {} file(s) with new syntax, oracle 3.13.4 ({:.0} s)", chosen.len(), t.elapsed().as_secs_f64());
        if chosen.is_empty() {
            list.remove(&name);
        } else {
            list.insert(name, chosen);
        }
    }
    if let Err(e) = oraculo::write_new_syntax(&a.corpus, &list) {
        eprintln!("sintaxe-nova.json: {e}");
        failed = true;
    }
    if failed { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

/// Runs the oracle over `targets`; if the analysis server crashes, splits in
/// half (a lone directory becomes its children) until the files that crash
/// it are isolated, which go to `excluded` (relative to `dir`).
fn bisect(
    sdk: oraculo::OracleSdk,
    dir: &Path,
    targets: Vec<PathBuf>,
    regs: &mut Vec<Record>,
    excluded: &mut Vec<String>,
) {
    match oraculo::run(sdk, dir, &targets, &oracle_cache()) {
        Ok(ds) => regs.extend(ds.iter().filter_map(|d| Record::from_json(d, dir))),
        Err(e) => {
            if targets.len() > 1 {
                let mid = targets.len() / 2;
                bisect(sdk, dir, targets[..mid].to_vec(), regs, excluded);
                bisect(sdk, dir, targets[mid..].to_vec(), regs, excluded);
            } else if targets[0].is_dir() {
                let mut children: Vec<PathBuf> = std::fs::read_dir(&targets[0])
                    .map(|r| {
                        r.flatten()
                            .map(|e| e.path())
                            .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == "dart"))
                            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                            .collect()
                    })
                    .unwrap_or_default();
                children.sort();
                if children.is_empty() {
                    eprintln!("  {}: {e}", targets[0].display());
                } else {
                    eprintln!("  the oracle crashed on {}; splitting", targets[0].display());
                    bisect(sdk, dir, children, regs, excluded);
                }
            } else {
                excluded.push(oraculo::relativo(&targets[0], dir).unwrap_or_default());
            }
        }
    }
}

/// Targets of each `dart analyze` run: the whole group, or the
/// subdirectories grouped up to `max` files per run (analysis server
/// memory). `dart analyze` accepts several targets at once.
fn slices(dir: &Path, max: usize) -> Vec<Vec<PathBuf>> {
    if corpus::arquivos_dart(dir).len() <= max {
        return vec![vec![dir.to_path_buf()]];
    }
    let mut subs: Vec<(PathBuf, usize)> = Vec::new();
    let mut loose = false;
    if let Ok(ents) = std::fs::read_dir(dir) {
        for e in ents.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() && !n.starts_with('.') {
                subs.push((p.clone(), corpus::arquivos_dart(&p).len()));
            } else if n.ends_with(".dart") {
                loose = true;
            }
        }
    }
    if loose {
        // Loose files at the root: the target would have to be the whole root.
        return vec![vec![dir.to_path_buf()]];
    }
    subs.sort();
    let mut out: Vec<Vec<PathBuf>> = Vec::new();
    let mut n_current = 0;
    for (p, n) in subs.into_iter().filter(|(_, n)| *n > 0) {
        if out.is_empty() || n_current + n > max {
            out.push(Vec::new());
            n_current = 0;
        }
        out.last_mut().expect("slice").push(p);
        n_current += n;
    }
    out
}
fn read_projects(corpus: &Path) -> Vec<(Projeto, Vec<Record>)> {
    let path = corpus.join("projetos.json");
    match std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => v,
        None => projetos::padrao().into_iter().map(|p| (p, Vec::new())).collect(),
    }
}

fn cmd_projects(a: &Args) -> ExitCode {
    let engine = engine();
    let mut registry = read_projects(&a.corpus);
    let workers = a.workers.first().copied().unwrap_or(1);
    let batch = if a.batch > 0 { a.batch } else { 400 };
    let mut total = Scoreboard::default();
    let mut s = String::new();
    for (p, oracle_regs) in registry.iter_mut() {
        if !p.caminho.join("pubspec.yaml").is_file() {
            let _ = writeln!(s, "{}: missing at {} (skipped)", p.nome, p.caminho.display());
            continue;
        }
        let options = filtros::Opcoes::ler(&p.caminho);
        if a.oracle {
            let t = Instant::now();
            eprintln!("{}: oracle {}…", p.nome, p.sdk.version());
            match oraculo::run(p.sdk, &p.caminho, &[p.caminho.clone()], &oracle_cache()) {
                Ok(ds) => {
                    *oracle_regs = ds.iter().filter_map(|d| Record::from_json(d, &p.caminho)).collect();
                    oracle_regs.sort();
                    p.oraculo = Some(oracle_regs.len());
                    eprintln!("  {} diagnostics, {:.0} s", oracle_regs.len(), t.elapsed().as_secs_f64());
                }
                Err(e) => {
                    let _ = writeln!(s, "{}: oracle failed: {e}", p.nome);
                    continue;
                }
            }
        }
        let files = projetos::arquivos(&p.caminho, &options);
        let cfg = p.caminho.join(".dart_tool/package_config.json");
        let t = Instant::now();
        eprintln!("{}: our side, {} files…", p.nome, files.len());
        // Nested packages (`example/` with its own pubspec) are separate
        // analysis contexts in the analyzer: each with its own package_config.
        let mut r = dartforge_parity::Run::default();
        for (pkg_root, fs) in projetos::por_pacote(&p.caminho, &files) {
            let c = pkg_root.join(".dart_tool/package_config.json");
            let c = if c.is_file() { c } else { cfg.clone() };
            let partial = run_ours(&engine, &p.caminho, &fs, Some(&c), &options, &execution(workers, batch));
            r.records.extend(partial.records);
            r.files += partial.files;
            r.batches += partial.batches;
            r.panics.extend(partial.panics);
            r.ambiguous += partial.ambiguous;
        }
        r.records.sort();
        let mut pl = Scoreboard::default();
        pl.compare(oracle_regs, &r.records);
        let tg = pl.total();
        let published = r
            .records
            .iter()
            .filter(|x| x.tipo == "SYNTACTIC_ERROR" || dartforge_parity::verificados().contains(&x.code.as_str()))
            .count();
        let _ = writeln!(
            s,
            "{} (oracle {}: {} diagnostics; {} files, {:.0} s): ours {} | FP {} | hits {} | published by the rule {} | panics {}",
            p.nome,
            p.sdk.version(),
            p.oraculo.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            r.files,
            t.elapsed().as_secs_f64(),
            tg.ours,
            tg.false_positives,
            tg.hits,
            published,
            r.panics.len()
        );
        total.add(&pl);
    }
    s.push('\n');
    s.push_str(&total.table("real projects (every diagnostic of ours without a pair in the oracle is an FP)"));
    if a.details {
        s.push_str("
samples:
");
        s.push_str(&total.samples_text());
    }
    if a.oracle {
        let text = serde_json::to_string_pretty(&registry).expect("JSON") + "\n";
        if let Err(e) = std::fs::write(a.corpus.join("projetos.json"), text) {
            eprintln!("projetos.json: {e}");
        }
    }
    if a.mutations > 0 || a.rerecord {
        s.push('\n');
        s.push_str(&mutations(a, &engine, &registry.iter().map(|x| x.0.clone()).collect::<Vec<_>>()));
    }
    print!("{s}");
    ExitCode::SUCCESS
}

fn mutations(a: &Args, engine: &Motor, list: &[Projeto]) -> String {
    let file = a.corpus.join("mutacoes.jsonl");
    let mut recorded: Vec<Mutacao> = std::fs::read_to_string(&file)
        .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
        .unwrap_or_default();
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch-paridade/projetos");
    let mut s = String::new();
    let mut scoreboard = Scoreboard::default();
    let mut stale = 0;
    let mut new = Vec::new();
    for p in list {
        if !p.caminho.join("pubspec.yaml").is_file() {
            continue;
        }
        let copy = scratch.join(&p.nome);
        let cfg = match projetos::copiar(p, &copy) {
            Ok(c) => c,
            Err(e) => {
                let _ = writeln!(s, "{}: copy failed: {e}", p.nome);
                continue;
            }
        };
        let options = filtros::Opcoes::ler(&copy);
        let files = projetos::arquivos(&copy, &options);
        let mut ms: Vec<Mutacao> = if a.rerecord {
            projetos::escolher(p, &copy, &files, a.mutations.max(1))
        } else {
            recorded.iter().filter(|m| m.projeto == p.nome).cloned().collect()
        };
        for m in ms.iter_mut() {
            let target = copy.join(&m.arquivo);
            let Ok(original) = std::fs::read_to_string(&target) else { continue };
            if projetos::hash_texto(&original) != m.hash_original {
                stale += 1;
                continue;
            }
            let mutant = m.aplicar(&original);
            if std::fs::write(&target, &mutant).is_err() {
                continue;
            }
            if a.rerecord {
                eprintln!("mutation {} {} {}…", p.nome, m.tipo, m.arquivo);
                match oraculo::run(p.sdk, &copy, std::slice::from_ref(&target), &oracle_cache()) {
                    Ok(ds) => {
                        m.oraculo = ds
                            .iter()
                            .filter_map(|d| Record::from_json(d, &copy))
                            .filter(|r| r.arquivo == m.arquivo)
                            .collect();
                        m.oraculo.sort();
                    }
                    Err(e) => eprintln!("  oracle failed: {e}"),
                }
            }
            let r = engine.analisar(&copy, std::slice::from_ref(&target), Some(&cfg));
            let ours: Vec<Record> = dartforge_parity::records(&r, &copy, &options, false)
                .into_iter()
                .filter(|x| x.arquivo == m.arquivo)
                .collect();
            scoreboard.compare(&m.oraculo, &ours);
            let _ = std::fs::write(&target, &original);
        }
        new.extend(ms);
    }
    if a.rerecord {
        let mut t = String::new();
        for m in &new {
            t.push_str(&serde_json::to_string(m).expect("JSON"));
            t.push('\n');
        }
        let _ = std::fs::write(&file, t);
        recorded = new;
    }
    let _ = writeln!(s, "mutations: {} recorded, {stale} stale", recorded.len());
    s.push_str(&scoreboard.table("mutations of the real projects"));
    s.push_str(&scoreboard.samples_text());
    s
}
