//! The builder executor through the Dart VM (`crates/build/src/vm.rs`) against
//! the `build_runner` oracle (`corpus/builders/`): the output must be byte for
//! byte that of `dart run build_runner build`, and incrementality must come
//! from the reads the `BuildStep` made through the host.
//!
//! Prerequisites, hence `#[ignore]`: the Dart 3.6.2 VM (`DARTFORGE_BUILD_DART`,
//! `DARTFORGE_DART_SDK/bin/dart` or `dart` on the `PATH`) and `dart pub get
//! --enforce-lockfile` in each case (the `package_config.json`), as `ci.yml`
//! does for the other corpus tests.
use dartforge_build::consulta::SemBanco;
use dartforge_build::grafo::AssetId;
use dartforge_build::motor::Origem;
use dartforge_build::vm::{ConfigDaVm, ExecutorVm};
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders")
}

fn dart() -> PathBuf {
    dartforge_build::vm::dart_do_ambiente()
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
        let id = AssetId::de_texto(s["asset"].as_str().expect("asset")).expect("package|path");
        let file = match s["build_to"].as_str() {
            Some("source") => dir.join("oraculo/source").join(id.caminho.as_ref()),
            _ => dir
                .join("oraculo/cache")
                .join(id.pacote.as_ref())
                .join(id.caminho.as_ref()),
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

fn vm_engine(dir: &Path) -> Motor {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Motor::novo(dir, &cfg, OpcoesMotor::default()).expect("engine");
    m.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        dir,
    ))));
    m
}

fn update(m: &mut Motor, changed: &[PathBuf]) -> (RelMotor, f64) {
    let t0 = Instant::now();
    let at = m
        .atualizar(
            &Contexto {
                banco: &SemBanco,
                programa: None,
            },
            changed,
            Demanda::Tudo,
        )
        .expect("update");
    for a in &at.avisos {
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
    println!("clean: {ms:.0} ms — {}", rel.texto());
    // json_serializable and combining_builder on lib/modelos.dart and
    // bin/main.dart (the latter without an annotation: no output, as in the
    // official tool), and `source_gen:part_cleanup` on the written part (the
    // anchor of the part that was not written is omitted, as in the official
    // tool).
    assert_eq!(
        (rel.dart, rel.acoes_executadas, rel.saidas_alteradas),
        (5, 5, 2),
        "{}",
        rel.texto()
    );
    let p = m.placar(&references(&origin));
    assert!(
        p.diferentes.is_empty() && p.pendentes.is_empty(),
        "scoreboard {}: {:?} {:?}",
        p.resumo(),
        p.diferentes,
        p.pendentes
    );
    assert_eq!(p.iguais.len(), 2);
    // `source_gen:part_cleanup` deletes the hidden part: it disappears from
    // the published generation (the official `FinalizedReader`), but the
    // `.g.dart` stays.
    assert!(!published(&m, "lib/modelos.json_serializable.g.part"));
    assert!(published(&m, "lib/modelos.g.dart"));

    // Nothing changed: nothing runs.
    let (rel, ms) = update(&mut m, &[]);
    println!("no change: {ms:.1} ms — {}", rel.texto());
    assert_eq!(rel.acoes_executadas, 0);

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
        println!("{step}: {ms:.0} ms — {}", rel.texto());
        assert_eq!(
            (rel.acoes_executadas, rel.dart, rel.saidas_alteradas),
            (run, run, changed_outputs),
            "{step}: {}",
            rel.texto()
        );
        // New engine, new process, kernel reused from disk.
        let mut fresh = vm_engine(&dir);
        let (_, ms) = update(&mut fresh, &[]);
        println!("{step}: from scratch with the reused kernel: {ms:.0} ms");
        assert_eq!(
            m.estado_canonico(),
            fresh.estado_canonico(),
            "{step}: incremental ≠ from scratch"
        );
    }
    let product = m
        .grafo
        .gerados
        .keys()
        .find(|id| id.caminho.as_ref() == "lib/produto.g.dart")
        .and_then(|id| m.registro(m.grafo.gerados[id].acao))
        .expect("action of produto.g.dart");
    assert_eq!(product.origem, Origem::Dart);
    let text = product
        .saidas
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
    c.atualizar(
        &Contexto {
            banco: &SemBanco,
            programa: None,
        },
        &[],
        Demanda::Carregador,
    )
    .expect("update");
    let g = |m: &Motor| {
        let id = AssetId::novo("corpus_json_serializable", "lib/modelos.g.dart");
        m.registro(m.grafo.gerados[&id].acao)
            .and_then(|r| r.saidas.iter().find(|(s, _)| *s == id)?.1.clone())
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
        let p = m.placar(&references(&origin));
        let reasons: Vec<String> = p
            .motivos()
            .into_iter()
            .map(|(k, n)| format!("{n}× {}", k.lines().next().unwrap_or("")))
            .collect();
        lines.push(format!(
            "| {name} | {} | {} | {} | {clean:.0} | {warm:.1} | {} |",
            p.iguais.len(),
            p.pendentes.len(),
            p.diferentes.len(),
            reasons.join("; ")
        ));
        for (id, reason) in &p.diferentes {
            different.push(format!("{name}: {} ({reason})", id.texto()));
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
                rel.acoes_executadas
            );
            if m.estado_canonico() != fresh.estado_canonico() {
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
    let process = |persistir: bool| {
        let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
        let options = OpcoesMotor {
            persistir,
            ..OpcoesMotor::default()
        };
        let mut m = Motor::novo(&dir, &cfg, options).unwrap();
        m.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
            dart(),
            &dir,
        ))));
        let t0 = Instant::now();
        let at = m
            .atualizar(
                &Contexto {
                    banco: &SemBanco,
                    programa: None,
                },
                &[],
                Demanda::Tudo,
            )
            .expect("update");
        // Like the CLI: the `source` outputs that changed go to disk.
        for (p, c) in m.saidas_source_nativas(&at.alterados) {
            std::fs::write(&p, &c[..]).unwrap();
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        (m.estado_canonico(), at.rel, ms)
    };
    let (first, rel, ms) = process(true);
    println!("first process: {ms:.0} ms — {}", rel.texto());
    assert!(rel.dart > 0);
    let state = std::fs::read_to_string(dir.join(".dart_tool/dartforge/build/estado/estado.json"))
        .expect("state written");
    assert!(
        state.contains("builders.dart") || state.contains("\"codigo_dart\":["),
        "builders' code recorded"
    );
    let (second, rel, ms) = process(true);
    println!("second process: {ms:.0} ms — {}", rel.texto());
    assert_eq!(rel.acoes_executadas, 0, "{}", rel.texto());
    let (zero, _, _) = process(false);
    assert_eq!(first, zero);
    assert_eq!(second, zero, "restored ≠ from scratch");
    assert_eq!(restored_equal_count(&dir, &origin), 2);
}

/// Outputs equal to the oracle in a new process that restores the state.
fn restored_equal_count(dir: &Path, origin: &Path) -> usize {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let options = OpcoesMotor {
        persistir: true,
        ..OpcoesMotor::default()
    };
    let mut m = Motor::novo(dir, &cfg, options).unwrap();
    update(&mut m, &[]);
    let p = m.placar(&references(origin));
    assert!(p.diferentes.is_empty(), "{:?}", p.diferentes);
    p.iguais.len()
}

/// The post-processor output `notas.rascunho.resumo` in the anchor's record.
fn summary(m: &Motor) -> Option<std::sync::Arc<[u8]>> {
    (0..m.grafo.acoes.len())
        .filter(|&a| m.grafo.acoes[a].pos)
        .filter_map(|a| m.registro(a))
        .flat_map(|r| r.saidas.iter())
        .find(|(s, _)| s.caminho.as_ref() == "lib/entrada/notas.rascunho.resumo")
        .and_then(|(_, c)| c.clone())
}

/// Does the published generation have a file ending in `suffix`?
fn published(m: &Motor, suffix: &str) -> bool {
    m.geracao()
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
    let anchors: Vec<usize> = (0..m.grafo.acoes.len())
        .filter(|&a| m.grafo.acoes[a].pos)
        .collect();
    assert_eq!(anchors.len(), 1, "one anchor: the only .rascunho input");
    assert!(
        m.registro(anchors[0])
            .is_some_and(|r| r.apagados.is_empty()),
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
        (rel.acoes_executadas, rel.saidas_alteradas),
        (1, 0),
        "{}",
        rel.texto()
    );

    // The first line changed: the output changes; incremental = from scratch.
    std::fs::write(&notes, "nota nova\nsegunda").unwrap();
    let (rel, _) = update(&mut m, std::slice::from_ref(&notes));
    assert_eq!(
        (rel.acoes_executadas, rel.saidas_alteradas),
        (1, 1),
        "{}",
        rel.texto()
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
        m.estado_canonico(),
        fresh.estado_canonico(),
        "incremental ≠ from scratch"
    );

    // Input deleted: the anchor and the output disappear.
    std::fs::remove_file(&notes).unwrap();
    let (rel, _) = update(&mut m, std::slice::from_ref(&notes));
    assert!(rel.saidas_alteradas >= 1, "{}", rel.texto());
    assert!(summary(&m).is_none());
    assert!(!published(&m, output), "output withdrawn from the generation");
    let mut fresh = vm_engine(&dir);
    update(&mut fresh, &[]);
    assert_eq!(
        m.estado_canonico(),
        fresh.estado_canonico(),
        "incremental ≠ from scratch"
    );

    // Release: `apagar: true` calls `deletePrimaryInput`.
    std::fs::write(&notes, "nota do post_process_builder limpeza\n").unwrap();
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let release = OpcoesMotor {
        release: true,
        ..OpcoesMotor::default()
    };
    let context = Contexto {
        banco: &SemBanco,
        programa: None,
    };
    let mut r = Motor::novo(&dir, &cfg, release.clone()).unwrap();
    r.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        &dir,
    ))));
    let at = r
        .atualizar(&context, &[], Demanda::Tudo)
        .expect("update in release");
    let anchor = (0..r.grafo.acoes.len())
        .find(|&a| r.grafo.acoes[a].pos)
        .unwrap();
    assert_eq!(
        r.registro(anchor).unwrap().apagados,
        vec![AssetId::novo(
            "corpus_cadeia_configuracao",
            "lib/entrada/notas.rascunho"
        )]
    );
    assert!(
        at.avisos.iter().any(|a| a.contains("deletePrimaryInput")),
        "{:?}",
        at.avisos
    );
    let mut strict = Motor::novo(
        &dir,
        &cfg,
        OpcoesMotor {
            estrito: true,
            ..release
        },
    )
    .unwrap();
    strict.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        &dir,
    ))));
    let error = strict
        .atualizar(&context, &[], Demanda::Tudo)
        .err()
        .expect("strict mode refuses the deletion that has no effect");
    assert!(error.contains("deletePrimaryInput"), "{error}");
}
