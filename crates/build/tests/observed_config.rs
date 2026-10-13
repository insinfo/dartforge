//! DF-BUILD-018: the engine watches the configuration that changes the plan
//! also outside the root's `build.yaml` — the `build.yaml` of each dependency
//! (whether it exists or not) and the root directory (a
//! `<package>.build.yaml` override that appears changes its marker) — and,
//! when it changes, rebuilds the engine by rereading `package_config.json`
//! (DF-BUILD-010).
//!
//! Uses `corpus/builders/i18n` (requires `dart pub get` in the case); without
//! the `package_config.json`, the test says so and returns.
use dartforge_build::{Engine, EngineOptions};
use dartforge_elements::config::PackageConfig;
use std::path::Path;

#[test]
fn watches_dependency_build_yaml_and_overrides() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders/i18n");
    let file = root.join(".dart_tool/package_config.json");
    if !file.is_file() {
        eprintln!("no {}: run `dart pub get` in the case", file.display());
        return;
    }
    let cfg = PackageConfig::load(&file).unwrap();
    let m = Engine::new(&root, &cfg, EngineOptions::default()).expect("engine");
    let watched = m.watched();
    let has = |f: &dyn Fn(&Path) -> bool| watched.iter().any(|p| f(p));
    // The `build.yaml` of the `i18n` package (a dependency, in the pub cache).
    assert!(
        has(&|p| p.ends_with("build.yaml") && p.to_string_lossy().contains("i18n-4.2.1")),
        "i18n's build.yaml is not watched: {watched:?}"
    );
    // That of a dependency without `build.yaml` too (creating it changes the
    // plan).
    let missing: Vec<_> = watched
        .iter()
        .filter(|p| p.ends_with("build.yaml") && !p.is_file())
        .collect();
    assert!(!missing.is_empty(), "no missing build.yaml is watched");
    // The root directory (`<package>.build.yaml` overrides).
    let canonical_root = dartforge_elements::generated::key(&root);
    assert!(has(&|p| *p == canonical_root), "root is not watched");
}
