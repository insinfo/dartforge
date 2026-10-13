//! The build engine API (`docs/BUILD-PEDIDOS-GERADOR-NG.md`): generating one
//! file at a time, with the index maintained incrementally, gives the same
//! text as generating the whole package (`generate_in`), and reports what it
//! queried. Runs over the ngdart corpus, with the semantic database.
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_ng_generator::resolution::Resolver;
use dartforge_ng_generator::{
    NgIndex, NgQuery, Package, Scoreboard, analyze_file, template_path, generate_in,
    generate_file,
};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

fn sources(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root.join("lib").join("src"))
        .expect("lib/src of the corpus")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.ends_with(".dart") && !n.ends_with(".template.dart")
        })
        .collect();
    v.sort();
    v
}

#[test]
fn one_file_at_a_time_gives_the_same_as_the_package() {
    let root = root();
    let cfg = root.join(".dart_tool").join("package_config.json");
    assert!(
        cfg.is_file(),
        "missing {}: run `dart pub get` in corpus/ngdart",
        cfg.display()
    );
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let Ok(sdk) = SdkLayout::load(&sdk_dir, "dartdevc") else {
        eprintln!("SDK unavailable; the incremental API was not checked");
        return;
    };
    let mut names = Interner::new();
    let entry = root.join("lib").join("corpus_ngdart.dart");
    let (program, _) = load_lenient(&entry, &sdk, Some(&cfg), &mut names);
    let resolver = Resolver::new(&program, &names);
    let package = Package {
        name: "corpus_ngdart".into(),
        root: root.clone(),
        ..Default::default()
    };

    // The whole package, through the usual path.
    let mut c = dartforge_elements::generated::GenerationBuilder::new();
    let mut scoreboard = Scoreboard::default();
    let mut i1 = Interner::new();
    generate_in(
        &package,
        &[root.join("lib")],
        &mut i1,
        &mut c,
        &mut scoreboard,
        Some(&resolver),
    );
    let whole_package = c.finish(1).expect("generation");

    // One at a time: the program index, and each file put back by
    // `update` as the engine does when it changes.
    let mut index = NgIndex::from_program(&resolver);
    let mut i2 = Interner::new();
    let mut findings = Vec::new();
    for p in sources(&root) {
        let text = std::fs::read_to_string(&p).unwrap();
        let a = analyze_file(&p, &text, &mut i2);
        index.update(&package, &p, &a, Some(&resolver));
        findings.push((p, a));
    }
    let mut generated = 0;
    let mut d01_queries = Vec::new();
    for (p, a) in &findings {
        let expected = whole_package
            .iter()
            .find(|(k, _)| k.file_name() == template_path(p).file_name())
            .map(|(_, f)| f.content.to_string());
        match generate_file(&package, p, a, Some(&resolver), &mut i2, &index) {
            Ok(s) => {
                generated += 1;
                assert_eq!(
                    Some(s.template.clone()),
                    expected,
                    "{} differs from the package generation",
                    p.display()
                );
                if p.ends_with("d01_dois_filhos.dart") {
                    d01_queries = s.queries.clone();
                }
            }
            Err(r) => assert!(
                expected.is_none(),
                "{} refused on its own ({}) and generated in the package",
                p.display(),
                r.text()
            ),
        }
    }
    assert!(generated > 50, "only {generated} generated one at a time");
    assert!(
        d01_queries.iter().any(|c| matches!(
            c,
            NgQuery::Child { class, selector, .. }
                if class == "A02TextoEstatico" && selector == "a02-texto-estatico"
        )),
        "d01 queried the children: {d01_queries:?}"
    );

    // Removing a file from the index drops what it declared, and whoever used
    // it now queries the missing selector.
    let child = root.join("lib").join("src").join("a02_texto_estatico.dart");
    assert!(index.declarer("a02-texto-estatico").is_some());
    index.remove(&child);
    assert!(index.declarer("a02-texto-estatico").is_none());
    let (p, a) = findings
        .iter()
        .find(|(p, _)| p.ends_with("d01_dois_filhos.dart"))
        .unwrap();
    let after = generate_file(&package, p, a, Some(&resolver), &mut i2, &index);
    assert!(
        after.is_err(),
        "d01 still generated without the child in the index"
    );
}
