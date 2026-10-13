//! Harness of the compatibility corpus (`corpus/builders/`, oracle of the
//! official `build_runner`). Prerequisite: `dart pub get --enforce-lockfile`
//! in each case (the `package_config.json`), hence `#[ignore]`: `ci.yml` runs
//! `pub get` and then `--ignored`.
//!
//! * **plan**: the engine's canonical form = that of `oraculo/plano.dart`;
//! * **outputs**: each manifest output falls into equal / pending (with a
//!   reason) / different; criterion: `different == 0`;
//! * **determinism**: 1, 4 and 8 workers give the same state;
//! * **incremental = from scratch**: with the `edicoes/` applied in sequence,
//!   the live engine and a new engine give the same state.
use dartforge_build::query::NoDb;
use dartforge_build::executor::{
    Availability, ExecutorError, DartExecutor, ActionRequest, ActionResult, BuilderScript,
    BuildStepService,
};
use dartforge_build::graph::AssetId;
use dartforge_build::engine::Origin;
use dartforge_build::{UpdateContext, Demand, Engine, EngineOptions};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders")
}

fn cases() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(corpus_root())
        .expect("corpus/builders")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("oraculo/manifesto.json").is_file())
        .collect();
    v.sort();
    v
}

fn cfg_of(dir: &Path) -> Option<PackageConfig> {
    PackageConfig::load(&dir.join(".dart_tool/package_config.json")).ok()
}

/// The manifest's outputs with the oracle's bytes.
fn references(dir: &Path) -> BTreeMap<AssetId, Vec<u8>> {
    let text = std::fs::read_to_string(dir.join("oraculo/manifesto.json")).expect("manifest");
    let m: serde_json::Value = serde_json::from_str(&text).expect("manifest JSON");
    let mut r = BTreeMap::new();
    for s in m["saidas"].as_array().into_iter().flatten() {
        let asset = s["asset"].as_str().expect("asset");
        let id = AssetId::from_text(asset).expect("package|path");
        let file = match s["build_to"].as_str() {
            Some("source") => dir.join("oraculo/source").join(id.path.as_ref()),
            _ => dir
                .join("oraculo/cache")
                .join(id.package.as_ref())
                .join(id.path.as_ref()),
        };
        r.insert(
            id,
            std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display())),
        );
    }
    r
}

fn engine(dir: &Path, workers: usize) -> Result<Engine, String> {
    let cfg = cfg_of(dir).ok_or("no package_config.json (run dart pub get)")?;
    let options = EngineOptions {
        workers: workers,
        measure_unchecked: true,
        ..Default::default()
    };
    let mut m = Engine::new(dir, &cfg, options)?;
    m.update(
        &UpdateContext {
            db: &NoDb,
            program: None,
        },
        &[],
        Demand::Everything,
    )?;
    Ok(m)
}

struct FakeDart {
    preparations: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    shutdowns: Arc<AtomicUsize>,
    /// The "builders' code" the executor declares (the real one's depfile).
    code: Option<Vec<PathBuf>>,
}

impl DartExecutor for FakeDart {
    fn availability(&self) -> Availability {
        Availability::Available
    }
    fn prepare(&mut self, _: &BuilderScript) -> Result<(), ExecutorError> {
        self.preparations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn execute(
        &mut self,
        p: &ActionRequest,
        s: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _ = s.read(&p.input);
        for id in &p.allowed_outputs {
            let bytes: Arc<[u8]> = format!("{}:{}\n", p.key, p.input.text())
                .into_bytes()
                .into();
            s.write(id, bytes)
                .map_err(|e| ExecutorError(format!("output refused: {}", e.0.text())))?;
        }
        Ok(ActionResult::default())
    }
    fn code(&self) -> Option<Vec<PathBuf>> {
        self.code.clone()
    }
    fn shutdown(&mut self) {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
#[ignore = "requires `dart pub get` in corpus/builders/cadeia_configuracao"]
fn injected_dart_executor_runs_and_reuses_actions() {
    let dir = corpus_root().join("cadeia_configuracao");
    let cfg = cfg_of(&dir).expect("package_config.json");
    let mut m = Engine::new(&dir, &cfg, EngineOptions::default()).unwrap();
    let preparations = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let shutdowns = Arc::new(AtomicUsize::new(0));
    m.set_dart_executor(Box::new(FakeDart {
        preparations: preparations.clone(),
        calls: calls.clone(),
        shutdowns: shutdowns.clone(),
        code: None,
    }));
    let ctx = UpdateContext {
        db: &NoDb,
        program: None,
    };
    m.update(&ctx, &[], Demand::Everything).unwrap();
    let done = calls.load(Ordering::SeqCst);
    assert!(
        done > 0,
        "the available Dart executor did not receive any action"
    );
    assert!(
        m.graph
            .actions
            .iter()
            .enumerate()
            .any(|(i, _)| m.record(i).is_some_and(|r| r.origin == Origin::Dart))
    );
    m.update(&ctx, &[], Demand::Everything).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        done,
        "clean action was rerun"
    );
    assert_eq!(
        preparations.load(Ordering::SeqCst),
        1,
        "builder script recompiled within the session"
    );
    drop(m);
    assert_eq!(
        shutdowns.load(Ordering::SeqCst),
        1,
        "executor was not shut down at the end of the session"
    );
}

/// One engine process (like `dartforge build`): new engine, new fake
/// executor, one update, `source` outputs on disk.
/// Returns the canonical state and how many actions the Dart executor received.
fn process(dir: &Path, persist: bool) -> (String, usize, Engine) {
    let cfg = cfg_of(dir).expect("package_config.json");
    let mut m = Engine::new(
        dir,
        &cfg,
        EngineOptions {
            persist,
            ..Default::default()
        },
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    m.set_dart_executor(Box::new(FakeDart {
        preparations: Arc::new(AtomicUsize::new(0)),
        calls: calls.clone(),
        shutdowns: Arc::new(AtomicUsize::new(0)),
        code: Some(vec![dir.join("tool/builders.dart")]),
    }));
    let at = m
        .update(
            &UpdateContext {
                db: &NoDb,
                program: None,
            },
            &[],
            Demand::Everything,
        )
        .unwrap();
    // "não gravado" matches the warning `engine.rs` emits when saving the state
    // fails (kept in Portuguese there).
    assert!(
        at.warnings.iter().all(|a| !a.contains("não gravado")),
        "{:?}",
        at.warnings
    );
    // Like the CLI: the `source` outputs that changed go to disk.
    for (p, c) in m.native_source_outputs(&at.altered_outputs) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, &c[..]).unwrap();
    }
    (m.canonical_state(), calls.load(Ordering::SeqCst), m)
}

/// State saved across processes (B04, `docs/BUILD-MOTOR.md` §4.1): a new
/// process reuses the valid actions and the state is that of an engine that
/// started from scratch; an edited input, a changed option and a changed
/// builder code (an input no query sees) invalidate what is needed.
#[test]
#[ignore = "requires `dart pub get` in corpus/builders/cadeia_configuracao"]
fn state_saved_across_processes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("cadeia_configuracao");
    copy_case(&corpus_root().join("cadeia_configuracao"), &dir);
    let state = dir.join(".dart_tool/dartforge/build/estado");

    // Without `persist`, nothing is written.
    let (from_scratch, all, _) = process(&dir, false);
    assert!(all > 0);
    assert!(!state.exists(), "engine without persistir wrote state");

    // The first process writes; the second runs nothing and reaches the same
    // state.
    let (first, n, _) = process(&dir, true);
    assert_eq!((first.as_str(), n), (from_scratch.as_str(), all));
    assert!(state.join("estado.json").is_file());
    let (second, n, _) = process(&dir, true);
    assert_eq!(n, 0, "new process reran valid actions");
    assert_eq!(second, from_scratch, "restored state ≠ from scratch");

    // Edited input: only what read it reruns.
    let alfa = dir.join("lib/entrada/alfa.txt");
    let mut text = std::fs::read_to_string(&alfa).unwrap();
    text.push_str("new line\n");
    std::fs::write(&alfa, &text).unwrap();
    let (after, n, _) = process(&dir, true);
    let (zero, n_zero, _) = process(&dir, false);
    assert_eq!(after, zero, "edited input: restored ≠ from scratch");
    assert!(n > 0 && n < n_zero, "reran {n} of {n_zero}");

    // Changed option (build.yaml's global_options): the phase changes
    // identity and its actions rerun.
    let yaml = dir.join("build.yaml");
    let original = std::fs::read_to_string(&yaml).unwrap();
    std::fs::write(&yaml, original.replace("      nivel: 3", "      nivel: 4")).unwrap();
    let (after, n, _) = process(&dir, true);
    let (zero, n_zero, _) = process(&dir, false);
    assert_eq!(after, zero, "changed option: restored ≠ from scratch");
    assert!(n > 0 && n < n_zero, "reran {n} of {n_zero}");

    // Changed builder code: no query sees it, but the Dart actions are no
    // longer valid.
    let tool = dir.join("tool/builders.dart");
    let mut code = std::fs::read_to_string(&tool).unwrap();
    code.push_str("\n// a change only the builder's code sees\n");
    std::fs::write(&tool, code).unwrap();
    let (after, n, _) = process(&dir, true);
    assert_eq!(after, zero);
    assert_eq!(
        n, n_zero,
        "builder code changed and Dart actions were reused"
    );

    // A `source` output deleted from disk between processes: the action
    // reruns.
    let (_, _, m) = process(&dir, true);
    let source = m
        .graph
        .generated
        .iter()
        .find(|(_, g)| !g.hidden)
        .map(|(id, _)| dir.join(id.path.as_ref()))
        .expect("source output in the case");
    drop(m);
    let _ = std::fs::remove_file(&source);
    let (_, n, _) = process(&dir, true);
    assert!(n >= 1, "source output deleted and nothing reran");

    // A corrupted state is ignored: everything runs.
    std::fs::write(state.join("estado.json"), b"{corrupted").unwrap();
    let (after, n, _) = process(&dir, true);
    assert_eq!((after, n), (zero, n_zero));
}

fn summary(lines: &[String]) {
    let text = lines.join("\n");
    println!("{text}");
    if let Ok(file) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(file) {
            let _ = writeln!(f, "{text}\n");
        }
    }
}

#[test]
#[ignore = "requires `dart pub get` in each case of corpus/builders"]
fn corpus_builders_plan_and_scoreboard() {
    let mut lines = vec![
        "### corpus/builders — plan and scoreboard (equal/pending/different)".to_string(),
        String::new(),
    ];
    lines.push("| case | plan | equal | pending | different | reasons |".into());
    lines.push("|---|---|---|---|---|---|".into());
    let mut failures = Vec::new();
    for dir in cases() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let Some(cfg) = cfg_of(&dir) else {
            failures.push(format!("{name}: no package_config.json (dart pub get)"));
            continue;
        };
        if !dartforge_build::detect(&cfg) {
            // Zero-cost control: without build_runner, no engine.
            assert!(
                !dir.join("oraculo/plano.dart").exists(),
                "{name}: no build_runner but a plano.dart"
            );
            lines.push(format!(
                "| {name} | no builders (engine not built) | 0 | 0 | 0 | |"
            ));
            continue;
        }
        let plan = match std::fs::read_to_string(dir.join("oraculo/plano.dart")) {
            Ok(text) => {
                let (_, _, p) = dartforge_build::project_plan(&dir, &cfg).expect("plan");
                match dartforge_build::oracle::compare(&p.applications, &text)
                    .expect("readable plano.dart")
                {
                    Ok(()) => format!("equal ({})", p.applications.len()),
                    Err(d) => {
                        failures.push(format!("{name}: different plan:\n{}", d.join("\n")));
                        "DIFFERENT".into()
                    }
                }
            }
            Err(_) => "-".into(),
        };
        let m = match engine(&dir, 4) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name}: engine: {e}"));
                continue;
            }
        };
        let p = m.scoreboard(&references(&dir));
        let reasons: Vec<String> = p
            .motivos()
            .into_iter()
            .map(|(k, n)| format!("{n}× {k}"))
            .collect();
        lines.push(format!(
            "| {name} | {plan} | {} | {} | {} | {} |",
            p.iguais.len(),
            p.pending.len(),
            p.different.len(),
            reasons.join("; ")
        ));
        for (id, m) in &p.different {
            failures.push(format!("{name}: different {}: {m}", id.text()));
        }
        for (id, m) in &p.pending {
            // "pós-processador" matches the reasons `engine.rs` gives for
            // post-processors (kept in Portuguese there).
            if m.starts_with("pós-processador") {
                println!("{name}: {} ({m})", id.text());
            }
        }
    }
    summary(&lines);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "requires `dart pub get` in each case of corpus/builders"]
fn corpus_builders_determinism() {
    for dir in cases() {
        let Some(cfg) = cfg_of(&dir) else { continue };
        if !dartforge_build::detect(&cfg) {
            continue;
        }
        let states: Vec<String> = [1, 4, 8]
            .iter()
            .map(|&n| engine(&dir, n).expect("engine").canonical_state())
            .collect();
        assert_eq!(
            states[0],
            states[1],
            "{}: 1 ≠ 4 workers",
            dir.display()
        );
        assert_eq!(
            states[0],
            states[2],
            "{}: 1 ≠ 8 workers",
            dir.display()
        );
    }
}

fn copy_case(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let name = e.file_name();
        if name == "oraculo" || name == "edicoes" || name == "build" {
            continue;
        }
        let target = to.join(&name);
        if p.is_dir() {
            if name == ".dart_tool" {
                std::fs::create_dir_all(&target).unwrap();
                std::fs::copy(
                    p.join("package_config.json"),
                    target.join("package_config.json"),
                )
                .unwrap();
                continue;
            }
            copy_case(&p, &target);
        } else {
            std::fs::copy(&p, &target).unwrap();
        }
    }
}

#[test]
#[ignore = "requires `dart pub get` in corpus/builders/sass_builder_compressed"]
fn sass_release_css_on_demand_without_support() {
    let origin = corpus_root().join("sass_builder_compressed");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("sass_builder_compressed");
    copy_case(&origin, &dir);
    // Without build_runner's support: the answer can only come from the native
    // generator.
    let scss = dir.join("web/principal.scss");
    std::fs::write(&scss, ".a { color: red; }\n").unwrap();
    let cfg = cfg_of(&dir).expect("package_config.json");
    let mut m = Engine::new(
        &dir,
        &cfg,
        EngineOptions {
            release: true,
            ..Default::default()
        },
    )
    .unwrap();
    let ctx = UpdateContext {
        db: &NoDb,
        program: None,
    };
    m.update(&ctx, &[], Demand::Loader).unwrap();
    let target = dartforge_elements::gerado::chave(&dir.join("web/principal.css"));
    assert!(
        m.natural_paths_map
            .keys()
            .any(|id| id.path.as_ref() == "web/principal.css"),
        "CSS outside the graph: {target:?}"
    );
    let css = m.materialize(&ctx, &target).unwrap_or_else(|| {
        let full_state = m.canonical_state();
        let state: Vec<_> = full_state
            .lines()
            .filter(|l| l.contains("principal.scss"))
            .collect();
        panic!("native CSS on demand: {state:?}")
    });
    assert_eq!(&*css, b".a{color:red}\n");
}

fn files(dir: &Path, base: &Path, v: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, base, v);
        } else {
            v.push(p.strip_prefix(base).unwrap().to_path_buf());
        }
    }
}

#[test]
#[ignore = "requires `dart pub get` in each case of corpus/builders"]
fn corpus_builders_incremental_equals_from_scratch() {
    let mut done = 0;
    for dir in cases() {
        let edits = dir.join("edicoes");
        if !edits.is_dir() || cfg_of(&dir).is_none() {
            continue;
        }
        let tmp = tempfile::tempdir().unwrap();
        let copy = tmp.path().join(dir.file_name().unwrap());
        copy_case(&dir, &copy);
        let mut live = engine(&copy, 4).expect("live engine");
        let mut steps: Vec<PathBuf> = std::fs::read_dir(&edits)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        steps.sort();
        for step in steps {
            let mut rel = Vec::new();
            files(&step, &step, &mut rel);
            let mut changed = Vec::new();
            for r in rel {
                let target = copy.join(&r);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::copy(step.join(&r), &target).unwrap();
                changed.push(target);
            }
            live.update(
                &UpdateContext {
                    db: &NoDb,
                    program: None,
                },
                &changed,
                Demand::Everything,
            )
            .expect("update");
            let fresh = engine(&copy, 4).expect("new engine");
            assert_eq!(
                live.canonical_state(),
                fresh.canonical_state(),
                "{}: incremental ≠ from scratch after {}",
                dir.display(),
                step.display()
            );
            done += 1;
        }
    }
    println!("{done} edits verified");
}
