//! ngdart through the engine (stage A) is **transparent**: the published
//! generation is the same as calling `ng_generator` directly
//! (`gerar_com_apoio`), and what it generates matches the oracle of
//! `corpus/ngdart/oraculo/` byte for byte — the same contract as
//! `crates/ng_generator/tests/corpus.rs`, now through the engine.
//! "Incremental = from scratch" also holds with template edits.
//!
//! Requires `dart pub get` in `corpus/ngdart` (the `package_config.json`):
//! hence `#[ignore]`, in the group that `ci.yml` satisfies.
use dartforge_build::query::NoDb;
use dartforge_build::{UpdateContext, Demand, Engine, EngineOptions};
use dartforge_elements::config::{PackageConfig, sem_verbatim};
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

fn sdk() -> SdkLayout {
    let dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    SdkLayout::load(&dir, "dartdevc").expect("SDK")
}

fn program(root: &Path) -> (dartforge_elements::model::Program, Interner) {
    let mut names = Interner::new();
    let cfg = root.join(".dart_tool/package_config.json");
    let (p, _) = dartforge_elements::load::load_lenient(
        &root.join("lib/corpus_ngdart.dart"),
        &sdk(),
        Some(&cfg),
        &mut names,
    );
    (p, names)
}

fn engine(root: &Path, p: &dartforge_elements::model::Program, names: &Interner) -> Engine {
    let cfg = PackageConfig::load(&root.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Engine::new(root, &cfg, EngineOptions::default()).expect("engine");
    m.update(
        &UpdateContext {
            db: &NoDb,
            program: Some((p, names)),
        },
        &[],
        Demand::Everything,
    )
    .expect("update");
    m
}

#[test]
#[ignore = "requires `dart pub get` in corpus/ngdart"]
fn ng_through_the_engine_equals_direct_generator_and_oracle() {
    let root = corpus();
    let (p, names) = program(&root);
    let m = engine(&root, &p, &names);
    let g = m.generation();

    // Directly, through ng_generator's public API.
    let resolver = dartforge_ng_generator::resolucao::Resolvedor::novo(&p, &names);
    let mut i = Interner::new();
    let package = dartforge_ng_generator::Pacote {
        nome: "corpus_ngdart".into(),
        raiz: root.clone(),
        ..Default::default()
    };
    let (direct, _) =
        dartforge_ng_generator::gerar_com_apoio(&package, &mut i, None, Some(&resolver));
    let expected: std::collections::HashSet<&PathBuf> = m.natural_paths_map.values().collect();
    let mut equal = 0;
    for (path, f) in direct.iter().filter(|(_, f)| f.gerador == "ngdart") {
        let k = dartforge_elements::gerado::chave(path);
        if !expected.contains(&k) {
            continue; // outside the plan (build_runner would not generate it either)
        }
        let via_engine = g.obter(&k).unwrap_or_else(|| {
            panic!("{}: generated directly and missing in the engine", k.display())
        });
        assert_eq!(
            via_engine.conteudo,
            f.conteudo,
            "{}: engine ≠ direct generator",
            k.display()
        );
        equal += 1;
    }
    assert!(equal > 0, "no native output checked");

    // Against the build_runner oracle.
    let oracle = root.join("oraculo");
    let mut checked = 0;
    let mut different = Vec::new();
    for (path, f) in g.iter().filter(|(_, f)| f.gerador == "ngdart:ngdart") {
        let Some(name) = path.file_name() else {
            continue;
        };
        let Ok(expected) = std::fs::read_to_string(oracle.join(name)) else {
            continue;
        };
        checked += 1;
        if expected.replace("\r\n", "\n") != *f.conteudo {
            different.push(path.display().to_string());
        }
    }
    println!(
        "ngdart through the engine: {equal} equal to the direct generator; {checked} checked against the oracle, {} different",
        different.len()
    );
    assert!(
        different.is_empty(),
        "different from the oracle: {different:?}"
    );
    assert!(checked >= 2);
}

fn copy_case(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (p, name) = (e.path(), e.file_name());
        let target = to.join(&name);
        if p.is_dir() {
            if name == ".dart_tool" {
                std::fs::create_dir_all(&target).unwrap();
                std::fs::copy(
                    p.join("package_config.json"),
                    target.join("package_config.json"),
                )
                .unwrap();
            } else if name != "oraculo" && name != "build" {
                copy_case(&p, &target);
            }
        } else {
            std::fs::copy(&p, &target).unwrap();
        }
    }
}

#[test]
#[ignore = "requires `dart pub get` in corpus/ngdart"]
fn ng_incremental_equals_from_scratch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("corpus_ngdart");
    copy_case(&corpus(), &root);
    let (p, names) = program(&root);
    let mut live = engine(&root, &p, &names);
    drop((p, names));
    // Edits: text in a template, a style, a component's `.dart` (body, a new
    // `@Input` in a child, a changed selector, a new import) and a new file.
    // Each one is `(file, snippet to replace or "" to append, new text)`.
    let htmls: Vec<PathBuf> = std::fs::read_dir(root.join("lib/src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "html"))
        .take(2)
        .collect();
    let src = root.join("lib/src");
    let mut steps: Vec<(PathBuf, &str, &str)> = htmls
        .iter()
        .map(|h| (h.clone(), "", "\n<span>edited</span>\n"))
        .collect();
    steps.push((
        src.join("b07_estilo.css"),
        "",
        "\n.b07-edited { color: red; }\n",
    ));
    steps.push((
        src.join("a02_texto_estatico.dart"),
        "",
        "\nint extraA02() => 1;\n",
    ));
    steps.push((
        src.join("a16_entrada_e_saida.dart"),
        "  String titulo = '';",
        "  String titulo = '';\n\n  @Input()\n  String subtitle = '';",
    ));
    steps.push((
        src.join("a13_componente_filho.dart"),
        "selector: 'a13-componente-filho'",
        "selector: 'a13-renamed-child'",
    ));
    steps.push((
        src.join("a13_componente_filho.dart"),
        "import 'a02_texto_estatico.dart';",
        "import 'a02_texto_estatico.dart';\nimport 'a01_interpolacao.dart';",
    ));
    steps.push((src.join("z_new.dart"), "", "class ZNew {}\n"));
    for (file, snippet, text) in steps {
        let mut current = std::fs::read_to_string(&file).unwrap_or_default();
        if snippet.is_empty() {
            current.push_str(text);
        } else {
            assert!(current.contains(snippet), "{}: snippet missing", file.display());
            current = current.replacen(snippet, text, 1);
        }
        std::fs::write(&file, &current).unwrap();
        let (p, names) = program(&root);
        let result = live
            .update(
                &UpdateContext {
                    db: &NoDb,
                    program: Some((&p, &names)),
                },
                &[file.clone()],
                Demand::Everything,
            )
            .expect("update");
        if file.extension().is_some_and(|e| e == "html" || e == "css") {
            assert_eq!(
                result.rel.native_unit_count, 1,
                "editing a resource must regenerate only its component"
            );
            assert_eq!(
                result.rel.generator_queries, 1,
                "only the digest of the changed resource must be recorded again"
            );
        }
        // An existing `.dart`: only the file and whoever reaches it through
        // import/export (the closure; `a02` is a child of 13 cases), not the
        // whole package (221 files).
        if file.extension().is_some_and(|e| e == "dart") && !file.ends_with("z_new.dart") {
            println!(
                "{}: {} units regenerated",
                file.display(),
                result.rel.native_unit_count
            );
            assert!(
                (1..=20).contains(&result.rel.native_unit_count),
                "{}: {} units regenerated",
                file.display(),
                result.rel.native_unit_count
            );
        }
        let fresh = engine(&root, &p, &names);
        assert_eq!(
            live.canonical_state(),
            fresh.canonical_state(),
            "incremental ≠ from scratch after {}",
            file.display()
        );
    }
}

/// Stage B sweep for `.dart` (B03): each corpus file gains a top-level
/// declaration, one at a time; the live engine (which regenerates only the
/// closure of importers) and a new engine must reach the same state.
///
/// The name stays in Portuguese: `ci.yml` skips it with
/// `--skip ng_edicao_dart_em_cada_arquivo`.
#[test]
#[ignore = "requires `dart pub get` in corpus/ngdart; slow (one new engine per file)"]
fn ng_edicao_dart_em_cada_arquivo_igual_ao_do_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("corpus_ngdart");
    copy_case(&corpus(), &root);
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("lib/src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dart"))
        .collect();
    files.sort();
    let (p, names) = program(&root);
    let mut live = engine(&root, &p, &names);
    drop((p, names));
    let (mut selective, mut total) = (0, 0);
    for (i, file) in files.iter().enumerate() {
        let mut text = std::fs::read_to_string(file).unwrap();
        text.push_str(&format!("\nint extraFromSweep{i}() => {i};\n"));
        std::fs::write(file, &text).unwrap();
        let (p, names) = program(&root);
        let at = live
            .update(
                &UpdateContext {
                    db: &NoDb,
                    program: Some((&p, &names)),
                },
                std::slice::from_ref(file),
                Demand::Everything,
            )
            .expect("update");
        total += 1;
        if at.rel.native_unit_count < 100 {
            selective += 1;
        }
        let fresh = engine(&root, &p, &names);
        assert_eq!(
            live.canonical_state(),
            fresh.canonical_state(),
            "incremental ≠ from scratch after {}",
            file.display()
        );
    }
    println!(
        "stage B on .dart: {selective} of {total} edits without regenerating the whole package"
    );
}

/// Dependent package (B03): `corpus/ngdart_dependente`, an application that
/// uses the component of a `path` dependency. The `.template.dart` files of
/// both packages come out of the native generator equal to the
/// `build_runner` oracle, and edits in the dependency and in the application
/// give the state of a from-scratch engine.
///
/// Requires `dart pub get` in `corpus/ngdart_dependente/app`.
#[test]
#[ignore = "requires `dart pub get` in corpus/ngdart_dependente/app"]
fn ng_dependency_through_the_engine_equals_oracle() {
    let origin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart_dependente");
    let tmp = tempfile::tempdir().unwrap();
    // The generation keys start from the canonical roots of `package_config`
    // (on Windows, the long name: the runner's temporary directory comes as
    // `C:\Users\RUNNER~1\…`, and the package root as `…\runneradmin\…`).
    let base = sem_verbatim(std::fs::canonicalize(tmp.path()).unwrap());
    for d in ["app", "dep"] {
        copy_case(&origin.join(d), &base.join(d));
    }
    let app = base.join("app");
    let dep = base.join("dep");
    let program = |app: &Path| {
        let mut names = Interner::new();
        let cfg = app.join(".dart_tool/package_config.json");
        // The component's library reaches the dependency; `web/main.dart`
        // only imports the `.template.dart`, which does not exist yet.
        let (p, _) = dartforge_elements::load::load_lenient(
            &app.join("lib/app.dart"),
            &sdk(),
            Some(&cfg),
            &mut names,
        );
        (p, names)
    };
    let fresh = |app: &Path, p: &dartforge_elements::model::Program, names: &Interner| {
        let cfg = PackageConfig::load(&app.join(".dart_tool/package_config.json"))
            .expect("package_config.json (dart pub get)");
        let mut m = Engine::new(app, &cfg, EngineOptions::default()).expect("engine");
        m.update(
            &UpdateContext {
                db: &NoDb,
                program: Some((p, names)),
            },
            &[],
            Demand::Everything,
        )
        .expect("update");
        m
    };
    let (p, names) = program(&app);
    let mut live = fresh(&app, &p, &names);
    let g = live.generation();
    let mut checked = 0;
    for (package, root) in [("app_ng", &app), ("dep_ng", &dep)] {
        let dir = origin.join("oraculo").join(package);
        let mut files = Vec::new();
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    files.push(e.path());
                }
            }
        }
        for o in files {
            let rel = o.strip_prefix(&dir).unwrap();
            let k = dartforge_elements::gerado::chave(&root.join(rel));
            let generated = g
                .obter(&k)
                .unwrap_or_else(|| panic!("{}: missing from the generation", k.display()));
            let expected = std::fs::read_to_string(&o).unwrap();
            assert_eq!(
                *generated.conteudo,
                expected,
                "{package}/{}: native ≠ oracle",
                rel.display()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 5);
    drop((p, names));
    // Edits: a resource and an `@Input` in the dependency, a body in the
    // application.
    let steps: [(PathBuf, &str, &str); 3] = [
        (
            dep.join("lib/src/botao.html"),
            "</button>",
            "</button><span>edited</span>",
        ),
        (
            dep.join("lib/src/botao.dart"),
            "  String rotulo = '';",
            "  String rotulo = '';\n\n  @Input()\n  bool active = false;",
        ),
        (
            app.join("lib/app.dart"),
            "  String texto = 'ok';",
            "  String texto = 'ok';\n  int counter = 0;",
        ),
    ];
    for (file, snippet, text) in steps {
        let current = std::fs::read_to_string(&file).unwrap();
        assert!(current.contains(snippet), "{}: snippet missing", file.display());
        std::fs::write(&file, current.replacen(snippet, text, 1)).unwrap();
        let (p, names) = program(&app);
        live.update(
            &UpdateContext {
                db: &NoDb,
                program: Some((&p, &names)),
            },
            &[file.clone()],
            Demand::Everything,
        )
        .expect("update");
        let zero = fresh(&app, &p, &names);
        assert_eq!(
            live.canonical_state(),
            zero.canonical_state(),
            "incremental ≠ from scratch after {}",
            file.display()
        );
    }
}
