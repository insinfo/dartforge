//! Acceptance of the expansion cache (docs/MACROS-PROTOCOLO.md §6) and of the
//! diff-based reload, without a VM: a fake executor implements in Rust three
//! deterministic macros that use the real protocol (request with the member
//! prefetch, `macro.query` including a negative answer, a structured
//! result assembled and reloaded by the host).
//!
//! * `@Label` (declarations and definitions) — declares `label()` on the
//!   class; the output depends only on the class name;
//! * `@Fields` (declarations) — declares one getter per field (prefetch) and
//!   asks for the top-level function `helper` (the "does not exist" answer
//!   is a dependency too); emits a warning;
//! * `@Consumer` (declarations) — reads the methods of `A` (which `@Label`
//!   augments) and declares `withToJson()` or `withoutToJson()`.
//!
//! Each scenario compiles once, edits the source and compiles again with the
//! same cache; the second compilation must give **the same** as a clean
//! compilation (texts, order of the expansions, diagnostics) and only talk
//! to the executor when something observed changed.
use dartforge_elements::load::load_lenient_generated;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::unit_cache::AnalyzedUnitCache;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::assembly::{Code, ExecutionResult, Part};
use dartforge_macros_host::cache::MacroCache;
use dartforge_macros_host::executor::{Availability, ExecutionRequest, MacroExecutor, Phase, QueryService};
use dartforge_macros_host::protocol::Handshake;
use dartforge_macros_host::{Output, apply, apply_incremental};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn sdk() -> Option<SdkLayout> {
    let lib = SdkLayout::discover()?;
    let mut sdk = SdkLayout::load(&lib, "dartdevc").ok()?;
    sdk.current_version = LanguageVersion::new(3, 6);
    sdk.experiments = vec![Feature::Macros];
    Some(sdk)
}

/// The fake executor. `allowed: false` fails the test if it is touched.
struct Fake {
    allowed: bool,
    instances: Vec<String>,
    /// `(macro, phase, target)` of each execution, in order.
    executions: Vec<(String, Phase, String)>,
    started: bool,
    /// The macro that throws when executed (a macro's error).
    fail: Option<&'static str>,
}

impl Fake {
    fn new() -> Self {
        Fake { allowed: true, instances: Vec::new(), executions: Vec::new(), started: false, fail: None }
    }
    fn forbidden() -> Self {
        Fake { allowed: false, ..Fake::new() }
    }
    fn touch(&self) {
        assert!(self.allowed, "the executor must not be touched in this compilation");
    }
}

fn text(s: &str) -> Part {
    Part::Text(s.into())
}

fn member(parts: Vec<Part>) -> Code {
    Code { kind: "declaration".into(), parts }
}

/// `resolveIdentifier` of `dart:core`.
fn core(queries: &mut dyn QueryService, name: &str) -> u64 {
    let r = queries.query("resolveIdentifier", &json!({"uri": "dart:core", "name": name})).expect(name);
    r["id"].as_u64().unwrap()
}

impl MacroExecutor for Fake {
    fn availability(&self) -> Availability {
        self.touch();
        Availability::Available
    }
    fn start(&mut self) -> Result<Handshake, String> {
        self.touch();
        self.started = true;
        Ok(Handshake { executor: "fake".into(), abi: String::new(), macros: Vec::new() })
    }
    fn instantiate(&mut self, macro_: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        self.touch();
        self.instances.push(macro_.to_string());
        let interfaces: &[&str] = match macro_.rsplit('#').next() {
            Some("Label") => &["ClassDeclarationsMacro", "ClassDefinitionMacro"],
            _ => &["ClassDeclarationsMacro"],
        };
        Ok((self.instances.len() as u64, interfaces.iter().map(|s| s.to_string()).collect()))
    }
    fn execute(&mut self, p: &ExecutionRequest, queries: &mut dyn QueryService) -> Result<ExecutionResult, String> {
        self.touch();
        let macro_ = self.instances[(p.instance - 1) as usize].rsplit('#').next().unwrap().to_string();
        let class = p.target["ident"]["id"].as_u64().unwrap();
        let name = p.target["ident"]["name"].as_str().unwrap().to_string();
        let uri = p.target["lib"]["uri"].as_str().unwrap().to_string();
        self.executions.push((macro_.clone(), p.phase, name.clone()));
        if self.fail == Some(macro_.as_str()) {
            return Err(format!("{macro_}: exception in the macro"));
        }
        let mut r = ExecutionResult::default();
        match (macro_.as_str(), p.phase) {
            ("Label", Phase::Declarations) => {
                let string = core(queries, "String");
                r.types = vec![(class, vec![member(vec![text("  external "), Part::Ident(string), text(" label();")])])];
            }
            ("Label", Phase::Definitions) => {
                let string = core(queries, "String");
                let body = format!(" label() => '{name}';");
                r.types = vec![(class, vec![member(vec![text("  augment "), Part::Ident(string), text(&body)])])];
            }
            ("Fields", Phase::Declarations) => {
                let int = core(queries, "int");
                let fields = p.model["members"][class.to_string()]["fields"].as_array().cloned().unwrap_or_default();
                let mut members = Vec::new();
                for c in &fields {
                    let n = c["ident"]["name"].as_str().unwrap();
                    members.push(member(vec![text("  external "), Part::Ident(int), text(&format!(" get field_{n};"))]));
                }
                // The negative answer ("does not exist") is a dependency too.
                let helper = queries.query("resolveIdentifier", &json!({"uri": uri, "name": "helper"}));
                let mark = if helper.is_ok() { "withHelper" } else { "withoutHelper" };
                members.push(member(vec![text(&format!("  external void {mark}();"))]));
                r.types = vec![(class, members)];
                r.diagnostics =
                    vec![json!({"severity": "warning", "message": {"text": format!("{name}: {} fields", fields.len())}})];
            }
            ("Consumer", Phase::Declarations) => {
                let a = queries.query("resolveIdentifier", &json!({"uri": uri, "name": "A"})).unwrap();
                let methods = queries.query("members", &json!({"owner": a["id"], "kind": "methods"})).unwrap();
                let has = methods.as_array().unwrap().iter().any(|m| m["ident"]["name"] == "toJson");
                let int = core(queries, "int");
                let name = if has { " withToJson();" } else { " withoutToJson();" };
                r.types = vec![(class, vec![member(vec![text("  external "), Part::Ident(int), text(name)])])];
            }
            _ => {}
        }
        Ok(r)
    }
    fn shut_down(&mut self) {
        self.touch();
        self.started = false;
    }
}

/// The case's source, with each scenario's edits.
#[derive(Clone, Default)]
struct Source {
    body_of_m: &'static str,
    extra_field_in_a: bool,
    to_json_in_a: bool,
    extra_field_in_b: bool,
    helper: bool,
}

impl Source {
    fn text(&self) -> String {
        let mut s = String::from("import 'macros.dart';\n\n");
        if self.helper {
            s.push_str("void helper() {}\n\n");
        }
        s.push_str("@Label()\nclass A {\n  final int a;\n");
        if self.extra_field_in_a {
            s.push_str("  final int other = 2;\n");
        }
        s.push_str(&format!("  A(this.a);\n  void m() {{ {} }}\n", self.body_of_m));
        if self.to_json_in_a {
            s.push_str("  Map<String, Object?> toJson() => {};\n");
        }
        s.push_str("}\n\n@Fields()\nclass B {\n  final int x;\n");
        if self.extra_field_in_b {
            s.push_str("  final String y = '';\n");
        }
        s.push_str("  B(this.x);\n}\n\n@Consumer()\nclass C {}\n\nvoid main() {}\n");
        s
    }
}

/// Writes the macros' library (the applications live in another one, as the
/// language requires) and returns the path of the input.
fn directory(dir: &Path) -> PathBuf {
    std::fs::write(
        dir.join("macros.dart"),
        "macro class Label { const Label(); }\nmacro class Fields { const Fields(); }\nmacro class Consumer { const Consumer(); }\n",
    )
    .unwrap();
    dir.join("main.dart")
}

/// What a compilation produced, to compare incremental with clean.
#[derive(Debug, PartialEq)]
struct Product {
    texts: Vec<(PathBuf, String)>,
    warnings: Vec<String>,
    expansions: Vec<(String, Phase)>,
}

fn product(s: &Output) -> Product {
    Product {
        texts: s.texts.iter().map(|t| (t.path.clone(), t.text.clone())).collect(),
        warnings: s.warnings.iter().map(|d| format!("{:?} {}", d.span, d.message)).collect(),
        expansions: s.expansions.iter().map(|e| (e.application.to_string(), e.phase)).collect(),
    }
}

fn compile(input: &Path, sdk: &SdkLayout, executor: &mut dyn MacroExecutor, cache: Option<&mut MacroCache>) -> Output {
    let mut names = Interner::new();
    let (p, d) = load_lenient_generated(input, sdk, None, &mut names, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut load = |i: &mut Interner, g, u: &mut AnalyzedUnitCache| load_lenient_generated(input, sdk, None, i, None, Some(u), g);
    match apply_incremental(p, &mut names, None, &mut load, executor, cache) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    }
}

/// Compiles `source` from scratch (new cache, new executor).
fn clean(input: &Path, sdk: &SdkLayout, source: &Source) -> Product {
    std::fs::write(input, source.text()).unwrap();
    let mut cache = MacroCache::new("fake");
    product(&compile(input, sdk, &mut Fake::new(), Some(&mut cache)))
}

struct Scenario {
    _dir: tempfile::TempDir,
    input: PathBuf,
    sdk: SdkLayout,
    cache: MacroCache,
}

impl Scenario {
    /// Compiles `initial` once, filling the cache.
    fn new(initial: &Source) -> Option<Scenario> {
        let Some(sdk) = sdk() else {
            eprintln!("no SDK: skipped");
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        let input = directory(dir.path());
        std::fs::write(&input, initial.text()).unwrap();
        let mut cache = MacroCache::new("fake");
        let mut executor = Fake::new();
        let s = compile(&input, &sdk, &mut executor, Some(&mut cache));
        assert_eq!(s.executed_macros, 4, "{:?}", executor.executions);
        assert_eq!(s.measurement.reused, 0);
        assert!(!executor.started, "the session shuts the executor down");
        Some(Scenario { _dir: dir, input, sdk, cache })
    }

    /// Recompiles `source` with the cache and checks it against the clean
    /// compilation.
    fn recompile(&mut self, source: &Source, executor: &mut Fake) -> Output {
        std::fs::write(&self.input, source.text()).unwrap();
        let s = compile(&self.input, &self.sdk, executor, Some(&mut self.cache));
        let incremental = product(&s);
        assert_eq!(incremental, clean(&self.input, &self.sdk, source), "incremental differs from the clean compilation");
        s
    }
}

#[test]
fn recompilation_without_change_does_not_talk_to_the_executor() {
    let source = Source::default();
    let Some(mut c) = Scenario::new(&source) else { return };
    let s = c.recompile(&source, &mut Fake::forbidden());
    assert_eq!(s.executed_macros, 0);
    assert_eq!(s.measurement.reused, 4);
    assert!(!s.measurement.executor_started);
    assert!(s.expansions.iter().all(|e| e.reused));
    // The second one too (the cache stays full).
    let s = c.recompile(&source, &mut Fake::forbidden());
    assert_eq!(s.measurement.reused, 4);
}

#[test]
fn unobserved_edit_reuses() {
    let Some(mut c) = Scenario::new(&Source::default()) else { return };
    // A method body: no macro sees bodies.
    let source = Source { body_of_m: "print('another body');", ..Source::default() };
    let s = c.recompile(&source, &mut Fake::forbidden());
    assert_eq!((s.executed_macros, s.measurement.reused), (0, 4));
}

#[test]
fn new_field_invalidates_only_who_observes_it() {
    let Some(mut c) = Scenario::new(&Source::default()) else { return };
    let source = Source { extra_field_in_b: true, ..Source::default() };
    let mut executor = Fake::new();
    let s = c.recompile(&source, &mut executor);
    assert_eq!(executor.executions, vec![("Fields".to_string(), Phase::Declarations, "B".to_string())]);
    assert_eq!(s.measurement.reused, 3);
    assert!(s.texts[0].text.contains("field_y"), "{}", s.texts[0].text);
}

#[test]
fn previously_missing_member_invalidates() {
    // `helper` did not exist: the negative answer was a dependency of @Fields.
    let Some(mut c) = Scenario::new(&Source::default()) else { return };
    let source = Source { helper: true, ..Source::default() };
    let mut executor = Fake::new();
    let s = c.recompile(&source, &mut executor);
    assert_eq!(executor.executions, vec![("Fields".to_string(), Phase::Declarations, "B".to_string())]);
    assert!(s.texts[0].text.contains("withHelper"));

    // `toJson` in `A`: changes the model of @Label (which runs again and
    // gives the same) and the `membros` answer that @Consumer read (which
    // runs again and changes).
    let source = Source { helper: true, to_json_in_a: true, ..Source::default() };
    let mut executor = Fake::new();
    let s = c.recompile(&source, &mut executor);
    assert_eq!(
        executor.executions,
        vec![
            ("Label".to_string(), Phase::Declarations, "A".to_string()),
            ("Consumer".to_string(), Phase::Declarations, "C".to_string()),
            ("Label".to_string(), Phase::Definitions, "A".to_string()),
        ]
    );
    assert!(s.texts[0].text.contains("withToJson"));
}

#[test]
fn same_output_does_not_invalidate_consumers() {
    let Some(mut c) = Scenario::new(&Source::default()) else { return };
    // A new field in `A`: @Label runs again (its model changed) and gives the
    // same result; @Consumer reads the methods of `A`, which did not change,
    // and does not run.
    let source = Source { extra_field_in_a: true, ..Source::default() };
    let mut executor = Fake::new();
    let s = c.recompile(&source, &mut executor);
    assert_eq!(
        executor.executions,
        vec![
            ("Label".to_string(), Phase::Declarations, "A".to_string()),
            ("Label".to_string(), Phase::Definitions, "A".to_string()),
        ]
    );
    assert_eq!(s.measurement.rerun_unchanged, 2);
    assert_eq!(s.measurement.reused, 2);
}

#[test]
fn edited_implementation_invalidates_its_applications() {
    let source = Source::default();
    let Some(mut c) = Scenario::new(&source) else { return };
    // The source of the macros' library is the implementation's identity.
    let macros = c.input.with_file_name("macros.dart");
    let mut text = std::fs::read_to_string(&macros).unwrap();
    text.push_str("// version 2\n");
    std::fs::write(&macros, text).unwrap();
    let mut executor = Fake::new();
    let s = c.recompile(&source, &mut executor);
    assert_eq!((s.executed_macros, s.measurement.reused), (4, 0));
    assert_eq!(s.measurement.rerun_unchanged, 4);
}

#[test]
fn with_and_without_cache_give_the_same() {
    // The full reload of `apply` and the diff-based one of
    // `apply_incremental` produce the same program and the same text.
    let Some(sdk) = sdk() else { return };
    let dir = tempfile::tempdir().unwrap();
    let input = directory(dir.path());
    let source = Source { extra_field_in_b: true, to_json_in_a: true, ..Source::default() };
    std::fs::write(&input, source.text()).unwrap();
    let mut names = Interner::new();
    let (p, _) = load_lenient_generated(&input, &sdk, None, &mut names, None, None, None);
    let mut load = |i: &mut Interner, g| load_lenient_generated(&input, &sdk, None, i, None, None, g);
    let full = apply(p, &mut names, None, &mut load, &mut Fake::new()).unwrap_or_else(|_| panic!("error"));
    let by_diff = compile(&input, &sdk, &mut Fake::new(), None);
    assert_eq!(product(&full), product(&by_diff));
    assert!(by_diff.measurement.reused_units > 0);
    assert_eq!(full.program.units.len(), by_diff.program.units.len());
}

/// Compiles without panicking on error: what the host returns.
fn try_compile(input: &Path, sdk: &SdkLayout, executor: &mut dyn MacroExecutor, cache: &mut MacroCache) -> Result<Output, String> {
    let mut names = Interner::new();
    let (p, d) = load_lenient_generated(input, sdk, None, &mut names, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut load = |i: &mut Interner, g, u: &mut AnalyzedUnitCache| load_lenient_generated(input, sdk, None, i, None, Some(u), g);
    apply_incremental(p, &mut names, None, &mut load, executor, Some(cache))
        .map_err(|ds| ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n"))
}

/// B07: a macro that throws makes the compilation fail with its error, and
/// the cache remains that of the last valid generation — going back to the
/// previous source executes nothing and gives the same text as before;
/// fixing the macro reruns only the application that failed.
#[test]
fn macro_error_keeps_the_last_valid_generation() {
    let initial = Source::default();
    let Some(mut c) = Scenario::new(&initial) else { return };
    let before = product(&compile(&c.input, &c.sdk, &mut Fake::forbidden(), Some(&mut c.cache)));

    // A new field in `B` wakes @Fields, which now throws.
    let edited = Source { extra_field_in_b: true, ..Source::default() };
    std::fs::write(&c.input, edited.text()).unwrap();
    let mut broken = Fake::new();
    broken.fail = Some("Fields");
    let error = try_compile(&c.input, &c.sdk, &mut broken, &mut c.cache).err().expect("the macro's exception becomes an error");
    assert!(error.contains("Fields: exception in the macro"), "{error}");

    // Back to the valid source: nothing runs and the text is that of the
    // last valid generation.
    std::fs::write(&c.input, initial.text()).unwrap();
    let s = try_compile(&c.input, &c.sdk, &mut Fake::forbidden(), &mut c.cache).expect("valid source");
    assert_eq!(s.measurement.reused, 4);
    assert_eq!(product(&s), before, "the failure spoiled the valid generation");

    // The fixed macro: only the observed application runs again, and the
    // result is that of a clean compilation.
    let mut executor = Fake::new();
    let s = c.recompile(&edited, &mut executor);
    assert_eq!(executor.executions, vec![("Fields".to_string(), Phase::Declarations, "B".to_string())]);
    assert!(s.texts[0].text.contains("field_y"));
}

/// B07: with `main.macro.dart` materialized in both forms (the 3.6.2 one,
/// `import augment` + `augment library`, and the current one, `part` +
/// `part of`), the library does not run the macros again: no execution and
/// no new augmentation — the declarations do not come out twice.
#[test]
fn materialized_in_both_forms_does_not_duplicate() {
    let Some(sdk) = sdk() else { return };
    let dir = tempfile::tempdir().unwrap();
    let input = directory(dir.path());
    let source = Source::default().text();
    std::fs::write(&input, &source).unwrap();
    let mut cache = MacroCache::new("fake");
    let s = compile(&input, &sdk, &mut Fake::new(), Some(&mut cache));
    let (path, text) = (s.texts[0].path.clone(), s.texts[0].text.clone());
    assert!(path.ends_with("main.macro.dart"), "{}", path.display());
    let body = text.split_once('\n').map(|(_, r)| r).unwrap();
    for (header, directive) in [
        ("augment library 'main.dart';", "import augment 'main.macro.dart';"),
        ("part of 'main.dart';", "part 'main.macro.dart';"),
    ] {
        std::fs::write(&path, format!("{header}\n{body}")).unwrap();
        std::fs::write(&input, source.replacen("import 'macros.dart';", &format!("import 'macros.dart';\n{directive}"), 1)).unwrap();
        let mut names = Interner::new();
        let (p, d) = load_lenient_generated(&input, &sdk, None, &mut names, None, None, None);
        assert!(d.is_empty(), "{directive}: {d:?}");
        let mut load =
            |i: &mut Interner, g, u: &mut AnalyzedUnitCache| load_lenient_generated(&input, &sdk, None, i, None, Some(u), g);
        let s = apply_incremental(p, &mut names, None, &mut load, &mut Fake::forbidden(), None)
            .unwrap_or_else(|ds| panic!("{directive}: {:?}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>()));
        assert_eq!(s.executed_macros, 0, "{directive}");
        assert!(s.texts.is_empty(), "{directive}: augmentation assembled again over the materialized one");
    }
}
