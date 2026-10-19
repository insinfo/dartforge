//! The ngdart corpus: one shape per file, with the `.template.dart` of the
//! official compiler next to it, in `corpus/ngdart/oraculo/`.
//!
//! The contract is the same as the JS backend corpus: what we generate **must**
//! come out byte for byte equal to the official output. Known refusals must
//! be listed explicitly; every other official artifact must be generated.
//!
//! The oracle is regenerated with `scripts/corpus-ngdart.ps1` (runs the
//! official `build_runner` once).
use dartforge_elements::config::PackageConfig;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_ng_generator::resolution::Resolver;
use dartforge_ng_generator::{Package, Scoreboard, generate_in, template_path};
use std::path::{Path, PathBuf};

/// The names under `dependencies:` in the corpus `pubspec.yaml` (the
/// top-level block, one package per line indented by two spaces).
fn pubspec_dependencies(root: &Path) -> Vec<String> {
    let text =
        std::fs::read_to_string(root.join("pubspec.yaml")).expect("pubspec.yaml of the corpus");
    let mut inside = false;
    let mut output = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            inside = line.trim_end() == "dependencies:";
            continue;
        }
        if inside
            && line.starts_with("  ")
            && !line.starts_with("   ")
            && let Some((name, _)) = line.trim().split_once(':')
        {
            output.push(name.trim().to_string());
        }
    }
    assert!(
        !output.is_empty(),
        "pubspec.yaml of the corpus without dependencies:"
    );
    output
}

fn corpus_root() -> PathBuf {
    // The crate directory is `crates/ng_generator`.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

/// A registered name is insufficient if its old machine's directory is gone.
fn dependency_directory(config: &PackageConfig, name: &str) -> Result<PathBuf, String> {
    let package = config
        .packages
        .get(name)
        .ok_or_else(|| format!("missing package `{name}`"))?;
    let directory = package
        .package_uri
        .to_file_path()
        .map_err(|_| format!("package `{name}` has no local directory"))?;
    if !directory.is_dir() {
        return Err(format!(
            "package `{name}` directory is missing: {}",
            directory.display()
        ));
    }
    Ok(directory)
}

/// Corpus cases that the generator must refuse, with the sub-shape of the
/// refusal. (g02, `<select>` with `<option>`, left this list in round 3: the
/// `NgSelectOption` is instantiated with the `@Host` of the `<select>`
/// accessor. b02, h02 and i19 — `providers:` on the component — and d09 and
/// h01 — `@ContentChild(ren)` on the component — left when the host view
/// started writing the providers and the empty result of the queries. b06
/// — `encapsulation:` —, i24 — several components in the file —, i30 —
/// `<template #t>` and `*ngTemplateOutlet` —, i72 — child with `providers:`
/// — and i73 — `@ContentChild(.., read:)` of a child — left in the following
/// round; the current refusals are probes that keep the official output.
/// i84 — `<template>` with a directive — and i85 — `@ViewChild` of a
/// `<template>` — left in the round of items NG04–NG10; i77 — a child that
/// injects a provider of its own node — with probe j61; i78 — a provider
/// with a dependency from outside the node — with j71; i76 — a provider of
/// the child requested by a content node — with the `eager` of
/// `_getDependency`.) These are the shapes still without a translation
/// (docs/GERADOR-NG.md §9): generating any of them while ignoring what is
/// missing would give wrong output.
const REFUSED: &[(&str, &str)] = &[];

#[test]
fn what_we_generate_equals_the_official_output() {
    let root = corpus_root();
    let oracle = root.join("oraculo");
    assert!(
        oracle.is_dir(),
        "no oracle in {}; run scripts/corpus-ngdart.ps1",
        oracle.display()
    );
    let mut required: std::collections::BTreeSet<_> = std::fs::read_dir(&oracle)
        .expect("oracle directory")
        .map(|entry| entry.expect("oracle entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dart")
        })
        .map(|path| path.file_name().unwrap().to_owned())
        .collect();
    assert!(!required.is_empty(), "empty oracle directory");
    for (file, _) in REFUSED {
        required.remove(template_path(Path::new(file)).file_name().unwrap());
    }
    let package = Package {
        name: "corpus_ngdart".into(),
        root: root.clone(),
        ..Default::default()
    };
    // The test runs the same path as production: it loads the project and
    // generates with the semantic database. Without it, cases like
    // `{{ item.nome }}` — which need the type of a member in another class —
    // would be left out of the check.
    let entry = root.join("lib").join("corpus_ngdart.dart");
    let cfg = root.join(".dart_tool").join("package_config.json");
    // Without the `package_config.json` (not in git), or with a stale one, the
    // load is lenient and goes on: `ngdart`/`ngforms` do not resolve, almost
    // everything is refused and the test would silently check much less than
    // it claims. That is why any dependency of `pubspec.yaml` missing is an
    // error here.
    let config = PackageConfig::load(&cfg).unwrap_or_else(|e| {
        panic!(
            "{e}: run `dart pub get` in corpus/ngdart (CI runs it in the step \
             \"pub get do corpus do ngdart\")"
        )
    });
    for dep in pubspec_dependencies(&root) {
        dependency_directory(&config, &dep)
            .unwrap_or_else(|error| panic!("{error}: run `dart pub get` in corpus/ngdart"));
    }
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc")
        .unwrap_or_else(|error| panic!("cannot load SDK {}: {error}", sdk_dir.display()));
    let mut names = Interner::new();
    let (program, _) = load_lenient(&entry, &sdk, Some(&cfg), &mut names);
    let resolver = Resolver::new(&program, &names);
    let mut interner = Interner::new();
    let mut c = dartforge_elements::generated::GenerationBuilder::new();
    let mut scoreboard = Scoreboard::default();
    generate_in(
        &package,
        &[root.join("lib")],
        &mut interner,
        &mut c,
        &mut scoreboard,
        Some(&resolver),
    );
    let ours = c.finish(1).expect("generation");

    let mut checked = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut different = Vec::new();
    for (path, f) in ours.iter() {
        let name = path.file_name().unwrap_or_default();
        let official = oracle.join(name);
        let Ok(expected) = std::fs::read_to_string(&official) else {
            continue;
        };
        checked += 1;
        seen.insert(name.to_owned());
        if expected.replace("\r\n", "\n") != f.content.as_ref() {
            different.push((
                path.clone(),
                expected.replace("\r\n", "\n"),
                f.content.to_string(),
            ));
        }
    }
    for (path, expected, actual) in &different {
        eprintln!("=== {} ===", path.display());
        for (i, (a, b)) in expected.lines().zip(actual.lines()).enumerate() {
            if a != b {
                eprintln!("line {}\n  official: {a}\n  ours:     {b}", i + 1);
            }
        }
        if expected.lines().count() != actual.lines().count() {
            eprintln!(
                "  lines: official {}, ours {}",
                expected.lines().count(),
                actual.lines().count()
            );
        }
    }
    assert!(
        different.is_empty(),
        "{} file(s) different from the official output",
        different.len()
    );
    println!(
        "ngdart corpus: {checked} checked, {} pending",
        scoreboard.pending.len()
    );
    for (p, c) in scoreboard.pending.iter().zip(&scoreboard.refusal_sets) {
        let refusals: Vec<String> = c.iter().map(|r| r.text()).collect();
        println!(
            "  pending {}: {}",
            p.file_name().unwrap_or_default().to_string_lossy(),
            refusals.join("; ")
        );
    }
    // The cases that exist to be refused: generating any of them would give
    // wrong output, and the refusal must come for the right reason.
    for (file, shape) in REFUSED {
        let found = scoreboard
            .pending
            .iter()
            .zip(&scoreboard.refusal_sets)
            .find(|(p, _)| p.file_name().is_some_and(|n| n == *file));
        let Some((_, set)) = found else {
            panic!("{file} should be refused ({shape}) and was generated");
        };
        assert!(
            set.iter().any(|r| r.shape == *shape),
            "{file} refused for another reason: {set:?}"
        );
    }
    let missing: Vec<_> = required.difference(&seen).collect();
    assert!(
        missing.is_empty(),
        "official artifacts were not generated: {missing:?}"
    );
}

/// Every `.dart` of the corpus has its official output: without it the corpus
/// silently stops covering what it claims to cover.
#[test]
fn every_corpus_file_has_an_oracle() {
    let root = corpus_root();
    let src = root.join("lib").join("src");
    let oracle = root.join("oraculo");
    assert!(
        src.is_dir() && oracle.is_dir(),
        "corpus sources or oracle are missing"
    );
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&src).expect("lib/src") {
        let p = entry.expect("corpus source entry").path();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.ends_with(".dart") || name.ends_with(".template.dart") {
            continue;
        }
        let expected = oracle.join(template_path(&p).file_name().unwrap_or_default());
        if !expected.is_file() {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "no oracle: {missing:?} — run scripts/corpus-ngdart.ps1"
    );
}

#[test]
fn registered_dependency_must_have_a_directory() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("package_config.json");
    std::fs::write(&file, r#"{"configVersion":2,"packages":[{"name":"missing","rootUri":"missing/","packageUri":"lib/"}]}"#).unwrap();
    let config = PackageConfig::load(&file).unwrap();
    assert!(
        dependency_directory(&config, "unknown")
            .unwrap_err()
            .contains("unknown")
    );
    assert!(
        dependency_directory(&config, "missing")
            .unwrap_err()
            .contains("directory is missing")
    );
    std::fs::create_dir_all(root.path().join("missing/lib")).unwrap();
    assert!(dependency_directory(&config, "missing").unwrap().is_dir());
}
