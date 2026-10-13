//! JavaScript emission in the DDC module contract (docs/EMISSAO-DDC.md).
//!
//! One ES6 module per user library, linking against `dart_sdk.js`.
#![allow(clippy::too_many_arguments, clippy::collapsible_if, clippy::collapsible_else_if)]

pub mod body;
pub mod call;
pub mod type_check;
pub mod ctx;
pub mod expr;
pub mod filter;
pub mod js;
pub mod module;
pub mod pattern;
pub mod own_sdk;
pub mod extension_type;

/// The property of a private name (or of a field storage, `C.x`/`_#C#x`) of a
/// program library in the production profile ([`module::ModState::private_sym`]):
/// the name with `.`/`#` replaced by `$` and the library tag at the end. Storage
/// starts with `$`.
pub fn production_private_name(name: &str, lib: dartforge_elements::model::LibraryId) -> String {
    let safe: String = name.chars().map(|c| if c == '.' || c == '#' { '$' } else { c }).collect();
    let prefix = if safe.starts_with('_') { "" } else { "$" };
    format!("{prefix}{safe}{}", recipe_tag(lib))
}

/// The Dart name of a [`production_private_name`] property (without the tag),
/// when the text has its shape.
pub fn dart_name_of_private(s: &str) -> Option<&str> {
    if !s.starts_with('_') {
        return None;
    }
    let (base, tag) = s.rsplit_once('$')?;
    (!base.is_empty() && !tag.is_empty() && tag.bytes().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())).then_some(base)
}

/// Classes that get the one-letter ids in the rti recipes of SDK mode
/// ([`recipe_ids`]): the ones that show up most in any program.
const FREQUENT_CLASSES: &[(&str, &str)] = &[
    ("dart:core", "int"),
    ("dart:core", "String"),
    ("dart:core", "bool"),
    ("dart:core", "double"),
    ("dart:core", "num"),
    ("dart:core", "Object"),
    ("dart:core", "Null"),
    ("dart:core", "List"),
    ("dart:core", "Map"),
    ("dart:core", "Iterable"),
    ("dart:core", "Function"),
    ("dart:async", "Future"),
    ("dart:core", "Set"),
    ("dart:core", "Type"),
    ("dart:core", "Record"),
    ("dart:core", "Symbol"),
    ("dart:core", "Comparable"),
    ("dart:core", "Iterator"),
    ("dart:core", "MapEntry"),
    ("dart:core", "Duration"),
    ("dart:core", "DateTime"),
    ("dart:core", "StackTrace"),
    ("dart:core", "Enum"),
    ("dart:core", "Pattern"),
    ("dart:core", "Match"),
    ("dart:core", "Error"),
    ("dart:core", "Exception"),
    ("dart:async", "Stream"),
    ("dart:async", "StreamSubscription"),
    ("dart:async", "Completer"),
    ("dart:async", "FutureOr"),
    ("dart:_interceptors", "JSArray"),
    ("dart:_interceptors", "JSString"),
    ("dart:_interceptors", "JSNumber"),
    ("dart:_interceptors", "JSInt"),
    ("dart:_interceptors", "JSBool"),
    ("dart:_interceptors", "JSObject"),
    ("dart:_interceptors", "LegacyJavaScriptObject"),
    ("dart:_interceptors", "JavaScriptObject"),
    ("dart:_interceptors", "JavaScriptFunction"),
    ("dart:_interceptors", "NativeError"),
    ("dart:_js_helper", "TrustedGetRuntimeType"),
    ("dart:_internal", "EfficientLengthIterable"),
    ("dart:collection", "LinkedHashMap"),
    ("dart:collection", "LinkedHashSet"),
    ("dart:collection", "ListBase"),
    ("dart:collection", "MapBase"),
];

/// First character of a recipe id ([`recipe_ids`]).
pub const ID_FIRST: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
/// Following characters of a recipe id.
pub const ID_NEXT: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789";

/// The ordinal of an id from [`recipe_ids`] (the inverse of the numbering):
/// `a` is 0, `$` is 53, `aa` is 54.
///
/// ```
/// assert_eq!(dartforge_emit_js::numero_de_id("a"), Some(0));
/// assert_eq!(dartforge_emit_js::numero_de_id("aa"), Some(54));
/// assert_eq!(dartforge_emit_js::numero_de_id("ba"), Some(55));
/// assert_eq!(dartforge_emit_js::numero_de_id("ab"), Some(54 + 54));
/// ```
pub fn id_number(id: &str) -> Option<usize> {
    let b = id.as_bytes();
    let first = *b.first()?;
    let mut n = ID_FIRST.iter().position(|c| *c == first)?;
    let mut weight = ID_FIRST.len();
    for c in &b[1..] {
        n += (ID_NEXT.iter().position(|x| x == c)? + 1) * weight;
        weight *= ID_NEXT.len();
    }
    Some(n)
}

/// The short id of each class (index: `ClassId`) in the rti recipes of SDK mode
/// (`Ctx::class_recipe`, `docs/JS-PRODUCAO-TAMANHO.md` §9.5): first the
/// [`FREQUENT_CLASSES`], then the others in program order, each one with the
/// next name of the sequence `a`, `b`, …, `$`, `aa`, `ba`… (the first letter out
/// of 54, the following ones out of 64). A function of the program only: the
/// emitter, the checker and the name table arrive at the same id.
pub fn recipe_ids(program: &dartforge_elements::model::Program, interner: &dartforge_intern::Interner) -> Vec<String> {
    use dartforge_elements::model::{ClassId, Element};
    const FIRST: &[u8] = ID_FIRST;
    const OTHERS: &[u8] = ID_NEXT;
    let name = |mut n: usize| -> String {
        let mut s = vec![FIRST[n % FIRST.len()]];
        n /= FIRST.len();
        while n > 0 {
            n -= 1;
            s.push(OTHERS[n % OTHERS.len()]);
            n /= OTHERS.len();
        }
        String::from_utf8(s).unwrap_or_default()
    };
    let mut order: Vec<ClassId> = Vec::with_capacity(program.classes.len());
    let mut seen = vec![false; program.classes.len()];
    for (uri, n) in FREQUENT_CLASSES {
        let Some(lib) = program.libraries.iter().position(|l| l.uri == *uri) else { continue };
        let Some(sym) = interner.lookup(n) else { continue };
        if let Some(Element::Class(c)) = program.library(dartforge_elements::model::LibraryId(lib as u32)).declared.get(&sym).and_then(|b| b.getter)
            && !seen[c.0 as usize]
        {
            seen[c.0 as usize] = true;
            order.push(c);
        }
    }
    for (i, v) in seen.iter().enumerate() {
        if !v {
            order.push(ClassId(i as u32));
        }
    }
    let mut out = vec![String::new(); program.classes.len()];
    for (k, c) in order.into_iter().enumerate() {
        out[c.0 as usize] = name(k);
    }
    out
}

/// Short library tag in the rti recipes of the production profile
/// (`Ctx::class_recipe` in SDK mode): `$` + the id in base 36. It starts with
/// `$`, which no SDK library identifier uses.
pub fn recipe_tag(lib: dartforge_elements::model::LibraryId) -> String {
    let mut n = lib.0;
    let mut d: Vec<u8> = Vec::new();
    loop {
        let r = (n % 36) as u8;
        d.push(if r < 10 { b'0' + r } else { b'a' + r - 10 });
        n /= 36;
        if n == 0 {
            break;
        }
    }
    d.reverse();
    format!("${}", String::from_utf8(d).unwrap_or_default())
}
pub mod ty;

/// Language options (`--language-version`, `--enable-experiment`) of the caller of
/// [`compile_with`]; see `docs/VERSOES-LINGUAGEM.md`.
pub use dartforge_elements::sdk::Linguagem;

use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{CoreTypes, TypeTable};

/// Emission result: the modules (file name, text) and `main.mjs`.
pub struct Emitted {
    pub modules: Vec<(String, String)>,
    pub entry: String,
}

/// The analyzed program (elements, outline, bodies), handed to callers compiling
/// in the production profile so they can compute the closed world before emitting.
pub struct Analysis<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
}

impl<'a> Analysis<'a> {
    /// Emission context without a filter (for queries: interop, JS names).
    pub fn ctx(&self) -> ctx::Ctx<'a> {
        ctx::Ctx::new(self.program, self.interner, self.table, self.core, self.outline, self.bodies)
    }

    /// Emits the program. `filter: None` is exactly [`emit_program`].
    pub fn emit<'b>(&'b self, filter: Option<&'b dyn filter::Liveness>) -> Result<Emitted, String> {
        let mut ctx: ctx::Ctx<'b> = ctx::Ctx::new(self.program, self.interner, self.table, self.core, self.outline, self.bodies);
        ctx.filter = filter;
        module::emit_with_cache(&ctx, None, None).map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))
    }

    /// Emits the program **and the SDK** through our own pipeline (SDK mode,
    /// [`own_sdk`]; `docs/JS-PRODUCAO-SDK-PROPRIO.md`). The SDK bodies must
    /// have been inferred ([`compile_with_options`] with `infer_sdk`).
    ///
    /// # Errors
    ///
    /// The language errors found during emission, one per line.
    pub fn emit_with_sdk<'b>(&'b self, filter: Option<&'b dyn filter::Liveness>) -> Result<EmittedWithSdk, String> {
        let mut ctx: ctx::Ctx<'b> = ctx::Ctx::new(self.program, self.interner, self.table, self.core, self.outline, self.bodies);
        ctx.filter = filter;
        let mode = own_sdk::SdkMode::new(&ctx);
        ctx.sdk = Some(Box::new(mode));
        let join = |ds: Vec<Diagnostic>| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        let emitted = module::emit_with_cache(&ctx, None, None).map_err(join)?;
        let (sdk, mut dartx) = own_sdk::emit_sdk_module(&ctx);
        let errors = std::mem::take(&mut *ctx.errors.borrow_mut());
        if !errors.is_empty() {
            return Err(join(errors));
        }
        // The `dartx` symbols that the program modules reference in the prelude
        // (`var $x = dartx.x;`), so the *bootstrap* creates each of them once.
        for (_, t) in &emitted.modules {
            for line in t.lines() {
                let Some(r) = line.strip_prefix("var $") else { continue };
                let Some((_, value)) = r.split_once(" = dartx") else { continue };
                let value = value.trim_end_matches(';');
                if let Some(n) = value.strip_prefix('.') {
                    dartx.insert(n.to_string());
                } else if let Some(n) = value.strip_prefix("[\"").and_then(|x| x.strip_suffix("\"]")) {
                    dartx.insert(n.replace("\\\\", "\\").replace("\\\"", "\""));
                }
            }
        }
        let late = sdk.contains("dart.lateFields(") || emitted.modules.iter().any(|(_, t)| t.contains("dart.lateFields("));
        let bootstrap = own_sdk::bootstrap(&ctx, &dartx, late);
        Ok(EmittedWithSdk { emitted: emitted, sdk, bootstrap })
    }
}

/// The program and the SDK emitted through our own pipeline ([`Analysis::emit_with_sdk`]).
pub struct EmittedWithSdk {
    /// The program modules, as in [`Analysis::emit`].
    pub emitted: Emitted,
    /// The SDK module (without the *bootstrap*).
    pub sdk: String,
    /// The *bootstrap*: `dart`, the namespaces, `dartx` and its symbols.
    pub bootstrap: String,
}

/// Emits one module per non-SDK library of the program, plus `main.mjs`.
pub fn emit_program(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
) -> Result<Emitted, Vec<Diagnostic>> {
    emit_program_with_cache(program, interner, table, core, outline, bodies, None)
}

/// Like [`emit_program`], keeping the emitted fragments so the next compilation
/// reuses what did not change.
pub fn emit_program_with_cache(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    cache: Option<&std::cell::RefCell<module::FragmentCache>>,
) -> Result<Emitted, Vec<Diagnostic>> {
    let ctx = ctx::Ctx::new(program, interner, table, core, outline, bodies);
    module::emit_with_cache(&ctx, None, cache)
}

/// Emits only the modules that contain one of the given libraries, for the
/// resident session (`dartforge dev`): the JS of a library that did not change,
/// and whose dependencies did not change their API, is still valid.
///
/// `Emitted::modules` carries only the emitted modules; `Emitted::entry` (the
/// `main.mjs`) always comes, because it is cheap and depends only on the entry
/// point. Also returns how long building the emission context took.
pub fn emit_modules(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    libraries: &[dartforge_elements::model::LibraryId],
) -> Result<(Emitted, std::time::Duration), Vec<Diagnostic>> {
    emit_modules_with_cache(program, interner, table, core, outline, bodies, libraries, None)
}

/// Like [`emit_modules`], reusing the fragments (text per class and per library)
/// already emitted. The session invalidates those of the changed libraries before
/// calling: that is what makes an edit re-emit one class instead of the 1,343 of a
/// module with 345 libraries in a cycle.
#[allow(clippy::too_many_arguments)]
pub fn emit_modules_with_cache(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    libraries: &[dartforge_elements::model::LibraryId],
    cache: Option<&std::cell::RefCell<module::FragmentCache>>,
) -> Result<(Emitted, std::time::Duration), Vec<Diagnostic>> {
    let t = std::time::Instant::now();
    let ctx = ctx::Ctx::new(program, interner, table, core, outline, bodies);
    let context = t.elapsed();
    let only: std::collections::HashSet<u32> = libraries.iter().map(|l| l.0).collect();
    module::emit_with_cache(&ctx, Some(&only), cache).map(|e| (e, context))
}

/// Per-phase timings and counts of a compilation (`dartforge compile-js --timings`).
#[derive(Debug, Default, Clone)]
pub struct Report {
    /// `(phase, duration)` in the order the phases ran.
    pub phases: Vec<(&'static str, std::time::Duration)>,
    /// SDK units (`.dart`), patches included.
    pub sdk_units: usize,
    /// User and package units.
    pub user_units: usize,
    pub libraries: usize,
    /// Dart sources that came from in-memory generation, not from disk.
    pub generated_sources: usize,
    pub outline_warnings: usize,
    pub body_warnings: usize,
    pub modules: usize,
    /// `"read"`, `"built"` or `""` (disabled/unavailable).
    pub sdk_cache: &'static str,
    /// Sub-phases of `load_lenient` (printed under "load program").
    pub load: dartforge_elements::model::TemposCarga,
}

impl Report {
    /// Records a phase measured from `start`.
    pub fn phase(&mut self, name: &'static str, start: std::time::Instant) {
        self.phases.push((name, start.elapsed()));
    }

    /// One line of text per phase, with the total.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut total = std::time::Duration::ZERO;
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        for (name, d) in &self.phases {
            out.push_str(&format!("{name:<24}{:>9.1} ms\n", ms(*d)));
            total += *d;
            if *name == "load program" {
                let c = &self.load;
                let sub = [
                    ("parallel read+lex", c.leitura_lex_paralelo),
                    ("serial read+lex", c.leitura),
                    ("parse", c.parse),
                    ("SDK from cache", c.sdk_cache),
                    ("directives/URIs", c.diretivas),
                    ("outline: declarations", c.outline_declaracoes),
                    ("outline: reexports", c.outline_reexports),
                    ("outline: scopes", c.outline_escopos),
                    ("outline: supertypes", c.outline_supertipos),
                ];
                for (n, d) in sub {
                    out.push_str(&format!("  {n:<22}{:>9.1} ms\n", ms(d)));
                }
                out.push_str(&format!(
                    "  {:<22}{:>9} files, {:.2} MiB, {} waves\n",
                    "read",
                    c.arquivos_lidos,
                    c.bytes_lidos as f64 / 1_048_576.0,
                    c.ondas
                ));
            }
        }
        out.push_str(&format!("{:<24}{:>9.1} ms\n", "total", total.as_secs_f64() * 1000.0));
        out.push_str(&format!(
            "units: {} SDK + {} user/packages; {} libraries; {} modules; warnings: {} outline + {} bodies; SDK cache: {}\n",
            self.sdk_units, self.user_units, self.libraries, self.modules, self.outline_warnings, self.body_warnings,
            if self.sdk_cache.is_empty() { "not used" } else { self.sdk_cache }
        ));
        out
    }
}

/// Runs the whole pipeline (elements → outline → bodies → emission) for an entry point.
///
/// `sdk_lib` is the SDK's `lib/` (discovered through `PATH` when `None`).
pub fn compile(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
) -> Result<Emitted, String> {
    compile_with_report(entry, sdk_lib, packages, &Default::default()).map(|(e, _)| e)
}

/// The project's `package_config.json`, from the given path or discovered from the
/// entry point.
fn package_config(
    entry: &std::path::Path,
    packages: Option<&std::path::Path>,
) -> Option<dartforge_elements::config::PackageConfig> {
    packages
        .map(|p| p.to_path_buf())
        .or_else(|| dartforge_elements::config::PackageConfig::discover(entry))
        .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok())
}

/// `DARTFORGE_GERADOS_PKGS=a,b` restricts generation to those packages — useful
/// for comparing against the disk without changing anything else.
fn package_filter() -> Option<std::collections::HashSet<String>> {
    std::env::var("DARTFORGE_GERADOS_PKGS")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
}

/// Like [`compile`], also returning the per-phase timing [`Report`].
pub fn compile_with_report(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    language: &dartforge_elements::sdk::Linguagem,
) -> Result<(Emitted, Report), String> {
    compile_with_report_and_generator(entry, sdk_lib, packages, language, None)
}

/// Whoever produces the generated sources (the build engine, `crates/build`) from
/// the program loaded without them — the `BuildStep.resolver` in one pass.
pub type Generator<'a> = &'a dyn Fn(
    &dartforge_elements::model::Program,
    &Interner,
) -> Result<std::sync::Arc<dartforge_elements::gerado::Geracao>, String>;

/// Locates the Dart API used by the macro executor, without looking it up for
/// programs that do not apply macros.
fn macros_api(entry: &std::path::Path) -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("DARTFORGE_MACROS_API").map(std::path::PathBuf::from) {
        return p.join("pubspec.yaml").is_file().then_some(p);
    }
    let bases = [std::env::current_exe().ok(), std::env::current_dir().ok(), Some(entry.to_path_buf())];
    for base in bases.into_iter().flatten() {
        for dir in base.ancestors() {
            let p = dir.join("packages/macros");
            if p.join("pubspec.yaml").is_file() { return Some(p); }
        }
    }
    None
}

/// Like [`compile_with_report`], with the project's source generator.
pub fn compile_with_report_and_generator(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    language: &dartforge_elements::sdk::Linguagem,
    generator: Option<Generator<'_>>,
) -> Result<(Emitted, Report), String> {
    let (emitted, mut rel) = compile_with_generator(entry, sdk_lib, packages, language, generator, |a| {
        emit_program(a.program, a.interner, a.table, a.core, a.outline, a.bodies)
            .map_err(|ds| ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))
    })?;
    rel.modules = emitted.modules.len();
    Ok((emitted, rel))
}

/// The whole pipeline up to body inference; emission is up to the caller
/// (`finish`), which receives the analyzed program. The production profile
/// computes the closed world in there, before emitting (`crates/emit_js_production`).
pub fn compile_with<R>(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    language: &dartforge_elements::sdk::Linguagem,
    finish: impl FnOnce(&Analysis<'_>) -> Result<R, String>,
) -> Result<(R, Report), String> {
    compile_with_generator(entry, sdk_lib, packages, language, None, finish)
}

/// Like [`compile_with`], with the source generator (the build engine).
pub fn compile_with_generator<R>(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    language: &dartforge_elements::sdk::Linguagem,
    generator: Option<Generator<'_>>,
    finish: impl FnOnce(&Analysis<'_>) -> Result<R, String>,
) -> Result<(R, Report), String> {
    compile_with_options(entry, sdk_lib, packages, language, generator, false, finish)
}

/// Like [`compile_with_generator`]; with `infer_sdk`, the bodies of the SDK
/// libraries are inferred as well — which SDK mode ([`Analysis::emit_with_sdk`])
/// and the closed world with the SDK need.
///
/// # Errors
///
/// The same as [`compile_with_generator`].
pub fn compile_with_options<R>(
    entry: &std::path::Path,
    sdk_lib: Option<&std::path::Path>,
    packages: Option<&std::path::Path>,
    language: &dartforge_elements::sdk::Linguagem,
    generator: Option<Generator<'_>>,
    infer_sdk: bool,
    finish: impl FnOnce(&Analysis<'_>) -> Result<R, String>,
) -> Result<(R, Report), String> {
    use dartforge_elements::sdk::SdkLayout;
    use std::time::Instant;
    let mut rel = Report::default();
    let t = Instant::now();
    let sdk_dir = match sdk_lib {
        Some(p) => p.to_path_buf(),
        None => SdkLayout::discover().unwrap_or_else(|| std::path::PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
    };
    let mut sdk = SdkLayout::load(&sdk_dir, "dartdevc")?;
    language.aplicar(&mut sdk);
    rel.phase("SDK layout", t);
    // Cache of the analyzed SDK (`target/dartforge/sdk-<hash>.bin`); the first
    // compilation builds it. `DARTFORGE_SDK_CACHE=0` turns it off.
    let t = Instant::now();
    let cache = if std::env::var("DARTFORGE_SDK_CACHE").is_ok_and(|v| v == "0") {
        None
    } else {
        match dartforge_elements::SdkCache::abrir_ou_construir(&sdk, "dartdevc") {
            Ok((c, opened)) => {
                rel.sdk_cache = if opened { "read" } else { "built" };
                Some(c)
            }
            Err(e) => {
                eprintln!("warning: SDK cache unavailable ({e}); reading the SDK from files");
                None
            }
        }
    };
    rel.phase("SDK cache", t);
    let t = Instant::now();
    let mut interner = Interner::new();
    // In-memory generated sources. With `generator` (the build engine, wired in
    // by `compile-js` when the project uses `build_runner`), generation comes from
    // it. `DARTFORGE_GERADOS` stays for one cycle as a synonym:
    // `build_runner` = support only (what `build_runner` already wrote — this is
    // how we verify that reading from memory gives the same JS as reading from
    // disk), `ng` = the engine (the default when there is a generator), `nenhum` =
    // off.
    let mode = std::env::var("DARTFORGE_GERADOS").ok();
    let generated = match (mode.as_deref(), generator) {
        (Some("nenhum"), _) => None,
        (Some("ng"), None) => {
            return Err("DARTFORGE_GERADOS=ng requires the build engine: the project does not use build_runner".into());
        }
        (Some("ng") | None, Some(g)) => {
            // Phase 1: load the project without the generated sources, only so
            // the engine has a semantic database — this is the
            // `BuildStep.resolver` of `package:build`. Loading is lenient: the
            // missing `.template.dart` files become diagnostics and the rest of
            // the program stays standing.
            let t_phase1 = Instant::now();
            let mut names_phase1 = Interner::new();
            let (program, _) = dartforge_elements::load::load_lenient(entry, &sdk, packages, &mut names_phase1);
            rel.phase("engine: resolution load", t_phase1);
            let t_engine = Instant::now();
            let g = g(&program, &names_phase1)?;
            rel.phase("build engine", t_engine);
            Some(g)
        }
        (Some("build_runner"), _) => {
            let cfg = package_config(entry, packages);
            cfg.map(|c| {
                dartforge_elements::gerado::do_build_runner(
                    &c,
                    &[".template.dart", ".css.shim.dart"],
                    package_filter().as_ref(),
                )
            })
        }
        _ => None,
    };
    if let Some(g) = &generated {
        rel.generated_sources = g.len();
        if std::env::var("DARTFORGE_GERADOS_DEBUG").is_ok() {
            let mut v: Vec<String> = g.caminhos().map(|p| p.display().to_string()).collect();
            v.sort();
            eprintln!("generation: {} sources", v.len());
            for c in v.iter().take(3) { eprintln!("  {c}"); }
            for c in v.iter().filter(|c| c.contains("limitless")).take(3) { eprintln!("  L {c}"); }
        }
    }
    let generated_for_macros = generated.clone();
    let (program, elements_diags) = dartforge_elements::load::load_lenient_gerados(
        entry,
        &sdk,
        packages,
        &mut interner,
        cache,
        None,
        generated,
    );
    rel.phase("load program", t);
    rel.load = program.tempos.clone();
    // Parts of the SDK have URI `file:///…/lib/core/int.dart`; what decides is the library.
    rel.sdk_units = program.units.iter().filter(|u| program.library(u.library).is_sdk).count();
    rel.user_units = program.units.len() - rel.sdk_units;
    rel.libraries = program.libraries.len();
    // No phase fails silently: load diagnostics (file or package not found,
    // syntax) abort; type diagnostics are warnings for now, because inference
    // still has gaps and the emitter falls back to dynamic dispatch where the
    // type is unknown.
    if !elements_diags.is_empty() {
        for d in &elements_diags {
            eprintln!("error: {d}");
        }
        return Err(format!("{} error(s) loading the program", elements_diags.len()));
    }
    // Macros (docs/MACROS-PROTOCOLO.md): only if the program declares some
    // `macro` class — without a pending application we do not start an executor.
    // By default the Dart VM runs our API; with `DARTFORGE_MACROS_NATIVO` (a
    // `dartforge` built with the `nativo` feature) the executor is the native one
    // (D4). The host reloads the augmentation straight from memory.
    let program = if dartforge_macros_host::has_macros(&program) {
        let t = Instant::now();
        let pending = dartforge_macros_host::pending_applications(&program, &interner);
        let mut vm = if pending.is_empty() {
            None
        } else {
            let api = macros_api(entry).ok_or(
                "macros API missing: set DARTFORGE_MACROS_API to packages/macros"
            )?;
            let dart = std::env::var_os("DARTFORGE_MACROS_DART")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    let exe = if cfg!(windows) { "dart.exe" } else { "dart" };
                    let in_sdk = sdk.root.parent().unwrap_or(&sdk.root).join("bin").join(exe);
                    if in_sdk.is_file() { in_sdk } else { std::path::PathBuf::from(exe) }
                });
            let cfg = dartforge_macros_host::vm::VmConfig {
                dart,
                api,
                work_dir: entry.parent().unwrap_or(std::path::Path::new("."))
                    .join(".dart_tool/dartforge/macros"),
            };
            let package_config_path = packages.map(std::path::Path::to_path_buf)
                .or_else(|| dartforge_elements::config::PackageConfig::discover(entry));
            // Native executor (D4): the bootstrap compiled by `compile-native` of
            // the given `dartforge`, with no Dart VM.
            match std::env::var_os("DARTFORGE_MACROS_NATIVO") {
                Some(compiler) => {
                    let native = dartforge_macros_host::native::NativeConfig {
                        compiler: std::path::PathBuf::from(compiler),
                        sdk_lib: sdk.root.clone(),
                        api: cfg.api.clone(),
                        work_dir: cfg.work_dir.join("nativo"),
                    };
                    Some(dartforge_macros_host::native::start(&native, &pending, package_config_path.as_deref())?)
                }
                None => Some(dartforge_macros_host::vm::start(&cfg, &pending, package_config_path.as_deref())?),
            }
        };
        let mut unavailable = dartforge_macros_host::executor::Unavailable::default();
        let executor: &mut dyn dartforge_macros_host::executor::MacroExecutor = match vm.as_mut() {
            Some(vm) => vm,
            None => &mut unavailable,
        };
        // Incremental reload: between one application and the next only the macro
        // augmentation changes; the other units are reused.
        let mut reload = |i: &mut Interner, g, u: &mut dartforge_elements::unidades::CacheUnidades| {
            dartforge_elements::load::load_lenient_gerados(entry, &sdk, packages, i, None, Some(u), g)
        };
        match dartforge_macros_host::apply_incremental(program, &mut interner, generated_for_macros, &mut reload, executor, None) {
            Ok(s) => {
                for a in &s.warnings {
                    eprintln!("warning: {a}");
                }
                rel.phase("macros", t);
                s.program
            }
            Err(ds) => {
                for d in &ds {
                    eprintln!("error: {d}");
                }
                return Err(format!("{} error(s) in macros", ds.len()));
            }
        }
    } else {
        program
    };
    let t = Instant::now();
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    rel.phase("outline (types)", t);
    let t = Instant::now();
    let (bodies, body_diags) = if infer_sdk {
        let all: Vec<dartforge_elements::model::LibraryId> =
            (0..program.libraries.len() as u32).map(dartforge_elements::model::LibraryId).collect();
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &all)
    } else {
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline)
    };
    rel.phase("body inference", t);
    // Language errors of the 3.7–3.13 features (docs/VERSOES-LINGUAGEM.md §3)
    // and reads of locals that are not definitely assigned abort like load errors
    // (`codes::e_erro_de_compilacao`); the rest of `types` is a warning.
    let errors: Vec<&dartforge_diagnostics::Diagnostic> = outline_diags
        .iter()
        .chain(body_diags.iter())
        .filter(|d| dartforge_types::codes::e_erro_de_compilacao(&d.message))
        .collect();
    if !errors.is_empty() {
        for d in &errors {
            eprintln!("error: {d}");
        }
        return Err(format!("{} compilation error(s)", errors.len()));
    }
    rel.outline_warnings = outline_diags.len();
    rel.body_warnings = body_diags.len();
    let warnings = outline_diags.len() + body_diags.len();
    if warnings > 0 {
        let limit = std::env::var("DARTFORGE_AVISOS").ok().and_then(|v| v.parse().ok()).unwrap_or(20usize);
        for d in outline_diags.iter().chain(body_diags.iter()).take(limit) {
            eprintln!("warning: {d}");
        }
        eprintln!("({warnings} type warning(s); DARTFORGE_AVISOS=N shows more)");
        if std::env::var("DARTFORGE_AVISOS_RESUMO").is_ok() {
            // Most frequent messages (without the position).
            let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for d in outline_diags.iter().chain(body_diags.iter()) {
                *counts.entry(d.message.clone()).or_default() += 1;
            }
            let mut v: Vec<(String, usize)> = counts.into_iter().collect();
            // Ties in the count are broken by the message (the `HashMap` has no order).
            v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            for (m, n) in v.iter().take(30) {
                eprintln!("{n:6}  {m}");
            }
        }
    }
    let t = Instant::now();
    let emitted = finish(&Analysis { program: &program, interner: &interner, table: &table, core: &core, outline: &outline, bodies: &bodies })?;
    rel.phase("emission", t);
    let t = Instant::now();
    drop(bodies);
    drop(outline);
    drop(table);
    drop(program);
    drop(interner);
    rel.phase("free memory", t);
    Ok((emitted, rel))
}

/// Writes `text` to `p` only if the current content is different (timestamp left
/// intact when nothing changed: Vite/HMR does not reload). Returns whether it wrote.
fn write_if_changed(p: &std::path::Path, text: &str, compare: bool) -> Result<bool, String> {
    if !compare {
        std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        return Ok(true);
    }
    if let Ok(current) = std::fs::read(p) {
        if current == text.as_bytes() {
            return Ok(false);
        }
    }
    std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(true)
}

/// Writes the modules and `main.mjs` into `dir` and copies `dart_sdk.js`.
///
/// Returns how many files were actually written (those identical to what was
/// already on disk are not rewritten).
pub fn write(emitted: &Emitted, dir: &std::path::Path, dart_sdk_js: &std::path::Path) -> Result<usize, String> {
    write_with_options(emitted, dir, dart_sdk_js, true)
}

/// Like [`write`], optionally skipping the comparison with the file on disk.
///
/// The resident session (`dartforge dev`) already knows which modules changed,
/// from the text of the previous compilation that it keeps in memory: re-reading
/// the file to compare would cost a read of megabytes per large module.
pub fn write_with_options(
    emitted: &Emitted,
    dir: &std::path::Path,
    dart_sdk_js: &std::path::Path,
    compare: bool,
) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut written = 0usize;
    for (path, text) in &emitted.modules {
        let p = dir.join(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        written += usize::from(write_if_changed(&p, text, compare)?);
    }
    written += usize::from(write_if_changed(&dir.join("main.mjs"), &emitted.entry, true)?);
    let dest = dir.join("dart_sdk.js");
    if !dest.exists() {
        std::fs::copy(dart_sdk_js, &dest)
            .map_err(|e| format!("{} → {}: {e}", dart_sdk_js.display(), dest.display()))?;
    }
    Ok(written)
}

/// Path of the `dart_sdk.js` generated by `scripts/gerar-dart-sdk.ps1`.
pub fn default_dart_sdk_js() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("DARTFORGE_DART_SDK_JS") {
        return std::path::PathBuf::from(p);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/ddc/dart_sdk.js")
}
