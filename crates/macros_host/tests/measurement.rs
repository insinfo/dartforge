//! Measurement of the macro host (docs/MACROS-PROTOCOLO.md §8): the time of
//! macro execution, of the queries, of the assembly and of the reload,
//! separately, on `corpus/macros/410_json_codable` and on a synthetic case
//! with `N` `@JsonCodable` classes in a single library (where the
//! accumulated reassembly of the declarations phase shows up). For each
//! case:
//!
//! 1. clean compilation with [`apply`] (full reload, no cache — the path
//!    before this measurement);
//! 2. clean compilation with [`apply_incremental`] (diff-based reload,
//!    empty cache);
//! 3. recompilation without change with the cache (no executor at all);
//! 4. recompilation after adding a field to a class (the cache reruns only
//!    the expansions that observe it).
//!
//! It measures performance, not correctness (CONTRIBUTING.md): the test
//! prints the numbers and checks only the essentials. It requires the Dart
//! 3.6.2 VM (`DARTFORGE_DART_SDK`) and the `pub get` of
//! `corpus/macros/410_json_codable`:
//!
//! ```text
//! cargo test --release -p dartforge-macros-host --test measurement -- --ignored --nocapture
//! ```
use dartforge_elements::load::load_lenient_generated;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::unit_cache::AnalyzedUnitCache;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::cache::MacroCache;
use dartforge_macros_host::executor::{DfexecExecutor, MacroExecutor};
use dartforge_macros_host::model::View;
use dartforge_macros_host::protocol::CanalDeProcesso;
use dartforge_macros_host::{Output, apply, apply_incremental};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sdk() -> SdkLayout {
    let lib = SdkLayout::discover().expect("SDK (DARTFORGE_SDK_LIB)");
    let mut sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
    sdk.current_version = LanguageVersion::new(3, 6);
    sdk.experiments = vec![Feature::Macros];
    sdk
}

fn dart() -> PathBuf {
    std::env::var_os("DARTFORGE_DART_SDK")
        .map(PathBuf::from)
        .or_else(|| SdkLayout::discover().and_then(|l| l.parent().map(Path::to_path_buf)))
        .map(|s| s.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }))
        .expect("DARTFORGE_DART_SDK")
}

/// The VM as executor, with the bootstrap of the applications of `input`.
fn vm(input: &Path, config: &Path, sdk: &SdkLayout, work: &Path) -> DfexecExecutor<CanalDeProcesso> {
    let mut names = Interner::new();
    let (p, _) = load_lenient_generated(input, sdk, Some(config), &mut names, None, None, None);
    let apps = dartforge_macros_host::applications::detect(&View { program: &p, interner: &names });
    let cfg = dartforge_macros_host::vm::VmConfig {
        dart: dart(),
        api: root().join("packages/macros"),
        work_dir: work.to_path_buf(),
    };
    dartforge_macros_host::vm::start(&cfg, &apps, Some(config)).unwrap()
}

fn failed(ds: Vec<dartforge_diagnostics::Diagnostic>) -> ! {
    panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n"))
}

/// One compilation; `cache: None` uses [`apply`] (full reload).
fn compile(
    input: &Path,
    config: &Path,
    sdk: &SdkLayout,
    executor: &mut dyn MacroExecutor,
    cache: Option<&mut MacroCache>,
    by_diff: bool,
) -> Output {
    let mut names = Interner::new();
    let (p, d) = load_lenient_generated(input, sdk, Some(config), &mut names, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    if by_diff {
        let mut load =
            |i: &mut Interner, g, u: &mut AnalyzedUnitCache| load_lenient_generated(input, sdk, Some(config), i, None, Some(u), g);
        apply_incremental(p, &mut names, None, &mut load, executor, cache).unwrap_or_else(|d| failed(d))
    } else {
        let mut load = |i: &mut Interner, g| load_lenient_generated(input, sdk, Some(config), i, None, None, g);
        apply(p, &mut names, None, &mut load, executor).unwrap_or_else(|d| failed(d))
    }
}

fn report(case: &str, stage: &str, total: std::time::Duration, s: &Output) {
    eprintln!(
        "[measurement] {case} | {stage}: total {:.1} ms | {}",
        total.as_secs_f64() * 1000.0,
        s.measurement.summary()
    );
}

/// The four stages of a case; `edit` makes the edit of stage 4.
fn measure(case: &str, input: &Path, config: &Path, sdk: &SdkLayout, edit: impl FnOnce()) {
    let work = tempfile::tempdir().unwrap();

    let t = Instant::now();
    let mut executor = vm(input, config, sdk, work.path());
    let clean = compile(input, config, sdk, &mut executor, None, false);
    report(case, "1 clean, full reload", t.elapsed(), &clean);

    let t = Instant::now();
    let mut executor = vm(input, config, sdk, work.path());
    let mut cache = MacroCache::new("vm dart 3.6.2");
    let by_diff = compile(input, config, sdk, &mut executor, Some(&mut cache), true);
    report(case, "2 clean, diff-based reload", t.elapsed(), &by_diff);
    let texts = |s: &Output| s.texts.iter().map(|t| t.text.clone()).collect::<Vec<_>>();
    assert_eq!(texts(&clean), texts(&by_diff));

    // No change: no executor is created.
    let t = Instant::now();
    let mut unavailable = dartforge_macros_host::executor::Unavailable::default();
    let unchanged = compile(input, config, sdk, &mut unavailable, Some(&mut cache), true);
    report(case, "3 recompilation without change", t.elapsed(), &unchanged);
    assert_eq!(unchanged.executed_macros, 0);
    assert_eq!(texts(&clean), texts(&unchanged));

    edit();
    let t = Instant::now();
    let mut executor = vm(input, config, sdk, work.path());
    let edited = compile(input, config, sdk, &mut executor, Some(&mut cache), true);
    report(case, "4 recompilation with a new field", t.elapsed(), &edited);
}

/// `n` `@JsonCodable` classes with three fields each, in one library.
fn synthetic(n: usize, extra: bool) -> String {
    let mut s = String::from("import 'package:json/json.dart';\n\n");
    for i in 0..n {
        let field = if extra && i == 0 { "  final bool? added;\n" } else { "" };
        s.push_str(&format!(
            "@JsonCodable()\nclass C{i} {{\n  final String name;\n  final int? value;\n  final List<int> scores;\n{field}}}\n\n"
        ));
    }
    s.push_str("void main() {\n  print(C0.fromJson({'name': 'a', 'scores': <int>[]}).toJson());\n}\n");
    s
}

#[test]
#[ignore = "measurement: requires the Dart 3.6.2 VM (DARTFORGE_DART_SDK) and the pub get of corpus/macros/410_json_codable"]
fn host_measurement() {
    let sdk = sdk();
    let case = root().join("corpus/macros/410_json_codable");
    let config = case.join(".dart_tool/package_config.json");
    assert!(config.is_file(), "missing the pub get of {}", case.display());
    // 410 in a copy (stage 4 edits the library).
    let copy = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(copy.path().join("lib")).unwrap();
    std::fs::copy(case.join("main.dart"), copy.path().join("main.dart")).unwrap();
    let models = copy.path().join("lib/modelos.dart");
    std::fs::copy(case.join("lib/modelos.dart"), &models).unwrap();
    let input = copy.path().join("main.dart");
    // `package:caso_json/` points at the copy.
    let mut pc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    for p in pc["packages"].as_array_mut().unwrap() {
        if p["name"] == "caso_json" {
            p["rootUri"] = serde_json::json!(format!("file://{}/", copy.path().display()));
        } else if let Some(r) = p["rootUri"].as_str().filter(|r| !r.contains(':')) {
            let abs = config.parent().unwrap().join(r);
            p["rootUri"] = serde_json::json!(format!("file://{}/", abs.display()));
        }
    }
    let copy_config = copy.path().join("package_config.json");
    std::fs::write(&copy_config, pc.to_string()).unwrap();
    measure("410_json_codable", &input, &copy_config, &sdk, || {
        let text = std::fs::read_to_string(&models).unwrap();
        std::fs::write(&models, text.replacen("  final int? numero;\n", "  final int? numero;\n  final bool? novo;\n", 1)).unwrap();
    });

    let n: Vec<usize> = std::env::var("DARTFORGE_MEDICAO_N")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![8, 32]);
    for n in n {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("main.dart");
        std::fs::write(&input, synthetic(n, false)).unwrap();
        measure(&format!("synthetic N={n}"), &input, &config, &sdk, || std::fs::write(&input, synthetic(n, true)).unwrap());
    }
}
