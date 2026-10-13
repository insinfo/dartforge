//! The builder executor through the Dart VM (`crates/build/src/vm.rs`) against
//! the `build_runner` oracle (`corpus/builders/`): the output must be byte for
//! byte that of `dart run build_runner build`, and incrementality must come
//! from the reads the `BuildStep` made through the host.
//!
//! Prerequisites, hence `#[ignore]`: the Dart 3.6.2 VM (`DARTFORGE_BUILD_DART`,
//! `DARTFORGE_DART_SDK/bin/dart` or `dart` on the `PATH`) and `dart pub get
//! --enforce-lockfile` in each case (the `package_config.json`), as `ci.yml`
//! does for the other corpus tests.
use dartforge_build::query::NoDb;
use dartforge_build::graph::AssetId;
use dartforge_build::engine::Origin;
use dartforge_build::vm::{VmConfig, VmExecutor};
use dartforge_build::{UpdateContext, Demand, Engine, EngineOptions, EngineReport};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders")
}

fn dart() -> PathBuf {
    dartforge_build::vm::dart_from_env()
        .or_else(|| {
            std::env::var_os("DARTFORGE_DART_SDK")
                .map(|s| PathBuf::from(s).join("bin").join("dart"))
        })
        .unwrap_or_else(|| PathBuf::from("dart"))
}

/// The manifest's outputs with the oracle's bytes.
fn references(dir: &Path) -> BTreeMap<AssetId, Vec<u8>> {
    let text = std::fs::read_to_string(dir.join("oraculo/manifesto.json")).expect("manifest");
    let m: serde_json::Value = serde_json::from_str(&text).expect("manifest JSON");
    let mut r = BTreeMap::new();
    for s in m["saidas"].as_array().into_iter().flatten() {
        let id = AssetId::from_text(s["asset"].as_str().expect("asset")).expect("package|path");
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

/// Copy of the case without the oracle, the edits, `build/` and `.dart_tool`
/// (only `package_config.json`): with no `build_runner` support on disk,
/// every output must come from the executor.
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

fn vm_engine(dir: &Path) -> Engine {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Engine::new(dir, &cfg, EngineOptions::default()).expect("engine");
    m.set_dart_executor(Box::new(VmExecutor::new(VmConfig::for_project(
        dart(),
        dir,
    ))));
    m
}

fn update(m: &mut Engine, changed: &[PathBuf]) -> (EngineReport, f64) {
    let t0 = Instant::now();
    let at = m
        .update(
            &UpdateContext {
                db: &NoDb,
                program: None,
            },
            changed,
            Demand::Everything,
        )
        .expect("update");
    for a in &at.warnings {
        println!("warning: {a}");
    }
    (at.rel, t0.elapsed().as_secs_f64() * 1000.0)
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

/// Applies a corpus edit and returns the changed files.
fn apply(step: &Path, copy: &Path) -> Vec<PathBuf> {
    let mut rel = Vec::new();
    files(step, step, &mut rel);
    rel.into_iter()
        .map(|r| {
            let target = copy.join(&r);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(step.join(&r), &target).unwrap();
            target
        })
        .collect()
}

#[test]
#[ignore = "requires the Dart VM and `dart pub get` in corpus/builders/json_serializable"]
fn json_serializable_through_the_vm_equals_build_runner_and_is_incremental() {
    let origin = corpus_root().join("json_serializable");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("json_serializable");
    copy_case(&origin, &dir);

    // Clean build: the manifest's two outputs, byte for byte.
    let mut m = vm_engine(&dir);
    let (rel, ms) = update(&mut m, &[]);
    println!("clean: {ms:.0} ms — {}", rel.text());
    // json_serializable and combining_builder on lib/modelos.dart and
    // bin/main.dart (the latter without an annotation: no output, as in the
    // official tool), and `source_gen:part_cleanup` on the written part (the
    // anchor of the part that was not written is omitted, as in the official
    // tool).
    assert_eq!(
        (rel.dart, rel.executed_actions, rel.changed_outputs),
        (5, 5, 2),
        "{}",
        rel.text()
    );
    let p = m.scoreboard(&references(&origin));
    assert!(
        p.different.is_empty() && p.pending.is_empty(),
        "scoreboard {}: {:?} {:?}",
        p.summary(),
        p.different,
        p.pending
    );
    assert_eq!(p.iguais.len(), 2);
    // `source_gen:part_cleanup` deletes the hidden part: it disappears from
    // the published generation (the official `FinalizedReader`), but the
    // `.g.dart` stays.
    assert!(!published(&m, "lib/modelos.json_serializable.g.part"));
    assert!(published(&m, "lib/modelos.g.dart"));

    // Nothing changed: nothing runs.
    let (rel, ms) = update(&mut m, &[]);
    println!("no change: {ms:.1} ms — {}", rel.text());
    assert_eq!(rel.executed_actions, 0);

    // The corpus edits, cumulative; each state equal to that of a new engine.
    let expected: [(&str, usize, usize); 3] = [
        // Method body: the actions that read `modelos.dart` rerun (its
        // json_serializable and that of `bin/main.dart`, which imports it,
        // and combining_builder, which resolves the library); no output
        // changes.
        ("01-corpo-de-metodo", 3, 0),
        // New field: the same three plus the part cleanup, which changed; the
        // part and the `.g.dart` change.
        ("02-campo-novo", 4, 2),
        // New file: only its two actions and the cleanup of the new part (the
        // part listing of the others did not change).
        ("03-arquivo-novo", 3, 2),
    ];
    for (step, run, changed_outputs) in expected {
        let changed = apply(&origin.join("edicoes").join(step), &dir);
        let (rel, ms) = update(&mut m, &changed);
        println!("{step}: {ms:.0} ms — {}", rel.text());
        assert_eq!(
            (rel.executed_actions, rel.dart, rel.changed_outputs),
            (run, run, changed_outputs),
            "{step}: {}",
            rel.text()
        );
        // New engine, new process, kernel reused from disk.
        let mut fresh = vm_engine(&dir);
        let (_, ms) = update(&mut fresh, &[]);
        println!("{step}: from scratch with the reused kernel: {ms:.0} ms");
        assert_eq!(
            m.canonical_state(),
            fresh.canonical_state(),
            "{step}: incremental ≠ from scratch"
        );
    }
    let product = m
        .graph
        .generated
        .keys()
        .find(|id| id.path.as_ref() == "lib/produto.g.dart")
        .and_then(|id| m.record(m.graph.generated[id].action))
        .expect("action of produto.g.dart");
    assert_eq!(product.origin, Origin::Dart);
    let text = product
        .outputs
        .iter()
        .find_map(|(_, c)| c.clone())
        .expect("produto.g.dart written");
    let text = std::str::from_utf8(&text).unwrap();
    assert!(
        text.contains("'codigo_interno'"),
        "fieldRename.snake applied:\n{text}"
    );

    // Loader demand (`dev`/`serve`/`compile-js`): the hidden part is not a
    // `.dart`, but combining_builder reads it by glob — without it the
    // `.g.dart` would come out empty. With the Dart executor, only the
    // optional one stays lazy.
    let mut c = vm_engine(&dir);
    c.update(
        &UpdateContext {
            db: &NoDb,
            program: None,
        },
        &[],
        Demand::Loader,
    )
    .expect("update");
    let g = |m: &Engine| {
        let id = AssetId::new("corpus_json_serializable", "lib/modelos.g.dart");
        m.record(m.graph.generated[&id].action)
            .and_then(|r| r.outputs.iter().find(|(s, _)| *s == id)?.1.clone())
    };
    assert!(g(&c).is_some(), "modelos.g.dart in the loader demand");
    assert_eq!(g(&c), g(&m));
}

/// The scoreboard of all cases through the VM (with the table in the CI
/// summary): the same yardstick as `corpus.rs`, with the real Dart executor
/// and no `build_runner` support on disk — zero different, and incremental =
/// from scratch on every edit of the case.
#[test]
#[ignore = "requires the Dart VM and `dart pub get` in each case of corpus/builders"]
fn corpus_builders_through_the_vm() {
    let mut cases: Vec<PathBuf> = std::fs::read_dir(corpus_root())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.join("oraculo/plano.dart").is_file()
                && p.join(".dart_tool/package_config.json").is_file()
        })
        .collect();
    cases.sort();
    let mut lines = vec![
        "### corpus/builders through the VM — equal/pending/different".to_string(),
        String::new(),
        "| case | equal | pending | different | clean (ms) | no change (ms) | reasons |"
            .into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    let mut different = Vec::new();
    for origin in cases {
        let name = origin.file_name().unwrap().to_string_lossy().to_string();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(&name);
        copy_case(&origin, &dir);
        let mut m = vm_engine(&dir);
        let (_, clean) = update(&mut m, &[]);
        let (_, warm) = update(&mut m, &[]);
        let p = m.scoreboard(&references(&origin));
        let reasons: Vec<String> = p
            .motivos()
            .into_iter()
            .map(|(k, n)| format!("{n}× {}", k.lines().next().unwrap_or("")))
            .collect();
        lines.push(format!(
            "| {name} | {} | {} | {} | {clean:.0} | {warm:.1} | {} |",
            p.iguais.len(),
            p.pending.len(),
            p.different.len(),
            reasons.join("; ")
        ));
        for (id, reason) in &p.different {
            different.push(format!("{name}: {} ({reason})", id.text()));
        }
        // Incremental = from scratch on the case's edits, with the real
        // executor.
        let mut steps: Vec<PathBuf> = std::fs::read_dir(origin.join("edicoes"))
            .map(|l| l.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        steps.sort();
        for step in steps {
            let changed = apply(&step, &dir);
            let (rel, ms) = update(&mut m, &changed);
            let mut fresh = vm_engine(&dir);
            update(&mut fresh, &[]);
            let step_name = step.file_name().unwrap().to_string_lossy().to_string();
            println!(
                "{name}/{step_name}: {ms:.0} ms, {} actions run",
                rel.executed_actions
            );
            if m.canonical_state() != fresh.canonical_state() {
                different.push(format!(
                    "{name}: incremental ≠ from scratch after {step_name}"
                ));
            }
        }
    }
    println!("{}", lines.join("\n"));
    if let Ok(file) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(file) {
            let _ = writeln!(f, "{}\n", lines.join("\n"));
        }
    }
    assert!(different.is_empty(), "{}", different.join("\n"));
}

/// State saved across processes with the real executor (B04): the second
/// process runs no builder, reaches the state of a from-scratch engine, and
/// the builders' code comes from the bootstrap's depfile.
#[test]
#[ignore = "requires the Dart VM and `dart pub get` in corpus/builders/json_serializable"]
fn state_saved_through_the_vm() {
    let origin = corpus_root().join("json_serializable");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("json_serializable");
    copy_case(&origin, &dir);
    let process = |persist: bool| {
        let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
        let options = EngineOptions {
            persist,
            ..EngineOptions::default()
        };
        let mut m = Engine::new(&dir, &cfg, options).unwrap();
        m.set_dart_executor(Box::new(VmExecutor::new(VmConfig::for_project(
            dart(),
            &dir,
        ))));
        let t0 = Instant::now();
        let at = m
            .update(
                &UpdateContext {
                    db: &NoDb,
                    program: None,
                },
                &[],
                Demand::Everything,
            )
            .expect("update");
        // Like the CLI: the `source` outputs that changed go to disk.
        for (p, c) in m.native_source_outputs(&at.altered_outputs) {
            std::fs::write(&p, &c[..]).unwrap();
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        (m.canonical_state(), at.rel, ms)
    };
    let (first, rel, ms) = process(true);
    println!("first process: {ms:.0} ms — {}", rel.text());
    assert!(rel.dart > 0);
    let state = std::fs::read_to_string(dir.join(".dart_tool/dartforge/build/estado/estado.json"))
        .expect("state written");
    assert!(
        state.contains("builders.dart") || state.contains("\"codigo_dart\":["),
        "builders' code recorded"
    );
    let (second, rel, ms) = process(true);
    println!("second process: {ms:.0} ms — {}", rel.text());
    assert_eq!(rel.executed_actions, 0, "{}", rel.text());
    let (zero, _, _) = process(false);
    assert_eq!(first, zero);
    assert_eq!(second, zero, "restored ≠ from scratch");
    assert_eq!(restored_equal_count(&dir, &origin), 2);
}

/// Outputs equal to the oracle in a new process that restores the state.
fn restored_equal_count(dir: &Path, origin: &Path) -> usize {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let options = EngineOptions {
        persist: true,
        ..EngineOptions::default()
    };
    let mut m = Engine::new(dir, &cfg, options).unwrap();
    update(&mut m, &[]);
    let p = m.scoreboard(&references(origin));
    assert!(p.different.is_empty(), "{:?}", p.different);
    p.iguais.len()
}

/// The post-processor output `notas.rascunho.resumo` in the anchor's record.
fn summary(m: &Engine) -> Option<std::sync::Arc<[u8]>> {
    (0..m.graph.actions.len())
        .filter(|&a| m.graph.actions[a].pos)
        .filter_map(|a| m.record(a))
        .flat_map(|r| r.outputs.iter())
        .find(|(s, _)| s.path.as_ref() == "lib/entrada/notas.rascunho.resumo")
        .and_then(|(_, c)| c.clone())
}

/// Does the published generation have a file ending in `suffix`?
fn published(m: &Engine, suffix: &str) -> bool {
    m.generation()
        .iter()
        .any(|(k, _)| k.to_string_lossy().replace('\\', "/").ends_with(suffix))
}

/// Post-processors (`post_process_builders`) through the VM: the anchor runs
/// the real `PostProcessBuildStep`, the output is `build_runner`'s byte for
/// byte, only the anchor whose input changed reruns, the output disappears
/// with the input, and `deletePrimaryInput` (the case's release option) is
/// recorded and refused in strict mode — DartForge does not have the merged
/// directory in which it would take effect.
#[test]
#[ignore = "requires the Dart VM and `dart pub get` in corpus/builders/cadeia_configuracao"]
fn post_processor_through_the_vm() {
    let origin = corpus_root().join("cadeia_configuracao");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("cadeia_configuracao");
    copy_case(&origin, &dir);
    let expected = std::fs::read(
        origin.join("oraculo/cache/corpus_cadeia_configuracao/lib/entrada/notas.rascunho.resumo"),
    )
    .unwrap();
    let output = "lib/entrada/notas.rascunho.resumo";

    let mut m = vm_engine(&dir);
    update(&mut m, &[]);
    assert_eq!(summary(&m).as_deref(), Some(expected.as_slice()));
    let anchors: Vec<usize> = (0..m.graph.actions.len())
        .filter(|&a| m.graph.actions[a].pos)
        .collect();
    assert_eq!(anchors.len(), 1, "one anchor: the only .rascunho input");
    assert!(
        m.record(anchors[0])
            .is_some_and(|r| r.deleted_outputs.is_empty()),
        "apagar: false in dev"
    );
    assert!(published(&m, output), "output published in the generation");

    // The second line changed: only the anchor reruns, the output is the same.
    let notes = dir.join("lib/entrada/notas.rascunho");
    std::fs::write(
        &notes,
        "nota do post_process_builder limpeza\noutra segunda linha",
    )
    .unwrap();
    let (rel, _) = update(&mut m, std::slice::from_ref(&notes));
    assert_eq!(
        (rel.executed_actions, rel.changed_outputs),
        (1, 0),
        "{}",
        rel.text()
    );

    // The first line changed: the output changes; incremental = from scratch.
    std::fs::write(&notes, "nota nova\nsegunda").unwrap();
    let (rel, _) = update(&mut m, std::slice::from_ref(&notes));
    assert_eq!(
        (rel.executed_actions, rel.changed_outputs),
        (1, 1),
        "{}",
        rel.text()
    );
    let text = summary(&m).unwrap();
    assert!(
        std::str::from_utf8(&text)
            .unwrap()
            .ends_with("---\nnota nova\n")
    );
    let mut fresh = vm_engine(&dir);
    update(&mut fresh, &[]);
    assert_eq!(
        m.canonical_state(),
        fresh.canonical_state(),
        "incremental ≠ from scratch"
    );

    // Input deleted: the anchor and the output disappear.
    std::fs::remove_file(&notes).unwrap();
    let (rel, _) = update(&mut m, std::slice::from_ref(&notes));
    assert!(rel.changed_outputs >= 1, "{}", rel.text());
    assert!(summary(&m).is_none());
    assert!(!published(&m, output), "output withdrawn from the generation");
    let mut fresh = vm_engine(&dir);
    update(&mut fresh, &[]);
    assert_eq!(
        m.canonical_state(),
        fresh.canonical_state(),
        "incremental ≠ from scratch"
    );

    // Release: `apagar: true` calls `deletePrimaryInput`.
    std::fs::write(&notes, "nota do post_process_builder limpeza\n").unwrap();
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let release = EngineOptions {
        release: true,
        ..EngineOptions::default()
    };
    let context = UpdateContext {
        db: &NoDb,
        program: None,
    };
    let mut r = Engine::new(&dir, &cfg, release.clone()).unwrap();
    r.set_dart_executor(Box::new(VmExecutor::new(VmConfig::for_project(
        dart(),
        &dir,
    ))));
    let at = r
        .update(&context, &[], Demand::Everything)
        .expect("update in release");
    let anchor = (0..r.graph.actions.len())
        .find(|&a| r.graph.actions[a].pos)
        .unwrap();
    assert_eq!(
        r.record(anchor).unwrap().deleted_outputs,
        vec![AssetId::new(
            "corpus_cadeia_configuracao",
            "lib/entrada/notas.rascunho"
        )]
    );
    assert!(
        at.warnings.iter().any(|a| a.contains("deletePrimaryInput")),
        "{:?}",
        at.warnings
    );
    let mut strict = Engine::new(
        &dir,
        &cfg,
        EngineOptions {
            strict: true,
            ..release
        },
    )
    .unwrap();
    strict.set_dart_executor(Box::new(VmExecutor::new(VmConfig::for_project(
        dart(),
        &dir,
    ))));
    let error = strict
        .update(&context, &[], Demand::Everything)
        .err()
        .expect("strict mode refuses the deletion that has no effect");
    assert!(error.contains("deletePrimaryInput"), "{error}");
}
