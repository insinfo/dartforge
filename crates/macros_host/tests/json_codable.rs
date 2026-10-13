//! The host's whole path against the oracle: detection, order, the three
//! phases with the program reloaded, the model and the queries, the
//! `dfexec/1` protocol, our macro API (Dart, `packages/macros`) running the
//! `@JsonCodable` of `package:json` 0.20.4 **unchanged**, and the assembly —
//! the text comes out byte for byte equal to what CFE 3.6.2 generates
//! (`corpus/macros/*/esperado/*.augmentation.dart`, extracted from the
//! `.dill` by `scripts/oracle_augmentation.dart`).
//!
//! Two tests:
//!
//! * [`recorded_session_reproduces_the_cfe_text`] — with no executor at all:
//!   the fake executor replays the recorded session
//!   (`esperado/sessao.dfexec`) and checks that every host message —
//!   requests, model, answers to the queries — is the recorded one. It needs
//!   no VM: only the SDK (the libraries) and the case's `pub get` (the
//!   `package:json`), which CI runs before the `#[ignore]` tests.
//! * [`vm_runs_the_macro_and_matches_the_cfe`] (`#[ignore]`) — the
//!   materialization executor: the same API on a Dart 3.6.2 VM
//!   (DARTFORGE_DART_SDK) with the case's `pub get`.
//!   `DARTFORGE_RECORD_SESSION=1` records the session again.
use dartforge_elements::load::load_lenient_generated;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::unit_cache::AnalyzedUnitCache;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::Output;
use dartforge_macros_host::assembly::ExecutionResult;
use dartforge_macros_host::cache::MacroCache;
use dartforge_macros_host::executor::{
    Availability, DfexecExecutor, ExecutionRequest, MacroExecutor, QueryService, RecordedChannel, RecordingChannel, session_to_text,
};
use dartforge_macros_host::model::View;
use dartforge_macros_host::protocol::Handshake;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sdk() -> Option<SdkLayout> {
    let lib = SdkLayout::discover()?;
    let mut sdk = SdkLayout::load(&lib, "dartdevc").ok()?;
    sdk.current_version = LanguageVersion::new(3, 6);
    sdk.experiments = vec![Feature::Macros];
    Some(sdk)
}

struct Case {
    dir: PathBuf,
    input: PathBuf,
    config: PathBuf,
}

fn case(name: &str) -> Option<Case> {
    let dir = root().join("corpus/macros").join(name);
    let config = dir.join(".dart_tool/package_config.json");
    config.is_file().then(|| Case { input: dir.join("main.dart"), config, dir })
}

/// Runs the whole host with `executor` and returns the text of each
/// augmented library.
fn apply(c: &Case, sdk: &SdkLayout, executor: &mut dyn MacroExecutor) -> Vec<(PathBuf, String)> {
    let mut names = Interner::new();
    let (p, d) = load_lenient_generated(&c.input, sdk, Some(&c.config), &mut names, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut load = |i: &mut Interner, g| load_lenient_generated(&c.input, sdk, Some(&c.config), i, None, None, g);
    let output = match dartforge_macros_host::apply(p, &mut names, None, &mut load, executor) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    };
    output.texts.into_iter().map(|t| (t.path, t.text)).collect()
}

fn check(c: &Case, texts: &[(PathBuf, String)]) {
    assert!(!texts.is_empty(), "no augmentation assembled");
    for (path, text) in texts {
        let name = path.file_name().unwrap().to_string_lossy().replace(".macro.dart", ".augmentation.dart");
        let expected = std::fs::read_to_string(c.dir.join("esperado").join(&name)).expect("CFE golden");
        if *text != expected {
            let line = text.lines().zip(expected.lines()).position(|(a, b)| a != b).unwrap_or(0);
            panic!(
                "{name}: differs from CFE 3.6.2 at line {}\n  ours: {:?}\n  CFE:  {:?}\n--- ours ---\n{text}",
                line + 1,
                text.lines().nth(line),
                expected.lines().nth(line)
            );
        }
    }
}

#[test]
#[ignore = "requires the SDK (DARTFORGE_SDK_LIB) and the pub get of corpus/macros"]
fn recorded_session_reproduces_the_cfe_text() {
    let (Some(sdk), Some(c)) = (sdk(), case("410_json_codable")) else {
        eprintln!("no SDK or no pub get of corpus/macros/410_json_codable: skipped");
        return;
    };
    let recorded = std::fs::read_to_string(c.dir.join("esperado/sessao.dfexec")).expect("recorded session");
    let mut executor = DfexecExecutor::new(RecordedChannel::from_text(&recorded).unwrap());
    let texts = apply(&c, &sdk, &mut executor);
    assert_eq!(executor.channel().remaining(), 0, "part of the recorded session was left unconsumed");
    check(&c, &texts);
}

/// Like [`apply`], with the expansion cache and the diff-based reload.
fn apply_with_cache(c: &Case, sdk: &SdkLayout, executor: &mut dyn MacroExecutor, cache: &mut MacroCache) -> Output {
    let mut names = Interner::new();
    let (p, d) = load_lenient_generated(&c.input, sdk, Some(&c.config), &mut names, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut load =
        |i: &mut Interner, g, u: &mut AnalyzedUnitCache| load_lenient_generated(&c.input, sdk, Some(&c.config), i, None, Some(u), g);
    match dartforge_macros_host::apply_incremental(p, &mut names, None, &mut load, executor, Some(cache)) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    }
}

/// An executor that fails the test if it is touched.
struct Forbidden;

impl MacroExecutor for Forbidden {
    fn availability(&self) -> Availability {
        panic!("the executor could not be queried")
    }
    fn start(&mut self) -> Result<Handshake, String> {
        panic!("the executor could not be started")
    }
    fn instantiate(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        panic!("the executor could not instantiate")
    }
    fn execute(&mut self, _: &ExecutionRequest, _: &mut dyn QueryService) -> Result<ExecutionResult, String> {
        panic!("the executor could not execute")
    }
    fn shut_down(&mut self) {
        panic!("the executor could not be shut down")
    }
}

/// The real `@JsonCodable` (recorded session) with the cache: the first
/// compilation talks to the executor exactly like the recorded session (the
/// cache does not change the protocol) and the second reuses the 8
/// expansions without touching any executor; both give the CFE text.
#[test]
#[ignore = "requires the SDK (DARTFORGE_SDK_LIB) and the pub get of corpus/macros"]
fn cache_reuses_json_codable_without_executor() {
    let (Some(sdk), Some(c)) = (sdk(), case("410_json_codable")) else {
        eprintln!("no SDK or no pub get of corpus/macros/410_json_codable: skipped");
        return;
    };
    let recorded = std::fs::read_to_string(c.dir.join("esperado/sessao.dfexec")).expect("recorded session");
    let mut executor = DfexecExecutor::new(RecordedChannel::from_text(&recorded).unwrap());
    let mut cache = MacroCache::new("recorded session");
    let first = apply_with_cache(&c, &sdk, &mut executor, &mut cache);
    assert_eq!(executor.channel().remaining(), 0, "part of the recorded session was left unconsumed");
    assert_eq!((first.executed_macros, first.measurement.reused), (8, 0));
    let texts: Vec<_> = first.texts.iter().map(|t| (t.path.clone(), t.text.clone())).collect();
    check(&c, &texts);

    let second = apply_with_cache(&c, &sdk, &mut Forbidden, &mut cache);
    assert_eq!((second.executed_macros, second.measurement.reused), (0, 8));
    let texts: Vec<_> = second.texts.iter().map(|t| (t.path.clone(), t.text.clone())).collect();
    check(&c, &texts);
    let order = |s: &Output| s.expansions.iter().map(|e| (e.application.clone(), e.phase)).collect::<Vec<_>>();
    assert_eq!(order(&first), order(&second));
}

#[test]
#[ignore = "requires the Dart 3.6.2 SDK (DARTFORGE_DART_SDK) and the pub get of corpus/macros"]
fn vm_runs_the_macro_and_matches_the_cfe() {
    let dart = std::env::var_os("DARTFORGE_DART_SDK")
        .map(PathBuf::from)
        .or_else(|| SdkLayout::discover().and_then(|l| l.parent().map(Path::to_path_buf)))
        .map(|s| s.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }))
        .expect("DARTFORGE_DART_SDK");
    let (Some(sdk), Some(c)) = (sdk(), case("410_json_codable")) else {
        panic!("no SDK or no pub get of corpus/macros/410_json_codable");
    };
    let apps = {
        let mut names = Interner::new();
        let (p, _) = load_lenient_generated(&c.input, &sdk, Some(&c.config), &mut names, None, None, None);
        dartforge_macros_host::applications::detect(&View { program: &p, interner: &names })
    };
    assert_eq!(apps.len(), 4, "{apps:?}");
    let work = tempfile::tempdir().unwrap();
    let cfg = dartforge_macros_host::vm::VmConfig {
        dart,
        api: root().join("packages/macros"),
        work_dir: work.path().to_path_buf(),
    };
    let vm = dartforge_macros_host::vm::start(&cfg, &apps, Some(&c.config)).unwrap();
    let channel = RecordingChannel { inner: vm.into_channel(), session: Vec::new() };
    let mut executor = DfexecExecutor::new(channel);
    let texts = apply(&c, &sdk, &mut executor);
    if std::env::var_os("DARTFORGE_RECORD_SESSION").is_some() {
        let s = session_to_text(&executor.channel().session);
        std::fs::write(c.dir.join("esperado/sessao.dfexec"), s).unwrap();
    }
    check(&c, &texts);
}

/// The **native** executor (D4): the bootstrap compiled by DartForge's own
/// `compile-native`, without a Dart VM, runs the macros of each corpus case
/// and the assembled augmentation is CFE 3.6.2's, byte for byte.
///
/// Requires a `dartforge` with the `native` feature in
/// `DARTFORGE_NATIVE_COMPILER` (its `compile-native` compiles the
/// executor) and the `pub get` of each case.
#[test]
#[ignore = "requires DARTFORGE_NATIVE_COMPILER (dartforge with the native feature), the SDK and the pub get of corpus/macros"]
fn native_executor_matches_the_cfe() {
    let compiler = PathBuf::from(std::env::var_os("DARTFORGE_NATIVE_COMPILER").expect("DARTFORGE_NATIVE_COMPILER"));
    let sdk = sdk().expect("SDK (DARTFORGE_SDK_LIB)");
    let mut checked = 0;
    for name in ["410_json_codable", "411_pedido_independente", "412_argumento_posicional", "413_argumentos_nomeados"] {
        let Some(c) = case(name) else { panic!("no pub get of corpus/macros/{name}") };
        let apps = {
            let mut names = Interner::new();
            let (p, _) = load_lenient_generated(&c.input, &sdk, Some(&c.config), &mut names, None, None, None);
            dartforge_macros_host::applications::detect(&View { program: &p, interner: &names })
        };
        assert!(!apps.is_empty(), "{name}: no application");
        let work = tempfile::tempdir().unwrap();
        let cfg = dartforge_macros_host::native::NativeConfig {
            compiler: compiler.clone(),
            sdk_lib: sdk.root.clone(),
            api: root().join("packages/macros"),
            work_dir: work.path().to_path_buf(),
        };
        let t0 = std::time::Instant::now();
        let exe = dartforge_macros_host::native::compile(&cfg, &apps, Some(&c.config)).unwrap();
        let compilation = t0.elapsed();
        // The second time comes from the cache: the same key, no compilation.
        assert_eq!(dartforge_macros_host::native::compile(&cfg, &apps, Some(&c.config)).unwrap(), exe);
        let mut executor = dartforge_macros_host::native::start(&cfg, &apps, Some(&c.config)).unwrap();
        let t1 = std::time::Instant::now();
        let texts = apply(&c, &sdk, &mut executor);
        println!("{name}: compilation {:.1} s, application {:.0} ms", compilation.as_secs_f64(), t1.elapsed().as_secs_f64() * 1000.0);
        check(&c, &texts);
        checked += 1;
    }
    assert_eq!(checked, 4);
}
