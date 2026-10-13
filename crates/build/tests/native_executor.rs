//! The **native** builder executor (`crates/build/src/executor_nativo.rs`,
//! B01) against the `build_runner` oracle (`corpus/builders/`): the bootstrap
//! of `packages/build_executor` compiled by DartForge's own `compile-native`,
//! with no VM, no `dart` and no `build_runner` support on disk. The yardstick
//! is that of `vm_executor.rs`: output byte for byte equal to the oracle's,
//! and incremental equal to from-scratch on every edit of the case.
//!
//! Prerequisites, hence `#[ignore]`: a `dartforge` with the `nativo` feature
//! (`DARTFORGE_COMPILADOR`, or `target/release/dartforge`), the SDK
//! (`DARTFORGE_SDK_LIB`/`DART_SDK`, like `compile-native`) and the
//! `package_config.json` of each case (`dart pub get --enforce-lockfile`).
//! Each case compiles its own executable (minutes: the whole `analyzer`),
//! which is deleted at the end of the case — they fit on disk one at a time.
//! `DARTFORGE_CORPUS_BUILDERS` changes the corpus root; `DARTFORGE_CASOS`
//! (comma-separated names) limits the cases.
use dartforge_build::consulta::SemBanco;
use dartforge_build::executor_nativo::{NativeConfig, NativeExecutor};
use dartforge_build::grafo::AssetId;
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn corpus_root() -> PathBuf {
    std::env::var_os("DARTFORGE_CORPUS_BUILDERS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders"))
}

fn compiler() -> PathBuf {
    std::env::var_os("DARTFORGE_COMPILADOR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/release/dartforge")
                .with_extension(std::env::consts::EXE_EXTENSION)
        })
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
        r.insert(id, std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display())));
    }
    r
}

/// Copy of the case without the oracle, the edits, `build/` and `.dart_tool`
/// (only `package_config.json`).
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
                std::fs::copy(p.join("package_config.json"), target.join("package_config.json")).unwrap();
                continue;
            }
            copy_case(&p, &target);
        } else {
            std::fs::copy(&p, &target).unwrap();
        }
    }
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

fn native_engine(dir: &Path) -> Motor {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Motor::novo(dir, &cfg, OpcoesMotor::default()).expect("engine");
    let sdk = std::env::var_os("DARTFORGE_SDK_LIB").map(PathBuf::from);
    m.definir_executor_dart(Box::new(NativeExecutor::new(NativeConfig::for_project(
        compiler(),
        sdk,
        dir,
    ))));
    m
}

fn update(m: &mut Motor, changed: &[PathBuf]) -> (RelMotor, f64) {
    let t0 = Instant::now();
    let at = m
        .atualizar(&Contexto { banco: &SemBanco, programa: None }, changed, Demanda::Tudo)
        .expect("update");
    for a in &at.avisos {
        println!("warning: {a}");
    }
    (at.rel, t0.elapsed().as_secs_f64() * 1000.0)
}

/// The scoreboard of all cases through the native executor: zero different
/// and incremental = from scratch on every edit.
#[test]
#[ignore = "requires dartforge with the nativo feature and `dart pub get` in each case of corpus/builders"]
fn corpus_builders_through_the_native_executor() {
    assert!(compiler().is_file(), "compiler {} does not exist", compiler().display());
    let filter: Option<Vec<String>> = std::env::var("DARTFORGE_CASOS")
        .ok()
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let mut cases: Vec<PathBuf> = std::fs::read_dir(corpus_root())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("oraculo/plano.dart").is_file() && p.join(".dart_tool/package_config.json").is_file())
        .filter(|p| {
            filter.as_ref().is_none_or(|f| f.iter().any(|n| p.file_name().is_some_and(|x| x == n.as_str())))
        })
        .collect();
    cases.sort();
    let mut lines = vec![
        "### corpus/builders through the native executor — equal/pending/different".to_string(),
        String::new(),
        "| case | equal | pending | different | clean (ms) | no change (ms) | reasons |".into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    let mut different = Vec::new();
    for origin in cases {
        let name = origin.file_name().unwrap().to_string_lossy().to_string();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(&name);
        copy_case(&origin, &dir);
        let mut m = native_engine(&dir);
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
        println!("{}", lines.last().unwrap());
        // The table only carries the reason's 1st line; the whole text (the
        // compiler's output, when compiling the executor fails) goes to the
        // log.
        for (k, n) in p.motivos() {
            if k.contains('\n') {
                println!("{name}: {n}× full reason:\n{k}");
            }
        }
        for (id, reason) in &p.diferentes {
            different.push(format!("{name}: {} ({reason})", id.texto()));
        }
        if !p.pendentes.is_empty() {
            different.push(format!("{name}: {} pending ({})", p.pendentes.len(), reasons.join("; ")));
        }
        let mut steps: Vec<PathBuf> = std::fs::read_dir(origin.join("edicoes"))
            .map(|l| l.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        steps.sort();
        for step in steps {
            let changed = apply(&step, &dir);
            let (rel, ms) = update(&mut m, &changed);
            // New engine, new process: the executable comes from the cache.
            let mut fresh = native_engine(&dir);
            update(&mut fresh, &[]);
            let step_name = step.file_name().unwrap().to_string_lossy().to_string();
            println!("{name}/{step_name}: {ms:.0} ms, {} actions run", rel.acoes_executadas);
            if m.estado_canonico() != fresh.estado_canonico() {
                different.push(format!("{name}: incremental ≠ from scratch after {step_name}"));
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
