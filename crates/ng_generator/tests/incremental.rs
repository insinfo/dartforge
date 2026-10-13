//! The build engine API (`docs/BUILD-PEDIDOS-GERADOR-NG.md`): generating one
//! file at a time, with the index maintained incrementally, gives the same
//! text as generating the whole package (`gerar_em`), and reports what it
//! queried. Runs over the ngdart corpus, with the semantic database.
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_ng_generator::resolucao::Resolvedor;
use dartforge_ng_generator::{
    ConsultaNg, Indice, Pacote, Placar, analisar_arquivo, caminho_do_template, gerar_arquivo,
    gerar_em,
};
use dartforge_intern::Interner;
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
    let resolver = Resolvedor::novo(&program, &names);
    let package = Pacote {
        nome: "corpus_ngdart".into(),
        raiz: root.clone(),
        ..Default::default()
    };

    // The whole package, through the usual path.
    let mut c = dartforge_elements::gerado::Construtor::nova();
    let mut scoreboard = Placar::default();
    let mut i1 = Interner::new();
    gerar_em(
        &package,
        &[root.join("lib")],
        &mut i1,
        &mut c,
        &mut scoreboard,
        Some(&resolver),
    );
    let whole_package = c.concluir(1).expect("generation");

    // One at a time: the program index, and each file put back by
    // `atualizar` as the engine does when it changes.
    let mut index = Indice::do_programa(&resolver);
    let mut i2 = Interner::new();
    let mut findings = Vec::new();
    for p in sources(&root) {
        let text = std::fs::read_to_string(&p).unwrap();
        let a = analisar_arquivo(&p, &text, &mut i2);
        index.atualizar(&package, &p, &a, Some(&resolver));
        findings.push((p, a));
    }
    let mut generated = 0;
    let mut d01_queries = Vec::new();
    for (p, a) in &findings {
        let expected = whole_package
            .iter()
            .find(|(k, _)| k.file_name() == caminho_do_template(p).file_name())
            .map(|(_, f)| f.conteudo.to_string());
        match gerar_arquivo(&package, p, a, Some(&resolver), &mut i2, &index) {
            Ok(s) => {
                generated += 1;
                assert_eq!(
                    Some(s.template.clone()),
                    expected,
                    "{} differs from the package generation",
                    p.display()
                );
                if p.ends_with("d01_dois_filhos.dart") {
                    d01_queries = s.consultas.clone();
                }
            }
            Err(r) => assert!(
                expected.is_none(),
                "{} refused on its own ({}) and generated in the package",
                p.display(),
                r.texto()
            ),
        }
    }
    assert!(generated > 50, "only {generated} generated one at a time");
    assert!(
        d01_queries.iter().any(|c| matches!(
            c,
            ConsultaNg::Filho { classe, seletor, .. }
                if classe == "A02TextoEstatico" && seletor == "a02-texto-estatico"
        )),
        "d01 queried the children: {d01_queries:?}"
    );

    // Removing a file from the index drops what it declared, and whoever used
    // it now queries the missing selector.
    let child = root.join("lib").join("src").join("a02_texto_estatico.dart");
    assert!(index.declarante("a02-texto-estatico").is_some());
    index.remover(&child);
    assert!(index.declarante("a02-texto-estatico").is_none());
    let (p, a) = findings
        .iter()
        .find(|(p, _)| p.ends_with("d01_dois_filhos.dart"))
        .unwrap();
    let after = gerar_arquivo(&package, p, a, Some(&resolver), &mut i2, &index);
    assert!(after.is_err(), "d01 still generated without the child in the index");
}
