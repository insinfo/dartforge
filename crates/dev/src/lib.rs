//! Resident session of `dartforge dev` (PLANO.md, "o compilador residente").
//!
//! The process does not exit on every edit: the `Interner`, the units already
//! analyzed and the per-library hashes stay alive across compilations. An
//! edit rereads the files' metadata (mtime and size), reanalyzes **only**
//! what changed and writes only the modules whose text changed.
//!
//! Memory discipline: what the session retains grows with the **project**,
//! not with the number of edits — the new unit replaces the old one in the
//! same cache slot (`crates/dev/tests/plateau.rs` watches it with
//! `live_bytes`).
pub mod generation;
pub mod hashes;

pub mod server;
pub mod ws;

use dartforge_elements::load::load_lenient_entries;
use dartforge_elements::model::{LibraryId, Program};
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::{AnalyzedUnitCache, SdkCache};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use hashes::{library_hashes, LibraryHashes};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What a compilation did, in time and in work.
#[derive(Debug, Default, Clone)]
pub struct Report {
    pub load: Duration,
    pub outline: Duration,
    pub bodies: Duration,
    pub emit: Duration,
    pub hashes: Duration,
    pub write: Duration,
    /// Units that came ready from the session cache.
    pub reused_units: usize,
    pub reanalyzed_units: usize,
    pub libraries: usize,
    /// Libraries whose content changed since the previous compilation.
    pub body_changed: Vec<String>,
    /// Libraries whose public API changed (their dependents recompile too).
    pub api_changed: Vec<String>,
    /// Dependents invalidated by an API change.
    pub invalidated_dependents: usize,
    /// Number of modules (the name is read by `crates/cli`).
    pub modules: usize,
    /// Modules whose text was actually generated in this compilation.
    pub reemitted_modules: usize,
    /// Reemitted libraries (`usize::MAX` = the whole program).
    pub reemitted_libraries: usize,
    /// Cost of building the emission context (hierarchy, groups, interop).
    pub emit_context: Duration,
    /// Modules written to disk (the name is read by `crates/cli`).
    pub written_modules: usize,
    pub type_warnings: usize,
    /// First compilation of the session (nothing reused).
    pub first: bool,
    /// Load sub-phases (read, parse, directives, outline).
    pub load_detail: dartforge_elements::model::LoadTimes,
    /// Code generation engine (`crates/build`): `None` when the project
    /// uses no builders — that is what the zero-cost gate checks.
    pub engine: Option<dartforge_build::EngineReport>,
    /// Warnings from the engine (possibly stale support, D-B5).
    pub engine_warnings: Vec<String>,
    /// Whole generation stages (engine + reload of the generated units that
    /// changed); it is included in `load`.
    pub stages: Duration,
    /// The reload caused by a new generation.
    pub generation_reload: Duration,
}

impl Report {
    pub fn total(&self) -> Duration {
        self.load + self.outline + self.bodies + self.emit + self.hashes + self.write
    }

    /// One line per phase, in the same format as `compile-js --timings`.
    pub fn text(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut s = String::new();
        let c = &self.load_detail;
        for (n, d) in [
            ("load", self.load),
            ("  parallel read+lex", c.parallel_read_lex),
            ("  serial read+lex", c.read),
            ("  parse", c.parse),
            ("  directives/URIs", c.directives),
            ("  outline: declarations", c.outline_declarations),
            ("  outline: reexports", c.outline_reexports),
            ("  outline: scopes", c.outline_scopes),
            ("  outline: supertypes", c.outline_supertypes),
            ("outline (types)", self.outline),
            ("body inference", self.bodies),
            ("  emission: context", self.emit_context),
            ("emission", self.emit),
            ("hashes", self.hashes),
            ("write", self.write),
        ] {
            s.push_str(&format!("{n:<24}{:>9.1} ms\n", ms(d)));
        }
        s.push_str(&format!("{:<24}{:>9.1} ms\n", "total", ms(self.total())));
        s.push_str(&format!(
            "units: {} reused + {} reanalyzed; {} libraries; body changed: {}; API changed: {} (+{} dependents); modules: {} ({} reemitted, {} written); warnings: {}\n",
            self.reused_units,
            self.reanalyzed_units,
            self.libraries,
            self.body_changed.len(),
            self.api_changed.len(),
            self.invalidated_dependents,
            self.modules,
            self.reemitted_modules,
            self.written_modules,
            self.type_warnings
        ));
        if let Some(m) = &self.engine {
            s.push_str(&m.text());
            s.push_str(&format!(
                "\ngeneration stages: {:.1} ms (reload of the generated units: {:.1} ms)\n",
                self.stages.as_secs_f64() * 1000.0,
                self.generation_reload.as_secs_f64() * 1000.0
            ));
        }
        for a in &self.engine_warnings {
            s.push_str(&format!("warning: {a}\n"));
        }
        s
    }
}

/// Resident session: keeps alive across compilations what did not change.
pub struct Session {
    /// Text already emitted per class and per library, reused across
    /// compilations (see `emit_modules_with_cache`).
    fragments: std::cell::RefCell<dartforge_emit_js::module::FragmentCache>,
    input: PathBuf,
    sdk: SdkLayout,
    packages: Option<PathBuf>,
    output: PathBuf,
    dart_sdk_js: PathBuf,
    /// Name arena: the `SymbolId`s of the stored trees belong to it.
    interner: Interner,
    units: AnalyzedUnitCache,
    /// `library uri` → hashes of the previous compilation.
    hashes: HashMap<String, LibraryHashes>,
    /// JS of each module of the last compilation (path → text).
    modules: HashMap<String, String>,
    /// `uri` → uris that it imports/exports (to propagate API changes).
    dependents: HashMap<String, Vec<String>>,
    /// Files invalidated since the last compilation: only the libraries that
    /// contain them need a new hash.
    dirty: std::collections::HashSet<PathBuf>,
    first: bool,
    /// Generation stages (build engine; later, macros). Empty when the
    /// project uses none: zero cost per edit.
    stages: Vec<Box<dyn generation::GenerationStage>>,
    /// Current generation of the stages (in-memory sources).
    generated: Option<std::sync::Arc<dartforge_elements::generated::Generation>>,
}

impl Session {
    /// Opens a session for an input; does not compile yet.
    ///
    /// `sdk_lib` is the SDK's `lib/` (discovered from the environment when
    /// `None`). If the project uses builders (`build_runner` in
    /// `package_config.json`), it turns on the generation engine; otherwise,
    /// nothing of it is built.
    ///
    /// # Errors
    ///
    /// When the SDK cannot be loaded.
    pub fn new(
        input: &Path,
        sdk_lib: Option<&Path>,
        packages: Option<&Path>,
        output: &Path,
    ) -> Result<Session, String> {
        let mut stages: Vec<Box<dyn generation::GenerationStage>> = Vec::new();
        match generation::build_stage(input, packages) {
            None => {}
            Some(Ok(e)) => stages.push(Box::new(e)),
            Some(Err(e)) => eprintln!("warning: build engine disabled: {e}"),
        }
        Self::with_stages(input, sdk_lib, packages, output, stages)
    }

    /// Like [`Session::new`], with the given stages (no detection).
    ///
    /// # Errors
    ///
    /// When the SDK cannot be loaded.
    pub fn with_stages(
        input: &Path,
        sdk_lib: Option<&Path>,
        packages: Option<&Path>,
        output: &Path,
        stages: Vec<Box<dyn generation::GenerationStage>>,
    ) -> Result<Session, String> {
        let dir = match sdk_lib {
            Some(p) => p.to_path_buf(),
            None => SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
        };
        let sdk = SdkLayout::load(&dir, "dartdevc")?;
        Ok(Session {
            fragments: Default::default(),
            input: input.to_path_buf(),
            sdk,
            packages: packages.map(|p| p.to_path_buf()),
            output: output.to_path_buf(),
            dart_sdk_js: dartforge_emit_js::default_dart_sdk_js(),
            interner: Interner::new(),
            units: AnalyzedUnitCache::new(),
            hashes: HashMap::new(),
            modules: HashMap::new(),
            dependents: HashMap::new(),
            dirty: std::collections::HashSet::new(),
            first: true,
            stages,
            generated: None,
        })
    }

    /// Is a generation stage (build engine) turned on?
    pub fn has_stages(&self) -> bool {
        !self.stages.is_empty()
    }

    /// Reader of the generated outputs for `serve`.
    pub fn generated_provider(&self) -> Option<generation::Provider> {
        self.stages.iter().find_map(|e| e.provider())
    }

    /// Source bytes retained by the unit cache (for the plateau test).
    pub fn retained_bytes(&self) -> usize {
        self.units.source_bytes() + self.interner.payload_bytes()
    }

    pub fn cached_units(&self) -> usize {
        self.units.len()
    }

    /// Files whose mtime or size changed since the last analysis: it is what
    /// `dartforge dev` checks at every interval (one `stat` per unit, without
    /// reading contents).
    pub fn changes(&mut self) -> Vec<PathBuf> {
        let mut v = self.units.changed();
        for e in &mut self.stages {
            v.extend(e.changes());
        }
        v
    }

    /// Files watched by the session.
    pub fn watched(&self) -> Vec<PathBuf> {
        let mut v = self.units.paths();
        for e in &self.stages {
            v.extend(e.watched());
        }
        v
    }

    /// Notifies that a file changed: the corresponding unit leaves the cache.
    ///
    /// It is not mandatory — the stamp (mtime and size) already detects the
    /// change in the next compilation —, but it saves a useless `stat` and
    /// covers the case of an edit with the same size within the clock
    /// granularity.
    pub fn file_changed(&mut self, path: &Path) {
        let p = dartforge_elements::config::without_verbatim(
            std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        );
        self.units.invalidate(&p);
        self.units.invalidate(path);
        self.dirty.insert(p);
        self.dirty.insert(path.to_path_buf());
    }

    /// Compiles (or recompiles) and writes what changed.
    ///
    /// # Errors
    ///
    /// Load diagnostics, mandatory language or definite-assignment errors, a
    /// failure of a generation stage, an emission error or a write error.
    /// Mandatory errors leave the last successful emitted output on disk.
    pub fn compile(&mut self) -> Result<Report, String> {
        let result = self.compile_attempt();
        if result.is_err() {
            // A failed attempt may have updated hashes, fragments or module text
            // before reporting its error. None of that is a successful revision.
            self.hashes.clear();
            self.dependents.clear();
            self.modules.clear();
            self.fragments.borrow_mut().clear();
        }
        result
    }

    /// Performs one attempt; compile invalidates reusable state on any error.
    fn compile_attempt(&mut self) -> Result<Report, String> {
        let mut rel = Report { first: self.first, ..Default::default() };

        let t = Instant::now();
        // The SDK comes from the artifact in `target/dartforge` on the first
        // compilation; afterwards its units are already in the session cache.
        let sdk_cache = if self.units.is_empty() {
            SdkCache::open_or_build(&self.sdk, "dartdevc").ok().map(|(c, _)| c)
        } else {
            None
        };
        // What changed since the previous compilation, for the generation
        // stages.
        let changed: Vec<PathBuf> =
            if self.stages.is_empty() { Vec::new() } else { self.dirty.iter().cloned().collect() };
        // The input and, with the engine, the ngdart inputs: a component
        // reachable only from its own `.template.dart` (which does not exist
        // yet) also enters the program, and the generator sees it.
        let roots: Vec<PathBuf> = std::iter::once(self.input.clone())
            .chain(self.stages.iter().flat_map(|e| e.roots()))
            .collect();
        let roots: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
        let (mut program, mut diags) = load_lenient_entries(
            &roots,
            &self.sdk,
            self.packages.as_deref(),
            &mut self.interner,
            sdk_cache,
            Some(&mut self.units),
            self.generated.clone(),
        );
        let mut read = program.times.files_read;
        if !self.stages.is_empty() {
            let t_stages = Instant::now();
            // First pass: an expected output that has not been generated yet
            // is not an error — the stage generates it now. The prefix is the
            // message of `dartforge_elements` for an unreadable file.
            let stages = &self.stages;
            diags.retain(|d| {
                let Some(rest) = d.message.strip_prefix("could not read ") else { return true };
                let path = rest.rsplit_once(": ").map(|(c, _)| c).unwrap_or(rest);
                !stages.iter().any(|e| e.expects(Path::new(path)))
            });
            let mut altered: Vec<PathBuf> = Vec::new();
            let mut new_generation = None;
            for stage in &mut self.stages {
                let ctx = generation::SessionCtx { program: &program, interner: &self.interner };
                match stage.update(&ctx, &changed) {
                    Ok(up) => {
                        altered.extend(up.changed);
                        rel.engine_warnings.extend(up.warnings);
                        rel.engine = Some(up.report);
                        new_generation = Some(up.generation);
                    }
                    Err(e) => {
                        self.units.reclaim(program);
                        return Err(format!("build engine: {e}"));
                    }
                }
            }
            let generation_changed = new_generation.as_ref().map(|g| g.id) != self.generated.as_ref().map(|g| g.id);
            let t_reload = Instant::now();
            if generation_changed || !altered.is_empty() {
                self.generated = new_generation;
                // The units go back to the cache and only the generated ones
                // whose text changed leave it: the reload reanalyzes those and
                // reuses the rest.
                self.units.reclaim(program);
                for p in &altered {
                    self.units.invalidate(p);
                    self.dirty.insert(p.clone());
                }
                let (p2, d2) = load_lenient_entries(
                    &roots,
                    &self.sdk,
                    self.packages.as_deref(),
                    &mut self.interner,
                    None,
                    Some(&mut self.units),
                    self.generated.clone(),
                );
                program = p2;
                diags = d2;
                read += program.times.files_read;
            }
            rel.generation_reload = t_reload.elapsed();
            rel.stages = t_stages.elapsed();
        }
        rel.load = t.elapsed();
        rel.load_detail = program.times.clone();
        rel.reused_units = program.times.reused_units;
        // Reanalyzed = files the load actually had to read and analyze (the
        // `Program` may have units that came from the SDK artifact, which
        // are neither one thing nor the other).
        rel.reanalyzed_units = read;
        rel.libraries = program.libraries.len();
        if !diags.is_empty() {
            let msg = format!("{} error(s) while loading: {}", diags.len(), diags[0]);
            // A failed load must not leave the cache with half the units.
            self.units.reclaim(program);
            return Err(msg);
        }

        let t = Instant::now();
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &program, &self.interner);
        let (mut outline, outline_diags) =
            dartforge_types::resolve_outline(&program, &self.interner, &mut table, &core);
        rel.outline = t.elapsed();

        // The hashes come before body inference: they are what says what
        // needs to be reanalyzed and reemitted.
        let t = Instant::now();
        let affected = self.compare_hashes(&program, &mut rel);
        rel.hashes = t.elapsed();

        let t = Instant::now();
        let (bodies, body_diags) = match &affected {
            None => dartforge_types::infer_program_bodies(&program, &self.interner, &mut table, &core, &mut outline),
            Some(libs) => dartforge_types::infer_library_bodies(
                &program, &self.interner, &mut table, &core, &mut outline, libs,
            ),
        };
        rel.bodies = t.elapsed();
        let mut compile_errors = outline_diags.iter().chain(body_diags.iter())
            .filter(|diagnostic| dartforge_types::codes::is_compile_error(&diagnostic.message));
        if let Some(first_error) = compile_errors.next() {
            let message = format!("{} compilation error(s): {first_error}", 1 + compile_errors.count());
            self.units.reclaim(program);
            return Err(message);
        }
        rel.type_warnings = outline_diags.len() + body_diags.len();

        let t = Instant::now();
        let mut emitted = match affected {
            // First compilation (or a graph change): emit everything.
            None => {
                self.fragments.borrow_mut().clear();
                let e = dartforge_emit_js::emit_program_with_cache(&program, &self.interner, &table, &core, &outline, &bodies, Some(&self.fragments))
                    .map_err(|ds| ds.first().map(|d| d.to_string()).unwrap_or_else(|| "emission failed".into()))?;
                rel.reemitted_libraries = usize::MAX;
                e
            }
            Some(libs) => {
                rel.reemitted_libraries = libs.len();
                // The fragments of the changed libraries leave the cache; the
                // rest of the module is reused as text.
                {
                    let ctx_tmp = dartforge_emit_js::ctx::Ctx::new(&program, &self.interner, &table, &core, &outline, &bodies);
                    self.fragments.borrow_mut().invalidate(&ctx_tmp, &libs);
                }
                let (e, context) = dartforge_emit_js::emit_modules_with_cache(
                    &program, &self.interner, &table, &core, &outline, &bodies, &libs, Some(&self.fragments),
                )
                .map_err(|ds| ds.first().map(|d| d.to_string()).unwrap_or_else(|| "emission failed".into()))?;
                rel.emit_context = context;
                e
            }
        };
        rel.reemitted_modules = emitted.modules.len();
        // The text of the modules that were not reemitted still holds from the
        // previous compilation. Of the reemitted ones, only those whose text
        // changed go to disk — the comparison is with what the session has in
        // memory, without reading the file (the module of a large package has
        // megabytes).
        let previous = std::mem::take(&mut self.modules);
        let mut changed_modules = Vec::new();
        for (path, text) in emitted.modules.drain(..) {
            let same = previous.get(&path).is_some_and(|t| *t == text);
            if !same {
                changed_modules.push((path.clone(), text.clone()));
            }
            self.modules.insert(path, text);
        }
        for (path, text) in previous {
            self.modules.entry(path).or_insert(text);
        }
        emitted.modules = changed_modules;
        rel.emit = t.elapsed();
        rel.modules = self.modules.len();

        // Write: only the reemitted modules can have text different from what
        // is on disk, so only they go to `write` (which still compares byte
        // for byte before writing). Comparing the 352 modules of `new_sali` on
        // every edit cost one read per file.
        let t = Instant::now();
        // On the first compilation the directory may already hold output of
        // another run: then it is worth comparing. On the following ones, the
        // session already knows the text changed (it compared it with the
        // previous compilation's, in memory).
        let compare = rel.first;
        rel.written_modules =
            dartforge_emit_js::write_with_options(&emitted, &self.output, &self.dart_sdk_js, compare)?;
        rel.write = t.elapsed();

        // The units go back to the cache; the rest of the compilation dies here.
        drop(bodies);
        drop(outline);
        drop(table);
        self.units.reclaim(program);
        self.first = false;
        Ok(rel)
    }

    /// Recomputes the two hashes of each user library, fills in the report
    /// what changed and returns **which libraries need to be reemitted**:
    /// those whose content changed and, through an API change, their
    /// dependents. `None` means "emit everything" — first compilation or a
    /// new/deleted library, where the previous set is of no use.
    fn compare_hashes(&mut self, program: &Program, rel: &mut Report) -> Option<Vec<LibraryId>> {
        let first = self.hashes.is_empty();
        let mut ids: HashMap<String, LibraryId> = HashMap::with_capacity(program.libraries.len());
        let mut new_hashes: HashMap<String, LibraryHashes> = HashMap::with_capacity(program.libraries.len());
        let mut api_changed: Vec<String> = Vec::new();
        let mut content_changed: Vec<String> = Vec::new();
        let mut added_or_lost = false;
        for i in 0..program.libraries.len() {
            let lib = LibraryId(i as u32);
            let l = program.library(lib);
            if l.is_sdk || l.units.is_empty() {
                continue;
            }
            // Only the libraries with some dirty file need a new hash: the
            // rest has the same trees as the previous compilation. (Rehashing
            // everything cost 26 ms per edit on `new_sali/core`.)
            let dirty = first
                || l.units.iter().any(|&u| {
                    program.unit(u).path.as_ref().is_some_and(|p| self.dirty.contains(p))
                });
            if !dirty
                && let Some(&old) = self.hashes.get(&l.uri) {
                    ids.insert(l.uri.clone(), lib);
                    new_hashes.insert(l.uri.clone(), old);
                    continue;
                }
            let h = library_hashes(program, lib);
            match self.hashes.get(&l.uri) {
                Some(old) => {
                    if old.content != h.content {
                        rel.body_changed.push(l.uri.clone());
                        content_changed.push(l.uri.clone());
                    }
                    if old.api != h.api {
                        api_changed.push(l.uri.clone());
                    }
                }
                // A library that did not exist in the previous compilation.
                None if !first => added_or_lost = true,
                None => {}
            }
            ids.insert(l.uri.clone(), lib);
            new_hashes.insert(l.uri.clone(), h);
        }
        if new_hashes.len() != self.hashes.len() && !first {
            added_or_lost = true;
        }
        // Reverse graph: who imports/exports each library.
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..program.libraries.len() {
            let l = &program.libraries[i];
            if l.is_sdk {
                continue;
            }
            for target in l.imports.iter().map(|x| x.library).chain(l.exports.iter().map(|x| x.library)) {
                let target_uri = &program.libraries[target.0 as usize].uri;
                if program.libraries[target.0 as usize].is_sdk {
                    continue;
                }
                let e = dependents.entry(target_uri.clone()).or_default();
                if !e.contains(&l.uri) {
                    e.push(l.uri.clone());
                }
            }
        }
        // Transitive closure of the dependents of the libraries whose API changed.
        let mut invalidated: Vec<String> = Vec::new();
        let mut queue: Vec<String> = api_changed.clone();
        while let Some(u) = queue.pop() {
            for d in dependents.get(&u).into_iter().flatten() {
                if !invalidated.contains(d) && !api_changed.contains(d) {
                    invalidated.push(d.clone());
                    queue.push(d.clone());
                }
            }
        }
        rel.invalidated_dependents = invalidated.len();
        self.hashes = new_hashes;
        self.dirty.clear();
        self.dependents = dependents;

        // Who needs to be reemitted: whoever changed content (the JS comes
        // from the body) and whoever depends on a changed API. On the first
        // compilation, or when the set of libraries changed, emit everything.
        let decision = if first || added_or_lost || self.modules.is_empty() {
            None
        } else {
            let mut libs: Vec<LibraryId> = Vec::new();
            for uri in content_changed.iter().chain(api_changed.iter()).chain(invalidated.iter()) {
                if let Some(&id) = ids.get(uri)
                    && !libs.contains(&id) {
                        libs.push(id);
                    }
            }
            Some(libs)
        };
        rel.api_changed = api_changed;
        decision
    }
}
