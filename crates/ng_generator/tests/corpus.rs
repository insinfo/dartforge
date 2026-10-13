//! The ngdart corpus: one shape per file, with the `.template.dart` of the
//! official compiler next to it, in `corpus/ngdart/oraculo/`.
//!
//! The contract is the same as the JS backend corpus: what we generate **must**
//! come out byte for byte equal to the official output. A shape we cannot
//! generate yet is not a failure; a shape we generate wrong is.
//!
//! The oracle is regenerated with `scripts/corpus-ngdart.ps1` (runs the
//! official `build_runner` once).
use dartforge_elements::config::PackageConfig;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_ng_generator::resolucao::Resolvedor;
use dartforge_ng_generator::{Pacote, Placar, caminho_do_template, gerar_em};
use dartforge_intern::Interner;
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
    if !oracle.is_dir() {
        eprintln!(
            "no oracle in {}; run scripts/corpus-ngdart.ps1",
            oracle.display()
        );
        return;
    }
    let package = Pacote {
        nome: "corpus_ngdart".into(),
        raiz: root.clone(),
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
        assert!(
            config.packages.contains_key(&dep),
            "`{dep}` missing from {}: run `dart pub get` in corpus/ngdart",
            cfg.display()
        );
    }
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let Ok(sdk) = SdkLayout::load(&sdk_dir, "dartdevc") else {
        eprintln!("SDK unavailable; the corpus was not checked");
        return;
    };
    let mut names = Interner::new();
    let (program, _) = load_lenient(&entry, &sdk, Some(&cfg), &mut names);
    let resolver = Resolvedor::novo(&program, &names);
    let mut interner = Interner::new();
    let mut c = dartforge_elements::gerado::Construtor::nova();
    let mut scoreboard = Placar::default();
    gerar_em(
        &package,
        &[root.join("lib")],
        &mut interner,
        &mut c,
        &mut scoreboard,
        Some(&resolver),
    );
    let ours = c.concluir(1).expect("generation");

    let mut checked = 0usize;
    let mut different = Vec::new();
    for (path, f) in ours.iter() {
        let name = path.file_name().unwrap_or_default();
        let official = oracle.join(name);
        let Ok(expected) = std::fs::read_to_string(&official) else {
            continue;
        };
        checked += 1;
        if expected.replace("\r\n", "\n") != f.conteudo.as_ref() {
            different.push((
                path.clone(),
                expected.replace("\r\n", "\n"),
                f.conteudo.to_string(),
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
        scoreboard.pendentes.len()
    );
    for (p, c) in scoreboard.pendentes.iter().zip(&scoreboard.conjuntos) {
        let refusals: Vec<String> = c.iter().map(|r| r.texto()).collect();
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
            .pendentes
            .iter()
            .zip(&scoreboard.conjuntos)
            .find(|(p, _)| p.file_name().is_some_and(|n| n == *file));
        let Some((_, set)) = found else {
            panic!("{file} should be refused ({shape}) and was generated");
        };
        assert!(
            set.iter().any(|r| r.forma == *shape),
            "{file} refused for another reason: {set:?}"
        );
    }
    // Do not let the corpus become decoration: at least the shapes we already
    // declared covered must be here.
    assert!(
        checked >= 2,
        "the corpus must check at least the covered shapes"
    );
}

/// Every `.dart` of the corpus has its official output: without it the corpus
/// silently stops covering what it claims to cover.
#[test]
fn every_corpus_file_has_an_oracle() {
    let root = corpus_root();
    let src = root.join("lib").join("src");
    let oracle = root.join("oraculo");
    if !src.is_dir() || !oracle.is_dir() {
        return;
    }
    let mut missing = Vec::new();
    for e in std::fs::read_dir(&src).expect("lib/src").flatten() {
        let p = e.path();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.ends_with(".dart") || name.ends_with(".template.dart") {
            continue;
        }
        let expected = oracle.join(caminho_do_template(&p).file_name().unwrap_or_default());
        if !expected.is_file() {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "no oracle: {missing:?} — run scripts/corpus-ngdart.ps1"
    );
}
