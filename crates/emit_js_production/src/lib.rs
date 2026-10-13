//! **Production** profile of the JavaScript backend: one file, with only what
//! the closed world reaches. The whole plan, with the reference studied and
//! the measurements that motivate it, is in `docs/JS-PRODUCAO.md`.
//!
//! Two prunings, one for each kind of code (`docs/JS-PRODUCAO.md` §1.2):
//!
//! * **user and package code** is pruned **before** emission, by the closed
//!   world computed over the element model (`crates/world`); the development
//!   emitter (`crates/emit_js`) receives the world as a filter and does not
//!   emit what is dead. Without a filter it is byte for byte the usual one;
//! * **`dart_sdk.js`**, which comes already compiled by DDC, is pruned on the
//!   text (`sdk.rs`), with the roots read from the already-pruned user text.
//!
//! The emitted text is checked against the world (`verify.rs`): whatever the
//! text cites and the world pruned comes back as a root, and the world is
//! recomputed.

pub mod alias_this;
pub mod bundle;
pub mod cache;
pub mod filter;
pub mod flatten;
pub mod inline;
pub mod minify;
pub mod named_args;
pub mod own_sdk;
pub mod properties;
pub mod reach;
pub mod rti_rules;
pub mod scan;
pub mod sdk;
pub mod shapes;
pub mod verify;

use std::time::{Duration, Instant};

/// What the production compilation produced, with the numbers the report
/// cares about (and that `crates/differential` prints).
pub struct Production {
    /// The single file.
    pub js: String,
    /// Development-profile modules that went in.
    pub modules: usize,
    /// `import` cycles found (should be empty; see `bundle::order`).
    pub cycles: Vec<String>,
    /// Size of `dart_sdk.js` before and after pruning.
    pub sdk_before: usize,
    pub sdk_after: usize,
    /// Units of `dart_sdk.js`: total and live.
    pub sdk_units: usize,
    pub sdk_live: usize,
    /// Bytes of the user and package modules that went into the file.
    pub user: usize,
    /// File size before compaction (equal to `js.len()` without it).
    pub before_compaction: usize,
    /// The user's closed world, when computed.
    pub world: Option<WorldReport>,
    /// Where the `dart_sdk.js` index came from (`cache.rs`) and how much it
    /// cost to obtain; `None` when assembly built it on its own.
    pub index: Option<(cache::Origin, Duration)>,
    /// Fixed point and re-emission of `dart_sdk.js` over the index.
    pub prune_time: Duration,
}

/// Numbers of the user's closed world (`docs/PESQUISA-OTIMIZACAO.md` §10:
/// a report, not just a number).
#[derive(Debug, Clone, Default)]
pub struct WorldReport {
    pub stats: dartforge_world::Statistics,
    /// World computation (all rounds).
    pub world_time: Duration,
    /// Filtered emission (all rounds).
    pub emit_time: Duration,
    /// Text verifier (all rounds) — the cost of the always-on verifier.
    pub verify_time: Duration,
    /// World→emission→verification rounds until the fixed point (1 = nothing
    /// was missing).
    pub rounds: usize,
    /// The analysis did not converge and the file went out without pruning
    /// the user code (the full emission).
    pub pruning_disabled: bool,
    /// What the verifier found missing (gaps in the analysis).
    pub fixes: Vec<String>,
    /// Pending references with no matching element (should be empty).
    pub no_element: Vec<String>,
    /// Inconsistencies of the dry-run check (`DARTFORGE_JSPROD_CHECK=1`).
    pub inconsistencies: Option<usize>,
    /// Words of the live `JS()` *templates* in the final world (JS names).
    pub js_names: std::collections::HashSet<String>,
    /// The named-parameter map used in the final emission (`named_args.rs`).
    pub named_args: Option<crate::named_args::NamedArgs>,
}

/// Profile options.
#[derive(Clone, Copy)]
pub struct Options {
    /// Prune `dart_sdk.js` by reachability (step 2 of the plan).
    pub prune_sdk: bool,
    /// Split the classes of `dart_sdk.js` per member (step 4 of the plan).
    pub per_member: bool,
    /// Prune user code by the closed world (step 5 of the plan).
    pub prune_user: bool,
    /// Verifier mode: what the world says is dead is emitted as a *stub*
    /// that reports the call (`DARTFORGE-PODADO: …` on stderr, exit 97).
    pub stub: bool,
    /// Compact the file (comments and whitespace that does not separate
    /// tokens; `minify.rs`), step 7 of the plan.
    pub minify: bool,
    /// Compile the SDK through our own pipeline (the emitter's SDK mode,
    /// `docs/JS-PRODUCAO-SDK-PROPRIO.md`) instead of pruning DDC's
    /// `dart_sdk.js`. `--sdk-ddc` goes back to the old path.
    pub own_sdk: bool,
    /// `--enable-asserts`: the program's `assert`s are emitted. Off by
    /// default, as in `dart2js` (`docs/JS-PRODUCAO-TAMANHO.md` §3.3).
    pub asserts: bool,
    /// `--omit-checks`: no implicit `dynamic` conversions and no
    /// covariance checks, like `dart2js -O4`. Unsafe.
    pub omit_checks: bool,
}

impl Default for Options {
    fn default() -> Self {
        let stub = std::env::var("DARTFORGE_JSPROD_VERIFY").is_ok_and(|v| v == "stub");
        // The own SDK is the default (it passes the whole corpus/js the same
        // as the VM); `DARTFORGE_JSPROD_SDK=ddc` or `--sdk-ddc` go back to the
        // pruned `dart_sdk.js`. The verifier's *stub* mode only exists on the
        // old path.
        let own_sdk = !stub && !std::env::var("DARTFORGE_JSPROD_SDK").is_ok_and(|v| v == "ddc");
        Options { prune_sdk: true, per_member: true, prune_user: true, stub, minify: true, own_sdk, asserts: false, omit_checks: false }
    }
}

/// Compiles `input` in the production profile with the SDK compiled through
/// our own pipeline (`docs/JS-PRODUCAO-SDK-PROPRIO.md`): not a single byte of
/// `dart_sdk.js`.
///
/// # Errors
///
/// Those of loading, analyzing and emitting the program or the SDK.
pub fn compile_own_sdk(
    input: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    op: Options,
    language: &dartforge_elements::sdk::LanguageOptions,
) -> Result<Production, String> {
    let ((e, report, names, rti), _) = dartforge_emit_js::compile_with_options(input, sdk_lib, packages, language, None, true, |a| {
        let (e, report) = own_sdk::emit(a, op)?;
        let empty = std::collections::HashSet::new();
        let names = properties::compute(a, report.as_ref().map_or(&empty, |r| &r.js_names), report.as_ref().and_then(|r| r.named_args.as_ref()).map(|n| &n.map));
        Ok((e, report, names, own_sdk::recipe_names(a)))
    })?;
    let mut p = own_sdk::assemble(&e, op, Some(&names), Some(&rti));
    p.world = report;
    Ok(p)
}

/// Compiles `input` in the production profile.
///
/// `dart_sdk_js` is the DDC runtime (`runtime/ddc/dart_sdk.js`); it is read,
/// pruned and **embedded** in the result.
///
/// # Errors
///
/// Failure to read `dart_sdk_js`, and those of loading, analyzing and
/// emitting the program.
pub fn compile(
    input: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    dart_sdk_js: &std::path::Path,
    op: Options,
    language: &dartforge_elements::sdk::LanguageOptions,
) -> Result<Production, String> {
    let sdk_text = std::fs::read_to_string(dart_sdk_js).map_err(|e| format!("{}: {e}", dart_sdk_js.display()))?;
    // The runtime index depends only on the text and on the granularity: it
    // comes from the on-disk cache when there is a valid one (`cache.rs`).
    let t = Instant::now();
    let (index, origin) = cache::load(&sdk_text, op.per_member);
    let index_time = t.elapsed();
    let ((emitted, report), _) = dartforge_emit_js::compile_with(input, sdk_lib, packages, language, |a| emit_with_world(a, &index, op))?;
    let mut p = assemble_with_index(&emitted, &sdk_text, op, &index);
    p.world = report;
    p.index = Some((origin, index_time));
    Ok(p)
}

/// Roots of the user's world as seen from JS: `main` and the names that
/// `dart_sdk.js` calls by string (`sdk::dynamic_selectors`, stored in the
/// index).
fn roots(a: &dartforge_emit_js::Analysis<'_>, index: &sdk::SdkIndex) -> dartforge_world::Roots {
    use dartforge_elements::model::Element;
    let mut r = dartforge_world::Roots::default();
    if let (Some(lib), Some(sym)) = (a.program.entry, a.interner.lookup("main")) {
        if let Some(Element::Function(f)) = a.program.library(lib).declared.get(&sym).and_then(|b| b.getter) {
            r.functions.push(f);
        }
    }
    r.selectors = index.dynamic_selectors();
    r
}

/// Closed world → filtered emission → text verification, until the fixed
/// point.
///
/// # Errors
///
/// Those of the emission.
pub fn emit_with_world(a: &dartforge_emit_js::Analysis<'_>, index: &sdk::SdkIndex, op: Options) -> Result<(dartforge_emit_js::Emitted, Option<WorldReport>), String> {
    if !op.prune_user {
        return Ok((a.emit(None)?, None));
    }
    let input = dartforge_world::WorldInput { program: a.program, interner: a.interner, table: a.table, outline: a.outline, bodies: a.bodies };
    let mut report = WorldReport::default();
    let t = Instant::now();
    let mut roots = roots(a, index);
    report.world_time += t.elapsed();
    // `DARTFORGE_JSPROD_ROUNDS=1` shows the gaps in the analysis without
    // fixing them.
    let max_rounds: usize = std::env::var("DARTFORGE_JSPROD_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(8).max(1);
    for round in 1..=max_rounds {
        let t = Instant::now();
        let world = dartforge_world::compute(input, &roots);
        report.world_time += t.elapsed();
        let ad = filter::Adapter { world: &world, program: a.program, stub: op.stub, signatures: None, constants: None, named_args: None, omit_checks: false };
        let t = Instant::now();
        let emitted = a.emit(Some(&ad))?;
        report.emit_time += t.elapsed();
        let t = Instant::now();
        // JS identifier → library, to translate `L$x.Name` into an element.
        let libs = verify::libraries_from_text(&emitted.modules, a.program);
        let missing = verify::check(&emitted.modules, &libs, a.program, a.interner, &world);
        report.verify_time += t.elapsed();
        report.rounds = round;
        report.no_element = missing.no_element.clone();
        if missing.is_empty() || round == max_rounds {
            let mut emitted = emitted;
            if !missing.is_empty() {
                report.fixes.extend(missing.description.iter().cloned());
                report.fixes.push(format!("fixed point not reached in {max_rounds} round(s)"));
                // Without the fixed point, the pruning was not validated: the
                // file goes out with the whole user code (the safe superset),
                // never with the pruned version that still references what it
                // removed.
                let t = Instant::now();
                emitted = a.emit(None)?;
                report.emit_time += t.elapsed();
                report.pruning_disabled = true;
                report.fixes.push("user code pruning disabled: the analysis did not converge".to_string());
            }
            report.stats = world.stats.clone();
            if std::env::var("DARTFORGE_JSPROD_CHECK").is_ok_and(|v| v != "0") {
                let inc = dartforge_world::check_consistency(input, &roots, &world);
                for i in inc.iter().take(10) {
                    eprintln!("[jsprod] world inconsistency: {i:?}");
                }
                report.inconsistencies = Some(inc.len());
            }
            if let Ok(target) = std::env::var("DARTFORGE_JSPROD_WHY") {
                explain(a, &world, &target);
            }
            return Ok((emitted, Some(report)));
        }
        report.fixes.extend(missing.description.iter().cloned());
        roots.functions.extend(missing.functions);
        roots.variables.extend(missing.variables);
        roots.type_classes.extend(missing.classes);
        roots.instantiated_classes.extend(missing.instantiated_classes);
        roots.tearoffs.extend(missing.tearoffs);
        roots.selectors.extend(missing.selectors);
    }
    unreachable!()
}

/// `DARTFORGE_JSPROD_WHY=Name`: why the class (or function) `Name` is in the
/// file — the chain of causes up to the root.
fn explain(a: &dartforge_emit_js::Analysis<'_>, world: &dartforge_world::World, target: &str) {
    use dartforge_elements::model::{ClassId, FunctionElementId};
    use dartforge_world::Cause;
    let p = a.program;
    let fn_name = |f: FunctionElementId| {
        let func = p.function(f);
        let c = func.class.map(|c| format!("{}.", a.interner.resolve(p.class(c).name))).unwrap_or_default();
        format!("{}::{c}{}", p.library(func.library).uri, a.interner.resolve(func.name))
    };
    let mut start: Vec<Cause> = Vec::new();
    for (i, c) in p.classes.iter().enumerate() {
        if a.interner.resolve(c.name) == target && !p.library(c.library).is_sdk {
            start.push(Cause::Class(ClassId(i as u32)));
        }
    }
    for (i, f) in p.functions.iter().enumerate() {
        if a.interner.resolve(f.name) == target && !p.library(f.library).is_sdk && world.is_live_function(FunctionElementId(i as u32)) {
            start.push(Cause::Function(FunctionElementId(i as u32)));
        }
    }
    for c in start {
        let mut chain: Vec<String> = Vec::new();
        let mut cur = Some(c);
        while let Some(x) = cur {
            if chain.len() > 40 {
                break;
            }
            let (txt, next) = match x {
                Cause::Root => ("(root)".to_string(), None),
                Cause::Function(f) => (fn_name(f), world.function_cause(f)),
                Cause::Class(k) => (format!("class {} [{:?}]", a.interner.resolve(p.class(k).name), world.class_level(k)), world.class_cause(k)),
                Cause::Variable(v) => (format!("variable {}", a.interner.resolve(p.variable(v).name)), world.variable_cause(v)),
            };
            chain.push(txt);
            cur = next;
        }
        eprintln!("[jsprod] {}", chain.join(" <- "));
    }
}

/// Like [`compile`], starting from a development emission already done.
/// Kept separate so that the test needs neither disk nor SDK; the runtime
/// index is built here, without the cache.
pub fn assemble(emitted: &dartforge_emit_js::Emitted, sdk_text: &str, op: Options) -> Production {
    assemble_with_index(emitted, sdk_text, op, &sdk::index(sdk_text, op.per_member))
}

/// Like [`assemble`], with the index of `sdk_text` already obtained (from
/// the cache).
pub fn assemble_with_index(emitted: &dartforge_emit_js::Emitted, sdk_text: &str, op: Options, index: &sdk::SdkIndex) -> Production {
    let modules: Vec<bundle::Module> = emitted
        .modules
        .iter()
        // The development profile's `preambulo.js` exists only to give
        // `self` to `dart_sdk.js` on Node; in the bundle that is the first
        // line.
        .filter(|(p, _)| p != "preambulo.js")
        .map(|(p, t)| bundle::split(p, t))
        .collect();
    let (modules, cycles) = bundle::order(modules);
    let entry = bundle::entry_from_mjs(&emitted.entry, &modules)
        .unwrap_or_else(|| "L$main".to_string());

    let sdk_before = sdk_text.len();
    let t = Instant::now();
    let (sdk_pruned, units, live) = if op.prune_sdk {
        let roots = sdk::user_roots(&modules);
        sdk::prune_with_index(sdk_text, index, &roots)
    } else {
        (bundle::sdk_without_export(sdk_text), 0, 0)
    };
    let prune_time = t.elapsed();
    let sdk_after = sdk_pruned.len();
    let user = modules.iter().map(|m| m.body.len() + m.namespaces.iter().map(|n| n.len() + 1).sum::<usize>()).sum();

    let preamble = if op.stub { bundle::STUB_PREAMBLE } else { "" };
    let js = bundle::assemble(&sdk_pruned, &modules, &entry, preamble);
    debug_assert!(
        // Determinism is a requirement (`docs/PESQUISA-OTIMIZACAO.md` §11):
        // `dartforge serve` reloads per generation, and a generation that
        // changes for no reason is a defect the user sees. In debug builds,
        // this checks for free that the file does not depend on the internal
        // order of anything.
        bundle::assemble(&sdk_pruned, &modules, &entry, preamble) == js,
        "assembly is not deterministic"
    );
    let before_compaction = js.len();
    let js = if op.minify { minify::compact(&js) } else { js };
    Production {
        js,
        modules: modules.len(),
        cycles,
        sdk_before,
        sdk_after,
        sdk_units: units,
        sdk_live: live,
        user,
        before_compaction,
        world: None,
        index: None,
        prune_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitted() -> dartforge_emit_js::Emitted {
        dartforge_emit_js::Emitted {
            modules: vec![
                (
                    "main.js".into(),
                    concat!(
                        "var L$main = Object.create(dart.library);\n",
                        "export { L$main as main };\n",
                        "import { core, dart } from './dart_sdk.js';\n",
                        "import { util as L$util } from './util.js';\n",
                        "L$main.main = function main() { core.print(L$util.dobro(21)); };\n",
                    )
                    .into(),
                ),
                (
                    "util.js".into(),
                    concat!(
                        "var L$util = Object.create(dart.library);\n",
                        "export { L$util as util };\n",
                        "import { dart } from './dart_sdk.js';\n",
                        "L$util.dobro = function dobro(x) { return x * 2; };\n",
                    )
                    .into(),
                ),
                ("preambulo.js".into(), "if (typeof self === 'undefined') globalThis.self = globalThis;\n".into()),
            ],
            entry: "import { main as m } from './main.js';\nm.main();\n".into(),
        }
    }

    const SDK: &str = concat!(
        "var core = Object.create(dart.library);\n",
        "export { dart, core };\n",
        "core.print = function print(o) { console.log(o); };\n",
        "core.Morta = class Morta {};\n",
    );

    #[test]
    fn assembles_one_file_in_topological_order() {
        let p = assemble(&emitted(), SDK, Options { prune_sdk: false, per_member: false, prune_user: false, stub: false, minify: false, own_sdk: false, asserts: true, omit_checks: false });
        assert!(p.cycles.is_empty());
        assert_eq!(p.modules, 2);
        // `util.js` imports nothing, so it comes before `main.js`.
        let a = p.js.find("L$util.dobro").unwrap();
        let b = p.js.find("L$main.main").unwrap();
        assert!(a < b, "a dependency must come before whoever imports it");
        // No `import`/`export` in the final file.
        assert!(!p.js.contains("\nimport "), "an import was left over");
        assert!(!p.js.contains("\nexport "), "an export was left over");
        // The namespaces are hoisted; the bodies stay in IIFEs.
        assert!(p.js.contains("var L$util = Object.create(dart.library);\n"));
        assert!(p.js.contains("(function () {"));
        assert!(p.js.trim_end().ends_with("L$main.main();"));
    }

    /// The libraries stay **separate** after bundling: this is the rule at
    /// the top of `docs/JS-PRODUCAO.md` (share representation, yes; unify
    /// library identity, no).
    #[test]
    fn bundling_does_not_merge_libraries() {
        let p = assemble(&emitted(), SDK, Options { prune_sdk: false, per_member: false, prune_user: false, stub: false, minify: false, own_sdk: false, asserts: true, omit_checks: false });
        assert_eq!(p.js.matches("Object.create(dart.library)").count(), 3, "core + the user's two");
    }

    /// Determinism (`docs/PESQUISA-OTIMIZACAO.md` §11): two assemblies of the
    /// same inputs give the same file, byte for byte.
    #[test]
    fn assembly_is_deterministic() {
        let op = Options::default();
        let a = assemble(&emitted(), SDK, op);
        let b = assemble(&emitted(), SDK, op);
        assert_eq!(a.js, b.js);
        assert_eq!(a.sdk_after, b.sdk_after);
    }

    /// Pruning removes from the runtime what the program does not reach, and
    /// keeps what it reaches — in the single file, not in a `dart_sdk.js`
    /// next to it.
    #[test]
    fn prunes_the_embedded_runtime() {
        let with = assemble(&emitted(), SDK, Options { prune_sdk: true, per_member: false, prune_user: false, stub: false, minify: false, own_sdk: false, asserts: true, omit_checks: false });
        let without = assemble(&emitted(), SDK, Options { prune_sdk: false, per_member: false, prune_user: false, stub: false, minify: false, own_sdk: false, asserts: true, omit_checks: false });
        assert!(with.js.contains("core.print = function"), "what the program uses stays");
        assert!(!with.js.contains("core.Morta"), "what it does not use goes");
        assert!(with.sdk_after < without.sdk_after);
    }
}
